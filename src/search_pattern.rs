// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PatternKind {
    Literal,
    Regexp,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PatternError {
    message: String,
}

impl PatternError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SearchPattern {
    kind: PatternKind,
    literal: String,
    regexp: Option<RegexpPattern>,
}

impl SearchPattern {
    pub(crate) fn compile(kind: PatternKind, input: &str) -> Result<Self, PatternError> {
        let regexp = match kind {
            PatternKind::Literal => None,
            PatternKind::Regexp => Some(RegexpPattern::compile(input)?),
        };
        Ok(Self {
            kind,
            literal: input.to_owned(),
            regexp,
        })
    }

    pub(crate) fn find_forward_in_line(
        &self,
        line: &str,
        minimum_byte: usize,
    ) -> Option<(usize, usize)> {
        match self.kind {
            PatternKind::Literal => line
                .match_indices(&self.literal)
                .find(|(start, _)| *start >= minimum_byte)
                .map(|(start, text)| (start, start + text.len())),
            PatternKind::Regexp => self
                .regexp
                .as_ref()
                .expect("regexp kind should have compiled pattern")
                .find_forward(line, minimum_byte),
        }
    }

    pub(crate) fn find_backward_in_line(
        &self,
        line: &str,
        maximum_byte: usize,
    ) -> Option<(usize, usize)> {
        match self.kind {
            PatternKind::Literal => line
                .match_indices(&self.literal)
                .filter(|(start, _)| *start < maximum_byte)
                .last()
                .map(|(start, text)| (start, start + text.len())),
            PatternKind::Regexp => self
                .regexp
                .as_ref()
                .expect("regexp kind should have compiled pattern")
                .find_backward(line, maximum_byte),
        }
    }

    pub(crate) fn match_ranges_in_line(&self, line: &str) -> Vec<(usize, usize)> {
        match self.kind {
            PatternKind::Literal => line
                .match_indices(&self.literal)
                .map(|(start, text)| (start, start + text.len()))
                .collect(),
            PatternKind::Regexp => self
                .regexp
                .as_ref()
                .expect("regexp kind should have compiled pattern")
                .match_ranges(line),
        }
    }

