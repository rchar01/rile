// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

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
