// SPDX-License-Identifier: AGPL-3.0-or-later
//! Recursive file-name search used by the explorer.
//!
//! The walker is deliberately conservative and dependency-free:
//!
//! - **Symlinked directories are never followed.** A symlink can appear in the
//!   results as a matching entry, but the walk does not descend into it, so a
//!   link loop cannot hang the search.
//! - Unreadable directories/files are skipped instead of failing the whole
//!   search.
//! - Results are capped (`max_results`) and the walk depth is limited
//!   (`max_depth`), keeping a search in a huge tree bounded.
//! - The caller passes a cancellation flag checked between directories and
//!   entries.
//!
//! This module is GPUI-free so it can be unit tested headlessly.

use std::borrow::Cow;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;

use crate::model::{entry_for_path, extension_of, is_hidden_name, Entry};

/// Kind filter applied to search results without re-running the search.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchKind {
    #[default]
    All,
    Files,
    Dirs,
}

impl SearchKind {
    pub fn matches(self, entry: &Entry) -> bool {
        match self {
            SearchKind::All => true,
            SearchKind::Files => !entry.is_dir,
            SearchKind::Dirs => entry.is_dir,
        }
    }
}

/// Filter already-collected search results by kind, optional extension and
/// optional name substring. The extension is case-insensitive and may be given
/// with or without a leading dot (`rs`, `.rs`); an empty extension means "no
/// extension filter". The name filter is a case-insensitive substring match.
pub fn filter_results(
    entries: &[Entry],
    kind: SearchKind,
    extension: Option<&str>,
    name: Option<&str>,
) -> Vec<Entry> {
    let extension = extension
        .map(|value| value.trim().trim_start_matches('.').to_lowercase())
        .filter(|value| !value.is_empty());
    let name = name
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());
    entries
        .iter()
        .filter(|entry| kind.matches(entry))
        .filter(|entry| match &extension {
            // Use the same rule as `kind_label`: a leading dot with no stem
            // (e.g. `.rs`) is not an extension. `extension_of` is lowercase.
            Some(extension) => extension_of(&entry.name_text) == *extension,
            None => true,
        })
        .filter(|entry| match &name {
            Some(name) => entry.name_text.to_lowercase().contains(name),
            None => true,
        })
        .cloned()
        .collect()
}

/// Limits and filtering options for [`search_by_name`].
#[derive(Clone, Copy, Debug)]
pub struct SearchOptions {
    pub max_results: usize,
    pub max_depth: usize,
    pub include_hidden: bool,
    /// Number of leading matches to skip. Used when a multi-root search
    /// redistributes its unused per-root budget and re-walks a capped root
    /// past the results it already collected (`max_results` still applies to
    /// the *returned* matches).
    pub skip: usize,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_results: 2_000,
            max_depth: 24,
            include_hidden: false,
            skip: 0,
        }
    }
}

/// A search hit with its optional content-match line number. Kept separate
/// from [`Entry`] so the filesystem model does not carry a view-only concern.
#[derive(Clone, Debug)]
pub struct SearchResult {
    pub entry: Entry,
    /// 1-based line number of the first content match (content search only).
    pub match_line: Option<u32>,
}

/// Normalize a user query for case-insensitive substring matching.
///
/// Surrounding whitespace is trimmed and the value is lowercased so that the
/// filesystem walker and the in-memory wasm search share exactly one rule.
/// Returns `None` for an empty or whitespace-only query, which must match no
/// entry instead of becoming a literal needle such as `"  "`.
pub fn normalized_query(query: &str) -> Option<String> {
    let needle = query.trim().to_lowercase();
    (!needle.is_empty()).then_some(needle)
}

/// Whether an entry name contains an already-normalized needle.
///
/// `needle` must come from [`normalized_query`]; callers must check that it is
/// `Some` before matching so an empty query never turns into a literal match.
pub fn name_matches(name: &str, needle: &str) -> bool {
    name.to_lowercase().contains(needle)
}

/// Search `root` recursively for entries whose file name contains `query`
/// (case-insensitive). Returns matching entries sorted by path.
pub fn search_by_name(
    root: &Path,
    query: &str,
    options: SearchOptions,
    cancel: &AtomicBool,
) -> Vec<SearchResult> {
    let Some(needle) = normalized_query(query) else {
        return Vec::new();
    };

    let mut matches: Vec<SearchResult> = Vec::new();
    let mut skipped = 0usize;
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];

    while let Some((dir, depth)) = stack.pop() {
        if cancel.load(Ordering::SeqCst) || matches.len() >= options.max_results {
            break;
        }
        let Ok(reader) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = reader
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .collect();
        children.sort();

        for path in children {
            if cancel.load(Ordering::SeqCst) || matches.len() >= options.max_results {
                break;
            }
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            let hidden = is_hidden_name(&name);
            if hidden && !options.include_hidden {
                continue;
            }
            let is_dir_without_following = std::fs::symlink_metadata(&path)
                .map(|metadata| metadata.is_dir())
                .unwrap_or(false);
            if name_matches(&name, &needle) {
                if let Some(entry) = entry_for_path(&path) {
                    if skipped < options.skip {
                        skipped += 1;
                    } else {
                        matches.push(SearchResult {
                            entry,
                            match_line: None,
                        });
                        if matches.len() >= options.max_results {
                            break;
                        }
                    }
                }
            }
            if is_dir_without_following && depth < options.max_depth {
                stack.push((path, depth + 1));
            }
        }
    }

    matches.sort_by(|left, right| left.entry.path.cmp(&right.entry.path));
    matches
}

/// Search mode: match file names or file contents.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchMode {
    #[default]
    Name,
    Content,
}

/// Where to search from when a query is entered.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SearchScope {
    /// Search only the current directory.
    #[default]
    Current,
    /// Search every open tab's directory.
    OpenTabs,
    /// Search every bookmarked directory.
    Bookmarks,
}

impl SearchScope {
    pub fn label(self) -> &'static str {
        match self {
            SearchScope::Current => "現在のフォルダ",
            SearchScope::OpenTabs => "開いているタブ",
            SearchScope::Bookmarks => "ブックマーク",
        }
    }
}

