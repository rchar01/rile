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
