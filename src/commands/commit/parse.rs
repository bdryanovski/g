//! Pure, side-effect-free parsing helpers used by the commit builder.
//!
//! Splitting these out makes them unit-testable without any git, ui, or
//! filesystem dependencies — see the inline `#[cfg(test)] mod tests` block.

use crate::cli::CommitArgs;

/// Extract the body from a commit message — everything after the first blank
/// line.  Returns `None` when there is no body, or when it is empty after
/// trimming.
pub(super) fn extract_body(message: &str) -> Option<String> {
    let lines: Vec<&str> = message.lines().collect();
    if lines.len() < 3 {
        return None;
    }

    // Find the first blank line (separates subject from body).
    let body_start = lines
        .iter()
        .position(|line| line.trim().is_empty())
        .map(|i| i + 1)?;

    let body = lines[body_start..].join("\n").trim().to_string();
    if body.is_empty() {
        None
    } else {
        Some(body)
    }
}

/// Parse an optional conventional commit type and scope from a subject line.
///
/// `"feat(auth): add login"` → `(Some("feat"), Some("auth"))`
/// `"fix: typo"`             → `(Some("fix"), None)`
/// `"random message"`        → `(None, None)`
pub(super) fn parse_conventional_type(subject: &str) -> (Option<String>, Option<String>) {
    // Match `type(scope): ...` or `type: ...`
    let before_colon = match subject.split_once(':') {
        Some((lhs, _)) => lhs,
        None => return (None, None),
    };

    if let Some((t, rest)) = before_colon.split_once('(') {
        let t = t.trim();
        let scope = rest.trim_end_matches(')').trim();
        if !t.is_empty() && t.chars().all(|c| c.is_alphanumeric() || c == '-') {
            return (Some(t.to_string()), Some(scope.to_string()));
        }
    }

    let t = before_colon.trim();
    if !t.is_empty() && t.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return (Some(t.to_string()), None);
    }

    (None, None)
}

/// Build a commit message from the `--message` / `--body` CLI flags, if set.
///
/// Returns `Some(message)` when `--message` is present, combining it with
/// `--body` separated by a blank line when a body is also given.
/// Returns `None` when `--message` is absent, signalling that the interactive
/// prompt flow should be used instead.
pub(super) fn message_from_flags(args: &CommitArgs) -> Option<String> {
    let msg = args.message.as_ref()?;
    let body = args.body.as_deref().unwrap_or_default();
    if body.is_empty() {
        Some(msg.clone())
    } else {
        Some(format!("{}\n\n{}", msg, body))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args_with(message: Option<&str>, body: Option<&str>) -> CommitArgs {
        CommitArgs {
            message: message.map(String::from),
            body: body.map(String::from),
            r#type: None,
            scope: None,
            no_verify: false,
            all: false,
            amend: false,
        }
    }

    // ── extract_body ────────────────────────────────────────────────────────

    #[test]
    fn extract_body_returns_none_for_single_line() {
        assert_eq!(extract_body("subject only"), None);
    }

    #[test]
    fn extract_body_returns_none_for_subject_plus_blank() {
        assert_eq!(extract_body("subject\n"), None);
    }

    #[test]
    fn extract_body_picks_up_text_after_blank_separator() {
        let msg = "subject\n\nthis is the body\nmore body";
        assert_eq!(
            extract_body(msg),
            Some("this is the body\nmore body".to_string())
        );
    }

    #[test]
    fn extract_body_returns_none_when_only_whitespace_after_separator() {
        assert_eq!(extract_body("subject\n\n   \n\t"), None);
    }

    #[test]
    fn extract_body_trims_outer_whitespace_only() {
        // Internal blank lines inside the body are preserved.
        let msg = "subject\n\nfirst para\n\nsecond para\n";
        assert_eq!(
            extract_body(msg),
            Some("first para\n\nsecond para".to_string())
        );
    }

    // ── parse_conventional_type ────────────────────────────────────────────

    #[test]
    fn parse_type_and_scope_from_full_prefix() {
        let (t, s) = parse_conventional_type("feat(auth): add login");
        assert_eq!(t.as_deref(), Some("feat"));
        assert_eq!(s.as_deref(), Some("auth"));
    }

    #[test]
    fn parse_type_only_when_scope_absent() {
        let (t, s) = parse_conventional_type("fix: typo");
        assert_eq!(t.as_deref(), Some("fix"));
        assert_eq!(s, None);
    }

    #[test]
    fn parse_returns_none_for_non_conventional_subject() {
        let (t, s) = parse_conventional_type("random commit message");
        assert_eq!(t, None);
        assert_eq!(s, None);
    }

    #[test]
    fn parse_rejects_types_with_invalid_characters() {
        // Space inside the type token isn't a conventional-commit type.
        let (t, s) = parse_conventional_type("feat with space: stuff");
        assert_eq!(t, None);
        assert_eq!(s, None);
    }

    #[test]
    fn parse_accepts_hyphenated_types() {
        let (t, _) = parse_conventional_type("breaking-change: x");
        assert_eq!(t.as_deref(), Some("breaking-change"));
    }

    // ── message_from_flags ──────────────────────────────────────────────────

    #[test]
    fn message_from_flags_returns_none_without_message() {
        assert_eq!(message_from_flags(&args_with(None, None)), None);
        // Body alone is not enough — message must be present.
        assert_eq!(message_from_flags(&args_with(None, Some("body"))), None);
    }

    #[test]
    fn message_from_flags_returns_subject_only_when_no_body() {
        let m = message_from_flags(&args_with(Some("fix: x"), None));
        assert_eq!(m.as_deref(), Some("fix: x"));
    }

    #[test]
    fn message_from_flags_combines_subject_and_body_with_blank_line() {
        let m = message_from_flags(&args_with(Some("feat: a"), Some("rationale here")));
        assert_eq!(m.as_deref(), Some("feat: a\n\nrationale here"));
    }

    #[test]
    fn message_from_flags_treats_empty_body_as_absent() {
        let m = message_from_flags(&args_with(Some("docs: tweak"), Some("")));
        assert_eq!(m.as_deref(), Some("docs: tweak"));
    }
}
