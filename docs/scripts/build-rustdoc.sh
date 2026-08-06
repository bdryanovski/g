#!/usr/bin/env bash
#
# build-rustdoc.sh — Layer 1: API reference from `cargo doc`.
#
# Runs `cargo doc` on the crate, then copies the generated rustdoc HTML into
# docs/public/api/ so Astro serves it as static assets at /api/.
#
# The Astro page /api-reference/ embeds this in an iframe.
#
# Usage:  bash scripts/build-rustdoc.sh
#
# Env:
#   RUSTDOC_PRIVATE (default: 1) — set to 0 to document public items only.

set -euo pipefail

# Resolve the repo root (docs/scripts/build-rustdoc.sh → ../../)
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
DOCS_PUBLIC_API="$REPO_ROOT/docs/public/api"

echo "→ Building rustdoc (Layer 1: API reference)…"

cd "$REPO_ROOT"

PRIVATE_FLAG=""
if [[ "${RUSTDOC_PRIVATE:-1}" != "0" ]]; then
  PRIVATE_FLAG="--document-private-items"
fi

# Build docs for this crate only (no dependencies).  `cargo doc` writes to
# target/doc/ — the crate's HTML is at target/doc/<crate-name>/ (i.e. g/),
# but rustdoc also emits shared assets (static.files/, crates.js, etc.) as
# SIBLINGS of the crate directory.  The crate HTML references these via
# relative paths like "../static.files/...".  Therefore we must copy the
# ENTIRE target/doc/ tree, not just target/doc/g/, or every CSS/JS/font
# link will 404.
echo "  running cargo doc --no-deps $PRIVATE_FLAG"
cargo doc --no-deps $PRIVATE_FLAG

DOC_OUT="$REPO_ROOT/target/doc"
CRATE_DIR="$DOC_OUT/g"
if [[ ! -d "$CRATE_DIR" ]]; then
  echo "✗ Expected rustdoc output at target/doc/g/ but it is missing." >&2
  echo "  Contents of target/doc/:" >&2
  ls -1 "$DOC_OUT" 2>/dev/null >&2 || true
  exit 1
fi

# Atomically replace the old API dir with the full doc tree.
rm -rf "$DOCS_PUBLIC_API.tmp"
cp -R "$DOC_OUT" "$DOCS_PUBLIC_API.tmp"
rm -rf "$DOCS_PUBLIC_API"
mv "$DOCS_PUBLIC_API.tmp" "$DOCS_PUBLIC_API"

echo "✓ API reference written to docs/public/api/"
echo "  Crate index: /api/g/index.html"
echo "  Embed page:  /api-reference/"