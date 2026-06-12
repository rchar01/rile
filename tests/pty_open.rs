// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;
use predicates::prelude::*;

use support::{fixtures, pty::RilePty};

#[test]
fn opens_visual_fixture_in_pty() -> Result<()> {
    let file = fixtures::fixture_path("numbered.txt");
    let mut rile = RilePty::spawn(&file, 10, 50)?;

    rile.wait_for_screen_contains("numbered.txt")?;
    rile.assert_screen_contains("000 | 0123456789")?;
    assert!(
        predicate::str::contains("Rile VISUAL").eval(&rile.snapshot_text()),
        "{}",
        rile.screen_dump()
    );
    rile.assert_status_contains("Rile VISUAL")?;
    rile.assert_status_contains("ACTIVE numbered.txt")?;
    rile.assert_cursor(0, 0)?;
    insta::assert_snapshot!(format!("{:?}", rile.cursor_position()), @r"(0, 0)");
    rile.quit()?;

    Ok(())
}
