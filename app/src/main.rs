// SPDX-License-Identifier: AGPL-3.0-or-later
//! GPUI file explorer prototype — Linux/Wayland first, cross-platform design.
//!
//! Stack: `gpui-kit` 0.9.1 (GPUI snapshot + GPUI Component + Lucide assets).
//! See `../README.md` for build/run instructions, key bindings and known
//! limitations, and `../PROGRESS.md` for the implementation log.

mod history;
mod icons;
mod model;
mod ops;
mod settings;
mod watcher;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_kit::component::{Icon, Theme, ThemeMode};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::history::History;
use crate::icons::{icon_for_entry, kind_for_entry, FileKind};
use crate::model::{
    breadcrumb_segments, clamp_index, create_unique_folder, display_path, fingerprint, first_index,
    human_age, human_datetime, human_size, last_index, parent_dir, read_entries, step_selection,
    visible_entries, Entry, Selection, SortKey, SortSpec,
};
use crate::ops::{CancelFlag, ConflictPolicy, OpError, OpStats};
use crate::settings::Settings;
use crate::watcher::DirWatcher;

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
        format!("{}（フォルダ）", entry.name)
    } else {
        format!(
            "{}（{} / {}）",
            entry.name,
            human_size(entry.size),
            human_age(entry.modified)
        )
    }
}

