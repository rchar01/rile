// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{Position, TextRange};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndoRecord {
    Insert {
        range: TextRange,
        text: String,
        cursor_before: Position,
        cursor_after: Position,
    },
    Delete {
        range: TextRange,
        text: String,
        cursor_before: Position,
        cursor_after: Position,
    },
    Replace {
        range: TextRange,
        old_text: String,
        new_text: String,
        cursor_before: Position,
        cursor_after: Position,
    },
}
