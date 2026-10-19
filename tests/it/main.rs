//! The integration suite, linked as one binary. Cargo builds one executable
//! per file directly under `tests/`, and each links every dependency again; a
//! module here costs a line instead. A new test goes in as `tests/it/<name>.rs`
//! plus a `mod <name>;` line below.

mod support;

mod changelog_history_gate;
mod cli;
mod cli_ast_probes;
mod cli_clock;
mod cli_declaration_changes;
mod cli_dump;
mod cli_empty_selection;
mod cli_import;
mod cli_input_exclude;
mod cli_json_probes;
mod cli_outputs;
mod cli_parametric;
mod cli_probes;
mod cli_schema;
mod cli_scope_gitignore;
mod cli_ttl;
mod cli_unreadable_inputs;
mod cli_unreadable_state;
mod cli_version;
mod docs_capture_paths;
mod gate_inputs_pin_the_toolchain;
mod gate_probe_normalisation;
