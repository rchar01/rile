// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;

use support::{fixtures, keys, pty::RilePty};

#[test]
fn clean_quit_exits_without_prompt() -> Result<()> {
    let file = fixtures::named_temp_file("clean quit fixture\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 100)?;

    rile.wait_for_screen_contains("clean quit fixture")?;
    rile.send("C-x C-c", keys::control_sequence("xc"))?;

    rile.expect_exit()?;
    Ok(())
}

#[test]
fn dirty_quit_prompt_can_be_cancelled() -> Result<()> {
    let file = fixtures::named_temp_file("dirty quit fixture\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 100)?;

    rile.wait_for_screen_contains("dirty quit fixture")?;
    rile.send("insert dirty marker", b"!")?;
    rile.assert_status_contains("modified:true")?;

    rile.send("C-x C-c", keys::control_sequence("xc"))?;
    rile.assert_screen_contains("Modified buffers exist; exit anyway? (yes or no)")?;

    rile.send("C-g", keys::control('g'))?;
    rile.assert_screen_contains("Quit")?;
    rile.assert_status_contains("modified:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn dirty_quit_prompt_empty_answer_keeps_editor_open() -> Result<()> {
    let file = fixtures::named_temp_file("dirty quit empty fixture\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 100)?;

    rile.wait_for_screen_contains("dirty quit empty fixture")?;
    rile.send("insert dirty marker", b"!")?;
    rile.send("C-x C-c", keys::control_sequence("xc"))?;
    rile.assert_screen_contains("Modified buffers exist; exit anyway? (yes or no)")?;
    rile.send("RET", keys::ENTER)?;

    rile.assert_screen_contains("Quit")?;
    rile.assert_status_contains("modified:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn dirty_quit_prompt_no_answer_keeps_editor_open() -> Result<()> {
    let file = fixtures::named_temp_file("dirty quit no fixture\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 100)?;

    rile.wait_for_screen_contains("dirty quit no fixture")?;
    rile.send("insert dirty marker", b"!")?;
    rile.send("C-x C-c", keys::control_sequence("xc"))?;
    rile.assert_screen_contains("Modified buffers exist; exit anyway? (yes or no)")?;
    rile.send("no", b"no")?;
    rile.send("RET", keys::ENTER)?;

    rile.assert_screen_contains("Quit")?;
    rile.assert_status_contains("modified:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn dirty_quit_prompt_exits_after_yes() -> Result<()> {
    let file = fixtures::named_temp_file("dirty quit confirm fixture\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 100)?;

    rile.wait_for_screen_contains("dirty quit confirm fixture")?;
    rile.send("insert dirty marker", b"!")?;
    rile.send("C-x C-c", keys::control_sequence("xc"))?;
    rile.assert_screen_contains("Modified buffers exist; exit anyway? (yes or no)")?;
    rile.send("yes", b"yes")?;
    rile.send("RET", keys::ENTER)?;

    rile.expect_exit()?;
    Ok(())
}
