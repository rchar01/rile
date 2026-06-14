// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use std::fs;

use anyhow::{Context, Result};

use support::{fixtures, keys, pty::RilePty};

#[test]
fn save_buffer_writes_disk_contents_and_clears_dirty_state() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let path = file.path().to_path_buf();
    let mut rile = RilePty::spawn(&path, 12, 80)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.assert_status_contains("modified:false")?;

    rile.send("insert text", b"saved ")?;
    rile.wait_for_screen_contains("saved alpha")?;
    rile.assert_status_contains("modified:true")?;

    rile.send("C-x C-s", keys::control_sequence("xs"))?;
    rile.wait_for_screen_contains("Wrote")?;
    rile.assert_status_contains("modified:false")?;

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read saved file {}", path.display()))?;
    assert_eq!(contents, "saved alpha\nbeta\n");

    rile.quit()?;
    Ok(())
}

#[test]
fn write_file_saves_as_new_path_and_clears_dirty_state() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let target = tempfile::NamedTempFile::new().context("failed to create target file")?;
    let target_path = target.path().to_path_buf();
    let target_text = target_path.to_string_lossy().into_owned();
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("insert text", b"saved ")?;
    rile.wait_for_screen_contains("saved alpha")?;
    rile.assert_status_contains("modified:true")?;

    rile.send("C-x C-w", keys::control_sequence("xw"))?;
    rile.assert_screen_contains("Write file:")?;
    rile.send("target path", target_text.as_bytes())?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("Wrote")?;
    rile.assert_status_contains("modified:false")?;

    let contents = fs::read_to_string(&target_path)
        .with_context(|| format!("failed to read saved file {}", target_path.display()))?;
    assert_eq!(contents, "saved alpha\nbeta\n");

    rile.quit()?;
    Ok(())
}

#[test]
fn insert_file_inserts_contents_at_point() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    let source = directory.path().join("source.txt");
    std::fs::write(&start, "before\nafter\n")?;
    std::fs::write(&source, "inserted line\n")?;
    let mut rile = RilePty::spawn(&start, 12, 100)?;

    rile.wait_for_screen_contains("before")?;
    rile.send("C-n", keys::control('n'))?;
    rile.send("C-x", keys::control('x'))?;
    rile.send("i", b"i")?;
    rile.assert_screen_contains("Insert file:")?;
    rile.send("source file", b"source.txt")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("inserted line")?;
    rile.assert_screen_contains("Inserted")?;
    rile.assert_status_contains("modified:true")?;

    rile.send("C-_", b"\x1f")?;
    if rile.snapshot_text().contains("inserted line") {
        anyhow::bail!("undo did not remove inserted file\n{}", rile.screen_dump());
    }

    rile.quit()?;
    Ok(())
}
