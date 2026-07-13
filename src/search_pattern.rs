// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::matching::is_smart_case_sensitive;
use crate::text::is_word_character;

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
    case_sensitive: bool,
    regexp: Option<RegexpPattern>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PatternMatch {
    pub(crate) range: (usize, usize),
    pub(crate) captures: Vec<Option<(usize, usize)>>,
}

impl SearchPattern {
    pub(crate) fn compile(kind: PatternKind, input: &str) -> Result<Self, PatternError> {
        let case_sensitive = match kind {
            PatternKind::Literal => is_smart_case_sensitive(input),
            PatternKind::Regexp => regexp_is_smart_case_sensitive(input),
        };
        let regexp = match kind {
            PatternKind::Literal => None,
            PatternKind::Regexp => Some(RegexpPattern::compile_with_case(input, case_sensitive)?),
        };
        Ok(Self {
            kind,
            literal: input.to_owned(),
            case_sensitive,
            regexp,
        })
    }

    #[cfg(test)]
    pub(crate) fn find_forward_in_line(
        &self,
        line: &str,
        minimum_byte: usize,
    ) -> Option<(usize, usize)> {
        self.find_forward_match_in_line(line, minimum_byte)
            .map(|pattern_match| pattern_match.range)
    }

    pub(crate) fn find_forward_match_in_line(
        &self,
        line: &str,
        minimum_byte: usize,
    ) -> Option<PatternMatch> {
        match self.kind {
            PatternKind::Literal => {
                find_literal_forward_match(line, &self.literal, minimum_byte, self.case_sensitive)
            }
            PatternKind::Regexp => self
                .regexp
                .as_ref()
                .expect("regexp kind should have compiled pattern")
                .find_forward_match(line, minimum_byte),
        }
    }

    pub(crate) fn find_backward_in_line(
        &self,
        line: &str,
        maximum_byte: usize,
    ) -> Option<(usize, usize)> {
        match self.kind {
            PatternKind::Literal => {
                find_literal_backward(line, &self.literal, maximum_byte, self.case_sensitive)
            }
            PatternKind::Regexp => self
                .regexp
                .as_ref()
                .expect("regexp kind should have compiled pattern")
                .find_backward(line, maximum_byte),
        }
    }

