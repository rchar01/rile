<!--
SPDX-FileCopyrightText: 2026 Robert Charusta <rch-public@posteo.net>
SPDX-License-Identifier: GPL-3.0-or-later
-->

# Plan: Self-Documentation Architecture

## Goal

Make Rile a self-documented editor whose user-visible commands, keys,
options, modes, buffers, and editor diagnostics can be inspected from inside
the editor.

The target is an Emacs-style discovery experience implemented with Rust-native
typed registries, explicit metadata, layered keymaps, schema-driven options,
normal help buffers, and tests that enforce documentation coverage.

## Change Description

Rile should move toward a self-describing editor kernel. User-visible behavior
should be represented as typed data with executable handlers rather than as
documentation copied separately from implementation.

The final architecture should provide one source of truth for:

- `M-x` command execution and completion.
- `C-h f` / `describe-command` command help.
- `C-h k` / `describe-key` key lookup.
- `C-h c` / `describe-key-briefly` short key lookup.
- `C-h b` / `describe-bindings` active binding tables.
- `C-h v` / `describe-variable` config option help.
- `C-h m` / `describe-mode` active mode help.
- `describe-buffer` buffer state inspection.
- `about-rile` build, terminal, feature, and config-path inspection.

Help commands should render live registries and editor state into read-only
help buffers. They should not duplicate prose in separate hand-written tables.

## Target Architecture

### Module Layout

```text
src/
  command/
    mod.rs          CommandId, CommandSpec, registry, dispatch types
    movement.rs     Movement command specs and handlers
    editing.rs      Editing command specs and handlers
    files.rs        File command specs and handlers
    buffers.rs      Buffer command specs and handlers
    windows.rs      Window command specs and handlers
    help.rs         Help command specs and handlers

  keymap/
    mod.rs          KeySeq, KeyMap, KeyBinding, layered resolution
    default.rs      Global/default key bindings

  option/
    mod.rs          OptionId, OptionSpec, typed values, validation

  mode/
    mod.rs          ModeId, ModeSpec, active mode stack, local keymaps

  help/
    mod.rs          describe-* renderers
    format.rs       Help tables and text formatting

  buffer/
  editor.rs
  terminal/
```

### Commands

Commands should have stable typed IDs for Rust and stable names for users,
configuration, help, and completion.

```rust
pub enum CommandId {
    ForwardChar,
    SaveBuffer,
    SwitchToBuffer,
    DescribeCommand,
}

pub struct CommandSpec {
    pub id: CommandId,
    pub name: &'static str,
    pub summary: &'static str,
    pub doc: &'static str,
    pub category: CommandCategory,
    pub interactive: bool,
    pub handler: CommandHandler,
}
```

Handlers should receive enough context for real editor commands instead of
being restricted to a trivial function shape.

```rust
pub type CommandHandler =
    fn(&mut Editor, CommandContext) -> Result<CommandOutcome>;

pub struct CommandContext {
    pub argument: Option<i32>,
    pub invoked_by: Invocation,
}

pub enum Invocation {
    Key(KeySeq),
    ExtendedCommand,
    Test,
}

pub enum CommandOutcome {
    Continue,
    Exit,
    StartedPrompt,
}
```

The command registry should support lookup by ID and by name, expose sorted
interactive command lists for completion, and provide category filtering for
help output.

### Keymaps

Keymaps should be named, typed, layered, and inspectable.

```rust
pub struct KeyMap {
    pub id: KeyMapId,
    pub name: &'static str,
    pub bindings: Vec<KeyBinding>,
}

pub struct KeyBinding {
    pub sequence: KeySeq,
    pub target: BindingTarget,
}

pub enum BindingTarget {
    Command(CommandId),
}
```

Active key resolution should walk a keymap stack in priority order:

```text
minibuffer keymap
transient keymap
special buffer keymap
minor mode keymaps
major mode keymap
global keymap
```

`describe-key` should report the resolved command, the keymap that supplied the
binding, and any shadowed lower-priority bindings when useful.

`describe-bindings` should show the active keymap stack for the current buffer
and mark shadowed bindings rather than only listing global keys.

### Options

Configuration should be schema-driven. Config parsing, validation, defaults,
completion, and `describe-variable` should all use the same option registry.

```rust
pub enum OptionId {
    TabWidth,
    LineNumbers,
    SyntaxHighlighting,
    CompletionStyle,
}

pub struct OptionSpec {
    pub id: OptionId,
    pub name: &'static str,
    pub config_key: &'static str,
    pub summary: &'static str,
    pub doc: &'static str,
    pub value_type: OptionType,
    pub default: OptionValue,
    pub validator: fn(&OptionValue) -> Result<()>,
}
```

