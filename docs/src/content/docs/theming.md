---
title: Theming
description: Built-in themes, border styles, density, and how to build and load your own theme.
order: 8
---

`g` styles **everything** through a single theme: not just colors, but also
**icons**, **box-drawing borders**, **layout density**, and reusable
**component styles**. Each theme owns all of these — colors, borders **and**
spacing live together in the theme file — so switching theme reshapes the whole
look. You only pick the theme in `[ui]`:

```toml
[ui]
theme = "dark"   # color palette + borders + spacing, all from the theme file
```

Borders and spacing are defined **inside the theme**, not in `[ui]`. For
example each built-in ships its own combination:

| Theme | `border_style` | `density` |
|-------|----------------|-----------|
| `dark` / `light` / `solarized-dark` | `sharp` | `normal` |
| `dracula` | `rounded` | `normal` |
| `nord` | `rounded` | `relaxed` |
| `gruvbox` | `heavy` | `normal` |
| `monochrome` | `ascii` | `compact` |

If you want to force one style regardless of the active theme, set the optional
overrides in `[ui]` (commented out by default):

```toml
[ui]
# border_style = "rounded"   # override every theme's borders
# density = "compact"        # override every theme's spacing
```

## Built-in themes

Pick any of these by name with `theme = "<name>"`:

| Name | Description |
|------|-------------|
| `dark` | Default. ANSI colors tuned for dark terminals. |
| `light` | Darker colors for light backgrounds. |
| `dracula` | Vivid colors on a dark background. |
| `nord` | Cool, muted arctic palette. |
| `gruvbox` | Warm retro palette (dark variant). |
| `solarized-dark` | Ethan Schoonover's low-contrast classic. |
| `monochrome` | Grayscale only — minimal / e-ink terminals. |

Pick a theme interactively — `g config --themes` opens a picker of every
recognised theme (built-in **and** custom); the active one is tagged
`· current`. Choosing one writes it to `[ui] theme` (your config comments are
preserved) so the choice is remembered:

```bash
g config --themes        # arrow/j-k to move, Enter to select, Esc to cancel
```

When output is piped or `--no-interactive` is set, the same command just prints
the list with the active theme marked.

### Built-ins are editable files

The built-in themes are **not** hard-coded — they ship as TOML and are written
into your themes directory on first run:

```text
~/.config/g/themes/dark.toml
~/.config/g/themes/nord.toml
~/.config/g/themes/dracula.toml
…
```

Edit any of these in place to tweak a built-in — no recompile needed. Delete a
file and it is restored from the copy embedded in the binary on the next run,
so you can always get back to the original.

### A complete theme, end to end

Here is the shipped **`nord`** theme in full — a real, working file you can copy
and tweak. Note how colors, borders **and** spacing all live together:

```toml
# ~/.config/g/themes/nord.toml
name = "nord"

border_style = "rounded"
density      = "relaxed"  # roomy spacing for a calm, airy feel

[palette]
primary  = "#88c0d0"
success  = "#a3be8c"
warning  = "#ebcb8b"
danger   = "#bf616a"
muted    = "#4c566a"
text     = "#d8dee9"
accent   = "#b48ead"
divider  = "#4c566a"

cc_feat     = "#a3be8c"
cc_fix      = "#bf616a"
cc_docs     = "#81a1c1"
cc_refactor = "#b48ead"
cc_perf     = "#d08770"
cc_test     = "#88c0d0"
cc_chore    = "#4c566a"
cc_revert   = "#bf616a"
```

Switch to it and see it instantly:

```bash
g config --themes        # pick "nord" from the list
g status                 # rounded borders, airy spacing, arctic colors
```

## Border styles

A theme's `border_style` swaps every rule, table divider and tree connector in
lock-step:

| Value | Looks like |
|-------|-----------|
| `sharp` | `┌─┐ │ └─┘` (default) |
| `rounded` | `╭─╮ │ ╰─╯` |
| `heavy` | `┏━┓ ┃ ┗━┛` |
| `double` | `╔═╗ ║ ╚═╝` |
| `ascii` | `+-+ \| +-+` (also forces ASCII icons) |

## Density

A theme's `density` controls indentation and the spacing between sections:

| Value | Effect |
|-------|--------|
| `compact` | Single-space indent, no blank lines between sections. |
| `normal` | Balanced default. |
| `relaxed` | Wider indents and extra breathing room. |

## Build your own theme

A custom theme is a small TOML file. It **extends** a built-in palette and
overrides only the roles you care about — anything you omit is inherited.

### 1. Create the file

Put it in the themes directory so you can reference it by name:

```bash
mkdir -p ~/.config/g/themes
$EDITOR ~/.config/g/themes/midnight.toml
```

```toml
# ~/.config/g/themes/midnight.toml
name = "Midnight"        # informational only
extends = "nord"         # start from a built-in theme (default: "dark")

# Borders and spacing are part of the theme. If omitted they are inherited
# from `extends`.
border_style = "rounded" # sharp | rounded | heavy | double | ascii
density = "relaxed"      # normal | compact | relaxed
ascii_icons = false      # force the ASCII icon set

[palette]
# Override only what you want; everything else comes from `extends`.
primary = "#89b4fa"      # hex (#RGB or #RRGGBB)
success = "green"        # ANSI color name
warning = "228"          # 256-color index (0–255)
danger  = "brightred"    # bright / light variants are supported
accent  = "#cba6f7"
muted   = "#6c7086"
text    = "#cdd6f4"
```

### Color formats

Every palette value accepts:

- **Hex** — `#RGB` or `#RRGGBB` (true color), e.g. `#ff8800`
- **256-color index** — `0`–`255`, e.g. `228`
- **ANSI names** — `black`, `red`, `green`, `yellow`, `blue`, `magenta`,
  `cyan`, `gray`/`grey`, `darkgray`, `white`, and the `bright*` / `light*`
  variants (e.g. `brightcyan`).

### Palette roles

| Key | Used for |
|-----|----------|
| `primary` | info icon, spinner, active branch |
| `success` | checkmarks, added lines, current branch |
| `warning` | warnings, commit hashes, staged changes |
| `danger` | errors, deleted lines, remote refs |
| `muted` | dates, dividers, graph lines, dim text |
| `text` | general body text |
| `accent` | section titles, tags, special refs |
| `divider` | slash fill in section headers |
| `cc_feat`, `cc_fix`, `cc_docs`, `cc_refactor`, `cc_perf`, `cc_test`, `cc_chore`, `cc_revert` | Conventional-Commit type prefixes |

### 2. Load it

By name (resolved under `~/.config/g/themes/<name>.toml`):

```toml
[ui]
theme = "midnight"
```

…or by an explicit path from anywhere:

```toml
[ui]
theme = "/Users/me/dotfiles/g/ocean.toml"
```

Verify it was picked up:

```bash
g config --themes      # should mark your theme as active
g status               # see it live
```

If a theme can't be found or fails to parse, `g` prints a warning and falls
back to `dark`, so a broken theme never blocks a command.

## Related docs

- [Configuration](./configuration/) — the full `config.toml` reference
- [Log & diff](./log-and-diff/) — where colors and borders show up most
