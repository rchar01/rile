// SPDX-FileCopyrightText: 2026 Rile contributors
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::buffer::Buffer;
use crate::{Result, RileError};

static SAVE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    buffer: Buffer,
    path: Option<PathBuf>,
    name: Option<String>,
    missing_on_open: bool,
}

impl Document {
    pub fn scratch() -> Self {
        Self {
            buffer: Buffer::new(),
            path: None,
            name: None,
            missing_on_open: false,
        }
    }

    pub fn welcome() -> Self {
        Self {
            buffer: Buffer::from_text(
                "Welcome to Rile.\n\n\
C-x C-f  Find file    C-x C-s  Save buffer    C-x C-c  Quit\n\
C-s      Search       M-%      Query replace  M-x      Command\n\n\
Rile is free software under GPL-3.0-or-later.\n",
            ),
            path: None,
            name: Some("*Rile*".to_owned()),
            missing_on_open: false,
        }
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        match fs::read(&path) {
            Ok(bytes) => {
                if bytes.contains(&0) {
                    return Err(RileError::InvalidInput(format!(
                        "{} appears to be a binary file and was not opened",
                        path.display()
                    )));
                }

                let text = String::from_utf8(bytes).map_err(|error| {
                    RileError::InvalidInput(format!(
                        "{} is not valid UTF-8: {}",
                        path.display(),
                        error.utf8_error()
                    ))
                })?;
                Ok(Self {
                    buffer: Buffer::from_text(&text),
                    path: Some(path),
                    name: None,
                    missing_on_open: false,
                })
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                buffer: Buffer::new(),
                path: Some(path),
                name: None,
                missing_on_open: true,
            }),
            Err(error) => Err(error.into()),
        }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn display_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| {
            self.path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "*scratch*".to_owned())
        })
    }

    pub fn is_dirty(&self) -> bool {
        self.buffer.is_dirty()
    }

    pub fn missing_on_open(&self) -> bool {
        self.missing_on_open
    }

    pub fn save(&mut self) -> Result<()> {
        let Some(path) = self.path.clone() else {
            return Err(RileError::InvalidInput(
                "cannot save unnamed buffer without a path".to_owned(),
            ));
        };
        self.write_to_path(&path)
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref().to_path_buf();
        self.write_to_path(&path)?;
        self.path = Some(path);
        self.name = None;
        self.missing_on_open = false;
        Ok(())
    }

    pub fn mode_line(&self) -> String {
        let dirty = if self.is_dirty() { "**" } else { "--" };
        let newline = if self.buffer.final_newline() {
            "LF"
        } else {
            "noeol"
        };
        let missing = if self.missing_on_open { " new" } else { "" };
        format!("{dirty} {} [{newline}{missing}]", self.display_name())
    }

    fn write_to_path(&mut self, path: &Path) -> Result<()> {
        safe_write(path, self.buffer.serialize().as_bytes())?;
        self.buffer.mark_clean();
        self.missing_on_open = false;
        Ok(())
    }
}

pub fn safe_write(path: &Path, bytes: &[u8]) -> Result<()> {
    let temporary = temporary_path(path);
    let write_result = write_temporary_then_rename(&temporary, path, bytes);
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result
}

fn write_temporary_then_rename(temporary: &Path, path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(temporary, path)?;
    sync_parent_directory(path);
    Ok(())
}

fn temporary_path(path: &Path) -> PathBuf {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "rile-buffer".into());
    let counter = SAVE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temporary_name = format!(".{file_name}.rile-tmp-{}-{counter}", std::process::id());

    match parent {
        Some(parent) => parent.join(temporary_name),
        None => PathBuf::from(temporary_name),
    }
}

