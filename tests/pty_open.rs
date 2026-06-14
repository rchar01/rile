// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;
use predicates::prelude::*;

use support::{fixtures, keys, pty::RilePty};

#[test]
fn opens_visual_fixture_in_pty() -> Result<()> {
    let file = fixtures::fixture_path("numbered.txt");
    let mut rile = RilePty::spawn(&file, 10, 50)?;

    rile.wait_for_screen_contains("numbered.txt")?;
    rile.assert_screen_contains("000 | 0123456789")?;
    assert!(
        predicate::str::contains("Rile VISUAL").eval(&rile.snapshot_text()),
        "{}",
        rile.screen_dump()
    );
    rile.assert_status_contains("Rile VISUAL")?;
    rile.assert_status_contains("ACTIVE numbered.txt")?;
    rile.assert_cursor(0, 0)?;
    insta::assert_snapshot!(format!("{:?}", rile.cursor_position()), @r"(0, 0)");
    rile.quit()?;

    Ok(())
}

#[test]
fn opens_file_read_only_and_blocks_editing() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    let target = directory.path().join("target.txt");
    std::fs::write(&start, "start\n")?;
    std::fs::write(&target, "read-only target\n")?;
    let mut rile = RilePty::spawn(&start, 12, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x", keys::control('x'))?;
    rile.send("C-r", keys::control('r'))?;
    rile.assert_screen_contains("Find file read-only:")?;
    rile.send("target.txt", b"target.txt")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("read-only target")?;
    rile.assert_screen_contains("Opened read-only")?;

    rile.send("x", b"x")?;
    rile.assert_screen_contains("Buffer is read-only:")?;
    if rile.snapshot_text().contains("xread-only target") {
        anyhow::bail!("read-only edit modified buffer\n{}", rile.screen_dump());
    }

    rile.send("C-x", keys::control('x'))?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_screen_contains("Error: save failed: invalid input: buffer is read-only")?;

    rile.quit()?;
    Ok(())
}
