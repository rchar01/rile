// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;

use support::{fixtures, keys, pty::RilePty};

#[test]
fn small_test_size_keeps_cursor_and_status_visible() -> Result<()> {
    let file = fixtures::fixture_path("numbered.txt");
    let mut rile = RilePty::spawn(&file, 4, 80)?;

    rile.wait_for_screen_contains("000 | 0123456789")?;
    rile.assert_screen_contains("000 | 0123456789")?;
    rile.assert_status_contains("Rile VISUAL")?;
    rile.assert_cursor(0, 0)?;

    rile.send("C-n", keys::control('n'))?;
    rile.assert_status_contains("Ln 002 Col 000")?;
    rile.assert_cursor(1, 0)?;

    rile.quit()?;
    Ok(())
}

#[test]
fn narrow_test_size_clips_long_lines_without_losing_status() -> Result<()> {
    let file = fixtures::fixture_path("long_lines.txt");
    let mut rile = RilePty::spawn(&file, 6, 28)?;

    rile.wait_for_screen_contains("000 | 0123456789")?;
    rile.assert_status_contains("Rile VISUAL")?;
    rile.assert_cursor(0, 0)?;

    rile.send("C-e", keys::control('e'))?;
    let (_, column) = rile.cursor_position();
    assert!(
        column < 28,
        "cursor left narrow screen\n{}",
        rile.screen_dump()
    );

    rile.quit()?;
    Ok(())
}
