// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;
use std::fs;

use support::{fixtures, keys, pty::RilePty};

fn control_x_text(text: &str) -> Vec<u8> {
    let mut sequence = keys::control('x').to_vec();
    sequence.extend_from_slice(text.as_bytes());
    sequence
}

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

#[test]
fn split_commands_preserve_active_pane_and_buffer_text() -> Result<()> {
    let file = fixtures::fixture_path("split_left.txt");
    let mut rile = RilePty::spawn(&file, 14, 120)?;

    rile.wait_for_screen_contains("split_left.txt")?;

    rile.send("C-x 2", control_x_text("2"))?;
    rile.assert_screen_contains("window 0 ACTIVE split_left.txt")?;
    rile.assert_screen_contains("window 1 inactive split_left.txt")?;
    rile.assert_screen_contains("left 000 | alpha")?;

    rile.send("C-x o", control_x_text("o"))?;
    rile.assert_screen_contains("window 0 inactive split_left.txt")?;
    rile.assert_screen_contains("window 1 ACTIVE split_left.txt")?;
    let (row, _) = rile.cursor_position();
    assert!(
        row > 5,
        "active cursor was not in lower pane\n{}",
        rile.screen_dump()
    );

    rile.send("C-x 0", control_x_text("0"))?;
    rile.assert_screen_contains("window 0 ACTIVE split_left.txt")?;
    assert!(
        !rile.snapshot_text().contains("inactive split_left.txt"),
        "deleted window still rendered\n{}",
        rile.screen_dump()
    );

    rile.send("C-x 3", control_x_text("3"))?;
    rile.assert_screen_contains("window 0 ACTIVE split_left.txt")?;
    rile.assert_screen_contains("window 2 inactive split_left.txt")?;
    rile.assert_screen_contains("left 000 | alpha")?;

    rile.send("C-x 1", control_x_text("1"))?;
    rile.assert_screen_contains("window 0 ACTIVE split_left.txt")?;
    assert!(
        !rile.snapshot_text().contains("inactive split_left.txt"),
        "other windows still rendered\n{}",
        rile.screen_dump()
    );

    rile.quit()?;
    Ok(())
}

#[test]
fn list_buffers_opens_in_inactive_lower_window() -> Result<()> {
    let file = fixtures::named_temp_file("list buffers fixture\n")?;
    let mut rile = RilePty::spawn(file.path(), 14, 120)?;

    rile.wait_for_screen_contains("list buffers fixture")?;
    rile.send("C-x C-b", keys::control_sequence("xb"))?;
    rile.assert_screen_contains("CRM Buffer")?;
    rile.assert_screen_contains("*Buffer List*")?;
    rile.assert_screen_contains("window 0 ACTIVE")?;
    rile.assert_screen_contains("window 1 inactive *Buffer List*")?;
    rile.assert_status_contains("modified:false")?;

    rile.send("C-x o", control_x_text("o"))?;
    rile.assert_screen_contains("window 1 ACTIVE *Buffer List*")?;
    rile.send("q", b"q")?;
    assert!(
        !rile.snapshot_text().contains("*Buffer List*"),
        "buffer list window still rendered\n{}",
        rile.screen_dump()
    );

    rile.quit()?;
    Ok(())
}

