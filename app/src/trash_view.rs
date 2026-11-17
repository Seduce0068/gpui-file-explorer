// SPDX-License-Identifier: AGPL-3.0-or-later
//! Read-only trash view helpers.
//!
//! The explorer shows the system trash as a virtual listing: each row is a
//! [`TrashItem`] mapped to a regular [`Entry`] whose `path` is the
//! platform-specific trash id (never a real filesystem path). Actions in this
//! view are limited to restoring items — permanent deletion is deliberately
//! not offered anywhere in the app.
//!
//! This module is GPUI-free so the mapping and sorting can be unit tested.
//!
//! # Platform availability
//!
//! `trash` 5.3.0 only exposes `os_limited` (list / restore) on Windows and on
//! Freedesktop-compliant Unix, and its mobile targets have no platform backend
//! at all. The `platform` module below mirrors that exact gate: on macOS / iOS
//! / Android the crate is not even a dependency (see `Cargo.toml`) and a
//! no-op fallback keeps the rest of the app compiling with the trash view
//! simply unavailable.

use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use crate::model::Entry;

/// Mirrors `trash` 5.3.0's `os_limited` cfg: `target_os = "windows"` or
/// `all(unix, not(macos), not(ios), not(android), feature = "freedesktop")`.
/// This app enables the `trash` crate's default features (including
/// `freedesktop`), so the `feature` part is always true for the dependency.
#[cfg(any(
    target_os = "windows",
    all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    )
))]
mod platform {
    use std::path::{Path, PathBuf};

    pub use trash::TrashItem;

    /// Blocking: walks the trash directories on the calling thread. UI code
    /// must offload this with `cx.background_spawn(async { trash_view::list() })`.
    pub fn list() -> Result<Vec<TrashItem>, String> {
        trash::os_limited::list().map_err(|error| error.to_string())
    }

    /// Blocking: call from the background executor, never from render/UI code.
    pub fn restore(items: Vec<TrashItem>) -> Result<(), String> {
        trash::os_limited::restore_all(items).map_err(|error| error.to_string())
    }

    pub fn delete<P: AsRef<Path>>(path: P) -> Result<(), String> {
        trash::delete(path).map_err(|error| error.to_string())
    }

    /// Blocking: calls [`list`] internally, so offload it from the UI thread.
    pub fn restore_by_id(entries: &[(PathBuf, String)]) -> Result<(usize, usize), String> {
        let items = list()?;
        let mut to_restore: Vec<TrashItem> = Vec::new();
        let mut missing = 0usize;
        for (_path, id) in entries {
            match items
                .iter()
                .find(|item| item.id.to_string_lossy() == id.as_str())
            {
                Some(item) => to_restore.push(item.clone()),
                None => missing += 1,
            }
        }
        let restored = to_restore.len();
        if restored > 0 {
            restore(to_restore)?;
        }
        Ok((restored, missing))
    }
}

/// Fallback for platforms where the `trash` crate cannot provide a trash view.
/// Every operation is a deterministic no-op / explicit error so callers keep
/// the same signatures and the UI stays functional (with an empty trash view).
#[cfg(not(any(
    target_os = "windows",
    all(
        unix,
        not(target_os = "macos"),
        not(target_os = "ios"),
        not(target_os = "android")
    )
)))]
mod platform {
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    /// Field-for-field stand-in for `trash::TrashItem`, so the shared mapping
    /// code and tests compile unchanged on every target.
    #[derive(Debug, Clone)]
    pub struct TrashItem {
        pub id: OsString,
        pub name: OsString,
        pub original_parent: PathBuf,
        pub time_deleted: i64,
    }

    impl TrashItem {
        pub fn original_path(&self) -> PathBuf {
            self.original_parent.join(&self.name)
        }
    }

    pub fn list() -> Result<Vec<TrashItem>, String> {
        Ok(Vec::new())
    }

    pub fn restore(_items: Vec<TrashItem>) -> Result<(), String> {
        Err("ゴミ箱の復元はこのプラットフォームでは利用できません".to_string())
    }

    pub fn delete<P: AsRef<Path>>(_path: P) -> Result<(), String> {
        Err("ゴミ箱への移動はこのプラットフォームでは利用できません".to_string())
    }

    pub fn restore_by_id(entries: &[(PathBuf, String)]) -> Result<(usize, usize), String> {
        Ok((0, entries.len()))
    }
}

pub use platform::{delete, list, restore, restore_by_id, TrashItem};

/// Map trash items to list rows (newest first). The `path` is the trash id, so
/// selection works without touching real files.
pub fn entries_from_items(items: &[TrashItem]) -> Vec<Entry> {
    let mut entries: Vec<Entry> = items
        .iter()
        .map(|item| {
            let modified = if item.time_deleted >= 0 {
                Some(UNIX_EPOCH + Duration::from_secs(item.time_deleted as u64))
            } else {
                None
            };
            Entry {
                name_text: item.name.to_string_lossy().into_owned(),
                name: item.name.clone(),
                sort_key: item.name.to_string_lossy().to_lowercase(),
                path: PathBuf::from(&item.id),
                is_dir: false,
                is_symlink: false,
                size: 0,
                modified,
            }
        })
        .collect();
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.modified));
    entries
}

