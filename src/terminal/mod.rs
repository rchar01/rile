// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{self, IsTerminal, Read, Write};
use std::os::fd::AsRawFd;
use std::path::Path;

use crate::buffer::{Buffer, Position};
use crate::config::{Config, ThemeName};
use crate::editor::{Editor, EditorOutcome};
use crate::file::Document;
use crate::input::KeyReader;
use crate::render::{Face, Span, clip_spans, merge_spans};
use crate::window::{Viewport, WindowLayout};
use crate::{Result, RileError};
use unicode_width::UnicodeWidthChar;

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
        None => Document::welcome(),
    };
    let mut editor = Editor::with_config(document, Config::load()?);

    let mut session = TerminalSession::enter(stdin, stdout)?;
    session.draw(&mut editor)?;
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
                EditorOutcome::Continue => self.draw(&mut editor)?,
            }
        }
    }

    fn draw(&mut self, editor: &mut Editor) -> Result<()> {
        let size = terminal_size(self.output_fd)?;
        draw_editor_frame(&mut self.screen.terminal, editor, size)
    }
}

fn draw_editor_frame<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    editor: &mut Editor,
    size: TerminalSize,
) -> Result<()> {
    terminal.move_cursor(1, 1)?;
    terminal.clear_screen()?;

    let window_rows = usize::from(size.rows.saturating_sub(1).max(1));
    let layouts = editor.window_layouts(window_rows, usize::from(size.columns.max(1)));
    ensure_current_window_visible(editor, &layouts)?;
    for layout in &layouts {
        draw_window(terminal, editor, *layout)?;
    }

    terminal.move_cursor(size.rows.max(1), 1)?;
    terminal.clear_line()?;
    if let Some(text) = editor.minibuffer().display_text() {
        let face = if text.starts_with("Error:") {
            Face::Error
        } else {
            Face::Minibuffer
        };
        write_text_with_face(terminal, &text, face, editor.theme())?;
    }

    move_cursor_to_current_window(terminal, editor, &layouts)?;
    terminal.show_cursor()?;
    terminal.flush()
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
    let cursor_display_column =
        cursor_absolute_display_column(document.buffer(), editor.cursor(), editor.tab_width())?;
    editor.ensure_current_window_contains_cursor(text_rows, text_columns, cursor_display_column);
    Ok(())
}

fn draw_window<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    editor: &Editor,
    layout: WindowLayout,
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
    for row in 0..text_rows {
        let screen_row = layout.rect.row + row + 1;
        let screen_column = layout.rect.column + 1;
        terminal.move_cursor(screen_row as u16, screen_column as u16)?;
        let line_index = viewport.first_visible_line + row;
        let gutter_width = line_number_gutter_width(editor, document.buffer());
        if gutter_width > 0 {
            write_line_number_gutter(terminal, line_index, gutter_width, editor.theme())?;
        }
        let text_columns = layout.rect.columns.saturating_sub(gutter_width);
        if text_columns == 0 {
            continue;
        }
        if let Some(line) = document.buffer().line(line_index) {
            let spans = editor.spans_for_buffer_line(viewport.buffer, line_index, line);
            write_buffer_line(
                terminal,
                document.buffer(),
                viewport,
                line_index,
                line,
                &spans,
                LineRenderOptions {
                    width: text_columns,
                    tab_width: editor.tab_width(),
                    theme: editor.theme(),
                },
            )?;
        } else {
            write_fixed_width_text(terminal, "", text_columns)?;
        }
    }

    let mode_line_row = layout.rect.row + layout.rect.rows;
    terminal.move_cursor(mode_line_row as u16, (layout.rect.column + 1) as u16)?;
    let major_mode = editor.major_mode_for_buffer(viewport.buffer).name();
    let position = mode_line_position(document.buffer(), viewport, text_rows, editor.tab_width())?;
    let mode_line = format!(
        "{}{}   {position}   ({major_mode})",
        if layout.id == editor.current_window_id() {
            "* "
        } else {
            "  "
        },
        document.mode_line()
    );
    write_fixed_width_text_with_face(
        terminal,
        &mode_line,
        layout.rect.columns,
        Face::ModeLine,
        editor.theme(),
    )
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
}

fn write_buffer_line<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    buffer: &Buffer,
    viewport: &Viewport,
    line_index: usize,
    line: &str,
    spans: &[Span],
    options: LineRenderOptions,
) -> Result<()> {
    let range = buffer.visible_range(line_index, viewport.first_visible_column, options.width)?;
    let segment = &line[range.clone()];
    let relative_spans = clip_spans(spans, range);
    let used_width = write_line_with_spans(
        terminal,
        segment,
        &relative_spans,
        options.tab_width,
        options.theme,
    )?;
    if used_width < options.width {
        terminal.write_text(&" ".repeat(options.width - used_width))?;
    }
    Ok(())
}

