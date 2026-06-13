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
