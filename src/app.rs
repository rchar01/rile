// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::ffi::{OsStr, OsString};
use std::path::PathBuf;

use crate::{Result, RileError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOptions {
    pub file: Option<PathBuf>,
    pub visual_test: bool,
    pub test_size: Option<TestSize>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestSize {
    pub columns: u16,
    pub rows: u16,
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
    let mut visual_test = false;
    let mut test_size = None;

    while let Some(argument) = args.next() {
        let argument = argument.into();
        if argument == OsStr::new("-h") || argument == OsStr::new("--help") {
            return Ok(RunMode::Help);
        }
        if argument == OsStr::new("-V") || argument == OsStr::new("--version") {
            return Ok(RunMode::Version);
        }
        if argument == OsStr::new("--visual-test") {
            visual_test = true;
            continue;
        }
        if argument == OsStr::new("--test-size") {
            let Some(value) = args.next() else {
                return Err(RileError::InvalidInput(
                    "--test-size requires WIDTHxHEIGHT".to_owned(),
                ));
            };
            test_size = Some(parse_test_size(&value.into())?);
            continue;
        }
        if let Some(value) = argument
            .to_string_lossy()
            .strip_prefix("--test-size=")
            .map(str::to_owned)
        {
            test_size = Some(parse_test_size(OsStr::new(&value))?);
            continue;
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

    Ok(RunMode::Edit(CliOptions {
        file,
        visual_test,
        test_size,
    }))
}

fn usage() -> &'static str {
    "Usage: rile [--visual-test] [--test-size WIDTHxHEIGHT] [file]\n\nRile Is Lightweight Emacs."
}

fn parse_test_size(value: &OsStr) -> Result<TestSize> {
    let value = value.to_string_lossy();
    let Some((columns, rows)) = value.split_once('x') else {
        return Err(invalid_test_size(&value));
    };
    let columns = columns
        .parse::<u16>()
        .map_err(|_| invalid_test_size(&value))?;
    let rows = rows.parse::<u16>().map_err(|_| invalid_test_size(&value))?;
    if columns == 0 || rows == 0 {
        return Err(invalid_test_size(&value));
    }
    Ok(TestSize { columns, rows })
}

fn invalid_test_size(value: &str) -> RileError {
    RileError::InvalidInput(format!(
        "--test-size must be WIDTHxHEIGHT with nonzero dimensions, got `{value}`"
    ))
}

#[cfg(test)]
mod tests {
    use super::{CliOptions, RunMode, TestSize, parse_args};

    #[test]
    fn parses_no_file() {
        let mode = parse_args(["rile"]).expect("arguments should parse");
        assert_eq!(
            mode,
            RunMode::Edit(CliOptions {
                file: None,
                visual_test: false,
                test_size: None,
            })
        );
    }

    #[test]
    fn parses_one_file() {
        let mode = parse_args(["rile", "notes.txt"]).expect("arguments should parse");
        assert_eq!(
            mode,
            RunMode::Edit(CliOptions {
                file: Some("notes.txt".into()),
                visual_test: false,
                test_size: None,
            })
        );
    }

    #[test]
    fn parses_visual_test_flags() {
        let mode = parse_args(["rile", "--visual-test", "--test-size", "80x24", "notes.txt"])
            .expect("arguments should parse");

        assert_eq!(
            mode,
            RunMode::Edit(CliOptions {
                file: Some("notes.txt".into()),
                visual_test: true,
                test_size: Some(TestSize {
                    columns: 80,
                    rows: 24,
                }),
            })
        );
    }

    #[test]
    fn parses_test_size_equals_form() {
        let mode = parse_args(["rile", "--test-size=100x30"]).expect("arguments should parse");

        assert_eq!(
            mode,
            RunMode::Edit(CliOptions {
                file: None,
                visual_test: false,
                test_size: Some(TestSize {
                    columns: 100,
                    rows: 30,
                }),
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

    #[test]
    fn rejects_invalid_test_size() {
        let error =
            parse_args(["rile", "--test-size", "80"]).expect_err("invalid size should fail");

        assert!(error.to_string().contains("--test-size must be"));
    }
}
