<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Backups and Auto-Save

This guide describes Rile's current save-backup and auto-save behavior.  These
features are separate: backups preserve the previous on-disk file when the user
explicitly saves, while auto-save files preserve unsaved buffer contents between
explicit saves.

## Defaults

Both safety features are disabled by default for new configurations:

```toml
backup_on_save = false
backup_directory = ""

auto_save = false
auto_save_interval = 300
auto_save_timeout_seconds = 30
auto_save_directory = ""
delete_auto_save_files = true
```

The empty directory settings mean Rile writes sibling safety files beside the
visited file.  Non-empty directory settings must name existing directories when
the safety file is written.

## Save Backups

When `backup_on_save` is true, Rile writes one persistent backup per buffer
visit before the first save attempt that successfully creates a backup.  The
backup contains the original file contents from before that attempt.  Later
saves during the same visit do not replace that backup, including a retry after
the backup succeeded but the visited-file write failed.

With an empty `backup_directory`, backups use the sibling Emacs-style name
`file~`.  With a configured backup directory, Rile writes a mapped path-based
name into that directory.  Directory separators in the visited path are replaced
with `!`, and the mapped name ends in `~`; for example, a configured backup
directory may receive a file like `!home!user!notes.txt~`.

Backup creation is part of the save safety path.  If Rile cannot create the
backup, the explicit save fails and the buffer remains dirty.  This keeps the
original visited file unchanged when the configured backup policy cannot be
honored.

On Unix, every backup uses mode `0600`, regardless of the visited file's mode,
parent-directory protections, configured backup directory, or an older backup's
mode.  Rile opens an existing backup source without following a final symbolic
link and reads its metadata and bytes from that one file descriptor.  This keeps
the backup bound to the regular file checked at the start of the save.

Files opened through a final symbolic link remain available for viewing, but
Rile rejects attempts to save them whether or not backups are enabled.  The link
and its target remain unchanged and the buffer remains dirty.  Existing final
paths of other non-regular types, such as FIFOs, are rejected as well.

`save_as` starts a new backup cycle only when the destination has a different
visited-path identity.  Retargeting through an alias of the same file preserves
the one-backup-per-visit contract.

## Auto-Save Files

When `auto_save` is true, dirty file-visiting buffers write Emacs-style
auto-save files.  Auto-save currently applies only to buffers that visit files;
unnamed scratch-style buffers are not written to generated recovery names.

With an empty `auto_save_directory`, auto-save files use sibling `#file#` names.
With a configured auto-save directory, Rile writes mapped path-based names
wrapped in `#...#`, using the same `!` separator replacement as configured
backup directories.

Auto-save writes are non-cleaning writes.  They serialize the current buffer
contents to the auto-save file, but they do not modify the visited file and do
not mark the buffer clean.  On Unix, a new auto-save file inherits the visited
file's permissions.  Rewriting an existing auto-save uses the intersection of
the visited and recovery file modes so the write cannot make either policy more
permissive.  Auto-save files for not-yet-created visited files use mode `0600`.

Auto-save can be triggered in two ways:

- `auto_save_interval` counts handled key events.  A value of `0` disables this
  trigger.
- `auto_save_timeout_seconds` writes dirty buffers after editor idle time.  A
  value of `0` disables this trigger.

Auto-save write failures are non-fatal.  Rile reports the error and keeps the
buffer dirty so editing can continue.

## Cleanup and Recovery

When `delete_auto_save_files` is true, successful explicit saves and successful
reverts delete only auto-save files written by the current Rile session for that
buffer.  Pre-existing auto-save files are preserved so Rile does not erase a
possible recovery file from a previous crash or interrupted session.

Cleanup records include the exact path and file stamp written by the session.
If Rile observes that pathname was repurposed or replaced, it preserves the
current file.  The visited-file write remains committed and clean, while the
editor reports an auto-save cleanup warning and retains other pending cleanup
records for later retries.  A narrow race remains if another process replaces
the pathname between the identity check and deletion.

Opening a file checks whether its corresponding auto-save file is strictly newer
than the visited file.  If it is newer, Rile warns that the auto-save file can be
opened manually for recovery.  Rile does not yet provide an interactive
`recover-file` command.

Equal modification times are not treated as newer.  This avoids warning about a
stale auto-save file after a successful save on filesystems with coarse timestamp
resolution.

## Implementation Touchpoints

The main implementation lives in these modules:

- `src/file.rs`: document settings, backup and auto-save path calculation,
  safety-file writes, cleanup, and newer-auto-save detection.
- `src/editor.rs`: runtime auto-save counters, idle polling, option exposure,
  and open-time newer-auto-save warnings.
- `src/config.rs`: config-file parsing and default runtime settings.
- `src/option.rs`: user-visible option metadata, parsing, and validation.
- `src/buffers.rs`: propagation of document settings when opening files.

The primary tests are unit tests beside those modules plus PTY coverage in
`tests/pty_save.rs` for config-loaded auto-save behavior.
