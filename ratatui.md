# Ratatui UI Redesign

## Overview

This document is the authoritative plan for migrating `g`'s UI layer from
`colored` + `indicatif` + `dialoguer` to a unified `ratatui`-based system.

**Goals:**

- **Consistency** — every piece of output, from a single status line to a
  full-screen interactive form, comes from one place with one style vocabulary.
- **Theming** — a `Theme`/`Palette` struct controls every color and modifier;
  swapping themes later requires changing one value.
- **Readability** — terminal-first design: no box borders, generous spacing,
  slash-style section dividers, clear information hierarchy.
- **No raw `println!`** — all output routes through ratatui's rendering
  pipeline or crossterm helpers that respect the active theme.

---

## Three Rendering Modes

```
Mode 1 — Styled print  (single lines: info, success, warning, error, key-value)
         ratatui Style + Span + Line → crossterm ANSI flush to stdout

Mode 2 — Inline widget  (tables, trees, section headers, progress)
         ratatui Terminal with Viewport::Inline(n) / insert_before
         Uses ratatui Table + ratatui-cheese Fieldset / Tree / List

Mode 3 — Full-screen TUI  (interactive: commit builder, file picker, selectors)
         ratatui::init() → event loop → ratatui::restore()
         Uses ratatui-cheese Help widget for keybinding hints
```

All three modes share the same **Theme** and are accessible through the same
`ui::*` public API. Command call sites do not change.

---

## Module Structure

```
src/ui/
  mod.rs          ← public facade (all existing ui:: call sites unchanged)
  theme.rs        ← Theme, Palette, Icons structs + global accessor
  print.rs        ← Mode 1 helpers: print_info, print_success, print_key_value…
  widgets.rs      ← Mode 2 helpers: render_table, render_tree, print_fieldset…
  interactive.rs  ← Mode 3 helpers: select, multi_select, input, confirm, forms
  render.rs       ← shared: terminal init/restore, background spinner thread
```

---

## Theme System

```rust
// src/ui/theme.rs

/// Semantic color palette — the single source of truth for all colors.
pub struct Palette {
    pub primary:     Color,  // cyan        — info, spinner, active branch
    pub success:     Color,  // green       — added lines, ✓ icon, current branch
    pub warning:     Color,  // yellow      — warnings, commit hashes
    pub danger:      Color,  // red         — errors, deleted lines, remote refs
    pub muted:       Color,  // bright_black — dates, dim text, graph lines
    pub text:        Color,  // white       — body text
    pub accent:      Color,  // magenta     — refactor type, special refs
    pub divider:     Color,  // bright_black — slash fill, rule lines

    // Conventional commit prefix colors
    pub cc_feat:     Color,  // → success (green)
    pub cc_fix:      Color,  // → danger  (red)
    pub cc_docs:     Color,  // → blue
    pub cc_refactor: Color,  // → accent  (magenta)
    pub cc_perf:     Color,  // → warning (yellow)
    pub cc_test:     Color,  // → primary (cyan)
    pub cc_chore:    Color,  // → muted
    pub cc_revert:   Color,  // → danger dimmed
}

/// Icon set — swappable for environments without Unicode support.
pub struct Icons {
    pub info:     &'static str,  // "ℹ"
    pub success:  &'static str,  // "✓"
    pub warning:  &'static str,  // "⚠"
    pub error:    &'static str,  // "✗"
    pub tip:      &'static str,  // "▶"
    pub current:  &'static str,  // "◉"
    pub other:    &'static str,  // "◯"
    pub ahead:    &'static str,  // "↑"
    pub behind:   &'static str,  // "↓"
    pub added:    &'static str,  // "✚"
    pub modified: &'static str,  // "✎"
    pub deleted:  &'static str,  // "✖"
}

pub struct Theme {
    pub palette: Palette,
    pub icons:   Icons,
}

impl Theme {
    pub fn default_dark() -> Self { … }
    // future: pub fn default_light() -> Self { … }
    // future: pub fn from_config(cfg: &UiConfig) -> Self { … }
}

// Global theme — set once at startup, read everywhere.
static THEME: OnceLock<Theme> = OnceLock::new();
pub fn current() -> &'static Theme { THEME.get_or_init(Theme::default_dark) }
pub fn init(theme: Theme) { let _ = THEME.set(theme); }
```

