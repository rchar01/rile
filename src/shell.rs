// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use crate::{Result, RileError};

const MAX_SHELL_COMMAND_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const SHELL_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const CHILD_STATUS_POLL_INTERVAL: Duration = Duration::from_millis(10);
const CHILD_TERMINATION_WAIT: Duration = Duration::from_secs(1);
const PIPE_BUFFER_BYTES: usize = 8 * 1024;

#[derive(Debug, Clone, Copy)]
struct ShellCommandLimits {
    max_output_bytes: usize,
    timeout: Duration,
}

impl Default for ShellCommandLimits {
    fn default() -> Self {
        Self {
            max_output_bytes: MAX_SHELL_COMMAND_OUTPUT_BYTES,
            timeout: SHELL_COMMAND_TIMEOUT,
        }
    }
}

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
    run_shell_command_with_limits(command, stdin, current_dir, ShellCommandLimits::default())
}

fn run_shell_command_with_limits(
    command: &str,
    stdin: &str,
    current_dir: &Path,
    limits: ShellCommandLimits,
) -> Result<ShellCommandOutput> {
    let mut child = Command::new("/bin/sh")
        .arg("-c")
        .arg(command)
        .current_dir(current_dir)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0)
        .spawn()?;

    let process_group = match libc::pid_t::try_from(child.id()) {
        Ok(process_group) => process_group,
        Err(_) => {
            let error = io::Error::new(
                io::ErrorKind::InvalidData,
                "shell command process ID does not fit pid_t",
            );
            let _ = terminate_child(&mut child, None);
            return Err(error.into());
        }
    };
    let child_stdin = child
        .stdin
        .take()
        .expect("shell stdin was configured as piped");
    let mut child_stdout = child
        .stdout
        .take()
        .expect("shell stdout was configured as piped");
    let mut child_stderr = child
        .stderr
        .take()
        .expect("shell stderr was configured as piped");

    if let Err(error) = set_nonblocking(child_stdin.as_raw_fd())
        .and_then(|()| set_nonblocking(child_stdout.as_raw_fd()))
        .and_then(|()| set_nonblocking(child_stderr.as_raw_fd()))
    {
        return Err(cleanup_error(&mut child, process_group, error.into()));
    }

    let deadline = Instant::now() + limits.timeout;
    let stdin_bytes = stdin.as_bytes();
    let mut stdin_offset = 0;
    let mut child_stdin = (!stdin_bytes.is_empty()).then_some(child_stdin);
    let mut stdout_open = true;
    let mut stderr_open = true;
    let mut stdout_bytes = Vec::new();
    let mut stderr_bytes = Vec::new();
    let mut retained_output_bytes = 0;
    let mut status = None;

    while status.is_none() || stdout_open || stderr_open {
        if status.is_none() {
            match child.try_wait() {
                Ok(child_status) => status = child_status,
                Err(error) => {
                    return Err(cleanup_error(&mut child, process_group, error.into()));
                }
            }
        }
        if status.is_some() {
            child_stdin = None;
        }
        if status.is_some() && !stdout_open && !stderr_open {
            break;
        }

        let mut poll_fds = [
            libc::pollfd {
                fd: child_stdin.as_ref().map_or(-1, AsRawFd::as_raw_fd),
                events: libc::POLLOUT,
                revents: 0,
            },
            libc::pollfd {
                fd: if stdout_open {
                    child_stdout.as_raw_fd()
                } else {
                    -1
                },
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: if stderr_open {
                    child_stderr.as_raw_fd()
                } else {
                    -1
                },
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        let timeout = poll_timeout_millis(deadline);
        // SAFETY: poll_fds is a valid mutable array for the supplied element count.
        let poll_result = unsafe {
            libc::poll(
                poll_fds.as_mut_ptr(),
                poll_fds.len() as libc::nfds_t,
                timeout,
            )
        };
        if poll_result == -1 {
            let error = io::Error::last_os_error();
            if error.kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return Err(cleanup_error(&mut child, process_group, error.into()));
        }
        if let Some(invalid) = poll_fds
            .iter()
            .find(|poll_fd| poll_fd.revents & libc::POLLNVAL != 0)
        {
            let error = io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid shell command pipe descriptor {}", invalid.fd),
            );
            return Err(cleanup_error(&mut child, process_group, error.into()));
        }

        if child_stdin.is_some()
            && poll_fds[0].revents & (libc::POLLOUT | libc::POLLERR | libc::POLLHUP) != 0
        {
            match write_available_stdin(
                child_stdin
                    .as_mut()
                    .expect("polled shell stdin should remain open"),
                stdin_bytes,
                &mut stdin_offset,
            ) {
                Ok(true) => child_stdin = None,
                Ok(false) => {}
                Err(error) => {
                    return Err(cleanup_error(&mut child, process_group, error.into()));
                }
            }
        }

        if stdout_open && poll_fds[1].revents & (libc::POLLIN | libc::POLLERR | libc::POLLHUP) != 0
        {
            match read_available_output(
                &mut child_stdout,
                &mut stdout_bytes,
                &mut retained_output_bytes,
                limits.max_output_bytes,
            ) {
                Ok(OutputRead::Open) => {}
                Ok(OutputRead::Closed) => stdout_open = false,
                Ok(OutputRead::LimitExceeded) => {
                    let error = shell_command_output_limit_error(limits.max_output_bytes);
                    return Err(cleanup_error(&mut child, process_group, error));
                }
                Err(error) => {
                    return Err(cleanup_error(&mut child, process_group, error.into()));
                }
            }
        }

        if stderr_open && poll_fds[2].revents & (libc::POLLIN | libc::POLLERR | libc::POLLHUP) != 0
        {
            match read_available_output(
                &mut child_stderr,
                &mut stderr_bytes,
                &mut retained_output_bytes,
                limits.max_output_bytes,
            ) {
                Ok(OutputRead::Open) => {}
                Ok(OutputRead::Closed) => stderr_open = false,
                Ok(OutputRead::LimitExceeded) => {
                    let error = shell_command_output_limit_error(limits.max_output_bytes);
                    return Err(cleanup_error(&mut child, process_group, error));
                }
                Err(error) => {
                    return Err(cleanup_error(&mut child, process_group, error.into()));
                }
            }
        }

        if status.is_none() {
            match child.try_wait() {
                Ok(child_status) => status = child_status,
                Err(error) => {
                    return Err(cleanup_error(&mut child, process_group, error.into()));
                }
            }
        }
        if status.is_some() {
            child_stdin = None;
        }
        if status.is_some() && !stdout_open && !stderr_open {
            break;
        }
        if Instant::now() >= deadline {
            let error = shell_command_timeout_error(limits.timeout);
            return Err(cleanup_error(&mut child, process_group, error));
        }
    }

    let stdout = String::from_utf8(stdout_bytes).map_err(|error| {
        RileError::InvalidInput(format!("shell command stdout is not UTF-8: {error}"))
    })?;
    let stderr = String::from_utf8(stderr_bytes).map_err(|error| {
        RileError::InvalidInput(format!("shell command stderr is not UTF-8: {error}"))
    })?;

    Ok(ShellCommandOutput {
        stdout,
        stderr,
        status_code: status
            .expect("shell command loop exits only after collecting a status")
            .code(),
    })
}

