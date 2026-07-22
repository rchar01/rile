// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs::{self, Metadata, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::SystemTime;

use crate::buffer::Buffer;
use crate::{Result, RileError};

static SAVE_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) const MAX_INSERT_FILE_BYTES: usize = 8 * 1024 * 1024;
pub(crate) const MAX_INSERT_FILE_NEWLINES: usize = 100_000;

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
    auto_save: bool,
    auto_save_directory: Option<PathBuf>,
    delete_auto_save_files: bool,
    auto_save_written: bool,
    file_stamp: Option<FileStamp>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DocumentSettings {
    pub backup_on_save: bool,
    pub backup_directory: Option<PathBuf>,
    pub auto_save: bool,
    pub auto_save_directory: Option<PathBuf>,
    pub delete_auto_save_files: bool,
}

impl Default for DocumentSettings {
    fn default() -> Self {
        Self {
            backup_on_save: false,
            backup_directory: None,
            auto_save: false,
            auto_save_directory: None,
            delete_auto_save_files: true,
        }
    }
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
            auto_save: false,
            auto_save_directory: None,
            delete_auto_save_files: true,
            auto_save_written: false,
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
            auto_save: false,
            auto_save_directory: None,
            delete_auto_save_files: true,
            auto_save_written: false,
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
            auto_save: false,
            auto_save_directory: None,
            delete_auto_save_files: true,
            auto_save_written: false,
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
            auto_save: false,
            auto_save_directory: None,
            delete_auto_save_files: true,
            auto_save_written: false,
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
            auto_save: false,
            auto_save_directory: None,
            delete_auto_save_files: true,
            auto_save_written: false,
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
            auto_save: false,
            auto_save_directory: None,
            delete_auto_save_files: true,
            auto_save_written: false,
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
            auto_save: false,
            auto_save_directory: None,
            delete_auto_save_files: true,
            auto_save_written: false,
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
                    auto_save: false,
                    auto_save_directory: None,
                    delete_auto_save_files: true,
                    auto_save_written: false,
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
                auto_save: false,
                auto_save_directory: None,
                delete_auto_save_files: true,
                auto_save_written: false,
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

    pub fn auto_save(&self) -> bool {
        self.auto_save
    }

    pub fn set_auto_save(&mut self, enabled: bool) {
        self.auto_save = enabled;
    }

    pub fn auto_save_directory(&self) -> Option<&Path> {
        self.auto_save_directory.as_deref()
    }

    pub fn set_auto_save_directory(&mut self, directory: Option<PathBuf>) {
        self.auto_save_directory = directory;
    }

    pub fn set_delete_auto_save_files(&mut self, enabled: bool) {
        self.delete_auto_save_files = enabled;
    }

    pub fn apply_settings(&mut self, settings: &DocumentSettings) {
        self.set_backup_on_save(settings.backup_on_save);
        self.set_backup_directory(settings.backup_directory.clone());
        self.set_auto_save(settings.auto_save);
        self.set_auto_save_directory(settings.auto_save_directory.clone());
        self.set_delete_auto_save_files(settings.delete_auto_save_files);
    }

    pub fn auto_save_path(&self) -> Option<PathBuf> {
        self.path
            .as_ref()
            .map(|path| auto_save_path(path, self.auto_save_directory.as_deref()))
    }

    pub fn auto_save_if_dirty(&mut self) -> Result<Option<PathBuf>> {
        if !self.auto_save || self.kind != DocumentKind::Normal || !self.is_dirty() {
            return Ok(None);
        }
        let Some(visited_path) = self.path.as_deref() else {
            return Ok(None);
        };
        let Some(path) = self.auto_save_path() else {
            return Ok(None);
        };
        let permissions = auto_save_permissions(visited_path, &path)?;
        safe_write_with_permissions(&path, self.buffer.serialize().as_bytes(), permissions)?;
        self.auto_save_written = true;
        Ok(Some(path))
    }

    pub fn delete_auto_save_file(&self) -> Result<()> {
        if !self.delete_auto_save_files {
            return Ok(());
        }
        let Some(path) = self.auto_save_path() else {
            return Ok(());
        };
        delete_file_if_exists(&path)
    }

