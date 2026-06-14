//! `mmz` is a memoized command runner.
//!
//! Prefix any command with `mmz`. A `mmz.yaml` manifest declares named input
//! scopes and command rules that reference them. When the invoked command
//! matches a rule and that rule's inputs are byte-for-byte unchanged since the
//! command last succeeded, `mmz` skips execution and exits 0; otherwise it runs
//! the command, streams its output, and records the new state on success.
//!
//! The cache identity is the matched rule, so the operator controls
//! granularity through how specifically rules are written. State lives in a
//! gitignored `.mmz/` directory and is throwaway.
//!
//! Modules: the manifest ([`manifest`]), pattern resolution ([`resolve`]),
//! content hashing ([`hashing`]), rule matching ([`matcher`]), the cache
//! ([`cache`]), and the orchestration engine ([`engine`]). The `mmz --…`
//! actions live in [`init`], [`schema`], and [`status`].

pub mod cache;
pub mod engine;
pub mod error;
pub mod hashing;
pub mod init;
pub mod manifest;
pub mod matcher;
pub mod resolve;
pub mod schema;
pub mod status;

pub use engine::run;
pub use error::{Error, Result};
