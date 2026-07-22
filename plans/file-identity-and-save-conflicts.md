<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Plan: Strengthen File Identity and Save Conflicts

## Goal

Track optional platform-specific hardening after path aliases, buffer ownership,
and sequential stale saves have been addressed.

## Current Context

- [x] Reuse saveable files across relative, `..`, and symlinked-parent aliases.
  Evidence: `fb94f17`; buffer-manager alias and identity-transition tests.
- [x] Keep final symbolic links view-only and hard-link pathnames distinct.
  Evidence: Unix buffer-manager and file-save regressions.
- [x] Reject ordinary saves after modification, creation, deletion, or pathname
  replacement. Evidence: file unit tests and the real-terminal PTY regression.
- [x] Reject `write-file` destinations owned by another live buffer and update
  manager-owned names after successful retargeting.
- [x] Bind auto-save cleanup to exact file stamps and preserve committed editor
  state when cleanup reports a warning.

## Remaining Risks

- Portable `rename` cannot atomically assert that a destination still has the
  expected identity. Rile checks immediately before rename, leaving a narrow
  check-to-rename race against a concurrent writer.
- Non-Unix file stamps currently use length and modification time without a
  stable file identifier or change time. Same-size changes on coarse-timestamp
  filesystems may evade detection.
- Missing paths whose parents do not yet exist use a best-effort absolute key.
  Buffer ownership is recomputed when the path becomes resolvable, but identity
  before that transition cannot predict future symlink creation.
- Auto-save cleanup checks identity before deletion but retains a narrow
  check-to-delete race against concurrent pathname replacement.
- There is no explicit force-save command after a legitimate conflict. Users
  must reload, resolve externally, or use `write-file` with another destination.

## Optional Follow-Up

### Phase 1: Platform File Identity

- [ ] Decide whether non-Unix identity work is preparatory crate portability or
  part of an intended editor platform-support commitment; current terminal
  operation remains Unix-oriented.
- [ ] Add stable Windows file identity and high-resolution change metadata.
- [ ] Define and test the conservative fallback for targets without stable file
  identifiers. Supported save targets must either detect pathname replacement
  or document that stale-save protection is best-effort.
- [ ] Add target-specific replacement and same-size modification regressions.

### Phase 2: Conditional Replacement Research

- [ ] Evaluate Linux and other supported-platform primitives for replacing a
  destination only when its expected identity remains current.
- [ ] Preserve same-directory atomicity, permissions, backup ordering, and
  final-symlink rejection in any platform-specific implementation.
- [ ] Add deterministic race hooks before replacement and cleanup deletion.

### Phase 3: Explicit Conflict Resolution

- [ ] Design a deliberate force-save command or confirmation flow that cannot
  be triggered accidentally by repeating `save-buffer`.
- [ ] Show the changed pathname and preserve recovery data before overwriting.
- [ ] Cover external modification, deletion, creation, and alias conflicts in
  unit and PTY tests.

## Non-Goals

- Do not silently overwrite a conflict on a second ordinary save.
- Do not merge hard-link pathnames while saves use atomic pathname replacement.
- Do not make final-symlink save policy depend on buffer-open order.
- Do not retain complete baseline file contents solely for conflict detection.

## Validation

- [ ] Run target-specific file identity and replacement tests.
- [ ] Run focused file, buffer, editor, auto-save, and PTY save tests.
- [ ] Run `make verify` without updating snapshots.

## Decision Log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-07-22 | Preserve displayed paths separately from visited identities. | Reuse aliases without surprising users by rewriting mode-line and buffer-list paths. |
| 2026-07-22 | Keep final symlinks and hard links distinct. | Final symlinks are view-only; atomic replacement makes hard-link pathnames diverge. |
| 2026-07-22 | Reject conflicts without an implicit force path. | Ordinary repeated save must not turn a warning into silent data loss. |
