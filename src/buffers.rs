// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::path::Path;

use crate::buffer::BufferId;
use crate::file::{Document, DocumentKind, DocumentSettings};
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

    pub fn entries_mut(&mut self) -> &mut [BufferEntry] {
        &mut self.entries
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

    pub fn find_by_kind(&self, kind: DocumentKind) -> Option<BufferId> {
        self.entries
            .iter()
            .find(|entry| entry.document.kind() == kind)
            .map(BufferEntry::id)
    }

    pub fn open_path(&mut self, path: impl AsRef<Path>) -> Result<OpenBufferResult> {
        self.open_path_with_settings(path, DocumentSettings::default())
    }

    pub fn open_path_with_settings(
        &mut self,
        path: impl AsRef<Path>,
        settings: DocumentSettings,
    ) -> Result<OpenBufferResult> {
        self.open_path_with_options(path, settings, false)
    }

    pub fn open_path_read_only(
        &mut self,
        path: impl AsRef<Path>,
        settings: DocumentSettings,
    ) -> Result<OpenBufferResult> {
        self.open_path_with_options(path, settings, true)
    }

    fn open_path_with_options(
        &mut self,
        path: impl AsRef<Path>,
        settings: DocumentSettings,
        read_only: bool,
    ) -> Result<OpenBufferResult> {
        let path = path.as_ref();
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.document.path() == Some(path))
        {
            if read_only {
                entry.document.set_read_only(true);
            }
            return Ok(OpenBufferResult {
                id: entry.id,
                created: false,
            });
        }

        let mut document = Document::open(path)?;
        document.apply_settings(&settings);
        document.set_read_only(read_only);
        let id = self.push(document);
        Ok(OpenBufferResult { id, created: true })
    }

    pub fn open_help(&mut self, text: impl AsRef<str>) -> BufferId {
        self.open_special(Document::help(text))
    }

    pub fn open_completions(&mut self, text: impl AsRef<str>) -> BufferId {
        self.open_special(Document::completions(text))
    }

    pub fn open_messages(&mut self, text: impl AsRef<str>) -> BufferId {
        self.open_special(Document::messages(text))
    }

    pub fn open_buffer_list(&mut self, text: impl AsRef<str>) -> BufferId {
        self.open_special(Document::buffer_list(text))
    }

    pub fn open_shell_output(&mut self, text: impl AsRef<str>) -> BufferId {
        self.open_special(Document::shell_output(text))
    }

    pub fn kill(&mut self, id: BufferId) -> Result<BufferId> {
        self.kill_with_policy(id, false)
    }

    pub fn kill_confirmed(&mut self, id: BufferId) -> Result<BufferId> {
        self.kill_with_policy(id, true)
    }

    fn kill_with_policy(&mut self, id: BufferId, allow_dirty: bool) -> Result<BufferId> {
        let Some(index) = self.entries.iter().position(|entry| entry.id == id) else {
            return Err(RileError::InvalidInput(format!("no such buffer: {}", id.0)));
        };
        if self.entries[index].document.is_dirty() && !allow_dirty {
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

    fn open_special(&mut self, document: Document) -> BufferId {
        let kind = document.kind();
        debug_assert!(kind != DocumentKind::Normal);
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|entry| entry.document.kind() == kind)
        {
            entry.document = document;
            return entry.id;
        }

        self.push(document)
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
        if !self.entries.iter().any(|entry| entry.name == base) {
            return base;
        }

        let mut suffix = id.0;
        loop {
            let candidate = format!("{base}<{suffix}>");
            if !self.entries.iter().any(|entry| entry.name == candidate) {
                return candidate;
            }
            suffix = suffix.checked_add(1).expect("buffer name suffix exhausted");
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::BufferManager;
    use crate::buffer::{BufferId, Position};
    use crate::file::{Document, DocumentKind};

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
    fn special_buffers_do_not_replace_normal_name_collisions() {
        assert_special_name_collision("*Help*", DocumentKind::Help, |manager, text| {
            manager.open_help(text)
        });
        assert_special_name_collision(
            "*Completions*",
            DocumentKind::Completions,
            |manager, text| manager.open_completions(text),
        );
        assert_special_name_collision("*Messages*", DocumentKind::Messages, |manager, text| {
            manager.open_messages(text)
        });
        assert_special_name_collision(
            "*Buffer List*",
            DocumentKind::BufferList,
            |manager, text| manager.open_buffer_list(text),
        );
        assert_special_name_collision(
            "*Shell Command Output*",
            DocumentKind::ShellOutput,
            |manager, text| manager.open_shell_output(text),
        );
    }

    #[test]
    fn generated_buffer_names_skip_existing_suffixes() {
        let directory = TestDir::new();
        let suffix_path = directory.path().join("notes.txt<3>");
        fs::write(&suffix_path, "suffix").expect("suffix fixture should write");
        let first_directory = directory.path().join("first");
        let second_directory = directory.path().join("second");
        fs::create_dir_all(&first_directory).expect("first directory should exist");
        fs::create_dir_all(&second_directory).expect("second directory should exist");
        let first_path = first_directory.join("notes.txt");
        let second_path = second_directory.join("notes.txt");
        fs::write(&first_path, "first").expect("first fixture should write");
        fs::write(&second_path, "second").expect("second fixture should write");
        let mut manager = BufferManager::new(Document::scratch());

        manager
            .open_path(&suffix_path)
            .expect("suffix fixture should open");
        manager
            .open_path(&first_path)
            .expect("first fixture should open");
        let second = manager
            .open_path(&second_path)
            .expect("second fixture should open");

        assert_eq!(manager.name(second.id), Some("notes.txt<4>"));
        assert_unique_names(&manager);
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

    #[test]
    fn confirmed_kill_removes_dirty_buffers() {
        let mut manager = BufferManager::new(Document::scratch());
        let id = manager.entries()[0].id();
        manager
            .document_mut(id)
            .expect("buffer should exist")
            .buffer_mut()
            .insert(Position::new(0, 0), "dirty")
            .expect("fixture should insert");

        let next = manager
            .kill_confirmed(id)
            .expect("confirmed dirty kill should succeed");

        assert_eq!(manager.len(), 1);
        assert_eq!(manager.name(next), Some("*scratch*"));
        assert!(
            !manager
                .document(next)
                .expect("scratch should exist")
                .is_dirty()
        );
    }

    fn assert_special_name_collision(
        name: &str,
        kind: DocumentKind,
        open_special: impl Fn(&mut BufferManager, &str) -> BufferId,
    ) {
        let directory = TestDir::new();
        let path = directory.path().join(name);
        fs::write(&path, "normal contents").expect("normal fixture should write");
        let mut manager = BufferManager::new(Document::scratch());
        let normal = manager
            .open_path(&path)
            .expect("normal file should open")
            .id;
        manager
            .document_mut(normal)
            .expect("normal document should exist")
            .buffer_mut()
            .insert(Position::new(0, 0), "edited ")
            .expect("normal document should become dirty");

        let special = open_special(&mut manager, "special contents");

        assert_ne!(normal, special);
        let normal_document = manager
            .document(normal)
            .expect("normal document should remain");
        assert_eq!(normal_document.kind(), DocumentKind::Normal);
        assert_eq!(normal_document.path(), Some(path.as_path()));
        assert_eq!(
            normal_document.buffer().serialize(),
            "edited normal contents"
        );
        assert!(normal_document.is_dirty());
        assert!(!normal_document.is_read_only());
        let special_document = manager
            .document(special)
            .expect("special document should exist");
        assert_eq!(special_document.kind(), kind);
        assert_eq!(special_document.path(), None);
        assert_eq!(special_document.buffer().serialize(), "special contents");
        assert!(special_document.is_read_only());
        assert_eq!(open_special(&mut manager, "updated contents"), special);
        assert_eq!(
            manager
                .document(special)
                .expect("special document should remain")
                .buffer()
                .serialize(),
            "updated contents"
        );
        assert_unique_names(&manager);

        manager
            .kill(special)
            .expect("special document should be killable");
        let reopened_special = open_special(&mut manager, "reopened contents");
        assert_ne!(reopened_special, special);
        assert_eq!(
            manager
                .document(reopened_special)
                .expect("special document should reopen")
                .kind(),
            kind
        );
        manager
            .kill_confirmed(normal)
            .expect("normal document should be killable with confirmation");
        let reopened_normal = manager
            .open_path(&path)
            .expect("normal document should reopen")
            .id;
        assert_eq!(
            manager
                .document(reopened_normal)
                .expect("normal document should reopen")
                .kind(),
            DocumentKind::Normal
        );
        assert_eq!(
            manager
                .document(reopened_normal)
                .expect("normal document should reopen")
                .path(),
            Some(path.as_path())
        );
        assert_unique_names(&manager);

        let mut reverse_manager = BufferManager::new(Document::scratch());
        let reverse_special = open_special(&mut reverse_manager, "special contents");
        let reverse_normal = reverse_manager
            .open_path(&path)
            .expect("normal file should open after special buffer")
            .id;
        assert_ne!(reverse_normal, reverse_special);
        assert_eq!(
            reverse_manager
                .document(reverse_special)
                .expect("special document should remain")
                .kind(),
            kind
        );
        let reverse_normal_document = reverse_manager
            .document(reverse_normal)
            .expect("normal document should exist");
        assert_eq!(reverse_normal_document.kind(), DocumentKind::Normal);
        assert_eq!(reverse_normal_document.path(), Some(path.as_path()));
        assert_eq!(
            reverse_normal_document.buffer().serialize(),
            "normal contents"
        );
        assert_unique_names(&reverse_manager);
    }

    fn assert_unique_names(manager: &BufferManager) {
        let mut names = manager
            .entries()
            .iter()
            .map(|entry| entry.name())
            .collect::<Vec<_>>();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), manager.len());
    }
}
