// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;

use support::{fixtures, keys, pty::RilePty};

#[test]
fn vertical_scrolling_keeps_cursor_visible() -> Result<()> {
    let file = fixtures::fixture_path("numbered.txt");
    let mut rile = RilePty::spawn(&file, 8, 60)?;

    rile.wait_for_screen_contains("numbered.txt")?;
    for _ in 0..12 {
        rile.send("C-n", keys::control('n'))?;
    }

    rile.assert_status_contains("Ln 013 Col 000")?;
    rile.assert_screen_contains("012 | 0123456789")?;
    let (row, _) = rile.cursor_position();
    assert!(row < 6, "cursor left text area\n{}", rile.screen_dump());

    for _ in 0..10 {
        rile.send("C-p", keys::control('p'))?;
    }

    rile.assert_status_contains("Ln 003 Col 000")?;
    rile.assert_screen_contains("002 | 0123456789")?;
    let (row, _) = rile.cursor_position();
    assert!(row < 6, "cursor left text area\n{}", rile.screen_dump());

    rile.quit()?;
    Ok(())
}

#[test]
fn horizontal_scrolling_keeps_cursor_visible_on_long_lines() -> Result<()> {
    let file = fixtures::fixture_path("long_lines.txt");
    let mut rile = RilePty::spawn(&file, 8, 32)?;

    rile.wait_for_screen_contains("000 | 0123456789")?;
    rile.send("C-e", keys::control('e'))?;

    let (row, column) = rile.cursor_position();
    assert_eq!(
        row,
        0,
        "cursor should stay on first row\n{}",
        rile.screen_dump()
    );
    assert!(
        column < 32,
        "cursor left narrow screen\n{}",
        rile.screen_dump()
    );
    assert!(
        !rile.snapshot_text().contains("000 | 0123456789"),
        "screen did not scroll horizontally\n{}",
        rile.screen_dump()
    );

    rile.quit()?;
    Ok(())
}
