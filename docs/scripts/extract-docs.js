#!/usr/bin/env node

// extract-docs.js — Layer 2: Module overviews from `//!` comments.
//
// Scans `src/**/*.rs` (configured via ROOT below) for module-level doc
// comments (`//!`), extracts their prose, detects feature-gated submodules
// (`#[cfg(feature = "...")]` preceding `mod xxx;`), and emits one Markdown
// page per module under OUTPUT_DIR.
//
// Generated files are build artefacts: they are git-ignored and recreated on
// every `npm run docs:extract`.  Never edit them by hand — edit the `//!`
// blocks in the Rust source instead.
//
// Usage:  node scripts/extract-docs.js
//
// No third-party dependencies — uses only Node built-ins.

import { readdir, readFile, writeFile, mkdir, rm } from "node:fs/promises";
import { dirname, join, relative, basename } from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = dirname(fileURLToPath(import.meta.url));
const DOCS_ROOT = join(__dirname, "..");
const REPO_ROOT = join(DOCS_ROOT, "..");
const RUST_SRC = join(REPO_ROOT, "src");
const OUTPUT_DIR = join(DOCS_ROOT, "src", "content", "docs", "modules");

// ── Walk a directory tree, returning every .rs file path ────────────────────
async function walkRs(dir) {
  const out = [];
  for (const ent of await readdir(dir, { withFileTypes: true })) {
    const full = join(dir, ent.name);
    if (ent.isDirectory()) out.push(...(await walkRs(full)));
    else if (ent.name.endsWith(".rs")) out.push(full);
  }
  return out;
}

// ── Extract the leading `//!` doc block of a file ───────────────────────────
// A module doc block is a contiguous run of lines starting with `//!`,
// appearing before any code.  Blank lines inside the run are allowed; the
// first non-blank, non-`//!` line terminates it.
function extractModuleDoc(lines) {
  const block = [];
  let started = false;
  for (const raw of lines) {
    const line = raw;
    const trimmed = line.trim();
    if (trimmed.startsWith("//!")) {
      started = true;
      // Strip `//! ` or `//!` prefix, preserving relative indentation.
      block.push(line.replace(/^\s*\/\/!\s?/, ""));
    } else if (started) {
      if (trimmed === "") {
        // Blank line inside the block — keep it (paragraph break).
        block.push("");
      } else {
        // First non-doc line → block is over.
        break;
      }
    }
  }
  // Trim trailing blank lines.
  while (block.length && block[block.length - 1] === "") block.pop();
  return block;
}

// ── Detect feature-gated submodule declarations ─────────────────────────────
// Looks for patterns like:
//
//   #[cfg(feature = "foo")]
//   mod bar;
//
// or multi-line attribute forms.  Returns an array of
//   { module: "bar", feature: "foo" }.
function extractFeatureGates(lines) {
  const gates = [];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trim();
    // Match  #[cfg(feature = "X")]  or  #[cfg(feature = "X", ...)]
    const m = line.match(/^#\[cfg\(.*feature\s*=\s*"([^"]+)"/);
    if (!m) continue;
    const feature = m[1];
    // Scan forward for the next `mod xxx;` (skipping blank/comment lines).
    let modName = null;
    for (let j = i + 1; j < Math.min(i + 6, lines.length); j++) {
      const lj = lines[j].trim();
      if (lj === "" || lj.startsWith("//")) continue;
      const mm = lj.match(/^mod\s+(\w+)\s*;/);
      if (mm) {
        modName = mm[1];
        break;
      }
      // If it's something else (not a mod decl), stop looking.
      break;
    }
    if (modName) gates.push({ module: modName, feature });
  }
  return gates;
}

