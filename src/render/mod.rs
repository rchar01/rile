// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Face {
    Default,
    CurrentSearchMatch,
    SearchMatch,
    Region,
    Minibuffer,
    ModeLine,
    Error,
    Warning,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start_byte: usize,
    pub end_byte: usize,
    pub face: Face,
}

pub trait DecorationProvider {
    fn spans_for_line(&self, line_index: usize, line: &str) -> Vec<Span>;
}
