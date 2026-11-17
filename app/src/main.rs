// SPDX-License-Identifier: AGPL-3.0-or-later
//! GPUI file explorer prototype — Linux/Wayland first, cross-platform design.
//!
//! Stack: `gpui-kit` 0.9.1 (GPUI snapshot + GPUI Component + Lucide assets).
//! See `../README.md` for build/run instructions, key bindings and known
//! limitations, and `../PROGRESS.md` for the implementation log.

#![forbid(unsafe_code)]

mod history;
mod icons;
mod model;
mod mounts;
mod ops;
mod preview;
mod search;
mod settings;
mod strings;
mod tabs;
mod trash_view;
mod undo;
mod watcher;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tracing_subscriber::EnvFilter;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_kit::component::{Icon, Theme, ThemeMode};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::history::{History, NavMode};
use crate::icons::{icon_for_entry, kind_for_entry, FileKind};
use crate::model::{
    breadcrumb_segments, clamp_index, create_unique_folder, display_path, fingerprint, first_index,
    grid_columns, grid_row_count, human_age, human_datetime, human_size, last_index, parent_dir,
    read_entries, resolve_location_input, step_selection, visible_entries, Entry, Selection,
    SortKey, SortSpec,
};
use crate::mounts::MountPoint;
use crate::ops::{CancelFlag, ConflictPolicy, OpError, OpStats};
use crate::search::{
    filter_results, merge_search_results, search_by_name, search_contents, search_roots,
    ContentSearchOptions, SearchKind, SearchMode, SearchOptions, SearchScope,
};
use crate::settings::{Settings, ViewLayout};
use crate::tabs::{Tab, Tabs};
use crate::undo::{UndoAction, UndoStack};
use crate::watcher::{removed_paths, DirWatcher, FsEvent};

// Catppuccin Mocha-ish palette.
const C_BASE: u32 = 0x1e1e2e;
const C_MANTLE: u32 = 0x181825;
const C_SURFACE: u32 = 0x313244;
const C_OVERLAY: u32 = 0x45475a;
const C_TEXT: u32 = 0xcdd6f4;
const C_SUBTEXT: u32 = 0xa6adc8;
const C_MUTED: u32 = 0x6c7086;
const C_DIM: u32 = 0x585b70;
const C_BLUE: u32 = 0x89b4fa;
const C_GREEN: u32 = 0xa6e3a1;
const C_YELLOW: u32 = 0xf9e2af;
const C_RED: u32 = 0xf38ba8;
const C_MAUVE: u32 = 0xcba6f7;
const C_TEAL: u32 = 0x94e2d5;
const C_LAVENDER: u32 = 0xb4befe;

const RECONCILE_INTERVAL: Duration = Duration::from_secs(10);
const SELF_OP_SUPPRESS: Duration = Duration::from_millis(1500);
/// Throttle progress-bar updates so a fast copy does not flood the UI.
const PROGRESS_THROTTLE: Duration = Duration::from_millis(80);

/// Layout metrics shared by the list page-step and the icon grid. Keeping
/// them named avoids drift between the row height and the grid cell height.
const ROW_HEIGHT_PX: f64 = 28.0;
const LIST_CHROME_HEIGHT_PX: f64 = 140.0;
const SIDEBAR_WIDTH_PX: f32 = 214.0;
const GRID_CELL_WIDTH_PX: f32 = 108.0;

/// Icon colour per file kind (folders are blue, everything else is tinted).
fn kind_color(kind: FileKind) -> u32 {
    match kind {
        FileKind::Folder => C_BLUE,
        FileKind::Image => C_GREEN,
        FileKind::Code => C_LAVENDER,
        FileKind::Archive => C_YELLOW,
        FileKind::Pdf => C_RED,
        FileKind::Text => C_SUBTEXT,
        FileKind::Audio => C_MAUVE,
        FileKind::Video => C_TEAL,
        FileKind::Unknown => C_MUTED,
    }
}

/// Expression in the status bar (relaxed English/Japanese)
fn format_selected(entry: &Entry) -> String {
    if entry.is_dir {
        format!("{}（フォルダ）", entry.name_text)
    } else {
        format!(
            "{}（{} / {}）",
            entry.name_text,
            human_size(entry.size),
            human_age(entry.modified)
        )
    }
}

/// Name shown for a row. Search results show their path relative to the
/// search root so duplicates in different folders are distinguishable.
/// For multi-root searches, fall back to another matching root or the full path.
fn display_name_for(
    entry: &Entry,
    view: ViewSource,
    root: &std::path::Path,
    search_roots: &[PathBuf],
) -> String {
    if view == ViewSource::Search {
        if let Ok(relative) = entry.path.strip_prefix(root) {
            return relative.display().to_string();
        }
        let mut best: Option<String> = None;
        for search_root in search_roots {
            if let Ok(relative) = entry.path.strip_prefix(search_root) {
                let text = relative.display().to_string();
                if best
                    .as_ref()
                    .is_none_or(|current| text.len() < current.len())
                {
                    best = Some(text);
                }
            }
        }
        if let Some(relative) = best {
            return relative;
        }
        return entry.path.display().to_string();
    }
    entry.name_text.clone()
}

/// Label for a tab: the directory's last component, or "ルート" for `/`.
fn tab_label(path: &std::path::Path) -> String {
    if path == std::path::Path::new("/") {
        return "ルート".to_string();
    }
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| display_path(path))
}

/// One-line result message for a completed copy/move operation.
fn summarize_op(verb: &str, stats: &OpStats) -> String {
    let mut parts: Vec<String> = Vec::new();
    if stats.files > 0 {
        parts.push(format!("{} {OP_FILES_SUFFIX}", stats.files));
    }
    if stats.dirs > 0 {
        parts.push(format!("{} {OP_DIRS_SUFFIX}", stats.dirs));
    }
    if !stats.skipped.is_empty() {
        parts.push(format!("{} {OP_SKIPPED_SUFFIX}", stats.skipped.len()));
    }
    let mut message = format!("{verb}{OP_DONE_SUFFIX}");
    if !parts.is_empty() {
        message.push_str(&format!("（{}）", parts.join(" / ")));
    }
    if let Some(dest) = stats.created.first() {
        if let Some(name) = dest.file_name() {
            message.push_str(&format!(": {}", name.to_string_lossy()));
        }
    }
    message
}

/// Which listing the window currently shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewSource {
    Directory,
    Trash,
    Search,
}

/// Message + optional hint shown in the middle of the list area when there is
/// nothing to render (or an error/loading state is active).
fn empty_state_message(
    view: ViewSource,
    loading: bool,
    error: Option<&str>,
    all_entries: usize,
    entries: usize,
) -> Option<(String, Option<String>)> {
    if let Some(error) = error {
        return Some((format!("エラー: {error}"), None));
    }
    if loading {
        return Some((LOADING.to_string(), None));
    }
    if all_entries == 0 {
        return Some(match view {
            ViewSource::Trash => (EMPTY_TRASH.0.to_string(), Some(EMPTY_TRASH.1.to_string())),
            ViewSource::Search => (EMPTY_SEARCH.0.to_string(), Some(EMPTY_SEARCH.1.to_string())),
            ViewSource::Directory => (
                EMPTY_DIRECTORY.0.to_string(),
                Some(EMPTY_DIRECTORY.1.to_string()),
            ),
        });
    }
    if entries == 0 {
        return Some(if view == ViewSource::Search {
            (
                EMPTY_FILTERED.0.to_string(),
                Some(EMPTY_FILTERED.1.to_string()),
            )
        } else {
            (
                format!("隠しファイル {all_entries} 件を非表示にしています（Ctrl+H で表示）"),
                None,
            )
        });
    }
    None
}

use crate::strings::{
    EMPTY_DIRECTORY, EMPTY_FILTERED, EMPTY_SEARCH, EMPTY_TRASH, HELP_SECTIONS, LOADING,
    OP_DIRS_SUFFIX, OP_DONE_SUFFIX, OP_FILES_SUFFIX, OP_SKIPPED_SUFFIX,
    STATUS_BOOKMARKS_ONLY_DIRECTORY, STATUS_CANCELLING, STATUS_CHANGED, STATUS_DROP_EMPTY,
    STATUS_DROP_NOT_FOUND, STATUS_DROP_NO_DESTINATION, STATUS_DROP_ONLY_DIRECTORY,
    STATUS_DROP_SAME_DIRECTORY, STATUS_FOLDER_CREATING, STATUS_LAST_TAB, STATUS_NOT_DIRECTORY,
    STATUS_NO_CLIPBOARD, STATUS_OP_RUNNING, STATUS_PATH_GONE, STATUS_REDO_NO_TARGET,
    STATUS_SEARCH_STALE, STATUS_TABS_ONLY_DIRECTORY, STATUS_TAB_MOVED_LEFT, STATUS_TAB_MOVED_RIGHT,
    STATUS_TARGET_GONE, STATUS_TRASH_LOADING, STATUS_TRASH_NO_COPY, STATUS_TRASH_NO_CUT,
    STATUS_TRASH_NO_PASTE, STATUS_TRASH_NO_PURGE, STATUS_TRASH_NO_RENAME, STATUS_UNDO_FAILED,
};

/// One sidebar "place".
///
/// Paths are resolved **once** (at startup) and their existence re-checked only
/// on reload: `dirs::desktop_dir()` and friends read `user-dirs.dirs` from disk
/// on every call, so calling them from `render()` would be synchronous I/O per
/// frame (and violate the UI-thread I/O rule).
struct Place {
    label: &'static str,
    icon: IconName,
    path: Option<PathBuf>,
    exists: bool,
    /// Special entry: opens the system trash view instead of a directory.
    trash: bool,
}

fn build_places() -> Vec<Place> {
    let entries: Vec<(&'static str, IconName, Option<PathBuf>)> = vec![
        ("ホーム", IconName::House, dirs::home_dir()),
        ("デスクトップ", IconName::Monitor, dirs::desktop_dir()),
        ("ドキュメント", IconName::FileText, dirs::document_dir()),
        ("ダウンロード", IconName::Download, dirs::download_dir()),
        ("ピクチャ", IconName::Image, dirs::picture_dir()),
        ("ミュージック", IconName::Music, dirs::audio_dir()),
        ("ビデオ", IconName::Video, dirs::video_dir()),
        ("ルート", IconName::HardDrive, Some(PathBuf::from("/"))),
    ];
    let mut places: Vec<Place> = entries
        .into_iter()
        .map(|(label, icon, path)| {
            let exists = path.as_ref().is_some_and(|target| target.is_dir());
            Place {
                label,
                icon,
                path,
                exists,
                trash: false,
            }
        })
        .collect();
    places.push(Place {
        label: "ゴミ箱",
        icon: IconName::Trash,
        path: None,
        exists: true,
        trash: true,
    });
    places
}

/// Inline rename dialog state. Dropping it closes the dialog and drops the
/// input subscription.
struct RenameState {
    target: PathBuf,
    original: String,
    input: Entity<InputState>,
    /// Keeps the `InputEvent::PressEnter` subscription alive while open.
    _subscription: Subscription,
    error: Option<String>,
}

/// Location bar (Ctrl+L): a path input with validation error.
struct LocationState {
    input: Entity<InputState>,
    /// Keeps the `InputEvent::PressEnter` subscription alive while open.
    _subscription: Subscription,
    error: Option<String>,
}

/// Read-only text preview (Space / F3).
struct PreviewState {
    title: String,
    loading: bool,
    text: Option<String>,
    /// Ready-to-render image (content-sniffed format + bytes).
    image: Option<preview::ImageBytes>,
    error: Option<String>,
    truncated: bool,
    size: u64,
    task: Option<Task<()>>,
}

/// Snapshot of a file/directory for the read-only properties dialog.
struct PropsState {
    name: String,
    path: PathBuf,
    kind: &'static str,
    is_dir: bool,
    size: Option<u64>,
    modified: Option<SystemTime>,
    permissions: Option<String>,
    symlink_target: Option<PathBuf>,
    /// Directory size calculation (read-only, background, cancelable).
    size_loading: bool,
    dir_size: Option<ops::DirStats>,
    size_task: Option<Task<()>>,
    size_cancel: Option<Arc<CancelFlag>>,
}

/// Boxed context-menu builder, used where the trash and directory views need
/// different menus for the same row.
type MenuBuilder = Box<dyn Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu>;

/// Internal cut/copy clipboard: one slot holding the current selection.
///
/// The OS clipboard is deliberately not used for file cut/copy: its cut/copy
/// semantics are platform-specific and it would overwrite whatever the user
/// had copied. Paste resolves the source path at operation time.
#[derive(Clone, Debug)]
struct ClipboardSlot {
    paths: Vec<PathBuf>,
    cut: bool,
}

/// Recursive search state, grouped so `Explorer` keeps a manageable field list.
#[derive(Default)]
struct SearchState {
    input: Option<Entity<InputState>>,
    subscription: Option<Subscription>,
    ext: Option<Entity<InputState>>,
    ext_subscription: Option<Subscription>,
    name_filter: Option<Entity<InputState>>,
    name_filter_subscription: Option<Subscription>,
    kind: SearchKind,
    mode: SearchMode,
    scope: SearchScope,
    query: String,
    active_roots: Vec<PathBuf>,
    fell_back_to_cwd: bool,
    task: Option<Task<()>>,
    cancel: Option<Arc<AtomicBool>>,
    generation: u64,
}

/// Development-only screenshot / deterministic smoke-test hooks.
///
/// Every field is driven by a `GPUI_FE_DEBUG_*` environment variable. Keeping
/// them in one struct lets `Explorer` expose a single field instead of twenty
/// unrelated flags.
#[derive(Default)]
struct DebugHooks {
    open_rename_once: bool,
    select_all_once: bool,
    open_trash_once: bool,
    restore_trash_once: bool,
    progress_once: bool,
    grid_once: bool,
    search_once: bool,
    properties_once: bool,
    help_once: bool,
    location_once: bool,
    preview_once: bool,
    undo_history_once: bool,
    undo: Option<String>,
    preview_report: bool,
    properties_size: bool,
    ephemeral: bool,
    tabs_once: bool,
    drop_path: Option<String>,
    drop_target: Option<String>,
}

/// Current directory load state; `Loading` and `Error` are mutually exclusive
/// by construction (DESIGN.md §5.5).
#[derive(Clone, Debug)]
enum LoadState {
    Loading { generation: u64 },
    Ready,
    Error { generation: u64, error: String },
}

impl LoadState {
    fn error_message(&self) -> Option<&str> {
        match self {
            LoadState::Error { error, .. } => Some(error),
            _ => None,
        }
    }

    fn is_loading(&self) -> bool {
        matches!(self, LoadState::Loading { .. })
    }

    fn generation(&self) -> Option<u64> {
        match self {
            LoadState::Loading { generation } | LoadState::Error { generation, .. } => {
                Some(*generation)
            }
            LoadState::Ready => None,
        }
    }
}

struct Explorer {
    settings: Settings,
    cwd: PathBuf,
    /// Every entry read from the current directory.
    all_entries: Vec<Entry>,
    /// Filtered + sorted view rendered by `uniform_list`.
    entries: Vec<Entry>,
    /// Focused row index (keyboard cursor / single-selection shortcut).
    selected: usize,
    /// Multi-selection keyed by path so it survives reloads and re-sorting.
    selection: Selection,
    /// Current listing source (real directory or the system trash).
    view: ViewSource,
    /// Trash items backing the virtual listing (empty in directory view).
    trash_items: Vec<trash::TrashItem>,
    /// Recursive search state (active only while `view == Search`).
    search: SearchState,
    /// Content-search line numbers keyed by path (display concern, not Entry).
    search_lines: std::collections::HashMap<PathBuf, u32>,
    history: History,
    focus_handle: FocusHandle,
    scroll_handle: UniformListScrollHandle,
    load: LoadState,
    status: String,
    /// Monotonic id of the latest load request; stale results are dropped.
    generation: u64,
    /// Fingerprint of `all_entries`, used by the 10s reconciliation timer.
    fingerprint: u64,
    /// Path to select once the next load finishes (new folder, etc.).
    pending_select: Option<PathBuf>,
    /// Foreground waiter of the latest load. Dropping it cancels the wait.
    load_task: Option<Task<()>>,
    watcher: DirWatcher,
    /// Self-triggered FS operations suppress the watcher for a short window.
    suppress_until: Option<Instant>,
    /// Cached sidebar places (resolved once; see [`Place`]).
    places: Vec<Place>,
    /// Visible mount points (Linux: /proc/mounts). Refreshed on reload only.
    mounts: Vec<MountPoint>,
    /// Open tabs (always at least one); the active tab mirrors `cwd`/`history`.
    tabs: Tabs,
    /// Existing recent directories (pruned on reload; never probed in render).
    recent: Vec<PathBuf>,
    /// Existing bookmarked directories (pruned on reload; never probed in render).
    bookmarks: Vec<PathBuf>,
    /// Internal cut/copy clipboard (may hold multiple selected paths).
    clipboard: Option<ClipboardSlot>,
    /// Running paste operation; dropping the task cancels it.
    op_task: Option<Task<()>>,
    /// Cancellation flag of the running operation (Esc cancels).
    op_cancel: Option<Arc<CancelFlag>>,
    /// Byte progress of the running copy/move: (done, total). `None` when idle.
    op_progress: Option<(u64, u64)>,
    /// Inline rename dialog (F2 / context menu). `None` when closed.
    renaming: Option<RenameState>,
    /// Read-only properties dialog state.
    properties: Option<PropsState>,
    /// Whether the F1 keyboard help overlay is open.
    help_open: bool,
    /// Path input opened with Ctrl+L.
    location: Option<LocationState>,
    /// Text preview overlay (Space / F3).
    preview: Option<PreviewState>,
    /// Bounded undo/redo stack for reversible operations (rename, trash).
    undo: UndoStack,
    /// Whether the undo-history overlay (Ctrl+Shift+H) is open.
    undo_history_open: bool,
    /// Set after a successful rename; the next render refocuses the list.
    focus_list_on_next_render: bool,
    /// Development-only hooks (screenshot / deterministic smoke tests).
    debug: DebugHooks,
}

impl Explorer {
    /// Read GPUI_FE_DEBUG_* hooks in debug builds only. Release builds
    /// never honour them so a leaked variable cannot touch real files.
    fn debug_hooks_from_env() -> DebugHooks {
        #[cfg(not(debug_assertions))]
        {
            return DebugHooks::default();
        }
        #[cfg(debug_assertions)]
        {
            DebugHooks {
                open_rename_once: std::env::var_os("GPUI_FE_DEBUG_RENAME").is_some(),
                select_all_once: std::env::var_os("GPUI_FE_DEBUG_SELECT_ALL").is_some(),
                open_trash_once: std::env::var_os("GPUI_FE_DEBUG_TRASH").is_some(),
                restore_trash_once: std::env::var_os("GPUI_FE_DEBUG_TRASH_RESTORE").is_some(),
                progress_once: std::env::var_os("GPUI_FE_DEBUG_PROGRESS").is_some(),
                grid_once: std::env::var_os("GPUI_FE_DEBUG_GRID").is_some(),
                search_once: std::env::var_os("GPUI_FE_DEBUG_SEARCH").is_some(),
                properties_once: std::env::var_os("GPUI_FE_DEBUG_PROPERTIES").is_some(),
                help_once: std::env::var_os("GPUI_FE_DEBUG_HELP").is_some(),
                location_once: std::env::var_os("GPUI_FE_DEBUG_LOCATION").is_some(),
                preview_once: std::env::var_os("GPUI_FE_DEBUG_PREVIEW").is_some(),
                preview_report: std::env::var_os("GPUI_FE_DEBUG_PREVIEW_REPORT").is_some(),
                undo: std::env::var("GPUI_FE_DEBUG_UNDO").ok(),
                undo_history_once: std::env::var_os("GPUI_FE_DEBUG_UNDO_HISTORY").is_some(),
                properties_size: std::env::var_os("GPUI_FE_DEBUG_PROPERTIES_SIZE").is_some(),
                ephemeral: [
                    "GPUI_FE_DEBUG_TABS",
                    "GPUI_FE_DEBUG_RECENT",
                    "GPUI_FE_DEBUG_BOOKMARKS",
                    "GPUI_FE_DEBUG_SEARCH",
                    "GPUI_FE_DEBUG_SEARCH_SCOPE",
                    "GPUI_FE_DEBUG_SEARCH_NAME",
                    "GPUI_FE_DEBUG_GRID",
                    "GPUI_FE_DEBUG_PROPERTIES",
                    "GPUI_FE_DEBUG_PROPERTIES_SIZE",
                    "GPUI_FE_DEBUG_HELP",
                    "GPUI_FE_DEBUG_DROP_PATH",
                    "GPUI_FE_DEBUG_SORT",
                    "GPUI_FE_DEBUG_SEARCH_HISTORY",
                    "GPUI_FE_DEBUG_TAB_MOVE",
                    "GPUI_FE_DEBUG_LOCATION",
                    "GPUI_FE_DEBUG_PREVIEW",
                    "GPUI_FE_DEBUG_PREVIEW_REPORT",
                    "GPUI_FE_DEBUG_UNDO",
                    "GPUI_FE_DEBUG_UNDO_HISTORY",
                ]
                .iter()
                .any(|name| std::env::var_os(name).is_some()),
                tabs_once: std::env::var_os("GPUI_FE_DEBUG_TABS").is_some(),
                drop_path: std::env::var("GPUI_FE_DEBUG_DROP_PATH").ok(),
                drop_target: std::env::var("GPUI_FE_DEBUG_DROP_TARGET").ok(),
            }
        }
    }

