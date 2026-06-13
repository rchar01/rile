// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;

use support::{fixtures, pty::RilePty};

#[test]
fn failed_cursor_assertion_includes_readable_screen_dump() -> Result<()> {
    let file = fixtures::fixture_path("numbered.txt");
    let mut rile = RilePty::spawn(&file, 10, 50)?;

    rile.wait_for_screen_contains("numbered.txt")?;
    let error = rile
        .assert_cursor(9, 49)
        .expect_err("wrong cursor assertion should fail");
    let message = error.to_string();

    assert!(message.contains("cursor was at"), "{message}");
    assert!(message.contains("expected (9, 49)"), "{message}");
    assert!(message.contains("after spawn"), "{message}");
    assert!(message.contains("001: 000 | 0123456789"), "{message}");
    assert!(message.contains("     ^"), "{message}");

    rile.quit()?;
    Ok(())
}
