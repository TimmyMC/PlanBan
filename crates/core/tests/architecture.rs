//! Architectural fitness functions — automated guards for the two invariants
//! that make Clabby what it is. These fail the build the moment a change erodes
//! them, instead of relying on review vigilance (Constitution §9).
//!
//! - §7 Core stays UI-free: the engine is a library with no UI/Tauri dependency.
//! - §11 Generic core: no tracker/agent/VCS vendor is hardcoded; vendor specifics
//!   live in command templates and config, never in `clabby-core`.

use std::path::Path;

/// Crates that would mean a UI, a network/tracker client, or a vendor SDK leaked
/// into the engine. The point of Clabby is that *none* of these belong here —
/// every external action is a rendered command template run by `runner.rs`.
const FORBIDDEN_DEPS: &[&str] = &[
    // UI / desktop
    "tauri",
    "egui",
    "gtk",
    "iced",
    "dioxus",
    "yew",
    "leptos",
    // network / http clients (core shells out; it never calls APIs directly)
    "reqwest",
    "hyper",
    "axum",
    "actix-web",
    "ureq",
    "isahc",
    // vendor SDKs
    "octocrab",
    "aws-sdk",
    "azure_",
    "google-",
];

fn core_manifest() -> String {
    // Tests run with the crate root as the working directory.
    std::fs::read_to_string("Cargo.toml").expect("read clabby-core Cargo.toml")
}

/// Returns the dependency-key lines of the `[dependencies]` / `[build-dependencies]`
/// tables (ignores `[dev-dependencies]`, which may pull in test-only tooling).
fn runtime_dependency_lines(manifest: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut in_runtime_deps = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_runtime_deps = trimmed == "[dependencies]" || trimmed == "[build-dependencies]";
            continue;
        }
        if in_runtime_deps && !trimmed.is_empty() && !trimmed.starts_with('#') {
            out.push(trimmed);
        }
    }
    out
}

#[test]
fn core_has_no_ui_or_vendor_dependencies() {
    let manifest = core_manifest();
    for line in runtime_dependency_lines(&manifest) {
        let dep = line.split(['=', ' ']).next().unwrap_or("").trim();
        for forbidden in FORBIDDEN_DEPS {
            assert!(
                !dep.starts_with(forbidden),
                "Constitution §7/§11 violated: clabby-core must stay UI/vendor-free, \
                 but its manifest depends on `{dep}`. Vendor/UI behavior belongs in a \
                 command template or a driver crate, not the engine."
            );
        }
    }
}

#[test]
fn core_source_imports_no_vendor_crate() {
    // Catches actual coupling (`use reqwest::...`) without false-positiving on doc
    // comments that *mention* a vendor to explain the §11 principle.
    let src = Path::new("src");
    let mut offenders = Vec::new();
    visit_rs_files(src, &mut |path, contents| {
        for line in contents.lines() {
            let t = line.trim_start();
            if !t.starts_with("use ") {
                continue;
            }
            for forbidden in FORBIDDEN_DEPS {
                let needle = forbidden.replace('-', "_");
                if t.starts_with(&format!("use {needle}"))
                    || t.starts_with(&format!("use ::{needle}"))
                {
                    offenders.push(format!("{}: {t}", path.display()));
                }
            }
        }
    });
    assert!(
        offenders.is_empty(),
        "Constitution §7/§11 violated: clabby-core imports a vendor/UI/network crate:\n{}",
        offenders.join("\n")
    );
}

fn visit_rs_files(dir: &Path, f: &mut impl FnMut(&Path, &str)) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            visit_rs_files(&path, f);
        } else if path.extension().is_some_and(|e| e == "rs") {
            if let Ok(contents) = std::fs::read_to_string(&path) {
                f(&path, &contents);
            }
        }
    }
}