Initialized in `main.rs` after config load:

```rust
ui::theme::init(Theme::from_config(&cfg.ui));
```

---

## Config Addition

```toml
[ui]
commit_mode = "interactive"  # "interactive" | "inline"
```

- `"interactive"` — full-screen ratatui TUI commit builder (default)
- `"inline"` — step-by-step prompts in the terminal stream (classic style,
  same ergonomics as the current `dialoguer`-based flow but styled via ratatui)

---

## Dependency Changes (Cargo.toml)

```toml
# Add
ratatui        = { version = "0.30", features = ["crossterm"] }
ratatui-cheese = "0.6"

# Bump (ratatui 0.30 requires crossterm 0.29; currently on 0.27)
crossterm = "0.29"

# Phase out in Phase 2
colored   = "2"     # → replaced by ratatui Style / Span in print.rs

# Phase out in Phase 3
dialoguer = "0.11"  # → replaced by interactive.rs
indicatif = "0.17"  # → replaced by ratatui-cheese Spinner in render.rs

# Keep
console   = "0.15"  # terminal_width() — may migrate to crossterm later
```

---

## Component API

### Mode 1 — Styled print (`print.rs`)

```rust
pub fn print_info(msg: &str)
pub fn print_success(msg: &str)
pub fn print_warning(msg: &str)                          // → stderr
pub fn print_error(msg: &str)                            // → stderr
pub fn print_tip(msg: &str)
pub fn print_blank()
pub fn print_step(n: usize, total: usize, msg: &str)
pub fn print_key_value_pairs(pairs: &[(&str, String)])
pub fn print_section(title: &str, count: Option<usize>) // bold header + ─── rule
pub fn print_rule()
```

All helpers build `ratatui::text::Line` / `Span` values styled with
`theme::current().palette`, then flush through crossterm to stdout.
The `theme::current()` call means every color comes from the active palette —
switching themes later requires no changes at the call site.

### Mode 2 — Inline widgets (`widgets.rs`)

```rust
// Section header with slash fill — left-aligned title.
// Output: /////  Title  ///////////////////////////////////
pub fn print_fieldset(title: &str)
pub fn print_fieldset_with_count(title: &str, count: usize)

// Table — uses ratatui Table widget inside Viewport::Inline.
pub fn render_table(headers: &[&str], rows: Vec<Vec<String>>)

// Tree — uses ratatui-cheese Tree widget inside Viewport::Inline.
pub struct TreeItem {
    pub label:      String,
    pub is_current: bool,
    pub meta:       String,
}
pub fn render_tree(root_label: &str, items: &[TreeItem])

// Diff stat bar (unchanged API, ratatui-backed colors).
pub fn render_stat_bar(added: usize, deleted: usize, width: usize) -> String

// Spinner — ratatui-cheese Spinner running in a background thread.
pub fn spinner(msg: &str) -> SpinnerHandle

pub struct SpinnerHandle { … }
impl SpinnerHandle {
    pub fn finish_success(self, msg: &str)
    pub fn finish_error(self, msg: &str)
    pub fn finish_and_clear(self)
}
```

**Spinner background-thread pattern** — `ratatui-cheese::spinner::Spinner` is
designed for controlled render loops, not blocking CLI operations. We wrap it
in a thread so the main thread can do real work while the spinner ticks:

