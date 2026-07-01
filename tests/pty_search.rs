// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;

use support::{fixtures, keys, pty::RilePty};

#[test]
fn incremental_search_wraps_after_boundary_failure() -> Result<()> {
    let file = fixtures::named_temp_file("one\ntwo\none\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("one")?;
    rile.send("C-s", keys::control('s'))?;
    rile.send("one", b"one")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_status_contains("Ln 003 Col 000")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_screen_contains("Failing I-search: one")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_screen_contains("Wrapped I-search: one")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_cursor(0, 0)?;

    rile.send("C-r", keys::control('r'))?;
    rile.send("one", b"one")?;
    rile.assert_screen_contains("Failing I-search backward: one")?;
    rile.send("C-r", keys::control('r'))?;
    rile.assert_screen_contains("Wrapped I-search backward: one")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_cursor(2, 0)?;

    rile.quit()?;
    Ok(())
}

#[test]
fn regexp_incremental_search_matches_and_repeats() -> Result<()> {
    let file = fixtures::named_temp_file("foo\nbar\nféo\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("foo")?;
    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("^", b"^")?;
    rile.assert_screen_contains("Regexp I-search: ^")?;
    rile.assert_status_contains("Ln 001 Col 000")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_status_contains("Ln 002 Col 000")?;
    rile.send("C-g", keys::control('g'))?;

    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("f.o", b"f.o")?;
    rile.assert_screen_contains("Regexp I-search: f.o")?;
    rile.assert_status_contains("Ln 001 Col 000")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_status_contains("Ln 003 Col 000")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_screen_contains("Failing regexp I-search: f.o")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_screen_contains("Wrapped regexp I-search: f.o")?;
    rile.assert_status_contains("Ln 001 Col 000")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-M-r", keys::ctrl_meta('r'))?;
    rile.send("f.o", b"f.o")?;
    rile.assert_screen_contains("Failing regexp I-search backward: f.o")?;
    rile.send("C-r", keys::control('r'))?;
    rile.assert_screen_contains("Wrapped regexp I-search backward: f.o")?;
    rile.assert_status_contains("Ln 003 Col 000")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("[", b"[")?;
    rile.assert_screen_contains("Invalid regexp I-search: [")?;
    rile.send("C-g", keys::control('g'))?;

    rile.quit()?;
    Ok(())
}
