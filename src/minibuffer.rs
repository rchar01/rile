// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct MinibufferState {
    pub message: Option<String>,
}

impl MinibufferState {
    pub fn set_message(&mut self, message: impl Into<String>) {
        self.message = Some(message.into());
    }

    pub fn clear(&mut self) {
        self.message = None;
    }
}
