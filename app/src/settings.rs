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
//!   "theme": "dark",
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
//!
//! An unrecognized persisted enum string (e.g. `"view": "list"` from a newer
//! build) is likewise kept read-only: the field deserializers substitute the
//! default so loading never fails, but [`Settings::load_from`] then marks the
//! session read-only with a [`Settings::load_error`] reason. Saving is a no-op
//! in that state, so the original string is never overwritten by the fallback
//! (which a newer build may understand).

use std::cell::Cell;
use std::collections::HashSet;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use gpui_kit::component::ThemeMode;
use gpui_kit::{bounds, point, px, size, Window, WindowBounds};

use crate::history::{History, HistorySnapshot};
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
/// Runtime tab cap used by the UI. Settings loading deliberately does **not**
/// truncate to this value: silently dropping tabs (and their back/forward
/// history) on startup is data loss. The runtime applies this cap explicitly;
/// settings only log a warning when a loaded file exceeds it.
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
///
/// Derived from the legal data volume, not an arbitrary guess: the runtime
/// keeps up to `MAX_WINDOW_SESSIONS` (8) windows × `MAX_TABS` (32) tabs × 200
/// history paths per tab (back + forward share one `MAX_HISTORY_ENTRIES`
/// budget) ≈ 51,200 paths, plus the legacy top-level lists and metadata. At a
/// realistic ~100 bytes per JSON-escaped path that is ~5 MiB, so 8 MiB leaves
/// headroom. Loading no longer truncates tabs, so this byte limit — not a tab
/// count — is the only bound on a legitimate session; a full but legitimate
/// file must never be backed up as if it were corrupt.
pub const MAX_SETTINGS_BYTES: u64 = 8 * 1024 * 1024;
/// Maximum byte length of a single persisted path.
pub const MAX_PATH_BYTES: usize = 4096;
/// Maximum byte length of a persisted search query.
pub const MAX_QUERY_BYTES: usize = 1024;
/// Maximum byte length of the persisted terminal launch command. It is split
/// into argv by the UI, so this is a generous bound for a program + flags.
pub const MAX_TERMINAL_COMMAND_BYTES: usize = 1024;
/// Maximum number of persisted keymap overrides. The shipped table has fewer
/// than 50 actions, so this leaves headroom without letting a crafted file
/// grow the list without bound.
pub const MAX_KEYMAP_OVERRIDES: usize = 64;
/// Maximum byte length of one keymap action name or chord string (both sides
/// of a `(action, chord)` pair get the same bound).
pub const MAX_CHORD_BYTES: usize = 64;

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

/// A persisted path is usable only if it is non-empty, bounded, free of an
/// embedded NUL (which would make every filesystem call fail) and valid UTF-8.
/// serde's `PathBuf` serializer cannot encode non-UTF-8 paths, so accepting one
/// here would make every subsequent `save_to` fail.
fn sane_path(path: &Path) -> bool {
    let bytes = path.as_os_str().as_encoded_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_PATH_BYTES
        && !bytes.contains(&0)
        && path.to_str().is_some()
}

fn sane_query(query: &str) -> bool {
    !query.is_empty() && query.len() <= MAX_QUERY_BYTES && !query.contains('\0')
}

/// Platform default terminal emulator command.
///
/// Linux targets Kubuntu/KDE, where `konsole` is available. Other platforms
/// have no universally installed emulator, so the action starts disabled
/// there until the user supplies a command.
///
/// This value is used by `Settings::default()` (which the Alt+T dialog uses to
/// prefill an empty editor) and by tests. It is deliberately **not** the serde
/// default for a missing `terminal_command` key: an absent/blank key loads as
/// the portable empty "unset" value, while a value the user explicitly wrote —
/// even if it happens to equal this platform default — is preserved verbatim.
fn default_terminal_command() -> String {
    if cfg!(target_os = "linux") {
        "konsole".to_string()
    } else {
        String::new()
    }
}

/// Persisted theme default. Older settings files predate the `theme` field, so
/// they get the app's historical dark appearance via this field-level default
/// (the container `#[serde(default)]` alone would pick `ThemeMode::Light`).
fn default_theme() -> ThemeMode {
    ThemeMode::Dark
}

std::thread_local! {
    /// Cross-deserializer signal that an unrecognized persisted enum string
    /// was replaced by a fallback on this thread. Every field-level enum
    /// deserializer (here and in `model`) reports through
    /// [`note_unknown_persisted_enum`]; [`Settings::load_from_with_backup`]
    /// clears it before parsing and consumes it afterwards so the loaded
    /// session can be kept read-only. Not persisted and never shared across
    /// threads.
    static UNKNOWN_ENUM_SEEN: Cell<bool> = const { Cell::new(false) };
}

/// Record that a persisted enum string is not recognized by this build.
///
/// The field deserializers still substitute a safe default so one bad value
/// cannot back up the whole file, but [`Settings::load_from_with_backup`] uses
/// this signal to make the session read-only: writing the fallback back would
/// silently destroy the original value.
pub(crate) fn note_unknown_persisted_enum() {
    UNKNOWN_ENUM_SEEN.with(|seen| seen.set(true));
}

/// Clear and return the signal for the current load. Called once before
/// deserialization (to drop any stray signal from a direct `from_str` call)
/// and once after it.
fn take_unknown_persisted_enum() -> bool {
    UNKNOWN_ENUM_SEEN.with(|seen| seen.replace(false))
}

/// Deserialize a persisted theme, falling back to [`default_theme`] for any
/// unknown name instead of rejecting the whole settings file.
///
/// `ThemeMode` comes from gpui-component and only has `light` / `dark`; a value
/// written by a newer build (e.g. `"system"`) must not cause
/// [`Settings::back_up_unusable`] to discard every other setting. This mirrors
/// the `#[serde(other)]` fallback used for locally-defined enums without
/// touching the external crate. The fallback also marks the load read-only via
/// [`note_unknown_persisted_enum`], so the unknown string is never rewritten.
fn deserialize_theme<'de, D>(deserializer: D) -> Result<ThemeMode, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(match value.as_str() {
        "dark" => ThemeMode::Dark,
        "light" => ThemeMode::Light,
        _ => {
            note_unknown_persisted_enum();
            default_theme()
        }
    })
}

/// Serde default for a missing `version` key. An unversioned file predates
/// schema stamping, so it must classify as [`LEGACY_SCHEMA_VERSION`] (0) rather
/// than the current [`SCHEMA_VERSION`]; otherwise a future v1→v2 migration
/// would be skipped for exactly the oldest settings files. The container-level
/// `#[serde(default)]` still supplies the other fields' defaults, and
/// `Settings::default()` explicitly carries `SCHEMA_VERSION`.
fn legacy_schema_version() -> u32 {
    LEGACY_SCHEMA_VERSION
}

