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

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use crate::model::{entry_for_path, is_hidden_name, Entry};

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

/// Filter already-collected search results by kind and optional extension.
/// The extension is case-insensitive and may be given with or without a
/// leading dot (`rs`, `.rs`); an empty extension means "no extension filter".
pub fn filter_results(entries: &[Entry], kind: SearchKind, extension: Option<&str>) -> Vec<Entry> {
    let extension = extension
        .map(|value| value.trim().trim_start_matches('.').to_lowercase())
        .filter(|value| !value.is_empty());
    entries
        .iter()
        .filter(|entry| kind.matches(entry))
        .filter(|entry| match &extension {
            Some(extension) => entry
                .name_text
                .to_lowercase()
                .ends_with(&format!(".{extension}")),
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
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_results: 2_000,
            max_depth: 24,
            include_hidden: false,
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

/// Search `root` recursively for entries whose file name contains `query`
/// (case-insensitive). Returns matching entries sorted by path.
pub fn search_by_name(
    root: &Path,
    query: &str,
    options: SearchOptions,
    cancel: &AtomicBool,
) -> Vec<SearchResult> {
    let needle = query.to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }

    let mut matches: Vec<SearchResult> = Vec::new();
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
            if name.to_lowercase().contains(&needle) {
                if let Some(entry) = entry_for_path(&path) {
                    matches.push(SearchResult {
                        entry,
                        match_line: None,
                    });
                    if matches.len() >= options.max_results {
                        break;
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
                return merged;
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
#[derive(Clone, Copy, Debug)]
pub struct ContentSearchOptions {
    pub max_results: usize,
    pub max_depth: usize,
    pub include_hidden: bool,
    /// Files larger than this are skipped (no partial reads).
    pub max_file_size: u64,
}

impl Default for ContentSearchOptions {
    fn default() -> Self {
        Self {
            max_results: 500,
            max_depth: 24,
            include_hidden: false,
            max_file_size: 1024 * 1024,
        }
    }
}

/// Return the 1-based line number of the first line containing `needle`
/// (case-insensitive), or `None` when the file has no match or is binary.
///
/// Reads one line at a time so a match near the start avoids reading the
/// whole file. A NUL byte anywhere in the read range marks the file as
/// binary, and cancellation is checked between lines.
fn first_matching_line_in_file(
    path: &Path,
    needle: &str,
    max_bytes: u64,
    cancel: &AtomicBool,
) -> std::io::Result<Option<u32>> {
    use std::io::{BufRead, BufReader, Read};

    let file = std::fs::File::open(path)?;
    let mut reader = BufReader::new(file.take(max_bytes));
    let mut line = Vec::new();
    let mut line_number = 0u32;
    loop {
        if cancel.load(Ordering::SeqCst) {
            return Ok(None);
        }
        line.clear();
        if reader.read_until(b'\n', &mut line)? == 0 {
            break;
        }
        line_number += 1;
        if line.contains(&0) {
            return Ok(None); // binary file
        }
        if String::from_utf8_lossy(&line)
            .to_lowercase()
            .contains(needle)
        {
            return Ok(Some(line_number));
        }
    }
    Ok(None)
}

/// Recursively search file **contents** for `query` (case-insensitive).
///
/// Safety/conservatism:
/// - symlinks are skipped entirely (no reading through links),
/// - files larger than `max_file_size` are skipped,
/// - files containing a NUL byte anywhere in the read range are binary,
/// - hidden entries follow `include_hidden`, unreadable entries are skipped,
/// - reads are streamed line-by-line with an early exit on the first match,
/// - results and depth are capped; cancellation is checked per entry and line.
pub fn search_contents(
    root: &Path,
    query: &str,
    options: ContentSearchOptions,
    cancel: &AtomicBool,
) -> Vec<SearchResult> {
    let needle = query.to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }

    let mut matches: Vec<SearchResult> = Vec::new();
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
            if !link_metadata.is_file() || link_metadata.len() > options.max_file_size {
                continue;
            }
            if let Ok(Some(line)) =
                first_matching_line_in_file(&path, &needle, options.max_file_size, cancel)
            {
                if let Some(entry) = entry_for_path(&path) {
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

    matches.sort_by(|left, right| left.entry.path.cmp(&right.entry.path));
    matches
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

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
        let outside = dir.join("outside");
        write(&outside.join("secret.txt"));
        let root = dir.join("root");
        fs::create_dir_all(&root).unwrap();
        write(&root.join("own.txt"));
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, root.join("link-to-outside")).unwrap();

        let found = search_by_name(
            &root,
            "secret",
            SearchOptions::default(),
            &AtomicBool::new(false),
        );
        assert!(found.is_empty(), "symlinked dir must not be traversed");

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
            names(&filter_results(&entries, SearchKind::All, None)).len(),
            3
        );
        assert_eq!(
            names(&filter_results(&entries, SearchKind::Files, None)),
            vec!["a.txt", "b.rs"]
        );
        assert_eq!(
            names(&filter_results(&entries, SearchKind::Dirs, None)),
            vec!["docs"]
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
            names(&filter_results(&entries, SearchKind::All, Some("txt"))),
            vec!["a.TXT"]
        );
        assert_eq!(
            names(&filter_results(&entries, SearchKind::All, Some(".RS"))),
            vec!["b.rs"]
        );
        assert_eq!(
            names(&filter_results(&entries, SearchKind::All, Some(""))).len(),
            3
        );
    }

    #[test]
    fn combines_kind_and_extension() {
        let entries = vec![entry("dir.rs", true), entry("file.rs", false)];
        assert_eq!(
            names(&filter_results(&entries, SearchKind::Files, Some("rs"))),
            vec!["file.rs"]
        );
        assert!(filter_results(&entries, SearchKind::Dirs, Some("txt")).is_empty());
    }
}

#[cfg(test)]
mod content_tests {
    use super::{search_contents, ContentSearchOptions};
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
    fn skips_symlinks_and_hidden_entries() {
        let dir = scratch("symlink");
        write(&dir.join("real.txt"), b"TARGET real");
        write(&dir.join(".hidden.txt"), b"TARGET hidden");
        write(&dir.join(".hidden-dir/inside.txt"), b"TARGET hidden dir");
        #[cfg(unix)]
        std::os::unix::fs::symlink(dir.join("real.txt"), dir.join("link.txt")).unwrap();

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
        assert_eq!(names, vec!["real.txt"]);

        let options = ContentSearchOptions {
            include_hidden: true,
            ..Default::default()
        };
        let found = search_contents(&dir, "target", options, &AtomicBool::new(false));
        let mut names: Vec<_> = found
            .iter()
            .map(|entry| entry.entry.name_text.clone())
            .collect();
        names.sort();
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
}
