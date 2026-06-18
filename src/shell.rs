// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

use crate::{Result, RileError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellCommandOutput {
    pub stdout: String,
    pub stderr: String,
    pub status_code: Option<i32>,
}

impl ShellCommandOutput {
    pub fn success(&self) -> bool {
        self.status_code == Some(0)
    }
}

pub fn run_shell_command(
    command: &str,
    stdin: &str,
    current_dir: &Path,
) -> Result<ShellCommandOutput> {
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .current_dir(current_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    if let Some(mut child_stdin) = child.stdin.take() {
        child_stdin.write_all(stdin.as_bytes())?;
    }

    let output = child.wait_with_output()?;
    let stdout = String::from_utf8(output.stdout).map_err(|error| {
        RileError::InvalidInput(format!("shell command stdout is not UTF-8: {error}"))
    })?;
    let stderr = String::from_utf8(output.stderr).map_err(|error| {
        RileError::InvalidInput(format!("shell command stderr is not UTF-8: {error}"))
    })?;

    Ok(ShellCommandOutput {
        stdout,
        stderr,
        status_code: output.status.code(),
    })
}

#[cfg(test)]
mod tests {
    use super::run_shell_command;

    #[test]
    fn captures_stdout_stderr_status_and_stdin() {
        let output = run_shell_command(
            "cat; printf 'err-line\\n' >&2; exit 7",
            "input-line\n",
            std::path::Path::new("."),
        )
        .expect("shell command should run");

        assert_eq!(output.stdout, "input-line\n");
        assert_eq!(output.stderr, "err-line\n");
        assert_eq!(output.status_code, Some(7));
        assert!(!output.success());
    }
}
