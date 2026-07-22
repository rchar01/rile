// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fmt;

pub type Result<T> = std::result::Result<T, RileError>;

#[derive(Debug)]
pub enum RileError {
    Io(std::io::Error),
    InvalidInput(String),
    InvalidPosition(String),
    SaveCommitted(Box<RileError>),
    NotTerminal,
    TooManyFiles,
    UnsupportedArgument(String),
}

impl fmt::Display for RileError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(f, "I/O error: {error}"),
            Self::InvalidInput(message) => write!(f, "invalid input: {message}"),
            Self::InvalidPosition(message) => write!(f, "invalid position: {message}"),
            Self::SaveCommitted(error) => {
                write!(f, "save committed but auto-save cleanup failed: {error}")
            }
            Self::NotTerminal => write!(f, "editing requires an interactive terminal"),
            Self::TooManyFiles => write!(f, "expected at most one file argument"),
            Self::UnsupportedArgument(argument) => {
                write!(f, "unsupported argument: {argument}")
            }
        }
    }
}

impl std::error::Error for RileError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::SaveCommitted(error) => Some(error.as_ref()),
            Self::InvalidInput(_)
            | Self::InvalidPosition(_)
            | Self::NotTerminal
            | Self::TooManyFiles
            | Self::UnsupportedArgument(_) => None,
        }
    }
}

impl From<std::io::Error> for RileError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
