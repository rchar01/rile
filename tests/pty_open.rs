// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;
use predicates::prelude::*;

use support::{fixtures, keys, pty::RilePty};

#[test]
fn opens_visual_fixture_in_pty() -> Result<()> {
    let file = fixtures::fixture_path("numbered.txt");
    let mut rile = RilePty::spawn(&file, 10, 50)?;

    rile.wait_for_screen_contains("numbered.txt")?;
    rile.assert_screen_contains("000 | 0123456789")?;
    assert!(
        predicate::str::contains("Rile VISUAL").eval(&rile.snapshot_text()),
        "{}",
        rile.screen_dump()
    );
    rile.assert_status_contains("Rile VISUAL")?;
    rile.assert_status_contains("ACTIVE numbered.txt")?;
    rile.assert_cursor(0, 0)?;
    insta::assert_snapshot!(format!("{:?}", rile.cursor_position()), @r"(0, 0)");
    rile.quit()?;

    Ok(())
}

#[test]
fn escapes_terminal_controls_from_file_name_and_contents() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let file = directory.path().join("name_\u{1b}]0;FILE_PWN\u{7}.txt");
    std::fs::write(&file, "body_\u{1b}]0;BODY_PWN\u{7}\r\n")?;
    let mut rile = RilePty::spawn(&file, 8, 160)?;

    rile.wait_for_screen_contains("body_\\u{1b}]0;BODY_PWN\\u{7}\\r")?;
    rile.assert_status_contains("name_\\u{1b}]0;FILE_PWN\\u{7}.txt")?;

    rile.quit()?;
    rile.assert_raw_output_excludes(b"\x1b]0;FILE_PWN\x07")?;
    rile.assert_raw_output_excludes(b"\x1b]0;BODY_PWN\x07")?;
    Ok(())
}

