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
//!   "view": "details"
//! }
//! ```

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::model::SortSpec;

/// Directory name under the platform config dir.
pub const APP_DIR: &str = "gpui-file-explorer";
/// Current settings schema version.
pub const SCHEMA_VERSION: u32 = 1;
/// Maximum number of remembered recent directories.
pub const MAX_RECENT_DIRS: usize = 10;
/// Maximum number of bookmarked directories.
pub const MAX_BOOKMARKS: usize = 50;

/// List view mode: the detailed list or the icon grid. Persisted in
/// `settings.json` as `"details"` / `"icons"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode {
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
    pub view: ViewMode,
    /// Recently visited directories, most recent first (bounded).
    pub recent_dirs: Vec<PathBuf>,
    /// Bookmarked directories, in insertion order (bounded).
    pub bookmarks: Vec<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SCHEMA_VERSION,
            last_dir: None,
            sort: SortSpec::default(),
            show_hidden: false,
            view: ViewMode::Details,
            recent_dirs: Vec::new(),
            bookmarks: Vec::new(),
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

    /// Load settings, falling back to defaults for a missing/corrupt file.
    pub fn load() -> Self {
        match Self::config_path() {
            Some(path) => Self::load_from(&path),
            None => Self::default(),
        }
    }

    pub fn load_from(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        let Ok(mut settings) = serde_json::from_str::<Settings>(&text) else {
            // Keep the broken file for debugging (DESIGN.md §8.1) instead of
            // failing to start.
            let backup =
                path.with_extension(format!("json.broken-{}", chrono::Utc::now().timestamp()));
            let _ = std::fs::rename(path, backup);
            return Self::default();
        };
        settings.version = SCHEMA_VERSION; // future migration hook
        settings.validate();
        settings
    }

    /// Drop a remembered directory that no longer exists.
    pub fn validate(&mut self) {
        if let Some(dir) = &self.last_dir {
            if !dir.is_dir() {
                self.last_dir = None;
            }
        }
    }

    /// Persist to the platform config path. Errors are ignored: settings are
    /// best-effort and must never break the UI.
    pub fn save(&self) {
        if let Some(path) = Self::config_path() {
            let _ = self.save_to(&path);
        }
    }

    /// Atomic write (tmp file + rename) so a crash never leaves a truncated
    /// settings file behind.
    pub fn save_to(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let text = serde_json::to_string_pretty(self).map_err(std::io::Error::other)?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, text)?;
        std::fs::rename(&tmp, path)
    }
}

#[cfg(test)]
mod tests {
    // Explicit imports only: the crate root glob re-exports GPUI's `test`
    // proc-macro which would shadow the built-in `#[test]`.
    use super::{Settings, ViewMode, APP_DIR, SCHEMA_VERSION};
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
        assert_eq!(settings.view, ViewMode::Details);
        assert!(settings.last_dir.is_none());
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
        assert_eq!(loaded.view, ViewMode::Details);
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
        assert_eq!(loaded.view, ViewMode::Icons);
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
