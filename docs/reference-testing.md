<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Reference Testing

Rile includes optional reference-testing tooling for studying user-visible behavior of other terminal editors. Current reference targets are GNU Zile, kg, and GNU Emacs.

Reference testing is not part of Rile's normal quality gate. It is a way to produce behavior evidence before writing original Rile requirements and tests.

## Licensing And Provenance

The reference-testing tooling in this repository is original Rile project material. It does not vendor, incorporate, copy, translate, or mechanically port GNU Zile, kg, Emacs, or Emacs package source code.

The Zile reference tooling may download a pinned upstream GNU Zile release into ignored local artifacts. Zile is GPL-3.0-or-later software from the GNU Project. Any local downloaded source tree keeps its upstream license files and notices in `artifacts/reference/zile/`, which is ignored by Git.

The kg reference tooling may clone a pinned upstream kg commit into ignored local artifacts. kg is BSD-2-Clause software by its upstream authors. Any local cloned source tree keeps its upstream license files and notices in `artifacts/reference/kg/`, which is ignored by Git.

The Emacs reference tooling uses Debian-packaged GNU Emacs and ELPA packages in a container. It writes local wrapper scripts, profile copies, Debian package copyright files, and provenance under `artifacts/reference/emacs/`, which is ignored by Git. Both `core` and `modern` use the shared reference early init for minimal terminal startup. The `core` profile otherwise stays empty, while the `modern` profile enables Vertico and Marginalia for modern completion UX evidence. Treat the modern profile as a curated reference profile, not canonical base Emacs behavior.

Generated screenshots, GIFs, temporary files, downloaded tarballs, extracted sources, and installed reference binaries are review evidence only. They are ignored local artifacts unless explicitly distributed with their required upstream notices.

Use reference-editor behavior as evidence for original Rile requirements. Do not copy reference implementation code into Rile.

## Directory Layout

Committed tooling lives under:

```text
tools/reference/zile/
tools/reference/kg/
tools/reference/emacs/
```

Ignored generated outputs live under:

```text
artifacts/reference/zile/
artifacts/reference/kg/
artifacts/reference/emacs/
```

Scenario files under `tools/reference/<editor>/scenarios/` are original Rile project scenario definitions. They describe fixtures, terminal sizes, keystrokes, and frame names for visual behavior capture. The capture scripts apply each scenario's `WIDTH` and `HEIGHT` with `stty cols` and `stty rows` before launching the reference editor.

## Build The Zile Reference

Build the reference container and download/build the pinned Zile release:

```sh
tools/reference/zile/build
```

The script verifies the pinned release tarball checksum before extracting and building it. The installed reference binary is written under:

```text
artifacts/reference/zile/install/bin/zile
```

## Build The kg Reference

Build the reference container and clone/build the pinned kg commit:

```sh
tools/reference/kg/build
```

The installed reference binary is written under:

```text
artifacts/reference/kg/install/bin/kg
```

## Build The Emacs Reference

Build the reference container and install local profile wrappers:

```sh
tools/reference/emacs/build
```

The installed reference wrappers are written under:

```text
artifacts/reference/emacs/install/bin/emacs-core
artifacts/reference/emacs/install/bin/emacs-modern
```

Both Emacs wrappers stage `early-init.el` and `init.el` in the scenario home,
then run `emacs -nw` with site files disabled. The staged early init suppresses
startup UI, dialogs, GUI bars, and cursor blinking before the first frame. The
`emacs-core` init profile is empty, while the `emacs-modern` init profile
enables Vertico and Marginalia from Debian ELPA packages for visual completion
comparison.

## Capture A Scenario

Capture the smoke scenario:

```sh
tools/reference/zile/capture smoke-open
```

Capture another scenario:

```sh
tools/reference/zile/capture open-line
```

Capture a kg scenario:

```sh
tools/reference/kg/capture baseline-ui
```

Capture an Emacs scenario:

```sh
tools/reference/emacs/capture m-x-completion-modern
```

Capture outputs are written under:

```text
artifacts/reference/zile/captures/<scenario>/
artifacts/reference/kg/captures/<scenario>/
artifacts/reference/emacs/captures/<scenario>/
```

Each capture directory may include:

- generated fixture files;
- generated VHS tape files;
- named PNG frames;
- optional GIF output;
- a temporary `HOME` used by the reference editor run.

## Scenario Format

Scenarios are Bash fragments sourced by the capture script. Keep them simple and deterministic:

```sh
SCENARIO_NAME=smoke-open
SCENARIO_DESCRIPTION='Open a fixture and quit.'
WIDTH=100
HEIGHT=24
FIXTURE_NAME=smoke-open.txt
FIXTURE_CONTENT=$'alpha\nbeta\n'

vhs_steps() {
  cat <<'EOF'
Wait+Screen /alpha/
Screenshot {{FRAME_DIR}}/00-open.png
Ctrl+X
Ctrl+C
EOF
}
```

Supported placeholders in `vhs_steps` output:

- `{{FRAME_DIR}}`: capture frame directory.
- `{{FIXTURE}}`: generated fixture path.
- `{{ZILE}}`: installed reference Zile binary.
- `{{KG}}`: installed reference kg binary.
- `{{EMACS}}`: installed reference Emacs profile wrapper.
- `{{HOME}}`: temporary home directory for the scenario.

Emacs scenarios must also set `EMACS_PROFILE` to `core` or `modern`. Use
`core` for base Emacs behavior with the shared reference early init and
`modern` for the curated Vertico/Marginalia profile.

## How To Use Evidence

For each feature scenario:

- Capture reference-editor frames with fixed fixture text and terminal size.
- Inspect the screenshots and write a short behavior summary.
- Turn that summary into original Rile requirements and acceptable differences.
- Add Rile unit tests, PTY tests, parsed-screen snapshots, or optional Rile VHS demos as appropriate.
- Verify Rile with `make verify` before committing feature work.

Screenshots are evidence, not the pass/fail oracle. Rile's automated PTY tests and parsed-screen snapshots remain the correctness gates.
