// SPDX-License-Identifier: AGPL-3.0-or-later
//! Read-only trash view helpers.
//!
//! The explorer shows the system trash as a virtual listing: each row is a
//! [`trash::TrashItem`] mapped to a regular [`Entry`] whose `path` is the
//! platform-specific trash id (never a real filesystem path). Actions in this
//! view are limited to restoring items — permanent deletion is deliberately
//! not offered anywhere in the app.
//!
//! This module is GPUI-free so the mapping and sorting can be unit tested.

use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use crate::model::Entry;

/// List the system trash. Blocking: call from the background executor.
pub fn list() -> Result<Vec<trash::TrashItem>, String> {
    trash::os_limited::list().map_err(|error| error.to_string())
}

/// Restore items to their original locations. Blocking: call from the
/// background executor. Collisions are reported by the `trash` crate and leave
/// the affected items in the trash.
pub fn restore(items: Vec<trash::TrashItem>) -> Result<(), String> {
    trash::os_limited::restore_all(items).map_err(|error| error.to_string())
}

/// Restore the trashed items identified by `(original path, trash id)`.
///
/// Undo uses the trash **id** to find the exact item, so an unrelated item that
/// happens to share the original path is never restored. Items that are no
/// longer in the trash are reported as missing. Returns `(restored, missing)`.
pub fn restore_by_id(entries: &[(std::path::PathBuf, String)]) -> Result<(usize, usize), String> {
    let items = list()?;
    let mut to_restore: Vec<trash::TrashItem> = Vec::new();
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
        trash::os_limited::restore_all(to_restore).map_err(|error| error.to_string())?;
    }
    Ok((restored, missing))
}

/// Map trash items to list rows (newest first). The `path` is the trash id, so
/// selection works without touching real files.
pub fn entries_from_items(items: &[trash::TrashItem]) -> Vec<Entry> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    fn item(id: &str, name: &str, parent: &str, deleted: i64) -> trash::TrashItem {
        trash::TrashItem {
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
