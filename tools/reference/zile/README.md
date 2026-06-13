<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Zile Reference Tooling

This directory contains optional tooling for capturing user-visible GNU Zile behavior as reference evidence for Rile feature work.

The tooling downloads and builds a pinned upstream Zile release into ignored local artifacts. It does not vendor Zile source into Rile and must not be used to copy, translate, or mechanically port Zile implementation code.

## Commands

Build the reference environment and Zile binary:

```sh
tools/reference/zile/build
```

Capture a scenario:

```sh
tools/reference/zile/capture smoke-open
```

Outputs are written under `artifacts/reference/zile/`, which is ignored by Git.

See `docs/reference-testing.md` for the workflow and licensing/provenance rules.
