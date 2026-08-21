use super::Error;

#[test]
fn messages_are_actionable() {
    let scope = Error::UnknownScope {
        command: "cargo test".to_owned(),
        scope: "rust".to_owned(),
    };
    assert!(scope.to_string().contains("unknown scope `rust`"));
    let dup = Error::DuplicateCommand("sh".to_owned());
    assert!(dup.to_string().contains("duplicate command name"));
    let blank = Error::EmptyCommandName(2);
    assert!(blank.to_string().contains("empty `name`"));

    let no_match = Error::NoMatch {
        command: "cargo build".to_owned(),
    };
    assert!(
        no_match
            .to_string()
            .contains("no rule matches `cargo build`")
    );
    let no_inputs = Error::NoInputs {
        rule: "cargo test".to_owned(),
    };
    assert!(no_inputs.to_string().contains("matched no input files"));
    let no_manifest = Error::NoManifest {
        start: std::path::PathBuf::from("/tmp/x"),
    };
    assert!(
        no_manifest
            .to_string()
            .contains("no .mmz/config.yaml found")
    );

    let bad_output = Error::InvalidOutput {
        command: "just cover".to_owned(),
        path: "target/*.info".to_owned(),
        reason: "outputs are literal paths, not patterns".to_owned(),
    };
    assert!(
        bad_output
            .to_string()
            .contains("declares invalid output `target/*.info`")
    );
    let missing = Error::MissingOutput {
        rule: "just cover".to_owned(),
        path: "target/coverage/lcov.info".to_owned(),
    };
    let text = missing.to_string();
    assert!(
        text.contains("target/coverage/lcov.info"),
        "the missing artifact is named: {text}"
    );
    assert!(
        text.contains("no cache record was written"),
        "and the consequence is spelled out: {text}"
    );
}

#[test]
fn probe_messages_name_the_probe_and_the_consequence() {
    let failed = Error::ProbeFailed {
        name: "fmt-recipe".to_owned(),
        run: "just --dump | jq .recipes".to_owned(),
        code: 5,
        stderr: "jq: error: no such key".to_owned(),
    };
    let text = failed.to_string();
    assert!(
        text.contains("probe `fmt-recipe`"),
        "names the probe: {text}"
    );
    assert!(text.contains("exit 5"), "names the exit code: {text}");
    assert!(text.contains("jq: error"), "carries stderr: {text}");
    assert!(
        text.contains("wrote no cache record"),
        "a failed probe never reaches the hasher, and says so: {text}"
    );

    let spawn = Error::ProbeSpawn {
        name: "toolchain".to_owned(),
        run: "rustc -vV".to_owned(),
        source: std::io::Error::other("no such file"),
    };
    let text = spawn.to_string();
    assert!(
        text.contains("probe `toolchain`"),
        "names the probe: {text}"
    );
    assert!(
        text.contains("wrote no cache record"),
        "an unspawnable probe is the same hard stop: {text}"
    );

    let empty = Error::ProbeEmpty {
        name: "selector".to_owned(),
        run: "jq -c .missing".to_owned(),
    };
    let text = empty.to_string();
    assert!(text.contains("probe `selector`"), "names the probe: {text}");
    assert!(
        text.contains("allow_empty"),
        "points at the opt-in rather than leaving it a dead end: {text}"
    );
}

#[test]
fn input_namespace_messages_are_actionable() {
    let unknown = Error::UnknownInput {
        command: "cargo test".to_owned(),
        input: "ghost".to_owned(),
    };
    let text = unknown.to_string();
    assert!(
        text.contains("unknown input `ghost`"),
        "names the entry: {text}"
    );
    assert!(
        text.contains("`scopes:` or `probes:`"),
        "names both places it could be declared: {text}"
    );

    let clash = Error::NameCollision {
        name: "rust".to_owned(),
    };
    assert!(
        clash.to_string().contains("one namespace"),
        "the collision explains why one name cannot be both"
    );
}
