// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::io::Read;

use crate::{Result, RileError};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyEvent {
    Ctrl(char),
    CtrlSpecial(SpecialKey),
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
    erase_byte: u8,
}

impl<R: Read> KeyReader<R> {
    pub fn new(reader: R) -> Self {
        Self::with_erase_byte(reader, 0x7f)
    }

    pub fn with_erase_byte(reader: R, erase_byte: u8) -> Self {
        Self {
            reader,
            buffer: Vec::new(),
            erase_byte,
        }
    }

    pub fn read_key(&mut self) -> Result<KeyEvent> {
        loop {
            if let Some(key) = self.read_key_or_timeout()? {
                return Ok(key);
            }
        }
    }

    pub fn read_key_or_timeout(&mut self) -> Result<Option<KeyEvent>> {
        loop {
            if let Some(parsed) = parse_key_sequence_with_erase_byte(&self.buffer, self.erase_byte)?
            {
                self.buffer.drain(..parsed.consumed);
                return Ok(Some(parsed.event));
            }

            let mut byte = [0];
            match self.reader.read(&mut byte) {
                Ok(0) if self.buffer == [0x1b] => {
                    self.buffer.clear();
                    return Ok(Some(KeyEvent::Special(SpecialKey::Escape)));
                }
                Ok(0) if self.buffer.is_empty() => return Ok(None),
                Ok(0) => continue,
                Ok(_) => self.buffer.push(byte[0]),
                Err(error) => return Err(error.into()),
            }
        }
    }
}

pub fn parse_key_sequence(bytes: &[u8]) -> Result<Option<ParsedKey>> {
    parse_key_sequence_with_erase_byte(bytes, 0x7f)
}

