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

#[test]
fn incremental_search_history_recalls_with_meta_keys() -> Result<()> {
    let file = fixtures::named_temp_file("foo\nbar\nféo\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("foo")?;
    rile.send("C-s", keys::control('s'))?;
    rile.send("bar", b"bar")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-r", keys::control('r'))?;
    rile.send("draft", b"draft")?;
    rile.assert_screen_contains("Failing I-search backward: draft")?;
    rile.send("M-p", keys::meta('p'))?;
    rile.assert_screen_contains("I-search backward: bar")?;
    rile.assert_status_contains("Ln 002 Col 000")?;
    rile.send("M-n", keys::meta('n'))?;
    rile.assert_screen_contains("Failing I-search backward: draft")?;
    rile.send("C-g", keys::control('g'))?;

    rile.quit()?;
    Ok(())
}

#[test]
fn query_replace_history_recalls_search_and_replacement() -> Result<()> {
    let file = fixtures::named_temp_file("foo\nbaz\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("foo")?;
    rile.send("M-%", keys::meta('%'))?;
    rile.send("search text", b"foo")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement text", b"qux")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replace all", b"!")?;
    rile.wait_for_screen_contains("Replaced 1 occurrence")?;

    rile.send("M-%", keys::meta('%'))?;
    rile.send("search draft", b"draft")?;
    rile.send("M-p", keys::meta('p'))?;
    rile.assert_screen_contains("Query replace: foo")?;
    rile.send("M-n", keys::meta('n'))?;
    rile.assert_screen_contains("Query replace: draft")?;
    rile.send("M-p", keys::meta('p'))?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("replacement draft", b"draft-replacement")?;
    rile.send("M-p", keys::meta('p'))?;
    rile.assert_screen_contains("Query replace foo with: qux")?;
    rile.send("M-n", keys::meta('n'))?;
    rile.assert_screen_contains("Query replace foo with: draft-replacement")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

#[test]
fn query_replace_regexp_replaces_matches() -> Result<()> {
    let file = fixtures::named_temp_file("foo fxo faa\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("foo fxo faa")?;
    rile.send("C-M-%", keys::csi_u_ctrl_meta('%'))?;
    rile.assert_screen_contains("Query replace regexp:")?;
    rile.send("regexp", b"f.o")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Query replace regexp f.o with:")?;
    rile.send("replacement", b"bar")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Query replacing regexp f.o with bar: (y, n, !, q)?")?;
    rile.send("replace all", b"!")?;

    rile.wait_for_screen_contains("bar bar faa")?;
    rile.assert_screen_contains("Replaced 2 occurrences")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn query_replace_regexp_rejects_invalid_and_zero_width_patterns() -> Result<()> {
    let file = fixtures::named_temp_file("foo\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("foo")?;
    rile.send("C-M-%", keys::csi_u_ctrl_meta('%'))?;
    rile.send("invalid regexp", b"[")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Error: invalid regexp")?;
    rile.assert_screen_contains("foo")?;

    rile.send("C-M-%", keys::csi_u_ctrl_meta('%'))?;
    rile.send("zero-width regexp", b"^")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Error: regexp can match empty string")?;
    rile.assert_screen_contains("foo")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn query_replace_regexp_history_is_separate() -> Result<()> {
    let file = fixtures::named_temp_file("foo\nfxo\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("foo")?;
    rile.send("C-M-%", keys::csi_u_ctrl_meta('%'))?;
    rile.send("regexp", b"f.o")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement", b"bar")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replace all", b"!")?;
    rile.wait_for_screen_contains("Replaced 2 occurrences")?;

    rile.send("C-M-%", keys::csi_u_ctrl_meta('%'))?;
    rile.send("search draft", b"draft")?;
    rile.send("M-p", keys::meta('p'))?;
    rile.assert_screen_contains("Query replace regexp: f.o")?;
    rile.send("M-n", keys::meta('n'))?;
    rile.assert_screen_contains("Query replace regexp: draft")?;
    rile.send("M-p", keys::meta('p'))?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("replacement draft", b"draft-replacement")?;
    rile.send("M-p", keys::meta('p'))?;
    rile.assert_screen_contains("Query replace regexp f.o with: bar")?;
    rile.send("M-n", keys::meta('n'))?;
    rile.assert_screen_contains("Query replace regexp f.o with: draft-replacement")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

#[test]
fn replace_regexp_replaces_matches() -> Result<()> {
    let file = fixtures::named_temp_file("foo fxo faa\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("foo fxo faa")?;
    execute_m_x(&mut rile, b"replace-regexp")?;
    rile.assert_screen_contains("Replace regexp:")?;
    rile.send("regexp", b"f.o")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Replace regexp f.o with:")?;
    rile.send("replacement", b"bar")?;
    rile.send("Enter", keys::ENTER)?;

    rile.wait_for_screen_contains("bar bar faa")?;
    rile.assert_screen_contains("Replaced 2 occurrences")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn replace_regexp_rejects_invalid_and_zero_width_patterns() -> Result<()> {
    let file = fixtures::named_temp_file("foo\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("foo")?;
    execute_m_x(&mut rile, b"replace-regexp")?;
    rile.send("invalid regexp", b"[")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Error: invalid regexp")?;
    rile.assert_screen_contains("foo")?;

    execute_m_x(&mut rile, b"replace-regexp")?;
    rile.send("zero-width regexp", b"^")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Error: regexp can match empty string")?;
    rile.assert_screen_contains("foo")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn replace_regexp_history_recalls_search_and_replacement() -> Result<()> {
    let file = fixtures::named_temp_file("foo\nfxo\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("foo")?;
    execute_m_x(&mut rile, b"replace-regexp")?;
    rile.send("regexp", b"f.o")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement", b"bar")?;
    rile.send("Enter", keys::ENTER)?;
    rile.wait_for_screen_contains("Replaced 2 occurrences")?;

    execute_m_x(&mut rile, b"replace-regexp")?;
    rile.send("search draft", b"draft")?;
    rile.send("M-p", keys::meta('p'))?;
    rile.assert_screen_contains("Replace regexp: f.o")?;
    rile.send("M-n", keys::meta('n'))?;
    rile.assert_screen_contains("Replace regexp: draft")?;
    rile.send("M-p", keys::meta('p'))?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("replacement draft", b"draft-replacement")?;
    rile.send("M-p", keys::meta('p'))?;
    rile.assert_screen_contains("Replace regexp f.o with: bar")?;
    rile.send("M-n", keys::meta('n'))?;
    rile.assert_screen_contains("Replace regexp f.o with: draft-replacement")?;

    rile.send("C-g", keys::control('g'))?;
    rile.quit()?;
    Ok(())
}

fn execute_m_x(rile: &mut RilePty, command: &[u8]) -> Result<()> {
    rile.send("M-x", keys::meta('x'))?;
    rile.send("command", command)?;
    rile.send("Enter", keys::ENTER)
}
