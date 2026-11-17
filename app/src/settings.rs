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
//!   "terminal_command": "konsole",
//!   "recent_dirs": ["/tmp"],
//!   "bookmarks": ["/home/koppe"],
//!   "bookmark_labels": [{ "path": "/home/koppe", "label": "home" }],
//!   "tabs": ["/home/koppe", "/tmp"],
//!   "active_tab": 0,
//!   "recent_searches": ["TODO"],
//!   "saved_searches": [{ "name": "todos", "query": "TODO" }],
//!   "windows": [
//!     {
//!       "bounds": { "x": 80, "y": 60, "width": 1100, "height": 700, "mode": "windowed" },
//!       "tabs": [
//!         { "cwd": "/home/koppe", "back": ["/tmp"], "forward": [] },
//!         { "cwd": "/tmp", "back": [], "forward": [] }
//!       ],
//!       "active_tab": 0
//!     }
//!   ]
//! }
//! ```
//!
//! ## Window scope
//!
//! Session state (frame bounds, tabs and each tab's back/forward history) is
//! **owned by a window**, not by the application: `windows[i]` is the session
//! of the i-th window in creation order. Only one window is opened today, so
//! `windows[0]` is the primary window and is the entry restored at launch. A
//! future second window must append its own entry instead of sharing the
//! primary one; closing a window updates only its own entry. The legacy
//! top-level `tabs` / `active_tab` fields remain as a path-only mirror for
//! older settings files.
//!
//! Untrusted on-disk input is validated in three layers: serde enforces field
//! types, [`Settings::migrate`] classifies the schema version (unknown newer
//! versions are loaded but never written back), and [`Settings::validate`]
//! enforces ranges/caps and drops unusable paths. [`Settings::save_to`] always
//! stamps the current [`SCHEMA_VERSION`] on what it writes.

use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use gpui_kit::{bounds, point, px, size, Window, WindowBounds};

use crate::history::History;
use crate::model::SortSpec;
use crate::tabs::Tab;
// Personalization types live in `model` (GPUI-free) but are re-exported here
// so the UI layer has a single import site for persisted settings state. The
// byte-limit constants are part of that UI-facing surface but unused inside
// this binary until the panels land (same pattern as the `model::error` re-export).
#[allow(unused_imports)]
pub use crate::model::{
    BookmarkLabel, SavedSearch, MAX_BOOKMARK_LABELS, MAX_BOOKMARK_LABEL_BYTES, MAX_SAVED_SEARCHES,
    MAX_SAVED_SEARCH_NAME_BYTES, MAX_SAVED_SEARCH_QUERY_BYTES,
};

use tracing::{error, warn};

/// Directory name under the platform config dir.
pub const APP_DIR: &str = "gpui-file-explorer";
/// Current settings schema version. Written on every save.
pub const SCHEMA_VERSION: u32 = 1;
/// Oldest accepted schema version. A missing `version` field deserializes to
/// 0 and is treated as a legacy v1 file.
pub const LEGACY_SCHEMA_VERSION: u32 = 0;
/// Maximum number of remembered recent directories.
pub const MAX_RECENT_DIRS: usize = 10;
/// Maximum number of bookmarked directories.
pub const MAX_BOOKMARKS: usize = 50;
/// Maximum number of remembered search queries.
pub const MAX_RECENT_SEARCHES: usize = 10;
/// Maximum number of tabs restored from settings.
pub const MAX_TABS: usize = 32;
/// Maximum number of window sessions kept in settings. Each window owns one
/// entry; the primary window is always index 0.
pub const MAX_WINDOW_SESSIONS: usize = 8;
/// `History::max_back` is 200 and back + forward can never exceed it, so the
/// persisted stacks use the same budget.
pub const MAX_HISTORY_ENTRIES: usize = 200;
/// Bounds smaller than this are almost certainly a corrupt/stray value.
pub const MIN_WINDOW_SIZE_PX: f32 = 320.0;
/// Guard against absurd saved sizes (e.g. a corrupt file or 8K-tiled setups).
pub const MAX_WINDOW_SIZE_PX: f32 = 8192.0;
/// Refuse to parse a settings file larger than this (hardening against a
/// crafted file exhausting memory at startup). Oversized files are backed up
/// like corrupt ones.
pub const MAX_SETTINGS_BYTES: u64 = 256 * 1024;
/// Maximum byte length of a single persisted path.
pub const MAX_PATH_BYTES: usize = 4096;
/// Maximum byte length of a persisted search query.
pub const MAX_QUERY_BYTES: usize = 1024;
/// Maximum byte length of the persisted terminal launch command. It is split
/// into argv by the UI, so this is a generous bound for a program + flags.
pub const MAX_TERMINAL_COMMAND_BYTES: usize = 1024;

/// Classification of a persisted schema version before normalization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SchemaStatus {
    /// Legacy (missing/zero) or current version: safe to load and save.
    Supported,
    /// A newer build wrote this file; unknown fields must survive, so the
    /// session is read-only even though the values are kept in memory.
    Future(u32),
}

fn schema_status(loaded_version: u32) -> SchemaStatus {
    if (LEGACY_SCHEMA_VERSION..=SCHEMA_VERSION).contains(&loaded_version) {
        SchemaStatus::Supported
    } else {
        SchemaStatus::Future(loaded_version)
    }
}

/// A persisted path is usable only if it is non-empty, bounded and cannot
/// contain an embedded NUL (which would make every filesystem call fail).
fn sane_path(path: &Path) -> bool {
    let bytes = path.as_os_str().as_encoded_bytes();
    !bytes.is_empty() && bytes.len() <= MAX_PATH_BYTES && !bytes.contains(&0)
}

fn sane_query(query: &str) -> bool {
    !query.is_empty() && query.len() <= MAX_QUERY_BYTES && !query.contains('\0')
}

/// Platform default terminal emulator command.
///
/// Linux targets Kubuntu/KDE, where `konsole` is available. Other platforms
/// have no universally installed emulator, so the action starts disabled
/// there until the user supplies a command. This is also the serde default for
/// older settings files that predate the field (legacy compatibility).
fn default_terminal_command() -> String {
    if cfg!(target_os = "linux") {
        "konsole".to_string()
    } else {
        String::new()
    }
}

/// A persisted terminal command is usable only if it is non-empty, bounded and
/// free of embedded control characters (NUL included). The UI splits it into
/// argv instead of passing it through a shell, so control bytes have no
/// legitimate use here.
fn sane_terminal_command(command: &str) -> bool {
    !command.trim().is_empty()
        && command.len() <= MAX_TERMINAL_COMMAND_BYTES
        && !command.chars().any(char::is_control)
}

