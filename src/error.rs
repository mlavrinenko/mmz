//! Error types for the `mmz` library.

use std::path::PathBuf;

use thiserror::Error;

/// Errors produced while loading the manifest, resolving inputs, hashing
/// files, or spawning the wrapped command.
///
/// Every path a manifest-loading error names is rendered the way a report
/// renders one — relative to the project root when it sits under it, absolute
/// otherwise (see [`crate::provenance::Provenance::display`]). So an error and
/// `--status` name the same file identically, and a fragment outside the tree
/// (a store path, the case composition exists to support) stays absolute,
/// which is the unambiguous form for it.
#[derive(Debug, Error)]
pub enum Error {
    /// An I/O operation failed somewhere no more specific variant covers:
    /// reading the root manifest, writing `--init`'s template, sweeping the
    /// cache. Reading a rule's declared inputs is deliberately not one of them
    /// — that is [`Error::InputVanished`] or [`Error::InputUnreadable`].
    #[error("i/o error: {0}")]
    Io(#[from] std::io::Error),

    /// An input a rule declared was resolved by the walk but was gone by the
    /// time the hasher opened it.
    ///
    /// Split from [`Error::Io`] for the two facts that message drops: which
    /// file, and that this is a condition of the tree rather than a bug in mmz
    /// — so it exits 8 rather than 70. Every resolve-then-hash pass has that
    /// window by construction, and mmz's own use (gating a parallel runner,
    /// where sibling arms rewrite the tree the walk just listed) sits in front
    /// of it.
    #[error(
        "input `{path}` disappeared after it was resolved; something removed it while mmz was hashing, and no cache record was written — so re-running once the tree has settled is safe"
    )]
    InputVanished {
        /// The input, rendered as a report renders one.
        path: PathBuf,
    },

    /// An input a rule declared could not be read for any reason other than
    /// being gone: permissions, a device error, a path that stopped being a
    /// regular file.
    ///
    /// Named separately from [`Error::Io`] for the same reason
    /// [`Error::ProbeFileUnreadable`] is — a bare "permission denied" leaves a
    /// reader hunting for which file it was about.
    #[error("input `{path}` could not be read; no cache record was written\n  {source}")]
    InputUnreadable {
        /// The input, rendered as a report renders one.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// No manifest was found searching upward from the working directory.
    #[error("no .mmz/config.yaml found in `{start}` or any parent; create one with `mmz --init`")]
    NoManifest {
        /// Directory the upward search started from.
        start: PathBuf,
    },

    /// No rule matched the invoked command and `no_match` strictness is on.
    #[error(
        "no rule matches `{command}`; add a matching rule to .mmz/config.yaml or relax `strict`"
    )]
    NoMatch {
        /// The invoked command, joined for display.
        command: String,
    },

    /// A matched rule resolved to zero input files and `no_inputs` strictness
    /// is on.
    #[error("rule `{rule}` matched no input files; fix its scopes or relax `strict`")]
    NoInputs {
        /// Name of the rule that resolved to nothing.
        rule: String,
    },

    /// The manifest failed to parse.
    #[error("failed to parse manifest {path}: {source}")]
    ManifestParse {
        /// Path of the offending manifest.
        path: PathBuf,
        /// Underlying parser error.
        source: Box<serde_yaml_ng::Error>,
    },

    /// A command rule's `{scope}` fan macro names a scope the manifest does not
    /// define. A macro fans over files, so a probe cannot stand in for it.
    #[error("command `{command}` references unknown scope `{scope}`")]
    UnknownScope {
        /// Name of the command rule.
        command: String,
        /// The missing scope name.
        scope: String,
    },

    /// A command rule's `inputs` names something the manifest declares neither
    /// as a scope nor as a probe. One namespace, so one error.
    #[error(
        "command `{command}` references unknown input `{input}`; declare it under `scopes:` or `probes:`"
    )]
    UnknownInput {
        /// Name of the command rule.
        command: String,
        /// The unresolvable `inputs` entry.
        input: String,
    },

    /// A probe and a scope share a name, so an `inputs:` entry naming it would
    /// be ambiguous.
    #[error(
        "`{name}` is declared as both a scope and a probe; `inputs:` has one namespace, so a name must be one or the other"
    )]
    NameCollision {
        /// The name claimed twice.
        name: String,
    },

    /// `probe_shell` was set to an empty list, so there is no program to run a
    /// probe under.
    #[error(
        "`probe_shell` is empty; it must name at least the program a probe's `run` line is passed to (the default is [\"sh\", \"-c\"])"
    )]
    EmptyProbeShell,

    /// A probe command exited non-zero, so its stdout is not a usable input.
    #[error(
        "probe `{name}` failed (exit {code}); mmz consumed no output and wrote no cache record\n  command: {run}\n  stderr: {stderr}"
    )]
    ProbeFailed {
        /// Name of the offending probe.
        name: String,
        /// The `run` line, as the manifest spells it.
        run: String,
        /// The probe's exit code (1 for a signal death).
        code: i32,
        /// What the probe wrote to stderr, trimmed and capped.
        stderr: String,
    },

    /// A probe command could not be spawned at all — the same hard stop as a
    /// probe that ran and failed.
    #[error(
        "probe `{name}` could not be run; mmz consumed no output and wrote no cache record\n  command: {run}\n  {source}"
    )]
    ProbeSpawn {
        /// Name of the offending probe.
        name: String,
        /// The `run` line, as the manifest spells it.
        run: String,
        /// Underlying spawn error.
        source: std::io::Error,
    },

    /// A probe printed nothing and did not opt into that with `allow_empty`.
    #[error(
        "probe `{name}` produced no output; that is almost always a selector that matched nothing — set `allow_empty: true` on the probe if empty really is a valid input\n  command: {run}"
    )]
    ProbeEmpty {
        /// Name of the offending probe.
        name: String,
        /// The `run` line, as the manifest spells it.
        run: String,
    },

    /// A probe's source keys do not describe one readable thing: both `run:`
    /// and `file:`, neither of them, or a `file:` with no `json:` to select
    /// from it.
    ///
    /// Checked at load with the manifest's other shape rules, so a malformed
    /// probe is refused even when no rule names it, and the message can name
    /// the probe — which a `serde` conversion on the value alone could not.
    #[error("probe `{name}` {reason}")]
    ProbeSource {
        /// Name of the offending probe.
        name: String,
        /// What is wrong with its source keys, and what to write instead.
        reason: String,
    },

    /// A probe's `file:` could not be read, so there are no bytes to select
    /// from. Named separately from [`Error::Io`] because a probe's error must
    /// name the probe: a bare "no such file" leaves a reader hunting.
    #[error(
        "probe `{name}` could not read `{path}`; mmz consumed no output and wrote no cache record\n  {source}"
    )]
    ProbeFileUnreadable {
        /// Name of the offending probe.
        name: String,
        /// The path, as the manifest spells it.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// The bytes a `json:` probe was pointed at are not one JSON value — an
    /// empty file, a tool that logged a line before its JSON, a truncated
    /// write.
    #[error(
        "probe `{name}` read {origin}, which is not one JSON value ({reason}); mmz consumed no output and wrote no cache record"
    )]
    ProbeJsonInput {
        /// Name of the offending probe.
        name: String,
        /// What was read, as the manifest points at it.
        origin: String,
        /// What the parser objected to.
        reason: String,
    },

    /// A `json:` program did not compile, or raised while running. One variant
    /// for both because the fix is the same edit — the program is wrong for
    /// the document it was pointed at — and the reason says which half broke.
    #[error(
        "probe `{name}` could not select from {origin} ({reason}); mmz consumed no output and wrote no cache record\n  json: {program}"
    )]
    ProbeJsonFailed {
        /// Name of the offending probe.
        name: String,
        /// The `json:` program, as the manifest spells it.
        program: String,
        /// What it was run against.
        origin: String,
        /// What jaq objected to.
        reason: String,
    },

    /// A `json:` selector yielded no value, or only `null`.
    ///
    /// The same refusal [`Error::ProbeEmpty`] makes for stdout, at the place
    /// the selection happens: a probe tracking `null` reports the same digest
    /// whatever the document does, so the rule is permanently fresh against an
    /// input nobody is measuring. `false` is a value and passes — jq's `-e`
    /// conflates the two only because a shell exit code cannot tell them
    /// apart, and mmz is under no such constraint.
    #[error(
        "probe `{name}` selected nothing from {origin}; that digest would measure nothing, so the rule would be fresh forever — fix the selector, or set `allow_empty: true` if an absent value really is a valid input\n  json: {program}"
    )]
    ProbeJsonEmpty {
        /// Name of the offending probe.
        name: String,
        /// The `json:` program, as the manifest spells it.
        program: String,
        /// What it was run against.
        origin: String,
    },

    /// A command rule has a blank `name`.
    #[error("command #{0} has an empty `name`; every command must declare a name")]
    EmptyCommandName(usize),

    /// Two command rules share the same name (the cache identity).
    #[error("duplicate command name: {0} (command names must be unique)")]
    DuplicateCommand(String),

    /// A command rule declares an output that is not a usable literal path.
    #[error("command `{command}` declares invalid output `{path}`: {reason}")]
    InvalidOutput {
        /// Name of the offending command.
        command: String,
        /// The offending output path, as written.
        path: String,
        /// Why it cannot be used as an output.
        reason: String,
    },

    /// A rule's command exited 0 without producing a declared output, so no
    /// record was written: a record here would claim an artifact that is not
    /// on disk, and every later invocation would skip on that claim.
    #[error(
        "`{rule}` succeeded without producing its declared output `{path}`; no cache record was written, so the rule stays stale"
    )]
    MissingOutput {
        /// Cache identity of the rule that ran.
        rule: String,
        /// The declared output that never appeared.
        path: String,
    },

    /// A command rule declares the same tag twice.
    #[error("command `{command}` declares tag `{tag}` twice")]
    DuplicateTag {
        /// Name of the offending command.
        command: String,
        /// The duplicated tag.
        tag: String,
    },

    /// A gate resolved to no rule at all, because the manifest declares none.
    ///
    /// One of three spellings of the same refusal (with
    /// [`Error::NoTaggedRules`] and [`Error::NoExpansions`]): `mmz --is-fresh`
    /// asserts that work is done, and an assertion over an empty set is
    /// vacuously true — a pass nobody earned. Every other selector that
    /// resolves to nothing is loud ([`Error::NoMatch`], [`Error::NoInputs`],
    /// [`Error::ProbeEmpty`]), so this one is too.
    #[error(
        "the manifest at {path} declares no commands, so this gate would pass without checking anything; declare a rule under `commands:`"
    )]
    NoRules {
        /// Path of the manifest that declares nothing to gate.
        path: PathBuf,
    },

    /// A `--tag` filter selected no rule, so the gate over it would be
    /// vacuously true. See [`Error::NoRules`].
    #[error(
        "no rule carries {tags}, so this gate would pass without checking anything; {declared}"
    )]
    NoTaggedRules {
        /// The filter, as a phrase naming every tag it required.
        tags: String,
        /// What the manifest does declare, so a typo is visible on the spot.
        declared: String,
    },

    /// Every rule the gate selected fans over a scope that resolved to no
    /// files, so the selection expanded to zero cache identities — an empty
    /// gate reached through the fan rather than through the filter. See
    /// [`Error::NoRules`].
    #[error(
        "every gated rule fans over a scope that resolved to no files ({rules}), so this gate would pass without checking anything"
    )]
    NoExpansions {
        /// The selected rule names, as the manifest spells them.
        rules: String,
    },

    /// `--is-fresh` (or another tag-filtered action) was given both a `--tag`
    /// filter and a specific command to target.
    #[error(
        "`--tag` cannot be combined with a command; a command already resolves to a single rule"
    )]
    TagWithCommand,

    /// A command rule's `name` carries a malformed `{scope}` fan macro.
    #[error("command `{name}` has a malformed `{{scope}}` macro: {reason}")]
    MacroSyntax {
        /// The offending rule name.
        name: String,
        /// What is wrong with the macro.
        reason: String,
    },

    /// Two rules resolve to the same cache identity, so which one owns the
    /// record — and its inputs — is ambiguous.
    #[error(
        "cache identity `{identity}` is claimed by multiple rules ({rules}); make their file sets or names disjoint"
    )]
    CollidingIdentity {
        /// The shared expanded identity.
        identity: String,
        /// The colliding rule names, for the operator to reconcile.
        rules: String,
    },

    /// A glob pattern was invalid.
    #[error("invalid pattern `{pattern}`: {source}")]
    Pattern {
        /// The offending pattern.
        pattern: String,
        /// Underlying glob error.
        source: globset::Error,
    },

    /// `MMZ_NOW` is set to something that is not a Unix epoch in seconds.
    ///
    /// Refused rather than ignored: falling back to the system clock would hide
    /// the misconfiguration and quietly restore the non-determinism the pin
    /// exists to remove.
    #[error(
        "`MMZ_NOW` is set to `{value}`, which is not a Unix epoch in seconds; set it to a whole number of seconds (e.g. `date +%s`) or unset it to use the system clock"
    )]
    InvalidNow {
        /// The offending value, as the environment spells it.
        value: String,
    },

    /// A cache record could not be serialized.
    #[error("failed to serialize cache record: {0}")]
    Serialize(Box<serde_yaml_ng::Error>),

    /// The wrapped command could not be spawned.
    #[error("failed to run `{program}`: {source}")]
    Spawn {
        /// The program mmz tried to execute.
        program: String,
        /// Underlying spawn error.
        source: std::io::Error,
    },

    /// mmz was invoked with no command to run.
    #[error("no command given")]
    EmptyCommand,

    /// `mmz --init` found a manifest already in place.
    #[error("{path} already exists; remove it first or edit it directly")]
    ManifestExists {
        /// Path of the existing manifest.
        path: PathBuf,
    },

    /// An invariant that should hold by construction did not.
    #[error("internal error: {0}")]
    Internal(String),

    /// A path named by `imports:` does not exist.
    #[error("import in {importer} names `{path}`, which does not exist")]
    ImportMissing {
        /// The file whose `imports:` list named the missing path.
        importer: PathBuf,
        /// The resolved path (directory or file) that was not found.
        path: PathBuf,
    },

    /// A path named by `imports:` exists but could not be read.
    ///
    /// Named so this cannot be confused with [`Error::Io`]'s pathless message —
    /// the root manifest keeps using that generic variant, since a missing or
    /// unreadable root manifest is not a new failure mode this feature adds.
    #[error("failed to read import {path}: {source}")]
    ImportNotReadable {
        /// The unreadable path.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },

    /// An `imports:` chain leads back to a file already being loaded.
    #[error("import cycle: {chain}")]
    ImportCycle {
        /// The chain of files, root first, ending back at the repeated path.
        chain: String,
    },

    /// Two different files each declare a scope with the same name.
    #[error("scope `{name}` is declared in both {first} and {second}")]
    DuplicateScope {
        /// The name declared twice.
        name: String,
        /// The file that declared it first.
        first: PathBuf,
        /// The file that declared it again.
        second: PathBuf,
    },

    /// Two different files each declare a probe with the same name.
    #[error("probe `{name}` is declared in both {first} and {second}")]
    DuplicateProbe {
        /// The name declared twice.
        name: String,
        /// The file that declared it first.
        first: PathBuf,
        /// The file that declared it again.
        second: PathBuf,
    },

    /// Two different files each declare a command rule with the same `name`.
    ///
    /// [`Error::DuplicateCommand`] keeps its single-file message unchanged, so
    /// a manifest with no imports sees no change; this is the cross-file
    /// spelling, naming both files.
    #[error("command `{name}` is declared in both {first} and {second}")]
    DuplicateCommandAcrossFiles {
        /// The command name declared twice.
        name: String,
        /// The file that declared it first.
        first: PathBuf,
        /// The file that declared it again.
        second: PathBuf,
    },

    /// An imported file (not the root manifest) sets a key that only the root
    /// manifest may set (`cache_dir`, `gitignore`, `strict`, `on_hit`).
    #[error("`{key}` is set in {path}, but may only be set in the root manifest")]
    FragmentPolicyKey {
        /// The offending top-level key.
        key: String,
        /// The fragment that set it.
        path: PathBuf,
    },

    /// The root manifest set a policy key (`gitignore`, `cache_dir` or
    /// `strict`) to explicit YAML `null`.
    ///
    /// Before composition existed these were plain, non-nullable fields, so
    /// `null` was a hard parse error; composition's shared per-file parser
    /// has to accept a present-but-null key so a *fragment* setting one is
    /// still caught by [`Error::FragmentPolicyKey`]. That means the root's
    /// own explicit null cannot be allowed to fall through to the default
    /// silently — it must still fail, just later.
    #[error("`{key}` is `null` in {path}; omit it to use the default, or give it a value")]
    NullPolicyKey {
        /// The key set to `null`.
        key: String,
        /// The root manifest that set it.
        path: PathBuf,
    },

    /// A probe's `ast:` key could not produce a digest. The detail — and which
    /// refusal it is — lives with the matcher in [`crate::ast::AstFailure`],
    /// because one of those cases is answered by a cargo feature rather than a
    /// manifest edit and would not survive being flattened into a string here.
    #[error("probe `{name}` {source}")]
    ProbeAst {
        /// Name of the offending probe.
        name: String,
        /// What the matcher refused, and the edit or flag that fixes it.
        source: Box<crate::ast::AstFailure>,
    },
}

/// Convenience alias for fallible operations in this crate.
pub type Result<T> = std::result::Result<T, Error>;

#[cfg(test)]
#[path = "error_tests.rs"]
mod tests;
