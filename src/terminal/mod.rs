// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{self, IsTerminal, Read, Write};
use std::ops::Range;
use std::os::fd::AsRawFd;
use std::path::Path;

use crate::buffer::{Buffer, Position};
use crate::completion::CompletionStyle;
use crate::config::{Config, ThemeName};
use crate::editor::{Editor, EditorOutcome, ShellCancellationRequest};
use crate::file::Document;
use crate::input::{KeyEvent, KeyReader};
use crate::minibuffer::PromptKind;
use crate::render::{Face, Span, clip_spans, merge_spans};
use crate::shell::{ShellJob, ShellJobPoll};
use crate::text::control_character_escape;
use crate::window::{Viewport, WindowLayout, WindowSeparator};
use crate::{Result, RileError};
use unicode_width::UnicodeWidthChar;

const MAX_RENDER_SOURCE_CHARS_PER_COLUMN: usize = 8;
const MAX_RENDER_SOURCE_CHAR_SLACK: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminalSize {
    pub rows: u16,
    pub columns: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeOptions<'a> {
    pub file: Option<&'a Path>,
    pub visual_test: bool,
    pub test_size: Option<TerminalSize>,
}

pub fn escape_terminal_controls(text: &str) -> String {
    let mut escaped = String::new();
    for character in text.chars() {
        if let Some(replacement) = control_character_escape(character) {
            escaped.push_str(&replacement);
        } else {
            escaped.push(character);
        }
    }
    escaped
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct FrameOptions {
    visual_test: bool,
    clear_screen: bool,
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

    pub fn erase_byte(&self) -> u8 {
        self.original
            .as_ref()
            .map(|termios| termios.c_cc[libc::VERASE])
            .unwrap_or(0x7f)
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

    pub fn enable(&mut self) -> Result<()> {
        if self.active {
            return Ok(());
        }
        let Some(original) = self.original.as_ref() else {
            return Ok(());
        };
        let raw = raw_termios_from_original(original);
        // SAFETY: raw is derived from a valid termios value for this fd.
        if unsafe { libc::tcsetattr(self.fd, libc::TCSAFLUSH, &raw) } == -1 {
            return Err(io::Error::last_os_error().into());
        }
        self.active = true;
        Ok(())
    }
}

fn raw_termios_from_original(original: &libc::termios) -> libc::termios {
    let mut raw = *original;
    raw.c_iflag &= !(libc::BRKINT | libc::ICRNL | libc::INPCK | libc::ISTRIP | libc::IXON);
    raw.c_oflag &= !libc::OPOST;
    raw.c_cflag |= libc::CS8;
    raw.c_lflag &= !(libc::ECHO | libc::ICANON | libc::IEXTEN | libc::ISIG);
    raw.c_cc[libc::VMIN] = 0;
    raw.c_cc[libc::VTIME] = 1;
    raw
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

    pub fn set_steady_block_cursor(&mut self) -> Result<()> {
        self.write_escape("2 q")
    }

    pub fn reset_cursor_style(&mut self) -> Result<()> {
        self.write_escape("0 q")
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

    fn write_safe_text(&mut self, text: &str) -> Result<()> {
        if text.chars().any(char::is_control) {
            return Err(RileError::InvalidInput(
                "terminal display text contains a control character".to_owned(),
            ));
        }
        self.writer.write_all(text.as_bytes())?;
        Ok(())
    }

    fn write_control_sequence(&mut self, sequence: &str) -> Result<()> {
        self.writer.write_all(sequence.as_bytes())?;
        Ok(())
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<()> {
        self.writer.write_all(bytes)?;
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

pub fn run_basic_editor(options: RuntimeOptions<'_>) -> Result<()> {
    let stdin = io::stdin();
    let stdout = io::stdout();
    if !stdin.is_terminal() || !stdout.is_terminal() {
        return Err(RileError::NotTerminal);
    }

    let document = match options.file {
        Some(path) => Document::open(path)?,
        None => Document::welcome(),
    };
    let config = if options.visual_test {
        Config::default()
    } else {
        Config::load()?
    };
    let mut editor = Editor::with_config(document, config);

    let mut session = TerminalSession::enter(stdin, stdout, options)?;
    session.draw(&mut editor)?;
    session.run(editor)
}

struct TerminalSession<R, W: Write> {
    screen: ScreenGuard<W>,
    raw_mode: RawModeGuard,
    input: KeyReader<R>,
    output_fd: libc::c_int,
    test_size: Option<TerminalSize>,
    frame_options: FrameOptions,
    last_size: Option<TerminalSize>,
    shell_job: Option<ShellJob>,
    shell_input_suppression: bool,
    shell_c_g_escalation: bool,
}

impl<R, W> TerminalSession<R, W>
where
    R: Read + AsRawFd,
    W: Write + AsRawFd,
{
    fn enter(input: R, output: W, options: RuntimeOptions<'_>) -> Result<Self> {
        let input_fd = input.as_raw_fd();
        let output_fd = output.as_raw_fd();
        let raw_mode = RawModeGuard::activate(input_fd)?;
        let erase_byte = raw_mode.erase_byte();
        let mut screen = ScreenGuard::enter(output)?;
        if options.visual_test {
            screen.set_steady_block_cursor()?;
        }
        screen.terminal.clear_screen()?;
        screen.terminal.flush()?;

        Ok(Self {
            screen,
            raw_mode,
            input: KeyReader::with_erase_byte(input, erase_byte),
            output_fd,
            test_size: options.test_size,
            frame_options: FrameOptions {
                visual_test: options.visual_test,
                clear_screen: false,
            },
            last_size: None,
            shell_job: None,
            shell_input_suppression: false,
            shell_c_g_escalation: false,
        })
    }

    fn run(&mut self, mut editor: Editor) -> Result<()> {
        loop {
            let allow_completion = !self.shell_input_suppression;
            let shell_changed = self.poll_shell_job(&mut editor, allow_completion)?;
            if shell_changed {
                self.draw(&mut editor)?;
            }
            if let Some(key) = self.input.read_key_or_timeout()? {
                if self.consume_shell_input_before_editor(&mut editor, &key)? {
                    self.poll_shell_job(&mut editor, false)?;
                    self.draw(&mut editor)?;
                    continue;
                }

                let outcome = editor.handle_key(key)?;
                self.process_shell_requests(&mut editor)?;
                match outcome {
                    EditorOutcome::Quit => {
                        self.cancel_shell_job_immediately();
                        return Ok(());
                    }
                    EditorOutcome::Continue => {
                        self.poll_shell_job(&mut editor, false)?;
                        self.draw(&mut editor)?;
                    }
                    EditorOutcome::Suspend => {
                        self.poll_shell_job(&mut editor, false)?;
                        self.suspend()?;
                        self.draw(&mut editor)?;
                    }
                }
            } else {
                let shell_changed = self.finish_shell_input_quiet_period(&mut editor)?;
                let redraw = if self.shell_job.is_some() || editor.shell_command_running() {
                    true
                } else {
                    shell_changed || editor.poll_auto_revert()?
                };
                if redraw {
                    self.draw(&mut editor)?;
                }
            }
        }
    }

    fn consume_shell_input_before_editor(
        &mut self,
        editor: &mut Editor,
        key: &KeyEvent,
    ) -> Result<bool> {
        if self.try_escalate_shell_cancellation(editor, key)? {
            return Ok(true);
        }
        Ok(self.shell_input_suppression
            && !editor.shell_command_running()
            && editor.minibuffer().prompt().is_none())
    }

    fn try_escalate_shell_cancellation(
        &mut self,
        editor: &mut Editor,
        key: &KeyEvent,
    ) -> Result<bool> {
        if *key != KeyEvent::Ctrl('g') {
            self.shell_c_g_escalation = false;
            return Ok(false);
        }
        if !self.shell_c_g_escalation
            || editor.minibuffer().prompt().is_some()
            || !self.shell_job.as_ref().is_some_and(ShellJob::is_cancelling)
        {
            return Ok(false);
        }

        self.shell_c_g_escalation = false;
        self.shell_job
            .as_mut()
            .expect("cancellation escalation should retain the shell job")
            .request_cancel();
        editor.report_shell_cancellation_escalated();
        Ok(true)
    }

    fn finish_shell_input_quiet_period(&mut self, editor: &mut Editor) -> Result<bool> {
        let shell_changed = self.poll_shell_job(editor, true)?;
        if self.shell_input_suppression && !editor.shell_command_running() {
            self.shell_input_suppression = false;
        }
        Ok(shell_changed)
    }

    fn process_shell_requests(&mut self, editor: &mut Editor) -> Result<()> {
        if let Some(request) = editor.take_shell_cancel_request() {
            match request {
                ShellCancellationRequest::Interrupt => {
                    self.shell_c_g_escalation = if let Some(job) = self.shell_job.as_mut() {
                        job.request_cancel();
                        job.is_cancelling()
                    } else {
                        false
                    };
                }
                ShellCancellationRequest::Quit => {
                    self.shell_c_g_escalation = false;
                    if let Some(job) = self.shell_job.as_mut() {
                        job.request_cancel();
                    }
                }
            }
        }

        let Some(request) = editor.take_pending_shell_request() else {
            return Ok(());
        };
        if self.shell_job.is_some() {
            editor.reject_shell_command_start(
                "cannot start a shell command while the previous command is still cancelling",
            );
            return Ok(());
        }

        let spawned = if request.stream_output {
            ShellJob::spawn_streaming(&request.command, &request.stdin, &request.current_dir)
        } else {
            ShellJob::spawn(&request.command, &request.stdin, &request.current_dir)
        };
        match spawned {
            Ok(job) => {
                self.shell_c_g_escalation = false;
                self.shell_job = Some(job);
                self.shell_input_suppression = true;
            }
            Err(error) => editor.complete_shell_command(Err(error))?,
        }
        Ok(())
    }

    fn poll_shell_job(&mut self, editor: &mut Editor, allow_completion: bool) -> Result<bool> {
        let Some(job) = self.shell_job.as_mut() else {
            return Ok(false);
        };
        let (terminal, cancelled) = match job.poll() {
            ShellJobPoll::Pending => (false, false),
            ShellJobPoll::Finished(_) | ShellJobPoll::Failed(_) => (true, false),
            ShellJobPoll::Cancelled => (true, true),
        };
        let streamed_output = job.streams_output();
        let output_changed = editor.append_shell_command_output(&job.take_streamed_output())?;
        if !terminal || !allow_completion {
            return Ok(output_changed);
        }

        self.shell_c_g_escalation = false;
        self.shell_input_suppression = false;
        let job = self
            .shell_job
            .take()
            .expect("terminal shell poll should retain the job");
        if cancelled {
            editor.finish_shell_cancellation(streamed_output);
        } else {
            editor.complete_shell_command(job.into_result())?;
        }
        Ok(true)
    }

    fn cancel_shell_job_immediately(&mut self) {
        self.shell_c_g_escalation = false;
        if let Some(job) = self.shell_job.as_mut() {
            job.request_cancel();
            job.request_cancel();
        }
    }

    fn suspend(&mut self) -> Result<()> {
        self.screen.leave()?;
        self.raw_mode.disable()?;
        // SAFETY: raising SIGTSTP suspends the current process when job control is available.
        if unsafe { libc::raise(libc::SIGTSTP) } == -1 {
            return Err(io::Error::last_os_error().into());
        }
        self.raw_mode.enable()?;
        self.screen.enter_again()?;
        self.last_size = None;
        Ok(())
    }

    fn draw(&mut self, editor: &mut Editor) -> Result<()> {
        let size = match self.test_size {
            Some(size) => size,
            None => terminal_size(self.output_fd)?,
        };
        let mut frame_options = self.frame_options;
        frame_options.clear_screen = self.last_size.is_some_and(|last_size| last_size != size);
        self.last_size = Some(size);

        let mut frame_terminal = AnsiTerminal::new(Vec::new());
        draw_editor_frame_with_options(&mut frame_terminal, editor, size, frame_options)?;
        self.screen
            .terminal
            .write_bytes(&frame_terminal.into_inner())?;
        self.screen.terminal.flush()
    }
}

#[cfg(test)]
fn draw_editor_frame<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    editor: &mut Editor,
    size: TerminalSize,
) -> Result<()> {
    draw_editor_frame_with_options(terminal, editor, size, FrameOptions::default())
}

fn draw_editor_frame_with_options<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    editor: &mut Editor,
    size: TerminalSize,
    options: FrameOptions,
) -> Result<()> {
    editor.refresh_messages_buffer();
    terminal.hide_cursor()?;
    if options.clear_screen {
        terminal.move_cursor(1, 1)?;
        terminal.clear_screen()?;
    }

    let completion_rows = completion_popup_rows(editor, size.rows);
    let total_rows = usize::from(size.rows.max(1));
    let window_rows = total_rows.saturating_sub(1 + completion_rows).max(1);
    let minibuffer_row = (window_rows + 1).min(total_rows);
    let columns = usize::from(size.columns.max(1));
    let layouts = editor.window_layouts(window_rows, columns);
    let separators = editor.window_separators(window_rows, columns);
    for layout in &layouts {
        editor.set_window_text_rows(layout.id, layout.rect.rows.saturating_sub(1));
    }
    ensure_current_window_visible(editor, &layouts)?;
    for layout in &layouts {
        draw_window(terminal, editor, *layout, options)?;
    }
    draw_window_separators(terminal, editor, &separators)?;

    draw_minibuffer(terminal, editor, size, minibuffer_row)?;
    draw_completion_popup(terminal, editor, size, minibuffer_row + 1, completion_rows)?;

    if let Some(cursor_column) = minibuffer_cursor_column(editor, usize::from(size.columns.max(1)))
    {
        move_cursor_to_minibuffer(terminal, size, minibuffer_row, cursor_column)?;
    } else {
        move_cursor_to_current_window(terminal, editor, &layouts)?;
    }
    terminal.show_cursor()
}

fn draw_window_separators<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    editor: &Editor,
    separators: &[WindowSeparator],
) -> Result<()> {
    for separator in separators {
        for row in 0..separator.rect.rows {
            terminal.move_cursor(
                (separator.rect.row + row + 1) as u16,
                (separator.rect.column + 1) as u16,
            )?;
            write_text_with_face(terminal, "|", Face::ModeLine, editor.theme())?;
        }
    }
    Ok(())
}

fn draw_minibuffer<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    editor: &Editor,
    size: TerminalSize,
    row: usize,
) -> Result<()> {
    terminal.move_cursor(row as u16, 1)?;
    terminal.clear_line()?;
    let columns = usize::from(size.columns.max(1));
    if let Some(line) = minibuffer_visible_line(editor, columns) {
        if let Some(face) = line.face {
            write_fixed_width_text_with_face(terminal, &line.text, columns, face, editor.theme())?;
        } else if line.spans.is_empty() {
            write_fixed_width_text(terminal, &line.text, columns)?;
        } else {
            write_fixed_width_text_with_spans(
                terminal,
                &line.text,
                &line.spans,
                columns,
                editor.theme(),
            )?;
        }
    } else {
        write_fixed_width_text(terminal, "", columns)?;
    }
    Ok(())
}

fn completion_popup_rows(editor: &Editor, terminal_rows: u16) -> usize {
    let Some(completion) = editor.completion() else {
        return 0;
    };
    if completion.style() != CompletionStyle::Vertical {
        return 0;
    }
    let available = usize::from(terminal_rows.saturating_sub(2));
    if available == 0 {
        return 0;
    }
    if completion.has_matches() {
        completion.view_items().len().min(available)
    } else {
        1
    }
}

fn draw_completion_popup<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    editor: &Editor,
    size: TerminalSize,
    start_row: usize,
    rows: usize,
) -> Result<()> {
    if rows == 0 {
        return Ok(());
    }
    let columns = usize::from(size.columns.max(1));
    let Some(completion) = editor.completion() else {
        return Ok(());
    };
    let items = completion.view_items();
    if items.is_empty() {
        terminal.move_cursor(start_row as u16, 1)?;
        let message = if completion.is_partial() {
            "No match in scanned entries [partial]"
        } else {
            "No match"
        };
        write_fixed_width_text_with_face(
            terminal,
            message,
            columns,
            Face::Warning,
            editor.theme(),
        )?;
        return Ok(());
    }
    let visible_items = items.into_iter().take(rows).collect::<Vec<_>>();
    let candidate_width = completion_candidate_column_width(&visible_items, columns);
    for (index, item) in visible_items.into_iter().enumerate() {
        terminal.move_cursor((start_row + index) as u16, 1)?;
        let line = if completion.show_annotations() && !item.candidate.annotation.is_empty() {
            format_completion_row(
                &item.candidate.display_label(),
                &item.candidate.annotation,
                candidate_width,
                columns,
            )
        } else {
            item.candidate.value.clone()
        };
        let face = if item.selected {
            Face::ModeLine
        } else {
            Face::Default
        };
        write_fixed_width_text_with_face(terminal, &line, columns, face, editor.theme())?;
    }
    Ok(())
}

fn minibuffer_cursor_column(editor: &Editor, columns: usize) -> Option<usize> {
    minibuffer_visible_line(editor, columns).and_then(|line| line.cursor)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MinibufferVisibleLine {
    text: String,
    cursor: Option<usize>,
    spans: Vec<Span>,
    face: Option<Face>,
}

fn minibuffer_visible_line(editor: &Editor, columns: usize) -> Option<MinibufferVisibleLine> {
    let (text, spans, face) = minibuffer_line_text_and_spans(editor)?;
    let cursor = raw_minibuffer_cursor_column(editor);
    let Some(cursor) = cursor else {
        return Some(MinibufferVisibleLine {
            text,
            cursor: None,
            spans,
            face,
        });
    };
    if columns == 0 || cursor < columns {
        return Some(MinibufferVisibleLine {
            text,
            cursor: Some(cursor),
            spans,
            face,
        });
    }
    let start_column = cursor + 1 - columns;
    let (text, spans) = expand_display_text(&text, &spans, 4);
    let start_byte = byte_from_display_column(&text, start_column);
    let text = text[start_byte..].to_owned();
    let spans = spans
        .into_iter()
        .filter_map(|span| {
            let start = span.start_byte.max(start_byte);
            let end = span.end_byte.max(start).min(start_byte + text.len());
            (start < end).then(|| Span::new(start - start_byte, end - start_byte, span.face))
        })
        .collect();
    Some(MinibufferVisibleLine {
        text,
        cursor: Some(columns - 1),
        spans,
        face,
    })
}

fn minibuffer_line_text_and_spans(editor: &Editor) -> Option<(String, Vec<Span>, Option<Face>)> {
    let Some(prompt) = editor.minibuffer().prompt() else {
        let text = editor.minibuffer_display_text()?;
        let face = text.starts_with("Error:").then_some(Face::Error);
        return Some((text, Vec::new(), face));
    };

    let mut text = String::new();
    let prompt_face_start = text.len();
    if let Some(completion) = editor.completion()
        && completion.style() == CompletionStyle::Vertical
    {
        let selected = completion.selected_match_number().unwrap_or(0);
        let partial = if completion.is_partial() {
            " [partial]"
        } else {
            ""
        };
        text.push_str(&format!(
            "{selected}/{}{partial}  ",
            completion.match_count()
        ));
    }

    text.push_str(&prompt.label);
    let prompt_end = text.len();
    text.push_str(&prompt.input);

    if let Some(completion) = editor.completion()
        && completion.style() == CompletionStyle::Ido
        && prompt_supports_ido_candidates(prompt.kind)
    {
        let candidates = if completion.has_matches() {
            completion
                .view_items()
                .into_iter()
                .map(|item| item.candidate.value.as_str())
                .collect::<Vec<_>>()
                .join(" | ")
        } else if completion.is_partial() {
            "No match in scanned entries".to_owned()
        } else {
            "No match".to_owned()
        };
        if completion.is_partial() {
            text.push_str("  [partial]");
        }
        text.push_str("  [");
        text.push_str(&candidates);
        text.push(']');
    }

    let spans = (prompt_face_start < prompt_end)
        .then(|| Span::new(prompt_face_start, prompt_end, Face::Minibuffer))
        .into_iter()
        .collect();
    Some((text, spans, None))
}

fn prompt_supports_ido_candidates(kind: PromptKind) -> bool {
    matches!(
        kind,
        PromptKind::DescribeFunction
            | PromptKind::DescribeVariable
            | PromptKind::ExtendedCommand
            | PromptKind::FindFile
            | PromptKind::FindFileReadOnly
            | PromptKind::InsertFile
            | PromptKind::KillBuffer
            | PromptKind::SwitchToBuffer
    )
}

fn raw_minibuffer_cursor_column(editor: &Editor) -> Option<usize> {
    let prompt = editor.minibuffer().prompt()?;
    let input_before_cursor = editor.minibuffer().prompt_input_before_cursor()?;
    let mut text = String::new();
    if let Some(completion) = editor.completion()
        && completion.style() == CompletionStyle::Vertical
    {
        let selected = completion.selected_match_number().unwrap_or(0);
        let partial = if completion.is_partial() {
            " [partial]"
        } else {
            ""
        };
        text.push_str(&format!(
            "{selected}/{}{partial}  ",
            completion.match_count()
        ));
    }
    text.push_str(&prompt.label);
    text.push_str(input_before_cursor);
    Some(text_display_width(&text))
}

#[cfg(test)]
fn text_from_display_column(text: &str, start_column: usize) -> String {
    text[byte_from_display_column(text, start_column)..].to_owned()
}

fn byte_from_display_column(text: &str, start_column: usize) -> usize {
    byte_from_display_column_with_tabs(text, start_column, 4)
}

fn byte_from_display_column_with_tabs(text: &str, start_column: usize, tab_width: usize) -> usize {
    let mut column = 0;
    let mut start_byte = text.len();
    for (byte, character) in text.char_indices() {
        let next_column = column + display_character_width(character, tab_width, column);
        if next_column > start_column {
            start_byte = byte;
            break;
        }
        column = next_column;
    }
    start_byte
}

fn move_cursor_to_minibuffer<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    size: TerminalSize,
    row: usize,
    column: usize,
) -> Result<()> {
    let columns = usize::from(size.columns.max(1));
    terminal.move_cursor(row as u16, (column.min(columns - 1) + 1) as u16)
}

fn text_display_width(text: &str) -> usize {
    display_width_with_tabs(text, 4)
}

fn completion_candidate_column_width(
    items: &[crate::completion::CompletionViewItem<'_>],
    columns: usize,
) -> usize {
    if columns == 0 {
        return 0;
    }
    let visible_max = items
        .iter()
        .map(|item| text_display_width(&item.candidate.display_label()))
        .max()
        .unwrap_or(1)
        .max(1);
    let cap = if columns > 40 {
        columns.saturating_sub(20).min(columns / 2)
    } else {
        columns.saturating_sub(4).max(1)
    };
    visible_max.min(cap.max(1))
}

fn format_completion_row(
    candidate: &str,
    annotation: &str,
    candidate_width: usize,
    columns: usize,
) -> String {
    if columns == 0 {
        return String::new();
    }
    let description_gap = 8;
    let candidate_limit = candidate_width.min(columns.saturating_sub(description_gap + 1));
    let candidate = clipped_text(candidate, candidate_limit);
    let padding = candidate_width.saturating_sub(text_display_width(&candidate)) + description_gap;
    let mut line = candidate;
    line.push_str(&" ".repeat(padding));
    if text_display_width(&line) < columns {
        line.push_str(annotation);
    }
    clipped_text(&line, columns)
}

fn ensure_current_window_visible(editor: &mut Editor, layouts: &[WindowLayout]) -> Result<()> {
    let Some(layout) = layouts
        .iter()
        .find(|layout| layout.id == editor.current_window_id())
    else {
        return Ok(());
    };
    let Some(viewport) = editor.window_viewport(layout.id) else {
        return Ok(());
    };
    let Some(document) = editor.document_for_buffer(viewport.buffer) else {
        return Ok(());
    };

    let gutter_width = line_number_gutter_width(editor, document.buffer());
    let text_rows = layout.rect.rows.saturating_sub(1);
    let text_columns = layout.rect.columns.saturating_sub(gutter_width);
    let cursor_display_column = if document.is_help() {
        0
    } else {
        cursor_absolute_display_column(document.buffer(), editor.cursor(), editor.tab_width())?
    };
    editor.ensure_current_window_contains_cursor(text_rows, text_columns, cursor_display_column);
    Ok(())
}

fn draw_window<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    editor: &Editor,
    layout: WindowLayout,
    options: FrameOptions,
) -> Result<()> {
    if layout.rect.rows == 0 || layout.rect.columns == 0 {
        return Ok(());
    }
    let Some(viewport) = editor.window_viewport(layout.id) else {
        return Ok(());
    };
    let Some(document) = editor.document_for_buffer(viewport.buffer) else {
        return Ok(());
    };

    let text_rows = layout.rect.rows.saturating_sub(1);
    if document.is_help() {
        draw_wrapped_help_rows(
            terminal,
            editor,
            document.buffer(),
            viewport,
            layout,
            text_rows,
        )?;
    } else {
        draw_buffer_rows(
            terminal,
            editor,
            document.buffer(),
            viewport,
            layout,
            text_rows,
        )?;
    }

    let mode_line_row = layout.rect.row + layout.rect.rows;
    terminal.move_cursor(mode_line_row as u16, (layout.rect.column + 1) as u16)?;
    let mode_line = if options.visual_test {
        visual_test_mode_line(editor, document, viewport, layout, text_rows)?
    } else {
        format_mode_line(editor, document, viewport, layout, text_rows)?
    };
    write_fixed_width_text_with_face(
        terminal,
        &mode_line,
        layout.rect.columns,
        Face::ModeLine,
        editor.theme(),
    )
}

fn draw_buffer_rows<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    editor: &Editor,
    buffer: &Buffer,
    viewport: &Viewport,
    layout: WindowLayout,
    text_rows: usize,
) -> Result<()> {
    for row in 0..text_rows {
        let screen_row = layout.rect.row + row + 1;
        let screen_column = layout.rect.column + 1;
        terminal.move_cursor(screen_row as u16, screen_column as u16)?;
        let line_index = viewport.first_visible_line + row;
        let gutter_width = line_number_gutter_width(editor, buffer);
        if gutter_width > 0 {
            write_line_number_gutter(terminal, line_index, gutter_width, editor.theme())?;
        }
        let text_columns = layout.rect.columns.saturating_sub(gutter_width);
        if text_columns == 0 {
            continue;
        }
        if let Some(line) = buffer.line(line_index) {
            let spans = editor.spans_for_buffer_line(viewport.buffer, line_index, line);
            write_buffer_line(
                terminal,
                buffer,
                viewport,
                line_index,
                line,
                &spans,
                LineRenderOptions {
                    width: text_columns,
                    tab_width: editor.tab_width(),
                    theme: editor.theme(),
                    highlight_line_end_space: editor
                        .region_highlights_line_end_space(viewport.buffer, line_index),
                },
            )?;
        } else {
            write_fixed_width_text(terminal, "", text_columns)?;
        }
    }
    Ok(())
}