`describe-variable` should show the option name, config key, current value,
default value, type, valid values or range, and documentation.

### Modes

Modes should be first-class metadata objects without copying Emacs Lisp's full
dynamic object model.

```rust
pub struct ModeSpec {
    pub id: ModeId,
    pub name: &'static str,
    pub summary: &'static str,
    pub doc: &'static str,
    pub kind: ModeKind,
    pub keymap: Option<KeyMapId>,
}

pub enum ModeKind {
    Major,
    Minor,
    SpecialBuffer,
}
```

Buffers should expose enough typed state for `describe-mode` and
`describe-buffer`:

```rust
pub struct BufferState {
    pub name: String,
    pub path: Option<PathBuf>,
    pub kind: BufferKind,
    pub major_mode: ModeId,
    pub minor_modes: Vec<ModeId>,
    pub modified: bool,
    pub read_only: bool,
    pub point: Position,
    pub encoding: Encoding,
    pub line_ending: LineEnding,
}
```

Special buffers such as `*Help*`, `*Messages*`, `*Completions*`, and
`*Buffer List*` should use special-buffer modes and local keymaps. Their keys
should be inspectable through the same keymap stack as normal editing keys.

### Help Buffers

Help output should be generated by renderers over registries and live editor
state. Help buffers should be ordinary read-only buffers with `help-mode` and a
local `help-mode-map`.

The help renderer should provide reusable formatting for command pages,
binding tables, option pages, mode pages, buffer state summaries, and about
pages.

