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
    let rows = rile.screen_rows();
    let prompt_row = rows
        .iter()
        .position(|row| row.contains("M-x toggle-s"))
        .expect("prompt row should be visible");
    let prompt_column = rows[prompt_row]
        .find("M-x toggle-s")
        .expect("prompt input should be visible")
        + "M-x toggle-s".chars().count();
    assert_eq!(
        rile.cursor_position(),
        (prompt_row as u16, prompt_column as u16)
    );
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
fn vertical_mx_completion_selection_moves_with_control_keys() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-x", keys::meta('x'))?;
    rile.send("toggle-s", b"toggle-s")?;
    rile.send("C-n", keys::control('n'))?;
    rile.assert_screen_contains("2/2  M-x toggle-s")?;
    rile.send("C-p", keys::control('p'))?;
    rile.assert_screen_contains("1/2  M-x toggle-s")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_mx_completion_pages_with_page_keys() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-x", keys::meta('x'))?;
    rile.send("toggle-s", b"toggle-s")?;
    rile.send("C-v", keys::control('v'))?;
    rile.assert_screen_contains("2/2  M-x toggle-s")?;
    rile.send("M-v", keys::meta('v'))?;
    rile.assert_screen_contains("1/2  M-x toggle-s")?;

    rile.send("PageDown", keys::PAGE_DOWN)?;
    rile.assert_screen_contains("2/2  M-x toggle-s")?;
    rile.send("PageUp", keys::PAGE_UP)?;
    rile.assert_screen_contains("1/2  M-x toggle-s")?;
    rile.send("PageDown", keys::PAGE_DOWN)?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Syntax highlighting disabled")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_mx_completion_tab_inserts_selected_command() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-x", keys::meta('x'))?;
    rile.send("find-file", b"find-file")?;
    rile.send("Down", keys::DOWN)?;
    rile.send("Tab", keys::TAB)?;

    rile.assert_screen_contains("M-x find-file-read-only")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_mx_completion_matches_orderless_components() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-x", keys::meta('x'))?;
    rile.send("find file", b"find file")?;

    rile.assert_screen_contains("find-file")?;
    rile.assert_screen_contains("find-file-read-only")?;

    rile.send("C-g", keys::control('g'))?;
    rile.send("M-x", keys::meta('x'))?;
    rile.send("file find", b"file find")?;

    rile.assert_screen_contains("find-file")?;
    rile.assert_screen_contains("find-file-read-only")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_mx_completion_matches_regex_anchor() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-x", keys::meta('x'))?;
    rile.send("^find", b"^find")?;

    rile.assert_screen_contains("find-file")?;
    rile.assert_screen_contains("find-file-read-only")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_mx_completion_enter_accepts_explicit_exact_selection() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-x", keys::meta('x'))?;
    rile.send("find-file", b"find-file")?;
    rile.send("Down", keys::DOWN)?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("Find file read-only:")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_describe_function_completion_accepts_explicit_selection() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("C-h", keys::control('h'))?;
    rile.send("f", b"f")?;
    rile.send("find-file", b"find-file")?;
    rile.send("Down", keys::DOWN)?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("find-file-read-only is an interactive command.")?;

    rile.send("q", b"q")?;
    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_describe_function_completion_tab_inserts_selected_command() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("C-h", keys::control('h'))?;
    rile.send("f", b"f")?;
    rile.send("find-file", b"find-file")?;
    rile.send("Down", keys::DOWN)?;
    rile.send("Tab", keys::TAB)?;

    rile.assert_screen_contains("Describe function: find-file-read-only")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_describe_variable_completion_tab_inserts_selected_option() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("C-h", keys::control('h'))?;
    rile.send("v", b"v")?;
    rile.send("completion", b"completion")?;
    rile.send("Down", keys::DOWN)?;
    rile.send("Tab", keys::TAB)?;

    rile.assert_screen_contains("Describe variable: completion_matching")?;

    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("completion_matching is a configuration variable.")?;

    rile.send("q", b"q")?;
    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_describe_variable_completion_accepts_explicit_selection() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("C-h", keys::control('h'))?;
    rile.send("v", b"v")?;
    rile.send("completion", b"completion")?;
    rile.send("Down", keys::DOWN)?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("completion_matching is a configuration variable.")?;

    rile.send("q", b"q")?;
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
fn vertical_find_file_completion_tab_inserts_selected_file() -> Result<()> {
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

    rile.assert_screen_contains("Find file: alpha-note.txt")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_completion_tab_inserts_partial_file_match() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::write(directory.path().join("alpha-note.txt"), "alpha note\n")?;
    fs::write(directory.path().join("alphabet.txt"), "alphabet\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("a-n", b"a-n")?;
    rile.send("Tab", keys::TAB)?;

    rile.assert_screen_contains("Find file: alpha-note.txt")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_completion_enter_accepts_word_component_match() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::write(directory.path().join("project-note.txt"), "project note\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("note", b"note")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("project note")?;
    rile.assert_status_contains("ACTIVE project-note.txt")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_completion_uses_smart_case_and_meta_enter_raw_input() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::write(directory.path().join("README.md"), "upper readme\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("readme.md", b"readme.md")?;
    rile.assert_screen_contains("README.md")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("upper readme")?;
    rile.assert_status_contains("ACTIVE README.md")?;

    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("readme.md", b"readme.md")?;
    rile.send("M-RET", keys::meta_enter())?;

    rile.assert_status_contains("ACTIVE readme.md")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_completion_keeps_raw_arbitrary_substring_input() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::write(directory.path().join("project-note.txt"), "project note\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("ote", b"ote")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_status_contains("ACTIVE ote")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_completion_keeps_raw_input_with_non_prefix_directory_match() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::create_dir(directory.path().join("alpha-note-dir"))?;
    fs::write(directory.path().join("beta-note.txt"), "beta note\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("note", b"note")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_status_contains("ACTIVE note")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_completion_enter_accepts_explicit_exact_selection() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::write(directory.path().join("alpha-note.txt"), "alpha note\n")?;
    fs::write(
        directory.path().join("alpha-note.txt-extra"),
        "alpha note extra\n",
    )?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("alpha-note.txt", b"alpha-note.txt")?;
    rile.send("Down", keys::DOWN)?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("alpha note extra")?;
    rile.assert_status_contains("ACTIVE alpha-note.txt-extra")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_completion_tab_enters_directory() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    let nested = directory.path().join("nested-dir");
    fs::create_dir(&nested)?;
    fs::write(&start, "start\n")?;
    fs::write(nested.join("note.txt"), "nested note\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("nested-dir", b"nested-dir")?;
    rile.send("Tab", keys::TAB)?;

    rile.assert_screen_contains("Find file: nested-dir/")?;

    rile.send("note.txt", b"note.txt")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("nested note")?;
    rile.assert_status_contains("ACTIVE note.txt")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_completion_keeps_raw_missing_file_input() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("new-note.txt", b"new-note.txt")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_status_contains("ACTIVE new-note.txt")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_read_only_completion_tab_inserts_selected_file() -> Result<()> {
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
    rile.send("C-x C-r", keys::control_sequence("xr"))?;
    rile.send("alp", b"alp")?;
    rile.send("Tab", keys::TAB)?;

    rile.assert_screen_contains("Find file read-only: alpha-note.txt")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_read_only_completion_accepts_explicit_selection() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::write(directory.path().join("alpha-note.txt"), "alpha note\n")?;
    fs::write(
        directory.path().join("alpha-note.txt-extra"),
        "alpha note extra\n",
    )?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-r", keys::control_sequence("xr"))?;
    rile.send("alpha-note.txt", b"alpha-note.txt")?;
    rile.send("Down", keys::DOWN)?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("alpha note extra")?;
    rile.assert_status_contains("ACTIVE alpha-note.txt-extra")?;
    rile.assert_status_contains("ro:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_read_only_completion_enter_accepts_prefix_match() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::write(directory.path().join("project-note.txt"), "project note\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-r", keys::control_sequence("xr"))?;
    rile.send("project", b"project")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("project note")?;
    rile.assert_status_contains("ACTIVE project-note.txt")?;
    rile.assert_status_contains("ro:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_insert_file_completion_tab_inserts_selected_file() -> Result<()> {
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
    rile.send("C-x", keys::control('x'))?;
    rile.send("i", b"i")?;
    rile.send("alp", b"alp")?;
    rile.send("Tab", keys::TAB)?;

    rile.assert_screen_contains("Insert file: alpha-note.txt")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_insert_file_completion_accepts_explicit_selection() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::write(directory.path().join("alpha-note.txt"), "alpha note\n")?;
    fs::write(
        directory.path().join("alpha-note.txt-extra"),
        "alpha note extra\n",
    )?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x", keys::control('x'))?;
    rile.send("i", b"i")?;
    rile.send("alpha-note.txt", b"alpha-note.txt")?;
    rile.send("Down", keys::DOWN)?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("alpha note extra")?;
    rile.assert_screen_contains("start")?;
    rile.assert_status_contains("modified:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_insert_file_completion_enter_accepts_prefix_match() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::write(directory.path().join("project-note.txt"), "project note\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x", keys::control('x'))?;
    rile.send("i", b"i")?;
    rile.send("project", b"project")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("project note")?;
    rile.assert_screen_contains("start")?;
    rile.assert_status_contains("modified:true")?;

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

    rile.assert_screen_contains("Switch to buffer (default alpha-buffer.txt): alpha-buffer.txt")?;

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
fn vertical_buffer_completion_enter_accepts_selected_default() -> Result<()> {
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

    rile.assert_screen_contains("alpha buffer")?;
    rile.assert_status_contains("ACTIVE alpha-buffer.txt")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_buffer_completion_empty_answer_switches_default() -> Result<()> {
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

    rile.assert_screen_contains("Switch to buffer (default alpha-buffer.txt):")?;

    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("alpha buffer")?;
    rile.assert_status_contains("ACTIVE alpha-buffer.txt")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_buffer_completion_tab_accepts_selected_default() -> Result<()> {
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
    rile.send("Tab", keys::TAB)?;

    rile.assert_screen_contains("Switch to buffer (default alpha-buffer.txt): alpha-buffer.txt")?;

    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("alpha buffer")?;
    rile.assert_status_contains("ACTIVE alpha-buffer.txt")?;

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

    rile.assert_screen_contains("Kill buffer (default alphabet-buffer.txt): alpha-buffer.txt")?;

    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("Killed buffer alpha-buffer.txt")?;

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

    rile.assert_screen_contains("Killed buffer alpha-buffer.txt")?;

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
fn vertical_kill_buffer_completion_confirms_dirty_buffer() -> Result<()> {
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

    rile.assert_screen_contains("Buffer dirty-buffer.txt modified; kill anyway? (y or n)")?;
    rile.assert_status_contains("ACTIVE start.txt")?;
    rile.send("y", b"y")?;

    rile.assert_screen_contains("Buffer dirty-buffer.txt modified; kill anyway? (y or n) y")?;
    rile.assert_status_contains("ACTIVE start.txt")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_kill_buffer_completion_cancels_dirty_buffer() -> Result<()> {
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
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Buffer dirty-buffer.txt modified; kill anyway? (y or n)")?;

    rile.send("n", b"n")?;
    rile.assert_screen_contains("Quit")?;
    rile.assert_status_contains("ACTIVE start.txt")?;

    rile.send("C-x b", keys::control('x'))?;
    rile.send("b", b"b")?;
    rile.send("dirty", b"dirty")?;
    rile.send("Tab", keys::TAB)?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_status_contains("ACTIVE dirty-buffer.txt")?;
    rile.assert_status_contains("modified:true")?;

    rile.send("C-x C-s", keys::control_sequence("xs"))?;
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