fn draw_wrapped_help_rows<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    editor: &Editor,
    buffer: &Buffer,
    viewport: &Viewport,
    layout: WindowLayout,
    text_rows: usize,
) -> Result<()> {
    let gutter_width = line_number_gutter_width(editor, buffer);
    let text_columns = layout.rect.columns.saturating_sub(gutter_width);
    let visual_lines = wrapped_help_visual_lines(
        buffer,
        viewport.first_visible_line,
        text_rows,
        text_columns,
        editor.tab_width(),
    );

    for row in 0..text_rows {
        let screen_row = layout.rect.row + row + 1;
        let screen_column = layout.rect.column + 1;
        terminal.move_cursor(screen_row as u16, screen_column as u16)?;
        let line_index = visual_lines
            .get(row)
            .and_then(|line| line.as_ref())
            .map(|line| line.line_index)
            .unwrap_or(viewport.first_visible_line + row);
        if gutter_width > 0 {
            write_line_number_gutter(terminal, line_index, gutter_width, editor.theme())?;
        }
        if text_columns == 0 {
            continue;
        }
        match visual_lines.get(row).and_then(|line| line.as_ref()) {
            Some(visual_line) => {
                let spans = buffer
                    .line(visual_line.line_index)
                    .map(|line| {
                        editor.spans_for_buffer_line(viewport.buffer, visual_line.line_index, line)
                    })
                    .unwrap_or_default();
                write_wrapped_help_line(
                    terminal,
                    buffer,
                    visual_line,
                    &spans,
                    LineRenderOptions {
                        width: text_columns,
                        tab_width: editor.tab_width(),
                        theme: editor.theme(),
                        highlight_line_end_space: editor
                            .region_highlights_line_end_space(viewport.buffer, line_index),
                    },
                )?;
            }
            None => write_fixed_width_text(terminal, "", text_columns)?,
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HelpVisualLine {
    line_index: usize,
    start_column: usize,
    continued: bool,
}

fn wrapped_help_visual_lines(
    buffer: &Buffer,
    first_line: usize,
    row_count: usize,
    width: usize,
    tab_width: usize,
) -> Vec<Option<HelpVisualLine>> {
    let mut rows = Vec::with_capacity(row_count);
    if width == 0 {
        rows.resize(row_count, None);
        return rows;
    }

    for line_index in first_line..buffer.line_count() {
        let Some(line) = buffer.line(line_index) else {
            break;
        };
        let line_width = display_width_with_tabs(line, tab_width);
        if line_width == 0 {
            rows.push(Some(HelpVisualLine {
                line_index,
                start_column: 0,
                continued: false,
            }));
            if rows.len() == row_count {
                return rows;
            }
            continue;
        }

        let mut start_column = 0;
        while start_column < line_width {
            let remaining_width = line_width - start_column;
            let continued = remaining_width > width;
            rows.push(Some(HelpVisualLine {
                line_index,
                start_column,
                continued,
            }));
            if rows.len() == row_count {
                return rows;
            }

            let content_width = wrapped_help_content_width(width, continued);
            start_column += content_width.max(1);
        }
    }

    rows.resize(row_count, None);
    rows
}

fn write_wrapped_help_line<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    buffer: &Buffer,
    visual_line: &HelpVisualLine,
    spans: &[Span],
    options: LineRenderOptions,
) -> Result<()> {
    let Some(line) = buffer.line(visual_line.line_index) else {
        return write_fixed_width_text(terminal, "", options.width);
    };
    let (line, spans) = expand_display_text(line, spans, options.tab_width);
    let content_width = wrapped_help_content_width(options.width, visual_line.continued);
    let range = visible_display_range(
        &line,
        visual_line.start_column,
        content_width,
        options.tab_width,
    );
    let relative_spans = clip_spans(&spans, range.clone());
    let segment = &line[range];
    let mut used_width = write_expanded_line_with_spans(
        terminal,
        segment,
        &relative_spans,
        options.tab_width,
        options.theme,
    )?;
    if visual_line.continued && options.width > 1 {
        let continuation_column = options.width.saturating_sub(1);
        if used_width < continuation_column {
            terminal.write_safe_text(&" ".repeat(continuation_column - used_width))?;
        }
        terminal.write_safe_text("\\")?;
        used_width = options.width;
    }
    if used_width < options.width {
        let padding = " ".repeat(options.width - used_width);
        if options.highlight_line_end_space {
            write_text_with_face_expanded(
                terminal,
                &padding,
                Face::Region,
                options.tab_width,
                used_width,
                options.theme,
            )?;
        } else {
            terminal.write_safe_text(&padding)?;
        }
    }
    Ok(())
}

fn wrapped_help_content_width(width: usize, continued: bool) -> usize {
    if continued && width > 1 {
        width - 1
    } else {
        width
    }
}

fn visual_test_mode_line(
    editor: &Editor,
    document: &Document,
    viewport: &Viewport,
    layout: WindowLayout,
    text_rows: usize,
) -> Result<String> {
    let cursor = viewport.cursor;
    document.buffer().validate_position(cursor)?;
    let active = if layout.id == editor.current_window_id() {
        "ACTIVE"
    } else {
        "inactive"
    };
    let name = editor
        .buffer_name_for(viewport.buffer)
        .expect("viewport buffer must exist");
    let column = cursor_absolute_display_column(document.buffer(), cursor, editor.tab_width())?;
    let position = mode_line_position(document.buffer(), viewport, text_rows, editor.tab_width())?;
    Ok(format!(
        "-- Rile VISUAL window {} {active} {name} Ln {:03} Col {:03} ro:{} modified:{} {position} --",
        layout.id.0,
        cursor.line + 1,
        column,
        document.is_read_only(),
        document.is_dirty()
    ))
}

fn format_mode_line(
    editor: &Editor,
    document: &Document,
    viewport: &Viewport,
    layout: WindowLayout,
    text_rows: usize,
) -> Result<String> {
    let active = if layout.id == editor.current_window_id() {
        "="
    } else {
        "-"
    };
    let modified = if document.is_dirty() { "**" } else { "--" };
    let read_only = if document.is_read_only() { "%" } else { "-" };
    let final_newline = if document.buffer().final_newline() {
        "F"
    } else {
        "N"
    };
    let new_file = if document.missing_on_open() { "N" } else { "-" };
    let major_mode = editor.major_mode_for_buffer(viewport.buffer).name();
    let position = mode_line_position(document.buffer(), viewport, text_rows, editor.tab_width())?;
    Ok(format!(
        "{active}-:{modified}{read_only}{final_newline}{new_file} {}   {position}   ({major_mode})",
        editor
            .buffer_name_for(viewport.buffer)
            .expect("viewport buffer must exist")
    ))
}

fn mode_line_position(
    buffer: &Buffer,
    viewport: &Viewport,
    text_rows: usize,
    tab_width: usize,
) -> Result<String> {
    let cursor = viewport.cursor;
    buffer.validate_position(cursor)?;
    let line_count = buffer.line_count();
    let visible_end = viewport.first_visible_line.saturating_add(text_rows);
    let location = if viewport.first_visible_line == 0 && visible_end >= line_count {
        "All".to_owned()
    } else if viewport.first_visible_line == 0 {
        "Top".to_owned()
    } else if visible_end >= line_count {
        "Bot".to_owned()
    } else {
        format!("{}%", ((cursor.line + 1) * 100 / line_count).clamp(1, 99))
    };
    let column = cursor_absolute_display_column(buffer, cursor, tab_width)?;
    Ok(format!("{location} ({},{column})", cursor.line + 1))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LineRenderOptions {
    width: usize,
    tab_width: usize,
    theme: ThemeName,
    highlight_line_end_space: bool,
}

#[derive(Debug, PartialEq, Eq)]
struct LineProjection {
    text: String,
    spans: Vec<Span>,
    left_hidden: bool,
    right_hidden: bool,
    #[cfg(test)]
    source_chars_scanned: usize,
}

fn write_buffer_line<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    _buffer: &Buffer,
    viewport: &Viewport,
    _line_index: usize,
    line: &str,
    spans: &[Span],
    options: LineRenderOptions,
) -> Result<()> {
    let projection = project_buffer_line(
        line,
        spans,
        viewport.first_visible_column,
        options.width,
        options.tab_width,
    );
    let (segment, relative_spans) = if projection.left_hidden || projection.right_hidden {
        mark_hidden_line_edges(
            &projection.text,
            &projection.spans,
            projection.left_hidden,
            projection.right_hidden,
        )
    } else {
        (projection.text, projection.spans)
    };
    let used_width = write_expanded_line_with_spans(
        terminal,
        &segment,
        &relative_spans,
        options.tab_width,
        options.theme,
    )?;
    if used_width < options.width {
        let padding = " ".repeat(options.width - used_width);
        if options.highlight_line_end_space {
            write_text_with_face_expanded(
                terminal,
                &padding,
                Face::Region,
                options.tab_width,
                used_width,
                options.theme,
            )?;
        } else {
            terminal.write_safe_text(&padding)?;
        }
    }
    Ok(())
}

fn project_buffer_line(
    line: &str,
    spans: &[Span],
    start_column: usize,
    width: usize,
    tab_width: usize,
) -> LineProjection {
    let end_column = start_column.saturating_add(width);
    let source_char_budget = end_column
        .saturating_mul(MAX_RENDER_SOURCE_CHARS_PER_COLUMN)
        .saturating_add(MAX_RENDER_SOURCE_CHAR_SLACK);
    let mut characters = line.char_indices().peekable();
    let mut text = String::new();
    let mut byte_map = vec![(0, 0)];
    let mut source_column = 0;
    let mut source_chars_scanned = 0;
    let mut visible_source_start = None;
    let mut visible_source_end = 0;
    let mut right_hidden = false;

    loop {
        if source_chars_scanned >= source_char_budget {
            // Conservatively mark uninspected source as hidden rather than let
            // an unbounded zero-width run monopolize every redraw.
            right_hidden = characters.peek().is_some();
            break;
        }
        let Some((source_start, character)) = characters.next() else {
            break;
        };
        source_chars_scanned += 1;
        let source_end = source_start + character.len_utf8();
        let target_start = text.len();
        source_column = append_projected_character(
            &mut text,
            character,
            source_column,
            start_column,
            end_column,
            tab_width,
        );
        let target_end = text.len();
        byte_map.push((source_start, target_start));
        byte_map.push((source_end, target_end));
        if target_start < target_end {
            visible_source_start.get_or_insert(source_start);
            visible_source_end = source_end;
        }
        if source_column > end_column {
            right_hidden = true;
            break;
        }
    }

    let spans = visible_source_start
        .map(|start| project_spans(spans, &byte_map, start..visible_source_end))
        .unwrap_or_default();
    LineProjection {
        text,
        spans,
        left_hidden: start_column > 0,
        right_hidden,
        #[cfg(test)]
        source_chars_scanned,
    }
}

fn append_projected_character(
    projected: &mut String,
    character: char,
    column: usize,
    start_column: usize,
    end_column: usize,
    tab_width: usize,
) -> usize {
    if character == '\t' {
        let spaces = tab_spaces(tab_width, column);
        return append_projected_text(
            projected,
            &" ".repeat(spaces),
            column,
            start_column,
            end_column,
        );
    }
    if let Some(replacement) = control_character_escape(character) {
        return append_projected_text(projected, &replacement, column, start_column, end_column);
    }

    let next_column = column.saturating_add(character.width().unwrap_or(0));
    if next_column > start_column && next_column <= end_column {
        projected.push(character);
    }
    next_column
}

fn append_projected_text(
    projected: &mut String,
    text: &str,
    mut column: usize,
    start_column: usize,
    end_column: usize,
) -> usize {
    for character in text.chars() {
        let next_column = column.saturating_add(character.width().unwrap_or(0));
        if next_column > start_column && next_column <= end_column {
            projected.push(character);
        }
        column = next_column;
    }
    column
}

fn project_spans(spans: &[Span], byte_map: &[(usize, usize)], range: Range<usize>) -> Vec<Span> {
    spans
        .iter()
        .filter_map(|span| {
            let source_start = span.start_byte.max(range.start);
            let source_end = span.end_byte.min(range.end);
            if source_start >= source_end {
                return None;
            }
            let start = mapped_byte(byte_map, source_start)?;
            let end = mapped_byte(byte_map, source_end)?;
            (start < end).then(|| Span::new(start, end, span.face))
        })
        .collect()
}

fn mark_hidden_line_edges(
    segment: &str,
    spans: &[Span],
    left_hidden: bool,
    right_hidden: bool,
) -> (String, Vec<Span>) {
    let characters = segment.char_indices().collect::<Vec<_>>();
    if characters.is_empty() {
        return ("$".to_owned(), Vec::new());
    }

    let first_visible = characters
        .iter()
        .position(|(_, character)| character.width().unwrap_or(0) > 0);
    let last_visible = characters
        .iter()
        .rposition(|(_, character)| character.width().unwrap_or(0) > 0);
    if first_visible.is_none() {
        return ("$".to_owned(), Vec::new());
    }
    let first_visible = first_visible.expect("positive-width character should exist");
    let last_visible = last_visible.expect("positive-width character should exist");
    let retained_start = if left_hidden { first_visible } else { 0 };
    let retained_end = if right_hidden {
        last_visible
    } else {
        characters.len() - 1
    };

    let mut marked = String::new();
    let mut byte_map = vec![(0, 0)];
    for (index, (byte, character)) in characters.iter().copied().enumerate() {
        let replacement =
            (left_hidden && index == first_visible) || (right_hidden && index == last_visible);
        let source_end = byte + character.len_utf8();
        let target_start = marked.len();
        if index >= retained_start && index <= retained_end {
            marked.push(if replacement { '$' } else { character });
        }
        let target_end = marked.len();
        byte_map.push((byte, target_start));
        byte_map.push((source_end, target_end));
    }

    let spans = spans
        .iter()
        .filter_map(|span| {
            let start = mapped_byte(&byte_map, span.start_byte)?;
            let end = mapped_byte(&byte_map, span.end_byte)?;
            (start < end).then(|| Span::new(start, end, span.face))
        })
        .collect();
    (marked, spans)
}

fn mapped_byte(byte_map: &[(usize, usize)], source: usize) -> Option<usize> {
    byte_map
        .binary_search_by_key(&source, |(from, _)| *from)
        .ok()
        .map(|index| byte_map[index].1)
}

fn write_fixed_width_text<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    text: &str,
    width: usize,
) -> Result<()> {
    let (text, _) = expand_display_text(text, &[], 4);
    let clipped = text_clipped_to_display_width(&text, width, 4);
    let used = write_display_text(terminal, &clipped, 4, 0)?;
    if used < width {
        terminal.write_safe_text(&" ".repeat(width - used))?;
    }
    Ok(())
}

fn write_fixed_width_text_with_face<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    text: &str,
    width: usize,
    face: Face,
    theme: ThemeName,
) -> Result<()> {
    let (text, _) = expand_display_text(text, &[], 4);
    let clipped = text_clipped_to_display_width(&text, width, 4);
    let used = write_text_with_face_expanded(terminal, &clipped, face, 4, 0, theme)?;
    if used < width {
        terminal.write_safe_text(&" ".repeat(width - used))?;
    }
    Ok(())
}

