// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd};
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Stdio};
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};

use crate::{Result, RileError};

const MAX_SHELL_COMMAND_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const SHELL_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const CHILD_STATUS_POLL_INTERVAL: Duration = Duration::from_millis(10);
const CHILD_TERMINATION_WAIT: Duration = Duration::from_secs(1);
const CANCELLATION_GRACE: Duration = Duration::from_millis(250);
const PIPE_BUFFER_BYTES: usize = 8 * 1024;
const POLL_BYTE_BUDGET: usize = 512 * 1024;
const POLL_OPERATION_BUDGET: usize = 256;

static SHELL_REAPER: OnceLock<std::result::Result<ShellReaper, String>> = OnceLock::new();

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

#[derive(Debug)]
pub enum ShellJobPoll<'a> {
    Pending,
    Finished(&'a ShellCommandOutput),
    Failed(&'a RileError),
    Cancelled,
}

#[derive(Debug)]
enum ShellJobState {
    Running,
    Reaping(ReapOutcome),
    Finished(ShellCommandOutput),
    Failed(RileError),
    Cancelled,
}

#[derive(Debug)]
enum ReapOutcome {
    Failed {
        error: RileError,
        reap_deadline: Instant,
    },
    Cancelled {
        kill_at: Instant,
        kill_sent: bool,
        reap_deadline: Option<Instant>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ShellOutputMode {
    Captured,
    Streaming,
}

#[derive(Debug)]
enum ChildOutput {
    Stdout(ChildStdout),
    Stderr(ChildStderr),
    Combined(File),
}

impl Read for ChildOutput {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Stdout(output) => output.read(buffer),
            Self::Stderr(output) => output.read(buffer),
            Self::Combined(output) => output.read(buffer),
        }
    }
}

impl AsRawFd for ChildOutput {
    fn as_raw_fd(&self) -> RawFd {
        match self {
            Self::Stdout(output) => output.as_raw_fd(),
            Self::Stderr(output) => output.as_raw_fd(),
            Self::Combined(output) => output.as_raw_fd(),
        }
    }
}

pub struct ShellJob {
    child: Option<Child>,
    child_stdin: Option<ChildStdin>,
    child_stdout: Option<ChildOutput>,
    child_stderr: Option<ChildOutput>,
    stdin_bytes: Vec<u8>,
    stdin_offset: usize,
    stdout_bytes: Vec<u8>,
    stderr_bytes: Vec<u8>,
    retained_output_bytes: usize,
    deadline: Instant,
    limits: ShellCommandLimits,
    status: Option<ExitStatus>,
    process_group: libc::pid_t,
    next_pipe: usize,
    state: ShellJobState,
    reaper: ShellReaper,
    output_mode: ShellOutputMode,
    streamed_output: String,
}

impl ShellJob {
    pub fn spawn(command: &str, stdin: &str, current_dir: &Path) -> Result<Self> {
        Self::spawn_with_limits_at(
            command,
            stdin,
            current_dir,
            ShellCommandLimits::default(),
            Instant::now(),
        )
    }

    pub fn spawn_streaming(command: &str, stdin: &str, current_dir: &Path) -> Result<Self> {
        Self::spawn_with_mode_and_limits_at(
            command,
            stdin,
            current_dir,
            ShellOutputMode::Streaming,
            ShellCommandLimits::default(),
            Instant::now(),
            shared_shell_reaper,
        )
    }

    fn spawn_with_limits_at(
        command: &str,
        stdin: &str,
        current_dir: &Path,
        limits: ShellCommandLimits,
        started_at: Instant,
    ) -> Result<Self> {
        Self::spawn_with_mode_and_limits_at(
            command,
            stdin,
            current_dir,
            ShellOutputMode::Captured,
            limits,
            started_at,
            shared_shell_reaper,
        )
    }

    fn spawn_with_mode_and_limits_at(
        command: &str,
        stdin: &str,
        current_dir: &Path,
        output_mode: ShellOutputMode,
        limits: ShellCommandLimits,
        started_at: Instant,
        acquire_reaper: impl FnOnce() -> Result<ShellReaper>,
    ) -> Result<Self> {
        let reaper = acquire_reaper()?;
        let mut process = Command::new("/bin/sh");
        process
            .arg("-c")
            .arg(command)
            .current_dir(current_dir)
            .stdin(Stdio::piped())
            .process_group(0);
        let combined_output = if output_mode == ShellOutputMode::Streaming {
            let (parent, child_stdout, child_stderr) = combined_output_pipe()?;
            process
                .stdout(Stdio::from(child_stdout))
                .stderr(Stdio::from(child_stderr));
            Some(parent)
        } else {
            process.stdout(Stdio::piped()).stderr(Stdio::piped());
            None
        };
        let mut child = process.spawn()?;

        let process_group = match libc::pid_t::try_from(child.id()) {
            Ok(process_group) => process_group,
            Err(_) => {
                let error = io::Error::new(
                    io::ErrorKind::InvalidData,
                    "shell command process ID does not fit pid_t",
                );
                return Err(cleanup_spawn_error(child, None, &reaper, error.into()));
            }
        };
        let child_stdin = child
            .stdin
            .take()
            .expect("shell stdin was configured as piped");
        let (child_stdout, child_stderr) = match combined_output {
            Some(output) => (ChildOutput::Combined(output), None),
            None => (
                ChildOutput::Stdout(
                    child
                        .stdout
                        .take()
                        .expect("shell stdout was configured as piped"),
                ),
                Some(ChildOutput::Stderr(
                    child
                        .stderr
                        .take()
                        .expect("shell stderr was configured as piped"),
                )),
            ),
        };

        if let Err(error) = set_nonblocking(child_stdin.as_raw_fd())
            .and_then(|()| set_nonblocking(child_stdout.as_raw_fd()))
            .and_then(|()| {
                child_stderr
                    .as_ref()
                    .map_or(Ok(()), |output| set_nonblocking(output.as_raw_fd()))
            })
        {
            return Err(cleanup_spawn_error(
                child,
                Some(process_group),
                &reaper,
                error.into(),
            ));
        }

        let stdin_bytes = stdin.as_bytes().to_vec();
        Ok(Self {
            child: Some(child),
            child_stdin: (!stdin_bytes.is_empty()).then_some(child_stdin),
            child_stdout: Some(child_stdout),
            child_stderr,
            stdin_bytes,
            stdin_offset: 0,
            stdout_bytes: Vec::new(),
            stderr_bytes: Vec::new(),
            retained_output_bytes: 0,
            deadline: started_at + limits.timeout,
            limits,
            status: None,
            process_group,
            next_pipe: 0,
            state: ShellJobState::Running,
            reaper,
            output_mode,
            streamed_output: String::new(),
        })
    }

    pub fn poll(&mut self) -> ShellJobPoll<'_> {
        self.poll_at(Instant::now())
    }

    fn poll_at(&mut self, now: Instant) -> ShellJobPoll<'_> {
        if matches!(self.state, ShellJobState::Running) {
            self.poll_running(now);
        }
        if matches!(self.state, ShellJobState::Reaping(_)) {
            self.poll_reaping(now);
        }

        match &self.state {
            ShellJobState::Running | ShellJobState::Reaping(_) => ShellJobPoll::Pending,
            ShellJobState::Finished(output) => ShellJobPoll::Finished(output),
            ShellJobState::Failed(error) => ShellJobPoll::Failed(error),
            ShellJobState::Cancelled => ShellJobPoll::Cancelled,
        }
    }

    pub fn request_cancel(&mut self) {
        self.request_cancel_at(Instant::now());
    }

    pub fn is_cancelling(&self) -> bool {
        matches!(
            self.state,
            ShellJobState::Reaping(ReapOutcome::Cancelled { .. })
        )
    }

    pub fn take_streamed_output(&mut self) -> String {
        std::mem::take(&mut self.streamed_output)
    }

    pub fn streams_output(&self) -> bool {
        self.output_mode == ShellOutputMode::Streaming
    }

    fn request_cancel_at(&mut self, now: Instant) {
        if matches!(self.state, ShellJobState::Running) {
            self.close_pipes(true);
            match self.signal(libc::SIGINT) {
                Ok(()) => {
                    self.state = ShellJobState::Reaping(ReapOutcome::Cancelled {
                        kill_at: now + CANCELLATION_GRACE,
                        kill_sent: false,
                        reap_deadline: None,
                    });
                }
                Err(error) => self.begin_failure(
                    RileError::InvalidInput(format!(
                        "shell command cancellation signal failed: {error}"
                    )),
                    now,
                ),
            }
            return;
        }

        let should_escalate = matches!(
            self.state,
            ShellJobState::Reaping(ReapOutcome::Cancelled {
                kill_sent: false,
                ..
            })
        );
        if should_escalate {
            self.escalate_cancellation(now);
        }
    }

    pub fn into_result(mut self) -> Result<ShellCommandOutput> {
        if matches!(
            self.state,
            ShellJobState::Running | ShellJobState::Reaping(_)
        ) {
            return Err(RileError::InvalidInput(
                "shell command has not finished".to_owned(),
            ));
        }
        let state = std::mem::replace(&mut self.state, ShellJobState::Cancelled);
        match state {
            ShellJobState::Finished(output) => Ok(output),
            ShellJobState::Failed(error) => Err(error),
            ShellJobState::Cancelled => Err(RileError::InvalidInput(
                "shell command was cancelled".to_owned(),
            )),
            ShellJobState::Running | ShellJobState::Reaping(_) => {
                unreachable!("running shell job was handled before taking its state")
            }
        }
    }

    fn poll_running(&mut self, now: Instant) {
        let mut budget = PollBudget::new();
        if let Err(error) = self.refresh_status(&mut budget) {
            self.begin_failure(error, now);
            return;
        }
        if self.ready_to_finish() {
            self.finish();
            return;
        }
        if now >= self.deadline {
            self.begin_failure(shell_command_timeout_error(self.limits.timeout), now);
            return;
        }

        while budget.can_poll_io() {
            let mut poll_fds = [
                libc::pollfd {
                    fd: self.child_stdin.as_ref().map_or(-1, AsRawFd::as_raw_fd),
                    events: libc::POLLOUT,
                    revents: 0,
                },
                libc::pollfd {
                    fd: self.child_stdout.as_ref().map_or(-1, AsRawFd::as_raw_fd),
                    events: libc::POLLIN,
                    revents: 0,
                },
                libc::pollfd {
                    fd: self.child_stderr.as_ref().map_or(-1, AsRawFd::as_raw_fd),
                    events: libc::POLLIN,
                    revents: 0,
                },
            ];
            if !budget.take_operation() {
                break;
            }
            // SAFETY: poll_fds is a valid mutable array for the supplied element count.
            let poll_result =
                unsafe { libc::poll(poll_fds.as_mut_ptr(), poll_fds.len() as libc::nfds_t, 0) };
            if poll_result == -1 {
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                self.begin_failure(error.into(), now);
                return;
            }
            if poll_result == 0 {
                break;
            }
            if let Some(invalid) = poll_fds
                .iter()
                .find(|poll_fd| poll_fd.revents & libc::POLLNVAL != 0)
            {
                let error = io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("invalid shell command pipe descriptor {}", invalid.fd),
                );
                self.begin_failure(error.into(), now);
                return;
            }

            let mut serviced = false;
            let first_pipe = self.next_pipe;
            self.next_pipe = (self.next_pipe + 1) % poll_fds.len();
            for offset in 0..poll_fds.len() {
                let pipe = (first_pipe + offset) % poll_fds.len();
                let ready_events = if pipe == 0 {
                    libc::POLLOUT | libc::POLLERR | libc::POLLHUP
                } else {
                    libc::POLLIN | libc::POLLERR | libc::POLLHUP
                };
                if poll_fds[pipe].fd == -1 || poll_fds[pipe].revents & ready_events == 0 {
                    continue;
                }
                serviced = true;
                let result = match pipe {
                    0 => self.write_stdin(&mut budget).map(|_| ()),
                    1 => self.read_stdout(&mut budget),
                    2 => self.read_stderr(&mut budget),
                    _ => unreachable!("shell job has exactly three pipes"),
                };
                if let Err(error) = result {
                    self.begin_failure(error, now);
                    return;
                }
                if !budget.can_io() {
                    break;
                }
            }
            if !serviced {
                break;
            }
        }

        if let Err(error) = self.refresh_status(&mut budget) {
            self.begin_failure(error, now);
            return;
        }
        if self.ready_to_finish() {
            self.finish();
        }
    }

    fn refresh_status(&mut self, budget: &mut PollBudget) -> Result<()> {
        if self.status.is_some() || !budget.take_operation() {
            return Ok(());
        }
        let status = self
            .child
            .as_mut()
            .expect("unreaped shell job should retain its child")
            .try_wait()?;
        if let Some(status) = status {
            self.status = Some(status);
            self.child = None;
            self.child_stdin = None;
        }
        Ok(())
    }

    fn write_stdin(&mut self, budget: &mut PollBudget) -> Result<InputWrite> {
        let result = write_available_stdin(
            self.child_stdin
                .as_mut()
                .expect("ready shell stdin should remain open"),
            &self.stdin_bytes,
            &mut self.stdin_offset,
            budget,
        )?;
        if result == InputWrite::Closed {
            self.child_stdin = None;
        }
        Ok(result)
    }

    fn read_stdout(&mut self, budget: &mut PollBudget) -> Result<()> {
        let result = read_available_output(
            self.child_stdout
                .as_mut()
                .expect("ready shell stdout should remain open"),
            &mut self.stdout_bytes,
            &mut self.retained_output_bytes,
            self.limits.max_output_bytes,
            budget,
        )?;
        if self.output_mode == ShellOutputMode::Streaming {
            self.decode_streamed_output(false)?;
        }
        self.handle_output_read(result, true)
    }

    fn read_stderr(&mut self, budget: &mut PollBudget) -> Result<()> {
        let result = read_available_output(
            self.child_stderr
                .as_mut()
                .expect("ready shell stderr should remain open"),
            &mut self.stderr_bytes,
            &mut self.retained_output_bytes,
            self.limits.max_output_bytes,
            budget,
        )?;
        self.handle_output_read(result, false)
    }

    fn handle_output_read(&mut self, result: OutputRead, stdout: bool) -> Result<()> {
        match result {
            OutputRead::Open => Ok(()),
            OutputRead::Closed => {
                if stdout {
                    if self.output_mode == ShellOutputMode::Streaming {
                        self.decode_streamed_output(true)?;
                    }
                    self.child_stdout = None;
                } else {
                    self.child_stderr = None;
                }
                Ok(())
            }
            OutputRead::LimitExceeded => Err(shell_command_output_limit_error(
                self.limits.max_output_bytes,
            )),
        }
    }

    fn ready_to_finish(&self) -> bool {
        self.status.is_some() && self.child_stdout.is_none() && self.child_stderr.is_none()
    }

    fn finish(&mut self) {
        if self.output_mode == ShellOutputMode::Streaming {
            self.state = ShellJobState::Finished(ShellCommandOutput {
                stdout: String::new(),
                stderr: String::new(),
                status_code: self
                    .status
                    .expect("finished shell job should have an exit status")
                    .code(),
            });
            return;
        }
        let stdout = match String::from_utf8(std::mem::take(&mut self.stdout_bytes)) {
            Ok(stdout) => stdout,
            Err(error) => {
                self.state = ShellJobState::Failed(RileError::InvalidInput(format!(
                    "shell command stdout is not UTF-8: {error}"
                )));
                return;
            }
        };
        let stderr = match String::from_utf8(std::mem::take(&mut self.stderr_bytes)) {
            Ok(stderr) => stderr,
            Err(error) => {
                self.state = ShellJobState::Failed(RileError::InvalidInput(format!(
                    "shell command stderr is not UTF-8: {error}"
                )));
                return;
            }
        };
        self.state = ShellJobState::Finished(ShellCommandOutput {
            stdout,
            stderr,
            status_code: self
                .status
                .expect("finished shell job should have an exit status")
                .code(),
        });
    }

    fn decode_streamed_output(&mut self, eof: bool) -> Result<()> {
        match std::str::from_utf8(&self.stdout_bytes) {
            Ok(text) => {
                self.streamed_output.push_str(text);
                self.stdout_bytes.clear();
                Ok(())
            }
            Err(error) => {
                let valid = error.valid_up_to();
                if valid > 0 {
                    let text = std::str::from_utf8(&self.stdout_bytes[..valid])
                        .expect("UTF-8 validator reported a valid prefix");
                    self.streamed_output.push_str(text);
                    self.stdout_bytes.drain(..valid);
                }
                if error.error_len().is_some() || eof {
                    return Err(RileError::InvalidInput(format!(
                        "shell command output is not UTF-8: {error}"
                    )));
                }
                Ok(())
            }
        }
    }

    fn begin_failure(&mut self, error: RileError, now: Instant) {
        self.close_pipes(true);
        let error = match self.signal(libc::SIGKILL) {
            Ok(()) => error,
            Err(cleanup_error) => append_cleanup_error(error, cleanup_error),
        };
        self.state = ShellJobState::Reaping(ReapOutcome::Failed {
            error,
            reap_deadline: now + CHILD_TERMINATION_WAIT,
        });
    }

    fn poll_reaping(&mut self, now: Instant) {
        if self.status.is_none() {
            match self
                .child
                .as_mut()
                .expect("unreaped shell job should retain its child")
                .try_wait()
            {
                Ok(Some(status)) => {
                    self.status = Some(status);
                    self.child = None;
                }
                Ok(None) => {}
                Err(error) => {
                    self.fail_terminal(
                        RileError::InvalidInput(format!(
                            "shell command status check failed while reaping: {error}"
                        )),
                        true,
                    );
                    return;
                }
            }
        }

        let cancellation = match &self.state {
            ShellJobState::Reaping(ReapOutcome::Cancelled {
                kill_at,
                kill_sent,
                reap_deadline,
            }) => Some((*kill_at, *kill_sent, *reap_deadline)),
            _ => None,
        };
        if let Some((kill_at, false, _)) = cancellation {
            let group_exists = match process_group_exists(self.process_group) {
                Ok(group_exists) => group_exists,
                Err(error) => {
                    self.begin_failure(
                        RileError::InvalidInput(format!(
                            "shell command process-group check failed: {error}"
                        )),
                        now,
                    );
                    return;
                }
            };
            if self.status.is_some() && !group_exists {
                self.state = ShellJobState::Cancelled;
                return;
            }
            if now >= kill_at {
                self.escalate_cancellation(now);
            }
        }

        let state = std::mem::replace(&mut self.state, ShellJobState::Cancelled);
        self.state = match state {
            ShellJobState::Reaping(ReapOutcome::Failed {
                error,
                reap_deadline: _,
            }) if self.status.is_some() => ShellJobState::Failed(error),
            ShellJobState::Reaping(ReapOutcome::Failed {
                error,
                reap_deadline,
            }) if now >= reap_deadline => {
                self.finish_reap_timeout(error);
                return;
            }
            ShellJobState::Reaping(ReapOutcome::Cancelled {
                kill_sent: true, ..
            }) if self.status.is_some() => ShellJobState::Cancelled,
            ShellJobState::Reaping(ReapOutcome::Cancelled {
                kill_sent: true,
                reap_deadline: Some(reap_deadline),
                ..
            }) if now >= reap_deadline => {
                self.finish_reap_timeout(RileError::InvalidInput(
                    "shell command cancellation did not reap the child after SIGKILL".to_owned(),
                ));
                return;
            }
            state => state,
        };
    }

    fn escalate_cancellation(&mut self, now: Instant) {
        match self.signal(libc::SIGKILL) {
            Ok(()) => {
                if let ShellJobState::Reaping(ReapOutcome::Cancelled {
                    kill_sent,
                    reap_deadline,
                    ..
                }) = &mut self.state
                {
                    *kill_sent = true;
                    *reap_deadline = Some(now + CHILD_TERMINATION_WAIT);
                }
            }
            Err(error) => self.begin_failure(
                RileError::InvalidInput(format!(
                    "shell command cancellation SIGKILL failed: {error}"
                )),
                now,
            ),
        }
    }

    fn finish_reap_timeout(&mut self, error: RileError) {
        let error = match self.signal(libc::SIGKILL) {
            Ok(()) => append_cleanup_error(
                error,
                io::Error::new(
                    io::ErrorKind::TimedOut,
                    "shell command did not exit after SIGKILL",
                ),
            ),
            Err(signal_error) => append_cleanup_error(
                append_cleanup_error(
                    error,
                    io::Error::new(
                        io::ErrorKind::TimedOut,
                        "shell command did not exit after SIGKILL",
                    ),
                ),
                signal_error,
            ),
        };
        self.fail_terminal(error, false);
    }

    fn fail_terminal(&mut self, error: RileError, retry_kill: bool) {
        self.close_pipes(true);
        let error = if retry_kill {
            match self.signal(libc::SIGKILL) {
                Ok(()) => error,
                Err(signal_error) => append_cleanup_error(error, signal_error),
            }
        } else {
            error
        };
        self.handoff_unreaped_child();
        self.state = ShellJobState::Failed(error);
    }

    fn handoff_unreaped_child(&mut self) {
        let Some(child) = self.child.take() else {
            return;
        };
        self.reaper.handoff(child);
    }

    fn close_pipes(&mut self, discard_output: bool) {
        self.child_stdin = None;
        self.child_stdout = None;
        self.child_stderr = None;
        self.stdin_bytes.clear();
        if discard_output {
            self.stdout_bytes.clear();
            self.stderr_bytes.clear();
            self.retained_output_bytes = 0;
        }
    }

    fn signal(&mut self, signal: libc::c_int) -> io::Result<()> {
        signal_child_group_and_direct(self.process_group, self.child.is_some(), signal)
    }
}

