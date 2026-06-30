// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use tempfile::{NamedTempFile, TempDir};

pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures")
        .join("visual")
        .join(name)
}

pub fn read_fixture(name: &str) -> Result<String> {
    let path = fixture_path(name);
    fs::read_to_string(&path).with_context(|| format!("failed to read {}", path.display()))
}

pub fn named_temp_file(contents: &str) -> Result<NamedTempFile> {
    let file = NamedTempFile::new().context("failed to create temporary file")?;
    fs::write(file.path(), contents)
        .with_context(|| format!("failed to write {}", file.path().display()))?;
    Ok(file)
}

pub fn named_temp_file_with_suffix(contents: &str, suffix: &str) -> Result<NamedTempFile> {
    let file = tempfile::Builder::new()
        .suffix(suffix)
        .tempfile()
        .context("failed to create temporary file")?;
    fs::write(file.path(), contents)
        .with_context(|| format!("failed to write {}", file.path().display()))?;
    Ok(file)
}

pub fn temp_home() -> Result<TempDir> {
    tempfile::tempdir().context("failed to create temporary HOME")
}