```rust
pub fn spinner(msg: &str) -> SpinnerHandle {
    let done   = Arc::new(AtomicBool::new(false));
    let result = Arc::new(Mutex::new(None::<SpinnerResult>));

    let thread = std::thread::spawn({
        let done   = done.clone();
        let result = result.clone();
        let msg    = msg.to_string();
        move || {
            let mut terminal = ratatui::init_with_options(TerminalOptions {
                viewport: Viewport::Inline(1),
            });
            let mut state = SpinnerState::new(SpinnerType::Dot);
            let start = Instant::now();

            while !done.load(Ordering::Relaxed) {
                state.tick(start.elapsed());
                terminal.draw(|f| {
                    // render ratatui-cheese Spinner widget + msg label
                }).ok();
                std::thread::sleep(Duration::from_millis(80));
            }

            ratatui::restore();
            // render final static line: ✓ msg  or  ✗ msg  or clear
        }
    });

    SpinnerHandle { thread, done, result }
}
```

### Mode 3 — Full-screen TUI (`interactive.rs`)

```rust
pub fn select(prompt: &str, options: &[SelectOption]) -> Option<usize>
pub fn multi_select(prompt: &str, options: &[SelectOption]) -> Vec<usize>
pub fn fuzzy_select(prompt: &str, options: &[&str]) -> Option<usize>
pub fn input(prompt: &str, default: Option<&str>) -> Option<String>
pub fn confirm(prompt: &str, default: bool) -> bool
```

Every TUI screen renders a **ratatui-cheese `Help` widget** at the bottom of
the screen showing the active keybindings for that screen. Example:

```
  ─────────────────────────────────────────────────
  j/k move   Space toggle   Enter confirm   Esc back   q quit
```

---

## ratatui-cheese Modules Used

| Module        | Used for                                                              |
|---------------|-----------------------------------------------------------------------|
| `fieldset`    | Section headers — `FieldsetFill::Slash`, left-aligned title           |
| `spinner`     | Async spinner in background render thread during long-running ops     |
| `tree`        | Stack tree (`g stack view`), workspace tree (`g workspace list`)      |
| `list`        | Paginated lists where content may overflow the terminal height        |
| `theme::Palette` | Foundation for `ui::theme::Palette` — semantic color vocabulary    |
| `help`        | Keybinding hint bar at the bottom of every full-screen TUI screen     |
| `paginator`   | Page indicator for long lists (stack list, branch list)               |

---

## Command Mockups

> **Design language**
> - 2-space left margin on all lines (`INDENT = "  "`)
> - `/////  Title  ////…` (slash fieldset, left-aligned) for top-level command sections
> - Bold header + `───` rule for subsections within a command output
> - No box borders — terminal-first, scrollback-friendly
> - Colors come exclusively from `Theme::palette` — never hardcoded

---

### `g status`

```
  ℹ  On branch feat/ratatui-ui

  Staged Changes (2)
  ─────────────────────────────────────────────────
  ✚  A  src/ui/theme.rs
  ✎  M  src/ui/mod.rs

  Unstaged Changes (1)
  ─────────────────────────────────────────────────
  ✎  M  Cargo.toml

  Untracked (1)
  ─────────────────────────────────────────────────
  ?     ratatui.md

  tip:  g commit  — commit staged changes
  tip:  g add     — stage specific files interactively
```

---

### `g log`

```
  abc1234  feat: add workspace init command         Bozhidar D    3 days ago   ( HEAD → feat/ratatui-ui )
  def5678  fix: handle missing remote gracefully    Bozhidar D    5 days ago
  ghi9012  docs: update readme                      Bozhidar D    1 week ago
  jkl3456  chore: bump dependencies                 Bozhidar D    1 week ago
  mno7890  refactor: extract ui helpers to module   Bozhidar D    2 weeks ago  ( origin/main · main )
```

Streaming — lines flow to stdout as git emits them. Style tokens come from
`Theme::palette` (hash color, subject prefix colors, author color, date color,
ref decoration color) but no ratatui frame is used — the output scrolls
naturally in the terminal.

---

### `g branch`

```
  Branch               Status          Last Commit    Remote
  ────────────────     ──────────      ───────────    ────────────────
  ◉  main              up to date      2 days ago     origin/main
  ◯  feat/ratatui-ui   ↑ 3 ahead       1 hour ago     —
  ◯  fix/typo          ↑ 1  ↓ 2        3 days ago     origin/fix/typo
```

