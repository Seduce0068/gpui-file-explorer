// SPDX-License-Identifier: AGPL-3.0-or-later
//! Filesystem data model: directory scanning, filtering, sorting and
//! human-readable formatting.
//!
//! This module is intentionally free of GPUI types so it can be unit tested
//! headlessly (`cargo test`) and reused on any platform: the filesystem access
//! is plain `std::fs`, and only the UI layer depends on gpui.

// `error.rs` lives next to this file. Declaring it here (instead of in
// `main.rs`) keeps the shared error type in scope without touching the UI
// module list; `ops.rs` reaches the same type through `crate::model::AppError`.
#[path = "error.rs"]
pub mod error;

// `FsErrorKind` is part of the pre-refactor `model` API; keep the re-export
// even though the non-test build no longer names it directly (the shared
// error module uses it internally).
#[allow(unused_imports)]
pub use error::{AppError, BoxedCause, FsErrorKind};

/// Filesystem error type kept as an alias: existing callers still write
/// `FsError::from_io(..)` and match on `FsError` variants through [`AppError`].
pub type FsError = AppError;

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use alphanumeric_sort::compare_str;
use serde::{Deserialize, Serialize};

use crate::strings::{
    AGE_DAYS_SUFFIX, AGE_FUTURE, AGE_HOURS_SUFFIX, AGE_JUST_NOW, AGE_MINUTES_SUFFIX,
    AGE_YEARS_SUFFIX, KIND_FOLDER, KIND_LINK, KIND_NONE, VALUE_UNKNOWN,
};

/// One entry of a directory listing.
#[derive(Clone, Debug)]
pub struct Entry {
    /// Raw OS file name. Never lossy-converted, so non-UTF-8 names survive I/O.
    pub name: std::ffi::OsString,
    /// Display/sort form of `name` (`to_string_lossy`). Cached on construction.
    pub name_text: String,
    /// Precomputed lowercase form of `name_text` for case-insensitive sorting.
    pub sort_key: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
}

impl Entry {
    /// Build an [`Entry`] from a `read_dir` item. Returns `None` for entries
    /// whose name cannot be represented (rare, platform dependent).
    fn from_dir_entry(item: std::fs::DirEntry) -> Option<Entry> {
        let path = item.path();
        let name = item.file_name();
        if name.is_empty() {
            return None;
        }
        let name_text = name.to_string_lossy().into_owned();
        let sort_key = name_text.to_lowercase();
        // `symlink_metadata` does not follow symlinks; `metadata` does. We use
        // the former for the symlink flag and the latter for dir-ness/size so
        // symlinked directories stay enterable.
        let symlink_metadata = std::fs::symlink_metadata(&path).ok();
        let is_symlink = symlink_metadata
            .as_ref()
            .map(|meta| meta.file_type().is_symlink())
            .unwrap_or(false);
        let metadata = std::fs::metadata(&path).ok();
        let is_dir = metadata.as_ref().map(|meta| meta.is_dir()).unwrap_or(false);
        let size = metadata.as_ref().map(|meta| meta.len()).unwrap_or(0);
        let modified = metadata.and_then(|meta| meta.modified().ok());
        Some(Entry {
            name,
            name_text,
            sort_key,
            path,
            is_dir,
            is_symlink,
            size,
            modified,
        })
    }
}

/// Blocking directory read. Callers on the UI thread must run this on the
/// background executor (see `Explorer::reload`).
pub fn read_entries(dir: &Path) -> Result<Vec<Entry>, FsError> {
    let dir_reader =
        std::fs::read_dir(dir).map_err(|err| FsError::from_io(dir.to_path_buf(), &err))?;
    let mut entries = Vec::new();
    for item in dir_reader {
        let Ok(item) = item else { continue };
        if let Some(entry) = Entry::from_dir_entry(item) {
            entries.push(entry);
        }
    }
    Ok(entries)
}

/// Build an [`Entry`] for a single path, used by watcher updates and search.
///
/// Symlinks are flagged from `symlink_metadata` while their target is followed
/// for size/time so symlinked directories remain enterable. If the target
/// cannot be resolved (dangling link), size/time stay unset — the same rule as
/// [`Entry::from_dir_entry`] — so both constructors produce an identical
/// fingerprint after watcher events.
pub fn entry_for_path(path: &Path) -> Option<Entry> {
    let name = path.file_name()?.to_os_string();
    if name.is_empty() {
        return None;
    }
    let name_text = name.to_string_lossy().into_owned();
    let sort_key = name_text.to_lowercase();
    let link_metadata = std::fs::symlink_metadata(path).ok()?;
    let is_symlink = link_metadata.file_type().is_symlink();
    let metadata = std::fs::metadata(path).ok();
    let is_dir = metadata.as_ref().is_some_and(|meta| meta.is_dir());
    let size = metadata.as_ref().map(|meta| meta.len()).unwrap_or(0);
    let modified = metadata.and_then(|meta| meta.modified().ok());
    Some(Entry {
        name,
        name_text,
        sort_key,
        path: path.to_path_buf(),
        is_dir,
        is_symlink,
        size,
        modified,
    })
}

/// Multi-selection state, keyed by path so it survives reloads and re-sorting.
///
/// The list keeps a separate "focused" row index; this struct only tracks which
/// paths are selected and the anchor used for shift-range selection.
#[derive(Clone, Debug, Default)]
pub struct Selection {
    /// Insertion/view order is preserved for anchor and range selection.
    paths: Vec<PathBuf>,
    /// Mirrors `paths` so the per-row `contains` check in rendering is O(1).
    path_set: std::collections::HashSet<PathBuf>,
    anchor: Option<PathBuf>,
}

impl Selection {
    fn set_paths(&mut self, paths: Vec<PathBuf>) {
        self.path_set = paths.iter().cloned().collect();
        self.paths = paths;
    }

    pub fn paths(&self) -> &[PathBuf] {
        &self.paths
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }

    pub fn is_empty(&self) -> bool {
        self.paths.is_empty()
    }

    pub fn contains(&self, path: &Path) -> bool {
        self.path_set.contains(path)
    }

    pub fn clear(&mut self) {
        self.paths.clear();
        self.path_set.clear();
        self.anchor = None;
    }

    /// Replace the selection with a single path (plain click / arrow key).
    pub fn select_single(&mut self, path: PathBuf) {
        self.paths = vec![path.clone()];
        self.path_set.clear();
        self.path_set.insert(path.clone());
        self.anchor = Some(path);
    }

    /// Ctrl+click: add or remove one path; the anchor follows the click.
    pub fn toggle(&mut self, path: PathBuf) {
        if self.path_set.remove(&path) {
            if let Some(position) = self.paths.iter().position(|selected| selected == &path) {
                self.paths.remove(position);
            }
            if self.anchor.as_deref() == Some(path.as_path()) {
                self.anchor = self.paths.last().cloned();
            }
        } else {
            self.path_set.insert(path.clone());
            self.paths.push(path);
            self.anchor = self.paths.last().cloned();
        }
    }

    /// Shift+click / Shift+arrow: select the inclusive range between the
    /// anchor and `target` in view order.
    pub fn select_range(&mut self, entries: &[PathBuf], target: &Path) {
        let Some(anchor) = self.anchor.clone() else {
            self.select_single(target.to_path_buf());
            return;
        };
        let Some(anchor_index) = entries.iter().position(|path| path == &anchor) else {
            self.select_single(target.to_path_buf());
            return;
        };
        let Some(target_index) = entries.iter().position(|path| path == target) else {
            return;
        };
        let (start, end) = if anchor_index <= target_index {
            (anchor_index, target_index)
        } else {
            (target_index, anchor_index)
        };
        self.set_paths(entries[start..=end].to_vec());
    }