/// Pure dark/light cycle used by the `toggle_theme` action.
///
/// DESIGN.md §8.2 asks for `dark | light | system`, but gpui-component 0.9.1
/// `ThemeMode` has only `Light` and `Dark` (no `System` variant), so "system"
/// cannot be represented and is intentionally not offered; see `app/README.md`.
pub fn next_theme(current: ThemeMode) -> ThemeMode {
    match current {
        ThemeMode::Dark => ThemeMode::Light,
        ThemeMode::Light => ThemeMode::Dark,
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

/// Whether a chord's key is one of the keys reserved for dialog ownership.
///
/// Only the final `+`-separated segment is the key: `f10` / `f12` contain the
/// substring "f1" but are not the reserved F1 dialog key. Comparison is
/// case-insensitive. This is the single definition shared by settings loading
/// and the UI's keymap dialog.
pub fn is_reserved_chord(chord: &str) -> bool {
    let lowered = chord.to_ascii_lowercase();
    let key = lowered.rsplit('+').next().unwrap_or(lowered.as_str());
    key == "f1" || key == "escape"
}

/// Normalize one persisted `(action_name, chord)` pair. Both sides are trimmed
/// and bounded, and a chord that mentions a dialog-ownership key (`escape` or
/// `f1`, see [`is_reserved_chord`]) is rejected so a bad file cannot take away
/// a way out of a dialog. Unknown action names are deliberately kept:
/// `KeyMap::from_settings` ignores names that are not in the shipped table.
fn sane_keymap_entry(action: &str, chord: &str) -> Option<(String, String)> {
    let action = action.trim();
    let chord = chord.trim();
    let sane = |value: &str| {
        !value.is_empty() && value.len() <= MAX_CHORD_BYTES && !value.chars().any(char::is_control)
    };
    if !sane(action) || !sane(chord) || is_reserved_chord(chord) {
        return None;
    }
    Some((action.to_string(), chord.to_string()))
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

/// Deserialize a persisted view layout, mapping unknown names (e.g. a future
/// `"list"`) to the default instead of backing up the entire settings file.
/// Known values are matched explicitly so only a genuinely unknown string
/// triggers [`note_unknown_persisted_enum`] and makes the load read-only.
fn deserialize_view_layout<'de, D>(deserializer: D) -> Result<ViewLayout, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(match value.as_str() {
        "details" => ViewLayout::Details,
        "icons" => ViewLayout::Icons,
        _ => {
            note_unknown_persisted_enum();
            ViewLayout::default()
        }
    })
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

/// Unknown persisted mode falls back to Windowed so one bad enum value cannot
/// back up the whole settings file. As with the other enum deserializers, the
/// unknown string is reported via [`note_unknown_persisted_enum`] so the
/// session stays read-only and the original value survives on disk.
fn deserialize_window_mode<'de, D>(deserializer: D) -> Result<WindowMode, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(match value.as_str() {
        "windowed" => WindowMode::Windowed,
        "maximized" => WindowMode::Maximized,
        "fullscreen" => WindowMode::Fullscreen,
        _ => {
            note_unknown_persisted_enum();
            WindowMode::default()
        }
    })
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
    /// Persisted frame mode. An unknown string falls back to `Windowed` and
    /// keeps the load read-only so the original value survives on disk.
    #[serde(deserialize_with = "deserialize_window_mode")]
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
    /// Snapshot a tab's navigation history through `History`'s snapshot API.
    /// The returned stacks use the persisted order: `back` oldest-first,
    /// `forward` next-first.
    pub fn from_tab(tab: &Tab) -> Self {
        let snapshot = tab.history.snapshot();
        Self {
            cwd: tab.cwd.clone(),
            back: snapshot.back,
            forward: snapshot.forward,
        }
    }

    /// Rebuild a `History` with the same back/forward stacks.
    ///
    /// Paths that serde cannot persist are dropped and the combined budget is
    /// enforced here, at the settings boundary, so the stored representation
    /// stays identical to the one `validate()` accepts on load. The stacks are
    /// then installed directly through `History::restore` — no synthetic
    /// navigation is needed.
    pub fn to_history(&self) -> History {
        let mut forward: Vec<PathBuf> = self
            .forward
            .iter()
            .filter(|path| sane_path(path))
            .cloned()
            .collect();
        forward.truncate(MAX_HISTORY_ENTRIES);
        let mut back: Vec<PathBuf> = self
            .back
            .iter()
            .filter(|path| sane_path(path))
            .cloned()
            .collect();
        back.truncate(MAX_HISTORY_ENTRIES.saturating_sub(forward.len()));

        let mut history = History::new();
        history.restore(HistorySnapshot { back, forward });
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
    /// Persisted schema version. A missing key means a legacy pre-versioning
    /// file and deserializes to [`LEGACY_SCHEMA_VERSION`] (0), so `migrate()`
    /// can still apply older migrations once `SCHEMA_VERSION` is bumped.
    #[serde(default = "legacy_schema_version")]
    pub version: u32,
    /// Last successfully listed directory. Ignored if it no longer exists.
    pub last_dir: Option<PathBuf>,
    pub sort: SortSpec,
    pub show_hidden: bool,
    /// Unknown values deserialize to [`ViewLayout::default`] so a single bad
    /// string cannot cost the whole settings file. The load is then read-only
    /// (see the module docs), so saving cannot replace the original string.
    #[serde(deserialize_with = "deserialize_view_layout")]
    pub view: ViewLayout,
    /// Component theme applied at startup and toggled at runtime. Added after
    /// v1: older files get `Dark` through the field-level serde default above.
    /// Only `dark` / `light` are representable in gpui-component 0.9.1; an
    /// unknown name (e.g. `"system"`) falls back to `Dark` and keeps the load
    /// read-only so the original name is not rewritten on save.
    #[serde(default = "default_theme", deserialize_with = "deserialize_theme")]
    pub theme: ThemeMode,
    /// Terminal emulator command launched with the selected directory as its
    /// working directory. Empty (or whitespace-only after validation) disables
    /// the action. Added after v1: a missing key loads as the portable empty
    /// "unset" value, while an explicit value is preserved exactly as written
    /// — including when it equals the current platform default.
    #[serde(default)]
    pub terminal_command: String,
    /// User keymap overrides as `(action_name, chord)` pairs, e.g.
    /// `("open_preview", "ctrl+shift+p")`. A chord is a `+`-joined list of
    /// `ctrl` / `shift` / `alt` modifiers (any order) with the key last.
    /// Added after v1: older files get an empty list via the serde default.
    #[serde(default)]
    pub keymap: Vec<(String, String)>,
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
    /// Set when a corrupt settings file could not be backed up, the schema is
    /// newer than this build, or an enum value is unrecognized. Prevents a
    /// subsequent `save()` from overwriting the retained original (with
    /// defaults, a downgraded schema, or a substituted enum fallback).
    /// Not persisted.
    #[serde(skip)]
    pub read_only: bool,
    /// Human-readable reason for `read_only` or an unavailable config location,
    /// shown in the status bar.
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
            theme: default_theme(),
            terminal_command: default_terminal_command(),
            keymap: Vec::new(),
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
        // Only persist paths serde can serialize: a non-UTF-8 path would make
        // every later save() fail.
        if !sane_path(path) {
            return;
        }
        self.recent_dirs.retain(|existing| existing != path);
        self.recent_dirs.insert(0, path.to_path_buf());
        self.recent_dirs.truncate(MAX_RECENT_DIRS);
    }

    /// Whether `path` can be written to the settings file at all.
    ///
    /// Public form of [`sane_path`]: callers that mutate bookmark/session state
    /// must consult this *before* changing anything, otherwise a path serde
    /// cannot serialise (e.g. non-UTF-8) would enter memory and break every
    /// later `save_to`.
    pub fn can_persist_path(path: &Path) -> bool {
        sane_path(path)
    }

    /// Whether `path` is bookmarked.
    pub fn is_bookmarked(&self, path: &std::path::Path) -> bool {
        self.bookmarks.iter().any(|bookmark| bookmark == path)
    }

    /// Add or remove `path` from the bookmarks. Returns `true` when it was
    /// added, `false` when it was removed (or the path was empty).
    pub fn toggle_bookmark(&mut self, path: &std::path::Path) -> bool {
        if !sane_path(path) {
            return false;
        }
        if let Some(index) = self.bookmarks.iter().position(|bookmark| bookmark == path) {
            self.bookmarks.remove(index);
            // Drop the label together with its bookmark. Otherwise re-adding
            // the same path would silently revive the stale label.
            self.remove_bookmark_label(path);
            return false;
        }
        self.bookmarks.push(path.to_path_buf());
        if self.bookmarks.len() > MAX_BOOKMARKS {
            // Eviction is a removal just like the explicit branch above: drop
            // the evicted path's label too, otherwise re-adding that path would
            // silently revive the stale label.
            let evicted = self.bookmarks.remove(0);
            self.remove_bookmark_label(&evicted);
        }
        true
    }

    /// Remember a search query (trimmed, de-duplicated, bounded).
    pub fn record_search(&mut self, query: &str) {
        let query = query.trim();
        // Share the load-time rule: a query that `validate()` would drop must
        // never enter the persisted history in the first place. `sane_query`
        // also covers the empty case after trimming.
        if !sane_query(query) {
            return;
        }
        self.recent_searches.retain(|existing| existing != query);
        self.recent_searches.insert(0, query.to_string());
        self.recent_searches.truncate(MAX_RECENT_SEARCHES);
    }
}

/// F6-38 personalization API, wired into the `main.rs` UI: the saved-search bar
/// calls `save_search` / `saved_search` / `remove_search`, and bookmark labels
/// call `set_bookmark_label` / `bookmark_label` / `remove_bookmark_label`.
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

    /// Ensure `path` has a bookmark so a label attached to it survives the
    /// `validate()` pass on load (which drops orphan labels). Returns `true`
    /// when a bookmark was added. This is the side-effect-free counterpart to
    /// the low-level `set_bookmark_label`: the UI calls it before assigning a
    /// label to a directory that is not bookmarked yet.
    pub fn ensure_bookmark_for_label(&mut self, path: &Path) -> bool {
        if self.is_bookmarked(path) {
            false
        } else {
            self.toggle_bookmark(path)
        }
    }

    /// Set the label of a path. A blank label clears it; an empty path,
    /// oversized label or a new entry past `MAX_BOOKMARK_LABELS` is refused.
    /// Returns `true` when the list changed.
    ///
    /// This low-level setter deliberately does not require `path` to be
    /// bookmarked (the UI layer owns that invariant). An orphan label works in
    /// memory, but `validate()` — which runs on every load — drops labels whose
    /// path is not bookmarked. Bookmark a path first when its label must
    /// survive a save/reload cycle; [`Self::ensure_bookmark_for_label`] is the
    /// UI-facing helper for that.
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

