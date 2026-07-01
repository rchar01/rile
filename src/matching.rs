// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

pub(crate) fn is_smart_case_sensitive(pattern: &str) -> bool {
    pattern.chars().any(char::is_uppercase)
}

pub(crate) fn smart_case_eq(value: &str, pattern: &str) -> bool {
    smart_case_eq_with_mode(value, pattern, is_smart_case_sensitive(pattern))
}

pub(crate) fn smart_case_contains(value: &str, pattern: &str) -> bool {
    smart_case_contains_with_mode(value, pattern, is_smart_case_sensitive(pattern))
}

pub(crate) fn smart_case_starts_with(value: &str, pattern: &str) -> bool {
    smart_case_starts_with_with_mode(value, pattern, is_smart_case_sensitive(pattern))
}

pub(crate) fn smart_case_eq_with_mode(value: &str, pattern: &str, case_sensitive: bool) -> bool {
    if case_sensitive {
        value == pattern
    } else {
        value.to_lowercase() == pattern.to_lowercase()
    }
}

pub(crate) fn smart_case_contains_with_mode(
    value: &str,
    pattern: &str,
    case_sensitive: bool,
) -> bool {
    if case_sensitive {
        value.contains(pattern)
    } else {
        value.to_lowercase().contains(&pattern.to_lowercase())
    }
}

pub(crate) fn smart_case_starts_with_with_mode(
    value: &str,
    pattern: &str,
    case_sensitive: bool,
) -> bool {
    if case_sensitive {
        value.starts_with(pattern)
    } else {
        value.to_lowercase().starts_with(&pattern.to_lowercase())
    }
}

pub(crate) fn smart_case_ends_with_with_mode(
    value: &str,
    pattern: &str,
    case_sensitive: bool,
) -> bool {
    if case_sensitive {
        value.ends_with(pattern)
    } else {
        value.to_lowercase().ends_with(&pattern.to_lowercase())
    }
}
