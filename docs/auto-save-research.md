<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Auto-Save Research

This note records reference-editor evidence used for Rile's auto-save and
recovery design.  It separates auto-save from save backups: auto-save protects
unsaved edits after a crash, while backups preserve the previous on-disk file
when the user explicitly saves.

## Summary

- GNU Emacs enables classic auto-save by default for file-visiting buffers.
- Emacs normally writes sibling `#file#` files, deletes them after a successful
  save, and offers `recover-file` / `recover-session` recovery commands.
- Zile 2.6.x does not show evidence of periodic Emacs-style `#file#`
  auto-save.  Instead, it writes `<name>.ZILESAVE` on abnormal exit for dirty
  buffers.
- Zile documents `file~` save backups through `make-backup-files` and
  `backup-directory`; this is separate from `.ZILESAVE` crash saving.
- Rile implements Emacs-style `#file#` auto-save as a default-off feature with
  cleanup of current-session auto-save files after explicit saves and an
  open-time warning when a newer auto-save file exists.  A richer interactive
  recovery command remains future work.

## Reference Evidence

### GNU Emacs 30.2

Local runtime introspection with `emacs --batch -Q` showed these defaults:

- `auto-save-default = t`
- `auto-save-interval = 300`
- `auto-save-timeout = 30`
- `delete-auto-save-files = t`
- `auto-save-file-name-transforms` maps remote files to the temporary directory
  by default.

Variable documentation says:

- `auto-save-default`: non-nil means auto-save every file-visiting buffer by
  default.
- `auto-save-interval`: number of input events between auto-saves; zero
  disables event-count auto-save.
- `auto-save-timeout`: idle seconds before auto-save; zero or nil disables idle
  auto-save.
- `delete-auto-save-files`: non-nil deletes the auto-save file when a buffer is
  saved.
- `auto-save-file-name-transforms`: transform rules may uniquify names by
  replacing directory separators with `!`.

Local behavior captures:

- For an existing visited file, enabling auto-save mode and calling
  `do-auto-save` created sibling `#foo.txt#`; the original `foo.txt` stayed
  unchanged.
- `save-buffer` deleted `#foo.txt#` when `delete-auto-save-files` was true.
- Exiting without saving left `#foo.txt#` available for recovery.
- A configured transform directory produced names like
  `#!tmp!opencode!example!foo.txt#`.
- A new unsaved buffer used a generated name like `#scratch-test#3ZYumS#` in
  the buffer's default directory.
- An unwritable target directory reported a non-fatal auto-save error and did
  not create an auto-save file.
- Editing a symlink path named the auto-save file from the visited symlink path,
  not from the resolved target: `linkdir/foo-link.txt` created
  `linkdir/#foo-link.txt#`.

Reference documentation validated by web research:

- GNU Emacs Manual, `Auto Save`, `Auto Save Files`, and `Recover` nodes.
- GNU Emacs Lisp Reference, `Auto-Saving` node.
- Emacs 30.1 source mirror, `lisp/files.el`, for default variable definitions
  and file-name transform behavior.

### GNU Zile 2.6.x

The installed local package is GNU Zile 2.6.2.  The official GNU 2.6.4 release
tarball was downloaded from `https://ftp.gnu.org/gnu/zile/zile-2.6.4.tar.gz`
and inspected under `/tmp/opencode`.

Zile 2.6.4 source evidence:

- `src/main.vala` writes dirty buffers on abnormal exit as
  `"%s.%sSAVE".printf (bp.get_filename_or_name (), PACKAGE.up ())`.  For the
  package name `zile`, this yields `<name>.ZILESAVE`.
- `src/main.vala` installs handlers for `SIGHUP`, `SIGINT`, and `SIGTERM` that
  call this abnormal-exit save path.  `SIGSEGV` and `SIGBUS` also attempt this
  path before aborting.
- `src/tbl_vars.vala` defines `make-backup-files` with default `t` and says it
  makes a backup the first time a file is saved by appending `~`.
- `src/tbl_vars.vala` defines `backup-directory` with default `nil`.
- `src/file.vala` maps backup-directory names by replacing `/` with `!` and
  appending `~`.
- Repository-wide source searches for `auto-save`, `autosave`, `recover`, and
  `preserve` found no periodic Emacs-style auto-save implementation in the
  2.6.4 `src/*.vala` files.

Local Zile 2.6.2 runtime captures:

- Editing `foo.txt`, typing unsaved text, waiting longer than 30 seconds, and
  killing the process with `timeout` did not create `#foo.txt#`.
- The same abnormal termination created `foo.txt.ZILESAVE` containing unsaved
  buffer contents; the original `foo.txt` stayed unchanged.
