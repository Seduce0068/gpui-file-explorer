// SPDX-License-Identifier: AGPL-3.0-or-later
//! Filesystem data model: directory scanning, filtering, sorting and
//! human-readable formatting.
//!
//! This module is intentionally free of GPUI types so it can be unit tested
//! headlessly (`cargo test`) and reused on any platform: the filesystem access
//! is plain `std::fs`, and only the UI layer depends on gpui.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use alphanumeric_sort::compare_str;
use serde::{Deserialize, Serialize};

/// One entry of a directory listing.
#[derive(Clone, Debug)]
pub struct Entry {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    /// 1-based line number of the first content match (content search only).
    pub match_line: Option<u32>,
}

impl Entry {
    /// Build an [`Entry`] from a `read_dir` item. Returns `None` for entries
    /// whose name cannot be represented (rare, platform dependent).
    fn from_dir_entry(item: std::fs::DirEntry) -> Option<Entry> {
        let path = item.path();
        let name = item.file_name().to_string_lossy().to_string();
        if name.is_empty() {
            return None;
        }
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
            path,
            is_dir,
            is_symlink,
            size,
            modified,
            match_line: None,
        })
    }
}

/// Blocking directory read. Callers on the UI thread must run this on the
/// background executor (see `Explorer::reload`).
pub fn read_entries(dir: &Path) -> Result<Vec<Entry>, String> {
    let dir_reader = std::fs::read_dir(dir).map_err(|err| format!("{}: {err}", dir.display()))?;
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
/// for size/time so symlinked directories remain enterable.
pub fn entry_for_path(path: &Path) -> Option<Entry> {
    let name = path.file_name()?.to_string_lossy().into_owned();
    if name.is_empty() {
        return None;
    }
    let link_metadata = std::fs::symlink_metadata(path).ok()?;
    let is_symlink = link_metadata.file_type().is_symlink();
    let metadata = std::fs::metadata(path).ok().or(Some(link_metadata));
    let is_dir = metadata.as_ref().is_some_and(|meta| meta.is_dir());
    let size = metadata.as_ref().map(|meta| meta.len()).unwrap_or(0);
    let modified = metadata.and_then(|meta| meta.modified().ok());
    Some(Entry {
        name,
        path: path.to_path_buf(),
        is_dir,
        is_symlink,
        size,
        modified,
        match_line: None,
    })
}

/// Multi-selection state, keyed by path so it survives reloads and re-sorting.
///
/// The list keeps a separate "focused" row index; this struct only tracks which
/// paths are selected and the anchor used for shift-range selection.
#[derive(Clone, Debug, Default)]
pub struct Selection {
    paths: Vec<PathBuf>,
    anchor: Option<PathBuf>,
}

impl Selection {
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
        self.paths.iter().any(|selected| selected == path)
    }

    pub fn clear(&mut self) {
        self.paths.clear();
        self.anchor = None;
    }

    /// Replace the selection with a single path (plain click / arrow key).
    pub fn select_single(&mut self, path: PathBuf) {
        self.paths = vec![path.clone()];
        self.anchor = Some(path);
    }

    /// Ctrl+click: add or remove one path; the anchor follows the click.
    pub fn toggle(&mut self, path: PathBuf) {
        if let Some(position) = self.paths.iter().position(|selected| selected == &path) {
            self.paths.remove(position);
            if self.anchor.as_deref() == Some(path.as_path()) {
                self.anchor = self.paths.last().cloned();
            }
        } else {
            self.paths.push(path.clone());
            self.anchor = Some(path);
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
        self.paths = entries[start..=end].to_vec();
    }

    pub fn select_all(&mut self, entries: &[PathBuf]) {
        self.paths = entries.to_vec();
        if self.anchor.is_none() {
            self.anchor = entries.first().cloned();
        }
    }

    /// Drop paths that no longer exist in the current listing.
    pub fn retain(&mut self, entries: &[PathBuf]) {
        self.paths.retain(|path| entries.contains(path));
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
fn extension_of(name: &str) -> String {
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
        return "フォルダ".to_string();
    }
    if entry.is_symlink {
        return "リンク".to_string();
    }
    let extension = extension_of(&entry.name);
    if extension.is_empty() {
        "—".to_string()
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
        extension_of(&entry.name)
    }
}

/// Filter hidden entries and sort, producing the list that is displayed.
pub fn visible_entries(all: &[Entry], show_hidden: bool, order: SortOrder) -> Vec<Entry> {
    let mut view: Vec<Entry> = all
        .iter()
        .filter(|entry| show_hidden || !is_hidden_name(&entry.name))
        .cloned()
        .collect();
    sort_entries(&mut view, order);
    view
}

/// Natural, case-insensitive name comparison ("file2" < "file10", "alpha" <
/// "Zebra"), with a case-sensitive pass as deterministic tie-breaker.
pub fn compare_names(a: &str, b: &str) -> std::cmp::Ordering {
    compare_str(a.to_lowercase(), b.to_lowercase()).then_with(|| compare_str(a, b))
}

/// Stable natural sort: directories first, then the chosen key with a natural
/// alphanumeric name comparison as tie-breaker.
pub fn sort_entries(entries: &mut Vec<Entry>, order: SortOrder) {
    let (mut dirs, mut files): (Vec<Entry>, Vec<Entry>) = std::mem::take(entries)
        .into_iter()
        .partition(|entry| entry.is_dir);

    let cmp = |a: &Entry, b: &Entry| {
        let primary = match order.key {
            SortKey::Name => compare_names(&a.name, &b.name),
            SortKey::Kind => kind_sort_key(a).cmp(&kind_sort_key(b)),
            SortKey::Size => a.size.cmp(&b.size),
            // `None` (unreadable timestamp) sorts before any timestamp.
            SortKey::Modified => a.modified.cmp(&b.modified),
        };
        primary.then_with(|| compare_names(&a.name, &b.name))
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
    pub key: SortKey,
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
        return "-".to_string();
    };
    let datetime: chrono::DateTime<chrono::Local> = time.into();
    datetime.format("%Y-%m-%d %H:%M").to_string()
}

/// Relative age (`3分前`, `2時間前`, ...) used in the status bar.
pub fn human_age(time: Option<SystemTime>) -> String {
    let Some(time) = time else {
        return "-".to_string();
    };
    let now = SystemTime::now();
    let Ok(elapsed) = now.duration_since(time) else {
        return "未来".to_string();
    };
    let secs = elapsed.as_secs();
    if secs < 60 {
        "たった今".to_string()
    } else if secs < 3600 {
        format!("{}分前", secs / 60)
    } else if secs < 86_400 {
        format!("{}時間前", secs / 3600)
    } else if secs < 86_400 * 365 {
        format!("{}日前", secs / 86_400)
    } else {
        format!("{}年前", secs / (86_400 * 365))
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
    while dir.join(&candidate).exists() {
        candidate = format!("{base} ({counter})");
        counter += 1;
        if counter > 10_000 {
            break;
        }
    }
    candidate
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
        fingerprint, first_index, human_age, human_datetime, human_size, is_hidden_name,
        kind_label, last_index, read_entries, sort_entries, step_selection, unique_child_name,
        visible_entries, Entry, SortDir, SortKey, SortOrder, SortSpec,
    };
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, SystemTime};

    fn scratch_dir(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("gpui-fe-model-{tag}-{}", std::process::id()))
    }

    fn entry(name: &str, is_dir: bool, size: u64, modified_secs: u64) -> Entry {
        Entry {
            name: name.to_string(),
            path: PathBuf::from("/tmp").join(name),
            is_dir,
            is_symlink: false,
            size,
            modified: Some(SystemTime::UNIX_EPOCH + Duration::from_secs(modified_secs)),
            match_line: None,
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
            .find(|entry| entry.name == "beta.txt")
            .unwrap();
        assert_eq!(beta.size, 11);
        assert!(beta.modified.is_some());
        assert!(beta.path.ends_with("beta.txt"));
        assert!(!beta.is_symlink);

        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn read_entries_reports_errors() {
        let missing = scratch_dir("missing");
        let _ = fs::remove_dir_all(&missing);
        assert!(read_entries(&missing).is_err());
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
                .map(|entry| entry.name.as_str())
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
        let names: Vec<&str> = entries.iter().map(|entry| entry.name.as_str()).collect();
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
                .map(|entry| entry.name.as_str())
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
                .map(|entry| entry.name.as_str())
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
        let names: Vec<_> = entries.iter().map(|entry| entry.name.clone()).collect();
        assert_eq!(names, vec!["docs", "noext", "a.rs", "b.txt", "zeta.txt"]);

        let mut entries = vec![entry("b.txt", false, 0, 0), entry("a.rs", false, 0, 0)];
        sort_entries(
            &mut entries,
            SortOrder {
                key: SortKey::Kind,
                ascending: false,
            },
        );
        let names: Vec<_> = entries.iter().map(|entry| entry.name.clone()).collect();
        assert_eq!(names, vec!["b.txt", "a.rs"]);
    }

    #[test]
    fn kind_labels_cover_dirs_links_and_extensions() {
        let mut file = entry("photo.PNG", false, 0, 0);
        assert_eq!(kind_label(&file), "png");
        file.name = "noext".to_string();
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
        assert!(!order.ascending);
        // Switching key resets direction: size starts descending.
        order = order.toggled(SortKey::Size);
        assert_eq!(order.key, SortKey::Size);
        assert!(!order.ascending);
        order = order.toggled(SortKey::Size);
        assert!(order.ascending);
        order = order.toggled(SortKey::Modified);
        assert!(!order.ascending);
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
        let now = SystemTime::now();
        assert_eq!(human_datetime(Some(now)).len(), 16); // YYYY-MM-DD HH:MM
        assert!(human_datetime(Some(now)).contains(':'));
        assert_eq!(human_age(Some(SystemTime::now())), "たった今");
    }

    #[test]
    fn fingerprint_changes_with_content() {
        let base = vec![entry("a", false, 1, 10), entry("b", true, 0, 20)];
        let same = vec![entry("a", false, 1, 10), entry("b", true, 0, 20)];
        assert_eq!(fingerprint(&base), fingerprint(&same));
        let mut changed = same.clone();
        changed[0].size = 99;
        assert_ne!(fingerprint(&base), fingerprint(&changed));
        let mut renamed = same.clone();
        renamed[1].name = "c".to_string();
        assert_ne!(fingerprint(&base), fingerprint(&renamed));
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
