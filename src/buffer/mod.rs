// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ops::Range;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::text::{move_word_backward_byte, move_word_forward_byte};
use crate::{Result, RileError};

pub mod undo;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Position {
    pub line: usize,
    pub byte: usize,
}

impl Position {
    pub const fn new(line: usize, byte: usize) -> Self {
        Self { line, byte }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextRange {
    pub start: Position,
    pub end: Position,
}

pub type RectangleEdit = (TextRange, String);

impl TextRange {
    pub const fn new(start: Position, end: Position) -> Self {
        Self { start, end }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Buffer {
    lines: Vec<String>,
    dirty: bool,
    final_newline: bool,
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            dirty: false,
            final_newline: false,
        }
    }

    pub fn from_text(text: &str) -> Self {
        let mut buffer = Self {
            lines: text.split('\n').map(str::to_owned).collect(),
            dirty: false,
            final_newline: text.ends_with('\n'),
        };
        if buffer.lines.is_empty() {
            buffer.lines.push(String::new());
        }
        buffer
    }

    pub fn serialize(&self) -> String {
        self.lines.join("\n")
    }

    pub fn serialized_len(&self) -> usize {
        self.lines.iter().map(String::len).sum::<usize>() + self.lines.len().saturating_sub(1)
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn line(&self, line: usize) -> Option<&str> {
        self.lines.get(line).map(String::as_str)
    }

    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn end_position(&self) -> Position {
        let line = self.lines.len() - 1;
        Position::new(line, self.lines[line].len())
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn mark_clean(&mut self) {
        self.dirty = false;
    }

    pub fn final_newline(&self) -> bool {
        self.final_newline
    }

    pub fn validate_position(&self, position: Position) -> Result<()> {
        let Some(line) = self.lines.get(position.line) else {
            return Err(RileError::InvalidPosition(format!(
                "line {} is outside buffer with {} lines",
                position.line,
                self.lines.len()
            )));
        };
        if position.byte > line.len() {
            return Err(RileError::InvalidPosition(format!(
                "byte {} is past end of line {} with {} bytes",
                position.byte,
                position.line,
                line.len()
            )));
        }
        if !line.is_char_boundary(position.byte) {
            return Err(RileError::InvalidPosition(format!(
                "byte {} in line {} is not a UTF-8 boundary",
                position.byte, position.line
            )));
        }
        Ok(())
    }

    pub fn validate_range(&self, range: TextRange) -> Result<()> {
        self.validate_position(range.start)?;
        self.validate_position(range.end)?;
        if range.start > range.end {
            return Err(RileError::InvalidPosition(
                "range start must not be after range end".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn insert(&mut self, at: Position, text: &str) -> Result<Position> {
        self.validate_position(at)?;
        if text.is_empty() {
            return Ok(at);
        }

        let inserted_lines: Vec<&str> = text.split('\n').collect();
        let line = self.lines[at.line].clone();
        let prefix = &line[..at.byte];
        let suffix = &line[at.byte..];

        let end = if inserted_lines.len() == 1 {
            self.lines[at.line].insert_str(at.byte, text);
            Position::new(at.line, at.byte + text.len())
        } else {
            let mut replacement = Vec::with_capacity(inserted_lines.len());
            replacement.push(format!("{}{}", prefix, inserted_lines[0]));
            for part in &inserted_lines[1..inserted_lines.len() - 1] {
                replacement.push((*part).to_owned());
            }
            let last = inserted_lines[inserted_lines.len() - 1];
            replacement.push(format!("{last}{suffix}"));

            let end = Position::new(at.line + inserted_lines.len() - 1, last.len());
            self.lines.splice(at.line..=at.line, replacement);
            end
        };

        self.dirty = true;
        self.recompute_final_newline();
        Ok(end)
    }

    pub fn delete_range(&mut self, range: TextRange) -> Result<String> {
        self.validate_range(range)?;
        if range.start == range.end {
            return Ok(String::new());
        }

        let deleted = self.text_in_range(range)?;
        if range.start.line == range.end.line {
            self.lines[range.start.line].replace_range(range.start.byte..range.end.byte, "");
        } else {
            let prefix = self.lines[range.start.line][..range.start.byte].to_owned();
            let suffix = self.lines[range.end.line][range.end.byte..].to_owned();
            self.lines.splice(
                range.start.line..=range.end.line,
                [format!("{prefix}{suffix}")],
            );
        }

        if self.lines.is_empty() {
            self.lines.push(String::new());
        }
        self.dirty = true;
        self.recompute_final_newline();
        Ok(deleted)
    }

    pub fn text_in_range(&self, range: TextRange) -> Result<String> {
        self.validate_range(range)?;
        if range.start == range.end {
            return Ok(String::new());
        }
        if range.start.line == range.end.line {
            return Ok(self.lines[range.start.line][range.start.byte..range.end.byte].to_owned());
        }

        let mut text = String::new();
        text.push_str(&self.lines[range.start.line][range.start.byte..]);
        text.push('\n');
        for line in range.start.line + 1..range.end.line {
            text.push_str(&self.lines[line]);
            text.push('\n');
        }
        text.push_str(&self.lines[range.end.line][..range.end.byte]);
        Ok(text)
    }

    pub fn text_in_display_rectangle(
        &self,
        start_line: usize,
        end_line: usize,
        start_column: usize,
        end_column: usize,
    ) -> Result<Vec<String>> {
        self.validate_display_rectangle(start_line, end_line, start_column, end_column)?;

        let mut text = Vec::new();
        for line_index in start_line..=end_line {
            let line = &self.lines[line_index];
            let start = self.byte_for_display_column(line_index, start_column)?;
            let end = self.byte_for_display_column(line_index, end_column)?;
            text.push(line[start..end].to_owned());
        }
        Ok(text)
    }

    pub fn delete_display_rectangle(
        &mut self,
        start_line: usize,
        end_line: usize,
        start_column: usize,
        end_column: usize,
    ) -> Result<(Vec<String>, Vec<RectangleEdit>)> {
        let text =
            self.text_in_display_rectangle(start_line, end_line, start_column, end_column)?;
        let mut deleted = Vec::new();

        for line_index in start_line..=end_line {
            let start = self.byte_for_display_column(line_index, start_column)?;
            let end = self.byte_for_display_column(line_index, end_column)?;
            if start == end {
                continue;
            }
            let range = TextRange::new(
                Position::new(line_index, start),
                Position::new(line_index, end),
            );
            let line_text = self.lines[line_index][start..end].to_owned();
            self.lines[line_index].replace_range(start..end, "");
            deleted.push((range, line_text));
        }

        if !deleted.is_empty() {
            self.dirty = true;
            self.recompute_final_newline();
        }

        Ok((text, deleted))
    }

    pub fn insert_display_rectangle(
        &mut self,
        at: Position,
        column: usize,
        rectangle: &[String],
    ) -> Result<(Vec<RectangleEdit>, Position)> {
        self.validate_position(at)?;
        let mut inserted = Vec::new();
        let mut cursor_after = at;

        for (offset, rectangle_line) in rectangle.iter().enumerate() {
            let line_index = at.line + offset;
            while line_index >= self.lines.len() {
                self.lines.push(String::new());
            }

            let line_width = Self::display_width(&self.lines[line_index]);
            let padding = column.saturating_sub(line_width);
            let mut text = String::new();
            text.extend(std::iter::repeat_n(' ', padding));
            text.push_str(rectangle_line);

            if text.is_empty() {
                cursor_after = Position::new(
                    line_index,
                    self.byte_for_display_column(line_index, column)?,
                );
                continue;
            }

            let byte = if padding > 0 {
                self.lines[line_index].len()
            } else {
                self.byte_for_display_column(line_index, column)?
            };
            self.lines[line_index].insert_str(byte, &text);
            let range = TextRange::new(
                Position::new(line_index, byte),
                Position::new(line_index, byte + text.len()),
            );
            cursor_after = range.end;
            inserted.push((range, text));
        }

        if !inserted.is_empty() {
            self.dirty = true;
            self.recompute_final_newline();
        }

        Ok((inserted, cursor_after))
    }

    pub fn move_grapheme_forward(&self, position: Position) -> Result<Position> {
        self.validate_position(position)?;
        let line = &self.lines[position.line];
        if position.byte < line.len() {
            let rest = &line[position.byte..];
            let next_len = rest
                .graphemes(true)
                .next()
                .expect("non-empty string has a grapheme")
                .len();
            return Ok(Position::new(position.line, position.byte + next_len));
        }
        if position.line + 1 < self.lines.len() {
            return Ok(Position::new(position.line + 1, 0));
        }
        Ok(position)
    }

    pub fn move_grapheme_backward(&self, position: Position) -> Result<Position> {
        self.validate_position(position)?;
        if position.byte > 0 {
            let line = &self.lines[position.line][..position.byte];
            let previous = line
                .grapheme_indices(true)
                .next_back()
                .expect("non-empty string has a grapheme")
                .0;
            return Ok(Position::new(position.line, previous));
        }
        if position.line > 0 {
            let previous_line = position.line - 1;
            return Ok(Position::new(
                previous_line,
                self.lines[previous_line].len(),
            ));
        }
        Ok(position)
    }

    pub fn move_word_forward(&self, position: Position) -> Result<Position> {
        self.validate_position(position)?;
        let text = self.serialize();
        let absolute = self.absolute_offset(position);
        self.position_for_absolute(move_word_forward_byte(&text, absolute))
    }

    pub fn move_word_backward(&self, position: Position) -> Result<Position> {
        self.validate_position(position)?;
        let text = self.serialize();
        let absolute = self.absolute_offset(position);
        self.position_for_absolute(move_word_backward_byte(&text, absolute))
    }

    pub fn move_line(
        &self,
        position: Position,
        delta: isize,
        goal_display_column: Option<usize>,
    ) -> Result<(Position, usize)> {
        self.validate_position(position)?;
        let goal = match goal_display_column {
            Some(column) => column,
            None => self.display_column(position)?,
        };
        let target_line = position
            .line
            .saturating_add_signed(delta)
            .min(self.lines.len() - 1);
        let target_byte = self.byte_for_display_column(target_line, goal)?;
        Ok((Position::new(target_line, target_byte), goal))
    }

    pub fn display_column(&self, position: Position) -> Result<usize> {
        self.validate_position(position)?;
        Ok(UnicodeWidthStr::width(
            &self.lines[position.line][..position.byte],
        ))
    }

    pub fn byte_for_display_column(&self, line: usize, target_column: usize) -> Result<usize> {
        let Some(text) = self.lines.get(line) else {
            return Err(RileError::InvalidPosition(format!(
                "line {line} is outside buffer with {} lines",
                self.lines.len()
            )));
        };

        let mut column = 0;
        for (byte, character) in text.char_indices() {
            let width = character.width().unwrap_or(0);
            if column + width > target_column {
                return Ok(byte);
            }
            column += width;
        }
        Ok(text.len())
    }

    pub fn display_width(text: &str) -> usize {
        UnicodeWidthStr::width(text)
    }

    pub fn visible_range(
        &self,
        line: usize,
        start_display_column: usize,
        width: usize,
    ) -> Result<Range<usize>> {
        let start = self.byte_for_display_column(line, start_display_column)?;
        let end = self.byte_for_display_column(line, start_display_column + width)?;
        Ok(start..end)
    }

    fn validate_display_rectangle(
        &self,
        start_line: usize,
        end_line: usize,
        start_column: usize,
        end_column: usize,
    ) -> Result<()> {
        if start_line > end_line {
            return Err(RileError::InvalidPosition(
                "rectangle start line must not be after end line".to_owned(),
            ));
        }
        if start_column > end_column {
            return Err(RileError::InvalidPosition(
                "rectangle start column must not be after end column".to_owned(),
            ));
        }
        if end_line >= self.lines.len() {
            return Err(RileError::InvalidPosition(format!(
                "line {end_line} is outside buffer with {} lines",
                self.lines.len()
            )));
        }
        Ok(())
    }

    fn recompute_final_newline(&mut self) {
        self.final_newline =
            self.lines.len() > 1 && self.lines.last().is_some_and(String::is_empty);
    }

    fn absolute_offset(&self, position: Position) -> usize {
        let preceding_lines: usize = self.lines[..position.line]
            .iter()
            .map(|line| line.len() + 1)
            .sum();
        preceding_lines + position.byte
    }

    fn position_for_absolute(&self, offset: usize) -> Result<Position> {
        let mut remaining = offset;
        for (line_index, line) in self.lines.iter().enumerate() {
            if remaining <= line.len() {
                return Ok(Position::new(line_index, remaining));
            }
            remaining -= line.len();
            if remaining == 0 {
                return Ok(Position::new(line_index, line.len()));
            }
            remaining -= 1;
        }
        Ok(self.end_position())
    }
}

#[cfg(test)]
mod tests {
    use super::{Buffer, Position, TextRange};

    #[test]
    fn loads_and_serializes_text_with_final_newline_tracking() {
        let empty = Buffer::from_text("");
        assert_eq!(empty.line_count(), 1);
        assert!(!empty.final_newline());
        assert_eq!(empty.serialize(), "");

        let buffer = Buffer::from_text("alpha\nbeta\n");
        assert_eq!(buffer.lines(), &["alpha", "beta", ""]);
        assert!(buffer.final_newline());
        assert_eq!(buffer.serialize(), "alpha\nbeta\n");
    }

    #[test]
    fn serialized_len_counts_text_bytes_and_line_separators() {
        for text in ["", "alpha", "alpha\nbeta\n", "\n\n", "é\r\n"] {
            assert_eq!(Buffer::from_text(text).serialized_len(), text.len());
        }
    }

    #[test]
    fn insert_text_splits_lines_and_sets_dirty_flag() {
        let mut buffer = Buffer::from_text("ab");
        let end = buffer
            .insert(Position::new(0, 1), "x\ny")
            .expect("insert should succeed");

        assert_eq!(end, Position::new(1, 1));
        assert_eq!(buffer.serialize(), "ax\nyb");
        assert!(buffer.is_dirty());
        assert!(!buffer.final_newline());
    }

    #[test]
    fn insert_trailing_newline_tracks_final_newline() {
        let mut buffer = Buffer::new();
        let end = buffer
            .insert(Position::new(0, 0), "hello\n")
            .expect("insert should succeed");

        assert_eq!(end, Position::new(1, 0));
        assert_eq!(buffer.lines(), &["hello", ""]);
        assert!(buffer.final_newline());
        assert_eq!(buffer.serialize(), "hello\n");
    }

    #[test]
    fn rejects_positions_inside_utf8_codepoints() {
        let buffer = Buffer::from_text("é");
        let error = buffer
            .validate_position(Position::new(0, 1))
            .expect_err("middle byte should be invalid");

        assert!(error.to_string().contains("not a UTF-8 boundary"));
    }

    #[test]
    fn deletes_ranges_across_lines() {
        let mut buffer = Buffer::from_text("alpha\nbeta\ngamma");
        let deleted = buffer
            .delete_range(TextRange::new(Position::new(0, 2), Position::new(2, 2)))
            .expect("delete should succeed");

        assert_eq!(deleted, "pha\nbeta\nga");
        assert_eq!(buffer.serialize(), "almma");
        assert!(buffer.is_dirty());
    }

    #[test]
    fn moves_by_grapheme_clusters() {
        let buffer = Buffer::from_text("ae\u{301}🙂");
        let after_a = buffer
            .move_grapheme_forward(Position::new(0, 0))
            .expect("movement should succeed");
        let after_combining = buffer
            .move_grapheme_forward(after_a)
            .expect("movement should succeed");
        let back_to_combining = buffer
            .move_grapheme_backward(buffer.end_position())
            .expect("movement should succeed");

        assert_eq!(after_a, Position::new(0, 1));
        assert_eq!(after_combining, Position::new(0, 4));
        assert_eq!(back_to_combining, Position::new(0, 4));
    }

    #[test]
    fn computes_display_columns_and_visible_ranges() {
        let buffer = Buffer::from_text("a界b");
        assert_eq!(Buffer::display_width("e\u{301}"), 1);
        assert_eq!(Buffer::display_width("a界"), 3);
        assert_eq!(
            buffer
                .display_column(Position::new(0, "a界".len()))
                .expect("column should compute"),
            3
        );
        assert_eq!(
            buffer
                .visible_range(0, 1, 2)
                .expect("visible range should compute"),
            1..4
        );
    }

    #[test]
    fn display_rectangles_handle_short_lines_and_wide_characters() {
        let mut buffer = Buffer::from_text("abcd\nx\na界b");
        let text = buffer
            .text_in_display_rectangle(0, 2, 1, 3)
            .expect("rectangle text should compute");
        assert_eq!(text, vec!["bc", "", "界"]);

        let (deleted, edits) = buffer
            .delete_display_rectangle(0, 2, 1, 3)
            .expect("rectangle delete should work");
        assert_eq!(deleted, vec!["bc", "", "界"]);
        assert_eq!(edits.len(), 2);
        assert_eq!(buffer.serialize(), "ad\nx\nab");

        let (inserted, cursor) = buffer
            .insert_display_rectangle(Position::new(1, 1), 3, &deleted)
            .expect("rectangle insert should work");
        assert_eq!(inserted.len(), 3);
        assert_eq!(buffer.serialize(), "ad\nx  bc\nab \n   界");
        assert_eq!(cursor, Position::new(3, "   界".len()));
    }

    #[test]
    fn moves_lines_using_goal_display_column() {
        let buffer = Buffer::from_text("abcd\na界d\nxy");
        let (line_one, goal) = buffer
            .move_line(Position::new(0, 3), 1, None)
            .expect("line movement should succeed");
        let (line_two, same_goal) = buffer
            .move_line(line_one, 1, Some(goal))
            .expect("line movement should succeed");

        assert_eq!(line_one, Position::new(1, "a界".len()));
        assert_eq!(line_two, Position::new(2, 2));
        assert_eq!(same_goal, 3);
    }

    #[test]
    fn moves_by_words() {
        let buffer = Buffer::from_text("one two_3\n四 five");
        assert_eq!(
            buffer
                .move_word_forward(Position::new(0, 0))
                .expect("word movement should succeed"),
            Position::new(0, 3)
        );
        assert_eq!(
            buffer
                .move_word_backward(Position::new(1, "四 five".len()))
                .expect("word movement should succeed"),
            Position::new(1, "四 ".len())
        );
    }
}