fn dedupe_paths(paths: &mut Vec<PathBuf>) {
    let mut seen = HashSet::with_capacity(paths.len());
    paths.retain(|path| seen.insert(path.clone()));
}

fn dedupe_strings(values: &mut Vec<String>) {
    let mut seen = HashSet::with_capacity(values.len());
    values.retain(|value| seen.insert(value.clone()));
}

/// List view mode: the detailed list or the icon grid. Persisted in
/// `settings.json` as `"details"` / `"icons"`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewLayout {
    #[default]
    Details,
    Icons,
}

/// Frame state of a window, persisted alongside its bounds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WindowMode {
    #[default]
    Windowed,
    Maximized,
    Fullscreen,
}

/// Last known frame of one window, in GPUI screen pixels.
///
/// This is window-scoped state: it belongs to the `WindowSession` that owns
/// the window, never to the application as a whole.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct WindowBoundsState {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub mode: WindowMode,
}

impl WindowBoundsState {
    /// Read the current frame from a live window. `None` for an unusable frame
    /// (minimized/headless windows can report degenerate values).
    pub fn from_window(window: &Window) -> Option<Self> {
        let window_bounds = window.window_bounds();
        let frame = window_bounds.get_bounds();
        let mode = match window_bounds {
            WindowBounds::Windowed(_) => WindowMode::Windowed,
            WindowBounds::Maximized(_) => WindowMode::Maximized,
            WindowBounds::Fullscreen(_) => WindowMode::Fullscreen,
        };
        let state = Self {
            x: frame.origin.x.as_f32(),
            y: frame.origin.y.as_f32(),
            width: frame.size.width.as_f32(),
            height: frame.size.height.as_f32(),
            mode,
        };
        state.is_valid().then_some(state)
    }

    /// Reject NaN/infinite coordinates and degenerate sizes from an untrusted
    /// settings file before they reach GPUI's window creation path.
    pub fn is_valid(&self) -> bool {
        let finite = |value: f32| value.is_finite() && value.abs() <= 100_000.0;
        finite(self.x)
            && finite(self.y)
            && self.width.is_finite()
            && self.height.is_finite()
            && self.width >= MIN_WINDOW_SIZE_PX
            && self.height >= MIN_WINDOW_SIZE_PX
            && self.width <= MAX_WINDOW_SIZE_PX
            && self.height <= MAX_WINDOW_SIZE_PX
    }

    /// Convert to GPUI's launch-time bounds, or `None` when the saved value is
    /// unusable (the caller then keeps the previous centered fallback).
    pub fn as_window_bounds(&self) -> Option<WindowBounds> {
        if !self.is_valid() {
            return None;
        }
        let frame = bounds(
            point(px(self.x), px(self.y)),
            size(px(self.width), px(self.height)),
        );
        Some(match self.mode {
            WindowMode::Windowed => WindowBounds::Windowed(frame),
            WindowMode::Maximized => WindowBounds::Maximized(frame),
            WindowMode::Fullscreen => WindowBounds::Fullscreen(frame),
        })
    }
}

/// Back/forward history of a single tab. `back` is oldest-first, `forward` is
/// next-first (the order `History::go_forward` returns entries).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TabSession {
    pub cwd: PathBuf,
    #[serde(default)]
    pub back: Vec<PathBuf>,
    #[serde(default)]
    pub forward: Vec<PathBuf>,
}

impl TabSession {
    /// Snapshot a tab's navigation history using only `History`'s public API
    /// (its stacks are private and deliberately stay encapsulated).
    pub fn from_tab(tab: &Tab) -> Self {
        let probe = PathBuf::from("__gpui_fe_history_probe__");
        let mut back = Vec::new();
        let mut walker = tab.history.clone();
        while let Some(path) = walker.go_back(probe.clone()) {
            back.push(path);
        }
        back.reverse();
        let mut forward = Vec::new();
        let mut walker = tab.history.clone();
        while let Some(path) = walker.go_forward(probe.clone()) {
            forward.push(path);
        }
        Self {
            cwd: tab.cwd.clone(),
            back,
            forward,
        }
    }

    /// Rebuild a `History` with the same back/forward stacks.
    ///
    /// `History` exposes no direct setter, so the stacks are rebuilt from
    /// navigations: `visit` appends to `back`, and `go_back(current)` pops one
    /// `back` entry while pushing the supplied `current` onto `forward`.
    /// Padding entries are appended to `back` and then consumed by `go_back`
    /// with the desired forward path as `current`.
    pub fn to_history(&self) -> History {
        let mut history = History::new();
        let mut back: Vec<PathBuf> = self
            .back
            .iter()
            .filter(|path| sane_path(path))
            .cloned()
            .collect();
        let mut forward: Vec<PathBuf> = self
            .forward
            .iter()
            .filter(|path| sane_path(path))
            .cloned()
            .collect();
        forward.truncate(MAX_HISTORY_ENTRIES);
        back.truncate(MAX_HISTORY_ENTRIES.saturating_sub(forward.len()));

        for window in back.windows(2) {
            history.visit(window[0].clone(), window[1].clone());
        }
        if let Some(last) = back.last() {
            history.visit(last.clone(), self.cwd.clone());
        }

        let mut cursor = self.cwd.clone();
        for index in 0..forward.len() {
            let pad = self.cwd.join(format!("__gpui_fe_pad_{index}__"));
            history.visit(cursor, pad.clone());
            cursor = pad;
        }
        // `go_back` pops the padding and pushes each forward target; because
        // `forward` is next-first, reverse iteration leaves the next target on
        // top of the forward stack.
        for target in forward.iter().rev() {
            history.go_back(target.clone());
        }
        history
    }
}