    pub fn select_all(&mut self, entries: &[PathBuf]) {
        // An empty listing must leave no stale anchor behind: the invariant is
        // "empty selection => anchor None", otherwise the next select_range
        // would silently restore a range from an item the user cannot see.
        if entries.is_empty() {
            self.clear();
            return;
        }
        self.set_paths(entries.to_vec());
        if self.anchor.is_none() {
            self.anchor = entries.first().cloned();
        }
    }

    /// Drop paths that no longer exist in the current listing.
    pub fn retain(&mut self, entries: &[PathBuf]) {
        self.paths.retain(|path| entries.contains(path));
        self.path_set = self.paths.iter().cloned().collect();
        if let Some(anchor) = &self.anchor {
            if !entries.contains(anchor) {
                self.anchor = self.paths.first().cloned();
            }
        }
        if self.paths.is_empty() {
            self.anchor = None;
        }
    }
}

/// Resolve a path typed into the location bar (Ctrl+L).
///
/// - empty/whitespace -> error,
/// - `~` and `~/...` expand to `home` (only when `home` is known),
/// - relative paths resolve against `cwd`,
/// - absolute paths are used as-is (no shell expansion or `~user` support).
pub fn resolve_location_input(
    cwd: &std::path::Path,
    input: &str,
    home: Option<&std::path::Path>,
) -> Result<PathBuf, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("パスが空です".to_string());
    }
    if input == "~" {
        return home
            .map(|home| home.to_path_buf())
            .ok_or_else(|| "ホームディレクトリが分かりません".to_string());
    }
    if let Some(rest) = input.strip_prefix("~/") {
        let Some(home) = home else {
            return Err("ホームディレクトリが分かりません".to_string());
        };
        return Ok(home.join(rest));
    }
    let path = PathBuf::from(input);
    if path.is_absolute() {
        Ok(path)
    } else {
        Ok(cwd.join(path))
    }
}

/// Render Unix permission bits (`mode & 0o777`) as `rwxr-xr-x`.
#[cfg(any(unix, test))]
pub fn permissions_string(mode: u32) -> String {
    let mut out = String::with_capacity(9);
    for shift in [6u32, 3, 0] {
        let bits = (mode >> shift) & 0o7;
        out.push(if bits & 0o4 != 0 { 'r' } else { '-' });
        out.push(if bits & 0o2 != 0 { 'w' } else { '-' });
        out.push(if bits & 0o1 != 0 { 'x' } else { '-' });
    }
    out
}

/// Number of icon-grid columns that fit into `available_width` when each cell
/// needs at least `min_cell_width` pixels.
pub fn grid_columns(available_width: f32, min_cell_width: f32) -> usize {
    if min_cell_width <= 0.0 || !available_width.is_finite() {
        return 1;
    }
    ((available_width / min_cell_width).floor() as usize).max(1)
}

/// Number of rows needed to show `items` in a grid with `columns` per row.
pub fn grid_row_count(items: usize, columns: usize) -> usize {
    if columns == 0 {
        return 0;
    }
    items.div_ceil(columns)
}

/// Which column the list is sorted by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortKey {
    #[default]
    Name,
    Kind,
    Size,
    Modified,
}

impl SortKey {
    pub fn label(self) -> &'static str {
        match self {
            SortKey::Name => "名前",
            SortKey::Kind => "種類",
            SortKey::Size => "サイズ",
            SortKey::Modified => "更新日時",
        }
    }
}

/// Deserialize a persisted sort key, mapping an unknown name (e.g. `"date"`)
/// to the default instead of rejecting the whole settings file.
///
/// A `#[serde(other)]` fallback variant would force a wildcard arm into every
/// exhaustive `SortKey` match (including the UI's header-id mapping in
/// `main.rs`), so the fallback is applied at the deserialization boundary
/// instead: the field becomes `SortKey::Name` and all sibling settings survive.
/// The unknown string is also reported to `settings` so the load becomes
/// read-only instead of writing the fallback back over the original value.
fn deserialize_sort_key<'de, D>(deserializer: D) -> Result<SortKey, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(match value.as_str() {
        "name" => SortKey::Name,
        "kind" => SortKey::Kind,
        "size" => SortKey::Size,
        "modified" => SortKey::Modified,
        _ => {
            crate::settings::note_unknown_persisted_enum();
            SortKey::default()
        }
    })
}

/// Sort configuration: key + direction. Directories always come first,
/// independent of the key/direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SortOrder {
    pub key: SortKey,
    pub ascending: bool,
}

impl Default for SortOrder {
    fn default() -> Self {
        Self {
            key: SortKey::Name,
            ascending: true,
        }
    }
}

impl SortOrder {
    /// Clicking a column header: same key toggles the direction, a new key
    /// starts ascending for names and descending for size/date (the usual
    /// "biggest/newest first" expectation).
    pub fn toggled(self, key: SortKey) -> Self {
        if self.key == key {
            Self {
                key,
                ascending: !self.ascending,
            }
        } else {
            Self {
                key,
                ascending: matches!(key, SortKey::Name | SortKey::Kind),
            }
        }
    }
}

pub fn is_hidden_name(name: &str) -> bool {
    name.starts_with('.')
}

/// Lower-case extension of a file name (without the dot), or an empty string
/// when there is no extension.
pub(crate) fn extension_of(name: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, extension)) if !stem.is_empty() && !extension.is_empty() => {
            extension.to_lowercase()
        }
        _ => String::new(),
    }
}

/// Human-readable kind label for the list's "種類" column.
pub fn kind_label(entry: &Entry) -> String {
    if entry.is_dir {
        return KIND_FOLDER.to_string();
    }
    if entry.is_symlink {
        return KIND_LINK.to_string();
    }
    let extension = extension_of(&entry.name_text);
    if extension.is_empty() {
        KIND_NONE.to_string()
    } else {
        extension
    }
}

/// Primary comparison key for `SortKey::Kind`: files sort by extension, and
/// directories all share the same key (their name is the tie-breaker).
fn kind_sort_key(entry: &Entry) -> String {
    if entry.is_dir {
        String::new()
    } else {
        extension_of(&entry.name_text)
    }
}

/// Filter hidden entries and sort, producing the list that is displayed.
pub fn visible_entries(all: &[Entry], show_hidden: bool, order: SortOrder) -> Vec<Entry> {
    let mut view: Vec<Entry> = all
        .iter()
        .filter(|entry| show_hidden || !is_hidden_name(&entry.name_text))
        .cloned()
        .collect();
    sort_entries(&mut view, order);
    view
}

/// Natural, case-insensitive name comparison ("file2" < "file10", "alpha" <
/// "Zebra"), with a case-sensitive pass as deterministic tie-breaker.
#[cfg(test)]
pub fn compare_names(a: &str, b: &str) -> std::cmp::Ordering {
    compare_str(a.to_lowercase(), b.to_lowercase()).then_with(|| compare_str(a, b))
}

/// Compare using a precomputed lowercase key, avoiding per-comparison allocation.
fn compare_precomputed(a: &Entry, b: &Entry) -> std::cmp::Ordering {
    compare_str(&a.sort_key, &b.sort_key).then_with(|| compare_str(&a.name_text, &b.name_text))
}

