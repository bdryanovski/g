---
title: Stats
description: Turn your git history into a local, private dashboard — heatmaps, top commands, commit-type breakdowns, and fuzzy search.
order: 9
---

`g stats` renders a **usage dashboard right in your terminal** — a contribution
heatmap, the commands you run most, your conventional-commit mix, busiest hours,
top authors, and more. Everything is computed locally and stored in
`~/.config/g/g.db` (SQLite). **Nothing is uploaded; nothing phones home.**

```bash
g stats
```

> First run feels a little empty? Some sections (commit search, duplicates,
> message length) read from an imported history. Run `g stats --import` once to
> backfill, then re-run `g stats`.

## Quick start

```bash
g stats                       # full report, last 365 days
g stats --days 90             # narrow the time-based sections to 90 days
g stats --no-git              # skip sections that shell out to git
g stats --import              # backfill commit history into the local db
g stats --search "fix login"  # fuzzy-search commit messages
g stats --duplicates          # find commit messages you've reused
g stats --message-stats       # subject-length trends over time
```

## What's in the report

A full `g stats` run prints these sections, each as its own panel:

| Section | What it shows | Source |
|---------|---------------|--------|
| **Usage Overview** | Totals: commands run, commits, active days | local db |
| **Commit Heatmap — Last 52 Weeks** | GitHub-style contribution grid | git |
| **Lines Changed — Last 60 Commits** | Additions vs deletions per commit | git |
| **Top Commands** | The `g`/git subcommands you run most | local db |
| **Commit Types** | Your `feat` / `fix` / `docs` … mix from `g commit` | local db |
| **Repository Activity** | Commits, churn, and recent momentum | git |
| **Activity by Hour (UTC)** | When in the day you commit | git |
| **Top Authors** | Who's been committing in this repo | git |

Pass `--no-git` to print only the panels that come from the local database —
handy in a directory that isn't a git repository, or for a fast summary.

## Importing history

The command-tracking panels grow naturally as you use `g`. The
**commit-message** features (search, duplicates, message length) need a one-time
import of your existing history:

```bash
g stats --import              # import everything
g stats --import-limit 2000   # cap the import to the most recent N commits
```

Re-running `--import` is safe: existing commits are skipped, only new ones are
added.

## Search your commit messages

Fuzzy-search across everything you've imported:

```bash
g stats --search "auth"
g stats --search "revert migration"
```

Each hit shows the short hash, author, date, and subject. If you get no
results, import first:

```bash
g stats --import && g stats --search "auth"
```

## Find duplicate & repetitive messages

Spot copy-pasted or low-signal commit subjects:

```bash
g stats --duplicates          # messages used more than once
```

Pair it with message statistics to see whether your subjects are trending too
long (the conventional-commit sweet spot is ≤ 72 characters):

```bash
g stats --message-stats
```

This prints overall length statistics plus a **12-month trend** of average
subject length, and a *Top Authors* / *Top Duplicate Messages* breakdown.

## Where the data lives

| Path | Contents |
|------|----------|
| `~/.config/g/g.db` | SQLite database: command usage + imported commits |
| `~/.config/g/config.toml` | Your configuration (see [Configuration](./configuration/)) |

Because it's a plain SQLite file, it's easy to inspect or wipe:

```bash
g developer db                # open the database in sqlite3
rm ~/.config/g/g.db           # start fresh (re-created on next run)
```

## Tips

- Run `g stats --days 30` on a Monday for a quick "what did I ship last month".
- `g stats --no-git` is instant — great for a shell prompt widget or alias.
- Combine with [Theming](./theming/): a `heavy`-border theme makes the heatmap
  and bars pop on a projector.

## Related docs

- [Configuration](./configuration/) — where the database and config live
- [CLI Reference](./cli-reference/) — every flag for `g stats`
- [Log & diff](./log-and-diff/) — the everyday history commands
