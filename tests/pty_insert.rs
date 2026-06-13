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
