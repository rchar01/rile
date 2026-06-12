// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{self, IsTerminal, Read, Write};
use std::os::fd::AsRawFd;
use std::path::Path;

use crate::editor::{Editor, EditorOutcome};
use crate::file::Document;
use crate::input::KeyReader;
use crate::{Result, RileError};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub rows: u16,
    pub columns: u16,
}

pub struct RawModeGuard {
    fd: libc::c_int,
    original: Option<libc::termios>,
    active: bool,
}

impl RawModeGuard {
    pub fn activate(fd: libc::c_int) -> Result<Self> {
        let mut original = std::mem::MaybeUninit::<libc::termios>::uninit();
        // SAFETY: tcgetattr initializes the termios struct for a valid terminal fd.
        if unsafe { libc::tcgetattr(fd, original.as_mut_ptr()) } == -1 {
            return Err(io::Error::last_os_error().into());
        }

        // SAFETY: tcgetattr succeeded, so original has been initialized.
        let original = unsafe { original.assume_init() };
        let mut raw = original;
        raw.c_iflag &= !(libc::BRKINT | libc::ICRNL | libc::INPCK | libc::ISTRIP | libc::IXON);
        raw.c_oflag &= !libc::OPOST;
        raw.c_cflag |= libc::CS8;
        raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::IEXTEN | libc::ISIG);
        raw.c_cc[libc::VMIN] = 0;
        raw.c_cc[libc::VTIME] = 1;

        // SAFETY: raw is derived from a valid termios value for this fd.
        if unsafe { libc::tcsetattr(fd, libc::TCSAFLUSH, &raw) } == -1 {
            return Err(io::Error::last_os_error().into());
        }

        Ok(Self {
            fd,
            original: Some(original),
            active: true,
        })
    }

    pub fn inactive() -> Self {
        Self {
            fd: -1,
            original: None,
            active: false,
        }
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn disable(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }

        if let Some(original) = self.original.as_ref() {
            // SAFETY: original is the termios value captured from this fd before raw mode.
            if unsafe { libc::tcsetattr(self.fd, libc::TCSAFLUSH, original) } == -1 {
                return Err(io::Error::last_os_error().into());
            }
        }
        self.active = false;
        Ok(())
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if self.active {
            let _ = self.disable();
        }
    }
}

pub fn terminal_size(fd: libc::c_int) -> Result<TerminalSize> {
    let mut size = std::mem::MaybeUninit::<libc::winsize>::zeroed();
    // SAFETY: ioctl writes a winsize struct for a valid terminal fd.
    if unsafe { libc::ioctl(fd, libc::TIOCGWINSZ, size.as_mut_ptr()) } == -1 {
        return Err(io::Error::last_os_error().into());
    }

    // SAFETY: ioctl succeeded, so size has been initialized.
    let size = unsafe { size.assume_init() };
    if size.ws_row == 0 || size.ws_col == 0 {
        return Ok(TerminalSize {
            rows: 24,
            columns: 80,
        });
    }

    Ok(TerminalSize {
        rows: size.ws_row,
        columns: size.ws_col,
    })
}

pub struct AnsiTerminal<W> {
    writer: W,
}

impl<W: Write> AnsiTerminal<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    pub fn into_inner(self) -> W {
        self.writer
    }

    pub fn enter_alternate_screen(&mut self) -> Result<()> {
        self.write_escape("?1049h")
    }

    pub fn leave_alternate_screen(&mut self) -> Result<()> {
        self.write_escape("?1049l")
    }

    pub fn hide_cursor(&mut self) -> Result<()> {
        self.write_escape("?25l")
    }

    pub fn show_cursor(&mut self) -> Result<()> {
        self.write_escape("?25h")
    }

    pub fn clear_screen(&mut self) -> Result<()> {
        self.write_escape("2J")
    }

    pub fn clear_line(&mut self) -> Result<()> {
        self.write_escape("2K")
    }

    pub fn move_cursor(&mut self, row: u16, column: u16) -> Result<()> {
        write!(self.writer, "\x1b[{};{}H", row.max(1), column.max(1))?;
        Ok(())
    }

    pub fn write_text(&mut self, text: &str) -> Result<()> {
        self.writer.write_all(text.as_bytes())?;
        Ok(())
    }

    pub fn flush(&mut self) -> Result<()> {
        self.writer.flush()?;
        Ok(())
    }

    fn write_escape(&mut self, code: &str) -> Result<()> {
        write!(self.writer, "\x1b[{code}")?;
        Ok(())
    }
}