    pub(crate) fn match_ranges_in_line(&self, line: &str) -> Vec<(usize, usize)> {
        match self.kind {
            PatternKind::Literal => literal_match_ranges(line, &self.literal, self.case_sensitive),
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

    pub(crate) fn is_case_sensitive(&self) -> bool {
        self.case_sensitive
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RegexpPattern {
    expression: Expression,
    capture_count: usize,
    case_sensitive: bool,
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
    WordStart,
    WordEnd,
    WordBoundary,
    NotWordBoundary,
    Consume { atom: Atom, quantifier: Quantifier },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Atom {
    Any,
    Literal(char),
    WordCharacter,
    NotWordCharacter,
    Class(CharacterClass),
    Group {
        index: usize,
        expression: Expression,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RegexpMatch {
    range: (usize, usize),
    captures: Vec<Option<(usize, usize)>>,
}

impl From<RegexpMatch> for PatternMatch {
    fn from(regexp_match: RegexpMatch) -> Self {
        Self {
            range: regexp_match.range,
            captures: regexp_match.captures,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MatchState {
    end_slot: usize,
    captures: Vec<Option<(usize, usize)>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RepeatBounds {
    minimum: usize,
    maximum: usize,
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
    Posix(PosixClass),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PosixClass {
    Alpha,
    Digit,
    Alnum,
    Space,
    Lower,
    Upper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Quantifier {
    One,
    ZeroOrMore,
    OneOrMore,
    ZeroOrOne,
    Counted {
        minimum: usize,
        maximum: Option<usize>,
    },
}

#[derive(Debug, Clone, Copy)]
struct CharSlot {
    byte: usize,
    character: char,
}

impl RegexpPattern {
    #[cfg(test)]
    fn compile(input: &str) -> Result<Self, PatternError> {
        Self::compile_with_case(input, regexp_is_smart_case_sensitive(input))
    }

    fn compile_with_case(input: &str, case_sensitive: bool) -> Result<Self, PatternError> {
        let mut parser = Parser::new(input);
        let expression = parser.parse()?;
        Ok(Self {
            expression,
            capture_count: parser.capture_count,
            case_sensitive,
        })
    }

    #[cfg(test)]
    fn find_forward(&self, line: &str, minimum_byte: usize) -> Option<(usize, usize)> {
        self.find_forward_match(line, minimum_byte)
            .map(|regexp_match| regexp_match.range)
    }

    fn find_forward_match(&self, line: &str, minimum_byte: usize) -> Option<PatternMatch> {
        let slots = char_slots(line);
        for start_slot in start_slots_from_byte(&slots, line.len(), minimum_byte) {
            if let Some(state) = self.match_state_from(&slots, start_slot) {
                return Some(
                    self.build_match(line.len(), &slots, start_slot, state)
                        .into(),
                );
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
            if let Some(state) = self.match_state_from(&slots, start_slot) {
                let end_byte = slot_byte(&slots, line.len(), state.end_slot);
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
            if let Some(state) = self.match_state_from(&slots, start_slot)
                && state.end_slot > start_slot
            {
                ranges.push((
                    slot_byte(&slots, line.len(), start_slot),
                    slot_byte(&slots, line.len(), state.end_slot),
                ));
                start_slot = state.end_slot;
            } else {
                start_slot += 1;
            }
        }
        ranges
    }

    fn match_state_from(&self, slots: &[CharSlot], start_slot: usize) -> Option<MatchState> {
        let captures = vec![None; self.capture_count];
        self.expression
            .match_states(
                slots,
                self.case_sensitive,
                MatchState {
                    end_slot: start_slot,
                    captures,
                },
            )
            .into_iter()
            .next()
    }

    fn build_match(
        &self,
        line_len: usize,
        slots: &[CharSlot],
        start_slot: usize,
        state: MatchState,
    ) -> RegexpMatch {
        RegexpMatch {
            range: (
                slot_byte(slots, line_len, start_slot),
                slot_byte(slots, line_len, state.end_slot),
            ),
            captures: state
                .captures
                .into_iter()
                .map(|range| {
                    range.map(|(start, end)| {
                        (
                            slot_byte(slots, line_len, start),
                            slot_byte(slots, line_len, end),
                        )
                    })
                })
                .collect(),
        }
    }

    fn can_match_empty(&self) -> bool {
        self.expression.can_match_empty()
    }
}

impl Expression {
    fn can_match_empty(&self) -> bool {
        self.alternatives.iter().any(Sequence::can_match_empty)
    }

    fn match_states(
        &self,
        slots: &[CharSlot],
        case_sensitive: bool,
        state: MatchState,
    ) -> Vec<MatchState> {
        self.alternatives
            .iter()
            .flat_map(|alternative| alternative.match_states(slots, case_sensitive, state.clone()))
            .collect()
    }
}

impl Sequence {
    fn can_match_empty(&self) -> bool {
        self.pieces.iter().all(Piece::can_match_empty)
    }

    fn match_states(
        &self,
        slots: &[CharSlot],
        case_sensitive: bool,
        state: MatchState,
    ) -> Vec<MatchState> {
        self.match_piece_states(slots, case_sensitive, 0, state)
    }

    fn match_piece_states(
        &self,
        slots: &[CharSlot],
        case_sensitive: bool,
        piece_index: usize,
        state: MatchState,
    ) -> Vec<MatchState> {
        let Some(piece) = self.pieces.get(piece_index) else {
            return vec![state];
        };
        match piece {
            Piece::AnchorStart => {
                if state.end_slot == 0 {
                    self.match_piece_states(slots, case_sensitive, piece_index + 1, state)
                } else {
                    Vec::new()
                }
            }
            Piece::AnchorEnd => {
                if state.end_slot == slots.len() {
                    self.match_piece_states(slots, case_sensitive, piece_index + 1, state)
                } else {
                    Vec::new()
                }
            }
            Piece::WordStart => {
                if is_word_start(slots, state.end_slot) {
                    self.match_piece_states(slots, case_sensitive, piece_index + 1, state)
                } else {
                    Vec::new()
                }
            }
            Piece::WordEnd => {
                if is_word_end(slots, state.end_slot) {
                    self.match_piece_states(slots, case_sensitive, piece_index + 1, state)
                } else {
                    Vec::new()
                }
            }
            Piece::WordBoundary => {
                if is_word_boundary(slots, state.end_slot) {
                    self.match_piece_states(slots, case_sensitive, piece_index + 1, state)
                } else {
                    Vec::new()
                }
            }
            Piece::NotWordBoundary => {
                if !is_word_boundary(slots, state.end_slot) {
                    self.match_piece_states(slots, case_sensitive, piece_index + 1, state)
                } else {
                    Vec::new()
                }
            }
            Piece::Consume { atom, quantifier } => self
                .repeat_matches(slots, case_sensitive, state, atom, *quantifier)
                .into_iter()
                .flat_map(|state| {
                    self.match_piece_states(slots, case_sensitive, piece_index + 1, state)
                })
                .collect(),
        }
    }

    fn repeat_matches(
        &self,
        slots: &[CharSlot],
        case_sensitive: bool,
        state: MatchState,
        atom: &Atom,
        quantifier: Quantifier,
    ) -> Vec<MatchState> {
        let (minimum, maximum) = match quantifier {
            Quantifier::One => (1, Some(1)),
            Quantifier::ZeroOrMore => (0, None),
            Quantifier::OneOrMore => (1, None),
            Quantifier::ZeroOrOne => (0, Some(1)),
            Quantifier::Counted { minimum, maximum } => (minimum, maximum),
        };
        let maximum =
            maximum.unwrap_or_else(|| slots.len().saturating_sub(state.end_slot).max(minimum));
        let mut results = Vec::new();
        let bounds = RepeatBounds { minimum, maximum };
        collect_repetition_matches(slots, case_sensitive, state, atom, 0, bounds, &mut results);
        results
    }
}

impl Piece {
    fn can_match_empty(&self) -> bool {
        match self {
            Self::AnchorStart
            | Self::AnchorEnd
            | Self::WordStart
            | Self::WordEnd
            | Self::WordBoundary
            | Self::NotWordBoundary => true,
            Self::Consume { atom, quantifier } => {
                quantifier.minimum() == 0 || atom.can_match_empty()
            }
        }
    }
}

impl Atom {
    fn can_match_empty(&self) -> bool {
        match self {
            Self::Group { expression, .. } => expression.can_match_empty(),
            Self::Any
            | Self::Literal(_)
            | Self::WordCharacter
            | Self::NotWordCharacter
            | Self::Class(_) => false,
        }
    }

    fn matches(&self, character: char, case_sensitive: bool) -> bool {
        match self {
            Self::Any => true,
            Self::Literal(literal) => chars_match(character, *literal, case_sensitive),
            Self::WordCharacter => is_word_character(character),
            Self::NotWordCharacter => !is_word_character(character),
            Self::Class(class) => class.matches(character, case_sensitive),
            Self::Group { .. } => false,
        }
    }

    fn match_states(
        &self,
        slots: &[CharSlot],
        case_sensitive: bool,
        state: MatchState,
    ) -> Vec<MatchState> {
        match self {
            Self::Any
            | Self::Literal(_)
            | Self::WordCharacter
            | Self::NotWordCharacter
            | Self::Class(_) => slots
                .get(state.end_slot)
                .filter(|slot| self.matches(slot.character, case_sensitive))
                .map(|_| {
                    vec![MatchState {
                        end_slot: state.end_slot + 1,
                        captures: state.captures,
                    }]
                })
                .unwrap_or_default(),
            Self::Group { index, expression } => expression
                .match_states(slots, case_sensitive, state.clone())
                .into_iter()
                .map(|mut group_state| {
                    group_state.captures[*index - 1] = Some((state.end_slot, group_state.end_slot));
                    group_state
                })
                .collect(),
        }
    }
}

impl Quantifier {
    fn minimum(self) -> usize {
        match self {
            Self::One | Self::OneOrMore => 1,
            Self::ZeroOrMore | Self::ZeroOrOne => 0,
            Self::Counted { minimum, .. } => minimum,
        }
    }
}

impl CharacterClass {
    fn matches(&self, character: char, case_sensitive: bool) -> bool {
        let contains = self.items.iter().any(|item| match item {
            ClassItem::Character(item) => chars_match(character, *item, case_sensitive),
            ClassItem::Range(start, end) => {
                character_in_range(character, *start, *end, case_sensitive)
            }
            ClassItem::Posix(class) => class.matches(character, case_sensitive),
        });
        if self.negated { !contains } else { contains }
    }
}

impl PosixClass {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "alpha" => Some(Self::Alpha),
            "digit" => Some(Self::Digit),
            "alnum" => Some(Self::Alnum),
            "space" => Some(Self::Space),
            "lower" => Some(Self::Lower),
            "upper" => Some(Self::Upper),
            _ => None,
        }
    }

    fn matches(self, character: char, case_sensitive: bool) -> bool {
        match self {
            Self::Alpha => character.is_ascii_alphabetic(),
            Self::Digit => character.is_ascii_digit(),
            Self::Alnum => character.is_ascii_alphanumeric(),
            Self::Space => character.is_ascii_whitespace(),
            Self::Lower if case_sensitive => character.is_ascii_lowercase(),
            Self::Upper if case_sensitive => character.is_ascii_uppercase(),
            Self::Lower | Self::Upper => character.is_ascii_alphabetic(),
        }
    }
}

struct Parser<'a> {
    chars: Vec<char>,
    index: usize,
    capture_count: usize,
    _input: &'a str,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            chars: input.chars().collect(),
            index: 0,
            capture_count: 0,
            _input: input,
        }
    }

    fn parse(&mut self) -> Result<Expression, PatternError> {
        self.parse_expression(false)
    }

    fn parse_expression(&mut self, in_group: bool) -> Result<Expression, PatternError> {
        let mut alternatives = Vec::new();
        let mut pieces = Vec::new();
        while self.peek().is_some() {
            match self.peek_escaped() {
                Some(')') if in_group => break,
                Some(')') => return Err(PatternError::new("unmatched group close")),
                Some('|') => {
                    self.index += 2;
                    alternatives.push(Sequence { pieces });
                    pieces = Vec::new();
                    continue;
                }
                _ => {}
            }
            let character = self.next().expect("peek should have found character");
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
                    match literal {
                        '(' => {
                            self.capture_count += 1;
                            let index = self.capture_count;
                            let expression = self.parse_expression(true)?;
                            if self.peek_escaped() != Some(')') {
                                return Err(PatternError::new("unterminated group"));
                            }
                            self.index += 2;
                            self.consume_piece(Atom::Group { index, expression })?
                        }
                        ')' => return Err(PatternError::new("unmatched group close")),
                        '<' => Piece::WordStart,
                        '>' => Piece::WordEnd,
                        'b' => Piece::WordBoundary,
                        'B' => Piece::NotWordBoundary,
                        'w' => self.consume_piece(Atom::WordCharacter)?,
                        'W' => self.consume_piece(Atom::NotWordCharacter)?,
                        '{' => return Err(PatternError::new("missing atom before quantifier")),
                        '}' => return Err(PatternError::new("unmatched counted repetition close")),
                        literal => self.consume_piece(Atom::Literal(literal))?,
                    }
                }
                '*' | '+' | '?' => {
                    return Err(PatternError::new("missing atom before quantifier"));
                }
                literal => self.consume_piece(Atom::Literal(literal))?,
            };
            pieces.push(piece);
        }
        alternatives.push(Sequence { pieces });
        Ok(Expression { alternatives })
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
            Some('\\') if self.peek_next() == Some('{') => self.parse_counted_quantifier()?,
            _ => Quantifier::One,
        };
        if matches!(self.peek(), Some('*' | '+' | '?')) || self.peek_escaped() == Some('{') {
            return Err(PatternError::new("repeated quantifier"));
        }
        Ok(Piece::Consume { atom, quantifier })
    }

    fn parse_counted_quantifier(&mut self) -> Result<Quantifier, PatternError> {
        self.index += 2;
        let minimum = self.parse_digits()?;
        let maximum = match self.peek() {
            Some(',') => {
                self.index += 1;
                if self.peek_escaped() == Some('}') {
                    self.index += 2;
                    None
                } else {
                    let maximum = self.parse_digits()?;
                    if maximum < minimum {
                        return Err(PatternError::new("invalid counted repetition range"));
                    }
                    self.consume_counted_close()?;
                    Some(maximum)
                }
            }
            _ => {
                self.consume_counted_close()?;
                Some(minimum)
            }
        };
        Ok(Quantifier::Counted { minimum, maximum })
    }

    fn parse_digits(&mut self) -> Result<usize, PatternError> {
        let start = self.index;
        while self
            .peek()
            .is_some_and(|character| character.is_ascii_digit())
        {
            self.index += 1;
        }
        if self.index == start {
            return Err(PatternError::new("missing counted repetition number"));
        }
        self.chars[start..self.index]
            .iter()
            .collect::<String>()
            .parse()
            .map_err(|_| PatternError::new("invalid counted repetition number"))
    }

    fn consume_counted_close(&mut self) -> Result<(), PatternError> {
        if self.peek_escaped() != Some('}') {
            return Err(PatternError::new("unterminated counted repetition"));
        }
        self.index += 2;
        Ok(())
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
            if character == '[' && self.peek() == Some(':') {
                items.push(ClassItem::Posix(self.parse_posix_class()?));
                continue;
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

    fn parse_posix_class(&mut self) -> Result<PosixClass, PatternError> {
        self.index += 1;
        let start = self.index;
        while self.peek().is_some() {
            if self.peek() == Some(':') && self.peek_next() == Some(']') {
                let name = self.chars[start..self.index].iter().collect::<String>();
                self.index += 2;
                return PosixClass::from_name(&name)
                    .ok_or_else(|| PatternError::new("unsupported POSIX character class"));
            }
            self.index += 1;
        }
        Err(PatternError::new("unterminated POSIX character class"))
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

    fn peek_escaped(&self) -> Option<char> {
        (self.peek() == Some('\\'))
            .then(|| self.peek_next())
            .flatten()
    }
}

fn collect_repetition_matches(
    slots: &[CharSlot],
    case_sensitive: bool,
    state: MatchState,
    atom: &Atom,
    count: usize,
    bounds: RepeatBounds,
    results: &mut Vec<MatchState>,
) {
    if count < bounds.maximum {
        for next_state in atom.match_states(slots, case_sensitive, state.clone()) {
            if next_state.end_slot == state.end_slot {
                if count + 1 < bounds.maximum {
                    collect_repetition_matches(
                        slots,
                        case_sensitive,
                        next_state,
                        atom,
                        count + 1,
                        bounds,
                        results,
                    );
                } else if count + 1 >= bounds.minimum {
                    results.push(next_state);
                }
                continue;
            }
            collect_repetition_matches(
                slots,
                case_sensitive,
                next_state,
                atom,
                count + 1,
                bounds,
                results,
            );
        }
    }
    if count >= bounds.minimum {
        results.push(state);
    }
}

fn regexp_is_smart_case_sensitive(pattern: &str) -> bool {
    let mut escaped = false;
    for character in pattern.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character.is_uppercase() {
            return true;
        }
    }
    false
}

fn find_literal_forward_match(
    line: &str,
    literal: &str,
    minimum_byte: usize,
    case_sensitive: bool,
) -> Option<PatternMatch> {
    find_literal_forward(line, literal, minimum_byte, case_sensitive).map(|range| PatternMatch {
        range,
        captures: Vec::new(),
    })
}

fn find_literal_forward(
    line: &str,
    literal: &str,
    minimum_byte: usize,
    case_sensitive: bool,
) -> Option<(usize, usize)> {
    line_boundaries(line)
        .filter(|start| *start >= minimum_byte)
        .find_map(|start| {
            literal_match_end(line, literal, start, case_sensitive).map(|end| (start, end))
        })
}

fn find_literal_backward(
    line: &str,
    literal: &str,
    maximum_byte: usize,
    case_sensitive: bool,
) -> Option<(usize, usize)> {
    line_boundaries(line)
        .filter(|start| *start < maximum_byte)
        .filter_map(|start| {
            literal_match_end(line, literal, start, case_sensitive).map(|end| (start, end))
        })
        .last()
}

fn literal_match_ranges(line: &str, literal: &str, case_sensitive: bool) -> Vec<(usize, usize)> {
    if literal.is_empty() {
        return line_boundaries(line).map(|byte| (byte, byte)).collect();
    }

    let mut ranges = Vec::new();
    let mut start = 0;
    while start <= line.len() {
        if let Some(end) = literal_match_end(line, literal, start, case_sensitive) {
            ranges.push((start, end));
            start = end;
        } else if let Some(next) = next_boundary_after(line, start) {
            start = next;
        } else {
            break;
        }
    }
    ranges
}

fn literal_match_end(
    line: &str,
    literal: &str,
    start: usize,
    case_sensitive: bool,
) -> Option<usize> {
    if !line.is_char_boundary(start) {
        return None;
    }
    if literal.is_empty() {
        return Some(start);
    }
    if case_sensitive {
        return line[start..]
            .starts_with(literal)
            .then_some(start + literal.len());
    }

    let folded_literal = literal.to_lowercase();
    let mut folded_candidate = String::new();
    for (offset, character) in line[start..].char_indices() {
        folded_candidate.extend(character.to_lowercase());
        let end = start + offset + character.len_utf8();
        if folded_candidate == folded_literal {
            return Some(end);
        }
        if !folded_literal.starts_with(&folded_candidate) {
            return None;
        }
    }
    None
}

fn line_boundaries(line: &str) -> impl Iterator<Item = usize> + '_ {
    line.char_indices()
        .map(|(byte, _)| byte)
        .chain(std::iter::once(line.len()))
}

fn next_boundary_after(line: &str, byte: usize) -> Option<usize> {
    line[byte..]
        .char_indices()
        .nth(1)
        .map(|(offset, _)| byte + offset)
        .or((byte < line.len()).then_some(line.len()))
}

fn chars_match(value: char, pattern: char, case_sensitive: bool) -> bool {
    if case_sensitive {
        value == pattern
    } else {
        value.to_lowercase().collect::<String>() == pattern.to_lowercase().collect::<String>()
    }
}

fn character_in_range(character: char, start: char, end: char, case_sensitive: bool) -> bool {
    if start <= character && character <= end {
        return true;
    }
    if case_sensitive
        || !character.is_ascii_alphabetic()
        || !start.is_ascii_alphabetic()
        || !end.is_ascii_alphabetic()
    {
        return false;
    }
    let folded_character = character.to_ascii_lowercase();
    let folded_start = start.to_ascii_lowercase();
    let folded_end = end.to_ascii_lowercase();
    folded_start <= folded_character && folded_character <= folded_end
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

fn is_word_start(slots: &[CharSlot], slot_index: usize) -> bool {
    is_word_at(slots, slot_index) && !is_word_before(slots, slot_index)
}

fn is_word_end(slots: &[CharSlot], slot_index: usize) -> bool {
    !is_word_at(slots, slot_index) && is_word_before(slots, slot_index)
}

fn is_word_boundary(slots: &[CharSlot], slot_index: usize) -> bool {
    is_word_start(slots, slot_index) || is_word_end(slots, slot_index)
}

fn is_word_at(slots: &[CharSlot], slot_index: usize) -> bool {
    slots
        .get(slot_index)
        .is_some_and(|slot| is_word_character(slot.character))
}

fn is_word_before(slots: &[CharSlot], slot_index: usize) -> bool {
    slot_index
        .checked_sub(1)
        .and_then(|index| slots.get(index))
        .is_some_and(|slot| is_word_character(slot.character))
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

    struct RegexpConformanceCase {
        name: &'static str,
        pattern: &'static str,
        line: &'static str,
        forward: Option<(usize, usize)>,
        backward: Option<(usize, usize)>,
        ranges: &'static [(usize, usize)],
        can_match_empty: bool,
    }

    #[test]
    fn regexp_conformance_matrix_covers_representative_documented_subset() {
        for case in [
            RegexpConformanceCase {
                name: "dot and plus",
                pattern: "f.o+",
                line: "xx fooo",
                forward: Some((3, 7)),
                backward: Some((3, 7)),
                ranges: &[(3, 7)],
                can_match_empty: false,
            },
            RegexpConformanceCase {
                name: "anchors",
                pattern: "^foo$",
                line: "foo",
                forward: Some((0, 3)),
                backward: Some((0, 3)),
                ranges: &[(0, 3)],
                can_match_empty: false,
            },
            RegexpConformanceCase {
                name: "character class range and repetition",
                pattern: r"[a-z]\{3\}",
                line: "123 abc xyz",
                forward: Some((4, 7)),
                backward: Some((8, 11)),
                ranges: &[(4, 7), (8, 11)],
                can_match_empty: false,
            },
            RegexpConformanceCase {
                name: "negated class",
                pattern: r"[^[:space:]]+",
                line: "a b\tc",
                forward: Some((0, 1)),
                backward: Some((4, 5)),
                ranges: &[(0, 1), (2, 3), (4, 5)],
                can_match_empty: false,
            },
            RegexpConformanceCase {
                name: "emacs grouping and alternation",
                pattern: r"\(cat\|dog\)s",
                line: "cats dogs cots",
                forward: Some((0, 4)),
                backward: Some((5, 9)),
                ranges: &[(0, 4), (5, 9)],
                can_match_empty: false,
            },
            RegexpConformanceCase {
                name: "counted repetition",
                pattern: r"[[:digit:]]\{4\}",
                line: "a 1 1234",
                forward: Some((4, 8)),
                backward: Some((4, 8)),
                ranges: &[(4, 8)],
                can_match_empty: false,
            },
            RegexpConformanceCase {
                name: "word start and end",
                pattern: r"\<cat\>",
                line: "cat concatenate bob_cat cat!",
                forward: Some((0, 3)),
                backward: Some((24, 27)),
                ranges: &[(0, 3), (24, 27)],
                can_match_empty: false,
            },
            RegexpConformanceCase {
                name: "word and non-word characters",
                pattern: r"\b\w+\W",
                line: "cat! dog?",
                forward: Some((0, 4)),
                backward: Some((5, 9)),
                ranges: &[(0, 4), (5, 9)],
                can_match_empty: false,
            },
            RegexpConformanceCase {
                name: "unicode word characters",
                pattern: r"\b\w+",
                line: ". déjà_vu!",
                forward: Some((2, 2 + "déjà_vu".len())),
                backward: Some((2, 2 + "déjà_vu".len())),
                ranges: &[(2, 11)],
                can_match_empty: false,
            },
            RegexpConformanceCase {
                name: "zero-width anchor",
                pattern: r"^",
                line: "abc",
                forward: Some((0, 0)),
                backward: Some((0, 0)),
                ranges: &[],
                can_match_empty: true,
            },
        ] {
            let pattern = regexp(case.pattern);
            assert_eq!(
                pattern.find_forward_in_line(case.line, 0),
                case.forward,
                "{} forward",
                case.name
            );
            assert_eq!(
                pattern.find_backward_in_line(case.line, case.line.len()),
                case.backward,
                "{} backward",
                case.name
            );
            assert_eq!(
                pattern.match_ranges_in_line(case.line),
                case.ranges,
                "{} ranges",
                case.name
            );
            assert_eq!(
                pattern.can_match_empty(),
                case.can_match_empty,
                "{} nullability",
                case.name
            );
        }
    }

    #[test]
    fn regexp_out_of_scope_emacs_constructs_are_not_treated_as_supported_forms() {
        struct UnsupportedEscapeCase {
            pattern: &'static str,
            line: &'static str,
            expected: Option<(usize, usize)>,
        }

        for case in [
            UnsupportedEscapeCase {
                pattern: r"\1",
                line: "1",
                expected: Some((0, 1)),
            },
            UnsupportedEscapeCase {
                pattern: r"\(a\)\1",
                line: "aa a1",
                expected: Some((3, 5)),
            },
            UnsupportedEscapeCase {
                pattern: r"\sw",
                line: "sw",
                expected: Some((0, 2)),
            },
            UnsupportedEscapeCase {
                pattern: r"\_<",
                line: "_<",
                expected: Some((0, 2)),
            },
        ] {
            assert_eq!(
                regexp(case.pattern).find_forward_in_line(case.line, 0),
                case.expected,
                "{} should use literal fallback semantics",
                case.pattern
            );
        }

        assert_eq!(
            regexp(r"\sw+").find_forward_in_line("www sw", 0),
            Some((4, 6))
        );
        for invalid in [r"[[:word:]]", r"[[:symbol:]]"] {
            assert!(
                SearchPattern::compile(PatternKind::Regexp, invalid).is_err(),
                "{invalid} should be invalid"
            );
        }
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
    fn regexp_parser_represents_empty_pattern_as_empty_sequence() {
        let pattern = RegexpPattern::compile("").expect("regexp should compile");

        assert_eq!(pattern.expression.alternatives.len(), 1);
        assert!(pattern.expression.alternatives[0].pieces.is_empty());
        assert_eq!(pattern.find_forward("abc", 0), Some((0, 0)));
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
        let pattern =
            RegexpPattern::compile(r"a?b*c+d\{2\}e\{1,\}f\{0,3\}").expect("regexp should compile");
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
                (
                    'd',
                    Quantifier::Counted {
                        minimum: 2,
                        maximum: Some(2),
                    },
                ),
                (
                    'e',
                    Quantifier::Counted {
                        minimum: 1,
                        maximum: None,
                    },
                ),
                (
                    'f',
                    Quantifier::Counted {
                        minimum: 0,
                        maximum: Some(3),
                    },
                ),
            ])
        );
    }

    #[test]
    fn regexp_parser_builds_alternatives() {
        let pattern = RegexpPattern::compile(r"foo\|bar\|baz").expect("regexp should compile");

        assert_eq!(pattern.expression.alternatives.len(), 3);
        assert_eq!(
            literal_text(&pattern.expression.alternatives[0].pieces),
            "foo"
        );
        assert_eq!(
            literal_text(&pattern.expression.alternatives[1].pieces),
            "bar"
        );
        assert_eq!(
            literal_text(&pattern.expression.alternatives[2].pieces),
            "baz"
        );
    }

    #[test]
    fn regexp_parser_builds_grouped_alternatives() {
        let pattern = RegexpPattern::compile(r"a\(bc\|d\)e").expect("regexp should compile");
        let pieces = &pattern.expression.alternatives[0].pieces;
        let group = consume_group(&pieces[1]).expect("middle piece should be a group");

        assert_eq!(pieces.len(), 3);
        assert_eq!(consume_literal(&pieces[0]), Some(('a', Quantifier::One)));
        assert_eq!(consume_literal(&pieces[2]), Some(('e', Quantifier::One)));
        assert_eq!(group.alternatives.len(), 2);
        assert_eq!(literal_text(&group.alternatives[0].pieces), "bc");
        assert_eq!(literal_text(&group.alternatives[1].pieces), "d");
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

    fn consume_group(piece: &Piece) -> Option<&super::Expression> {
        match piece {
            Piece::Consume {
                atom: Atom::Group { expression, .. },
                quantifier: Quantifier::One,
            } => Some(expression),
            _ => None,
        }
    }

    fn literal_text(pieces: &[Piece]) -> String {
        pieces
            .iter()
            .map(|piece| consume_literal(piece).map(|(character, _)| character))
            .collect::<Option<String>>()
            .expect("all pieces should be literal atoms")
    }

    #[test]
    fn literal_pattern_uses_smart_case() {
        let pattern =
            SearchPattern::compile(PatternKind::Literal, "foo").expect("literal compiles");

        assert_eq!(pattern.find_forward_in_line("Foo foo", 0), Some((0, 3)));
        assert_eq!(pattern.find_backward_in_line("FOO foo", 6), Some((4, 7)));
        assert_eq!(
            pattern.match_ranges_in_line("Foo foo FOO"),
            vec![(0, 3), (4, 7), (8, 11)]
        );
    }

    #[test]
    fn literal_pattern_uppercase_is_case_sensitive() {
        let pattern =
            SearchPattern::compile(PatternKind::Literal, "Foo").expect("literal compiles");

        assert_eq!(pattern.find_forward_in_line("foo Foo FOO", 0), Some((4, 7)));
        assert_eq!(pattern.find_backward_in_line("Foo foo", 7), Some((0, 3)));
        assert_eq!(pattern.match_ranges_in_line("Foo foo FOO"), vec![(0, 3)]);
    }

    #[test]
    fn literal_pattern_smart_case_preserves_utf8_ranges() {
        let pattern =
            SearchPattern::compile(PatternKind::Literal, "écho").expect("literal compiles");

        assert_eq!(
            pattern.find_forward_in_line("Écho écho", 0),
            Some((0, "Écho".len()))
        );
        assert_eq!(
            pattern.match_ranges_in_line("Écho écho"),
            vec![(0, "Écho".len()), ("Écho ".len(), "Écho écho".len())]
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
    fn regexp_pattern_uses_smart_case_for_literals() {
        assert_eq!(
            regexp("status").find_forward_in_line("Status status", 0),
            Some((0, 6))
        );
        assert_eq!(
            regexp("status").match_ranges_in_line("Status status STATUS"),
            vec![(0, 6), (7, 13), (14, 20)]
        );
        assert_eq!(
            regexp("Status").match_ranges_in_line("Status status STATUS"),
            vec![(0, 6)]
        );
    }

    #[test]
    fn regexp_pattern_ignores_escaped_uppercase_for_smart_case_detection() {
        assert_eq!(
            regexp(r"\Wcat").find_forward_in_line("!Cat", 0),
            Some((0, 4))
        );
        assert_eq!(
            regexp(r"\Cat").find_forward_in_line("cat Cat", 0),
            Some((0, 3))
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
    fn regexp_character_classes_use_smart_case() {
        assert_eq!(
            regexp("[a-z]+").match_ranges_in_line("ABC abc"),
            vec![(0, 3), (4, 7)]
        );
        assert_eq!(
            regexp("[A-Z]+").match_ranges_in_line("ABC abc"),
            vec![(0, 3)]
        );
        assert_eq!(regexp("[a-z]+").find_forward_in_line("İ", 0), None);
        assert_eq!(
            regexp("[é]+").find_forward_in_line("É", 0),
            Some((0, "É".len()))
        );
    }

    #[test]
    fn regexp_supports_word_constructs() {
        assert_eq!(
            regexp(r"\<cat\>").match_ranges_in_line("cat concatenate bob_cat cat!"),
            vec![(0, 3), (24, 27)]
        );
        assert_eq!(
            regexp(r"\bcat").find_forward_in_line("concatenate cat", 0),
            Some((12, 15))
        );
        assert_eq!(
            regexp(r"\Bcat").find_forward_in_line("concatenate cat", 0),
            Some((3, 6))
        );
        assert_eq!(
            regexp(r"\w+").find_forward_in_line(". déjà_vu!", 0),
            Some((2, "déjà_vu".len() + 2))
        );
        assert_eq!(
            regexp(r"\W+").find_forward_in_line("abc, def", 0),
            Some((3, 5))
        );
        assert_eq!(regexp(r"\>").find_forward_in_line("cat", 0), Some((3, 3)));
        assert_eq!(regexp(r"\B").find_forward_in_line("", 0), Some((0, 0)));
        assert_eq!(regexp(r"\B").find_forward_in_line("!!!", 0), Some((0, 0)));
        assert_eq!(
            regexp(r"\<cat\>").find_backward_in_line("cat concatenate cat", 19),
            Some((16, 19))
        );
        assert_eq!(
            regexp(r"\Bcat").find_backward_in_line("concatenate cat", 15),
            Some((3, 6))
        );
        assert_eq!(
            regexp(r"\b\w+\W").find_backward_in_line("cat! dog?", 9),
            Some((5, 9))
        );
    }

    #[test]
    fn regexp_supports_posix_character_classes() {
        assert_eq!(
            regexp(r"[[:digit:]]\{2,4\}").find_forward_in_line("a 1 12345", 0),
            Some((4, 8))
        );
        assert_eq!(
            regexp(r"[[:alpha:]_]+").find_forward_in_line("123 Ab_z", 0),
            Some((4, 8))
        );
        assert_eq!(
            regexp(r"[[:upper:]][[:lower:]]+").find_forward_in_line("Éclair Cat", 0),
            Some((2, 7))
        );
        assert_eq!(
            regexp(r"[[:lower:]]+").match_ranges_in_line("ABC abc"),
            vec![(0, 3), (4, 7)]
        );
        assert_eq!(
            regexp(r"[[:upper:]]+").match_ranges_in_line("ABC abc"),
            vec![(0, 3), (4, 7)]
        );
        assert_eq!(
            regexp(r"[[:alnum:]]+").match_ranges_in_line("a1 _ b2"),
            vec![(0, 2), (5, 7)]
        );
        assert_eq!(
            regexp(r"[^[:space:]]+").match_ranges_in_line("a b\tc"),
            vec![(0, 1), (2, 3), (4, 5)]
        );
    }

    #[test]
    fn regexp_supports_alternation_and_groups() {
        assert_eq!(
            regexp(r"cat\|dog").find_forward_in_line("fox dog cat", 0),
            Some((4, 7))
        );
        assert_eq!(
            regexp(r"fo\(o\|x\)").match_ranges_in_line("foo fox fob"),
            vec![(0, 3), (4, 7)]
        );
        assert_eq!(
            regexp(r"\(ab\|a\)b").find_forward_in_line("ab", 0),
            Some((0, 2))
        );
        assert_eq!(
            regexp(r"\(ab\)+").find_forward_in_line("xx abab", 0),
            Some((3, 7))
        );
    }

    #[test]
    fn regexp_supports_counted_repetition() {
        assert_eq!(
            regexp(r"ba\{2\}").find_forward_in_line("baa ba", 0),
            Some((0, 3))
        );
        assert_eq!(
            regexp(r"ba\{2,\}").find_forward_in_line("ba baaa", 0),
            Some((3, 7))
        );
        assert_eq!(
            regexp(r"ba\{1,2\}").match_ranges_in_line("b ba baa baaa"),
            vec![(2, 4), (5, 8), (9, 12)]
        );
        assert_eq!(
            regexp(r"\(ab\)\{2\}").find_forward_in_line("ab abab", 0),
            Some((3, 7))
        );
        assert_eq!(
            regexp(r"ba\{0\}").find_forward_in_line("b ba", 0),
            Some((0, 1))
        );
    }

    #[test]
    fn regexp_supports_nested_groups() {
        assert_eq!(
            regexp(r"a\(b\(c\|d\)\|e\)f").match_ranges_in_line("abcf abdf aef"),
            vec![(0, 4), (5, 9), (10, 13)]
        );
    }

    #[test]
    fn regexp_match_reports_simple_captures() {
        let pattern = RegexpPattern::compile(r"\(foo\)-\(bar\)").expect("regexp should compile");
        let regexp_match = pattern
            .find_forward_match("xx foo-bar", 0)
            .expect("regexp should match");

        assert_eq!(regexp_match.range, (3, 10));
        assert_eq!(regexp_match.captures, vec![Some((3, 6)), Some((7, 10))]);
    }

    #[test]
    fn regexp_match_reports_nested_captures() {
        let pattern = RegexpPattern::compile(r"\(a\(b\)\)").expect("regexp should compile");
        let regexp_match = pattern
            .find_forward_match("xab", 0)
            .expect("regexp should match");

        assert_eq!(regexp_match.range, (1, 3));
        assert_eq!(regexp_match.captures, vec![Some((1, 3)), Some((2, 3))]);
    }

    #[test]
    fn regexp_match_reports_unmatched_alternative_captures() {
        let pattern = RegexpPattern::compile(r"\(foo\)\|\(bar\)").expect("regexp should compile");
        let regexp_match = pattern
            .find_forward_match("bar", 0)
            .expect("regexp should match");

        assert_eq!(regexp_match.range, (0, 3));
        assert_eq!(regexp_match.captures, vec![None, Some((0, 3))]);
    }

    #[test]
    fn regexp_match_reports_backtracked_capture_branch() {
        let pattern = RegexpPattern::compile(r"\(ab\|a\)b").expect("regexp should compile");
        let regexp_match = pattern
            .find_forward_match("ab", 0)
            .expect("regexp should match");

        assert_eq!(regexp_match.range, (0, 2));
        assert_eq!(regexp_match.captures, vec![Some((0, 1))]);
    }

    #[test]
    fn regexp_match_clears_capture_when_backtracking_skips_optional_group() {
        let pattern = RegexpPattern::compile(r"\(a\)?a").expect("regexp should compile");
        let regexp_match = pattern
            .find_forward_match("a", 0)
            .expect("regexp should match");

        assert_eq!(regexp_match.range, (0, 1));
        assert_eq!(regexp_match.captures, vec![None]);
    }

    #[test]
    fn regexp_match_clears_nested_capture_from_abandoned_branch() {
        let pattern = RegexpPattern::compile(r"\(\(a\)b\|a\)c").expect("regexp should compile");
        let regexp_match = pattern
            .find_forward_match("ac", 0)
            .expect("regexp should match");

        assert_eq!(regexp_match.range, (0, 2));
        assert_eq!(regexp_match.captures, vec![Some((0, 1)), None]);
    }

    #[test]
    fn regexp_match_reports_last_repeated_capture() {
        let pattern = RegexpPattern::compile(r"\(ab\)+").expect("regexp should compile");
        let regexp_match = pattern
            .find_forward_match("abab", 0)
            .expect("regexp should match");

        assert_eq!(regexp_match.range, (0, 4));
        assert_eq!(regexp_match.captures, vec![Some((2, 4))]);
    }

    #[test]
    fn regexp_match_reports_zero_width_repeated_capture() {
        let pattern = RegexpPattern::compile(r"\(\)\{2\}").expect("regexp should compile");
        let regexp_match = pattern
            .find_forward_match("abc", 0)
            .expect("regexp should match");

        assert_eq!(regexp_match.range, (0, 0));
        assert_eq!(regexp_match.captures, vec![Some((0, 0))]);
    }

    #[test]
    fn regexp_match_reports_utf8_capture_byte_ranges() {
        let pattern = RegexpPattern::compile(r"\(é.\)").expect("regexp should compile");
        let regexp_match = pattern
            .find_forward_match("x éa", 0)
            .expect("regexp should match");

        assert_eq!(regexp_match.range, (2, 5));
        assert_eq!(regexp_match.captures, vec![Some((2, 5))]);
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
        assert!(regexp(r"a\{0\}").can_match_empty());
        assert!(regexp(r"\(a*\)\{2\}").can_match_empty());
        assert!(regexp("^a*$").can_match_empty());
        assert!(regexp(r"\<").can_match_empty());
        assert!(regexp(r"\>").can_match_empty());
        assert!(regexp(r"\b").can_match_empty());
        assert!(regexp(r"\B").can_match_empty());
        assert!(!regexp("a+").can_match_empty());
        assert!(!regexp(r"a\{2\}").can_match_empty());
        assert!(!regexp("f.o").can_match_empty());
        assert!(!regexp(r"\<cat\>").can_match_empty());
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
        for pattern in [
            "\\",
            r"\(",
            r"\)",
            r"\(abc",
            r"\{2\}",
            r"\}",
            r"a\{\}",
            r"a\{,\}",
            r"a\{2,1\}",
            r"a\{2",
            r"a\{2}",
            r"a\{1\}*",
            r"a*\{1\}",
            "[abc",
            "[]",
            "[z-a]",
            "[[:word:]]",
            "[[:digit]",
            "[[:digit:]]*+",
            "*a",
            "a**",
            r"\b*",
        ] {
            assert!(
                SearchPattern::compile(PatternKind::Regexp, pattern).is_err(),
                "{pattern} should be invalid"
            );
        }
    }
}
