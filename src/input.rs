// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::Read;

use crate::{Result, RileError};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyEvent {
    Ctrl(char),
    Meta(char),
    MetaSpecial(SpecialKey),
    Text(String),
    Special(SpecialKey),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecialKey {
    Backspace,
    Delete,
    Enter,
    Tab,
    Escape,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Home,
    End,
    PageUp,
    PageDown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedKey {
    pub event: KeyEvent,
    pub consumed: usize,
}

pub struct KeyReader<R> {
    reader: R,
    buffer: Vec<u8>,
}

impl<R: Read> KeyReader<R> {
    pub fn new(reader: R) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
        }
    }

    pub fn read_key(&mut self) -> Result<KeyEvent> {
        loop {
            if let Some(parsed) = parse_key_sequence(&self.buffer)? {
                self.buffer.drain(..parsed.consumed);
                return Ok(parsed.event);
            }

            let mut byte = [0];
            match self.reader.read(&mut byte) {
                Ok(0) if self.buffer == [0x1b] => {
                    self.buffer.clear();
                    return Ok(KeyEvent::Special(SpecialKey::Escape));
                }
                Ok(0) => continue,
                Ok(_) => self.buffer.push(byte[0]),
                Err(error) => return Err(error.into()),
            }
        }
    }
}

pub fn parse_key_sequence(bytes: &[u8]) -> Result<Option<ParsedKey>> {
    let Some(&first) = bytes.first() else {
        return Ok(None);
    };

    let event = match first {
        b'\r' | b'\n' => KeyEvent::Special(SpecialKey::Enter),
        b'\t' => KeyEvent::Special(SpecialKey::Tab),
        0x7f | 0x08 => KeyEvent::Special(SpecialKey::Backspace),
        0x00 => KeyEvent::Ctrl('@'),
        0x01..=0x1a => KeyEvent::Ctrl((b'a' + first - 1) as char),
        0x1f => KeyEvent::Ctrl('_'),
        0x1b => return parse_escape_sequence(bytes),
        0x20..=0x7e => KeyEvent::Text(char::from(first).to_string()),
        0x80..=0xff => return parse_utf8_text(bytes),
        _ => {
            return Err(RileError::InvalidInput(format!(
                "unsupported control byte 0x{first:02x}"
            )));
        }
    };

    Ok(Some(ParsedKey { event, consumed: 1 }))
}

fn parse_escape_sequence(bytes: &[u8]) -> Result<Option<ParsedKey>> {
    if bytes.len() == 1 {
        return Ok(None);
    }

    let second = bytes[1];
    if second == b'[' {
        return parse_csi_sequence(bytes);
    }
    if second == b'O' {
        return parse_ss3_sequence(bytes);
    }

    if second.is_ascii() {
        let key = match second {
            b'\r' | b'\n' => KeyEvent::Special(SpecialKey::Enter),
            b'\t' => KeyEvent::Special(SpecialKey::Tab),
            0x7f | 0x08 => KeyEvent::MetaSpecial(SpecialKey::Backspace),
            0x01..=0x1a => KeyEvent::Ctrl((b'a' + second - 1) as char),
            0x20..=0x7e => KeyEvent::Meta(char::from(second)),
            _ => KeyEvent::Special(SpecialKey::Escape),
        };
        let consumed = if matches!(key, KeyEvent::Special(SpecialKey::Escape)) {
            1
        } else {
            2
        };
        return Ok(Some(ParsedKey {
            event: key,
            consumed,
        }));
    }

    if let Some(ParsedKey {
        event: KeyEvent::Text(text),
        consumed,
    }) = parse_utf8_text(&bytes[1..])?
    {
        let Some(meta) = text.chars().next() else {
            return Ok(None);
        };
        return Ok(Some(ParsedKey {
            event: KeyEvent::Meta(meta),
            consumed: consumed + 1,
        }));
    }

    Ok(None)
}

fn parse_csi_sequence(bytes: &[u8]) -> Result<Option<ParsedKey>> {
    if bytes.len() < 3 {
        return Ok(None);
    }

    let event = match bytes[2] {
        b'A' => Some((KeyEvent::Special(SpecialKey::ArrowUp), 3)),
        b'B' => Some((KeyEvent::Special(SpecialKey::ArrowDown), 3)),
        b'C' => Some((KeyEvent::Special(SpecialKey::ArrowRight), 3)),
        b'D' => Some((KeyEvent::Special(SpecialKey::ArrowLeft), 3)),
        b'H' => Some((KeyEvent::Special(SpecialKey::Home), 3)),
        b'F' => Some((KeyEvent::Special(SpecialKey::End), 3)),
        b'1' | b'7' => parse_tilde_sequence(bytes, SpecialKey::Home)?,
        b'3' => parse_tilde_sequence(bytes, SpecialKey::Delete)?,
        b'4' | b'8' => parse_tilde_sequence(bytes, SpecialKey::End)?,
        b'5' => parse_tilde_sequence(bytes, SpecialKey::PageUp)?,
        b'6' => parse_tilde_sequence(bytes, SpecialKey::PageDown)?,
        _ => Some((KeyEvent::Special(SpecialKey::Escape), 1)),
    };

    Ok(event.map(|(event, consumed)| ParsedKey { event, consumed }))
}

