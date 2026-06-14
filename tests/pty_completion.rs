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

#[test]
fn vertical_buffer_completion_tab_extends_and_switches() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    let alpha = directory.path().join("alpha-buffer.txt");
    let alphabet = directory.path().join("alphabet-buffer.txt");
    fs::write(&start, "start\n")?;
    fs::write(&alpha, "alpha buffer\n")?;
    fs::write(&alphabet, "alphabet buffer\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    let alpha_path = alpha.display().to_string();
    rile.send("alpha-buffer path", alpha_path.as_bytes())?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("alpha buffer")?;

    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    let alphabet_path = alphabet.display().to_string();
    rile.send("alphabet-buffer path", alphabet_path.as_bytes())?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("alphabet buffer")?;

    rile.send("C-x", keys::control('x'))?;
    rile.send("b", b"b")?;
    rile.send("alpha-b", b"alpha-b")?;
    rile.send("Tab", keys::TAB)?;

    rile.assert_screen_contains("Switch to buffer: alpha-buffer.txt")?;

    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("alpha buffer")?;
    rile.assert_status_contains("ACTIVE alpha-buffer.txt")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_buffer_completion_preserves_space_sensitive_exact_name() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    let spaced = directory.path().join(" alpha-buffer.txt");
    let alphabet = directory.path().join("alphabet-buffer.txt");
    fs::write(&start, "start\n")?;
    fs::write(&spaced, "leading alpha buffer\n")?;
    fs::write(&alphabet, "alphabet buffer\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    let spaced_path = spaced.display().to_string();
    rile.send("spaced-buffer path", spaced_path.as_bytes())?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("leading alpha buffer")?;

    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    let alphabet_path = alphabet.display().to_string();
    rile.send("alphabet-buffer path", alphabet_path.as_bytes())?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("alphabet buffer")?;

    rile.send("C-x", keys::control('x'))?;
    rile.send("b", b"b")?;
    rile.send("space-sensitive name", b" alpha-buffer.txt")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("leading alpha buffer")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_mx_prompt_history_recalls_previous_command() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-x", keys::meta('x'))?;
    rile.send("toggle-line-numbers", b"toggle-line-numbers")?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("Line numbers enabled")?;

    rile.send("M-x", keys::meta('x'))?;
    rile.send("M-p", keys::meta('p'))?;

    rile.assert_screen_contains("M-x toggle-line-numbers")?;

    rile.send("M-n", keys::meta('n'))?;
    rile.assert_screen_contains("M-x")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}
