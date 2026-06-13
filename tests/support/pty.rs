// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, bail};
use expectrl::{Eof, Expect, Session, process::unix::PtyStream, process::unix::UnixProcess};
use tempfile::TempDir;

use super::{fixtures, keys, screen};

type PtySession = Session<UnixProcess, PtyStream>;

pub struct RilePty {
    session: PtySession,
    parser: vt100::Parser,
    _home: TempDir,
    file: PathBuf,
    rows: u16,
    columns: u16,
    last_action: String,
    closed: bool,
}

impl RilePty {
    pub fn spawn(file: &Path, rows: u16, columns: u16) -> Result<Self> {
        let home = fixtures::temp_home()?;
        let binary = assert_cmd::cargo::cargo_bin("rile");
        let mut command = Command::new(binary);
        command
            .arg("--visual-test")
            .arg("--test-size")
            .arg(format!("{columns}x{rows}"))
            .arg(file)
            .env("HOME", home.path())
            .env("NO_COLOR", "1")
            .env("TERM", "xterm-256color");

        let mut session = Session::spawn(command).context("failed to spawn rile in PTY")?;
        session.set_expect_timeout(Some(Duration::from_secs(1)));

        Ok(Self {
            session,
            parser: vt100::Parser::new(rows, columns, 0),
            _home: home,
            file: file.to_path_buf(),
            rows,
            columns,
            last_action: "spawn".to_owned(),
            closed: false,
        })
    }

    pub fn send(&mut self, action: &str, bytes: impl AsRef<[u8]>) -> Result<()> {
        self.last_action = action.to_owned();
        self.session
            .send(bytes)
            .with_context(|| format!("failed to send {action}"))?;
        self.drain_for(Duration::from_millis(50))?;
        Ok(())
    }

    pub fn drain_for(&mut self, duration: Duration) -> Result<()> {
        let deadline = Instant::now() + duration;
        let mut buffer = [0; 4096];
        while Instant::now() < deadline {
            match self.session.try_read(&mut buffer) {
                Ok(0) => return Ok(()),
                Ok(bytes_read) => self.parser.process(&buffer[..bytes_read]),
                Err(error) if error.kind() == ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(10));
                }
                Err(error) => {
                    return Err(error)
                        .with_context(|| format!("failed to read PTY after {}", self.last_action));
                }
            }
        }
        Ok(())
    }

    pub fn wait_for_screen_contains(&mut self, expected: &str) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(2);
        while Instant::now() < deadline {
            self.drain_for(Duration::from_millis(50))?;
            if self.snapshot_text().contains(expected) {
                return Ok(());
            }
        }
        bail!(
            "screen did not contain `{expected}` after {}\n{}",
            self.last_action,
            self.screen_dump()
        );
    }

    pub fn assert_screen_contains(&self, expected: &str) -> Result<()> {
        if !self.snapshot_text().contains(expected) {
            bail!(
                "screen did not contain `{expected}` after {}\n{}",
                self.last_action,
                self.screen_dump()
            );
        }
        Ok(())
    }

    pub fn assert_status_contains(&self, expected: &str) -> Result<()> {
        let status_row = self
            .screen_rows()
            .into_iter()
            .rev()
            .find(|row| row.contains("Rile VISUAL"))
            .unwrap_or_default();
        if !status_row.contains(expected) {
            bail!(
                "status line did not contain `{expected}` after {}\nstatus: {status_row}\n{}",
                self.last_action,
                self.screen_dump()
            );
        }
        Ok(())
    }

    pub fn cursor_position(&self) -> (u16, u16) {
        self.parser.screen().cursor_position()
    }

    pub fn assert_cursor(&self, expected_row: u16, expected_column: u16) -> Result<()> {
        let actual = self.cursor_position();
        if actual != (expected_row, expected_column) {
            bail!(
                "cursor was at {:?}, expected ({expected_row}, {expected_column}) after {}\n{}",
                actual,
                self.last_action,
                self.screen_dump()
            );
        }
        Ok(())
    }

    pub fn snapshot_text(&self) -> String {
        screen::text(self.parser.screen())
    }

    pub fn snapshot_screen(&self) -> String {
        screen::snapshot(self.parser.screen())
    }

    pub fn screen_dump(&self) -> String {
        let mut dump = format!(
            "file: {}\nsize: {}x{}\nlast action: {}\n",
            self.file.display(),
            self.columns,
            self.rows,
            self.last_action
        );
        dump.push_str(&screen::dump(self.parser.screen()));
        dump
    }

    pub fn quit(&mut self) -> Result<()> {
        if self.closed {
            return Ok(());
        }
        self.last_action = "quit".to_owned();
        self.session
            .send(keys::control_sequence("xc"))
            .context("failed to send quit sequence")?;
        self.session
            .expect(Eof)
            .context("rile did not exit cleanly")?;
        self.closed = true;
        Ok(())
    }

    fn screen_rows(&self) -> Vec<String> {
        let (_, columns) = self.parser.screen().size();
        self.parser
            .screen()
            .rows(0, columns)
            .map(|row| row.trim_end().to_owned())
            .collect()
    }
}

impl Drop for RilePty {
    fn drop(&mut self) {
        if !self.closed {
            let _ = self.session.send(keys::control_sequence("xc"));
        }
    }
}
