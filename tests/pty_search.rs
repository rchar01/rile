// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;

use support::{fixtures, keys, pty::RilePty};

#[test]
fn incremental_search_wraps_after_boundary_failure() -> Result<()> {
    let file = fixtures::named_temp_file("one\ntwo\none\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("one")?;
    rile.send("C-s", keys::control('s'))?;
    rile.send("one", b"one")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_status_contains("Ln 003 Col 000")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_screen_contains("Failing I-search: one")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_screen_contains("Wrapped I-search: one")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_cursor(0, 0)?;

    rile.send("C-r", keys::control('r'))?;
    rile.send("one", b"one")?;
    rile.assert_screen_contains("Failing I-search backward: one")?;
    rile.send("C-r", keys::control('r'))?;
    rile.assert_screen_contains("Wrapped I-search backward: one")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_cursor(2, 0)?;

    rile.quit()?;
    Ok(())
}