    fn new(cx: &mut Context<Self>) -> Self {
        let debug = Self::debug_hooks_from_env();
        let mut settings = Settings::load();
        // Any active debug hook must never write to the real settings file.
        settings.ephemeral = debug.ephemeral;
        if std::env::var("GPUI_FE_DEBUG_SORT").is_ok_and(|value| value == "kind") {
            settings.sort = SortSpec {
                key: SortKey::Kind,
                dir: crate::model::SortDir::Asc,
                dirs_first: true,
            };
        }
        let (saved_tabs, start_dir, restored_active) = restore_session(
            &settings.tabs,
            settings.active_tab,
            settings.last_dir.as_deref(),
            startup_dir(),
            dirs::home_dir(),
        );
        if std::env::var_os("GPUI_FE_DEBUG_RECENT").is_some() {
            settings.record_recent(&start_dir);
            if let Some(home) = dirs::home_dir() {
                settings.record_recent(&home);
            }
            settings.record_recent(&PathBuf::from("/"));
        }
        if std::env::var_os("GPUI_FE_DEBUG_BOOKMARKS").is_some() {
            settings.bookmarks.clear();
            settings.toggle_bookmark(&start_dir);
            if let Some(home) = dirs::home_dir() {
                settings.toggle_bookmark(&home);
            }
            settings.toggle_bookmark(&PathBuf::from("/"));
        }
        if std::env::var_os("GPUI_FE_DEBUG_SEARCH_HISTORY").is_some() {
            settings.recent_searches =
                vec!["TODO".to_string(), "rs".to_string(), "config".to_string()];
        }

        let (watch_tx, watch_rx) = async_channel::unbounded::<Vec<FsEvent>>();
        let watcher = DirWatcher::new(move |events| {
            let _ = watch_tx.try_send(events);
        });

        let places = build_places();
        let tabs = Tabs::from_paths(saved_tabs, restored_active)
            .unwrap_or_else(|| Tabs::new(start_dir.clone()));
        let history = tabs.active().history.clone();
        let mut this = Self {
            settings,
            cwd: start_dir,
            all_entries: Vec::new(),
            entries: Vec::new(),
            selected: 0,
            selection: Selection::default(),
            view: ViewSource::Directory,
            trash_items: Vec::new(),
            search: SearchState::default(),
            search_lines: std::collections::HashMap::new(),
            history,
            focus_handle: cx.focus_handle(),
            scroll_handle: UniformListScrollHandle::new(),
            load: LoadState::Loading { generation: 0 },
            status: String::new(),
            generation: 0,
            fingerprint: 0,
            pending_select: None,
            load_task: None,
            watcher,
            suppress_until: None,
            places,
            mounts: mounts::list_mounts(),
            tabs,
            recent: Vec::new(),
            bookmarks: Vec::new(),
            clipboard: None,
            op_task: None,
            op_cancel: None,
            op_progress: None,
            renaming: None,
            properties: None,
            help_open: false,
            location: None,
            preview: None,
            undo: UndoStack::new(),
            undo_history_open: false,
            focus_list_on_next_render: false,
            debug,
        };

        // Remember the directory we actually started in (also proves the
        // settings file round-trips on the very first launch).
        this.settings.last_dir = Some(this.cwd.clone());
        this.settings.save();
        if let Some(reason) = this.settings.load_error.as_deref() {
            this.status = format!("設定を読み取り専用で開きました: {reason}");
        }

        this.spawn_watch_loop(watch_rx, cx);
        this.spawn_reconcile_loop(cx);
        this.reload(cx);
        this
    }

    // ---------------------------------------------------------------- loading

    /// Kick off an asynchronous directory read. The blocking `read_dir` runs
    /// on the GPUI background executor; the result is applied on the
    /// foreground executor only if it still belongs to the latest generation.
    fn reload(&mut self, cx: &mut Context<Self>) {
        // Search results are their own listing; reloading the cwd while the
        // search bar is open would replace them. The view is switched back to
        // Directory before `reload` when leaving search.
        if self.view == ViewSource::Search {
            return;
        }
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.load = LoadState::Loading { generation };
        self.refresh_places();
        self.refresh_mounts();
        self.refresh_recent();
        self.refresh_bookmarks();
        self.update_watcher();
        if self.view == ViewSource::Trash {
            self.reload_trash(cx);
            return;
        }
        let dir = self.cwd.clone();
        // Drop the previous waiter: cancelling its Task discards the stale
        // result; the generation check is the second line of defence.
        self.load_task.take();
        let background = cx.background_spawn(async move { read_entries(&dir) });
        let task = cx.spawn(async move |this, cx| {
            let result = background.await;
            let _ = this.update(cx, |this, cx| {
                if this.load.generation() != Some(generation) {
                    return;
                }
                this.load = LoadState::Ready;
                match result {
                    Ok(entries) => {
                        this.apply_loaded(entries);
                        // Keep self-operation notifications ("ゴミ箱へ移動しました",
                        // "フォルダを作成しました", "変更を検出しました") visible; they are
                        // replaced by the next action's message.
                    }
                    Err(error) => {
                        this.all_entries.clear();
                        this.entries.clear();
                        this.selected = 0;
                        this.selection.clear();
                        this.status.clear();
                        this.load = LoadState::Error {
                            generation,
                            error: error.message_ja(),
                        };
                    }
                }
                cx.notify();
            });
        });
        self.load_task = Some(task);
        cx.notify();
    }

    /// Load the system trash into the virtual listing (background executor).
    fn reload_trash(&mut self, cx: &mut Context<Self>) {
        let generation = self.generation;
        self.load = LoadState::Loading { generation };
        self.load_task.take();
        let background = cx.background_spawn(async move { trash_view::list() });
        let task = cx.spawn(async move |this, cx| {
            let result = background.await;
            let _ = this.update(cx, |this, cx| {
                if this.load.generation() != Some(generation) {
                    return;
                }
                this.load = LoadState::Ready;
                match result {
                    Ok(items) => {
                        this.trash_items = items;
                        this.apply_trash_loaded();
                    }
                    Err(error) => {
                        this.trash_items.clear();
                        this.all_entries.clear();
                        this.entries.clear();
                        this.selected = 0;
                        this.selection.clear();
                        this.load = LoadState::Error {
                            generation,
                            error: format!("ゴミ箱を読み込めません: {error}"),
                        };
                    }
                }
                cx.notify();
            });
        });
        self.load_task = Some(task);
        cx.notify();
    }

    /// Map trash items to rows without applying the directory hidden/sort
    /// filters (the trash is always newest-first).
    fn apply_trash_loaded(&mut self) {
        let keep = self
            .entries
            .get(self.selected)
            .map(|entry| entry.path.clone());
        let entries = trash_view::entries_from_items(&self.trash_items);
        let paths: Vec<PathBuf> = entries.iter().map(|entry| entry.path.clone()).collect();
        self.selection.retain(&paths);
        let focused = keep
            .and_then(|path| paths.iter().position(|candidate| candidate == &path))
            .or_else(|| {
                self.selection
                    .paths()
                    .first()
                    .and_then(|path| paths.iter().position(|candidate| candidate == path))
            })
            .unwrap_or(0);
        self.all_entries = entries.clone();
        self.entries = entries;
        self.fingerprint = fingerprint(&self.all_entries);
        self.selected = clamp_index(focused, self.entries.len());
        if self.selection.is_empty() {
            if let Some(entry) = self.entries.get(self.selected) {
                self.selection.select_single(entry.path.clone());
            }
        }
        if !self.entries.is_empty() {
            self.scroll_handle
                .scroll_to_item(self.selected, ScrollStrategy::Nearest);
        }
    }

    fn enter_trash(&mut self, cx: &mut Context<Self>) {
        if self.view == ViewSource::Trash {
            self.reload(cx);
            return;
        }
        self.clear_search();
        self.view = ViewSource::Trash;
        self.selection.clear();
        self.selected = 0;
        self.pending_select = None;
        self.status = STATUS_TRASH_LOADING.to_string();
        self.reload(cx);
    }

    fn exit_trash(&mut self, cx: &mut Context<Self>) {
        if self.view != ViewSource::Trash {
            return;
        }
        self.view = ViewSource::Directory;
        self.selection.clear();
        self.selected = 0;
        self.status.clear();
        self.reload(cx);
    }

    /// Apply a freshly loaded listing, keeping the selection on the same path
    /// when possible and honouring `pending_select` (new folder).
    fn apply_loaded(&mut self, entries: Vec<Entry>) {
        let keep = self
            .pending_select
            .take()
            .or_else(|| self.selected_entry().map(|entry| entry.path.clone()));
        self.all_entries = entries;
        self.fingerprint = fingerprint(&self.all_entries);
        self.rebuild_view(keep);
    }

    /// Re-filter and re-sort the cached listing.
    fn rebuild_view(&mut self, keep: Option<PathBuf>) {
        let order = self.settings.sort.order();
        let view = visible_entries(&self.all_entries, self.settings.show_hidden, order);
        let paths: Vec<PathBuf> = view.iter().map(|entry| entry.path.clone()).collect();
        // Paths are the stable key: keep what still exists after a reload and
        // drop the rest, then derive the focused row.
        self.selection.retain(&paths);
        let focused = keep
            .and_then(|path| paths.iter().position(|candidate| candidate == &path))
            .or_else(|| {
                self.selection
                    .paths()
                    .first()
                    .and_then(|path| paths.iter().position(|candidate| candidate == path))
            })
            .unwrap_or(0);
        self.entries = view;
        self.selected = clamp_index(focused, self.entries.len());
        if self.selection.is_empty() {
            if let Some(entry) = self.entries.get(self.selected) {
                self.selection.select_single(entry.path.clone());
            }
        }
        if !self.entries.is_empty() {
            self.scroll_handle
                .scroll_to_item(self.selected, ScrollStrategy::Nearest);
        }
    }

    /// Offsets of the watched directories: current dir + its parent
    /// (non-recursive), so renames/moves performed by file managers that only
    /// touch the directory entry itself still refresh the list.
    fn watched_dirs(&self) -> Vec<PathBuf> {
        let mut dirs = vec![self.cwd.clone()];
        if let Some(parent) = parent_dir(&self.cwd) {
            if parent != self.cwd {
                dirs.push(parent);
            }
        }
        dirs
    }

    fn update_watcher(&mut self) {
        let dirs = if self.view == ViewSource::Directory {
            self.watched_dirs()
        } else {
            Vec::new()
        };
        self.watcher.watch(&dirs);
    }

    /// Re-check whether the cached sidebar places exist. Called from `reload`
    /// only — never from `render`.
    fn refresh_places(&mut self) {
        for place in &mut self.places {
            if place.trash {
                place.exists = true;
            } else {
                place.exists = place.path.as_ref().is_some_and(|target| target.is_dir());
            }
        }
    }

    /// Re-check which recent directories still exist. Called from `reload`
    /// only — never from render.
    fn refresh_recent(&mut self) {
        self.settings.recent_dirs.retain(|path| path.is_dir());
        self.recent = self.settings.recent_dirs.clone();
    }

    /// Re-check which bookmarks still exist. Called from `reload` only — never
    /// from render.
    fn refresh_bookmarks(&mut self) {
        self.settings.bookmarks.retain(|path| path.is_dir());
        self.bookmarks = self.settings.bookmarks.clone();
    }

    /// Toggle a bookmark for the current directory (Ctrl+D).
    fn toggle_current_bookmark(&mut self, cx: &mut Context<Self>) {
        if self.view != ViewSource::Directory {
            self.status = STATUS_BOOKMARKS_ONLY_DIRECTORY.to_string();
            cx.notify();
            return;
        }
        let path = self.cwd.clone();
        let was_bookmarked = self.settings.is_bookmarked(&path);
        let added = self.settings.toggle_bookmark(&path);
        debug_assert_eq!(added, !was_bookmarked);
        self.settings.save();
        self.refresh_bookmarks();
        self.status = if added {
            format!("ブックマークに追加しました: {}", display_path(&path))
        } else {
            format!("ブックマークを解除しました: {}", display_path(&path))
        };
        cx.notify();
    }

    /// Re-read the mount list. Called from `reload` only — never from render.
    fn refresh_mounts(&mut self) {
        self.mounts = mounts::list_mounts();
    }

