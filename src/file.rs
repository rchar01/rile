// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::{self, Metadata, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use crate::buffer::Buffer;
use crate::{Result, RileError};

static SAVE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    Normal,
    Welcome,
    Help,
    Messages,
    Completions,
    BufferList,
    ShellOutput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileStamp {
    modified: Option<SystemTime>,
    len: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    buffer: Buffer,
    path: Option<PathBuf>,
    name: Option<String>,
    kind: DocumentKind,
    read_only: bool,
    missing_on_open: bool,
    backup_on_save: bool,
    backup_directory: Option<PathBuf>,
    backup_written: bool,
    file_stamp: Option<FileStamp>,
}

impl Document {
    pub fn scratch() -> Self {
        Self {
            buffer: Buffer::new(),
            path: None,
            name: None,
            kind: DocumentKind::Normal,
            read_only: false,
            missing_on_open: false,
            backup_on_save: false,
            backup_directory: None,
            backup_written: false,
            file_stamp: None,
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
            kind: DocumentKind::Welcome,
            read_only: false,
            missing_on_open: false,
            backup_on_save: false,
            backup_directory: None,
            backup_written: false,
            file_stamp: None,
        }
    }

    pub fn help(text: impl AsRef<str>) -> Self {
        Self {
            buffer: Buffer::from_text(text.as_ref()),
            path: None,
            name: Some("*Help*".to_owned()),
            kind: DocumentKind::Help,
            read_only: false,
            missing_on_open: false,
            backup_on_save: false,
            backup_directory: None,
            backup_written: false,
            file_stamp: None,
        }
    }

    pub fn completions(text: impl AsRef<str>) -> Self {
        Self {
            buffer: Buffer::from_text(text.as_ref()),
            path: None,
            name: Some("*Completions*".to_owned()),
            kind: DocumentKind::Completions,
            read_only: false,
            missing_on_open: false,
            backup_on_save: false,
            backup_directory: None,
            backup_written: false,
            file_stamp: None,
        }
    }

    pub fn messages(text: impl AsRef<str>) -> Self {
        Self {
            buffer: Buffer::from_text(text.as_ref()),
            path: None,
            name: Some("*Messages*".to_owned()),
            kind: DocumentKind::Messages,
            read_only: false,
            missing_on_open: false,
            backup_on_save: false,
            backup_directory: None,
            backup_written: false,
            file_stamp: None,
        }
    }

    pub fn buffer_list(text: impl AsRef<str>) -> Self {
        Self {
            buffer: Buffer::from_text(text.as_ref()),
            path: None,
            name: Some("*Buffer List*".to_owned()),
            kind: DocumentKind::BufferList,
            read_only: false,
            missing_on_open: false,
            backup_on_save: false,
            backup_directory: None,
            backup_written: false,
            file_stamp: None,
        }
    }