/// Session of one window: frame plus its tabs and per-tab navigation history.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct WindowSession {
    #[serde(default)]
    pub bounds: Option<WindowBoundsState>,
    #[serde(default)]
    pub tabs: Vec<TabSession>,
    #[serde(default)]
    pub active_tab: usize,
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
    /// Terminal emulator command launched with the selected directory as its
    /// working directory. Empty (or whitespace-only after validation) disables
    /// the action. Added after v1: older files receive the platform default
    /// through the field-level serde default below.
    #[serde(default = "default_terminal_command")]
    pub terminal_command: String,
    /// Recently visited directories, most recent first (bounded).
    pub recent_dirs: Vec<PathBuf>,
    /// Bookmarked directories, in insertion order (bounded).
    pub bookmarks: Vec<PathBuf>,
    /// Labels attached to bookmarked paths, in insertion order (bounded,
    /// unique by path; added in F6-38, optional in older schema files).
    #[serde(default)]
    pub bookmark_labels: Vec<BookmarkLabel>,
    /// Directories of the tabs that were open at exit.
    pub tabs: Vec<PathBuf>,
    /// Index of the active tab in `tabs` (clamped when restoring).
    pub active_tab: usize,
    /// Window-scoped sessions. Index 0 is the primary window; each additional
    /// window owns its own entry (never shared). See the module docs.
    #[serde(default)]
    pub windows: Vec<WindowSession>,
    /// Recently used search queries, most recent first (bounded).
    pub recent_searches: Vec<String>,
    /// Named reusable searches, in insertion order (bounded, unique by name;
    /// added in F6-38, optional in older schema files).
    #[serde(default)]
    pub saved_searches: Vec<SavedSearch>,
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
            terminal_command: default_terminal_command(),
            recent_dirs: Vec::new(),
            bookmarks: Vec::new(),
            bookmark_labels: Vec::new(),
            tabs: Vec::new(),
            active_tab: 0,
            windows: Vec::new(),
            recent_searches: Vec::new(),
            saved_searches: Vec::new(),
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

    /// Session of the primary window (index 0), if any window has been saved.
    pub fn primary_window(&self) -> Option<&WindowSession> {
        self.windows.first()
    }

    /// Mutable session of the primary window; creates the entry on first use.
    /// Other windows must append their own entry instead of using this method.
    pub fn primary_window_mut(&mut self) -> &mut WindowSession {
        if self.windows.is_empty() {
            self.windows.push(WindowSession::default());
        }
        &mut self.windows[0]
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
}

/// F6-38 personalization API. These methods are intentionally not wired into
/// the UI yet; `main.rs` will call them when the saved-search / label views
/// land. `#[allow(dead_code)]` keeps the `-D warnings` gate green until then
/// without weakening the rest of the module.
#[allow(dead_code)]
impl Settings {
    /// Save a named search. A name that already exists (case-insensitively) is
    /// replaced in place; a new name is refused at `MAX_SAVED_SEARCHES`.
    /// Returns `true` when the entry was stored.
    pub fn save_search(&mut self, name: &str, query: &str) -> bool {
        let Some(search) = SavedSearch::new(name, query) else {
            return false;
        };
        if let Some(index) = self
            .saved_searches
            .iter()
            .position(|existing| existing.name_key() == search.name_key())
        {
            self.saved_searches[index] = search;
            return true;
        }
        if self.saved_searches.len() >= MAX_SAVED_SEARCHES {
            return false;
        }
        self.saved_searches.push(search);
        true
    }

    /// Look up a saved search by name (case-insensitive).
    pub fn saved_search(&self, name: &str) -> Option<&SavedSearch> {
        let key = name.trim().to_lowercase();
        self.saved_searches
            .iter()
            .find(|existing| existing.name_key() == key)
    }

    /// Remove a saved search by name (case-insensitive).
    /// Returns `true` when an entry was removed.
    pub fn remove_search(&mut self, name: &str) -> bool {
        let key = name.trim().to_lowercase();
        if key.is_empty() {
            return false;
        }
        if let Some(index) = self
            .saved_searches
            .iter()
            .position(|existing| existing.name_key() == key)
        {
            self.saved_searches.remove(index);
            return true;
        }
        false
    }

    /// Set the label of a bookmarked path. A blank label clears it; an empty
    /// path, oversized label or a new entry past `MAX_BOOKMARK_LABELS` is
    /// refused. Returns `true` when the list changed.
    pub fn set_bookmark_label(&mut self, path: &Path, label: &str) -> bool {
        if !sane_path(path) {
            return false;
        }
        if label.trim().is_empty() {
            return self.remove_bookmark_label(path);
        }
        let Some(entry) = BookmarkLabel::new(path, label) else {
            return false;
        };
        if let Some(index) = self
            .bookmark_labels
            .iter()
            .position(|existing| existing.path == entry.path)
        {
            self.bookmark_labels[index] = entry;
            return true;
        }
        if self.bookmark_labels.len() >= MAX_BOOKMARK_LABELS {
            return false;
        }
        self.bookmark_labels.push(entry);
        true
    }

    /// Label attached to `path`, if any.
    pub fn bookmark_label(&self, path: &Path) -> Option<&str> {
        self.bookmark_labels
            .iter()
            .find(|entry| entry.path == path)
            .map(|entry| entry.label.as_str())
    }

    /// Remove the label of `path`. Returns `true` when one was removed.
    pub fn remove_bookmark_label(&mut self, path: &Path) -> bool {
        if let Some(index) = self
            .bookmark_labels
            .iter()
            .position(|entry| entry.path == path)
        {
            self.bookmark_labels.remove(index);
            return true;
        }
        false
    }
}

/// Terminal-command helpers consumed by the launcher UI (F6-50). They live
/// next to the persistence code so on-disk validation and runtime use share
/// one definition of a "usable command". `#[allow(dead_code)]` keeps the
/// `-D warnings` gate green until the UI task calls them.
#[allow(dead_code)]
impl Settings {
    /// Validate and store a terminal command. Blank input is accepted as
    /// "disabled"; oversized or control-byte-bearing input is refused without
    /// changing the stored value. Returns `true` when the value was stored.
    pub fn set_terminal_command(&mut self, command: &str) -> bool {
        let command = command.trim();
        if !command.is_empty() && !sane_terminal_command(command) {
            return false;
        }
        self.terminal_command = command.to_string();
        true
    }

    /// Split the configured command into argv tokens without invoking a shell.
    /// `None` when the action is disabled or the persisted value is unusable.
    /// The caller passes the target directory via `Command::current_dir`, so a
    /// user-controlled path never becomes part of the command string.
    pub fn terminal_command_argv(&self) -> Option<Vec<String>> {
        if !sane_terminal_command(&self.terminal_command) {
            return None;
        }
        let argv: Vec<String> = self
            .terminal_command
            .split_whitespace()
            .map(str::to_string)
            .collect();
        (!argv.is_empty()).then_some(argv)
    }
}

impl Settings {
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