    /// 10-second reconciliation: re-read the directory in the background and
    /// reload only when the content fingerprint actually changed. This covers
    /// events lost by the OS watcher (queue overflow, unwatchable paths, ...).
    fn reconcile(&mut self, cx: &mut Context<Self>) {
        // The trash view has its own listing; the directory reconcile must not
        // overwrite it with the current directory contents.
        if self.load.is_loading() || self.view != ViewSource::Directory {
            return;
        }
        let generation = self.generation;
        let dir = self.cwd.clone();
        let task = cx.background_spawn(async move { read_entries(&dir) });
        cx.spawn(async move |this, cx| {
            let entries = match task.await {
                Ok(entries) => entries,
                Err(error) => {
                    // The reconcile is the fallback for watcher events we
                    // missed; surface its failure instead of silently losing
                    // it (second cross-review H-4).
                    let _ = this.update(cx, |this, cx| {
                        if this.generation == generation {
                            this.status = format!("再スキャンに失敗しました: {error}");
                            cx.notify();
                        }
                    });
                    return;
                }
            };
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation || this.load.is_loading() {
                    return;
                }
                if fingerprint(&entries) == this.fingerprint {
                    return;
                }
                this.apply_loaded(entries);
                cx.notify();
            });
        })
        .detach();
    }

    /// Watcher events arrive on a background thread; this foreground loop
    /// coalesces bursts and applies incremental updates where possible.
    fn spawn_watch_loop(
        &self,
        watch_rx: async_channel::Receiver<Vec<FsEvent>>,
        cx: &mut Context<Self>,
    ) {
        cx.spawn(async move |this, cx| {
            while let Ok(events) = watch_rx.recv().await {
                let mut batch = vec![events];
                while let Ok(more) = watch_rx.try_recv() {
                    batch.push(more);
                }
                let updated = this.update(cx, |this, cx| {
                    if this.is_suppressed() || this.op_cancel.is_some() {
                        return;
                    }
                    let search_view = this.view == ViewSource::Search;
                    if !search_view && this.view != ViewSource::Directory {
                        return;
                    }
                    let batch: Vec<FsEvent> = batch.into_iter().flatten().collect();
                    if search_view {
                        // A search result set is a snapshot; at least drop
                        // entries that vanished so the view cannot act on a
                        // deleted path. Full re-search is deferred to the user
                        // (documented limitation).
                        // Rescan/rename cannot be applied to a search snapshot
                        // safely; tell the user to re-run the search.
                        if batch
                            .iter()
                            .any(|event| matches!(event, FsEvent::Rescan | FsEvent::Renamed { .. }))
                        {
                            this.status = STATUS_SEARCH_STALE.to_string();
                            cx.notify();
                            return;
                        }
                        let removed = removed_paths(&batch);
                        if removed.is_empty() {
                            return;
                        }
                        let before = this.all_entries.len();
                        this.all_entries
                            .retain(|entry| !removed.contains(&entry.path));
                        for path in &removed {
                            this.search_lines.remove(path);
                        }
                        if this.all_entries.len() == before {
                            return;
                        }
                        let remaining: Vec<PathBuf> = this
                            .all_entries
                            .iter()
                            .map(|entry| entry.path.clone())
                            .collect();
                        this.selection.retain(&remaining);
                        this.apply_search_filter(cx);
                        this.status = STATUS_CHANGED.to_string();
                        cx.notify();
                        return;
                    }
                    if batch
                        .iter()
                        .any(|event| matches!(event, FsEvent::Rescan | FsEvent::Renamed { .. }))
                    {
                        this.status = STATUS_CHANGED.to_string();
                        this.reload(cx);
                        return;
                    }
                    let keep = this.selected_entry().map(|entry| entry.path.clone());
                    let mut changed = false;
                    for event in batch {
                        match event {
                            FsEvent::Removed(paths) => {
                                for path in paths {
                                    let before = this.all_entries.len();
                                    this.all_entries.retain(|entry| entry.path != path);
                                    changed |= this.all_entries.len() != before;
                                }
                            }
                            FsEvent::Created(paths) | FsEvent::Modified(paths) => {
                                for path in paths {
                                    // Parent watches also report changes above cwd.
                                    if path.parent() != Some(this.cwd.as_path()) {
                                        continue;
                                    }
                                    if let Some(entry) = crate::model::entry_for_path(&path) {
                                        if let Some(slot) = this
                                            .all_entries
                                            .iter_mut()
                                            .find(|entry| entry.path == path)
                                        {
                                            *slot = entry;
                                        } else {
                                            this.all_entries.push(entry);
                                        }
                                    } else {
                                        this.all_entries.retain(|entry| entry.path != path);
                                    }
                                    changed = true;
                                }
                            }
                            FsEvent::Rescan | FsEvent::Renamed { .. } => {
                                this.status = STATUS_CHANGED.to_string();
                                this.reload(cx);
                                return;
                            }
                        }
                    }
                    if changed {
                        this.fingerprint = fingerprint(&this.all_entries);
                        this.rebuild_view(keep);
                        this.status = STATUS_CHANGED.to_string();
                        cx.notify();
                    }
                });
                if updated.is_err() {
                    break;
                }
            }
        })
        .detach();
    }

    fn spawn_reconcile_loop(&self, cx: &mut Context<Self>) {
        let background = cx.background_executor().clone();
        cx.spawn(async move |this, cx| loop {
            background.timer(RECONCILE_INTERVAL).await;
            if this.update(cx, |this, cx| this.reconcile(cx)).is_err() {
                break;
            }
        })
        .detach();
    }

    /// Whether events caused by our own trash/mkdir operations should be
    /// ignored for a short moment.
    fn is_suppressed(&self) -> bool {
        self.suppress_until
            .is_some_and(|until| Instant::now() < until)
    }

    fn suppress_for(&mut self, duration: Duration) {
        self.suppress_until = Some(Instant::now() + duration);
    }

    // -------------------------------------------------------------------- tabs

    /// Persist the current directory/history into the active tab.
    fn sync_active_tab(&mut self) {
        self.tabs.sync(self.cwd.clone(), self.history.clone());
    }

    /// Mirror the open tabs and the active index into settings (the caller
    /// saves the settings file).
    fn persist_tabs(&mut self) {
        if self.debug.ephemeral {
            return;
        }
        self.tabs.sync(self.cwd.clone(), self.history.clone());
        self.settings.tabs = self.tabs.tabs().iter().map(|tab| tab.cwd.clone()).collect();
        self.settings.active_tab = self.tabs.active_index();
    }

    /// Load a tab's directory/history into the explorer.
    fn load_tab(&mut self, tab: Tab, cx: &mut Context<Self>) {
        self.clear_search();
        self.view = ViewSource::Directory;
        self.cwd = tab.cwd;
        self.history = tab.history;
        self.selection.clear();
        self.selected = 0;
        self.pending_select = None;
        self.load = LoadState::Ready;
        self.status.clear();
        self.settings.last_dir = Some(self.cwd.clone());
        self.settings.record_recent(&self.cwd);
        self.persist_tabs();
        self.settings.save();
        self.reload(cx);
    }

    fn switch_tab(&mut self, index: usize, cx: &mut Context<Self>) {
        if index == self.tabs.active_index() {
            return;
        }
        self.sync_active_tab();
        if let Some(tab) = self.tabs.activate(index) {
            self.load_tab(tab, cx);
        }
    }

    fn new_tab(&mut self, cx: &mut Context<Self>) {
        if self.view != ViewSource::Directory {
            self.status = STATUS_TABS_ONLY_DIRECTORY.to_string();
            cx.notify();
            return;
        }
        self.sync_active_tab();
        self.tabs.new_tab(self.cwd.clone());
        let tab = self.tabs.active().clone();
        self.load_tab(tab, cx);
    }

    fn close_tab(&mut self, cx: &mut Context<Self>) {
        if self.view != ViewSource::Directory {
            self.status = STATUS_TABS_ONLY_DIRECTORY.to_string();
            cx.notify();
            return;
        }
        if self.tabs.len() <= 1 {
            self.status = STATUS_LAST_TAB.to_string();
            cx.notify();
            return;
        }
        self.sync_active_tab();
        if let Some(tab) = self.tabs.close_active() {
            self.load_tab(tab, cx);
        }
    }

    fn restore_closed_tab(&mut self, cx: &mut Context<Self>) {
        self.sync_active_tab();
        if let Some(cwd) = self.tabs.restore_last_closed() {
            let tab = self.tabs.active().clone();
            self.load_tab(tab, cx);
            self.status = format!("タブを復元しました: {}", display_path(&cwd));
        } else {
            self.status = "復元できるタブがありません".to_string();
        }
        cx.notify();
    }

    fn next_tab(&mut self, cx: &mut Context<Self>) {
        self.sync_active_tab();
        let tab = self.tabs.next();
        self.load_tab(tab, cx);
    }

    fn prev_tab(&mut self, cx: &mut Context<Self>) {
        self.sync_active_tab();
        let tab = self.tabs.prev();
        self.load_tab(tab, cx);
    }

    fn move_tab_left(&mut self, cx: &mut Context<Self>) {
        self.sync_active_tab();
        if self.tabs.move_active_left() {
            self.persist_tabs();
            self.settings.save();
            self.status = STATUS_TAB_MOVED_LEFT.to_string();
            cx.notify();
        }
    }

    fn move_tab_right(&mut self, cx: &mut Context<Self>) {
        self.sync_active_tab();
        if self.tabs.move_active_right() {
            self.persist_tabs();
            self.settings.save();
            self.status = STATUS_TAB_MOVED_RIGHT.to_string();
            cx.notify();
        }
    }

    /// Horizontal tab bar (directory view only).
    fn tab_bar(&self, cx: &mut Context<Self>) -> Option<Div> {
        if self.view != ViewSource::Directory {
            return None;
        }
        let mut bar = div()
            .flex()
            .items_center()
            .gap_1()
            .px_3()
            .py_1()
            .bg(rgb(C_BASE))
            .border_b_1()
            .border_color(rgb(C_SURFACE))
            .overflow_hidden();
        for (index, tab) in self.tabs.tabs().iter().enumerate() {
            let active = index == self.tabs.active_index();
            let label = tab_label(&tab.cwd);
            let mut item = div()
                .id(("tab", index))
                .flex()
                .items_center()
                .gap_1()
                .px_2()
                .py_0p5()
                .rounded_md()
                .text_sm()
                .cursor_pointer()
                .when(active, |item| item.bg(rgb(C_SURFACE)))
                .text_color(if active { rgb(C_TEXT) } else { rgb(C_SUBTEXT) })
                .hover(|style| style.bg(rgb(C_SURFACE)))
                .on_click(cx.listener(move |this, _event, window, cx| {
                    this.focus_handle.focus(window, cx);
                    this.switch_tab(index, cx);
                }))
                .child(div().max_w(px(160.0)).truncate().child(label));
            if active {
                item = item.child(
                    div()
                        .id("tab-close")
                        .px_1()
                        .rounded_md()
                        .hover(|style| style.bg(rgb(C_OVERLAY)))
                        .child("×")
                        .on_click(cx.listener(|this, _event, window, cx| {
                            this.focus_handle.focus(window, cx);
                            this.close_tab(cx);
                        })),
                );
            }
            bar = bar.child(item);
        }
        bar = bar.child(
            div()
                .id("tab-new")
                .px_2()
                .py_0p5()
                .rounded_md()
                .text_sm()
                .text_color(rgb(C_SUBTEXT))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(C_SURFACE)))
                .child("+")
                .on_click(cx.listener(|this, _event, window, cx| {
                    this.focus_handle.focus(window, cx);
                    this.new_tab(cx);
                })),
        );
        Some(bar)
    }

    // ------------------------------------------------------------- navigation

    fn set_cwd(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.clear_search();
        self.view = ViewSource::Directory;
        if path == self.cwd {
            self.reload(cx);
            return;
        }
        self.cwd = path;
        self.selected = 0;
        self.pending_select = None;
        self.load = LoadState::Ready;
        self.status.clear();
        self.settings.last_dir = Some(self.cwd.clone());
        self.settings.record_recent(&self.cwd);
        self.persist_tabs();
        self.settings.save();
        self.reload(cx);
    }

    fn navigate_to(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.navigate_with_mode(path, NavMode::Push, cx);
    }

    fn navigate_with_mode(&mut self, path: PathBuf, mode: NavMode, cx: &mut Context<Self>) {
        self.clear_search();
        self.view = ViewSource::Directory;
        if path == self.cwd {
            // Re-loading the same directory must not touch history.
            self.history.apply(self.cwd.clone(), path, NavMode::Replace);
            self.reload(cx);
            return;
        }
        if !path.is_dir() {
            self.load = LoadState::Error {
                generation: self.generation,
                error: format!("{STATUS_NOT_DIRECTORY}: {}", display_path(&path)),
            };
            cx.notify();
            return;
        }
        self.history.apply(self.cwd.clone(), path.clone(), mode);
        self.set_cwd(path, cx);
    }

    fn go_back(&mut self, cx: &mut Context<Self>) {
        if self.view == ViewSource::Trash {
            self.exit_trash(cx);
            return;
        }
        if let Some(target) = self
            .history
            .apply(self.cwd.clone(), PathBuf::new(), NavMode::Back)
        {
            if target.is_dir() {
                self.set_cwd(target, cx);
            } else {
                self.status = format!("{STATUS_TARGET_GONE}: {}", display_path(&target));
                cx.notify();
            }
        }
    }

    fn go_forward(&mut self, cx: &mut Context<Self>) {
        if self.view == ViewSource::Trash {
            self.exit_trash(cx);
            return;
        }
        if let Some(target) = self
            .history
            .apply(self.cwd.clone(), PathBuf::new(), NavMode::Forward)
        {
            if target.is_dir() {
                self.set_cwd(target, cx);
            } else {
                self.status = format!("{STATUS_TARGET_GONE}: {}", display_path(&target));
                cx.notify();
            }
        }
    }

    fn go_parent(&mut self, cx: &mut Context<Self>) {
        if self.view == ViewSource::Trash {
            self.exit_trash(cx);
            return;
        }
        if let Some(parent) = parent_dir(&self.cwd) {
            if parent != self.cwd {
                self.navigate_with_mode(parent, NavMode::Up, cx);
            }
        }
    }

    fn open_selected(&mut self, cx: &mut Context<Self>) {
        if self.view == ViewSource::Trash {
            self.restore_selected(cx);
            return;
        }
        self.open_index(self.selected, cx);
    }

    fn open_index(&mut self, index: usize, cx: &mut Context<Self>) {
        let Some(path) = self.entries.get(index).map(|entry| entry.path.clone()) else {
            return;
        };
        self.open_path(path, cx);
    }

    /// Open a target by path. Resolved at click time (never captured by row
    /// index) so a watcher-driven re-sort cannot redirect the action.
    fn open_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !path.exists() {
            self.status = format!("{STATUS_PATH_GONE}: {}", display_path(&path));
            self.reload(cx);
            return;
        }
        if path.is_dir() {
            self.navigate_to(path, cx);
        } else {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().to_string())
                .unwrap_or_else(|| display_path(&path));
            cx.open_with_system(&path);
            self.status = format!("{name} を既定のアプリで開きました");
            cx.notify();
        }
    }

    // -------------------------------------------------------------- selection

    /// Select one row (plain click / arrow key) and move the keyboard cursor.
    fn select_single_index(&mut self, index: usize, cx: &mut Context<Self>) {
        let index = clamp_index(index, self.entries.len());
        self.selected = index;
        if let Some(entry) = self.entries.get(index) {
            self.selection.select_single(entry.path.clone());
        } else {
            self.selection.clear();
        }
        if !self.entries.is_empty() {
            self.scroll_handle
                .scroll_to_item(index, ScrollStrategy::Nearest);
        }
        cx.notify();
    }

    /// Apply a click with Ctrl (toggle) or Shift (range) modifiers.
    fn select_with_modifiers(
        &mut self,
        index: usize,
        control: bool,
        shift: bool,
        cx: &mut Context<Self>,
    ) {
        let index = clamp_index(index, self.entries.len());
        let Some(entry) = self.entries.get(index) else {
            return;
        };
        let path = entry.path.clone();
        let paths: Vec<PathBuf> = self
            .entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
        if shift {
            self.selection.select_range(&paths, &path);
        } else if control {
            self.selection.toggle(path);
        } else {
            self.selection.select_single(path);
        }
        self.selected = index;
        self.scroll_handle
            .scroll_to_item(index, ScrollStrategy::Nearest);
        cx.notify();
    }

    /// Switch between the details list and the icon grid (persisted).
    fn set_view(&mut self, mode: ViewLayout, cx: &mut Context<Self>) {
        if self.settings.view == mode {
            return;
        }
        self.settings.view = mode;
        self.settings.save();
        cx.notify();
    }

    fn select_all(&mut self, cx: &mut Context<Self>) {
        let paths: Vec<PathBuf> = self
            .entries
            .iter()
            .map(|entry| entry.path.clone())
            .collect();
        self.selection.select_all(&paths);
        cx.notify();
    }

    fn move_selection(&mut self, delta: isize, extend: bool, cx: &mut Context<Self>) {
        let next = step_selection(self.selected, self.entries.len(), delta);
        self.select_with_modifiers(next, false, extend, cx);
    }

    fn select_first(&mut self, extend: bool, cx: &mut Context<Self>) {
        self.select_with_modifiers(first_index(self.entries.len()), false, extend, cx);
    }

    fn select_last(&mut self, extend: bool, cx: &mut Context<Self>) {
        self.select_with_modifiers(last_index(self.entries.len()), false, extend, cx);
    }

    /// Paths an action (copy/cut/trash) should apply to: the multi-selection
    /// when non-empty, otherwise the focused row.
    fn action_paths(&self) -> Vec<PathBuf> {
        if !self.selection.is_empty() {
            return self.selection.paths().to_vec();
        }
        self.entries
            .get(self.selected)
            .map(|entry| vec![entry.path.clone()])
            .unwrap_or_default()
    }

    fn selected_entry(&self) -> Option<&Entry> {
        self.entries.get(self.selected)
    }

    // --------------------------------------------------------------- shortcuts

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        // While the rename dialog is open the text input owns most keys; only
        // Escape is handled here (Enter arrives as `InputEvent::PressEnter`).
        if self.renaming.is_some() {
            if event.keystroke.key == "escape" {
                self.cancel_rename(window, cx);
            }
            return;
        }
        if event.keystroke.key == "f1" {
            self.help_open = !self.help_open;
            cx.notify();
            return;
        }
        if self.help_open {
            if event.keystroke.key == "escape" {
                self.help_open = false;
                cx.notify();
            }
            return;
        }
        if self.location.is_some() {
            if event.keystroke.key == "escape" {
                self.close_location(cx);
            }
            return;
        }
        if self.undo_history_open {
            if event.keystroke.key == "escape" {
                self.undo_history_open = false;
                cx.notify();
            }
            return;
        }
        if self.preview.is_some() {
            if event.keystroke.key == "escape"
                || event.keystroke.key == "space"
                || event.keystroke.key == "f3"
            {
                self.close_preview(cx);
            }
            return;
        }
        if self.properties.is_some() {
            if event.keystroke.key == "escape" {
                self.close_properties(cx);
            }
            return;
        }
        if self.view == ViewSource::Search {
            if event.keystroke.key == "escape" {
                self.close_search(cx);
            }
            return;
        }
        let keystroke = &event.keystroke;
        let key = keystroke.key.as_str();
        let modifiers = &keystroke.modifiers;
        if modifiers.control {
            match (key, modifiers.shift) {
                ("h", _) => self.toggle_hidden(cx),
                ("a", false) => self.select_all(cx),
                ("d", false) => self.toggle_current_bookmark(cx),
                ("f", false) => self.open_search(window, cx),
                ("f", true) => {
                    if let Some(input) = self.search.name_filter.clone() {
                        let focus = input.read(cx).focus_handle(cx).clone();
                        focus.focus(window, cx);
                        cx.notify();
                    }
                }
                ("i", false) => self.open_selected_properties(cx),
                ("l", false) => self.open_location(window, cx),
                ("z", false) => self.undo_last(cx),
                ("z", true) => self.redo_last(cx),
                ("u", true) => {
                    self.undo_history_open = !self.undo_history_open;
                    cx.notify();
                }
                ("t", false) => self.new_tab(cx),
                ("t", true) => self.restore_closed_tab(cx),
                ("w", false) => self.close_tab(cx),
                ("tab", false) => self.next_tab(cx),
                ("tab", true) => self.prev_tab(cx),
                ("pageup", true) => self.move_tab_left(cx),
                ("pagedown", true) => self.move_tab_right(cx),
                ("1", false) => self.set_view(ViewLayout::Details, cx),
                ("2", false) => self.set_view(ViewLayout::Icons, cx),
                ("r", _) => self.reload(cx),
                ("n", true) => self.new_folder(cx),
                ("c", false) => {
                    if self.view == ViewSource::Trash {
                        self.status = STATUS_TRASH_NO_COPY.to_string();
                        cx.notify();
                    } else {
                        self.copy_selected(cx);
                    }
                }
                ("x", false) => {
                    if self.view == ViewSource::Trash {
                        self.status = STATUS_TRASH_NO_CUT.to_string();
                        cx.notify();
                    } else {
                        self.cut_selected(cx);
                    }
                }
                ("v", false) => {
                    if self.view == ViewSource::Trash {
                        self.status = STATUS_TRASH_NO_PASTE.to_string();
                        cx.notify();
                    } else {
                        self.paste(cx);
                    }
                }
                ("q", _) => cx.quit(),
                _ => {}
            }
            return;
        }
        if modifiers.alt {
            match key {
                "left" => self.go_back(cx),
                "right" => self.go_forward(cx),
                "up" => self.go_parent(cx),
                _ => {}
            }
            return;
        }
        // Page step: how many 28px rows fit into the window viewport.
        let page_step = {
            let viewport_height = window.viewport_size().height.to_f64();
            ((viewport_height - LIST_CHROME_HEIGHT_PX) / ROW_HEIGHT_PX)
                .floor()
                .max(1.0) as isize
        };
        match key {
            "up" => self.move_selection(-1, modifiers.shift, cx),
            "down" => self.move_selection(1, modifiers.shift, cx),
            "pageup" => self.move_selection(-page_step, modifiers.shift, cx),
            "pagedown" => self.move_selection(page_step, modifiers.shift, cx),
            "home" => self.select_first(modifiers.shift, cx),
            "end" => self.select_last(modifiers.shift, cx),
            "enter" => self.open_selected(cx),
            "backspace" => self.go_parent(cx),
            "f5" => self.reload(cx),
            "space" | "f3" => self.open_preview_selected(cx),
            "f2" => {
                if self.view == ViewSource::Trash {
                    self.status = STATUS_TRASH_NO_RENAME.to_string();
                    cx.notify();
                } else {
                    self.start_rename(window, cx);
                }
            }
            "delete" => {
                if self.view == ViewSource::Trash {
                    // Permanent deletion is intentionally not offered.
                    self.status = STATUS_TRASH_NO_PURGE.to_string();
                    cx.notify();
                } else {
                    self.trash_selected(window, cx);
                }
            }
            "escape" => self.cancel_op(cx),
            _ => {}
        }
    }

    // ------------------------------------------------------------------ actions

    fn toggle_hidden(&mut self, cx: &mut Context<Self>) {
        self.settings.show_hidden = !self.settings.show_hidden;
        self.settings.save();
        let keep = self.selected_entry().map(|entry| entry.path.clone());
        self.rebuild_view(keep);
        self.status = if self.settings.show_hidden {
            "隠しファイルを表示しています".to_string()
        } else {
            "隠しファイルを非表示にしました".to_string()
        };
        cx.notify();
    }

    fn set_sort(&mut self, key: SortKey, cx: &mut Context<Self>) {
        let order = self.settings.sort.order().toggled(key);
        self.settings.sort = SortSpec::from_order(order);
        self.settings.save();
        let keep = self.selected_entry().map(|entry| entry.path.clone());
        self.rebuild_view(keep);
        cx.notify();
    }

    /// Create `新しいフォルダ` (or ` (2)`, ` (3)`, ...) off the UI thread.
    /// Name-collision probing and `create_dir` are both blocking I/O, so they
    /// run on the background executor; the UI only updates afterwards.
    fn new_folder(&mut self, cx: &mut Context<Self>) {
        let dir = self.cwd.clone();
        let expected_dir = dir.clone();
        self.status = STATUS_FOLDER_CREATING.to_string();
        cx.notify();
        let task = cx.background_spawn(async move { create_unique_folder(&dir) });
        cx.spawn(async move |this, cx| {
            let result = task.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(path) => {
                        this.suppress_for(SELF_OP_SUPPRESS);
                        let name = path
                            .file_name()
                            .map(|name| name.to_string_lossy().to_string())
                            .unwrap_or_default();
                        if this.cwd == expected_dir {
                            this.status = format!("フォルダを作成しました: {name}");
                            this.pending_select = Some(path);
                            this.reload(cx);
                        } else {
                            this.status = format!(
                                "フォルダを作成しました: {}（別のフォルダを表示中）",
                                display_path(&path)
                            );
                        }
                    }
                    Err(error) => {
                        this.load = LoadState::Error {
                            generation: this.generation,
                            error: format!("フォルダを作成できません: {error}"),
                        };
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn copy_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(display_path(&path)));
        self.status = format!("パスをコピーしました: {}", display_path(&path));
        cx.notify();
    }

    /// Remember the selected entries for a later paste (Ctrl+C).
    fn copy_selected(&mut self, cx: &mut Context<Self>) {
        let paths = self.action_paths();
        if paths.is_empty() {
            return;
        }
        let count = paths.len();
        let status = if count == 1 {
            format!(
                "コピーしました: {}（Ctrl+V で貼り付け）",
                display_path(&paths[0])
            )
        } else {
            format!("{count} 件をコピーしました（Ctrl+V で貼り付け）")
        };
        self.clipboard = Some(ClipboardSlot { paths, cut: false });
        self.status = status;
        cx.notify();
    }

    /// Remember the selected entries as move sources (Ctrl+X).
    fn cut_selected(&mut self, cx: &mut Context<Self>) {
        let paths = self.action_paths();
        if paths.is_empty() {
            return;
        }
        let count = paths.len();
        let status = if count == 1 {
            format!(
                "切り取りました: {}（Ctrl+V で移動）",
                display_path(&paths[0])
            )
        } else {
            format!("{count} 件を切り取りました（Ctrl+V で移動）")
        };
        self.clipboard = Some(ClipboardSlot { paths, cut: true });
        self.status = status;
        cx.notify();
    }

    /// Cancel the running paste operation, if any (Esc).
    fn cancel_op(&mut self, cx: &mut Context<Self>) {
        let Some(cancel) = &self.op_cancel else {
            return;
        };
        cancel.cancel();
        self.status = STATUS_CANCELLING.to_string();
        cx.notify();
    }

    /// Paste the internal clipboard into the current directory.
    ///
    /// Safety: the destination conflict policy is always
    /// [`ConflictPolicy::Rename`], so an existing file is never overwritten.
    /// The blocking copy/move runs on the background executor; the UI thread
    /// only applies the result. `Esc` cancels cooperatively.
    fn paste(&mut self, cx: &mut Context<Self>) {
        let Some(slot) = self.clipboard.clone() else {
            self.status = STATUS_NO_CLIPBOARD.to_string();
            cx.notify();
            return;
        };
        if self.op_task.is_some() {
            self.status = STATUS_OP_RUNNING.to_string();
            cx.notify();
            return;
        }

        let cwd = self.cwd.clone();
        let cut = slot.cut;

        // Drop missing sources and same-directory cut sources; keep the rest.
        let mut missing: Vec<PathBuf> = Vec::new();
        let mut same_dir = 0usize;
        let mut paths: Vec<PathBuf> = Vec::new();
        for path in slot.paths {
            if !path.exists() && !path.is_symlink() {
                missing.push(path);
                continue;
            }
            if cut && path.parent() == Some(cwd.as_path()) {
                same_dir += 1;
                continue;
            }
            paths.push(path);
        }
        if !missing.is_empty() && paths.is_empty() {
            self.clipboard = None;
            self.status = format!("{} 件は既に存在しません", missing.len());
            self.reload(cx);
            return;
        }
        if paths.is_empty() {
            self.status = if same_dir > 0 {
                "同じフォルダのため移動しません".to_string()
            } else {
                STATUS_NO_CLIPBOARD.to_string()
            };
            cx.notify();
            return;
        }

        self.run_transfer(cwd.clone(), paths, cut, missing.len(), cx);
    }

    /// Copy (`cut == false`) or move (`cut == true`) `paths` into `destination`
    /// on the background executor.
    ///
    /// Safety: destinations always use [`ConflictPolicy::Rename`], so an
    /// existing file is never overwritten; `Esc` cancels cooperatively.
    fn run_transfer(
        &mut self,
        destination: PathBuf,
        paths: Vec<PathBuf>,
        cut: bool,
        skipped: usize,
        cx: &mut Context<Self>,
    ) {
        let verb = if cut { "移動" } else { "コピー" };
        let target_prefix = if destination == self.cwd {
            String::new()
        } else {
            format!("「{}」へ ", display_path(&destination))
        };
        self.status = format!("{verb}中…（Esc でキャンセル）");
        cx.notify();

        let cancel = Arc::new(CancelFlag::new());
        self.op_cancel = Some(cancel.clone());

        // Byte progress from the background copy/move is forwarded through a
        // channel and throttled to ~80 ms so the UI is not flooded with
        // notifications. The pump stops when the operation drops its sender.
        let (progress_tx, progress_rx) = async_channel::unbounded::<(u64, u64)>();
        let progress_verb = verb;
        cx.spawn(async move |this, cx| {
            let mut last_report = Instant::now();
            let mut last_bytes = 0u64;
            while let Ok((done, total)) = progress_rx.recv().await {
                if done <= last_bytes && done != 0 {
                    continue;
                }
                let first_update = last_bytes == 0;
                if !first_update && last_report.elapsed() < PROGRESS_THROTTLE {
                    continue;
                }
                last_report = Instant::now();
                last_bytes = done;
                let _ = this.update(cx, |this, cx| {
                    // Ignore progress that arrives after the operation settled
                    // (the final status message must not be overwritten).
                    if this.op_cancel.is_none() {
                        return;
                    }
                    this.op_progress = Some((done, total));
                    this.status = format!(
                        "{progress_verb}中… {} / {}（Esc でキャンセル）",
                        human_size(done),
                        human_size(total)
                    );
                    cx.notify();
                });
            }
        })
        .detach();

        let task_cancel = cancel.clone();
        let destination_for_task = destination.clone();
        let destination_for_ui = destination.clone();
        let progress_tx_for_task = progress_tx.clone();
        let background = cx.background_spawn(async move {
            let total_bytes = ops::total_bytes(&paths);
            let _ = progress_tx_for_task.try_send((0, total_bytes));
            let mut total = OpStats::default();
            let mut first_error: Option<OpError> = None;
            let mut first_created: Option<PathBuf> = None;
            let mut offset = 0u64;
            for path in paths {
                if task_cancel.is_cancelled() {
                    first_error = Some(OpError::Cancelled);
                    break;
                }
                let base = offset;
                let tx = progress_tx_for_task.clone();
                let mut progress = move |bytes: u64| {
                    let _ = tx.try_send((base + bytes, total_bytes));
                };
                let result = if cut {
                    ops::move_entry(
                        &path,
                        &destination_for_task,
                        ConflictPolicy::Rename,
                        &task_cancel,
                        &mut progress,
                    )
                } else {
                    ops::copy_entry(
                        &path,
                        &destination_for_task,
                        ConflictPolicy::Rename,
                        &task_cancel,
                        &mut progress,
                    )
                };
                match result {
                    Ok(stats) => {
                        offset += stats.bytes;
                        total.files += stats.files;
                        total.dirs += stats.dirs;
                        total.bytes += stats.bytes;
                        total.skipped.extend(stats.skipped);
                        if first_created.is_none() {
                            first_created = stats.created.first().cloned();
                        }
                    }
                    Err(error) => {
                        let cancelled = matches!(error, OpError::Cancelled);
                        if first_error.is_none() {
                            first_error = Some(error);
                        }
                        if cancelled {
                            break;
                        }
                    }
                }
            }
            (total, first_error, first_created)
        });
        let task = cx.spawn(async move |this, cx| {
            let (stats, first_error, first_created) = background.await;
            let _ = this.update(cx, |this, cx| {
                this.op_task = None;
                this.op_cancel = None;
                this.op_progress = None;
                let error_text = first_error
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                tracing::info!(
                    op = verb,
                    files = stats.files,
                    dirs = stats.dirs,
                    bytes = stats.bytes,
                    skipped = stats.skipped.len(),
                    error = %error_text,
                    "file operation completed"
                );
                let succeeded = stats.files + stats.dirs > 0;
                let status = match first_error {
                    Some(OpError::Cancelled) => "キャンセルしました".to_string(),
                    Some(error) if succeeded => {
                        format!("{verb}しました（一部失敗: {error}）")
                    }
                    Some(error) => format!("{verb}できません: {error}"),
                    None => summarize_op(verb, &stats),
                };
                let status = if skipped > 0 {
                    format!("{status}（存在しない項目 {skipped} 件をスキップ）")
                } else {
                    status
                };
                this.suppress_for(SELF_OP_SUPPRESS);
                if cut {
                    // Moved sources are gone; never keep a stale cut clipboard.
                    this.clipboard = None;
                }
                if let Some(dest) = first_created {
                    if destination_for_ui == this.cwd
                        && dest.parent() == Some(destination_for_ui.as_path())
                    {
                        this.pending_select = Some(dest);
                    }
                }
                this.status = format!("{target_prefix}{status}");
                this.reload(cx);
                cx.notify();
            });
        });
        self.op_task = Some(task);
        cx.notify();
    }

    /// Copy files/directories dropped from the OS into the current directory.
    fn drop_external(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        self.drop_external_into(self.cwd.clone(), paths, cx);
    }

    /// Copy OS-dropped paths into `destination` (current directory, or the
    /// folder row they were dropped on).
    ///
    /// Safety: a drop is always a **copy** (never a move) and uses the same
    /// `ConflictPolicy::Rename` pipeline as paste, so nothing is overwritten.
    /// Drops are ignored in the trash/search views and while another
    /// operation is running; the destination must be an existing directory.
    fn drop_external_into(
        &mut self,
        destination: PathBuf,
        paths: Vec<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        if self.view != ViewSource::Directory {
            self.status = STATUS_DROP_ONLY_DIRECTORY.to_string();
            cx.notify();
            return;
        }
        if self.op_task.is_some() {
            self.status = STATUS_OP_RUNNING.to_string();
            cx.notify();
            return;
        }
        if !destination.is_dir() {
            self.status = STATUS_DROP_NO_DESTINATION.to_string();
            cx.notify();
            return;
        }
        let mut existing: Vec<PathBuf> = Vec::new();
        let mut missing = 0usize;
        let mut same = 0usize;
        let mut special = 0usize;
        for path in paths {
            // `symlink_metadata` sees broken links too and never follows them.
            let Ok(link_metadata) = std::fs::symlink_metadata(&path) else {
                missing += 1;
                continue;
            };
            let file_type = link_metadata.file_type();
            if !file_type.is_symlink() && !file_type.is_file() && !file_type.is_dir() {
                // Devices, sockets and FIFOs cannot be copied safely; skip them
                // here instead of failing the whole transfer in `copy_recursive`.
                special += 1;
                continue;
            }
            if path == destination {
                same += 1;
                continue;
            }
            existing.push(path);
        }
        if existing.is_empty() {
            self.status = if same > 0 {
                STATUS_DROP_SAME_DIRECTORY.to_string()
            } else if missing > 0 {
                STATUS_DROP_NOT_FOUND.to_string()
            } else if special > 0 {
                "特殊ファイルのためコピーできません".to_string()
            } else {
                STATUS_DROP_EMPTY.to_string()
            };
            cx.notify();
            return;
        }
        let mut notices = Vec::new();
        if missing > 0 {
            notices.push(format!("{missing} 件は存在しないためスキップ"));
        }
        if special > 0 {
            notices.push(format!("特殊ファイル {special} 件をスキップ"));
        }
        if !notices.is_empty() {
            self.status = notices.join(" / ");
        }
        self.run_transfer(destination, existing, false, 0, cx);
    }

    // ------------------------------------------------------------------ undo

    /// Overlay listing what can still be undone/redone (read-only).
    fn undo_history_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.undo_history_open {
            return None;
        }
        let close = cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.undo_history_open = false;
            cx.notify();
        });
        let backdrop = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(gpui::black().opacity(0.45))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _window, cx| {
                    this.undo_history_open = false;
                    cx.notify();
                }),
            );
        let mut body = div()
            .id("undo-history")
            .w(px(720.0))
            .bg(rgb(C_SURFACE))
            .rounded_lg()
            .border_1()
            .border_color(rgb(C_OVERLAY))
            .shadow_lg()
            .p_5()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("操作の履歴"),
            );
        let undo_actions = self.undo.undo_actions();
        let redo_actions = self.undo.redo_actions();
        if undo_actions.is_empty() && redo_actions.is_empty() {
            body = body.child(
                div()
                    .text_sm()
                    .text_color(rgb(C_MUTED))
                    .child("元に戻せる操作はありません（名前変更とゴミ箱のみ記録します）"),
            );
        }
        if !undo_actions.is_empty() {
            body = body.child(div().text_xs().text_color(rgb(C_MUTED)).child(format!(
                "元に戻せる操作（{} 件・新しい順）",
                undo_actions.len()
            )));
            for action in &undo_actions {
                body = body.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w(px(120.0))
                                .text_sm()
                                .text_color(rgb(C_TEXT))
                                .child(action.label()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(rgb(C_SUBTEXT))
                                .child(Self::undo_action_detail(action)),
                        ),
                );
            }
        }
        if !redo_actions.is_empty() {
            body = body.child(div().text_xs().text_color(rgb(C_MUTED)).child(format!(
                "やり直せる操作（{} 件・新しい順）",
                redo_actions.len()
            )));
            for action in &redo_actions {
                body = body.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .w(px(120.0))
                                .text_sm()
                                .text_color(rgb(C_TEXT))
                                .child(action.label()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(rgb(C_SUBTEXT))
                                .child(Self::undo_action_detail(action)),
                        ),
                );
            }
        }
        body = body.child(
            div().flex().justify_end().child(
                Button::new("undo-history-close")
                    .label("閉じる")
                    .on_click(close),
            ),
        );
        body = body.child(
            div()
                .text_xs()
                .text_color(rgb(C_MUTED))
                .child("Ctrl+Shift+U / Esc で閉じる（読み取り専用）"),
        );
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(backdrop)
                .child(body),
        )
    }

    /// One-line description of an undoable action.
    fn undo_action_detail(action: &UndoAction) -> String {
        match action {
            UndoAction::Rename { from, to } => format!(
                "{} → {}",
                from.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| display_path(from)),
                to.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| display_path(to)),
            ),
            UndoAction::Trash { items } => {
                if items.len() == 1 {
                    display_path(&items[0].0)
                } else {
                    format!("{} 件", items.len())
                }
            }
        }
    }

    /// Development hook: exercise a reversible operation and its undo.
    fn run_debug_undo(&mut self, mode: &str, cx: &mut Context<Self>) {
        let Some(entry) = self.entries.iter().find(|entry| !entry.is_dir).cloned() else {
            eprintln!("undo-report: no file to operate on");
            return;
        };
        match mode {
            "trash" => {
                let path = entry.path.clone();
                match trash::delete(&path) {
                    Ok(()) => {
                        let recorded = trash_view::list()
                            .ok()
                            .and_then(|items| {
                                items
                                    .iter()
                                    .find(|item| item.original_path() == path)
                                    .map(|item| {
                                        vec![(path.clone(), item.id.to_string_lossy().into_owned())]
                                    })
                            })
                            .unwrap_or_default();
                        eprintln!(
                            "undo-report: trashed {} recorded={}",
                            path.display(),
                            recorded.len()
                        );
                        if !recorded.is_empty() {
                            self.undo.push(UndoAction::Trash { items: recorded });
                            self.undo_last(cx);
                        }
                    }
                    Err(error) => eprintln!("undo-report: trash failed: {error}"),
                }
            }
            _ => {
                let from = entry.path.clone();
                let name = format!("{}.renamed-by-debug", entry.name_text);
                match ops::rename_entry(&from, &name) {
                    Ok(to) => {
                        eprintln!(
                            "undo-report: renamed {} -> {}",
                            from.display(),
                            to.display()
                        );
                        self.undo.push(UndoAction::Rename {
                            from: from.clone(),
                            to: to.clone(),
                        });
                        self.undo_last(cx);
                    }
                    Err(error) => eprintln!("undo-report: rename failed: {error}"),
                }
            }
        }
    }

    /// Undo the most recent reversible operation (Ctrl+Z).
    ///
    /// Only rename and trash are undoable; copy/move are not, because undoing
    /// them would delete data the user may have already changed.
    fn undo_last(&mut self, cx: &mut Context<Self>) {
        if self.op_task.is_some() {
            self.status = STATUS_OP_RUNNING.to_string();
            cx.notify();
            return;
        }
        let Some(action) = self.undo.pop_undo() else {
            self.status = "元に戻せる操作がありません".to_string();
            cx.notify();
            return;
        };
        let label = action.label();
        let affected = action.len();
        let retry = action.clone();
        match action {
            UndoAction::Rename { from, to } => {
                let target = from.clone();
                let current = to.clone();
                let name = from
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                if !current.exists() {
                    self.status =
                        format!("元に戻せません: {} が存在しません", display_path(&current));
                    // Keep the action so a later retry or other undo is possible.
                    self.undo.push_undo(retry.clone());
                    cx.notify();
                    return;
                }
                if target.exists() {
                    self.status =
                        format!("元に戻せません: {} が既に存在します", display_path(&target));
                    self.undo.push_undo(retry.clone());
                    cx.notify();
                    return;
                }
                let rename_source = current.clone();
                let background =
                    cx.background_spawn(async move { ops::rename_entry(&rename_source, &name) });
                let action_for_redo = UndoAction::Rename { from, to: current };
                cx.spawn(async move |this, cx| {
                    let result = background.await;
                    let _ = this.update(cx, |this, cx| {
                        match result {
                            Ok(restored) => {
                                this.suppress_for(SELF_OP_SUPPRESS);
                                this.undo.push_redo(action_for_redo);
                                eprintln!(
                                    "undo-report: rename undone, restored={}",
                                    restored.display()
                                );
                                this.status = format!(
                                    "{label}を元に戻しました（{affected} 件）: {}",
                                    display_path(&restored)
                                );
                                this.pending_select = Some(restored);
                                this.reload(cx);
                            }
                            Err(error) => {
                                this.undo.push_undo(action_for_redo);
                                this.status = format!("{STATUS_UNDO_FAILED}: {error}");
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            UndoAction::Trash { items } => {
                let action_for_redo = UndoAction::Trash {
                    items: items.clone(),
                };
                let background =
                    cx.background_spawn(async move { trash_view::restore_by_id(&items) });
                cx.spawn(async move |this, cx| {
                    let result = background.await;
                    let _ = this.update(cx, |this, cx| {
                        match result {
                            Ok((restored, missing)) => {
                                this.suppress_for(SELF_OP_SUPPRESS);
                                if restored > 0 {
                                    this.undo.push_redo(action_for_redo);
                                } else {
                                    this.undo.push_undo(action_for_redo);
                                }
                                eprintln!(
                                    "undo-report: trash undone, restored={restored} missing={missing}"
                                );
                                this.status = if missing == 0 {
                                    format!("{label}を元に戻しました（{restored} 件）")
                                } else {
                                    format!(
                                        "{label}を元に戻しました（{restored} 件、{missing} 件はゴミ箱にありません）"
                                    )
                                };
                                this.reload(cx);
                            }
                            Err(error) => {
                                this.undo.push_undo(action_for_redo);
                                this.status = format!("{STATUS_UNDO_FAILED}: {error}");
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
        }
        cx.notify();
    }

    /// Redo the most recently undone operation (Ctrl+Shift+Z).
    fn redo_last(&mut self, cx: &mut Context<Self>) {
        if self.op_task.is_some() {
            self.status = STATUS_OP_RUNNING.to_string();
            cx.notify();
            return;
        }
        let Some(action) = self.undo.pop_redo() else {
            self.status = "やり直せる操作がありません".to_string();
            cx.notify();
            return;
        };
        let label = action.label();
        match action {
            UndoAction::Rename { from, to } => {
                // Redo performs the rename again: from -> to.
                let current = from.clone();
                let name = to
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default();
                let action_for_undo = UndoAction::Rename {
                    from: from.clone(),
                    to: to.clone(),
                };
                let background =
                    cx.background_spawn(async move { ops::rename_entry(&current, &name) });
                cx.spawn(async move |this, cx| {
                    let result = background.await;
                    let _ = this.update(cx, |this, cx| {
                        match result {
                            Ok(redone) => {
                                this.suppress_for(SELF_OP_SUPPRESS);
                                this.undo.push_undo(action_for_undo);
                                tracing::info!(
                                    op = "redo",
                                    action = "rename",
                                    path = %display_path(&redone),
                                    "undo/redo operation"
                                );
                                this.status =
                                    format!("{label}をやり直しました: {}", display_path(&redone));
                                this.pending_select = Some(redone);
                                this.reload(cx);
                            }
                            Err(error) => {
                                this.undo.push_redo(action_for_undo);
                                this.status = format!("やり直せません: {error}");
                            }
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
            UndoAction::Trash { items } => {
                // Redo means trashing the restored paths again; look them up by
                // path and send them back to the trash.
                let paths: Vec<PathBuf> = items
                    .iter()
                    .map(|(path, _)| path.clone())
                    .filter(|path| path.exists() || path.is_symlink())
                    .collect();
                if paths.is_empty() {
                    self.undo.push_redo(UndoAction::Trash { items });
                    self.status = STATUS_REDO_NO_TARGET.to_string();
                    cx.notify();
                    return;
                }
                let action_for_undo = UndoAction::Trash {
                    items: items.clone(),
                };
                let background = cx.background_spawn(async move {
                    let mut trashed: Vec<PathBuf> = Vec::new();
                    let mut failures: Vec<(PathBuf, String)> = Vec::new();
                    for path in paths {
                        match trash::delete(&path) {
                            Ok(()) => trashed.push(path),
                            Err(error) => failures.push((path, error.to_string())),
                        }
                    }
                    (trashed, failures)
                });
                cx.spawn(async move |this, cx| {
                    let result = background.await;
                    let _ = this.update(cx, |this, cx| {
                        let (trashed, failures) = result;
                        tracing::info!(
                            op = "redo",
                            action = "trash",
                            items = trashed.len(),
                            failures = failures.len(),
                            "undo/redo operation"
                        );
                        if trashed.is_empty() {
                            this.undo.push_redo(action_for_undo);
                            this.status = "やり直せません".to_string();
                        } else {
                            this.suppress_for(SELF_OP_SUPPRESS);
                            if let Ok(items) = trash_view::list() {
                                let recorded: Vec<(PathBuf, String)> = trashed
                                    .iter()
                                    .filter_map(|path| {
                                        items.iter().find(|item| item.original_path() == *path).map(
                                            |item| {
                                                (
                                                    path.clone(),
                                                    item.id.to_string_lossy().into_owned(),
                                                )
                                            },
                                        )
                                    })
                                    .collect();
                                this.undo.push_undo(UndoAction::Trash { items: recorded });
                            }
                            this.status = if failures.is_empty() {
                                format!("{label}をやり直しました（{} 件）", trashed.len())
                            } else {
                                let (path, reason) = &failures[0];
                                let name = path
                                    .file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| display_path(path));
                                format!(
                                    "{label}をやり直しました（{} 件、{} 件失敗: 「{}」: {}）",
                                    trashed.len(),
                                    failures.len(),
                                    name,
                                    reason
                                )
                            };
                            this.reload(cx);
                        }
                        cx.notify();
                    });
                })
                .detach();
            }
        }
        cx.notify();
    }

    // ----------------------------------------------------------------- preview

    fn open_preview_selected(&mut self, cx: &mut Context<Self>) {
        if self.view != ViewSource::Directory {
            self.status = "プレビューはディレクトリ表示でのみ使えます".to_string();
            cx.notify();
            return;
        }
        let Some(entry) = self.entries.get(self.selected) else {
            return;
        };
        if entry.is_dir {
            self.status = "フォルダはプレビューできません".to_string();
            cx.notify();
            return;
        }
        if entry.is_symlink {
            self.status = "シンボリックリンクはプレビューできません".to_string();
            cx.notify();
            return;
        }
        let path = entry.path.clone();
        let title = entry.name_text.clone();
        self.open_preview(path, title, cx);
    }

    /// Read a text or image preview on the background executor (bounded,
    /// read-only). Known image extensions load as an image, everything else
    /// as text.
    fn open_preview(&mut self, path: PathBuf, title: String, cx: &mut Context<Self>) {
        let image_format = preview::image_format_for_name(&title);
        self.preview = Some(PreviewState {
            title,
            loading: true,
            text: None,
            image: None,
            error: None,
            truncated: false,
            size: 0,
            task: None,
        });
        cx.notify();
        let background = cx.background_spawn(async move {
            match image_format {
                Some(format) => {
                    preview::read_image_preview(&path, format, preview::MAX_IMAGE_BYTES)
                        .map(|image| (Some(image), None))
                }
                None => {
                    let read = preview::read_preview(&path, preview::MAX_PREVIEW_BYTES)?;
                    Ok((None, Some(read)))
                }
            }
        });
        let task = cx.spawn(async move |this, cx| {
            let result = background.await;
            let _ = this.update(cx, |this, cx| {
                let Some(preview) = this.preview.as_mut() else {
                    return;
                };
                preview.loading = false;
                preview.task = None;
                if this.debug.preview_report {
                    match &result {
                        Ok((Some(image), _)) => eprintln!(
                            "preview-report: ok kind=image format={:?} bytes={}",
                            image.format,
                            image.bytes.len()
                        ),
                        Ok((None, Some(read))) => eprintln!(
                            "preview-report: ok kind=text bytes={} truncated={} chars={}",
                            read.size,
                            read.truncated,
                            read.text.chars().count()
                        ),
                        Ok((None, None)) => {
                            eprintln!("preview-report: ok kind=unknown")
                        }
                        Err(error) => eprintln!("preview-report: error {error}"),
                    }
                }
                match result {
                    Ok((image, text)) => {
                        if let Some(image) = &image {
                            preview.size = image.bytes.len() as u64;
                        }
                        preview.image = image;
                        if let Some(read) = text {
                            preview.truncated = read.truncated;
                            preview.size = read.size;
                            preview.text = Some(read.text);
                        }
                    }
                    Err(error) => preview.error = Some(error),
                }
                cx.notify();
            });
        });
        if let Some(preview) = self.preview.as_mut() {
            preview.task = Some(task);
        }
    }

    fn close_preview(&mut self, cx: &mut Context<Self>) {
        self.preview = None;
        self.focus_list_on_next_render = true;
        cx.notify();
    }

    /// Read-only preview overlay with a scrollable text area.
    fn preview_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        let preview = self.preview.as_ref()?;
        let close = cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.close_preview(cx);
        });
        let backdrop = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(gpui::black().opacity(0.45))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _window, cx| {
                    this.close_preview(cx);
                }),
            );
        let meta = if preview.loading {
            "読み込み中…".to_string()
        } else if let Some(error) = &preview.error {
            error.clone()
        } else if preview.truncated {
            format!(
                "{}（先頭 {} のみ表示）",
                human_size(preview.size),
                human_size(preview::MAX_PREVIEW_BYTES)
            )
        } else {
            human_size(preview.size)
        };
        let body_text = preview.text.clone().unwrap_or_default();
        let mut panel = div()
            .id("preview-dialog")
            .w(px(820.0))
            .max_h(px(620.0))
            .bg(rgb(C_SURFACE))
            .rounded_lg()
            .border_1()
            .border_color(rgb(C_OVERLAY))
            .shadow_lg()
            .p_5()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .text_lg()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .truncate()
                            .child(preview.title.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(if preview.error.is_some() {
                                rgb(C_RED)
                            } else {
                                rgb(C_MUTED)
                            })
                            .child(meta),
                    ),
            );
        if let Some(image) = &preview.image {
            panel = panel.child(
                div()
                    .id("preview-image")
                    .max_h(px(480.0))
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(rgb(C_BASE))
                    .rounded_md()
                    .p_3()
                    .child(
                        gpui_kit::img(std::sync::Arc::new(gpui_kit::Image::from_bytes(
                            image.format,
                            image.bytes.clone(),
                        )))
                        .max_h(px(440.0))
                        .max_w(px(740.0)),
                    ),
            );
        } else if preview.text.is_some() {
            panel = panel.child(
                div()
                    .id("preview-text")
                    .max_h(px(480.0))
                    .overflow_y_scroll()
                    .bg(rgb(C_BASE))
                    .rounded_md()
                    .p_3()
                    .font_family("monospace")
                    .text_xs()
                    .text_color(rgb(C_TEXT))
                    .child(body_text),
            );
        }
        panel = panel.child(
            div()
                .flex()
                .justify_end()
                .child(Button::new("preview-close").label("閉じる").on_click(close)),
        );
        panel = panel.child(
            div()
                .text_xs()
                .text_color(rgb(C_MUTED))
                .child("Space / F3 / Esc で閉じる（読み取り専用）"),
        );
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(backdrop)
                .child(panel),
        )
    }

    // ---------------------------------------------------------------- location

    /// Open the Ctrl+L location bar with the current path prefilled.
    fn open_location(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.view != ViewSource::Directory {
            self.status = "パス入力はディレクトリ表示でのみ使えます".to_string();
            cx.notify();
            return;
        }
        if let Some(location) = &self.location {
            let focus = location.input.read(cx).focus_handle(cx).clone();
            focus.focus(window, cx);
            return;
        }
        let value = display_path(&self.cwd);
        let input = cx.new(|cx| InputState::new(window, cx).default_value(value));
        let subscription = cx.subscribe(&input, |this, _input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.confirm_location(cx);
            }
        });
        let focus = input.read(cx).focus_handle(cx).clone();
        focus.focus(window, cx);
        self.location = Some(LocationState {
            input,
            _subscription: subscription,
            error: None,
        });
        cx.notify();
    }

    fn close_location(&mut self, cx: &mut Context<Self>) {
        self.location = None;
        self.focus_list_on_next_render = true;
        cx.notify();
    }

    fn confirm_location(&mut self, cx: &mut Context<Self>) {
        let Some(location) = self.location.as_ref() else {
            return;
        };
        let value = location.input.read(cx).value().to_string();
        let home = dirs::home_dir();
        let resolved = match resolve_location_input(&self.cwd, &value, home.as_deref()) {
            Ok(path) => path,
            Err(error) => {
                self.set_location_error(error, cx);
                return;
            }
        };
        if !resolved.is_dir() {
            self.set_location_error(
                format!("{STATUS_NOT_DIRECTORY}: {}", display_path(&resolved)),
                cx,
            );
            return;
        }
        self.location = None;
        self.focus_list_on_next_render = true;
        self.navigate_to(resolved, cx);
    }

    fn set_location_error(&mut self, error: String, cx: &mut Context<Self>) {
        if let Some(location) = self.location.as_mut() {
            location.error = Some(error);
        }
        cx.notify();
    }

    /// Path input bar shown while Ctrl+L is open.
    fn location_bar(&self) -> Option<Div> {
        let location = self.location.as_ref()?;
        let mut bar = div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_1()
            .bg(rgb(C_MANTLE))
            .border_b_1()
            .border_color(rgb(C_SURFACE))
            .child(div().text_sm().text_color(rgb(C_SUBTEXT)).child("パス:"))
            .child(div().flex_1().child(Input::new(&location.input)))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(C_MUTED))
                    .child("Enter で移動 / Esc で閉じる"),
            );
        if let Some(error) = &location.error {
            bar = bar.child(div().text_xs().text_color(rgb(C_RED)).child(error.clone()));
        }
        Some(bar)
    }

    // -------------------------------------------------------------------- help

    /// F1 keyboard-help overlay (read-only).
    fn help_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.help_open {
            return None;
        }
        let close = cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.help_open = false;
            cx.notify();
        });
        let backdrop = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(gpui::black().opacity(0.45))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _window, cx| {
                    this.help_open = false;
                    cx.notify();
                }),
            );
        let mut list = div()
            .id("help-list")
            .flex()
            .flex_col()
            .gap_3()
            .max_h(px(500.0))
            .overflow_y_scroll();
        for (section, shortcuts) in HELP_SECTIONS {
            list = list.child(div().text_xs().text_color(rgb(C_MUTED)).child(*section));
            for (key, description) in *shortcuts {
                list = list.child(
                    div()
                        .flex()
                        .items_center()
                        .gap_3()
                        .child(
                            div()
                                .w(px(230.0))
                                .text_sm()
                                .text_color(rgb(C_TEXT))
                                .child(*key),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .text_color(rgb(C_SUBTEXT))
                                .child(*description),
                        ),
                );
            }
        }
        let body = div()
            .w(px(720.0))
            .bg(rgb(C_SURFACE))
            .rounded_lg()
            .border_1()
            .border_color(rgb(C_OVERLAY))
            .shadow_lg()
            .p_5()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("キーボードショートカット"),
            )
            .child(list)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(C_MUTED))
                            .child("F1 / Esc で閉じる（一覧はスクロールできます）"),
                    )
                    .child(Button::new("help-close").label("閉じる").on_click(close)),
            );
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(backdrop)
                .child(body),
        )
    }

    // -------------------------------------------------------------- properties

    /// Open the read-only properties dialog for `path`.
    ///
    /// Metadata is read once here (never from render). Symlinks are inspected
    /// with `symlink_metadata` and their target with `read_link`.
    fn open_properties(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        let link_metadata = std::fs::symlink_metadata(&path);
        let Ok(link_metadata) = link_metadata else {
            self.status = format!("{STATUS_PATH_GONE}: {}", display_path(&path));
            self.reload(cx);
            return;
        };
        let is_symlink = link_metadata.file_type().is_symlink();
        let metadata = std::fs::metadata(&path)
            .ok()
            .or(Some(link_metadata.clone()));
        let kind = if is_symlink {
            "シンボリックリンク"
        } else if link_metadata.is_dir() {
            "フォルダ"
        } else {
            "ファイル"
        };
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| display_path(&path));
        let size = if link_metadata.is_dir() {
            None
        } else {
            metadata.as_ref().map(|metadata| metadata.len())
        };
        let modified = metadata
            .as_ref()
            .and_then(|metadata| metadata.modified().ok());
        #[cfg(unix)]
        let permissions = {
            use std::os::unix::fs::PermissionsExt;
            metadata
                .as_ref()
                .map(|metadata| model::permissions_string(metadata.permissions().mode() & 0o777))
        };
        #[cfg(not(unix))]
        let permissions: Option<String> = None;
        let symlink_target = if is_symlink {
            std::fs::read_link(&path).ok()
        } else {
            None
        };
        self.properties = Some(PropsState {
            name,
            path,
            kind,
            is_dir: link_metadata.is_dir(),
            size,
            modified,
            permissions,
            symlink_target,
            size_loading: false,
            dir_size: None,
            size_task: None,
            size_cancel: None,
        });
        cx.notify();
    }

    fn open_selected_properties(&mut self, cx: &mut Context<Self>) {
        if self.view != ViewSource::Directory {
            self.status = "プロパティはディレクトリ表示でのみ表示できます".to_string();
            cx.notify();
            return;
        }
        let Some(entry) = self.entries.get(self.selected) else {
            return;
        };
        self.open_properties(entry.path.clone(), cx);
    }

    fn close_properties(&mut self, cx: &mut Context<Self>) {
        if let Some(cancel) = self
            .properties
            .as_mut()
            .and_then(|props| props.size_cancel.take())
        {
            cancel.cancel();
        }
        self.properties = None;
        cx.notify();
    }

    /// Calculate a directory's recursive size on the background executor.
    /// Read-only; symlinks are not followed; closing the dialog cancels it.
    fn start_dir_size(&mut self, cx: &mut Context<Self>) {
        let Some(props) = self.properties.as_ref() else {
            return;
        };
        if !props.is_dir || props.size_loading {
            return;
        }
        let path = props.path.clone();
        let cancel = Arc::new(CancelFlag::new());
        if let Some(props) = self.properties.as_mut() {
            props.size_loading = true;
            props.dir_size = None;
            props.size_cancel = Some(cancel.clone());
        }
        cx.notify();
        let background = cx.background_spawn(async move { ops::dir_stats(&path, &cancel) });
        let task = cx.spawn(async move |this, cx| {
            let stats = background.await;
            let _ = this.update(cx, |this, cx| {
                let Some(props) = this.properties.as_mut() else {
                    return;
                };
                props.size_loading = false;
                props.size_task = None;
                props.size_cancel = None;
                props.dir_size = Some(stats);
                cx.notify();
            });
        });
        if let Some(props) = self.properties.as_mut() {
            props.size_task = Some(task);
        }
    }

    /// Read-only properties dialog.
    fn properties_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        let props = self.properties.as_ref()?;
        let close = cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.close_properties(cx);
        });
        let backdrop = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(gpui::black().opacity(0.45))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _window, cx| {
                    this.close_properties(cx);
                }),
            );
        let row = |label: &'static str, value: String| {
            div()
                .flex()
                .gap_3()
                .items_start()
                .child(
                    div()
                        .w(px(96.0))
                        .text_xs()
                        .text_color(rgb(C_MUTED))
                        .child(label),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .text_color(rgb(C_TEXT))
                        .child(value),
                )
        };
        let mut form = div()
            .w(px(560.0))
            .bg(rgb(C_SURFACE))
            .rounded_lg()
            .border_1()
            .border_color(rgb(C_OVERLAY))
            .shadow_lg()
            .p_5()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("プロパティ"),
            )
            .child(row("名前", props.name.clone()))
            .child(row("パス", display_path(&props.path)))
            .child(row("種類", props.kind.to_string()));
        if props.is_dir {
            let value = if props.size_loading {
                "計算中…".to_string()
            } else if let Some(stats) = props.dir_size {
                format!(
                    "{}（{} ファイル / {} フォルダ）",
                    human_size(stats.bytes),
                    stats.files,
                    stats.dirs
                )
            } else {
                "-".to_string()
            };
            let calculate = cx.listener(|this, _: &ClickEvent, _window, cx| {
                this.start_dir_size(cx);
            });
            form = form
                .child(row("サイズ", value))
                .child(div().flex().justify_end().child(if props.size_loading {
                    div()
                        .text_xs()
                        .text_color(rgb(C_MUTED))
                        .child("サイズを計算中…（Esc で中止）")
                        .into_any_element()
                } else {
                    Button::new("props-size")
                        .label("サイズを計算")
                        .on_click(calculate)
                        .into_any_element()
                }));
        } else if let Some(size) = props.size {
            form = form.child(row(
                "サイズ",
                format!("{}（{size} バイト）", human_size(size)),
            ));
        } else {
            form = form.child(row("サイズ", "-".to_string()));
        }
        form = form.child(row("更新日時", human_datetime(props.modified)));
        if let Some(permissions) = &props.permissions {
            form = form.child(row("パーミッション", permissions.clone()));
        }
        if let Some(target) = &props.symlink_target {
            form = form.child(row("リンク先", display_path(target)));
        }
        form = form.child(
            div()
                .flex()
                .justify_end()
                .child(Button::new("props-close").label("閉じる").on_click(close)),
        );
        form = form.child(
            div()
                .text_xs()
                .text_color(rgb(C_MUTED))
                .child("Esc で閉じる（読み取り専用）"),
        );
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(backdrop)
                .child(form),
        )
    }

    // ------------------------------------------------------------------ rename

    /// Open the inline rename dialog for the selected entry (F2 / menu).
    fn start_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.renaming.is_some() {
            return;
        }
        let Some(entry) = self.entries.get(self.selected) else {
            return;
        };
        let target = entry.path.clone();
        let original = entry.name_text.clone();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(original.clone()));
        let subscription = cx.subscribe(&input, |this, _input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.confirm_rename(cx);
            }
        });
        let focus = input.read(cx).focus_handle(cx).clone();
        focus.focus(window, cx);
        self.renaming = Some(RenameState {
            target,
            original,
            input,
            _subscription: subscription,
            error: None,
        });
        cx.notify();
    }

    /// Validate the typed name and rename on the background executor.
    ///
    /// Safety: the new name is validated, an existing destination is rejected
    /// before spawning, and the engine refuses to overwrite. On error the
    /// dialog stays open and the original file is untouched.
    fn confirm_rename(&mut self, cx: &mut Context<Self>) {
        let Some(rename) = self.renaming.as_ref() else {
            return;
        };
        let value = rename.input.read(cx).value().to_string();
        let target = rename.target.clone();
        let original = rename.original.clone();
        if value == original {
            self.close_rename(cx);
            return;
        }
        if let Err(error) = ops::validate_file_name(&value) {
            self.set_rename_error(&error.to_string(), cx);
            return;
        }
        let Some(parent) = target.parent().map(|parent| parent.to_path_buf()) else {
            self.set_rename_error("親ディレクトリがありません", cx);
            return;
        };
        let destination = parent.join(&value);
        if destination.exists() {
            self.set_rename_error(&format!("既に存在します: {}", destination.display()), cx);
            return;
        }

        let rename_source = target.clone();
        let background =
            cx.background_spawn(async move { ops::rename_entry(&rename_source, &value) });
        let rename_original = target;
        cx.spawn(async move |this, cx| {
            let result = background.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(new_path) => {
                        this.suppress_for(SELF_OP_SUPPRESS);
                        tracing::info!(
                            op = "rename",
                            path = %display_path(&new_path),
                            "file operation completed"
                        );
                        this.undo.push(UndoAction::Rename {
                            from: rename_original,
                            to: new_path.clone(),
                        });
                        this.status = format!(
                            "名前を変更しました: {} → {}",
                            original,
                            new_path
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        );
                        this.pending_select = Some(new_path);
                        this.close_rename(cx);
                        this.reload(cx);
                        this.restore_focus_next_frame();
                    }
                    Err(error) => {
                        this.set_rename_error(&format!("変更できません: {error}"), cx);
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn cancel_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.renaming.take().is_some() {
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    fn close_rename(&mut self, cx: &mut Context<Self>) {
        if self.renaming.take().is_some() {
            cx.notify();
        }
    }

    fn set_rename_error(&mut self, message: &str, cx: &mut Context<Self>) {
        if let Some(rename) = &mut self.renaming {
            rename.error = Some(message.to_string());
        }
        cx.notify();
    }

    /// Ask the next render to return keyboard focus to the list. The dialog's
    /// input entity is gone after a successful rename, so focus would
    /// otherwise be lost; scheduling from `render` is the only place where a
    /// `Window` is available on the completion path.
    fn restore_focus_next_frame(&mut self) {
        self.focus_list_on_next_render = true;
    }

    /// Modal overlay for the rename dialog (backdrop + centered form).
    fn rename_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        let rename = self.renaming.as_ref()?;
        let backdrop = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(gpui::black().opacity(0.45))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    this.cancel_rename(window, cx);
                }),
            );
        let cancel = cx.listener(|this, _: &ClickEvent, window, cx| {
            this.cancel_rename(window, cx);
        });
        let confirm = cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.confirm_rename(cx);
        });

        let mut form = div()
            .w(px(480.0))
            .bg(rgb(C_SURFACE))
            .rounded_lg()
            .border_1()
            .border_color(rgb(C_OVERLAY))
            .shadow_lg()
            .p_5()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("名前変更"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(C_SUBTEXT))
                    .truncate()
                    .child(display_path(&rename.target)),
            )
            .child(Input::new(&rename.input).w_full());
        if let Some(error) = &rename.error {
            form = form.child(div().text_sm().text_color(rgb(C_RED)).child(error.clone()));
        }
        form = form.child(
            div()
                .flex()
                .justify_end()
                .gap_2()
                .child(
                    Button::new("rename-cancel")
                        .label("キャンセル")
                        .on_click(cancel),
                )
                .child(
                    Button::new("rename-confirm")
                        .label("名前変更")
                        .on_click(confirm),
                ),
        );
        form = form.child(
            div()
                .text_xs()
                .text_color(rgb(C_MUTED))
                .child("Enter で確定 / Esc でキャンセル"),
        );

        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(backdrop)
                .child(form),
        )
    }

    fn reveal(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if !path.exists() {
            self.status = format!("{STATUS_PATH_GONE}: {}", display_path(&path));
            self.reload(cx);
            return;
        }
        cx.reveal_path(&path);
        self.status = format!("ファイルマネージャで表示: {}", display_path(&path));
        cx.notify();
    }

    fn trash_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = self.action_paths();
        if paths.is_empty() {
            return;
        }
        self.trash_paths(paths, window, cx);
    }

    /// Ask for confirmation, then move the given paths to the system trash on
    /// the background executor.
    ///
    /// Safety: confirmation is always shown (with the item count), sources are
    /// re-checked before deletion, and permanent deletion is never performed.
    fn trash_paths(&mut self, paths: Vec<PathBuf>, window: &mut Window, cx: &mut Context<Self>) {
        let mut existing: Vec<PathBuf> = Vec::new();
        for path in paths {
            if path.exists() || path.is_symlink() {
                existing.push(path);
            }
        }
        if existing.is_empty() {
            self.status = "対象は既に存在しません".to_string();
            self.reload(cx);
            return;
        }
        let count = existing.len();
        let names: Vec<String> = existing
            .iter()
            .filter_map(|path| {
                path.file_name()
                    .map(|name| name.to_string_lossy().into_owned())
            })
            .collect();
        let (message, detail) = if count == 1 {
            (
                format!("「{}」をゴミ箱へ移動しますか？", names[0]),
                "システムのゴミ箱へ移動します（あとで元に戻せます）。".to_string(),
            )
        } else {
            let preview = names.iter().take(3).cloned().collect::<Vec<_>>().join("、");
            (
                format!("{count} 件をゴミ箱へ移動しますか？"),
                format!("{preview} ほか。システムのゴミ箱へ移動します（あとで元に戻せます）。"),
            )
        };
        let receiver = window.prompt(
            PromptLevel::Warning,
            &message,
            Some(&detail),
            &["cancel", "ゴミ箱へ移動"],
            cx,
        );
        let background = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let answer = receiver.await.unwrap_or(0);
            if answer != 1 {
                return;
            }
            let targets = existing.clone();
            let result = background
                .spawn(async move {
                    let mut trashed: Vec<PathBuf> = Vec::new();
                    let mut failures: Vec<(PathBuf, String)> = Vec::new();
                    for path in targets {
                        match trash::delete(&path) {
                            Ok(()) => trashed.push(path),
                            Err(error) => failures.push((path, error.to_string())),
                        }
                    }
                    (trashed, failures)
                })
                .await;
            let _ = this.update(cx, |this, cx| {
                let (trashed, failures) = result;
                if trashed.is_empty() {
                    this.load = LoadState::Error {
                        generation: this.generation,
                        error: format!(
                            "ゴミ箱へ移動できません: {}",
                            failures
                                .first()
                                .map(|(_, message)| message.clone())
                                .unwrap_or_default()
                        ),
                    };
                } else {
                    this.suppress_for(SELF_OP_SUPPRESS);
                    let mut message = if trashed.len() == 1 {
                        format!(
                            "「{}」をゴミ箱へ移動しました",
                            trashed[0]
                                .file_name()
                                .map(|name| name.to_string_lossy().into_owned())
                                .unwrap_or_default()
                        )
                    } else {
                        format!("{} 件をゴミ箱へ移動しました", trashed.len())
                    };
                    if let Some((path, reason)) = failures.first() {
                        let name = path
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| display_path(path));
                        message.push_str(&format!(
                            "（{} 件失敗: 「{}」: {}）",
                            failures.len(),
                            name,
                            reason
                        ));
                    }
                    this.status = message;
                    if let Ok(items) = trash_view::list() {
                        let recorded: Vec<(PathBuf, String)> = trashed
                            .iter()
                            .filter_map(|path| {
                                items.iter().find(|item| item.original_path() == *path).map(
                                    |item| (path.clone(), item.id.to_string_lossy().into_owned()),
                                )
                            })
                            .collect();
                        if !recorded.is_empty() {
                            this.undo.push(UndoAction::Trash { items: recorded });
                        }
                    }
                    this.all_entries
                        .retain(|entry| !trashed.contains(&entry.path));
                    this.fingerprint = fingerprint(&this.all_entries);
                    this.rebuild_view(None);
                    this.reload(cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    // ------------------------------------------------------------------- search

    /// Open the search bar (Ctrl+F). Only available from the directory view.
    fn open_search(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.view == ViewSource::Search {
            if let Some(input) = &self.search.input {
                let focus = input.read(cx).focus_handle(cx).clone();
                focus.focus(window, cx);
            }
            return;
        }
        if self.view != ViewSource::Directory {
            self.status = "検索はディレクトリ表示でのみ使えます".to_string();
            cx.notify();
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx));
        let subscription = cx.subscribe(&input, |this, _input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.run_search(cx);
            }
        });
        let ext_input = cx.new(|cx| InputState::new(window, cx));
        let ext_subscription =
            cx.subscribe(
                &ext_input,
                |this, _input, event: &InputEvent, cx| match event {
                    InputEvent::Change => this.apply_search_filter(cx),
                    InputEvent::PressEnter { .. } => this.run_search(cx),
                    _ => {}
                },
            );
        let focus = input.read(cx).focus_handle(cx).clone();
        focus.focus(window, cx);
        self.search.input = Some(input);
        self.search.subscription = Some(subscription);
        self.search.ext = Some(ext_input);
        self.search.ext_subscription = Some(ext_subscription);
        let name_input = cx.new(|cx| InputState::new(window, cx));
        let name_subscription = cx.subscribe(
            &name_input,
            |this, _input, event: &InputEvent, cx| match event {
                InputEvent::Change => this.apply_search_filter(cx),
                InputEvent::PressEnter { .. } => this.run_search(cx),
                _ => {}
            },
        );
        self.search.name_filter = Some(name_input);
        self.search.name_filter_subscription = Some(name_subscription);
        self.search.kind = SearchKind::All;
        self.search.mode = SearchMode::Name;
        self.search.scope = SearchScope::Current;
        self.search.query.clear();
        self.search.active_roots.clear();
        self.search.fell_back_to_cwd = false;
        self.search.generation = self.search.generation.wrapping_add(1);
        self.view = ViewSource::Search;
        self.selection.clear();
        self.selected = 0;
        self.status = "検索語を入力して Enter（Esc で閉じる）".to_string();
        self.update_watcher();
        cx.notify();
    }

    /// Per-root result cap for a multi-root search.
    ///
    /// Every root gets the same share so an early root cannot starve later
    /// ones; `merge_search_results` applies the global cap afterwards. A
    /// single root keeps the full global budget.
    fn per_root_cap(global_cap: usize, root_count: usize) -> usize {
        if root_count <= 1 {
            global_cap
        } else {
            // Divide the global cap fairly. Capping each root at 500 made a
            // 2-root search stop at 1000 even though the global cap was 2000
            // (third cross-review F6-07).
            global_cap.div_ceil(root_count).max(1)
        }
    }

    /// Recursive search on the background executor (Enter in the search bar).
    fn run_search(&mut self, cx: &mut Context<Self>) {
        let Some(input) = self.search.input.clone() else {
            return;
        };
        let query = input.read(cx).value().trim().to_string();
        if query.is_empty() {
            self.status = "検索語が空です".to_string();
            cx.notify();
            return;
        }
        self.search.query = query.clone();
        self.search.generation = self.search.generation.wrapping_add(1);
        let generation = self.search.generation;
        // Three layers guard against a stale result: the cooperative flag
        // stops the background walk, dropping the task discards its callback,
        // and the generation guard rejects anything still in flight.
        if let Some(cancel) = self.search.cancel.take() {
            cancel.store(true, Ordering::SeqCst);
        }
        let cancel = Arc::new(AtomicBool::new(false));
        self.search.cancel = Some(cancel.clone());
        self.search.task.take();

        let include_hidden = self.settings.show_hidden;
        let mode = self.search.mode;
        let scope = self.search.scope;
        let tab_paths: Vec<PathBuf> = self.tabs.tabs().iter().map(|tab| tab.cwd.clone()).collect();
        let bookmarks = self.bookmarks.clone();
        let cwd = self.cwd.clone();
        let resolved = search_roots(scope, &cwd, &tab_paths, &bookmarks);
        let roots = resolved.roots;
        self.search.active_roots = roots.clone();
        self.search.fell_back_to_cwd = resolved.fell_back_to_cwd;
        if resolved.fell_back_to_cwd {
            self.status = format!(
                "検索中…「{query}」（{}に有効な場所がないため現在のフォルダ）",
                scope.label()
            );
        } else if scope == SearchScope::Current {
            self.status = format!("検索中…「{query}」");
        } else {
            self.status = format!("検索中…「{query}」（{} 箇所）", roots.len());
        }
        cx.notify();
        let cancel_for_task = cancel.clone();
        let background = cx.background_spawn(async move {
            let global_cap = 2_000usize;
            let mut batches = Vec::with_capacity(roots.len());
            let mut total = 0usize;
            for root in &roots {
                if cancel_for_task.load(std::sync::atomic::Ordering::SeqCst) || total >= global_cap
                {
                    break;
                }
                let per_root_cap = Self::per_root_cap(global_cap, roots.len());
                let batch = match mode {
                    SearchMode::Name => search_by_name(
                        root,
                        &query,
                        SearchOptions {
                            include_hidden,
                            max_results: per_root_cap,
                            ..Default::default()
                        },
                        &cancel_for_task,
                    ),
                    SearchMode::Content => search_contents(
                        root,
                        &query,
                        ContentSearchOptions {
                            include_hidden,
                            max_results: per_root_cap,
                            ..Default::default()
                        },
                        &cancel_for_task,
                    ),
                };
                total += batch.len();
                batches.push(batch);
            }
            merge_search_results(batches, global_cap)
        });
        let task = cx.spawn(async move |this, cx| {
            let entries = background.await;
            let _ = this.update(cx, |this, cx| {
                if this.search.generation != generation {
                    return;
                }
                this.load = LoadState::Ready;
                this.search.task = None;
                this.search.cancel = None;
                this.search_lines = entries
                    .iter()
                    .filter_map(|result| {
                        result
                            .match_line
                            .map(|line| (result.entry.path.clone(), line))
                    })
                    .collect();
                this.all_entries = entries.into_iter().map(|result| result.entry).collect();
                this.apply_search_filter(cx);
                this.settings.record_search(&this.search.query);
                if !this.debug.ephemeral {
                    this.settings.save();
                }
            });
        });
        self.search.task = Some(task);
    }

    /// Recompute the visible search results from the last search's entries
    /// (kind/extension filters). Does not re-run the search.
    fn apply_search_filter(&mut self, cx: &mut Context<Self>) {
        let extension = self
            .search
            .ext
            .as_ref()
            .map(|input| input.read(cx).value().to_string());
        let name_filter = self
            .search
            .name_filter
            .as_ref()
            .map(|input| input.read(cx).value().to_string());
        let filtered = filter_results(
            &self.all_entries,
            self.search.kind,
            extension.as_deref(),
            name_filter.as_deref(),
        );
        let paths: Vec<PathBuf> = filtered.iter().map(|entry| entry.path.clone()).collect();
        self.selection.retain(&paths);
        self.entries = filtered;
        self.fingerprint = fingerprint(&self.entries);
        if self.selection.is_empty() {
            if let Some(entry) = self.entries.first() {
                self.selection.select_single(entry.path.clone());
            }
        }
        // Keep the focused row in sync with the surviving selection instead of
        // always resetting to 0 (third cross-review F6-08).
        self.selected = self
            .selection
            .paths()
            .first()
            .and_then(|path| self.entries.iter().position(|entry| &entry.path == path))
            .unwrap_or(0);
        self.scroll_handle
            .scroll_to_item(0, ScrollStrategy::Nearest);
        let mode_label = if self.search.mode == SearchMode::Content {
            "（内容）"
        } else {
            ""
        };
        let scope_label = match self.search.scope {
            SearchScope::Current => String::new(),
            scope => {
                if self.search.fell_back_to_cwd {
                    format!("・{}（現在のフォルダ）", scope.label())
                } else {
                    let count = self.search.active_roots.len();
                    if count > 0 {
                        format!("・{}（{} 箇所）", scope.label(), count)
                    } else {
                        format!("・{}", scope.label())
                    }
                }
            }
        };
        self.status = format!(
            "検索{mode_label}{scope_label}: 「{}」 {} 件",
            self.search.query,
            self.entries.len()
        );
        cx.notify();
    }

    /// Drop search state without touching the current view (used when
    /// navigating away).
    fn clear_search(&mut self) {
        if let Some(cancel) = self.search.cancel.take() {
            cancel.store(true, Ordering::SeqCst);
        }
        self.search.task.take();
        self.search.input = None;
        self.search.subscription = None;
        self.search.ext = None;
        self.search.ext_subscription = None;
        self.search.name_filter = None;
        self.search.name_filter_subscription = None;
        self.search.kind = SearchKind::All;
        self.search.mode = SearchMode::Name;
        self.search.scope = SearchScope::Current;
        self.search.query.clear();
        self.search.active_roots.clear();
        self.search.fell_back_to_cwd = false;
        self.search_lines.clear();
    }

    fn close_search(&mut self, cx: &mut Context<Self>) {
        if self.view != ViewSource::Search {
            return;
        }
        self.clear_search();
        self.view = ViewSource::Directory;
        self.selection.clear();
        self.selected = 0;
        self.status.clear();
        self.reload(cx);
    }

    fn search_bar(&self, cx: &mut Context<Self>) -> Option<Div> {
        let input = self.search.input.as_ref()?;
        let hint = if self.load.is_loading() {
            "検索中…"
        } else {
            "Enter で検索 / Esc で閉じる"
        };
        let mut modes = div().flex().items_center().gap_1();
        for (mode, label) in [(SearchMode::Name, "名前"), (SearchMode::Content, "内容")] {
            let active = self.search.mode == mode;
            modes = modes.child(
                div()
                    .id(("search-mode", mode as u64))
                    .px_2()
                    .py_0p5()
                    .rounded_md()
                    .text_xs()
                    .cursor_pointer()
                    .when(active, |button| button.bg(rgb(C_SURFACE)))
                    .text_color(if active { rgb(C_TEXT) } else { rgb(C_SUBTEXT) })
                    .hover(|style| style.bg(rgb(C_SURFACE)))
                    .child(label)
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        if this.search.mode == mode {
                            return;
                        }
                        this.search.mode = mode;
                        if mode == SearchMode::Content {
                            this.search.kind = SearchKind::All;
                        }
                        this.run_search(cx);
                    })),
            );
        }
        let mut scopes = div().flex().items_center().gap_1();
        for scope in [
            SearchScope::Current,
            SearchScope::OpenTabs,
            SearchScope::Bookmarks,
        ] {
            let active = self.search.scope == scope;
            scopes = scopes.child(
                div()
                    .id(("search-scope", scope as u64))
                    .px_2()
                    .py_0p5()
                    .rounded_md()
                    .text_xs()
                    .cursor_pointer()
                    .when(active, |button| button.bg(rgb(C_SURFACE)))
                    .text_color(if active { rgb(C_TEXT) } else { rgb(C_SUBTEXT) })
                    .hover(|style| style.bg(rgb(C_SURFACE)))
                    .child(scope.label())
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        if this.search.scope == scope {
                            return;
                        }
                        this.search.scope = scope;
                        this.run_search(cx);
                    })),
            );
        }
        let mut kinds = div().flex().items_center().gap_1();
        for (kind, label) in [
            (SearchKind::All, "すべて"),
            (SearchKind::Files, "ファイル"),
            (SearchKind::Dirs, "フォルダ"),
        ] {
            let active = self.search.kind == kind;
            kinds = kinds.child(
                div()
                    .id(("search-kind", kind as u64))
                    .px_2()
                    .py_0p5()
                    .rounded_md()
                    .text_xs()
                    .cursor_pointer()
                    .when(active, |button| button.bg(rgb(C_SURFACE)))
                    .text_color(if active { rgb(C_TEXT) } else { rgb(C_SUBTEXT) })
                    .hover(|style| style.bg(rgb(C_SURFACE)))
                    .child(label)
                    .on_click(cx.listener(move |this, _event, _window, cx| {
                        this.search.kind = kind;
                        this.apply_search_filter(cx);
                    })),
            );
        }
        let extension_input = self.search.ext.as_ref().map(|ext| {
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().text_xs().text_color(rgb(C_MUTED)).child("拡張子:"))
                .child(div().w(px(110.0)).child(Input::new(ext)))
        });
        let name_input = self.search.name_filter.as_ref().map(|input| {
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().text_xs().text_color(rgb(C_MUTED)).child("名前:"))
                .child(div().w(px(110.0)).child(Input::new(input)))
        });
        Some(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_1()
                .bg(rgb(C_MANTLE))
                .border_b_1()
                .border_color(rgb(C_SURFACE))
                .child(div().text_sm().text_color(rgb(C_SUBTEXT)).child("検索:"))
                .child(div().flex_1().child(Input::new(input)))
                .child(modes)
                .child(scopes)
                .children((self.search.mode == SearchMode::Name).then_some(kinds))
                .children(extension_input)
                .children(name_input)
                .child(div().text_xs().text_color(rgb(C_MUTED)).child(hint)),
        )
    }

    /// Chips with recent search queries, shown while the query field is empty.
    fn search_history_bar(&self, cx: &mut Context<Self>) -> Option<Div> {
        let input = self.search.input.as_ref()?;
        if !input.read(cx).value().is_empty() || self.settings.recent_searches.is_empty() {
            return None;
        }
        let mut row = div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_1()
            .bg(rgb(C_MANTLE))
            .border_b_1()
            .border_color(rgb(C_SURFACE))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(C_MUTED))
                    .child("最近の検索:"),
            );
        for query in self.settings.recent_searches.iter().take(8) {
            let value = query.clone();
            row = row.child(
                div()
                    .id(format!("search-history-{value}"))
                    .px_2()
                    .py_0p5()
                    .rounded_md()
                    .bg(rgb(C_BASE))
                    .text_xs()
                    .text_color(rgb(C_SUBTEXT))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(C_SURFACE)).text_color(rgb(C_TEXT)))
                    .child(value.clone())
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        if let Some(input) = this.search.input.clone() {
                            input.update(cx, |state, cx| {
                                state.set_value(value.clone(), window, cx);
                            });
                        }
                        this.run_search(cx);
                    })),
            );
        }
        Some(row)
    }

    // ------------------------------------------------------------------- trash

    fn restore_selected(&mut self, cx: &mut Context<Self>) {
        let paths = self.action_paths();
        self.restore_paths(paths, cx);
    }

    /// Restore trash items to their original locations (background executor).
    ///
    /// Restoring is non-destructive; collisions are reported by the `trash`
    /// crate and leave the affected items in the trash.
    fn restore_paths(&mut self, paths: Vec<PathBuf>, cx: &mut Context<Self>) {
        if self.view != ViewSource::Trash {
            return;
        }
        let items: Vec<trash::TrashItem> = self
            .trash_items
            .iter()
            .filter(|item| paths.contains(&PathBuf::from(&item.id)))
            .cloned()
            .collect();
        if items.is_empty() {
            self.status = "復元対象がありません".to_string();
            cx.notify();
            return;
        }
        let count = items.len();
        self.status = format!("{count} 件を復元中…");
        cx.notify();
        let background = cx.background_spawn(async move { trash_view::restore(items) });
        cx.spawn(async move |this, cx| {
            let result = background.await;
            let _ = this.update(cx, |this, cx| {
                this.status = match result {
                    Ok(()) => format!("{count} 件を元の場所へ復元しました"),
                    Err(error) => format!("復元できません: {error}"),
                };
                this.reload(cx);
                cx.notify();
            });
        })
        .detach();
    }

    // ------------------------------------------------------------- context menu

    /// Build the per-row context menu.
    ///
    /// The row **path** is captured when the menu is opened and every action is
    /// resolved by path on click, so a watcher refresh that re-sorts the list
    /// while the menu is open cannot make the action hit a different entry.
    fn entry_menu(
        entity: Entity<Self>,
        path: PathBuf,
        is_dir: bool,
        batch_paths: Vec<PathBuf>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        let weak = entity.downgrade();
        let batch_count = batch_paths.len();
        let batch_suffix = if batch_count > 1 {
            format!("（{batch_count} 件）")
        } else {
            String::new()
        };
        move |menu, _window, _menu_cx| {
            let open_weak = weak.clone();
            let reveal_weak = weak.clone();
            let copy_weak = weak.clone();
            let props_weak = weak.clone();
            let copy_sel_weak = weak.clone();
            let cut_sel_weak = weak.clone();
            let paste_weak = weak.clone();
            let rename_weak = weak.clone();
            let trash_weak = weak.clone();
            let new_folder_weak = weak.clone();
            let undo_weak = weak.clone();
            let history_weak = weak.clone();

            let open_path = path.clone();
            let reveal_path = path.clone();
            let copy_path = path.clone();
            let props_path = path.clone();
            let trash_paths = batch_paths.clone();
            let open_label = if is_dir {
                "開く"
            } else {
                "既定のアプリで開く"
            };

            menu.item(
                PopupMenuItem::new(open_label)
                    .icon(Icon::new(IconName::ExternalLink))
                    .on_click(move |_, _window, cx| {
                        let target = open_path.clone();
                        let _ = open_weak.update(cx, |this, cx| this.open_path(target, cx));
                    }),
            )
            .item(
                PopupMenuItem::new("ファイルマネージャで表示")
                    .icon(Icon::new(IconName::FolderOpen))
                    .on_click(move |_, _window, cx| {
                        let target = reveal_path.clone();
                        let _ = reveal_weak.update(cx, |this, cx| this.reveal(target, cx));
                    }),
            )
            .item(
                PopupMenuItem::new("パスをコピー")
                    .icon(Icon::new(IconName::Copy))
                    .on_click(move |_, _window, cx| {
                        let target = copy_path.clone();
                        let _ = copy_weak.update(cx, |this, cx| this.copy_path(target, cx));
                    }),
            )
            .item(
                PopupMenuItem::new("操作の履歴")
                    .icon(Icon::new(IconName::Clock))
                    .on_click(move |_, _window, cx| {
                        let _ = history_weak.update(cx, |this, cx| {
                            this.undo_history_open = true;
                            cx.notify();
                        });
                    }),
            )
            .item(
                PopupMenuItem::new("元に戻す")
                    .icon(Icon::new(IconName::Undo2))
                    .on_click(move |_, _window, cx| {
                        let _ = undo_weak.update(cx, |this, cx| this.undo_last(cx));
                    }),
            )
            .item(
                PopupMenuItem::new("プロパティ")
                    .icon(Icon::new(IconName::Info))
                    .on_click(move |_, _window, cx| {
                        let target = props_path.clone();
                        let _ = props_weak.update(cx, |this, cx| this.open_properties(target, cx));
                    }),
            )
            .item(
                PopupMenuItem::new(format!("コピー{batch_suffix}"))
                    .icon(Icon::new(IconName::ClipboardCopy))
                    .on_click(move |_, _window, cx| {
                        let _ = copy_sel_weak.update(cx, |this, cx| this.copy_selected(cx));
                    }),
            )
            .item(
                PopupMenuItem::new(format!("切り取り{batch_suffix}"))
                    .icon(Icon::new(IconName::ClipboardMinus))
                    .on_click(move |_, _window, cx| {
                        let _ = cut_sel_weak.update(cx, |this, cx| this.cut_selected(cx));
                    }),
            )
            .item(
                PopupMenuItem::new("貼り付け")
                    .icon(Icon::new(IconName::ClipboardPaste))
                    .on_click(move |_, _window, cx| {
                        let _ = paste_weak.update(cx, |this, cx| this.paste(cx));
                    }),
            )
            .item(
                PopupMenuItem::new("名前変更")
                    .icon(Icon::new(IconName::Pencil))
                    .on_click(move |_, _window, cx| {
                        let _ = rename_weak.update_in(cx, |this, window, cx| {
                            this.start_rename(window, cx);
                        });
                    }),
            )
            .separator()
            .item(
                PopupMenuItem::new(format!("ゴミ箱へ移動{batch_suffix}"))
                    .icon(Icon::new(IconName::Trash))
                    .on_click(move |_, _window, cx| {
                        let targets = trash_paths.clone();
                        let _ = trash_weak.update_in(cx, |this, window, cx| {
                            this.trash_paths(targets, window, cx);
                        });
                    }),
            )
            .separator()
            .item(
                PopupMenuItem::new("新規フォルダ")
                    .icon(Icon::new(IconName::FolderPlus))
                    .on_click(move |_, _window, cx| {
                        let _ = new_folder_weak.update(cx, |this, cx| this.new_folder(cx));
                    }),
            )
        }
    }

    /// Context menu for trash rows: restore only (no permanent deletion).
    fn trash_entry_menu(
        entity: Entity<Self>,
        batch_paths: Vec<PathBuf>,
    ) -> impl Fn(PopupMenu, &mut Window, &mut Context<PopupMenu>) -> PopupMenu + 'static {
        let weak = entity.downgrade();
        let count = batch_paths.len();
        let suffix = if count > 1 {
            format!("（{count} 件）")
        } else {
            String::new()
        };
        move |menu, _window, _menu_cx| {
            let restore_weak = weak.clone();
            let targets = batch_paths.clone();
            menu.item(
                PopupMenuItem::new(format!("元の場所へ復元{suffix}"))
                    .icon(Icon::new(IconName::ArchiveRestore))
                    .on_click(move |_, _window, cx| {
                        let targets = targets.clone();
                        let _ = restore_weak.update(cx, |this, cx| {
                            this.restore_paths(targets, cx);
                        });
                    }),
            )
        }
    }

    // ------------------------------------------------------------------- render

    fn sort_header_cell(
        &self,
        key: SortKey,
        width: Option<f32>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = match key {
            SortKey::Name => 30_001u64,
            SortKey::Kind => 30_004,
            SortKey::Size => 30_002,
            SortKey::Modified => 30_003,
        };
        let active = self.settings.sort.key == key;
        let arrow = if active {
            if self.settings.sort.dir.is_ascending() {
                "▲"
            } else {
                "▼"
            }
        } else {
            " "
        };
        let base = div()
            .id(id)
            .h_full()
            .flex()
            .items_center()
            .px_2()
            .cursor_pointer()
            .text_color(if active { rgb(C_TEXT) } else { rgb(C_MUTED) })
            .hover(|style| style.text_color(rgb(C_TEXT)));
        let base = match width {
            Some(width) => base.w(px(width)).text_right(),
            None => base.flex_1(),
        };
        base.on_click(cx.listener(move |this, _event, window, cx| {
            this.focus_handle.focus(window, cx);
            this.set_sort(key, cx);
        }))
        .child(format!("{arrow} {}", key.label()))
    }

    fn sort_header(&self, cx: &mut Context<Self>) -> Div {
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .h(px(26.0))
            .bg(rgb(C_MANTLE))
            .border_b_1()
            .border_color(rgb(C_SURFACE))
            .text_xs()
            .text_color(rgb(C_MUTED))
            .child(div().w(px(18.0)))
            .child(self.sort_header_cell(SortKey::Name, None, cx))
            .child(self.sort_header_cell(SortKey::Kind, Some(90.0), cx))
            .child(self.sort_header_cell(SortKey::Size, Some(90.0), cx))
            .child(self.sort_header_cell(SortKey::Modified, Some(150.0), cx))
    }

    fn overlay(&self) -> Option<Div> {
        let (message, hint) = empty_state_message(
            self.view,
            self.load.is_loading(),
            self.load.error_message(),
            self.all_entries.len(),
            self.entries.len(),
        )?;
        let color = if self.load.error_message().is_some() {
            C_RED
        } else {
            C_MUTED
        };
        Some(
            div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .flex()
                .items_center()
                .justify_center()
                .child(
                    div()
                        .max_w(px(640.0))
                        .px_4()
                        .flex()
                        .flex_col()
                        .items_center()
                        .gap_1()
                        .child(div().text_sm().text_color(rgb(color)).child(message))
                        .children(
                            hint.map(|hint| div().text_xs().text_color(rgb(C_MUTED)).child(hint)),
                        ),
                ),
        )
    }

    fn list_row(this: &mut Self, index: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let entry = this.entries.get(index).cloned()?;
        let display_name =
            display_name_for(&entry, this.view, &this.cwd, &this.search.active_roots);
        let display_name = match this.search_lines.get(&entry.path) {
            Some(line) if this.view == ViewSource::Search => format!("{display_name}:{line}"),
            _ => display_name,
        };
        let selected = this.selection.contains(&entry.path);
        let focused = this.selected == index;
        let is_dir = entry.is_dir;
        let (icon, icon_color) = if this.view == ViewSource::Trash {
            (IconName::Trash, C_MUTED)
        } else {
            (
                icon_for_entry(&entry),
                if entry.is_symlink {
                    C_MUTED
                } else {
                    kind_color(kind_for_entry(&entry))
                },
            )
        };

        let click = cx.listener(move |this, event: &ClickEvent, window, cx| {
            this.focus_handle.focus(window, cx);
            let modifiers = event.modifiers();
            this.select_with_modifiers(index, modifiers.control, modifiers.shift, cx);
            if event.click_count() >= 2 {
                if this.view == ViewSource::Trash {
                    this.restore_selected(cx);
                } else {
                    this.open_index(index, cx);
                }
            }
        });
        let right_click = cx.listener(move |this, _event: &MouseDownEvent, window, cx| {
            this.focus_handle.focus(window, cx);
            // Right-clicking inside an existing multi-selection keeps it, so
            // batch actions (copy/trash) can be applied to the whole selection.
            let already_selected = this
                .entries
                .get(index)
                .is_some_and(|entry| this.selection.contains(&entry.path));
            if already_selected {
                this.selected = index;
                cx.notify();
            } else {
                this.select_single_index(index, cx);
            }
        });
        let entity = cx.entity();
        let batch_paths = if this.selection.contains(&entry.path) {
            this.selection.paths().to_vec()
        } else {
            vec![entry.path.clone()]
        };
        let drag_paths = batch_paths.clone();
        let drag_out = this.view == ViewSource::Directory;
        let drop_target = if drag_out && is_dir && !entry.is_symlink {
            Some(entry.path.clone())
        } else {
            None
        };
        let context_menu: MenuBuilder = if this.view == ViewSource::Trash {
            Box::new(Self::trash_entry_menu(entity, batch_paths))
        } else {
            Box::new(Self::entry_menu(
                entity,
                entry.path.clone(),
                is_dir,
                batch_paths,
            ))
        };

        Some(
            div()
                .id(index)
                .w_full()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .h(px(28.0))
                .text_sm()
                .bg(if selected {
                    rgb(C_OVERLAY)
                } else if focused {
                    rgb(0x252537)
                } else {
                    rgb(C_BASE)
                })
                .cursor_pointer()
                .hover(|style| {
                    style.bg(if selected {
                        rgb(C_OVERLAY)
                    } else {
                        rgb(0x252537)
                    })
                })
                .on_click(click)
                .on_mouse_down(MouseButton::Right, right_click)
                .when(drag_out, |row| {
                    row.on_drag(drag_paths, |_paths, _, _, cx| cx.new(|_| Empty))
                        .external_drag_payload(|paths: &Vec<PathBuf>, _window, _cx| {
                            Some(ExternalDragPayload::Files(FileDragPaths::new(
                                paths.iter().map(|path| (path.clone(), path.is_dir())),
                            )))
                        })
                })
                .when_some(drop_target, |row, target| {
                    row.on_drop::<ExternalPaths>(cx.listener(
                        move |this, paths: &ExternalPaths, _window, cx| {
                            this.drop_external_into(target.clone(), paths.paths().to_vec(), cx);
                        },
                    ))
                })
                .context_menu(context_menu)
                .child(
                    div()
                        .w(px(18.0))
                        .flex()
                        .justify_center()
                        .child(Icon::new(icon).text_color(rgb(icon_color))),
                )
                .child(div().flex_1().truncate().child(display_name))
                .child(
                    div()
                        .w(px(90.0))
                        .text_xs()
                        .truncate()
                        .text_color(rgb(C_MUTED))
                        .child(if this.view == ViewSource::Trash {
                            "—".to_string()
                        } else {
                            model::kind_label(&entry)
                        }),
                )
                .child(
                    div()
                        .w(px(90.0))
                        .text_xs()
                        .text_right()
                        .text_color(rgb(C_MUTED))
                        .child(if entry.is_dir || this.view == ViewSource::Trash {
                            "-".to_string()
                        } else {
                            human_size(entry.size)
                        }),
                )
                .child(
                    div()
                        .w(px(150.0))
                        .text_xs()
                        .text_right()
                        .text_color(rgb(C_MUTED))
                        .child(human_datetime(entry.modified)),
                )
                .into_any_element(),
        )
    }

    /// One row of the icon grid: up to `columns` cells.
    fn grid_row(
        this: &mut Self,
        row_index: usize,
        columns: usize,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let start = row_index.checked_mul(columns)?;
        if start >= this.entries.len() {
            return None;
        }
        let end = (start + columns).min(this.entries.len());
        let mut cells = Vec::with_capacity(end - start);
        for index in start..end {
            if let Some(cell) = Self::grid_cell(this, index, cx) {
                cells.push(cell);
            }
        }
        if cells.is_empty() {
            return None;
        }
        Some(
            div()
                .flex()
                .w_full()
                .items_start()
                .gap_2()
                .px_3()
                .py_1()
                .children(cells)
                .into_any_element(),
        )
    }

    fn grid_cell(this: &mut Self, index: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let entry = this.entries.get(index).cloned()?;
        let display_name =
            display_name_for(&entry, this.view, &this.cwd, &this.search.active_roots);
        let display_name = match this.search_lines.get(&entry.path) {
            Some(line) if this.view == ViewSource::Search => format!("{display_name}:{line}"),
            _ => display_name,
        };
        let selected = this.selection.contains(&entry.path);
        let focused = this.selected == index;
        let is_dir = entry.is_dir;
        let (icon, icon_color) = if this.view == ViewSource::Trash {
            (IconName::Trash, C_MUTED)
        } else {
            (
                icon_for_entry(&entry),
                if entry.is_symlink {
                    C_MUTED
                } else {
                    kind_color(kind_for_entry(&entry))
                },
            )
        };
        let click = cx.listener(move |this, event: &ClickEvent, window, cx| {
            this.focus_handle.focus(window, cx);
            let modifiers = event.modifiers();
            this.select_with_modifiers(index, modifiers.control, modifiers.shift, cx);
            if event.click_count() >= 2 {
                if this.view == ViewSource::Trash {
                    this.restore_selected(cx);
                } else {
                    this.open_index(index, cx);
                }
            }
        });
        let right_click = cx.listener(move |this, _event: &MouseDownEvent, window, cx| {
            this.focus_handle.focus(window, cx);
            let already_selected = this
                .entries
                .get(index)
                .is_some_and(|entry| this.selection.contains(&entry.path));
            if already_selected {
                this.selected = index;
                cx.notify();
            } else {
                this.select_single_index(index, cx);
            }
        });
        let entity = cx.entity();
        let batch_paths = if this.selection.contains(&entry.path) {
            this.selection.paths().to_vec()
        } else {
            vec![entry.path.clone()]
        };
        let drag_paths = batch_paths.clone();
        let drag_out = this.view == ViewSource::Directory;
        let drop_target = if drag_out && is_dir && !entry.is_symlink {
            Some(entry.path.clone())
        } else {
            None
        };
        let context_menu: MenuBuilder = if this.view == ViewSource::Trash {
            Box::new(Self::trash_entry_menu(entity, batch_paths))
        } else {
            Box::new(Self::entry_menu(
                entity,
                entry.path.clone(),
                is_dir,
                batch_paths,
            ))
        };
        Some(
            div()
                .id(("grid-cell", index))
                .w(px(108.0))
                .h(px(104.0))
                .rounded_md()
                .px_1()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .gap_1()
                .bg(if selected {
                    rgb(C_OVERLAY)
                } else if focused {
                    rgb(0x252537)
                } else {
                    rgb(C_BASE)
                })
                .cursor_pointer()
                .hover(|style| {
                    style.bg(if selected {
                        rgb(C_OVERLAY)
                    } else {
                        rgb(0x252537)
                    })
                })
                .on_click(click)
                .on_mouse_down(MouseButton::Right, right_click)
                .when(drag_out, |cell| {
                    cell.on_drag(drag_paths, |_paths, _, _, cx| cx.new(|_| Empty))
                        .external_drag_payload(|paths: &Vec<PathBuf>, _window, _cx| {
                            Some(ExternalDragPayload::Files(FileDragPaths::new(
                                paths.iter().map(|path| (path.clone(), path.is_dir())),
                            )))
                        })
                })
                .when_some(drop_target, |cell, target| {
                    cell.on_drop::<ExternalPaths>(cx.listener(
                        move |this, paths: &ExternalPaths, _window, cx| {
                            this.drop_external_into(target.clone(), paths.paths().to_vec(), cx);
                        },
                    ))
                })
                .context_menu(context_menu)
                .child(
                    div()
                        .text_2xl()
                        .child(Icon::new(icon).text_color(rgb(icon_color))),
                )
                .child(
                    div()
                        .w_full()
                        .text_xs()
                        .text_center()
                        .truncate()
                        .child(display_name),
                )
                .into_any_element(),
        )
    }

    fn sidebar(&self, cx: &mut Context<Self>) -> Div {
        let mut sidebar = div()
            .w(px(190.0))
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(C_MANTLE))
            .border_r_1()
            .border_color(rgb(C_SURFACE))
            .py_2()
            .child(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(rgb(C_MUTED))
                    .child("場所"),
            );
        // Cached places only: no `dirs::*_dir()` / `is_dir()` I/O in render.
        for (index, place) in self.places.iter().enumerate() {
            let existing = place.exists;
            let is_trash = place.trash;
            let active = if is_trash {
                self.view == ViewSource::Trash
            } else {
                self.view == ViewSource::Directory
                    && place
                        .path
                        .as_ref()
                        .is_some_and(|target| target == &self.cwd)
            };
            let mut row = div()
                .id(20_000 + index as u64)
                .mx_2()
                .px_2()
                .h(px(30.0))
                .rounded_md()
                .flex()
                .items_center()
                .gap_2()
                .text_sm()
                .text_color(if existing { rgb(C_SUBTEXT) } else { rgb(C_DIM) })
                .when(active, |row| row.bg(rgb(C_SURFACE)).text_color(rgb(C_TEXT)))
                .child(Icon::new(place.icon))
                .child(place.label);
            if is_trash {
                row = row
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(C_SURFACE)))
                    .on_click(cx.listener(|this, _event, window, cx| {
                        this.focus_handle.focus(window, cx);
                        this.enter_trash(cx);
                    }));
            } else if let Some(target) = place.path.clone().filter(|_| existing) {
                row = row
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(C_SURFACE)))
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.focus_handle.focus(window, cx);
                        this.navigate_to(target.clone(), cx);
                    }));
            }
            sidebar = sidebar.child(row);
        }
        if !self.bookmarks.is_empty() {
            sidebar = sidebar.child(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(rgb(C_MUTED))
                    .child("ブックマーク"),
            );
            for (index, path) in self.bookmarks.iter().enumerate() {
                let active = self.view == ViewSource::Directory && path == &self.cwd;
                let label = tab_label(path);
                let target = path.clone();
                sidebar = sidebar.child(
                    div()
                        .id(32_000 + index as u64)
                        .mx_2()
                        .px_2()
                        .h(px(30.0))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_sm()
                        .text_color(rgb(C_SUBTEXT))
                        .when(active, |row| row.bg(rgb(C_SURFACE)).text_color(rgb(C_TEXT)))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(C_SURFACE)))
                        .on_click(cx.listener(move |this, _event, window, cx| {
                            this.focus_handle.focus(window, cx);
                            this.navigate_to(target.clone(), cx);
                        }))
                        .child(Icon::new(IconName::Star))
                        .child(div().min_w_0().truncate().child(label)),
                );
            }
        }
        let recent = &self.recent;
        if !recent.is_empty() {
            sidebar = sidebar.child(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(rgb(C_MUTED))
                    .child("最近"),
            );
            for (index, path) in recent.iter().enumerate() {
                let active = self.view == ViewSource::Directory && path == &self.cwd;
                let label = tab_label(path);
                let target = path.clone();
                sidebar = sidebar.child(
                    div()
                        .id(31_000 + index as u64)
                        .mx_2()
                        .px_2()
                        .h(px(30.0))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_sm()
                        .text_color(rgb(C_SUBTEXT))
                        .when(active, |row| row.bg(rgb(C_SURFACE)).text_color(rgb(C_TEXT)))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(C_SURFACE)))
                        .on_click(cx.listener(move |this, _event, window, cx| {
                            this.focus_handle.focus(window, cx);
                            this.navigate_to(target.clone(), cx);
                        }))
                        .child(Icon::new(IconName::Clock))
                        .child(div().min_w_0().truncate().child(label)),
                );
            }
        }
        if !self.mounts.is_empty() {
            sidebar = sidebar.child(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(rgb(C_MUTED))
                    .child("ドライブ"),
            );
            for (index, mount) in self.mounts.iter().enumerate() {
                let active = self.view == ViewSource::Directory && mount.path == self.cwd;
                let label = if mount.path == std::path::Path::new("/") {
                    "ルート (/)".to_string()
                } else {
                    mount.path.display().to_string()
                };
                let target = mount.path.clone();
                sidebar = sidebar.child(
                    div()
                        .id(30_000 + index as u64)
                        .mx_2()
                        .px_2()
                        .h(px(30.0))
                        .rounded_md()
                        .flex()
                        .items_center()
                        .gap_2()
                        .text_sm()
                        .text_color(rgb(C_SUBTEXT))
                        .when(active, |row| row.bg(rgb(C_SURFACE)).text_color(rgb(C_TEXT)))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(C_SURFACE)))
                        .on_click(cx.listener(move |this, _event, window, cx| {
                            this.focus_handle.focus(window, cx);
                            this.navigate_to(target.clone(), cx);
                        }))
                        .child(Icon::new(IconName::HardDrive))
                        .child(div().min_w_0().truncate().child(label)),
                );
            }
        }
        sidebar
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> Div {
        let mut breadcrumb = div()
            .flex()
            .items_center()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .gap_1()
            .mx_2();
        if self.view == ViewSource::Trash {
            breadcrumb = breadcrumb.child(
                div()
                    .px_1()
                    .py_0p5()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .child("ゴミ箱"),
            );
        }
        let segments = if self.view == ViewSource::Trash {
            Vec::new()
        } else {
            breadcrumb_segments(&self.cwd)
        };
        let last = segments.len().saturating_sub(1);
        for (index, (label, path)) in segments.into_iter().enumerate() {
            if index > 0 {
                breadcrumb = breadcrumb.child(div().text_xs().text_color(rgb(C_DIM)).child("›"));
            }
            let is_last = index == last;
            breadcrumb = breadcrumb.child(
                div()
                    .id(10_000 + index as u64)
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .text_sm()
                    .truncate()
                    .text_color(if is_last { rgb(C_TEXT) } else { rgb(C_SUBTEXT) })
                    .when(is_last, |cell| cell.font_weight(FontWeight::SEMIBOLD))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(C_SURFACE)))
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.focus_handle.focus(window, cx);
                        this.navigate_to(path.clone(), cx);
                    }))
                    .child(label),
            );
        }

        let back = icon_button(
            "nav-back",
            IconName::ArrowLeft,
            self.history.can_go_back() || self.view == ViewSource::Trash,
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.go_back(cx);
            }),
        );
        let forward = icon_button(
            "nav-forward",
            IconName::ArrowRight,
            self.history.can_go_forward() || self.view == ViewSource::Trash,
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.go_forward(cx);
            }),
        );
        let up = icon_button(
            "nav-up",
            IconName::ArrowUp,
            self.view == ViewSource::Trash || parent_dir(&self.cwd).is_some(),
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.go_parent(cx);
            }),
        );
        let refresh = icon_button(
            "refresh",
            IconName::RefreshCw,
            true,
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.reload(cx);
            }),
        );
        let undo_button = icon_button(
            "undo",
            IconName::Undo2,
            self.undo.can_undo(),
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.undo_last(cx);
            }),
        );
        let redo_button = icon_button(
            "redo",
            IconName::Redo2,
            self.undo.can_redo(),
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.redo_last(cx);
            }),
        );
        let view_details = icon_button_active(
            "view-details",
            IconName::List,
            true,
            self.settings.view == ViewLayout::Details,
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.set_view(ViewLayout::Details, cx);
            }),
        );
        let view_grid = icon_button_active(
            "view-grid",
            IconName::LayoutGrid,
            true,
            self.settings.view == ViewLayout::Icons,
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.set_view(ViewLayout::Icons, cx);
            }),
        );
        let new_folder = icon_button(
            "new-folder",
            IconName::FolderPlus,
            self.view == ViewSource::Directory,
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.new_folder(cx);
            }),
        );
        let hidden_icon = if self.settings.show_hidden {
            IconName::EyeOff
        } else {
            IconName::Eye
        };
        let toggle_hidden = icon_button(
            "toggle-hidden",
            hidden_icon,
            true,
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.toggle_hidden(cx);
            }),
        );

        div()
            .flex()
            .items_center()
            .gap_1()
            .px_3()
            .py_1()
            .bg(rgb(C_MANTLE))
            .border_b_1()
            .border_color(rgb(C_SURFACE))
            .child(back)
            .child(forward)
            .child(up)
            .child(refresh)
            .child(undo_button)
            .child(redo_button)
            .child(view_details)
            .child(view_grid)
            .child(breadcrumb)
            .child(new_folder)
            .child(toggle_hidden)
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> Div {
        let hidden_count = self.all_entries.len().saturating_sub(self.entries.len());
        let left = div()
            .flex()
            .items_center()
            .gap_2()
            .min_w_0()
            .child(
                div()
                    .max_w(px(520.0))
                    .truncate()
                    .text_color(rgb(C_SUBTEXT))
                    .child(if self.view == ViewSource::Trash {
                        "ゴミ箱".to_string()
                    } else {
                        display_path(&self.cwd)
                    }),
            )
            .child(div().text_color(rgb(C_DIM)).child("|"))
            .child(format!("{} 件", self.entries.len()))
            .child(
                div()
                    .text_color(rgb(C_MUTED))
                    .child(if self.selection.len() > 1 {
                        format!("{} 件選択", self.selection.len())
                    } else {
                        match self.selected_entry() {
                            Some(entry)
                                if !self.entries.is_empty() && self.view == ViewSource::Trash =>
                            {
                                format!(
                                    "選択: {}（削除: {}）",
                                    entry.name_text,
                                    human_datetime(entry.modified)
                                )
                            }
                            Some(entry) if !self.entries.is_empty() => {
                                format!("選択: {}", format_selected(entry))
                            }
                            _ => "選択なし".to_string(),
                        }
                    }),
            );
        let right = div()
            .flex()
            .items_center()
            .gap_2()
            .child(if let Some(error) = self.load.error_message() {
                div()
                    .text_color(rgb(C_RED))
                    .child(format!("エラー: {error}"))
                    .into_any_element()
            } else if self.load.is_loading() {
                div()
                    .text_color(rgb(C_YELLOW))
                    .child("読み込み中…")
                    .into_any_element()
            } else if !self.status.is_empty() {
                div()
                    .text_color(rgb(C_GREEN))
                    .child(self.status.clone())
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if let Some((done, total)) = self.op_progress {
                let fraction = if total == 0 {
                    0.0
                } else {
                    (done as f64 / total as f64).clamp(0.0, 1.0)
                };
                div()
                    .flex()
                    .items_center()
                    .gap_1()
                    .child(
                        div()
                            .w(px(120.0))
                            .h(px(5.0))
                            .rounded_full()
                            .bg(rgb(C_SURFACE))
                            .flex()
                            .child(
                                div()
                                    .h_full()
                                    .rounded_full()
                                    .bg(rgb(C_BLUE))
                                    .w(px(120.0 * fraction as f32)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(C_MUTED))
                            .child(format!("{:.0}%", fraction * 100.0)),
                    )
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if self.view == ViewSource::Search && hidden_count > 0 {
                // Search results were narrowed by the kind/extension filter.
                div()
                    .text_color(rgb(C_MUTED))
                    .child(format!("除外 {hidden_count} 件"))
                    .into_any_element()
            } else if hidden_count > 0 && !self.settings.show_hidden {
                div()
                    .text_color(rgb(C_MUTED))
                    .child(format!("隠し {hidden_count} 件"))
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if self.undo.undo_len() + self.undo.redo_len() == 0 {
                div().into_any_element()
            } else {
                div()
                    .id("undo-counter")
                    .text_color(rgb(C_MUTED))
                    .cursor_pointer()
                    .hover(|style| style.text_color(rgb(C_TEXT)))
                    .child(format!(
                        "元に戻す {} / やり直す {}",
                        self.undo.undo_len(),
                        self.undo.redo_len()
                    ))
                    .on_click(cx.listener(|this, _event: &ClickEvent, _window, cx| {
                        this.undo_history_open = true;
                        cx.notify();
                    }))
                    .into_any_element()
            })
            .child(if self.view == ViewSource::Trash {
                div()
                    .text_color(rgb(C_MUTED))
                    .child("ゴミ箱（自動更新なし）")
                    .into_any_element()
            } else if self.watcher.is_supported() {
                if let Some(reason) = self.watcher.last_error() {
                    div()
                        .text_color(rgb(C_YELLOW))
                        .child(format!("監視: 一部無効 — {reason}"))
                        .into_any_element()
                } else {
                    div()
                        .text_color(rgb(C_MUTED))
                        .child(format!(
                            "監視: {} ディレクトリ",
                            self.watcher.watched_paths().len()
                        ))
                        .into_any_element()
                }
            } else {
                let reason = self
                    .watcher
                    .last_error()
                    .unwrap_or("監視の初期化に失敗しました");
                div()
                    .text_color(rgb(C_YELLOW))
                    .child(format!("監視: 無効 — {reason}"))
                    .into_any_element()
            });
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .px_3()
            .h(px(26.0))
            .bg(rgb(C_MANTLE))
            .border_t_1()
            .border_color(rgb(C_SURFACE))
            .text_xs()
            .text_color(rgb(C_MUTED))
            .child(left)
            .child(right)
    }
}

/// Toolbar button: Lucide icon inside a hoverable square.
/// Toolbar button that shows a persistent active state (e.g. the current
/// list/grid layout).
fn icon_button_active(
    id: &'static str,
    icon: IconName,
    enabled: bool,
    active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(28.0))
        .h(px(28.0))
        .rounded_md()
        .when(active, |button| button.bg(rgb(C_SURFACE)))
        .text_color(if enabled { rgb(C_TEXT) } else { rgb(C_DIM) })
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|style| style.bg(rgb(C_SURFACE)))
                .on_click(on_click)
        })
        .child(Icon::new(icon))
}