impl Drop for ShellJob {
    fn drop(&mut self) {
        self.close_pipes(true);
        let needs_cleanup = self.child.is_some()
            || matches!(
                self.state,
                ShellJobState::Running | ShellJobState::Reaping(_)
            );
        if needs_cleanup {
            if self.signal(libc::SIGKILL).is_err()
                && let Some(child) = self.child.as_mut()
            {
                let _ = child.kill();
            }
            self.handoff_unreaped_child();
        }
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
    let mut job =
        ShellJob::spawn_with_limits_at(command, stdin, current_dir, limits, Instant::now())?;
    loop {
        let terminal = !matches!(job.poll(), ShellJobPoll::Pending);
        if terminal {
            return job.into_result();
        }
        std::thread::sleep(CHILD_STATUS_POLL_INTERVAL);
    }
}

#[derive(Debug)]
struct PollBudget {
    remaining_bytes: usize,
    remaining_operations: usize,
}

impl PollBudget {
    fn new() -> Self {
        Self {
            remaining_bytes: POLL_BYTE_BUDGET,
            remaining_operations: POLL_OPERATION_BUDGET,
        }
    }

    fn can_poll_io(&self) -> bool {
        self.remaining_bytes > 0 && self.remaining_operations >= 2
    }

    fn can_io(&self) -> bool {
        self.remaining_bytes > 0 && self.remaining_operations > 0
    }

