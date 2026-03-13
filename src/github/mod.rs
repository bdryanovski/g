use anyhow::{bail, Context, Result};

use crate::commands::git as gitcmd;

// ─── GitHub API Types ─────────────────────────────────────────────────────────

/// Lightweight PR info used by stack operations
pub struct PrInfo {
    pub number: u64,
    pub html_url: String,
    pub base_ref: String,
}

// ─── Detect Repo From Remote ─────────────────────────────────────────────────

/// Detect owner/repo from origin remote URL
pub fn detect_repo() -> Result<(String, String)> {
    let url = gitcmd::git_output(&["remote", "get-url", "origin"])
        .context("No 'origin' remote found. Add one with: git remote add origin <url>")?;
    parse_github_url(&url)
}

fn parse_github_url(url: &str) -> Result<(String, String)> {
    // HTTPS: https://github.com/owner/repo.git
    // SSH:   git@github.com:owner/repo.git
    let cleaned = url
        .trim()
        .trim_end_matches(".git");

    if let Some(path) = cleaned.strip_prefix("https://github.com/") {
        let parts: Vec<&str> = path.splitn(2, '/').collect();
        if parts.len() == 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
    }

    if let Some(path) = cleaned.strip_prefix("git@github.com:") {
        let parts: Vec<&str> = path.splitn(2, '/').collect();
        if parts.len() == 2 {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
    }

    // GitHub Enterprise HTTPS
    if cleaned.contains('/') {
        if let Some(pos) = cleaned.rfind('/') {
            let repo = &cleaned[pos + 1..];
            if let Some(owner_start) = cleaned[..pos].rfind('/') {
                let owner = &cleaned[owner_start + 1..pos];
                if !owner.is_empty() && !repo.is_empty() {
                    return Ok((owner.to_string(), repo.to_string()));
                }
            }
        }
    }

    bail!(
        "Could not parse GitHub owner/repo from remote URL: {}\nExpected format: https://github.com/owner/repo or git@github.com:owner/repo",
        url
    )
}

// ─── API Helpers ─────────────────────────────────────────────────────────────

fn make_request(token: &str, api_base: &str, method: &str, path: &str) -> ureq::Request {
    let url = format!("{}/{}", api_base.trim_end_matches('/'), path.trim_start_matches('/'));
    ureq::request(method, &url)
        .set("Authorization", &format!("Bearer {}", token))
        .set("Accept", "application/vnd.github+json")
        .set("X-GitHub-Api-Version", "2022-11-28")
        .set("User-Agent", "vcli/0.1")
}

// ─── PR Operations ────────────────────────────────────────────────────────────

/// Find an existing open PR for a given head branch
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

            if let Some(pr) = prs.first() {
                Ok(Some(PrInfo {
                    number: pr["number"].as_u64().unwrap_or(0),
                    html_url: pr["html_url"].as_str().unwrap_or("").to_string(),
                    base_ref: pr["base"]["ref"].as_str().unwrap_or("").to_string(),
                }))
            } else {
                Ok(None)
            }
        }
        Err(ureq::Error::Status(code, _)) => {
            bail!("GitHub API error {}: could not list PRs for {}", code, head_branch);
        }
        Err(e) => bail!("Network error: {}", e),
    }
}

/// Create a new pull request
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

    let resp = make_request(token, api_base, "POST", &path)
        .send_json(body);

    match resp {
        Ok(response) => {
            let pr: serde_json::Value = response
                .into_json()
                .context("Failed to parse create PR response")?;
            Ok(PrInfo {
                number: pr["number"].as_u64().unwrap_or(0),
                html_url: pr["html_url"].as_str().unwrap_or("").to_string(),
                base_ref: base.to_string(),
            })
        }
        Err(ureq::Error::Status(422, resp)) => {
            let body: serde_json::Value = resp.into_json().unwrap_or_default();
            let msg = body["message"].as_str().unwrap_or("Validation failed");
            // PR might already exist
            if msg.contains("already exists") {
                // Try to find it
                if let Ok(Some(existing)) = find_pr(token, api_base, owner, repo, head) {
                    return Ok(existing);
                }
            }
            bail!("GitHub API 422: {}", msg);
        }
        Err(ureq::Error::Status(code, resp)) => {
            let body: serde_json::Value = resp.into_json().unwrap_or_default();
            let msg = body["message"].as_str().unwrap_or("Unknown error");
            bail!("GitHub API error {}: {}", code, msg);
        }
        Err(e) => bail!("Network error: {}", e),
    }
}

/// Update the base branch of an existing PR
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

    let resp = make_request(token, api_base, "PATCH", &path)
        .send_json(body);

    match resp {
        Ok(response) => {
            let pr: serde_json::Value = response
                .into_json()
                .context("Failed to parse update PR response")?;
            Ok(PrInfo {
                number: pr["number"].as_u64().unwrap_or(pr_number),
                html_url: pr["html_url"].as_str().unwrap_or("").to_string(),
                base_ref: new_base.to_string(),
            })
        }
        Err(ureq::Error::Status(code, _)) => {
            bail!("GitHub API error {} updating PR #{}", code, pr_number);
        }
        Err(e) => bail!("Network error: {}", e),
    }
}

// ─── PR Body Template ─────────────────────────────────────────────────────────

fn generate_pr_body(head: &str, base: &str) -> String {
    // Get recent commits for the PR body
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

    body.push_str("\n---\n");
    body.push_str("*Created with [vcli](https://github.com/your-org/vcli) — stacked PR workflow*\n");
    body
}
