// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;

use support::{fixtures, keys, pty::RilePty};

#[test]
fn statusline_tracks_visual_state_positions_save_and_errors() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.assert_status_contains("Rile VISUAL")?;
    rile.assert_status_contains("window 0 ACTIVE")?;
    rile.assert_status_contains("Ln 001 Col 000")?;
    rile.assert_status_contains("modified:false")?;
    rile.assert_cursor(0, 0)?;

    rile.send("C-n", keys::control('n'))?;
    rile.assert_status_contains("Ln 002 Col 000")?;
    rile.assert_cursor(1, 0)?;

    rile.send("C-f", keys::control('f'))?;
    rile.assert_status_contains("Ln 002 Col 001")?;
    rile.assert_cursor(1, 1)?;

    rile.send("insert dirty marker", b"!")?;
    rile.wait_for_screen_contains("b!eta")?;
    rile.assert_status_contains("Ln 002 Col 002")?;
    rile.assert_status_contains("modified:true")?;

    rile.send("C-x C-s", keys::control_sequence("xs"))?;
    rile.wait_for_screen_contains("Wrote")?;
    rile.assert_status_contains("Ln 002 Col 002")?;
    rile.assert_status_contains("modified:false")?;

    rile.send("C-w without mark", keys::control('w'))?;
    rile.wait_for_screen_contains("Error: no active region")?;
    rile.assert_status_contains("modified:false")?;

    rile.quit()?;
    Ok(())
}
