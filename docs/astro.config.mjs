import { defineConfig } from "astro/config";
import react from "@astrojs/react";

const owner = process.env.GITHUB_REPOSITORY_OWNER ?? "";
const repo = process.env.GITHUB_REPOSITORY?.split("/")[1] ?? "";
const onGithubActions = process.env.GITHUB_ACTIONS === "true";
const base = onGithubActions && repo ? `/${repo}/` : "/";
const site =
  onGithubActions && owner
    ? `https://${owner}.github.io`
    : undefined;

// https://astro.build/config
export default defineConfig({
  site,
  base,
  trailingSlash: "always",
  integrations: [react()],
});
