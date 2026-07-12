// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn regexp_engine_is_private_to_search_pattern_module() {
    let source_root = manifest_path("src");
    let allowed = source_root.join("search_pattern.rs");
    let allowed_contents = fs::read_to_string(&allowed).expect("search pattern source is readable");

    assert!(
        allowed_contents.contains("struct RegexpPattern"),
        "src/search_pattern.rs should define the built-in regexp engine"
    );
    assert!(
        !allowed_contents.contains("pub(crate) struct RegexpPattern")
            && !allowed_contents.contains("pub struct RegexpPattern"),
        "RegexpPattern should stay private; callers should use SearchPattern"
    );

    let mut offenders = Vec::new();

    for path in rust_source_files(&source_root) {
        if path == allowed {
            continue;
        }
        let contents = fs::read_to_string(&path).expect("source file should be readable");
        if source_without_line_comments(&contents).contains("RegexpPattern") {
            offenders.push(relative_to_manifest(&path));
        }
    }

    assert!(
        offenders.is_empty(),
        "regexp internals should stay private to src/search_pattern.rs; offenders: {offenders:?}"
    );
}

#[test]
fn crate_does_not_depend_on_external_regex_engine() {
    let cargo_toml =
        fs::read_to_string(manifest_path("Cargo.toml")).expect("Cargo.toml should be readable");
    let forbidden = forbidden_regex_dependency_entries(&cargo_toml);

    assert!(
        forbidden.is_empty(),
        "regexp support should stay in the built-in search_pattern module; forbidden dependencies: {forbidden:?}"
    );
}

fn forbidden_regex_dependency_entries(cargo_toml: &str) -> Vec<String> {
    let mut forbidden = Vec::new();
    let mut in_dependency_table = false;

    for raw_line in cargo_toml.lines() {
        let line = raw_line.split('#').next().unwrap_or_default().trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_dependency_table = matches!(
                line,
                "[dependencies]"
                    | "[dev-dependencies]"
                    | "[build-dependencies]"
                    | "[workspace.dependencies]"
            );
            if forbidden_dependency_table_header(line).is_some() {
                forbidden.push(raw_line.trim().to_owned());
            }
            continue;
        }
        if in_dependency_table && forbidden_dependency_assignment(line).is_some() {
            forbidden.push(raw_line.trim().to_owned());
        }
    }

    forbidden
}

fn forbidden_dependency_assignment(line: &str) -> Option<&'static str> {
    ["regex", "regex-lite", "regex-syntax"]
        .into_iter()
        .find(|name| {
            line.strip_prefix(name)
                .is_some_and(|rest| rest.trim_start().starts_with('='))
        })
}

fn forbidden_dependency_table_header(line: &str) -> Option<&'static str> {
    ["regex", "regex-lite", "regex-syntax"]
        .into_iter()
        .find(|name| {
            line == format!("[dependencies.{name}]")
                || line == format!("[dev-dependencies.{name}]")
                || line == format!("[build-dependencies.{name}]")
                || line == format!("[workspace.dependencies.{name}]")
        })
}

fn source_without_line_comments(contents: &str) -> String {
    contents
        .lines()
        .map(|line| line.split("//").next().unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

fn rust_source_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_rust_source_files(root, &mut files);
    files
}

fn collect_rust_source_files(path: &Path, files: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(path).expect("source directory should be readable") {
        let entry = entry.expect("source entry should be readable");
        let path = entry.path();
        if path.is_dir() {
            collect_rust_source_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn manifest_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn relative_to_manifest(path: &Path) -> PathBuf {
    path.strip_prefix(env!("CARGO_MANIFEST_DIR"))
        .expect("path should be under manifest directory")
        .to_owned()
}
