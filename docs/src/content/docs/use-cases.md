---
title: Use cases
description: Detailed, personal playbooks—who you are, what you’re trying to do, and exact commands.
order: 1
---

These are **workflow recipes** written as if you’re in the middle of a real day: a bit of context, a concrete goal, and **what to type** (and **why**). For animated diagrams next to the ideas, see [Git flows](./git-flows/), [Stacks](./stacks/), and [Workspaces](./workspaces/).

---

## Split a huge feature into stacked PRs

**Who:** You ship backend or full-stack work. You have been on one branch for weeks and the diff has become unreviewable.

**Goal:** Land **payment schema**, then **API**, then **webhooks** as separate PRs, each targeting the branch below until everything reaches `main`.

**What to do**

1. Start from fresh `main`:

   ```bash
   git switch main && git pull
   g stack new payments
   ```

2. First PR slice — only persistence / models:

   ```bash
   g stack add pay/db-schema
   # implement, test, stage
   g commit -a -m "feat(pay): add ledger tables"
   ```

3. Second slice — HTTP on top of that schema:

   ```bash
   g stack add pay/api
   g commit -a -m "feat(pay): expose REST endpoints"
   ```

4. Optional third slice — async / webhooks / workers:

   ```bash
   g stack add pay/webhooks
   g commit -a -m "feat(pay): handle provider callbacks"
   ```

5. Inspect locally before GitHub:

   ```bash
   g stack view
   g stack details
   ```

6. Push and open chained PRs (`GITHUB_TOKEN` required for `pr`):

   ```bash
   g stack push
   g stack pr --draft
   ```

**When `main` moves** while you wait on review: run `g stack sync`, fix conflicts, `g stack push` again. More detail: [Stacks](./stacks/).

---

## Hotfix production without abandoning your feature branch

**Who:** You are mid-feature on `feat/notifications` with a dirty tree. Something on `main` must be fixed now.

**Goal:** Fix prod in **another directory**, same repo, without `git stash` gymnastics.

**What to do**

1. From a clean-enough moment at repo root (commit or stash if the *main* repo must be clean—worktree creation rules still follow Git):

   ```bash
   git switch main && git pull
   g workspace create hotfix-oauth
   ```

   Or attach to an existing fix branch:

   ```bash
   g workspace create hotfix-oauth -b fix/oauth-token
   ```

2. Enter that checkout:

   ```bash
   g workspace switch hotfix-oauth
   pwd
   ```

3. Fix, test, commit, push, open PR—same as always.

4. Leave the subshell when done: `exit`.

5. Your original folder is still on `feat/notifications` with your layout intact.

6. After merge, remove the extra tree:

   ```bash
   g workspace delete hotfix-oauth
   ```

**Why it feels better:** separate `node_modules`, build artifacts, and editor roots; no “where did my stash go” Monday. [Workspaces](./workspaces/).

---

## Review a branch without opening GitHub for history

**Who:** You lead review or you triage incoming work.

**Goal:** Answer “what landed on `feature/flags` since `main`?” in the terminal, with a graph and readable subjects.

**What to do**

```bash
g log main..feature/flags --oneline -n 30
g compare --commits main feature/flags
```

Before a deep read, skim file churn:

```bash
g compare --stat main feature/flags
```

Full patch through your team diff tool:

```bash
g compare --diff main feature/flags
```

Tune limits and colors under `[ui]` and `[diff]` in config. [Log & diff](./log-and-diff/).

---

## Make diffs look the same for everyone on the team

**Who:** You care about consistency in Slack screenshots and local review.

**Goal:** One tool drives `g diff`, `g show`, and `g compare --diff`.

**What to do**