Help prose should eventually be formatted for terminal reading rather than
emitted as unbounded paragraphs. Emacs's default `fill-column` is 70, so Rile
should use an explicit help fill width near 70 columns for generated prose
unless a specific help view needs a table or preformatted block. Separately,
help buffers should render long logical lines with visible terminal
continuation when the current window is narrower than the formatted text: show
`\` at the right edge and continue the same logical line on the next screen row.
This keeps help usable in both standard 80-column terminals and narrower split
windows. The continuation behavior can be implemented before prose filling;
the explicit fill-width formatter remains part of the broader help-rendering
architecture.

## Non-Goals

- Do not implement an Emacs Lisp runtime.
- Do not implement Lisp symbols, advice, hooks, property lists, or dynamic
  variable rebinding as part of this milestone.
- Do not make Markdown docs the source of truth for command, option, or keymap
  metadata.
- Do not make macros the core architecture. Macros may reduce boilerplate after
  the registry shapes are stable.
- Do not require plugin or extension APIs before the core self-documentation
  model is working.

## Assumptions

- Breaking internal changes are allowed before public release.
- User-visible command names should remain stable once public releases begin.
- The architecture should favor explicit Rust types and testable registries over
  runtime dynamism.
- Help output should be useful in a terminal and should avoid depending on rich
  UI widgets.

## Open Questions

- [ ] Should command names follow Emacs names exactly where behavior matches,
      or should Rile prefer clearer names when behavior intentionally differs?
- [ ] Should option names use Emacs-style hyphenated names, TOML-style
      snake_case keys, or support both through aliases?
- [ ] Should special-buffer modes be visible in the mode line from the first
      mode implementation, or only through `describe-mode`?
- [ ] Should command categories be a fixed enum or a lightweight static string
      taxonomy?

## Implementation Plan

### Phase 1: Registry Foundations

Goal: Define the typed metadata model before moving behavior.

- [x] Introduce `CommandId`, `CommandSpec`, `CommandCategory`,
      `CommandContext`, `Invocation`, `CommandOutcome`, and `CommandHandler`.
- [x] Build a command registry API with lookup by ID, lookup by name,
      interactive command iteration, and category filtering.
- [x] Add tests that reject duplicate command IDs and duplicate command names.
- [x] Add tests requiring every interactive command to have a non-empty summary
      and full doc string.

Validation gate:

- [x] Command registry unit tests pass.
- [x] Existing command completion and key dispatch behavior is preserved.

### Phase 2: Command Dispatch Migration

Goal: Move command execution to registered handlers while preserving command
semantics.

- [x] Convert movement commands to registered handlers.
- [x] Convert editing-category commands, including normal kill/yank commands,
      to registered handlers.
- [x] Convert window commands to registered handlers.
- [x] Convert buffer commands to registered handlers.
- [x] Convert file, search, query-replace, shell, register, rectangle, macro,
      and help commands to registered handlers.
- [x] Route key execution and `M-x` execution through the same command dispatch
      path.
- [x] Preserve universal argument, keyboard macro recording, kill-command
      coalescing, yank-pop state, minibuffer prompt startup, and exit outcomes.

Validation gate:

- [x] Focused command execution tests pass.
- [x] `M-x` command completion and execution tests pass.
- [x] PTY smoke tests for common keybindings pass.

### Phase 3: Layered Keymaps

Goal: Make all active bindings inspectable through named keymaps.

- [x] Introduce `KeyMapId`, named `KeyMap`, `KeySeq`, and `BindingTarget`.
- [x] Replace string command targets with `CommandId` targets.
- [x] Implement active keymap stack resolution for the current global layer.
- [x] Move special-buffer keys such as help `q`, messages `q`, buffer-list
      `RET`, and buffer-list `q` into local keymaps.
- [x] Add shadowing-aware binding lookup for help output.

Validation gate:

- [x] Tests prove every keybinding targets an existing command.
- [x] Tests prove active keymap precedence behavior.
- [x] Tests prove shadowing behavior.
- [x] Prefix help and existing keybindings keep their visible behavior after
      local keymaps are added.

### Phase 4: Help Command Expansion

Goal: Make help output render command and keymap registries.

- [ ] Rename or alias command help to `describe-command` if that is the chosen
      user-facing name.
- [ ] Expand `describe-command` to show command name, category, summary, full
      docs, interactivity, and bound keys.
- [ ] Expand `describe-key` to show key sequence, resolved command, source
      keymap, summary, full docs, and shadowed bindings when applicable.
- [ ] Add `describe-key-briefly` for echo-area command-name lookup.
- [ ] Add `describe-bindings` for the current active keymap stack.
- [ ] Add reusable help formatting for headings, key tables, command tables,
      and wrapped prose.
- [ ] Format generated help prose to an explicit readable fill width near 70
      columns while preserving tables and preformatted blocks.
- [x] Render help buffers with visible `\` continuation rows when a logical line
      is wider than the current window.

Validation gate:

- [ ] Unit tests cover help rendering for command, key, brief-key, and bindings
      output.
- [ ] Unit tests cover help prose filling and narrow-window continuation rows.
- [ ] PTY tests cover opening and leaving help buffers.

### Phase 5: Option Registry And `describe-variable`

Goal: Make config options typed, validated, documented, and inspectable.

- [ ] Introduce `OptionId`, `OptionSpec`, `OptionType`, and `OptionValue`.
- [ ] Move config defaults and validation into option specs.
- [ ] Route config parsing through the option registry.
- [ ] Add command-name-style completion for `describe-variable`.
- [ ] Render option name, config key, current value, default value, type,
      valid values or range, and docs.

Validation gate:

- [ ] Config parser tests pass through the option registry.
- [ ] Tests require every option to have summary, docs, type, default, and
      validation.
- [ ] `describe-variable` unit and PTY tests pass.

### Phase 6: Modes And Buffer Inspection

Goal: Make active modes and buffer state inspectable.

- [ ] Introduce `ModeId`, `ModeSpec`, `ModeKind`, and a mode registry.
- [ ] Represent normal editing modes, syntax modes, and special-buffer modes as
      mode specs.
- [ ] Attach major mode, minor modes, and special-buffer mode data to buffers.
- [ ] Add local keymaps to relevant modes.
- [ ] Implement `describe-mode` from the active mode stack.
- [ ] Implement `describe-buffer` from typed buffer state.

Validation gate:

- [ ] Tests require every mode to have summary and docs.
- [ ] `describe-mode` shows active major, minor, and special-buffer modes.
- [ ] `describe-buffer` shows name, path, kind, modified state, read-only state,
      point, encoding, line ending, and active modes.

### Phase 7: About And Diagnostics

Goal: Provide editor-level introspection without overbuilding diagnostics.

- [ ] Add `about-rile` with version, build profile, enabled features, terminal
      backend, config path, and runtime-relevant paths.
- [ ] Add structured diagnostic metadata only for diagnostics that users can act
      on from inside the editor.
- [ ] Render diagnostics through normal help or messages buffers.

Validation gate:

- [ ] `about-rile` output is deterministic enough for tests where possible.
- [ ] Runtime-specific fields are tested with stable predicates rather than
      brittle full snapshots.

### Phase 8: Documentation And Release Notes

Goal: Keep public docs concise while the editor becomes the detailed source of
truth.

- [ ] Update `README.md` to describe self-documentation commands and reduce any
      duplicated command tables that help can now generate.
- [ ] Update `docs/development.md` with the registry, keymap, option, mode, and
      help architecture.
- [ ] Update `docs/testing.md` with metadata coverage tests and PTY help tests.
- [ ] Update `NEWS` for user-visible help and introspection changes.
- [ ] Update `ChangeLog` for file-level implementation history.

Validation gate:

- [ ] Documentation matches implemented command names and keybindings.
- [ ] No user-visible self-documentation behavior exists only in prose.

## Testing Strategy

- [ ] Unit-test registry invariants: no duplicate IDs, no duplicate names, no
      undocumented interactive commands, no undocumented options, no
      undocumented modes.
- [ ] Unit-test keymap resolution, active stack priority, prefix handling, and
      shadowed binding reporting.
- [ ] Unit-test help rendering with stable text fixtures for commands, keys,
      options, modes, and buffers.
- [ ] PTY-test representative help commands through the real terminal UI.
- [ ] Keep `make verify` as the full quality gate before merging each phase.

## Risks

- Command dispatch migration can subtly break command bookkeeping such as
  universal arguments, keyboard macro recording, kill coalescing, and yank-pop.
- Layered keymaps can change precedence if special-buffer, mode, and global
  bindings are not ordered carefully.
- Option registry migration can drift from existing config file behavior unless
  parser tests cover all supported keys and validation errors.
- Help output can become noisy if full docs are too long for terminal buffers.
- Metadata can become stale unless coverage tests are strict and new commands
  cannot be added without docs.

## Acceptance Criteria

- `M-x`, key dispatch, and help commands all use the same command registry.
- All active keybindings are visible through `describe-key` and
  `describe-bindings`.
- All documented options are backed by option specs used by config parsing.
- Active modes and buffer state are inspectable through help commands.
- Help buffers are normal read-only buffers with their own mode and keymap.
- Tests fail when an interactive command, option, mode, or keybinding lacks
  required metadata.
- `make verify` passes after each merged phase.

## Progress Log

| Date | Update | Evidence |
| --- | --- | --- |
| 2026-06-22 | Plan created. | User requested a written future architecture and implementation plan. |
| 2026-06-23 | Help buffers render narrow-window continuation rows. | `make verify` passed before committing the terminal help-wrap implementation. |
| 2026-06-23 | Phase 1 command registry foundations implemented. | Added typed command IDs, categories, dispatch context types, by-ID/category lookup, metadata validation tests, and a default-keybinding registry coverage test. |
| 2026-06-23 | Phase 2 movement command dispatch migration started. | Registered handlers for movement commands and routed handler-backed commands through shared command bookkeeping with legacy fallback for unmigrated commands. |
| 2026-06-23 | Phase 2 editing-category dispatch migration continued. | Registered handlers for editing-category commands, including normal kill/yank commands, and added registry coverage requiring those commands to use handlers. |
| 2026-06-23 | Phase 2 window command dispatch migration continued. | Registered handlers for window commands and added registry coverage requiring those commands to use handlers. |
| 2026-06-23 | Phase 2 buffer command dispatch migration continued. | Registered handlers for buffer commands and added registry coverage requiring those commands to use handlers. |
| 2026-06-23 | Phase 2 command dispatch migration completed. | Registered handlers for every default command, removed the legacy dispatch fallback, added registry coverage requiring every command to use a handler, and verified with focused dispatch tests plus `make verify`. |
| 2026-06-23 | Phase 3 layered keymap migration started. | Introduced typed keymap IDs, key sequences, binding targets, and command-ID-backed global key bindings; focused `keymap`, `completion`, `describe_key`, and key-dispatch tests passed before `make verify`. |
| 2026-06-23 | Phase 3 active keymap stack plumbing added. | Added `KeyMapStack` resolution with keymap source metadata and tests covering global fallback plus higher-priority map precedence, including prefix shadowing. |
| 2026-06-23 | Phase 3 special-buffer local keymaps added. | Moved help/messages/shell-output `q` plus buffer-list `q` and `RET` into named local keymaps backed by registered commands, and made help binding lookup use the active stack. |

## Decision Log

| Date | Decision | Reason |
| --- | --- | --- |
| 2026-06-22 | Treat breaking internal changes as allowed before public release. | User explicitly stated the software is not public yet and can be rewritten. |
| 2026-06-22 | Target a Rust-native self-describing editor kernel instead of Emacs Lisp introspection. | Keeps the user experience Emacs-like while preserving typed, explicit Rust architecture. |
