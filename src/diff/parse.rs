//! Pure, side-effect-free parser for unified-diff output (`git diff --no-color`).
//!
//! Walks the raw text and produces a typed [`FileDiff`] tree that the
//! highlighter and every renderer (stack / split / side-by-side / merge) consume.
//! Keeping this pure means it can be unit-tested without spawning git.
//!
//! # Grammar accepted
//!
//! ```text
//! diff --git a/path b/path
//! <file-header lines: index, old/new mode, new file mode, deleted file mode,
//!   similarity, rename from/to, copy from/to, old/new name>
//! --- a/path
//! +++ b/path
//! @@ -old_start,old_count +new_start,new_count @@ <section>
//!  context line
//! +added line
//! -deleted line
//! \ No newline at end of file
//! ```
//!
//! Pairs of `---`/`+++` headers may be absent for binary files; we record those
//! as [`Status::Binary`] with no hunks.

// ─── Types ───────────────────────────────────────────────────────────────────

/// One parsed file in a `git diff` stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileDiff {
    /// Path as shown by git in the `b/` side.  For renames this is the *new* path.
    pub path: String,
    /// Original path for renames/copies; `None` for plain modifications.
    pub old_path: Option<String>,
    /// How this file changed.
    pub status: Status,
    /// Optional four-line git stat, e.g. `3 | 5 +++--`.  Populated lazily by
    /// callers; the parser never sets it.
    pub stat: Option<Stat>,
    /// Hunks belonging to this file, in order.
    pub hunks: Vec<Hunk>,
}

/// What `git diff` says happened to a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Newly added (file did not exist before).
    Added,
    /// In-place edit.
    Modified,
    /// File removed.
    Deleted,
    /// File moved or copied.
    Renamed,
    /// File copied.
    Copied,
    /// Mode change only.
    ModeChange,
    /// Binary file — no textual hunks emitted.
    Binary,
    /// Type change (e.g. symlink → regular file).
    TypeChange,
}

/// Hunk-level diff stat — populated by the caller, not the parser.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stat {
    /// Lines added across all hunks.
    pub added: u32,
    /// Lines deleted across all hunks.
    pub deleted: u32,
}

/// One `@@ -o,os +n,ns @@ …` hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hunk {
    /// 1-based start line in the old file (per unified-diff convention).
    pub old_start: u32,
    /// Number of old-side lines the hunk covers.
    pub old_count: u32,
    /// 1-based start line in the new file.
    pub new_start: u32,
    /// Number of new-side lines the hunk covers.
    pub new_count: u32,
    /// Text after the second `@@` — typically a function/section header.
    pub section: Option<String>,
    /// Lines in this hunk, in order.
    pub lines: Vec<Line>,
}

/// A single line inside a hunk, annotated with its old/new line numbers.
///
/// Line numbers are 1-based and `Some` only on the side(s) the line appears on.
/// The parser tracks these as it walks, incrementing per side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Line {
    /// A line present in both old and new (prefix ` `).
    Context {
        /// Raw text without the leading space prefix.
        text: String,
        /// 1-based line in the old file.
        old_no: u32,
        /// 1-based line in the new file.
        new_no: u32,
    },
    /// A line added on the new side (prefix `+`).
    Add {
        /// Raw text without the leading `+`.
        text: String,
        /// 1-based line in the new file.
        new_no: u32,
    },
    /// A line removed from the old side (prefix `-`).
    Del {
        /// Raw text without the leading `-`.
        text: String,
        /// 1-based line in the old file.
        old_no: u32,
    },
    /// `\ No newline at end of file` marker.  Carries no line numbers; it
    /// annotates the *preceding* line.
    NoNewline,
}

impl Line {
    /// Return the text content of the line, regardless of kind.  Markers
    /// (`+`/`-`/` `) are stripped; `NoNewline` returns the empty string.
    #[must_use]
    pub fn text(&self) -> &str {
        match self {
            Self::Context { text, .. } | Self::Add { text, .. } | Self::Del { text, .. } => text,
            Self::NoNewline => "",
        }
    }

