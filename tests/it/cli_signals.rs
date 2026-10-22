//! Signals sent to mmz while it runs a command: SIGTERM and SIGHUP reach the
//! command, Ctrl-C reaches it through the process group, mmz waits for it
//! either way, and a run a signal interrupted records no pass. A signal the
//! caller ignores, as `nohup` does, stays ignored.
//!
//! Each test syncs on a file the command writes once its trap is set, never on
//! a fixed sleep. That sync holds only where mmz catches signals before it
//! spawns the command, which is Linux; elsewhere a signal can still land
//! between the two, so the tests that send one to mmz run on Linux alone.
#![cfg(unix)]
#![cfg_attr(not(target_os = "linux"), allow(dead_code, unused_imports))]

use std::fs;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use predicates::prelude::predicate;
use rustix::process::{Pid, Signal, kill_process, kill_process_group};

use crate::support::{mmz, write_manifest};

/// Longest the command may take to reach its next step before the test calls
/// it hung rather than slow.
const PATIENCE: Duration = Duration::from_secs(10);

/// Traps each signal into `got` and exits 0, so only mmz's record can tell an
/// interrupted run from a finished one. Untrapped, it writes `done` instead.
const SLOW: &str = "trap 'echo got > got; kill $! 2>/dev/null; exit 0' HUP INT TERM
sleep 30 &
echo > ready
wait
echo finished > done
";

/// Dies by its own SIGHUP unless it inherited the signal ignored, then holds
/// until the test writes `go`, or for 30 seconds at most, so a test that died
/// leaves no orphan behind.
const NOHUP: &str = "kill -HUP $$
echo alive > alive
echo > ready
i=0
while [ ! -e go ] && [ $i -lt 3000 ]; do sleep 0.01; i=$((i+1)); done
";

/// A `sh` rule over `*.sh`, plus `script.sh` holding `body`.
fn write_project(dir: &Path, body: &str) {
    write_manifest(
        dir,
        "scopes:\n  src: [\"*.sh\"]\ncommands:\n  - name: sh\n    inputs: [src]\n",
    );
    fs::write(dir.join("script.sh"), body).expect("write script");
}

/// `mmz sh script.sh` in the background, in a process group of its own so a
/// group-wide signal stays away from the test runner.
fn spawn_mmz(dir: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_mmz"))
        .args(["sh", "script.sh"])
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0)
        .spawn()
        .expect("spawn mmz")
}

fn wait_for(path: &Path) {
    let deadline = Instant::now() + PATIENCE;
    while !path.exists() {
        assert!(
            Instant::now() < deadline,
            "{} never appeared",
            path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

/// Asserts the run recorded no pass: the next `--is-fresh` calls it failed.
fn assert_records_failure(dir: &Path) {
    mmz(dir)
        .arg("--is-fresh")
        .assert()
        .code(1)
        .stderr(predicate::str::contains("`sh` is failed"));
}

fn assert_waited_for_the_command(status: ExitStatus, signal: &str) {
    assert_eq!(
        status.code(),
        Some(0),
        "after {signal}, mmz waits for the command and exits with its status, got {status}"
    );
}

#[test]
#[cfg(target_os = "linux")]
fn sigterm_and_sighup_reach_the_command_and_record_no_pass() {
    for (name, signal) in [("SIGTERM", Signal::TERM), ("SIGHUP", Signal::HUP)] {
        let dir = tempfile::tempdir().expect("tempdir");
        write_project(dir.path(), SLOW);
        let mut child = spawn_mmz(dir.path());
        wait_for(&dir.path().join("ready"));

        kill_process(Pid::from_child(&child), signal).expect("signal mmz");
        let status = child.wait().expect("wait for mmz");

        assert!(
            dir.path().join("got").exists(),
            "the command saw the {name} sent to mmz"
        );
        assert_waited_for_the_command(status, name);
        assert!(
            !dir.path().join("done").exists(),
            "{name} cut the command short"
        );
        assert_records_failure(dir.path());
    }
}

#[test]
#[cfg(target_os = "linux")]
fn ctrl_c_reaches_the_command_through_the_group_and_mmz_waits_for_it() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), SLOW);
    let mut child = spawn_mmz(dir.path());
    wait_for(&dir.path().join("ready"));

    kill_process_group(Pid::from_child(&child), Signal::INT).expect("signal the group");
    let status = child.wait().expect("wait for mmz");

    assert_waited_for_the_command(status, "SIGINT");
    assert!(
        dir.path().join("got").exists(),
        "the command saw the SIGINT"
    );
    assert_records_failure(dir.path());
}

#[test]
fn a_sighup_ignored_by_the_caller_stays_ignored_for_mmz_and_the_command() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), NOHUP);
    let mut child = Command::new("sh")
        .args(["-c", "trap '' HUP; exec \"$0\" sh script.sh"])
        .arg(env!("CARGO_BIN_EXE_mmz"))
        .current_dir(dir.path())
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn mmz with SIGHUP ignored");
    wait_for(&dir.path().join("ready"));

    kill_process(Pid::from_child(&child), Signal::HUP).expect("signal mmz");
    fs::write(dir.path().join("go"), b"").expect("release the command");
    let status = child.wait().expect("wait for mmz");

    assert!(
        dir.path().join("alive").exists(),
        "the command inherited SIGHUP ignored"
    );
    assert_eq!(
        status.code(),
        Some(0),
        "mmz ignored the SIGHUP, got {status}"
    );
    // Only Linux lets mmz read which signals it inherited ignored, so only
    // there does it know to leave SIGHUP alone rather than catch it.
    if cfg!(target_os = "linux") {
        mmz(dir.path()).arg("--is-fresh").assert().success();
    }
}

/// Signals its caller, the test process, which must pass the TERM back to it
/// instead of dying by it.
const SIGNALS_ITS_CALLER: &str = "trap 'echo got > got; kill $! 2>/dev/null; exit 0' TERM
kill -TERM $PPID
sleep 30 &
wait
";

#[test]
#[cfg(target_os = "linux")]
fn a_second_run_in_one_process_still_passes_sigterm_on() {
    let dir = tempfile::tempdir().expect("tempdir");
    write_project(dir.path(), SIGNALS_ITS_CALLER);
    let argv = |args: &[&str]| args.iter().map(|&arg| arg.to_owned()).collect::<Vec<_>>();

    let first = mmz::run(&argv(&["sh", "-c", "exit 1"]), dir.path()).expect("first run");
    assert_eq!(
        first, 1,
        "the first run records a failure, so the second runs"
    );
    let second = mmz::run(&argv(&["sh", "script.sh"]), dir.path()).expect("second run");

    assert_eq!(second, 0, "the command's trap exits 0");
    assert!(
        dir.path().join("got").exists(),
        "the TERM came back to the command"
    );
}