fn icon_button(
    id: &'static str,
    icon: IconName,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    div()
        .id(id)
        .flex()
        .items_center()
        .justify_center()
        .w(px(28.0))
        .h(px(28.0))
        .rounded_md()
        .text_color(if enabled { rgb(C_TEXT) } else { rgb(C_DIM) })
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|style| style.bg(rgb(C_SURFACE)))
                .on_click(on_click)
        })
        .child(Icon::new(icon))
}

impl Explorer {
    /// Development hooks evaluated before the bar elements are built.
    ///
    /// Each hook clears its own flag, so it fires once per activation. Kept
    /// out of `render()` so the drawing path stays readable.
    fn apply_startup_hooks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Debug hooks can perform real filesystem operations (trash, drop
        // external files, rename). Never run them in release builds even if a
        // GPUI_FE_DEBUG_* variable leaks into the environment (F6-03).
        #[cfg(not(debug_assertions))]
        {
            let _ = (window, cx);
            return;
        }
        #[cfg(debug_assertions)]
        if self.debug.open_trash_once && self.view == ViewSource::Directory {
            self.debug.open_trash_once = false;
            self.enter_trash(cx);
        }
        if self.debug.grid_once {
            self.debug.grid_once = false;
            self.settings.view = ViewLayout::Icons;
        }
        if self.debug.progress_once && !self.entries.is_empty() {
            self.debug.progress_once = false;
            self.op_progress = Some((35_000_000, 100_000_000));
            self.suppress_for(Duration::from_secs(30));
            self.status = "コピー中… 35.0 MB / 100.0 MB（Esc でキャンセル）".to_string();
        }
        if self.debug.restore_trash_once
            && self.view == ViewSource::Trash
            && !self.load.is_loading()
            && !self.entries.is_empty()
        {
            self.debug.restore_trash_once = false;
            self.restore_selected(cx);
        }
        if self.debug.select_all_once && !self.entries.is_empty() && self.selection.len() <= 1 {
            self.debug.select_all_once = false;
            self.select_all(cx);
        }
        if self.debug.open_rename_once && !self.entries.is_empty() && self.renaming.is_none() {
            self.debug.open_rename_once = false;
            self.start_rename(window, cx);
        }
        if self.focus_list_on_next_render {
            self.focus_list_on_next_render = false;
            let handle = self.focus_handle.clone();
            window.on_next_frame(move |window, cx| handle.focus(window, cx));
        }
        if !self.load.is_loading() {
            if self.debug.properties_once && !self.load.is_loading() && !self.entries.is_empty() {
                self.debug.properties_once = false;
                let path = self.entries[0].path.clone();
                self.open_properties(path, cx);
                if self.debug.properties_size {
                    self.start_dir_size(cx);
                }
            }
            if let Some(path) = self.debug.drop_path.take() {
                let source = PathBuf::from(path);
                let target = self.debug.drop_target.take().and_then(|name| {
                    self.entries
                        .iter()
                        .find(|entry| entry.name_text == name && entry.is_dir)
                        .map(|entry| entry.path.clone())
                });
                match target {
                    Some(directory) => {
                        self.drop_external_into(directory, vec![source], cx);
                    }
                    None => self.drop_external(vec![source], cx),
                }
            }
        }
        if self.debug.tabs_once && self.view == ViewSource::Directory && !self.load.is_loading() {
            self.debug.tabs_once = false;
            self.sync_active_tab();
            if let Some(home) = dirs::home_dir() {
                self.tabs.new_tab(home);
            }
            self.tabs.new_tab(PathBuf::from("/"));
            let tab = self.tabs.active().clone();
            self.load_tab(tab, cx);
            if std::env::var("GPUI_FE_DEBUG_TAB_MOVE").is_ok_and(|value| value == "left") {
                self.move_tab_left(cx);
            }
        }
        if self.debug.search_once && self.view == ViewSource::Directory && !self.load.is_loading() {
            self.debug.search_once = false;
            let query =
                std::env::var("GPUI_FE_DEBUG_SEARCH_QUERY").unwrap_or_else(|_| "txt".to_string());
            self.open_search(window, cx);
            if let Ok(mode) = std::env::var("GPUI_FE_DEBUG_SEARCH_MODE") {
                if mode == "content" {
                    self.search.mode = SearchMode::Content;
                    self.search.kind = SearchKind::All;
                }
            }
            if let Ok(kind) = std::env::var("GPUI_FE_DEBUG_SEARCH_KIND") {
                self.search.kind = match kind.as_str() {
                    "files" => SearchKind::Files,
                    "dirs" => SearchKind::Dirs,
                    _ => SearchKind::All,
                };
            }
            if let Ok(scope) = std::env::var("GPUI_FE_DEBUG_SEARCH_SCOPE") {
                self.search.scope = match scope.as_str() {
                    "tabs" => SearchScope::OpenTabs,
                    "bookmarks" => SearchScope::Bookmarks,
                    _ => SearchScope::Current,
                };
            }
            if let Ok(extension) = std::env::var("GPUI_FE_DEBUG_SEARCH_EXT") {
                if let Some(ext) = self.search.ext.clone() {
                    ext.update(cx, |state, cx| {
                        state.set_value(extension.clone(), window, cx);
                    });
                }
            }
            if let Ok(name) = std::env::var("GPUI_FE_DEBUG_SEARCH_NAME") {
                if let Some(input) = self.search.name_filter.clone() {
                    input.update(cx, |state, cx| {
                        state.set_value(name.clone(), window, cx);
                    });
                }
            }
            if let Some(input) = self.search.input.clone() {
                input.update(cx, |state, cx| {
                    state.set_value(query.clone(), window, cx);
                });
            }
            self.run_search(cx);
        }
    }

