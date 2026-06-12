// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CommandSpec {
    pub name: &'static str,
    pub description: &'static str,
    pub interactive: bool,
}

impl CommandSpec {
    pub const fn new(name: &'static str, description: &'static str, interactive: bool) -> Self {
        Self {
            name,
            description,
            interactive,
        }
    }
}
