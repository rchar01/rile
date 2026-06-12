// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use crate::{Result, RileError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOptions {
    pub file: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunMode {
    Help,
    Version,
    Edit(CliOptions),
}

impl RunMode {
    pub fn message(&self) -> Option<String> {
        match self {
            Self::Help => Some(usage().to_owned()),
            Self::Version => Some(format!("rile {}", env!("CARGO_PKG_VERSION"))),
            Self::Edit(options) => match &options.file {
                Some(path) => Some(format!(
                    "Rile scaffold: editor implementation pending for {}",
                    path.display()
                )),
                None => Some("Rile scaffold: editor implementation pending".to_owned()),
            },
        }
    }
}

pub fn run<I, T>(args: I) -> Result<RunMode>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    parse_args(args)
}

pub fn parse_args<I, T>(args: I) -> Result<RunMode>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut args = args.into_iter();
    let _program = args.next();
    let mut file = None;

    for argument in args {
        let argument = argument.into();
        if argument == OsStr::new("-h") || argument == OsStr::new("--help") {
            return Ok(RunMode::Help);
        }
        if argument == OsStr::new("-V") || argument == OsStr::new("--version") {
            return Ok(RunMode::Version);
        }
        if argument.as_encoded_bytes().starts_with(b"-") {
            return Err(RileError::UnsupportedArgument(
                argument.to_string_lossy().into_owned(),
            ));
        }
        if file.replace(PathBuf::from(argument)).is_some() {
            return Err(RileError::TooManyFiles);
        }
    }

    Ok(RunMode::Edit(CliOptions { file }))
}

fn usage() -> &'static str {
    "Usage: rile [file]\n\nRile Is Lightweight Emacs."
}

#[cfg(test)]
mod tests {
    use super::{CliOptions, RunMode, parse_args};

    #[test]
    fn parses_no_file() {
        let mode = parse_args(["rile"]).expect("arguments should parse");
        assert_eq!(mode, RunMode::Edit(CliOptions { file: None }));
    }

    #[test]
    fn parses_one_file() {
        let mode = parse_args(["rile", "notes.txt"]).expect("arguments should parse");
        assert_eq!(
            mode,
            RunMode::Edit(CliOptions {
                file: Some("notes.txt".into())
            })
        );
    }

    #[test]
    fn parses_help() {
        let mode = parse_args(["rile", "--help"]).expect("arguments should parse");
        assert_eq!(mode, RunMode::Help);
    }

    #[test]
    fn rejects_multiple_files() {
        let error = parse_args(["rile", "a.txt", "b.txt"])
            .expect_err("multiple file arguments should fail");
        assert_eq!(error.to_string(), "expected at most one file argument");
    }
}
