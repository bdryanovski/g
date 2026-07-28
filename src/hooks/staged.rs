//! Staged file filtering for hooks.
//!
//! Filters staged files by glob patterns to determine which hooks should run
//! and which files to pass to hook commands.

use anyhow::Result;

/// Filter staged files by glob patterns.
///
/// Returns the list of staged files that match any of the given patterns.
/// If patterns is empty, returns all staged files.
pub fn filter_by_patterns(staged_files: &[String], patterns: &[String]) -> Vec<String> {
    if patterns.is_empty() {
        return staged_files.to_vec();
    }

    staged_files
        .iter()
        .filter(|file| matches_any_pattern(file, patterns))
        .cloned()
        .collect()
}

/// Check if a file matches any of the given glob patterns.
fn matches_any_pattern(file: &str, patterns: &[String]) -> bool {
    for pattern in patterns {
        if matches_glob(file, pattern) {
            return true;
        }
    }
    false
}

/// Simple glob matching supporting `*` and `**`.
///
/// - `*` matches any characters except `/`
/// - `**` matches any characters including `/`
/// - `*.rs` matches `foo.rs` but not `src/foo.rs`
/// - `**/*.rs` matches `foo.rs` and `src/foo.rs`
fn matches_glob(file: &str, pattern: &str) -> bool {
    // Handle **/ prefix (match any directory depth)
    if let Some(suffix) = pattern.strip_prefix("**/") {
        // Match the suffix at any depth
        if matches_simple_glob(file, suffix) {
            return true;
        }
        // Also try matching against each path component
        let parts: Vec<&str> = file.split('/').collect();
        for i in 0..parts.len() {
            let subpath = parts[i..].join("/");
            if matches_simple_glob(&subpath, suffix) {
                return true;
            }
        }
        return false;
    }

    // Handle /** suffix (match anything under this path)
    if let Some(prefix) = pattern.strip_suffix("/**") {
        return file.starts_with(prefix) || file.starts_with(&format!("{}/", prefix));
    }

    // Simple glob match
    matches_simple_glob(file, pattern)
}

/// Simple glob matching for patterns without `**/`.
fn matches_simple_glob(file: &str, pattern: &str) -> bool {
    // Convert glob to regex-like matching
    let mut file_chars = file.chars().peekable();
    let mut pattern_chars = pattern.chars().peekable();

    while let Some(p) = pattern_chars.next() {
        match p {
            '*' => {
                // Check for ** (shouldn't happen here but handle anyway)
                if pattern_chars.peek() == Some(&'*') {
                    pattern_chars.next();
                    // ** matches everything
                    let rest: String = pattern_chars.collect();
                    if rest.is_empty() {
                        return true;
                    }
                    // Try matching rest at every position
                    let remaining: String = file_chars.collect();
                    for i in 0..=remaining.len() {
                        if matches_simple_glob(&remaining[i..], &rest) {
                            return true;
                        }
                    }
                    return false;
                }

                // * matches any non-/ characters
                let next_pattern: Option<char> = pattern_chars.peek().copied();

                // Consume file chars until we hit / or match next pattern char
                loop {
                    match file_chars.peek() {
                        None => break,
                        Some('/') => break,
                        Some(&c) if Some(c) == next_pattern => break,
                        _ => {
                            file_chars.next();
                        }
                    }
                }
            }
            '?' => {
                // ? matches any single character except /
                match file_chars.next() {
                    Some('/') | None => return false,
                    _ => {}
                }
            }
            c => {
                // Literal character match
                if file_chars.next() != Some(c) {
                    return false;
                }
            }
        }
    }

    // Both must be exhausted for a match
    file_chars.next().is_none()
}

/// Get the list of currently staged files from git.
pub fn get_staged_files() -> Result<Vec<String>> {
    let output = crate::commands::git::git_output(&["diff", "--cached", "--name-only"])?;
    Ok(output
        .lines()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_extension_match() {
        assert!(matches_glob("foo.rs", "*.rs"));
        assert!(matches_glob("bar.rs", "*.rs"));
        assert!(!matches_glob("foo.ts", "*.rs"));
        assert!(!matches_glob("src/foo.rs", "*.rs")); // * doesn't match /
    }

    #[test]
    fn test_double_star_prefix() {
        assert!(matches_glob("foo.rs", "**/*.rs"));
        assert!(matches_glob("src/foo.rs", "**/*.rs"));
        assert!(matches_glob("src/nested/foo.rs", "**/*.rs"));
        assert!(!matches_glob("foo.ts", "**/*.rs"));
    }

    #[test]
    fn test_double_star_suffix() {
        assert!(matches_glob("src/foo.rs", "src/**"));
        assert!(matches_glob("src/nested/foo.rs", "src/**"));
        assert!(!matches_glob("other/foo.rs", "src/**"));
    }

    #[test]
    fn test_directory_match() {
        assert!(matches_glob("src/foo.rs", "src/*.rs"));
        assert!(!matches_glob("src/nested/foo.rs", "src/*.rs"));
    }

    #[test]
    fn test_filter_by_patterns() {
        let staged = vec![
            "src/main.rs".to_string(),
            "src/lib.rs".to_string(),
            "tests/test.rs".to_string(),
            "package.json".to_string(),
            "README.md".to_string(),
        ];

        let patterns = vec!["**/*.rs".to_string()];
        let filtered = filter_by_patterns(&staged, &patterns);
        assert_eq!(filtered.len(), 3);
        assert!(filtered.contains(&"src/main.rs".to_string()));
        assert!(filtered.contains(&"src/lib.rs".to_string()));
        assert!(filtered.contains(&"tests/test.rs".to_string()));
    }

    #[test]
    fn test_empty_patterns_returns_all() {
        let staged = vec!["foo.rs".to_string(), "bar.ts".to_string()];
        let filtered = filter_by_patterns(&staged, &[]);
        assert_eq!(filtered, staged);
    }
}