// ── Turn a file path into a human-readable module title ─────────────────────
function moduleTitle(relPath) {
  // src/commands/git/mod.rs  → "commands::git"
  // src/main.rs              → "crate root (g)"
  let p = relPath.replace(/\\/g, "/");
  p = p.replace(/^src\//, "");
  p = p.replace(/\/mod\.rs$/, "");
  p = p.replace(/\.rs$/, "");
  if (p === "main" || p === "lib") return "crate root (g)";
  return p.split("/").join("::");
}

// ── Slug from a path (for the file name / URL) ─────────────────────────────
function moduleSlug(relPath) {
  let p = relPath.replace(/\\/g, "/");
  p = p.replace(/^src\//, "");
  p = p.replace(/\/mod\.rs$/, "");
  p = p.replace(/\.rs$/, "");
  if (p === "main" || p === "lib") p = "crate-root";
  return p.split("/").map((s) => s.replace(/_/g, "-")).join("-");
}

// ── Build the Markdown page for one module ──────────────────────────────────
function renderPage(file, docLines, gates) {
  const rel = relative(RUST_SRC, file).replace(/\\/g, "/");
  const title = moduleTitle(rel);
  const slug = moduleSlug(rel);

  // Derive a one-line description from the first non-empty doc line.
  const firstLine = docLines.find((l) => l.trim() !== "") ?? title;
  const description = firstLine.replace(/^#+\s*/, "").slice(0, 200);

  const sourceRel = relative(REPO_ROOT, file).replace(/\\/g, "/");

  let body = `---
title: "${title.replace(/"/g, '\\"')}"
description: "${description.replace(/"/g, '\\"')}"
section: modules
generated: true
order: 100
source: "${sourceRel}"
---

<!-- Auto-generated from ${sourceRel} //! comments.  Do not edit.        -->
<!-- Run \`npm run docs:extract\` to regenerate after editing the source. -->

`;

  // The module doc prose itself.  Rust doc-comments use fence attributes like
  // ```ignore, ```no_run, ```rust,ignore that are not valid Shiki languages —
  // normalise them so the Astro markdown renderer doesn't fall back or warn.
  const prose = docLines
    .join("\n")
    .trim()
    .replace(/```rust,?\s*(ignore|no_run|should_panic|edition\d+|compile_fail)/g, "```rust")
    .replace(/```(ignore|no_run|should_panic|compile_fail)\b/g, "```text");
  body += prose + "\n";

  // Feature-gated submodules section.
  if (gates.length > 0) {
    body += "\n\n---\n\n## Feature-gated submodules\n\n";
    body += "The following submodules are only compiled when their feature is enabled:\n\n";
    body += "| Submodule | Feature |\n|---|---|\n";
    for (const g of gates) {
      body += `| \`${g.module}\` | \`${g.feature}\` |\n`;
    }
  }

  const link = githubLink(sourceRel);
  body += link
    ? `\n\n> Source: [\`${sourceRel}\`](${link})\n`
    : `\n\n> Source: \`${sourceRel}\`\n`;
  return { slug, content: body };
}

// ── Best-effort GitHub source link ──────────────────────────────────────────
// Returns null when no real repo URL is configured so we avoid a broken link.
function githubLink(sourceRel) {
  const repo = process.env.PUBLIC_GITHUB_REPO_URL;
  if (!repo || repo.includes("YOUR_ORG")) return null;
  const branch = process.env.DOCS_SOURCE_BRANCH ?? "main";
  return `${repo.replace(/\/$/, "")}/blob/${branch}/${sourceRel}`;
}

// ── Decide whether a file is a "module overview" target ─────────────────────
// By default we only process module-root files (mod.rs, lib.rs, main.rs) —
// these carry the folder-layout / overview / public-surface prose that makes
// a good standalone page.  Set EXTRACT_ALL_FILES=1 to also emit per-file
// pages for every .rs file that has a //! block.
function isOverviewFile(relPath) {
  const base = basename(relPath);
  return base === "mod.rs" || base === "lib.rs" || base === "main.rs";
}

// ── Main ────────────────────────────────────────────────────────────────────
async function main() {
  const allFiles = process.env.EXTRACT_ALL_FILES === "1";
  console.log(
    `→ Extracting module overviews from src/**/*.rs (${allFiles ? "all ! files" : "mod.rs / lib.rs / main.rs only"}) …`,
  );

  const files = (await walkRs(RUST_SRC)).filter((f) => {
    if (allFiles) return true;
    return isOverviewFile(relative(RUST_SRC, f));
  });
  if (files.length === 0) {
    console.error("No matching .rs files found under src/");
    process.exit(1);
  }

  // Clean the output directory so stale pages don't linger.
  await rm(OUTPUT_DIR, { recursive: true, force: true });
  await mkdir(OUTPUT_DIR, { recursive: true });

  let count = 0;
  for (const file of files) {
    const content = await readFile(file, "utf-8");
    const lines = content.split("\n");
    const docLines = extractModuleDoc(lines);
    if (docLines.length === 0) continue; // no //! block → skip

    const gates = extractFeatureGates(lines);
    const { slug, content: md } = renderPage(file, docLines, gates);

    const outPath = join(OUTPUT_DIR, `${slug}.md`);
    await mkdir(dirname(outPath), { recursive: true });
    await writeFile(outPath, md);
    count++;
    console.log(`  ✓ ${relative(REPO_ROOT, file)} → ${relative(DOCS_ROOT, outPath)}`);
  }

  console.log(`\nDone — generated ${count} module overview pages under ${relative(DOCS_ROOT, OUTPUT_DIR)}/`);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});