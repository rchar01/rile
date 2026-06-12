// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt::Write as _;

pub fn text(screen: &vt100::Screen) -> String {
    let (_, columns) = screen.size();
    screen
        .rows(0, columns)
        .map(|row| row.trim_end().to_owned())
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn dump(screen: &vt100::Screen) -> String {
    let mut output = String::new();
    let (cursor_row, cursor_column) = screen.cursor_position();
    let rows = text(screen);
    for (index, row) in rows.lines().enumerate() {
        let _ = writeln!(output, "{:03}: {row}", index + 1);
        if index == usize::from(cursor_row) {
            let _ = writeln!(output, "     {}^", " ".repeat(usize::from(cursor_column)));
        }
    }
    output
}