    /// Development hooks evaluated after the bars and before the dialogs.
    fn apply_overlay_hooks(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        #[cfg(not(debug_assertions))]
        {
            let _ = (window, cx);
            return;
        }
        #[cfg(debug_assertions)]
        if self.debug.help_once {
            self.debug.help_once = false;
            self.help_open = true;
        }
        if self.debug.location_once && !self.load.is_loading() {
            self.debug.location_once = false;
            self.open_location(window, cx);
        }
        if self.debug.undo_history_once && !self.load.is_loading() {
            self.debug.undo_history_once = false;
            self.undo_history_open = true;
        }
        if self.debug.undo.is_some() && !self.load.is_loading() && !self.entries.is_empty() {
            let mode = self.debug.undo.take().unwrap_or_default();
            self.run_debug_undo(&mode, cx);
        }
        if self.debug.preview_once && !self.load.is_loading() && !self.entries.is_empty() {
            self.debug.preview_once = false;
            if let Some(entry) = self
                .entries
                .iter()
                .find(|entry| !entry.is_dir && !entry.is_symlink)
            {
                let path = entry.path.clone();
                let title = entry.name_text.clone();
                self.open_preview(path, title, cx);
            }
        }
    }
}

impl Render for Explorer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.apply_startup_hooks(window, cx);
        let tab_bar = self.tab_bar(cx);
        let search_bar = self.search_bar(cx);
        let search_history_bar = self.search_history_bar(cx);
        let location_bar = self.location_bar();
        self.apply_overlay_hooks(window, cx);
        let rename_dialog = self.rename_dialog(cx);
        let properties_dialog = self.properties_dialog(cx);
        let help_dialog = self.help_dialog(cx);
        let preview_dialog = self.preview_dialog(cx);
        let undo_history_dialog = self.undo_history_dialog(cx);

        let grid_mode = self.settings.view == ViewLayout::Icons;
        // Column headers only make sense in the details list.
        let header = if grid_mode {
            None
        } else {
            Some(self.sort_header(cx))
        };
        let columns = grid_columns(
            window.viewport_size().width.to_f64() as f32 - SIDEBAR_WIDTH_PX,
            GRID_CELL_WIDTH_PX,
        );
        let item_count = if grid_mode {
            grid_row_count(self.entries.len(), columns)
        } else {
            self.entries.len()
        };
        let list = uniform_list(
            "explorer-list",
            item_count,
            cx.processor(
                move |this: &mut Self, range: std::ops::Range<usize>, _window, cx| {
                    let mut rows = Vec::with_capacity(range.len());
                    for index in range {
                        let row = if grid_mode {
                            Self::grid_row(this, index, columns, cx)
                        } else {
                            Self::list_row(this, index, cx)
                        };
                        if let Some(row) = row {
                            rows.push(row);
                        }
                    }
                    rows
                },
            ),
        )
        .h_full()
        .track_scroll(&self.scroll_handle);

        let mut list_area = div()
            .relative()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(C_BASE))
            .child(list)
            .on_drop::<ExternalPaths>(cx.listener(|this, paths: &ExternalPaths, _window, cx| {
                this.drop_external(paths.paths().to_vec(), cx);
            }));
        if let Some(overlay) = self.overlay() {
            list_area = list_area.child(overlay);
        }

        let body = div()
            .flex_1()
            .min_h_0()
            .flex()
            .child(self.sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .children(header)
                    .child(list_area),
            );

        div()
            .id("explorer-root")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .relative()
            .size_full()
            .flex()
            .flex_col()
            .bg(rgb(C_BASE))
            .text_color(rgb(C_TEXT))
            .child(self.toolbar(cx))
            .children(tab_bar)
            .children(search_bar)
            .children(search_history_bar)
            .children(location_bar)
            .child(body)
            .child(self.status_bar(cx))
            .children(rename_dialog)
            .children(properties_dialog)
            .children(help_dialog)
            .children(preview_dialog)
            .children(undo_history_dialog)
    }
}

