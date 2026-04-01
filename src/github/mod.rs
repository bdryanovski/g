//! Minimal GitHub REST API wrapper for stacked-PR operations.
//!
//! ## Tutorial overview
//!
//! This module handles all communication with GitHub's REST API.  It focuses on
//! the operations needed for the "Stacked PRs" workflow:
//!
//! - Detecting the repository owner/name from the `origin` remote URL.
//! - Finding existing open PRs for a branch.
//! - Creating new PRs that target the correct base branch.
//! - Updating the base branch of an existing PR when the stack is reorganised.
//!
//! It uses `ureq` for simple, synchronous (blocking) HTTP requests and
//! `serde_json` for building and parsing JSON payloads.
//!
//! ## Rust concepts used here
//!
//! - `Result<(String, String)>` for returning multiple values on success.
//! - String manipulation (`trim_start_matches`, `splitn`) to parse remote URLs.
//! - A custom `struct` ([`PrInfo`]) for grouping related API response data.
//! - A `loop` with `break` to walk paginated API responses.
//! - The `json!` macro for ergonomic JSON body construction.

use anyhow::{bail, Context, Result};
use std::collections::HashMap;

use crate::commands::git as gitcmd;

// ─── GitHub API types ─────────────────────────────────────────────────────────

/// Lightweight pull-request info returned by stack operations.
///
/// Only the fields needed by the stacked-PR workflow are captured here; the
/// full GitHub PR object has dozens of additional fields.
#[derive(Debug, Clone, PartialEq)]
pub struct PrInfo {
    /// PR number on GitHub (e.g. `42`).
    pub number: u64,
    /// Full web URL for humans (e.g. `https://github.com/owner/repo/pull/42`).
    pub html_url: String,
    /// Name of the base branch the PR targets (e.g. `"main"`).
    pub base_ref: String,
}

// ─── Detect repo from remote ─────────────────────────────────────────────────

/// Detect the GitHub `(owner, repo)` pair from the `origin` remote URL.
///
/// Supports HTTPS (`https://github.com/owner/repo.git`) and SSH
/// (`git@github.com:owner/repo.git`) formats, as well as a best-effort parse
/// for GitHub Enterprise URLs.
///
/// # Errors
///
/// Returns an error if:
/// - There is no `origin` remote configured.
/// - The remote URL cannot be parsed as a GitHub-style URL.
pub fn detect_repo() -> Result<(String, String)> {
    let url = gitcmd::git_output(&["remote", "get-url", "origin"])
        .context("No 'origin' remote found. Add one with: git remote add origin <url>")?;
    parse_github_url(&url)
}

/// Parse a GitHub-style remote URL into `(owner, repo)`.
///
/// Accepts:
/// - HTTPS: `https://github.com/owner/repo.git`
/// - SSH: `git@github.com:owner/repo.git`
/// - GitHub Enterprise HTTPS (best-effort)
///
/// # Errors
///
/// Returns an error if the URL does not match any recognised format.
fn parse_github_url(url: &str) -> Result<(String, String)> {
    let cleaned = url.trim().trim_end_matches(".git");

    if let Some(path) = cleaned.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = path.splitn(2, '/').collect();
        if parts.len() == 2 {
            return Ok((parts[0].to_owned(), parts[1].to_owned()));
        }
    }

    if let Some(path) = cleaned.strip_prefix("git@github.com:") {
        let parts: Vec<&str> = path.splitn(2, '/').collect();
        if parts.len() == 2 {
            return Ok((parts[0].to_owned(), parts[1].to_owned()));
        }
    }

    // GitHub Enterprise HTTPS — best-effort: take the last two path components.
    if cleaned.contains('/') {
        if let Some(pos) = cleaned.rfind('/') {
            let repo = &cleaned[pos + 1..];
            if let Some(owner_start) = cleaned[..pos].rfind('/') {
                let owner = &cleaned[owner_start + 1..pos];
                if !owner.is_empty() && !repo.is_empty() {
                    return Ok((owner.to_owned(), repo.to_owned()));
                }
            }
        }
    }

    bail!(
        "Could not parse GitHub owner/repo from remote URL: {}\n\
         Expected format: https://github.com/owner/repo  or  git@github.com:owner/repo",
        url
    )
}

// ─── API helpers ─────────────────────────────────────────────────────────────

