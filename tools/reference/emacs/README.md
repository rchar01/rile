<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Emacs Reference Tooling

This directory contains optional tooling for capturing user-visible terminal
GNU Emacs behavior as reference evidence for Rile feature work.

The tooling uses Debian-packaged Emacs and ELPA packages inside a container. It
does not vendor Emacs, Vertico, Marginalia, or Modus source into Rile and must
not be used to copy, translate, or mechanically port their implementation code.

## Profiles

- `core`: runs `emacs -Q -nw`; use this for baseline GNU Emacs behavior.
- `modern`: runs `emacs -Q -nw` with a tiny Debian-packaged profile enabling
  Vertico, Marginalia, and a Modus theme; use this for modern completion UX
  evidence only, not as canonical base Emacs behavior.

## Commands

Build the reference environment wrappers and provenance files:

```sh
tools/reference/emacs/build
```

Capture a scenario:

```sh
tools/reference/emacs/capture m-x-completion-modern
```

Outputs are written under `artifacts/reference/emacs/`, which is ignored by Git.

Initial inspection scenarios:

- `baseline-ui-core`: base `emacs -Q -nw` screen layout.
- `m-x-completion-core`: base Emacs `M-x` completion behavior.
- `m-x-completion-modern`: Vertico/Marginalia `M-x` completion behavior.
- `find-file-completion-modern`: Vertico/Marginalia file completion behavior.
- `incremental-search-wrap-core`: base Emacs incremental search wrapping behavior.
- `insert-file-core`: base Emacs `C-x i` insert-file behavior.
- `buffer-completion-modern`: Vertico/Marginalia buffer completion behavior.
- `prompt-history-modern`: Vertico/Marginalia minibuffer history behavior.
- `help-describe-core`: base Emacs `C-h k` and `C-h f` help behavior.
- `read-only-open-core`: base Emacs `C-x C-r` read-only file open behavior.
- `toggle-read-only-core`: base Emacs `C-x C-q` toggle-read-only behavior.

Scenario files may define `setup_reference_files CAPTURE_DIR CAPTURE_REL` when a
capture needs extra files inside its ignored artifact directory.

See `docs/reference-testing.md` for the workflow and licensing/provenance rules.