#[test]
fn opened_message_escapes_sgr_from_file_name_across_redraw() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    let hostile = directory.path().join("conceal_\u{1b}[8m.txt");
    std::fs::write(&start, "start\n")?;
    std::fs::write(&hostile, "VISIBLE AFTER REDRAW\n")?;
    let mut rile = RilePty::spawn(&start, 10, 180)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    rile.send("file prefix", b"conceal_")?;
    rile.wait_for_screen_contains("conceal_\\u{1b}[8m.txt")?;
    rile.send("Tab", keys::TAB)?;
    rile.send("Enter", keys::ENTER)?;

    rile.wait_for_screen_contains("Opened ")?;
    let opened_row = rile
        .screen_rows()
        .into_iter()
        .find(|row| row.contains("Opened "))
        .unwrap_or_default();
    assert!(
        opened_row.contains("conceal_\\u{1b}[8m.txt"),
        "opened-file message should display the escaped SGR\n{}",
        rile.screen_dump()
    );
    rile.assert_raw_output_excludes(b"\x1b[8m")?;

    rile.send("C-f", keys::control('f'))?;
    rile.wait_for_screen_contains("VISIBLE AFTER REDRAW")?;
    assert!(
        !rile.snapshot_text().contains("Opened "),
        "movement should clear the opened-file message\n{}",
        rile.screen_dump()
    );
    rile.assert_raw_output_excludes(b"\x1b[8m")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn opens_file_read_only_and_blocks_editing() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let start = directory.path().join("start.txt");
    let target = directory.path().join("target.txt");
    std::fs::write(&start, "start\n")?;
    std::fs::write(&target, "read-only target\n")?;
    let mut rile = RilePty::spawn(&start, 12, 100)?;

    rile.wait_for_screen_contains("start")?;
    rile.send("C-x", keys::control('x'))?;
    rile.send("C-r", keys::control('r'))?;
    rile.assert_screen_contains("Find file read-only:")?;
    rile.send("target.txt", b"target.txt")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("read-only target")?;
    rile.assert_screen_contains("Opened read-only")?;

    rile.send("x", b"x")?;
    rile.assert_screen_contains("Buffer is read-only:")?;
    if rile.snapshot_text().contains("xread-only target") {
        anyhow::bail!("read-only edit modified buffer\n{}", rile.screen_dump());
    }

    rile.send("C-x", keys::control('x'))?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_screen_contains("Error: save failed: invalid input: buffer is read-only")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn toggles_file_read_only_and_writable() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let target = directory.path().join("toggle.txt");
    std::fs::write(&target, "toggle target\n")?;
    let mut rile = RilePty::spawn(&target, 12, 100)?;

    rile.wait_for_screen_contains("toggle target")?;

    rile.send("C-x", keys::control('x'))?;
    rile.send("C-q", keys::control('q'))?;
    rile.assert_screen_contains("Buffer is now read-only")?;

    rile.send("x", b"x")?;
    rile.assert_screen_contains("Buffer is read-only:")?;
    if rile.snapshot_text().contains("xtoggle target") {
        anyhow::bail!("read-only edit modified buffer\n{}", rile.screen_dump());
    }

    rile.send("C-x", keys::control('x'))?;
    rile.send("C-q", keys::control('q'))?;
    rile.assert_screen_contains("Buffer is now writable")?;

    rile.send("x", b"x")?;
    rile.assert_screen_contains("xtoggle target")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn revert_buffer_reloads_file_after_confirmation() -> Result<()> {
    let file = fixtures::named_temp_file("before\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("before")?;
    std::fs::write(file.path(), "after\n")?;
    rile.send("x", b"x")?;
    rile.assert_screen_contains("xbefore")?;

    rile.send("C-x", keys::control('x'))?;
    rile.send("C-v", keys::control('v'))?;
    rile.assert_screen_contains("Buffer modified; revert anyway?")?;
    rile.send("yes", b"yes")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("after")?;
    rile.assert_screen_contains("Reverted")?;
    if rile.snapshot_text().contains("xbefore") {
        anyhow::bail!(
            "revert did not replace dirty contents\n{}",
            rile.screen_dump()
        );
    }

    rile.quit()?;
    Ok(())
}

#[test]
fn not_modified_clears_dirty_marker_without_saving() -> Result<()> {
    let file = fixtures::named_temp_file("clean\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("clean")?;
    rile.send("x", b"x")?;
    rile.assert_screen_contains("modified:true")?;

    rile.send("M-x", keys::meta('x'))?;
    rile.send("not-modified", b"not-modified")?;
    rile.send("Enter", keys::ENTER)?;

    rile.assert_screen_contains("Modification flag cleared")?;
    rile.assert_screen_contains("modified:false")?;
    assert_eq!(std::fs::read_to_string(file.path())?, "clean\n");

    rile.quit()?;
    Ok(())
}

#[test]
fn auto_revert_mode_reloads_clean_file_after_idle_poll() -> Result<()> {
    let file = fixtures::named_temp_file("before\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("before")?;
    rile.send("M-x", keys::meta('x'))?;
    rile.send("auto-revert-mode", b"auto-revert-mode")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Auto-revert for")?;
    std::fs::write(file.path(), "after\n")?;

    rile.wait_for_screen_contains("after")?;
    rile.assert_screen_contains("Reverted")?;
    if rile.snapshot_text().contains("before") {
        anyhow::bail!(
            "auto-revert did not replace old contents\n{}",
            rile.screen_dump()
        );
    }

    rile.quit()?;
    Ok(())
}

#[test]
fn auto_revert_mode_does_not_reload_dirty_buffer() -> Result<()> {
    let file = fixtures::named_temp_file("before\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("before")?;
    rile.send("M-x", keys::meta('x'))?;
    rile.send("auto-revert-mode", b"auto-revert-mode")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Auto-revert for")?;
    rile.send("dirty edit", b"dirty ")?;
    rile.assert_screen_contains("dirty before")?;
    std::fs::write(file.path(), "after\n")?;
    rile.drain_for(std::time::Duration::from_millis(300))?;

    rile.assert_screen_contains("dirty before")?;
    if rile.snapshot_text().contains("after") {
        anyhow::bail!(
            "auto-revert discarded dirty contents\n{}",
            rile.screen_dump()
        );
    }

    rile.quit()?;
    Ok(())
}
