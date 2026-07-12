// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::{Path, PathBuf};

#[test]
fn regexp_engine_is_private_to_search_pattern_module() {
    let source_root = manifest_path("src");
    let allowed = source_root.join("search_pattern.rs");
    let mut offenders = Vec::new();

    for path in rust_source_files(&source_root) {
        if path == allowed {
            continue;
        }
        let contents = fs::read_to_string(&path).expect("source file should be readable");
        if contents.contains("RegexpPattern") {
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
    let forbidden = cargo_toml
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with('#'))
        .filter(|line| {
            line.starts_with("regex =")
                || line.starts_with("regex-lite =")
                || line.starts_with("regex-syntax =")
        })
        .collect::<Vec<_>>();

    assert!(
        forbidden.is_empty(),
        "regexp support should stay in the built-in search_pattern module; forbidden dependencies: {forbidden:?}"
    );
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
