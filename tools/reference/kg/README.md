<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# kg Reference Tooling

This directory contains optional tooling for capturing user-visible kg behavior
as reference evidence for Rile feature work.

The tooling clones and builds a pinned upstream kg commit into ignored local
artifacts. It does not vendor kg source into Rile and must not be used to copy,
translate, or mechanically port kg implementation code.

## Commands

Build the reference environment and kg binary:

```sh
tools/reference/kg/build
```

Capture a scenario:

```sh
tools/reference/kg/capture baseline-ui
```

Outputs are written under `artifacts/reference/kg/`, which is ignored by Git.

Initial inspection scenarios:

- `baseline-ui`: basic screen layout and mode line.
- `back-to-indentation`: `M-m` back-to-indentation behavior.
- `consecutive-kills`: consecutive kill and yank behavior.
- `join-line`: `M-^` join-line behavior.
- `list-buffers`: `C-x C-b` list-buffers behavior.
- `mark-whole-buffer`: `C-x h` mark-whole-buffer behavior.
- `open-line`: `C-o` behavior with point in the first line.
- `goto-line`: `M-g` prompt and `line:col` navigation behavior.
- `m-x-completion`: command prompt completion after Tab.
- `find-file-completion`: file prompt completion after Tab.
- `insert-file`: `C-x i` insert-file behavior.
- `quoted-insert`: `C-q` quoted-insert behavior.
- `buffer-completion`: switch-buffer prompt completion after Tab.
- `prompt-history`: minibuffer history with `M-p` and `M-n`.
- `incremental-search`: `C-s` search prompt, repeat, and accept flow.
- `query-replace`: `M-%` prompts and choice-key workflow.
- `split-windows`: split layout and window switching behavior.
- `help-general`: general `C-h` help screen behavior.
- `read-only-open`: `C-x C-r` read-only file open behavior.
- `toggle-read-only`: `C-x C-q` toggle-read-only behavior.

Scenario files may define `setup_reference_files CAPTURE_DIR CAPTURE_REL` when a
capture needs extra files inside its ignored artifact directory.

See `docs/reference-testing.md` for the workflow and licensing/provenance rules.