/// Resolved search roots for a scope, including whether the search fell back
/// to the current directory because no valid scoped roots were available.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchRoots {
    pub roots: Vec<PathBuf>,
    pub fell_back_to_cwd: bool,
}

/// Collect unique existing directory roots for a scope.
pub fn search_roots(
    scope: SearchScope,
    cwd: &Path,
    tabs: &[PathBuf],
    bookmarks: &[PathBuf],
) -> SearchRoots {
    use std::collections::HashSet;
    let candidates: Vec<PathBuf> = match scope {
        SearchScope::Current => vec![cwd.to_path_buf()],
        SearchScope::OpenTabs => tabs.to_vec(),
        SearchScope::Bookmarks => bookmarks.to_vec(),
    };
    let mut seen = HashSet::new();
    let mut roots = Vec::new();
    for path in candidates {
        if !path.is_dir() {
            continue;
        }
        // canonicalize best-effort for dedupe; fall back to original path
        let key = path.canonicalize().unwrap_or_else(|_| path.clone());
        if seen.insert(key) {
            roots.push(path);
        }
    }
    if roots.is_empty() {
        // Keep a safe fallback so a search still runs somewhere meaningful.
        SearchRoots {
            roots: vec![cwd.to_path_buf()],
            fell_back_to_cwd: scope != SearchScope::Current,
        }
    } else {
        SearchRoots {
            roots,
            fell_back_to_cwd: false,
        }
    }
}

/// Merge per-root search results into one list, de-duplicated by path and
/// capped globally. Input batches are assumed to be sorted by path.
pub fn merge_search_results(
    batches: Vec<Vec<SearchResult>>,
    max_total: usize,
) -> Vec<SearchResult> {
    use std::collections::HashSet;
    let mut seen = HashSet::new();
    let mut merged = Vec::new();
    for batch in batches {
        for result in batch {
            if merged.len() >= max_total {
                break;
            }
            if seen.insert(result.entry.path.clone()) {
                merged.push(result);
            }
        }
    }
    merged.sort_by(|left, right| left.entry.path.cmp(&right.entry.path));
    merged
}

/// Limits for [`search_contents`].
#[derive(Clone, Debug)]
pub struct ContentSearchOptions {
    pub max_results: usize,
    pub max_depth: usize,
    pub include_hidden: bool,
    /// Files larger than this are skipped (no partial reads).
    pub max_file_size: u64,
    /// Number of leading matches to skip (see [`SearchOptions::skip`]).
    pub skip: usize,
    /// Files skipped because they exceed `max_file_size`. Shared with the
    /// caller so the search summary can explain an incomplete result set.
    pub skipped_large_files: Arc<AtomicUsize>,
    /// Files that could not be searched exactly because they are not valid
    /// UTF-8 (e.g. CP932). Shared with the caller for the same reason.
    pub non_utf8_files: Arc<AtomicUsize>,
}

impl Default for ContentSearchOptions {
    fn default() -> Self {
        Self {
            max_results: 500,
            max_depth: 24,
            include_hidden: false,
            max_file_size: 1024 * 1024,
            skip: 0,
            skipped_large_files: Arc::new(AtomicUsize::new(0)),
            non_utf8_files: Arc::new(AtomicUsize::new(0)),
        }
    }
}

/// Identity of the inode inspected by `symlink_metadata`. A path swapped for
/// a symlink between that check and `File::open` resolves to a different inode
/// and must be discarded, never read (BA13). `std` only, Unix `MetadataExt`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct FileIdentity {
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
}

impl FileIdentity {
    fn of(metadata: &std::fs::Metadata) -> Self {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Self {
                dev: metadata.dev(),
                ino: metadata.ino(),
            }
        }
        #[cfg(not(unix))]
        {
            let _ = metadata;
            Self {}
        }
    }
}

// Headless seam used by tests to simulate a swap between the metadata check
// and the open; `#[cfg(test)]` only, thread-local so parallel tests cannot
// influence each other.
#[cfg(test)]
thread_local! {
    static SWAP_OPEN_WITH: std::cell::RefCell<Option<PathBuf>> = const {
        std::cell::RefCell::new(None)
    };
}

fn open_content_file(path: &Path) -> std::io::Result<std::fs::File> {
    #[cfg(test)]
    if let Some(swap) = SWAP_OPEN_WITH.with(|cell| cell.borrow().clone()) {
        return std::fs::File::open(swap);
    }
    std::fs::File::open(path)
}

/// Return the 1-based line number of the first line containing `needle`
/// (case-insensitive), or `None` when the file has no match or is binary.
///
/// The whole bounded read range is checked first: a NUL byte anywhere marks
/// the file as binary. Lines are split on `\n`, on `\r`, and on `\r\n` (a
/// single separator), so CR-only files get correct logical line numbers.
/// A line that is not valid UTF-8 also marks the file as binary — without
/// this check `from_utf8_lossy` would replace e.g. CP932 bytes with U+FFFD
/// and could create a false match. The one exception is the final segment
/// when the read stopped exactly at `max_bytes`: a multi-byte character may
/// have been cut by that boundary, so lossy decoding is allowed there only.
#[cfg(test)]
fn first_matching_line_in_file(
    path: &Path,
    needle: &str,
    max_bytes: u64,
    cancel: &AtomicBool,
) -> std::io::Result<Option<u32>> {
    // Standalone callers take their own identity right before opening.
    let identity = FileIdentity::of(&std::fs::symlink_metadata(path)?);
    let non_utf8 = AtomicUsize::new(0);
    first_matching_line_in_file_checked(path, needle, max_bytes, cancel, identity, &non_utf8)
}

/// Decode one logical line. Invalid UTF-8 is rejected unless the caller
/// allows lossy decoding (only true for a final segment cut by `max_bytes`).
fn line_text(line: &[u8], allow_lossy: bool) -> Option<Cow<'_, str>> {
    match std::str::from_utf8(line) {
        Ok(text) => Some(Cow::Borrowed(text)),
        Err(_) if allow_lossy => Some(String::from_utf8_lossy(line)),
        Err(_) => None,
    }
}