/// Stable natural sort: directories first, then the chosen key with a natural
/// alphanumeric name comparison as tie-breaker.
pub fn sort_entries(entries: &mut Vec<Entry>, order: SortOrder) {
    let (mut dirs, mut files): (Vec<Entry>, Vec<Entry>) = std::mem::take(entries)
        .into_iter()
        .partition(|entry| entry.is_dir);

    // Use the precomputed lowercase key so sorting never allocates.
    let cmp = |a: &Entry, b: &Entry| {
        let primary = match order.key {
            SortKey::Name => compare_precomputed(a, b),
            SortKey::Kind => kind_sort_key(a).cmp(&kind_sort_key(b)),
            SortKey::Size => a.size.cmp(&b.size),
            // `None` (unreadable timestamp) sorts before any timestamp.
            SortKey::Modified => a.modified.cmp(&b.modified),
        };
        primary.then_with(|| compare_precomputed(a, b))
    };

    dirs.sort_by(cmp);
    files.sort_by(cmp);
    if !order.ascending {
        dirs.reverse();
        files.reverse();
    }
    dirs.extend(files);
    *entries = dirs;
}

/// Sort direction as persisted in the settings file (`asc` / `desc`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDir {
    #[default]
    Asc,
    Desc,
}

/// Unknown persisted direction falls back to Asc at the deserialize boundary
/// so one bad enum value cannot back up the whole settings file. As with the
/// sort key, the unknown string makes the `Settings` load read-only so it is
/// never overwritten on save.
fn deserialize_sort_dir<'de, D>(deserializer: D) -> Result<SortDir, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = String::deserialize(deserializer)?;
    Ok(match value.as_str() {
        "asc" => SortDir::Asc,
        "desc" => SortDir::Desc,
        _ => {
            crate::settings::note_unknown_persisted_enum();
            SortDir::default()
        }
    })
}

impl SortDir {
    pub fn is_ascending(self) -> bool {
        matches!(self, SortDir::Asc)
    }
}

impl From<bool> for SortDir {
    fn from(ascending: bool) -> Self {
        if ascending {
            SortDir::Asc
        } else {
            SortDir::Desc
        }
    }
}

/// Persisted sort configuration (DESIGN.md §5.2 / §8.1).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SortSpec {
    #[serde(deserialize_with = "deserialize_sort_key")]
    pub key: SortKey,
    #[serde(deserialize_with = "deserialize_sort_dir")]
    pub dir: SortDir,
    /// Directories always sort before files in P0 (kept in the schema for
    /// future "mix files and folders" options).
    pub dirs_first: bool,
}

impl Default for SortSpec {
    fn default() -> Self {
        Self {
            key: SortKey::Name,
            dir: SortDir::Asc,
            dirs_first: true,
        }
    }
}

impl SortSpec {
    pub fn order(self) -> SortOrder {
        SortOrder {
            key: self.key,
            ascending: self.dir.is_ascending(),
        }
    }

    pub fn from_order(order: SortOrder) -> Self {
        Self {
            key: order.key,
            dir: SortDir::from(order.ascending),
            dirs_first: true,
        }
    }
}

// --- Personalization data model (F6-38) -----------------------------------
//
// These types are deliberately GPUI-free and live in `model` so they can be
// unit tested headlessly. `settings.rs` owns their persistence, caps and
// duplicate pruning; the UI layer wires them through `Settings` methods.

/// Maximum number of named searches persisted in settings.
pub const MAX_SAVED_SEARCHES: usize = 50;
/// Maximum byte length of a saved search's display name.
pub const MAX_SAVED_SEARCH_NAME_BYTES: usize = 128;
/// Maximum byte length of a saved search query (matches `settings::MAX_QUERY_BYTES`).
pub const MAX_SAVED_SEARCH_QUERY_BYTES: usize = 1024;
/// Maximum byte length of a bookmark label.
pub const MAX_BOOKMARK_LABEL_BYTES: usize = 128;
/// Maximum number of labeled bookmarks (tracks `settings::MAX_BOOKMARKS`).
pub const MAX_BOOKMARK_LABELS: usize = 50;

/// Shared text rule for persisted personalization values: non-empty after
/// trimming, bounded on the raw byte length, and free of embedded NUL.
fn sane_personal_text(value: &str, max_bytes: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max_bytes && !value.contains('\0')
}

/// A named, reusable search query (F6-38 personalization).
///
/// Persisted as `saved_searches: [{ "name": ..., "query": ... }]`. Values are
/// normalized (trimmed) on construction; entries that are blank, oversized or
/// contain NUL are dropped by `Settings::validate` after deserialization.
/// Names are compared case-insensitively for lookup and duplicate detection.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct SavedSearch {
    /// User-facing name, unique (case-insensitively) within the settings list.
    pub name: String,
    /// Search expression replayed by the UI; the query language is unchanged.
    pub query: String,
}

impl SavedSearch {
    /// Build a normalized entry, or `None` for blank/oversized/NUL text.
    pub fn new(name: &str, query: &str) -> Option<Self> {
        let search = Self {
            name: name.trim().to_owned(),
            query: query.trim().to_owned(),
        };
        search.is_valid().then_some(search)
    }

    /// Whether both fields are usable for persistence.
    pub fn is_valid(&self) -> bool {
        sane_personal_text(&self.name, MAX_SAVED_SEARCH_NAME_BYTES)
            && sane_personal_text(&self.query, MAX_SAVED_SEARCH_QUERY_BYTES)
    }

    /// Case-insensitive key used for duplicate detection and lookup.
    pub fn name_key(&self) -> String {
        // Trim here, not just on insert: settings.json can be hand-edited and
        // must still resolve the same saved search by name.
        self.name.trim().to_lowercase()
    }
}

/// A user label attached to a bookmarked path (F6-38 personalization).
///
/// Persisted as `bookmark_labels: [{ "path": ..., "label": ... }]`. This type
/// owns label validation only; the full path rule (non-empty, bounded,
/// NUL-free) is applied by `Settings::validate` with the same `sane_path`
/// check used for `bookmarks`, keeping this module independent of settings.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct BookmarkLabel {
    /// Bookmark path the label belongs to; unique within the settings list.
    pub path: PathBuf,
    /// Short user-facing description of the bookmark.
    pub label: String,
}

impl BookmarkLabel {
    /// Build a normalized entry, rejecting an empty path or an unusable label.
    pub fn new(path: &Path, label: &str) -> Option<Self> {
        if path.as_os_str().is_empty() {
            return None;
        }
        let entry = Self {
            path: path.to_path_buf(),
            label: label.trim().to_owned(),
        };
        entry.is_valid().then_some(entry)
    }

    /// Whether the label is usable for persistence (path checked by settings).
    pub fn is_valid(&self) -> bool {
        sane_personal_text(&self.label, MAX_BOOKMARK_LABEL_BYTES)
    }
}

/// Clamp a selection index into `0..len` (0 for an empty list).
pub fn clamp_index(index: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        index.min(len - 1)
    }
}

/// Move the selection by `delta` rows, clamped to the list bounds.
pub fn step_selection(selected: usize, len: usize, delta: isize) -> usize {
    if len == 0 {
        return 0;
    }
    let last = (len - 1) as isize;
    (selected as isize + delta).clamp(0, last) as usize
}

pub fn first_index(_len: usize) -> usize {
    0
}

pub fn last_index(len: usize) -> usize {
    len.saturating_sub(1)
}

/// Cheap content fingerprint used by the 10s reconciliation timer to detect
/// changes the OS watcher missed (or that happened while watching was broken).
///
/// `modified` is hashed at one-second resolution, so two edits within the same
/// second that leave the name and size unchanged are indistinguishable. The
/// OS watcher is the primary detector; this is a coarse backstop only.
pub fn fingerprint(entries: &[Entry]) -> u64 {
    let mut hasher = DefaultHasher::new();
    entries.len().hash(&mut hasher);
    for entry in entries {
        entry.name.hash(&mut hasher);
        entry.is_dir.hash(&mut hasher);
        entry.size.hash(&mut hasher);
        entry.modified.and_then(unix_secs).hash(&mut hasher);
    }
    hasher.finish()
}

