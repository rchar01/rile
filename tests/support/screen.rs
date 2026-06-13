// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt::Write as _;

pub fn text(screen: &vt100::Screen) -> String {
    rows(screen).join("\n")
}

pub fn dump(screen: &vt100::Screen) -> String {
    let mut output = String::new();
    let (cursor_row, cursor_column) = screen.cursor_position();
    for (index, row) in rows(screen).into_iter().enumerate() {
        let _ = if row.is_empty() {
            writeln!(output, "{:03}:", index + 1)
        } else {
            writeln!(output, "{:03}: {row}", index + 1)
        };
        if index == usize::from(cursor_row) {
            let _ = writeln!(output, "     {}^", " ".repeat(usize::from(cursor_column)));
        }
    }
    output
}

pub fn snapshot(screen: &vt100::Screen) -> String {
    let (rows, columns) = screen.size();
    let (cursor_row, cursor_column) = screen.cursor_position();
    format!(
        "size: {columns}x{rows}\ncursor: row {cursor_row}, column {cursor_column}\n{}",
        dump(screen)
    )
}

fn rows(screen: &vt100::Screen) -> Vec<String> {
    let (_, columns) = screen.size();
    screen
        .rows(0, columns)
        .map(|row| row.trim_end().to_owned())
        .collect()
}
