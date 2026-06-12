// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use crate::input::KeyEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyBinding {
    pub sequence: Vec<KeyEvent>,
    pub command: &'static str,
}