    pub fn newer_auto_save_path(&self) -> Result<Option<PathBuf>> {
        if self.kind != DocumentKind::Normal {
            return Ok(None);
        }
        let Some(path) = self.path.as_ref() else {
            return Ok(None);
        };
        let auto_save = auto_save_path(path, self.auto_save_directory.as_deref());
        let Ok(auto_metadata) = fs::metadata(&auto_save) else {
            return Ok(None);
        };
        if !auto_metadata.is_file() {
            return Ok(None);
        }
        let auto_modified = auto_metadata.modified().ok();
        let file_modified = fs::metadata(path)
            .ok()
            .and_then(|metadata| metadata.modified().ok());
        if file_modified.is_none()
            || auto_modified
                .zip(file_modified)
                .is_some_and(|(auto_modified, file_modified)| auto_modified > file_modified)
        {
            Ok(Some(auto_save))
        } else {
            Ok(None)
        }
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
        self.write_to_path(&path)?;
        self.delete_current_session_auto_save_file(&path)?;
        Ok(())
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
        let cleanup_path = self.path.as_deref().unwrap_or(&path).to_path_buf();
        self.delete_current_session_auto_save_file(&cleanup_path)?;
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
        let auto_save = self.auto_save;
        let auto_save_directory = self.auto_save_directory.clone();
        let delete_auto_save_files = self.delete_auto_save_files;
        let auto_save_written = self.auto_save_written;
        let mut reloaded = Self::open(&path)?;
        if delete_auto_save_files && auto_save_written {
            delete_file_if_exists(&auto_save_path(&path, auto_save_directory.as_deref()))?;
        }
        reloaded.read_only = read_only;
        reloaded.backup_on_save = backup_on_save;
        reloaded.backup_directory = backup_directory;
        reloaded.auto_save = auto_save;
        reloaded.auto_save_directory = auto_save_directory;
        reloaded.delete_auto_save_files = delete_auto_save_files;
        reloaded.auto_save_written = auto_save_written && !delete_auto_save_files;
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
        let existing = existing_save_file_metadata(path)?;
        if self.backup_on_save && !self.backup_written {
            if let Some(expected) = existing.as_ref() {
                write_backup(path, self.backup_directory.as_deref(), expected)?;
            }
            self.backup_written = true;
        }
        safe_write_with_permissions(
            path,
            self.buffer.serialize().as_bytes(),
            existing.map(|metadata| metadata.permissions()),
        )?;
        self.buffer.mark_clean();
        self.missing_on_open = false;
        self.file_stamp = file_stamp_from_path(path)?;
        Ok(())
    }

    fn delete_current_session_auto_save_file(&mut self, path: &Path) -> Result<()> {
        if self.delete_auto_save_files && self.auto_save_written {
            delete_file_if_exists(&auto_save_path(path, self.auto_save_directory.as_deref()))?;
            self.auto_save_written = false;
        }
        Ok(())
    }
}

pub fn read_text_file(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    read_text_file_with_limits(
        path,
        fs::File::open(path)?,
        MAX_INSERT_FILE_BYTES,
        MAX_INSERT_FILE_NEWLINES,
    )
}

fn read_text_file_with_limits(
    path: &Path,
    reader: impl Read,
    byte_limit: usize,
    newline_limit: usize,
) -> Result<String> {
    let read_limit = byte_limit
        .checked_add(1)
        .ok_or_else(|| RileError::InvalidInput("insert-file byte limit is too large".to_owned()))?;
    let mut bytes = Vec::new();
    reader.take(read_limit as u64).read_to_end(&mut bytes)?;
    if bytes.len() > byte_limit {
        return Err(RileError::InvalidInput(format!(
            "insert-file input exceeded the {byte_limit}-byte limit: {}",
            path.display()
        )));
    }
    if bytes.iter().filter(|byte| **byte == b'\n').count() > newline_limit {
        return Err(RileError::InvalidInput(format!(
            "insert-file input exceeded the {newline_limit}-line-break limit: {}",
            path.display()
        )));
    }
    decode_text_file_bytes(path, bytes)
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

fn write_backup(path: &Path, backup_directory: Option<&Path>, expected: &Metadata) -> Result<()> {
    let source = open_backup_source(path)?;
    let (bytes, metadata) = read_backup_source(source)?;
    if !same_file(expected, &metadata) {
        return Err(RileError::InvalidInput(format!(
            "file changed while creating backup: {}",
            path.display()
        )));
    }
    safe_write_with_permissions(
        &backup_path(path, backup_directory),
        &bytes,
        Some(backup_permissions(&metadata)),
    )
}

fn open_backup_source(path: &Path) -> Result<fs::File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;

        options.custom_flags(libc::O_NOFOLLOW);
    }
    match options.open(path) {
        Ok(file) => Ok(file),
        Err(error) => {
            #[cfg(unix)]
            if error.raw_os_error() == Some(libc::ELOOP) {
                return Err(symbolic_link_save_error(path));
            }
            Err(error.into())
        }
    }
}