Rendered via `render_table` (Mode 2 / ratatui `Table` widget, inline viewport).

---

### `g commit` — interactive mode (`commit_mode = "interactive"`)

Full-screen ratatui TUI. Each step renders inside a persistent frame; Esc
moves back to the previous step. The help bar updates per step.

**Step 1/5 — Type**

```
/////  Commit Builder  ///////////////////////////

  [1/5]  Type

  > feat      A new feature
    fix       A bug fix
    docs      Documentation changes
    refactor  Code change without feature or fix
    perf      Performance improvement
    test      Adding missing tests
    chore     Build process or auxiliary tools
    style     Formatting, no logic change
    build     Build system changes
    ci        CI configuration changes
    revert    Revert a previous commit

  ─────────────────────────────────────────────────
  j/k move   Enter select   q quit
```

**Step 2/5 — Scope**

```
/////  Commit Builder  ///////////////////////////

  ✓  feat

  [2/5]  Scope  (optional — Enter to skip)

  > _

  ─────────────────────────────────────────────────
  Enter confirm   Esc back
```

**Step 3/5 — Subject**

```
/////  Commit Builder  ///////////////////////////

  ✓  feat(ui)

  [3/5]  Subject

  > migrate output helpers to ratatui style system_

  ─────────────────────────────────────────────────
  Enter confirm   Esc back                  68/72
```

**Step 4/5 — Body**

```
/////  Commit Builder  ///////////////////////////

  ✓  feat(ui): migrate output helpers to ratatui style system

  [4/5]  Body  (optional — Enter to skip)

  > _

  ─────────────────────────────────────────────────
  Enter confirm   Esc back
```

**Step 5/5 — Confirm**

```
/////  Commit Builder  ///////////////////////////

  feat(ui): migrate output helpers to ratatui style system

  Staged files (3)
  ─────────────────────────────────────────────────
  src/ui/theme.rs
  src/ui/mod.rs
  Cargo.toml

  Commit? [Y/n]  _
```

---

### `g commit` — inline mode (`commit_mode = "inline"`)

Step-by-step prompts that scroll down the terminal stream. Styled via ratatui
but no frame is taken; output remains in scrollback.

```
  [1/5]  Type
  > feat

  [2/5]  Scope (optional)
  > ui

  [3/5]  Subject
  > migrate output helpers to ratatui style system

  [4/5]  Body (optional — empty to skip)
  >

  [5/5]  Breaking change? [y/N]
  >

  ✓  feat(ui): migrate output helpers to ratatui style system
```

---

### `g add` (full-screen TUI)

Replaces the custom crossterm file picker in `git.rs` with a ratatui
full-screen TUI. Shows unstaged changes and untracked files; Space toggles
selection; Enter stages the selected files.

```
/////  Stage Files  //////////////////////////////

  Unstaged Changes
  ─────────────────────────────────────────────────
  [✓]  ✚  A  src/ui/theme.rs
  [ ]  ✎  M  src/ui/mod.rs
  [ ]  ✎  M  Cargo.toml

  Untracked
  ─────────────────────────────────────────────────
  [ ]  ?     ratatui.md

  ─────────────────────────────────────────────────
  Space toggle   a all   n none   Enter confirm   q quit
```

---

### `g workspace list`

```
/////  Workspaces  ///////////////////////////////

  ◉  main                                [primary]
  │
  ├── ◯  feat/ratatui-ui    2 days ago
  │
  └── ◯  fix/typo           5 days ago

  3 workspaces
```

Tree rendered via ratatui-cheese `Tree` widget (Mode 2, inline viewport).

---

### `g workspace status`

```
/////  Current Workspace  ////////////////////////

  Name      feat/ratatui-ui
  Branch    feat/ratatui-ui
  Path      /Users/b/Github/vcli/ratatui
  Created   2 days ago

/////  Worktrees  ////////////////////////////////

  ◉  main              /Users/b/Github/vcli/main
  │
  ├── ◯  feat/ratatui   /Users/b/Github/vcli/feat-ratatui-ui
  │
  └── ◯  fix/typo       /Users/b/Github/vcli/fix-typo
```