fn sync_parent_directory(path: &Path) {
    let Some(parent) = path.parent() else {
        return;
    };
    if let Ok(directory) = fs::File::open(parent) {
        let _ = directory.sync_all();
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::Document;
    use crate::buffer::Position;

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let counter = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path =
                std::env::temp_dir().join(format!("rile-test-{}-{counter}", std::process::id()));
            fs::create_dir(&path).expect("test directory should be created");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn opens_utf8_file_into_clean_buffer() {
        let directory = TestDir::new();
        let path = directory.path().join("notes.txt");
        fs::write(&path, "hello\nκόσμε\n").expect("file should be written");

        let document = Document::open(&path).expect("file should open");

        assert_eq!(document.buffer().serialize(), "hello\nκόσμε\n");
        assert!(document.buffer().final_newline());
        assert!(!document.is_dirty());
        assert!(!document.missing_on_open());
    }

    #[test]
    fn missing_file_creates_named_clean_buffer() {
        let directory = TestDir::new();
        let path = directory.path().join("new.txt");

        let document = Document::open(&path).expect("missing file should create buffer");

        assert_eq!(document.path(), Some(path.as_path()));
        assert_eq!(document.buffer().serialize(), "");
        assert!(!document.is_dirty());
        assert!(document.missing_on_open());
    }

    #[test]
    fn welcome_document_is_named_and_clean() {
        let directory = TestDir::new();
        let path = directory.path().join("welcome.txt");
        let mut document = Document::welcome();

        assert_eq!(document.display_name(), "*Rile*");
        assert!(document.buffer().serialize().contains("Welcome to Rile."));
        assert!(!document.is_dirty());

        document
            .save_as(&path)
            .expect("welcome should save as file");
        assert_eq!(document.path(), Some(path.as_path()));
        assert_eq!(document.display_name(), path.display().to_string());
    }

    #[test]
    fn opens_empty_file() {
        let directory = TestDir::new();
        let path = directory.path().join("empty.txt");
        fs::write(&path, "").expect("file should be written");

        let document = Document::open(&path).expect("empty file should open");

        assert_eq!(document.buffer().line_count(), 1);
        assert_eq!(document.buffer().serialize(), "");
        assert!(!document.buffer().final_newline());
        assert!(!document.is_dirty());
    }

    #[test]
    fn save_current_buffer_writes_safely_and_marks_clean() {
        let directory = TestDir::new();
        let path = directory.path().join("save.txt");
        fs::write(&path, "old").expect("file should be written");
        let mut document = Document::open(&path).expect("file should open");

        document
            .buffer_mut()
            .insert(Position::new(0, 3), "\nnew")
            .expect("insert should succeed");
        assert!(document.is_dirty());

        document.save().expect("save should succeed");

        assert_eq!(
            fs::read_to_string(&path).expect("file should read"),
            "old\nnew"
        );
        assert!(!document.is_dirty());
        assert!(!document.missing_on_open());
    }

    #[test]
    fn save_as_sets_new_path_and_preserves_no_final_newline() {
        let directory = TestDir::new();
        let path = directory.path().join("written.txt");
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "no newline")
            .expect("insert should succeed");

        document.save_as(&path).expect("save-as should succeed");

        assert_eq!(document.path(), Some(path.as_path()));
        assert_eq!(
            fs::read_to_string(&path).expect("file should read"),
            "no newline"
        );
        assert!(!document.buffer().final_newline());
        assert!(!document.is_dirty());
    }

    #[test]
    fn rejects_invalid_utf8_without_lossy_conversion() {
        let directory = TestDir::new();
        let path = directory.path().join("binary.dat");
        fs::write(&path, [0xff, 0xfe, b'a']).expect("file should be written");

        let error = Document::open(&path).expect_err("invalid UTF-8 should fail");

        assert!(error.to_string().contains("not valid UTF-8"));
    }

    #[test]
    fn rejects_nul_containing_binary_files() {
        let directory = TestDir::new();
        let path = directory.path().join("binary.dat");
        fs::write(&path, b"text\0more").expect("file should be written");

        let error = Document::open(&path).expect_err("binary file should fail");

        assert!(error.to_string().contains("appears to be a binary file"));
    }

    #[test]
    fn reports_directory_open_errors() {
        let directory = TestDir::new();

        let error =
            Document::open(directory.path()).expect_err("directory should not open as file");

        assert!(error.to_string().contains("I/O error"));
    }

    #[test]
    fn save_to_directory_path_fails_and_keeps_buffer_dirty() {
        let directory = TestDir::new();
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "text")
            .expect("insert should succeed");

        let error = document
            .save_as(directory.path())
            .expect_err("saving over a directory should fail");

        assert!(error.to_string().contains("I/O error"));
        assert!(document.is_dirty());
        assert_eq!(document.path(), None);
    }

    #[test]
    fn save_to_missing_parent_fails_and_keeps_buffer_dirty() {
        let directory = TestDir::new();
        let path = directory.path().join("missing-parent").join("file.txt");
        let mut document = Document::scratch();
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "text")
            .expect("insert should succeed");

        let error = document
            .save_as(&path)
            .expect_err("saving inside a missing parent should fail");

        assert!(error.to_string().contains("I/O error"));
        assert!(document.is_dirty());
        assert_eq!(document.path(), None);
    }

    #[test]
    fn preserves_crlf_bytes_when_round_tripping_current_policy() {
        let directory = TestDir::new();
        let path = directory.path().join("crlf.txt");
        fs::write(&path, "a\r\nb\r\n").expect("file should be written");
        let mut document = Document::open(&path).expect("file should open");

        document.save().expect("save should succeed");

        assert_eq!(
            fs::read_to_string(&path).expect("file should read"),
            "a\r\nb\r\n"
        );
    }
}
