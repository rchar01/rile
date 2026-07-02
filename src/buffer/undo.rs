// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use super::{Position, TextRange};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UndoRecord {
    Batch(Vec<UndoRecord>),
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

impl UndoRecord {
    pub fn inverse(&self) -> Self {
        match self {
            Self::Batch(records) => Self::Batch(records.iter().rev().map(Self::inverse).collect()),
            Self::Insert {
                range,
                text,
                cursor_before,
                cursor_after,
            } => Self::Delete {
                range: *range,
                text: text.clone(),
                cursor_before: *cursor_after,
                cursor_after: *cursor_before,
            },
            Self::Delete {
                range,
                text,
                cursor_before,
                cursor_after,
            } => Self::Insert {
                range: *range,
                text: text.clone(),
                cursor_before: *cursor_after,
                cursor_after: *cursor_before,
            },
            Self::Replace {
                range,
                old_text,
                new_text,
                cursor_before,
                cursor_after,
            } => Self::Replace {
                range: TextRange::new(range.start, position_after_text(range.start, old_text)),
                old_text: new_text.clone(),
                new_text: old_text.clone(),
                cursor_before: *cursor_after,
                cursor_after: *cursor_before,
            },
        }
    }
}

fn position_after_text(start: Position, text: &str) -> Position {
    let mut parts = text.split('\n');
    let first = parts.next().unwrap_or("");
    let mut line = start.line;
    let mut byte = start.byte + first.len();
    for part in parts {
        line += 1;
        byte = part.len();
    }
    Position::new(line, byte)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse_insert_becomes_delete() {
        let record = UndoRecord::Insert {
            range: TextRange::new(Position::new(0, 1), Position::new(0, 2)),
            text: "x".to_owned(),
            cursor_before: Position::new(0, 1),
            cursor_after: Position::new(0, 2),
        };

        assert_eq!(
            record.inverse(),
            UndoRecord::Delete {
                range: TextRange::new(Position::new(0, 1), Position::new(0, 2)),
                text: "x".to_owned(),
                cursor_before: Position::new(0, 2),
                cursor_after: Position::new(0, 1),
            }
        );
    }

    #[test]
    fn inverse_delete_becomes_insert() {
        let record = UndoRecord::Delete {
            range: TextRange::new(Position::new(0, 1), Position::new(1, 1)),
            text: "x\ny".to_owned(),
            cursor_before: Position::new(0, 1),
            cursor_after: Position::new(0, 1),
        };

        assert_eq!(
            record.inverse(),
            UndoRecord::Insert {
                range: TextRange::new(Position::new(0, 1), Position::new(1, 1)),
                text: "x\ny".to_owned(),
                cursor_before: Position::new(0, 1),
                cursor_after: Position::new(0, 1),
            }
        );
    }

    #[test]
    fn inverse_replace_swaps_text_and_uses_old_text_range() {
        let record = UndoRecord::Replace {
            range: TextRange::new(Position::new(0, 1), Position::new(0, 2)),
            old_text: "long".to_owned(),
            new_text: "x".to_owned(),
            cursor_before: Position::new(0, 1),
            cursor_after: Position::new(0, 2),
        };

        assert_eq!(
            record.inverse(),
            UndoRecord::Replace {
                range: TextRange::new(Position::new(0, 1), Position::new(0, 5)),
                old_text: "x".to_owned(),
                new_text: "long".to_owned(),
                cursor_before: Position::new(0, 2),
                cursor_after: Position::new(0, 1),
            }
        );
    }

    #[test]
    fn inverse_batch_reverses_record_order() {
        let first = UndoRecord::Insert {
            range: TextRange::new(Position::new(0, 0), Position::new(0, 1)),
            text: "a".to_owned(),
            cursor_before: Position::new(0, 0),
            cursor_after: Position::new(0, 1),
        };
        let second = UndoRecord::Delete {
            range: TextRange::new(Position::new(0, 1), Position::new(0, 2)),
            text: "b".to_owned(),
            cursor_before: Position::new(0, 2),
            cursor_after: Position::new(0, 1),
        };

        assert_eq!(
            UndoRecord::Batch(vec![first.clone(), second.clone()]).inverse(),
            UndoRecord::Batch(vec![second.inverse(), first.inverse()])
        );
    }
}
