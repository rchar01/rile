// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::Result;
use crate::buffer::{Buffer, Position, TextRange};
use crate::search_pattern::SearchPattern;

use super::SearchDirection;

pub(super) fn find_match(
    buffer: &Buffer,
    pattern: &SearchPattern,
    start: Position,
    direction: SearchDirection,
) -> Result<Option<TextRange>> {
    Ok(find_pattern_match(buffer, pattern, start, direction)?
        .map(|pattern_match| pattern_match.range))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct EditorPatternMatch {
    pub(super) range: TextRange,
    pub(super) captures: Vec<Option<TextRange>>,
}

pub(super) fn find_pattern_match(
    buffer: &Buffer,
    pattern: &SearchPattern,
    start: Position,
    direction: SearchDirection,
) -> Result<Option<EditorPatternMatch>> {
    buffer.validate_position(start)?;

    match direction {
        SearchDirection::Forward => find_forward(buffer, pattern, start),
        SearchDirection::Backward => find_backward(buffer, pattern, start).map(|range| {
            range.map(|range| EditorPatternMatch {
                range,
                captures: Vec::new(),
            })
        }),
    }
}

fn find_forward(
    buffer: &Buffer,
    pattern: &SearchPattern,
    start: Position,
) -> Result<Option<EditorPatternMatch>> {
    for line_index in start.line..buffer.line_count() {
        let line = buffer.line(line_index).expect("line index is in range");
        let minimum_byte = if line_index == start.line {
            start.byte
        } else {
            0
        };
        if let Some(pattern_match) = pattern.find_forward_match_in_line(line, minimum_byte) {
            return Ok(Some(EditorPatternMatch {
                range: TextRange::new(
                    Position::new(line_index, pattern_match.range.0),
                    Position::new(line_index, pattern_match.range.1),
                ),
                captures: pattern_match
                    .captures
                    .into_iter()
                    .map(|range| {
                        range.map(|(start, end)| {
                            TextRange::new(
                                Position::new(line_index, start),
                                Position::new(line_index, end),
                            )
                        })
                    })
                    .collect(),
            }));
        }
    }
    Ok(None)
}

fn find_backward(
    buffer: &Buffer,
    pattern: &SearchPattern,
    start: Position,
) -> Result<Option<TextRange>> {
    for line_index in (0..=start.line).rev() {
        let line = buffer.line(line_index).expect("line index is in range");
        let maximum_byte = if line_index == start.line {
            start.byte
        } else {
            line.len()
        };
        if let Some((match_start, match_end)) = pattern.find_backward_in_line(line, maximum_byte) {
            return Ok(Some(TextRange::new(
                Position::new(line_index, match_start),
                Position::new(line_index, match_end),
            )));
        }
    }
    Ok(None)
}

pub(super) fn search_start_after(buffer: &Buffer, position: Position) -> Result<Position> {
    buffer.validate_position(position)?;
    let line = buffer.line(position.line).expect("line index is in range");
    if position.byte < line.len() {
        let character_width = line[position.byte..]
            .chars()
            .next()
            .expect("position before line end has a character")
            .len_utf8();
        return Ok(Position::new(
            position.line,
            position.byte + character_width,
        ));
    }
    if position.line + 1 < buffer.line_count() {
        return Ok(Position::new(position.line + 1, 0));
    }
    Ok(buffer.end_position())
}

pub(super) fn search_start_before(buffer: &Buffer, position: Position) -> Result<Position> {
    buffer.validate_position(position)?;
    let line = buffer.line(position.line).expect("line index is in range");
    if position.byte > 0 {
        let character_width = line[..position.byte]
            .chars()
            .next_back()
            .expect("position after line start has a previous character")
            .len_utf8();
        return Ok(Position::new(
            position.line,
            position.byte - character_width,
        ));
    }
    if position.line > 0 {
        let previous_line = position.line - 1;
        let previous_line_len = buffer
            .line(previous_line)
            .expect("previous line index is in range")
            .len();
        return Ok(Position::new(previous_line, previous_line_len));
    }
    Ok(Position::new(0, 0))
}

#[cfg(test)]
mod tests {
    use crate::file::Document;
    use crate::search_pattern::{PatternKind, SearchPattern};

    use super::*;

    fn document_with(text: &str) -> Document {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), text)
            .expect("fixture should insert");
        document
    }

    #[test]
    fn find_match_returns_utf8_byte_range() {
        let document = document_with("alpha\nécho écho");

        let range = find_match(
            document.buffer(),
            &SearchPattern::compile(PatternKind::Literal, "écho").expect("pattern compiles"),
            Position::new(0, 0),
            SearchDirection::Forward,
        )
        .expect("search should succeed")
        .expect("match should exist");

        assert_eq!(
            range,
            TextRange::new(Position::new(1, 0), Position::new(1, "écho".len()))
        );
    }

    #[test]
    fn find_match_respects_forward_and_backward_start_boundaries() {
        let document = document_with("foo bar foo");

        let forward = find_match(
            document.buffer(),
            &SearchPattern::compile(PatternKind::Literal, "foo").expect("pattern compiles"),
            Position::new(0, 1),
            SearchDirection::Forward,
        )
        .expect("forward search should succeed")
        .expect("forward match should exist");
        assert_eq!(
            forward,
            TextRange::new(Position::new(0, 8), Position::new(0, 11))
        );

        let backward = find_match(
            document.buffer(),
            &SearchPattern::compile(PatternKind::Literal, "foo").expect("pattern compiles"),
            Position::new(0, 8),
            SearchDirection::Backward,
        )
        .expect("backward search should succeed")
        .expect("backward match should exist");
        assert_eq!(
            backward,
            TextRange::new(Position::new(0, 0), Position::new(0, 3))
        );
    }

    #[test]
    fn search_start_after_advances_by_utf8_character() {
        let document = document_with("éx\ny");

        let after_e_acute = search_start_after(document.buffer(), Position::new(0, 0))
            .expect("start after first character should succeed");
        assert_eq!(after_e_acute, Position::new(0, "é".len()));

        let next_line = search_start_after(document.buffer(), Position::new(0, "éx".len()))
            .expect("start after line end should succeed");
        assert_eq!(next_line, Position::new(1, 0));
    }

    #[test]
    fn search_start_before_retreats_by_utf8_character() {
        let document = document_with("éx\ny");

        let before_x = search_start_before(document.buffer(), Position::new(0, "éx".len()))
            .expect("start before line end should succeed");
        assert_eq!(before_x, Position::new(0, "é".len()));

        let previous_line = search_start_before(document.buffer(), Position::new(1, 0))
            .expect("start before line start should succeed");
        assert_eq!(previous_line, Position::new(0, "éx".len()));
    }
}
