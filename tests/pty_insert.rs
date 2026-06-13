// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;

use support::{fixtures, keys, pty::RilePty};

#[test]
fn insert_delete_and_backspace_update_visible_buffer() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.assert_status_contains("Ln 001 Col 000")?;
    rile.assert_status_contains("modified:false")?;

    rile.send("insert ASCII", b"Z")?;
    rile.wait_for_screen_contains("Zalpha")?;
    rile.assert_status_contains("Ln 001 Col 001")?;
    rile.assert_status_contains("modified:true")?;

    rile.send("insert UTF-8", "é".as_bytes())?;
    rile.wait_for_screen_contains("Zéalpha")?;
    rile.assert_status_contains("Ln 001 Col 002")?;

    rile.send("RET", keys::ENTER)?;
    rile.wait_for_screen_contains("Zé")?;
    rile.assert_screen_contains("alpha")?;
    rile.assert_status_contains("Ln 002 Col 000")?;

    rile.send("Backspace", keys::BACKSPACE)?;
    rile.wait_for_screen_contains("Zéalpha")?;
    rile.assert_status_contains("Ln 001 Col 002")?;

    rile.send("Delete", keys::DELETE)?;
    rile.wait_for_screen_contains("Zélpha")?;
    rile.assert_status_contains("Ln 001 Col 002")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn open_line_keeps_cursor_and_shifts_text_down() -> Result<()> {
    let file = fixtures::named_temp_file("alpha beta gamma\nsecond line\nthird line\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("alpha beta gamma")?;
    for _ in 0..5 {
        rile.send("C-f", keys::control('f'))?;
    }
    rile.assert_cursor(0, 5)?;

    rile.send("C-o", keys::control('o'))?;
    rile.wait_for_screen_contains(" beta gamma")?;

    rile.assert_screen_contains("alpha")?;
    rile.assert_screen_contains("second line")?;
    rile.assert_screen_contains("third line")?;
    rile.assert_cursor(0, 5)?;
    rile.assert_status_contains("Ln 001 Col 005")?;
    rile.assert_status_contains("modified:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn exchange_point_and_mark_keeps_region_active() -> Result<()> {
    let file = fixtures::named_temp_file("abcdef\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("abcdef")?;
    rile.send("C-f", keys::control('f'))?;
    rile.send("C-f", keys::control('f'))?;
    rile.send("C-@", b"\0")?;
    rile.send("C-f", keys::control('f'))?;
    rile.send("C-f", keys::control('f'))?;
    rile.assert_cursor(0, 4)?;

    rile.send("C-x C-x", keys::control_sequence("xx"))?;
    rile.assert_cursor(0, 2)?;

    rile.send("C-w", keys::control('w'))?;
    rile.assert_screen_contains("abef")?;
    rile.assert_status_contains("modified:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn kill_word_and_backward_kill_word_edit_visible_buffer() -> Result<()> {
    let file = fixtures::named_temp_file("one two three")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("one two three")?;
    rile.send("M-d", keys::meta('d'))?;
    rile.assert_screen_contains("two three")?;
    rile.assert_cursor(0, 0)?;

    rile.send("M->", keys::meta('>'))?;
    rile.assert_cursor(0, 10)?;
    rile.send("M-Backspace", keys::meta_backspace())?;
    rile.assert_screen_contains("two")?;
    assert!(!rile.snapshot_text().contains("three"));
    rile.assert_status_contains("modified:true")?;

    rile.quit()?;
    Ok(())
}
