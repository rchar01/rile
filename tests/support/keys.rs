// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

pub const BACKSPACE: &[u8] = b"\x7f";
pub const DELETE: &[u8] = b"\x1b[3~";
pub const DOWN: &[u8] = b"\x1b[B";
pub const ENTER: &[u8] = b"\r";
pub const LEFT: &[u8] = b"\x1b[D";
pub const PAGE_DOWN: &[u8] = b"\x1b[6~";
pub const PAGE_UP: &[u8] = b"\x1b[5~";
pub const RIGHT: &[u8] = b"\x1b[C";
pub const TAB: &[u8] = b"\t";
pub const UP: &[u8] = b"\x1b[A";

pub fn control(letter: char) -> [u8; 1] {
    assert!(letter.is_ascii_alphabetic());
    [(letter.to_ascii_lowercase() as u8) - b'a' + 1]
}

pub fn control_sequence(letters: &str) -> Vec<u8> {
    letters.chars().flat_map(control).collect::<Vec<u8>>()
}

pub fn meta(letter: char) -> Vec<u8> {
    assert!(letter.is_ascii());
    vec![b'\x1b', letter as u8]
}

pub fn meta_backspace() -> Vec<u8> {
    vec![b'\x1b', b'\x7f']
}