/// Same as [`first_matching_line_in_file`], but with the identity captured by
/// the walker's `symlink_metadata` check, so a post-check swap is detectable.
/// `non_utf8` is incremented once per file rejected for invalid UTF-8.
fn first_matching_line_in_file_checked(
    path: &Path,
    needle: &str,
    max_bytes: u64,
    cancel: &AtomicBool,
    expected: FileIdentity,
    non_utf8: &AtomicUsize,
) -> std::io::Result<Option<u32>> {
    use std::io::Read;

    if cancel.load(Ordering::SeqCst) {
        return Ok(None);
    }
    let file = open_content_file(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        // `File::metadata` is fstat on the *opened* descriptor, so this sees
        // the replacement even if the path was swapped after our check.
        let opened = file.metadata()?;
        if opened.dev() != expected.dev || opened.ino() != expected.ino {
            return Ok(None);
        }
    }
    #[cfg(not(unix))]
    let _ = expected;

    // The read range is bounded by `max_bytes` (the caller's max_file_size),
    // so materializing it is cheap and lets us split on \r as well as \n.
    let mut bytes = Vec::new();
    // BD07: read one extra byte so a file whose exact size equals max_bytes is
    // not mistaken for a truncated read. Only an actual overrun permits lossy
    // decoding of the final segment.
    file.take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let cut_at_limit = bytes.len() as u64 > max_bytes;
    if cut_at_limit {
        bytes.truncate(max_bytes as usize);
    }
    if bytes.contains(&0) {
        return Ok(None); // binary file
    }

    let mut first_match = None;
    let mut line_number = 0u32;
    let mut start = 0usize;
    let len = bytes.len();
    let mut i = 0usize;
    while i < len {
        let byte = bytes[i];
        if byte != b'\n' && byte != b'\r' {
            i += 1;
            continue;
        }
        line_number += 1;
        let line = &bytes[start..i];
        // `\r\n` is one separator, not two.
        if byte == b'\r' && i + 1 < len && bytes[i + 1] == b'\n' {
            i += 1;
        }
        if cancel.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let Some(text) = line_text(line, false) else {
            non_utf8.fetch_add(1, Ordering::SeqCst);
            return Ok(None);
        };
        if first_match.is_none() && text.to_lowercase().contains(needle) {
            first_match = Some(line_number);
        }
        start = i + 1;
        i += 1;
    }
    if start < len {
        line_number += 1;
        if cancel.load(Ordering::SeqCst) {
            return Ok(None);
        }
        let line = &bytes[start..len];
        let Some(text) = line_text(line, cut_at_limit) else {
            non_utf8.fetch_add(1, Ordering::SeqCst);
            return Ok(None);
        };
        if first_match.is_none() && text.to_lowercase().contains(needle) {
            first_match = Some(line_number);
        }
    }
    Ok(first_match)
}

