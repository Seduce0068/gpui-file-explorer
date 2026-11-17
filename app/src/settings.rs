// SPDX-License-Identifier: AGPL-3.0-or-later
//! Persistent user settings (`~/.config/gpui-file-explorer/settings.json`).
//!
//! Schema follows DESIGN.md §5.7 / §8.1:
//!
//! ```json
//! {
//!   "version": 1,
//!   "last_dir": "/home/koppe/document",
//!   "sort": { "key": "name", "dir": "asc", "dirs_first": true },
//!   "show_hidden": false,
//!   "view": "details",
//!   "recent_dirs": ["/tmp"],
//!   "bookmarks": ["/home/koppe"],
//!   "tabs": ["/home/koppe", "/tmp"],
//!   "active_tab": 0,
//!   "recent_searches": ["TODO"]
//! }
//! ```

use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::SortSpec;

use tracing::{error, warn};

/// Directory name under the platform config dir.
pub const APP_DIR: &str = "gpui-file-explorer";
/// Current settings schema version.
pub const SCHEMA_VERSION: u32 = 1;
/// Maximum number of remembered recent directories.
pub const MAX_RECENT_DIRS: usize = 10;
/// Maximum number of bookmarked directories.
pub const MAX_BOOKMARKS: usize = 50;
/// Maximum number of remembered search queries.
pub const MAX_RECENT_SEARCHES: usize = 10;

/// List view mode: the detailed list or the icon grid. Persisted in
/// `settings.json` as `"details"` / `"icons"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewLayout {
    #[default]
    Details,
    Icons,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    /// Last successfully listed directory. Ignored if it no longer exists.
    pub last_dir: Option<PathBuf>,
    pub sort: SortSpec,
    pub show_hidden: bool,
    pub view: ViewLayout,
    /// Recently visited directories, most recent first (bounded).
    pub recent_dirs: Vec<PathBuf>,
    /// Bookmarked directories, in insertion order (bounded).
    pub bookmarks: Vec<PathBuf>,
    /// Directories of the tabs that were open at exit.
    pub tabs: Vec<PathBuf>,
    /// Index of the active tab in `tabs` (clamped when restoring).
    pub active_tab: usize,
    /// Recently used search queries, most recent first (bounded).
    pub recent_searches: Vec<String>,
    /// Set when a corrupt settings file could not be backed up. Prevents a
    /// subsequent `save()` from overwriting the retained original with
    /// defaults. Not persisted.
    #[serde(skip)]
    pub read_only: bool,
    /// Human-readable reason for `read_only`, shown in the status bar.
    #[serde(skip)]
    pub load_error: Option<String>,
    /// When set (debug hooks active), never write to disk. Not persisted.
    #[serde(skip)]
    pub ephemeral: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            last_dir: None,
            sort: SortSpec::default(),
            show_hidden: false,
            view: ViewLayout::Details,
            recent_dirs: Vec::new(),
            bookmarks: Vec::new(),
            tabs: Vec::new(),
            active_tab: 0,
            recent_searches: Vec::new(),
            read_only: false,
            load_error: None,
            ephemeral: false,
        }
    }
}

impl Settings {
    /// Platform config path, e.g. `~/.config/gpui-file-explorer/settings.json`
    /// on Linux.
    pub fn config_path() -> Option<PathBuf> {
        dirs::config_dir().map(|dir| dir.join(APP_DIR).join("settings.json"))
    }

    /// Remember `path` as the most recent directory (de-duplicated, bounded).
    pub fn record_recent(&mut self, path: &std::path::Path) {
        if path.as_os_str().is_empty() {
            return;
        }
        self.recent_dirs.retain(|existing| existing != path);
        self.recent_dirs.insert(0, path.to_path_buf());
        self.recent_dirs.truncate(MAX_RECENT_DIRS);
    }

    /// Whether `path` is bookmarked.
    pub fn is_bookmarked(&self, path: &std::path::Path) -> bool {
        self.bookmarks.iter().any(|bookmark| bookmark == path)
    }

    /// Add or remove `path` from the bookmarks. Returns `true` when it was
    /// added, `false` when it was removed (or the path was empty).
    pub fn toggle_bookmark(&mut self, path: &std::path::Path) -> bool {
        if path.as_os_str().is_empty() {
            return false;
        }
        if let Some(index) = self.bookmarks.iter().position(|bookmark| bookmark == path) {
            self.bookmarks.remove(index);
            return false;
        }
        self.bookmarks.push(path.to_path_buf());
        if self.bookmarks.len() > MAX_BOOKMARKS {
            self.bookmarks.remove(0);
        }
        true
    }

