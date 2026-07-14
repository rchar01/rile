// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

use std::process::Command;

#[test]
fn startup_diagnostics_escape_terminal_controls() {
    let binary = assert_cmd::cargo::cargo_bin("rile");
    let output = Command::new(binary)
        .arg("--bad_\u{1b}]0;CLI_PWN\u{7}")
        .output()
        .expect("rile should run");

    assert!(!output.status.success());
    assert!(!contains_bytes(&output.stderr, b"\x1b]0;CLI_PWN\x07"));
    assert!(
        String::from_utf8(output.stderr)
            .expect("stderr should be UTF-8")
            .contains("--bad_\\u{1b}]0;CLI_PWN\\u{7}")
    );
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}
