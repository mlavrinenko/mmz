# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Memoized command runner: prefix a command with `mmz` to skip it when the
  matched rule's declared inputs are unchanged since it last succeeded.
- `mmz.yaml` manifest with named `scopes`, ordered `commands` (token-prefix
  matchers), and `gitignore` (default true).
- `strict` list (default: all): the runtime cases mmz errors on rather than
  falling back — `no_match` and `no_inputs`. Use a subset, or `[]`, to relax.
- `mmz --init`, `mmz --status`, and `mmz --schema` actions.
- JSON Schema for `mmz.yaml` at `schema/mmz.schema.json`.

### Notes

- Fails closed by default: a missing or invalid manifest always errors, as do
  unmatched commands and matched rules with no inputs (relaxable via `strict`).
