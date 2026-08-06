import { defineCollection, z } from "astro:content";

// Single content collection covers all three documentation layers:
//
//   section "guides"   — hand-written Markdown pages in docs/src/content/docs/
//   section "modules"  — generated from //! comments (build artefact, git-ignored)
//
// The rustdoc API reference (Layer 1) is served from docs/public/api/ as
// static HTML and linked via the /api-reference/ page — it is NOT part of
// this content collection.
const docs = defineCollection({
  type: "content",
  schema: z.object({
    title: z.string(),
    description: z.string().optional(),
    order: z.number().optional(),
    // Which nav section the page belongs to.  Defaults to "guides".
    section: z.enum(["guides", "modules"]).default("guides"),
    // True for machine-generated pages — the layout renders a "generated"
    // banner and the .gitignore rules these out of version control.
    generated: z.boolean().default(false),
    // Path to the source .rs file, for generated module pages.
    source: z.string().optional(),
  }),
});

export const collections = { docs };