- Saving with `C-x C-s` changed `foo.txt` and did not leave `#foo.txt#`.

Observed discrepancy:

- Zile 2.6.x documents `make-backup-files = t`, but the local 2.6.2 save probe
  did not leave a `file~` backup.  The 2.6.4 source still guards backup creation
  with `bp.backup && make-backup-files` while the observed runtime starts from a
  false backup flag.  Treat Zile's documented backup defaults as intent, but do
  not rely on the local package's runtime backup behavior as authoritative.

## Behavior Matrix

| Case | Emacs 30.2 | Zile 2.6.x | Rile Recommendation |
| --- | --- | --- | --- |
| Existing dirty file | Periodic `#file#` auto-save | No `#file#`; `.ZILESAVE` only on abnormal exit | Implement `#file#` separately from backups |
| Explicit save | Deletes auto-save file | Saves file; no `#file#` observed | Delete auto-save file after successful save |
| Crash or signal | Leaves auto-save file for recovery | Writes `<name>.ZILESAVE` on signal/crash path | Prefer Emacs `#file#`; consider `.RILESAVE` only as later crash hook |
| New unsaved buffer | Generated `#buffer#random#` in default directory | Writes name-derived `.ZILESAVE` on abnormal exit | Defer or add separate unnamed-buffer policy |
| Unwritable directory | Non-fatal auto-save error | Not tested for `.ZILESAVE` | Report non-fatal status; keep editing |
| Symlink path | Names from visited path | Not tested | Name from visited path for user predictability |
| Directory mapping | Uses transform rules; `!` uniquifies path | Backups use `/` to `!` mapping | Reuse Rile backup-directory mapping style if adding auto-save directory |

## Rile Recommendation

Rile implements a classic Emacs-style auto-save feature separately from save
backups.

Recommended defaults:

- `auto_save = false` initially, because the first implementation warns about
  newer auto-save files but does not yet provide an interactive recovery command.
- Consider `auto_save = true` later if recovery detection, prompt, cleanup, and
  tests include a complete interactive recovery flow.

Recommended options:

- `auto_save`: boolean.
- `auto_save_interval`: input/change count threshold, default 300 if enabled.
- `auto_save_timeout_seconds`: idle threshold, default 30 if enabled.
- `auto_save_directory`: optional directory; empty means sibling `#file#`.
- `delete_auto_save_files`: boolean, default true for auto-save files written by
  the current session.

Recommended naming:

- Existing visited file with sibling policy: `#file#`.
- Directory policy: wrap a mapped path in `#...#`, using the same `!` escaping
  strategy already used for backup-directory names.
- New unsaved buffers: defer initially, or use Emacs-like generated names only
  after there is a discoverable recovery list.

Recommended runtime behavior:

- Track dirty buffers separately from the explicit-save backup state.
- Write auto-save files after enough input changes or idle time.
- Auto-save writes must not mark the buffer clean and must not change the
  visited file.
- Auto-save write failures should be non-fatal and should not block editing.
- Successful explicit save should delete the buffer's current-session auto-save
  file when `delete_auto_save_files` is true, while preserving pre-existing
  recovery files.
- Opening a file with a newer `#file#` warns before silently overwriting the
  auto-save file; an interactive recovery command is deferred.

## Minimum Implementation Checklist

- [x] Add config and option metadata for the approved auto-save settings.
- [x] Add per-document auto-save path calculation for sibling and configured
  directory policies.
- [x] Add auto-save write logic that serializes current buffer contents without
  changing modified state.
- [x] Add editor-loop triggers for input-count and idle-time thresholds.
- [x] Delete auto-save files after successful explicit save when configured.
- [x] Detect stale/newer auto-save files on file open and expose a conservative
  warning.
- [x] Add unit tests for naming, cleanup, write failures, and dirty-state
  preservation.
- [x] Add PTY tests for visible auto-save behavior.
- [x] Document user-visible behavior in `README.md`, `NEWS`,
  `docs/development.md`, and `ChangeLog` when implemented.

## Commands Used

- `emacs --batch -Q --eval ...` for default variables and function docs.
- `emacs --batch -Q <file> --eval ...` for auto-save file creation and cleanup
  captures.
- `zile --version` and `zile --help` for local package identity.
- `apt source zile` for Debian's 2.6.2 source package.
- `curl -L https://ftp.gnu.org/gnu/zile/zile-2.6.4.tar.gz` for official Zile
  2.6.4 source inspection.
- `script -qfec 'zile -q <file>'` with isolated temp directories for real Zile
  terminal behavior captures.