pub fn parse_key_sequence_with_erase_byte(
    bytes: &[u8],
    erase_byte: u8,
) -> Result<Option<ParsedKey>> {
    let Some(&first) = bytes.first() else {
        return Ok(None);
    };

    let event = match first {
        b'\r' => KeyEvent::Special(SpecialKey::Enter),
        b'\n' => KeyEvent::Ctrl('j'),
        b'\t' => KeyEvent::Special(SpecialKey::Tab),
        0x7f => KeyEvent::Special(SpecialKey::Backspace),
        0x08 if erase_byte == 0x08 => KeyEvent::Special(SpecialKey::Backspace),
        0x08 => KeyEvent::Ctrl('h'),
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
            b'\r' | b'\n' => KeyEvent::MetaSpecial(SpecialKey::Enter),
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
    if let Some(parsed) = parse_csi_u_sequence(bytes) {
        return Ok(Some(parsed));
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

fn parse_csi_u_sequence(bytes: &[u8]) -> Option<ParsedKey> {
    let end = bytes.iter().position(|byte| *byte == b'u')?;
    if end < 3 {
        return Some(csi_u_fallback(end));
    }
    let Ok(sequence) = std::str::from_utf8(&bytes[2..end]) else {
        return Some(csi_u_fallback(end));
    };
    let mut parts = sequence.split(';');
    let Some(key_code) = parts.next().and_then(|part| part.parse::<u32>().ok()) else {
        return Some(csi_u_fallback(end));
    };
    let modifier = parts.next().and_then(|part| part.parse::<u32>().ok());
    let has_alt = modifier.is_some_and(|modifier| modifier > 1 && (modifier - 1) & 2 != 0);
    let has_ctrl = modifier.is_some_and(|modifier| modifier > 1 && (modifier - 1) & 4 != 0);
    csi_u_key_event(key_code, has_alt, has_ctrl)
        .map(|event| ParsedKey {
            event,
            consumed: end + 1,
        })
        .or_else(|| Some(csi_u_fallback(end)))
}

fn csi_u_key_event(key_code: u32, has_alt: bool, has_ctrl: bool) -> Option<KeyEvent> {
    if let Some(special) = csi_u_special_key(key_code) {
        return Some(if has_alt {
            KeyEvent::MetaSpecial(special)
        } else if has_ctrl {
            KeyEvent::CtrlSpecial(special)
        } else {
            KeyEvent::Special(special)
        });
    }
    if has_ctrl {
        return csi_u_ctrl_key(key_code).map(KeyEvent::Ctrl);
    }
    let character = char::from_u32(key_code)?;
    Some(if has_alt {
        KeyEvent::Meta(character)
    } else {
        KeyEvent::Text(character.to_string())
    })
}

fn csi_u_special_key(key_code: u32) -> Option<SpecialKey> {
    match key_code {
        127 => Some(SpecialKey::Backspace),
        9 => Some(SpecialKey::Tab),
        10 | 13 => Some(SpecialKey::Enter),
        27 => Some(SpecialKey::Escape),
        _ => None,
    }
}

fn csi_u_ctrl_key(key_code: u32) -> Option<char> {
    match char::from_u32(key_code)? {
        '@' | ' ' => Some('@'),
        '\u{8}' => Some('h'),
        '_' => Some('_'),
        character if character.is_ascii_alphabetic() => Some(character.to_ascii_lowercase()),
        _ => None,
    }
}

fn csi_u_fallback(end: usize) -> ParsedKey {
    ParsedKey {
        event: KeyEvent::Special(SpecialKey::Escape),
        consumed: end + 1,
    }
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
        if bytes[3..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || *byte == b';')
        {
            return Ok(None);
        }
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
    use super::parse_key_sequence_with_erase_byte;
    use super::{KeyEvent, ParsedKey, SpecialKey, parse_key_sequence};

    fn parse(bytes: &[u8]) -> ParsedKey {
        parse_key_sequence(bytes)
            .expect("sequence should be valid")
            .expect("sequence should be complete")
    }

    fn parse_with_erase_byte(bytes: &[u8], erase_byte: u8) -> ParsedKey {
        parse_key_sequence_with_erase_byte(bytes, erase_byte)
            .expect("sequence should be valid")
            .expect("sequence should be complete")
    }

    #[test]
    fn parses_ctrl_key() {
        assert_eq!(parse(&[0x00]).event, KeyEvent::Ctrl('@'));
        assert_eq!(parse(&[0x01]).event, KeyEvent::Ctrl('a'));
        assert_eq!(parse(&[0x0a]).event, KeyEvent::Ctrl('j'));
        assert_eq!(parse(&[0x1a]).event, KeyEvent::Ctrl('z'));
        assert_eq!(parse(&[0x08]).event, KeyEvent::Ctrl('h'));
        assert_eq!(parse(&[0x1f]).event, KeyEvent::Ctrl('_'));
    }

    #[test]
    fn parses_printable_utf8() {
        assert_eq!(parse("é".as_bytes()).event, KeyEvent::Text("é".into()));
    }

    #[test]
    fn parses_meta_key() {
        assert_eq!(parse(b"\x1bf").event, KeyEvent::Meta('f'));
        assert_eq!(parse(b"\x1b!").event, KeyEvent::Meta('!'));
        assert_eq!(parse(b"\x1b|").event, KeyEvent::Meta('|'));
        assert_eq!(parse("\u{1b}é".as_bytes()).event, KeyEvent::Meta('é'));
        assert_eq!(
            parse(b"\x1b\x7f").event,
            KeyEvent::MetaSpecial(SpecialKey::Backspace)
        );
        assert_eq!(
            parse(b"\x1b\x08").event,
            KeyEvent::MetaSpecial(SpecialKey::Backspace)
        );
        assert_eq!(
            parse(b"\x1b\r").event,
            KeyEvent::MetaSpecial(SpecialKey::Enter)
        );
    }

    #[test]
    fn parses_csi_u_keys() {
        assert_eq!(
            parse(b"\x1b[13u").event,
            KeyEvent::Special(SpecialKey::Enter)
        );
        assert_eq!(
            parse(b"\x1b[10u").event,
            KeyEvent::Special(SpecialKey::Enter)
        );
        assert_eq!(
            parse(b"\x1b[13;3u").event,
            KeyEvent::MetaSpecial(SpecialKey::Enter)
        );
        assert_eq!(
            parse(b"\x1b[127;5u").event,
            KeyEvent::CtrlSpecial(SpecialKey::Backspace)
        );
        assert_eq!(parse(b"\x1b[8;5u").event, KeyEvent::Ctrl('h'));
        assert_eq!(parse(b"\x1b[102;3u").event, KeyEvent::Meta('f'));
        assert_eq!(parse(b"\x1b[65;5u").event, KeyEvent::Ctrl('a'));
        let fallback = parse(b"\x1b[x;5u");
        assert_eq!(fallback.event, KeyEvent::Special(SpecialKey::Escape));
        assert_eq!(fallback.consumed, b"\x1b[x;5u".len());
    }

    #[test]
    fn parses_special_single_bytes() {
        assert_eq!(parse(b"\r").event, KeyEvent::Special(SpecialKey::Enter));
        assert_eq!(parse(b"\t").event, KeyEvent::Special(SpecialKey::Tab));
        assert_eq!(
            parse(&[0x7f]).event,
            KeyEvent::Special(SpecialKey::Backspace)
        );
        assert_eq!(
            parse_with_erase_byte(&[0x08], 0x08).event,
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