/// Terminal-command helpers consumed by the launcher UI (F6-50) and the
/// Alt+T settings dialog. They live next to the persistence code so on-disk
/// validation and runtime use share one definition of a "usable command".
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
        let path = Self::config_path();
        Self::load_from_optional_path(path.as_deref())
    }

    /// `load()` with an injectable config path so the "no config directory"
    /// branch is testable without unsetting HOME. A missing config dir is not
    /// the same as a fresh install: nothing could be read and nothing can be
    /// written, so report why instead of silently handing out defaults.
    fn load_from_optional_path(path: Option<&Path>) -> Self {
        match path {
            Some(path) => Self::load_from(path),
            None => {
                let reason = "設定ディレクトリを特定できないため、設定を読み込めませんでした。書き込みは行いません";
                error!(%reason, "settings config directory unavailable");
                Self {
                    load_error: Some(reason.to_string()),
                    ..Self::default()
                }
            }
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
        // `read_to_string` allocates the whole file before a post-hoc size
        // check, so a TOCTOU size race could still exhaust memory. Read through
        // `take` instead: at most MAX_SETTINGS_BYTES + 1 bytes can be allocated.
        let read_failed = |error: std::io::Error| -> Self {
            let reason = format!(
                "設定ファイルを読み込めませんでした（{}: {error}）。書き込みは行いません",
                path.display()
            );
            error!(%reason, "settings read failed");
            Self {
                read_only: true,
                load_error: Some(reason),
                ..Self::default()
            }
        };
        let mut bytes = Vec::new();
        let read = std::fs::File::open(path).and_then(|file| {
            use std::io::Read;
            file.take(MAX_SETTINGS_BYTES + 1).read_to_end(&mut bytes)
        });
        if let Err(error) = read {
            // A fresh install has no settings file yet; defaults are fine.
            if error.kind() == std::io::ErrorKind::NotFound {
                return Self::default();
            }
            // EACCES, EISDIR and other read failures must not be overwritten by
            // a later save() with defaults (third cross-review F6-04).
            return read_failed(error);
        }
        // Metadata can be stale/racy, so enforce the limit on the bytes actually
        // read as well. The extra byte above only serves as an oversize sentinel
        // and is never parsed.
        if bytes.len() as u64 > MAX_SETTINGS_BYTES {
            warn!(
                size = bytes.len(),
                limit = MAX_SETTINGS_BYTES,
                "oversized settings file"
            );
            return Self::back_up_unusable(path, &backup);
        }
        // Invalid UTF-8 is corruption, not a transient read error: preserve the
        // original under a backup name and start from defaults.
        let text = match String::from_utf8(bytes) {
            Ok(text) => text,
            Err(_) => {
                warn!(path = %path.display(), "settings file is not valid UTF-8");
                return Self::back_up_unusable(path, &backup);
            }
        };
        // Serde rejects wrong field types outright; `#[serde(default)]` only
        // fills in fields that are absent, so a mistyped `version` or
        // `show_hidden` lands in the backup-and-defaults branch below.
        // The enum field deserializers report unknown strings through a
        // thread-local signal. Clear any stale value first (a direct
        // `serde_json::from_str::<Settings>` elsewhere on this thread may have
        // set it) and consume the result immediately after parsing.
        let _ = take_unknown_persisted_enum();
        let Ok(mut settings) = serde_json::from_str::<Settings>(&text) else {
            // Keep the broken file for debugging (DESIGN.md §8.1) instead of
            // failing to start. If the backup rename fails, stay read-only so
            // a later save() cannot overwrite the retained original.
            return Self::back_up_unusable(path, &backup);
        };
        let unknown_enum = take_unknown_persisted_enum();
        let loaded_version = settings.version;
        settings.migrate(loaded_version);
        settings.validate();
        if unknown_enum && !settings.read_only {
            // An unknown enum was replaced by a fallback in memory. Writing
            // that fallback would permanently destroy the original string,
            // which a newer build may still understand. Reuse the existing
            // read-only mechanism so every save() becomes a no-op and the
            // status bar can explain why via `load_error`.
            let reason = "設定にこのバージョンでは解釈できない値が含まれています。元の値を保持するため書き込みは行いません";
            warn!(%reason, "unknown persisted enum; settings are read-only");
            settings.read_only = true;
            settings.load_error = Some(reason.to_string());
        }
        settings
    }

    /// Convert a deserialized settings file to the current schema.
    ///
    /// `SCHEMA_VERSION` is 1 today. The F6-38 fields (`saved_searches` and
    /// `bookmark_labels`) and the later `keymap` field are additive and carry
    /// `#[serde(default)]`, so a v1 file written before they existed loads with
    /// those lists empty; no version bump or data conversion is required.
    /// When version 2 is introduced, add a real transformation step here for
    /// versions below it before the common `version` normalization below. A
    /// missing `version`
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

        // De-duplicate before truncating: with MAX copies of one path followed
        // by a unique path, truncating first would discard the unique entry.
        self.recent_dirs.retain(|path| sane_path(path));
        dedupe_paths(&mut self.recent_dirs);
        self.recent_dirs.truncate(MAX_RECENT_DIRS);

        self.bookmarks.retain(|path| sane_path(path));
        dedupe_paths(&mut self.bookmarks);
        self.bookmarks.truncate(MAX_BOOKMARKS);

        // Normalize persisted personalization text *before* validating it.
        // `is_valid()` (via `sane_personal_text`) checks the raw byte length, so
        // a hand-edited value padded with whitespace could exceed the cap raw
        // while being perfectly valid once trimmed. Trimming here keeps such an
        // entry instead of silently dropping it on load. `recent_searches` gets
        // the same treatment just below.
        for search in &mut self.saved_searches {
            search.name = search.name.trim().to_owned();
            search.query = search.query.trim().to_owned();
        }
        for entry in &mut self.bookmark_labels {
            entry.label = entry.label.trim().to_owned();
        }

        // Personalization lists get the same harden-on-load treatment: drop
        // unusable entries, collapse duplicates (case-insensitive name for
        // searches, exact path for labels), then enforce the cap.
        // A label belongs to a bookmark: dropping labels whose path is no
        // longer bookmarked prevents a stale label from being revived if the
        // path is bookmarked again later (mirrors `toggle_bookmark`). This runs
        // before the cap so orphan entries cannot crowd out live labels.
        self.bookmark_labels
            .retain(|entry| self.bookmarks.contains(&entry.path));
        self.bookmark_labels
            .retain(|entry| sane_path(&entry.path) && entry.is_valid());
        let mut seen_labels = HashSet::with_capacity(self.bookmark_labels.len());
        self.bookmark_labels
            .retain(|entry| seen_labels.insert(entry.path.clone()));
        self.bookmark_labels.truncate(MAX_BOOKMARK_LABELS);

        // Keep every path as-is. The runtime allows several tabs with the same
        // cwd, each with its own back/forward history, so de-duplicating by
        // path here would silently destroy a tab on the next restart. Nor do
        // we truncate to MAX_TABS on load: the runtime cap is applied by the
        // UI, and load-time truncation would lose tab history without telling
        // the user.
        self.tabs.retain(|path| sane_path(path));
        if self.tabs.len() > MAX_TABS {
            warn!(
                count = self.tabs.len(),
                limit = MAX_TABS,
                "settings contain more tabs than the runtime cap; keeping all of them"
            );
        }
        self.active_tab = self.active_tab.min(self.tabs.len().saturating_sub(1));

        // Window-scoped sessions get the same hardening as the legacy fields.
        // A bogus frame is dropped (the caller falls back to a centered window)
        // instead of reaching GPUI's window creation path.
        self.windows.truncate(MAX_WINDOW_SESSIONS);
        for session in &mut self.windows {
            if session.bounds.is_none_or(|bounds| !bounds.is_valid()) {
                session.bounds = None;
            }
            // Same rule as the legacy top-level tabs: repeated cwds are legal
            // (distinct histories) and no tab is dropped just for exceeding
            // MAX_TABS. Only unusable paths are filtered.
            session.tabs.retain(|tab| sane_path(&tab.cwd));
            if session.tabs.len() > MAX_TABS {
                warn!(
                    count = session.tabs.len(),
                    limit = MAX_TABS,
                    "window session contains more tabs than the runtime cap; keeping all of them"
                );
            }
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
        dedupe_strings(&mut self.recent_searches);
        self.recent_searches.truncate(MAX_RECENT_SEARCHES);

        self.saved_searches.retain(|search| search.is_valid());
        let mut seen_names = HashSet::with_capacity(self.saved_searches.len());
        self.saved_searches
            .retain(|search| seen_names.insert(search.name_key()));
        self.saved_searches.truncate(MAX_SAVED_SEARCHES);

        // Terminal command: normalize outer whitespace, then disable the
        // action when the value is blank or unsafe to pass through argv. An
        // unusable value becomes empty rather than backing up the whole file,
        // matching how the other field-level values are hardened on load.
        // Explicit values are never normalized to "unset": a user who chose
        // exactly the platform default still expects the launcher to work
        // after a restart. Only an absent/blank key loads as unset.
        self.terminal_command = self.terminal_command.trim().to_string();
        if !sane_terminal_command(&self.terminal_command) {
            self.terminal_command.clear();
        }

        // Keymap overrides get the same harden-on-load treatment: trim both
        // sides, drop unusable/reserved entries, cap the count and keep only
        // the first override per action AND per chord. Two actions bound to
        // the same chord would make key resolution depend on iteration order,
        // so a later entry with a duplicate chord is dropped. Unknown action
        // names are kept; the application ignores them when building the map.
        let mut keymap = Vec::new();
        let mut seen_actions = HashSet::new();
        let mut seen_chords = HashSet::new();
        for (action, chord) in std::mem::take(&mut self.keymap) {
            if keymap.len() >= MAX_KEYMAP_OVERRIDES {
                break;
            }
            let Some((action, chord)) = sane_keymap_entry(&action, &chord) else {
                continue;
            };
            // Reserve the chord first: a duplicate chord must not reserve the
            // action, otherwise a later (valid) rebind of that action to a
            // different chord would be dropped.
            if !seen_chords.insert(chord.clone()) {
                continue;
            }
            if seen_actions.insert(action.clone()) {
                keymap.push((action, chord));
            } else {
                // The chord was reserved speculatively but the action is
                // already bound, so release it for a later entry.
                seen_chords.remove(&chord);
            }
        }
        self.keymap = keymap;

        // A validated settings value always carries the current version.
        self.version = SCHEMA_VERSION;
    }

    /// Persist settings to the platform config path. Returns `false` when no
    /// config directory can be determined or a real write failed. Read-only /
    /// ephemeral sessions still count as success: nothing was expected to be
    /// saved, and `save_to()` keeps handling those cases itself. Failure is
    /// reported on stderr (journald on Linux) but never breaks the UI.
    pub fn save(&self) -> bool {
        let path = Self::config_path();
        self.save_to_optional_path(path.as_deref())
    }

    /// `save()` with an injectable path so the "no config directory" branch is
    /// testable without unsetting HOME.
    fn save_to_optional_path(&self, path: Option<&Path>) -> bool {
        let Some(path) = path else {
            if self.ephemeral || self.read_only {
                return true;
            }
            error!("設定ディレクトリを特定できないため、設定を保存できません");
            return false;
        };
        if let Err(error) = self.save_to(path) {
            error!(path = %path.display(), %error, "failed to save settings");
            return false;
        }
        true
    }

    /// Serialize concurrent saves from multiple app instances. A lock file
    /// created with O_EXCL is atomic on every supported platform; a stale lock
    /// older than 60 s is reclaimed so a crashed process cannot block saves
    /// forever. The lock is advisory — atomic rename already prevents a
    /// truncated settings file — but it prevents last-writer-wins surprise.
    fn acquire_save_lock(path: &Path) -> std::io::Result<PathBuf> {
        use std::io::ErrorKind;
        let lock = path.with_extension("json.lock");
        for _ in 0..50 {
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock)
            {
                Ok(file) => {
                    drop(file);
                    return Ok(lock);
                }
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                    let stale = std::fs::metadata(&lock)
                        .ok()
                        .and_then(|meta| meta.modified().ok())
                        .and_then(|time| time.elapsed().ok())
                        .map(|age| age > std::time::Duration::from_secs(60))
                        .unwrap_or(false);
                    if stale {
                        let _ = std::fs::remove_file(&lock);
                        continue;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::new(
            ErrorKind::WouldBlock,
            "settings save lock timeout",
        ))
    }

    /// Drop paths serde cannot serialize from a *copy* of the settings before
    /// writing. `sane_path()` already guards the setters and `validate()` on
    /// load, but live paths can enter unchecked: `TabSession::from_tab()`
    /// copies `tab.cwd` verbatim and `main.rs` stores the current directory
    /// directly. A single non-UTF-8 path would make every later `save_to()`
    /// fail, so filter it at the last point before serialization.
    ///
    /// This mirrors only the path filtering of `validate()`: no existence
    /// checks, de-duplication, truncation, migration or `read_only` changes,
    /// and it is applied to the snapshot so in-memory state is untouched.
    fn sanitize_paths_for_save(settings: &mut Settings) {
        if settings
            .last_dir
            .as_deref()
            .is_some_and(|dir| !sane_path(dir))
        {
            settings.last_dir = None;
        }
        settings.recent_dirs.retain(|path| sane_path(path));
        settings.bookmarks.retain(|path| sane_path(path));
        settings
            .bookmark_labels
            .retain(|entry| sane_path(&entry.path));
        settings.tabs.retain(|path| sane_path(path));
        for window in &mut settings.windows {
            window.tabs.retain(|tab| sane_path(&tab.cwd));
            for tab in &mut window.tabs {
                tab.back.retain(|path| sane_path(path));
                tab.forward.retain(|path| sane_path(path));
            }
        }
    }

    /// Atomic write (tmp file + rename) so a crash never leaves a truncated
    /// settings file behind. The tmp file is flushed with `sync_all()` before
    /// the rename; a directory fsync is not performed (best-effort).
    ///
    /// Read-only / ephemeral sessions are a successful no-op. If serialization
    /// would exceed [`MAX_SETTINGS_BYTES`], this returns an error without
    /// creating any file: a file over the cap would be backed up as corrupt on
    /// the next load, turning an oversized-but-valid session into total loss.
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
        let lock = Self::acquire_save_lock(path)?;
        let result = (|| -> std::io::Result<()> {
            // Stamp the current schema version on the persisted copy even if
            // the in-memory struct was built with a stale/absent version.
            let mut snapshot = self.clone();
            snapshot.version = SCHEMA_VERSION;
            // Persist the command exactly as the user chose it. Previously an
            // explicit `konsole` on Linux was rewritten to "" here, silently
            // disabling terminal launch after the next restart. An unset field
            // is already empty in memory (the missing-key serde default), so
            // nothing platform-specific is invented or carried to another OS.
            Self::sanitize_paths_for_save(&mut snapshot);
            let text = serde_json::to_string_pretty(&snapshot).map_err(std::io::Error::other)?;
            // `load_from` refuses files above this limit (and backs them up),
            // so writing one here would destroy the user's whole session on
            // the next start. Fail the save instead; never create the file and
            // let the caller surface STATUS_SETTINGS_SAVE_FAILED.
            if text.len() as u64 > MAX_SETTINGS_BYTES {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "serialized settings are {} bytes, over the {MAX_SETTINGS_BYTES}-byte limit",
                        text.len()
                    ),
                ));
            }
            // O_EXCL reservation: a predictable tmp name with `create(true)`
            // follows a pre-planted symlink (CWE-59). `create_new(true)`
            // refuses an existing path, including symlinks. Nanos + PID keep
            // stale temp files from earlier runs from blocking normal saves.
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
            let written = (|| -> std::io::Result<()> {
                file.write_all(text.as_bytes())?;
                file.sync_all()
            })();
            drop(file);
            // A failed write AND a failed rename share the same cleanup
            // branch: a leftover tmp file would block later saves because
            // the reservation uses `create_new(true)`.
            if let Err(error) = written.and_then(|()| std::fs::rename(&tmp, path)) {
                let _ = std::fs::remove_file(&tmp);
                return Err(error);
            }
            Ok(())
        })();
        if let Err(error) = std::fs::remove_file(&lock) {
            // Without this log a failed release looks like a 60-second stale
            // lock on every later save, with no clue why it was not removed.
            warn!(path = %lock.display(), %error, "failed to release settings save lock");
        }
        result
    }
}

