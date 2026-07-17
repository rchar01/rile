// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use std::time::Duration;

use anyhow::Result;

use support::{fixtures, keys, pty::RilePty};

#[test]
fn meta_bang_displays_output_and_prefix_inserts_stdout() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-!", keys::meta('!'))?;
    rile.assert_screen_contains("Shell command:")?;
    rile.send("shell output command", b"printf 'shell-out\\n'")?;
    rile.send("RET", keys::ENTER)?;
    rile.wait_for_screen_contains("shell-out")?;
    rile.assert_screen_contains("*Shell Command Output*")?;

    rile.send("q", b"q")?;
    rile.wait_for_screen_contains("alpha")?;
    rile.send("C-u", keys::control('u'))?;
    rile.send("M-!", keys::meta('!'))?;
    rile.assert_screen_contains("Shell command:")?;
    rile.send("shell insert command", b"printf 'INSERTED'")?;
    rile.send("RET", keys::ENTER)?;
    rile.wait_for_screen_contains("INSERTEDalpha")?;
    rile.assert_status_contains("modified:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn shell_output_escapes_terminal_control_sequences() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 100)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-!", keys::meta('!'))?;
    rile.send(
        "hostile shell output command",
        b"printf '\\033[999;999H\\302\\2332J\\007'",
    )?;
    rile.send("RET", keys::ENTER)?;

    rile.wait_for_screen_contains("\\u{1b}[999;999H\\u{9b}2J\\u{7}")?;
    rile.assert_raw_output_excludes(b"\x1b[999;999H")?;
    rile.assert_raw_output_excludes(b"\xc2\x9b2J")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn shell_command_on_large_region_avoids_duplex_pipe_deadlock() -> Result<()> {
    let region = "x".repeat(2 * 1024 * 1024);
    let file = fixtures::named_temp_file(&region)?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("xxxx")?;
    rile.send("C-x h", keys::control('x'))?;
    rile.send("h", b"h")?;
    rile.send("M-|", keys::meta('|'))?;
    rile.assert_screen_contains("Shell command on region:")?;
    rile.send("cat command", b"cat")?;
    rile.send("RET", keys::ENTER)?;

    rile.wait_for_screen_contains_for(
        "Shell command completed (2097152 bytes)",
        Duration::from_secs(10),
    )?;
    rile.assert_screen_contains("*Shell Command Output*")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn foreground_completion_discards_input_queued_after_enter() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-!", keys::meta('!'))?;
    rile.send("short shell command", b"printf shell-out")?;
    let mut submission = keys::ENTER.to_vec();
    submission.extend_from_slice(b"QUEUED");
    rile.send("RET with queued text", submission)?;

    rile.wait_for_screen_contains("shell-out")?;
    rile.send("q", b"q")?;
    rile.wait_for_screen_contains("alpha")?;
    rile.assert_status_contains("modified:false")?;
    assert!(!rile.snapshot_text().contains("QUEUED"));

    rile.quit()?;
    Ok(())
}

#[test]
fn foreground_shell_cancels_and_resumes_editing() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("M-!", keys::meta('!'))?;
    rile.send(
        "SIGINT-resistant shell command",
        b"trap '' 2; while :; do :; done",
    )?;
    rile.send("RET", keys::ENTER)?;
    rile.wait_for_screen_contains("Running shell command... (C-g to cancel)")?;

    rile.send("suppressed foreground text", b"IGNORED")?;
    rile.assert_screen_contains("Running shell command... (C-g to cancel)")?;
    let mut cancel = keys::control('g').to_vec();
    cancel.extend_from_slice(&keys::control('g'));
    rile.send("C-g C-g", cancel)?;
    rile.wait_for_screen_contains("Shell command cancellation escalated")?;
    rile.wait_for_screen_contains("Shell command cancelled")?;

    rile.send("normal text after cancellation", b"Z")?;
    rile.wait_for_screen_contains("Zalpha")?;
    rile.assert_status_contains("modified:true")?;
    assert!(!rile.snapshot_text().contains("IGNORED"));

    rile.quit()?;
    Ok(())
}
