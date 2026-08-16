//! `mmz` is a memoized command runner.
//!
//! Prefix any command with `mmz`. A `.mmz/config.yaml` manifest declares named input
//! scopes and command rules that reference them. When the invoked command
//! matches a rule and that rule's inputs are byte-for-byte unchanged since the
//! command last succeeded, `mmz` skips execution and exits 0; otherwise it runs
//! the command, streams its output, and records the new state on success. A
//! rule may also declare `outputs` — literal artifact paths whose absence voids
//! the record, however the inputs hash (see [`outputs`]) — and draw an input
//! from a named command's stdout rather than a file (see [`probe`]).
//!
//! The cache identity is the matched rule, so the operator controls
//! granularity through how specifically rules are written. State lives in a
//! gitignored cache directory (`.mmz/cache` by default) and is throwaway.
//!
//! # Use as a library
//!
//! The binary is a thin wrapper over this crate; the same entry points are
//! public. [`run`] memoizes one invocation against the nearest manifest,
//! returning the exit code to propagate:
//!
//! ```no_run
//! use std::path::Path;
//!
//! let argv = vec!["cargo".to_owned(), "test".to_owned()];
//! let exit_code: u8 = mmz::run(&argv, Path::new("."))?;
//! # let _ = exit_code;
//! # Ok::<(), mmz::Error>(())
//! ```
//!
//! [`status::report_json`] renders the freshness report, [`freshness::evaluate`]
//! gates a rule's freshness without running it, [`prune::prune`] sweeps orphaned
//! records, and [`Manifest`] loads and validates a `.mmz/config.yaml` for callers
//! that want the parsed model directly.
//!
//! Modules: the manifest ([`manifest`]), pattern resolution ([`resolve`]),
//! content hashing ([`hashing`]), command-driven inputs ([`probe`]), declared
//! artifact outputs ([`outputs`]), rule matching ([`matcher`]), glob-fanned
//! parametric rules ([`parametric`]), the cache ([`cache`]), cache-hit notices
//! ([`notice`]), and the orchestration engine ([`engine`]). The `mmz --…`
//! actions live in [`init`], [`schema`], [`status`], [`freshness`], and
//! [`prune`].

pub mod cache;
pub mod engine;
pub mod error;
pub mod freshness;
pub mod hashing;
pub mod init;
pub mod manifest;
pub mod matcher;
pub mod notice;
pub mod outputs;
pub mod parametric;
pub mod probe;
pub mod prune;
pub mod resolve;
pub mod schema;
pub mod status;

pub use engine::run;
pub use error::{Error, Result};
pub use manifest::Manifest;