fn unix_secs(time: SystemTime) -> Option<i64> {
    match time.duration_since(UNIX_EPOCH) {
        Ok(duration) => Some(duration.as_secs() as i64),
        Err(_) => None,
    }
}

/// `1023 B`, `1.5 K`, `12.3 M`, ... (binary units, 1 K = 1024 B).
pub fn human_size(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "K", "M", "G", "T", "P"];
    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} B")
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}

/// Format a modification time in local time as `YYYY-MM-DD HH:MM`.
pub fn human_datetime(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return VALUE_UNKNOWN.to_string();
    };
    let datetime: chrono::DateTime<chrono::Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M").to_string()
}

/// Relative age (`3分前`, `2時間前`, ...) used in the status bar.
pub fn human_age(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return VALUE_UNKNOWN.to_string();
    };
    let now = SystemTime::now();
    let Ok(elapsed) = now.duration_since(time) else {
        return AGE_FUTURE.to_string();
    };
    let secs = elapsed.as_secs();
    if secs < 60 {
        AGE_JUST_NOW.to_string()
    } else if secs < 3600 {
        format!("{}{}", secs / 60, AGE_MINUTES_SUFFIX)
    } else if secs < 86_400 {
        format!("{}{}", secs / 3600, AGE_HOURS_SUFFIX)
    } else if secs < 86_400 * 365 {
        format!("{}{}", secs / 86_400, AGE_DAYS_SUFFIX)
    } else {
        format!("{}{}", secs / (86_400 * 365), AGE_YEARS_SUFFIX)
    }
}

/// Path shown in the status bar / used for the window title.
pub fn display_path(path: &Path) -> String {
    path.display().to_string()
}

/// Unique folder name following the `新しいフォルダ (2)` convention.
pub fn unique_child_name(dir: &Path, base: &str) -> String {
    let mut candidate = base.to_string();
    let mut counter = 2u32;
    loop {
        let path = dir.join(&candidate);
        // Mirror `ops::unique_path`: `exists()` follows the link and reports
        // false for a dangling symlink, but the name is still occupied and
        // `create_dir` would fail with EEXIST on it forever. Skip it too.
        if !path.exists() && !path.is_symlink() {
            return candidate;
        }
        candidate = format!("{base} ({counter})");
        counter += 1;
        if counter > 10_000 {
            return candidate;
        }
    }
}

/// Create a uniquely named folder inside `dir`.
///
/// Blocking: call this on the background executor, never on the UI thread.
/// Retries with the next name when a concurrent creator wins the race
/// (`create_dir` returns `AlreadyExists`).
pub fn create_unique_folder(dir: &Path) -> Result<PathBuf, String> {
    for _ in 0..100 {
        let name = unique_child_name(dir, "新しいフォルダ");
        let path = dir.join(&name);
        match std::fs::create_dir(&path) {
            Ok(()) => return Ok(path),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(format!("{}: {error}", path.display())),
        }
    }
    Err(format!(
        "{}: 一意なフォルダ名を作成できません",
        dir.display()
    ))
}

/// Breadcrumb segments: `("/", "/"), ("home", "/home"), ("koppe", "/home/koppe")`.
pub fn breadcrumb_segments(path: &Path) -> Vec<(String, PathBuf)> {
    use std::path::Component;
    let mut segments = Vec::new();
    let mut accumulated = PathBuf::new();
    for component in path.components() {
        accumulated.push(component.as_os_str());
        match component {
            Component::RootDir => segments.push(("/".to_string(), accumulated.clone())),
            Component::Normal(name) => {
                segments.push((name.to_string_lossy().to_string(), accumulated.clone()))
            }
            _ => {}
        }
    }
    segments
}

/// `dir` itself for files/lists, but for the list header we need the count.
pub fn parent_dir(path: &Path) -> Option<PathBuf> {
    path.parent().map(Path::to_path_buf)
}