fn write_fixed_width_text_with_spans<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    text: &str,
    spans: &[Span],
    width: usize,
    theme: ThemeName,
) -> Result<()> {
    let (text, spans) = expand_display_text(text, spans, 4);
    let clipped = text_clipped_to_display_width(&text, width, 4);
    let clipped_spans = clip_spans(&spans, 0..clipped.len());
    let used = write_expanded_line_with_spans(terminal, &clipped, &clipped_spans, 4, theme)?;
    if used < width {
        terminal.write_safe_text(&" ".repeat(width - used))?;
    }
    Ok(())
}

fn text_clipped_to_display_width(text: &str, width: usize, tab_width: usize) -> String {
    let mut clipped = String::new();
    let mut used = 0;
    for character in text.chars() {
        let character_width = display_character_width(character, tab_width, used);
        if used + character_width > width {
            break;
        }
        clipped.push(character);
        used += character_width;
    }
    clipped
}

fn clipped_text(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if text_display_width(text) <= width {
        return text.to_owned();
    }
    let end = byte_from_display_column(text, width - 1);
    let prefix = &text[..end];
    let padding = width - 1 - text_display_width(prefix);
    format!("{prefix}{}$", " ".repeat(padding))
}

fn write_text_with_face<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    text: &str,
    face: Face,
    theme: ThemeName,
) -> Result<()> {
    write_text_with_face_expanded(terminal, text, face, 4, 0, theme).map(|_| ())
}

