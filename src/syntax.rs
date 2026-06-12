// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use crate::render::{Face, Span};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyntaxMode {
    PlainText,
    Rust,
    C,
    Shell,
    Markdown,
    Toml,
}

impl SyntaxMode {
    pub fn for_path(path: Option<&Path>) -> Self {
        let Some(path) = path else {
            return Self::PlainText;
        };
        let extension = path
            .extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase);
        match extension.as_deref() {
            Some("rs") => Self::Rust,
            Some("c" | "h") => Self::C,
            Some("sh" | "bash" | "zsh") => Self::Shell,
            Some("md" | "markdown") => Self::Markdown,
            Some("toml") => Self::Toml,
            _ => Self::PlainText,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::PlainText => "Plain Text",
            Self::Rust => "Rust",
            Self::C => "C",
            Self::Shell => "Shell",
            Self::Markdown => "Markdown",
            Self::Toml => "TOML",
        }
    }
}

pub trait Highlighter {
    fn mode(&self) -> SyntaxMode;
    fn highlight_line(&self, line_index: usize, line: &str) -> Vec<Span>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SyntaxHighlighter {
    mode: SyntaxMode,
}

impl SyntaxHighlighter {
    pub const fn new(mode: SyntaxMode) -> Self {
        Self { mode }
    }
}

impl Highlighter for SyntaxHighlighter {
    fn mode(&self) -> SyntaxMode {
        self.mode
    }

    fn highlight_line(&self, _line_index: usize, line: &str) -> Vec<Span> {
        match self.mode {
            SyntaxMode::PlainText => Vec::new(),
            SyntaxMode::Rust => highlight_code_line(line, "//", RUST_KEYWORDS, &['"']),
            SyntaxMode::C => highlight_c_line(line),
            SyntaxMode::Shell => highlight_shell_line(line),
            SyntaxMode::Markdown => highlight_markdown_line(line),
            SyntaxMode::Toml => highlight_toml_line(line),
        }
    }
}

const RUST_KEYWORDS: &[&str] = &[
    "as", "async", "await", "break", "const", "continue", "crate", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "Self", "self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while",
];

const C_KEYWORDS: &[&str] = &[
    "auto", "break", "case", "char", "const", "continue", "default", "do", "double", "else",
    "enum", "extern", "float", "for", "goto", "if", "inline", "int", "long", "register",
    "restrict", "return", "short", "signed", "sizeof", "static", "struct", "switch", "typedef",
    "union", "unsigned", "void", "volatile", "while",
];

const SHELL_KEYWORDS: &[&str] = &[
    "case", "do", "done", "elif", "else", "esac", "fi", "for", "function", "if", "in", "select",
    "then", "until", "while",
];

fn highlight_c_line(line: &str) -> Vec<Span> {
    let mut spans = highlight_code_line(line, "//", C_KEYWORDS, &['"', '\'']);
    let trimmed_start = line.len() - line.trim_start().len();
    if line[trimmed_start..].starts_with('#') {
        spans.push(Span::new(trimmed_start, line.len(), Face::SyntaxKeyword));
    }
    spans
}

fn highlight_shell_line(line: &str) -> Vec<Span> {
    highlight_code_line(line, "#", SHELL_KEYWORDS, &['"', '\''])
}

fn highlight_toml_line(line: &str) -> Vec<Span> {
    let mut spans = highlight_code_line(line, "#", &[], &['"', '\'']);
    let trimmed_start = line.len() - line.trim_start().len();
    let trimmed = &line[trimmed_start..];
    if let Some(section_end) = trimmed
        .strip_prefix('[')
        .and_then(|text| text.find(']').map(|end| trimmed_start + end + 2))
    {
        spans.push(Span::new(trimmed_start, section_end, Face::SyntaxKeyword));
        return spans;
    }
    if let Some(equals) = line.find('=') {
        let key_end = line[..equals].trim_end().len();
        if trimmed_start < key_end {
            spans.push(Span::new(trimmed_start, key_end, Face::SyntaxKeyword));
        }
    }
    spans
}

fn highlight_markdown_line(line: &str) -> Vec<Span> {
    let mut spans = Vec::new();
    let trimmed_start = line.len() - line.trim_start().len();
    if line[trimmed_start..].starts_with('#') {
        spans.push(Span::new(trimmed_start, line.len(), Face::SyntaxKeyword));
        return spans;
    }

    let mut cursor = 0;
    while let Some(start) = line[cursor..].find('`').map(|offset| cursor + offset) {
        let content_start = start + 1;
        let Some(end) = line[content_start..]
            .find('`')
            .map(|offset| content_start + offset + 1)
        else {
            break;
        };
        spans.push(Span::new(start, end, Face::SyntaxString));
        cursor = end;
    }
    spans
}

fn highlight_code_line(
    line: &str,
    comment_marker: &str,
    keywords: &[&str],
    string_delimiters: &[char],
) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut code_ranges = Vec::new();
    let mut segment_start = 0;
    let mut cursor = 0;

