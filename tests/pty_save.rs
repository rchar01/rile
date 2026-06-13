// SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
// SPDX-License-Identifier: GPL-3.0-or-later

mod support;

use std::fs;

use anyhow::{Context, Result};

use support::{fixtures, keys, pty::RilePty};

#[test]
fn save_buffer_writes_disk_contents_and_clears_dirty_state() -> Result<()> {
    let file = fixtures::named_temp_file("alpha\nbeta\n")?;
    let path = file.path().to_path_buf();
    let mut rile = RilePty::spawn(&path, 12, 80)?;

    rile.wait_for_screen_contains("alpha")?;
    rile.assert_status_contains("modified:false")?;

    rile.send("insert text", b"saved ")?;
    rile.wait_for_screen_contains("saved alpha")?;
    rile.assert_status_contains("modified:true")?;

    rile.send("C-x C-s", keys::control_sequence("xs"))?;
    rile.wait_for_screen_contains("Wrote")?;
    rile.assert_status_contains("modified:false")?;

    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read saved file {}", path.display()))?;
    assert_eq!(contents, "saved alpha\nbeta\n");

    rile.quit()?;
    Ok(())
}