---

### `g stack list`

```
  Stack            Active   Branches   State
  ─────────────    ──────   ────────   ────────────
  my-feature       ◉        3          synced
  auth-refactor    ◯        2          ↑ 1 ahead
```

Rendered via `render_table` (Mode 2).

---

### `g stack view`

```
/////  Stack: my-feature  ////////////////////////

  ◉  feat/step-3    ↑ 2 ahead   PR #123  github.com/org/repo/pull/123
  │
  ├── ◯  feat/step-2    ↑ 1 ahead   PR #122  github.com/org/repo/pull/122
  │
  └── ◯  feat/step-1    up to date  PR #121  github.com/org/repo/pull/121

  tip:  g stack up / g stack down  — navigate between branches
  tip:  g stack sync               — rebase the whole chain
  tip:  g stack pr                 — create or update GitHub PRs
```

---

### `g stack details`

```
/////  Stack: my-feature  ////////////////////////

  ◉  feat/step-3
  │    ↑ 2 ahead of feat/step-2
  │    PR #123  Open    github.com/org/repo/pull/123
  │
  ├── ◯  feat/step-2
  │    ↑ 1 ahead of feat/step-1
  │    PR #122  Open    github.com/org/repo/pull/122
  │
  └── ◯  feat/step-1
       up to date with main
       PR #121  Merged  github.com/org/repo/pull/121
```

---

### `g compare`

```
/////  feat/ratatui-ui → main  ///////////////////

  ↑ 3 ahead   ↓ 0 behind

  Commits (3)
  ─────────────────────────────────────────────────
  abc1234  feat: migrate ui helpers to ratatui   Bozhidar D   1 hour ago
  def5678  chore: add ratatui to Cargo.toml      Bozhidar D   2 hours ago
  ghi9012  docs: add ratatui.md plan             Bozhidar D   3 hours ago

  Changed Files (5)
  ─────────────────────────────────────────────────
  M  src/ui/mod.rs     ██████░░░░  +42  -12
  M  Cargo.toml        ████░░░░░░  +8   -2
  A  src/ui/theme.rs   ██████████  +120 -0
  A  src/ui/print.rs   ██████████  +80  -0
  A  ratatui.md        ██████████  +200 -0
```

---

### `g config`

```
/////  Configuration  ////////////////////////////

  Config file   /Users/b/.config/g/config.toml
  Database      /Users/b/.config/g/g.db

/////  General  //////////////////////////////////

  default_branch    main
  auto_fetch        false
  pager             (auto)

/////  UI  ///////////////////////////////////////

  colors            true
  icons             true
  date_format       relative
  log_limit         30
  show_graph        true
  commit_mode       interactive

/////  Commit  ///////////////////////////////////

  require_scope      false
  require_body       false
  max_subject        72
  gpg_sign           false
  emoji              false

/////  Diff  /////////////////////////////////////

  tool              auto
  context_lines     3

/////  GitHub  ///////////////////////////////////

  api_base          https://api.github.com
  token             *** (set)
```

Each `[section]` in the config gets its own slash fieldset, rendered via
`print_fieldset` (Mode 2) followed by `print_key_value_pairs` (Mode 1).

---

### `g show <hash>`

```
/////  abc1234  //  feat: add workspace init  ////

  Author    Bozhidar Dryanovski
  Date      Mon Apr 01 2026  (3 days ago)
  Branch    feat/ratatui-ui

//////////////////////////////////////////////////

(diff streams below — passthrough to delta / diff-so-fancy / builtin)
```

The header block uses `print_fieldset` with both a left title (the hash) and
right title (the subject). The diff output streams via the existing passthrough
path and is not affected by the ratatui migration.

---

### `g developer repos`

```
/////  Tracked Repositories  ////////////////////

  ID   Path                                    Last Used
  ──   ─────────────────────────────────────   ─────────
  1    /Users/b/Github/vcli/ratatui            1 hour ago
  2    /Users/b/Github/other-project           3 days ago

  2 repositories
```

