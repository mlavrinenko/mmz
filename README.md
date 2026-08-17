<!-- Generated from docs/src/readme.typ by `just docs md`. Do not edit; edit the source. -->

<h1 align="center"><img src="https://raw.githubusercontent.com/mlavrinenko/mmz/main/www/assets/images/logo.svg" alt="mmz" width="96" /><br />
mmz</h1><h4 align="center">A <a href="LICENSE-MIT">MIT</a>-licensed memoized command runner.</h4><h4 align="center"><a href="https://mlavrinenko.github.io/mmz/">Documentation</a> · <a href="https://mlavrinenko.github.io/mmz/quickstart/">Quickstart</a> · <a href="https://mlavrinenko.github.io/mmz/comparison/">Comparison</a></h4>

---

[<img src="https://github.com/mlavrinenko/mmz/actions/workflows/ci.yml/badge.svg" alt="CI" />](https://github.com/mlavrinenko/mmz/actions/workflows/ci.yml) [<img src="https://img.shields.io/crates/v/mmz.svg" alt="crates.io" />](https://crates.io/crates/mmz) [<img src="https://img.shields.io/crates/l/mmz.svg" alt="License: MIT" />](LICENSE-MIT)

Prefix any command with `mmz`. When the matched rule’s declared inputs are byte-for-byte unchanged since that command last succeeded, `mmz` skips it and exits 0. Otherwise it runs the command, streams its output, and records the result on success.

It answers one question per invocation: is this rule’s work still done? [The comparison page](https://mlavrinenko.github.io/mmz/comparison/) places it beside the build systems, task runners and compiler caches you might reach for instead.

```yaml
# .mmz/config.yaml
scopes:
  rust: ["**/*.rs", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml"]

commands:
  - name: cargo test
    inputs: [rust]
```

```console
validate: 4 orders, 2 customers OK
mmz: skipped ./bin/validate.sh (inputs unchanged)
```

## Installation

```bash
cargo install mmz
```

[Quickstart](https://mlavrinenko.github.io/mmz/quickstart/) covers the prebuilt binaries, `nix run`, and scaffolding a manifest.

## Usage

```bash
mmz --init                # write a starter .mmz/config.yaml
mmz cargo test            # skipped when the declared inputs are unchanged
mmz --status              # each rule's freshness and record age
mmz --is-fresh --tag gate # exit 0 if every gate-tagged rule is fresh; runs nothing
mmz --prune               # drop records whose rule no longer exists
```

## The trade

`mmz` cannot see a dependency you did not declare, so the asymmetry is the whole contract:

- Under-declaring a rule’s inputs skips a command that should have run — a false green, and dangerous.
- Over-declaring buys an unnecessary re-run — and nothing else.

Broaden the scope when in doubt. `mmz` fails closed everywhere else: a missing or invalid manifest always errors, and so do an unmatched command and a matched rule with no inputs, unless `strict` relaxes them.

## Documentation

- [Quickstart](https://mlavrinenko.github.io/mmz/quickstart/) — Install mmz, scaffold a manifest, and watch a command skip itself.
- [Concepts](https://mlavrinenko.github.io/mmz/concepts/) — The model behind mmz: rules, records, freshness, and the asymmetry that decides every design question.
- [Inputs: scopes and probes](https://mlavrinenko.github.io/mmz/inputs/) — Named glob sets, the gitignore filter and how to opt one scope out of it, and probes for the inputs that are not files.
- [Matching and parametric rules](https://mlavrinenko.github.io/mmz/matching/) — How an invocation is matched to a rule, how the cache identity follows from that, and how one rule can fan over a scope's files.
- [Declared outputs](https://mlavrinenko.github.io/mmz/outputs/) — A producer command's record can be undone without touching an input. Declaring what a run leaves behind is how mmz notices.
- [Gating with tags](https://mlavrinenko.github.io/mmz/gating/) — Use --is-fresh to require that an expensive check already passed, and tags to decide which rules a gate is allowed to ask about.
- [Manifest reference](https://mlavrinenko.github.io/mmz/manifest/) — Every key .mmz/config.yaml can declare, generated from the JSON Schema the binary ships.
- [CLI reference](https://mlavrinenko.github.io/mmz/cli/) — Every action mmz accepts, every exit code it returns, and the JSON it can be asked for — generated from the binary's own help text.
- [Rust library](https://mlavrinenko.github.io/mmz/library/) — mmz ships as a library crate as well as a binary; the binary is a thin wrapper over it.
- [For AI agents](https://mlavrinenko.github.io/mmz/agents/) — Driving mmz from an agent: machine-readable state, honest exit codes, and the one mistake an agent is most likely to make with it.
- [Comparison](https://mlavrinenko.github.io/mmz/comparison/) — Where mmz sits among build systems, task runners and compiler caches — and which of them you should reach for instead.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). mmz memoizes its own checks, so `just check` is itself the worked example — and closing a task asserts those checks already passed with `mmz --is-fresh --tag gate`.

## License

MIT.