/// One-line result message for a completed copy/move operation.
fn summarize_op(verb: &str, stats: &OpStats) -> String {
    let mut parts: Vec<String> = Vec::new();
    if stats.files > 0 {
        parts.push(format!("{} ファイル", stats.files));
    }
    if stats.dirs > 0 {
        parts.push(format!("{} フォルダ", stats.dirs));
    }
    if !stats.skipped.is_empty() {
        parts.push(format!("{} 件スキップ", stats.skipped.len()));
    }
    let mut message = format!("{verb}しました");
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
    entries
        .into_iter()
        .map(|(label, icon, path)| {
            let exists = path.as_ref().is_some_and(|target| target.is_dir());
            Place {
                label,
                icon,
                path,
                exists,
            }
        })
        .collect()
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

/// One entry on the internal cut/copy clipboard (P1: single selection).
///
/// The OS clipboard is deliberately not used for file cut/copy: its cut/copy
/// semantics are platform-specific and it would overwrite whatever the user
/// had copied. Paste resolves the source path at operation time.
#[derive(Clone, Debug)]
struct ClipboardSlot {
    paths: Vec<PathBuf>,
    cut: bool,
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
    history: History,
    focus_handle: FocusHandle,
    scroll_handle: UniformListScrollHandle,
    loading: bool,
    error: Option<String>,
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
    /// Internal cut/copy clipboard (single entry in P1).
    clipboard: Option<ClipboardSlot>,
    /// Running paste operation; dropping the task cancels it.
    op_task: Option<Task<()>>,
    /// Cancellation flag of the running operation (Esc cancels).
    op_cancel: Option<Arc<CancelFlag>>,
    /// Inline rename dialog (F2 / context menu). `None` when closed.
    renaming: Option<RenameState>,
    /// Set after a successful rename; the next render refocuses the list.
    focus_list_on_next_render: bool,
    /// Development-only hook: `GPUI_FE_DEBUG_RENAME=1` opens the rename dialog
    /// for the first entry after the initial load (used for UI screenshots
    /// where synthetic keyboard input is unavailable).
    debug_open_rename_once: bool,
    /// Development-only hook: `GPUI_FE_DEBUG_SELECT_ALL=1` selects every entry
    /// after the first load (multi-selection screenshot).
    debug_select_all_once: bool,
}

impl Explorer {
    fn new(cx: &mut Context<Self>) -> Self {
        let settings = Settings::load();
        let start_dir = startup_dir()
            .map(|path| std::fs::canonicalize(&path).unwrap_or(path))
            .filter(|path| path.is_dir())
            .or_else(|| settings.last_dir.clone())
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("/"));

        let (watch_tx, watch_rx) = async_channel::unbounded::<()>();
        let watcher = DirWatcher::new(move || {
            let _ = watch_tx.try_send(());
        });

        let places = build_places();
        let mut this = Self {
            settings,
            cwd: start_dir,
            all_entries: Vec::new(),
            entries: Vec::new(),
            selected: 0,
            selection: Selection::default(),
            history: History::new(),
            focus_handle: cx.focus_handle(),
            scroll_handle: UniformListScrollHandle::new(),
            loading: true,
            error: None,
            status: String::new(),
            generation: 0,
            fingerprint: 0,
            pending_select: None,
            load_task: None,
            watcher,
            suppress_until: None,
            places,
            clipboard: None,
            op_task: None,
            op_cancel: None,
            renaming: None,
            focus_list_on_next_render: false,
            debug_open_rename_once: std::env::var_os("GPUI_FE_DEBUG_RENAME").is_some(),
            debug_select_all_once: std::env::var_os("GPUI_FE_DEBUG_SELECT_ALL").is_some(),
        };

        // Remember the directory we actually started in (also proves the
        // settings file round-trips on the very first launch).
        this.settings.last_dir = Some(this.cwd.clone());
        this.settings.save();

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
        self.generation = self.generation.wrapping_add(1);
        let generation = self.generation;
        self.loading = true;
        self.error = None;
        self.refresh_places();
        self.update_watcher();
        let dir = self.cwd.clone();
        // Drop the previous waiter: cancelling its Task discards the stale
        // result; the generation check is the second line of defence.
        self.load_task.take();
        let background = cx.background_spawn(async move { read_entries(&dir) });
        let task = cx.spawn(async move |this, cx| {
            let result = background.await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.loading = false;
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
                        this.error = Some(error);
                    }
                }
                cx.notify();
            });
        });
        self.load_task = Some(task);
        cx.notify();
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
        let dirs = self.watched_dirs();
        self.watcher.watch(&dirs);
    }

    /// Re-check whether the cached sidebar places exist. Called from `reload`
    /// only — never from `render`.
    fn refresh_places(&mut self) {
        for place in &mut self.places {
            place.exists = place.path.as_ref().is_some_and(|target| target.is_dir());
        }
    }

    /// 10-second reconciliation: re-read the directory in the background and
    /// reload only when the content fingerprint actually changed. This covers
    /// events lost by the OS watcher (queue overflow, unwatchable paths, ...).
    fn reconcile(&mut self, cx: &mut Context<Self>) {
        if self.loading {
            return;
        }
        let generation = self.generation;
        let dir = self.cwd.clone();
        let task = cx.background_spawn(async move { read_entries(&dir) });
        cx.spawn(async move |this, cx| {
            let Ok(entries) = task.await else { return };
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation || this.loading {
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
    /// coalesces bursts and refreshes the listing.
    fn spawn_watch_loop(&self, watch_rx: async_channel::Receiver<()>, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| {
            while watch_rx.recv().await.is_ok() {
                while watch_rx.try_recv().is_ok() {}
                let updated = this.update(cx, |this, cx| {
                    if this.is_suppressed() {
                        return;
                    }
                    this.status = "変更を検出しました".to_string();
                    this.reload(cx);
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

    // ------------------------------------------------------------- navigation

    fn set_cwd(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if path == self.cwd {
            self.reload(cx);
            return;
        }
        self.cwd = path;
        self.selected = 0;
        self.pending_select = None;
        self.error = None;
        self.status.clear();
        self.settings.last_dir = Some(self.cwd.clone());
        self.settings.save();
        self.reload(cx);
    }

    fn navigate_to(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if path == self.cwd {
            self.reload(cx);
            return;
        }
        if !path.is_dir() {
            self.error = Some(format!(
                "ディレクトリではありません: {}",
                display_path(&path)
            ));
            cx.notify();
            return;
        }
        self.history.visit(self.cwd.clone(), path.clone());
        self.set_cwd(path, cx);
    }

    fn go_back(&mut self, cx: &mut Context<Self>) {
        if let Some(target) = self.history.go_back(self.cwd.clone()) {
            if target.is_dir() {
                self.set_cwd(target, cx);
            } else {
                self.status = format!("移動先が存在しません: {}", display_path(&target));
                cx.notify();
            }
        }
    }

    fn go_forward(&mut self, cx: &mut Context<Self>) {
        if let Some(target) = self.history.go_forward(self.cwd.clone()) {
            if target.is_dir() {
                self.set_cwd(target, cx);
            } else {
                self.status = format!("移動先が存在しません: {}", display_path(&target));
                cx.notify();
            }
        }
    }

    fn go_parent(&mut self, cx: &mut Context<Self>) {
        if let Some(parent) = parent_dir(&self.cwd) {
            if parent != self.cwd {
                self.navigate_to(parent, cx);
            }
        }
    }

    fn open_selected(&mut self, cx: &mut Context<Self>) {
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
            self.status = format!("対象が存在しません: {}", display_path(&path));
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
        let keystroke = &event.keystroke;
        let key = keystroke.key.as_str();
        let modifiers = &keystroke.modifiers;
        if modifiers.control {
            match (key, modifiers.shift) {
                ("h", _) => self.toggle_hidden(cx),
                ("a", false) => self.select_all(cx),
                ("r", _) => self.reload(cx),
                ("n", true) => self.new_folder(cx),
                ("c", false) => self.copy_selected(cx),
                ("x", false) => self.cut_selected(cx),
                ("v", false) => self.paste(cx),
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
            ((viewport_height - 140.0) / 28.0).floor().max(1.0) as isize
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
            "f2" => self.start_rename(window, cx),
            "delete" => self.trash_selected(window, cx),
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
        self.status = "フォルダを作成中…".to_string();
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
                        this.error = Some(format!("フォルダを作成できません: {error}"));
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
        self.status = "キャンセルしています…".to_string();
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
            self.status = "コピー/切り取りした項目がありません".to_string();
            cx.notify();
            return;
        };
        if self.op_task.is_some() {
            self.status = "ファイル操作を実行中です（Esc でキャンセル）".to_string();
            cx.notify();
            return;
        }

        let cwd = self.cwd.clone();
        let cut = slot.cut;
        let verb = if cut { "移動" } else { "コピー" };

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
        if !missing.is_empty() {
            self.clipboard = None;
            self.status = format!("{} 件は既に存在しません", missing.len());
            self.reload(cx);
            return;
        }
        if paths.is_empty() {
            self.status = if same_dir > 0 {
                "同じフォルダのため移動しません".to_string()
            } else {
                "コピー/切り取りした項目がありません".to_string()
            };
            cx.notify();
            return;
        }

        self.status = format!("{verb}中…（Esc でキャンセル）");
        cx.notify();

        let cancel = Arc::new(CancelFlag::new());
        self.op_cancel = Some(cancel.clone());

        // Byte progress from the background copy/move is forwarded through a
        // channel and throttled to ~80 ms so the UI is not flooded with
        // notifications. The pump stops when the operation drops its sender.
        let (progress_tx, progress_rx) = async_channel::unbounded::<u64>();
        let progress_verb = verb;
        cx.spawn(async move |this, cx| {
            let mut last_report = Instant::now();
            let mut last_bytes = 0u64;
            while let Ok(bytes) = progress_rx.recv().await {
                if bytes <= last_bytes || last_report.elapsed() < Duration::from_millis(80) {
                    continue;
                }
                last_report = Instant::now();
                last_bytes = bytes;
                let _ = this.update(cx, |this, cx| {
                    // Ignore progress that arrives after the operation settled
                    // (the final status message must not be overwritten).
                    if this.op_cancel.is_none() {
                        return;
                    }
                    this.status = format!(
                        "{progress_verb}中… {}（Esc でキャンセル）",
                        human_size(bytes)
                    );
                    cx.notify();
                });
            }
        })
        .detach();

        let task_cancel = cancel.clone();
        let cwd_for_task = cwd.clone();
        let progress_tx_for_task = progress_tx.clone();
        let background = cx.background_spawn(async move {
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
                    let _ = tx.try_send(base + bytes);
                };
                let result = if cut {
                    ops::move_entry(
                        &path,
                        &cwd_for_task,
                        ConflictPolicy::Rename,
                        &task_cancel,
                        &mut progress,
                    )
                } else {
                    ops::copy_entry(
                        &path,
                        &cwd_for_task,
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
                let succeeded = stats.files + stats.dirs > 0;
                let status = match first_error {
                    Some(OpError::Cancelled) => "キャンセルしました".to_string(),
                    Some(error) if succeeded => {
                        format!("{verb}しました（一部失敗: {error}）")
                    }
                    Some(error) => format!("{verb}できません: {error}"),
                    None => summarize_op(verb, &stats),
                };
                this.suppress_for(SELF_OP_SUPPRESS);
                if cut {
                    // Moved sources are gone; never keep a stale cut clipboard.
                    this.clipboard = None;
                }
                if let Some(dest) = first_created {
                    if dest.parent() == Some(this.cwd.as_path()) {
                        this.pending_select = Some(dest);
                    }
                }
                this.status = status;
                this.reload(cx);
                cx.notify();
            });
        });
        self.op_task = Some(task);
        cx.notify();
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
        let original = entry.name.clone();
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

        let background = cx.background_spawn(async move { ops::rename_entry(&target, &value) });
        cx.spawn(async move |this, cx| {
            let result = background.await;
            let _ = this.update(cx, |this, cx| {
                match result {
                    Ok(new_path) => {
                        this.suppress_for(SELF_OP_SUPPRESS);
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
            self.status = format!("対象が存在しません: {}", display_path(&path));
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
                    this.error = Some(format!(
                        "ゴミ箱へ移動できません: {}",
                        failures
                            .first()
                            .map(|(_, message)| message.clone())
                            .unwrap_or_default()
                    ));
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
                    if !failures.is_empty() {
                        message.push_str(&format!("（{} 件失敗）", failures.len()));
                    }
                    this.status = message;
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
            let copy_sel_weak = weak.clone();
            let cut_sel_weak = weak.clone();
            let paste_weak = weak.clone();
            let rename_weak = weak.clone();
            let trash_weak = weak.clone();
            let new_folder_weak = weak.clone();

            let open_path = path.clone();
            let reveal_path = path.clone();
            let copy_path = path.clone();
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

    // ------------------------------------------------------------------- render

    fn sort_header_cell(
        &self,
        key: SortKey,
        width: Option<f32>,
        cx: &mut Context<Self>,
    ) -> Stateful<Div> {
        let id = match key {
            SortKey::Name => 30_001u64,
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
            .child(self.sort_header_cell(SortKey::Size, Some(90.0), cx))
            .child(self.sort_header_cell(SortKey::Modified, Some(150.0), cx))
    }

    fn overlay(&self) -> Option<Div> {
        let (message, color) = if let Some(error) = &self.error {
            (format!("エラー: {error}"), C_RED)
        } else if self.loading {
            ("読み込み中…".to_string(), C_MUTED)
        } else if self.all_entries.is_empty() {
            ("このフォルダは空です".to_string(), C_MUTED)
        } else if self.entries.is_empty() {
            (
                format!(
                    "隠しファイル {} 件を非表示にしています（Ctrl+H で表示）",
                    self.all_entries.len()
                ),
                C_MUTED,
            )
        } else {
            return None;
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
                        .text_sm()
                        .text_color(rgb(color))
                        .child(message),
                ),
        )
    }

    fn list_row(this: &mut Self, index: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let entry = this.entries.get(index).cloned()?;
        let selected = this.selection.contains(&entry.path);
        let focused = this.selected == index;
        let is_dir = entry.is_dir;
        let icon = icon_for_entry(&entry);
        let icon_color = if entry.is_symlink {
            C_MUTED
        } else {
            kind_color(kind_for_entry(&entry))
        };

        let click = cx.listener(move |this, event: &ClickEvent, window, cx| {
            this.focus_handle.focus(window, cx);
            let modifiers = event.modifiers();
            this.select_with_modifiers(index, modifiers.control, modifiers.shift, cx);
            if event.click_count() >= 2 {
                this.open_index(index, cx);
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
        let context_menu = Self::entry_menu(entity, entry.path.clone(), is_dir, batch_paths);

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
                .context_menu(context_menu)
                .child(
                    div()
                        .w(px(18.0))
                        .flex()
                        .justify_center()
                        .child(Icon::new(icon).text_color(rgb(icon_color))),
                )
                .child(div().flex_1().truncate().child(entry.name.clone()))
                .child(
                    div()
                        .w(px(90.0))
                        .text_xs()
                        .text_right()
                        .text_color(rgb(C_MUTED))
                        .child(if entry.is_dir {
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
            let active = place
                .path
                .as_ref()
                .is_some_and(|target| target == &self.cwd);
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
            if let Some(target) = place.path.clone().filter(|_| existing) {
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
        let segments = breadcrumb_segments(&self.cwd);
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
            self.history.can_go_back(),
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.go_back(cx);
            }),
        );
        let forward = icon_button(
            "nav-forward",
            IconName::ArrowRight,
            self.history.can_go_forward(),
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.go_forward(cx);
            }),
        );
        let up = icon_button(
            "nav-up",
            IconName::ArrowUp,
            parent_dir(&self.cwd).is_some(),
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
        let new_folder = icon_button(
            "new-folder",
            IconName::FolderPlus,
            true,
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
            .child(breadcrumb)
            .child(new_folder)
            .child(toggle_hidden)
    }

    fn status_bar(&self) -> Div {
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
                    .child(display_path(&self.cwd)),
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
            .child(if let Some(error) = &self.error {
                div()
                    .text_color(rgb(C_RED))
                    .child(format!("エラー: {error}"))
                    .into_any_element()
            } else if self.loading {
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
            .child(if hidden_count > 0 && !self.settings.show_hidden {
                div()
                    .text_color(rgb(C_MUTED))
                    .child(format!("隠し {hidden_count} 件"))
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(if self.watcher.is_supported() {
                div()
                    .text_color(rgb(C_MUTED))
                    .child(format!(
                        "監視: {} ディレクトリ",
                        self.watcher.watched_paths().len()
                    ))
                    .into_any_element()
            } else {
                div()
                    .text_color(rgb(C_YELLOW))
                    .child("監視: 無効")
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

impl Render for Explorer {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.debug_select_all_once && !self.entries.is_empty() && self.selection.len() <= 1 {
            self.debug_select_all_once = false;
            self.select_all(cx);
        }
        if self.debug_open_rename_once && !self.entries.is_empty() && self.renaming.is_none() {
            self.debug_open_rename_once = false;
            self.start_rename(window, cx);
        }
        if self.focus_list_on_next_render {
            self.focus_list_on_next_render = false;
            let handle = self.focus_handle.clone();
            window.on_next_frame(move |window, cx| handle.focus(window, cx));
        }
        let header = self.sort_header(cx);
        let rename_dialog = self.rename_dialog(cx);

        let list = uniform_list(
            "explorer-list",
            self.entries.len(),
            cx.processor(
                |this: &mut Self, range: std::ops::Range<usize>, _window, cx| {
                    let mut rows = Vec::with_capacity(range.len());
                    for index in range {
                        if let Some(row) = Self::list_row(this, index, cx) {
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
            .child(list);
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
                    .child(header)
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
            .child(body)
            .child(self.status_bar())
            .children(rename_dialog)
    }
}

/// Handle `--version` / `--help` before starting GPUI.
///
/// Returns `true` when the process should exit without opening a window.
fn handle_cli() -> bool {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|arg| arg == "--version" || arg == "-V") {
        println!("gpui-file-explorer {}", env!("CARGO_PKG_VERSION"));
        return true;
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!(
            "gpui-file-explorer {}\n\n\
             使い方: gpui-file-explorer [ディレクトリ]\n\n\
             引数なしの場合は前回のディレクトリ（無ければ $HOME）を開きます。\n\
             主なキー: F2 名前変更 / Ctrl+C・Ctrl+X・Ctrl+V コピー・切り取り・貼り付け /\n\
             Ctrl+H 隠しファイル / Ctrl+Shift+N 新規フォルダ / Alt+←→ 戻る・進む / Esc キャンセル\n\n\
             ライセンス: AGPL-3.0-or-later",
            env!("CARGO_PKG_VERSION")
        );
        return true;
    }
    false
}

/// First non-flag argument, treated as the initial directory.
fn startup_dir() -> Option<PathBuf> {
    std::env::args()
        .skip(1)
        .find(|arg| !arg.starts_with('-'))
        .map(PathBuf::from)
}

fn main() {
    if handle_cli() {
        return;
    }
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

        gpui_kit::open_window(options, cx, |window, cx| {
            let view = cx.new(Explorer::new);
            let focus_handle = view.read(cx).focus_handle.clone();
            focus_handle.focus(window, cx);
            view
        })
        .expect("failed to open window");

        cx.activate(true);
    });
}

#[cfg(test)]
mod tests {
    // Explicit imports only: the crate-root `use gpui_kit::*` would shadow the
    // built-in `#[test]` attribute if pulled in via `use super::*`.
    use super::summarize_op;
    use crate::ops::OpStats;
    use std::path::PathBuf;

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
    fn summarize_op_handles_empty_stats() {
        let message = summarize_op("移動", &OpStats::default());
        assert_eq!(message, "移動しました");
    }
}