    pub(crate) fn can_match_empty(&self) -> bool {
        match self.kind {
            PatternKind::Literal => self.literal.is_empty(),
            PatternKind::Regexp => self
                .regexp
                .as_ref()
                .expect("regexp kind should have compiled pattern")
                .can_match_empty(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RegexpPattern {
    expression: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Expression {
    alternatives: Vec<Sequence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Sequence {
    pieces: Vec<Piece>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    AnchorStart,
    AnchorEnd,
    Consume { atom: Atom, quantifier: Quantifier },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Atom {
    Any,
    Literal(char),
    Class(CharacterClass),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CharacterClass {
    negated: bool,
    items: Vec<ClassItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ClassItem {
    Character(char),
    Range(char, char),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quantifier {
    One,
    ZeroOrMore,
    OneOrMore,
    ZeroOrOne,
}

#[derive(Debug, Clone, Copy)]
struct CharSlot {
    byte: usize,
    character: char,
}

impl RegexpPattern {
    fn compile(input: &str) -> Result<Self, PatternError> {
        let mut parser = Parser::new(input);
        let expression = parser.parse()?;
        Ok(Self { expression })
    }

    fn find_forward(&self, line: &str, minimum_byte: usize) -> Option<(usize, usize)> {
        let slots = char_slots(line);
        for start_slot in start_slots_from_byte(&slots, line.len(), minimum_byte) {
            if let Some(end_slot) = self.match_from(&slots, start_slot) {
                return Some((
                    slot_byte(&slots, line.len(), start_slot),
                    slot_byte(&slots, line.len(), end_slot),
                ));
            }
        }
        None
    }

    fn find_backward(&self, line: &str, maximum_byte: usize) -> Option<(usize, usize)> {
        let slots = char_slots(line);
        let mut found = None;
        for start_slot in 0..=slots.len() {
            let start_byte = slot_byte(&slots, line.len(), start_slot);
            if start_byte > maximum_byte {
                break;
            }
            if let Some(end_slot) = self.match_from(&slots, start_slot) {
                let end_byte = slot_byte(&slots, line.len(), end_slot);
                if start_byte < maximum_byte || (start_byte == line.len() && end_byte == start_byte)
                {
                    found = Some((start_byte, end_byte));
                }
            }
        }
        found
    }

    fn match_ranges(&self, line: &str) -> Vec<(usize, usize)> {
        let slots = char_slots(line);
        let mut ranges = Vec::new();
        let mut start_slot = 0;
        while start_slot <= slots.len() {
            if let Some(end_slot) = self.match_from(&slots, start_slot)
                && end_slot > start_slot
            {
                ranges.push((
                    slot_byte(&slots, line.len(), start_slot),
                    slot_byte(&slots, line.len(), end_slot),
                ));
                start_slot = end_slot;
            } else {
                start_slot += 1;
            }
        }
        ranges
    }

    fn match_from(&self, slots: &[CharSlot], start_slot: usize) -> Option<usize> {
        self.expression.match_from(slots, start_slot)
    }

    fn can_match_empty(&self) -> bool {
        self.match_from(&[], 0) == Some(0)
    }
}

impl Expression {
    fn sequence(pieces: Vec<Piece>) -> Self {
        Self {
            alternatives: vec![Sequence { pieces }],
        }
    }

    fn match_from(&self, slots: &[CharSlot], start_slot: usize) -> Option<usize> {
        self.alternatives
            .iter()
            .find_map(|alternative| alternative.match_from(slots, start_slot))
    }
}

impl Sequence {
    fn match_from(&self, slots: &[CharSlot], start_slot: usize) -> Option<usize> {
        self.match_piece(slots, 0, start_slot)
    }

    fn match_piece(
        &self,
        slots: &[CharSlot],
        piece_index: usize,
        slot_index: usize,
    ) -> Option<usize> {
        let Some(piece) = self.pieces.get(piece_index) else {
            return Some(slot_index);
        };
        match piece {
            Piece::AnchorStart => {
                (slot_index == 0).then(|| self.match_piece(slots, piece_index + 1, slot_index))?
            }
            Piece::AnchorEnd => (slot_index == slots.len())
                .then(|| self.match_piece(slots, piece_index + 1, slot_index))?,
            Piece::Consume { atom, quantifier } => {
                self.match_consume(slots, piece_index, slot_index, atom, *quantifier)
            }
        }
    }

    fn match_consume(
        &self,
        slots: &[CharSlot],
        piece_index: usize,
        slot_index: usize,
        atom: &Atom,
        quantifier: Quantifier,
    ) -> Option<usize> {
        let max = max_repetitions(slots, slot_index, atom);
        let (minimum, maximum) = match quantifier {
            Quantifier::One => (1, 1),
            Quantifier::ZeroOrMore => (0, max),
            Quantifier::OneOrMore => (1, max),
            Quantifier::ZeroOrOne => (0, max.min(1)),
        };
        if max < minimum {
            return None;
        }
        for count in (minimum..=maximum).rev() {
            let next_slot = slot_index + count;
            if let Some(end) = self.match_piece(slots, piece_index + 1, next_slot) {
                return Some(end);
            }
        }
        None
    }
}

impl Atom {
    fn matches(&self, character: char) -> bool {
        match self {
            Self::Any => true,
            Self::Literal(literal) => *literal == character,
            Self::Class(class) => class.matches(character),
        }
    }
}

impl CharacterClass {
    fn matches(&self, character: char) -> bool {
        let contains = self.items.iter().any(|item| match item {
            ClassItem::Character(item) => *item == character,
            ClassItem::Range(start, end) => *start <= character && character <= *end,
        });
        if self.negated { !contains } else { contains }
    }
}

struct Parser<'a> {
    chars: Vec<char>,
    index: usize,
    _input: &'a str,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().collect(),
            index: 0,
            _input: input,
        }
    }

    fn parse(&mut self) -> Result<Expression, PatternError> {
        let mut pieces = Vec::new();
        while let Some(character) = self.next() {
            let piece = match character {
                '^' => Piece::AnchorStart,
                '$' => Piece::AnchorEnd,
                '.' => self.consume_piece(Atom::Any)?,
                '[' => {
                    let class = self.parse_class()?;
                    self.consume_piece(Atom::Class(class))?
                }
                '\\' => {
                    let literal = self
                        .next()
                        .ok_or_else(|| PatternError::new("trailing escape"))?;
                    self.consume_piece(Atom::Literal(literal))?
                }
                '*' | '+' | '?' => {
                    return Err(PatternError::new("missing atom before quantifier"));
                }
                literal => self.consume_piece(Atom::Literal(literal))?,
            };
            pieces.push(piece);
        }
        Ok(Expression::sequence(pieces))
    }

    fn consume_piece(&mut self, atom: Atom) -> Result<Piece, PatternError> {
        let quantifier = match self.peek() {
            Some('*') => {
                self.index += 1;
                Quantifier::ZeroOrMore
            }
            Some('+') => {
                self.index += 1;
                Quantifier::OneOrMore
            }
            Some('?') => {
                self.index += 1;
                Quantifier::ZeroOrOne
            }
            _ => Quantifier::One,
        };
        if matches!(self.peek(), Some('*' | '+' | '?')) {
            return Err(PatternError::new("repeated quantifier"));
        }
        Ok(Piece::Consume { atom, quantifier })
    }

    fn parse_class(&mut self) -> Result<CharacterClass, PatternError> {
        let negated = if self.peek() == Some('^') {
            self.index += 1;
            true
        } else {
            false
        };
        let mut items = Vec::new();
        while let Some(character) = self.next() {
            if character == ']' {
                if items.is_empty() {
                    return Err(PatternError::new("empty character class"));
                }
                return Ok(CharacterClass { negated, items });
            }
            let start = if character == '\\' {
                self.next()
                    .ok_or_else(|| PatternError::new("trailing escape in character class"))?
            } else {
                character
            };
            if self.peek() == Some('-') && self.peek_next().is_some_and(|next| next != ']') {
                self.index += 1;
                let end = match self.next() {
                    Some('\\') => self.next().ok_or_else(|| {
                        PatternError::new("trailing escape in character class range")
                    })?,
                    Some(character) => character,
                    None => return Err(PatternError::new("unterminated character class")),
                };
                if start > end {
                    return Err(PatternError::new("invalid character class range"));
                }
                items.push(ClassItem::Range(start, end));
            } else {
                items.push(ClassItem::Character(start));
            }
        }
        Err(PatternError::new("unterminated character class"))
    }

    fn next(&mut self) -> Option<char> {
        let character = self.chars.get(self.index).copied()?;
        self.index += 1;
        Some(character)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.index + 1).copied()
    }
}

fn max_repetitions(slots: &[CharSlot], slot_index: usize, atom: &Atom) -> usize {
    let mut count = 0;
    while slots
        .get(slot_index + count)
        .is_some_and(|slot| atom.matches(slot.character))
    {
        count += 1;
    }
    count
}

fn char_slots(line: &str) -> Vec<CharSlot> {
    line.char_indices()
        .map(|(byte, character)| CharSlot { byte, character })
        .collect()
}

fn slot_byte(slots: &[CharSlot], line_len: usize, slot_index: usize) -> usize {
    slots
        .get(slot_index)
        .map(|slot| slot.byte)
        .unwrap_or(line_len)
}

fn start_slots_from_byte(
    slots: &[CharSlot],
    line_len: usize,
    byte: usize,
) -> impl Iterator<Item = usize> + '_ {
    (0..=slots.len()).filter(move |slot| slot_byte(slots, line_len, *slot) >= byte)
}

#[cfg(test)]
mod tests {
    use super::{Atom, PatternKind, Piece, Quantifier, RegexpPattern, SearchPattern};

    fn regexp(pattern: &str) -> SearchPattern {
        SearchPattern::compile(PatternKind::Regexp, pattern).expect("regexp should compile")
    }

    #[test]
    fn regexp_parser_builds_expression_sequence() {
        let pattern = RegexpPattern::compile("^f[ao]+$").expect("regexp should compile");
        let pieces = &pattern.expression.alternatives[0].pieces;

        assert_eq!(pattern.expression.alternatives.len(), 1);
        assert_eq!(pieces.len(), 4);
        assert_eq!(pieces[0], Piece::AnchorStart);
        assert_eq!(pieces[3], Piece::AnchorEnd);
        assert_eq!(consume_literal(&pieces[1]), Some(('f', Quantifier::One)));
        assert!(matches!(
            pieces[2],
            Piece::Consume {
                atom: Atom::Class(_),
                quantifier: Quantifier::OneOrMore,
            }
        ));
    }

    #[test]
    fn regexp_parser_preserves_escaped_metacharacters_as_literals() {
        let pattern = RegexpPattern::compile(r"\.\*\+\?\^\$\[\]").expect("regexp should compile");
        let pieces = &pattern.expression.alternatives[0].pieces;
        let literals = pieces
            .iter()
            .map(consume_literal)
            .collect::<Option<Vec<_>>>()
            .expect("all pieces should be literal atoms");

        assert_eq!(
            literals,
            ['.', '*', '+', '?', '^', '$', '[', ']']
                .map(|character| (character, Quantifier::One))
                .to_vec()
        );
    }

    #[test]
    fn regexp_parser_keeps_future_emacs_syntax_bare_literals() {
        let pattern = RegexpPattern::compile("(){}|").expect("regexp should compile");
        let pieces = &pattern.expression.alternatives[0].pieces;
        let literals = pieces
            .iter()
            .map(consume_literal)
            .collect::<Option<Vec<_>>>()
            .expect("all pieces should be literal atoms");

        assert_eq!(
            literals,
            ['(', ')', '{', '}', '|']
                .map(|character| (character, Quantifier::One))
                .to_vec()
        );
    }

    #[test]
    fn regexp_parser_attaches_quantifiers_to_previous_atoms() {
        let pattern = RegexpPattern::compile("a?b*c+").expect("regexp should compile");
        let pieces = &pattern.expression.alternatives[0].pieces;

        assert_eq!(
            pieces
                .iter()
                .map(consume_literal)
                .collect::<Option<Vec<_>>>(),
            Some(vec![
                ('a', Quantifier::ZeroOrOne),
                ('b', Quantifier::ZeroOrMore),
                ('c', Quantifier::OneOrMore),
            ])
        );
    }

    fn consume_literal(piece: &Piece) -> Option<(char, Quantifier)> {
        match piece {
            Piece::Consume {
                atom: Atom::Literal(character),
                quantifier,
            } => Some((*character, *quantifier)),
            _ => None,
        }
    }

    #[test]
    fn literal_pattern_matches_existing_literal_semantics() {
        let pattern =
            SearchPattern::compile(PatternKind::Literal, "foo").expect("literal compiles");

        assert_eq!(pattern.find_forward_in_line("Foo foo", 0), Some((4, 7)));
        assert_eq!(pattern.find_backward_in_line("foo foo", 6), Some((4, 7)));
        assert_eq!(
            pattern.match_ranges_in_line("foo foo"),
            vec![(0, 3), (4, 7)]
        );
    }

    #[test]
    fn regexp_supports_common_atoms_and_quantifiers() {
        assert_eq!(
            regexp("f.o").find_forward_in_line("xx foo", 0),
            Some((3, 6))
        );
        assert_eq!(regexp("fo+").find_forward_in_line("f foo", 0), Some((2, 5)));
        assert_eq!(regexp("fo*").find_forward_in_line("f foo", 0), Some((0, 1)));
        assert_eq!(
            regexp("colou?r").find_forward_in_line("color colour", 0),
            Some((0, 5))
        );
    }

    #[test]
    fn regexp_supports_anchors() {
        assert_eq!(
            regexp("^foo").find_forward_in_line("foo bar", 0),
            Some((0, 3))
        );
        assert_eq!(regexp("^bar").find_forward_in_line("foo bar", 0), None);
        assert_eq!(
            regexp("bar$").find_forward_in_line("foo bar", 0),
            Some((4, 7))
        );
        assert_eq!(regexp("^foo$").find_forward_in_line("foo", 0), Some((0, 3)));
        assert_eq!(regexp("^").find_forward_in_line("foo", 0), Some((0, 0)));
        assert_eq!(regexp("$").find_forward_in_line("foo", 0), Some((3, 3)));
    }

    #[test]
    fn regexp_supports_character_classes() {
        assert_eq!(
            regexp("[bc]at").find_forward_in_line("bat cat", 0),
            Some((0, 3))
        );
        assert_eq!(
            regexp("[^b]at").find_forward_in_line("bat cat", 0),
            Some((4, 7))
        );
        assert_eq!(
            regexp("[a-z]+").find_forward_in_line("123 abc", 0),
            Some((4, 7))
        );
    }

    #[test]
    fn regexp_uses_utf8_safe_byte_ranges() {
        assert_eq!(regexp("é.").find_forward_in_line("x éa", 0), Some((2, 5)));
        assert_eq!(
            regexp("[é-ê]").match_ranges_in_line("é ê e"),
            vec![(0, 2), (3, 5)]
        );
    }

    #[test]
    fn regexp_finds_backward_by_start_boundary() {
        let pattern = regexp("f.o");

        assert_eq!(pattern.find_backward_in_line("foo fxo", 7), Some((4, 7)));
        assert_eq!(pattern.find_backward_in_line("foo fxo", 4), Some((0, 3)));
    }

    #[test]
    fn regexp_ranges_are_non_overlapping_and_ignore_zero_length_matches() {
        assert_eq!(regexp("aa").match_ranges_in_line("aaa"), vec![(0, 2)]);
        assert_eq!(regexp("a*").match_ranges_in_line("aa bb"), vec![(0, 2)]);
        assert_eq!(
            regexp("^").match_ranges_in_line("abc"),
            Vec::<(usize, usize)>::new()
        );
    }

    #[test]
    fn regexp_search_can_find_zero_length_matches() {
        assert_eq!(regexp("a*").find_forward_in_line("bbb", 0), Some((0, 0)));
        assert_eq!(regexp("$").find_backward_in_line("abc", 4), Some((3, 3)));
    }

    #[test]
    fn pattern_reports_whether_it_can_match_empty_text() {
        assert!(regexp("^").can_match_empty());
        assert!(regexp("$").can_match_empty());
        assert!(regexp("a*").can_match_empty());
        assert!(regexp("a?").can_match_empty());
        assert!(regexp("^a*$").can_match_empty());
        assert!(!regexp("a+").can_match_empty());
        assert!(!regexp("f.o").can_match_empty());
        assert!(
            SearchPattern::compile(PatternKind::Literal, "")
                .expect("literal compiles")
                .can_match_empty()
        );
        assert!(
            !SearchPattern::compile(PatternKind::Literal, "foo")
                .expect("literal compiles")
                .can_match_empty()
        );
    }

    #[test]
    fn regexp_reports_invalid_patterns() {
        for pattern in ["\\", "[abc", "[]", "[z-a]", "*a", "a**"] {
            assert!(
                SearchPattern::compile(PatternKind::Regexp, pattern).is_err(),
                "{pattern} should be invalid"
            );
        }
    }
}