    /// Move an unusable settings file aside and return defaults. If the rename
    /// fails, stay read-only so a later `save()` cannot overwrite the original
    /// that had to be retained.
    fn back_up_unusable(path: &Path, backup: &Path) -> Self {
        if let Err(error) = std::fs::rename(path, backup) {
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
        Self::default()
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
        // Check the size before reading the whole file into memory: a crafted
        // settings.json must not exhaust memory during startup.
        if let Ok(metadata) = std::fs::metadata(path) {
            if metadata.len() > MAX_SETTINGS_BYTES {
                warn!(
                    size = metadata.len(),
                    limit = MAX_SETTINGS_BYTES,
                    "oversized settings file"
                );
                return Self::back_up_unusable(path, &backup);
            }
        }
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
        // Metadata can be stale/racy, so bound the decoded length as well.
        if text.len() as u64 > MAX_SETTINGS_BYTES {
            warn!(
                size = text.len(),
                limit = MAX_SETTINGS_BYTES,
                "oversized settings file"
            );
            return Self::back_up_unusable(path, &backup);
        }
        // Serde rejects wrong field types outright; `#[serde(default)]` only
        // fills in fields that are absent, so a mistyped `version` or
        // `show_hidden` lands in the backup-and-defaults branch below.
        let Ok(mut settings) = serde_json::from_str::<Settings>(&text) else {
            // Keep the broken file for debugging (DESIGN.md §8.1) instead of
            // failing to start. If the backup rename fails, stay read-only so
            // a later save() cannot overwrite the retained original.
            return Self::back_up_unusable(path, &backup);
        };
        let loaded_version = settings.version;
        settings.migrate(loaded_version);
        settings.validate();
        settings
    }

    /// Convert a deserialized settings file to the current schema.
    ///
    /// `SCHEMA_VERSION` is 1 today. The F6-38 fields (`saved_searches` and
    /// `bookmark_labels`) are additive and carry `#[serde(default)]`, so a v1
    /// file written before they existed loads with both lists empty; no
    /// version bump or data conversion is required. When version 2 is
    /// introduced, add a real transformation step here for versions below it
    /// before the common `version` normalization below. A missing `version`
    /// field deserializes to 0 and is treated as a legacy v1 file for backward
    /// compatibility.
    /// Anything above `SCHEMA_VERSION` is refused for writing: a newer build
    /// may have added fields serde ignores, and writing it back as v1 would
    /// silently drop those settings (second cross-review H-6).
    fn migrate(&mut self, loaded_version: u32) {
        match schema_status(loaded_version) {
            SchemaStatus::Supported => {}
            SchemaStatus::Future(future) => {
                let reason =
                    format!("設定スキーマ v{future} は未対応のため、書き込まずに読み込みました");
                warn!(%reason, "unsupported settings schema");
                self.read_only = true;
                self.load_error = Some(reason);
            }
        }
        self.version = SCHEMA_VERSION;
    }

    /// Enforce value ranges and caps on settings that came from disk: drop
    /// unusable paths, de-duplicate and bound every list, clamp `active_tab`
    /// and drop a `last_dir` that no longer exists.
    pub fn validate(&mut self) {
        if let Some(dir) = &self.last_dir {
            if !sane_path(dir) || !dir.is_dir() {
                self.last_dir = None;
            }
        }

        self.recent_dirs.retain(|path| sane_path(path));
        self.recent_dirs.truncate(MAX_RECENT_DIRS);
        dedupe_paths(&mut self.recent_dirs);

        self.bookmarks.retain(|path| sane_path(path));
        self.bookmarks.truncate(MAX_BOOKMARKS);
        dedupe_paths(&mut self.bookmarks);

        // Personalization lists get the same harden-on-load treatment: drop
        // unusable entries, enforce the cap, then collapse duplicates
        // (case-insensitive name for searches, exact path for labels).
        self.bookmark_labels
            .retain(|entry| sane_path(&entry.path) && entry.is_valid());
        self.bookmark_labels.truncate(MAX_BOOKMARK_LABELS);
        let mut seen_labels = HashSet::with_capacity(self.bookmark_labels.len());
        self.bookmark_labels
            .retain(|entry| seen_labels.insert(entry.path.clone()));

        self.tabs.retain(|path| sane_path(path));
        self.tabs.truncate(MAX_TABS);
        dedupe_paths(&mut self.tabs);
        self.active_tab = self.active_tab.min(self.tabs.len().saturating_sub(1));

        // Window-scoped sessions get the same hardening as the legacy fields.
        // A bogus frame is dropped (the caller falls back to a centered window)
        // instead of reaching GPUI's window creation path.
        self.windows.truncate(MAX_WINDOW_SESSIONS);
        for session in &mut self.windows {
            if session.bounds.is_none_or(|bounds| !bounds.is_valid()) {
                session.bounds = None;
            }
            session.tabs.retain(|tab| sane_path(&tab.cwd));
            session.tabs.truncate(MAX_TABS);
            let mut seen = HashSet::with_capacity(session.tabs.len());
            session.tabs.retain(|tab| seen.insert(tab.cwd.clone()));
            for tab in &mut session.tabs {
                tab.back.retain(|path| sane_path(path));
                tab.forward.retain(|path| sane_path(path));
                tab.forward.truncate(MAX_HISTORY_ENTRIES);
                tab.back.truncate(MAX_HISTORY_ENTRIES - tab.forward.len());
            }
            session.active_tab = session.active_tab.min(session.tabs.len().saturating_sub(1));
        }

        self.recent_searches = std::mem::take(&mut self.recent_searches)
            .into_iter()
            .map(|query| query.trim().to_string())
            .filter(|query| sane_query(query))
            .collect();
        self.recent_searches.truncate(MAX_RECENT_SEARCHES);
        dedupe_strings(&mut self.recent_searches);

        self.saved_searches.retain(|search| search.is_valid());
        self.saved_searches.truncate(MAX_SAVED_SEARCHES);
        let mut seen_names = HashSet::with_capacity(self.saved_searches.len());
        self.saved_searches
            .retain(|search| seen_names.insert(search.name_key()));

        // Terminal command: normalize outer whitespace, then disable the
        // action when the value is blank or unsafe to pass through argv. An
        // unusable value becomes empty rather than backing up the whole file,
        // matching how the other field-level values are hardened on load.
        self.terminal_command = self.terminal_command.trim().to_string();
        if !sane_terminal_command(&self.terminal_command) {
            self.terminal_command.clear();
        }

        // A validated settings value always carries the current version.
        self.version = SCHEMA_VERSION;
    }

    /// Persist to the platform config path. Failure is reported on stderr
    /// (journald on Linux) but never breaks the UI: settings are best-effort.
    /// Persist settings. Returns `false` when a real write failed (read-only
    /// / ephemeral sessions count as success: nothing was expected to be saved).
    pub fn save(&self) -> bool {
        if let Some(path) = Self::config_path() {
            if let Err(error) = self.save_to(&path) {
                error!(path = %path.display(), %error, "failed to save settings");
                return false;
            }
        }
        true
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
        // Stamp the current schema version on the persisted copy even if the
        // in-memory struct was built with a stale/absent version by a caller.
        let mut snapshot = self.clone();
        snapshot.version = SCHEMA_VERSION;
        let text = serde_json::to_string_pretty(&snapshot).map_err(std::io::Error::other)?;
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
    fn save_reports_failure_on_unwritable_path() {
        let dir = scratch("unwritable");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // A directory as the target path makes the final rename fail.
        let settings = Settings::default();
        assert!(settings.save_to(&dir).is_err());
        fs::remove_dir_all(&dir).unwrap();
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

#[cfg(test)]
mod schema_validation_tests {
    use super::{
        schema_status, SchemaStatus, Settings, ViewLayout, MAX_BOOKMARKS, MAX_PATH_BYTES,
        MAX_QUERY_BYTES, MAX_RECENT_DIRS, MAX_RECENT_SEARCHES, MAX_SETTINGS_BYTES, MAX_TABS,
        SCHEMA_VERSION,
    };
    use std::fs;
    use std::path::{Path, PathBuf};

    fn scratch(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("gpui-fe-schema-{tag}-{}", std::process::id()))
    }

    fn setup(tag: &str) -> (PathBuf, PathBuf) {
        let dir = scratch(tag);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        (dir, path)
    }

    fn backup_names(dir: &Path) -> Vec<String> {
        fs::read_dir(dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains("broken"))
            .collect()
    }

    #[test]
    fn schema_status_recognizes_supported_and_future_versions() {
        assert_eq!(schema_status(0), SchemaStatus::Supported);
        assert_eq!(schema_status(SCHEMA_VERSION), SchemaStatus::Supported);
        assert_eq!(
            schema_status(SCHEMA_VERSION + 1),
            SchemaStatus::Future(SCHEMA_VERSION + 1)
        );
    }

    #[test]
    fn oversized_settings_file_is_backed_up_and_defaults_are_used() {
        let (dir, path) = setup("oversized");
        let padding = "x".repeat(MAX_SETTINGS_BYTES as usize + 1);
        fs::write(
            &path,
            format!(r#"{{"version":1,"show_hidden":true,"padding":"{padding}"}}"#),
        )
        .unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded, Settings::default());
        assert!(!path.exists(), "oversized file must be moved aside");
        assert_eq!(backup_names(&dir).len(), 1);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn all_persisted_arrays_are_bounded_on_load() {
        let (dir, path) = setup("array-caps");
        let json = serde_json::json!({
            "version": 1,
            "recent_dirs": (0..MAX_RECENT_DIRS + 5)
                .map(|index| format!("/recent-{index}"))
                .collect::<Vec<_>>(),
            "bookmarks": (0..MAX_BOOKMARKS + 5)
                .map(|index| format!("/bookmark-{index}"))
                .collect::<Vec<_>>(),
            "recent_searches": (0..MAX_RECENT_SEARCHES + 5)
                .map(|index| format!("query-{index}"))
                .collect::<Vec<_>>(),
            "tabs": (0..MAX_TABS + 5)
                .map(|index| format!("/tab-{index}"))
                .collect::<Vec<_>>(),
        });
        fs::write(&path, json.to_string()).unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.recent_dirs.len(), MAX_RECENT_DIRS);
        assert_eq!(loaded.bookmarks.len(), MAX_BOOKMARKS);
        assert_eq!(loaded.recent_searches.len(), MAX_RECENT_SEARCHES);
        assert_eq!(loaded.tabs.len(), MAX_TABS);
        assert_eq!(loaded.active_tab, 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn active_tab_is_clamped_to_restored_tabs() {
        let (dir, path) = setup("active-tab");
        fs::write(
            &path,
            br#"{"version":1,"tabs":["/a","/b"],"active_tab":999}"#,
        )
        .unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.tabs.len(), 2);
        assert_eq!(loaded.active_tab, 1);

        fs::write(&path, br#"{"version":1,"tabs":[],"active_tab":7}"#).unwrap();
        let loaded = Settings::load_from(&path);
        assert!(loaded.tabs.is_empty());
        assert_eq!(loaded.active_tab, 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn mistyped_fields_are_rejected_and_backed_up() {
        for (tag, text) in [
            ("version-string", r#"{"version":"1","show_hidden":true}"#),
            ("version-float", r#"{"version":1.5}"#),
            ("bool-string", r#"{"version":1,"show_hidden":"yes"}"#),
            ("negative-tab", r#"{"version":1,"active_tab":-1}"#),
        ] {
            let (dir, path) = setup(tag);
            fs::write(&path, text).unwrap();
            let loaded = Settings::load_from(&path);
            assert_eq!(loaded, Settings::default(), "accepted {text}");
            assert!(!path.exists());
            assert_eq!(backup_names(&dir).len(), 1);
            fs::remove_dir_all(&dir).unwrap();
        }
    }

    #[test]
    fn future_version_is_kept_read_only_and_never_rewritten() {
        let (dir, path) = setup("future-version");
        let original = br#"{"version":42,"view":"icons","show_hidden":true}"#;
        fs::write(&path, original).unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.version, SCHEMA_VERSION);
        assert_eq!(loaded.view, ViewLayout::Icons);
        assert!(loaded.read_only);
        assert!(loaded
            .load_error
            .as_deref()
            .is_some_and(|reason| reason.contains("v42")));
        loaded.save_to(&path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn legacy_file_without_version_is_accepted_and_upgraded_on_save() {
        let (dir, path) = setup("legacy");
        fs::write(
            &path,
            br#"{"sort":{"key":"modified","dir":"desc","dirs_first":true},"show_hidden":true,"view":"icons"}"#,
        )
        .unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.version, SCHEMA_VERSION);
        assert!(loaded.show_hidden);
        assert_eq!(loaded.view, ViewLayout::Icons);
        assert!(!loaded.read_only);

        let saved = dir.join("saved.json");
        loaded.save_to(&saved).unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&saved).unwrap()).unwrap();
        assert_eq!(value["version"].as_u64(), Some(SCHEMA_VERSION as u64));
        assert_eq!(value["show_hidden"].as_bool(), Some(true));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn save_always_stamps_current_schema_version() {
        let (dir, path) = setup("stamp-version");
        let settings = Settings {
            version: SCHEMA_VERSION + 9,
            ..Settings::default()
        };
        settings.save_to(&path).unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["version"].as_u64(), Some(SCHEMA_VERSION as u64));
        let reloaded = Settings::load_from(&path);
        assert!(!reloaded.read_only);
        assert_eq!(reloaded.version, SCHEMA_VERSION);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unusable_paths_and_queries_are_dropped_on_load() {
        let (dir, path) = setup("unusable-values");
        let long_path = "p".repeat(MAX_PATH_BYTES + 1);
        let long_query = "q".repeat(MAX_QUERY_BYTES + 1);
        let json = serde_json::json!({
            "version": 1,
            "last_dir": "",
            "recent_dirs": ["/ok", "", "bad\u{0}path", long_path],
            "bookmarks": ["/dup", "/dup", "/second"],
            "recent_searches": ["  ping  ", "", long_query, "dup", "dup"],
        });
        fs::write(&path, json.to_string()).unwrap();
        let loaded = Settings::load_from(&path);
        assert!(loaded.last_dir.is_none());
        assert_eq!(loaded.recent_dirs, vec![PathBuf::from("/ok")]);
        assert_eq!(
            loaded.bookmarks,
            vec![PathBuf::from("/dup"), PathBuf::from("/second")]
        );
        assert_eq!(
            loaded.recent_searches,
            vec!["ping".to_string(), "dup".to_string()]
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg(test)]
mod window_session_tests {
    use super::{
        Settings, TabSession, WindowBoundsState, WindowMode, WindowSession, MAX_HISTORY_ENTRIES,
        MAX_WINDOW_SESSIONS,
    };
    use crate::history::History;
    use crate::tabs::Tab;
    use std::path::PathBuf;

    fn p(value: &str) -> PathBuf {
        PathBuf::from(value)
    }

    fn frame() -> WindowBoundsState {
        WindowBoundsState {
            x: 80.0,
            y: 60.0,
            width: 1100.0,
            height: 700.0,
            mode: WindowMode::Windowed,
        }
    }

    #[test]
    fn window_bounds_roundtrip_and_reject_untrusted_values() {
        let saved = frame();
        let window_bounds = saved.as_window_bounds().expect("valid bounds");
        let gpu = window_bounds.get_bounds();
        assert_eq!(gpu.origin.x.as_f32(), 80.0);
        assert_eq!(gpu.origin.y.as_f32(), 60.0);
        assert_eq!(gpu.size.width.as_f32(), 1100.0);
        assert_eq!(gpu.size.height.as_f32(), 700.0);

        let mut bad = frame();
        bad.width = f32::NAN;
        assert!(bad.as_window_bounds().is_none());
        let mut bad = frame();
        bad.height = 10.0;
        assert!(bad.as_window_bounds().is_none());
        let mut bad = frame();
        bad.x = f32::INFINITY;
        assert!(bad.as_window_bounds().is_none());
    }

    #[test]
    fn window_session_survives_save_and_load() {
        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-window-session-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("settings.json");
        let mut settings = Settings::default();
        let primary = settings.primary_window_mut();
        primary.bounds = Some(frame());
        primary.tabs = vec![TabSession {
            cwd: p("/tmp"),
            back: vec![p("/home")],
            forward: vec![],
        }];
        primary.active_tab = 0;
        settings.save_to(&path).unwrap();

        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.primary_window(), settings.primary_window());
        // The primary window is created on demand and stays index 0.
        assert_eq!(loaded.primary_window().unwrap().bounds, Some(frame()));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn tab_session_restores_back_and_forward_order() {
        let mut history = History::new();
        history.visit(p("/a"), p("/b"));
        history.visit(p("/b"), p("/c"));
        history.visit(p("/c"), p("/d"));
        history.go_back(p("/d")); // at /c: back [/a, /b], forward [/d]
        let tab = Tab {
            cwd: p("/c"),
            history,
        };
        let session = TabSession::from_tab(&tab);
        assert_eq!(session.cwd, p("/c"));
        assert_eq!(session.back, vec![p("/a"), p("/b")]);
        assert_eq!(session.forward, vec![p("/d")]);

        let restored = session.to_history();
        let mut walker = restored.clone();
        let mut back = Vec::new();
        while let Some(path) = walker.go_back(p("/current")) {
            back.push(path);
        }
        back.reverse();
        assert_eq!(back, vec![p("/a"), p("/b")]);
        let mut walker = restored;
        let mut forward = Vec::new();
        while let Some(path) = walker.go_forward(p("/current")) {
            forward.push(path);
        }
        assert_eq!(forward, vec![p("/d")]);
    }

    #[test]
    fn tab_session_restores_multiple_forward_entries() {
        let mut history = History::new();
        history.visit(p("/a"), p("/b"));
        history.visit(p("/b"), p("/c"));
        history.go_back(p("/c"));
        history.go_back(p("/b")); // at /a: back [], forward [/b, /c] (next /b)
        let session = TabSession::from_tab(&Tab {
            cwd: p("/a"),
            history,
        });
        assert!(session.back.is_empty());
        assert_eq!(session.forward, vec![p("/b"), p("/c")]);

        let restored = session.to_history();
        let mut walker = restored.clone();
        let mut back = Vec::new();
        while let Some(path) = walker.go_back(p("/current")) {
            back.push(path);
        }
        assert!(back.is_empty(), "padding entries must not leak into back");
        let mut walker = restored;
        let mut forward = Vec::new();
        while let Some(path) = walker.go_forward(p("/current")) {
            forward.push(path);
        }
        assert_eq!(forward, vec![p("/b"), p("/c")]);
    }

    #[test]
    fn window_sessions_are_bounded_and_history_is_capped() {
        let mut settings = Settings::default();
        for index in 0..(MAX_WINDOW_SESSIONS + 3) {
            settings.windows.push(WindowSession {
                bounds: Some(WindowBoundsState {
                    x: index as f32,
                    y: 0.0,
                    width: 900.0,
                    height: 600.0,
                    mode: WindowMode::Windowed,
                }),
                tabs: vec![],
                active_tab: 7,
            });
        }
        settings.validate();
        assert_eq!(settings.windows.len(), MAX_WINDOW_SESSIONS);
        assert_eq!(settings.primary_window().unwrap().active_tab, 0);

        let mut settings = Settings::default();
        settings.primary_window_mut().tabs = vec![TabSession {
            cwd: p("/tmp"),
            back: (0..MAX_HISTORY_ENTRIES + 20)
                .map(|index| p(&format!("/back-{index}")))
                .collect(),
            forward: (0..10)
                .map(|index| p(&format!("/forward-{index}")))
                .collect(),
        }];
        settings.validate();
        let tab = &settings.primary_window().unwrap().tabs[0];
        assert_eq!(tab.forward.len(), 10);
        assert_eq!(tab.back.len(), MAX_HISTORY_ENTRIES - 10);
    }
}

#[cfg(test)]
mod personalization_settings_tests {
    use super::{
        BookmarkLabel, SavedSearch, Settings, MAX_BOOKMARK_LABELS, MAX_BOOKMARK_LABEL_BYTES,
        MAX_SAVED_SEARCHES, SCHEMA_VERSION,
    };
    use std::fs;
    use std::path::{Path, PathBuf};

    fn scratch(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("gpui-fe-personal-{tag}-{}", std::process::id()))
    }

    fn setup(tag: &str) -> (PathBuf, PathBuf) {
        let dir = scratch(tag);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        (dir.clone(), dir.join("settings.json"))
    }

    #[test]
    fn saved_search_upsert_is_case_insensitive_bounded_and_validated() {
        let mut settings = Settings::default();
        assert!(settings.save_search("  TODO ", "  name:rs  "));
        assert_eq!(settings.saved_searches[0].name, "TODO");
        assert_eq!(settings.saved_searches[0].query, "name:rs");
        // Same name in another case replaces in place, keeping the position
        // and adopting the newly submitted spelling.
        assert!(settings.save_search("todo", "ext:rs"));
        settings.save_search("other", "x");
        assert_eq!(settings.saved_searches.len(), 2);
        assert_eq!(settings.saved_searches[0].name, "todo");
        assert_eq!(settings.saved_searches[0].query, "ext:rs");

        // Blank / NUL-bearing values are rejected without touching the list.
        assert!(!settings.save_search("  ", "q"));
        assert!(!settings.save_search("n", "   "));
        assert!(!settings.save_search("bad\0name", "q"));

        for index in 0..MAX_SAVED_SEARCHES {
            settings.save_search(&format!("s-{index}"), "q");
        }
        assert_eq!(settings.saved_searches.len(), MAX_SAVED_SEARCHES);
        assert!(!settings.save_search("one-too-many", "q"));
        // Replacing an existing entry still works at the cap.
        assert!(settings.save_search("s-0", "updated"));
        assert_eq!(settings.saved_search("S-0").unwrap().query, "updated");
    }

    #[test]
    fn saved_search_lookup_remove_and_file_roundtrip() {
        let (dir, path) = setup("saved-search-roundtrip");
        let mut settings = Settings::default();
        settings.save_search("Todos", "TODO");
        assert_eq!(settings.saved_search("todos").unwrap().query, "TODO");
        assert!(settings.saved_search("missing").is_none());

        settings.save_to(&path).unwrap();
        let mut loaded = Settings::load_from(&path);
        assert_eq!(loaded, settings);
        assert!(loaded.saved_search("  TODOS ").is_some());
        assert!(loaded.remove_search("todos"));
        assert!(!loaded.remove_search("todos"));
        assert!(loaded.saved_searches.is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn bookmark_label_set_clear_lookup_and_bounded() {
        let mut settings = Settings::default();
        assert!(!settings.set_bookmark_label(Path::new(""), "home"));
        assert!(!settings.set_bookmark_label(Path::new("/a"), "   "));
        assert!(settings.set_bookmark_label(Path::new("/a"), " home "));
        assert_eq!(settings.bookmark_label(Path::new("/a")), Some("home"));
        // Re-labelling replaces the existing entry instead of appending.
        assert!(settings.set_bookmark_label(Path::new("/a"), "workspace"));
        assert_eq!(settings.bookmark_labels.len(), 1);
        assert_eq!(settings.bookmark_label(Path::new("/a")), Some("workspace"));
        // A blank label clears it and a second removal is a no-op.
        assert!(settings.set_bookmark_label(Path::new("/a"), "  "));
        assert!(settings.bookmark_label(Path::new("/a")).is_none());
        assert!(!settings.remove_bookmark_label(Path::new("/a")));

        for index in 0..MAX_BOOKMARK_LABELS {
            settings.set_bookmark_label(Path::new(&format!("/b-{index}")), "label");
        }
        assert_eq!(settings.bookmark_labels.len(), MAX_BOOKMARK_LABELS);
        assert!(!settings.set_bookmark_label(Path::new("/one-too-many"), "label"));
        assert!(settings.set_bookmark_label(Path::new("/b-0"), "updated"));
        assert_eq!(settings.bookmark_label(Path::new("/b-0")), Some("updated"));
    }

    #[test]
    fn legacy_file_without_personalization_fields_loads_empty_and_saves_new_shape() {
        let (dir, path) = setup("legacy-personalization");
        fs::write(
            &path,
            br#"{"version":1,"show_hidden":true,"recent_searches":["TODO"]}"#,
        )
        .unwrap();
        let loaded = Settings::load_from(&path);
        assert!(!loaded.read_only);
        assert_eq!(loaded.version, SCHEMA_VERSION);
        assert!(loaded.saved_searches.is_empty());
        assert!(loaded.bookmark_labels.is_empty());
        assert_eq!(loaded.recent_searches, vec!["TODO".to_string()]);

        let saved = dir.join("saved.json");
        loaded.save_to(&saved).unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&saved).unwrap()).unwrap();
        assert_eq!(value["saved_searches"].as_array().unwrap().len(), 0);
        assert_eq!(value["bookmark_labels"].as_array().unwrap().len(), 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn personalization_lists_are_validated_on_load() {
        let (dir, path) = setup("personalization-validation");
        let oversized_label = "x".repeat(MAX_BOOKMARK_LABEL_BYTES + 1);
        let json = serde_json::json!({
            "version": 1,
            "saved_searches": [
                { "name": "dup", "query": "keep" },
                { "name": "DUP", "query": "duplicate" },
                { "name": "", "query": "blank" },
                { "name": "bad\u{0}name", "query": "nul" }
            ],
            "bookmark_labels": [
                { "path": "/a", "label": "first" },
                { "path": "/a", "label": "duplicate" },
                { "path": "", "label": "empty-path" },
                { "path": "/b", "label": oversized_label }
            ]
        });
        fs::write(&path, json.to_string()).unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.saved_searches.len(), 1);
        assert_eq!(loaded.saved_searches[0].query, "keep");
        assert_eq!(loaded.bookmark_labels.len(), 1);
        assert_eq!(loaded.bookmark_label(Path::new("/a")), Some("first"));
        fs::remove_dir_all(&dir).unwrap();

        // Caps are enforced by validate() for values built in memory too.
        let mut settings = Settings::default();
        for index in 0..(MAX_SAVED_SEARCHES + 5) {
            settings
                .saved_searches
                .push(SavedSearch::new(&format!("s-{index}"), "q").unwrap());
        }
        for index in 0..(MAX_BOOKMARK_LABELS + 5) {
            settings
                .bookmark_labels
                .push(BookmarkLabel::new(Path::new(&format!("/p-{index}")), "label").unwrap());
        }
        settings.validate();
        assert_eq!(settings.saved_searches.len(), MAX_SAVED_SEARCHES);
        assert_eq!(settings.bookmark_labels.len(), MAX_BOOKMARK_LABELS);
    }

    #[test]
    fn partial_personalization_entries_are_dropped_on_load() {
        let (dir, path) = setup("partial-personalization");
        fs::write(
            &path,
            br#"{"version":1,"saved_searches":[{"name":"x"}],"bookmark_labels":[{"path":"/a"}]}"#,
        )
        .unwrap();
        let loaded = Settings::load_from(&path);
        assert!(loaded.saved_searches.is_empty());
        assert!(loaded.bookmark_labels.is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn future_schema_with_personalization_stays_read_only() {
        let (dir, path) = setup("future-personalization");
        let original = br#"{"version":42,"saved_searches":[{"name":"x","query":"q"}],"bookmark_labels":[{"path":"/a","label":"l"}]}"#;
        fs::write(&path, original).unwrap();
        let loaded = Settings::load_from(&path);
        assert!(loaded.read_only);
        assert_eq!(loaded.saved_searches.len(), 1);
        assert_eq!(loaded.bookmark_labels.len(), 1);
        loaded.save_to(&path).unwrap();
        assert_eq!(fs::read(&path).unwrap(), original);
        fs::remove_dir_all(&dir).unwrap();
    }
}

#[cfg(test)]
mod terminal_command_tests {
    use super::{
        default_terminal_command, sane_terminal_command, Settings, MAX_TERMINAL_COMMAND_BYTES,
        SCHEMA_VERSION,
    };
    use std::fs;
    use std::path::PathBuf;

    fn scratch(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("gpui-fe-terminal-{tag}-{}", std::process::id()))
    }

    fn setup(tag: &str) -> (PathBuf, PathBuf) {
        let dir = scratch(tag);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        (dir, path)
    }

    #[test]
    fn default_is_linux_specific_and_shared_with_serde() {
        let default = default_terminal_command();
        if cfg!(target_os = "linux") {
            assert_eq!(default, "konsole");
            assert!(sane_terminal_command(&default));
        } else {
            assert!(default.is_empty(), "unsupported platforms start disabled");
        }
        assert_eq!(Settings::default().terminal_command, default);
    }

    #[test]
    fn empty_command_disables_and_survives_a_roundtrip() {
        let (dir, path) = setup("disabled");
        let mut settings = Settings::default();
        assert!(settings.set_terminal_command("   "));
        assert!(settings.terminal_command.is_empty());
        assert!(settings.terminal_command_argv().is_none());

        settings.save_to(&path).unwrap();
        let loaded = Settings::load_from(&path);
        assert!(loaded.terminal_command.is_empty());
        assert!(loaded.terminal_command_argv().is_none());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn command_roundtrips_and_splits_into_argv_without_a_shell() {
        let (dir, path) = setup("argv");
        let mut settings = Settings::default();
        assert!(settings.set_terminal_command("  konsole --title build  "));
        assert_eq!(settings.terminal_command, "konsole --title build");
        assert_eq!(
            settings.terminal_command_argv(),
            Some(vec![
                "konsole".to_string(),
                "--title".to_string(),
                "build".to_string(),
            ])
        );

        settings.save_to(&path).unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.terminal_command, "konsole --title build");
        assert_eq!(
            loaded.terminal_command_argv(),
            settings.terminal_command_argv()
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn invalid_commands_are_refused_by_setter_and_disabled_on_load() {
        let mut settings = Settings::default();
        let oversized = "x".repeat(MAX_TERMINAL_COMMAND_BYTES + 1);
        assert!(!settings.set_terminal_command(&oversized));
        assert!(!settings.set_terminal_command("konsole\0--title x"));
        assert!(!settings.set_terminal_command("konsole\nsh"));
        assert_eq!(settings.terminal_command, default_terminal_command());

        let (dir, path) = setup("invalid-load");
        for raw in [
            "bad\0cmd".to_string(),
            oversized,
            "   ".to_string(),
            "konsole\ttab".to_string(),
        ] {
            let json = serde_json::json!({ "version": 1, "terminal_command": raw });
            fs::write(&path, json.to_string()).unwrap();
            let loaded = Settings::load_from(&path);
            assert!(loaded.terminal_command.is_empty(), "accepted {raw:?}");
            assert!(loaded.terminal_command_argv().is_none());
            assert!(!loaded.read_only, "a bad field must not back up the file");
            assert!(path.exists());
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn legacy_file_without_the_field_gets_the_platform_default() {
        let (dir, path) = setup("legacy");
        fs::write(&path, br#"{"version":1,"show_hidden":true,"view":"icons"}"#).unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.version, SCHEMA_VERSION);
        assert_eq!(loaded.terminal_command, default_terminal_command());
        assert!(!loaded.read_only);
        assert!(loaded.show_hidden);

        // Saving materializes the new field in the current schema shape.
        let saved = dir.join("saved.json");
        loaded.save_to(&saved).unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&saved).unwrap()).unwrap();
        assert_eq!(
            value["terminal_command"].as_str(),
            Some(default_terminal_command().as_str())
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn mistyped_command_is_rejected_like_other_wrong_types() {
        let (dir, path) = setup("mistyped");
        fs::write(&path, br#"{"version":1,"terminal_command":42}"#).unwrap();
        assert_eq!(Settings::load_from(&path), Settings::default());
        fs::remove_dir_all(&dir).unwrap();
    }
}
