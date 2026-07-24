---
title: Installation
description: Install g via installer script, prebuilt binaries, or from source.
order: 2
---

## Requirements

- **Git** (2.0+) — required, `g` is a Git enhancement layer
- **GitHub Token** — optional, needed for PR and stack features

Check versions:

```bash
git --version
```

## Quick Install (Recommended)

**Using curl:**

```bash
curl -fsSL https://raw.githubusercontent.com/bdryanovski/g/main/install.sh | bash
```

**Using wget:**

```bash
wget -qO- https://raw.githubusercontent.com/bdryanovski/g/main/install.sh | bash
```

The installer will:

1. Detect your OS and architecture
2. Download the correct binary from GitHub releases
3. Install to `~/.local/bin`
4. Show shell setup instructions

**Custom install location:**

```bash
INSTALL_DIR=/usr/local/bin curl -fsSL https://raw.githubusercontent.com/bdryanovski/g/main/install.sh | bash
```

## Manual Download

Download the appropriate archive from the [Releases page](https://github.com/bdryanovski/g/releases):

| Platform | Architecture  | Download                             |
| -------- | ------------- | ------------------------------------ |
| Linux    | x86_64        | `g-x86_64-unknown-linux-gnu.tar.gz`  |
| Linux    | ARM64         | `g-aarch64-unknown-linux-gnu.tar.gz` |
| macOS    | Intel         | `g-x86_64-apple-darwin.tar.gz`       |
| macOS    | Apple Silicon | `g-aarch64-apple-darwin.tar.gz`      |
| Windows  | x86_64        | `g-x86_64-pc-windows-msvc.zip`       |

**Extract and install:**

```bash
# Linux/macOS
tar -xzf g-*.tar.gz
chmod +x g
mv g ~/.local/bin/

# Windows (PowerShell)
Expand-Archive g-*.zip -DestinationPath .
Move-Item g.exe $env:LOCALAPPDATA\Programs\
```

## Build from Source

Requires the Rust toolchain (stable):

```bash
git clone https://github.com/bdryanovski/g.git
cd g
cargo install --path .
```

The binary is placed in `~/.cargo/bin` by default. Ensure it is on your `PATH`:

```bash
echo 'export PATH="$HOME/.cargo/bin:$PATH"' >> ~/.zshrc
source ~/.zshrc
```

## Update

Re-run the installer script, or:

```bash
# From source
cd g && git pull && cargo install --path . --force
```

## GitHub token (stacks & PRs)

Fine-grained or classic PAT with repo scope (as appropriate for your org):

```bash
export GITHUB_TOKEN=ghp_your_token_here
```

Persist in shell config:

```bash
echo 'export GITHUB_TOKEN=ghp_…' >> ~/.zshrc
```

Optional config file (prefer env for CI and shared machines):

```toml
# ~/.config/g/config.toml
[github]
# token = "ghp_…"
default_labels = ["needs-review"]
```

## Optional: shell alias

```bash
alias gg='g'
```

Aliasing **`git` itself** to `g` can break scripts that expect stock Git; prefer a dedicated command name.

## Verify enhanced commands

```bash
g log -n 3
g status
g diff --stat
```

## Shell completions

Generate completions for your shell:

```bash
# Bash
g completions bash >> ~/.bash_completion

# Zsh
g completions zsh > ~/.zsh/completions/_g

# Fish
g completions fish > ~/.config/fish/completions/g.fish
```

## Next

- [Introduction](./introduction/)
- [Configuration](./configuration/)