fn read_backup_source(mut source: fs::File) -> Result<(Vec<u8>, Metadata)> {
    let metadata = source.metadata()?;
    if !metadata.is_file() {
        return Err(RileError::InvalidInput(
            "backup source is not a regular file".to_owned(),
        ));
    }
    let mut bytes = Vec::new();
    source.read_to_end(&mut bytes)?;
    Ok((bytes, metadata))
}

#[cfg(unix)]
fn same_file(expected: &Metadata, actual: &Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    expected.dev() == actual.dev() && expected.ino() == actual.ino()
}

#[cfg(not(unix))]
fn same_file(_expected: &Metadata, _actual: &Metadata) -> bool {
    true
}

#[cfg(unix)]
fn backup_permissions(_source: &Metadata) -> fs::Permissions {
    use std::os::unix::fs::PermissionsExt;

    fs::Permissions::from_mode(0o600)
}

#[cfg(not(unix))]
fn backup_permissions(source: &Metadata) -> fs::Permissions {
    source.permissions()
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
    let mut name = mapped_path_name(path);
    name.push('~');
    name
}

fn auto_save_path(path: &Path, auto_save_directory: Option<&Path>) -> PathBuf {
    match auto_save_directory {
        Some(directory) => directory.join(format!("#{}#", mapped_path_name(path))),
        None => sibling_auto_save_path(path),
    }
}

fn sibling_auto_save_path(path: &Path) -> PathBuf {
    let mut auto_save = PathBuf::from(path);
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| "rile-buffer".into());
    auto_save.set_file_name(format!("#{file_name}#"));
    auto_save
}

fn mapped_path_name(path: &Path) -> String {
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
    name
}

fn write_temporary_then_rename(path: &Path, bytes: &[u8]) -> Result<()> {
    let permissions = existing_save_file_metadata(path)?.map(|metadata| metadata.permissions());
    safe_write_with_permissions(path, bytes, permissions)
}

fn existing_save_file_metadata(path: &Path) -> Result<Option<Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(symbolic_link_save_error(path)),
        Ok(metadata) if metadata.is_file() => Ok(Some(metadata)),
        Ok(_) => Err(RileError::InvalidInput(format!(
            "refusing to replace non-regular file: {}",
            path.display()
        ))),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}

fn symbolic_link_save_error(path: &Path) -> RileError {
    RileError::InvalidInput(format!(
        "refusing to save through symbolic link: {}",
        path.display()
    ))
}

fn delete_file_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

