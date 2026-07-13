// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    Default,
    CurrentSearchMatch,
    SearchMatch,
    Region,
    Minibuffer,
    ModeLine,
    LineNumber,
    Error,
    Warning,
    UserHighlight,
    UserHighlightAlt,
    UserHighlightLine,
    SyntaxKeyword,
    SyntaxString,
    SyntaxComment,
}

impl Face {
    pub const fn priority(self) -> u8 {
        match self {
            Self::Default => 0,
            Self::Minibuffer => 10,
            Self::ModeLine => 10,
            Self::LineNumber => 10,
            Self::SyntaxKeyword => 15,
            Self::SyntaxString => 15,
            Self::SyntaxComment => 15,
            Self::Warning => 20,
            Self::UserHighlight => 25,
            Self::UserHighlightAlt => 25,
            Self::UserHighlightLine => 25,
            Self::Error => 30,
            Self::Region => 40,
            Self::SearchMatch => 50,
            Self::CurrentSearchMatch => 60,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
    pub face: Face,
}

impl Span {
    pub const fn new(start_byte: usize, end_byte: usize, face: Face) -> Self {
        Self {
            start_byte,
            end_byte,
            face,
        }
    }

    pub const fn is_empty(self) -> bool {
        self.start_byte >= self.end_byte
    }
}

pub trait DecorationProvider {
    fn spans_for_line(&self, line_index: usize, line: &str) -> Vec<Span>;
}

pub fn collect_spans_for_line(
    providers: &[&dyn DecorationProvider],
    line_index: usize,
    line: &str,
) -> Vec<Span> {
    let spans = providers
        .iter()
        .flat_map(|provider| provider.spans_for_line(line_index, line))
        .collect::<Vec<_>>();
    merge_spans(line, spans)
}

pub fn clip_spans(spans: &[Span], range: Range<usize>) -> Vec<Span> {
    spans
        .iter()
        .filter_map(|span| {
            let start = span.start_byte.max(range.start);
            let end = span.end_byte.min(range.end);
            (start < end).then(|| Span::new(start - range.start, end - range.start, span.face))
        })
        .collect()
}

pub fn merge_spans(line: &str, spans: impl IntoIterator<Item = Span>) -> Vec<Span> {
    let spans = spans
        .into_iter()
        .filter(|span| valid_span(line, *span))
        .collect::<Vec<_>>();
    if spans.is_empty() {
        return Vec::new();
    }

    let mut boundaries = vec![0, line.len()];
    for span in &spans {
        boundaries.push(span.start_byte);
        boundaries.push(span.end_byte);
    }
    boundaries.sort_unstable();
    boundaries.dedup();

    let mut merged: Vec<Span> = Vec::new();
    for window in boundaries.windows(2) {
        let start = window[0];
        let end = window[1];
        if start == end {
            continue;
        }
        let Some(face) = spans
            .iter()
            .filter(|span| span.start_byte <= start && span.end_byte >= end)
            .map(|span| span.face)
            .max_by_key(|face| face.priority())
        else {
            continue;
        };
        if face == Face::Default {
            continue;
        }
        if let Some(previous) = merged.last_mut()
            && previous.end_byte == start
            && previous.face == face
        {
            previous.end_byte = end;
            continue;
        }
        merged.push(Span::new(start, end, face));
    }
    merged
}

fn valid_span(line: &str, span: Span) -> bool {
    !span.is_empty()
        && span.end_byte <= line.len()
        && line.is_char_boundary(span.start_byte)
        && line.is_char_boundary(span.end_byte)
}

#[cfg(test)]
mod tests {
    use super::{DecorationProvider, Face, Span, clip_spans, collect_spans_for_line, merge_spans};

    struct StaticProvider(Vec<Span>);

    impl DecorationProvider for StaticProvider {
        fn spans_for_line(&self, _line_index: usize, _line: &str) -> Vec<Span> {
            self.0.clone()
        }
    }

    #[test]
    fn merges_overlapping_spans_by_face_priority() {
        let spans = merge_spans(
            "abcdef",
            [
                Span::new(0, 6, Face::Region),
                Span::new(1, 4, Face::SearchMatch),
                Span::new(2, 3, Face::CurrentSearchMatch),
            ],
        );

        assert_eq!(
            spans,
            vec![
                Span::new(0, 1, Face::Region),
                Span::new(1, 2, Face::SearchMatch),
                Span::new(2, 3, Face::CurrentSearchMatch),
                Span::new(3, 4, Face::SearchMatch),
                Span::new(4, 6, Face::Region),
            ]
        );
    }

    #[test]
    fn ignores_invalid_utf8_boundaries() {
        let spans = merge_spans("éx", [Span::new(1, 2, Face::SearchMatch)]);

        assert!(spans.is_empty());
    }

    #[test]
    fn clips_spans_to_visible_range() {
        let spans = clip_spans(&[Span::new(2, 6, Face::Region)], 4..8);

        assert_eq!(spans, vec![Span::new(0, 2, Face::Region)]);
    }

    #[test]
    fn collects_and_merges_decorator_provider_spans() {
        let first = StaticProvider(vec![Span::new(0, 4, Face::Region)]);
        let second = StaticProvider(vec![Span::new(1, 3, Face::SearchMatch)]);

        let spans = collect_spans_for_line(&[&first, &second], 0, "abcd");

        assert_eq!(
            spans,
            vec![
                Span::new(0, 1, Face::Region),
                Span::new(1, 3, Face::SearchMatch),
                Span::new(3, 4, Face::Region),
            ]
        );
    }
}
