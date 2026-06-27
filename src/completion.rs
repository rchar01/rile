// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::fs;
use std::path::{Path, PathBuf};

use crate::command::CommandRegistry;
use crate::keymap::{KeyMap, format_key_sequence};
use crate::option::OptionRegistry;

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
    Orderless,
    Prefix,
    Substring,
}

impl CompletionMatching {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Orderless => "orderless",
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
            matching: CompletionMatching::Orderless,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionCandidate {
    pub value: String,
    pub annotation: String,
    pub key_binding: Option<String>,
    directory: bool,
}

impl CompletionCandidate {
    pub fn new(value: impl Into<String>, annotation: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            annotation: annotation.into(),
            key_binding: None,
            directory: false,
        }
    }

    pub fn with_key_binding(mut self, key_binding: impl Into<String>) -> Self {
        self.key_binding = Some(key_binding.into());
        self
    }

    pub fn display_label(&self) -> String {
        match &self.key_binding {
            Some(key_binding) => format!("{} ({key_binding})", self.value),
            None => self.value.clone(),
        }
    }

    pub fn directory(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            annotation: "Directory".to_owned(),
            key_binding: None,
            directory: true,
        }
    }

    pub fn file(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            annotation: "File".to_owned(),
            key_binding: None,
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
    Buffers,
    Options,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionSession {
    source: CompletionSource,
    title: String,
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
    pub fn commands(registry: &CommandRegistry, keymap: &KeyMap, config: CompletionConfig) -> Self {
        let candidates = registry
            .interactive_commands()
            .map(|command| {
                let candidate = CompletionCandidate::new(command.name, command.summary);
                match keymap.bindings_for_command(command.command).first() {
                    Some(binding) => {
                        candidate.with_key_binding(format_key_sequence(&binding.sequence))
                    }
                    None => candidate,
                }
            })
            .collect::<Vec<_>>();
        let mut session = Self {
            source: CompletionSource::Commands,
            title: "M-x".to_owned(),
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

    pub fn options(registry: &OptionRegistry, config: CompletionConfig) -> Self {
        let candidates = registry
            .options()
            .map(|option| CompletionCandidate::new(option.name, option.summary))
            .collect::<Vec<_>>();
        let mut session = Self {
            source: CompletionSource::Options,
            title: "Describe variable".to_owned(),
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
            title: "Find file".to_owned(),
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

    pub fn buffers(names: impl IntoIterator<Item = String>, config: CompletionConfig) -> Self {
        Self::buffers_with_title(names, config, "Switch to buffer")
    }

    pub fn buffers_with_title(
        names: impl IntoIterator<Item = String>,
        config: CompletionConfig,
        title: impl Into<String>,
    ) -> Self {
        let candidates = names
            .into_iter()
            .map(|name| CompletionCandidate::new(name, "Buffer"))
            .collect::<Vec<_>>();
        let mut session = Self {
            source: CompletionSource::Buffers,
            title: title.into(),
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

    pub fn title(&self) -> &str {
        &self.title
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
            CompletionSource::Commands | CompletionSource::Buffers | CompletionSource::Options => {
                let matching = self.config.matching;
                if matching == CompletionMatching::Orderless && !input.is_empty() {
                    let components = parse_orderless_components(input);
                    let mut matches = self
                        .candidates
                        .iter()
                        .enumerate()
                        .filter_map(|(index, candidate)| {
                            orderless_match_score_for_components(&candidate.value, &components)
                                .map(|score| (index, score))
                        })
                        .collect::<Vec<_>>();
                    if components.iter().any(|component| !component.negated) {
                        matches.sort_by(|(left_index, left_score), (right_index, right_score)| {
                            left_score
                                .cmp(right_score)
                                .then_with(|| {
                                    self.candidates[*left_index]
                                        .value
                                        .len()
                                        .cmp(&self.candidates[*right_index].value.len())
                                })
                                .then_with(|| left_index.cmp(right_index))
                        });
                    }
                    matches.into_iter().map(|(index, _)| index).collect()
                } else {
                    self.candidates
                        .iter()
                        .enumerate()
                        .filter_map(|(index, candidate)| {
                            item_matches(matching, &candidate.value, input).then_some(index)
                        })
                        .collect()
                }
            }
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

    pub fn move_selection_page(&mut self, direction: isize) {
        if self.matches.is_empty() || direction == 0 {
            self.selected = 0;
            return;
        }
        let step = self.max_candidates();
        let last = self.matches.len() - 1;
        self.selected = if direction.is_positive() {
            self.selected.saturating_add(step).min(last)
        } else {
            self.selected.saturating_sub(step)
        };
        self.selection_explicit = true;
    }

    pub fn selected(&self) -> Option<&CompletionCandidate> {
        self.matches
            .get(self.selected)
            .and_then(|index| self.candidates.get(*index))
    }

    pub fn match_count(&self) -> usize {
        self.matches.len()
    }

    pub fn selected_match_number(&self) -> Option<usize> {
        self.has_matches().then_some(self.selected + 1)
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

    fn refresh_file_candidates(&mut self, input: &str) {
        let Some(base_dir) = self.base_dir.as_deref() else {
            self.candidates.clear();
            return;
        };
        let parts = file_completion_parts(base_dir, input);
        let Ok(entries) = fs::read_dir(&parts.search_dir) else {
            self.candidates.clear();
            return;
        };

        let entries = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                let Ok(file_type) = entry.file_type() else {
                    return None;
                };
                Some((name, file_type))
            })
            .collect::<Vec<_>>();
        let use_file_category_matching = self.config.matching == CompletionMatching::Orderless;

        let mut scored_candidates = entries
            .into_iter()
            .filter_map(|(name, file_type)| {
                let score = if use_file_category_matching {
                    file_match_score(&name, &parts.name_prefix)
                } else {
                    item_match_score(self.config.matching, &name, &parts.name_prefix)
                };
                let score = score?;
                let value = format!("{}{}", parts.display_prefix, name);
                let candidate = if file_type.is_dir() {
                    CompletionCandidate::directory(format!("{value}/"))
                } else {
                    CompletionCandidate::file(value)
                };
                Some((candidate, score))
            })
            .collect::<Vec<_>>();
        scored_candidates.sort_by(|(left, left_score), (right, right_score)| {
            left_score
                .cmp(right_score)
                .then_with(|| right.is_directory().cmp(&left.is_directory()))
                .then_with(|| left.value.cmp(&right.value))
        });
        self.candidates = scored_candidates
            .into_iter()
            .map(|(candidate, _)| candidate)
            .collect();
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileCompletionParts {
    search_dir: PathBuf,
    display_prefix: String,
    name_prefix: String,
}

fn file_completion_parts(base_dir: &Path, input: &str) -> FileCompletionParts {
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
    FileCompletionParts {
        search_dir,
        display_prefix: dir_part,
        name_prefix,
    }
}

fn item_matches(matching: CompletionMatching, value: &str, input: &str) -> bool {
    item_match_score(matching, value, input).is_some()
}

fn file_match_score(name: &str, input: &str) -> Option<MatchScore> {
    if input.is_empty() {
        return Some(MatchScore {
            quality: MatchQuality::Prefix,
            component_count: 0,
        });
    }
    if smart_case_eq(name, input) {
        return Some(MatchScore {
            quality: MatchQuality::Exact,
            component_count: 1,
        });
    }
    if smart_case_starts_with(name, input) {
        return Some(MatchScore {
            quality: MatchQuality::Prefix,
            component_count: 1,
        });
    }
    let components = split_file_completion_components(input);
    if components.is_empty() {
        return Some(MatchScore {
            quality: MatchQuality::Prefix,
            component_count: 0,
        });
    }
    if file_word_components_match(name, &components) {
        return Some(MatchScore {
            quality: MatchQuality::WordBoundary,
            component_count: components.len(),
        });
    }
    file_substring_components_match(name, &components).then_some(MatchScore {
        quality: MatchQuality::Substring,
        component_count: components.len(),
    })
}

fn split_file_completion_components(input: &str) -> Vec<&str> {
    input
        .split(|character: char| character == '-' || character.is_ascii_whitespace())
        .filter(|component| !component.is_empty())
        .collect()
}

fn file_word_components_match(name: &str, components: &[&str]) -> bool {
    let words = name
        .split(['-', '_', '.', ' '])
        .filter(|word| !word.is_empty())
        .collect::<Vec<_>>();
    components.iter().all(|component| {
        words
            .iter()
            .any(|word| smart_case_starts_with(word, component))
    })
}

fn file_substring_components_match(name: &str, components: &[&str]) -> bool {
    components
        .iter()
        .all(|component| smart_case_contains(name, component))
}

fn item_match_score(matching: CompletionMatching, value: &str, input: &str) -> Option<MatchScore> {
    match matching {
        CompletionMatching::Orderless => orderless_match_score(value, input),
        CompletionMatching::Prefix => {
            smart_case_starts_with(value, input).then_some(prefix_match_score(value, input))
        }
        CompletionMatching::Substring => {
            smart_case_contains(value, input).then_some(substring_match_score(value, input))
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct MatchScore {
    quality: MatchQuality,
    component_count: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum MatchQuality {
    Exact,
    Prefix,
    WordBoundary,
    Substring,
}

fn prefix_match_score(value: &str, input: &str) -> MatchScore {
    MatchScore {
        quality: literal_match_quality(value, input, is_smart_case_sensitive(input))
            .unwrap_or(MatchQuality::Prefix),
        component_count: 1,
    }
}

fn substring_match_score(value: &str, input: &str) -> MatchScore {
    MatchScore {
        quality: literal_match_quality(value, input, is_smart_case_sensitive(input))
            .unwrap_or(MatchQuality::Substring),
        component_count: 1,
    }
}

fn orderless_match_score(value: &str, input: &str) -> Option<MatchScore> {
    let components = parse_orderless_components(input);
    orderless_match_score_for_components(value, &components)
}

fn orderless_match_score_for_components(
    value: &str,
    components: &[OrderlessComponent],
) -> Option<MatchScore> {
    if components.is_empty() {
        return Some(MatchScore {
            quality: MatchQuality::Prefix,
            component_count: 0,
        });
    }
    let mut quality = MatchQuality::Exact;
    let mut positive_count = 0;
    for component in components {
        let component_quality = component.match_quality(value);
        if component.negated {
            if component_quality.is_some() {
                return None;
            }
        } else {
            quality = quality.max(component_quality?);
            positive_count += 1;
        }
    }
    Some(MatchScore {
        quality: if positive_count == 0 {
            MatchQuality::Prefix
        } else {
            quality
        },
        component_count: positive_count,
    })
}

#[derive(Debug)]
struct OrderlessComponent {
    text: String,
    case_sensitive: bool,
    negated: bool,
    matcher: OrderlessMatcher,
}

#[derive(Debug)]
enum OrderlessMatcher {
    Literal,
    PrefixAnchor,
    SuffixAnchor,
    ExactAnchor,
}

impl OrderlessComponent {
    fn new(component: String) -> Self {
        let (negated, text) = match component.strip_prefix('!') {
            Some(text) => (true, text),
            None => (false, component.as_str()),
        };
        let (force_literal, text) = match text.strip_prefix('=') {
            Some(text) => (true, text),
            None => (false, text),
        };
        let text = text.to_owned();
        let case_sensitive = text.chars().any(char::is_uppercase);
        let matcher = if force_literal {
            OrderlessMatcher::Literal
        } else {
            OrderlessMatcher::for_text(&text)
        };
        Self {
            text,
            case_sensitive,
            negated,
            matcher,
        }
    }

    fn match_quality(&self, value: &str) -> Option<MatchQuality> {
        match &self.matcher {
            OrderlessMatcher::Literal => {
                literal_match_quality(value, &self.text, self.case_sensitive)
            }
            OrderlessMatcher::PrefixAnchor => anchored_prefix_match_quality(
                value,
                self.text
                    .strip_prefix('^')
                    .expect("prefix anchor should start with ^"),
                self.case_sensitive,
            ),
            OrderlessMatcher::SuffixAnchor => anchored_suffix_match_quality(
                value,
                self.text
                    .strip_suffix('$')
                    .expect("suffix anchor should end with $"),
                self.case_sensitive,
            ),
            OrderlessMatcher::ExactAnchor => anchored_exact_match_quality(
                value,
                self.text
                    .strip_prefix('^')
                    .and_then(|text| text.strip_suffix('$'))
                    .expect("exact anchor should be wrapped by ^ and $"),
                self.case_sensitive,
            ),
        }
    }
}

impl OrderlessMatcher {
    fn for_text(text: &str) -> Self {
        if text.len() > 2 && text.starts_with('^') && text.ends_with('$') {
            Self::ExactAnchor
        } else if text.len() > 1 && text.starts_with('^') {
            Self::PrefixAnchor
        } else if text.len() > 1 && text.ends_with('$') {
            Self::SuffixAnchor
        } else {
            Self::Literal
        }
    }
}

fn parse_orderless_components(input: &str) -> Vec<OrderlessComponent> {
    split_orderless_components(input)
        .into_iter()
        .map(OrderlessComponent::new)
        .collect()
}

fn split_orderless_components(input: &str) -> Vec<String> {
    let mut components = Vec::new();
    let mut current = String::new();
    let mut characters = input.chars().peekable();
    while let Some(character) = characters.next() {
        if character == '\\' {
            if characters.peek().is_some_and(char::is_ascii_whitespace) {
                if let Some(escaped) = characters.next() {
                    current.push(escaped);
                }
            } else {
                current.push(character);
            }
        } else if character.is_ascii_whitespace() {
            if !current.is_empty() {
                components.push(std::mem::take(&mut current));
            }
        } else {
            current.push(character);
        }
    }
    if !current.is_empty() {
        components.push(current);
    }
    components
}

fn literal_match_quality(
    value: &str,
    component: &str,
    case_sensitive: bool,
) -> Option<MatchQuality> {
    if component.is_empty() {
        return Some(MatchQuality::Prefix);
    }
    let (value, component) = if case_sensitive {
        (value.to_owned(), component.to_owned())
    } else {
        (value.to_lowercase(), component.to_lowercase())
    };
    if value == component {
        return Some(MatchQuality::Exact);
    }
    if value.starts_with(&component) {
        return Some(MatchQuality::Prefix);
    }
    if word_boundary_contains(&value, &component) {
        return Some(MatchQuality::WordBoundary);
    }
    value
        .contains(&component)
        .then_some(MatchQuality::Substring)
}

fn anchored_prefix_match_quality(
    value: &str,
    component: &str,
    case_sensitive: bool,
) -> Option<MatchQuality> {
    smart_case_starts_with_with_mode(value, component, case_sensitive).then(|| {
        if smart_case_eq_with_mode(value, component, case_sensitive) {
            MatchQuality::Exact
        } else {
            MatchQuality::Prefix
        }
    })
}

fn anchored_suffix_match_quality(
    value: &str,
    component: &str,
    case_sensitive: bool,
) -> Option<MatchQuality> {
    smart_case_ends_with_with_mode(value, component, case_sensitive).then(|| {
        if smart_case_eq_with_mode(value, component, case_sensitive) {
            MatchQuality::Exact
        } else {
            MatchQuality::Substring
        }
    })
}

fn anchored_exact_match_quality(
    value: &str,
    component: &str,
    case_sensitive: bool,
) -> Option<MatchQuality> {
    smart_case_eq_with_mode(value, component, case_sensitive).then_some(MatchQuality::Exact)
}

fn smart_case_starts_with(value: &str, component: &str) -> bool {
    if is_smart_case_sensitive(component) {
        value.starts_with(component)
    } else {
        value.to_lowercase().starts_with(&component.to_lowercase())
    }
}

fn smart_case_contains(value: &str, component: &str) -> bool {
    if is_smart_case_sensitive(component) {
        value.contains(component)
    } else {
        value.to_lowercase().contains(&component.to_lowercase())
    }
}

fn is_smart_case_sensitive(component: &str) -> bool {
    component.chars().any(char::is_uppercase)
}

fn smart_case_eq(value: &str, component: &str) -> bool {
    smart_case_eq_with_mode(value, component, is_smart_case_sensitive(component))
}

fn smart_case_eq_with_mode(value: &str, component: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        value == component
    } else {
        value.to_lowercase() == component.to_lowercase()
    }
}

fn smart_case_starts_with_with_mode(value: &str, component: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        value.starts_with(component)
    } else {
        value.to_lowercase().starts_with(&component.to_lowercase())
    }
}

fn smart_case_ends_with_with_mode(value: &str, component: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        value.ends_with(component)
    } else {
        value.to_lowercase().ends_with(&component.to_lowercase())
    }
}

fn word_boundary_contains(value: &str, component: &str) -> bool {
    value
        .match_indices(component)
        .any(|(index, _)| index > 0 && is_word_boundary(value, index))
}

fn is_word_boundary(value: &str, index: usize) -> bool {
    let mut before = value[..index].chars().rev();
    let Some(previous) = before.next() else {
        return true;
    };
    let Some(current) = value[index..].chars().next() else {
        return false;
    };
    !previous.is_ascii_alphanumeric()
        || (previous.is_ascii_lowercase() && current.is_ascii_uppercase())
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

    use super::{CompletionCandidate, CompletionConfig, CompletionMatching, CompletionSession};
    use crate::command::CommandRegistry;
    use crate::keymap::KeyMap;
    use crate::option::OptionRegistry;

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
        let keymap = KeyMap::default();
        let mut session =
            CompletionSession::commands(&registry, &keymap, CompletionConfig::default());

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
        let keymap = KeyMap::default();
        let mut session =
            CompletionSession::commands(&registry, &keymap, CompletionConfig::default());

        session.update("toggle");

        assert_eq!(session.common_prefix("toggle").as_deref(), Some("toggle-"));
    }

    #[test]
    fn completion_page_selection_moves_by_visible_page_and_clamps() {
        let mut session = CompletionSession::buffers(
            ["one", "two", "three", "four", "five"].map(str::to_owned),
            CompletionConfig {
                max_candidates: 2,
                ..CompletionConfig::default()
            },
        );

        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("one")
        );

        session.move_selection_page(1);
        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("three")
        );

        session.move_selection_page(1);
        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("five")
        );

        session.move_selection_page(1);
        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("five")
        );

        session.move_selection_page(-1);
        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("three")
        );

        session.move_selection_page(-1);
        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("one")
        );
    }

    #[test]
    fn command_completion_records_first_key_binding() {
        let registry = CommandRegistry::default();
        let keymap = KeyMap::default();
        let mut session =
            CompletionSession::commands(&registry, &keymap, CompletionConfig::default());

        session.update("save-buffer");

        assert_eq!(
            session.selected().map(CompletionCandidate::display_label),
            Some("save-buffer (C-x C-s)".to_owned())
        );
    }

    #[test]
    fn command_completion_uses_command_summaries() {
        let registry = CommandRegistry::default();
        let keymap = KeyMap::default();
        let mut session =
            CompletionSession::commands(&registry, &keymap, CompletionConfig::default());

        session.update("save-buffer");

        let candidate = session.selected().expect("save-buffer should be selected");
        assert_eq!(candidate.annotation, "Save current buffer");
    }

    #[test]
    fn orderless_completion_requires_all_components_in_any_order() {
        let mut session = CompletionSession::buffers(
            [
                "copy-rectangle-to-register".to_owned(),
                "rectangle-number-lines".to_owned(),
                "insert-register".to_owned(),
                "string-rectangle".to_owned(),
            ],
            CompletionConfig::default(),
        );

        session.update("rect reg");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["copy-rectangle-to-register"]);

        session.update("reg rect");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["copy-rectangle-to-register"]);
    }

    #[test]
    fn orderless_completion_matches_short_components_in_any_order() {
        let mut session = CompletionSession::buffers(
            [
                "readme.md".to_owned(),
                "reader.org".to_owned(),
                "manual.md".to_owned(),
            ],
            CompletionConfig::default(),
        );

        session.update("re md");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["readme.md"]);

        session.update("md re");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["readme.md"]);
    }

    #[test]
    fn orderless_completion_uses_space_components_not_hyphen_shorthand() {
        let mut session = CompletionSession::buffers(
            [
                "pass-coffin-open-timer".to_owned(),
                "pass-coffin-close".to_owned(),
                "passphrase-cache".to_owned(),
            ],
            CompletionConfig::default(),
        );

        session.update("p c o t");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["pass-coffin-open-timer"]);

        session.update("pass cof");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["pass-coffin-close", "pass-coffin-open-timer"]);

        session.update("p-c-o-t");
        assert_eq!(session.match_count(), 0);

        session.update("passcof");
        assert_eq!(session.match_count(), 0);
    }

    #[test]
    fn orderless_completion_preserves_escaped_space_components() {
        let mut session = CompletionSession::buffers(
            [
                "alpha beta final.txt".to_owned(),
                "alpha beta draft.txt".to_owned(),
                "alpha-gamma beta final.txt".to_owned(),
                "alpha.txt".to_owned(),
            ],
            CompletionConfig::default(),
        );

        session.update(r"alpha\ beta final");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["alpha beta final.txt"]);

        session.update(r"final alpha\ beta");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["alpha beta final.txt"]);
    }

    #[test]
    fn orderless_completion_uses_smart_case() {
        let mut session = CompletionSession::buffers(
            ["find-file".to_owned(), "Find-Function".to_owned()],
            CompletionConfig::default(),
        );

        session.update("find");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["find-file", "Find-Function"]);

        session.update("Find");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["Find-Function"]);
    }

    #[test]
    fn orderless_completion_supports_simple_literal_anchors() {
        let mut session = CompletionSession::buffers(
            [
                "find-file".to_owned(),
                "project-find-file".to_owned(),
                "find-file-read-only".to_owned(),
            ],
            CompletionConfig::default(),
        );

        session.update("^find");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["find-file", "find-file-read-only"]);

        session.update("file$");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["find-file", "project-find-file"]);

        session.update("^find-file$");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["find-file"]);
    }

    #[test]
    fn orderless_completion_treats_regex_metacharacters_as_literal_text() {
        let mut session = CompletionSession::buffers(
            [
                "foo.txt".to_owned(),
                "fooxtxt".to_owned(),
                "foo|bar".to_owned(),
                "foo-bar".to_owned(),
                "literal-[abc]".to_owned(),
                "literal-a".to_owned(),
                "literal-.*".to_owned(),
                "literal-anything".to_owned(),
            ],
            CompletionConfig::default(),
        );

        session.update("foo.txt");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["foo.txt"]);

        session.update("foo|bar");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["foo|bar"]);

        session.update("[abc]");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["literal-[abc]"]);

        session.update(".*");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["literal-.*"]);
    }

    #[test]
    fn orderless_completion_treats_bare_anchors_as_literal_text() {
        let mut session = CompletionSession::buffers(
            [
                "^".to_owned(),
                "^literal".to_owned(),
                "$".to_owned(),
                "literal$".to_owned(),
                "literal".to_owned(),
            ],
            CompletionConfig::default(),
        );

        session.update("^");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["^", "^literal"]);

        session.update("$");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["$", "literal$"]);
    }

    #[test]
    fn orderless_completion_supports_negation() {
        let mut session = CompletionSession::buffers(
            [
                "find-file".to_owned(),
                "find-file-read-only".to_owned(),
                "project-find-file".to_owned(),
                "read-file-name".to_owned(),
            ],
            CompletionConfig::default(),
        );

        session.update("find !read");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["find-file", "project-find-file"]);
    }

    #[test]
    fn orderless_completion_filters_negation_only_without_reordering() {
        let mut session = CompletionSession::buffers(
            [
                "toggle-syntax-highlighting".to_owned(),
                "toggle-search-highlighting".to_owned(),
                "toggle-line-numbers".to_owned(),
            ],
            CompletionConfig::default(),
        );

        session.update("!syntax");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            vec!["toggle-search-highlighting", "toggle-line-numbers"]
        );
    }

    #[test]
    fn orderless_completion_supports_force_literal() {
        let mut session = CompletionSession::buffers(
            ["foo.txt".to_owned(), "fooxtxt".to_owned()],
            CompletionConfig::default(),
        );

        session.update("foo.txt");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["foo.txt"]);

        session.update("=foo.txt");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["foo.txt"]);
    }

    #[test]
    fn orderless_completion_composes_negation_and_force_literal() {
        let mut session = CompletionSession::buffers(
            ["foo.txt".to_owned(), "fooxtxt".to_owned()],
            CompletionConfig::default(),
        );

        session.update("!=foo.txt");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["fooxtxt"]);

        session.update("=!foo");
        assert_eq!(session.match_count(), 0);
    }

    #[test]
    fn orderless_completion_uses_smart_case_for_operators() {
        let mut session = CompletionSession::buffers(
            ["find-file".to_owned(), "Find-Function".to_owned()],
            CompletionConfig::default(),
        );

        session.update("=find");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["find-file", "Find-Function"]);

        session.update("=Find");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["Find-Function"]);

        session.update("!Find");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["find-file"]);
    }

    #[test]
    fn orderless_completion_force_literal_preserves_anchor_text() {
        let mut session = CompletionSession::buffers(
            [
                "^find".to_owned(),
                "find-file".to_owned(),
                "file$".to_owned(),
                "project-file".to_owned(),
            ],
            CompletionConfig::default(),
        );

        session.update("=^find");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["^find"]);

        session.update("=file$");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["file$"]);
    }

    #[test]
    fn orderless_completion_ranks_exact_prefix_before_substring() {
        let mut session = CompletionSession::buffers(
            [
                "project-find-file".to_owned(),
                "find-file-read-only".to_owned(),
                "find-file".to_owned(),
            ],
            CompletionConfig::default(),
        );

        session.update("find-file");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            vec!["find-file", "find-file-read-only", "project-find-file"]
        );
    }

    #[test]
    fn option_completion_filters_and_uses_summaries() {
        let registry = OptionRegistry::default();
        let mut session = CompletionSession::options(&registry, CompletionConfig::default());

        session.update("completion_mat");

        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("completion_matching")
        );
        assert_eq!(
            session
                .selected()
                .map(|candidate| candidate.annotation.as_str()),
            Some("Completion matching")
        );
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

    #[test]
    fn default_file_completion_uses_substring_matching() {
        let directory = TestDir::new();
        fs::write(directory.path().join("NOTICE.md"), "notice").expect("fixture should write");
        fs::write(directory.path().join("README.md"), "readme").expect("fixture should write");
        let mut session = CompletionSession::files(directory.path(), CompletionConfig::default());

        session.update("tice");

        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["NOTICE.md"]);
    }

    #[test]
    fn default_file_completion_uses_partial_completion() {
        let directory = TestDir::new();
        fs::write(directory.path().join("alpha-note.txt"), "alpha").expect("fixture should write");
        fs::write(directory.path().join("alphabet.txt"), "alphabet").expect("fixture should write");
        fs::write(directory.path().join("README.md"), "readme").expect("fixture should write");
        let mut session = CompletionSession::files(directory.path(), CompletionConfig::default());

        session.update("a-n");

        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("alpha-note.txt")
        );

        session.update("note");
        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("alpha-note.txt")
        );

        session.update("re me");
        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("README.md")
        );

        session.update("me re");
        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("README.md")
        );

        session.update("re-md");
        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("README.md")
        );
    }

    #[test]
    fn default_file_completion_allows_unordered_file_components() {
        let directory = TestDir::new();
        fs::write(directory.path().join("README.md"), "readme").expect("fixture should write");
        fs::write(directory.path().join("manual.md"), "manual").expect("fixture should write");
        let mut session = CompletionSession::files(directory.path(), CompletionConfig::default());

        session.update("md re");

        assert_eq!(
            session.selected().map(|candidate| candidate.value.as_str()),
            Some("README.md")
        );
    }

    #[test]
    fn default_file_completion_uses_smart_case() {
        let directory = TestDir::new();
        fs::write(directory.path().join("README.md"), "upper").expect("fixture should write");
        fs::write(directory.path().join("ReadMe.txt"), "mixed").expect("fixture should write");
        fs::write(directory.path().join("readme.org"), "lower").expect("fixture should write");
        let mut session = CompletionSession::files(directory.path(), CompletionConfig::default());

        session.update("read");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["README.md", "ReadMe.txt", "readme.org"]);

        session.update("Read");
        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["ReadMe.txt"]);
    }

    #[test]
    fn prefix_and_substring_matching_use_smart_case() {
        let mut prefix_session = CompletionSession::buffers(
            [
                "README.md".to_owned(),
                "ReadMe.txt".to_owned(),
                "readme.org".to_owned(),
            ],
            CompletionConfig {
                matching: CompletionMatching::Prefix,
                ..CompletionConfig::default()
            },
        );

        prefix_session.update("read");
        let values = prefix_session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["README.md", "ReadMe.txt", "readme.org"]);

        prefix_session.update("Read");
        let values = prefix_session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["ReadMe.txt"]);

        let mut substring_session = CompletionSession::buffers(
            ["notes-README.md".to_owned(), "notes-ReadMe.txt".to_owned()],
            CompletionConfig {
                matching: CompletionMatching::Substring,
                ..CompletionConfig::default()
            },
        );

        substring_session.update("read");
        let values = substring_session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(values, vec!["notes-README.md", "notes-ReadMe.txt"]);

        substring_session.update("Read");
        assert_eq!(
            substring_session
                .selected()
                .map(|candidate| candidate.value.as_str()),
            Some("notes-ReadMe.txt")
        );
    }

    #[test]
    fn substring_matching_can_include_non_prefix_matches() {
        let mut session = CompletionSession::buffers(
            [
                "alpha-buffer.txt".to_owned(),
                "notes-alpha.txt".to_owned(),
                "alphabet-buffer.txt".to_owned(),
            ],
            CompletionConfig {
                matching: CompletionMatching::Substring,
                ..CompletionConfig::default()
            },
        );

        session.update("alpha");

        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            vec!["alpha-buffer.txt", "notes-alpha.txt", "alphabet-buffer.txt"]
        );
    }

    #[test]
    fn buffer_completion_filters_buffer_names() {
        let mut session = CompletionSession::buffers(
            [
                "notes.txt".to_owned(),
                "alpha-buffer.txt".to_owned(),
                "alphabet-buffer.txt".to_owned(),
                "notes-alpha.txt".to_owned(),
            ],
            CompletionConfig::default(),
        );

        session.update("alpha");

        let values = session
            .view_items()
            .into_iter()
            .map(|item| item.candidate.value.as_str().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(
            values,
            vec!["alpha-buffer.txt", "alphabet-buffer.txt", "notes-alpha.txt"]
        );

        session.update("alpha-b");

        assert_eq!(
            session.common_prefix("alpha-b").as_deref(),
            Some("alpha-buffer.txt")
        );
    }
}