    /// Single-character prefix for re-emitting this line as unified diff: `' '`,
    /// `'+'`, `'-'`, or `'\\'` for `NoNewline`.
    #[must_use]
    pub fn marker(&self) -> char {
        match self {
            Self::Context { .. } => ' ',
            Self::Add { .. } => '+',
            Self::Del { .. } => '-',
            Self::NoNewline => '\\',
        }
    }

    /// Line number on the old (left) side, when applicable.
    #[must_use]
    pub fn old_no(&self) -> Option<u32> {
        match self {
            Self::Context { old_no, .. } | Self::Del { old_no, .. } => Some(*old_no),
            Self::Add { .. } | Self::NoNewline => None,
        }
    }

    /// Line number on the new (right) side, when applicable.
    #[must_use]
    pub fn new_no(&self) -> Option<u32> {
        match self {
            Self::Context { new_no, .. } | Self::Add { new_no, .. } => Some(*new_no),
            Self::Del { .. } | Self::NoNewline => None,
        }
    }
}

impl Hunk {
    /// Compute added/deleted counts by walking lines.  Useful when the caller
    /// wants per-hunk stats independent of git's own summary.
    #[must_use]
    pub fn stat(&self) -> Stat {
        let mut added = 0;
        let mut deleted = 0;
        for line in &self.lines {
            match line {
                Line::Add { .. } => added += 1,
                Line::Del { .. } => deleted += 1,
                _ => {}
            }
        }
        Stat { added, deleted }
    }
}

impl FileDiff {
    /// Total stat across all hunks.  Returns [`Stat::default`] for binary files.
    #[must_use]
    pub fn stat(&self) -> Stat {
        self.hunks
            .iter()
            .map(Hunk::stat)
            .fold(Stat::default(), |a, b| Stat {
                added: a.added + b.added,
                deleted: a.deleted + b.deleted,
            })
    }
}

// ─── Public entry point ──────────────────────────────────────────────────────