---

### Dry run banner

```
/////  Dry Run  //////////////////////////////////

  Preview mode — no changes will be made.

  Step 1 ▸  git checkout -b feat/ratatui-ui
            Create a new feature branch

  Step 2 ▸  git push origin feat/ratatui-ui
            Push the branch to remote

  ─────────────────────────────────────────────────
  2 steps would run
```

---

## Migration Phases

### Phase 1 — Foundation ✅ COMPLETE

**Scope:** infrastructure only, no visible behavior change.

- [x] Add `ratatui 0.30`, `ratatui-cheese 0.6` to `Cargo.toml`; bump `crossterm` to `0.29`
- [x] Create `src/ui/theme.rs` — `Theme`, `Palette`, `Icons` structs with `default_dark()`
- [x] Create `src/ui/render.rs` — `ct_color`, `paint_*`, `Spinner` (ratatui-cheese
  `SpinnerState`-backed), `ProgressBar`, `terminal_width`
- [x] Add `commit_mode` and `theme` fields to `UiConfig`; TOML defaults updated
- [x] Initialize theme in `main.rs`: `ui::theme::init(Theme::from_config(&cfg.ui.theme))`

### Phase 2 — Static output ✅ COMPLETE

**Scope:** replace `colored` + `indicatif`; all output now theme-aware.

- [x] `src/ui/print.rs` — semantic styling helpers (`primary`, `success`, `muted`, …),
  all `print_*` helpers, `print_indented`, `print_line`
- [x] `src/ui/widgets.rs`:
  - `print_fieldset` — ratatui-cheese `Fieldset` with `FieldsetFill::Slash`, rendered
    via `Buffer` → `print_buffer_row` (no `Terminal` instance needed)
  - `Table` — ANSI-aware column widths via `console::measure_text_width`
  - `commit_subject_width(show_graph)` — dynamic subject column width from `terminal_width()`
  - `render_stat_bar`, `format_refs`, `format_ahead_behind`, `colorize_graph`
  - `color_*` git helpers all reading from `theme::current()`
- [x] `src/ui/mod.rs` — pure re-export facade; all `ui::*` call sites unchanged
- [x] `colored` crate removed; `indicatif` crate removed
- [x] `print_fieldset` wired to: `g config` (all sections), `g workspace list/status`,
  `g stack list/view/details`, `g stack pr`, `g compare`, `g show`, `g developer repos`,
  dry-run banner/footer
- [x] Zero raw `println!` calls remain in command files

**Implementation notes:**
- ratatui-cheese `SpinnerState` (`SpinnerType::Dot`) drives the braille animation;
  each frame is rendered into a 1-cell `Buffer` and the symbol extracted.
- ratatui-cheese `Fieldset` is rendered to an in-memory `Buffer`; each row is flushed
  to stdout via `print_buffer_row` using crossterm — no `Terminal` required for static CLI output.

### Phase 3 — Interactive components ✅ COMPLETE

**Scope:** replace `dialoguer` + the custom crossterm file picker.

- [x] `src/ui/interactive.rs`:
  - `select` — list picker with ratatui-cheese `Help` bar + window-based scroll + `Paginator`
  - `multi_select` — checkbox list with same features
  - `fuzzy_select` — real-time substring filter with arrow-key navigation
  - `input` / `input_validated` — text input with cursor, backspace, validation
  - `confirm` — yes/no with arrow-key toggle and `y`/`n` shortcuts
  - `is_interactive()` helper checks both `is_terminal()` and `is_no_interactive()`
- [x] `g commit` — uses `build_commit_message_interactive()` (ratatui TUI) for both
  `commit_mode` values; non-TTY without `--message` gives a clear error
- [x] `g add` — two sequential pickers:
  1. "Unstage Files" — shows staged files; selection runs `git restore --staged`
  2. "Stage Files" — shows unstaged + untracked; selection runs `git add`
- [x] Workspace: `Confirm` → `ui::confirm`, `FuzzySelect` → `ui::fuzzy_select`,
  `MultiSelect` → `ui::multi_select`
