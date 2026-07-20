// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use std::{fs, time::Duration};

use anyhow::{Context, Result};

use support::{fixtures, keys, pty::RilePty};

#[test]
fn save_buffer_writes_disk_contents_and_clears_dirty_state() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let path = file.path().to_path_buf();
    let mut rile = RilePty::spawn(&path, 12, 80)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.assert_status_contains("modified:false")?;

    rile.send("insert text", b"saved ")?;
    rile.wait_for_screen_contains("saved alpha")?;
    rile.assert_status_contains("modified:true")?;

    rile.send("C-x C-s", keys::control_sequence("xs"))?;
    rile.wait_for_screen_contains("Wrote")?;
    rile.assert_status_contains("modified:false")?;

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read saved file {}", path.display()))?;
    assert_eq!(contents, "saved alpha\nbeta\n");

    rile.quit()?;
    Ok(())
}

#[test]
fn auto_save_writes_hash_file_from_loaded_config() -> Result<()> {
    let home = fixtures::temp_home()?;
    let config_dir = home.path().join(".config").join("rile");
    fs::create_dir_all(&config_dir)?;
    fs::write(
        config_dir.join("config.toml"),
        "auto_save = true\nauto_save_interval = 1\nauto_save_timeout_seconds = 0\n",
    )?;
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("notes.txt");
    fs::write(&path, "alpha\n")?;
    let auto_save = directory.path().join("#notes.txt#");
    let mut rile = RilePty::spawn_with_loaded_config(&path, 12, 80, home)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("insert text", b"saved ")?;
    rile.wait_for_screen_contains("Auto-saved")?;

    let contents = fs::read_to_string(&auto_save)
        .with_context(|| format!("failed to read auto-save file {}", auto_save.display()))?;
    assert_eq!(contents, "saved alpha\n");
    assert_eq!(fs::read_to_string(&path)?, "alpha\n");

    rile.quit()?;
    Ok(())
}

#[test]
fn write_file_saves_as_new_path_and_clears_dirty_state() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let target = tempfile::NamedTempFile::new().context("failed to create target file")?;
    let target_path = target.path().to_path_buf();
    let target_text = target_path.to_string_lossy().into_owned();
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("insert text", b"saved ")?;
    rile.wait_for_screen_contains("saved alpha")?;
    rile.assert_status_contains("modified:true")?;

    rile.send("C-x C-w", keys::control_sequence("xw"))?;
    rile.assert_screen_contains("Write file:")?;
    rile.send("target path", target_text.as_bytes())?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("Wrote")?;
    rile.assert_status_contains("modified:false")?;

    let contents = fs::read_to_string(&target_path)
        .with_context(|| format!("failed to read saved file {}", target_path.display()))?;
    assert_eq!(contents, "saved alpha\nbeta\n");

    rile.quit()?;
    Ok(())
}

#[test]
fn save_some_buffers_prompts_for_each_modified_file_buffer() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let first = directory.path().join("first.txt");
    let second = directory.path().join("second.txt");
    std::fs::write(&first, "first original\n")?;
    std::fs::write(&second, "second original\n")?;
    let mut rile = RilePty::spawn(&first, 12, 100)?;

    rile.wait_for_screen_contains("first original")?;
    rile.send("edit first", b"saved ")?;
    rile.wait_for_screen_contains("saved first original")?;

    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    let second_path = second.display().to_string();
    rile.send("second path", second_path.as_bytes())?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("second original")?;
    rile.send("edit second", b"unsaved ")?;
    rile.wait_for_screen_contains("unsaved second original")?;

    rile.send("C-x", keys::control('x'))?;
    rile.send("s", b"s")?;
    rile.assert_screen_contains("Save file first.txt?")?;
    rile.send("yes", b"yes")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Save file second.txt?")?;
    rile.send("no", b"no")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Saved 1 buffer")?;

    let first_contents = fs::read_to_string(&first)
        .with_context(|| format!("failed to read saved file {}", first.display()))?;
    let second_contents = fs::read_to_string(&second)
        .with_context(|| format!("failed to read skipped file {}", second.display()))?;
    assert_eq!(first_contents, "saved first original\n");
    assert_eq!(second_contents, "second original\n");

    rile.quit()?;
    Ok(())
}

#[test]
fn save_some_buffers_prompt_escapes_terminal_controls() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let path = directory
        .path()
        .join("name_\u{1b}]0;SAVE_SOME_PWN\u{7}.txt");
    std::fs::write(&path, "original\n")?;
    let mut rile = RilePty::spawn(&path, 12, 160)?;

    rile.wait_for_screen_contains("original")?;
    rile.send("edit file", b"modified ")?;
    rile.wait_for_screen_contains("modified original")?;
    rile.send("C-x", keys::control('x'))?;
    rile.send("s", b"s")?;

    rile.wait_for_screen_contains("Save file name_\\u{1b}]0;SAVE_SOME_PWN\\u{7}.txt? (yes or no)")?;
    rile.assert_raw_output_excludes(b"\x1b]0;SAVE_SOME_PWN\x07")?;

    rile.send("no", b"no")?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("No buffers saved")?;
    rile.assert_raw_output_excludes(b"\x1b]0;SAVE_SOME_PWN\x07")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn save_some_buffers_cancel_leaves_buffers_unsaved() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let first = directory.path().join("first-cancel.txt");
    let second = directory.path().join("second-cancel.txt");
    std::fs::write(&first, "first original\n")?;
    std::fs::write(&second, "second original\n")?;
    let mut rile = RilePty::spawn(&first, 12, 100)?;

    rile.wait_for_screen_contains("first original")?;
    rile.send("edit first", b"saved ")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    let second_path = second.display().to_string();
    rile.send("second path", second_path.as_bytes())?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("second original")?;
    rile.send("edit second", b"saved ")?;

    rile.send("C-x", keys::control('x'))?;
    rile.send("s", b"s")?;
    rile.assert_screen_contains("Save file first-cancel.txt?")?;
    rile.send("C-g", keys::control('g'))?;
    rile.assert_screen_contains("Quit")?;

    let first_contents = fs::read_to_string(&first)
        .with_context(|| format!("failed to read unsaved file {}", first.display()))?;
    let second_contents = fs::read_to_string(&second)
        .with_context(|| format!("failed to read unsaved file {}", second.display()))?;
    assert_eq!(first_contents, "first original\n");
    assert_eq!(second_contents, "second original\n");

    rile.send("C-x", keys::control('x'))?;
    rile.send("s", b"s")?;
    rile.assert_screen_contains("Save file first-cancel.txt?")?;
    rile.send("C-g", keys::control('g'))?;

    rile.quit()?;
    Ok(())
}

