# mmz

[![CI](https://github.com/mlavrinenko/mmz/actions/workflows/ci.yml/badge.svg)](https://github.com/mlavrinenko/mmz/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/mmz.svg)](https://crates.io/crates/mmz)
[![License: MIT](https://img.shields.io/crates/l/mmz.svg)](LICENSE-MIT)

memoized command runner

## Install

### From crates.io

```bash
cargo install mmz
```

### From binary releases

Download a pre-built binary from the
[latest release](https://github.com/mlavrinenko/mmz/releases/latest).

## Usage

`mmz` is a memoized command runner: prefix any command with `mmz`, and when the
matched rule's declared inputs are byte-for-byte unchanged since the command last
succeeded, `mmz` skips it and exits 0. Otherwise it runs the command, streams its
output, and records the result on success.

Scaffold a manifest:

```bash
mmz --init        # writes a starter mmz.yaml
```

```yaml
# mmz.yaml — nearest one, searching upward from the working directory
scopes:
  rust: ["**/*.rs", Cargo.toml, Cargo.lock]
commands:
  - name: cargo test      # token-prefix matcher and cache identity
    inputs: [rust]
# strict: [no_match, no_inputs]   # the default; omit, or use [] to relax
```

Then wrap commands wherever memoization is wanted — a Justfile recipe, a shell,
a git hook:

```bash
mmz cargo test            # skipped when the rust inputs are unchanged
mmz --status              # show each rule's freshness
mmz --schema              # print the mmz.yaml JSON Schema
```

`mmz` fails closed: it errors when no manifest is found, the manifest is invalid,
no rule matches, or a matched rule has no inputs. Relax the last two per project
with the `strict` list. See [DESIGN.md](DESIGN.md) for the full model.

## Development

Prerequisites: [Nix](https://nixos.org/) with flakes enabled.

```bash
direnv allow         # or: nix develop

just check           # fmt + clippy + tests + file-size + drift check
just build
just test
just cover           # code coverage (70% minimum)
just fmt             # format code
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for coding conventions.

## License

MIT
