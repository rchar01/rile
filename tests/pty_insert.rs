// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;

use support::{fixtures, keys, pty::RilePty};

#[test]
fn insert_delete_and_backspace_update_visible_buffer() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.assert_status_contains("Ln 001 Col 000")?;
    rile.assert_status_contains("modified:false")?;

    rile.send("insert ASCII", b"Z")?;
    rile.wait_for_screen_contains("Zalpha")?;
    rile.assert_status_contains("Ln 001 Col 001")?;
    rile.assert_status_contains("modified:true")?;

    rile.send("insert UTF-8", "é".as_bytes())?;
    rile.wait_for_screen_contains("Zéalpha")?;
    rile.assert_status_contains("Ln 001 Col 002")?;

    rile.send("RET", keys::ENTER)?;
    rile.wait_for_screen_contains("Zé")?;
    rile.assert_screen_contains("alpha")?;
    rile.assert_status_contains("Ln 002 Col 000")?;

    rile.send("Backspace", keys::BACKSPACE)?;
    rile.wait_for_screen_contains("Zéalpha")?;
    rile.assert_status_contains("Ln 001 Col 002")?;

    rile.send("Delete", keys::DELETE)?;
    rile.wait_for_screen_contains("Zélpha")?;
    rile.assert_status_contains("Ln 001 Col 002")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn open_line_keeps_cursor_and_shifts_text_down() -> Result<()> {
    let file = fixtures::named_temp_file("alpha beta gamma\nsecond line\nthird line\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("alpha beta gamma")?;
    for _ in 0..5 {
        rile.send("C-f", keys::control('f'))?;
    }
    rile.assert_cursor(0, 5)?;

    rile.send("C-o", keys::control('o'))?;
    rile.wait_for_screen_contains(" beta gamma")?;

    rile.assert_screen_contains("alpha")?;
    rile.assert_screen_contains("second line")?;
    rile.assert_screen_contains("third line")?;
    rile.assert_cursor(0, 5)?;
    rile.assert_status_contains("Ln 001 Col 005")?;
    rile.assert_status_contains("modified:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn universal_argument_repeats_visible_movement_and_insert() -> Result<()> {
    let file = fixtures::named_temp_file("abcdef\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("abcdef")?;
    rile.send("C-u", keys::control('u'))?;
    rile.assert_screen_contains("C-u-")?;
    rile.send("C-f", keys::control('f'))?;
    rile.assert_cursor(0, 4)?;
    rile.assert_status_contains("Ln 001 Col 004")?;

    rile.send("C-a", keys::control('a'))?;
    rile.send("C-u", keys::control('u'))?;
    rile.send("3", b"3")?;
    rile.assert_screen_contains("C-u 3-")?;
    rile.send("x", b"x")?;
    rile.wait_for_screen_contains("xxxabcdef")?;
    rile.assert_cursor(0, 3)?;
    rile.assert_status_contains("modified:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn keyboard_macro_replays_visible_editing_with_repeat_count() -> Result<()> {
    let file = fixtures::named_temp_file("one\ntwo\nthree\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("one")?;
    rile.send("C-x", keys::control('x'))?;
    rile.send("(", b"(")?;
    rile.assert_screen_contains("Defining keyboard macro")?;
    rile.send(">", b">")?;
    rile.send("C-n", keys::control('n'))?;
    rile.send("C-a", keys::control('a'))?;
    rile.send("C-x", keys::control('x'))?;
    rile.send(")", b")")?;
    rile.wait_for_screen_contains(">one")?;
    rile.assert_screen_contains("Keyboard macro defined")?;

    rile.send("C-u", keys::control('u'))?;
    rile.send("2", b"2")?;
    rile.send("C-x", keys::control('x'))?;
    rile.send("e", b"e")?;
    rile.wait_for_screen_contains(">three")?;
    rile.assert_screen_contains(">two")?;
    rile.assert_cursor(3, 0)?;

    rile.quit()?;
    Ok(())
}

#[test]
fn rectangle_mark_copy_and_regular_yank_paste_columns() -> Result<()> {
    let file = fixtures::named_temp_file("abcdef\n123456\nuvwxyz\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("abcdef")?;
    rile.send("C-f", keys::control('f'))?;
    rile.send("C-x", keys::control('x'))?;
    rile.send("SPC", b" ")?;
    rile.assert_screen_contains("Mark set (rectangle mode)")?;
    rile.send("C-f", keys::control('f'))?;
    rile.send("C-f", keys::control('f'))?;
    rile.send("C-n", keys::control('n'))?;
    rile.send("M-w", keys::meta('w'))?;
    rile.assert_screen_contains("Copied rectangle")?;

    rile.send("C-e", keys::control('e'))?;
    rile.send("C-n", keys::control('n'))?;
    rile.send("C-y", keys::control('y'))?;
    rile.wait_for_screen_contains("uvwxyzbc")?;
    rile.assert_screen_contains("      23")?;
    rile.assert_cursor(3, 8)?;
    rile.assert_status_contains("modified:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn c_x_r_rectangle_copy_and_yank_paste_columns() -> Result<()> {
    let file = fixtures::named_temp_file("abcdef\n123456\nuvwxyz\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("abcdef")?;
    rile.send("C-f", keys::control('f'))?;
    rile.send("C-x", keys::control('x'))?;
    rile.send("SPC", b" ")?;
    rile.send("C-f", keys::control('f'))?;
    rile.send("C-f", keys::control('f'))?;
    rile.send("C-n", keys::control('n'))?;
    rile.send("C-x", keys::control('x'))?;
    rile.send("r", b"r")?;
    rile.send("M-w", keys::meta('w'))?;
    rile.assert_screen_contains("Copied rectangle")?;

    rile.send("C-e", keys::control('e'))?;
    rile.send("C-n", keys::control('n'))?;
    rile.send("C-x", keys::control('x'))?;
    rile.send("r", b"r")?;
    rile.send("y", b"y")?;
    rile.wait_for_screen_contains("uvwxyzbc")?;
    rile.assert_screen_contains("      23")?;
    rile.assert_cursor(3, 8)?;
    rile.assert_status_contains("modified:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn join_line_merges_current_line_with_previous() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\n  beta\n\n    gamma\nlast\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("C-n", keys::control('n'))?;
    rile.assert_cursor(1, 0)?;

    rile.send("M-^", keys::meta('^'))?;
    rile.wait_for_screen_contains("alpha beta")?;
    rile.assert_cursor(0, 6)?;
    rile.assert_status_contains("modified:true")?;

    rile.send("C-n", keys::control('n'))?;
    rile.send("C-n", keys::control('n'))?;
    rile.send("M-^ blank previous", keys::meta('^'))?;
    rile.wait_for_screen_contains("gamma")?;
    rile.assert_cursor(1, 0)?;

    rile.send("undo second join", b"\x1f")?;
    rile.wait_for_screen_contains("    gamma")?;
    rile.assert_cursor(2, 6)?;

    rile.quit()?;
    Ok(())
}

#[test]
fn quoted_insert_inserts_literal_text_tab_and_newline() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("C-q", keys::control('q'))?;
    rile.assert_screen_contains("C-q-")?;
    rile.send("quoted z", b"z")?;
    rile.wait_for_screen_contains("zalpha")?;
    rile.assert_cursor(0, 1)?;

    rile.send("C-q", keys::control('q'))?;
    rile.send("quoted Tab", keys::TAB)?;
    rile.wait_for_screen_contains("z   alpha")?;
    rile.assert_cursor(0, 4)?;

    rile.send("C-q", keys::control('q'))?;
    rile.send("quoted Enter", keys::ENTER)?;
    rile.assert_screen_contains("z")?;
    rile.assert_screen_contains("alpha")?;
    rile.assert_cursor(1, 0)?;

    rile.send("C-q", keys::control('q'))?;
    rile.send("quoted C-a", keys::control('a'))?;
    rile.assert_screen_contains("Error: quoted control insertion is not supported")?;
    rile.assert_cursor(1, 0)?;
    rile.assert_status_contains("modified:true")?;

    rile.send("C-q", keys::control('q'))?;
    rile.send("cancel quoted insert", keys::control('g'))?;
    rile.assert_screen_contains("Quit")?;
    rile.assert_cursor(1, 0)?;

    rile.send("normal x after cancel", b"x")?;
    rile.wait_for_screen_contains("xalpha")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn exchange_point_and_mark_keeps_region_active() -> Result<()> {
    let file = fixtures::named_temp_file("abcdef\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("abcdef")?;
    rile.send("C-f", keys::control('f'))?;
    rile.send("C-f", keys::control('f'))?;
    rile.send("C-@", b"\0")?;
    rile.send("C-f", keys::control('f'))?;
    rile.send("C-f", keys::control('f'))?;
    rile.assert_cursor(0, 4)?;

    rile.send("C-x C-x", keys::control_sequence("xx"))?;
    rile.assert_cursor(0, 2)?;

    rile.send("C-w", keys::control('w'))?;
    rile.assert_screen_contains("abef")?;
    rile.assert_status_contains("modified:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn mark_whole_buffer_selects_entire_buffer() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\n  beta\nlast line\n")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.send("C-n", keys::control('n'))?;
    rile.send("C-f", keys::control('f'))?;
    rile.assert_cursor(1, 1)?;

    rile.send("C-x h", keys::control('x'))?;
    rile.send("h", b"h")?;
    rile.assert_cursor(0, 0)?;
    rile.assert_screen_contains("Mark set")?;
    rile.assert_status_contains("modified:false")?;

    rile.send("C-x C-x", keys::control_sequence("xx"))?;
    rile.assert_cursor(3, 0)?;
    rile.assert_status_contains("Ln 004 Col 000")?;

    rile.send("C-w", keys::control('w'))?;
    rile.assert_screen_contains("Killed region")?;
    rile.assert_status_contains("modified:true")?;
    assert!(!rile.snapshot_text().contains("alpha"));

    rile.quit()?;
    Ok(())
}

#[test]
fn kill_word_and_backward_kill_word_edit_visible_buffer() -> Result<()> {
    let file = fixtures::named_temp_file("one two three")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("one two three")?;
    rile.send("M-d", keys::meta('d'))?;
    rile.assert_screen_contains("two three")?;
    rile.assert_cursor(0, 0)?;

    rile.send("M->", keys::meta('>'))?;
    rile.assert_cursor(0, 10)?;
    rile.send("M-Backspace", keys::meta_backspace())?;
    rile.assert_screen_contains("two")?;
    assert!(!rile.snapshot_text().contains("three"));
    rile.assert_status_contains("modified:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn consecutive_kill_words_yank_as_one_entry() -> Result<()> {
    let file = fixtures::named_temp_file("one two three")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("one two three")?;
    rile.send("M-d", keys::meta('d'))?;
    rile.send("M-d", keys::meta('d'))?;
    rile.assert_screen_contains("three")?;
    assert!(!rile.snapshot_text().contains("one two three"));

    rile.send("C-y", keys::control('y'))?;
    rile.assert_screen_contains("one two three")?;
    rile.assert_status_contains("modified:true")?;

    rile.quit()?;
    Ok(())
}

#[test]
fn yank_pop_rotates_visible_yank() -> Result<()> {
    let file = fixtures::named_temp_file("one\ntwo\nthree")?;
    let mut rile = RilePty::spawn(file.path(), 12, 80)?;

    rile.wait_for_screen_contains("one")?;
    rile.send("C-k", keys::control('k'))?;
    rile.send("C-n", keys::control('n'))?;
    rile.send("C-k", keys::control('k'))?;
    rile.send("C-y", keys::control('y'))?;
    rile.assert_screen_contains("two")?;
    assert!(!rile.snapshot_text().contains("one"));

    rile.send("M-y", keys::meta('y'))?;
    rile.assert_screen_contains("one")?;
    assert!(!rile.snapshot_text().contains("two"));

    rile.send("M-y again", keys::meta('y'))?;
    rile.assert_screen_contains("two")?;
    assert!(!rile.snapshot_text().contains("one"));

    rile.quit()?;
    Ok(())
}
