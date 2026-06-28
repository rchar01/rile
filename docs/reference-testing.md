<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Reference Testing

Rile includes optional reference-testing tooling for studying user-visible behavior of other terminal editors and comparing it with first-party Rile captures. Current reference targets are GNU Zile, kg, GNU Emacs, and Rile itself.

Reference testing is not part of Rile's normal quality gate. It is a way to produce behavior evidence before writing original Rile requirements and tests.

## Licensing And Provenance

The reference-testing tooling in this repository is original Rile project material. It does not vendor, incorporate, copy, translate, or mechanically port GNU Zile, kg, Emacs, or Emacs package source code.

The Rile reference target is first-party behavior capture of the current Rile binary. It is comparison evidence for hardening existing behavior, not an external behavior source and not a correctness oracle.

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
tools/reference/rile/
```

Ignored generated outputs live under:

```text
artifacts/reference/zile/
artifacts/reference/kg/
artifacts/reference/emacs/
artifacts/reference/rile/
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

## Build The Rile Reference

The Rile reference target builds the current repository binary during capture
with `cargo build --locked` inside the visual tooling container. The build uses
an ignored target directory under `artifacts/reference/rile/build/target/` so
reference captures do not depend on or modify the normal `target/` directory. It
uses `Containerfile.visual`, not a separate external-editor container, so VHS,
ttyd, Chromium, and Rust are available together.

Rile scenarios launch the local binary with deterministic visual flags:

```sh
artifacts/reference/rile/build/target/debug/rile --visual-test --test-size WIDTHxHEIGHT <fixture>
```

## Capture A Scenario

Capture the smoke scenario:

```sh
tools/reference/zile/capture smoke-open
```

The Makefile also provides a repo-level wrapper for targeted captures:

```sh
make reference-capture REF_EDITOR=zile REF_SCENARIO=smoke-open
```

Capture all scenarios for all reference editors:

```sh
make reference-capture-all
```

Capture all scenarios for one reference editor:

```sh
make reference-capture-all REF_EDITOR=emacs
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

Capture a Rile scenario:

```sh
make reference-capture REF_EDITOR=rile REF_SCENARIO=open-line
```

The paired Emacs/Rile `recenter-core` and `recenter` captures compare repeated
`C-l` behavior in short and long buffers. They record the stable state before
recenter, after the first center placement, after the second top placement, and
after the third bottom placement, including an end-of-buffer case where centering
can leave blank rows below point.

The Emacs, Zile, and Rile `long-line-core`/`long-line` captures compare long
logical-line display at a narrow terminal width. They record file-buffer edge
markers, forward and backward point movement across horizontal display
thresholds, region faces on clipped long lines, region display through a short
line break, and help-buffer continuation or clipping markers.

The paired Emacs/Rile `long-line-hscroll-core` and `long-line-hscroll` captures
focus on truncated long-line horizontal auto-scrolling. They record single-step
`C-f`/`C-b` and `M-f`/`M-b` transitions around Emacs' default horizontal scroll
margin and recenter behavior.

Capture outputs are written under:

```text
artifacts/reference/zile/captures/<scenario>/
artifacts/reference/kg/captures/<scenario>/
artifacts/reference/emacs/captures/<scenario>/
artifacts/reference/rile/captures/<scenario>/
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
- `{{RILE}}`: current repository Rile binary.
- `{{HOME}}`: temporary home directory for the scenario.

Emacs scenarios must also set `EMACS_PROFILE` to `core` or `modern`. Use
`core` for base Emacs behavior with the shared reference early init and
`modern` for the curated Vertico/Marginalia profile.

## Scenario Timing

Capture scripts expand scenario steps before running VHS. By default they add
`Sleep 500ms` after input commands such as `Ctrl+X`, `Type "r"`, `Enter`,
and `Tab`, and around each `Screenshot` when the neighboring meaningful step
was not already a sleep. This keeps prompt transitions, prefix-key states,
captured frames, and command results visible without requiring every scenario to
hand-code waits after each key.

The expander does not add a pause after `Escape` because existing scenarios use
`Escape` followed by another key to express Meta-key input.

Scenarios may still include explicit `Sleep` lines for transitions that need a
longer or feature-specific delay. The expander does not add another automatic
pause next to an existing sleep, including before or after screenshots.

Screenshots should record semantic evidence, not every keystroke. For each
feature command, capture the stable state before the command, prompt or
key-reading states that affect behavior, typed minibuffer text before `Enter`,
state immediately after an executing `Enter`, and the final visible result.
Avoid adjacent before/after screenshots when no command or prompt transition
happens between them.

Prefer frame names that describe the observable state:

- `before-...`: state before starting a feature command.
- `prompt-...`: minibuffer prompt or command waiting for input.
- `typed-...`: typed query, argument, or minibuffer content before execution.
- `after-enter-...`: state immediately after `Enter` confirms or executes.
- `after-...`: final command result.

When a reference editor does not support a command, name the frames as probes
instead of prompts, such as `before-...-probe`, `after-...-command-key`, and
`after-...-probe`. This keeps fall-through text insertion, unknown-command
messages, and other differences visible without implying the editor entered the
same prompt state as Emacs.

Timing controls are optional:

