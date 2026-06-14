// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;
use std::fs;

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

#[test]
fn vertical_find_file_completion_filters_and_opens_sibling() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    let alpha = directory.path().join("alpha-note.txt");
    fs::write(&start, "start\n")?;
    fs::write(&alpha, "alpha note\n")?;
    fs::write(
        directory.path().join("alphabet-note.txt"),
        "alphabet note\n",
    )?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("alpha-n", b"alpha-n")?;
    rile.send("Tab", keys::TAB)?;

    rile.assert_screen_contains("alpha-note.txt")?;

    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("alpha note")?;
    rile.assert_status_contains("ACTIVE alpha-note.txt")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_completion_tab_extends_common_prefix() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::write(directory.path().join("alpha-note.txt"), "alpha note\n")?;
    fs::write(
        directory.path().join("alphabet-note.txt"),
        "alphabet note\n",
    )?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("alp", b"alp")?;
    rile.send("Tab", keys::TAB)?;

    rile.assert_screen_contains("Find file: alpha")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}
