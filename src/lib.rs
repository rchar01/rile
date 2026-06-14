// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

pub mod app;
pub mod buffer;
pub mod buffers;
pub mod command;
pub mod completion;
pub mod config;
pub mod editor;
pub mod error;
pub mod file;
pub mod input;
pub mod keymap;
pub mod minibuffer;
pub mod render;
pub mod syntax;
pub mod terminal;
pub mod window;

pub use error::{Result, RileError};