/// Select the trash items addressed by virtual row paths.
///
/// Trash rows use the trash id as their `path`, so a keyboard (Enter / R) or
/// context-menu restore must resolve ids from the current listing rather than
/// from row indices. Pure: no filesystem access, and the order follows
/// `items` (newest first) so multi-selection restores stay deterministic.
pub fn items_for_paths(items: &[TrashItem], paths: &[PathBuf]) -> Vec<TrashItem> {
    items
        .iter()
        .filter(|item| paths.contains(&PathBuf::from(&item.id)))
        .cloned()
        .collect()
}

/// Map already-trashed paths to their trash ids using a listing read earlier.
///
/// Pure (no filesystem access): callers can take a snapshot with the blocking
/// [`list`] on the background executor and reuse the ids here for undo
/// bookkeeping without ever walking the trash directories on the UI thread.
/// Paths with no matching item are skipped, matching the previous inline
/// behaviour (an empty result simply means the undo entry stays empty).
pub fn record_ids(items: &[TrashItem], paths: &[PathBuf]) -> Vec<(PathBuf, String)> {
    paths
        .iter()
        .filter_map(|path| {
            items
                .iter()
                .find(|item| item.original_path() == *path)
                .map(|item| (path.clone(), item.id.to_string_lossy().into_owned()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn item(id: &str, name: &str, parent: &str, deleted: i64) -> TrashItem {
        TrashItem {
            id: OsString::from(id),
            name: OsString::from(name),
            original_parent: PathBuf::from(parent),
            time_deleted: deleted,
        }
    }

    #[test]
    fn maps_items_to_rows_and_sorts_newest_first() {
        let items = vec![
            item("/trash/info/old.trashinfo", "old.txt", "/home/u", 100),
            item("/trash/info/new.trashinfo", "new.txt", "/home/u", 300),
        ];
        let entries = entries_from_items(&items);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].name, "new.txt");
        assert_eq!(entries[0].path, PathBuf::from("/trash/info/new.trashinfo"));
        assert_eq!(
            entries[0].modified,
            Some(UNIX_EPOCH + Duration::from_secs(300))
        );
        assert_eq!(entries[1].name, "old.txt");
        assert!(!entries[0].is_dir);
    }

    #[test]
    fn negative_timestamps_map_to_none() {
        let entries = entries_from_items(&[item("/trash/info/x.trashinfo", "x", "/home/u", -1)]);
        assert_eq!(entries[0].modified, None);
    }

    #[test]
    fn items_for_paths_resolves_multi_selection_and_ignores_unknown_ids() {
        let items = vec![
            item("/trash/info/old.trashinfo", "old.txt", "/home/u", 100),
            item("/trash/info/new.trashinfo", "new.txt", "/home/u", 300),
        ];
        let selected = vec![
            PathBuf::from("/trash/info/new.trashinfo"),
            PathBuf::from("/trash/info/old.trashinfo"),
        ];
        let resolved = items_for_paths(&items, &selected);
        assert_eq!(resolved.len(), 2, "every selected id must be restored");
        assert_eq!(
            resolved
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>(),
            vec![
                OsString::from("/trash/info/old.trashinfo"),
                OsString::from("/trash/info/new.trashinfo"),
            ],
            "order follows the listing, not the click order"
        );
        assert!(items_for_paths(&items, &[PathBuf::from("/trash/info/missing")]).is_empty());
    }

    #[test]
    fn record_ids_maps_only_paths_present_in_the_snapshot() {
        let items = vec![
            item("/trash/info/a.trashinfo", "a.txt", "/tmp/t", 100),
            item("/trash/info/b.trashinfo", "b.txt", "/tmp/t", 200),
        ];
        let paths = vec![
            PathBuf::from("/tmp/t/b.txt"),
            PathBuf::from("/tmp/t/missing.txt"),
            PathBuf::from("/tmp/t/a.txt"),
        ];
        assert_eq!(
            record_ids(&items, &paths),
            vec![
                (
                    PathBuf::from("/tmp/t/b.txt"),
                    "/trash/info/b.trashinfo".to_string()
                ),
                (
                    PathBuf::from("/tmp/t/a.txt"),
                    "/trash/info/a.trashinfo".to_string()
                ),
            ]
        );
        assert!(record_ids(&items, &[]).is_empty());
    }
}

#[cfg(test)]
mod restore_by_id_tests {
    use super::restore_by_id;

    #[test]
    fn unknown_ids_are_reported_as_missing() {
        // A non-existent id must not restore anything and must not error.
        let result = restore_by_id(&[(
            std::path::PathBuf::from("/definitely/not/in/trash.txt"),
            "definitely-not-a-real-trash-id".to_string(),
        )]);
        // A system without a usable trash directory is acceptable in CI.
        if let Ok((restored, missing)) = result {
            assert_eq!(restored, 0);
            assert_eq!(missing, 1);
        }
    }

    #[test]
    fn empty_input_is_a_noop() {
        let result = restore_by_id(&[]);
        if let Ok((restored, missing)) = result {
            assert_eq!(restored, 0);
            assert_eq!(missing, 0);
        }
    }
}
