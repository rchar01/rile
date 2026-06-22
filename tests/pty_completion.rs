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

    rile.assert_screen_contains("1/2  M-x toggle-s")?;
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
    rile.assert_screen_contains("2/2  M-x toggle-s")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("Syntax highlighting disabled")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_mx_completion_clips_long_visible_rows() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 10, 42)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-x", keys::meta('x'))?;
    rile.send("rectangle", b"rectangle")?;

    rile.assert_screen_contains("M-x rectangle")?;
    assert!(
        rile.screen_rows().iter().any(|row| row.ends_with('$')),
        "expected a clipped completion row\n{}",
        rile.screen_dump()
    );

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_mx_completion_shows_command_key_binding() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-x", keys::meta('x'))?;
    rile.send("save-b", b"save-b")?;

    rile.assert_screen_contains("save-buffer (C-x C-s)")?;
    rile.assert_screen_contains("Save current buffer")?;

    rile.send("C-g", keys::control('g'))?;
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
fn vertical_buffer_completion_enter_rejects_ambiguous_raw_input() -> Result<()> {
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
    rile.send("alpha", b"alpha")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("no such buffer: alpha")?;
    rile.assert_status_contains("ACTIVE alphabet-buffer.txt")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_kill_buffer_completion_tab_extends_and_kills() -> Result<()> {
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
    rile.send("k", b"k")?;
    rile.assert_screen_contains("Kill buffer (default alphabet-buffer.txt):")?;
    rile.send("alpha-b", b"alpha-b")?;
    rile.send("Tab", keys::TAB)?;

    rile.assert_screen_contains("Kill buffer (default alphabet-buffer.txt): alpha-buffer.txt")?;

    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("Killed buffer alpha-buffer.txt")?;
    rile.assert_status_contains("ACTIVE alphabet-buffer.txt")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_kill_buffer_completion_tab_accepts_selected_default() -> Result<()> {
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
    rile.send("k", b"k")?;
    rile.send("alpha", b"alpha")?;
    rile.send("Tab", keys::TAB)?;

    rile.assert_screen_contains("Kill buffer (default alphabet-buffer.txt): alphabet-buffer.txt")?;

    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("Killed buffer alphabet-buffer.txt")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_kill_buffer_completion_enter_accepts_selected_default() -> Result<()> {
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
    rile.send("k", b"k")?;
    rile.send("alpha", b"alpha")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("Killed buffer alphabet-buffer.txt")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_kill_buffer_completion_preserves_space_sensitive_exact_name() -> Result<()> {
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
    rile.send("k", b"k")?;
    rile.send("space-sensitive name", b" alpha-buffer.txt")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("Killed buffer  alpha-buffer.txt")?;
    rile.assert_status_contains("ACTIVE alphabet-buffer.txt")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_kill_buffer_empty_answer_kills_default() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    let alpha = directory.path().join("alpha-buffer.txt");
    fs::write(&start, "start\n")?;
    fs::write(&alpha, "alpha buffer\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    let alpha_path = alpha.display().to_string();
    rile.send("alpha-buffer path", alpha_path.as_bytes())?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("alpha buffer")?;

    rile.send("C-x", keys::control('x'))?;
    rile.send("k", b"k")?;
    rile.assert_screen_contains("Kill buffer (default alpha-buffer.txt):")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("Killed buffer alpha-buffer.txt")?;
    rile.assert_screen_contains("start")?;
    rile.assert_status_contains("ACTIVE start.txt")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_kill_buffer_completion_refuses_dirty_buffer() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    let dirty = directory.path().join("dirty-buffer.txt");
    fs::write(&start, "start\n")?;
    fs::write(&dirty, "dirty buffer\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    let dirty_path = dirty.display().to_string();
    rile.send("dirty-buffer path", dirty_path.as_bytes())?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("dirty buffer")?;
    rile.send("insert dirty marker", b"!")?;
    rile.assert_status_contains("modified:true")?;

    rile.send("C-x b", keys::control('x'))?;
    rile.send("b", b"b")?;
    rile.send("start", b"start")?;
    rile.send("Tab", keys::TAB)?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_status_contains("ACTIVE start.txt")?;

    rile.send("C-x", keys::control('x'))?;
    rile.send("k", b"k")?;
    rile.send("dirty", b"dirty")?;
    rile.send("Tab", keys::TAB)?;
    rile.assert_screen_contains("Kill buffer (default start.txt): dirty-buffer.txt")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("unsaved changes")?;
    rile.assert_status_contains("ACTIVE start.txt")?;

    rile.send("C-x C-c", keys::control_sequence("xc"))?;
    rile.wait_for_screen_contains("Modified buffers exist; exit anyway? (yes or no)")?;
    rile.send("yes", b"yes")?;
    rile.send("Enter", keys::ENTER)?;
    rile.expect_exit()?;
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