fn set_nonblocking(fd: RawFd) -> io::Result<()> {
    // SAFETY: fd belongs to a live pipe owned by this process.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    if flags == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fd remains live and F_SETFL accepts the retrieved flags plus O_NONBLOCK.
    if unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn poll_timeout_millis(deadline: Instant) -> libc::c_int {
    let timeout = deadline
        .saturating_duration_since(Instant::now())
        .min(CHILD_STATUS_POLL_INTERVAL);
    timeout.as_millis().max(1).min(libc::c_int::MAX as u128) as libc::c_int
}

fn shell_command_output_limit_error(limit: usize) -> RileError {
    RileError::InvalidInput(format!(
        "shell command output exceeded the {limit}-byte limit"
    ))
}

fn shell_command_timeout_error(timeout: Duration) -> RileError {
    RileError::InvalidInput(format!("shell command timed out after {timeout:?}"))
}

fn write_available_stdin(
    writer: &mut impl Write,
    input: &[u8],
    offset: &mut usize,
) -> io::Result<bool> {
    while *offset < input.len() {
        match writer.write(&input[*offset..]) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(written) => *offset += written,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) if error.kind() == io::ErrorKind::BrokenPipe => return Ok(true),
            Err(error) => return Err(error),
        }
    }
    Ok(true)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OutputRead {
    Open,
    Closed,
    LimitExceeded,
}

fn read_available_output(
    reader: &mut impl Read,
    output: &mut Vec<u8>,
    retained_bytes: &mut usize,
    limit: usize,
) -> io::Result<OutputRead> {
    let mut buffer = [0; PIPE_BUFFER_BYTES];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) => return Ok(OutputRead::Closed),
            Ok(read) if read > limit.saturating_sub(*retained_bytes) => {
                return Ok(OutputRead::LimitExceeded);
            }
            Ok(read) => {
                output.extend_from_slice(&buffer[..read]);
                *retained_bytes += read;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                return Ok(OutputRead::Open);
            }
            Err(error) => return Err(error),
        }
    }
}

fn cleanup_error(child: &mut Child, process_group: libc::pid_t, error: RileError) -> RileError {
    match terminate_child_group(child, process_group) {
        Ok(()) => error,
        Err(cleanup_error) => RileError::InvalidInput(format!(
            "{error}; shell command cleanup also failed: {cleanup_error}"
        )),
    }
}

fn terminate_child_group(child: &mut Child, process_group: libc::pid_t) -> io::Result<()> {
    terminate_child(child, Some(process_group))
}