#[test]
fn list_buffers_ret_opens_selected_row_in_list_window() -> Result<()> {
    let file = fixtures::named_temp_file("list buffers ret fixture\n")?;
    let file_name = file
        .path()
        .file_name()
        .expect("fixture should have file name")
        .to_string_lossy()
        .into_owned();
    let mut rile = RilePty::spawn(file.path(), 14, 120)?;

    rile.wait_for_screen_contains("list buffers ret fixture")?;
    rile.send("C-x C-b", keys::control_sequence("xb"))?;
    rile.assert_screen_contains("window 1 inactive *Buffer List*")?;

    rile.send("C-x o", control_x_text("o"))?;
    rile.assert_screen_contains("window 1 ACTIVE *Buffer List*")?;
    rile.send("C-n", keys::control('n'))?;
    rile.send("C-n", keys::control('n'))?;
    rile.send("RET", keys::ENTER)?;

    assert!(
        !rile.snapshot_text().contains("*Buffer List*"),
        "buffer list window still rendered\n{}",
        rile.screen_dump()
    );
    rile.assert_screen_contains(&format!("window 1 ACTIVE {file_name}"))?;
    rile.assert_screen_contains("list buffers ret fixture")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn switch_buffer_in_split_preserves_window_and_buffer_points() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let left = directory.path().join("left-buffer.txt");
    let right = directory.path().join("right-buffer.txt");
    fs::write(&left, "left one\nleft two\nleft three\n")?;
    fs::write(&right, "right one\nright two\nright three\nright four\n")?;
    let mut rile = RilePty::spawn(&left, 14, 100)?;

    rile.wait_for_screen_contains("left one")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    let right_path = right.display().to_string();
    rile.send("right path", right_path.as_bytes())?;
    rile.send("RET", keys::ENTER)?;
    rile.wait_for_screen_contains("right one")?;
    rile.send("C-n", keys::control('n'))?;
    rile.send("C-n", keys::control('n'))?;
    rile.assert_status_contains("ACTIVE right-buffer.txt Ln 003 Col 000")?;

    rile.send("C-x b", control_x_text("b"))?;
    rile.send("left buffer", b"left-buffer.txt")?;
    rile.send("RET", keys::ENTER)?;
    rile.assert_status_contains("ACTIVE left-buffer.txt Ln 001 Col 000")?;

    rile.send("C-x 2", control_x_text("2"))?;
    rile.assert_screen_contains("window 0 ACTIVE left-buffer.txt")?;
    rile.assert_screen_contains("window 1 inactive left-buffer.txt")?;
    rile.send("C-x o", control_x_text("o"))?;
    rile.assert_screen_contains("window 1 ACTIVE left-buffer.txt")?;

    rile.send("C-x b", control_x_text("b"))?;
    rile.send("right buffer", b"right-buffer.txt")?;
    rile.send("RET", keys::ENTER)?;
    rile.assert_screen_contains("window 0 inactive left-buffer.txt")?;
    rile.assert_status_contains("window 1 ACTIVE right-buffer.txt Ln 003 Col 000")?;
    let (lower_row, _) = rile.cursor_position();
    assert!(
        lower_row > 5,
        "switch-buffer did not keep the lower split selected\n{}",
        rile.screen_dump()
    );

    rile.send("C-p", keys::control('p'))?;
    rile.assert_status_contains("window 1 ACTIVE right-buffer.txt Ln 002 Col 000")?;
    rile.send("C-x o", control_x_text("o"))?;
    rile.assert_screen_contains("window 0 ACTIVE left-buffer.txt Ln 001 Col 000")?;
    rile.send("C-x o", control_x_text("o"))?;
    rile.assert_status_contains("window 1 ACTIVE right-buffer.txt Ln 002 Col 000")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn kill_buffer_replaces_all_windows_showing_target() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let fallback = directory.path().join("fallback-buffer.txt");
    let target = directory.path().join("target-buffer.txt");
    fs::write(&fallback, "fallback buffer\n")?;
    fs::write(&target, "target buffer\n")?;
    let mut rile = RilePty::spawn(&fallback, 14, 100)?;

    rile.wait_for_screen_contains("fallback buffer")?;
    rile.send("C-x C-f", keys::control_sequence("xf"))?;
    let target_path = target.display().to_string();
    rile.send("target path", target_path.as_bytes())?;
    rile.send("RET", keys::ENTER)?;
    rile.wait_for_screen_contains("target buffer")?;

    rile.send("C-x 2", control_x_text("2"))?;
    rile.assert_screen_contains("window 0 ACTIVE target-buffer.txt")?;
    rile.assert_screen_contains("window 1 inactive target-buffer.txt")?;

    rile.send("C-x k", control_x_text("k"))?;
    rile.assert_screen_contains("Kill buffer (default target-buffer.txt):")?;
    rile.send("RET", keys::ENTER)?;

    rile.assert_screen_contains("Killed buffer target-buffer.txt")?;
    rile.assert_screen_contains("window 0 ACTIVE fallback-buffer.txt")?;
    rile.assert_screen_contains("window 1 inactive fallback-buffer.txt")?;
    rile.assert_screen_contains("fallback buffer")?;
    assert!(
        !rile.snapshot_text().contains("target buffer"),
        "killed buffer text still rendered\n{}",
        rile.screen_dump()
    );

    rile.quit()?;
    Ok(())
}
