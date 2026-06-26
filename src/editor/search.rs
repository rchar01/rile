// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::Result;
use crate::buffer::{Buffer, Position, TextRange};

use super::SearchDirection;

pub(super) fn find_match(
    buffer: &Buffer,
    query: &str,
    start: Position,
    direction: SearchDirection,
) -> Result<Option<TextRange>> {
    buffer.validate_position(start)?;
    if query.is_empty() {
        return Ok(None);
    }

    match direction {
        SearchDirection::Forward => find_forward(buffer, query, start),
        SearchDirection::Backward => find_backward(buffer, query, start),
    }
}

fn find_forward(buffer: &Buffer, query: &str, start: Position) -> Result<Option<TextRange>> {
    for line_index in start.line..buffer.line_count() {
        let line = buffer.line(line_index).expect("line index is in range");
        let minimum_byte = if line_index == start.line {
            start.byte
        } else {
            0
        };
        if let Some((match_start, match_text)) = line
            .match_indices(query)
            .find(|(match_start, _)| *match_start >= minimum_byte)
        {
            return Ok(Some(TextRange::new(
                Position::new(line_index, match_start),
                Position::new(line_index, match_start + match_text.len()),
            )));
        }
    }
    Ok(None)
}

fn find_backward(buffer: &Buffer, query: &str, start: Position) -> Result<Option<TextRange>> {
    for line_index in (0..=start.line).rev() {
        let line = buffer.line(line_index).expect("line index is in range");
        let maximum_byte = if line_index == start.line {
            start.byte
        } else {
            line.len()
        };
        if let Some((match_start, match_text)) = line
            .match_indices(query)
            .filter(|(match_start, _)| *match_start < maximum_byte)
            .last()
        {
            return Ok(Some(TextRange::new(
                Position::new(line_index, match_start),
                Position::new(line_index, match_start + match_text.len()),
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

#[cfg(test)]
mod tests {
    use crate::file::Document;

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
            "écho",
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
            "foo",
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
            "foo",
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
}