#[cfg(test)]
mod tests {
    // NOTE: explicit imports only — the crate root glob-imports `gpui_kit::*`,
    // which re-exports GPUI's `test` proc-macro and would shadow `#[test]`.
    use super::{
        breadcrumb_segments, clamp_index, compare_names, create_unique_folder, display_path,
        entry_for_path, fingerprint, first_index, human_age, human_datetime, human_size,
        is_hidden_name, kind_label, last_index, parent_dir, read_entries, sort_entries,
        step_selection, unique_child_name, visible_entries, AppError, Entry, FsError, FsErrorKind,
        SortDir, SortKey, SortOrder, SortSpec,
    };
    use std::ffi::OsString;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn scratch_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("gpui-fe-model-{tag}-{}", std::process::id()))
    }

    fn entry(name: &str, is_dir: bool, size: u64, modified_secs: u64) -> Entry {
        Entry {
            name: OsString::from(name),
            name_text: name.to_string(),
            sort_key: name.to_lowercase(),
            path: PathBuf::from("/tmp").join(name),
            is_dir,
            is_symlink: false,
            size,
            modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(modified_secs)),
        }
    }

    #[test]
    fn read_entries_lists_dirs_first_with_metadata() {
        let dir = scratch_dir("read");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("zeta_dir")).unwrap();
        fs::create_dir_all(dir.join("alpha_dir")).unwrap();
        fs::write(dir.join("beta.txt"), b"hello world").unwrap();
        fs::write(dir.join("Alpha.txt"), b"x").unwrap();

        let entries = read_entries(&dir).expect("directory should be readable");
        assert_eq!(entries.len(), 4);
        assert_eq!(entries.iter().filter(|entry| entry.is_dir).count(), 2);
        let beta = entries
            .iter()
            .find(|entry| entry.name_text == "beta.txt")
            .unwrap();
        assert_eq!(beta.size, 11);
        assert!(beta.modified.is_some());
        assert!(beta.path.ends_with("beta.txt"));
        assert!(!beta.is_symlink);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn read_entries_keeps_non_utf8_names_lossless() {
        use std::os::unix::ffi::OsStringExt;

        let dir = scratch_dir("non-utf8");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let raw = std::ffi::OsString::from_vec(vec![b'x', 0xff, b'.', b't', b'x', b't']);
        fs::write(dir.join(&raw), b"data").unwrap();

        let entries = read_entries(&dir).expect("directory should be readable");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, raw, "raw OS name must survive intact");
        assert!(
            entries[0].name_text.contains("x"),
            "display form is a lossy rendering of the same name"
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn read_entries_reports_errors() {
        let missing = scratch_dir("missing");
        let _ = fs::remove_dir_all(&missing);
        let error = read_entries(&missing).expect_err("missing dir must fail");
        assert_eq!(error.fs_kind(), Some(FsErrorKind::NotFound));
        assert_eq!(error.fs_path(), Some(missing.as_path()));
        assert!(
            std::error::Error::source(&error).is_some(),
            "the underlying io::Error must stay reachable through source()"
        );
        assert!(error.message_ja().contains("見つかりません"));
        assert!(error.to_string().contains("見つかりません"));
        assert!(
            error.message_ja().contains('('),
            "OS detail should be shown when available: {}",
            error.message_ja()
        );
    }

    #[test]
    fn fs_error_maps_io_kinds() {
        use std::io::{Error, ErrorKind};
        let err = FsError::from_io(
            PathBuf::from("/tmp/x"),
            &Error::from(ErrorKind::PermissionDenied),
        );
        assert_eq!(err.fs_kind(), Some(FsErrorKind::PermissionDenied));
        assert_eq!(err.fs_os_message(), Some("permission denied"));

        let err = FsError::from_io(PathBuf::from("/tmp/x"), &Error::from(ErrorKind::NotFound));
        assert_eq!(err.fs_kind(), Some(FsErrorKind::NotFound));

        let err = FsError::from_io(PathBuf::from("/tmp/x"), &Error::other("custom"));
        assert_eq!(err.fs_kind(), Some(FsErrorKind::Unknown));

        let err = FsError::from_io(
            PathBuf::from("/tmp/x"),
            &Error::from(ErrorKind::NotADirectory),
        );
        assert_eq!(err.fs_kind(), Some(FsErrorKind::NotADirectory));
        assert!(err.message_ja().contains("ディレクトリではありません"));

        let err = FsError::from_io(
            PathBuf::from("/tmp/x"),
            &Error::from(ErrorKind::InvalidInput),
        );
        assert_eq!(err.fs_kind(), Some(FsErrorKind::Io));
        assert!(err.message_ja().contains("読み取れませんでした"));

        let denied = FsError::from_io(
            PathBuf::from("/tmp/x"),
            &Error::from(ErrorKind::PermissionDenied),
        );
        assert!(denied.message_ja().contains("アクセスが拒否されました"));
        let unknown = FsError::from_io(PathBuf::from("/tmp/x"), &Error::other("custom"));
        assert!(unknown.message_ja().contains("エラーが発生しました"));

        // No OS detail: the message stays short without an empty "()".
        let plain = AppError::Fs {
            kind: FsErrorKind::NotFound,
            path: PathBuf::from("/tmp/x"),
            os_message: None,
        };
        assert_eq!(plain.message_ja(), "/tmp/x: 見つかりません");
        let empty_detail = AppError::Fs {
            kind: FsErrorKind::NotFound,
            path: PathBuf::from("/tmp/x"),
            os_message: Some(String::new()),
        };
        assert_eq!(empty_detail.message_ja(), "/tmp/x: 見つかりません");
    }

    #[test]
    fn sort_key_labels_are_japanese() {
        assert_eq!(SortKey::Name.label(), "名前");
        assert_eq!(SortKey::Kind.label(), "種類");
        assert_eq!(SortKey::Size.label(), "サイズ");
        assert_eq!(SortKey::Modified.label(), "更新日時");
    }

    #[test]
    fn hidden_filter_drops_dotfiles_only() {
        assert!(is_hidden_name(".bashrc"));
        assert!(!is_hidden_name("bashrc"));
        let all = vec![
            entry(".hidden", false, 0, 0),
            entry("visible", false, 0, 0),
            entry(".dir", true, 0, 0),
        ];
        let shown = visible_entries(&all, false, SortOrder::default());
        assert_eq!(
            shown
                .iter()
                .map(|entry| entry.name_text.as_str())
                .collect::<Vec<_>>(),
            vec!["visible"]
        );
        let shown_all = visible_entries(&all, true, SortOrder::default());
        assert_eq!(shown_all.len(), 3);
        // Hidden directories stay first even when shown.
        assert!(shown_all[0].is_dir);
    }

    #[test]
    fn sort_dirs_first_then_natural_name_order() {
        let mut entries = vec![
            entry("file10.txt", false, 0, 0),
            entry("file2.txt", false, 0, 0),
            entry("Zebra", true, 0, 0),
            entry("alpha", true, 0, 0),
        ];
        sort_entries(&mut entries, SortOrder::default());
        let names: Vec<&str> = entries
            .iter()
            .map(|entry| entry.name_text.as_str())
            .collect();
        assert_eq!(names, vec!["alpha", "Zebra", "file2.txt", "file10.txt"]);
        // Case-insensitive natural ordering: lowercase sorts before uppercase.
        assert_eq!(compare_names("alpha", "Zebra"), std::cmp::Ordering::Less);
        assert_eq!(compare_names("file2", "file10"), std::cmp::Ordering::Less);
    }

    #[test]
    fn sort_by_size_and_modified_respects_direction() {
        let mut entries = vec![
            entry("small", false, 10, 100),
            entry("big", false, 1000, 50),
            entry("mid", false, 100, 500),
        ];
        let order = SortOrder {
            key: SortKey::Size,
            ascending: false,
        };
        sort_entries(&mut entries, order);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.name_text.as_str())
                .collect::<Vec<_>>(),
            vec!["big", "mid", "small"]
        );

        let order = SortOrder {
            key: SortKey::Modified,
            ascending: true,
        };
        sort_entries(&mut entries, order);
        assert_eq!(
            entries
                .iter()
                .map(|entry| entry.name_text.as_str())
                .collect::<Vec<_>>(),
            vec!["big", "small", "mid"]
        );
    }

    #[test]
    fn sort_by_kind_uses_extension_then_name() {
        let mut entries = vec![
            entry("b.txt", false, 0, 0),
            entry("a.rs", false, 0, 0),
            entry("noext", false, 0, 0),
            entry("zeta.txt", false, 0, 0),
            entry("docs", true, 0, 0),
        ];
        sort_entries(
            &mut entries,
            SortOrder {
                key: SortKey::Kind,
                ascending: true,
            },
        );
        let names: Vec<_> = entries
            .iter()
            .map(|entry| entry.name_text.clone())
            .collect();
        assert_eq!(names, vec!["docs", "noext", "a.rs", "b.txt", "zeta.txt"]);

        let mut entries = vec![entry("b.txt", false, 0, 0), entry("a.rs", false, 0, 0)];
        sort_entries(
            &mut entries,
            SortOrder {
                key: SortKey::Kind,
                ascending: false,
            },
        );
        let names: Vec<_> = entries
            .iter()
            .map(|entry| entry.name_text.clone())
            .collect();
        assert_eq!(names, vec!["b.txt", "a.rs"]);
    }

    #[test]
    fn kind_labels_cover_dirs_links_and_extensions() {
        let mut file = entry("photo.PNG", false, 0, 0);
        assert_eq!(kind_label(&file), "png");
        file.name = OsString::from("noext");
        file.name_text = "noext".to_string();
        assert_eq!(kind_label(&file), "—");
        file.is_symlink = true;
        assert_eq!(kind_label(&file), "リンク");
        let dir = entry("docs", true, 0, 0);
        assert_eq!(kind_label(&dir), "フォルダ");
    }

    #[test]
    fn sort_toggle_direction_and_new_key_defaults() {
        let mut order = SortOrder::default();
        order = order.toggled(SortKey::Name);
        assert_eq!(order.key, SortKey::Name);
        assert!(!order.ascending, "clicking the active key flips direction");
        order = order.toggled(SortKey::Name);
        assert!(order.ascending);

        // Switching to a new key resets the direction from that key's default:
        // Name/Kind start ascending, Size/Modified start descending.
        order = order.toggled(SortKey::Kind);
        assert_eq!(order.key, SortKey::Kind);
        assert!(order.ascending, "a new Kind key must default to ascending");
        order = order.toggled(SortKey::Kind);
        assert!(
            !order.ascending,
            "the active Kind key must toggle to descending"
        );

        order = order.toggled(SortKey::Size);
        assert_eq!(order.key, SortKey::Size);
        assert!(
            !order.ascending,
            "a new Size key must default to descending"
        );
        order = order.toggled(SortKey::Size);
        assert!(order.ascending);

        order = order.toggled(SortKey::Modified);
        assert_eq!(order.key, SortKey::Modified);
        assert!(
            !order.ascending,
            "a new Modified key must default to descending"
        );
        order = order.toggled(SortKey::Modified);
        assert!(order.ascending);
    }

    #[test]
    fn human_size_units() {
        assert_eq!(human_size(0), "0 B");
        assert_eq!(human_size(1023), "1023 B");
        assert_eq!(human_size(1024), "1.0 K");
        assert_eq!(human_size(1536), "1.5 K");
        assert_eq!(human_size(1024 * 1024), "1.0 M");
        assert_eq!(human_size(3 * 1024 * 1024 * 1024), "3.0 G");
    }

    #[test]
    fn datetime_and_age_handle_missing_times() {
        assert_eq!(human_datetime(None), "-");
        assert_eq!(human_age(None), "-");

        // Exact shape check: YYYY-MM-DD HH:MM in local time (no timezone text).
        let formatted = human_datetime(Some(SystemTime::now()));
        let bytes = formatted.as_bytes();
        assert_eq!(
            bytes.len(),
            16,
            "expected YYYY-MM-DD HH:MM, got {formatted:?}"
        );
        for (index, &byte) in bytes.iter().enumerate() {
            match index {
                4 | 7 => assert_eq!(byte, b'-', "expected '-' at {index} in {formatted:?}"),
                10 => assert_eq!(byte, b' ', "expected space at {index} in {formatted:?}"),
                13 => assert_eq!(byte, b':', "expected ':' at {index} in {formatted:?}"),
                _ => assert!(
                    byte.is_ascii_digit(),
                    "expected digit at {index} in {formatted:?}"
                ),
            }
        }
        let year: u32 = formatted[0..4].parse().unwrap();
        let month: u32 = formatted[5..7].parse().unwrap();
        let day: u32 = formatted[8..10].parse().unwrap();
        let hour: u32 = formatted[11..13].parse().unwrap();
        let minute: u32 = formatted[14..16].parse().unwrap();
        assert!((1..=9999).contains(&year), "invalid year in {formatted:?}");
        assert!((1..=12).contains(&month), "invalid month in {formatted:?}");
        assert!((1..=31).contains(&day), "invalid day in {formatted:?}");
        assert!(hour < 24, "invalid hour in {formatted:?}");
        assert!(minute < 60, "invalid minute in {formatted:?}");

        assert_eq!(human_age(Some(SystemTime::now())), "たった今");
    }

    #[test]
    fn human_age_covers_all_relative_units() {
        let now = SystemTime::now();
        let future = now + Duration::from_secs(3600);
        assert_eq!(human_age(Some(future)), "未来");
        assert_eq!(human_age(Some(now - Duration::from_secs(120))), "2分前");
        assert_eq!(human_age(Some(now - Duration::from_secs(7200))), "2時間前");
        assert_eq!(human_age(Some(now - Duration::from_secs(172_800))), "2日前");
        assert_eq!(
            human_age(Some(now - Duration::from_secs(86_400 * 730))),
            "2年前"
        );

        // Boundary pairs: the unit changes exactly at each threshold, so a
        // regression from `<` to `<=` (or an off-by-one divisor) is caught.
        let boundary = |secs| human_age(Some(now - Duration::from_secs(secs)));
        assert_eq!(boundary(59), "たった今");
        assert_eq!(boundary(60), "1分前");
        assert_eq!(boundary(3599), "59分前");
        assert_eq!(boundary(3600), "1時間前");
        assert_eq!(boundary(86_399), "23時間前");
        assert_eq!(boundary(86_400), "1日前");
        assert_eq!(boundary(86_400 * 364), "364日前");
        assert_eq!(boundary(86_400 * 365), "1年前");
    }

    #[test]
    fn fingerprint_changes_with_content() {
        let base = vec![entry("a", false, 1, 10), entry("b", true, 0, 20)];
        let same = vec![entry("a", false, 1, 10), entry("b", true, 0, 20)];
        assert_eq!(fingerprint(&base), fingerprint(&same));
        let mut changed = same.clone();
        changed[0].size = 99;
        assert_ne!(fingerprint(&base), fingerprint(&changed));
        // A different `modified` alone must change the fingerprint: name,
        // is_dir and size are identical, only the timestamp is one second later.
        let mut newer_modified = same.clone();
        newer_modified[0].modified = Some(base[0].modified.unwrap() + Duration::from_secs(1));
        assert_ne!(fingerprint(&base), fingerprint(&newer_modified));
        let mut renamed = same.clone();
        renamed[1].name = OsString::from("c");
        renamed[1].name_text = "c".to_string();
        assert_ne!(fingerprint(&base), fingerprint(&renamed));

        let mut pre_epoch = same.clone();
        pre_epoch[0].modified = Some(UNIX_EPOCH - Duration::from_secs(1));
        // Pre-epoch timestamps hash as None, so an otherwise identical entry
        // set with `modified: None` must produce the same fingerprint.
        let mut same_entries_with_modified_none = same.clone();
        same_entries_with_modified_none[0].modified = None;
        assert_eq!(
            fingerprint(&pre_epoch),
            fingerprint(&same_entries_with_modified_none)
        );
    }

    #[cfg(unix)]
    #[test]
    fn dangling_symlink_is_identical_from_both_constructors() {
        let dir = scratch_dir("dangling-symlink");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let link = dir.join("dangling");
        std::os::unix::fs::symlink(dir.join("no-such-target"), &link).unwrap();

        let dir_entry = fs::read_dir(&dir)
            .unwrap()
            .filter_map(Result::ok)
            .find(|item| item.file_name() == std::ffi::OsStr::new("dangling"))
            .expect("dangling symlink should appear in read_dir");
        let from_dir = Entry::from_dir_entry(dir_entry).expect("entry from read_dir");
        let from_path = entry_for_path(&link).expect("entry from path");

        assert!(from_dir.is_symlink);
        assert!(from_path.is_symlink);
        assert_eq!(from_dir.is_dir, from_path.is_dir);
        assert_eq!(from_dir.size, from_path.size);
        assert_eq!(from_dir.modified, from_path.modified);
        assert_eq!(
            from_dir.size, 0,
            "unresolvable target must not report link length"
        );
        assert_eq!(from_dir.modified, None);

        // A watcher rebuild via `entry_for_path` must not change the fingerprint.
        assert_eq!(
            fingerprint(std::slice::from_ref(&from_dir)),
            fingerprint(std::slice::from_ref(&from_path))
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn parent_dir_handles_root_and_relative_paths() {
        assert_eq!(parent_dir(Path::new("/")), None);
        assert_eq!(parent_dir(Path::new("/a")), Some(PathBuf::from("/")));
        assert_eq!(parent_dir(Path::new("/a/b")), Some(PathBuf::from("/a")));
        assert_eq!(parent_dir(Path::new("a")), Some(PathBuf::from("")));
    }

    #[test]
    fn breadcrumb_segments_build_accumulated_paths() {
        let segments = breadcrumb_segments(Path::new("/home/koppe"));
        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0], ("/".to_string(), PathBuf::from("/")));
        assert_eq!(segments[1], ("home".to_string(), PathBuf::from("/home")));
        assert_eq!(
            segments[2],
            ("koppe".to_string(), PathBuf::from("/home/koppe"))
        );

        // CurDir / ParentDir produce no label, but remain in the accumulated
        // path so each breadcrumb still points at the original location.
        let relative = breadcrumb_segments(Path::new("./a/../b"));
        assert_eq!(
            relative,
            vec![
                ("a".to_string(), PathBuf::from("./a")),
                ("b".to_string(), PathBuf::from("./a/../b")),
            ]
        );
    }

    #[test]
    fn unique_child_name_avoids_existing_entries() {
        let dir = scratch_dir("unique");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert_eq!(unique_child_name(&dir, "新しいフォルダ"), "新しいフォルダ");
        fs::create_dir(dir.join("新しいフォルダ")).unwrap();
        assert_eq!(
            unique_child_name(&dir, "新しいフォルダ"),
            "新しいフォルダ (2)"
        );
        fs::create_dir(dir.join("新しいフォルダ (2)")).unwrap();
        assert_eq!(
            unique_child_name(&dir, "新しいフォルダ"),
            "新しいフォルダ (3)"
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn display_path_is_lossless_for_utf8() {
        assert_eq!(display_path(Path::new("/tmp/日本語")), "/tmp/日本語");
    }
    #[test]
    fn selection_clamping_handles_empty_and_bounds() {
        assert_eq!(clamp_index(0, 0), 0);
        assert_eq!(clamp_index(5, 0), 0);
        assert_eq!(clamp_index(0, 3), 0);
        assert_eq!(clamp_index(2, 3), 2);
        assert_eq!(clamp_index(9, 3), 2);
    }

    #[test]
    fn selection_steps_are_clamped() {
        assert_eq!(step_selection(0, 0, 1), 0);
        assert_eq!(step_selection(0, 3, 1), 1);
        assert_eq!(step_selection(2, 3, 1), 2);
        assert_eq!(step_selection(0, 3, -1), 0);
        assert_eq!(step_selection(2, 3, -5), 0);
        assert_eq!(step_selection(1, 3, 99), 2);
        assert_eq!(first_index(0), 0);
        assert_eq!(last_index(0), 0);
        assert_eq!(first_index(10), 0);
        assert_eq!(last_index(10), 9);
    }

    #[test]
    fn sort_spec_roundtrips_with_sort_order() {
        let spec = SortSpec::default();
        assert_eq!(spec.order(), SortOrder::default());
        assert_eq!(
            spec.order(),
            SortOrder {
                key: SortKey::Name,
                ascending: true
            }
        );

        let descending_size = SortOrder {
            key: SortKey::Size,
            ascending: false,
        };
        let spec = SortSpec::from_order(descending_size);
        assert_eq!(spec.dir, SortDir::Desc);
        assert!(spec.dirs_first);
        assert_eq!(spec.order(), descending_size);

        // The persisted JSON shape matches DESIGN.md §8.1.
        let json = serde_json::to_string(&SpecWrapper { sort: spec }).unwrap();
        assert!(json.contains(r#""key":"size""#), "json was {json}");
        assert!(json.contains(r#""dir":"desc""#), "json was {json}");
        assert!(json.contains(r#""dirs_first":true"#), "json was {json}");
    }

    #[derive(serde::Serialize)]
    struct SpecWrapper {
        sort: SortSpec,
    }

    #[test]
    fn sort_spec_deserializes_missing_fields_with_defaults() {
        let spec: SortSpec = serde_json::from_str("{}").unwrap();
        assert_eq!(spec, SortSpec::default());
        let spec: SortSpec = serde_json::from_str(r#"{"key": "modified"}"#).unwrap();
        assert_eq!(spec.key, SortKey::Modified);
        assert_eq!(spec.dir, SortDir::Asc);
        assert!(spec.dirs_first);
    }

    #[test]
    fn unknown_sort_dir_falls_back_without_losing_siblings() {
        let spec: SortSpec =
            serde_json::from_str(r#"{"key":"size","dir":"ascending","dirs_first":false}"#).unwrap();
        assert_eq!(spec.key, SortKey::Size);
        assert_eq!(spec.dir, SortDir::Asc);
        assert!(!spec.dirs_first);

        let valid: SortSpec =
            serde_json::from_str(r#"{"key":"kind","dir":"desc","dirs_first":true}"#).unwrap();
        assert_eq!(valid.dir, SortDir::Desc);
    }

    #[test]
    fn create_unique_folder_retries_and_creates_directory() {
        let dir = scratch_dir("create-folder");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let first = create_unique_folder(&dir).expect("first folder");
        let second = create_unique_folder(&dir).expect("second folder");
        assert_ne!(first, second);
        assert!(first.is_dir());
        assert!(second.is_dir());
        assert_eq!(
            first.file_name().unwrap().to_string_lossy(),
            "新しいフォルダ"
        );
        assert_eq!(
            second.file_name().unwrap().to_string_lossy(),
            "新しいフォルダ (2)"
        );

        fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn create_unique_folder_skips_dangling_symlink_name() {
        let dir = scratch_dir("dangling-folder");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let link = dir.join("新しいフォルダ");
        std::os::unix::fs::symlink(dir.join("no-such-target"), &link).unwrap();

        assert!(!link.exists(), "the symlink is dangling");
        assert!(link.is_symlink());
        assert_eq!(
            unique_child_name(&dir, "新しいフォルダ"),
            "新しいフォルダ (2)"
        );

        let created = create_unique_folder(&dir).expect("must pick a free name");
        assert_eq!(
            created.file_name().unwrap().to_string_lossy(),
            "新しいフォルダ (2)"
        );
        assert!(created.is_dir());
        assert!(link.is_symlink(), "the dangling symlink must be untouched");

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn create_unique_folder_reports_missing_parent() {
        let dir = scratch_dir("create-missing-parent");
        let _ = fs::remove_dir_all(&dir);
        let missing = dir.join("no-such-parent");
        let error = create_unique_folder(&missing).expect_err("missing parent must fail");
        assert!(error.contains("no-such-parent"), "unexpected: {error}");
    }
}

#[cfg(test)]
mod selection_tests {
    use super::Selection;
    use std::path::{Path, PathBuf};

    fn paths(names: &[&str]) -> Vec<PathBuf> {
        names.iter().map(PathBuf::from).collect()
    }

    #[test]
    fn single_and_toggle_selection() {
        let mut selection = Selection::default();
        selection.select_single(PathBuf::from("/a"));
        assert_eq!(selection.len(), 1);
        assert!(selection.contains(Path::new("/a")));

        selection.toggle(PathBuf::from("/b"));
        assert_eq!(selection.len(), 2);
        assert!(selection.contains(Path::new("/b")));

        selection.toggle(PathBuf::from("/a"));
        assert_eq!(selection.len(), 1);
        assert!(!selection.contains(Path::new("/a")));
        assert!(selection.contains(Path::new("/b")));
    }

    #[test]
    fn range_selection_uses_anchor() {
        let entries = paths(&["/a", "/b", "/c", "/d"]);
        let mut selection = Selection::default();
        selection.select_single(PathBuf::from("/b"));
        selection.select_range(&entries, Path::new("/d"));
        assert_eq!(
            selection.paths(),
            &paths(&["/b", "/c", "/d"])[..],
            "range must include both endpoints"
        );

        selection.select_range(&entries, Path::new("/a"));
        assert_eq!(selection.paths(), &paths(&["/a", "/b"])[..]);
    }

    #[test]
    fn range_selection_without_usable_anchor_falls_back_to_single() {
        let entries = paths(&["/a", "/b"]);
        let mut selection = Selection::default();
        // No anchor yet: selecting a range behaves like a single selection.
        selection.select_range(&entries, Path::new("/b"));
        assert_eq!(selection.paths(), &paths(&["/b"])[..]);

        // An anchor outside the visible entries also falls back.
        let mut outside = Selection::default();
        outside.select_single(PathBuf::from("/gone"));
        outside.select_range(&entries, Path::new("/a"));
        assert_eq!(outside.paths(), &paths(&["/a"])[..]);

        // An unknown target leaves the existing selection untouched.
        outside.select_range(&entries, Path::new("/missing"));
        assert_eq!(outside.paths(), &paths(&["/a"])[..]);
    }

    #[test]
    fn select_all_and_retain() {
        let entries = paths(&["/a", "/b", "/c"]);
        let mut selection = Selection::default();
        selection.select_all(&entries);
        assert_eq!(selection.len(), 3);

        let remaining = paths(&["/b", "/c", "/d"]);
        selection.retain(&remaining);
        assert_eq!(selection.paths(), &paths(&["/b", "/c"])[..]);

        selection.retain(&[]);
        assert!(selection.is_empty());
    }

    #[test]
    fn select_all_empty_clears_stale_anchor() {
        let entries = paths(&["/a", "/b", "/c"]);
        let mut selection = Selection::default();
        selection.select_single(PathBuf::from("/b"));
        assert!(selection.contains(Path::new("/b")));

        // Selecting all of an empty listing empties the selection and must
        // drop the anchor left by the earlier single selection.
        selection.select_all(&[]);
        assert!(selection.is_empty());

        // A stale "/b" anchor would turn this into the range "/b"..="/c".
        // Without an anchor, select_range behaves like a single selection.
        selection.select_range(&entries, Path::new("/c"));
        assert_eq!(selection.paths(), &paths(&["/c"])[..]);
    }

    #[test]
    fn hash_index_stays_in_sync_with_ordered_paths() {
        let entries = paths(&["/a", "/b", "/c", "/d"]);
        let mut selection = Selection::default();

        let assert_in_sync = |selection: &Selection| {
            for path in selection.paths() {
                assert!(selection.contains(path), "missing index entry for {path:?}");
            }
            for path in &entries {
                assert_eq!(
                    selection.contains(path),
                    selection.paths().contains(path),
                    "index disagrees with ordered paths for {path:?}"
                );
            }
        };

        selection.select_all(&entries);
        assert_in_sync(&selection);
        selection.toggle(PathBuf::from("/b"));
        assert_in_sync(&selection);
        selection.select_single(PathBuf::from("/c"));
        assert_in_sync(&selection);
        selection.select_range(&entries, Path::new("/a"));
        assert_in_sync(&selection);
        selection.retain(&paths(&["/a", "/c"]));
        assert_in_sync(&selection);
        selection.clear();
        assert_in_sync(&selection);
    }
}

#[cfg(test)]
mod grid_tests {
    use super::{grid_columns, grid_row_count};

    #[test]
    fn columns_are_at_least_one() {
        assert_eq!(grid_columns(0.0, 100.0), 1);
        assert_eq!(grid_columns(50.0, 100.0), 1);
        assert_eq!(grid_columns(1000.0, 100.0), 10);
        assert_eq!(grid_columns(-5.0, 100.0), 1);
        assert_eq!(grid_columns(1000.0, 0.0), 1);
    }

    #[test]
    fn row_count_rounds_up() {
        assert_eq!(grid_row_count(0, 4), 0);
        assert_eq!(grid_row_count(1, 4), 1);
        assert_eq!(grid_row_count(4, 4), 1);
        assert_eq!(grid_row_count(5, 4), 2);
        assert_eq!(grid_row_count(8, 4), 2);
        assert_eq!(grid_row_count(5, 0), 0);
    }
}

#[cfg(test)]
mod permission_tests {
    use super::permissions_string;

    #[test]
    fn renders_common_modes() {
        assert_eq!(permissions_string(0o755), "rwxr-xr-x");
        assert_eq!(permissions_string(0o644), "rw-r--r--");
        assert_eq!(permissions_string(0o600), "rw-------");
        assert_eq!(permissions_string(0o000), "---------");
        assert_eq!(permissions_string(0o777), "rwxrwxrwx");
    }
}

#[cfg(test)]
mod location_tests {
    use super::resolve_location_input;
    use std::path::{Path, PathBuf};

    fn cwd() -> PathBuf {
        PathBuf::from("/work/dir")
    }

    fn home() -> PathBuf {
        PathBuf::from("/home/user")
    }

    #[test]
    fn empty_input_is_rejected() {
        assert!(resolve_location_input(&cwd(), "   ", Some(&home())).is_err());
    }

    #[test]
    fn absolute_paths_are_used_as_is() {
        assert_eq!(
            resolve_location_input(&cwd(), "/tmp", Some(&home())).unwrap(),
            PathBuf::from("/tmp")
        );
    }

    #[test]
    fn relative_paths_resolve_against_cwd() {
        assert_eq!(
            resolve_location_input(&cwd(), "sub/../x", Some(&home())).unwrap(),
            PathBuf::from("/work/dir/sub/../x")
        );
    }

    #[test]
    fn tilde_expands_to_home() {
        assert_eq!(
            resolve_location_input(&cwd(), "~", Some(&home())).unwrap(),
            home()
        );
        assert_eq!(
            resolve_location_input(&cwd(), "~/docs", Some(&home())).unwrap(),
            Path::new("/home/user/docs")
        );
        assert!(resolve_location_input(&cwd(), "~/docs", None).is_err());
        assert!(resolve_location_input(&cwd(), "~", None).is_err());
    }

    #[test]
    fn other_tilde_forms_are_literal() {
        assert_eq!(
            resolve_location_input(&cwd(), "~user", Some(&home())).unwrap(),
            PathBuf::from("/work/dir/~user")
        );
    }
}

#[cfg(test)]
mod personalization_tests {
    use super::{
        BookmarkLabel, SavedSearch, MAX_BOOKMARK_LABEL_BYTES, MAX_SAVED_SEARCH_NAME_BYTES,
        MAX_SAVED_SEARCH_QUERY_BYTES,
    };
    use std::path::Path;

    #[test]
    fn saved_search_new_trims_and_rejects_invalid_text() {
        let search = SavedSearch::new("  TODO  ", "  name:rs  ").unwrap();
        assert_eq!(search.name, "TODO");
        assert_eq!(search.query, "name:rs");
        assert_eq!(search.name_key(), "todo");

        assert!(SavedSearch::new("   ", "q").is_none());
        assert!(SavedSearch::new("n", "  ").is_none());
        assert!(SavedSearch::new("bad\0name", "q").is_none());
        assert!(SavedSearch::new("n", "bad\0query").is_none());
        assert!(SavedSearch::new(&"n".repeat(MAX_SAVED_SEARCH_NAME_BYTES + 1), "q").is_none());
        assert!(SavedSearch::new("n", &"q".repeat(MAX_SAVED_SEARCH_QUERY_BYTES + 1)).is_none());
    }

    #[test]
    fn saved_search_json_roundtrip_and_partial_defaults() {
        let search = SavedSearch::new("todos", "TODO").unwrap();
        let text = serde_json::to_string(&search).unwrap();
        assert!(text.contains("\"name\":\"todos\""));
        assert!(text.contains("\"query\":\"TODO\""));
        assert_eq!(serde_json::from_str::<SavedSearch>(&text).unwrap(), search);

        // A partial object is tolerated by serde but is not valid for storage;
        // Settings::validate drops such entries on load.
        let partial: SavedSearch = serde_json::from_str(r#"{"name":"todos"}"#).unwrap();
        assert!(partial.query.is_empty());
        assert!(!partial.is_valid());
    }

    #[test]
    fn bookmark_label_new_trims_and_rejects_invalid_input() {
        let entry = BookmarkLabel::new(Path::new("/home/koppe"), " home ").unwrap();
        assert_eq!(entry.path, Path::new("/home/koppe"));
        assert_eq!(entry.label, "home");
        assert!(entry.is_valid());

        assert!(BookmarkLabel::new(Path::new(""), "home").is_none());
        assert!(BookmarkLabel::new(Path::new("/a"), "   ").is_none());
        assert!(BookmarkLabel::new(Path::new("/a"), "bad\0label").is_none());
        assert!(
            BookmarkLabel::new(Path::new("/a"), &"x".repeat(MAX_BOOKMARK_LABEL_BYTES + 1))
                .is_none()
        );
    }

    #[test]
    fn bookmark_label_json_roundtrip_and_partial_defaults() {
        let entry = BookmarkLabel::new(Path::new("/home/koppe"), "home").unwrap();
        let text = serde_json::to_string(&entry).unwrap();
        assert!(text.contains("\"path\":\"/home/koppe\""));
        assert!(text.contains("\"label\":\"home\""));
        assert_eq!(serde_json::from_str::<BookmarkLabel>(&text).unwrap(), entry);

        let partial: BookmarkLabel = serde_json::from_str(r#"{"path":"/a"}"#).unwrap();
        assert!(!partial.is_valid());
    }
}