    /// Remember a search query (trimmed, de-duplicated, bounded).
    pub fn record_search(&mut self, query: &str) {
        let query = query.trim();
        if query.is_empty() {
            return;
        }
        self.recent_searches.retain(|existing| existing != query);
        self.recent_searches.insert(0, query.to_string());
        self.recent_searches.truncate(MAX_RECENT_SEARCHES);
    }

    /// Unique backup path for a corrupt settings file (nanoseconds + PID).
    fn backup_path_for(path: &Path) -> PathBuf {
        path.with_extension(format!(
            "json.broken-{}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0),
            std::process::id()
        ))
    }

    /// Load settings, falling back to defaults for a missing/corrupt file.
    pub fn load() -> Self {
        match Self::config_path() {
            Some(path) => Self::load_from(&path),
            None => Self::default(),
        }
    }

    pub fn load_from(path: &Path) -> Self {
        Self::load_from_with_backup(path, Self::backup_path_for(path))
    }

    /// `load_from` with an injectable backup path so the failure branch can be
    /// tested deterministically.
    fn load_from_with_backup(path: &Path, backup: PathBuf) -> Self {
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                // A fresh install has no settings file yet; defaults are fine.
                return Self::default();
            }
            Err(error) => {
                // EACCES and other read failures must not be overwritten by a
                // later save() with defaults (third cross-review F6-04).
                let reason = format!(
                    "設定ファイルを読み込めませんでした（{}: {error}）。書き込みは行いません",
                    path.display()
                );
                error!(%reason, "settings read failed");
                return Self {
                    read_only: true,
                    load_error: Some(reason),
                    ..Self::default()
                };
            }
        };
        let Ok(mut settings) = serde_json::from_str::<Settings>(&text) else {
            // Keep the broken file for debugging (DESIGN.md §8.1) instead of
            // failing to start. If the backup rename fails, stay read-only so
            // a later save() cannot overwrite the retained original.
            if let Err(error) = std::fs::rename(path, &backup) {
                let reason = format!(
                    "壊れた設定の退避に失敗しました（{}: {error}）。元ファイルは保持されます",
                    backup.display()
                );
                error!(%reason, "corrupt settings backup failed");
                return Self {
                    read_only: true,
                    load_error: Some(reason),
                    ..Self::default()
                };
            }
            return Self::default();
        };
        let loaded_version = settings.version;
        settings.migrate(loaded_version);
        settings.validate();
        settings
    }

    /// Convert a deserialized settings file to the current schema.
    ///
    /// `SCHEMA_VERSION` is 1 today. When version 2 is introduced, add a
    /// `2 => { ... }` arm that rewrites the old representation before the
    /// common `version` normalization below. A missing `version` field
    /// deserializes to 0 and is treated as v1 for backward compatibility.
    fn migrate(&mut self, loaded_version: u32) {
        match loaded_version {
            0 | 1 => {}
            future => {
                // A newer schema may contain fields serde ignores. Writing it
                // back as v1 would silently drop those settings, so keep the
                // session read-only (second cross-review H-6).
                let reason =
                    format!("設定スキーマ v{future} は未対応のため、書き込まずに読み込みました");
                warn!(%reason, "unsupported settings schema");
                self.read_only = true;
                self.load_error = Some(reason);
            }
        }
        self.version = SCHEMA_VERSION;
    }

    /// Drop a remembered directory that no longer exists.
    pub fn validate(&mut self) {
        if let Some(dir) = &self.last_dir {
            if !dir.is_dir() {
                self.last_dir = None;
            }
        }
    }

    /// Persist to the platform config path. Failure is reported on stderr
    /// (journald on Linux) but never breaks the UI: settings are best-effort.
    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            if let Err(error) = self.save_to(&path) {
                error!(path = %path.display(), %error, "failed to save settings");
            }
        }
    }

    /// Atomic write (tmp file + rename) so a crash never leaves a truncated
    /// settings file behind. The tmp file is flushed with `sync_all()` before
    /// the rename; a directory fsync is not performed (best-effort).
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if self.ephemeral {
            return Ok(());
        }
        if self.read_only {
            if let Some(reason) = &self.load_error {
                warn!(%reason, "settings are read-only; not saved");
            }
            return Ok(());
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        // O_EXCL reservation: a predictable tmp name with `create(true)`
        // follows a pre-planted symlink (CWE-59). `create_new(true)` refuses
        // to open an existing path, including symlinks. Use nanos + PID so
        // stale temp files from earlier runs don't block normal saves.
        let tmp = path.with_extension(format!(
            "json.tmp-{}-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or(0),
            std::process::id()
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&tmp)?;
        // Settings contain bookmarks / recent paths; keep them private.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = file.set_permissions(std::fs::Permissions::from_mode(0o600));
        }
        let result = (|| -> std::io::Result<()> {
            file.write_all(text.as_bytes())?;
            file.sync_all()
        })();
        drop(file);
        if let Err(error) = result {
            let _ = std::fs::remove_file(&tmp);
            return Err(error);
        }
        std::fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    // Explicit imports only: the crate root glob re-exports GPUI's `test`
    // proc-macro which would shadow the built-in `#[test]`.
    use super::{Settings, ViewLayout, APP_DIR, SCHEMA_VERSION};
    use crate::model::{SortDir, SortKey, SortSpec};
    use std::fs;
    use std::path::{Path, PathBuf};

    fn scratch(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("gpui-fe-settings-{tag}-{}", std::process::id()))
    }

    #[test]
    fn defaults_are_sane() {
        let settings = Settings::default();
        assert_eq!(settings.version, SCHEMA_VERSION);
        assert_eq!(settings.sort, SortSpec::default());
        assert_eq!(settings.sort.key, SortKey::Name);
        assert_eq!(settings.sort.dir, SortDir::Asc);
        assert!(settings.sort.dirs_first);
        assert!(!settings.show_hidden);
        assert_eq!(settings.view, ViewLayout::Details);
        assert!(settings.last_dir.is_none());
    }

    #[test]
    fn ephemeral_settings_never_write_to_disk() {
        let dir = scratch("ephemeral");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let settings = Settings {
            ephemeral: true,
            ..Settings::default()
        };
        settings.save_to(&path).unwrap();
        assert!(!path.exists(), "ephemeral settings must not create a file");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = scratch("roundtrip");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("settings.json");
        let settings = Settings {
            last_dir: Some(dir.clone()),
            sort: SortSpec {
                key: SortKey::Modified,
                dir: SortDir::Desc,
                dirs_first: true,
            },
            show_hidden: true,
            ..Settings::default()
        };
        settings.save_to(&path).expect("save should succeed");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "settings file must be private");
        }
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded, settings);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn json_shape_matches_design() {
        let dir = scratch("shape");
        let _ = fs::remove_dir_all(&dir);
        let path = dir.join("settings.json");
        let settings = Settings {
            last_dir: Some(PathBuf::from("/home/koppe/document")),
            sort: SortSpec {
                key: SortKey::Size,
                dir: SortDir::Desc,
                dirs_first: true,
            },
            show_hidden: false,
            ..Settings::default()
        };
        settings.save_to(&path).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        for needle in [
            "\"version\": 1",
            "\"last_dir\": \"/home/koppe/document\"",
            "\"sort\"",
            "\"key\": \"size\"",
            "\"dir\": \"desc\"",
            "\"dirs_first\": true",
            "\"show_hidden\": false",
            "\"view\": \"details\"",
        ] {
            assert!(text.contains(needle), "missing {needle} in:\n{text}");
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_from_missing_file_returns_default() {
        let dir = scratch("missing");
        let _ = fs::remove_dir_all(&dir);
        let loaded = Settings::load_from(&dir.join("nope.json"));
        assert_eq!(loaded, Settings::default());
    }

    #[test]
    fn load_from_unreadable_file_stays_read_only() {
        let dir = scratch("unreadable");
        let _ = fs::remove_dir_all(&dir);
        // A directory at the settings path makes read_to_string fail (EISDIR).
        fs::create_dir_all(&dir).unwrap();
        let loaded = Settings::load_from(&dir);
        assert!(loaded.read_only);
        assert!(loaded
            .load_error
            .as_deref()
            .is_some_and(|s| s.contains("読み込めません")));
        // A later save is a no-op and leaves the path untouched.
        loaded.save_to(&dir).unwrap();
        assert!(dir.is_dir());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn load_from_corrupt_file_returns_default_and_backs_up() {
        let dir = scratch("corrupt");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, b"{ not json").unwrap();
        assert_eq!(Settings::load_from(&path), Settings::default());
        // Broken file is preserved under a .broken-* name, not deleted.
        let backups: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.contains("broken"))
            .collect();
        assert_eq!(backups.len(), 1, "expected a .broken-* backup: {backups:?}");
        assert!(
            backups[0].contains(&std::process::id().to_string()),
            "backup should include PID to avoid cross-process collisions: {backups:?}"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn corrupt_backup_failure_keeps_original_and_disables_save() {
        let dir = scratch("corrupt-backup-failure");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, b"{ not json").unwrap();

        // Make the chosen backup path a directory so the rename must fail.
        let backup = Settings::backup_path_for(&path);
        fs::create_dir(&backup).unwrap();
        let loaded = Settings::load_from_with_backup(&path, backup);
        assert!(loaded.read_only, "must enter read-only mode");
        assert!(loaded
            .load_error
            .as_deref()
            .is_some_and(|s| s.contains("退避に失敗")));

        // Original file survives untouched; a later save is a no-op.
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
        loaded.save_to(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "{ not json");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn partial_json_uses_field_defaults() {
        let dir = scratch("partial");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(
            &path,
            br#"{ "show_hidden": true, "sort": { "key": "modified" } }"#,
        )
        .unwrap();
        let loaded = Settings::load_from(&path);
        assert!(loaded.show_hidden);
        assert_eq!(loaded.sort.key, SortKey::Modified);
        assert_eq!(loaded.sort.dir, SortDir::Asc);
        assert_eq!(loaded.version, SCHEMA_VERSION);
        assert_eq!(loaded.view, ViewLayout::Details);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn migration_keeps_v1_values() {
        let dir = scratch("migrate-v1");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(
            &path,
            br#"{ "version": 1, "show_hidden": true, "view": "icons" }"#,
        )
        .unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.version, SCHEMA_VERSION);
        assert!(loaded.show_hidden);
        assert_eq!(loaded.view, ViewLayout::Icons);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn migration_treats_missing_version_as_v1() {
        let mut settings = Settings {
            version: 0,
            ..Settings::default()
        };
        settings.migrate(0);
        assert_eq!(settings.version, SCHEMA_VERSION);
    }

    #[test]
    fn migration_normalizes_future_version_without_dropping_values() {
        let dir = scratch("migrate-future");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(
            &path,
            br#"{ "version": 42, "view": "icons", "show_hidden": true }"#,
        )
        .unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.version, SCHEMA_VERSION);
        assert_eq!(loaded.view, ViewLayout::Icons);
        assert!(loaded.show_hidden);
        // Future schema is read-only so a later save cannot drop unknown fields.
        assert!(loaded.read_only);
        assert!(loaded
            .load_error
            .as_deref()
            .is_some_and(|s| s.contains("v42")));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let dir = scratch("unknown");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        fs::write(&path, br#"{ "future_field": 42, "view": "icons" }"#).unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.view, ViewLayout::Icons);
        assert_eq!(loaded.sort, SortSpec::default());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn validate_drops_vanished_directory() {
        let dir = scratch("validate");
        let _ = fs::remove_dir_all(&dir);
        let mut settings = Settings {
            last_dir: Some(dir.clone()),
            ..Settings::default()
        };
        settings.validate();
        assert!(settings.last_dir.is_none());

        // An existing directory is kept.
        let existing = scratch("validate-existing");
        let _ = fs::remove_dir_all(&existing);
        fs::create_dir_all(&existing).unwrap();
        let mut settings = Settings {
            last_dir: Some(existing.clone()),
            ..Settings::default()
        };
        settings.validate();
        assert_eq!(settings.last_dir, Some(existing.clone()));
        fs::remove_dir_all(&existing).unwrap();
    }

    #[test]
    fn config_path_ends_with_app_dir() {
        if let Some(path) = Settings::config_path() {
            assert!(path.ends_with(Path::new(APP_DIR).join("settings.json")));
        }
    }
}

#[cfg(test)]
mod recent_tests {
    use super::{Settings, MAX_RECENT_DIRS};
    use std::path::PathBuf;

    fn p(value: &str) -> PathBuf {
        PathBuf::from(value)
    }

    #[test]
    fn record_recent_dedupes_and_orders() {
        let mut settings = Settings::default();
        settings.record_recent(&p("/a"));
        settings.record_recent(&p("/b"));
        settings.record_recent(&p("/a"));
        assert_eq!(settings.recent_dirs, vec![p("/a"), p("/b")]);
    }

    #[test]
    fn record_recent_is_bounded() {
        let mut settings = Settings::default();
        for index in 0..(MAX_RECENT_DIRS + 5) {
            settings.record_recent(&p(&format!("/dir-{index}")));
        }
        assert_eq!(settings.recent_dirs.len(), MAX_RECENT_DIRS);
        assert_eq!(
            settings.recent_dirs[0],
            p(&format!("/dir-{}", MAX_RECENT_DIRS + 4))
        );
    }

    #[test]
    fn record_recent_ignores_empty_path() {
        let mut settings = Settings::default();
        settings.record_recent(&p(""));
        assert!(settings.recent_dirs.is_empty());
    }
}

#[cfg(test)]
mod bookmark_tests {
    use super::{Settings, MAX_BOOKMARKS};
    use std::path::PathBuf;

    fn p(value: &str) -> PathBuf {
        PathBuf::from(value)
    }

    #[test]
    fn toggle_adds_and_removes() {
        let mut settings = Settings::default();
        assert!(settings.toggle_bookmark(&p("/a")));
        assert!(settings.is_bookmarked(&p("/a")));
        assert!(!settings.toggle_bookmark(&p("/a")));
        assert!(!settings.is_bookmarked(&p("/a")));
        assert!(settings.bookmarks.is_empty());
    }

    #[test]
    fn toggle_keeps_insertion_order_and_dedupes() {
        let mut settings = Settings::default();
        settings.toggle_bookmark(&p("/b"));
        settings.toggle_bookmark(&p("/a"));
        settings.toggle_bookmark(&p("/b"));
        assert_eq!(settings.bookmarks, vec![p("/a")]);
        settings.toggle_bookmark(&p("/b"));
        assert_eq!(settings.bookmarks, vec![p("/a"), p("/b")]);
    }

    #[test]
    fn bookmarks_are_bounded() {
        let mut settings = Settings::default();
        for index in 0..(MAX_BOOKMARKS + 5) {
            settings.toggle_bookmark(&p(&format!("/dir-{index}")));
        }
        assert_eq!(settings.bookmarks.len(), MAX_BOOKMARKS);
        assert_eq!(settings.bookmarks[0], p("/dir-5"));
    }

    #[test]
    fn empty_path_is_ignored() {
        let mut settings = Settings::default();
        assert!(!settings.toggle_bookmark(&p("")));
        assert!(settings.bookmarks.is_empty());
    }
}

#[cfg(test)]
mod tab_settings_tests {
    use super::Settings;
    use std::path::PathBuf;

    #[test]
    fn tabs_default_to_empty() {
        let settings = Settings::default();
        assert!(settings.tabs.is_empty());
        assert_eq!(settings.active_tab, 0);
    }

    #[test]
    fn tabs_roundtrip_through_json() {
        let settings = Settings {
            tabs: vec![PathBuf::from("/a"), PathBuf::from("/b")],
            active_tab: 1,
            ..Settings::default()
        };
        let text = serde_json::to_string(&settings).unwrap();
        assert!(text.contains("\"tabs\""));
        assert!(text.contains("\"active_tab\":1"));
        let loaded: Settings = serde_json::from_str(&text).unwrap();
        assert_eq!(loaded.tabs, settings.tabs);
        assert_eq!(loaded.active_tab, 1);
    }
}

#[cfg(test)]
mod search_history_tests {
    use super::{Settings, MAX_RECENT_SEARCHES};

    #[test]
    fn records_searches_newest_first_and_deduped() {
        let mut settings = Settings::default();
        settings.record_search("  TODO ");
        settings.record_search("rs");
        settings.record_search("TODO");
        assert_eq!(
            settings.recent_searches,
            vec!["TODO".to_string(), "rs".to_string()]
        );
    }

    #[test]
    fn search_history_is_bounded() {
        let mut settings = Settings::default();
        for index in 0..(MAX_RECENT_SEARCHES + 3) {
            settings.record_search(&format!("query-{index}"));
        }
        assert_eq!(settings.recent_searches.len(), MAX_RECENT_SEARCHES);
        assert_eq!(
            settings.recent_searches[0],
            format!("query-{}", MAX_RECENT_SEARCHES + 2)
        );
    }

    #[test]
    fn empty_search_is_ignored() {
        let mut settings = Settings::default();
        settings.record_search("   ");
        assert!(settings.recent_searches.is_empty());
    }
}
