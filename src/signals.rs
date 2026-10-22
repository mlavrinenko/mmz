//! Signals mmz receives while the wrapped command runs.
//!
//! mmz and the command share a process group, so Ctrl-C reaches both: mmz
//! catches SIGINT only to wait for the command instead of dying first. SIGTERM
//! and SIGHUP are often aimed at mmz alone (a `timeout`, a CI cancel, a
//! `kill`), so mmz passes each one on to the command's pid. Either way mmz
//! waits for the command and reports that a signal arrived, and the engine
//! records no pass for that run, even when the command's trap exits 0.
//!
//! A signal mmz inherited ignored, as under `nohup`, stays ignored for mmz and
//! the command alike. On Linux `/proc/self/status` says which those are, so
//! the guard goes in before the command is spawned and no signal falls between
//! the two. The command still inherits the dispositions as they were: exec
//! resets a caught signal to its default, and an ignored one gets no handler.
//! No safe API reads them elsewhere, so there the command is spawned first, to
//! inherit an ignore untouched, and mmz catches all three; a signal in that gap
//! kills mmz as it did before.
//!
//! std spawns through `posix_spawn` where libc has it, which keeps the handlers
//! from running in the child before exec. On a fork path, a signal that lands
//! before exec runs the handler in the child's own memory, so mmz at most wakes
//! once for nothing.
//!
//! signal-hook cannot uninstall its handler. Instead, each signal that had its
//! default action gets one fallback for the life of the process, from
//! `signal_hook::flag::register_conditional_default`, which runs that action
//! whenever no command is running.
//!
//! Windows has no SIGTERM or SIGHUP to pass on, and keeps a plain wait.

use std::io;
use std::process::{Command, ExitStatus};

/// A finished command: its status, and whether a signal reached mmz while it
/// ran.
pub struct Ran {
    pub status: ExitStatus,
    pub interrupted: bool,
}

/// Spawns `command` and waits for it, passing SIGTERM and SIGHUP on to it.
///
/// # Errors
///
/// Returns the I/O error from spawning or waiting for the command.
#[cfg(unix)]
pub fn run(command: &mut Command) -> io::Result<Ran> {
    unix::run(command)
}

/// Spawns `command` and waits for it.
///
/// # Errors
///
/// Returns the I/O error from spawning or waiting for the command.
#[cfg(not(unix))]
pub fn run(command: &mut Command) -> io::Result<Ran> {
    Ok(Ran {
        status: command.status()?,
        interrupted: false,
    })
}

