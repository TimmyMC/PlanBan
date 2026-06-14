//! Use-case coverage gate (Constitution §9, `docs/testing.md`).
//!
//! Walks the clap command tree — the single source of truth in
//! `clabby::use_case_keys()` — and **fails the build** if any leaf command or
//! flag is missing a row in `docs/use-cases.md`, or if a row references a
//! black-box test that no longer exists. This is the deterministic teeth behind
//! "100% of CLI use cases are tested": a new command/flag can't ship without a
//! registry row *and* a real test. CI catches it; no human vigilance required.

use std::fs;

const REGISTRY: &str = "../../docs/use-cases.md";
const BLACKBOX: &str = "tests/cli_blackbox.rs";

#[test]
fn every_cli_command_and_flag_is_registered_and_tested() {
    let registry = fs::read_to_string(REGISTRY).expect("read docs/use-cases.md");

    // 1) Completeness — every clap leaf command + flag has a `key` row.
    let missing: Vec<String> = clabby::use_case_keys()
        .into_iter()
        .filter(|k| !registry.contains(&format!("`{k}`")))
        .collect();
    assert!(
        missing.is_empty(),
        "CLI use cases missing a row in docs/use-cases.md (add a row + a test):\n  {}",
        missing.join("\n  ")
    );

    // 2) No dangling refs — every `cli_blackbox::<fn>` named in the registry exists.
    let blackbox = fs::read_to_string(BLACKBOX).expect("read cli_blackbox.rs");
    let dangling: Vec<String> = registry
        .split(|c: char| c.is_whitespace() || c == '`' || c == '|')
        .filter_map(|t| t.strip_prefix("cli_blackbox::"))
        .filter(|f| !blackbox.contains(&format!("fn {f}")))
        .map(str::to_string)
        .collect();
    assert!(
        dangling.is_empty(),
        "docs/use-cases.md references tests that don't exist in cli_blackbox.rs: {dangling:?}"
    );
}