#[test]
fn save_some_buffers_skips_read_only_modified_buffers() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let read_only = directory.path().join("read-only.txt");
    let writable = directory.path().join("writable.txt");
    std::fs::write(&read_only, "read only original\n")?;
    std::fs::write(&writable, "writable original\n")?;
    let mut rile = RilePty::spawn(&read_only, 12, 100)?;

    rile.wait_for_screen_contains("read only original")?;
    rile.send("edit read-only", b"skipped ")?;
    rile.send("C-x C-q", keys::control_sequence("xq"))?;
    rile.assert_screen_contains("Buffer is now read-only")?;

    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    let writable_path = writable.display().to_string();
    rile.send("writable path", writable_path.as_bytes())?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("writable original")?;
    rile.send("edit writable", b"saved ")?;

    rile.send("C-x", keys::control('x'))?;
    rile.send("s", b"s")?;
    rile.assert_screen_contains("Save file writable.txt?")?;
    rile.send("yes", b"yes")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Saved 1 buffer")?;

    let read_only_contents = fs::read_to_string(&read_only)
        .with_context(|| format!("failed to read skipped file {}", read_only.display()))?;
    let writable_contents = fs::read_to_string(&writable)
        .with_context(|| format!("failed to read saved file {}", writable.display()))?;
    assert_eq!(read_only_contents, "read only original\n");
    assert_eq!(writable_contents, "saved writable original\n");

    rile.quit()?;
    Ok(())
}

#[test]
fn save_some_buffers_reports_save_failure_and_clears_prompt() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("vanished-parent.txt");
    std::fs::write(&file, "original\n")?;
    let mut rile = RilePty::spawn(&file, 12, 100)?;

    rile.wait_for_screen_contains("original")?;
    rile.send("edit file", b"changed ")?;
    std::fs::remove_dir_all(directory.path())?;

    rile.send("C-x", keys::control('x'))?;
    rile.send("s", b"s")?;
    rile.assert_screen_contains("Save file vanished-parent.txt?")?;
    rile.send("yes", b"yes")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Error: save failed:")?;

    rile.send("C-x", keys::control('x'))?;
    rile.send("s", b"s")?;
    rile.assert_screen_contains("Save file vanished-parent.txt?")?;
    rile.send("C-g", keys::control('g'))?;

    rile.quit()?;
    Ok(())
}

#[test]
fn insert_file_inserts_contents_at_point() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    let source = directory.path().join("source.txt");
    std::fs::write(&start, "before\nafter\n")?;
    std::fs::write(&source, "inserted line\n")?;
    let mut rile = RilePty::spawn(&start, 12, 100)?;

    rile.wait_for_screen_contains("before")?;
    rile.send("C-n", keys::control('n'))?;
    rile.send("C-x", keys::control('x'))?;
    rile.send("i", b"i")?;
    rile.assert_screen_contains("Insert file:")?;
    rile.send("source file", b"source.txt")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("inserted line")?;
    rile.assert_screen_contains("Inserted")?;
    rile.assert_status_contains("modified:true")?;

    rile.send("C-_", b"\x1f")?;
    if rile.snapshot_text().contains("inserted line") {
        anyhow::bail!("undo did not remove inserted file\n{}", rile.screen_dump());
    }

    rile.quit()?;
    Ok(())
}

#[test]
fn insert_file_rejects_oversized_input_and_remains_responsive() -> Result<()> {
    const OVERSIZED_INSERT_BYTES: u64 = 8 * 1024 * 1024 + 1;

    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    let source = directory.path().join("oversized.txt");
    fs::write(&start, "before\n")?;
    fs::File::create(&source)?.set_len(OVERSIZED_INSERT_BYTES)?;
    let mut rile = RilePty::spawn(&start, 12, 100)?;

    rile.wait_for_screen_contains("before")?;
    rile.send("C-x", keys::control('x'))?;
    rile.send("i", b"i")?;
    rile.assert_screen_contains("Insert file:")?;
    rile.send("oversized file", b"oversized.txt")?;
    rile.send("Enter", keys::ENTER)?;

    rile.wait_for_screen_contains_for("8388608-byte limit", Duration::from_secs(5))?;
    rile.assert_status_contains("modified:false")?;
    rile.send("post-rejection edit", b"safe ")?;
    rile.wait_for_screen_contains("safe before")?;

    rile.quit()?;
    Ok(())
}
