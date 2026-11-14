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

use crate::model::{is_hidden_name, Entry};

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

fn entry_for(path: &Path, name: &str) -> Entry {
    let link_metadata = std::fs::symlink_metadata(path).ok();
    let is_symlink = link_metadata
        .as_ref()
        .map(|metadata| metadata.file_type().is_symlink())
        .unwrap_or(false);
    let metadata = std::fs::metadata(path).ok().or(link_metadata);
    let is_dir = metadata
        .as_ref()
        .map(|metadata| metadata.is_dir())
        .unwrap_or(false);
    Entry {
        name: name.to_string(),
        path: path.to_path_buf(),
        is_dir,
        is_symlink,
        size: metadata
            .as_ref()
            .map(|metadata| metadata.len())
            .unwrap_or(0),
        modified: metadata.and_then(|metadata| metadata.modified().ok()),
    }
}

/// Search `root` recursively for entries whose file name contains `query`
/// (case-insensitive). Returns matching entries sorted by path.
pub fn search_by_name(
    root: &Path,
    query: &str,
    options: SearchOptions,
    cancel: &AtomicBool,
) -> Vec<Entry> {
    let needle = query.to_lowercase();
    if needle.is_empty() {
        return Vec::new();
    }

    let mut matches: Vec<Entry> = Vec::new();
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
                matches.push(entry_for(&path, &name));
                if matches.len() >= options.max_results {
                    break;
                }
            }
            if is_dir_without_following && depth < options.max_depth {
                stack.push((path, depth + 1));
            }
        }
    }

    matches.sort_by(|left, right| left.path.cmp(&right.path));
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

    fn names(entries: &[Entry]) -> Vec<String> {
        entries.iter().map(|entry| entry.name.clone()).collect()
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
        assert!(found.iter().all(|entry| !entry.is_dir));

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