fn safe_write_with_permissions(
    path: &Path,
    bytes: &[u8],
    permissions: Option<fs::Permissions>,
) -> Result<()> {
    let (temporary, mut file) = create_temporary_file(path, permissions.as_ref())?;
    let result = (|| -> Result<()> {
        #[cfg(unix)]
        let permissions = {
            use std::os::unix::fs::PermissionsExt;

            let final_permissions = match permissions {
                Some(permissions) => permissions,
                None => file.metadata()?.permissions(),
            };
            file.set_permissions(fs::Permissions::from_mode(0o600))?;
            Some(final_permissions)
        };
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

fn auto_save_permissions(
    visited_path: &Path,
    auto_save_path: &Path,
) -> Result<Option<fs::Permissions>> {
    let visited = existing_file_permissions(visited_path)?;
    let auto_save = existing_file_permissions(auto_save_path)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let visited_mode = visited.map_or(0o600, |permissions| permissions.mode());
        let mode = auto_save.map_or(visited_mode, |permissions| {
            visited_mode & permissions.mode()
        });
        Ok(Some(fs::Permissions::from_mode(mode & 0o7777)))
    }

    #[cfg(not(unix))]
    {
        Ok(auto_save.or(visited))
    }
}

fn create_temporary_file(
    path: &Path,
    permissions: Option<&fs::Permissions>,
) -> Result<(PathBuf, fs::File)> {
    #[cfg(not(unix))]
    let _ = permissions;
    let mut last_error = None;
    for _ in 0..16 {
        let temporary = temporary_path(path);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        if permissions.is_some() {
            use std::os::unix::fs::OpenOptionsExt;

            options.mode(0o600);
        }
        match options.open(&temporary) {
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
    use std::io::{self, Cursor, Read};
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{
        Document, DocumentKind, SAVE_COUNTER, backup_path, read_text_file_with_limits, safe_write,
        temporary_path,
    };
    use crate::buffer::Position;

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct RepeatingReader {
        bytes_read: usize,
    }

    impl Read for RepeatingReader {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            buffer.fill(b'a');
            self.bytes_read += buffer.len();
            Ok(buffer.len())
        }
    }

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
    fn bounded_text_read_accepts_exact_byte_limit() {
        let path = Path::new("source.txt");

        let text = read_text_file_with_limits(path, Cursor::new("é".as_bytes()), 2, 0)
            .expect("complete UTF-8 at the byte limit should be accepted");

        assert_eq!(text, "é");
    }

    #[test]
    fn bounded_text_read_rejects_input_above_limit_before_decoding() {
        let path = Path::new("source.txt");

        let error = read_text_file_with_limits(path, Cursor::new([0xff, b'a']), 1, 0)
            .expect_err("input above the byte limit should be rejected");

        assert_eq!(
            error.to_string(),
            "invalid input: insert-file input exceeded the 1-byte limit: source.txt"
        );
    }

    #[test]
    fn bounded_text_read_stops_non_terminating_reader_after_detection_byte() {
        let path = Path::new("source.txt");
        let mut reader = RepeatingReader { bytes_read: 0 };

        let error = read_text_file_with_limits(path, &mut reader, 5, 0)
            .expect_err("non-terminating input should exceed the limit");

        assert!(error.to_string().contains("exceeded the 5-byte limit"));
        assert_eq!(reader.bytes_read, 6);
    }

    #[test]
    fn bounded_text_read_rejects_excessive_line_breaks() {
        let path = Path::new("source.txt");
        let accepted = read_text_file_with_limits(path, Cursor::new(b"a\nb\n"), 4, 2)
            .expect("input at the line-break limit should be accepted");
        assert_eq!(accepted, "a\nb\n");

        let error = read_text_file_with_limits(path, Cursor::new(b"\n\n"), 2, 1)
            .expect_err("newline-dense input should exceed the limit");
        assert_eq!(
            error.to_string(),
            "invalid input: insert-file input exceeded the 1-line-break limit: source.txt"
        );
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

    #[cfg(unix)]
    #[test]
    fn backup_on_save_uses_private_permissions() {
        use std::os::unix::fs::PermissionsExt;

        for source_mode in [0o600, 0o644] {
            let directory = TestDir::new();
            let path = directory.path().join(format!("mode-{source_mode:o}.txt"));
            let backup = directory.path().join(format!("mode-{source_mode:o}.txt~"));
            fs::write(&path, "old").expect("file should be written");
            fs::set_permissions(&path, fs::Permissions::from_mode(source_mode))
                .expect("source permissions should be set");
            fs::write(&backup, "stale").expect("old backup should be written");
            fs::set_permissions(&backup, fs::Permissions::from_mode(0o666))
                .expect("old backup permissions should be set");
            let mut document = Document::open(&path).expect("file should open");
            document.set_backup_on_save(true);
            document
                .buffer_mut()
                .insert(Position::new(0, 3), " new")
                .expect("insert should succeed");

            document.save().expect("save should succeed");

            assert_eq!(
                fs::read_to_string(&backup).expect("backup should read"),
                "old"
            );
            assert_eq!(
                fs::metadata(&backup)
                    .expect("backup metadata should read")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn save_rejects_symbolic_link_paths_and_keeps_buffer_dirty() {
        use std::os::unix::fs::symlink;

        for backup_on_save in [false, true] {
            let directory = TestDir::new();
            let target = directory.path().join("target.txt");
            let path = directory.path().join("link.txt");
            let backup = directory.path().join("link.txt~");
            fs::write(&target, "secret").expect("target should be written");
            symlink(&target, &path).expect("symbolic link should be created");
            let mut document = Document::open(&path).expect("symbolic link should open");
            document.set_backup_on_save(backup_on_save);
            document
                .buffer_mut()
                .insert(Position::new(0, 6), " changed")
                .expect("insert should succeed");

            let error = document
                .save()
                .expect_err("saving through a symbolic link should fail");

            assert!(
                error
                    .to_string()
                    .contains("refusing to save through symbolic link")
            );
            assert!(
                fs::symlink_metadata(&path)
                    .expect("link metadata should read")
                    .file_type()
                    .is_symlink()
            );
            assert_eq!(
                fs::read_to_string(&target).expect("target should read"),
                "secret"
            );
            assert!(!backup.exists());
            assert!(document.is_dirty());
        }
    }

    #[cfg(unix)]
    #[test]
    fn backup_replaces_destination_symlink_without_writing_its_target() {
        use std::os::unix::fs::{PermissionsExt, symlink};

        let directory = TestDir::new();
        let path = directory.path().join("save.txt");
        let backup = directory.path().join("save.txt~");
        let sentinel = directory.path().join("sentinel.txt");
        fs::write(&path, "old").expect("file should be written");
        fs::write(&sentinel, "sentinel").expect("sentinel should be written");
        symlink(&sentinel, &backup).expect("backup symlink should be created");
        let mut document = Document::open(&path).expect("file should open");
        document.set_backup_on_save(true);
        document
            .buffer_mut()
            .insert(Position::new(0, 3), " new")
            .expect("insert should succeed");

        document.save().expect("save should succeed");

        assert_eq!(
            fs::read_to_string(&sentinel).expect("sentinel should read"),
            "sentinel"
        );
        assert!(
            !fs::symlink_metadata(&backup)
                .expect("backup metadata should read")
                .file_type()
                .is_symlink()
        );
        assert_eq!(
            fs::read_to_string(&backup).expect("backup should read"),
            "old"
        );
        assert_eq!(
            fs::metadata(&backup)
                .expect("backup metadata should read")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn opened_backup_source_retains_identity_after_path_replacement() {
        use std::os::unix::fs::symlink;

        let directory = TestDir::new();
        let path = directory.path().join("source.txt");
        let replacement = directory.path().join("replacement.txt");
        fs::write(&path, "original").expect("source should be written");
        fs::write(&replacement, "replacement").expect("replacement should be written");
        let expected = fs::symlink_metadata(&path).expect("source metadata should read");
        let source = super::open_backup_source(&path).expect("backup source should open");
        fs::remove_file(&path).expect("source path should be removed");
        symlink(&replacement, &path).expect("replacement link should be created");

        let (bytes, actual) = super::read_backup_source(source).expect("source should read");

        assert_eq!(bytes, b"original");
        assert!(super::same_file(&expected, &actual));
    }

    #[cfg(unix)]
    #[test]
    fn backup_rejects_source_replacement_after_metadata_capture() {
        let directory = TestDir::new();
        let path = directory.path().join("source.txt");
        let replacement = directory.path().join("replacement.txt");
        let backup = directory.path().join("source.txt~");
        fs::write(&path, "original").expect("source should be written");
        fs::write(&replacement, "replacement").expect("replacement should be written");
        let expected = fs::symlink_metadata(&path).expect("source metadata should read");
        fs::rename(&replacement, &path).expect("replacement should replace source path");

        let error = super::write_backup(&path, None, &expected)
            .expect_err("replaced backup source should fail");

        assert!(
            error
                .to_string()
                .contains("file changed while creating backup")
        );
        assert!(!backup.exists());
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
    fn auto_save_writes_sibling_hash_file_without_marking_clean() {
        let directory = TestDir::new();
        let path = directory.path().join("notes.txt");
        let auto_save = directory.path().join("#notes.txt#");
        fs::write(&path, "old\n").expect("file should be written");
        let mut document = Document::open(&path).expect("file should open");
        document.set_auto_save(true);
        document
            .buffer_mut()
            .insert(Position::new(1, 0), "new\n")
            .expect("insert should succeed");

        let written = document
            .auto_save_if_dirty()
            .expect("auto-save should write")
            .expect("dirty file-backed document should auto-save");

        assert_eq!(written, auto_save);
        assert_eq!(
            fs::read_to_string(&path).expect("file should read"),
            "old\n"
        );
        assert_eq!(
            fs::read_to_string(&auto_save).expect("auto-save should read"),
            "old\nnew\n"
        );
        assert!(document.is_dirty());
    }

    #[cfg(unix)]
    #[test]
    fn auto_save_inherits_visited_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDir::new();
        let path = directory.path().join("private.txt");
        let auto_save = directory.path().join("#private.txt#");
        fs::write(&path, "secret\n").expect("file should be written");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("permissions should be set");
        let mut document = Document::open(&path).expect("file should open");
        document.set_auto_save(true);
        document
            .buffer_mut()
            .insert(Position::new(1, 0), "unsaved\n")
            .expect("insert should succeed");

        document
            .auto_save_if_dirty()
            .expect("auto-save should write");

        assert_eq!(
            fs::metadata(&auto_save)
                .expect("auto-save metadata should read")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn auto_save_for_missing_visited_file_uses_private_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDir::new();
        let path = directory.path().join("new-private.txt");
        let auto_save = directory.path().join("#new-private.txt#");
        let mut document = Document::open(&path).expect("missing file should open");
        document.set_auto_save(true);
        document
            .buffer_mut()
            .insert(Position::new(0, 0), "unsaved secret\n")
            .expect("insert should succeed");

        document
            .auto_save_if_dirty()
            .expect("auto-save should write");

        assert_eq!(
            fs::metadata(&auto_save)
                .expect("auto-save metadata should read")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[cfg(unix)]
    #[test]
    fn configured_auto_save_directory_inherits_visited_file_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDir::new();
        let auto_save_directory = directory.path().join("auto-save");
        fs::create_dir(&auto_save_directory).expect("auto-save directory should create");
        let path = directory.path().join("private.txt");
        fs::write(&path, "secret\n").expect("file should be written");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o640))
            .expect("permissions should be set");
        let mut document = Document::open(&path).expect("file should open");
        document.set_auto_save(true);
        document.set_auto_save_directory(Some(auto_save_directory));
        document
            .buffer_mut()
            .insert(Position::new(1, 0), "unsaved\n")
            .expect("insert should succeed");

        let written = document
            .auto_save_if_dirty()
            .expect("auto-save should write")
            .expect("dirty file-backed document should auto-save");

        assert_eq!(
            fs::metadata(written)
                .expect("auto-save metadata should read")
                .permissions()
                .mode()
                & 0o777,
            0o640
        );
    }

    #[cfg(unix)]
    #[test]
    fn auto_save_rewrites_use_most_restrictive_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDir::new();
        let path = directory.path().join("private.txt");
        let auto_save = directory.path().join("#private.txt#");
        fs::write(&path, "secret\n").expect("file should be written");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644))
            .expect("visited permissions should be set");
        fs::write(&auto_save, "older recovery\n").expect("auto-save should be written");
        fs::set_permissions(&auto_save, fs::Permissions::from_mode(0o600))
            .expect("auto-save permissions should be set");
        let mut document = Document::open(&path).expect("file should open");
        document.set_auto_save(true);
        document
            .buffer_mut()
            .insert(Position::new(1, 0), "first\n")
            .expect("insert should succeed");

        document
            .auto_save_if_dirty()
            .expect("auto-save should preserve restrictive mode");
        assert_eq!(
            fs::metadata(&auto_save)
                .expect("auto-save metadata should read")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );

        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("visited permissions should be tightened");
        fs::set_permissions(&auto_save, fs::Permissions::from_mode(0o644))
            .expect("auto-save permissions should be widened for regression setup");
        document
            .buffer_mut()
            .insert(Position::new(2, 0), "second\n")
            .expect("second insert should succeed");

        document
            .auto_save_if_dirty()
            .expect("auto-save should tighten permissive mode");
        assert_eq!(
            fs::metadata(&auto_save)
                .expect("auto-save metadata should read")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }

    #[test]
    fn auto_save_directory_maps_path_and_wraps_name_in_hashes() {
        let directory = TestDir::new();
        let auto_save_directory = directory.path().join("auto-save");
        fs::create_dir(&auto_save_directory).expect("auto-save directory should create");
        let path = directory.path().join("nested").join("notes.txt");
        fs::create_dir(path.parent().expect("path should have parent"))
            .expect("nested directory should create");
        fs::write(&path, "old\n").expect("file should be written");
        let mut document = Document::open(&path).expect("file should open");
        document.set_auto_save(true);
        document.set_auto_save_directory(Some(auto_save_directory.clone()));
        document
            .buffer_mut()
            .insert(Position::new(1, 0), "new\n")
            .expect("insert should succeed");

        let written = document
            .auto_save_if_dirty()
            .expect("auto-save should write")
            .expect("dirty file-backed document should auto-save");

        assert!(written.starts_with(&auto_save_directory));
        let file_name = written
            .file_name()
            .expect("auto-save should have file name")
            .to_string_lossy();
        assert!(file_name.starts_with('#'));
        assert!(file_name.ends_with('#'));
        assert!(file_name.contains('!'));
        assert_eq!(
            fs::read_to_string(&written).expect("auto-save should read"),
            "old\nnew\n"
        );
    }

    #[test]
    fn save_deletes_auto_save_file_by_default() {
        let directory = TestDir::new();
        let path = directory.path().join("notes.txt");
        let auto_save = directory.path().join("#notes.txt#");
        fs::write(&path, "old\n").expect("file should be written");
        let mut document = Document::open(&path).expect("file should open");
        document.set_auto_save(true);
        document
            .buffer_mut()
            .insert(Position::new(1, 0), "new\n")
            .expect("insert should succeed");
        document
            .auto_save_if_dirty()
            .expect("auto-save should write");
        assert!(auto_save.exists());

        document.save().expect("save should succeed");

        assert!(!auto_save.exists());
        assert_eq!(
            fs::read_to_string(&path).expect("file should read"),
            "old\nnew\n"
        );
    }

    #[test]
    fn save_keeps_auto_save_file_when_cleanup_is_disabled() {
        let directory = TestDir::new();
        let path = directory.path().join("notes.txt");
        let auto_save = directory.path().join("#notes.txt#");
        fs::write(&path, "old\n").expect("file should be written");
        let mut document = Document::open(&path).expect("file should open");
        document.set_auto_save(true);
        document.set_delete_auto_save_files(false);
        document
            .buffer_mut()
            .insert(Position::new(1, 0), "new\n")
            .expect("insert should succeed");
        document
            .auto_save_if_dirty()
            .expect("auto-save should write");

        document.save().expect("save should succeed");

        assert!(auto_save.exists());
    }

    #[test]
    fn save_as_deletes_old_visited_path_auto_save_file() {
        let directory = TestDir::new();
        let old_path = directory.path().join("old.txt");
        let new_path = directory.path().join("new.txt");
        let old_auto_save = directory.path().join("#old.txt#");
        fs::write(&old_path, "old\n").expect("old file should be written");
        let mut document = Document::open(&old_path).expect("old file should open");
        document.set_auto_save(true);
        document
            .buffer_mut()
            .insert(Position::new(1, 0), "new\n")
            .expect("insert should succeed");
        document
            .auto_save_if_dirty()
            .expect("auto-save should write");
        assert!(old_auto_save.exists());

        document.save_as(&new_path).expect("save-as should succeed");

        assert!(!old_auto_save.exists());
        assert_eq!(document.path(), Some(new_path.as_path()));
    }

    #[test]
    fn save_preserves_preexisting_auto_save_file_not_written_this_session() {
        let directory = TestDir::new();
        let path = directory.path().join("notes.txt");
        let auto_save = directory.path().join("#notes.txt#");
        fs::write(&path, "old\n").expect("file should be written");
        fs::write(&auto_save, "old\ncrash\n").expect("auto-save should be written");
        let mut document = Document::open(&path).expect("file should open");
        document
            .buffer_mut()
            .insert(Position::new(1, 0), "new\n")
            .expect("insert should succeed");

        document.save().expect("save should succeed");

        assert!(auto_save.exists());
        assert_eq!(
            fs::read_to_string(&auto_save).expect("auto-save should read"),
            "old\ncrash\n"
        );
    }

    #[test]
    fn saved_current_session_auto_save_is_not_reported_as_newer_when_kept() {
        let directory = TestDir::new();
        let path = directory.path().join("notes.txt");
        fs::write(&path, "old\n").expect("file should be written");
        let mut document = Document::open(&path).expect("file should open");
        document.set_auto_save(true);
        document.set_delete_auto_save_files(false);
        document
            .buffer_mut()
            .insert(Position::new(1, 0), "new\n")
            .expect("insert should succeed");
        document
            .auto_save_if_dirty()
            .expect("auto-save should write");

        document.save().expect("save should succeed");

        assert_eq!(
            document
                .newer_auto_save_path()
                .expect("newer auto-save check should work"),
            None
        );
    }

    #[test]
    fn reload_from_disk_deletes_current_session_auto_save_file() {
        let directory = TestDir::new();
        let path = directory.path().join("notes.txt");
        let auto_save = directory.path().join("#notes.txt#");
        fs::write(&path, "old\n").expect("file should be written");
        let mut document = Document::open(&path).expect("file should open");
        document.set_auto_save(true);
        document
            .buffer_mut()
            .insert(Position::new(1, 0), "new\n")
            .expect("insert should succeed");
        document
            .auto_save_if_dirty()
            .expect("auto-save should write");
        assert!(auto_save.exists());

        document.reload_from_disk().expect("reload should succeed");

        assert!(!auto_save.exists());
        assert_eq!(document.buffer().serialize(), "old\n");
        assert!(!document.is_dirty());
    }

    #[test]
    fn auto_save_failure_keeps_buffer_dirty() {
        let directory = TestDir::new();
        let path = directory.path().join("notes.txt");
        let missing_auto_save_directory = directory.path().join("missing-auto-save");
        fs::write(&path, "old\n").expect("file should be written");
        let mut document = Document::open(&path).expect("file should open");
        document.set_auto_save(true);
        document.set_auto_save_directory(Some(missing_auto_save_directory));
        document
            .buffer_mut()
            .insert(Position::new(1, 0), "new\n")
            .expect("insert should succeed");

        let error = document
            .auto_save_if_dirty()
            .expect_err("missing auto-save directory should fail");

        assert!(error.to_string().contains("I/O error"));
        assert_eq!(
            fs::read_to_string(&path).expect("file should read"),
            "old\n"
        );
        assert!(document.is_dirty());
    }

    #[test]
    fn newer_auto_save_path_reports_stale_recovery_file() {
        let directory = TestDir::new();
        let path = directory.path().join("notes.txt");
        let auto_save = directory.path().join("#notes.txt#");
        fs::write(&path, "old\n").expect("file should be written");
        std::thread::sleep(std::time::Duration::from_millis(1100));
        fs::write(&auto_save, "old\nnew\n").expect("auto-save should be written");
        let mut document = Document::open(&path).expect("file should open");
        document.set_auto_save(true);

        assert_eq!(
            document
                .newer_auto_save_path()
                .expect("newer auto-save check should work"),
            Some(auto_save)
        );
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
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            assert_eq!(
                fs::metadata(&backup)
                    .expect("backup metadata should read")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }
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
    fn successful_backup_survives_failed_source_write_and_retry() {
        let directory = TestDir::new();
        let path = directory.path().join("save.txt");
        let backup = directory.path().join("save.txt~");
        fs::write(&path, "old").expect("file should be written");
        let mut document = Document::open(&path).expect("file should open");
        document.set_backup_on_save(true);
        document
            .buffer_mut()
            .insert(Position::new(0, 3), " new")
            .expect("insert should succeed");
        let temporary_paths = (0..256)
            .map(|counter| {
                directory.path().join(format!(
                    ".save.txt.rile-tmp-{}-{counter}",
                    std::process::id()
                ))
            })
            .collect::<Vec<_>>();
        for temporary in &temporary_paths {
            fs::write(temporary, "collision").expect("collision should be written");
        }
        SAVE_COUNTER.store(0, Ordering::Relaxed);

        let error = document
            .save()
            .expect_err("source temporary collisions should fail the save");

        assert!(error.to_string().contains("I/O error"));
        assert_eq!(fs::read_to_string(&path).expect("file should read"), "old");
        assert_eq!(
            fs::read_to_string(&backup).expect("backup should read"),
            "old"
        );
        assert!(document.is_dirty());

        for temporary in temporary_paths {
            fs::remove_file(temporary).expect("collision should be removed");
        }
        document
            .save()
            .expect("retry should save without replacing backup");

        assert_eq!(
            fs::read_to_string(&path).expect("file should read"),
            "old new"
        );
        assert_eq!(
            fs::read_to_string(&backup).expect("backup should read"),
            "old"
        );
        assert!(!document.is_dirty());
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

        assert!(
            error
                .to_string()
                .contains("refusing to replace non-regular file")
        );
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

    #[cfg(unix)]
    #[test]
    fn temporary_file_is_private_before_final_permissions_are_applied() {
        use std::os::unix::fs::PermissionsExt;

        let directory = TestDir::new();
        let path = directory.path().join("private.txt");
        let permissions = fs::Permissions::from_mode(0o644);

        let (temporary, file) = super::create_temporary_file(&path, Some(&permissions))
            .expect("temporary file should be created");
        drop(file);

        assert_eq!(
            fs::metadata(&temporary)
                .expect("temporary metadata should read")
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        fs::remove_file(temporary).expect("temporary file should be removed");
    }

    #[cfg(unix)]
    #[test]
    fn safe_write_rejects_existing_fifo() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::FileTypeExt;

        let directory = TestDir::new();
        let path = directory.path().join("pipe");
        let path_bytes =
            CString::new(path.as_os_str().as_bytes()).expect("path should not contain NUL");
        // SAFETY: path_bytes is a live NUL-terminated path and mode is valid.
        let result = unsafe { libc::mkfifo(path_bytes.as_ptr(), 0o600) };
        assert_eq!(result, 0, "FIFO should be created");

        let error = safe_write(&path, b"secret").expect_err("FIFO replacement should fail");

        assert!(
            error
                .to_string()
                .contains("refusing to replace non-regular file")
        );
        assert!(
            fs::symlink_metadata(&path)
                .expect("FIFO metadata should read")
                .file_type()
                .is_fifo()
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
