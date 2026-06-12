// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::buffer::{BufferId, Position};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub usize);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    pub buffer: BufferId,
    pub cursor: Position,
    pub first_visible_line: usize,
    pub first_visible_column: usize,
}
