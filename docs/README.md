# g documentation site

Standalone **Astro** + **React** site for the `g` Git CLI. It lives under `docs/` and is **not** part of the Rust crate (see root `Cargo.toml` `exclude`).

## Develop

```bash
cd docs
npm install
npm run dev
```

Visit `http://localhost:4321`.

## Edit documentation

Markdown sources are in `src/content/docs/`. Each file has frontmatter:

```yaml
---
title: Page title
description: Short summary for meta / nav
order: 0
---
```

Lower `order` values appear earlier in the sidebar. Add new `.md` files there; Astro regenerates routes under `/docs/<slug>/`.

The docs chrome groups pages under **Start** / **Workflows** / **Reference** in `src/layouts/DocsLayout.astro` (update the `groups` array when you add pages). Playbooks live in `use-cases.md` and in the “When teams reach for g” strip on every doc page.

## GitHub link in the header

Set at build time:

```bash
export PUBLIC_GITHUB_REPO_URL=https://github.com/you/vcli
npm run build
```

In GitHub Actions this repository URL is passed automatically (see workflow).

## Build

```bash
npm run build
```

Output: `docs/dist/`.

## GitHub Pages

The workflow `.github/workflows/docs.yml` builds on push to `main` and deploys with **GitHub Pages** (artifact + `deploy-pages`).

Enable Pages in the repository settings: **Build and deployment → Source: GitHub Actions**.

For a project site, `astro.config.mjs` sets `base` to `/<repository>/` on CI so assets resolve correctly.
