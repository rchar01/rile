// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::{Path, PathBuf};

use crate::command::CommandRegistry;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionStyle {
    Vertical,
    CompletionsBuffer,
    Ido,
}

impl CompletionStyle {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Vertical => "vertical",
            Self::CompletionsBuffer => "completions-buffer",
            Self::Ido => "ido",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionMatching {
    Prefix,
    Substring,
}

impl CompletionMatching {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Prefix => "prefix",
            Self::Substring => "substring",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CompletionConfig {
    pub style: CompletionStyle,
    pub max_candidates: usize,
    pub show_annotations: bool,
    pub matching: CompletionMatching,
}

impl Default for CompletionConfig {
    fn default() -> Self {
        Self {
            style: CompletionStyle::Vertical,
            max_candidates: 8,
            show_annotations: true,
            matching: CompletionMatching::Prefix,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionCandidate {
    pub value: String,
    pub annotation: String,
    directory: bool,
}

impl CompletionCandidate {
    pub fn new(value: impl Into<String>, annotation: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            annotation: annotation.into(),
            directory: false,
        }
    }

    pub fn directory(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            annotation: "Directory".to_owned(),
            directory: true,
        }
    }

    pub fn file(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            annotation: "File".to_owned(),
            directory: false,
        }
    }