    pub fn shell_output(text: impl AsRef<str>) -> Self {
        Self {
            buffer: Buffer::from_text(text.as_ref()),
            path: None,
            name: Some("*Shell Command Output*".to_owned()),
            kind: DocumentKind::ShellOutput,
            read_only: false,
            missing_on_open: false,
            backup_on_save: false,
            backup_directory: None,
            backup_written: false,
            file_stamp: None,
        }
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_path_buf();
        match fs::read(&path) {
            Ok(bytes) => {
                let text = decode_text_file_bytes(&path, bytes)?;
                let file_stamp = file_stamp_from_path(&path)?;
                Ok(Self {
                    buffer: Buffer::from_text(&text),
                    path: Some(path),
                    name: None,
                    kind: DocumentKind::Normal,
                    read_only: false,
                    missing_on_open: false,
                    backup_on_save: false,
                    backup_directory: None,
                    backup_written: false,
                    file_stamp,
                })
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Self {
                buffer: Buffer::new(),
                path: Some(path),
                name: None,
                kind: DocumentKind::Normal,
                read_only: false,
                missing_on_open: true,
                backup_on_save: false,
                backup_directory: None,
                backup_written: false,
                file_stamp: None,
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

    pub fn kind(&self) -> DocumentKind {
        self.kind
    }

    pub fn is_read_only(&self) -> bool {
        self.kind != DocumentKind::Normal || self.read_only
    }

    pub fn set_read_only(&mut self, read_only: bool) {
        self.read_only = read_only;
    }

    pub fn is_help(&self) -> bool {
        self.kind == DocumentKind::Help
    }

    pub fn is_completions(&self) -> bool {
        self.kind == DocumentKind::Completions
    }

    pub fn is_messages(&self) -> bool {
        self.kind == DocumentKind::Messages
    }

    pub fn is_buffer_list(&self) -> bool {
        self.kind == DocumentKind::BufferList
    }

    pub fn is_shell_output(&self) -> bool {
        self.kind == DocumentKind::ShellOutput
    }

    pub fn is_dirty(&self) -> bool {
        self.buffer.is_dirty()
    }

    pub fn mark_clean(&mut self) {
        self.buffer.mark_clean();
    }

    pub fn missing_on_open(&self) -> bool {
        self.missing_on_open
    }

    pub fn backup_on_save(&self) -> bool {
        self.backup_on_save
    }

    pub fn set_backup_on_save(&mut self, enabled: bool) {
        self.backup_on_save = enabled;
    }

    pub fn backup_directory(&self) -> Option<&Path> {
        self.backup_directory.as_deref()
    }

    pub fn set_backup_directory(&mut self, directory: Option<PathBuf>) {
        self.backup_directory = directory;
    }

    pub fn file_changed_on_disk(&self) -> Result<bool> {
        if self.kind != DocumentKind::Normal || self.path.is_none() {
            return Ok(false);
        }
        let path = self.path.as_ref().expect("path checked above");
        let current = file_stamp_from_path(path)?;
        Ok(current.is_some() && current != self.file_stamp)
    }

    pub fn save(&mut self) -> Result<()> {
        if self.is_read_only() {
            return Err(RileError::InvalidInput("buffer is read-only".to_owned()));
        }
        let Some(path) = self.path.clone() else {
            return Err(RileError::InvalidInput(
                "cannot save unnamed buffer without a path".to_owned(),
            ));
        };
        self.write_to_path(&path)
    }

    pub fn save_as(&mut self, path: impl AsRef<Path>) -> Result<()> {
        if self.is_read_only() {
            return Err(RileError::InvalidInput("buffer is read-only".to_owned()));
        }
        let path = path.as_ref().to_path_buf();
        let previous_backup_written = self.backup_written;
        if self.path.as_ref() != Some(&path) {
            self.backup_written = false;
        }
        if let Err(error) = self.write_to_path(&path) {
            self.backup_written = previous_backup_written;
            return Err(error);
        }
        self.path = Some(path);
        self.name = None;
        self.missing_on_open = false;
        Ok(())
    }

    pub fn reload_from_disk(&mut self) -> Result<()> {
        if self.kind != DocumentKind::Normal {
            return Err(RileError::InvalidInput(
                "cannot revert a special buffer".to_owned(),
            ));
        }
        let Some(path) = self.path.clone() else {
            return Err(RileError::InvalidInput(
                "cannot revert unnamed buffer without a path".to_owned(),
            ));
        };
        let read_only = self.read_only;
        let backup_on_save = self.backup_on_save;
        let backup_directory = self.backup_directory.clone();
        let mut reloaded = Self::open(&path)?;
        reloaded.read_only = read_only;
        reloaded.backup_on_save = backup_on_save;
        reloaded.backup_directory = backup_directory;
        *self = reloaded;
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
        let read_only = if self.read_only { " RO" } else { "" };
        format!(
            "{dirty} {} [{newline}{missing}{read_only}]",
            self.display_name()
        )
    }

    fn write_to_path(&mut self, path: &Path) -> Result<()> {
        if self.backup_on_save && !self.backup_written {
            write_backup(path, self.backup_directory.as_deref())?;
            self.backup_written = true;
        }
        safe_write(path, self.buffer.serialize().as_bytes())?;
        self.buffer.mark_clean();
        self.missing_on_open = false;
        self.file_stamp = file_stamp_from_path(path)?;
        Ok(())
    }
}

pub fn read_text_file(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    decode_text_file_bytes(path, fs::read(path)?)
}

fn file_stamp_from_path(path: &Path) -> Result<Option<FileStamp>> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(Some(FileStamp::from_metadata(&metadata))),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

impl FileStamp {
    fn from_metadata(metadata: &Metadata) -> Self {
        Self {
            modified: metadata.modified().ok(),
            len: metadata.len(),
        }
    }
}

fn decode_text_file_bytes(path: &Path, bytes: Vec<u8>) -> Result<String> {
    if bytes.contains(&0) {
        return Err(RileError::InvalidInput(format!(
            "{} appears to be a binary file",
            path.display()
        )));
    }

    String::from_utf8(bytes).map_err(|error| {
        RileError::InvalidInput(format!(
            "{} is not valid UTF-8: {}",
            path.display(),
            error.utf8_error()
        ))
    })
}

pub fn safe_write(path: &Path, bytes: &[u8]) -> Result<()> {
    write_temporary_then_rename(path, bytes)
}

fn write_backup(path: &Path, backup_directory: Option<&Path>) -> Result<()> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => {
            let bytes = fs::read(path)?;
            safe_write_with_permissions(
                &backup_path(path, backup_directory),
                &bytes,
                Some(metadata.permissions()),
            )
        }
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn backup_path(path: &Path, backup_directory: Option<&Path>) -> PathBuf {
    match backup_directory {
        Some(directory) => directory.join(mapped_backup_name(path)),
        None => sibling_backup_path(path),
    }
}

fn sibling_backup_path(path: &Path) -> PathBuf {
    let mut backup = PathBuf::from(path);
    let mut file_name = path
        .file_name()
        .map(|name| name.to_os_string())
        .unwrap_or_else(|| "rile-buffer".into());
    file_name.push("~");
    backup.set_file_name(file_name);
    backup
}

fn mapped_backup_name(path: &Path) -> String {
    let mut name = String::new();
    for character in path.display().to_string().chars() {
        match character {
            '/' => name.push('!'),
            '!' => name.push_str("!!"),
            _ => name.push(character),
        }
    }
    if name.is_empty() {
        name.push_str("rile-buffer");
    }
    name.push('~');
    name
}

fn write_temporary_then_rename(path: &Path, bytes: &[u8]) -> Result<()> {
    let permissions = existing_file_permissions(path)?;
    safe_write_with_permissions(path, bytes, permissions)
}

fn safe_write_with_permissions(
    path: &Path,
    bytes: &[u8],
    permissions: Option<fs::Permissions>,
) -> Result<()> {
    let (temporary, mut file) = create_temporary_file(path)?;
    let result = (|| -> Result<()> {
        file.write_all(bytes)?;
        if let Some(permissions) = permissions {
            file.set_permissions(permissions)?;
        }
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, path)?;
        sync_parent_directory(path);
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn existing_file_permissions(path: &Path) -> Result<Option<fs::Permissions>> {
    match fs::metadata(path) {
        Ok(metadata) if metadata.is_file() => Ok(Some(metadata.permissions())),
        Ok(_) => Ok(None),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn create_temporary_file(path: &Path) -> Result<(PathBuf, fs::File)> {
    let mut last_error = None;
    for _ in 0..16 {
        let temporary = temporary_path(path);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                last_error = Some(error);
            }
            Err(error) => return Err(error.into()),
        }
    }
    Err(last_error
        .unwrap_or_else(|| std::io::Error::other("temporary path collision"))
        .into())
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

    use super::{Document, DocumentKind, SAVE_COUNTER, backup_path, safe_write, temporary_path};
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
    fn normal_document_can_be_marked_read_only() {
        let directory = TestDir::new();
        let path = directory.path().join("notes.txt");
        fs::write(&path, "hello\n").expect("file should be written");
        let mut document = Document::open(&path).expect("file should open");

        document.set_read_only(true);

        assert!(document.is_read_only());
        assert!(document.mode_line().contains("[LF RO]"));
        let error = document
            .save()
            .expect_err("read-only normal buffer should not save");
        assert!(error.to_string().contains("read-only"));
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
        let mut document = Document::welcome();

        assert_eq!(document.display_name(), "*Rile*");
        assert_eq!(document.kind(), DocumentKind::Welcome);
        assert!(document.is_read_only());
        assert!(document.buffer().serialize().contains("Welcome to Rile."));
        assert!(!document.is_dirty());

        let error = document
            .save()
            .expect_err("welcome buffer should be read-only");
        assert!(error.to_string().contains("read-only"));
    }

    #[test]
    fn help_document_is_named_clean_and_read_only() {
        let mut document = Document::help("Help text\n");

        assert_eq!(document.display_name(), "*Help*");
        assert_eq!(document.kind(), DocumentKind::Help);
        assert!(document.is_help());
        assert!(document.is_read_only());
        assert_eq!(document.buffer().serialize(), "Help text\n");
        assert!(!document.is_dirty());

        let error = document
            .save()
            .expect_err("help buffer should be read-only");
        assert!(error.to_string().contains("read-only"));
    }

    #[test]
    fn completions_document_is_named_clean_and_read_only() {
        let mut document = Document::completions("command\n");

        assert_eq!(document.display_name(), "*Completions*");
        assert_eq!(document.kind(), DocumentKind::Completions);
        assert!(document.is_completions());
        assert!(document.is_read_only());
        assert_eq!(document.buffer().serialize(), "command\n");
        assert!(!document.is_dirty());

        let error = document
            .save()
            .expect_err("completions buffer should be read-only");
        assert!(error.to_string().contains("read-only"));
    }

    #[test]
    fn messages_document_is_named_clean_and_read_only() {
        let mut document = Document::messages("Saved file\n");

        assert_eq!(document.display_name(), "*Messages*");
        assert_eq!(document.kind(), DocumentKind::Messages);
        assert!(document.is_messages());
        assert!(document.is_read_only());
        assert_eq!(document.buffer().serialize(), "Saved file\n");
        assert!(!document.is_dirty());

        let error = document
            .save()
            .expect_err("messages buffer should be read-only");
        assert!(error.to_string().contains("read-only"));
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
    fn save_writes_backup_when_enabled() {
        let directory = TestDir::new();
        let path = directory.path().join("save.txt");
        let backup = directory.path().join("save.txt~");
        fs::write(&path, "old").expect("file should be written");
        let mut document = Document::open(&path).expect("file should open");
        document.set_backup_on_save(true);
        document
            .buffer_mut()
            .insert(Position::new(0, 3), "\nnew")
            .expect("insert should succeed");

        document.save().expect("save should succeed");

        assert_eq!(
            fs::read_to_string(&path).expect("file should read"),
            "old\nnew"
        );
        assert_eq!(
            fs::read_to_string(&backup).expect("backup should read"),
            "old"
        );
        assert!(!document.is_dirty());
    }

    #[test]
    fn backup_on_save_writes_one_backup_per_document_visit() {
        let directory = TestDir::new();
        let path = directory.path().join("save.txt");
        let backup = directory.path().join("save.txt~");
        fs::write(&path, "old").expect("file should be written");
        let mut document = Document::open(&path).expect("file should open");
        document.set_backup_on_save(true);

        document
            .buffer_mut()
            .insert(Position::new(0, 3), " first")
            .expect("insert should succeed");
        document.save().expect("first save should succeed");
        document
            .buffer_mut()
            .insert(Position::new(0, 9), " second")
            .expect("second insert should succeed");
        document.save().expect("second save should succeed");

        assert_eq!(
            fs::read_to_string(&path).expect("file should read"),
            "old first second"
        );
        assert_eq!(
            fs::read_to_string(&backup).expect("backup should read"),
            "old"
        );
    }

    #[test]
    fn save_does_not_write_backup_by_default() {
        let directory = TestDir::new();
        let path = directory.path().join("save.txt");
        let backup = directory.path().join("save.txt~");
        fs::write(&path, "old").expect("file should be written");
        let mut document = Document::open(&path).expect("file should open");
        document
            .buffer_mut()
            .insert(Position::new(0, 3), "\nnew")
            .expect("insert should succeed");

        document.save().expect("save should succeed");

        assert!(!backup.exists());
    }

    #[test]
    fn backup_directory_maps_path_to_backup_file() {
        let directory = TestDir::new();
        let backup_directory = directory.path().join("backups");
        fs::create_dir(&backup_directory).expect("backup directory should create");
        let path = directory.path().join("nested").join("save.txt");
        fs::create_dir(path.parent().expect("path should have parent"))
            .expect("nested directory should create");
        fs::write(&path, "old").expect("file should be written");
        let backup = backup_path(&path, Some(&backup_directory));
        let mut document = Document::open(&path).expect("file should open");
        document.set_backup_on_save(true);
        document.set_backup_directory(Some(backup_directory.clone()));

        document
            .buffer_mut()
            .insert(Position::new(0, 3), " new")
            .expect("insert should succeed");
        document.save().expect("save should succeed");

        assert!(backup.starts_with(&backup_directory));
        assert_eq!(
            fs::read_to_string(&backup).expect("backup should read"),
            "old"
        );
        assert_eq!(
            fs::read_to_string(&path).expect("file should read"),
            "old new"
        );
    }

    #[test]
    fn backup_failure_blocks_save_and_keeps_buffer_dirty() {
        let directory = TestDir::new();
        let path = directory.path().join("save.txt");
        let missing_backup_directory = directory.path().join("missing-backups");
        fs::write(&path, "old").expect("file should be written");
        let mut document = Document::open(&path).expect("file should open");
        document.set_backup_on_save(true);
        document.set_backup_directory(Some(missing_backup_directory));
        document
            .buffer_mut()
            .insert(Position::new(0, 3), " new")
            .expect("insert should succeed");

        let error = document
            .save()
            .expect_err("missing backup directory should fail");

        assert!(error.to_string().contains("I/O error"));
        assert_eq!(fs::read_to_string(&path).expect("file should read"), "old");
        assert!(document.is_dirty());
    }

    #[test]
    fn save_as_new_path_gets_a_new_backup_cycle() {
        let directory = TestDir::new();
        let first = directory.path().join("first.txt");
        let second = directory.path().join("second.txt");
        fs::write(&first, "first old").expect("first file should be written");
        fs::write(&second, "second old").expect("second file should be written");
        let mut document = Document::open(&first).expect("first file should open");
        document.set_backup_on_save(true);

        document
            .buffer_mut()
            .insert(Position::new(0, 9), " saved")
            .expect("insert should succeed");
        document.save().expect("first save should succeed");
        document
            .save_as(&second)
            .expect("write-file should succeed");

        assert_eq!(
            fs::read_to_string(directory.path().join("first.txt~"))
                .expect("first backup should read"),
            "first old"
        );
        assert_eq!(
            fs::read_to_string(directory.path().join("second.txt~"))
                .expect("second backup should read"),
            "second old"
        );
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

    #[cfg(unix)]
    #[test]
    fn safe_write_preserves_existing_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDir::new();
        let path = directory.path().join("mode.txt");
        fs::write(&path, "old").expect("file should be written");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640))
            .expect("permissions should be set");

        safe_write(&path, b"new").expect("safe write should succeed");

        assert_eq!(
            fs::metadata(&path)
                .expect("metadata should read")
                .permissions()
                .mode()
                & 0o777,
            0o640
        );
        assert_eq!(fs::read_to_string(&path).expect("file should read"), "new");
    }

    #[cfg(unix)]
    #[test]
    fn safe_write_preserves_read_only_permissions_after_writing() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDir::new();
        let path = directory.path().join("read-only-mode.txt");
        fs::write(&path, "old").expect("file should be written");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o444))
            .expect("permissions should be set");

        safe_write(&path, b"new").expect("safe write should succeed");

        assert_eq!(fs::read_to_string(&path).expect("file should read"), "new");
        assert_eq!(
            fs::metadata(&path)
                .expect("metadata should read")
                .permissions()
                .mode()
                & 0o777,
            0o444
        );
    }

    #[test]
    fn safe_write_retries_stale_temporary_path_collisions() {
        let directory = TestDir::new();
        let path = directory.path().join("save.txt");
        fs::write(&path, "old").expect("file should be written");
        SAVE_COUNTER.store(0, Ordering::Relaxed);
        let stale = temporary_path(&path);
        fs::write(&stale, "stale temp").expect("stale temp should be written");
        SAVE_COUNTER.store(0, Ordering::Relaxed);

        safe_write(&path, b"new").expect("safe write should retry after temp collision");

        assert_eq!(fs::read_to_string(&path).expect("file should read"), "new");
        assert_eq!(
            fs::read_to_string(&stale).expect("stale temp should remain untouched"),
            "stale temp"
        );
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
