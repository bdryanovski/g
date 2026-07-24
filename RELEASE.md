# Release Guide

This document describes how to create, validate, and publish releases for `g`.

## Version Numbering

We use [Calendar Versioning](https://calver.org/) with the format `YY.M.PATCH`:

- **YY**: Two-digit year (e.g., 24, 25, 26)
- **M**: Month without leading zero (1-12)
- **PATCH**: Incremental patch number within the month (starts at 0)

**Examples:**
- `v24.7.0` — First release in July 2024
- `v24.7.1` — Second release in July 2024 (bug fix)
- `v24.8.0` — First release in August 2024
- `v26.1.0` — First release in January 2026

**Pre-release versions** use suffixes: `v24.7.0-alpha`, `v24.7.0-beta.1`, `v24.7.0-rc.1`

## Creating a Release

### 1. Prepare the Release

```bash
# Ensure you're on main and up to date
git checkout main
git pull origin main

# Verify all tests pass
cargo test

# Verify the build works
cargo build --release

# Check for any uncommitted changes
git status
```

### 2. Determine Version

Calculate the version based on today's date:

```bash
# Format: YY.M.PATCH
# Example for first release in July 2024: 24.7.0
# Example for second release in July 2024: 24.7.1

# Check existing tags for this month
git tag -l "v$(date +%y.%-m).*"
```

### 3. Update Version

Update the version in `Cargo.toml`:

```toml
[package]
version = "24.7.0"  # Update to current YY.M.PATCH
```

Commit the version bump:

```bash
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to v24.7.0"
git push origin main
```

### 4. Create and Push Tag

```bash
# Create annotated tag
git tag -a v24.7.0 -m "Release v24.7.0"

# Push the tag (this triggers the release workflow)
git push origin v24.7.0
```

### 5. Monitor the Build

1. Go to **Actions** tab in GitHub
2. Watch the "Release" workflow
3. Wait for all platform builds to complete (~10-15 minutes)

### 6. Verify the Release

Once the workflow completes:

1. Go to **Releases** page
2. Verify all assets are present:
   - `g-x86_64-unknown-linux-gnu.tar.gz`
   - `g-aarch64-unknown-linux-gnu.tar.gz`
   - `g-x86_64-apple-darwin.tar.gz`
   - `g-aarch64-apple-darwin.tar.gz`
   - `g-x86_64-pc-windows-msvc.zip`
   - `checksums-sha256.txt`
   - Source code (zip and tar.gz)

3. Verify checksums:
   ```bash
   # Download checksums file
   curl -LO https://github.com/bdryanovski/g/releases/download/v24.7.0/checksums-sha256.txt
   
   # Download a binary
   curl -LO https://github.com/bdryanovski/g/releases/download/v24.7.0/g-aarch64-apple-darwin.tar.gz
   
   # Verify checksum
   sha256sum -c checksums-sha256.txt --ignore-missing
   ```

4. Test the install script:
   ```bash
   # Test installation
   curl -fsSL https://raw.githubusercontent.com/bdryanovski/g/main/install.sh | bash
   
   # Verify it works
   g --version
   g --help
   ```

## Manual Release (Workflow Dispatch)

You can also trigger a release manually without pushing a tag:

1. Go to **Actions** > **Release**
2. Click **Run workflow**
3. Enter the tag name (e.g., `v24.7.0`)
4. Click **Run workflow**

Note: This creates the release but doesn't create the git tag. You should create the tag afterwards:

```bash
git tag -a v24.7.0 -m "Release v24.7.0"
git push origin v24.7.0
```

## Validating a Release

### Quick Validation

```bash
# Test install script
curl -fsSL https://raw.githubusercontent.com/bdryanovski/g/main/install.sh | bash

# Check version
g --version

# Run basic commands
g --help
g status
g log --oneline -5
```

### Full Validation Checklist

- [ ] All platform binaries present in release
- [ ] Checksums file present and valid
- [ ] Install script works on macOS
- [ ] Install script works on Linux
- [ ] Install script works on Windows (WSL)
- [ ] `g --version` shows correct version
- [ ] `g --help` displays properly
- [ ] Basic git commands work (`status`, `log`, `diff`)
- [ ] Enhanced commands work (`branch`, `push`)
- [ ] Shell completions generate correctly

### Platform-Specific Testing

**macOS (Apple Silicon):**
```bash
curl -fsSL https://raw.githubusercontent.com/bdryanovski/g/main/install.sh | bash
g --version
```

**macOS (Intel):**
```bash
arch -x86_64 bash -c 'curl -fsSL https://raw.githubusercontent.com/bdryanovski/g/main/install.sh | bash'
```

**Linux:**
```bash
docker run --rm -it ubuntu:latest bash -c '
  apt-get update && apt-get install -y curl git
  curl -fsSL https://raw.githubusercontent.com/bdryanovski/g/main/install.sh | bash
  ~/.local/bin/g --version
'
```

**Windows (via WSL):**
```bash
wsl bash -c 'curl -fsSL https://raw.githubusercontent.com/bdryanovski/g/main/install.sh | bash'
```

## Hotfix Release

For urgent fixes to a released version:

```bash
# Create hotfix branch from the release tag
git checkout -b hotfix/v24.7.1 v24.7.0

# Make fixes
# ... edit files ...

# Commit and push
git add .
git commit -m "fix: critical bug description"
git push origin hotfix/v24.7.1

# Create PR to main, merge it, then:
git checkout main
git pull

# Tag the hotfix release
git tag -a v24.7.1 -m "Hotfix release v24.7.1"
git push origin v24.7.1
```

## Rollback a Release

If a release has critical issues:

1. **Delete the release** (not the tag) from GitHub Releases page
2. Fix the issues
3. Create a new patch release (e.g., v24.7.1)

Do NOT delete tags that have been published, as users may have already downloaded them.

## Release Notes

Release notes are auto-generated from commit messages between tags. For better notes:

- Use conventional commits: `feat:`, `fix:`, `docs:`, `chore:`
- Write clear, descriptive commit messages
- Reference issues: `fix: resolve crash on empty repo (#123)`

You can edit release notes after creation on the GitHub Releases page.

## Troubleshooting

### Build Fails on One Platform

1. Check the workflow logs for the specific platform
2. Common issues:
   - Missing cross-compilation tools (Linux ARM)
   - Code signing issues (macOS)
   - Windows path length limits

### Release Workflow Doesn't Trigger

- Ensure the tag matches pattern `v*`
- Check if workflow is enabled in repository settings
- Verify you have push permissions

### Checksums Don't Match

1. Re-download the file (might be corrupted transfer)
2. Check if the file was modified after download
3. Verify you're comparing the correct file
