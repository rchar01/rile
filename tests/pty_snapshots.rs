// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use anyhow::Result;

use support::{fixtures, keys, pty::RilePty};

fn control_x_text(text: &str) -> Vec<u8> {
    let mut sequence = keys::control('x').to_vec();
    sequence.extend_from_slice(text.as_bytes());
    sequence
}

fn snapshot_tests_enabled() -> bool {
    std::env::var_os("RILE_SNAPSHOT_TEST").is_some()
}

#[test]
fn snapshot_open_numbered_50x10() -> Result<()> {
    if !snapshot_tests_enabled() {
        return Ok(());
    }

    let file = fixtures::fixture_path("numbered.txt");
    let mut rile = RilePty::spawn(&file, 10, 50)?;

    rile.wait_for_screen_contains("numbered.txt")?;
    insta::assert_snapshot!("open_numbered_50x10", rile.snapshot_screen());
    rile.quit()?;

    Ok(())
}

#[test]
fn snapshot_movement_after_c_n_c_f_60x12() -> Result<()> {
    if !snapshot_tests_enabled() {
        return Ok(());
    }

    let file = fixtures::fixture_path("numbered.txt");
    let mut rile = RilePty::spawn(&file, 12, 60)?;

    rile.wait_for_screen_contains("numbered.txt")?;
    rile.send("C-n", keys::control('n'))?;
    rile.send("C-n", keys::control('n'))?;
    rile.send("C-f", keys::control('f'))?;
    rile.send("C-f", keys::control('f'))?;

    insta::assert_snapshot!("movement_after_c_n_c_f_60x12", rile.snapshot_screen());
    rile.quit()?;

    Ok(())
}

#[test]
fn snapshot_split_right_after_other_window_80x16() -> Result<()> {
    if !snapshot_tests_enabled() {
        return Ok(());
    }

    let file = fixtures::fixture_path("split_left.txt");
    let mut rile = RilePty::spawn(&file, 16, 80)?;

    rile.wait_for_screen_contains("split_left.txt")?;
    rile.send("C-x 3", control_x_text("3"))?;
    rile.send("C-x o", control_x_text("o"))?;

    insta::assert_snapshot!(
        "split_right_after_other_window_80x16",
        rile.snapshot_screen()
    );
    rile.quit()?;

    Ok(())
}

#[test]
fn snapshot_split_below_after_other_window_80x16() -> Result<()> {
    if !snapshot_tests_enabled() {
        return Ok(());
    }

    let file = fixtures::fixture_path("split_left.txt");
    let mut rile = RilePty::spawn(&file, 16, 80)?;

    rile.wait_for_screen_contains("split_left.txt")?;
    rile.send("C-x 2", control_x_text("2"))?;
    rile.send("C-x o", control_x_text("o"))?;

    insta::assert_snapshot!(
        "split_below_after_other_window_80x16",
        rile.snapshot_screen()
    );
    rile.quit()?;

    Ok(())
}
