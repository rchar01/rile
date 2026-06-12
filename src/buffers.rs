// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use crate::buffer::BufferId;
use crate::file::Document;
use crate::{Result, RileError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferEntry {
    id: BufferId,
    name: String,
    document: Document,
}

impl BufferEntry {
    pub fn id(&self) -> BufferId {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferManager {
    entries: Vec<BufferEntry>,
    next_id: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenBufferResult {
    pub id: BufferId,
    pub created: bool,
}

impl BufferManager {
    pub fn new(initial: Document) -> Self {
        let mut manager = Self {
            entries: Vec::new(),
            next_id: 0,
        };
        manager.push(initial);
        manager
    }

    pub fn entries(&self) -> &[BufferEntry] {
        &self.entries
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn document(&self, id: BufferId) -> Option<&Document> {
        self.entry(id).map(BufferEntry::document)
    }

    pub fn document_mut(&mut self, id: BufferId) -> Option<&mut Document> {
        self.entry_mut(id).map(BufferEntry::document_mut)
    }

    pub fn name(&self, id: BufferId) -> Option<&str> {
        self.entry(id).map(BufferEntry::name)
    }

    pub fn find_by_name(&self, name: &str) -> Option<BufferId> {
        self.entries
            .iter()
            .find(|entry| entry.name == name)
            .map(BufferEntry::id)
    }

    pub fn open_path(&mut self, path: impl AsRef<Path>) -> Result<OpenBufferResult> {
        self.open_path_with_backup(path, false)
    }

    pub fn open_path_with_backup(
        &mut self,
        path: impl AsRef<Path>,
        backup_on_save: bool,
    ) -> Result<OpenBufferResult> {
        let path = path.as_ref();
        if let Some(entry) = self
            .entries
            .iter()
            .find(|entry| entry.document.path() == Some(path))
        {
            return Ok(OpenBufferResult {
                id: entry.id,
                created: false,
            });
        }

        let mut document = Document::open(path)?;
        document.set_backup_on_save(backup_on_save);
        let id = self.push(document);
        Ok(OpenBufferResult { id, created: true })
    }

    pub fn kill(&mut self, id: BufferId) -> Result<BufferId> {
        let Some(index) = self.entries.iter().position(|entry| entry.id == id) else {
            return Err(RileError::InvalidInput(format!("no such buffer: {}", id.0)));
        };
        if self.entries[index].document.is_dirty() {
            return Err(RileError::InvalidInput(format!(
                "buffer {} has unsaved changes",
                self.entries[index].name
            )));
        }

        self.entries.remove(index);
        if self.entries.is_empty() {
            return Ok(self.push(Document::scratch()));
        }
        let next_index = index.min(self.entries.len() - 1);
        Ok(self.entries[next_index].id)
    }

    fn entry(&self, id: BufferId) -> Option<&BufferEntry> {
        self.entries.iter().find(|entry| entry.id == id)
    }

    fn entry_mut(&mut self, id: BufferId) -> Option<&mut BufferEntry> {
        self.entries.iter_mut().find(|entry| entry.id == id)
    }

    fn push(&mut self, document: Document) -> BufferId {
        let id = BufferId(self.next_id);
        self.next_id += 1;
        let name = self.unique_name(id, &document);
        self.entries.push(BufferEntry { id, name, document });
        id
    }

    fn unique_name(&self, id: BufferId, document: &Document) -> String {
        let base = document
            .path()
            .and_then(Path::file_name)
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| document.display_name());
        if self.entries.iter().any(|entry| entry.name == base) {
            format!("{base}<{}>", id.0)
        } else {
            base
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::BufferManager;
    use crate::buffer::Position;
    use crate::file::Document;

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let counter = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "rile-buffers-test-{}-{counter}",
                std::process::id()
            ));
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
    fn opens_files_once_and_assigns_stable_ids() {
        let directory = TestDir::new();
        let path = directory.path().join("notes.txt");
        fs::write(&path, "notes").expect("fixture should write");
        let mut manager = BufferManager::new(Document::scratch());

        let first = manager.open_path(&path).expect("file should open");
        let second = manager.open_path(&path).expect("file should reuse buffer");

        assert!(first.created);
        assert!(!second.created);
        assert_eq!(first.id, second.id);
        assert_eq!(manager.len(), 2);
        assert_eq!(manager.name(first.id), Some("notes.txt"));
    }

    #[test]
    fn refuses_to_kill_dirty_buffers() {
        let mut manager = BufferManager::new(Document::scratch());
        let id = manager.entries()[0].id();
        manager
            .document_mut(id)
            .expect("buffer should exist")
            .buffer_mut()
            .insert(Position::new(0, 0), "dirty")
            .expect("fixture should insert");

        let error = manager.kill(id).expect_err("dirty kill should fail");

        assert!(error.to_string().contains("unsaved changes"));
        assert_eq!(manager.len(), 1);
    }
}
