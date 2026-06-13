// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;

use support::{fixtures, keys, pty::RilePty};

#[test]
fn movement_demo_flow_updates_cursor_and_status() -> Result<()> {
    let file = fixtures::fixture_path("numbered.txt");
    let mut rile = RilePty::spawn(&file, 32, 120)?;

    rile.wait_for_screen_contains("numbered.txt")?;
    rile.assert_cursor(0, 0)?;
    rile.assert_status_contains("Ln 001 Col 000")?;

    rile.send("C-n", keys::control('n'))?;
    rile.assert_cursor(1, 0)?;
    rile.assert_status_contains("Ln 002 Col 000")?;

    rile.send("C-n", keys::control('n'))?;
    rile.assert_cursor(2, 0)?;
    rile.assert_status_contains("Ln 003 Col 000")?;

    rile.send("C-f", keys::control('f'))?;
    rile.assert_cursor(2, 1)?;
    rile.assert_status_contains("Ln 003 Col 001")?;

    rile.send("C-f", keys::control('f'))?;
    rile.assert_cursor(2, 2)?;
    rile.assert_status_contains("Ln 003 Col 002")?;

    rile.send("C-p", keys::control('p'))?;
    rile.assert_cursor(1, 2)?;
    rile.assert_status_contains("Ln 002 Col 002")?;

    rile.send("C-a", keys::control('a'))?;
    rile.assert_cursor(1, 0)?;
    rile.assert_status_contains("Ln 002 Col 000")?;

    rile.send("C-e", keys::control('e'))?;
    rile.assert_cursor(1, 49)?;
    rile.assert_status_contains("Ln 002 Col 049")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn movement_commands_cover_backward_arrows_and_words() -> Result<()> {
    let file = fixtures::named_temp_file("alpha beta\ngamma delta")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("alpha beta")?;
    rile.assert_cursor(0, 0)?;

    rile.send("Right", keys::RIGHT)?;
    rile.assert_cursor(0, 1)?;
    rile.assert_status_contains("Ln 001 Col 001")?;

    rile.send("Left", keys::LEFT)?;
    rile.assert_cursor(0, 0)?;
    rile.assert_status_contains("Ln 001 Col 000")?;

    rile.send("M-f", keys::meta('f'))?;
    rile.assert_cursor(0, 5)?;
    rile.assert_status_contains("Ln 001 Col 005")?;

    rile.send("M-f", keys::meta('f'))?;
    rile.assert_cursor(0, 10)?;
    rile.assert_status_contains("Ln 001 Col 010")?;

    rile.send("M-b", keys::meta('b'))?;
    rile.assert_cursor(0, 6)?;
    rile.assert_status_contains("Ln 001 Col 006")?;

    rile.send("M-b", keys::meta('b'))?;
    rile.assert_cursor(0, 0)?;
    rile.assert_status_contains("Ln 001 Col 000")?;

    rile.send("Down", keys::DOWN)?;
    rile.assert_cursor(1, 0)?;
    rile.assert_status_contains("Ln 002 Col 000")?;

    rile.send("Right", keys::RIGHT)?;
    rile.assert_cursor(1, 1)?;
    rile.assert_status_contains("Ln 002 Col 001")?;

    rile.send("Up", keys::UP)?;
    rile.assert_cursor(0, 1)?;
    rile.assert_status_contains("Ln 001 Col 001")?;

    rile.send("C-b", keys::control('b'))?;
    rile.assert_cursor(0, 0)?;
    rile.assert_status_contains("Ln 001 Col 000")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn goto_line_prompt_moves_to_line_and_column() -> Result<()> {
    let file = fixtures::named_temp_file("line 001\nline 002\nline 003\nline 004\nline 005 abc\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("line 001")?;
    let mut goto_line = keys::meta('g');
    goto_line.extend_from_slice(b"g");
    rile.send("M-g g", goto_line)?;
    rile.assert_screen_contains("Goto line:")?;

    rile.send("5:4", b"5:4")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_cursor(4, 4)?;
    rile.assert_status_contains("Ln 005 Col 004")?;

    rile.quit()?;
    Ok(())
}