- [x] `dialoguer` crate removed; custom crossterm picker deleted from `git.rs`

**Implementation notes:**
- ratatui-cheese `Binding` / `Help` (not `HelpItem`) is the correct type for the
  keybinding bar — `Help::default().bindings(vec![Binding::new(key, desc)])`.
- ratatui-cheese `Paginator` uses `StatefulWidget`; call via
  `f.render_stateful_widget(paginator, area, &mut state)`.

### Phase 4 — Polish ✅ COMPLETE

**Scope:** finish-line improvements and future-proofing.

- [x] `Theme::default_light()` — blue primary, black text, dark colors for light terminals
- [x] `Theme::from_config(mode)` — factory mapping `"dark"` / `"light"` / unknown → dark
- [x] `[ui] theme = "dark"` in `UiConfig` with `#[serde(default)]`; shown in `g config`
- [x] `--no-interactive` global flag in `Cli` struct; `ui::set_no_interactive()` /
  `ui::is_no_interactive()` global in `render.rs`; all interactive functions respect it
- [x] ratatui-cheese `Paginator` (Arabic mode) in `select` and `multi_select` for lists
  taller than the terminal — `four_zone` layout adds a paginator row between list and help
- [x] ratatui-cheese `Help` widget on every TUI screen with context-specific bindings
- [x] Zero raw `println!` / `eprintln!` calls remain outside `src/ui/`

---

## Backward Compatibility Notes

- All `ui::*` function signatures in `mod.rs` remain identical. Command files
  (`commands/*.rs`) do not reference any sub-modules directly.
- The `INDENT` constant stays at `"  "` (2 spaces) — single source of the left margin.
- `terminal_width()` uses `console::Term::stdout().size()` for ANSI-aware measurement.
- Dry-run mode is unaffected — it prints planned steps via Mode 1 output.
- `--no-interactive` is safe for CI/piped use: all interactive functions return
  their default values immediately without blocking.

---

## Resolved questions

1. **`g log` column proportions** — resolved: `commit_subject_width(show_graph)` in
   `widgets.rs` computes the subject column dynamically from `terminal_width()` minus
   the fixed-column budget (hash, author, date). The hardcoded `55` is gone.

2. **`g add` unstaging** — resolved: `interactive_add` now presents two sequential
   pickers — "Unstage Files" first (if staged changes exist), then "Stage Files"
   (if unstaged/untracked files exist).  Unstaging runs `git restore --staged`.

3. **Phase granularity** — all four phases implemented and merged sequentially.

---

## Post-plan additions

### Shell completions (`g completions <shell>`)

Added `clap_complete` dependency and a top-level `g completions <shell>` command that
writes a shell completion script to stdout.  Supports all shells that `clap_complete`
covers: `bash`, `zsh`, `fish`, `elvish`, `powershell`.

Usage:
```sh
# Zsh
g completions zsh > ~/.zsh/completions/_g
# Bash
g completions bash >> ~/.bash_completion
# Fish
g completions fish > ~/.config/fish/completions/g.fish
```

### ratatui-cheese `Tree` widget — decision not to replace manual rendering

The `Tree` widget uses a chevron (`▼`/`▶`) expand/collapse visual style.  Our stack
and workspace trees use git-style `├──`/`└──` connectors which are a better fit for
a developer tool context and match the conventions users expect from git GUIs.  The
`Tree` widget would change the visual language significantly with no functional gain
for static (always-expanded) CLI output.

The widget is noted as a future item for a full-screen interactive tree browser
(e.g., `g workspace explore`).  The `print_stack_tree` stub in `widgets.rs` remains
as a planned API surface for that feature.

### `commit_mode` default changed to `"interactive"`

Both `"interactive"` and `"inline"` values now use the full-screen ratatui TUI commit
builder (`build_commit_message_interactive`).  The distinction is reserved for a future
streaming-inline mode.  The default was changed from `"inline"` → `"interactive"` to
be honest about what actually happens.
