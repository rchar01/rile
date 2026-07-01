// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::{self, IsTerminal, Read, Write};
use std::os::fd::AsRawFd;
use std::path::Path;

use crate::buffer::{Buffer, Position};
use crate::completion::CompletionStyle;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimeOptions<'a> {
    pub file: Option<&'a Path>,
    pub visual_test: bool,
    pub test_size: Option<TerminalSize>,
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

    pub fn write_text(&mut self, text: &str) -> Result<()> {
        self.writer.write_all(text.as_bytes())?;
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
    _raw_mode: RawModeGuard,
    input: KeyReader<R>,
    output_fd: libc::c_int,
    test_size: Option<TerminalSize>,
    frame_options: FrameOptions,
    last_size: Option<TerminalSize>,
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
            _raw_mode: raw_mode,
            input: KeyReader::with_erase_byte(input, erase_byte),
            output_fd,
            test_size: options.test_size,
            frame_options: FrameOptions {
                visual_test: options.visual_test,
                clear_screen: false,
            },
            last_size: None,
        })
    }

    fn run(&mut self, mut editor: Editor) -> Result<()> {
        loop {
            if let Some(key) = self.input.read_key_or_timeout()? {
                match editor.handle_key(key)? {
                    EditorOutcome::Quit => return Ok(()),
                    EditorOutcome::Continue => self.draw(&mut editor)?,
                }
            } else if editor.poll_auto_revert()? {
                self.draw(&mut editor)?;
            }
        }
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
    let layouts = editor.window_layouts(window_rows, usize::from(size.columns.max(1)));
    for layout in &layouts {
        editor.set_window_text_rows(layout.id, layout.rect.rows.saturating_sub(1));
    }
    ensure_current_window_visible(editor, &layouts)?;
    for layout in &layouts {
        draw_window(terminal, editor, *layout, options)?;
    }

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

fn draw_minibuffer<W: Write>(
    terminal: &mut AnsiTerminal<W>,
    editor: &Editor,
    size: TerminalSize,
    row: usize,
) -> Result<()> {
    terminal.move_cursor(row as u16, 1)?;
    terminal.clear_line()?;
    let columns = usize::from(size.columns.max(1));
    if let Some((text, _)) = minibuffer_visible_text_and_cursor(editor, columns) {
        let face = if editor
            .minibuffer_display_text()
            .is_some_and(|text| text.starts_with("Error:"))
        {
            Face::Error
        } else {
            Face::Minibuffer
        };
        write_fixed_width_text_with_face(terminal, &text, columns, face, editor.theme())?;
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
        write_fixed_width_text_with_face(
            terminal,
            "No match",
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
    minibuffer_visible_text_and_cursor(editor, columns).and_then(|(_, cursor)| cursor)
}

fn minibuffer_visible_text_and_cursor(
    editor: &Editor,
    columns: usize,
) -> Option<(String, Option<usize>)> {
    let text = editor.minibuffer_display_text()?;
    let cursor = raw_minibuffer_cursor_column(editor);
    let Some(cursor) = cursor else {
        return Some((text, None));
    };
    if columns == 0 || cursor < columns {
        return Some((text, Some(cursor)));
    }
    let start_column = cursor + 1 - columns;
    Some((
        text_from_display_column(&text, start_column),
        Some(columns - 1),
    ))
}

fn raw_minibuffer_cursor_column(editor: &Editor) -> Option<usize> {
    let prompt = editor.minibuffer().prompt()?;
    let input_before_cursor = editor.minibuffer().prompt_input_before_cursor()?;
    let prompt_column = text_display_width(&prompt.label) + text_display_width(input_before_cursor);
    let Some(completion) = editor.completion() else {
        return Some(prompt_column);
    };
    if completion.style() == CompletionStyle::Vertical {
        let selected = completion.selected_match_number().unwrap_or(0);
        let prefix = format!("{selected}/{}  ", completion.match_count());
        return Some(text_display_width(&prefix) + prompt_column);
    }
    Some(prompt_column)
}

fn text_from_display_column(text: &str, start_column: usize) -> String {
    let mut column = 0;
    let mut start_byte = text.len();
    for (byte, character) in text.char_indices() {
        let next_column = column + character.width().unwrap_or(0);
        if next_column > start_column {
            start_byte = byte;
            break;
        }
        column = next_column;
    }
    text[start_byte..].to_owned()
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
    text.chars()
        .map(|character| character.width().unwrap_or(0))
        .sum()
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
        .map(|item| item.candidate.display_label().chars().count())
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
    let padding = candidate_width.saturating_sub(candidate.chars().count()) + description_gap;
    let mut line = candidate;
    line.push_str(&" ".repeat(padding));
    if line.chars().count() < columns {
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
    let content_width = wrapped_help_content_width(options.width, visual_line.continued);
    let range = buffer.visible_range(
        visual_line.line_index,
        visual_line.start_column,
        content_width,
    )?;
    let relative_spans = clip_spans(spans, range.clone());
    let segment = &line[range];
    let mut used_width = write_line_with_spans(
        terminal,
        segment,
        &relative_spans,
        options.tab_width,
        options.theme,
    )?;
    if visual_line.continued && options.width > 1 {
        let continuation_column = options.width.saturating_sub(1);
        if used_width < continuation_column {
            terminal.write_text(&" ".repeat(continuation_column - used_width))?;
        }
        terminal.write_text("\\")?;
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
            terminal.write_text(&padding)?;
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
    let name = visual_test_document_name(document);
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

fn visual_test_document_name(document: &Document) -> String {
    document
        .path()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| document.display_name())
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
        mode_line_document_name(document)
    ))
}

fn mode_line_document_name(document: &Document) -> String {
    document
        .path()
        .and_then(Path::file_name)
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| document.display_name())
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
    let line_width = display_width_with_tabs(line, options.tab_width);
    let left_hidden = viewport.first_visible_column > 0;
    let right_hidden = line_width > viewport.first_visible_column.saturating_add(options.width);
    let relative_spans = clip_spans(spans, range);
    let (segment, relative_spans) = if left_hidden || right_hidden {
        mark_hidden_line_edges(segment, &relative_spans, left_hidden, right_hidden)
    } else {
        (segment.to_owned(), relative_spans)
    };
    let used_width = write_line_with_spans(
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
            terminal.write_text(&padding)?;
        }
    }
    Ok(())
}

fn mark_hidden_line_edges(
    segment: &str,
    spans: &[Span],
    left_hidden: bool,
    right_hidden: bool,
) -> (String, Vec<Span>) {
    let character_count = segment.chars().count();
    if character_count == 0 {
        return ("$".to_owned(), Vec::new());
    }

    let mut marked = String::new();
    let mut byte_map = vec![(0, 0)];
    for (index, (byte, character)) in segment.char_indices().enumerate() {
        let replacement =
            (left_hidden && index == 0) || (right_hidden && index + 1 == character_count);
        let source_end = byte + character.len_utf8();
        let target_start = marked.len();
        marked.push(if replacement { '$' } else { character });
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
        .iter()
        .find_map(|(from, to)| (*from == source).then_some(*to))
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

fn clipped_text(text: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut characters = text.chars().collect::<Vec<_>>();
    if characters.len() <= width {
        return text.to_owned();
    }
    characters.truncate(width);
    let last = characters.len() - 1;
    characters[last] = '$';
    characters.into_iter().collect()
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
}

impl<W: Write> Drop for ScreenGuard<W> {
    fn drop(&mut self) {
        if self.active {
            if self.reset_cursor_style_on_drop {
                let _ = self.terminal.reset_cursor_style();
            }
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
        AnsiTerminal, FrameOptions, LineRenderOptions, TerminalSize, clipped_text,
        draw_editor_frame, draw_editor_frame_with_options, format_completion_row,
        mode_line_position, text_from_display_column, wrapped_help_cursor_position,
        wrapped_help_visual_lines, write_buffer_line, write_fixed_width_text_with_face,
        write_line_number_gutter, write_line_with_spans,
    };
    use crate::buffer::{Buffer, BufferId, Position};
    use crate::completion::{CompletionConfig, CompletionStyle};
    use crate::config::{Config, ThemeName};
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
    fn clipped_text_marks_hidden_right_edge() {
        assert_eq!(clipped_text("abcdef", 4), "abc$");
        assert_eq!(clipped_text("abcdef", 1), "$");
        assert_eq!(clipped_text("abc", 4), "abc");
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

    fn rendered_cursor_position(editor: &mut Editor, size: TerminalSize) -> Option<(usize, usize)> {
        last_cursor_position(rendered_frame_bytes(editor, size).as_slice())
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
