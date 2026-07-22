<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Plan: Bound Syntax Rendering Work

## Goal

Track optional work that would make decorated long-line redraws viewport-aware
after the quadratic span-merge vulnerability has been fixed.

## Current Context

- [x] Replace per-boundary full-span scans with an ordered event sweep.
  Evidence: `0b85bec`; worst-case merging is `O(s log s)` rather than `O(s^2)`.
- [x] Preserve priority, equal-priority input order, invalid-span filtering, and
  adjacent-face coalescing. Evidence: generated differential and focused tie
  tests in `src/render/mod.rs`.
- [x] Exercise 20,000 Rust keyword spans through the default editor path.
  Evidence: `dense_syntax_line_preserves_all_keyword_spans`.
- [x] Add optional open and redraw smoke cases for a 100,000-column Rust line.
  Evidence: `e90d189`; a one-repetition Rile smoke run completed both cases
  without a timeout on 2026-07-22.

## Remaining Risks

- Syntax, active search, and persistent user highlights still inspect complete
  logical lines before terminal viewport projection. The fixed path is no
  longer quadratic, but repeated redraw remains linear or linearithmic in full
  line and span count.
- Projecting visible spans still iterates the complete merged span vector.
- Split windows displaying the same line recompute decorations independently.
- Horizontal projection work grows with the requested starting column because
  display width and lexical state depend on the preceding source.
- Performance smoke tests use timeouts and machine-local timings. They provide
  runtime evidence but are not deterministic correctness gates.

## Optional Follow-Up

### Phase 1: Separate Projection From Decoration

- [ ] Keep existing providers full-line in this phase; project source first and
  clamp their output so only visible spans reach merging and terminal output.
- [ ] Refactor normal-row projection to return the visible source-byte range and
  byte map before requesting decoration spans.
- [ ] Request, clamp, and merge only spans intersecting that visible range.
- [ ] Preserve tabs, control escapes, wide characters, combining characters,
  edge markers, and horizontal scrolling behavior.

Validation gate:

- [ ] Differential projection tests match the current renderer below its source
  budget.
- [ ] Existing PTY scrolling and snapshot tests remain unchanged.

### Phase 2: Add Range-Aware Providers

- [ ] Extend the decoration interface with a requested source range without
  silently changing providers that need line-prefix context.
- [ ] Let syntax scanners process only the prefix needed to establish string or
  comment state and emit spans only for the requested range.
- [ ] Add range-aware search and persistent-highlight matching that preserves
  matches crossing the visible-range boundary.
- [ ] Cover clipping inside strings, escaped strings, comments, Markdown code
  spans, TOML strings, and C preprocessor lines.

Validation gate:

- [ ] Deterministic counters bound emitted spans and inspected source near
  column zero.
- [ ] Range-aware output matches full-line highlighting followed by clipping.

### Phase 3: Evaluate Bounded Caching

- [ ] Measure whether split windows or repeated redraws justify a decoration
  cache after range-aware providers land.
- [ ] If justified, key entries by buffer identity, revision, line, mode,
  enabled providers, and requested range.
- [ ] Set explicit entry and byte limits before retaining spans across frames.

## Non-Goals

- Do not disable syntax highlighting or change its default.
- Do not cap or silently drop valid visible spans.
- Do not slice a visible substring without carrying lexical prefix state.
- Do not add caching before measurements show that range-aware rendering alone
  is insufficient.

## Validation

- [ ] Run focused render, syntax, editor, terminal, and PTY scrolling tests.
- [ ] Run `make perf-smoke PERF_EDITORS=rile PERF_REPETITIONS=1`.
- [ ] Run `make verify` without updating snapshots.

## Decision Log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-07-22 | Keep viewport-aware decoration as optional follow-up work. | The ordered sweep closes the demonstrated quadratic denial of service without changing visible highlighting; range-aware providers are broader and carry lexical-boundary risk. |