fn write_fixed_width_text<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    text: &str,
    width: usize,
) -> Result<()> {
    let clipped: String = text.chars().take(width).collect();
    terminal.write_text(&clipped)?;
    let used = clipped.chars().count();
    if used < width {
        terminal.write_text(&" ".repeat(width - used))?;
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
    let clipped: String = text.chars().take(width).collect();
    let used = clipped.chars().count();
    write_text_with_face(terminal, &clipped, face, theme)?;
    if used < width {
        terminal.write_text(&" ".repeat(width - used))?;
    }
    Ok(())
}

fn write_text_with_face<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    text: &str,
    face: Face,
    theme: ThemeName,
) -> Result<()> {
    if let Some(start_code) = face_start_code(face, theme) {
        terminal.write_text(start_code)?;
        terminal.write_text(text)?;
        terminal.write_text("\x1b[0m")?;
    } else {
        terminal.write_text(text)?;
    }
    Ok(())
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
    let cursor_row = cursor
        .line
        .saturating_sub(viewport.first_visible_line)
        .min(text_rows - 1);
    let gutter_width = line_number_gutter_width(editor, document.buffer());
    let cursor_column = (gutter_width
        + cursor_display_column(document.buffer(), viewport, cursor, editor.tab_width())?)
    .min(layout.rect.columns.saturating_sub(1));
    terminal.move_cursor(
        (layout.rect.row + cursor_row + 1) as u16,
        (layout.rect.column + cursor_column + 1) as u16,
    )
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

fn write_line_with_spans<W: Write>(
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

fn write_text_with_face_expanded<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    text: &str,
    face: Face,
    tab_width: usize,
    column: usize,
    theme: ThemeName,
) -> Result<usize> {
    if let Some(start_code) = face_start_code(face, theme) {
        terminal.write_text(start_code)?;
        let column = write_display_text(terminal, text, tab_width, column)?;
        terminal.write_text("\x1b[0m")?;
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
            terminal.write_text(&" ".repeat(spaces))?;
            column += spaces;
        } else {
            terminal.write_text(&character.to_string())?;
            column += character.width().unwrap_or(0);
        }
    }
    Ok(column)
}

fn display_width_with_tabs(text: &str, tab_width: usize) -> usize {
    text.chars().fold(0, |column, character| {
        if character == '\t' {
            column + tab_spaces(tab_width, column)
        } else {
            column + character.width().unwrap_or(0)
        }
    })
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
            Face::SyntaxKeyword => Some("\x1b[34;1m"),
            Face::SyntaxString => Some("\x1b[32m"),
            Face::SyntaxComment => Some("\x1b[2m"),
            _ => None,
        },
        ThemeName::Mono => match face {
            Face::CurrentSearchMatch | Face::Region | Face::ModeLine => Some("\x1b[7m"),
            Face::SearchMatch => Some("\x1b[4m"),
            Face::Minibuffer | Face::LineNumber | Face::SyntaxComment => Some("\x1b[2m"),
            Face::Error | Face::Warning => Some("\x1b[1m"),
            _ => None,
        },
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
    use super::{
        AnsiTerminal, TerminalSize, draw_editor_frame, mode_line_position,
        write_fixed_width_text_with_face, write_line_number_gutter, write_line_with_spans,
    };
    use crate::buffer::{Buffer, BufferId, Position};
    use crate::config::ThemeName;
    use crate::editor::Editor;
    use crate::file::Document;
    use crate::input::KeyEvent;
    use crate::render::{Face, Span};
    use crate::window::Viewport;

    #[test]
    fn writes_buffered_ansi_sequences() {
        let mut terminal = AnsiTerminal::new(Vec::new());
        terminal.hide_cursor().expect("hide cursor should write");
        terminal.move_cursor(2, 3).expect("move should write");
        terminal.clear_line().expect("clear line should write");
        terminal.write_text("status").expect("text should write");
        terminal.show_cursor().expect("show cursor should write");

        assert_eq!(
            terminal.into_inner(),
            b"\x1b[?25l\x1b[2;3H\x1b[2Kstatus\x1b[?25h".to_vec()
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

    fn rendered_cursor_position(editor: &mut Editor, size: TerminalSize) -> Option<(usize, usize)> {
        let mut terminal = AnsiTerminal::new(Vec::new());
        draw_editor_frame(&mut terminal, editor, size).expect("frame should draw");
        last_cursor_position(&terminal.into_inner())
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