fn move_cursor_to_current_window<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    editor: &Editor,
    layouts: &[WindowLayout],
) -> Result<()> {
    let Some(layout) = layouts
        .iter()
        .find(|layout| layout.id == editor.current_window_id())
    else {
        return Ok(());
    };
    let Some(viewport) = editor.window_viewport(layout.id) else {
        return Ok(());
    };
    let Some(document) = editor.document_for_buffer(viewport.buffer) else {
        return Ok(());
    };
    let cursor = viewport.cursor;
    let text_rows = layout.rect.rows.saturating_sub(1).max(1);
    let gutter_width = line_number_gutter_width(editor, document.buffer());
    let text_columns = layout.rect.columns.saturating_sub(gutter_width);
    let (cursor_row, text_cursor_column) = if document.is_help() {
        wrapped_help_cursor_position(
            document.buffer(),
            viewport,
            cursor,
            text_rows,
            text_columns,
            editor.tab_width(),
        )?
    } else {
        (
            cursor
                .line
                .saturating_sub(viewport.first_visible_line)
                .min(text_rows - 1),
            cursor_display_column(document.buffer(), viewport, cursor, editor.tab_width())?,
        )
    };
    let cursor_column =
        (gutter_width + text_cursor_column).min(layout.rect.columns.saturating_sub(1));
    terminal.move_cursor(
        (layout.rect.row + cursor_row + 1) as u16,
        (layout.rect.column + cursor_column + 1) as u16,
    )
}

fn wrapped_help_cursor_position(
    buffer: &Buffer,
    viewport: &Viewport,
    cursor: Position,
    text_rows: usize,
    width: usize,
    tab_width: usize,
) -> Result<(usize, usize)> {
    let cursor_column = cursor_absolute_display_column(buffer, cursor, tab_width)?;
    let rows = wrapped_help_visual_lines(
        buffer,
        viewport.first_visible_line,
        text_rows,
        width,
        tab_width,
    );
    for (row, visual_line) in rows.iter().enumerate() {
        let Some(visual_line) = visual_line else {
            continue;
        };
        if visual_line.line_index != cursor.line {
            continue;
        }
        let content_width = wrapped_help_content_width(width, visual_line.continued).max(1);
        let segment_end = visual_line.start_column + content_width;
        let cursor_on_segment = if visual_line.continued {
            cursor_column < segment_end
        } else {
            cursor_column <= segment_end
        };
        if cursor_on_segment {
            return Ok((
                row,
                cursor_column
                    .saturating_sub(visual_line.start_column)
                    .min(width.saturating_sub(1)),
            ));
        }
    }

    Ok((
        cursor
            .line
            .saturating_sub(viewport.first_visible_line)
            .min(text_rows - 1),
        0,
    ))
}

fn cursor_display_column(
    buffer: &Buffer,
    viewport: &Viewport,
    cursor: Position,
    tab_width: usize,
) -> Result<usize> {
    Ok(cursor_absolute_display_column(buffer, cursor, tab_width)?
        .saturating_sub(viewport.first_visible_column))
}

fn cursor_absolute_display_column(
    buffer: &Buffer,
    cursor: Position,
    tab_width: usize,
) -> Result<usize> {
    buffer.validate_position(cursor)?;
    let line = buffer.line(cursor.line).expect("cursor line is valid");
    Ok(display_width_with_tabs(&line[..cursor.byte], tab_width))
}

#[cfg(test)]
fn write_line_with_spans<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    line: &str,
    spans: &[Span],
    tab_width: usize,
    theme: ThemeName,
) -> Result<usize> {
    let (line, spans) = expand_display_text(line, spans, tab_width);
    write_expanded_line_with_spans(terminal, &line, &spans, tab_width, theme)
}

fn write_expanded_line_with_spans<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    line: &str,
    spans: &[Span],
    tab_width: usize,
    theme: ThemeName,
) -> Result<usize> {
    let merged_spans = merge_spans(line, spans.iter().copied());
    let mut cursor = 0;
    let mut column = 0;
    for span in &merged_spans {
        if span.start_byte >= span.end_byte
            || span.end_byte > line.len()
            || !line.is_char_boundary(span.start_byte)
            || !line.is_char_boundary(span.end_byte)
            || span.start_byte < cursor
        {
            continue;
        }

        column = write_display_text(terminal, &line[cursor..span.start_byte], tab_width, column)?;
        column = write_text_with_face_expanded(
            terminal,
            &line[span.start_byte..span.end_byte],
            span.face,
            tab_width,
            column,
            theme,
        )?;
        cursor = span.end_byte;
    }
    write_display_text(terminal, &line[cursor..], tab_width, column)
}

fn expand_display_text(text: &str, spans: &[Span], tab_width: usize) -> (String, Vec<Span>) {
    let mut expanded = String::new();
    let mut byte_map = vec![(0, 0)];
    let mut column = 0;
    for (source_start, character) in text.char_indices() {
        let source_end = source_start + character.len_utf8();
        let target_start = expanded.len();
        if character == '\t' {
            let spaces = tab_spaces(tab_width, column);
            expanded.push_str(&" ".repeat(spaces));
            column += spaces;
        } else if let Some(replacement) = control_character_escape(character) {
            expanded.push_str(&replacement);
            column += replacement.len();
        } else {
            expanded.push(character);
            column += character.width().unwrap_or(0);
        }
        let target_end = expanded.len();
        byte_map.push((source_start, target_start));
        byte_map.push((source_end, target_end));
    }

    let spans = spans
        .iter()
        .filter_map(|span| {
            let start = mapped_byte(&byte_map, span.start_byte)?;
            let end = mapped_byte(&byte_map, span.end_byte)?;
            (start < end).then(|| Span::new(start, end, span.face))
        })
        .collect();
    (expanded, spans)
}

fn write_text_with_face_expanded<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    text: &str,
    face: Face,
    tab_width: usize,
    column: usize,
    theme: ThemeName,
) -> Result<usize> {
    if let Some(start_code) = face_start_code(face, theme) {
        terminal.write_control_sequence(start_code)?;
        let column = write_display_text(terminal, text, tab_width, column)?;
        terminal.write_control_sequence("\x1b[0m")?;
        Ok(column)
    } else {
        write_display_text(terminal, text, tab_width, column)
    }
}

fn write_display_text<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    text: &str,
    tab_width: usize,
    mut column: usize,
) -> Result<usize> {
    for character in text.chars() {
        if character == '\t' {
            let spaces = tab_spaces(tab_width, column);
            terminal.write_safe_text(&" ".repeat(spaces))?;
            column += spaces;
        } else if let Some(replacement) = control_character_escape(character) {
            terminal.write_safe_text(&replacement)?;
            column += replacement.len();
        } else {
            terminal.write_safe_text(&character.to_string())?;
            column += character.width().unwrap_or(0);
        }
    }
    Ok(column)
}

fn display_width_with_tabs(text: &str, tab_width: usize) -> usize {
    text.chars().fold(0, |column, character| {
        column + display_character_width(character, tab_width, column)
    })
}

fn display_character_width(character: char, tab_width: usize, column: usize) -> usize {
    if character == '\t' {
        tab_spaces(tab_width, column)
    } else if let Some(replacement) = control_character_escape(character) {
        replacement.len()
    } else {
        character.width().unwrap_or(0)
    }
}

fn visible_display_range(
    text: &str,
    start_column: usize,
    width: usize,
    tab_width: usize,
) -> Range<usize> {
    let start = byte_from_display_column_with_tabs(text, start_column, tab_width);
    let end =
        byte_from_display_column_with_tabs(text, start_column.saturating_add(width), tab_width);
    start..end
}

fn tab_spaces(tab_width: usize, column: usize) -> usize {
    let tab_width = tab_width.max(1);
    tab_width - (column % tab_width)
}

fn line_number_gutter_width(editor: &Editor, buffer: &Buffer) -> usize {
    if editor.line_numbers() {
        decimal_digits(buffer.line_count()) + 1
    } else {
        0
    }
}

fn write_line_number_gutter<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    line_index: usize,
    width: usize,
    theme: ThemeName,
) -> Result<()> {
    let gutter = format!("{:>width$} ", line_index + 1, width = width - 1);
    write_text_with_face(terminal, &gutter, Face::LineNumber, theme)
}

fn decimal_digits(value: usize) -> usize {
    value.max(1).ilog10() as usize + 1
}

fn face_start_code(face: Face, theme: ThemeName) -> Option<&'static str> {
    match theme {
        ThemeName::Default => match face {
            Face::CurrentSearchMatch => Some("\x1b[7m"),
            Face::SearchMatch => Some("\x1b[4m"),
            Face::Region => Some("\x1b[44m"),
            Face::Minibuffer => Some("\x1b[36m"),
            Face::ModeLine => Some("\x1b[7m"),
            Face::Error => Some("\x1b[31m"),
            Face::Warning => Some("\x1b[33m"),
            Face::LineNumber => Some("\x1b[2m"),
            Face::UserHighlight => Some("\x1b[43;30m"),
            Face::UserHighlightAlt => Some("\x1b[45;37m"),
            Face::UserHighlightLine => Some("\x1b[43;30m"),
            Face::UserHighlightGreen => Some("\x1b[42;30m"),
            Face::UserHighlightBlue => Some("\x1b[44;37m"),
            Face::UserHighlightSalmon => Some("\x1b[41;37m"),
            Face::UserHighlightAquamarine => Some("\x1b[46;30m"),
            Face::UserHighlightBlackBold => Some("\x1b[40;37;1m"),
            Face::UserHighlightBlueBold => Some("\x1b[44;37;1m"),
            Face::UserHighlightRedBold => Some("\x1b[41;37;1m"),
            Face::UserHighlightGreenBold => Some("\x1b[42;30;1m"),
            Face::UserHighlightBlackHeavyBold => Some("\x1b[40;97;1m"),
            Face::SyntaxKeyword => Some("\x1b[34;1m"),
            Face::SyntaxString => Some("\x1b[32m"),
            Face::SyntaxComment => Some("\x1b[2m"),
            _ => None,
        },
        ThemeName::Mono => match face {
            Face::CurrentSearchMatch | Face::Region | Face::ModeLine => Some("\x1b[7m"),
            Face::SearchMatch => Some("\x1b[4m"),
            Face::UserHighlight
            | Face::UserHighlightAlt
            | Face::UserHighlightLine
            | Face::UserHighlightGreen
            | Face::UserHighlightBlue
            | Face::UserHighlightSalmon
            | Face::UserHighlightAquamarine
            | Face::UserHighlightBlackBold
            | Face::UserHighlightBlueBold
            | Face::UserHighlightRedBold
            | Face::UserHighlightGreenBold
            | Face::UserHighlightBlackHeavyBold => Some("\x1b[4m"),
            Face::Minibuffer | Face::LineNumber | Face::SyntaxComment => Some("\x1b[2m"),
            Face::Error | Face::Warning => Some("\x1b[1m"),
            _ => None,
        },
    }
}