- `INPUT_PAUSE`: per-scenario post-input pause, default `500ms`.
- `SCREENSHOT_PAUSE`: per-scenario pause around screenshots, default `INPUT_PAUSE`.
- `AUTO_INPUT_PAUSES=0`: disables automatic pause expansion for a scenario.
- `TYPING_SPEED`: optional VHS typing speed for typed text animation.

The same controls can be overridden from the environment with
`REFERENCE_INPUT_PAUSE`, `REFERENCE_SCREENSHOT_PAUSE`, and
`REFERENCE_AUTO_PAUSES`.

## Modern Completion Review Notes

The modern Emacs reference profile enables Vertico and Marginalia. Use its
captures as UX evidence for minibuffer completion prompts where Rile exposes a
selected candidate list.

Current Rile alignment shares selected-candidate mechanics across completion
prompts:

- `M-x`, `C-h f`, `C-h v`, `C-x C-f`, `C-x C-r`, `C-x i`, `C-x b`, and
  `C-x k` move the selected candidate with Down/Up or `C-n`/`C-p`, and page the
  selected candidate with PageDown/PageUp or `C-v`/`M-v`;
- Tab inserts the selected candidate into the minibuffer;
- Enter accepts the selected candidate except where exact typed text is
  deliberately preserved, `M-RET` submits raw minibuffer input, and an explicitly
  moved selection wins over exact minibuffer text;
- minibuffer prompt text uses an input cursor with `C-f`/Right, `C-b`/Left,
  `M-f`, `M-b`, `C-a`/Home, and `C-e`/End movement, point-aware insertion,
  Backspace and `C-d`/Delete, and kill-ring deletion with `C-k`, `M-d`,
  `M-Backspace`, and terminal-encoded `C-Backspace`;
- command, option, and buffer prompts use orderless component matching by
  default, including smart case, simple literal anchors, `!foo` negation, and
  `=foo` literal components;
- file prompts use Emacs file-category matching by default, including prefix,
  word-component partial completion, substring matching, smart case, raw `M-RET`
  input, and directory descent after Tab insertion, exact input, explicit
  selection, empty input, or a typed prefix of the selected directory;
- empty input remains prompt-specific: `C-x b` switches to the default previous
  buffer, while `C-x k` kills the default current buffer.

The Emacs and Rile `m-x-completion-modern` captures cover command completion
review with selected-candidate Tab insertion and selected Enter over exact
input. Rile's matching capture also records Orderless component and simple-anchor
frames for visual inspection, but Orderless component correctness is covered by
Rile's PTY tests because interactive VHS space input can be misleading for
`M-x`.

The Emacs and Rile `find-file-completion-modern` captures cover file prompt
prefix filtering, file-category word-component partial-completion matching,
substring matching, selected-candidate Tab insertion, selected Enter over exact
input, directory descent, exact files, and raw `M-RET` missing-file input.

The Emacs and Rile `minibuffer-navigation-modern` captures focus on
minibuffer movement: history with `M-p`/`M-n`, line candidate movement with
`C-n`/`C-p`, and visible-page candidate movement with `C-v`/`M-v` across
command, help, variable, and file prompts.

Scenario conclusion: Rile now aligns with the modern Emacs completion model for
the user-visible mechanics it intentionally implements: selected-candidate
movement, Tab insertion, selected Enter behavior, Orderless-style command
matching, and file-category partial completion instead of global Orderless file
matching. The captures also show intentional display differences. Emacs modern
has a much larger command universe, richer Marginalia file metadata, and longer
absolute file prompts; Rile shows its smaller command registry, simpler
annotations, and compact file prompts. Treat those as acceptable product-scope
differences unless a future feature explicitly targets richer annotations or a
larger command surface.

## Kill Buffer Completion Review Notes

The C-x k comparison is scoped to editors that prompt for a named buffer:
Emacs-modern, Zile, and Rile. kg is not included because its C-x k kills the
current buffer directly rather than entering named-buffer completion.

Current Rile alignment work shares buffer-prompt completion mechanics between
C-x b and C-x k:

- C-x k keeps the current/default buffer first in kill-buffer completion
  candidates;
- Tab inserts the selected candidate;
- Enter accepts the selected candidate, and an explicitly moved selection wins
  over exact minibuffer text;
- empty input remains prompt-specific: C-x b switches to the default previous
  buffer, while C-x k kills the default current buffer;
- dirty buffers ask for Emacs-style y-or-n-p confirmation before they are killed.

The comparison captures record the visible Tab-selected state and the resulting
switch or kill. Direct Enter on ambiguous input has the same selected-candidate
rule, but is covered by automated unit and PTY tests because there is no
intermediate visual completion state before Enter executes.

Review next whether switch-buffer candidates should prioritize the current,
previous, or most recently used buffer like a fuller Emacs setup, and whether
ido and *Completions* styles need style-specific documentation beyond the shared
prompt semantics.

## How To Use Evidence

For each feature scenario:

- Capture reference-editor frames with fixed fixture text and terminal size.
- Capture matching first-party Rile frames when comparison evidence is useful.
- Inspect the screenshots and write a short behavior summary.
- Turn that summary into original Rile requirements and acceptable differences.
- Add Rile unit tests, PTY tests, parsed-screen snapshots, or optional Rile VHS demos as appropriate.
- Verify Rile with `make verify` before committing feature work.

Screenshots are evidence, not the pass/fail oracle. Rile's automated PTY tests and parsed-screen snapshots remain the correctness gates.