/// What the command line asks the process to do before starting GPUI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CliAction {
    Run,
    Version,
    Help,
}

/// Pure CLI classification, kept separate from `std::env` and stdout for tests.
fn classify_cli(args: &[String]) -> CliAction {
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        CliAction::Version
    } else if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        CliAction::Help
    } else {
        CliAction::Run
    }
}

/// Handle `--version` / `--help` before starting GPUI.
///
/// Returns `true` when the process should exit without opening a window.
fn handle_cli() -> bool {
    // args_os() avoids a panic on non-UTF-8 arguments (F6-06). Flag matching
    // is ASCII only, so lossy conversion is safe here; paths stay as OsString.
    let args: Vec<String> = std::env::args_os()
        .skip(1)
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    match classify_cli(&args) {
        CliAction::Version => {
            println!("gpui-file-explorer {}", env!("CARGO_PKG_VERSION"));
            true
        }
        CliAction::Help => {
            println!(
                "gpui-file-explorer {}\n\n\
                 使い方: gpui-file-explorer [ディレクトリ]\n\n\
                 引数なしの場合は前回のディレクトリ（無ければ $HOME）を開きます。\n\
                 主なキー: F2 名前変更 / Ctrl+C・Ctrl+X・Ctrl+V コピー・切り取り・貼り付け /\n\
                 Ctrl+H 隠しファイル / Ctrl+Shift+N 新規フォルダ / Alt+←→ 戻る・進む / Esc キャンセル\n\n\
                 ライセンス: AGPL-3.0-or-later",
                env!("CARGO_PKG_VERSION")
            );
            true
        }
        CliAction::Run => false,
    }
}