struct ScreenGuard<W: Write> {
    terminal: AnsiTerminal<W>,
    active: bool,
    reset_cursor_style_on_drop: bool,
}

impl<W: Write> ScreenGuard<W> {
    fn enter(writer: W) -> Result<Self> {
        let mut terminal = AnsiTerminal::new(writer);
        terminal.enter_alternate_screen()?;
        terminal.flush()?;
        Ok(Self {
            terminal,
            active: true,
            reset_cursor_style_on_drop: false,
        })
    }

    fn set_steady_block_cursor(&mut self) -> Result<()> {
        self.terminal.set_steady_block_cursor()?;
        self.reset_cursor_style_on_drop = true;
        Ok(())
    }

    fn leave(&mut self) -> Result<()> {
        if !self.active {
            return Ok(());
        }
        if self.reset_cursor_style_on_drop {
            self.terminal.reset_cursor_style()?;
        }
        self.terminal.show_cursor()?;
        self.terminal.leave_alternate_screen()?;
        self.terminal.flush()?;
        self.active = false;
        Ok(())
    }

    fn enter_again(&mut self) -> Result<()> {
        if self.active {
            return Ok(());
        }
        self.terminal.enter_alternate_screen()?;
        if self.reset_cursor_style_on_drop {
            self.terminal.set_steady_block_cursor()?;
        }
        self.terminal.clear_screen()?;
        self.terminal.flush()?;
        self.active = true;
        Ok(())
    }
}

impl<W: Write> Drop for ScreenGuard<W> {
    fn drop(&mut self) {
        let _ = self.leave();
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixStream;
    use std::thread;
    use std::time::{Duration, Instant};

    use super::{
        AnsiTerminal, FrameOptions, LineRenderOptions, MAX_RENDER_SOURCE_CHAR_SLACK,
        MAX_RENDER_SOURCE_CHARS_PER_COLUMN, RawModeGuard, ScreenGuard, TerminalSession,
        TerminalSize, clipped_text, display_width_with_tabs, draw_editor_frame,
        draw_editor_frame_with_options, escape_terminal_controls, expand_display_text,
        format_completion_row, minibuffer_visible_line, mode_line_position, project_buffer_line,
        text_clipped_to_display_width, text_display_width, text_from_display_column,
        visible_display_range, wrapped_help_cursor_position, wrapped_help_visual_lines,
        write_buffer_line, write_fixed_width_text_with_face, write_fixed_width_text_with_spans,
        write_line_number_gutter, write_line_with_spans,
    };
    use crate::buffer::{Buffer, BufferId, Position};
    use crate::completion::{CompletionConfig, CompletionStyle};
    use crate::config::{Config, ThemeName};
    use crate::editor::Editor;
    use crate::file::{Document, DocumentKind};
    use crate::input::{KeyEvent, KeyReader, SpecialKey};
    use crate::minibuffer::PromptKind;
    use crate::render::{Face, Span};
    use crate::shell::ShellJobPoll;
    use crate::window::Viewport;

    fn shell_test_session() -> (
        TerminalSession<UnixStream, UnixStream>,
        UnixStream,
        UnixStream,
    ) {
        let (input, input_peer) = UnixStream::pair().expect("input socket pair should open");
        let (output, output_peer) = UnixStream::pair().expect("output socket pair should open");
        let output_fd = output.as_raw_fd();
        (
            TerminalSession {
                screen: ScreenGuard {
                    terminal: AnsiTerminal::new(output),
                    active: false,
                    reset_cursor_style_on_drop: false,
                },
                raw_mode: RawModeGuard::inactive(),
                input: KeyReader::new(input),
                output_fd,
                test_size: Some(TerminalSize {
                    rows: 12,
                    columns: 80,
                }),
                frame_options: FrameOptions::default(),
                last_size: None,
                shell_job: None,
                shell_input_suppression: false,
                shell_c_g_escalation: false,
            },
            input_peer,
            output_peer,
        )
    }

    fn submit_shell_command(editor: &mut Editor, command: &str) {
        editor
            .handle_key(KeyEvent::Meta('!'))
            .expect("M-! should prompt");
        editor
            .handle_key(KeyEvent::Text(command.to_owned()))
            .expect("shell command should be entered");
        editor
            .handle_key(KeyEvent::Special(SpecialKey::Enter))
            .expect("shell command should submit");
    }

    #[test]
    fn shell_completion_waits_until_completion_is_allowed() {
        let (mut session, _input_peer, _output_peer) = shell_test_session();
        let mut editor = Editor::new(Document::scratch());
        submit_shell_command(&mut editor, "printf held");
        session
            .process_shell_requests(&mut editor)
            .expect("shell job should start");
        assert!(session.shell_input_suppression);

        let deadline = Instant::now() + Duration::from_secs(2);
        let mut output_changed = false;
        loop {
            output_changed |= session
                .poll_shell_job(&mut editor, false)
                .expect("shell polling should succeed");
            let terminal = session.shell_job.as_mut().is_some_and(|job| {
                matches!(
                    job.poll(),
                    ShellJobPoll::Finished(_) | ShellJobPoll::Failed(_)
                )
            });
            if terminal {
                break;
            }
            assert!(Instant::now() < deadline, "shell job should finish");
            thread::sleep(Duration::from_millis(5));
        }

        assert!(editor.shell_command_running());
        assert!(output_changed, "streamed output should request a redraw");
        assert_eq!(editor.current_buffer_name(), "*Shell Command Output*");
        assert!(editor.document().buffer().serialize().contains("held"));
        assert!(
            !session
                .poll_shell_job(&mut editor, false)
                .expect("terminal completion should remain held")
        );
        assert!(
            session
                .finish_shell_input_quiet_period(&mut editor)
                .expect("quiet-period completion should be delivered")
        );
        assert!(!session.shell_input_suppression);
        assert!(!editor.shell_command_running());
        assert_eq!(editor.current_buffer_name(), "*Shell Command Output*");
        assert!(editor.document().buffer().serialize().contains("held"));
    }

    #[test]
    fn cancelled_shell_discards_queued_text_until_quiet_period() {
        let (mut session, _input_peer, _output_peer) = shell_test_session();
        let mut editor = Editor::new(Document::scratch());
        submit_shell_command(&mut editor, "sleep 10");
        session
            .process_shell_requests(&mut editor)
            .expect("shell job should start");
        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("first C-g should cancel logically");
        session
            .process_shell_requests(&mut editor)
            .expect("first C-g should request interruption");

        assert!(!editor.shell_command_running());
        assert!(session.shell_input_suppression);
        assert!(
            session
                .consume_shell_input_before_editor(
                    &mut editor,
                    &KeyEvent::Text("queued".to_owned()),
                )
                .expect("queued text should be consumed")
        );
        session
            .poll_shell_job(&mut editor, false)
            .expect("cancelled job should keep making progress");
        assert_eq!(editor.document().buffer().serialize(), "");
        assert!(session.shell_input_suppression);

        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            session
                .poll_shell_job(&mut editor, false)
                .expect("cancelled job should keep making progress");
            let terminal = session.shell_job.as_mut().is_some_and(|job| {
                matches!(
                    job.poll(),
                    ShellJobPoll::Cancelled | ShellJobPoll::Failed(_)
                )
            });
            if terminal {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "shell cancellation should finish"
            );
            thread::sleep(Duration::from_millis(5));
        }

        assert!(session.shell_job.is_some());
        assert!(session.shell_input_suppression);
        assert!(
            session
                .finish_shell_input_quiet_period(&mut editor)
                .expect("quiet period should finalize cancellation")
        );
        assert!(session.shell_job.is_none());
        assert!(!session.shell_input_suppression);
    }

    #[test]
    fn second_c_g_escalates_before_editor_dispatch() {
        let (mut session, _input_peer, _output_peer) = shell_test_session();
        let mut editor = Editor::new(Document::scratch());
        submit_shell_command(&mut editor, "trap '' 2; sleep 10");
        session
            .process_shell_requests(&mut editor)
            .expect("shell job should start");
        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("first C-g should cancel logically");
        session
            .process_shell_requests(&mut editor)
            .expect("first C-g should request interruption");

        assert!(session.shell_c_g_escalation);
        assert!(session.shell_input_suppression);
        assert!(
            session
                .consume_shell_input_before_editor(&mut editor, &KeyEvent::Ctrl('g'))
                .expect("second C-g should escalate")
        );
        assert!(!session.shell_c_g_escalation);
        assert_eq!(
            editor.minibuffer().message.as_deref(),
            Some("Shell command cancellation escalated")
        );
    }

    #[test]
    fn dirty_quit_cancellation_does_not_arm_c_g_escalation() {
        let (mut session, _input_peer, _output_peer) = shell_test_session();
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "dirty")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        submit_shell_command(&mut editor, "sleep 10");
        session
            .process_shell_requests(&mut editor)
            .expect("shell job should start");
        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("C-x should be consumed");
        editor
            .handle_key(KeyEvent::Ctrl('c'))
            .expect("C-x C-c should prompt for dirty buffers");
        session
            .process_shell_requests(&mut editor)
            .expect("quit cancellation should be requested");