#[cfg(unix)]
mod unix {
    use std::ffi::c_int;
    use std::io;
    use std::process::{Child, Command};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, LazyLock, Mutex, MutexGuard, PoisonError};

    use rustix::process::{Pid, Signal, kill_process};
    use signal_hook::consts::{SIGCHLD, SIGHUP, SIGINT, SIGTERM};
    use signal_hook::flag;
    use signal_hook::iterator::Signals;

    use super::Ran;

    /// The signals mmz catches while the command runs.
    const CAUGHT: [c_int; 3] = [SIGINT, SIGTERM, SIGHUP];

    pub fn run(command: &mut Command) -> io::Result<Ran> {
        let (guard, mut child) = if let Some(status) = proc_status() {
            let guard = Guard::install(&status);
            (guard, command.spawn()?)
        } else {
            let child = command.spawn()?;
            (Guard::install(""), child)
        };
        match guard {
            Ok(mut guard) => guard.wait(&mut child),
            Err(err) => {
                log::warn!("mmz: cannot catch signals ({err}); waiting without passing them on");
                Ok(Ran {
                    status: child.wait()?,
                    interrupted: false,
                })
            }
        }
    }

    /// What a signal did when the command started.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Disposition {
        Default,
        Ignored,
        Caught,
    }

    /// mmz's `/proc/self/status`, or `None` when it lists no signal masks.
    #[cfg(target_os = "linux")]
    fn proc_status() -> Option<String> {
        std::fs::read_to_string("/proc/self/status")
            .ok()
            .filter(|status| status.contains("\nSigIgn:"))
    }

    /// No safe API reads the dispositions here.
    #[cfg(not(target_os = "linux"))]
    fn proc_status() -> Option<String> {
        None
    }

    /// Reads `signal`'s disposition off a `/proc/<pid>/status` text.
    fn disposition(status: &str, signal: c_int) -> Disposition {
        if in_mask(status, "SigIgn", signal) {
            Disposition::Ignored
        } else if in_mask(status, "SigCgt", signal) {
            Disposition::Caught
        } else {
            Disposition::Default
        }
    }

    /// Whether the hex mask `field` sets `signal`'s bit, `1 << (signal - 1)`.
    fn in_mask(status: &str, field: &str, signal: c_int) -> bool {
        let Some(bit) = u32::try_from(signal).ok().and_then(|n| n.checked_sub(1)) else {
            return false;
        };
        status
            .lines()
            .find_map(|line| line.strip_prefix(field)?.strip_prefix(':'))
            .and_then(|hex| u64::from_str_radix(hex.trim(), 16).ok())
            .and_then(|mask| mask.checked_shr(bit))
            .is_some_and(|mask| mask & 1 == 1)
    }

    /// Set while no command runs, and read by every default-action fallback.
    static IDLE: LazyLock<Arc<AtomicBool>> = LazyLock::new(|| Arc::new(AtomicBool::new(true)));

    /// Process-wide, as signal-hook's handlers are: a fallback stays registered
    /// for good, so a second run must neither stack another nor be killed by
    /// the first one's.
    static SHARED: Mutex<Shared> = Mutex::new(Shared {
        live: 0,
        fallbacks: Vec::new(),
    });

    /// How many guards are live, and which signals have their fallback.
    struct Shared {
        live: usize,
        fallbacks: Vec<c_int>,
    }

    fn shared() -> MutexGuard<'static, Shared> {
        SHARED.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// One live guard's count in [`SHARED`]. The last to drop sets [`IDLE`].
    struct Lease;

    impl Lease {
        /// Counts a live guard, then gives each of `defaults` its fallback
        /// unless an earlier guard did.
        fn take(defaults: &[c_int]) -> io::Result<Self> {
            let lease = {
                let mut state = shared();
                state.live += 1;
                IDLE.store(false, Ordering::SeqCst);
                Self
            };
            let mut state = shared();
            for &signal in defaults {
                if !state.fallbacks.contains(&signal) {
                    flag::register_conditional_default(signal, Arc::clone(&IDLE))?;
                    state.fallbacks.push(signal);
                }
            }
            drop(state);
            Ok(lease)
        }
    }

    impl Drop for Lease {
        fn drop(&mut self) {
            let mut state = shared();
            state.live = state.live.saturating_sub(1);
            if state.live == 0 {
                IDLE.store(true, Ordering::SeqCst);
            }
        }
    }

    /// Catches [`CAUGHT`] while the command runs, plus SIGCHLD to wake when it
    /// exits.
    struct Guard {
        /// Declared first, so it drops first: the default actions come back
        /// before the catch goes, and no signal falls between the two.
        _lease: Lease,
        signals: Signals,
    }

    impl Guard {
        /// Catches every signal in [`CAUGHT`] that `status` does not show
        /// ignored. One that had its default action gets that action back once
        /// no command runs; one another handler caught keeps that handler,
        /// which signal-hook runs before its own.
        fn install(status: &str) -> io::Result<Self> {
            let mut caught = Vec::new();
            let mut defaults = Vec::new();
            for signal in CAUGHT {
                match disposition(status, signal) {
                    Disposition::Ignored => {}
                    Disposition::Caught => caught.push(signal),
                    Disposition::Default => {
                        caught.push(signal);
                        defaults.push(signal);
                    }
                }
            }
            let signals = Signals::new(caught.into_iter().chain([SIGCHLD]))?;
            Ok(Self {
                _lease: Lease::take(&defaults)?,
                signals,
            })
        }

        /// Waits for `child`, passing each caught signal on as it arrives.
        ///
        /// Only this thread reaps the child, so a pid it signals is still the
        /// command's, a zombie at worst. A signal that lands as the command
        /// exits still counts, but goes nowhere: the pid is reaped by then and
        /// may name another process.
        fn wait(&mut self, child: &mut Child) -> io::Result<Ran> {
            let pid = Pid::from_child(child);
            let mut interrupted = false;
            let status = loop {
                if let Some(status) = child.try_wait()? {
                    break status;
                }
                for signal in self.signals.wait() {
                    interrupted |= relay(signal, pid);
                }
            };
            interrupted |= self.signals.pending().any(|signal| signal != SIGCHLD);
            Ok(Ran {
                status,
                interrupted,
            })
        }
    }

    /// Passes SIGTERM or SIGHUP on to the command; SIGINT reached it already,
    /// through the process group. Returns whether `signal` is one of
    /// [`CAUGHT`], not the SIGCHLD that only wakes the loop.
    fn relay(signal: c_int, pid: Pid) -> bool {
        let onward = match signal {
            SIGTERM => Signal::TERM,
            SIGHUP => Signal::HUP,
            SIGINT => return true,
            _ => return false,
        };
        if let Err(err) = kill_process(pid, onward) {
            log::warn!("mmz: cannot pass signal {signal} on to the command: {err}");
        }
        true
    }

    #[cfg(test)]
    mod tests {
        use super::{Disposition, SIGHUP, SIGINT, SIGTERM, disposition};

        /// SIGHUP (bit 0) ignored, SIGINT (bit 1) caught, SIGTERM (bit 14)
        /// left at its default, as `/proc/self/status` prints the masks.
        const STATUS: &str = "Name:\tmmz\nSigBlk:\t0000000000000000\n\
            SigIgn:\t0000000000000001\nSigCgt:\t0000000000000002\n";

        #[test]
        fn reads_each_signal_s_disposition_off_its_bit() {
            assert_eq!(disposition(STATUS, SIGHUP), Disposition::Ignored);
            assert_eq!(disposition(STATUS, SIGINT), Disposition::Caught);
            assert_eq!(disposition(STATUS, SIGTERM), Disposition::Default);
        }

        #[test]
        fn the_top_bit_of_the_mask_is_the_highest_signal() {
            let status = "SigIgn:\t8000000000000000\n";
            assert_eq!(disposition(status, 64), Disposition::Ignored);
            assert_eq!(disposition(status, 63), Disposition::Default);
        }

        #[test]
        fn an_unreadable_status_reads_every_signal_as_default() {
            for status in ["", "SigIgn:\tnot-hex\nSigCgt:\n", "SigIgnore:\t1\n"] {
                assert_eq!(
                    disposition(status, SIGHUP),
                    Disposition::Default,
                    "{status:?}"
                );
            }
        }

        #[test]
        fn a_signal_number_off_the_mask_is_default() {
            let status = "SigIgn:\tffffffffffffffff\n";
            assert_eq!(disposition(status, 0), Disposition::Default);
            assert_eq!(disposition(status, 65), Disposition::Default);
            assert_eq!(disposition(status, -1), Disposition::Default);
        }
    }
}