/// First non-flag argument, treated as the initial directory.
fn startup_dir() -> Option<PathBuf> {
    // Keep raw bytes: a path may contain non-UTF-8 bytes on Unix (F6-06).
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .find(|path| !path.as_os_str().to_string_lossy().starts_with('-'))
}

/// Restore the persisted tab set and choose the directory to open first.
///
/// Kept separate from `Explorer::new` so the precedence (saved tabs → CLI
/// argument → last directory → home → `/`) can be tested headlessly. The
/// existence checks are intentionally done by the caller before this point
/// for the CLI path (see `Explorer::new`), while persisted paths are filtered
/// here.
fn restore_session(
    saved: &[PathBuf],
    active: usize,
    last_dir: Option<&std::path::Path>,
    cli_dir: Option<PathBuf>,
    home: Option<PathBuf>,
) -> (Vec<PathBuf>, PathBuf, usize) {
    let cli_dir = cli_dir
        .map(|path| std::fs::canonicalize(&path).unwrap_or(path))
        .filter(|path| path.is_dir());
    let tabs: Vec<PathBuf> = saved.iter().filter(|path| path.is_dir()).cloned().collect();
    // An explicit CLI argument (`.desktop %f`, shell invocation) must win over
    // persisted tabs; otherwise passing a folder to the app silently opens a
    // different location (third cross-review F6-12).
    if let Some(cli) = cli_dir {
        let mut tabs = tabs;
        let position = tabs.iter().position(|path| path == &cli);
        let index = match position {
            Some(index) => index,
            None => {
                tabs.push(cli.clone());
                tabs.len() - 1
            }
        };
        return (tabs, cli, index);
    }
    if !tabs.is_empty() {
        let index = active.min(tabs.len() - 1);
        let start = tabs[index].clone();
        return (tabs, start, index);
    }
    let start = last_dir
        .filter(|path| path.is_dir())
        .map(PathBuf::from)
        .or(home)
        .unwrap_or_else(|| PathBuf::from("/"));
    (tabs, start, 0)
}