/// Extract a [`PrInfo`] from a GitHub PR JSON object.
///
/// `base_ref_override` is used when we already know the base ref (e.g. when
/// we just created the PR and supplied it ourselves) instead of reading it
/// from the response JSON — avoids an extra field access.
fn pr_info_from_json(pr: &serde_json::Value, base_ref_override: Option<&str>) -> PrInfo {
    PrInfo {
        number: pr["number"].as_u64().unwrap_or(0),
        html_url: pr["html_url"].as_str().unwrap_or_default().to_owned(),
        base_ref: base_ref_override
            .map(str::to_owned)
            .unwrap_or_else(|| pr["base"]["ref"].as_str().unwrap_or_default().to_owned()),
    }
}

/// Convert a `ureq` transport error into an `anyhow::Error` with a consistent message.
///
/// Every API call in this module ends with the same two error arms:
/// ```text
/// Err(ureq::Error::Status(code, _)) => bail!("GitHub API error {}: <context>", code),
/// Err(e)                            => bail!("network error: {}", e),
/// ```
/// This helper centralises both arms so callers only need to call
/// `api_error(e, "context string")` instead of writing them out each time.
///
/// The `create_pr` function has an additional 422-specific arm that is handled
/// separately before this helper would be reached.
fn api_error(e: ureq::Error, context: &str) -> anyhow::Error {
    match e {
        ureq::Error::Status(code, _) => {
            anyhow::anyhow!("GitHub API error {}: {}", code, context)
        }
        other => anyhow::anyhow!("network error: {}", other),
    }
}

