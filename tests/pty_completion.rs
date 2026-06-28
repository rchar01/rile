// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;
use std::fs;

use support::{fixtures, keys, pty::RilePty};

fn assert_prompt_counter(rile: &RilePty, selected: usize, prompt: &str) -> Result<()> {
    rile.assert_screen_contains(&format!("{selected}/"))?;
    rile.assert_screen_contains(prompt)
}

fn exercise_prompt_movement_keys(rile: &mut RilePty, prompt: &str) -> Result<()> {
    assert_prompt_counter(rile, 1, prompt)?;

    rile.send("C-n", keys::control('n'))?;
    assert_prompt_counter(rile, 2, prompt)?;

    rile.send("C-p", keys::control('p'))?;
    assert_prompt_counter(rile, 1, prompt)?;

    rile.send("C-v", keys::control('v'))?;
    assert_prompt_counter(rile, 9, prompt)?;

    rile.send("M-v", keys::meta('v'))?;
    assert_prompt_counter(rile, 1, prompt)
}

fn open_file_by_path(rile: &mut RilePty, path: &std::path::Path, visible_text: &str) -> Result<()> {
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    let path = path.display().to_string();
    rile.send("file path", path.as_bytes())?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains(visible_text)
}

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
fn vertical_mx_empty_input_accepts_selected_command() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-x", keys::meta('x'))?;
    rile.assert_screen_contains("about-rile")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("About Rile:")?;

    rile.send("q", b"q")?;
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
fn vertical_completion_movement_keys_cover_prompt_sources() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    for index in 0..10 {
        fs::write(
            directory.path().join(format!("alpha-{index:02}.txt")),
            format!("alpha {index}\n"),
        )?;
        fs::write(
            directory.path().join(format!("buffer-{index:02}.txt")),
            format!("buffer {index}\n"),
        )?;
    }

    let mut rile = RilePty::spawn(&start, 18, 100)?;
    rile.wait_for_screen_contains("start")?;

    rile.send("M-x", keys::meta('x'))?;
    exercise_prompt_movement_keys(&mut rile, "M-x")?;
    rile.send("C-g", keys::control('g'))?;

    rile.send("C-h", keys::control('h'))?;
    rile.send("f", b"f")?;
    exercise_prompt_movement_keys(&mut rile, "Describe function:")?;
    rile.send("C-g", keys::control('g'))?;

    rile.send("C-h", keys::control('h'))?;
    rile.send("v", b"v")?;
    exercise_prompt_movement_keys(&mut rile, "Describe variable:")?;
    rile.send("C-g", keys::control('g'))?;

    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("alpha prefix", b"alpha-")?;
    exercise_prompt_movement_keys(&mut rile, "Find file: alpha-")?;
    rile.send("C-g", keys::control('g'))?;

    for index in 0..10 {
        let path = directory.path().join(format!("buffer-{index:02}.txt"));
        open_file_by_path(&mut rile, &path, &format!("buffer {index}"))?;
    }

    rile.send("C-x b", keys::control('x'))?;
    rile.send("b", b"b")?;
    exercise_prompt_movement_keys(&mut rile, "Switch to buffer")?;
    rile.send("C-g", keys::control('g'))?;

    rile.send("C-x", keys::control('x'))?;
    rile.send("k", b"k")?;
    exercise_prompt_movement_keys(&mut rile, "Kill buffer")?;
    rile.send("C-g", keys::control('g'))?;

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
fn vertical_mx_completion_matches_simple_anchor() -> Result<()> {
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
fn vertical_describe_function_empty_input_accepts_selection() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("C-h", keys::control('h'))?;
    rile.send("f", b"f")?;
    rile.send("Down", keys::DOWN)?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("is an interactive command.")?;
    if rile.snapshot_text().contains("No such command:") {
        anyhow::bail!(
            "empty selected command was rejected\n{}",
            rile.screen_dump()
        );
    }

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
fn vertical_describe_variable_empty_input_accepts_selection() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("C-h", keys::control('h'))?;
    rile.send("v", b"v")?;
    rile.send("Down", keys::DOWN)?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("is a configuration variable.")?;
    if rile.snapshot_text().contains("No such variable:") {
        anyhow::bail!("empty selected option was rejected\n{}", rile.screen_dump());
    }

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
fn vertical_find_file_empty_input_accepts_selected_file() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::write(directory.path().join("000-target.txt"), "target\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.assert_screen_contains("000-target.txt")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("target")?;
    rile.assert_status_contains("ACTIVE 000-target.txt")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_empty_input_enters_selected_directory() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::create_dir(directory.path().join("aaa-dir"))?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("Find file: aaa-dir/")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_empty_input_enters_selected_child_directory() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    let parent = directory.path().join("aaa-dir");
    fs::write(&start, "start\n")?;
    fs::create_dir(&parent)?;
    fs::create_dir(parent.join("child-dir"))?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Find file: aaa-dir/")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("Find file: aaa-dir/child-dir/")?;
    assert!(!rile.snapshot_text().contains("Is a directory"));

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_directory_prefix_enters_selected_child_directory() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    let parent = directory.path().join("aaa-dir");
    fs::write(&start, "start\n")?;
    fs::create_dir(&parent)?;
    fs::create_dir(parent.join("child-dir"))?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("aaa", b"aaa")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Find file: aaa-dir/")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("Find file: aaa-dir/child-dir/")?;
    assert!(!rile.snapshot_text().contains("Is a directory"));

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
fn vertical_find_file_completion_accepts_substring_match() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::write(directory.path().join("NOTICE.md"), "notice\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("tice", b"tice")?;
    rile.assert_screen_contains("NOTICE.md")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("notice")?;
    rile.assert_status_contains("ACTIVE NOTICE.md")?;

    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("tice", b"tice")?;
    rile.send("M-RET", keys::meta_enter())?;

    rile.assert_status_contains("ACTIVE tice")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn vertical_find_file_completion_matches_space_components() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::write(directory.path().join("README.md"), "readme\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("re me", b"re me")?;
    rile.assert_screen_contains("README.md")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("readme")?;
    rile.assert_status_contains("ACTIVE README.md")?;

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
fn vertical_find_file_read_only_empty_input_accepts_selected_file() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::write(directory.path().join("000-target.txt"), "target\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-r", keys::control_sequence("xr"))?;
    rile.assert_screen_contains("000-target.txt")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("target")?;
    rile.assert_status_contains("ACTIVE 000-target.txt")?;
    rile.assert_status_contains("ro:true")?;

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
fn vertical_insert_file_empty_input_accepts_selected_file() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    fs::write(&start, "start\n")?;
    fs::write(directory.path().join("000-source.txt"), "inserted\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x", keys::control('x'))?;
    rile.send("i", b"i")?;
    rile.assert_screen_contains("000-source.txt")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("inserted")?;
    rile.assert_screen_contains("start")?;

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

#[test]
fn vertical_prompt_history_meta_keys_cover_completion_prompt_sources() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    let alpha = directory.path().join("alpha-history.txt");
    fs::write(&start, "start\n")?;
    fs::write(&alpha, "alpha history\n")?;
    let mut rile = RilePty::spawn(&start, 14, 100)?;

    rile.wait_for_screen_contains("start")?;

    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("alpha-history", b"alpha-history.txt")?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("alpha history")?;

    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("file draft", b"draft-file")?;
    rile.send("M-p", keys::meta('p'))?;
    rile.assert_screen_contains("Find file: alpha-history.txt")?;
    rile.send("M-n", keys::meta('n'))?;
    rile.assert_screen_contains("Find file: draft-file")?;
    rile.send("C-g", keys::control('g'))?;

    rile.send("C-h", keys::control('h'))?;
    rile.send("f", b"f")?;
    rile.send("find-file", b"find-file")?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("find-file is an interactive command.")?;
    rile.send("q", b"q")?;

    rile.send("C-h", keys::control('h'))?;
    rile.send("f", b"f")?;
    rile.send("function draft", b"toggle")?;
    rile.send("M-p", keys::meta('p'))?;
    rile.assert_screen_contains("Describe function: find-file")?;
    rile.send("M-n", keys::meta('n'))?;
    rile.assert_screen_contains("Describe function: toggle")?;
    rile.send("C-g", keys::control('g'))?;

    rile.send("C-h", keys::control('h'))?;
    rile.send("v", b"v")?;
    rile.send("completion_style", b"completion_style")?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("completion_style is a configuration variable.")?;
    rile.send("q", b"q")?;

    rile.send("C-h", keys::control('h'))?;
    rile.send("v", b"v")?;
    rile.send("variable draft", b"completion")?;
    rile.send("M-p", keys::meta('p'))?;
    rile.assert_screen_contains("Describe variable: completion_style")?;
    rile.send("M-n", keys::meta('n'))?;
    rile.assert_screen_contains("Describe variable: completion")?;
    rile.send("C-g", keys::control('g'))?;

    rile.send("C-x b", keys::control('x'))?;
    rile.send("b", b"b")?;
    rile.send("start buffer", b"start.txt")?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("start")?;

    rile.send("C-x b", keys::control('x'))?;
    rile.send("b", b"b")?;
    rile.send("buffer draft", b"draft-buffer")?;
    rile.send("M-p", keys::meta('p'))?;
    rile.assert_screen_contains("Switch to buffer (default alpha-history.txt): start.txt")?;
    rile.send("M-n", keys::meta('n'))?;
    rile.assert_screen_contains("Switch to buffer (default alpha-history.txt): draft-buffer")?;
    rile.send("C-g", keys::control('g'))?;

    rile.quit()?;
    Ok(())
}