/// Install a compact structured-log subscriber. `RUST_LOG` selects the level
/// (defaults to `info`); failures are ignored so logging never blocks startup.
fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}

fn main() {
    if handle_cli() {
        return;
    }
    // Structured logs carry a level and timestamp; RUST_LOG controls verbosity
    // (e.g. RUST_LOG=debug). Initialization is best-effort and never blocks UI.
    init_logging();
    let app = gpui_kit::application().with_assets(gpui_kit::assets::AllAssets);
    app.run(|cx: &mut App| {
        gpui_kit::init(cx);
        // Dark component theme so context menus / prompts match the app.
        Theme::change(ThemeMode::Dark, None, cx);

        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        let bounds = Bounds::centered(None, size(px(1100.0), px(700.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some("GPUI File Explorer".into()),
                ..Default::default()
            }),
            focus: true,
            show: true,
            app_id: Some("gpui-file-explorer".to_string()),
            ..Default::default()
        };

        if let Err(error) = gpui_kit::open_window(options, cx, |window, cx| {
            let view = cx.new(Explorer::new);
            let focus_handle = view.read(cx).focus_handle.clone();
            focus_handle.focus(window, cx);
            view
        }) {
            // A missing display / compositor is a normal failure on headless
            // hosts; report it instead of panicking with a backtrace.
            tracing::error!(%error, "failed to open window");
            cx.quit();
            return;
        }

        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    // Explicit imports only: the crate-root `use gpui_kit::*` would shadow the
    // built-in `#[test]` attribute if pulled in via `use super::*`.
    use super::{
        build_places, classify_cli, format_selected, kind_color, summarize_op, tab_label,
        CliAction, Explorer,
    };
    use crate::icons::FileKind;
    use crate::model::Entry;
    use crate::ops::OpStats;
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::time::SystemTime;

    #[test]
    fn build_places_always_includes_trash_last() {
        let places = build_places();
        assert!(places.len() >= 2);
        let trash = places.last().unwrap();
        assert_eq!(trash.label, "ゴミ箱");
        assert!(trash.trash);
        assert!(trash.path.is_none());
        assert!(trash.exists);
        // Root is always present and its directory existence is checked.
        assert!(places
            .iter()
            .any(|place| place.label == "ルート" && place.exists));
    }

    #[test]
    fn kind_colors_are_distinct_per_file_kind() {
        use std::collections::HashSet;
        let kinds = [
            FileKind::Folder,
            FileKind::Image,
            FileKind::Code,
            FileKind::Archive,
            FileKind::Pdf,
            FileKind::Text,
            FileKind::Audio,
            FileKind::Video,
            FileKind::Unknown,
        ];
        let colors: HashSet<u32> = kinds.iter().map(|kind| kind_color(*kind)).collect();
        assert_eq!(colors.len(), kinds.len(), "each kind needs its own color");
    }

    #[test]
    fn tab_label_uses_last_component_or_root() {
        use std::path::Path;
        assert_eq!(tab_label(Path::new("/")), "ルート");
        assert_eq!(tab_label(Path::new("/home/koppe")), "koppe");
        assert_eq!(tab_label(Path::new("relative/dir")), "dir");
    }

    #[test]
    fn format_selected_covers_dirs_and_files() {
        let base = Entry {
            name: OsString::from("item"),
            name_text: "item".to_string(),
            sort_key: "item".to_string(),
            path: PathBuf::from("/tmp/item"),
            is_dir: false,
            is_symlink: false,
            size: 2048,
            modified: Some(SystemTime::UNIX_EPOCH),
        };
        let file = format_selected(&base);
        assert!(file.starts_with("item（"), "{file}");
        assert!(file.contains("2.0 K"), "{file}");

        let dir = Entry {
            is_dir: true,
            ..base
        };
        assert_eq!(format_selected(&dir), "item（フォルダ）");
    }

    #[test]
    fn summarize_op_reports_files_dirs_and_skips() {
        let stats = OpStats {
            files: 3,
            dirs: 1,
            bytes: 42,
            skipped: vec![PathBuf::from("/tmp/skip")],
            created: vec![PathBuf::from("/tmp/dest")],
        };
        let message = summarize_op("コピー", &stats);
        assert!(message.contains("コピーしました"), "{message}");
        assert!(message.contains("3 ファイル"), "{message}");
        assert!(message.contains("1 フォルダ"), "{message}");
        assert!(message.contains("1 件スキップ"), "{message}");
        assert!(message.ends_with(": dest"), "{message}");
    }

    #[test]
    fn help_sections_are_well_formed() {
        use crate::strings::HELP_SECTIONS;
        assert!(HELP_SECTIONS.len() >= 4);
        let mut total = 0usize;
        for (section, shortcuts) in HELP_SECTIONS {
            assert!(!section.is_empty());
            assert!(!shortcuts.is_empty(), "empty section {section}");
            for (key, description) in *shortcuts {
                assert!(!key.is_empty());
                assert!(!description.is_empty());
                total += 1;
            }
        }
        assert!(
            total >= 20,
            "expected a useful number of shortcuts, got {total}"
        );
    }

    #[test]
    fn empty_state_covers_all_views() {
        use super::empty_state_message;
        use super::ViewSource;

        let (title, hint) = empty_state_message(ViewSource::Directory, false, None, 0, 0).unwrap();
        assert_eq!(title, "このフォルダは空です");
        assert!(hint.unwrap().contains("ドロップ"));

        let (title, hint) = empty_state_message(ViewSource::Search, false, None, 0, 0).unwrap();
        assert_eq!(title, "一致する項目がありません");
        assert!(hint.unwrap().contains("検索語"));

        let (title, hint) = empty_state_message(ViewSource::Search, false, None, 5, 0).unwrap();
        assert_eq!(title, "絞り込みで全て除外されました");
        assert!(hint.unwrap().contains("拡張子"));

        let (title, hint) = empty_state_message(ViewSource::Trash, false, None, 0, 0).unwrap();
        assert_eq!(title, "ゴミ箱は空です");
        assert!(hint.unwrap().contains("完全削除"));

        let (title, _) = empty_state_message(ViewSource::Directory, false, None, 5, 0).unwrap();
        assert!(title.contains("隠しファイル 5 件"));

        let (title, _) = empty_state_message(ViewSource::Directory, true, None, 0, 0).unwrap();
        assert_eq!(title, "読み込み中…");

        let (title, _) =
            empty_state_message(ViewSource::Directory, false, Some("読めません"), 0, 0).unwrap();
        assert_eq!(title, "エラー: 読めません");

        assert!(empty_state_message(ViewSource::Directory, false, None, 3, 2).is_none());
    }

    #[test]
    fn summarize_op_handles_empty_stats() {
        let message = summarize_op("移動", &OpStats::default());
        assert_eq!(message, "移動しました");
    }

    #[test]
    fn display_name_for_search_prefers_matching_roots_then_full_path() {
        use super::{display_name_for, ViewSource};
        use crate::model::Entry;
        use std::ffi::OsString;

        let cwd = PathBuf::from("/work/project");
        let other_root = PathBuf::from("/data/shared");
        let roots = vec![cwd.clone(), other_root.clone()];

        let in_cwd = Entry {
            name: OsString::from("main.rs"),
            name_text: "main.rs".to_string(),
            sort_key: "main.rs".to_lowercase(),
            path: PathBuf::from("/work/project/src/main.rs"),
            is_dir: false,
            is_symlink: false,
            size: 1,
            modified: None,
        };
        assert_eq!(
            display_name_for(&in_cwd, ViewSource::Search, &cwd, &roots),
            "src/main.rs"
        );

        let in_other_root = Entry {
            name: OsString::from("notes.txt"),
            name_text: "notes.txt".to_string(),
            sort_key: "notes.txt".to_lowercase(),
            path: PathBuf::from("/data/shared/docs/notes.txt"),
            is_dir: false,
            is_symlink: false,
            size: 1,
            modified: None,
        };
        assert_eq!(
            display_name_for(&in_other_root, ViewSource::Search, &cwd, &roots),
            "docs/notes.txt"
        );

        let outside = Entry {
            name: OsString::from("outside.txt"),
            name_text: "outside.txt".to_string(),
            sort_key: "outside.txt".to_lowercase(),
            path: PathBuf::from("/elsewhere/outside.txt"),
            is_dir: false,
            is_symlink: false,
            size: 1,
            modified: None,
        };
        assert_eq!(
            display_name_for(&outside, ViewSource::Search, &cwd, &roots),
            "/elsewhere/outside.txt"
        );

        let nested_parent = PathBuf::from("/data");
        let nested_child = PathBuf::from("/data/shared");
        let nested_roots = vec![cwd.clone(), nested_parent, nested_child];
        assert_eq!(
            display_name_for(&in_other_root, ViewSource::Search, &cwd, &nested_roots),
            "docs/notes.txt"
        );

        assert_eq!(
            display_name_for(&in_other_root, ViewSource::Directory, &cwd, &roots),
            "notes.txt"
        );
    }

    #[test]
    fn per_root_cap_is_fair_across_roots() {
        assert_eq!(Explorer::per_root_cap(2000, 0), 2000);
        assert_eq!(Explorer::per_root_cap(2000, 1), 2000);
        assert_eq!(Explorer::per_root_cap(2000, 2), 1000);
        assert_eq!(Explorer::per_root_cap(2000, 4), 500);
        assert_eq!(Explorer::per_root_cap(2000, 10), 200);
        assert_eq!(Explorer::per_root_cap(2000, 100), 20);
    }

    #[test]
    fn load_state_is_exclusive_and_tracks_generation() {
        use super::LoadState;

        let loading = LoadState::Loading { generation: 7 };
        assert!(loading.is_loading());
        assert_eq!(loading.error_message(), None);
        assert_eq!(loading.generation(), Some(7));

        let error = LoadState::Error {
            generation: 8,
            error: "読み込み失敗".to_string(),
        };
        assert!(!error.is_loading());
        assert_eq!(error.error_message(), Some("読み込み失敗"));
        assert_eq!(error.generation(), Some(8));

        let ready = LoadState::Ready;
        assert!(!ready.is_loading());
        assert_eq!(ready.error_message(), None);
        assert_eq!(ready.generation(), None);
    }

    #[test]
    fn cli_classification_prefers_version_then_help() {
        let args = |values: &[&str]| values.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(classify_cli(&args(&[])), CliAction::Run);
        assert_eq!(classify_cli(&args(&["/tmp/dir"])), CliAction::Run);
        assert_eq!(classify_cli(&args(&["-h"])), CliAction::Help);
        assert_eq!(classify_cli(&args(&["--help"])), CliAction::Help);
        assert_eq!(classify_cli(&args(&["-V"])), CliAction::Version);
        assert_eq!(classify_cli(&args(&["--version"])), CliAction::Version);
        assert_eq!(
            classify_cli(&args(&["--help", "--version"])),
            CliAction::Version
        );
    }

    /// Normalize Windows verbatim paths (`\\?\C:\...`) for cross-platform
    /// comparisons in tests. No-op on Unix.
    fn canonical_for_compare(path: &std::path::Path) -> std::path::PathBuf {
        let text = path.to_string_lossy();
        let stripped = text.strip_prefix(r"\\?\").unwrap_or(&text);
        std::path::PathBuf::from(stripped)
    }

    #[test]
    fn restore_session_prefers_saved_tabs_then_cli_then_last_then_home() {
        use super::restore_session;
        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-session-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let saved = vec![dir.join("gone"), dir.clone()];
        let (tabs, start, index) = restore_session(&saved, 99, None, None, None);
        assert_eq!(tabs, vec![dir.clone()], "vanished paths are dropped");
        assert_eq!(start, dir, "active index is clamped to the saved tab");
        assert_eq!(index, 0);

        let (_, start, _) = restore_session(&[], 0, None, Some(dir.clone()), None);
        // On Windows canonicalize() adds a `\\?\` verbatim prefix; compare via
        // file_name-based identity rather than raw path bytes.
        assert_eq!(
            canonical_for_compare(&start),
            canonical_for_compare(&dir),
            "CLI argument wins when no tabs are saved"
        );

        // F6-12: an explicit CLI argument must beat persisted tabs.
        let cli = dir.join("cli-target");
        std::fs::create_dir_all(&cli).unwrap();
        let (tabs, start, index) =
            restore_session(std::slice::from_ref(&dir), 0, None, Some(cli.clone()), None);
        assert_eq!(canonical_for_compare(&start), canonical_for_compare(&cli));
        assert_eq!(index, tabs.len() - 1, "CLI tab must be active");
        assert!(tabs
            .iter()
            .any(|p| canonical_for_compare(p) == canonical_for_compare(&cli)));

        let home = dir.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let (_, start, _) = restore_session(&[], 0, Some(&home), None, Some(dir.clone()));
        assert_eq!(start, home, "last_dir wins over home");

        let (_, start, _) = restore_session(&[], 0, None, None, Some(dir.clone()));
        assert_eq!(start, dir, "home is the final fallback");

        let _ = std::fs::remove_dir_all(&dir);
    }
}