    pub fn is_directory(&self) -> bool {
        self.directory
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompletionSource {
    Commands,
    Files,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionSession {
    source: CompletionSource,
    config: CompletionConfig,
    base_dir: Option<PathBuf>,
    candidates: Vec<CompletionCandidate>,
    matches: Vec<usize>,
    selected: usize,
    selection_explicit: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionViewItem<'a> {
    pub candidate: &'a CompletionCandidate,
    pub selected: bool,
}

impl CompletionSession {
    pub fn commands(registry: &CommandRegistry, config: CompletionConfig) -> Self {
        let candidates = registry
            .commands()
            .iter()
            .filter(|command| command.interactive)
            .map(|command| CompletionCandidate::new(command.name, command.description))
            .collect::<Vec<_>>();
        let mut session = Self {
            source: CompletionSource::Commands,
            config,
            base_dir: None,
            candidates,
            matches: Vec::new(),
            selected: 0,
            selection_explicit: false,
        };
        session.update("");
        session
    }

    pub fn files(base_dir: impl Into<PathBuf>, config: CompletionConfig) -> Self {
        let mut session = Self {
            source: CompletionSource::Files,
            config,
            base_dir: Some(base_dir.into()),
            candidates: Vec::new(),
            matches: Vec::new(),
            selected: 0,
            selection_explicit: false,
        };
        session.update("");
        session
    }

    pub fn source(&self) -> CompletionSource {
        self.source
    }

    pub fn style(&self) -> CompletionStyle {
        self.config.style
    }

    pub fn max_candidates(&self) -> usize {
        self.config.max_candidates.max(1)
    }

    pub fn show_annotations(&self) -> bool {
        self.config.show_annotations
    }

    pub fn update(&mut self, input: &str) {
        self.matches = match self.source {
            CompletionSource::Commands => self
                .candidates
                .iter()
                .enumerate()
                .filter_map(|(index, candidate)| {
                    self.matches_input(candidate, input).then_some(index)
                })
                .collect(),
            CompletionSource::Files => {
                self.refresh_file_candidates(input);
                (0..self.candidates.len()).collect()
            }
        };
        if self.matches.is_empty() {
            self.selected = 0;
        } else {
            self.selected = self.selected.min(self.matches.len() - 1);
        }
        self.selection_explicit = false;
    }

    pub fn move_selection(&mut self, delta: isize) {
        if self.matches.is_empty() {
            self.selected = 0;
            return;
        }
        let len = self.matches.len() as isize;
        self.selected = (self.selected as isize + delta).rem_euclid(len) as usize;
        self.selection_explicit = true;
    }

    pub fn selected(&self) -> Option<&CompletionCandidate> {
        self.matches
            .get(self.selected)
            .and_then(|index| self.candidates.get(*index))
    }

    pub fn selection_explicit(&self) -> bool {
        self.selection_explicit
    }

    pub fn common_prefix(&self, input: &str) -> Option<String> {
        let mut values = self.matches.iter().filter_map(|index| {
            self.candidates
                .get(*index)
                .map(|candidate| candidate.value.as_str())
        });
        let first = values.next()?;
        let mut prefix = first.to_owned();
        for value in values {
            prefix = common_prefix(&prefix, value);
            if prefix.is_empty() {
                break;
            }
        }
        (prefix.len() > input.len()).then_some(prefix)
    }

    pub fn view_items(&self) -> Vec<CompletionViewItem<'_>> {
        let max = self.max_candidates();
        let start = if self.selected >= max {
            self.selected + 1 - max
        } else {
            0
        };
        self.matches
            .iter()
            .enumerate()
            .skip(start)
            .take(max)
            .filter_map(|(match_index, candidate_index)| {
                self.candidates
                    .get(*candidate_index)
                    .map(|candidate| CompletionViewItem {
                        candidate,
                        selected: match_index == self.selected,
                    })
            })
            .collect()
    }

    pub fn has_matches(&self) -> bool {
        !self.matches.is_empty()
    }

    fn matches_input(&self, candidate: &CompletionCandidate, input: &str) -> bool {
        match self.config.matching {
            CompletionMatching::Prefix => candidate.value.starts_with(input),
            CompletionMatching::Substring => candidate.value.contains(input),
        }
    }

    fn refresh_file_candidates(&mut self, input: &str) {
        let Some(base_dir) = self.base_dir.as_deref() else {
            self.candidates.clear();
            return;
        };
        let Some(parts) = file_completion_parts(base_dir, input) else {
            self.candidates.clear();
            return;
        };
        let Ok(entries) = fs::read_dir(&parts.search_dir) else {
            self.candidates.clear();
            return;
        };

        let mut candidates = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                if !file_name_matches(self.config.matching, &name, &parts.name_prefix) {
                    return None;
                }
                let Ok(file_type) = entry.file_type() else {
                    return None;
                };
                let value = format!("{}{}", parts.display_prefix, name);
                if file_type.is_dir() {
                    Some(CompletionCandidate::directory(format!("{value}/")))
                } else {
                    Some(CompletionCandidate::file(value))
                }
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .is_directory()
                .cmp(&left.is_directory())
                .then_with(|| left.value.cmp(&right.value))
        });
        self.candidates = candidates;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileCompletionParts {
    search_dir: PathBuf,
    display_prefix: String,
    name_prefix: String,
}

fn file_completion_parts(base_dir: &Path, input: &str) -> Option<FileCompletionParts> {
    let (dir_part, name_prefix) = match input.rsplit_once('/') {
        Some((dir, name)) => (format!("{dir}/"), name.to_owned()),
        None => (String::new(), input.to_owned()),
    };
    let search_dir = if Path::new(input).is_absolute() {
        PathBuf::from(if dir_part.is_empty() { "/" } else { &dir_part })
    } else if dir_part.is_empty() {
        base_dir.to_path_buf()
    } else {
        base_dir.join(&dir_part)
    };
    Some(FileCompletionParts {
        search_dir,
        display_prefix: dir_part,
        name_prefix,
    })
}

fn file_name_matches(matching: CompletionMatching, name: &str, input: &str) -> bool {
    match matching {
        CompletionMatching::Prefix => name.starts_with(input),
        CompletionMatching::Substring => name.contains(input),
    }
}

fn common_prefix(left: &str, right: &str) -> String {
    left.chars()
        .zip(right.chars())
        .take_while(|(left, right)| left == right)
        .map(|(character, _)| character)
        .collect()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::{CompletionConfig, CompletionSession};
    use crate::command::CommandRegistry;

    static TEST_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

    struct TestDir {
        path: PathBuf,
    }

    impl TestDir {
        fn new() -> Self {
            let counter = TEST_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "rile-completion-test-{}-{counter}",
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
    fn command_completion_filters_and_selects_candidates() {
        let registry = CommandRegistry::default();
        let mut session = CompletionSession::commands(&registry, CompletionConfig::default());

        session.update("toggle-s");

        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("toggle-search-highlighting")
        );
        session.move_selection(1);
        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("toggle-syntax-highlighting")
        );
    }

    #[test]
    fn command_completion_extends_common_prefix() {
        let registry = CommandRegistry::default();
        let mut session = CompletionSession::commands(&registry, CompletionConfig::default());

        session.update("toggle");

        assert_eq!(session.common_prefix("toggle").as_deref(), Some("toggle-"));
    }

    #[test]
    fn file_completion_filters_files_and_directories() {
        let directory = TestDir::new();
        fs::write(directory.path().join("alpha.txt"), "alpha").expect("fixture should write");
        fs::write(directory.path().join("alphabet.txt"), "alphabet").expect("fixture should write");
        fs::create_dir(directory.path().join("alpha-dir")).expect("directory should create");
        fs::write(directory.path().join("beta.txt"), "beta").expect("fixture should write");
        let mut session = CompletionSession::files(directory.path(), CompletionConfig::default());

        session.update("alpha");

        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["alpha-dir/", "alpha.txt", "alphabet.txt"]);
        assert!(
            session
                .selected()
                .expect("directory should be selected first")
                .is_directory()
        );
        assert_eq!(session.common_prefix("alpha"), None);
    }
}
