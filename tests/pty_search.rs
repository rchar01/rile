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
fn incremental_search_uses_smart_case() -> Result<()> {
    let file = fixtures::named_temp_file("Status\nstatus\nSTATUS\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("Status")?;
    rile.send("C-s", keys::control('s'))?;
    rile.send("lowercase search", b"status")?;
    rile.assert_screen_contains("I-search: status")?;
    rile.assert_status_contains("Ln 001 Col 000")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_status_contains("Ln 002 Col 000")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_status_contains("Ln 003 Col 000")?;
    rile.send("Enter", keys::ENTER)?;
    rile.quit()?;

    let file = fixtures::named_temp_file("Status\nstatus\nSTATUS\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("Status")?;
    rile.send("C-s", keys::control('s'))?;
    rile.send("uppercase search", b"Status")?;
    rile.assert_screen_contains("I-search: Status")?;
    rile.assert_status_contains("Ln 001 Col 000")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_screen_contains("Failing I-search: Status")?;
    rile.send("C-g", keys::control('g'))?;

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
fn regexp_incremental_search_uses_smart_case() -> Result<()> {
    let file = fixtures::named_temp_file("Status\nstatus\n123 ABC\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("Status")?;
    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("lowercase regexp", b"status")?;
    rile.assert_screen_contains("Regexp I-search: status")?;
    rile.assert_status_contains("Ln 001 Col 000")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_status_contains("Ln 002 Col 000")?;
    rile.send("Enter", keys::ENTER)?;
    rile.quit()?;

    let file = fixtures::named_temp_file("123 ABC\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("123 ABC")?;
    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("folded class regexp", b"[a-z]+")?;
    rile.assert_screen_contains("Regexp I-search: [a-z]+")?;
    rile.assert_status_contains("Ln 001 Col 004")?;
    rile.send("Enter", keys::ENTER)?;

    rile.quit()?;
    Ok(())
}

#[test]
fn hi_lock_highlight_commands_use_emacs_style_keys() -> Result<()> {
    let file = fixtures::named_temp_file("foo bar\nFoo   bar\nTODO\nplain\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("foo bar")?;
    rile.send("M-s", keys::meta('s'))?;
    rile.send("h", b"h")?;
    rile.send("p", b"p")?;
    rile.assert_screen_contains("Highlight phrase:")?;
    rile.send("foo bar", b"foo bar")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Highlighted foo bar")?;

    rile.send("M-s", keys::meta('s'))?;
    rile.send("h", b"h")?;
    rile.send("r", b"r")?;
    rile.assert_screen_contains("Highlight regexp:")?;
    rile.send("TODO", b"TODO")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Highlighted TODO")?;

    rile.send("M-s", keys::meta('s'))?;
    rile.send("h", b"h")?;
    rile.send("l", b"l")?;
    rile.assert_screen_contains("Highlight lines matching regexp:")?;
    rile.send("plain", b"plain")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Highlighted lines matching plain")?;

    rile.send("M-s", keys::meta('s'))?;
    rile.send("h", b"h")?;
    rile.send("u", b"u")?;
    rile.assert_screen_contains("Unhighlight regexp:")?;
    rile.send("TODO", b"TODO")?;
    rile.send("Enter", keys::ENTER)?;
    rile.assert_screen_contains("Removed 1 highlight")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn regexp_incremental_search_smart_case_ignores_escaped_uppercase() -> Result<()> {
    let file = fixtures::named_temp_file("!Cat\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("!Cat")?;
    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("escaped uppercase regexp", br"\Wcat")?;
    rile.assert_screen_contains(r"Regexp I-search: \Wcat")?;
    rile.assert_status_contains("Ln 001 Col 000")?;
    rile.send("Enter", keys::ENTER)?;

    rile.quit()?;
    Ok(())
}

#[test]
fn regexp_incremental_search_uses_groups_alternation_and_counts() -> Result<()> {
    let file = fixtures::named_temp_file("xx cats\nxx dogs\nxx dogss\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("xx cats")?;
    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("regexp", br"\(cat\|dog\)s\{1,2\}")?;
    rile.assert_screen_contains(r"Regexp I-search: \(cat\|dog\)s\{1,2\}")?;
    rile.assert_status_contains("Ln 001 Col 003")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_status_contains("Ln 002 Col 003")?;
    rile.send("C-s", keys::control('s'))?;
    rile.assert_status_contains("Ln 003 Col 003")?;
    rile.send("Enter", keys::ENTER)?;

    rile.quit()?;
    Ok(())
}

#[test]
fn regexp_incremental_search_uses_word_and_posix_classes() -> Result<()> {
    let file = fixtures::named_temp_file("concatenate\ncat 1234\nbob_cat!\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("concatenate")?;
    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("regexp", br"\Bcat")?;
    rile.assert_screen_contains(r"Regexp I-search: \Bcat")?;
    rile.assert_status_contains("Ln 001 Col 003")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("regexp", br"\bcat")?;
    rile.assert_screen_contains(r"Regexp I-search: \bcat")?;
    rile.assert_status_contains("Ln 002 Col 000")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("regexp", br"\<cat\>")?;
    rile.assert_screen_contains(r"Regexp I-search: \<cat\>")?;
    rile.assert_status_contains("Ln 002 Col 000")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("regexp", br"[[:digit:]]\{2,4\}")?;
    rile.assert_screen_contains(r"Regexp I-search: [[:digit:]]\{2,4\}")?;
    rile.assert_status_contains("Ln 002 Col 004")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("regexp", br"\w+\W")?;
    rile.assert_screen_contains(r"Regexp I-search: \w+\W")?;
    rile.assert_status_contains("Ln 003 Col 000")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-M-r", keys::ctrl_meta('r'))?;
    rile.send("regexp", br"[[:digit:]]\{4\}")?;
    rile.assert_screen_contains(r"Regexp I-search backward: [[:digit:]]\{4\}")?;
    rile.assert_status_contains("Ln 002 Col 004")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("regexp", b"bob_cat")?;
    rile.assert_status_contains("Ln 003 Col 000")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-M-r", keys::ctrl_meta('r'))?;
    rile.send("regexp", br"\bcat")?;
    rile.assert_screen_contains(r"Regexp I-search backward: \bcat")?;
    rile.assert_status_contains("Ln 002 Col 000")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("regexp", b"bob_cat")?;
    rile.assert_status_contains("Ln 003 Col 000")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-M-r", keys::ctrl_meta('r'))?;
    rile.send("regexp", br"\Bcat")?;
    rile.assert_screen_contains(r"Regexp I-search backward: \Bcat")?;
    rile.assert_status_contains("Ln 001 Col 003")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("regexp", b"bob_cat")?;
    rile.assert_status_contains("Ln 003 Col 000")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-M-r", keys::ctrl_meta('r'))?;
    rile.send("regexp", br"\b\w+\W")?;
    rile.assert_screen_contains(r"Regexp I-search backward: \b\w+\W")?;
    rile.assert_status_contains("Ln 002 Col 000")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-M-s", keys::ctrl_meta('s'))?;
    rile.send("regexp", b"bob_cat")?;
    rile.assert_status_contains("Ln 003 Col 000")?;
    rile.send("Enter", keys::ENTER)?;

    rile.send("C-M-r", keys::ctrl_meta('r'))?;
    rile.send("regexp", br"\<cat\>")?;
    rile.assert_screen_contains(r"Regexp I-search backward: \<cat\>")?;
    rile.assert_status_contains("Ln 002 Col 000")?;
    rile.send("Enter", keys::ENTER)?;

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
fn query_replace_uses_smart_case_matching() -> Result<()> {
    let file = fixtures::named_temp_file("Status status STATUS\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("Status status STATUS")?;
    rile.send("M-%", keys::meta('%'))?;
    rile.send("search text", b"status")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement text", b"state")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replace all", b"!")?;

    rile.wait_for_screen_contains("State state STATE")?;
    rile.assert_screen_contains("Replaced 3 occurrences")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn query_replace_smart_case_keeps_uppercase_search_exact() -> Result<()> {
    let file = fixtures::named_temp_file("Status status STATUS\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("Status status STATUS")?;
    rile.send("M-%", keys::meta('%'))?;
    rile.send("search text", b"Status")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement text", b"state")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replace all", b"!")?;

    rile.wait_for_screen_contains("state status STATUS")?;
    rile.assert_screen_contains("Replaced 1 occurrence")?;

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
fn query_replace_regexp_expands_replacement_captures() -> Result<()> {
    let file = fixtures::named_temp_file("foo-bar baz-qux\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("foo-bar baz-qux")?;
    rile.send("C-M-%", keys::csi_u_ctrl_meta('%'))?;
    rile.send("regexp", br"\([a-z]+\)-\([a-z]+\)")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement", br"\2/\1")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replace all", b"!")?;

    rile.wait_for_screen_contains("bar/foo qux/baz")?;
    rile.assert_screen_contains("Replaced 2 occurrences")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn query_replace_regexp_uses_word_and_posix_classes() -> Result<()> {
    let file = fixtures::named_temp_file("cat concatenate bob_cat 1234\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("cat concatenate bob_cat 1234")?;
    rile.send("C-M-%", keys::csi_u_ctrl_meta('%'))?;
    rile.send("regexp", br"\<cat\>\|[[:digit:]]\{2,4\}")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement", b"hit")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replace all", b"!")?;

    rile.wait_for_screen_contains("hit concatenate bob_cat hit")?;
    rile.assert_screen_contains("Replaced 2 occurrences")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn query_replace_regexp_uses_smart_case_matching() -> Result<()> {
    let file = fixtures::named_temp_file("Status status STATUS\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("Status status STATUS")?;
    rile.send("C-M-%", keys::csi_u_ctrl_meta('%'))?;
    rile.send("regexp", b"status")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement", b"state")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replace all", b"!")?;

    rile.wait_for_screen_contains("State state STATE")?;
    rile.assert_screen_contains("Replaced 3 occurrences")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn query_replace_regexp_smart_case_keeps_uppercase_search_exact() -> Result<()> {
    let file = fixtures::named_temp_file("Status status STATUS\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("Status status STATUS")?;
    rile.send("C-M-%", keys::csi_u_ctrl_meta('%'))?;
    rile.send("regexp", b"Status")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement", b"state")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replace all", b"!")?;

    rile.wait_for_screen_contains("state status STATUS")?;
    rile.assert_screen_contains("Replaced 1 occurrence")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn query_replace_regexp_preserves_unsupported_escapes_and_utf8() -> Result<()> {
    let file = fixtures::named_temp_file("éx bar\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("éx bar")?;
    rile.send("C-M-%", keys::csi_u_ctrl_meta('%'))?;
    rile.send("regexp", "\\(éx\\)\\|\\(bar\\)".as_bytes())?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement", br"\1/\2/\9/\\/\q")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replace all", b"!")?;

    rile.wait_for_screen_contains(r"éx///\/\q /bar//\/\q")?;
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
fn replace_regexp_uses_groups_alternation_and_counts() -> Result<()> {
    let file = fixtures::named_temp_file("cats dogs dogss cots\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("cats dogs dogss cots")?;
    execute_m_x(&mut rile, b"replace-regexp")?;
    rile.send("regexp", br"\(cat\|dog\)s\{1,2\}")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement", b"pet")?;
    rile.send("Enter", keys::ENTER)?;

    rile.wait_for_screen_contains("pet pet pet cots")?;
    rile.assert_screen_contains("Replaced 3 occurrences")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn replace_regexp_uses_word_and_posix_classes() -> Result<()> {
    let file = fixtures::named_temp_file("cat concatenate bob_cat 1234\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("cat concatenate bob_cat 1234")?;
    execute_m_x(&mut rile, b"replace-regexp")?;
    rile.send("regexp", br"\<cat\>\|[[:digit:]]\{2,4\}")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement", b"hit")?;
    rile.send("Enter", keys::ENTER)?;

    rile.wait_for_screen_contains("hit concatenate bob_cat hit")?;
    rile.assert_screen_contains("Replaced 2 occurrences")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn replace_regexp_uses_word_boundary_and_word_character_constructs() -> Result<()> {
    let file = fixtures::named_temp_file("cat! dog? x_cat.\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("cat! dog? x_cat.")?;
    execute_m_x(&mut rile, b"replace-regexp")?;
    rile.send("regexp", br"\b\w+\W")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement", b"hit")?;
    rile.send("Enter", keys::ENTER)?;

    rile.wait_for_screen_contains("hit hit hit")?;
    rile.assert_screen_contains("Replaced 3 occurrences")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn replace_regexp_uses_smart_case_matching() -> Result<()> {
    let file = fixtures::named_temp_file("Status status STATUS\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("Status status STATUS")?;
    execute_m_x(&mut rile, b"replace-regexp")?;
    rile.send("regexp", b"status")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement", b"state")?;
    rile.send("Enter", keys::ENTER)?;

    rile.wait_for_screen_contains("State state STATE")?;
    rile.assert_screen_contains("Replaced 3 occurrences")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn replace_regexp_smart_case_keeps_uppercase_search_exact() -> Result<()> {
    let file = fixtures::named_temp_file("Status status STATUS\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("Status status STATUS")?;
    execute_m_x(&mut rile, b"replace-regexp")?;
    rile.send("regexp", b"Status")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement", b"state")?;
    rile.send("Enter", keys::ENTER)?;

    rile.wait_for_screen_contains("state status STATUS")?;
    rile.assert_screen_contains("Replaced 1 occurrence")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn replace_regexp_expands_whole_match_and_captures() -> Result<()> {
    let file = fixtures::named_temp_file("foo-bar foo-baz\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("foo-bar foo-baz")?;
    execute_m_x(&mut rile, b"replace-regexp")?;
    rile.send("regexp", br"\(foo\)-\([a-z]+\)")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement", br"[\&]=\2/\1")?;
    rile.send("Enter", keys::ENTER)?;

    rile.wait_for_screen_contains("[foo-bar]=bar/foo [foo-baz]=baz/foo")?;
    rile.assert_screen_contains("Replaced 2 occurrences")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn replace_regexp_adapts_case_after_capture_expansion() -> Result<()> {
    let file = fixtures::named_temp_file("Foo-Bar FOO-BAR foo-bar\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("Foo-Bar FOO-BAR foo-bar")?;
    execute_m_x(&mut rile, b"replace-regexp")?;
    rile.send("regexp", br"\(foo\)-\(bar\)")?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement", br"x\1-y\2")?;
    rile.send("Enter", keys::ENTER)?;

    rile.wait_for_screen_contains("XFoo-YBar XFOO-YBAR xfoo-ybar")?;
    rile.assert_screen_contains("Replaced 3 occurrences")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn replace_regexp_preserves_unsupported_escapes_and_utf8() -> Result<()> {
    let file = fixtures::named_temp_file("éx bar\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("éx bar")?;
    execute_m_x(&mut rile, b"replace-regexp")?;
    rile.send("regexp", "\\(éx\\)\\|\\(bar\\)".as_bytes())?;
    rile.send("Enter", keys::ENTER)?;
    rile.send("replacement", br"\1/\2/\9/\\/\q")?;
    rile.send("Enter", keys::ENTER)?;

    rile.wait_for_screen_contains(r"éx///\/\q /bar//\/\q")?;
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
