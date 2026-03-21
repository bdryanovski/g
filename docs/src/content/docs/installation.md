---
title: Installation
description: Install g with Cargo from crates.io or from source, plus GitHub token setup.
order: 1
---

## Requirements

- **Rust toolchain** (stable) with `cargo` on your `PATH`
- **Git** installed and available as `git` (or configure `general.git_path` in config)

## Install from crates.io

When the crate is published, install the binary globally:

```bash
cargo install g
```

Verify:

```bash
g --version
```

> **Note:** The package name in this repository is `g` (see the root `Cargo.toml`). If the crates.io name differs when you publish, replace `g` with the published crate name.

## Install from this repository

Clone and install from the repo root (parent of `docs/`):

```bash
git clone https://github.com/YOUR_ORG/vcli.git
cd vcli
cargo install --path .
```

This compiles the `g` binary and places it in `~/.cargo/bin`. Ensure that directory is on your `PATH`.

## Optional: alias as `git`

Some users symlink or alias `g` to complement `git`. The tool is designed so **`g` is the command name**; aliasing `git` to `g` can work but may surprise scripts that expect vanilla Git behavior. Prefer calling `g` explicitly.

## GitHub token (stacks & PRs)

For `g stack pr` and related GitHub features, set a token:

```bash
export GITHUB_TOKEN=ghp_your_token_here
```

Add the same line to `~/.zshrc` or `~/.bashrc` as needed. You can also store a token under `[github]` in `config.toml` — environment variable is preferred for security and CI.

## Shell completions

Completion scripts are not generated yet by the CLI (see project roadmap). Until then, you can complete `g` as `git` for passthrough commands in many setups.
