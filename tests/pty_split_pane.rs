// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;

use support::{fixtures, keys, pty::RilePty};

#[test]
fn split_pane_demo_flow_opens_file_in_other_window() -> Result<()> {
    let left = fixtures::fixture_path("split_left.txt");
    let right = fixtures::fixture_path("split_right.txt");
    let mut rile = RilePty::spawn(&left, 32, 120)?;

    rile.wait_for_screen_contains("split_left.txt")?;
    rile.assert_screen_contains("left 000 | alpha")?;
    rile.assert_status_contains("ACTIVE split_left.txt Ln 001 Col 000")?;

    let mut split_right = keys::control('x').to_vec();
    split_right.extend_from_slice(b"3");
    rile.send("C-x 3", split_right)?;
    rile.assert_screen_contains("inactive split_left.txt")?;
    rile.assert_status_contains("window 0 ACTIVE split_left.txt")?;

    let mut other_window = keys::control('x').to_vec();
    other_window.extend_from_slice(b"o");
    rile.send("C-x o", other_window)?;
    rile.assert_status_contains("window 1 ACTIVE split_left.txt")?;

    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("find-file path", right.to_string_lossy().as_bytes())?;
    rile.send("RET", keys::ENTER)?;
    rile.wait_for_screen_contains("right 000 | one")?;
    rile.assert_screen_contains("left 000 | alpha")?;
    rile.assert_screen_contains("inactive split_left.txt")?;
    rile.assert_status_contains("window 1 ACTIVE split_right.txt Ln 001 Col 00")?;

    rile.send("C-n", keys::control('n'))?;
    rile.assert_status_contains("window 1 ACTIVE split_right.txt Ln 002 Col 00")?;

    rile.send("C-n", keys::control('n'))?;
    rile.assert_status_contains("window 1 ACTIVE split_right.txt Ln 003 Col 00")?;

    rile.quit()?;
    Ok(())
}