1. Install [delta](https://github.com/dandavison/delta) (or [diff-so-fancy](https://github.com/so-fancy/diff-so-fancy)).

2. Pin it in `~/.config/g/config.toml`:

   ```toml
   [diff]
   tool = "delta"
   ```

3. Verify:

   ```bash
   g diff HEAD~1
   g show HEAD
   ```

4. Paste the snippet into your team onboarding doc. [Configuration](./configuration/).

---

## Solo maintainer: better UX without adopting stacks

**Who:** It is just you; you may never run `g stack pr`.

**Goal:** Nicer log/status/diff and optional conventional commits, still full `git` passthrough.

**What to do**

- Keep using `g pull`, `g push`, `g switch`, etc.—they forward to `git`.

- Prefer `g log` and `g status` for daily reading.

- Try interactive commit once:

  ```bash
  g commit
  ```

Stacks and worktrees stay optional until a problem actually hurts. [Introduction](./introduction/).

---

## Repair a stack after you amended or merged into a lower branch

**Who:** You already use stacks. You rewrote `pay/db-schema` or merged `main` into it; `pay/api` still sits on the old parent.

**Goal:** Let `g` rebase the chain in order instead of hand-rolling five `git rebase` commands.

**What to do**

```bash
g stack details
g --dry-run stack sync
g stack sync
# resolve conflicts per step, then:
g stack push
```

Story-level picture: [Git flows](./git-flows/).

---

## Two long-running tracks at once without two clones

**Who:** You owe **search** and **billing** in parallel; both take weeks.

**Goal:** Two checkouts, one object database, names you will remember.

**What to do**

```bash
git switch main && git pull
g workspace create search-v2 --description "Elasticsearch rollout"
g workspace create billing-ui -b feat/billing-shell
g workspace list
```

Switch with `g workspace switch` (no argument opens the fuzzy picker) and `exit` when done. Delete a workspace when that line of work ships: `g workspace delete billing-ui`.

---

## Start a project workspace-first

**Who:** You are cloning a repo that you know will involve parallel branches — a client project, a new service, a long engagement.

**Goal:** Get the container layout (all worktrees as named sub-directories) from the very first `clone`.

**What to do**

```bash
g clone https://github.com/org/api-service.git --workspace
cd api-service/main
```

`g` detects the remote default branch before cloning, creates the container, and places the primary checkout at `api-service/main/`. From now on `g workspace create <name>` puts new worktrees at `api-service/<name>/`.

---

## Convert an existing repo to workspace layout

**Who:** You cloned the repo months ago. You have started accumulating stashes and losing track of which terminal is on which branch.

**Goal:** Reorganise the existing clone in place without losing work.

**What to do**

1. Commit or stash anything in progress, then run:

   ```bash
   cd ~/projects/myapp
   g workspace init
   ```

2. `init` prints exactly what will move and asks for confirmation. After you approve, navigate to the new inner location:

   ```bash
   cd ~/projects/myapp/main
   ```

3. Create your first extra workspace — it lands inside the container automatically:

   ```bash
   g workspace create feature-x
   # → ~/projects/myapp/feature-x/
   ```

---

## Copy local config files to a new workspace

**Who:** Your project uses `.env` and `config/local.yml` that are gitignored and only live in your checkout. You need them in every new worktree.

**Goal:** New workspace with all your local config files already in place — no manual copying.

**What to do**

```bash
g workspace create feature-payments --copy
```

An interactive checklist shows every untracked and gitignored file from your current workspace. Toggle with Space, confirm with Enter. Only the files you select are copied.

---

## Cut a release: sober diff against main

**Who:** You are stabilising `release/1.4` against `main`.

**Goal:** Commit list + file stats before tagging.

**What to do**

```bash
g compare --commits main release/1.4
g compare --stat main release/1.4
g compare --diff main release/1.4   # optional full patch
```

---

## Onboard a teammate who will use stacks on GitHub

**Checklist you can forward**

1. Install — [Installation](./installation/).

2. Token:

   ```bash
   export GITHUB_TOKEN=ghp_…
   ```

3. Show config path and optional edit for default labels/reviewers:

   ```bash
   g config --path
   g config --edit
   ```

4. Reading order: [Introduction](./introduction/) → [Git flows](./git-flows/) → this page’s **Split a huge feature** section.

5. First exercise: dummy repo, `g stack new practice`, two `g stack add` layers, `g stack view`, tear down.

---

## Quick lookup: situation to command

| I need to… | Start here |
|------------|------------|
| Split a giant PR | `g stack new` / `g stack add` / `g stack pr` |
| Fix prod while coding a feature | `g workspace create` / `g workspace switch` |
| Clone a repo workspace-ready | `g clone <url> --workspace` |
| Convert an existing repo to worktree layout | `g workspace init` |
| Navigate between workspaces interactively | `g workspace switch` (no argument) |
| Copy `.env` and local config to a new workspace | `g workspace create <name> --copy` |
| Check out a remote branch as a workspace | `g workspace create <name> -b <branch>` (auto-tracks `origin/`) |
| Read branch history fast | `g log`, `g compare --commits` |
| Unify diff appearance | `[diff] tool` in config |
| Fix stack after amending mid-stack | `g stack sync` |
| One commit per stacked branch | `g stack squash` (then `g stack push --force` if already pushed) |
| Merge a stack layer into its parent (keep history) | `g stack fold` (`--keep` to keep the child branch name) |
| One commit on a plain feature branch | `g branch squash` (set upstream or pass `--base`) |
| Preview destructive stuff | `g --dry-run …` |

---

## Next

- [Installation](./installation/)  
- [Introduction](./introduction/)  
- [Git flows](./git-flows/)  
- [Stacks](./stacks/) / [Workspaces](./workspaces/)  