        assert!(!session.shell_c_g_escalation);
        assert!(session.shell_input_suppression);
        assert_eq!(
            editor.minibuffer().prompt_kind(),
            Some(PromptKind::QuitDirtyBuffers)
        );
        assert!(
            !session
                .consume_shell_input_before_editor(&mut editor, &KeyEvent::Ctrl('g'))
                .expect("prompt C-g should not escalate")
        );
        editor
            .handle_key(KeyEvent::Ctrl('g'))
            .expect("prompt C-g should retain normal semantics");
        assert!(editor.minibuffer().prompt().is_none());
    }

    #[test]
    fn writes_buffered_ansi_sequences() {
        let mut terminal = AnsiTerminal::new(Vec::new());
        terminal.hide_cursor().expect("hide cursor should write");
        terminal.move_cursor(2, 3).expect("move should write");
        terminal.clear_line().expect("clear line should write");
        terminal
            .write_safe_text("status")
            .expect("text should write");
        terminal.show_cursor().expect("show cursor should write");

        assert_eq!(
            terminal.into_inner(),
            b"\x1b[?25l\x1b[2;3H\x1b[2Kstatus\x1b[?25h".to_vec()
        );
    }

    #[test]
    fn rejects_controls_from_the_safe_terminal_text_sink() {
        let mut terminal = AnsiTerminal::new(Vec::new());

        let error = terminal
            .write_safe_text("unsafe\u{1b}]0;title\u{7}")
            .expect_err("control characters should be rejected");

        assert!(error.to_string().contains("terminal display text"));
        assert!(terminal.into_inner().is_empty());
    }

    #[test]
    fn renders_control_characters_as_visible_escapes_with_faces() {
        let text = "a\u{1b}]0;title\u{7}\r\u{9b}z";
        let spans = [Span::new(1, 2, Face::Region)];
        let mut terminal = AnsiTerminal::new(Vec::new());

        write_line_with_spans(&mut terminal, text, &spans, 4, ThemeName::Default)
            .expect("render should succeed");

        assert_eq!(
            String::from_utf8(terminal.into_inner()).expect("output should be UTF-8"),
            "a\x1b[44m\\u{1b}\x1b[0m]0;title\\u{7}\\r\\u{9b}z"
        );
        assert_eq!(display_width_with_tabs("a\u{1b}\tb", 4), 9);
    }

    #[test]
    fn clipping_keeps_expanded_controls_and_tabs_on_source_boundaries() {
        let (control, _) = expand_display_text("a\u{1b}b", &[], 4);
        let (tab, _) = expand_display_text("a\tb", &[], 4);

        assert_eq!(text_clipped_to_display_width(&control, 7, 4), "a\\u{1b}");
        assert_eq!(&control[visible_display_range(&control, 2, 5, 4)], "u{1b}");
        assert_eq!(&tab[visible_display_range(&tab, 2, 2, 4)], "  ");
    }

    #[test]
    fn escapes_controls_for_terminal_diagnostics() {
        assert_eq!(
            escape_terminal_controls("bad\u{1b}]0;title\u{7}\npath\tname"),
            "bad\\u{1b}]0;title\\u{7}\\npath\\tname"
        );
    }

    #[test]
    fn renders_search_spans_with_ansi_faces() {
        let spans = [
            Span {
                start_byte: 0,
                end_byte: 3,
                face: Face::CurrentSearchMatch,
            },
            Span {
                start_byte: 4,
                end_byte: 7,
                face: Face::SearchMatch,
            },
        ];
        let mut terminal = AnsiTerminal::new(Vec::new());

        write_line_with_spans(&mut terminal, "one two", &spans, 4, ThemeName::Default)
            .expect("render should succeed");

        assert_eq!(
            terminal.into_inner(),
            b"\x1b[7mone\x1b[0m \x1b[4mtwo\x1b[0m".to_vec()
        );
    }

    #[test]
    fn renders_user_highlight_spans_with_ansi_faces() {
        let cases = [
            (Face::UserHighlight, b"\x1b[43;30mone\x1b[0m two".to_vec()),
            (
                Face::UserHighlightAlt,
                b"\x1b[45;37mone\x1b[0m two".to_vec(),
            ),
            (
                Face::UserHighlightLine,
                b"\x1b[43;30mone\x1b[0m two".to_vec(),
            ),
            (
                Face::UserHighlightGreen,
                b"\x1b[42;30mone\x1b[0m two".to_vec(),
            ),
            (
                Face::UserHighlightBlue,
                b"\x1b[44;37mone\x1b[0m two".to_vec(),
            ),
            (
                Face::UserHighlightSalmon,
                b"\x1b[41;37mone\x1b[0m two".to_vec(),
            ),
            (
                Face::UserHighlightAquamarine,
                b"\x1b[46;30mone\x1b[0m two".to_vec(),
            ),
            (
                Face::UserHighlightBlackBold,
                b"\x1b[40;37;1mone\x1b[0m two".to_vec(),
            ),
            (
                Face::UserHighlightBlueBold,
                b"\x1b[44;37;1mone\x1b[0m two".to_vec(),
            ),
            (
                Face::UserHighlightRedBold,
                b"\x1b[41;37;1mone\x1b[0m two".to_vec(),
            ),
            (
                Face::UserHighlightGreenBold,
                b"\x1b[42;30;1mone\x1b[0m two".to_vec(),
            ),
            (
                Face::UserHighlightBlackHeavyBold,
                b"\x1b[40;97;1mone\x1b[0m two".to_vec(),
            ),
        ];

        for (face, expected) in cases {
            let spans = [Span::new(0, 3, face)];
            let mut terminal = AnsiTerminal::new(Vec::new());

            write_line_with_spans(&mut terminal, "one two", &spans, 4, ThemeName::Default)
                .expect("render should succeed");

            assert_eq!(terminal.into_inner(), expected);
        }
    }

    #[test]
    fn renders_fixed_width_text_with_faces() {
        let mut terminal = AnsiTerminal::new(Vec::new());

        write_fixed_width_text_with_face(
            &mut terminal,
            "mode",
            6,
            Face::ModeLine,
            ThemeName::Default,
        )
        .expect("render should succeed");

        assert_eq!(terminal.into_inner(), b"\x1b[7mmode\x1b[0m  ".to_vec());
    }

    #[test]
    fn renders_fixed_width_text_with_spans_using_plain_minibuffer_clipping() {
        let mut terminal = AnsiTerminal::new(Vec::new());

        write_fixed_width_text_with_spans(
            &mut terminal,
            "abcdef",
            &[Span::new(0, 3, Face::Minibuffer)],
            4,
            ThemeName::Default,
        )
        .expect("render should succeed");

        assert_eq!(terminal.into_inner(), b"\x1b[36mabc\x1b[0md".to_vec());
    }

    #[test]
    fn renders_fixed_width_text_with_spans_using_display_width() {
        let mut terminal = AnsiTerminal::new(Vec::new());

        write_fixed_width_text_with_spans(
            &mut terminal,
            "\ta",
            &[Span::new(0, 1, Face::Minibuffer)],
            5,
            ThemeName::Default,
        )
        .expect("render should succeed");

        assert_eq!(
            String::from_utf8(terminal.into_inner()).expect("output should be UTF-8"),
            "\x1b[36m    \x1b[0ma"
        );
    }

    #[test]
    fn clipped_text_marks_hidden_right_edge() {
        assert_eq!(clipped_text("abcdef", 4), "abc$");
        assert_eq!(clipped_text("abcdef", 1), "$");
        assert_eq!(clipped_text("abc", 4), "abc");
        assert_eq!(clipped_text("界x", 2), " $");
        assert_eq!(clipped_text("a界x", 3), "a $");
    }

    #[test]
    fn text_from_display_column_keeps_prompt_tail() {
        let text = "Find file: /very/long/path/name.txt";
        let start = "Find file: /very/".chars().count();

        assert_eq!(text_from_display_column(text, start), "long/path/name.txt");
    }

    #[test]
    fn hidden_edge_markers_preserve_region_face() {
        let buffer = Buffer::from_text("abcdefghij");
        let viewport = Viewport {
            buffer: BufferId(0),
            cursor: Position::new(0, 0),
            first_visible_line: 0,
            first_visible_column: 2,
            text_rows: 1,
        };
        let spans = [Span::new(0, 10, Face::Region)];
        let mut terminal = AnsiTerminal::new(Vec::new());

        write_buffer_line(
            &mut terminal,
            &buffer,
            &viewport,
            0,
            buffer.line(0).expect("line should exist"),
            &spans,
            LineRenderOptions {
                width: 5,
                tab_width: 4,
                theme: ThemeName::Default,
                highlight_line_end_space: false,
            },
        )
        .expect("render should succeed");

        assert_eq!(terminal.into_inner(), b"\x1b[44m$def$\x1b[0m".to_vec());
    }

    #[test]
    fn hidden_edge_markers_preserve_region_face_with_multibyte_edges() {
        let buffer = Buffer::from_text("abcédef");
        let viewport = Viewport::new(BufferId(0));
        let spans = [Span::new(0, "abcédef".len(), Face::Region)];
        let mut terminal = AnsiTerminal::new(Vec::new());

        write_buffer_line(
            &mut terminal,
            &buffer,
            &viewport,
            0,
            buffer.line(0).expect("line should exist"),
            &spans,
            LineRenderOptions {
                width: 4,
                tab_width: 4,
                theme: ThemeName::Default,
                highlight_line_end_space: false,
            },
        )
        .expect("render should succeed");

        assert_eq!(terminal.into_inner(), b"\x1b[44mabc$\x1b[0m".to_vec());
    }

    #[test]
    fn long_line_projection_stops_after_visible_ascii_prefix() {
        let line = "x".repeat(1_000_000);

        let projection = project_buffer_line(&line, &[], 0, 80, 4);

        assert_eq!(projection.text, "x".repeat(80));
        assert!(projection.right_hidden);
        assert!(projection.source_chars_scanned <= 81);
    }

    #[test]
    fn long_zero_width_run_stops_at_source_budget() {
        let width = 80;
        let budget = width * MAX_RENDER_SOURCE_CHARS_PER_COLUMN + MAX_RENDER_SOURCE_CHAR_SLACK;
        let line = format!("{}x", "\u{301}".repeat(budget + 1));

        let projection = project_buffer_line(&line, &[], 0, width, 4);

        assert!(projection.text.is_empty());
        assert!(projection.right_hidden);
        assert_eq!(projection.source_chars_scanned, budget);
    }

    #[test]
    fn projected_tabs_controls_and_spans_preserve_visible_source() {
        let line = "a\t\u{1b}b";
        let control_start = "a\t".len();
        let spans = [Span::new(
            control_start,
            control_start + '\u{1b}'.len_utf8(),
            Face::Region,
        )];

        let projection = project_buffer_line(line, &spans, 2, 8, 4);

        assert_eq!(projection.text, "  \\u{1b}");
        assert!(projection.left_hidden);
        assert!(projection.right_hidden);
        assert_eq!(
            projection.spans,
            vec![Span::new(2, projection.text.len(), Face::Region)]
        );
    }

    #[test]
    fn right_edge_marker_replaces_trailing_zero_width_run() {
        let width = 1;
        let budget = width * MAX_RENDER_SOURCE_CHARS_PER_COLUMN + MAX_RENDER_SOURCE_CHAR_SLACK;
        let line = format!("a{}b", "\u{301}".repeat(budget));
        let buffer = Buffer::from_text(&line);
        let viewport = Viewport::new(BufferId(0));
        let spans = [Span::new(0, line.len(), Face::Region)];
        let mut terminal = AnsiTerminal::new(Vec::new());

        write_buffer_line(
            &mut terminal,
            &buffer,
            &viewport,
            0,
            buffer.line(0).expect("line should exist"),
            &spans,
            LineRenderOptions {
                width,
                tab_width: 4,
                theme: ThemeName::Default,
                highlight_line_end_space: false,
            },
        )
        .expect("render should succeed");

        assert_eq!(terminal.into_inner(), b"\x1b[44m$\x1b[0m".to_vec());
    }

    #[test]
    fn bounded_projection_matches_full_pipeline_below_budget() {
        let cases = [
            ("a\tb", 0, 2),
            ("a\tb", 2, 2),
            ("a\u{1b}b", 2, 5),
            ("a\u{9b}b", 3, 4),
            ("a界b", 0, 2),
            ("a界b", 1, 2),
            ("\u{301}a", 0, 1),
            ("a\u{301}", 0, 1),
        ];

        for (line, start_column, width) in cases {
            let spans = [Span::new(0, line.len(), Face::Region)];
            let projection = project_buffer_line(line, &spans, start_column, width, 4);
            let (expected_text, expected_spans, expected_left, expected_right) =
                full_line_projection(line, &spans, start_column, width, 4);

            assert_eq!(projection.text, expected_text, "line={line:?}");
            assert_eq!(projection.spans, expected_spans, "line={line:?}");
            assert_eq!(projection.left_hidden, expected_left, "line={line:?}");
            assert_eq!(projection.right_hidden, expected_right, "line={line:?}");
        }
    }

    #[test]
    fn selected_line_end_space_uses_region_face() {
        let buffer = Buffer::from_text("short");
        let viewport = Viewport::new(BufferId(0));
        let spans = [Span::new(0, 5, Face::Region)];
        let mut terminal = AnsiTerminal::new(Vec::new());

        write_buffer_line(
            &mut terminal,
            &buffer,
            &viewport,
            0,
            buffer.line(0).expect("line should exist"),
            &spans,
            LineRenderOptions {
                width: 10,
                tab_width: 4,
                theme: ThemeName::Default,
                highlight_line_end_space: true,
            },
        )
        .expect("render should succeed");

        assert_eq!(
            terminal.into_inner(),
            b"\x1b[44mshort\x1b[0m\x1b[44m     \x1b[0m".to_vec()
        );
    }

    #[test]
    fn wrapped_help_visual_lines_continue_long_logical_lines() {
        let buffer = Buffer::from_text("abcdefghij\nklm");

        let rows = wrapped_help_visual_lines(&buffer, 0, 4, 6, 4);

        assert_eq!(rows[0].as_ref().map(|line| line.line_index), Some(0));
        assert_eq!(rows[0].as_ref().map(|line| line.start_column), Some(0));
        assert_eq!(rows[0].as_ref().map(|line| line.continued), Some(true));
        assert_eq!(rows[1].as_ref().map(|line| line.line_index), Some(0));
        assert_eq!(rows[1].as_ref().map(|line| line.start_column), Some(5));
        assert_eq!(rows[1].as_ref().map(|line| line.continued), Some(false));
        assert_eq!(rows[2].as_ref().map(|line| line.line_index), Some(1));
        assert_eq!(rows[2].as_ref().map(|line| line.start_column), Some(0));
        assert_eq!(rows[2].as_ref().map(|line| line.continued), Some(false));
        assert!(rows[3].is_none());
    }

    #[test]
    fn wrapped_help_visual_lines_keep_content_at_width_one() {
        let buffer = Buffer::from_text("abc");

        let rows = wrapped_help_visual_lines(&buffer, 0, 3, 1, 4);

        assert_eq!(rows[0].as_ref().map(|line| line.start_column), Some(0));
        assert_eq!(rows[0].as_ref().map(|line| line.continued), Some(true));
        assert_eq!(rows[1].as_ref().map(|line| line.start_column), Some(1));
        assert_eq!(rows[1].as_ref().map(|line| line.continued), Some(true));
        assert_eq!(rows[2].as_ref().map(|line| line.start_column), Some(2));
        assert_eq!(rows[2].as_ref().map(|line| line.continued), Some(false));
    }

    #[test]
    fn wrapped_help_cursor_position_uses_visual_rows() {
        let buffer = Buffer::from_text("abcdefghij");
        let viewport = Viewport::new(BufferId(0));

        assert_eq!(
            wrapped_help_cursor_position(&buffer, &viewport, Position::new(0, 7), 3, 6, 4)
                .expect("cursor position should resolve"),
            (1, 2)
        );
    }

    #[test]
    fn help_buffers_render_continuation_rows_in_narrow_windows() {
        let document = Document::help("abcdefghij");
        let mut editor = Editor::new(document);
        let size = TerminalSize {
            rows: 5,
            columns: 6,
        };

        let frame = rendered_frame(&mut editor, size);

        assert!(frame.contains("abcde\\"));
        assert!(frame.contains("fghij "));
    }

    #[test]
    fn help_buffers_render_region_face() {
        let document = Document::help("alpha beta");
        let mut editor = Editor::new(document);
        editor
            .execute_command_by_name("mark-whole-buffer")
            .expect("mark-whole-buffer should activate region");
        let size = TerminalSize {
            rows: 4,
            columns: 20,
        };

        let frame = rendered_frame_bytes(&mut editor, size);

        assert!(contains_bytes(&frame, b"\x1b[44malpha beta\x1b[0m"));
    }

    #[test]
    fn wrapped_help_buffers_render_region_face() {
        let document = Document::help("abcdefghij\nnext");
        let mut editor = Editor::new(document);
        editor
            .execute_command_by_name("mark-whole-buffer")
            .expect("mark-whole-buffer should activate region");
        let size = TerminalSize {
            rows: 5,
            columns: 6,
        };

        let frame = rendered_frame_bytes(&mut editor, size);

        assert!(contains_bytes(&frame, b"\x1b[44mabcde\x1b[0m\\"));
        assert!(contains_bytes(&frame, b"\x1b[44mfghij\x1b[0m"));
        assert!(contains_bytes(
            &frame,
            b"\x1b[44mfghij\x1b[0m\x1b[44m \x1b[0m"
        ));
    }

    #[test]
    fn help_buffers_show_content_in_one_column_windows() {
        let document = Document::help("ab");
        let mut editor = Editor::new(document);
        let size = TerminalSize {
            rows: 4,
            columns: 1,
        };

        let frame = rendered_frame(&mut editor, size);

        assert!(frame.contains("\x1b[1;1Ha"));
        assert!(frame.contains("\x1b[2;1Hb"));
        assert!(!frame.contains('\\'));
    }

    #[test]
    fn help_buffer_cursor_uses_continuation_row() {
        let document = Document::help("abcdefghij");
        let mut editor = Editor::new(document);
        let size = TerminalSize {
            rows: 5,
            columns: 6,
        };

        for _ in 0..7 {
            editor
                .handle_key(KeyEvent::Ctrl('f'))
                .expect("cursor should move in help buffer");
        }

        assert_eq!(rendered_cursor_position(&mut editor, size), Some((2, 3)));
    }

    #[test]
    fn prompt_cursor_uses_minibuffer_row_for_vertical_completion() {
        let mut editor = Editor::new(Document::scratch());
        let size = TerminalSize {
            rows: 8,
            columns: 80,
        };

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");

        let completion = editor.completion().expect("completion should start");
        let prefix = format!(
            "{}/{}  ",
            completion.selected_match_number().unwrap_or(0),
            completion.match_count()
        );
        let expected_column = prefix.chars().count() + "M-x ".chars().count() + 1;

        assert_eq!(
            rendered_cursor_position(&mut editor, size),
            Some((2, expected_column))
        );
    }

    #[test]
    fn prompt_cursor_stays_before_ido_candidates() {
        let mut editor = Editor::with_config(
            Document::scratch(),
            Config {
                completion: CompletionConfig {
                    style: CompletionStyle::Ido,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );
        let size = TerminalSize {
            rows: 8,
            columns: 80,
        };

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("toggle-s".to_owned()))
            .expect("prompt input should update completion");

        assert_eq!(
            rendered_cursor_position(&mut editor, size),
            Some((8, "M-x toggle-s".chars().count() + 1))
        );
    }

    #[test]
    fn minibuffer_prompt_face_does_not_cover_m_x_input() {
        let mut editor = Editor::new(Document::scratch());
        let size = TerminalSize {
            rows: 8,
            columns: 80,
        };

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("save-buffer".to_owned()))
            .expect("prompt input should insert");

        let frame = rendered_frame_bytes(&mut editor, size);
        assert_minibuffer_visible_line_matches_display(&editor, 80);
        let line = minibuffer_visible_line(&editor, 80).expect("prompt should render");
        let input_start = line
            .text
            .find("save-buffer")
            .expect("prompt input should be visible");
        assert!(line.text[..input_start].contains("M-x "));
        assert_eq!(
            line.spans,
            vec![Span::new(0, input_start, Face::Minibuffer)]
        );

        let faced_prompt = format!("\x1b[36m{}\x1b[0msave-buffer", &line.text[..input_start]);
        assert!(contains_bytes(&frame, faced_prompt.as_bytes()));
        assert!(!contains_bytes(&frame, b"\x1b[36mM-x save-buffer\x1b[0m"));
    }

    #[test]
    fn minibuffer_prompt_face_does_not_cover_find_file_input() {
        let mut editor = Editor::new(Document::scratch());
        let size = TerminalSize {
            rows: 8,
            columns: 120,
        };

        editor
            .execute_command_by_name("find-file")
            .expect("find-file should start prompt");
        editor
            .handle_key(KeyEvent::Text("target.txt".to_owned()))
            .expect("prompt input should insert");

        let frame = rendered_frame_bytes(&mut editor, size);
        assert_minibuffer_visible_line_matches_display(&editor, 120);
        let line = minibuffer_visible_line(&editor, 120).expect("prompt should render");
        let prompt_end = line
            .text
            .find("Find file: ")
            .map(|start| start + "Find file: ".len())
            .expect("prompt label should be visible");

        assert!(line.text[prompt_end..].contains("target.txt"));
        assert_eq!(line.spans, vec![Span::new(0, prompt_end, Face::Minibuffer)]);
        let faced_prompt = format!("\x1b[36m{}\x1b[0m", &line.text[..prompt_end]);
        assert!(contains_bytes(&frame, faced_prompt.as_bytes()));
        assert!(contains_bytes(&frame, b"target.txt"));
        assert!(!contains_bytes(&frame, b"\x1b[36mFind file: target.txt"));
    }

    #[test]
    fn minibuffer_prompt_face_does_not_cover_describe_function_input() {
        let mut editor = Editor::new(Document::scratch());
        let size = TerminalSize {
            rows: 8,
            columns: 100,
        };

        editor
            .execute_command_by_name("describe-function")
            .expect("describe-function should start prompt");
        editor
            .handle_key(KeyEvent::Text("find-file".to_owned()))
            .expect("prompt input should insert");

        let frame = rendered_frame_bytes(&mut editor, size);
        assert_minibuffer_visible_line_matches_display(&editor, 100);
        let line = minibuffer_visible_line(&editor, 100).expect("prompt should render");
        let input_start = line
            .text
            .find("find-file")
            .expect("prompt input should be visible");

        assert!(line.text[..input_start].contains("Describe function: "));
        assert_eq!(
            line.spans,
            vec![Span::new(0, input_start, Face::Minibuffer)]
        );
        let faced_prompt = format!("\x1b[36m{}\x1b[0mfind-file", &line.text[..input_start]);
        assert!(contains_bytes(&frame, faced_prompt.as_bytes()));
        assert!(!contains_bytes(
            &frame,
            b"\x1b[36mDescribe function: find-file\x1b[0m"
        ));
    }

    #[test]
    fn minibuffer_ido_candidates_keep_plain_display_parity() {
        let mut editor = Editor::with_config(
            Document::scratch(),
            Config {
                completion: CompletionConfig {
                    style: CompletionStyle::Ido,
                    ..CompletionConfig::default()
                },
                ..Config::default()
            },
        );
        let size = TerminalSize {
            rows: 8,
            columns: 120,
        };

        editor
            .handle_key(KeyEvent::Meta('x'))
            .expect("M-x should start prompt");
        editor
            .handle_key(KeyEvent::Text("toggle-s".to_owned()))
            .expect("prompt input should update completion");

        let frame = rendered_frame_bytes(&mut editor, size);
        assert_minibuffer_visible_line_matches_display(&editor, 120);

        assert!(contains_bytes(&frame, b"\x1b[36mM-x \x1b[0mtoggle-s  ["));
        assert!(!contains_bytes(&frame, b"\x1b[36mM-x toggle-s"));
    }

    #[test]
    fn minibuffer_prompt_face_covers_generated_query_replace_label_only() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "red one\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        let size = TerminalSize {
            rows: 8,
            columns: 80,
        };

        editor
            .handle_key(KeyEvent::Meta('%'))
            .expect("query-replace should start prompt");
        editor
            .handle_key(KeyEvent::Text("red".to_owned()))
            .expect("search prompt input should insert");
        editor
            .handle_key(KeyEvent::Special(crate::input::SpecialKey::Enter))
            .expect("query-replace search should submit");
        editor
            .handle_key(KeyEvent::Text("green".to_owned()))
            .expect("replacement prompt input should insert");

        let frame = rendered_frame_bytes(&mut editor, size);

        assert!(contains_bytes(
            &frame,
            b"\x1b[36mQuery replace red with: \x1b[0mgreen"
        ));
        assert!(!contains_bytes(
            &frame,
            b"\x1b[36mQuery replace red with: green\x1b[0m"
        ));
    }

    #[test]
    fn minibuffer_prompt_face_does_not_cover_isearch_input() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "red one\n")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        let size = TerminalSize {
            rows: 8,
            columns: 80,
        };

        editor
            .handle_key(KeyEvent::Ctrl('s'))
            .expect("isearch should start prompt");
        editor
            .handle_key(KeyEvent::Text("red".to_owned()))
            .expect("isearch prompt input should insert");

        let frame = rendered_frame_bytes(&mut editor, size);

        assert!(contains_bytes(&frame, b"\x1b[36mI-search: \x1b[0mred"));
        assert!(!contains_bytes(&frame, b"\x1b[36mI-search: red\x1b[0m"));
    }

    #[test]
    fn minibuffer_prefix_key_echo_uses_default_face() {
        let mut editor = Editor::new(Document::scratch());
        let size = TerminalSize {
            rows: 8,
            columns: 80,
        };

        editor
            .handle_key(KeyEvent::Ctrl('x'))
            .expect("prefix key should update minibuffer");

        let frame = rendered_frame_bytes(&mut editor, size);

        assert!(contains_bytes(&frame, b"C-x-"));
        assert!(!contains_bytes(&frame, b"\x1b[36mC-x-\x1b[0m"));
    }

    #[test]
    fn minibuffer_visible_line_clips_prompt_span_after_prompt_start() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .execute_command_by_name("goto-line")
            .expect("goto-line should start prompt");
        editor
            .handle_key(KeyEvent::Text("1234567890".to_owned()))
            .expect("prompt input should insert");

        let line = minibuffer_visible_line(&editor, 20).expect("prompt should render");

        assert_eq!(line.text, "to line: 1234567890");
        assert_eq!(
            line.spans,
            vec![Span::new(0, "to line: ".len(), Face::Minibuffer)]
        );
        assert_eq!(line.cursor, Some(19));
    }

    #[test]
    fn minibuffer_visible_line_clips_prompt_span_at_input_boundary() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .execute_command_by_name("goto-line")
            .expect("goto-line should start prompt");
        editor
            .handle_key(KeyEvent::Text("12345".to_owned()))
            .expect("prompt input should insert");

        let line = minibuffer_visible_line(&editor, 6).expect("prompt should render");

        assert_eq!(line.text, "12345");
        assert!(line.spans.is_empty());
        assert_eq!(line.cursor, Some(5));
    }

    #[test]
    fn minibuffer_visible_line_clips_prompt_span_inside_input() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .execute_command_by_name("goto-line")
            .expect("goto-line should start prompt");
        editor
            .handle_key(KeyEvent::Text("12345".to_owned()))
            .expect("prompt input should insert");

        let line = minibuffer_visible_line(&editor, 3).expect("prompt should render");

        assert_eq!(line.text, "45");
        assert!(line.spans.is_empty());
        assert_eq!(line.cursor, Some(2));
    }

    #[test]
    fn minibuffer_visible_line_keeps_combining_input_when_clipping_inside_it() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .execute_command_by_name("goto-line")
            .expect("goto-line should start prompt");
        editor
            .handle_key(KeyEvent::Text("e\u{301}abc".to_owned()))
            .expect("prompt input should insert");

        let line = minibuffer_visible_line(&editor, 5).expect("prompt should render");

        assert_eq!(line.text, "e\u{301}abc");
        assert!(line.spans.is_empty());
        assert_eq!(line.cursor, Some(4));
    }

    #[test]
    fn minibuffer_visible_line_can_start_inside_expanded_control_character() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .execute_command_by_name("goto-line")
            .expect("goto-line should start prompt");
        editor
            .handle_key(KeyEvent::Text("123\u{1b}456".to_owned()))
            .expect("prompt input should insert");

        let line = minibuffer_visible_line(&editor, 8).expect("prompt should render");

        assert_eq!(line.text, "{1b}456");
        assert!(line.spans.is_empty());
        assert_eq!(line.cursor, Some(7));
        assert_eq!(text_display_width(&line.text), 7);
    }

    #[test]
    fn minibuffer_visible_line_expands_tabs_before_clipping() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .execute_command_by_name("goto-line")
            .expect("goto-line should start prompt");
        editor
            .handle_key(KeyEvent::Text("123\t456".to_owned()))
            .expect("prompt input should insert");

        let line = minibuffer_visible_line(&editor, 8).expect("prompt should render");

        assert_eq!(line.text, "23  456");
        assert!(line.spans.is_empty());
        assert_eq!(line.cursor, Some(7));
        assert_eq!(text_display_width(&line.text), 7);
    }

    #[test]
    fn minibuffer_prompt_face_renders_combining_input_without_coloring_it() {
        let mut editor = Editor::new(Document::scratch());
        let size = TerminalSize {
            rows: 8,
            columns: 40,
        };

        editor
            .execute_command_by_name("goto-line")
            .expect("goto-line should start prompt");
        editor
            .handle_key(KeyEvent::Text("e\u{301}".to_owned()))
            .expect("prompt input should insert");

        let frame = rendered_frame_bytes(&mut editor, size);

        assert!(contains_bytes(
            &frame,
            "\x1b[36mGoto line: \x1b[0me\u{301}".as_bytes()
        ));
        assert!(!contains_bytes(
            &frame,
            "\x1b[36mGoto line: e\u{301}".as_bytes()
        ));
    }

    #[test]
    fn completion_rows_clip_candidates_and_annotations() {
        assert_eq!(
            format_completion_row("rmail", "Read and edit incoming mail.", 34, 80),
            format!(
                "{}{}{}",
                "rmail",
                " ".repeat(37),
                "Read and edit incoming mail."
            )
        );
        assert_eq!(
            format_completion_row("repeat (C-x z)", "Repeat last command.", 34, 80),
            format!(
                "{}{}{}",
                "repeat (C-x z)",
                " ".repeat(28),
                "Repeat last command."
            )
        );
        assert_eq!(
            format_completion_row(
                "emacs-lisp-native-compile-and-load",
                "Native-compile synchronously the current file.",
                34,
                100,
            ),
            format!(
                "{}{}{}",
                "emacs-lisp-native-compile-and-load",
                " ".repeat(8),
                "Native-compile synchronously the current file."
            )
        );
        assert!(
            format_completion_row("very-long-command", "Long annotation", 8, 24)
                .starts_with("very-lo$")
        );
        assert!(format_completion_row("remember", "A very long annotation", 8, 18).ends_with('$'));
    }

    #[test]
    fn expands_tabs_using_configured_width() {
        let mut terminal = AnsiTerminal::new(Vec::new());

        let width = write_line_with_spans(&mut terminal, "a\tb", &[], 2, ThemeName::Default)
            .expect("render should succeed");

        assert_eq!(width, 3);
        assert_eq!(terminal.into_inner(), b"a b".to_vec());
    }

    #[test]
    fn renders_line_number_gutter_with_face() {
        let mut terminal = AnsiTerminal::new(Vec::new());

        write_line_number_gutter(&mut terminal, 8, 3, ThemeName::Default)
            .expect("gutter should render");

        assert_eq!(terminal.into_inner(), b"\x1b[2m 9 \x1b[0m".to_vec());
    }

    #[test]
    fn formats_mode_line_position_like_emacs() {
        let buffer = Buffer::from_text("one\ntwo\nthree\nfour\nfive");
        let mut viewport = Viewport::new(BufferId(0));

        assert_eq!(
            mode_line_position(&buffer, &viewport, 10, 4).expect("position should format"),
            "All (1,0)"
        );

        viewport.cursor = Position::new(2, 0);
        assert_eq!(
            mode_line_position(&buffer, &viewport, 2, 4).expect("position should format"),
            "Top (3,0)"
        );

        viewport.first_visible_line = 3;
        viewport.cursor = Position::new(4, 0);
        assert_eq!(
            mode_line_position(&buffer, &viewport, 2, 4).expect("position should format"),
            "Bot (5,0)"
        );
    }

    #[test]
    fn normal_mode_line_uses_modern_position_style() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo\nthree")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        let size = TerminalSize {
            rows: 6,
            columns: 60,
        };

        let frame = rendered_frame(&mut editor, size);

        assert!(frame.contains("=-:"));
        assert!(frame.contains("*scratch*"));
        assert!(frame.contains("All (1,0)"));
        assert!(frame.contains("(Fundamental)"));
    }

    #[test]
    fn redraw_refreshes_visible_messages_buffer() {
        let mut editor = Editor::new(Document::scratch());
        editor
            .execute_command_by_name("missing-command")
            .expect("unknown command should set message");
        editor
            .execute_command_by_name("view-echo-area-messages")
            .expect("messages buffer should open");

        editor
            .handle_key(KeyEvent::Text("x".to_owned()))
            .expect("read-only insert should set message");
        let size = TerminalSize {
            rows: 8,
            columns: 80,
        };
        rendered_frame(&mut editor, size);

        assert!(
            editor
                .document()
                .buffer()
                .serialize()
                .contains("Buffer is read-only: *Messages*")
        );
    }

    #[test]
    fn redraw_preserves_dirty_normal_file_named_messages() {
        let directory = tempfile::tempdir().expect("temporary directory should exist");
        let path = directory.path().join("*Messages*");
        fs::write(&path, "normal contents").expect("fixture should write");
        let mut document = Document::open(&path).expect("document should open");
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "edited ")
            .expect("document should become dirty");
        let mut editor = Editor::new(document);
        let size = TerminalSize {
            rows: 8,
            columns: 80,
        };

        rendered_frame(&mut editor, size);

        assert_eq!(editor.document().kind(), DocumentKind::Normal);
        assert_eq!(editor.document().path(), Some(path.as_path()));
        assert_eq!(
            editor.document().buffer().serialize(),
            "edited normal contents"
        );
        assert!(editor.document().is_dirty());
        assert!(!editor.document().is_read_only());
    }

    #[test]
    fn redraw_moves_cursor_up_on_each_previous_line() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo\nthree\nfour\nfive")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        let size = TerminalSize {
            rows: 6,
            columns: 40,
        };

        for _ in 0..3 {
            editor
                .handle_key(KeyEvent::Ctrl('n'))
                .expect("cursor should move down");
        }
        assert_eq!(rendered_cursor_position(&mut editor, size), Some((4, 1)));

        editor
            .handle_key(KeyEvent::Ctrl('p'))
            .expect("cursor should move up");
        assert_eq!(rendered_cursor_position(&mut editor, size), Some((3, 1)));

        editor
            .handle_key(KeyEvent::Ctrl('p'))
            .expect("cursor should move up again");
        assert_eq!(rendered_cursor_position(&mut editor, size), Some((2, 1)));
    }

    #[test]
    fn redraw_hides_cursor_until_final_cursor_move() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo\nthree")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        let size = TerminalSize {
            rows: 5,
            columns: 40,
        };

        editor
            .handle_key(KeyEvent::Ctrl('n'))
            .expect("cursor should move down");
        let frame = rendered_frame_bytes(&mut editor, size);

        assert!(
            frame.starts_with(b"\x1b[?25l"),
            "redraw should hide cursor before painting"
        );
        let final_cursor = frame
            .windows(b"\x1b[2;1H".len())
            .rposition(|window| window == b"\x1b[2;1H")
            .expect("redraw should move cursor to final point");
        let show_cursor = frame
            .windows(b"\x1b[?25h".len())
            .rposition(|window| window == b"\x1b[?25h")
            .expect("redraw should show cursor after painting");
        assert!(
            final_cursor < show_cursor,
            "redraw should show cursor only after final cursor move"
        );
    }

    #[test]
    fn normal_redraw_does_not_clear_screen() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        let size = TerminalSize {
            rows: 5,
            columns: 40,
        };

        let frame = rendered_frame_bytes(&mut editor, size);

        assert!(
            !contains_bytes(&frame, b"\x1b[2J"),
            "normal redraw should not clear the whole screen"
        );
    }

    #[test]
    fn resize_redraw_can_clear_screen_while_cursor_hidden() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        let size = TerminalSize {
            rows: 5,
            columns: 40,
        };

        let frame = rendered_frame_bytes_with_options(
            &mut editor,
            size,
            FrameOptions {
                clear_screen: true,
                ..FrameOptions::default()
            },
        );

        assert!(
            frame.starts_with(b"\x1b[?25l\x1b[1;1H\x1b[2J"),
            "resize redraw should hide cursor before clearing"
        );
    }

    #[test]
    fn redraw_moves_cursor_up_after_viewport_scrolled_down() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo\nthree\nfour\nfive\nsix")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        let size = TerminalSize {
            rows: 5,
            columns: 40,
        };

        for _ in 0..4 {
            editor
                .handle_key(KeyEvent::Ctrl('n'))
                .expect("cursor should move down");
            rendered_cursor_position(&mut editor, size);
        }
        assert_eq!(rendered_cursor_position(&mut editor, size), Some((3, 1)));

        editor
            .handle_key(KeyEvent::Ctrl('p'))
            .expect("cursor should move up");
        assert_eq!(rendered_cursor_position(&mut editor, size), Some((2, 1)));

        editor
            .handle_key(KeyEvent::Ctrl('p'))
            .expect("cursor should move up again");
        assert_eq!(rendered_cursor_position(&mut editor, size), Some((1, 1)));
    }

    #[test]
    fn redraw_updates_mode_line_position_during_repeated_previous_line_scroll() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "one\ntwo\nthree\nfour\nfive\nsix")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        let size = TerminalSize {
            rows: 5,
            columns: 60,
        };

        for _ in 0..5 {
            editor
                .handle_key(KeyEvent::Ctrl('n'))
                .expect("cursor should move down");
            rendered_frame(&mut editor, size);
        }
        assert!(rendered_frame(&mut editor, size).contains("Bot (6,0)"));

        editor
            .handle_key(KeyEvent::Ctrl('p'))
            .expect("cursor should move up");
        assert!(rendered_frame(&mut editor, size).contains("Bot (5,0)"));

        editor
            .handle_key(KeyEvent::Ctrl('p'))
            .expect("cursor should move up again");
        assert!(rendered_frame(&mut editor, size).contains("Bot (4,0)"));
    }

    #[test]
    fn visual_test_mode_line_is_deterministic() {
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "hello\nworld")
            .expect("fixture should insert");
        let mut editor = Editor::new(document);
        let size = TerminalSize {
            rows: 5,
            columns: 80,
        };

        let frame = rendered_frame_with_options(
            &mut editor,
            size,
            FrameOptions {
                visual_test: true,
                ..FrameOptions::default()
            },
        );

        assert!(frame.contains("Rile VISUAL window 0 ACTIVE *scratch*"));
        assert!(frame.contains("Ln 001 Col 000"));
        assert!(frame.contains("modified:true"));
    }

    #[test]
    fn rendered_frame_escapes_controls_from_file_names_and_contents() {
        let directory = tempfile::tempdir().expect("test directory should exist");
        let path = directory.path().join("name_\u{1b}]0;FILE_PWN\u{7}.txt");
        fs::write(&path, "body_\u{1b}]0;BODY_PWN\u{7}\r\n")
            .expect("malicious fixture should write");
        let document = Document::open(&path).expect("malicious fixture should open");
        let mut editor = Editor::new(document);
        let size = TerminalSize {
            rows: 6,
            columns: 160,
        };

        let bytes = rendered_frame_bytes_with_options(
            &mut editor,
            size,
            FrameOptions {
                visual_test: true,
                ..FrameOptions::default()
            },
        );
        let frame = String::from_utf8(bytes.clone()).expect("frame should be UTF-8");

        assert!(!contains_bytes(&bytes, b"\x1b]0;FILE_PWN\x07"));
        assert!(!contains_bytes(&bytes, b"\x1b]0;BODY_PWN\x07"));
        assert!(frame.contains("body_\\u{1b}]0;BODY_PWN\\u{7}\\r"));
        assert!(frame.contains("name_\\u{1b}]0;FILE_PWN\\u{7}.txt"));
    }

    fn rendered_cursor_position(editor: &mut Editor, size: TerminalSize) -> Option<(usize, usize)> {
        last_cursor_position(rendered_frame_bytes(editor, size).as_slice())
    }

    fn full_line_projection(
        line: &str,
        spans: &[Span],
        start_column: usize,
        width: usize,
        tab_width: usize,
    ) -> (String, Vec<Span>, bool, bool) {
        let (line, spans) = expand_display_text(line, spans, tab_width);
        let range = visible_display_range(&line, start_column, width, tab_width);
        let text = line[range.clone()].to_owned();
        let spans = crate::render::clip_spans(&spans, range);
        let left_hidden = start_column > 0;
        let right_hidden =
            display_width_with_tabs(&line, tab_width) > start_column.saturating_add(width);
        (text, spans, left_hidden, right_hidden)
    }

    fn rendered_frame(editor: &mut Editor, size: TerminalSize) -> String {
        String::from_utf8(rendered_frame_bytes(editor, size)).expect("frame should be UTF-8")
    }

    fn rendered_frame_bytes(editor: &mut Editor, size: TerminalSize) -> Vec<u8> {
        let mut terminal = AnsiTerminal::new(Vec::new());
        draw_editor_frame(&mut terminal, editor, size).expect("frame should draw");
        terminal.into_inner()
    }

    fn rendered_frame_with_options(
        editor: &mut Editor,
        size: TerminalSize,
        options: FrameOptions,
    ) -> String {
        String::from_utf8(rendered_frame_bytes_with_options(editor, size, options))
            .expect("frame should be UTF-8")
    }

    fn rendered_frame_bytes_with_options(
        editor: &mut Editor,
        size: TerminalSize,
        options: FrameOptions,
    ) -> Vec<u8> {
        let mut terminal = AnsiTerminal::new(Vec::new());
        draw_editor_frame_with_options(&mut terminal, editor, size, options)
            .expect("frame should draw");
        terminal.into_inner()
    }

    fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
        haystack
            .windows(needle.len())
            .any(|window| window == needle)
    }

    fn assert_minibuffer_visible_line_matches_display(editor: &Editor, columns: usize) {
        let line = minibuffer_visible_line(editor, columns).expect("minibuffer should be visible");
        assert_eq!(
            line.text,
            editor
                .minibuffer_display_text()
                .expect("plain display text")
        );
    }

    fn last_cursor_position(output: &[u8]) -> Option<(usize, usize)> {
        let mut position = None;
        let mut index = 0;
        while index < output.len() {
            if output[index] != 0x1b || output.get(index + 1) != Some(&b'[') {
                index += 1;
                continue;
            }
            let mut cursor = index + 2;
            let row_start = cursor;
            while output.get(cursor).is_some_and(u8::is_ascii_digit) {
                cursor += 1;
            }
            if output.get(cursor) != Some(&b';') || row_start == cursor {
                index += 1;
                continue;
            }
            let row = std::str::from_utf8(&output[row_start..cursor])
                .ok()
                .and_then(|text| text.parse().ok())?;
            cursor += 1;
            let column_start = cursor;
            while output.get(cursor).is_some_and(u8::is_ascii_digit) {
                cursor += 1;
            }
            if output.get(cursor) == Some(&b'H') && column_start != cursor {
                let column = std::str::from_utf8(&output[column_start..cursor])
                    .ok()
                    .and_then(|text| text.parse().ok())?;
                position = Some((row, column));
            }
            index = cursor + 1;
        }
        position
    }
}
