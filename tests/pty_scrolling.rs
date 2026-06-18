// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;

use support::{fixtures, keys, pty::RilePty};

fn control_x_text(text: &str) -> Vec<u8> {
    let mut sequence = keys::control('x').to_vec();
    sequence.extend_from_slice(text.as_bytes());
    sequence
}

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
fn page_scroll_keys_move_by_visible_page_with_overlap() -> Result<()> {
    let file = fixtures::fixture_path("numbered.txt");
    let mut rile = RilePty::spawn(&file, 8, 60)?;

    rile.wait_for_screen_contains("000 | 0123456789")?;
    rile.send("C-v", keys::control('v'))?;
    rile.assert_status_contains("Ln 006 Col 000")?;
    rile.assert_screen_contains("005 | 0123456789")?;
    rile.assert_cursor(0, 0)?;

    rile.send("M-v", keys::meta('v'))?;
    rile.assert_status_contains("Ln 001 Col 000")?;
    rile.assert_screen_contains("000 | 0123456789")?;
    rile.assert_cursor(0, 0)?;

    rile.send("PageDown", keys::PAGE_DOWN)?;
    rile.assert_status_contains("Ln 006 Col 000")?;
    rile.send("PageUp", keys::PAGE_UP)?;
    rile.assert_status_contains("Ln 001 Col 000")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn recenter_keeps_point_and_moves_viewport() -> Result<()> {
    let file = fixtures::fixture_path("numbered.txt");
    let mut rile = RilePty::spawn(&file, 8, 60)?;

    rile.wait_for_screen_contains("000 | 0123456789")?;
    for _ in 0..12 {
        rile.send("C-n", keys::control('n'))?;
    }
    rile.assert_status_contains("Ln 013 Col 000")?;

    rile.send("C-l", keys::control('l'))?;
    rile.assert_status_contains("Ln 013 Col 000")?;
    rile.assert_screen_contains("009 | 0123456789")?;
    rile.assert_cursor(3, 0)?;

    rile.quit()?;
    Ok(())
}

#[test]
fn page_scroll_and_recenter_use_selected_split_height() -> Result<()> {
    let file = fixtures::fixture_path("numbered.txt");
    let mut rile = RilePty::spawn(&file, 12, 60)?;

    rile.wait_for_screen_contains("000 | 0123456789")?;
    rile.send("C-x 2", control_x_text("2"))?;
    rile.send("C-x o", control_x_text("o"))?;

    rile.send("C-v", keys::control('v'))?;
    rile.assert_status_contains("window 1 ACTIVE numbered.txt Ln 004 Col 000")?;
    rile.assert_screen_contains("003 | 0123456789")?;
    rile.assert_cursor(6, 0)?;

    rile.send("C-l", keys::control('l'))?;
    rile.assert_status_contains("window 1 ACTIVE numbered.txt Ln 004 Col 000")?;
    rile.assert_screen_contains("001 | 0123456789")?;
    rile.assert_cursor(8, 0)?;

    rile.quit()?;
    Ok(())
}

#[test]
fn horizontal_scrolling_keeps_cursor_visible_on_long_lines() -> Result<()> {
    let file = fixtures::fixture_path("long_lines.txt");
    let mut rile = RilePty::spawn(&file, 8, 32)?;

    rile.wait_for_screen_contains("000 | 0123456789")?;
    assert!(
        rile.screen_rows()[0].ends_with('$'),
        "long line should mark hidden right edge\n{}",
        rile.screen_dump()
    );
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
    assert!(
        rile.screen_rows()[0].starts_with('$'),
        "horizontally scrolled line should mark hidden left edge\n{}",
        rile.screen_dump()
    );

    rile.quit()?;
    Ok(())
}

#[test]
fn horizontal_scrolling_marks_both_hidden_edges() -> Result<()> {
    let file = fixtures::fixture_path("long_lines.txt");
    let mut rile = RilePty::spawn(&file, 8, 32)?;

    rile.wait_for_screen_contains("000 | 0123456789")?;
    for _ in 0..35 {
        rile.send("C-f", keys::control('f'))?;
    }

    let first_row = &rile.screen_rows()[0];
    assert!(
        first_row.starts_with('$') && first_row.ends_with('$'),
        "middle horizontal viewport should mark both hidden edges\n{}",
        rile.screen_dump()
    );

    rile.quit()?;
    Ok(())
}
