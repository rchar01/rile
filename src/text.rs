// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use unicode_segmentation::UnicodeSegmentation;

pub(crate) fn is_word_character(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

pub(crate) fn move_word_forward_byte(text: &str, byte: usize) -> usize {
    assert!(text.is_char_boundary(byte));
    let mut seen_word = false;

    for (offset, character) in text[byte..].char_indices() {
        let is_word = is_word_character(character);
        if seen_word && !is_word {
            return next_grapheme_boundary(text, byte + offset);
        }
        seen_word |= is_word;
    }

    text.len()
}

pub(crate) fn move_word_backward_byte(text: &str, byte: usize) -> usize {
    assert!(text.is_char_boundary(byte));
    let mut word_start = None;
    let mut in_word = false;

    for (offset, character) in text[..byte].char_indices() {
        if is_word_character(character) {
            if !in_word {
                word_start = Some(offset);
            }
            in_word = true;
        } else {
            in_word = false;
        }
    }

    previous_grapheme_boundary(text, word_start.unwrap_or(0))
}

fn next_grapheme_boundary(text: &str, byte: usize) -> usize {
    for (start, grapheme) in text.grapheme_indices(true) {
        let end = start + grapheme.len();
        if byte <= start {
            return start;
        }
        if byte < end {
            return end;
        }
    }
    text.len()
}

fn previous_grapheme_boundary(text: &str, byte: usize) -> usize {
    for (start, grapheme) in text.grapheme_indices(true) {
        let end = start + grapheme.len();
        if byte == start || byte == end {
            return byte;
        }
        if byte < end {
            return start;
        }
    }
    text.len()
}

#[cfg(test)]
mod tests {
    use super::{move_word_backward_byte, move_word_forward_byte};

    #[test]
    fn moves_forward_by_word_boundaries() {
        assert_eq!(move_word_forward_byte("one two", 0), 3);
        assert_eq!(move_word_forward_byte("one two", 3), 7);
        assert_eq!(move_word_forward_byte("one two", 7), 7);
    }

    #[test]
    fn moves_backward_by_word_boundaries() {
        assert_eq!(move_word_backward_byte("one two", 7), 4);
        assert_eq!(move_word_backward_byte("one two", 4), 0);
        assert_eq!(move_word_backward_byte("one two", 0), 0);
    }

    #[test]
    fn treats_underscore_and_unicode_alnum_as_word_characters() {
        assert_eq!(move_word_forward_byte("déjà_vu next", 0), "déjà_vu".len());
        assert_eq!(move_word_backward_byte("déjà_vu next", "déjà_vu ".len()), 0);
    }

    #[test]
    fn word_movement_preserves_grapheme_boundaries() {
        let text = "e\u{301} next";

        assert_eq!(move_word_forward_byte(text, 0), "e\u{301}".len());
        assert_eq!(move_word_backward_byte(text, "e".len()), 0);
    }
}