/// Parse raw `git diff` output into one [`FileDiff`] per file.
///
/// Pure: takes the full diff text including headers, returns the typed tree.
/// Tolerant — lines that don't match any known header pass through silently
/// (git supports many auxiliary headers we don't model).  Returns an empty
/// vector for input with no `diff --git` markers (e.g. an empty diff).
#[must_use]
pub fn parse(raw: &str) -> Vec<FileDiff> {
    let mut files = Vec::new();
    let mut cur: Option<FileDiff> = None;

    for line in raw.lines() {
        // New file boundary — flush the previous one and start fresh.
        if line.starts_with("diff --git ") {
            if let Some(f) = cur.take() {
                files.push(f);
            }
            cur = Some(FileDiff {
                path: parse_diff_path(line).unwrap_or_default(),
                old_path: None,
                status: Status::Modified,
                stat: None,
                hunks: Vec::new(),
            });
            continue;
        }

        let Some(file) = cur.as_mut() else {
            // Stray line outside any file block — ignore.
            continue;
        };

        // Header lines describing the file change.
        if line.starts_with("new file mode") {
            file.status = Status::Added;
            continue;
        }
        if line.starts_with("deleted file mode") {
            file.status = Status::Deleted;
            continue;
        }
        if let Some(rest) = line.strip_prefix("rename from ") {
            file.old_path = Some(rest.to_string());
            file.status = Status::Renamed;
            continue;
        }
        if let Some(rest) = line.strip_prefix("rename to ") {
            file.path = rest.to_string();
            file.status = Status::Renamed;
            continue;
        }
        if let Some(rest) = line.strip_prefix("copy from ") {
            file.old_path = Some(rest.to_string());
            file.status = Status::Copied;
            continue;
        }
        if let Some(rest) = line.strip_prefix("copy to ") {
            file.path = rest.to_string();
            file.status = Status::Copied;
            continue;
        }
        if line.starts_with("old mode ") || line.starts_with("new mode ") {
            file.status = Status::ModeChange;
            continue;
        }
        if line.starts_with("similarity index") || line.starts_with("dissimilarity index") {
            continue;
        }
        if line.starts_with("index ") {
            continue;
        }
        if line.starts_with("Binary files ") || line.starts_with("GIT binary patch") {
            file.status = Status::Binary;
            continue;
        }
        if line.starts_with("symlink ") || line.contains("type change") {
            file.status = Status::TypeChange;
            continue;
        }

        // --- / +++ headers — only meaningful in the file scope.
        if let Some(rest) = line.strip_prefix("--- ") {
            if file.old_path.is_none() {
                // `--- a/path` (or `i/path` for index-side diffs).  Strip the
                // single-char git prefix when present so we surface the real
                // path.  Keep the old_path only when it differs from the new
                // one — that's how the renderer knows to show `←`.
                if let Some(stripped) = strip_diff_prefix(rest) {
                    if file.path != stripped {
                        file.old_path = Some(stripped.to_string());
                    }
                }
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("+++ ") {
            // `+++ b/path` — needed because renames from a/ have already set path.
            if file.status != Status::Renamed && file.status != Status::Copied {
                file.path = strip_diff_prefix(rest).unwrap_or(rest).to_string();
            }
            continue;
        }

        // Hunk header.
        if line.starts_with("@@ ") {
            if let Some(h) = parse_hunk_header(line) {
                file.hunks.push(h);
            }
            continue;
        }

        // Body lines belong to the current hunk (if any).
        if let Some(hunk) = file.hunks.last_mut() {
            match line.chars().next() {
                Some('+') => {
                    let text = line[1..].to_string();
                    let new_no = hunk.new_start + new_line_offset(hunk);
                    hunk.lines.push(Line::Add { text, new_no });
                }
                Some('-') => {
                    let text = line[1..].to_string();
                    let old_no = hunk.old_start + old_line_offset(hunk);
                    hunk.lines.push(Line::Del { text, old_no });
                }
                Some(' ') => {
                    let text = line[1..].to_string();
                    let old_no = hunk.old_start + old_line_offset(hunk);
                    let new_no = hunk.new_start + new_line_offset(hunk);
                    hunk.lines.push(Line::Context {
                        text,
                        old_no,
                        new_no,
                    });
                }
                Some('\\') => {
                    // `\ No newline at end of file` — annotate the previous line.
                    hunk.lines.push(Line::NoNewline);
                }
                Some(_) => {
                    // Hunk section text already consumed by the header; any
                    // other non-prefixed line inside a hunk body is git noise
                    // (e.g. `--` color separators in `--color=always` output
                    // when the parser was fed colour codes by mistake).
                }
                None => {
                    // empty line — treat as a context line of empty text.
                    let old_no = hunk.old_start + old_line_offset(hunk);
                    let new_no = hunk.new_start + new_line_offset(hunk);
                    hunk.lines.push(Line::Context {
                        text: String::new(),
                        old_no,
                        new_no,
                    });
                }
            }
        }
    }

    if let Some(f) = cur.take() {
        files.push(f);
    }

    files
}

// ─── Header helpers ──────────────────────────────────────────────────────────

/// Extract the right-hand path from a `diff --git a/x b/y` header.
///
/// Returns the path with any git single-char prefix (`a/`, `b/`, `i/`, `w/`)
/// stripped, or `None` if the line does not parse.  For `git diff --no-index
/// a.txt b.txt` the paths have no `a/`/`b/` prefixes and are returned as-is.
fn parse_diff_path(line: &str) -> Option<String> {
    let rest = line.strip_prefix("diff --git ")?;
    // Find the right-side boundary.  Git emits other prefix markers too —
    // `i/` (index) and `w/` (worktree) appear in unstaged `git diff` output.
    // We look for the last " X/" separator where X is a single lowercase git
    // prefix letter, so paths with embedded spaces still split cleanly.
    let right_start = last_diff_split(rest)?;
    let right = &rest[right_start..];
    Some(strip_diff_prefix(right)?.to_string())
}

/// Find the byte offset inside `rest` where the right-hand path begins.
///
/// Searches for the last `" a/"`, `" b/"`, `" i/"`, or `" w/"` separator from
/// the right (preserving paths with embedded spaces) and returns the index of
/// the path itself (after the prefix).  Returns `None` if none are present.
fn last_diff_split(rest: &str) -> Option<usize> {
    let mut best: Option<usize> = None;
    for prefix in [" b/", " a/", " i/", " w/"] {
        if let Some(pos) = rest.rfind(prefix) {
            let candidate = pos + prefix.len();
            match best {
                None => best = Some(candidate),
                Some(b) if candidate > b => best = Some(candidate),
                _ => {}
            }
        }
    }
    best
}

/// Strip a leading git diff prefix (`a/`, `b/`, `i/`, `w/` or others) from `s`.
///
/// Returns `s` unchanged when `/dev/null` (the special "no file" marker) is
/// found, so callers can branch on the empty-side marker explicitly.
fn strip_diff_prefix(s: &str) -> Option<&str> {
    if s == "/dev/null" {
        return Some(s);
    }
    // Single-char git prefix forms: `a/path`, `b/path`, `i/path`, `w/path`.
    if s.len() >= 2 && s.as_bytes()[1] == b'/' && s.as_bytes()[0].is_ascii_alphabetic() {
        return Some(&s[2..]);
    }
    Some(s)
}

/// Parse a `@@ -X,Y +A,B @@ section` header into a [`Hunk`] with empty lines.
///
/// Git may omit `,N` when the count is 1 (e.g. `@@ -1 +1 @@`); we treat the
/// missing value as 1 per the unified-diff spec.
fn parse_hunk_header(line: &str) -> Option<Hunk> {
    let inner = line.strip_prefix("@@ ")?;
    let end = inner.find(" @@")?;
    let spec = &inner[..end];
    let section_trim = inner[end + 3..].trim();
    let section = if section_trim.is_empty() {
        None
    } else {
        Some(section_trim.to_string())
    };

    // Split into the `-X,Y` and `+A,B` parts.
    let plus_pos = spec.find('+')?;
    let old_part = &spec[..plus_pos]; // begins with '-'
    let new_part = &spec[plus_pos..]; // begins with '+'

    let (old_start, old_count) = parse_range(old_part.strip_prefix('-')?)?;
    let (new_start, new_count) = parse_range(new_part.strip_prefix('+')?)?;

    Some(Hunk {
        old_start,
        old_count,
        new_start,
        new_count,
        section,
        lines: Vec::new(),
    })
}

/// Parse `X` or `X,Y` into `(start, count)` where omitted count defaults to 1.
fn parse_range(s: &str) -> Option<(u32, u32)> {
    let s = s.trim();
    if let Some((a, b)) = s.split_once(',') {
        Some((a.parse().ok()?, b.parse().ok()?))
    } else {
        Some((s.parse().ok()?, 1))
    }
}

/// Count the old-side lines already emitted in `hunk`'s body so the next
/// `Del`/`Context` row gets the correct `old_no`.
fn old_line_offset(hunk: &Hunk) -> u32 {
    hunk.lines.iter().filter(|l| l.old_no().is_some()).count() as u32
}

/// Count the new-side lines already emitted so the next `Add`/`Context` row
/// gets the correct `new_no`.
fn new_line_offset(hunk: &Hunk) -> u32 {
    hunk.lines.iter().filter(|l| l.new_no().is_some()).count() as u32
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
diff --git a/src/main.rs b/src/main.rs
index 0123456..789abcde 100644
--- a/src/main.rs
+++ b/src/main.rs
@@ -10,3 +10,5 @@ fn main() {
     let x = 1;
-    let y = 2;
+    let y = 3;
+    let z = 4;
 }
@@ -20,2 +22,2 @@ fn helper() {
-fn helper() {}
+fn helper() {
+    // body
 }

diff --git a/README.md b/README.md
new file mode 100644
index 0000000..abc1234
--- /dev/null
+++ b/README.md
@@ -0,0 +1,2 @@
+# Title
+Body text.
\\ No newline at end of file
";

    #[test]
    fn parses_multiple_files() {
        let files = parse(SAMPLE);
        assert_eq!(files.len(), 2);
        assert_eq!(files[0].path, "src/main.rs");
        assert_eq!(files[0].status, Status::Modified);
        assert_eq!(files[1].path, "README.md");
        assert_eq!(files[1].status, Status::Added);
    }

    #[test]
    fn parses_two_hunks_for_main_rs() {
        let files = parse(SAMPLE);
        let main = &files[0];
        assert_eq!(main.hunks.len(), 2);
        let first = &main.hunks[0];
        assert_eq!(first.old_start, 10);
        assert_eq!(first.old_count, 3);
        assert_eq!(first.new_start, 10);
        assert_eq!(first.new_count, 5);
        assert_eq!(first.section.as_deref(), Some("fn main() {"));
    }

    #[test]
    fn counts_lines_in_first_hunk() {
        let first = &parse(SAMPLE)[0].hunks[0];
        assert_eq!(first.lines.len(), 5); // 3 context + 1 del + 2 add… but we have 1 ctx, 1 del, 2 add, 1 ctx = 5
                                          // Walk expected kinds:
        use Line::*;
        let kinds: Vec<&'static str> = first
            .lines
            .iter()
            .map(|l| match l {
                Context { .. } => "C",
                Add { .. } => "+",
                Del { .. } => "-",
                NoNewline => "\\",
            })
            .collect();
        assert_eq!(kinds, vec!["C", "-", "+", "+", "C"]);
    }

    #[test]
    fn line_numbers_assigned_correctly() {
        let first = &parse(SAMPLE)[0].hunks[0];
        // At old_start=10, new_start=10, with our walking pattern:
        //  Context   old=10 new=10
        //  Del       old=11
        //  Add       new=11
        //  Add       new=12
        //  Context   old=12 new=13
        let ctx0 = &first.lines[0];
        assert!(matches!(
            ctx0,
            Line::Context {
                old_no: 10,
                new_no: 10,
                ..
            }
        ));
        let del = &first.lines[1];
        assert!(matches!(del, Line::Del { old_no: 11, .. }));
        let add1 = &first.lines[2];
        assert!(matches!(add1, Line::Add { new_no: 11, .. }));
        let add2 = &first.lines[3];
        assert!(matches!(add2, Line::Add { new_no: 12, .. }));
        let ctx1 = &first.lines[4];
        assert!(matches!(
            ctx1,
            Line::Context {
                old_no: 12,
                new_no: 13,
                ..
            }
        ));
    }

    #[test]
    fn per_hunk_stat_sums_additions_and_deletions() {
        let main = &parse(SAMPLE)[0];
        // Hunk 0: ctx + del + 2 add + ctx → 2 added, 1 deleted.
        assert_eq!(
            main.hunks[0].stat(),
            Stat {
                added: 2,
                deleted: 1
            }
        );
        // Hunk 1: del + 2 add + ctx → 2 added, 1 deleted.
        assert_eq!(
            main.hunks[1].stat(),
            Stat {
                added: 2,
                deleted: 1
            }
        );
        // File total: 4 added, 2 deleted.
        assert_eq!(
            main.stat(),
            Stat {
                added: 4,
                deleted: 2
            }
        );
    }

    #[test]
    fn handles_new_file_with_dev_null_old_side() {
        let readme = &parse(SAMPLE)[1];
        assert_eq!(readme.hunks.len(), 1);
        let h = &readme.hunks[0];
        assert_eq!(h.old_start, 0);
        assert_eq!(h.new_start, 1);
        // Both body lines are additions; NoNewline is the trailing marker.
        assert_eq!(h.lines.len(), 3);
        assert!(matches!(h.lines[0], Line::Add { new_no: 1, .. }));
        assert!(matches!(h.lines[1], Line::Add { new_no: 2, .. }));
        assert!(matches!(h.lines[2], Line::NoNewline));
    }

    #[test]
    fn empty_diff_yields_empty_vec() {
        assert!(parse("").is_empty());
        assert!(parse("no diff content here\njust text").is_empty());
    }

    #[test]
    fn renamed_file_keeps_both_paths() {
        let raw = "\
diff --git a/old.rs b/new.rs
similarity index 90%
rename from old.rs
rename to new.rs
index 0..1 100644
--- a/old.rs
+++ b/new.rs
@@ -10,3 +10,3 @@
 a
-b
+c
 d
";
        let files = parse(raw);
        assert_eq!(files.len(), 1);
        let f = &files[0];
        assert_eq!(f.path, "new.rs");
        assert_eq!(f.old_path.as_deref(), Some("old.rs"));
        assert_eq!(f.status, Status::Renamed);
    }

    #[test]
    fn binary_file_recorded_with_no_hunks() {
        let raw = "\
diff --git a/data.bin b/data.bin
index 0..1 100644
Binary files a/data.bin and b/data.bin differ
";
        let files = parse(raw);
        assert_eq!(files.len(), 1);
        let f = &files[0];
        assert_eq!(f.status, Status::Binary);
        assert!(f.hunks.is_empty());
    }

    #[test]
    fn deleted_file_status_recognized() {
        let raw = "\
diff --git a/gone.txt b/gone.txt
deleted file mode 100644
index 0..1
--- a/gone.txt
+++ /dev/null
@@ -1,2 +0,0 @@
-line one
-line two
";
        let files = parse(raw);
        assert_eq!(files.len(), 1);
        let f = &files[0];
        assert_eq!(f.status, Status::Deleted);
        assert_eq!(f.hunks.len(), 1);
        assert!(matches!(f.hunks[0].lines[0], Line::Del { old_no: 1, .. }));
        assert!(matches!(f.hunks[0].lines[1], Line::Del { old_no: 2, .. }));
    }

    #[test]
    fn line_text_and_marker_helpers() {
        let ctx = Line::Context {
            text: "x".into(),
            old_no: 1,
            new_no: 1,
        };
        assert_eq!(ctx.text(), "x");
        assert_eq!(ctx.marker(), ' ');
        assert_eq!(ctx.old_no(), Some(1));
        assert_eq!(ctx.new_no(), Some(1));

        let add = Line::Add {
            text: "y".into(),
            new_no: 2,
        };
        assert_eq!(add.text(), "y");
        assert_eq!(add.marker(), '+');
        assert_eq!(add.old_no(), None);
        assert_eq!(add.new_no(), Some(2));

        let del = Line::Del {
            text: "z".into(),
            old_no: 3,
        };
        assert_eq!(del.text(), "z");
        assert_eq!(del.marker(), '-');

        let nl = Line::NoNewline;
        assert_eq!(nl.text(), "");
        assert_eq!(nl.marker(), '\\');
        assert_eq!(nl.old_no(), None);
        assert_eq!(nl.new_no(), None);
    }

    #[test]
    fn hunk_header_without_comma_count_defaults_to_one() {
        let raw = "\
diff --git a/x b/x
--- a/x
+++ b/x
@@ -1 +1 @@
 a
-b
+c
";
        let files = parse(raw);
        let h = &files[0].hunks[0];
        assert_eq!(h.old_count, 1);
        assert_eq!(h.new_count, 1);
    }
}