pub fn run_basic_editor(file: Option<&Path>) -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    if !stdin.is_terminal() || !stdout.is_terminal() {
        return Err(RileError::NotTerminal);
    }

    let document = match file {
        Some(path) => Document::open(path)?,
        None => Document::scratch(),
    };
    let editor = Editor::new(document);

    let mut session = TerminalSession::enter(stdin, stdout)?;
    session.draw(&editor)?;
    session.run(editor)
}

struct TerminalSession<R, W: Write> {
    screen: ScreenGuard<W>,
    _raw_mode: RawModeGuard,
    input: KeyReader<R>,
    output_fd: libc::c_int,
}

impl<R, W> TerminalSession<R, W>
where
    R: Read + AsRawFd,
    W: Write + AsRawFd,
{
    fn enter(input: R, output: W) -> Result<Self> {
        let input_fd = input.as_raw_fd();
        let output_fd = output.as_raw_fd();
        let raw_mode = RawModeGuard::activate(input_fd)?;
        let mut screen = ScreenGuard::enter(output)?;
        screen.terminal.hide_cursor()?;
        screen.terminal.clear_screen()?;
        screen.terminal.flush()?;

        Ok(Self {
            screen,
            _raw_mode: raw_mode,
            input: KeyReader::new(input),
            output_fd,
        })
    }

    fn run(&mut self, mut editor: Editor) -> Result<()> {
        loop {
            match editor.handle_key(self.input.read_key()?)? {
                EditorOutcome::Quit => return Ok(()),
                EditorOutcome::Continue => self.draw(&editor)?,
            }
        }
    }

    fn draw(&mut self, editor: &Editor) -> Result<()> {
        let size = terminal_size(self.output_fd)?;
        self.screen.terminal.move_cursor(1, 1)?;
        self.screen.terminal.clear_screen()?;

        let text_rows = size.rows.saturating_sub(2).max(1);
        for row in 0..text_rows {
            self.screen.terminal.move_cursor(row + 1, 1)?;
            self.screen.terminal.clear_line()?;
            if let Some(line) = editor.document().buffer().line(usize::from(row)) {
                self.screen.terminal.write_text(line)?;
            } else {
                self.screen.terminal.write_text("~")?;
            }
        }

        let status_row = size.rows.saturating_sub(1).max(1);
        self.screen.terminal.move_cursor(status_row, 1)?;
        self.screen.terminal.clear_line()?;
        self.screen.terminal.write_text(&format!(
            "{} | C-x C-s save | C-x C-c quit | M-x",
            editor.document().mode_line()
        ))?;

        self.screen.terminal.move_cursor(size.rows.max(1), 1)?;
        self.screen.terminal.clear_line()?;
        if let Some(message) = &editor.minibuffer().message {
            self.screen.terminal.write_text(message)?;
        }

        let cursor = editor.cursor();
        let cursor_row = (cursor.line + 1).min(usize::from(text_rows)) as u16;
        let cursor_column = editor.document().buffer().display_column(cursor)? + 1;
        self.screen
            .terminal
            .move_cursor(cursor_row.max(1), cursor_column as u16)?;
        self.screen.terminal.flush()
    }
}

struct ScreenGuard<W: Write> {
    terminal: AnsiTerminal<W>,
    active: bool,
}

impl<W: Write> ScreenGuard<W> {
    fn enter(writer: W) -> Result<Self> {
        let mut terminal = AnsiTerminal::new(writer);
        terminal.enter_alternate_screen()?;
        terminal.flush()?;
        Ok(Self {
            terminal,
            active: true,
        })
    }
}

impl<W: Write> Drop for ScreenGuard<W> {
    fn drop(&mut self) {
        if self.active {
            let _ = self.terminal.show_cursor();
            let _ = self.terminal.leave_alternate_screen();
            let _ = self.terminal.flush();
            self.active = false;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AnsiTerminal;

    #[test]
    fn writes_buffered_ansi_sequences() {
        let mut terminal = AnsiTerminal::new(Vec::new());
        terminal.hide_cursor().expect("hide cursor should write");
        terminal.move_cursor(2, 3).expect("move should write");
        terminal.clear_line().expect("clear line should write");
        terminal.write_text("status").expect("text should write");

        assert_eq!(
            terminal.into_inner(),
            b"\x1b[?25l\x1b[2;3H\x1b[2Kstatus".to_vec()
        );
    }
}