fn parse_ss3_sequence(bytes: &[u8]) -> Result<Option<ParsedKey>> {
    if bytes.len() < 3 {
        return Ok(None);
    }

    let event = match bytes[2] {
        b'H' => KeyEvent::Special(SpecialKey::Home),
        b'F' => KeyEvent::Special(SpecialKey::End),
        b'A' => KeyEvent::Special(SpecialKey::ArrowUp),
        b'B' => KeyEvent::Special(SpecialKey::ArrowDown),
        b'C' => KeyEvent::Special(SpecialKey::ArrowRight),
        b'D' => KeyEvent::Special(SpecialKey::ArrowLeft),
        _ => KeyEvent::Special(SpecialKey::Escape),
    };
    let consumed = if matches!(event, KeyEvent::Special(SpecialKey::Escape)) {
        1
    } else {
        3
    };

    Ok(Some(ParsedKey { event, consumed }))
}

fn parse_tilde_sequence(bytes: &[u8], key: SpecialKey) -> Result<Option<(KeyEvent, usize)>> {
    if bytes.len() < 4 {
        return Ok(None);
    }
    if bytes[3] != b'~' {
        return Ok(Some((KeyEvent::Special(SpecialKey::Escape), 1)));
    }
    Ok(Some((KeyEvent::Special(key), 4)))
}

fn parse_utf8_text(bytes: &[u8]) -> Result<Option<ParsedKey>> {
    let Some(length) = utf8_char_width(bytes[0]) else {
        return Err(RileError::InvalidInput(format!(
            "invalid UTF-8 start byte 0x{:02x}",
            bytes[0]
        )));
    };

    if bytes.len() < length {
        return Ok(None);
    }

    let text = std::str::from_utf8(&bytes[..length])
        .map_err(|error| RileError::InvalidInput(format!("invalid UTF-8 key input: {error}")))?;

    Ok(Some(ParsedKey {
        event: KeyEvent::Text(text.to_owned()),
        consumed: length,
    }))
}

fn utf8_char_width(byte: u8) -> Option<usize> {
    match byte {
        0x00..=0x7f => Some(1),
        0xc2..=0xdf => Some(2),
        0xe0..=0xef => Some(3),
        0xf0..=0xf4 => Some(4),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{KeyEvent, ParsedKey, SpecialKey, parse_key_sequence};

    fn parse(bytes: &[u8]) -> ParsedKey {
        parse_key_sequence(bytes)
            .expect("sequence should be valid")
            .expect("sequence should be complete")
    }

    #[test]
    fn parses_ctrl_key() {
        assert_eq!(parse(&[0x00]).event, KeyEvent::Ctrl('@'));
        assert_eq!(parse(&[0x01]).event, KeyEvent::Ctrl('a'));
        assert_eq!(parse(&[0x1a]).event, KeyEvent::Ctrl('z'));
        assert_eq!(parse(&[0x1f]).event, KeyEvent::Ctrl('_'));
    }

    #[test]
    fn parses_printable_utf8() {
        assert_eq!(parse("é".as_bytes()).event, KeyEvent::Text("é".into()));
    }

    #[test]
    fn parses_meta_key() {
        assert_eq!(parse(b"\x1bf").event, KeyEvent::Meta('f'));
        assert_eq!(parse("\u{1b}é".as_bytes()).event, KeyEvent::Meta('é'));
        assert_eq!(
            parse(b"\x1b\x7f").event,
            KeyEvent::MetaSpecial(SpecialKey::Backspace)
        );
        assert_eq!(
            parse(b"\x1b\x08").event,
            KeyEvent::MetaSpecial(SpecialKey::Backspace)
        );
    }

    #[test]
    fn parses_special_single_bytes() {
        assert_eq!(parse(b"\r").event, KeyEvent::Special(SpecialKey::Enter));
        assert_eq!(parse(b"\t").event, KeyEvent::Special(SpecialKey::Tab));
        assert_eq!(
            parse(&[0x7f]).event,
            KeyEvent::Special(SpecialKey::Backspace)
        );
    }

    #[test]
    fn parses_arrows_and_navigation() {
        assert_eq!(
            parse(b"\x1b[A").event,
            KeyEvent::Special(SpecialKey::ArrowUp)
        );
        assert_eq!(
            parse(b"\x1b[B").event,
            KeyEvent::Special(SpecialKey::ArrowDown)
        );
        assert_eq!(
            parse(b"\x1b[C").event,
            KeyEvent::Special(SpecialKey::ArrowRight)
        );
        assert_eq!(
            parse(b"\x1b[D").event,
            KeyEvent::Special(SpecialKey::ArrowLeft)
        );
        assert_eq!(parse(b"\x1b[H").event, KeyEvent::Special(SpecialKey::Home));
        assert_eq!(parse(b"\x1b[F").event, KeyEvent::Special(SpecialKey::End));
        assert_eq!(
            parse(b"\x1b[5~").event,
            KeyEvent::Special(SpecialKey::PageUp)
        );
        assert_eq!(
            parse(b"\x1b[6~").event,
            KeyEvent::Special(SpecialKey::PageDown)
        );
        assert_eq!(
            parse(b"\x1b[3~").event,
            KeyEvent::Special(SpecialKey::Delete)
        );
    }

    #[test]
    fn reports_incomplete_escape_sequences() {
        assert_eq!(parse_key_sequence(b"\x1b").expect("valid"), None);
        assert_eq!(parse_key_sequence(b"\x1b[").expect("valid"), None);
    }
}