    while cursor < line.len() {
        if !comment_marker.is_empty() && line[cursor..].starts_with(comment_marker) {
            if segment_start < cursor {
                code_ranges.push(segment_start..cursor);
            }
            spans.push(Span::new(cursor, line.len(), Face::SyntaxComment));
            segment_start = line.len();
            break;
        }

        let character = line[cursor..]
            .chars()
            .next()
            .expect("cursor before line end has a character");
        if string_delimiters.contains(&character) {
            if segment_start < cursor {
                code_ranges.push(segment_start..cursor);
            }
            let end = string_end(line, cursor, character);
            spans.push(Span::new(cursor, end, Face::SyntaxString));
            cursor = end;
            segment_start = cursor;
            continue;
        }
        cursor += character.len_utf8();
    }

    if segment_start < line.len() {
        code_ranges.push(segment_start..line.len());
    }
    for range in code_ranges {
        spans.extend(keyword_spans(&line[range.clone()], range.start, keywords));
    }
    spans
}

fn string_end(line: &str, start: usize, delimiter: char) -> usize {
    let mut escaped = false;
    let mut cursor = start + delimiter.len_utf8();
    while cursor < line.len() {
        let character = line[cursor..]
            .chars()
            .next()
            .expect("cursor before line end has a character");
        cursor += character.len_utf8();
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' {
            escaped = true;
            continue;
        }
        if character == delimiter {
            return cursor;
        }
    }
    line.len()
}

fn keyword_spans(segment: &str, base: usize, keywords: &[&str]) -> Vec<Span> {
    let mut spans = Vec::new();
    let mut word_start = None;
    for (offset, character) in segment.char_indices() {
        if is_word_character(character) {
            word_start.get_or_insert(offset);
        } else if let Some(start) = word_start.take() {
            push_keyword_span(&mut spans, segment, base, start, offset, keywords);
        }
    }
    if let Some(start) = word_start {
        push_keyword_span(&mut spans, segment, base, start, segment.len(), keywords);
    }
    spans
}

fn push_keyword_span(
    spans: &mut Vec<Span>,
    segment: &str,
    base: usize,
    start: usize,
    end: usize,
    keywords: &[&str],
) {
    if keywords.contains(&&segment[start..end]) {
        spans.push(Span::new(base + start, base + end, Face::SyntaxKeyword));
    }
}

fn is_word_character(character: char) -> bool {
    character == '_' || character.is_alphanumeric()
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::{Highlighter, SyntaxHighlighter, SyntaxMode};
    use crate::render::{Face, Span};

    #[test]
    fn selects_modes_from_extensions_with_plain_fallback() {
        assert_eq!(
            SyntaxMode::for_path(Some(Path::new("main.rs"))),
            SyntaxMode::Rust
        );
        assert_eq!(
            SyntaxMode::for_path(Some(Path::new("main.c"))),
            SyntaxMode::C
        );
        assert_eq!(
            SyntaxMode::for_path(Some(Path::new("script.sh"))),
            SyntaxMode::Shell
        );
        assert_eq!(
            SyntaxMode::for_path(Some(Path::new("README.md"))),
            SyntaxMode::Markdown
        );
        assert_eq!(
            SyntaxMode::for_path(Some(Path::new("Cargo.toml"))),
            SyntaxMode::Toml
        );
        assert_eq!(
            SyntaxMode::for_path(Some(Path::new("notes.txt"))),
            SyntaxMode::PlainText
        );
        assert_eq!(SyntaxMode::for_path(None), SyntaxMode::PlainText);
    }

    #[test]
    fn highlights_rust_keywords_strings_and_comments() {
        let highlighter = SyntaxHighlighter::new(SyntaxMode::Rust);

        let spans = highlighter.highlight_line(0, "fn main() { let s = \"hi\"; // note }");

        assert!(spans.contains(&Span::new(0, 2, Face::SyntaxKeyword)));
        assert!(spans.contains(&Span::new(12, 15, Face::SyntaxKeyword)));
        assert!(spans.contains(&Span::new(20, 24, Face::SyntaxString)));
        assert!(spans.contains(&Span::new(26, 35, Face::SyntaxComment)));
    }

    #[test]
    fn highlights_c_shell_markdown_and_toml_lines() {
        assert!(
            SyntaxHighlighter::new(SyntaxMode::C)
                .highlight_line(0, "#include <stdio.h>")
                .contains(&Span::new(
                    0,
                    "#include <stdio.h>".len(),
                    Face::SyntaxKeyword
                ))
        );
        assert!(
            SyntaxHighlighter::new(SyntaxMode::Shell)
                .highlight_line(0, "if test; then # ok")
                .contains(&Span::new(0, 2, Face::SyntaxKeyword))
        );
        assert!(
            SyntaxHighlighter::new(SyntaxMode::Markdown)
                .highlight_line(0, "# Heading")
                .contains(&Span::new(0, "# Heading".len(), Face::SyntaxKeyword))
        );
        assert!(
            SyntaxHighlighter::new(SyntaxMode::Toml)
                .highlight_line(0, "name = \"rile\"")
                .contains(&Span::new(0, 4, Face::SyntaxKeyword))
        );
    }
}