/// Build an authenticated GitHub API request with the required headers.
///
/// The returned [`ureq::Request`] has the `Authorization`, `Accept`,
/// `X-GitHub-Api-Version`, and `User-Agent` headers pre-populated.
fn make_request(token: &str, api_base: &str, method: &str, path: &str) -> ureq::Request {
    let url = format!(
        "{}/{}",
        api_base.trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    ureq::request(method, &url)
        .set("Authorization", &format!("Bearer {}", token))
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        // User-Agent uses the binary name so GitHub's logs show the real tool name.
        // The version comes from Cargo.toml via the CARGO_PKG_VERSION env var.
        .set(
            "User-Agent",
            &format!("{}/{}", crate::bin_name(), env!("CARGO_PKG_VERSION")),
        )
}

// ─── PR operations ────────────────────────────────────────────────────────────

/// Find the first open PR whose head branch matches `head_branch`.
///
/// Returns `Ok(Some(pr))` if a matching PR is found, `Ok(None)` if none
/// exists, or an error if the API call fails.
///
/// # Errors
///
/// Returns an error if:
/// - The GitHub API returns a non-2xx status code.
/// - The response body cannot be parsed as JSON.
pub fn find_pr(
    token: &str,
    api_base: &str,
    owner: &str,
    repo: &str,
    head_branch: &str,
) -> Result<Option<PrInfo>> {
    let path = format!("repos/{}/{}/pulls", owner, repo);
    let resp = make_request(token, api_base, "GET", &path)
        .query("state", "open")
        .query("head", &format!("{}:{}", owner, head_branch))
        .call();

    match resp {
        Ok(response) => {
            let prs: Vec<serde_json::Value> = response
                .into_json()
                .context("Failed to parse PR list response")?;

            Ok(prs.first().map(|pr| pr_info_from_json(pr, None)))
        }
        Err(e) => Err(api_error(
            e,
            &format!("could not list PRs for {}", head_branch),
        )),
    }
}

/// Fetch all open PRs for a repository in a single pass, keyed by head branch.
///
/// Pagination is handled automatically; up to 100 PRs are fetched per page.
/// The returned map allows O(1) lookup by branch name when building the stack
/// display.
///
/// # Errors
///
/// Returns an error if:
/// - Any GitHub API page returns a non-2xx status code.
/// - Any response body cannot be parsed as JSON.
pub fn list_open_prs(
    token: &str,
    api_base: &str,
    owner: &str,
    repo: &str,
) -> Result<HashMap<String, PrInfo>> {
    let mut map = HashMap::new();
    let mut page: u32 = 1;

    loop {
        let path = format!("repos/{}/{}/pulls", owner, repo);
        let resp = make_request(token, api_base, "GET", &path)
            .query("state", "open")
            .query("per_page", "100")
            .query("page", &page.to_string())
            .call();

        match resp {
            Ok(response) => {
                let prs: Vec<serde_json::Value> = response
                    .into_json()
                    .context("Failed to parse PR list response")?;

                if prs.is_empty() {
                    break;
                }

                for pr in &prs {
                    if let Some(head_ref) = pr["head"]["ref"].as_str() {
                        map.insert(head_ref.to_owned(), pr_info_from_json(pr, None));
                    }
                }

                if prs.len() < 100 {
                    break;
                }
                page += 1;
            }
            Err(e) => return Err(api_error(e, "could not list open PRs")),
        }
    }

    Ok(map)
}

/// Create a new pull request on GitHub.
// The GitHub API requires all these parameters; a builder pattern would add
// complexity with no simplification benefit for an internal API used in one place.
#[allow(clippy::too_many_arguments)]
///
/// If the API returns HTTP 422 with an "already exists" message, this function
/// falls back to [`find_pr`] and returns the existing PR instead of erroring.
///
/// # Errors
///
/// Returns an error if:
/// - The API returns a non-2xx, non-422 status code.
/// - A 422 error occurs for a reason other than "already exists".
/// - The response body cannot be parsed as JSON.
pub fn create_pr(
    token: &str,
    api_base: &str,
    owner: &str,
    repo: &str,
    title: &str,
    head: &str,
    base: &str,
    draft: bool,
) -> Result<PrInfo> {
    let path = format!("repos/{}/{}/pulls", owner, repo);
    let body = serde_json::json!({
        "title": title,
        "head": head,
        "base": base,
        "draft": draft,
        "body": generate_pr_body(head, base),
    });

    let resp = make_request(token, api_base, "POST", &path).send_json(body);

    match resp {
        Ok(response) => {
            let pr: serde_json::Value = response
                .into_json()
                .context("Failed to parse create PR response")?;
            // Pass `base` as override: we know exactly what base was used.
            Ok(pr_info_from_json(&pr, Some(base)))
        }
        Err(ureq::Error::Status(422, resp)) => {
            let body: serde_json::Value = resp.into_json().unwrap_or_default();
            let msg = body["message"].as_str().unwrap_or("Validation failed");
            // A PR for this branch already exists — fetch and return it.
            if msg.contains("already exists") {
                if let Ok(Some(existing)) = find_pr(token, api_base, owner, repo, head) {
                    return Ok(existing);
                }
            }
            bail!("GitHub API 422: {}", msg);
        }
        Err(ureq::Error::Status(code, resp)) => {
            let body: serde_json::Value = resp.into_json().unwrap_or_default();
            let msg = body["message"].as_str().unwrap_or("unknown error");
            bail!("GitHub API error {}: {}", code, msg);
        }
        Err(e) => bail!("network error: {}", e),
    }
}

/// Update the base branch of an existing pull request.
///
/// This is called when the stack is reorganised and the "parent" branch of a PR
/// changes (e.g. after a `fold` or `squash`).
///
/// # Errors
///
/// Returns an error if:
/// - The GitHub API returns a non-2xx status code.
/// - The response body cannot be parsed as JSON.
pub fn update_pr_base(
    token: &str,
    api_base: &str,
    owner: &str,
    repo: &str,
    pr_number: u64,
    new_base: &str,
) -> Result<PrInfo> {
    let path = format!("repos/{}/{}/pulls/{}", owner, repo, pr_number);
    let body = serde_json::json!({ "base": new_base });

    let resp = make_request(token, api_base, "PATCH", &path).send_json(body);

    match resp {
        Ok(response) => {
            let pr: serde_json::Value = response
                .into_json()
                .context("Failed to parse update PR response")?;
            // `new_base` is authoritative; use pr_number as fallback for the number field.
            let mut info = pr_info_from_json(&pr, Some(new_base));
            if info.number == 0 {
                info.number = pr_number;
            }
            Ok(info)
        }
        Err(e) => Err(api_error(e, &format!("updating PR #{}", pr_number))),
    }
}

// ─── PR body template ─────────────────────────────────────────────────────────

/// Generate a Markdown PR body containing the last 10 commit subjects in the
/// `base..head` range.
fn generate_pr_body(head: &str, base: &str) -> String {
    let commits = gitcmd::git_output_lossy(&[
        "log",
        "--format=- %s",
        "-10",
        &format!("{}..{}", base, head),
    ]);

    let mut body = String::new();
    body.push_str("## Changes\n\n");

    if !commits.is_empty() {
        body.push_str(&commits);
        body.push('\n');
    }

    body
}
