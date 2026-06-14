// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;

use support::{fixtures, keys, pty::RilePty};

#[test]
fn vertical_mx_completion_filters_and_accepts_command() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-x", keys::meta('x'))?;
    rile.send("toggle-s", b"toggle-s")?;

    rile.assert_screen_contains("toggle-search-highlighting")?;
    rile.assert_screen_contains("Toggle search highlighting")?;
    rile.assert_screen_contains("toggle-syntax-highlighting")?;

    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("Search highlighting disabled")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_mx_completion_selection_moves_with_down() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-x", keys::meta('x'))?;
    rile.send("toggle-s", b"toggle-s")?;
    rile.send("Down", keys::DOWN)?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("Syntax highlighting disabled")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_mx_completion_keeps_minibuffer_visible_in_tiny_terminal() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 5, 80)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-x", keys::meta('x'))?;
    rile.send("toggle-s", b"toggle-s")?;

    rile.assert_screen_contains("toggle-search-highlighting")?;
    rile.assert_screen_contains("M-x toggle-s")?;

    rile.send("C-g", keys::control('g'))?;
    rile.assert_screen_contains("Quit")?;

    rile.quit()?;
    Ok(())
}
