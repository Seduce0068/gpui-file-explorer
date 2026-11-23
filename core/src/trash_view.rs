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
//! プラットフォーム固有の一覧取得・復元（ blocking な `trash` API と
//! cfg no-op fallback）は app 側のアダプタに残す。このモジュールは
//! データ契約 [`TrashItem`] と純粋な変換関数のみを持ち、全ターゲットで
//! 単体テストが通る。UI スレッドへの橋渡しも呼び出し側が担当する。

use std::ffi::OsString;
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

use crate::model::Entry;

/// ゴミ箱の一行（プラットフォーム非依存のデータ契約）。
///
/// `id` は非 UTF-8 があり得るため `OsString` のまま保持し、失落的な
/// `String` 変換を避ける。`trash` クレートの `TrashItem` とフィールドを
/// 合わせているため、app 側のアダプタは変換なしで純関数へ渡せる。
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

/// Split a trash listing into the items addressed by `ids` and a count of ids
/// with no match. This is the pure core of [`restore_by_id`]: no filesystem
/// access, so the id match / missing accounting can be tested deterministically
/// instead of depending on a usable system trash directory.
///
/// IDs are compared as `OsString` (not `String`) so non-UTF-8 bytes in a trash
/// id match exactly; a lossy conversion here would break undo bookkeeping.
/// Each requested id is resolved independently: the returned items follow the
/// order of `ids`, and a repeated id yields a repeated item (and a missing id
/// is counted once per occurrence), mirroring the previous inline behaviour.
pub fn partition_restore_ids(items: &[TrashItem], ids: &[OsString]) -> (Vec<TrashItem>, usize) {
    let mut to_restore = Vec::new();
    let mut missing = 0usize;
    for id in ids {
        match items
            .iter()
            .find(|item| item.id.as_os_str() == id.as_os_str())
        {
            Some(item) => to_restore.push(item.clone()),
            None => missing += 1,
        }
    }
    (to_restore, missing)
}

/// Ids in `ids` with no match in `items`, preserving request order (and
/// counting a repeated id once per occurrence). Pure counterpart of the
/// `missing` count in [`partition_restore_ids`] that keeps the ids themselves,
/// so the caller can leave them in the undo entry for a later retry.
pub fn missing_restore_ids(items: &[TrashItem], ids: &[OsString]) -> Vec<OsString> {
    ids.iter()
        .filter(|id| {
            !items
                .iter()
                .any(|item| item.id.as_os_str() == id.as_os_str())
        })
        .cloned()
        .collect()
}

/// Per-item outcome of an id-addressed trash restore.
///
/// A partially successful restore must never be collapsed into one `Err`: the
/// ids that came back belong in redo and the ones that did not belong back in
/// undo, so a retry restores only what is still missing.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RestoreByIdOutcome {
    /// Ids that were actually restored (redo entry).
    pub restored_ids: Vec<OsString>,
    /// Ids with no match in the trash listing; nothing was attempted.
    pub missing_ids: Vec<OsString>,
    /// Ids whose restore failed, paired with the reason (undo entry).
    pub failed: Vec<(OsString, String)>,
}

impl RestoreByIdOutcome {
    /// The first restore error, if any. Kept as an `Option<error>` so callers
    /// can report one concrete reason without losing the per-item split.
    pub fn error(&self) -> Option<&str> {
        self.failed.first().map(|(_, reason)| reason.as_str())
    }

    /// Every id that was not restored, so the undo entry can be retried.
    pub fn remaining_ids(&self) -> Vec<OsString> {
        self.missing_ids
            .iter()
            .chain(self.failed.iter().map(|(id, _)| id))
            .cloned()
            .collect()
    }

    /// True when not a single requested id was restored.
    pub fn is_empty(&self) -> bool {
        self.restored_ids.is_empty()
    }
}

/// Fold single-item restore attempts into a [`RestoreByIdOutcome`].
///
/// `attempts` pairs each id resolved from the trash listing with the result of
/// restoring that one item. Pure (no filesystem access), so a partial success
/// — one collision among several ids — is deterministic to test without a
/// usable system trash directory.
pub fn partition_restore_outcome(
    missing: &[OsString],
    attempts: &[(OsString, Result<(), String>)],
) -> RestoreByIdOutcome {
    let mut outcome = RestoreByIdOutcome {
        missing_ids: missing.to_vec(),
        ..RestoreByIdOutcome::default()
    };
    for (id, result) in attempts {
        match result {
            Ok(()) => outcome.restored_ids.push(id.clone()),
            Err(reason) => outcome.failed.push((id.clone(), reason.clone())),
        }
    }
    outcome
}

