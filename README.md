<!-- Generated from docs/src/readme.typ by `just docs md`. Do not edit; edit the source. -->

<h1 align="center"><img src="https://raw.githubusercontent.com/mlavrinenko/mmz/main/www/assets/images/logo.svg" alt="mmz" width="96" /><br />
mmz</h1><h4 align="center">Memoize what’s done.</h4><h4 align="center"><a href="https://mlavrinenko.github.io/mmz/">Documentation</a> · <a href="https://mlavrinenko.github.io/mmz/quickstart/">Quickstart</a> · <a href="https://mlavrinenko.github.io/mmz/comparison/">Comparison</a></h4>

---

[<img src="https://github.com/mlavrinenko/mmz/actions/workflows/ci.yml/badge.svg" alt="CI" />](https://github.com/mlavrinenko/mmz/actions/workflows/ci.yml) [<img src="https://img.shields.io/crates/v/mmz.svg" alt="crates.io" />](https://crates.io/crates/mmz) [<img src="https://img.shields.io/crates/l/mmz.svg" alt="License: MIT" />](LICENSE-MIT)

Configure inputs for a command and run it with `mmz ...` as a prefix. `mmz` remembers the last successful run and skips further executions while those inputs remain unchanged.

For example, the [demo project’s](https://github.com/mlavrinenko/mmz/tree/main/examples/demo) `.mmz/config.yaml` declares two commands and their inputs:

```yaml
scopes:
  data: ["data/*.json"]
  bin: ["bin/*.sh"]

probes:
  # jq's version is an input, but not a file.
  jq-version:
    run: jq --version

on_hit: "mmz: skipped {cache:command} (inputs unchanged)"

commands:
  - name: ./bin/validate.sh
    inputs: [data, bin, jq-version]
    tags: [gate]

  - name: ./bin/report.sh
    inputs: [data, bin, jq-version]
    outputs:
      - out/report.txt
    tags: [gate, report]
```

Running `mmz ./bin/validate.sh` does the work:

```console
validate: 4 orders, 2 customers OK
```

Running it again skips the command:

```console
mmz: skipped ./bin/validate.sh (inputs unchanged)
```

Then, you can put `mmz --is-fresh --tag gate` in your git pre-commit hook to refuse a commit when a gated check is out of date:

```bash
#!/usr/bin/env sh
# .git/hooks/pre-commit
mmz --is-fresh --tag gate || {
  echo "run the gated checks first" >&2
  exit 1
}
```

No checks run during `git commit`, but you’re still covered.

See how it [compares](https://mlavrinenko.github.io/mmz/comparison/) with other tools.

## Installation

```bash
cargo install mmz
```

[Quickstart](https://mlavrinenko.github.io/mmz/quickstart/) covers the prebuilt binaries, `nix run`, and scaffolding a manifest.

## Usage

```bash
mmz --init                # write a starter .mmz/config.yaml
mmz cargo test            # skipped when its inputs are unchanged
mmz --status              # each command's state and last-run age
mmz --is-fresh --tag gate # exit 0 if every gate-tagged command is up to date; runs nothing
mmz --prune               # drop cache records for commands that no longer exist
mmz --import ../wt        # copy cache records from another checkout
```

## Documentation

**Start here**

- [Quickstart](https://mlavrinenko.github.io/mmz/quickstart/) — Install mmz and watch a command skip.
- [Concepts](https://mlavrinenko.github.io/mmz/concepts/) — How rules, records, and freshness fit together.

**Writing a manifest**

- [Inputs](https://mlavrinenko.github.io/mmz/inputs/) — The files and tool versions a command depends on.
- [Code inputs](https://mlavrinenko.github.io/mmz/code/) — Depend on a function or type instead of a whole file.
- [Matching](https://mlavrinenko.github.io/mmz/matching/) — How a command finds its rule, and how one rule can cover many files.
- [Declared outputs](https://mlavrinenko.github.io/mmz/outputs/) — Declare what a run leaves behind.
- [Gating with tags](https://mlavrinenko.github.io/mmz/gating/) — Require a check to have passed, without running it again.
- [Composition](https://mlavrinenko.github.io/mmz/composition/) — Split one manifest across several files.

**Reference**

- [Manifest reference](https://mlavrinenko.github.io/mmz/manifest/) — Every key .mmz/config.yaml can declare.
- [CLI reference](https://mlavrinenko.github.io/mmz/cli/) — Every action, flag, and exit code.
- [Rust library](https://mlavrinenko.github.io/mmz/library/) — Use mmz as a Rust library.

**More**

- [For AI agents](https://mlavrinenko.github.io/mmz/agents/) — Machine-readable state and exit codes for agents.
- [Comparison](https://mlavrinenko.github.io/mmz/comparison/) — With similar tools.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT.