fn terminate_child(child: &mut Child, process_group: Option<libc::pid_t>) -> io::Result<()> {
    let group_error = process_group.and_then(|process_group| {
        // SAFETY: process_group is the checked positive PID of the child group leader.
        if unsafe { libc::kill(-process_group, libc::SIGKILL) } == -1 {
            let error = io::Error::last_os_error();
            (error.raw_os_error() != Some(libc::ESRCH)).then_some(error)
        } else {
            None
        }
    });
    let direct_kill_error = child.kill().err();
    let deadline = Instant::now() + CHILD_TERMINATION_WAIT;

    loop {
        match child.try_wait() {
            Ok(Some(_)) => return group_error.map_or(Ok(()), Err),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(CHILD_STATUS_POLL_INTERVAL);
            }
            Ok(None) => {
                let mut message = "shell command did not exit after SIGKILL".to_owned();
                if let Some(error) = group_error.as_ref() {
                    message.push_str(&format!("; process-group kill failed: {error}"));
                }
                if let Some(error) = direct_kill_error.as_ref() {
                    message.push_str(&format!("; direct kill failed: {error}"));
                }
                return Err(io::Error::new(io::ErrorKind::TimedOut, message));
            }
            Err(error) => return Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{thread, time::Duration};

    use super::{ShellCommandLimits, run_shell_command, run_shell_command_with_limits};

    fn test_limits(max_output_bytes: usize, timeout: Duration) -> ShellCommandLimits {
        ShellCommandLimits {
            max_output_bytes,
            timeout,
        }
    }

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

    #[test]
    fn accepts_combined_output_exactly_at_limit() {
        let output = run_shell_command_with_limits(
            "printf 'abc'; printf 'def' >&2",
            "",
            std::path::Path::new("."),
            test_limits(6, Duration::from_secs(1)),
        )
        .expect("output at the byte limit should be accepted");

        assert_eq!(output.stdout, "abc");
        assert_eq!(output.stderr, "def");
    }

    #[test]
    fn rejects_combined_output_above_limit() {
        let error = run_shell_command_with_limits(
            "printf 'abc'; printf 'def' >&2",
            "",
            std::path::Path::new("."),
            test_limits(5, Duration::from_secs(1)),
        )
        .expect_err("output above the byte limit should fail");

        assert_eq!(
            error.to_string(),
            "invalid input: shell command output exceeded the 5-byte limit"
        );
    }

    #[test]
    fn terminates_an_infinite_producer_at_output_limit() {
        let error = run_shell_command_with_limits(
            "while :; do printf '0123456789'; done",
            "",
            std::path::Path::new("."),
            test_limits(1024, Duration::from_secs(1)),
        )
        .expect_err("infinite output should hit the byte limit");

        assert_eq!(
            error.to_string(),
            "invalid input: shell command output exceeded the 1024-byte limit"
        );
    }

    #[test]
    fn applies_output_limit_before_utf8_decoding() {
        let output = run_shell_command_with_limits(
            r"printf '\303\251'",
            "",
            std::path::Path::new("."),
            test_limits(2, Duration::from_secs(1)),
        )
        .expect("complete UTF-8 at the byte limit should be accepted");
        assert_eq!(output.stdout, "\u{e9}");

        let error = run_shell_command_with_limits(
            r"printf '\303\251'",
            "",
            std::path::Path::new("."),
            test_limits(1, Duration::from_secs(1)),
        )
        .expect_err("UTF-8 output above the byte limit should fail as oversized");
        assert_eq!(
            error.to_string(),
            "invalid input: shell command output exceeded the 1-byte limit"
        );
    }

    #[test]
    fn times_out_and_reaps_a_silent_command() {
        let error = run_shell_command_with_limits(
            "sleep 1",
            "",
            std::path::Path::new("."),
            test_limits(1024, Duration::from_millis(50)),
        )
        .expect_err("silent command should time out");

        assert_eq!(
            error.to_string(),
            "invalid input: shell command timed out after 50ms"
        );
    }

    #[test]
    fn timeout_terminates_shell_descendants() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let ready = directory.path().join("ready");
        let survived = directory.path().join("survived");

        let error = run_shell_command_with_limits(
            "(sleep 1; printf survived > survived) & printf ready > ready; wait",
            "",
            directory.path(),
            test_limits(1024, Duration::from_millis(500)),
        )
        .expect_err("shell process group should time out");
        assert_eq!(
            error.to_string(),
            "invalid input: shell command timed out after 500ms"
        );
        assert!(ready.exists(), "shell should launch its background child");

        thread::sleep(Duration::from_millis(750));
        assert!(
            !survived.exists(),
            "background child should be killed with its process group"
        );
    }

    #[test]
    fn pumps_large_stdin_and_stdout_without_pipe_deadlock() {
        let stdin = "x".repeat(2 * 1024 * 1024);
        let output = run_shell_command_with_limits(
            "cat",
            &stdin,
            std::path::Path::new("."),
            test_limits(3 * 1024 * 1024, Duration::from_secs(5)),
        )
        .expect("duplex pipe traffic should complete");

        assert_eq!(output.stdout, stdin);
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn succeeds_when_child_closes_large_stdin_without_reading_it() {
        let stdin = "x".repeat(2 * 1024 * 1024);
        let output = run_shell_command_with_limits(
            "printf output",
            &stdin,
            std::path::Path::new("."),
            test_limits(1024, Duration::from_secs(1)),
        )
        .expect("unused region input should not fail the command");

        assert_eq!(output.stdout, "output");
        assert!(output.success());
    }
}