    fn take_operation(&mut self) -> bool {
        if self.remaining_operations == 0 {
            return false;
        }
        self.remaining_operations -= 1;
        true
    }

    fn io_len(&mut self, requested: usize) -> Option<usize> {
        if !self.can_io() || !self.take_operation() {
            return None;
        }
        Some(requested.min(self.remaining_bytes))
    }

    fn record_bytes(&mut self, bytes: usize) {
        self.remaining_bytes -= bytes;
    }
}

fn combined_output_pipe() -> io::Result<(File, OwnedFd, OwnedFd)> {
    let mut descriptors = [-1; 2];
    // SAFETY: descriptors points to storage for the two pipe descriptors.
    if unsafe { libc::pipe2(descriptors.as_mut_ptr(), libc::O_CLOEXEC) } == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: pipe2 initialized both descriptors, and each is given one owner.
    let read_end = unsafe { OwnedFd::from_raw_fd(descriptors[0]) };
    // SAFETY: pipe2 initialized both descriptors, and each is given one owner.
    let write_end = unsafe { OwnedFd::from_raw_fd(descriptors[1]) };
    set_nonblocking(read_end.as_raw_fd())?;

    // SAFETY: F_DUPFD_CLOEXEC duplicates the live write descriptor with a new owner.
    let stderr_fd = unsafe { libc::fcntl(write_end.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if stderr_fd == -1 {
        return Err(io::Error::last_os_error());
    }
    // SAFETY: fcntl returned a new owned descriptor.
    let stderr_end = unsafe { OwnedFd::from_raw_fd(stderr_fd) };
    Ok((File::from(read_end), write_end, stderr_end))
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
    budget: &mut PollBudget,
) -> io::Result<InputWrite> {
    if *offset == input.len() {
        return Ok(InputWrite::Closed);
    }
    let Some(write_len) = budget.io_len((input.len() - *offset).min(PIPE_BUFFER_BYTES)) else {
        return Ok(InputWrite::Open);
    };
    match writer.write(&input[*offset..*offset + write_len]) {
        Ok(0) => Err(io::ErrorKind::WriteZero.into()),
        Ok(written) => {
            *offset += written;
            budget.record_bytes(written);
            Ok(if *offset == input.len() {
                InputWrite::Closed
            } else {
                InputWrite::Open
            })
        }
        Err(error) if error.kind() == io::ErrorKind::Interrupted => Ok(InputWrite::Open),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(InputWrite::Open),
        Err(error) if error.kind() == io::ErrorKind::BrokenPipe => Ok(InputWrite::Closed),
        Err(error) => Err(error),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputWrite {
    Open,
    Closed,
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
    budget: &mut PollBudget,
) -> io::Result<OutputRead> {
    let Some(read_len) = budget.io_len(PIPE_BUFFER_BYTES) else {
        return Ok(OutputRead::Open);
    };
    let mut buffer = [0; PIPE_BUFFER_BYTES];
    match reader.read(&mut buffer[..read_len]) {
        Ok(0) => Ok(OutputRead::Closed),
        Ok(read) => {
            budget.record_bytes(read);
            if read > limit.saturating_sub(*retained_bytes) {
                return Ok(OutputRead::LimitExceeded);
            }
            output.extend_from_slice(&buffer[..read]);
            *retained_bytes += read;
            Ok(OutputRead::Open)
        }
        Err(error) if error.kind() == io::ErrorKind::Interrupted => Ok(OutputRead::Open),
        Err(error) if error.kind() == io::ErrorKind::WouldBlock => Ok(OutputRead::Open),
        Err(error) => Err(error),
    }
}

fn append_cleanup_error(error: RileError, cleanup_error: io::Error) -> RileError {
    let message = match error {
        RileError::InvalidInput(message) => message,
        error => error.to_string(),
    };
    RileError::InvalidInput(format!(
        "{message}; shell command cleanup also failed: {cleanup_error}"
    ))
}

fn signal_child_group_and_direct(
    process_group: libc::pid_t,
    direct_child_may_be_alive: bool,
    signal: libc::c_int,
) -> io::Result<()> {
    // SAFETY: process_group is the checked positive PID of the child group leader.
    let group_error = signal_error(unsafe { libc::kill(-process_group, signal) });

    let direct_error = direct_child_may_be_alive.then(|| {
        // SAFETY: process_group is also the checked PID of the unreaped direct child.
        signal_error(unsafe { libc::kill(process_group, signal) })
    });
    let direct_error = direct_error.flatten();
    if group_error.is_none() && direct_error.is_none() {
        return Ok(());
    }

    let mut failures = Vec::new();
    if let Some(error) = group_error {
        failures.push(format!("process-group signal failed: {error}"));
    }
    if let Some(error) = direct_error {
        failures.push(format!("direct signal failed: {error}"));
    }
    Err(io::Error::other(failures.join("; ")))
}

fn signal_error(result: libc::c_int) -> Option<io::Error> {
    if result == 0 {
        return None;
    }
    let error = io::Error::last_os_error();
    (error.raw_os_error() != Some(libc::ESRCH)).then_some(error)
}

fn process_group_exists(process_group: libc::pid_t) -> io::Result<bool> {
    // SAFETY: process_group is the checked positive PID of the original child group leader.
    if unsafe { libc::kill(-process_group, 0) } == 0 {
        return Ok(true);
    }
    let error = io::Error::last_os_error();
    match error.raw_os_error() {
        Some(libc::ESRCH) => Ok(false),
        Some(libc::EPERM) => Ok(true),
        _ => Err(error),
    }
}

#[derive(Debug, Clone)]
struct ShellReaper {
    queue: Arc<ShellReaperQueue>,
}

#[derive(Debug, Default)]
struct ShellReaperQueue {
    children: Mutex<Vec<ReaperChild>>,
    wake: Condvar,
}

#[derive(Debug)]
struct ReaperChild {
    child: Child,
    retry_at: Instant,
    unexpected_errors: u32,
}

impl ReaperChild {
    fn new(child: Child) -> Self {
        Self {
            child,
            retry_at: Instant::now(),
            unexpected_errors: 0,
        }
    }

    fn poll(&mut self, now: Instant) -> bool {
        if now < self.retry_at {
            return true;
        }
        let retain = should_retry_reap(self.child.try_wait(), &mut self.unexpected_errors);
        if retain {
            let shift = self.unexpected_errors.saturating_sub(1).min(6);
            self.retry_at = now + CHILD_STATUS_POLL_INTERVAL * (1 << shift);
        }
        retain
    }
}

impl ShellReaper {
    fn start() -> io::Result<Self> {
        Self::start_with(|queue| {
            std::thread::Builder::new()
                .name("rile-shell-reaper".to_owned())
                .spawn(move || reap_children(queue))
                .map(|_| ())
        })
    }

    fn start_with(spawn: impl FnOnce(Arc<ShellReaperQueue>) -> io::Result<()>) -> io::Result<Self> {
        let queue = Arc::new(ShellReaperQueue::default());
        spawn(Arc::clone(&queue))?;
        Ok(Self { queue })
    }

    fn handoff(&self, child: Child) {
        let mut children = self
            .queue
            .children
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        children.push(ReaperChild::new(child));
        self.queue.wake.notify_one();
    }
}

fn shared_shell_reaper() -> Result<ShellReaper> {
    match SHELL_REAPER.get_or_init(|| ShellReaper::start().map_err(|error| error.to_string())) {
        Ok(reaper) => Ok(reaper.clone()),
        Err(error) => Err(RileError::InvalidInput(format!(
            "cannot start shell command reaper: {error}"
        ))),
    }
}

fn reap_children(queue: Arc<ShellReaperQueue>) {
    loop {
        let mut children = queue
            .children
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while children.is_empty() {
            children = queue
                .wake
                .wait(children)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        let mut pending = std::mem::take(&mut *children);
        drop(children);

        let now = Instant::now();
        pending.retain_mut(|child| child.poll(now));
        if pending.is_empty() {
            continue;
        }
        std::thread::sleep(CHILD_STATUS_POLL_INTERVAL);

        let mut children = queue
            .children
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        children.extend(pending);
    }
}

fn should_retry_reap(result: io::Result<Option<ExitStatus>>, unexpected_errors: &mut u32) -> bool {
    match result {
        Ok(None) => {
            *unexpected_errors = 0;
            true
        }
        Err(error) if error.kind() == io::ErrorKind::Interrupted => true,
        Err(error) if error.raw_os_error() == Some(libc::ECHILD) => false,
        Err(_) => {
            *unexpected_errors = unexpected_errors.saturating_add(1);
            true
        }
        Ok(Some(_)) => false,
    }
}

fn cleanup_spawn_error(
    mut child: Child,
    process_group: Option<libc::pid_t>,
    reaper: &ShellReaper,
    error: RileError,
) -> RileError {
    let signal_error = match process_group {
        Some(process_group) => {
            signal_child_group_and_direct(process_group, true, libc::SIGKILL).err()
        }
        None => child.kill().err(),
    };
    reaper.handoff(child);
    match signal_error {
        Some(cleanup_error) => append_cleanup_error(error, cleanup_error),
        None => error,
    }
}

#[cfg(test)]
mod tests {
    use std::{
        io, thread,
        time::{Duration, Instant},
    };

    use super::{
        CANCELLATION_GRACE, InputWrite, OutputRead, POLL_BYTE_BUDGET, POLL_OPERATION_BUDGET,
        PollBudget, ShellCommandLimits, ShellJob, ShellJobPoll, read_available_output,
        run_shell_command, run_shell_command_with_limits, should_retry_reap, write_available_stdin,
    };

    struct AlwaysReadyReader;
    struct AlwaysReadyWriter;

    impl io::Read for AlwaysReadyReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            buffer.fill(b'x');
            Ok(buffer.len())
        }
    }

    impl io::Write for AlwaysReadyWriter {
        fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
            Ok(buffer.len().min(1))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn test_limits(max_output_bytes: usize, timeout: Duration) -> ShellCommandLimits {
        ShellCommandLimits {
            max_output_bytes,
            timeout,
        }
    }

    fn wait_for_path(path: &std::path::Path) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while !path.exists() && Instant::now() < deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(path.exists(), "expected path was not created: {path:?}");
    }

    fn process_exists(process: libc::pid_t) -> bool {
        // SAFETY: signal 0 checks the observed child PID without sending a signal.
        if unsafe { libc::kill(process, 0) } == 0 {
            return true;
        }
        io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }

    fn poll_until_terminal(job: &mut ShellJob) -> ShellJobTerminal {
        let test_deadline = Instant::now() + Duration::from_secs(2);
        loop {
            match job.poll() {
                ShellJobPoll::Pending if Instant::now() < test_deadline => {
                    thread::sleep(Duration::from_millis(5));
                }
                ShellJobPoll::Pending => panic!("shell job did not become terminal"),
                ShellJobPoll::Finished(_) => return ShellJobTerminal::Finished,
                ShellJobPoll::Failed(error) => panic!("shell job failed: {error}"),
                ShellJobPoll::Cancelled => return ShellJobTerminal::Cancelled,
            }
        }
    }

    fn poll_until_output(job: &mut ShellJob) {
        let test_deadline = Instant::now() + Duration::from_secs(2);
        while job.stdout_bytes.is_empty() && Instant::now() < test_deadline {
            assert!(matches!(job.poll(), ShellJobPoll::Pending));
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            !job.stdout_bytes.is_empty(),
            "shell command should become ready"
        );
    }

    #[derive(Debug, PartialEq, Eq)]
    enum ShellJobTerminal {
        Finished,
        Cancelled,
    }

    #[test]
    fn reaper_initialization_failure_prevents_child_spawn() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let result = ShellJob::spawn_with_mode_and_limits_at(
            "printf spawned > spawned",
            "",
            directory.path(),
            super::ShellOutputMode::Captured,
            test_limits(1024, Duration::from_secs(1)),
            Instant::now(),
            || {
                Err(crate::RileError::InvalidInput(
                    "synthetic reaper failure".to_owned(),
                ))
            },
        );
        let error = match result {
            Ok(_) => panic!("reaper startup should fail"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "invalid input: synthetic reaper failure");
        assert!(!directory.path().join("spawned").exists());
    }

    #[test]
    fn reaper_handles_terminal_and_retryable_wait_results() {
        let mut unexpected_errors = 0;
        assert!(should_retry_reap(Ok(None), &mut unexpected_errors));
        assert!(should_retry_reap(
            Err(io::ErrorKind::Interrupted.into()),
            &mut unexpected_errors
        ));
        assert!(!should_retry_reap(
            Err(io::Error::from_raw_os_error(libc::ECHILD)),
            &mut unexpected_errors
        ));
        assert!(should_retry_reap(
            Err(io::ErrorKind::InvalidInput.into()),
            &mut unexpected_errors
        ));
        assert_eq!(unexpected_errors, 1);
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
    fn streaming_combines_stdout_and_stderr_in_pipe_order() {
        let mut job = ShellJob::spawn_streaming(
            "printf stdout; printf stderr >&2",
            "",
            std::path::Path::new("."),
        )
        .expect("streaming shell job should spawn");
        let mut streamed = String::new();

        loop {
            let terminal = !matches!(job.poll(), ShellJobPoll::Pending);
            streamed.push_str(&job.take_streamed_output());
            if terminal {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(streamed, "stdoutstderr");
        assert!(job.into_result().expect("job should succeed").success());
    }

    #[test]
    fn streaming_emits_output_before_process_exit() {
        let mut job = ShellJob::spawn_streaming(
            "printf first; sleep 0.2; printf second",
            "",
            std::path::Path::new("."),
        )
        .expect("streaming shell job should spawn");
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut first = String::new();

        while first.is_empty() && Instant::now() < deadline {
            assert!(matches!(job.poll(), ShellJobPoll::Pending));
            first.push_str(&job.take_streamed_output());
            thread::sleep(Duration::from_millis(5));
        }

        assert_eq!(first, "first");
        assert!(matches!(job.poll(), ShellJobPoll::Pending));
        assert_eq!(poll_until_terminal(&mut job), ShellJobTerminal::Finished);
        assert_eq!(job.take_streamed_output(), "second");
    }

    #[test]
    fn streaming_retains_incomplete_utf8_between_polls() {
        let mut job = ShellJob::spawn_streaming(
            r"printf '\303'; sleep 0.1; printf '\251'",
            "",
            std::path::Path::new("."),
        )
        .expect("streaming shell job should spawn");
        let deadline = Instant::now() + Duration::from_secs(1);

        while job.stdout_bytes.is_empty() && Instant::now() < deadline {
            assert!(matches!(job.poll(), ShellJobPoll::Pending));
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(job.stdout_bytes, [0xc3]);
        assert!(job.take_streamed_output().is_empty());

        assert_eq!(poll_until_terminal(&mut job), ShellJobTerminal::Finished);
        assert_eq!(job.take_streamed_output(), "é");
    }

    #[test]
    fn streaming_output_limit_counts_already_delivered_bytes() {
        let mut job = ShellJob::spawn_with_mode_and_limits_at(
            "printf abc; sleep 0.1; printf def",
            "",
            std::path::Path::new("."),
            super::ShellOutputMode::Streaming,
            test_limits(5, Duration::from_secs(1)),
            Instant::now(),
            super::shared_shell_reaper,
        )
        .expect("streaming shell job should spawn");
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut streamed = String::new();

        while streamed.is_empty() && Instant::now() < deadline {
            assert!(matches!(job.poll(), ShellJobPoll::Pending));
            streamed.push_str(&job.take_streamed_output());
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(streamed, "abc");

        let failure = loop {
            match job.poll() {
                ShellJobPoll::Pending => thread::sleep(Duration::from_millis(5)),
                ShellJobPoll::Failed(error) => break error.to_string(),
                poll => panic!("expected streaming limit failure, got {poll:?}"),
            }
        };
        assert_eq!(
            failure,
            "invalid input: shell command output exceeded the 5-byte limit"
        );
        assert!(job.take_streamed_output().is_empty());
    }

    #[test]
    fn streaming_preserves_delivered_prefix_across_invalid_utf8_failure() {
        let mut job = ShellJob::spawn_streaming(
            r"printf good; sleep 0.1; printf '\377'",
            "",
            std::path::Path::new("."),
        )
        .expect("streaming shell job should spawn");
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut streamed = String::new();

        while streamed.is_empty() && Instant::now() < deadline {
            assert!(matches!(job.poll(), ShellJobPoll::Pending));
            streamed.push_str(&job.take_streamed_output());
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(streamed, "good");

        let failure = loop {
            match job.poll() {
                ShellJobPoll::Pending => {
                    streamed.push_str(&job.take_streamed_output());
                    thread::sleep(Duration::from_millis(5));
                }
                ShellJobPoll::Failed(error) => break error.to_string(),
                poll => panic!("expected UTF-8 failure, got {poll:?}"),
            }
        };
        streamed.push_str(&job.take_streamed_output());
        assert_eq!(streamed, "good");
        assert!(failure.contains("shell command output is not UTF-8"));
    }

    #[test]
    fn streaming_preserves_delivered_prefix_after_cancellation() {
        let mut job = ShellJob::spawn_streaming(
            "printf partial; trap '' 2; while :; do sleep 1; done",
            "",
            std::path::Path::new("."),
        )
        .expect("streaming shell job should spawn");
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut streamed = String::new();

        while streamed.is_empty() && Instant::now() < deadline {
            assert!(matches!(job.poll(), ShellJobPoll::Pending));
            streamed.push_str(&job.take_streamed_output());
            thread::sleep(Duration::from_millis(5));
        }
        assert_eq!(streamed, "partial");

        job.request_cancel();
        job.request_cancel();
        assert_eq!(poll_until_terminal(&mut job), ShellJobTerminal::Cancelled);
        assert!(job.take_streamed_output().is_empty());
        assert_eq!(streamed, "partial");
    }

    #[test]
    fn public_poll_returns_pending_for_sleep() {
        let now = Instant::now();
        let mut job = ShellJob::spawn_with_limits_at(
            "sleep 1",
            "",
            std::path::Path::new("."),
            test_limits(1024, Duration::from_secs(2)),
            now,
        )
        .expect("shell job should spawn");

        assert!(matches!(job.poll(), ShellJobPoll::Pending));
        job.request_cancel();
        job.request_cancel();
        assert_eq!(poll_until_terminal(&mut job), ShellJobTerminal::Cancelled);
    }

    #[test]
    fn cancellation_discards_partial_output_and_completes() {
        let mut job = ShellJob::spawn(
            "trap 'exit 0' 2; printf ready; while :; do sleep 1; done",
            "",
            std::path::Path::new("."),
        )
        .expect("shell job should spawn");
        poll_until_output(&mut job);

        job.request_cancel();

        assert!(job.stdout_bytes.is_empty());
        assert!(job.stderr_bytes.is_empty());
        assert_eq!(poll_until_terminal(&mut job), ShellJobTerminal::Cancelled);
    }

    #[test]
    fn second_cancellation_escalates_immediately() {
        let mut job = ShellJob::spawn(
            "trap '' 2; printf ready; while :; do sleep 1; done",
            "",
            std::path::Path::new("."),
        )
        .expect("shell job should spawn");
        poll_until_output(&mut job);
        let now = Instant::now();

        job.request_cancel_at(now);
        job.request_cancel_at(now);

        assert!(matches!(
            job.state,
            super::ShellJobState::Reaping(super::ReapOutcome::Cancelled {
                kill_sent: true,
                reap_deadline: Some(_),
                ..
            })
        ));
        assert_eq!(poll_until_terminal(&mut job), ShellJobTerminal::Cancelled);
    }

    #[test]
    fn poll_escalates_cancellation_after_grace() {
        let mut job = ShellJob::spawn(
            "trap '' 2; printf ready; while :; do sleep 1; done",
            "",
            std::path::Path::new("."),
        )
        .expect("shell job should spawn");
        poll_until_output(&mut job);
        let now = Instant::now();
        job.request_cancel_at(now);

        assert!(matches!(
            job.poll_at(now + CANCELLATION_GRACE),
            ShellJobPoll::Pending | ShellJobPoll::Cancelled
        ));
        assert!(matches!(
            job.state,
            super::ShellJobState::Reaping(super::ReapOutcome::Cancelled {
                kill_sent: true,
                reap_deadline: Some(_),
                ..
            }) | super::ShellJobState::Cancelled
        ));
        assert_eq!(poll_until_terminal(&mut job), ShellJobTerminal::Cancelled);
    }

    #[test]
    fn cancellation_escalation_kills_sigint_ignoring_descendant() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let ready = directory.path().join("ready");
        let survived = directory.path().join("survived");
        let mut job = ShellJob::spawn(
            "trap 'exit 0' 2; (trap '' 2; sleep 1; printf survived > survived) & printf ready > ready; wait",
            "",
            directory.path(),
        )
        .expect("shell job should spawn");
        wait_for_path(&ready);
        let cancelled_at = Instant::now();

        job.request_cancel_at(cancelled_at);
        let direct_reap_deadline = Instant::now() + Duration::from_secs(2);
        while job.status.is_none() && Instant::now() < direct_reap_deadline {
            assert!(matches!(job.poll_at(cancelled_at), ShellJobPoll::Pending));
            thread::sleep(Duration::from_millis(5));
        }
        assert!(
            job.status.is_some(),
            "direct shell should exit after SIGINT"
        );
        assert!(matches!(job.poll_at(cancelled_at), ShellJobPoll::Pending));

        assert!(matches!(
            job.poll_at(cancelled_at + CANCELLATION_GRACE),
            ShellJobPoll::Pending | ShellJobPoll::Cancelled
        ));
        assert_eq!(poll_until_terminal(&mut job), ShellJobTerminal::Cancelled);
        thread::sleep(Duration::from_millis(1100));
        assert!(
            !survived.exists(),
            "SIGKILL escalation should stop the surviving process-group member"
        );
    }

    #[test]
    fn reap_state_has_a_bounded_terminal_failure() {
        let now = Instant::now();
        let mut job = ShellJob::spawn("sleep 10", "", std::path::Path::new("."))
            .expect("shell job should spawn");
        job.close_pipes(true);
        job.state = super::ShellJobState::Reaping(super::ReapOutcome::Failed {
            error: crate::RileError::InvalidInput("synthetic shell failure".to_owned()),
            reap_deadline: now,
        });

        match job.poll_at(now) {
            ShellJobPoll::Failed(error) => {
                assert!(error.to_string().contains("did not exit after SIGKILL"));
            }
            poll => panic!("expected bounded failure, got {poll:?}"),
        }
        assert!(job.child.is_none(), "unreaped child should be handed off");
    }

    #[test]
    fn drop_kills_and_reaps_an_active_child() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let ready = directory.path().join("ready");
        let survived = directory.path().join("survived");
        let job = ShellJob::spawn(
            "(trap '' 2; sleep 1; printf survived > survived) & printf ready > ready; wait",
            "",
            directory.path(),
        )
        .expect("shell job should spawn");
        let child_pid = job.process_group;
        wait_for_path(&ready);

        drop(job);

        let reap_deadline = Instant::now() + Duration::from_secs(2);
        while process_exists(child_pid) && Instant::now() < reap_deadline {
            thread::sleep(Duration::from_millis(5));
        }
        assert!(!process_exists(child_pid), "dropped child should be reaped");
        thread::sleep(Duration::from_millis(1100));
        assert!(
            !survived.exists(),
            "drop should kill the active process group"
        );
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
    fn event_loop_cadence_preserves_large_duplex_throughput() {
        let stdin = "x".repeat(2 * 1024 * 1024);
        let mut job = ShellJob::spawn_with_limits_at(
            "cat",
            &stdin,
            std::path::Path::new("."),
            test_limits(3 * 1024 * 1024, Duration::from_secs(5)),
            Instant::now(),
        )
        .expect("shell job should spawn");
        let test_deadline = Instant::now() + Duration::from_secs(8);

        loop {
            match job.poll() {
                ShellJobPoll::Pending if Instant::now() < test_deadline => {
                    thread::sleep(Duration::from_millis(100));
                }
                ShellJobPoll::Pending => panic!("large duplex shell job did not complete"),
                ShellJobPoll::Finished(_) => break,
                ShellJobPoll::Failed(error) => panic!("large duplex shell job failed: {error}"),
                ShellJobPoll::Cancelled => panic!("large duplex shell job was cancelled"),
            }
        }

        let output = job.into_result().expect("shell job should succeed");
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

    #[test]
    fn continuously_ready_reader_respects_per_poll_byte_budget() {
        let mut output = Vec::new();
        let mut retained = 0;
        let mut budget = PollBudget::new();
        while budget.can_io() {
            assert_eq!(
                read_available_output(
                    &mut AlwaysReadyReader,
                    &mut output,
                    &mut retained,
                    usize::MAX,
                    &mut budget,
                )
                .expect("synthetic reader should not fail"),
                OutputRead::Open
            );
        }

        assert_eq!(retained, POLL_BYTE_BUDGET);
    }

    #[test]
    fn continuously_ready_writer_respects_per_poll_operation_budget() {
        let input = vec![b'x'; POLL_BYTE_BUDGET];
        let mut offset = 0;
        let mut budget = PollBudget::new();
        while budget.can_io() {
            assert_eq!(
                write_available_stdin(&mut AlwaysReadyWriter, &input, &mut offset, &mut budget,)
                    .expect("synthetic writer should not fail"),
                InputWrite::Open
            );
        }

        assert_eq!(offset, POLL_OPERATION_BUDGET);
    }
}