#[cfg(test)]
mod tests {
    // Explicit imports only: the crate root glob re-exports GPUI's `test`
    // proc-macro which would shadow the built-in `#[test]`.
    use super::{
        next_theme, Settings, ViewLayout, APP_DIR, LEGACY_SCHEMA_VERSION, MAX_SETTINGS_BYTES,
        SCHEMA_VERSION,
    };
    use crate::model::{SortDir, SortKey, SortSpec};
    use gpui_kit::component::ThemeMode;
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
        assert_eq!(settings.theme, ThemeMode::Dark);
        assert!(settings.last_dir.is_none());
    }

    #[test]
    fn missing_version_deserializes_to_legacy_zero_before_migration() {
        assert_eq!(LEGACY_SCHEMA_VERSION, 0);
        // The container-level `#[serde(default)]` must not stamp the current
        // schema onto an unversioned file: `migrate()` needs to see v0 so it
        // can run older migrations once SCHEMA_VERSION is bumped.
        let parsed: Settings = serde_json::from_str(r#"{}"#).unwrap();
        assert_eq!(parsed.version, LEGACY_SCHEMA_VERSION);
        let partial: Settings = serde_json::from_str(r#"{"show_hidden":true}"#).unwrap();
        assert_eq!(partial.version, LEGACY_SCHEMA_VERSION);
        // An explicit version is still honored before migration.
        let explicit: Settings = serde_json::from_str(r#"{"version":1}"#).unwrap();
        assert_eq!(explicit.version, 1);
        // The programmatic default remains the current schema.
        assert_eq!(Settings::default().version, SCHEMA_VERSION);
    }

    #[test]
    fn theme_defaults_to_dark_even_in_older_settings_files() {
        let loaded = Settings::load_from(&scratch("theme-default-missing.json"));
        // No file exists yet at this path, so the field-level serde default
        // (not `ThemeMode`'s own Light default) must supply dark.
        assert_eq!(loaded.theme, ThemeMode::Dark);

        let parsed: Settings = serde_json::from_str(r#"{"version":1,"view":"icons"}"#).unwrap();
        assert_eq!(parsed.theme, ThemeMode::Dark);
        let explicit: Settings = serde_json::from_str(r#"{"version":1,"theme":"light"}"#).unwrap();
        assert_eq!(explicit.theme, ThemeMode::Light);
    }

    #[test]
    fn next_theme_cycles_between_the_two_supported_modes() {
        assert_eq!(next_theme(ThemeMode::Dark), ThemeMode::Light);
        assert_eq!(next_theme(ThemeMode::Light), ThemeMode::Dark);
        assert_eq!(
            next_theme(next_theme(ThemeMode::Dark)),
            ThemeMode::Dark,
            "cycle must return to the starting mode"
        );
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
    fn oversized_serialized_settings_are_never_written() {
        let dir = scratch("oversized-save");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        // Each query is individually valid (<= MAX_QUERY_BYTES), but enough of
        // them push the serialized document past the load-time cap. Writing
        // such a file would make the next load back it up as corrupt and lose
        // the whole session, so save_to must fail before creating any file.
        let settings = Settings {
            recent_searches: (0..9_000)
                .map(|index| format!("{index:08}-{}", "q".repeat(1_000)))
                .collect(),
            ..Settings::default()
        };
        assert!(
            serde_json::to_string(&settings).unwrap().len() as u64 > MAX_SETTINGS_BYTES,
            "test data must actually exceed the cap"
        );
        let error = settings
            .save_to(&path)
            .expect_err("an oversized save must fail");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        assert!(
            !path.exists(),
            "an unreadable settings file must never be written"
        );
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            leftovers.is_empty(),
            "no tmp/lock/backup file should survive: {leftovers:?}"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn missing_config_dir_reports_a_load_error() {
        // Inject `None` instead of unsetting HOME: the helper is the exact
        // branch `load()` takes when `config_dir()` is unavailable.
        let settings = Settings::load_from_optional_path(None);
        assert_eq!(settings.version, SCHEMA_VERSION);
        assert!(settings.last_dir.is_none());
        let reason = settings
            .load_error
            .as_deref()
            .expect("an unavailable config dir must be reported");
        assert!(!reason.is_empty());
        assert!(reason.contains("設定"));
    }

    #[test]
    fn missing_config_dir_fails_to_save_except_when_nothing_was_expected() {
        // A normal session cannot save anywhere -> real failure.
        assert!(!Settings::default().save_to_optional_path(None));
        // Read-only / ephemeral sessions keep their "nothing to save" success
        // semantics even when no path is available.
        let read_only = Settings {
            read_only: true,
            ..Settings::default()
        };
        assert!(read_only.save_to_optional_path(None));
        let ephemeral = Settings {
            ephemeral: true,
            ..Settings::default()
        };
        assert!(ephemeral.save_to_optional_path(None));

        // The injectable `Some` path still performs a real write.
        let dir = scratch("optional-save");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        assert!(Settings::default().save_to_optional_path(Some(&path)));
        assert!(path.exists());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn save_removes_tmp_file_when_rename_fails() {
        let dir = scratch("tmp-cleanup");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        // A directory as the target path makes the final rename fail, while
        // keeping the tmp file inside `dir` where the test can inspect it.
        let target = dir.join("settings.json");
        fs::create_dir(&target).unwrap();
        let settings = Settings::default();
        assert!(settings.save_to(&target).is_err());
        let leftovers: Vec<_> = fs::read_dir(&dir)
            .unwrap()
            .filter_map(|entry| entry.ok())
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.contains("json.tmp-"))
            .collect();
        assert!(
            leftovers.is_empty(),
            "no json.tmp-* file should survive a failed rename: {leftovers:?}"
        );
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
    fn save_releases_lock_file() {
        let dir = scratch("save-lock");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("settings.json");
        let lock = path.with_extension("json.lock");
        let settings = Settings::default();
        settings.save_to(&path).unwrap();
        assert!(!lock.exists(), "lock must be released after save");
        // A second save must also succeed (no stale lock left behind).
        settings.save_to(&path).unwrap();
        assert!(!lock.exists());
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
            // Explicit commands are persisted verbatim (including when they
            // equal the platform default), so this checks plain round-trip
            // fidelity rather than a normalization rule.
            terminal_command: "custom-terminal --title build".to_string(),
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
    fn removing_bookmark_drops_its_label() {
        let mut settings = Settings::default();
        assert!(settings.toggle_bookmark(&p("/a")));
        assert!(settings.set_bookmark_label(&p("/a"), "home"));
        assert_eq!(settings.bookmark_label(&p("/a")), Some("home"));

        // Removing the bookmark also removes its label.
        assert!(!settings.toggle_bookmark(&p("/a")));
        assert!(settings.bookmark_labels.is_empty());
        assert!(settings.bookmark_label(&p("/a")).is_none());

        // Re-adding the same path starts clean: the old label is not revived.
        assert!(settings.toggle_bookmark(&p("/a")));
        assert!(settings.is_bookmarked(&p("/a")));
        assert!(settings.bookmark_label(&p("/a")).is_none());
    }

    #[test]
    fn removing_bookmark_keeps_unrelated_labels() {
        let mut settings = Settings::default();
        settings.toggle_bookmark(&p("/a"));
        settings.toggle_bookmark(&p("/b"));
        assert!(settings.set_bookmark_label(&p("/a"), "home"));
        assert!(settings.set_bookmark_label(&p("/b"), "workspace"));

        settings.toggle_bookmark(&p("/a"));

        assert_eq!(settings.bookmarks, vec![p("/b")]);
        assert_eq!(settings.bookmark_labels.len(), 1);
        assert!(settings.bookmark_label(&p("/a")).is_none());
        assert_eq!(settings.bookmark_label(&p("/b")), Some("workspace"));
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
    fn evicting_a_bookmark_drops_its_label() {
        let mut settings = Settings::default();
        assert!(settings.toggle_bookmark(&p("/a")));
        assert!(settings.set_bookmark_label(&p("/a"), "home"));
        assert_eq!(settings.bookmark_label(&p("/a")), Some("home"));

        // Filling past the cap evicts the oldest entry (/a). Its label must go
        // with it, exactly as in the explicit-removal branch.
        for index in 0..MAX_BOOKMARKS {
            assert!(settings.toggle_bookmark(&p(&format!("/dir-{index}"))));
        }
        assert_eq!(settings.bookmarks.len(), MAX_BOOKMARKS);
        assert!(!settings.is_bookmarked(&p("/a")));
        assert!(settings.bookmark_label(&p("/a")).is_none());

        // Re-adding the evicted path must not revive the stale label.
        assert!(settings.toggle_bookmark(&p("/a")));
        assert!(settings.is_bookmarked(&p("/a")));
        assert!(settings.bookmark_label(&p("/a")).is_none());
    }

    #[test]
    fn empty_path_is_ignored() {
        let mut settings = Settings::default();
        assert!(!settings.toggle_bookmark(&p("")));
        assert!(settings.bookmarks.is_empty());
    }

    #[test]
    fn ensure_bookmark_for_label_creates_owner_only_when_missing() {
        let mut settings = Settings::default();
        let path = p("/work/project");

        // No bookmark yet: the helper adds one so the label has an owner.
        assert!(settings.ensure_bookmark_for_label(&path));
        assert!(settings.is_bookmarked(&path));

        // Already bookmarked: no duplicate bookmark is created.
        assert!(!settings.ensure_bookmark_for_label(&path));
        assert_eq!(settings.bookmarks.len(), 1);

        // An unusable path cannot be bookmarked; the helper is a no-op.
        assert!(!settings.ensure_bookmark_for_label(&p("")));
        assert_eq!(settings.bookmarks.len(), 1);
    }

    #[test]
    fn label_on_auto_bookmarked_path_survives_validate() {
        let mut settings = Settings::default();
        let path = p("/work/project");

        // The UI path: ensure a bookmark owner first, then set the label.
        assert!(settings.ensure_bookmark_for_label(&path));
        assert!(settings.set_bookmark_label(&path, "work"));
        assert_eq!(settings.bookmark_label(&path), Some("work"));

        // validate() (run on every load) keeps labels whose path is bookmarked.
        settings.validate();
        assert!(settings.is_bookmarked(&path));
        assert_eq!(settings.bookmark_label(&path), Some("work"));
    }

    #[test]
    fn validate_dedupes_before_truncating_so_later_paths_survive() {
        let mut settings = Settings {
            bookmarks: vec![p("/a"); MAX_BOOKMARKS],
            ..Settings::default()
        };
        settings.bookmarks.push(p("/b"));

        settings.validate();

        assert_eq!(settings.bookmarks, vec![p("/a"), p("/b")]);
    }
}

#[cfg(test)]
mod tab_settings_tests {
    use super::{Settings, TabSession, MAX_TABS};
    use std::path::PathBuf;

    #[test]
    fn tabs_default_to_empty() {
        let settings = Settings::default();
        assert!(settings.tabs.is_empty());
        assert_eq!(settings.active_tab, 0);
    }

    #[test]
    fn validate_keeps_tabs_with_the_same_cwd_and_their_history() {
        // The runtime allows several tabs on the same directory with
        // independent back/forward stacks; de-duplicating by cwd used to lose
        // a whole tab (and its history) on every restart.
        let mut settings = Settings {
            tabs: vec![PathBuf::from("/same"), PathBuf::from("/same")],
            active_tab: 1,
            windows: vec![super::WindowSession {
                tabs: vec![
                    TabSession {
                        cwd: PathBuf::from("/same"),
                        back: vec![PathBuf::from("/old-a")],
                        forward: Vec::new(),
                    },
                    TabSession {
                        cwd: PathBuf::from("/same"),
                        back: vec![PathBuf::from("/old-b")],
                        forward: vec![PathBuf::from("/next-b")],
                    },
                ],
                active_tab: 1,
                ..super::WindowSession::default()
            }],
            ..Settings::default()
        };

        settings.validate();

        assert_eq!(settings.tabs, vec![PathBuf::from("/same"); 2]);
        assert_eq!(settings.active_tab, 1);
        let window_tabs = &settings.primary_window().unwrap().tabs;
        assert_eq!(window_tabs.len(), 2);
        assert_eq!(window_tabs[0].back, vec![PathBuf::from("/old-a")]);
        assert_eq!(window_tabs[1].back, vec![PathBuf::from("/old-b")]);
        assert_eq!(window_tabs[1].forward, vec![PathBuf::from("/next-b")]);
        assert_eq!(settings.primary_window().unwrap().active_tab, 1);
    }

    #[test]
    fn validate_does_not_truncate_tabs_to_the_runtime_cap() {
        let many = MAX_TABS + 7;
        let mut settings = Settings {
            tabs: (0..many)
                .map(|i| PathBuf::from(format!("/tab-{i}")))
                .collect(),
            windows: vec![super::WindowSession {
                tabs: (0..many)
                    .map(|i| TabSession {
                        cwd: PathBuf::from(format!("/window-tab-{i}")),
                        back: Vec::new(),
                        forward: Vec::new(),
                    })
                    .collect(),
                ..super::WindowSession::default()
            }],
            ..Settings::default()
        };

        settings.validate();

        assert_eq!(settings.tabs.len(), many);
        assert_eq!(settings.primary_window().unwrap().tabs.len(), many);
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
    use super::{Settings, MAX_QUERY_BYTES, MAX_RECENT_SEARCHES};

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

    #[test]
    fn invalid_search_is_not_recorded() {
        let mut settings = Settings::default();
        settings.record_search("keep");
        // Same rule as `validate()`/`sane_query`: over the byte cap or NUL.
        settings.record_search(&"x".repeat(MAX_QUERY_BYTES + 1));
        settings.record_search("bad\u{0}query");
        assert_eq!(settings.recent_searches, vec!["keep".to_string()]);
    }
}

#[cfg(test)]
mod schema_validation_tests {
    use super::{
        schema_status, BookmarkLabel, SavedSearch, SchemaStatus, Settings, TabSession, ViewLayout,
        WindowSession, MAX_BOOKMARKS, MAX_BOOKMARK_LABELS, MAX_HISTORY_ENTRIES, MAX_PATH_BYTES,
        MAX_QUERY_BYTES, MAX_RECENT_DIRS, MAX_RECENT_SEARCHES, MAX_SAVED_SEARCHES,
        MAX_SETTINGS_BYTES, MAX_TABS, MAX_WINDOW_SESSIONS, SCHEMA_VERSION,
    };
    use crate::model::{SortDir, SortKey, SortSpec};
    use gpui_kit::component::ThemeMode;
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
    fn large_but_legal_settings_roundtrip_without_backup() {
        let (dir, path) = setup("large-legal");
        // Maximum legal volume: 8 windows x 32 tabs x 200 history paths per
        // tab. With ~90-byte paths this is ~5 MB once JSON-escaped: bigger
        // than the old 256 KiB limit, but still below MAX_SETTINGS_BYTES.
        let suffix = "x".repeat(60);
        let windows: Vec<_> = (0..MAX_WINDOW_SESSIONS)
            .map(|w| {
                let tabs: Vec<_> = (0..MAX_TABS)
                    .map(|t| {
                        let back: Vec<_> = (0..MAX_HISTORY_ENTRIES)
                            .map(|h| format!("/tmp/gpui-fe-large/w{w}/t{t}/h{h}/{suffix}"))
                            .collect();
                        serde_json::json!({
                            "cwd": format!("/tmp/gpui-fe-large/w{w}/t{t}/{suffix}"),
                            "back": back,
                            "forward": [],
                        })
                    })
                    .collect();
                serde_json::json!({"tabs": tabs, "active_tab": 0})
            })
            .collect();
        let raw = serde_json::json!({"version": SCHEMA_VERSION, "windows": windows}).to_string();
        assert!(raw.len() > 256 * 1024, "file is only {} bytes", raw.len());
        assert!((raw.len() as u64) < MAX_SETTINGS_BYTES);
        fs::write(&path, &raw).unwrap();

        let loaded = Settings::load_from(&path);
        assert!(!loaded.read_only, "legal data must not be read-only");
        assert!(loaded.load_error.is_none());
        assert!(
            backup_names(&dir).is_empty(),
            "a legal full-size file must never be backed up"
        );
        assert_eq!(loaded.windows.len(), MAX_WINDOW_SESSIONS);
        for window in &loaded.windows {
            assert_eq!(window.tabs.len(), MAX_TABS);
            for tab in &window.tabs {
                assert_eq!(tab.back.len(), MAX_HISTORY_ENTRIES);
            }
        }

        let saved = dir.join("saved.json");
        loaded.save_to(&saved).unwrap();
        let reloaded = Settings::load_from(&saved);
        assert!(!reloaded.read_only);
        assert_eq!(reloaded, loaded, "a legal full-size file must round-trip");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn invalid_utf8_settings_file_is_backed_up() {
        let (dir, path) = setup("invalid-utf8");
        fs::write(&path, [b'{', b'"', 0xff, b'"', b':', b'1', b'}']).unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded, Settings::default());
        assert!(!path.exists(), "corrupt file must be moved aside");
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
        // Tabs are intentionally not capped on load: the runtime owns that
        // cap, and truncating here would silently destroy tab history.
        assert_eq!(loaded.tabs.len(), MAX_TABS + 5);
        assert_eq!(loaded.active_tab, 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn dedupe_precedes_truncation_for_every_bounded_list() {
        let mut settings = Settings {
            recent_dirs: vec![PathBuf::from("/a"); MAX_RECENT_DIRS],
            recent_searches: vec!["dup".to_string(); MAX_RECENT_SEARCHES],
            saved_searches: vec![SavedSearch::new("dup", "q").unwrap(); MAX_SAVED_SEARCHES],
            bookmarks: vec![PathBuf::from("/l"), PathBuf::from("/m")],
            bookmark_labels: vec![
                BookmarkLabel::new(Path::new("/l"), "dup").unwrap();
                MAX_BOOKMARK_LABELS
            ],
            ..Settings::default()
        };
        settings.recent_dirs.push(PathBuf::from("/b"));
        settings.recent_searches.push("unique".to_string());
        settings
            .saved_searches
            .push(SavedSearch::new("unique", "q").unwrap());
        settings
            .bookmark_labels
            .push(BookmarkLabel::new(Path::new("/m"), "unique").unwrap());

        settings.validate();

        assert_eq!(
            settings.recent_dirs,
            vec![PathBuf::from("/a"), PathBuf::from("/b")]
        );
        assert_eq!(
            settings.recent_searches,
            vec!["dup".to_string(), "unique".to_string()]
        );
        assert_eq!(settings.saved_searches.len(), 2);
        assert_eq!(settings.saved_searches[1].name, "unique");
        assert_eq!(settings.bookmark_labels.len(), 2);
        assert_eq!(settings.bookmark_label(Path::new("/m")), Some("unique"));
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

    #[cfg(unix)]
    #[test]
    fn non_utf8_paths_are_rejected_and_dropped_before_save() {
        use std::os::unix::ffi::OsStringExt;

        use super::sane_path;

        // A path whose bytes are not valid UTF-8 cannot be serialized by serde;
        // one such entry would break every later settings save.
        let bad = PathBuf::from(std::ffi::OsString::from_vec(vec![
            b'/', b't', b'm', b'p', b'/', 0xff,
        ]));
        assert!(!sane_path(&bad), "non-UTF-8 paths must be refused");
        assert!(
            !Settings::can_persist_path(&bad),
            "the public guard must agree with sane_path"
        );
        assert!(Settings::can_persist_path(Path::new("/tmp/valid")));
        let mut via_setters = Settings::default();
        via_setters.record_recent(&bad);
        assert!(via_setters.recent_dirs.is_empty());
        assert!(!via_setters.toggle_bookmark(&bad));
        assert!(via_setters.bookmarks.is_empty());

        let (dir, path) = setup("non-utf8-path");
        let mut settings = Settings {
            recent_dirs: vec![bad.clone(), PathBuf::from("/ok")],
            bookmarks: vec![bad.clone()],
            ..Settings::default()
        };
        settings.validate();
        assert!(!settings.recent_dirs.contains(&bad));
        assert!(!settings.bookmarks.contains(&bad));
        assert_eq!(settings.recent_dirs, vec![PathBuf::from("/ok")]);
        settings
            .save_to(&path)
            .expect("save must succeed once bad paths are dropped");
        fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn save_sanitizes_non_utf8_tab_paths_without_mutating_memory() {
        use std::os::unix::ffi::OsStringExt;

        use super::sane_path;

        // `TabSession::from_tab()` copies a live tab's cwd unchecked, so a
        // directory opened through the OS dialog can put this path in memory.
        let bad = PathBuf::from(std::ffi::OsString::from_vec(vec![
            b'/', b't', b'm', b'p', b'/', 0xff,
        ]));
        assert!(!sane_path(&bad));
        let good = PathBuf::from("/tmp/valid");

        let (dir, path) = setup("non-utf8-tab-cwd");
        let settings = Settings {
            last_dir: Some(bad.clone()),
            recent_dirs: vec![bad.clone(), good.clone()],
            bookmarks: vec![bad.clone(), good.clone()],
            bookmark_labels: vec![BookmarkLabel {
                path: bad.clone(),
                label: "bad".to_string(),
            }],
            tabs: vec![bad.clone()],
            windows: vec![WindowSession {
                tabs: vec![
                    TabSession {
                        cwd: bad.clone(),
                        back: vec![bad.clone()],
                        forward: vec![bad.clone()],
                    },
                    TabSession {
                        cwd: good.clone(),
                        back: vec![bad.clone(), good.clone()],
                        forward: vec![bad.clone(), good.clone()],
                    },
                ],
                ..WindowSession::default()
            }],
            ..Settings::default()
        };

        // Serde cannot encode 0xff: before save-time sanitization this failed
        // for every later save, including ones unrelated to the bad tab.
        settings.save_to(&path).expect("save must succeed");

        // Only the serialized snapshot is sanitized; in-memory state survives.
        assert_eq!(settings.last_dir.as_deref(), Some(bad.as_path()));
        assert!(settings.recent_dirs.contains(&bad));
        assert!(settings.bookmarks.contains(&bad));
        assert_eq!(settings.bookmark_labels[0].path, bad);
        assert_eq!(settings.tabs, vec![bad.clone()]);
        assert_eq!(settings.windows[0].tabs[0].cwd, bad);

        let loaded = Settings::load_from(&path);
        assert!(loaded.last_dir.is_none());
        assert_eq!(loaded.recent_dirs, vec![good.clone()]);
        assert_eq!(loaded.bookmarks, vec![good.clone()]);
        assert!(loaded.bookmark_labels.is_empty());
        assert!(loaded.tabs.is_empty());
        let tabs = &loaded.windows[0].tabs;
        assert_eq!(tabs.len(), 1, "the tab with a bad cwd must be dropped");
        assert_eq!(tabs[0].cwd, good);
        assert_eq!(tabs[0].back, vec![good.clone()]);
        assert_eq!(tabs[0].forward, vec![good.clone()]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_view_value_falls_back_and_keeps_the_file_read_only() {
        let (dir, path) = setup("unknown-view");
        fs::write(
            &path,
            serde_json::json!({
                "version": 1,
                "view": "list",
                "show_hidden": true,
                "theme": "light",
                "terminal_command": "konsole --title build",
                "bookmarks": ["/home/koppe"],
                "tabs": ["/a", "/a"],
            })
            .to_string(),
        )
        .unwrap();

        let loaded = Settings::load_from(&path);
        assert!(
            loaded.read_only,
            "an unknown enum must make the session read-only so the original survives"
        );
        assert!(loaded
            .load_error
            .is_some_and(|reason| reason.contains("設定")));
        assert!(path.exists(), "the original settings file must survive");
        assert_eq!(loaded.view, ViewLayout::Details);
        assert!(loaded.show_hidden);
        assert_eq!(loaded.theme, ThemeMode::Light);
        assert_eq!(loaded.terminal_command, "konsole --title build");
        assert_eq!(loaded.bookmarks, vec![PathBuf::from("/home/koppe")]);
        assert_eq!(loaded.tabs, vec![PathBuf::from("/a"); 2]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_window_mode_falls_back_and_keeps_the_file_read_only() {
        let (dir, path) = setup("unknown-window-mode");
        fs::write(
            &path,
            serde_json::json!({
                "version": 1,
                "view": "icons",
                "show_hidden": true,
                "windows": [{
                    "tabs": [],
                    "active_tab": 0,
                    "bounds": {
                        "x": 0.0, "y": 0.0, "width": 800.0,
                        "height": 600.0, "mode": "minimized"
                    }
                }]
            })
            .to_string(),
        )
        .unwrap();

        let loaded = Settings::load_from(&path);
        assert!(
            loaded.read_only,
            "unknown mode must keep the load read-only"
        );
        assert!(loaded.load_error.is_some());
        assert!(path.exists());
        assert_eq!(loaded.view, ViewLayout::Icons);
        assert!(loaded.show_hidden);
        assert_eq!(
            loaded
                .primary_window()
                .unwrap()
                .bounds
                .as_ref()
                .unwrap()
                .mode,
            super::WindowMode::Windowed
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_sort_key_falls_back_and_keeps_the_file_read_only() {
        let (dir, path) = setup("unknown-sort-key");
        fs::write(
            &path,
            serde_json::json!({
                "version": 1,
                "sort": { "key": "date", "dir": "desc", "dirs_first": false },
                "show_hidden": true,
                "view": "icons",
                "bookmarks": ["/home/koppe"],
            })
            .to_string(),
        )
        .unwrap();

        let loaded = Settings::load_from(&path);
        assert!(
            loaded.read_only,
            "unknown sort key must keep the load read-only"
        );
        assert!(loaded.load_error.is_some());
        assert!(path.exists());
        assert_eq!(loaded.sort.key, SortKey::Name);
        // Sibling fields of the same struct, and unrelated settings, survive.
        assert_eq!(loaded.sort.dir, SortDir::Desc);
        assert!(!loaded.sort.dirs_first);
        assert_eq!(
            loaded.sort,
            SortSpec {
                key: SortKey::Name,
                dir: SortDir::Desc,
                dirs_first: false,
            }
        );
        assert!(loaded.show_hidden);
        assert_eq!(loaded.view, ViewLayout::Icons);
        assert_eq!(loaded.bookmarks, vec![PathBuf::from("/home/koppe")]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_theme_falls_back_and_keeps_the_file_read_only() {
        let (dir, path) = setup("unknown-theme");
        fs::write(
            &path,
            serde_json::json!({
                "version": 1,
                "theme": "system",
                "show_hidden": true,
                "view": "icons",
                "sort": { "key": "size", "dir": "asc", "dirs_first": true },
                "bookmarks": ["/home/koppe"],
            })
            .to_string(),
        )
        .unwrap();

        let loaded = Settings::load_from(&path);
        assert!(
            loaded.read_only,
            "unknown theme must keep the load read-only"
        );
        assert!(loaded.load_error.is_some());
        assert!(path.exists());
        assert_eq!(loaded.theme, ThemeMode::Dark);
        assert!(loaded.show_hidden);
        assert_eq!(loaded.view, ViewLayout::Icons);
        assert_eq!(loaded.sort.key, SortKey::Size);
        assert_eq!(loaded.bookmarks, vec![PathBuf::from("/home/koppe")]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn unknown_enum_roundtrip_never_overwrites_the_original_file() {
        let (dir, path) = setup("unknown-enum-roundtrip");
        let original = serde_json::json!({
            "version": 1,
            "view": "list",
            "theme": "system",
            "sort": { "key": "date", "dir": "sideways", "dirs_first": true },
            "show_hidden": true,
            "windows": [{
                "bounds": {
                    "x": 0.0, "y": 0.0, "width": 800.0, "height": 600.0,
                    "mode": "minimized"
                },
                "tabs": [],
                "active_tab": 0
            }]
        })
        .to_string();
        fs::write(&path, &original).unwrap();

        let loaded = Settings::load_from(&path);
        assert!(
            loaded.read_only,
            "unknown enums must keep the load read-only"
        );
        assert!(loaded.load_error.is_some());
        // Fallbacks apply in memory so the app can still start.
        assert_eq!(loaded.view, ViewLayout::Details);
        assert_eq!(loaded.theme, ThemeMode::Dark);
        assert_eq!(loaded.sort.key, SortKey::Name);
        assert_eq!(loaded.sort.dir, SortDir::Asc);
        assert_eq!(
            loaded
                .primary_window()
                .unwrap()
                .bounds
                .as_ref()
                .unwrap()
                .mode,
            super::WindowMode::Windowed
        );

        // save() is a no-op in read-only mode: the exact original bytes,
        // including every unknown enum string, must survive untouched.
        loaded.save_to(&path).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), original);

        // Reloading still sees the unknown values instead of the fallbacks
        // having been persisted over them.
        let reloaded = Settings::load_from(&path);
        assert!(reloaded.read_only);
        assert!(reloaded.load_error.is_some());
        assert_eq!(reloaded.view, ViewLayout::Details);
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
    fn tab_roundtrips_through_session_preserving_cwd_back_and_forward() {
        let mut history = History::new();
        history.visit(p("/a"), p("/b"));
        history.visit(p("/b"), p("/c"));
        history.visit(p("/c"), p("/d"));
        history.go_back(p("/d")); // at /c: back [/a, /b], forward [/d]
        let tab = Tab {
            cwd: p("/c"),
            history: history.clone(),
        };

        let session = TabSession::from_tab(&tab);
        assert_eq!(session.cwd, p("/c"));
        assert_eq!(session.back, vec![p("/a"), p("/b")]);
        assert_eq!(session.forward, vec![p("/d")]);

        let restored = Tab {
            cwd: session.cwd.clone(),
            history: session.to_history(),
        };
        assert_eq!(restored.cwd, tab.cwd);
        assert_eq!(
            restored.history, tab.history,
            "back/forward stacks must survive the roundtrip byte-for-byte"
        );
        assert_eq!(TabSession::from_tab(&restored), session);
    }

    #[test]
    fn tab_roundtrip_preserves_multiple_forward_entries_and_empty_stacks() {
        let mut history = History::new();
        history.visit(p("/a"), p("/b"));
        history.visit(p("/b"), p("/c"));
        history.go_back(p("/c"));
        history.go_back(p("/b")); // at /a: back [], forward [/b, /c]
        let tab = Tab {
            cwd: p("/a"),
            history: history.clone(),
        };
        let session = TabSession::from_tab(&tab);
        assert!(session.back.is_empty());
        assert_eq!(session.forward, vec![p("/b"), p("/c")]);
        let restored = Tab {
            cwd: session.cwd.clone(),
            history: session.to_history(),
        };
        assert_eq!(restored.cwd, tab.cwd);
        assert_eq!(restored.history, tab.history);

        // A fresh tab has no stacks to lose.
        let empty = Tab {
            cwd: p("/tmp"),
            history: History::new(),
        };
        let session = TabSession::from_tab(&empty);
        assert!(session.back.is_empty());
        assert!(session.forward.is_empty());
        let restored = Tab {
            cwd: session.cwd.clone(),
            history: session.to_history(),
        };
        assert_eq!(restored.cwd, empty.cwd);
        assert_eq!(restored.history, History::new());
    }

    #[test]
    fn tab_session_json_shape_is_unchanged() {
        let session = TabSession {
            cwd: p("/c"),
            back: vec![p("/a"), p("/b")],
            forward: vec![p("/d")],
        };
        assert_eq!(
            serde_json::to_value(&session).unwrap(),
            serde_json::json!({"cwd": "/c", "back": ["/a", "/b"], "forward": ["/d"]})
        );
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
        MAX_SAVED_SEARCHES, MAX_SAVED_SEARCH_NAME_BYTES, MAX_SAVED_SEARCH_QUERY_BYTES,
        SCHEMA_VERSION,
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
        // Use a custom command so this test also exercises an explicit value:
        // commands are no longer normalized to "unset" on save, so any value
        // the user chose must round-trip verbatim.
        assert!(settings.set_terminal_command("custom-terminal"));
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
    fn validate_drops_labels_without_a_bookmark() {
        let mut settings = Settings::default();
        settings.toggle_bookmark(Path::new("/keep"));
        assert!(settings.set_bookmark_label(Path::new("/keep"), "kept"));
        // The low-level setter allows a path that is not bookmarked; validate
        // must prune that orphan label.
        assert!(settings.set_bookmark_label(Path::new("/orphan"), "stale"));
        assert_eq!(settings.bookmark_labels.len(), 2);

        settings.validate();

        assert_eq!(settings.bookmark_labels.len(), 1);
        assert_eq!(settings.bookmark_label(Path::new("/keep")), Some("kept"));
        assert!(settings.bookmark_label(Path::new("/orphan")).is_none());
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
            "bookmarks": ["/a", "/b"],
            "bookmark_labels": [
                { "path": "/a", "label": "first" },
                { "path": "/a", "label": "duplicate" },
                { "path": "", "label": "empty-path" },
                { "path": "/b", "label": oversized_label },
                { "path": "/orphan", "label": "stale" }
            ]
        });
        fs::write(&path, json.to_string()).unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.saved_searches.len(), 1);
        assert_eq!(loaded.saved_searches[0].query, "keep");
        assert_eq!(loaded.bookmark_labels.len(), 1);
        assert_eq!(loaded.bookmark_label(Path::new("/a")), Some("first"));
        assert!(loaded.bookmark_label(Path::new("/orphan")).is_none());
        fs::remove_dir_all(&dir).unwrap();

        // Caps are enforced by validate() for values built in memory too.
        let mut settings = Settings::default();
        for index in 0..(MAX_SAVED_SEARCHES + 5) {
            settings
                .saved_searches
                .push(SavedSearch::new(&format!("s-{index}"), "q").unwrap());
        }
        for index in 0..(MAX_BOOKMARK_LABELS + 5) {
            let path = PathBuf::from(format!("/p-{index}"));
            settings.bookmarks.push(path.clone());
            settings
                .bookmark_labels
                .push(BookmarkLabel::new(&path, "label").unwrap());
        }
        settings.validate();
        assert_eq!(settings.saved_searches.len(), MAX_SAVED_SEARCHES);
        assert_eq!(settings.bookmark_labels.len(), MAX_BOOKMARK_LABELS);
    }

    #[test]
    fn whitespace_padded_personalization_survives_load() {
        // These values exceed their cap as raw bytes but fit once trimmed.
        // `is_valid()` checks the raw length, so `validate()` must normalize
        // before calling it or a hand-edited settings.json loses the entry.
        let (dir, path) = setup("padded-personalization");
        let name = "n".repeat(MAX_SAVED_SEARCH_NAME_BYTES - 1);
        let query = "q".repeat(MAX_SAVED_SEARCH_QUERY_BYTES - 1);
        let label = "l".repeat(MAX_BOOKMARK_LABEL_BYTES - 1);
        let padded_name = format!("  {name}  ");
        let padded_query = format!("  {query}  ");
        let padded_label = format!("  {label}  ");
        assert!(padded_name.len() > MAX_SAVED_SEARCH_NAME_BYTES);
        assert!(padded_query.len() > MAX_SAVED_SEARCH_QUERY_BYTES);
        assert!(padded_label.len() > MAX_BOOKMARK_LABEL_BYTES);
        let json = serde_json::json!({
            "version": SCHEMA_VERSION,
            "saved_searches": [{ "name": padded_name, "query": padded_query }],
            "bookmarks": ["/a"],
            "bookmark_labels": [{ "path": "/a", "label": padded_label }]
        });
        fs::write(&path, json.to_string()).unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.saved_searches.len(), 1);
        assert_eq!(loaded.saved_searches[0].name, name);
        assert_eq!(loaded.saved_searches[0].query, query);
        assert_eq!(loaded.bookmark_label(Path::new("/a")), Some(label.as_str()));
        fs::remove_dir_all(&dir).unwrap();
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
        let original = br#"{"version":42,"saved_searches":[{"name":"x","query":"q"}],"bookmarks":["/a"],"bookmark_labels":[{"path":"/a","label":"l"}]}"#;
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
    fn legacy_file_without_the_field_uses_the_portable_unset_value() {
        let (dir, path) = setup("legacy");
        fs::write(&path, br#"{"version":1,"show_hidden":true,"view":"icons"}"#).unwrap();
        let loaded = Settings::load_from(&path);
        assert_eq!(loaded.version, SCHEMA_VERSION);
        // A missing key loads as the portable empty "unset" value: no
        // OS-specific command is invented in memory or carried into the next
        // saved file. The platform default still exists in
        // Settings::default() for the dialog's prefill.
        assert!(loaded.terminal_command.is_empty());
        assert!(loaded.terminal_command_argv().is_none());
        assert_eq!(
            Settings::default().terminal_command,
            default_terminal_command()
        );
        assert!(!loaded.read_only);
        assert!(loaded.show_hidden);

        // Saving materializes the field with the portable empty value. The
        // settings dialog still previews the platform default when it is empty.
        let saved = dir.join("saved.json");
        loaded.save_to(&saved).unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&saved).unwrap()).unwrap();
        assert_eq!(value["terminal_command"].as_str(), Some(""));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn explicit_platform_default_is_preserved_and_survives_a_roundtrip() {
        let (dir, path) = setup("explicit-default");
        let default = default_terminal_command();

        // An explicitly configured value equal to the platform default is a
        // real user choice: it must survive both validate() and save_to().
        fs::write(
            &path,
            serde_json::json!({"version": 1, "terminal_command": default}).to_string(),
        )
        .unwrap();
        let loaded = Settings::load_from(&path);
        if cfg!(target_os = "linux") {
            assert_eq!(loaded.terminal_command, default);
            assert_eq!(loaded.terminal_command_argv(), Some(vec![default.clone()]));
        } else {
            // On unsupported platforms the default is empty by definition.
            assert!(loaded.terminal_command.is_empty());
        }
        assert!(!loaded.read_only);
        assert!(path.exists());

        let saved = dir.join("saved.json");
        loaded.save_to(&saved).unwrap();
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&saved).unwrap()).unwrap();
        assert_eq!(value["terminal_command"].as_str(), Some(default.as_str()));
        let reloaded = Settings::load_from(&saved);
        assert_eq!(reloaded.terminal_command, default);

        // An in-memory unset value still persists as "" (no OS-specific
        // command is invented on the save path).
        let unset_saved = dir.join("unset.json");
        let mut unset = Settings::default();
        assert!(unset.set_terminal_command(""));
        unset.save_to(&unset_saved).unwrap();
        let unset_value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&unset_saved).unwrap()).unwrap();
        assert_eq!(unset_value["terminal_command"].as_str(), Some(""));

        let custom = "custom-terminal --title build";
        let mut custom_settings = Settings::default();
        assert!(custom_settings.set_terminal_command(custom));
        let custom_saved = dir.join("custom.json");
        custom_settings.save_to(&custom_saved).unwrap();
        let custom_value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&custom_saved).unwrap()).unwrap();
        assert_eq!(custom_value["terminal_command"].as_str(), Some(custom));
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

#[cfg(test)]
mod keymap_settings_tests {
    use super::{
        is_reserved_chord, sane_keymap_entry, Settings, MAX_CHORD_BYTES, MAX_KEYMAP_OVERRIDES,
    };
    use std::fs;
    use std::path::PathBuf;

    fn setup(tag: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!("gpui-fe-keymap-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        (dir.clone(), dir.join("settings.json"))
    }

    #[test]
    fn keymap_defaults_empty_and_roundtrips_through_settings_json() {
        let (dir, path) = setup("roundtrip");
        fs::write(&path, br#"{"version":1}"#).unwrap();
        let loaded = Settings::load_from(&path);
        assert!(
            loaded.keymap.is_empty(),
            "an old file has no keymap overrides"
        );

        let mut settings = Settings::default();
        settings
            .keymap
            .push(("open_preview".to_string(), "ctrl+shift+p".to_string()));
        settings.save_to(&path).unwrap();
        let reloaded = Settings::load_from(&path);
        assert_eq!(reloaded.keymap, settings.keymap);
        let value: serde_json::Value =
            serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!(value["keymap"][0][0].as_str(), Some("open_preview"));
        assert_eq!(value["keymap"][0][1].as_str(), Some("ctrl+shift+p"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn keymap_validation_drops_invalid_and_duplicate_entries() {
        let mut settings = Settings::default();
        let overlong = "x".repeat(MAX_CHORD_BYTES + 1);
        settings.keymap = vec![
            (
                "  open_preview  ".to_string(),
                "  ctrl+shift+p  ".to_string(),
            ),
            (String::new(), "ctrl+p".to_string()),
            ("blank_chord".to_string(), "   ".to_string()),
            ("nul_action\0".to_string(), "ctrl+p".to_string()),
            ("nul_chord".to_string(), "ctrl+\0p".to_string()),
            (overlong.clone(), "ctrl+p".to_string()),
            ("overlong".to_string(), format!("ctrl+{overlong}")),
            ("escape_key".to_string(), "escape".to_string()),
            ("f1_key".to_string(), "CTRL+F1".to_string()),
            ("open_preview".to_string(), "ctrl+o".to_string()),
            ("unknown_action".to_string(), "ctrl+y".to_string()),
        ];
        settings.validate();
        assert_eq!(
            settings.keymap,
            vec![
                ("open_preview".to_string(), "ctrl+shift+p".to_string()),
                ("unknown_action".to_string(), "ctrl+y".to_string()),
            ]
        );
    }

    #[test]
    fn keymap_validation_keeps_first_entry_per_chord() {
        let mut settings = Settings {
            keymap: vec![
                ("a".to_string(), "ctrl+p".to_string()),
                ("b".to_string(), "ctrl+p".to_string()),
                ("c".to_string(), "ctrl+shift+p".to_string()),
            ],
            ..Settings::default()
        };
        settings.validate();
        assert_eq!(
            settings.keymap,
            vec![
                ("a".to_string(), "ctrl+p".to_string()),
                ("c".to_string(), "ctrl+shift+p".to_string()),
            ],
            "the first action bound to a chord must win"
        );

        // A rejected duplicate action must not reserve its chord: a later
        // action can still legitimately claim that chord.
        let mut settings = Settings {
            keymap: vec![
                ("a".to_string(), "ctrl+p".to_string()),
                ("a".to_string(), "ctrl+shift+p".to_string()),
                ("b".to_string(), "ctrl+shift+p".to_string()),
            ],
            ..Settings::default()
        };
        settings.validate();
        assert_eq!(
            settings.keymap,
            vec![
                ("a".to_string(), "ctrl+p".to_string()),
                ("b".to_string(), "ctrl+shift+p".to_string()),
            ],
            "the duplicate chord of a rejected action must stay available"
        );
    }

    #[test]
    fn keymap_validation_does_not_reserve_action_when_chord_is_duplicate() {
        let mut settings = Settings {
            keymap: vec![
                ("a".to_string(), "ctrl+p".to_string()),
                ("b".to_string(), "ctrl+p".to_string()),
                ("b".to_string(), "ctrl+o".to_string()),
            ],
            ..Settings::default()
        };
        settings.validate();
        assert_eq!(
            settings.keymap,
            vec![
                ("a".to_string(), "ctrl+p".to_string()),
                ("b".to_string(), "ctrl+o".to_string()),
            ],
            "a duplicate chord must not reserve its action for later entries"
        );
    }

    #[test]
    fn is_reserved_chord_matches_only_the_final_key_segment() {
        for chord in ["f1", "escape", "ctrl+f1", "alt+escape"] {
            assert!(is_reserved_chord(chord), "reserved: {chord}");
        }
        // Case-insensitive, like the chord parser in main.rs.
        assert!(is_reserved_chord("CTRL+F1"));
        assert!(is_reserved_chord("Escape"));
        // `f10` / `f12` contain the substring "f1" but are ordinary keys.
        for chord in ["f10", "ctrl+f12", "shift+f19", "ctrl+p", ""] {
            assert!(!is_reserved_chord(chord), "not reserved: {chord}");
        }
    }

    #[test]
    fn sane_keymap_entry_allows_high_function_keys_and_rejects_reserved_keys() {
        for chord in ["f10", "ctrl+f12", "shift+f19"] {
            assert!(
                sane_keymap_entry("act", chord).is_some(),
                "{chord} should be accepted"
            );
        }
        for chord in ["f1", "ctrl+f1", "escape", "alt+escape"] {
            assert!(
                sane_keymap_entry("act", chord).is_none(),
                "{chord} should be rejected"
            );
        }
    }

    #[test]
    fn keymap_validation_caps_the_number_of_overrides() {
        let mut settings = Settings {
            keymap: (0..MAX_KEYMAP_OVERRIDES + 12)
                .map(|index| (format!("action-{index}"), format!("ctrl+{index}")))
                .collect(),
            ..Settings::default()
        };
        settings.validate();
        assert_eq!(settings.keymap.len(), MAX_KEYMAP_OVERRIDES);
        assert_eq!(settings.keymap[0].0, "action-0");
        assert_eq!(
            settings.keymap[MAX_KEYMAP_OVERRIDES - 1].0,
            format!("action-{}", MAX_KEYMAP_OVERRIDES - 1)
        );
    }
}