/// Recorded `(path, id)` pairs that are safe to re-trash for a redo.
///
/// A recorded id that is *still present* in the trash listing means that item
/// was never restored (a restore collision leaves it there) or was trashed
/// again since then. Its original path must not be touched: if a foreign file
/// was created at that path meanwhile, trashing it would delete data the user
/// never asked to delete. Only ids absent from the listing — i.e. confirmed
/// restored — are valid redo targets. Ids are compared byte-exactly as
/// `OsString`. Pure: the listing snapshot is supplied by the caller.
pub fn redo_trash_targets(
    recorded: &[(PathBuf, OsString)],
    trash_items: &[TrashItem],
) -> Vec<PathBuf> {
    recorded
        .iter()
        .filter(|(_, id)| {
            !trash_items
                .iter()
                .any(|item| item.id.as_os_str() == id.as_os_str())
        })
        .map(|(path, _)| path.clone())
        .collect()
}

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

/// Restore destination of the trash item addressed by a virtual row path.
///
/// Trash rows carry the trash id as their [`Entry::path`], so the original
/// parent is recovered from the listing snapshot instead of being stored in
/// the `Entry` itself. Pure/display-only: used for tooltips and the status bar.
pub fn original_parent_for_path(items: &[TrashItem], path: &std::path::Path) -> Option<String> {
    items
        .iter()
        .find(|item| item.id.as_os_str() == path.as_os_str())
        .map(|item| item.original_parent.to_string_lossy().into_owned())
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
pub fn record_ids(items: &[TrashItem], paths: &[PathBuf]) -> Vec<(PathBuf, OsString)> {
    paths
        .iter()
        .filter_map(|path| {
            items
                .iter()
                .find(|item| item.original_path() == *path)
                .map(|item| (path.clone(), item.id.clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::ffi::OsString;
    use std::path::Path;

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
    fn original_parent_for_path_resolves_restore_destination() {
        let items = vec![
            item("/trash/info/a.trashinfo", "a.txt", "/home/u/Documents", 100),
            item("/trash/info/b.trashinfo", "b.txt", "/mnt/usb", 200),
        ];
        let rows = entries_from_items(&items);
        // Rows are keyed by trash id; lookups must follow them, not the name.
        let parents: Vec<_> = rows
            .iter()
            .map(|row| original_parent_for_path(&items, &row.path))
            .collect();
        assert_eq!(
            parents,
            vec![
                Some("/mnt/usb".to_string()),
                Some("/home/u/Documents".to_string()),
            ]
        );
        assert_eq!(
            original_parent_for_path(&items, Path::new("/not/in/trash")),
            None
        );
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
                    OsString::from("/trash/info/b.trashinfo")
                ),
                (
                    PathBuf::from("/tmp/t/a.txt"),
                    OsString::from("/trash/info/a.trashinfo")
                ),
            ]
        );
        assert!(record_ids(&items, &[]).is_empty());
    }

    #[test]
    fn partition_restore_ids_matches_requested_ids_in_request_order() {
        let items = vec![
            item("/trash/info/a.trashinfo", "a.txt", "/home/u", 100),
            item("/trash/info/b.trashinfo", "b.txt", "/home/u", 300),
        ];
        let ids = vec![
            OsString::from("/trash/info/b.trashinfo"),
            OsString::from("/trash/info/a.trashinfo"),
        ];
        let (to_restore, missing) = partition_restore_ids(&items, &ids);
        assert_eq!(missing, 0);
        assert_eq!(
            to_restore
                .iter()
                .map(|item| item.id.clone())
                .collect::<Vec<_>>(),
            ids,
            "resolved items follow the requested id order"
        );
    }

    #[test]
    fn partition_restore_ids_reports_unmatched_ids_without_restoring() {
        let items = vec![item("/trash/info/a.trashinfo", "a.txt", "/home/u", 100)];
        let ids = vec![
            OsString::from("/trash/info/a.trashinfo"),
            OsString::from("not-in-listing"),
        ];
        let (to_restore, missing) = partition_restore_ids(&items, &ids);
        assert_eq!(to_restore.len(), 1);
        assert_eq!(to_restore[0].id, ids[0]);
        assert_eq!(missing, 1);
    }

    #[test]
    fn partition_restore_ids_handles_empty_inputs() {
        let items = vec![item("/trash/info/a.trashinfo", "a.txt", "/home/u", 100)];
        let (to_restore, missing) = partition_restore_ids(&items, &[]);
        assert!(to_restore.is_empty());
        assert_eq!(missing, 0);

        let id = OsString::from("unmatched");
        let (to_restore, missing) = partition_restore_ids(&[], &[id]);
        assert!(to_restore.is_empty());
        assert_eq!(missing, 1);

        let (to_restore, missing) = partition_restore_ids(&[], &[]);
        assert!(to_restore.is_empty());
        assert_eq!(missing, 0);
    }

    #[test]
    fn partition_restore_ids_counts_duplicate_ids_per_occurrence() {
        let items = vec![item("/trash/info/a.trashinfo", "a.txt", "/home/u", 100)];
        let id = OsString::from("/trash/info/a.trashinfo");
        let missing_id = OsString::from("gone");
        let (to_restore, missing) = partition_restore_ids(
            &items,
            &[id.clone(), id.clone(), missing_id.clone(), missing_id],
        );
        assert_eq!(
            to_restore.len(),
            2,
            "each occurrence of a matching id is resolved"
        );
        assert!(to_restore.iter().all(|item| item.id == id));
        assert_eq!(missing, 2, "each occurrence of a missing id is counted");
    }

    #[test]
    fn missing_restore_ids_keeps_the_unmatched_ids_in_request_order() {
        let items = vec![item("/trash/info/a.trashinfo", "a.txt", "/home/u", 100)];
        let ids = vec![
            OsString::from("gone-1"),
            OsString::from("/trash/info/a.trashinfo"),
            OsString::from("gone-2"),
            OsString::from("gone-1"),
        ];
        assert_eq!(
            missing_restore_ids(&items, &ids),
            vec![
                OsString::from("gone-1"),
                OsString::from("gone-2"),
                OsString::from("gone-1"),
            ]
        );
        assert!(missing_restore_ids(&items, &[]).is_empty());
    }

    #[test]
    fn partition_restore_outcome_splits_partial_success_per_item() {
        let kept = OsString::from("still-trashed");
        let missing = OsString::from("not-in-listing");
        let attempts = vec![
            (OsString::from("restored-1"), Ok(())),
            (kept.clone(), Err("collision".to_string())),
            (OsString::from("restored-2"), Ok(())),
        ];
        let outcome = partition_restore_outcome(std::slice::from_ref(&missing), &attempts);
        assert_eq!(
            outcome.restored_ids,
            vec![OsString::from("restored-1"), OsString::from("restored-2")]
        );
        assert_eq!(outcome.missing_ids, vec![missing.clone()]);
        assert_eq!(
            outcome.failed,
            vec![(kept.clone(), "collision".to_string())]
        );
        assert_eq!(outcome.error(), Some("collision"));
        assert_eq!(
            outcome.remaining_ids(),
            vec![missing, kept.clone()],
            "missing and failed ids both stay in undo"
        );
        assert!(!outcome.is_empty());

        let all_ok = partition_restore_outcome(&[], &[(kept.clone(), Ok(()))]);
        assert_eq!(all_ok.error(), None);
        assert!(all_ok.remaining_ids().is_empty());
        let nothing = partition_restore_outcome(&[], &[]);
        assert!(nothing.is_empty());
        assert_eq!(nothing.error(), None);
    }

    #[test]
    fn redo_trash_targets_refuses_ids_still_present_in_the_trash() {
        let in_trash = item("/trash/info/kept.trashinfo", "kept.txt", "/home/u", 100);
        let recorded = vec![
            (
                PathBuf::from("/home/u/restored.txt"),
                OsString::from("/trash/info/restored.trashinfo"),
            ),
            (
                PathBuf::from("/home/u/kept.txt"),
                OsString::from("/trash/info/kept.trashinfo"),
            ),
        ];
        assert_eq!(
            redo_trash_targets(&recorded, std::slice::from_ref(&in_trash)),
            vec![PathBuf::from("/home/u/restored.txt")],
            "an id still in the trash was never restored; its path may be foreign"
        );
        assert!(redo_trash_targets(&recorded, &[]).len() == 2);
        assert!(redo_trash_targets(&[], std::slice::from_ref(&in_trash)).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn restore_split_and_redo_targets_compare_non_utf8_ids_byte_exactly() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let id_bytes = b"/trash/info/non-utf8-\xff.trashinfo".to_vec();
        let id = OsString::from_vec(id_bytes.clone());
        let near_miss = OsString::from_vec(b"/trash/info/non-utf8-\xfe.trashinfo".to_vec());
        let item = TrashItem {
            id: id.clone(),
            name: OsString::from("v.txt"),
            original_parent: PathBuf::from("/home/u"),
            time_deleted: 100,
        };

        assert_eq!(
            missing_restore_ids(std::slice::from_ref(&item), std::slice::from_ref(&id)),
            Vec::<OsString>::new()
        );
        assert_eq!(
            missing_restore_ids(
                std::slice::from_ref(&item),
                std::slice::from_ref(&near_miss)
            ),
            vec![near_miss.clone()],
            "lossy string conversion would make 0xff and 0xfe collide"
        );

        let outcome =
            partition_restore_outcome(std::slice::from_ref(&near_miss), &[(id.clone(), Ok(()))]);
        assert_eq!(outcome.restored_ids, vec![id.clone()]);
        assert_eq!(outcome.missing_ids, vec![near_miss.clone()]);

        let recorded = vec![
            (PathBuf::from("/home/u/v.txt"), id.clone()),
            (PathBuf::from("/home/u/other.txt"), near_miss.clone()),
        ];
        assert_eq!(
            redo_trash_targets(&recorded, std::slice::from_ref(&item)),
            vec![PathBuf::from("/home/u/other.txt")],
            "id still in trash must be skipped even when bytes differ only at 0xff"
        );
        assert_eq!(recorded[0].1.as_os_str().as_bytes(), id_bytes.as_slice());
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_id_round_trips_through_trash_structs() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        // A real OS trash round-trip needs a desktop session, so exercise the
        // exact ID representation used by undo bookkeeping with raw bytes that
        // are invalid UTF-8 (0xff). A `String` would replace that byte with
        // U+FFFD and could no longer match the listed item.
        let id_bytes = b"/trash/info/non-utf8-\xff.trashinfo".to_vec();
        let id = OsString::from_vec(id_bytes.clone());
        let item = TrashItem {
            id: id.clone(),
            name: OsString::from_vec(b"non-utf8-\xff.txt".to_vec()),
            original_parent: PathBuf::from("/tmp/t"),
            time_deleted: 100,
        };
        let original_path = item.original_path();

        // Undo bookkeeping stores the id verbatim, not a lossy `String`.
        let recorded = record_ids(
            std::slice::from_ref(&item),
            std::slice::from_ref(&original_path),
        );
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].0, original_path);
        assert_eq!(recorded[0].1, id);
        assert_eq!(recorded[0].1.as_os_str().as_bytes(), id_bytes.as_slice());

        // The virtual row path is built from the `OsString`, so selection and
        // restore-by-id resolve the same bytes back to the original item.
        let rows = entries_from_items(std::slice::from_ref(&item));
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].path.as_os_str().as_bytes(), id_bytes.as_slice());
        assert_eq!(PathBuf::from(&recorded[0].1), rows[0].path);
        let resolved = items_for_paths(std::slice::from_ref(&item), &[rows[0].path.clone()]);
        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].id, id);
        assert_eq!(resolved[0].id.as_os_str().as_bytes(), id_bytes.as_slice());

        // restore-by-id's pure core compares the same raw bytes: a lossy
        // `String` would turn 0xff into U+FFFD and fail to match the item.
        let (to_restore, missing) =
            partition_restore_ids(std::slice::from_ref(&item), std::slice::from_ref(&id));
        assert_eq!(to_restore.len(), 1);
        assert_eq!(to_restore[0].id.as_os_str().as_bytes(), id_bytes.as_slice());
        assert_eq!(missing, 0);
    }
}