/// Recursively search file **contents** for `query` (case-insensitive).
///
/// Safety/conservatism:
/// - symlinks are skipped entirely (no reading through links),
/// - files larger than `max_file_size` are skipped,
/// - files containing a NUL byte anywhere in the read range are binary, as are
///   files whose text is not valid UTF-8 (both are counted for the summary),
/// - hidden entries follow `include_hidden`, unreadable entries are skipped,
/// - reads are bounded by `max_file_size`, split on LF/CR/CRLF, and the whole
///   read range is checked for NUL before the first match is reported,
/// - results and depth are capped; cancellation is checked per entry and line.
pub fn search_contents(
    root: &Path,
    query: &str,
    options: ContentSearchOptions,
    cancel: &AtomicBool,
) -> Vec<SearchResult> {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }

    let mut matches: Vec<SearchResult> = Vec::new();
    let mut skipped = 0usize;
    let mut stack: Vec<(PathBuf, usize)> = vec![(root.to_path_buf(), 0)];

    while let Some((dir, depth)) = stack.pop() {
        if cancel.load(Ordering::SeqCst) || matches.len() >= options.max_results {
            break;
        }
        let Ok(reader) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut children: Vec<PathBuf> = reader
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.path())
            .collect();
        children.sort();

        for path in children {
            if cancel.load(Ordering::SeqCst) || matches.len() >= options.max_results {
                break;
            }
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            if name.is_empty() {
                continue;
            }
            if is_hidden_name(&name) && !options.include_hidden {
                continue;
            }
            let Ok(link_metadata) = std::fs::symlink_metadata(&path) else {
                continue;
            };
            let is_symlink = link_metadata.file_type().is_symlink();
            if is_symlink {
                // Never read through symlinks in content search.
                continue;
            }
            if link_metadata.is_dir() {
                if depth < options.max_depth {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            if !link_metadata.is_file() {
                continue;
            }
            if link_metadata.len() > options.max_file_size {
                options.skipped_large_files.fetch_add(1, Ordering::SeqCst);
                continue;
            }
            // Reuse the identity from the symlink_metadata check above: if the
            // path is replaced before the open, the reader discards it instead
            // of reading outside the search root (BA13).
            let identity = FileIdentity::of(&link_metadata);
            if let Ok(Some(line)) = first_matching_line_in_file_checked(
                &path,
                &needle,
                options.max_file_size,
                cancel,
                identity,
                &options.non_utf8_files,
            ) {
                if let Some(entry) = entry_for_path(&path) {
                    if skipped < options.skip {
                        skipped += 1;
                    } else {
                        matches.push(SearchResult {
                            entry,
                            match_line: Some(line),
                        });
                        if matches.len() >= options.max_results {
                            break;
                        }
                    }
                }
            }
        }
    }

    matches.sort_by(|left, right| left.entry.path.cmp(&right.entry.path));
    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-search-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn normalized_query_handles_empty_whitespace_and_case() {
        assert_eq!(normalized_query(""), None);
        assert_eq!(normalized_query("   "), None);
        assert_eq!(normalized_query(" \t\n\r "), None);
        assert_eq!(normalized_query("  Foo  ").as_deref(), Some("foo"));
        assert_eq!(normalized_query("README.md").as_deref(), Some("readme.md"));
        assert_eq!(
            normalized_query("  MIXED Case  ").as_deref(),
            Some("mixed case")
        );
    }

    #[test]
    fn name_matches_is_case_insensitive() {
        assert!(name_matches("README.md", "readme"));
        assert!(name_matches("notes.txt", "TXT".to_lowercase().as_str()));
        assert!(!name_matches("notes.txt", "readme"));
        // An empty needle would match everything, so callers must filter the
        // query through `normalized_query` before reaching this helper.
        assert!(normalized_query("   ").is_none());
    }

    #[test]
    fn search_by_name_uses_same_normalization_as_helper() {
        let dir = scratch("normalize");
        write(&dir.join("MixedCase.TXT"));
        let cancel = AtomicBool::new(false);
        // Leading/trailing whitespace is trimmed, case is folded.
        assert_eq!(
            names(&search_by_name(
                &dir,
                "  mixedcase  ",
                SearchOptions::default(),
                &cancel
            )),
            vec!["MixedCase.TXT"]
        );
        assert!(search_by_name(&dir, "\t  ", SearchOptions::default(), &cancel).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn scope_labels_and_empty_query() {
        assert_eq!(SearchScope::Current.label(), "現在のフォルダ");
        assert_eq!(SearchScope::OpenTabs.label(), "開いているタブ");
        assert_eq!(SearchScope::Bookmarks.label(), "ブックマーク");

        let dir = scratch("empty-query");
        let cancel = AtomicBool::new(false);
        assert!(search_by_name(&dir, "", SearchOptions::default(), &cancel).is_empty());
        assert!(search_contents(&dir, "", ContentSearchOptions::default(), &cancel).is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn whitespace_only_query_is_not_a_literal_needle() {
        let dir = scratch("whitespace-query");
        // Both haystacks really contain two consecutive spaces: without
        // trimming, `"  "` would be a literal needle and match both files.
        write(&dir.join("name  with  gaps.txt"));
        fs::write(dir.join("content.txt"), b"value  with  double spaces\n").unwrap();
        let cancel = AtomicBool::new(false);

        // The fixtures are discoverable with a normal query (non-vacuous).
        assert_eq!(
            names(&search_by_name(
                &dir,
                "gaps",
                SearchOptions::default(),
                &cancel
            )),
            vec!["name  with  gaps.txt"]
        );
        assert_eq!(
            names(&search_contents(
                &dir,
                "double spaces",
                ContentSearchOptions::default(),
                &cancel
            )),
            vec!["content.txt"]
        );

        assert!(
            search_by_name(&dir, "  ", SearchOptions::default(), &cancel).is_empty(),
            "a whitespace-only name query must not match literal double spaces"
        );
        assert!(
            search_contents(&dir, "  ", ContentSearchOptions::default(), &cancel).is_empty(),
            "a whitespace-only content query must not match literal double spaces"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    fn write(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"x").unwrap();
    }

    fn names(results: &[SearchResult]) -> Vec<String> {
        results
            .iter()
            .map(|result| result.entry.name_text.clone())
            .collect()
    }

    #[test]
    fn finds_nested_matches_case_insensitively() {
        let dir = scratch("nested");
        write(&dir.join("Alpha.txt"));
        write(&dir.join("sub/beta.TXT"));
        write(&dir.join("sub/deeper/gamma.md"));

        let found = search_by_name(
            &dir,
            "txt",
            SearchOptions::default(),
            &AtomicBool::new(false),
        );
        assert_eq!(names(&found), vec!["Alpha.txt", "beta.TXT"]);
        assert!(found.iter().all(|result| !result.entry.is_dir));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn hidden_entries_are_skipped_by_default() {
        let dir = scratch("hidden");
        write(&dir.join("visible.txt"));
        write(&dir.join(".hidden.txt"));
        write(&dir.join(".hiddendir/inside.txt"));

        let found = search_by_name(
            &dir,
            "txt",
            SearchOptions::default(),
            &AtomicBool::new(false),
        );
        assert_eq!(names(&found), vec!["visible.txt"]);

        let options = SearchOptions {
            include_hidden: true,
            ..Default::default()
        };
        let found = search_by_name(&dir, "txt", options, &AtomicBool::new(false));
        assert_eq!(names(&found).len(), 3);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn does_not_follow_symlinked_directories() {
        let dir = scratch("symlink");
        // Fixture OUTSIDE the search root: the walk must never escape `root`.
        // This is a non-vacuous exclusion check on every platform.
        let outside = dir.join("outside");
        write(&outside.join("secret.txt"));
        let root = dir.join("root");
        fs::create_dir_all(&root).unwrap();
        // Positive control: the query really matches a visible in-root file,
        // so the exclusions below are meaningful rather than vacuous.
        write(&root.join("secret-own.txt"));
        // Hidden in-root match: excluded by the hidden filter everywhere.
        write(&root.join(".secret-hidden.txt"));
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root.join("link-to-outside")).unwrap();

        let found = search_by_name(
            &root,
            "secret",
            SearchOptions::default(),
            &AtomicBool::new(false),
        );
        // Verified on all platforms: only the visible in-root match is
        // returned. `outside/secret.txt` is out of scope and
        // `.secret-hidden.txt` is hidden. On Unix the symlink is not followed,
        // so `secret.txt` is absent even though the link lives in `root`.
        assert_eq!(names(&found), vec!["secret-own.txt"]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn respects_max_results_and_depth() {
        let dir = scratch("limits");
        for index in 0..10 {
            write(&dir.join(format!("match-{index}.txt")));
        }
        let options = SearchOptions {
            max_results: 3,
            ..Default::default()
        };
        let found = search_by_name(&dir, "match", options, &AtomicBool::new(false));
        assert_eq!(found.len(), 3);

        let nested = scratch("depth");
        write(&nested.join("one.txt"));
        write(&nested.join("a/two.txt"));
        write(&nested.join("a/b/three.txt"));
        let options = SearchOptions {
            max_depth: 1,
            ..Default::default()
        };
        let found = search_by_name(&nested, "txt", options, &AtomicBool::new(false));
        assert_eq!(names(&found), vec!["two.txt", "one.txt"]);

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(&nested);
    }

    #[test]
    fn cancellation_returns_early() {
        let dir = scratch("cancel");
        write(&dir.join("a.txt"));
        let cancel = AtomicBool::new(true);
        let found = search_by_name(&dir, "txt", SearchOptions::default(), &cancel);
        assert!(found.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn skip_continues_after_an_already_collected_batch() {
        let dir = scratch("skip");
        for index in 0..5 {
            write(&dir.join(format!("match-{index}.txt")));
        }
        let cancel = AtomicBool::new(false);
        let first = search_by_name(
            &dir,
            "match",
            SearchOptions {
                max_results: 2,
                ..Default::default()
            },
            &cancel,
        );
        assert_eq!(names(&first), vec!["match-0.txt", "match-1.txt"]);
        let rest = search_by_name(
            &dir,
            "match",
            SearchOptions {
                max_results: 2,
                skip: 2,
                ..Default::default()
            },
            &cancel,
        );
        assert_eq!(names(&rest), vec!["match-2.txt", "match-3.txt"]);
        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod filter_tests {
    use super::{filter_results, SearchKind};
    use crate::model::Entry;
    use std::ffi::OsString;
    use std::path::PathBuf;

    fn entry(name: &str, is_dir: bool) -> Entry {
        Entry {
            name: OsString::from(name),
            name_text: name.to_string(),
            sort_key: name.to_lowercase(),
            path: PathBuf::from(format!("/root/{name}")),
            is_dir,
            is_symlink: false,
            size: 0,
            modified: None,
        }
    }

    fn names(entries: &[Entry]) -> Vec<String> {
        entries
            .iter()
            .map(|entry| entry.name_text.clone())
            .collect()
    }

    #[test]
    fn filters_by_kind() {
        let entries = vec![
            entry("a.txt", false),
            entry("docs", true),
            entry("b.rs", false),
        ];
        assert_eq!(
            names(&filter_results(&entries, SearchKind::All, None, None)).len(),
            3
        );
        assert_eq!(
            names(&filter_results(&entries, SearchKind::Files, None, None)),
            vec!["a.txt", "b.rs"]
        );
        assert_eq!(
            names(&filter_results(&entries, SearchKind::Dirs, None, None)),
            vec!["docs"]
        );
    }

    #[test]
    fn dotfile_without_stem_does_not_match_extension_filter() {
        use crate::model::kind_label;

        let entries = vec![entry(".rs", false), entry("real.rs", false)];
        // A leading dot with no stem is not an extension in the model either.
        assert_eq!(kind_label(&entries[0]), "—");
        assert_eq!(
            names(&filter_results(&entries, SearchKind::All, Some("rs"), None)),
            vec!["real.rs"]
        );
        // Case-insensitivity is preserved.
        assert_eq!(
            names(&filter_results(
                &entries,
                SearchKind::All,
                Some(".RS"),
                None
            )),
            vec!["real.rs"]
        );
    }

    #[test]
    fn filters_by_extension_case_insensitively() {
        let entries = vec![
            entry("a.TXT", false),
            entry("b.rs", false),
            entry("c.rs.bak", false),
        ];
        assert_eq!(
            names(&filter_results(
                &entries,
                SearchKind::All,
                Some("txt"),
                None
            )),
            vec!["a.TXT"]
        );
        assert_eq!(
            names(&filter_results(
                &entries,
                SearchKind::All,
                Some(".RS"),
                None
            )),
            vec!["b.rs"]
        );
        assert_eq!(
            names(&filter_results(&entries, SearchKind::All, Some(""), None)).len(),
            3
        );
    }

    #[test]
    fn filters_by_name_substring_case_insensitively() {
        let entries = vec![
            entry("report-2026.txt", false),
            entry("REPORT-draft.txt", false),
            entry("notes.md", false),
        ];
        assert_eq!(
            names(&filter_results(
                &entries,
                SearchKind::All,
                None,
                Some("report")
            )),
            vec!["report-2026.txt", "REPORT-draft.txt"]
        );
        // Empty and whitespace-only filters are ignored.
        assert_eq!(
            names(&filter_results(&entries, SearchKind::All, None, Some("  "))).len(),
            3
        );
        // Name and extension filters combine.
        assert_eq!(
            names(&filter_results(
                &entries,
                SearchKind::All,
                Some("md"),
                Some("note")
            )),
            vec!["notes.md"]
        );
    }

    #[test]
    fn combines_kind_and_extension() {
        let entries = vec![entry("dir.rs", true), entry("file.rs", false)];
        assert_eq!(
            names(&filter_results(
                &entries,
                SearchKind::Files,
                Some("rs"),
                None
            )),
            vec!["file.rs"]
        );
        assert!(filter_results(&entries, SearchKind::Dirs, Some("txt"), None).is_empty());
    }
}

#[cfg(test)]
mod content_tests {
    use super::{search_contents, ContentSearchOptions, SWAP_OPEN_WITH};
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::AtomicBool;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-content-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write(path: &PathBuf, bytes: &[u8]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, bytes).unwrap();
    }

    #[test]
    fn finds_content_with_line_numbers() {
        let dir = scratch("lines");
        write(&dir.join("a.txt"), b"alpha\nbeta TARGET\ngamma\n");
        write(&dir.join("sub/b.txt"), b"nothing here\n");
        write(&dir.join("sub/c.txt"), b"TARGET first line\n");

        let found = search_contents(
            &dir,
            "target",
            ContentSearchOptions::default(),
            &AtomicBool::new(false),
        );
        let names: Vec<_> = found
            .iter()
            .map(|entry| entry.entry.name_text.clone())
            .collect();
        assert_eq!(names, vec!["a.txt", "c.txt"]);
        assert_eq!(found[0].match_line, Some(2));
        assert_eq!(found[1].match_line, Some(1));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn content_search_respects_max_depth() {
        let dir = scratch("content-depth");
        // Depth 0 (the root) and depth 1 (one directory down) are in scope; a
        // file deeper than that must be skipped even when its content matches.
        write(&dir.join("root.txt"), b"TARGET root\n");
        write(&dir.join("one/level.txt"), b"TARGET level one\n");
        write(&dir.join("one/two/deep.txt"), b"TARGET level two\n");

        let options = ContentSearchOptions {
            max_depth: 1,
            ..Default::default()
        };
        let found = search_contents(&dir, "target", options, &AtomicBool::new(false));
        let names: Vec<_> = found
            .iter()
            .map(|entry| entry.entry.name_text.clone())
            .collect();
        // Non-vacuous: the excluded deep file really holds the needle, so its
        // absence below is caused by the depth cap, not by the fixture data.
        assert_eq!(names, vec!["level.txt", "root.txt"]);
        assert!(!names.iter().any(|name| name == "deep.txt"));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_binary_and_oversized_files() {
        let dir = scratch("binary");
        write(&dir.join("binary.bin"), b"TARGET\0\0\0");
        write(&dir.join("big.txt"), b"TARGET but too big");
        write(&dir.join("ok.txt"), b"TARGET ok");

        let options = ContentSearchOptions {
            max_file_size: 10,
            ..Default::default()
        };
        let found = search_contents(&dir, "target", options, &AtomicBool::new(false));
        let names: Vec<_> = found
            .iter()
            .map(|entry| entry.entry.name_text.clone())
            .collect();
        assert_eq!(names, vec!["ok.txt"]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_binary_with_nul_after_first_8kib() {
        let dir = scratch("binary-late");
        let mut bytes = vec![b'a'; 8 * 1024];
        bytes.extend_from_slice(b"\nTARGET\0tail");
        write(&dir.join("late.bin"), &bytes);

        let found = search_contents(
            &dir,
            "target",
            ContentSearchOptions::default(),
            &AtomicBool::new(false),
        );
        assert!(found.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn nul_after_first_match_discards_result() {
        let dir = scratch("binary-after-match");
        write(
            &dir.join("late-nul.txt"),
            b"TARGET on the first line\nplain second line\nthird\0line\n",
        );
        write(&dir.join("clean.txt"), b"TARGET clean\n");

        let found = search_contents(
            &dir,
            "target",
            ContentSearchOptions::default(),
            &AtomicBool::new(false),
        );
        let names: Vec<_> = found
            .iter()
            .map(|entry| entry.entry.name_text.clone())
            .collect();
        assert_eq!(names, vec!["clean.txt"]);
        assert_eq!(found[0].match_line, Some(1));

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn skips_symlinks_and_hidden_entries() {
        let dir = scratch("symlink");
        let root = dir.join("root");
        fs::create_dir_all(&root).unwrap();
        // Fixture OUTSIDE the search root: content search must stay in `root`
        // on every platform (non-vacuous root-confinement control).
        let outside = dir.join("outside");
        write(&outside.join("secret.txt"), b"TARGET outside");
        // Positive control: a visible in-root file with matching content.
        write(&root.join("real.txt"), b"TARGET real");
        // Hidden in-root matches: excluded by default, included on request.
        write(&root.join(".hidden.txt"), b"TARGET hidden");
        write(&root.join(".hidden-dir/inside.txt"), b"TARGET hidden dir");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root.join("link-to-outside")).unwrap();

        let found = search_contents(
            &root,
            "target",
            ContentSearchOptions::default(),
            &AtomicBool::new(false),
        );
        let names: Vec<_> = found
            .iter()
            .map(|entry| entry.entry.name_text.clone())
            .collect();
        // Verified everywhere: the visible in-root file is found while hidden
        // entries and the out-of-root file are excluded. On Unix the symlink
        // is skipped, so "secret.txt" never appears even though the link is
        // present in `root`.
        assert_eq!(names, vec!["real.txt"]);

        let options = ContentSearchOptions {
            include_hidden: true,
            ..Default::default()
        };
        let found = search_contents(&root, "target", options, &AtomicBool::new(false));
        let mut names: Vec<_> = found
            .iter()
            .map(|entry| entry.entry.name_text.clone())
            .collect();
        names.sort();
        // Hidden entries now appear; root confinement still holds (no
        // "secret.txt") on all platforms, and the symlink is skipped on Unix.
        assert_eq!(names, vec![".hidden.txt", "inside.txt", "real.txt"]);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn respects_max_results() {
        let dir = scratch("limit");
        for index in 0..5 {
            write(&dir.join(format!("f{index}.txt")), b"TARGET");
        }
        let options = ContentSearchOptions {
            max_results: 2,
            ..Default::default()
        };
        let found = search_contents(&dir, "target", options, &AtomicBool::new(false));
        assert_eq!(found.len(), 2);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cancellation_returns_early() {
        let dir = scratch("cancel");
        write(&dir.join("a.txt"), b"TARGET");
        let cancel = AtomicBool::new(true);
        let found = search_contents(&dir, "target", ContentSearchOptions::default(), &cancel);
        assert!(found.is_empty());
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn already_cancelled_line_loop_returns_none_before_reading_a_match() {
        use super::first_matching_line_in_file;

        let dir = scratch("cancel-line-loop");
        let file = dir.join("match.txt");
        write(&file, b"TARGET on the very first line\nplain second line\n");

        // Positive control: the fixture really has a match on line 1, so the
        // cancelled result below is caused by the flag, not by the file data.
        let live = AtomicBool::new(false);
        assert_eq!(
            first_matching_line_in_file(&file, "target", 1024, &live).unwrap(),
            Some(1)
        );

        // `search_contents` checks cancellation in its outer loops, so a
        // pre-cancelled flag there never reaches the per-line check inside
        // `first_matching_line_in_file`. Calling it directly covers that branch.
        let cancelled = AtomicBool::new(true);
        assert_eq!(
            first_matching_line_in_file(&file, "target", 1024, &cancelled).unwrap(),
            None
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn a_file_swapped_after_the_metadata_check_is_not_read() {
        let dir = scratch("swap-after-metadata");
        let root = dir.join("root");
        fs::create_dir(&root).unwrap();
        write(&root.join("harmless.txt"), b"nothing to see here\n");
        // Outside the search root, containing the needle. If the walker were
        // to read this after checking `harmless.txt`, it would escape root.
        let outside = dir.join("outside.txt");
        write(&outside, b"TARGET secret outside the root\n");

        // No swap: the ordinary file is searched normally.
        let options = ContentSearchOptions::default();
        let normal = search_contents(&root, "target", options.clone(), &AtomicBool::new(false));
        assert!(normal.is_empty(), "fixture must not match without the swap");

        // Simulate the path being replaced between symlink_metadata and open.
        SWAP_OPEN_WITH.with(|cell| *cell.borrow_mut() = Some(outside.clone()));
        let swapped = search_contents(&root, "target", options.clone(), &AtomicBool::new(false));
        SWAP_OPEN_WITH.with(|cell| *cell.borrow_mut() = None);
        assert!(
            swapped.is_empty(),
            "the opened file's dev/ino differs from the checked path: {swapped:?}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn invalid_utf8_without_nul_is_not_a_match() {
        use super::first_matching_line_in_file;

        let dir = scratch("invalid-utf8");
        // A false match under `from_utf8_lossy`: TARGET followed by CP932-ish
        // bytes that become U+FFFD but still leave "target" intact.
        let invalid = dir.join("invalid.txt");
        write(&invalid, b"plain first\nTARGET\xff\xfe\n");
        // Control: a clean line really does match.
        let clean = dir.join("clean.txt");
        write(&clean, b"TARGET clean\n");

        let live = AtomicBool::new(false);
        assert_eq!(
            first_matching_line_in_file(&invalid, "target", 1024, &live).unwrap(),
            None,
            "invalid UTF-8 must be treated as binary"
        );
        assert_eq!(
            first_matching_line_in_file(&clean, "target", 1024, &live).unwrap(),
            Some(1),
            "clean control line must still match"
        );

        let found = search_contents(
            &dir,
            "target",
            ContentSearchOptions::default(),
            &AtomicBool::new(false),
        );
        let names: Vec<_> = found
            .iter()
            .map(|result| result.entry.name_text.clone())
            .collect();
        assert_eq!(names, vec!["clean.txt"]);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn exact_max_bytes_size_is_not_a_truncated_read() {
        use super::first_matching_line_in_file;

        let dir = scratch("exact-max-bytes");
        let file = dir.join("exact.txt");
        // Exactly max_bytes (8) with an invalid byte in the final line. A
        // naive `bytes.len() == max_bytes` truncation check would allow lossy
        // decoding and report a false match; an exact-size file is complete.
        write(&file, b"TARGET\xff\xff");
        assert_eq!(file.metadata().unwrap().len(), 8);

        let live = AtomicBool::new(false);
        assert_eq!(
            first_matching_line_in_file(&file, "target", 8, &live).unwrap(),
            None,
            "an exact-size file with invalid UTF-8 must not match"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn invalid_utf8_line_discards_an_earlier_match_and_is_counted() {
        use super::first_matching_line_in_file;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let dir = scratch("invalid-utf8-discard");
        // A genuine match on line 1 must NOT survive a non-UTF-8 byte on line 2:
        // the whole file is undecodable, so reporting line 1 would search it
        // only partially. The old fixture put TARGET *on* the invalid line and
        // therefore pinned neither the discard nor the caller's counter.
        let invalid = dir.join("invalid.txt");
        write(&invalid, b"TARGET valid first line\nplain \xff\xfe\n");
        // Control: a clean file still matches and must not inflate the counter.
        let clean = dir.join("clean.txt");
        write(&clean, b"TARGET clean\n");

        let live = AtomicBool::new(false);
        assert_eq!(
            first_matching_line_in_file(&invalid, "target", 1024, &live).unwrap(),
            None,
            "an invalid UTF-8 line must discard the preceding valid match"
        );
        assert_eq!(
            first_matching_line_in_file(&clean, "target", 1024, &live).unwrap(),
            Some(1),
            "clean control line must still match"
        );

        let non_utf8 = Arc::new(AtomicUsize::new(0));
        let options = ContentSearchOptions {
            non_utf8_files: Arc::clone(&non_utf8),
            ..ContentSearchOptions::default()
        };
        let found = search_contents(&dir, "target", options, &AtomicBool::new(false));
        let names: Vec<_> = found
            .iter()
            .map(|result| result.entry.name_text.clone())
            .collect();
        assert_eq!(names, vec!["clean.txt"]);
        assert_eq!(
            non_utf8.load(Ordering::SeqCst),
            1,
            "exactly one file must be counted as not valid UTF-8"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn take_boundary_splitting_multibyte_is_not_treated_as_binary() {
        use super::first_matching_line_in_file;

        let dir = scratch("utf8-boundary");
        let file = dir.join("cut.txt");
        // "TARGET あ" — the 3-byte あ starts at offset 7, so a read capped at
        // 8 bytes cuts it in the middle. That final segment must be decoded
        // lossily instead of being rejected as invalid UTF-8.
        write(&file, "TARGET あ".as_bytes());
        assert_eq!(
            first_matching_line_in_file(&file, "target", 8, &AtomicBool::new(false)).unwrap(),
            Some(1)
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cr_only_line_endings_get_logical_line_numbers() {
        let dir = scratch("cr-lines");
        write(&dir.join("cr.txt"), b"alpha\rTARGET here\rmiddle\r");
        let found = search_contents(
            &dir,
            "target",
            ContentSearchOptions::default(),
            &AtomicBool::new(false),
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].match_line, Some(2));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn crlf_counts_as_one_separator() {
        let dir = scratch("crlf-lines");
        write(&dir.join("crlf.txt"), b"alpha\r\nTARGET here\r\nlast\r\n");
        let found = search_contents(
            &dir,
            "target",
            ContentSearchOptions::default(),
            &AtomicBool::new(false),
        );
        assert_eq!(found.len(), 1);
        // If \r\n were counted twice the match would be reported on line 3.
        assert_eq!(found[0].match_line, Some(2));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn mixed_lf_and_crlf_line_endings() {
        let dir = scratch("mixed-lines");
        write(&dir.join("mixed.txt"), b"first\nsecond\rTARGET\r\nfourth\n");
        let found = search_contents(
            &dir,
            "target",
            ContentSearchOptions::default(),
            &AtomicBool::new(false),
        );
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].match_line, Some(3));
        let _ = fs::remove_dir_all(&dir);
    }
}

#[cfg(test)]
mod scope_tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-scope-{tag}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn search_roots_dedupes_and_filters_missing() {
        let dir = scratch("roots");
        let a = dir.join("a");
        let b = dir.join("b");
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        let missing = dir.join("missing");
        let resolved = search_roots(
            SearchScope::OpenTabs,
            &dir,
            &[a.clone(), b.clone(), a.clone(), missing],
            &[],
        );
        assert_eq!(resolved.roots, vec![a, b]);
        assert!(!resolved.fell_back_to_cwd);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_roots_dedupes_alternative_spellings_of_one_directory() {
        let dir = scratch("roots-alias");
        let real = dir.join("real");
        fs::create_dir_all(&real).unwrap();
        let dot_alias = real.join(".");
        #[cfg(unix)]
        let symlink_alias = dir.join("link-to-real");

        #[cfg(unix)]
        std::os::unix::fs::symlink(&real, &symlink_alias).unwrap();

        // Positive controls: both aliases really are directories and the
        // symlink is a distinct path, so the dedupe below is non-vacuous.
        assert!(dot_alias.is_dir());
        #[cfg(unix)]
        {
            assert!(symlink_alias.is_dir());
            assert_ne!(symlink_alias, real);
        }

        let mut candidates = vec![real.clone(), dot_alias];
        #[cfg(unix)]
        candidates.push(symlink_alias);

        let resolved = search_roots(SearchScope::OpenTabs, &dir, &candidates, &[]);
        assert_eq!(
            resolved.roots.len(),
            1,
            "canonicalize must collapse dir, dir/. and the symlink: {:?}",
            resolved.roots
        );
        // The first spelling wins; all candidates map to the same real path.
        assert_eq!(resolved.roots, vec![real]);
        assert!(!resolved.fell_back_to_cwd);

        // The canonicalize-failure fallback key (`unwrap_or_else(|_| path.clone())`)
        // is intentionally not forced here: `search_roots` first requires
        // `path.is_dir()`, and every deterministic way to make `canonicalize`
        // fail (missing path, dangling link, ELOOP, invalid bytes) also makes
        // `is_dir()` fail. Reaching it needs a filesystem race or a production
        // test seam, so it is reported as deferred instead of refactored.
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_roots_falls_back_to_cwd_when_empty() {
        let dir = scratch("fallback");
        let resolved = search_roots(SearchScope::Bookmarks, &dir, &[], &[]);
        assert_eq!(resolved.roots, vec![dir.clone()]);
        assert!(resolved.fell_back_to_cwd);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_roots_current_scope_is_not_marked_as_fallback() {
        let dir = scratch("current");
        let resolved = search_roots(SearchScope::Current, &dir, &[], &[]);
        assert_eq!(resolved.roots, vec![dir.clone()]);
        assert!(!resolved.fell_back_to_cwd);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_roots_current_scope_keeps_a_missing_cwd_without_a_fallback() {
        // A non-existent cwd is filtered out like any other candidate, but the
        // Current scope must still search the path the user is standing in and
        // must not report it as a fallback to elsewhere.
        let dir = scratch("current-missing");
        let missing = dir.join("gone");
        let resolved = search_roots(SearchScope::Current, &missing, &[], &[]);
        assert_eq!(resolved.roots, vec![missing]);
        assert!(!resolved.fell_back_to_cwd);
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn merge_search_results_dedupes_by_path_and_caps() {
        let p1 = PathBuf::from("/tmp/a");
        let p2 = PathBuf::from("/tmp/b");
        let e = |path: &PathBuf| {
            let name = path.file_name().unwrap_or_default().to_os_string();
            SearchResult {
                match_line: None,
                entry: Entry {
                    name_text: name.to_string_lossy().into_owned(),
                    sort_key: name.to_string_lossy().to_lowercase(),
                    name,
                    path: path.clone(),
                    is_dir: false,
                    is_symlink: false,
                    size: 0,
                    modified: None,
                },
            }
        };
        let merged = merge_search_results(vec![vec![e(&p1), e(&p2)], vec![e(&p1)]], 10);
        let paths: Vec<_> = merged
            .iter()
            .map(|result| result.entry.path.clone())
            .collect();
        assert_eq!(paths, vec![p1.clone(), p2.clone()]);
        let capped = merge_search_results(vec![vec![e(&p1), e(&p2)]], 1);
        assert_eq!(capped.len(), 1);
    }

    #[test]
    fn merge_search_results_sorts_by_path_when_cap_is_hit() {
        let p1 = PathBuf::from("/tmp/a");
        let p2 = PathBuf::from("/tmp/z");
        let e = |path: &PathBuf| {
            let name = path.file_name().unwrap_or_default().to_os_string();
            SearchResult {
                match_line: None,
                entry: Entry {
                    name_text: name.to_string_lossy().into_owned(),
                    sort_key: name.to_string_lossy().to_lowercase(),
                    name,
                    path: path.clone(),
                    is_dir: false,
                    is_symlink: false,
                    size: 0,
                    modified: None,
                },
            }
        };
        // Unordered batch reaching the cap: the final sort must still run.
        let capped = merge_search_results(vec![vec![e(&p2), e(&p1)]], 2);
        let paths: Vec<_> = capped
            .iter()
            .map(|result| result.entry.path.clone())
            .collect();
        assert_eq!(paths, vec![p1, p2]);
    }
}
