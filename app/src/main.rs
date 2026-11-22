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
mod settings_ui;
mod strings;
mod system_icons;
mod tabs;
mod trash_view;
mod undo;
mod watcher;

use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tracing::Level;
use tracing_subscriber::EnvFilter;

use gpui_kit::assets::IconName;
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::menu::{ContextMenuExt, PopupMenu, PopupMenuItem};
use gpui_kit::component::tooltip::Tooltip;
use gpui_kit::component::{Icon, Theme, ThemeMode};
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::history::{History, NavMode};
use crate::icons::{icon_for_entry, kind_for_entry, FileKind};
use crate::model::{
    breadcrumb_segments, clamp_index, create_unique_folder, display_path, fingerprint, first_index,
    grid_columns, grid_row_count, human_age, human_datetime, human_size, last_index, parent_dir,
    read_entries, resolve_location_input, sort_entries, step_selection, visible_entries, Entry,
    FsError, FsErrorKind, Selection, SortKey, SortSpec,
};
use crate::mounts::MountPoint;
use crate::ops::{CancelFlag, ConflictPolicy, OpError, OpStats};
use crate::search::{
    filter_results, merge_search_results, search_by_name, search_contents, search_roots,
    ContentSearchOptions, SearchKind, SearchMode, SearchOptions, SearchScope,
};
use crate::settings::{
    next_theme_choice, resolve_theme, Settings, SettingsSaveError, TabSession, ThemeChoice,
    ViewLayout, WindowBoundsState, MAX_CHORD_BYTES, MAX_KEYMAP_OVERRIDES, MAX_TABS,
};
use crate::system_icons::SystemIconResolver;
use crate::tabs::{Tab, Tabs};
use crate::undo::{UndoAction, UndoStack};
use crate::watcher::{removed_paths, DirWatcher, FsEvent};

// Catppuccin Mocha values: kept as the dark defaults because pure unit tests
// (and a few self-free helpers) still reference these constants. Rendered views
// use `Explorer::palette`, which selects the Mocha or Latte set from the
// effective theme.
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

/// Every hand-drawn color in the explorer, for the active light/dark theme.
///
/// GPUI Component themes its own widgets, but the file explorer paints its
/// list, sidebar, toolbar and dialogs directly. Keeping both Catppuccin
/// flavors in one `Copy` value lets a theme change refresh every surface with
/// a single field assignment (see [`Explorer::apply_theme_mode`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct AppPalette {
    base: u32,
    mantle: u32,
    surface: u32,
    overlay: u32,
    text: u32,
    subtext: u32,
    muted: u32,
    dim: u32,
    blue: u32,
    green: u32,
    yellow: u32,
    red: u32,
    mauve: u32,
    teal: u32,
    lavender: u32,
    /// Keyboard-focus row tint (a darker/lighter step off `base`).
    focus: u32,
}

impl AppPalette {
    /// Catppuccin Mocha (the historical application palette).
    const fn dark() -> Self {
        Self {
            base: C_BASE,
            mantle: C_MANTLE,
            surface: C_SURFACE,
            overlay: C_OVERLAY,
            text: C_TEXT,
            subtext: C_SUBTEXT,
            muted: C_MUTED,
            dim: C_DIM,
            blue: C_BLUE,
            green: C_GREEN,
            yellow: C_YELLOW,
            red: C_RED,
            mauve: C_MAUVE,
            teal: C_TEAL,
            lavender: C_LAVENDER,
            focus: 0x252537,
        }
    }

    /// Catppuccin Latte. `dim` maps to Latte `surface2`, the same rung Mocha
    /// uses for its de-emphasized text.
    const fn light() -> Self {
        Self {
            base: 0xeff1f5,
            mantle: 0xe6e9ef,
            surface: 0xccd0da,
            overlay: 0xbcc0cc,
            text: 0x4c4f69,
            subtext: 0x5c5f77,
            muted: 0x6c6f85,
            dim: 0xacb0be,
            blue: 0x1e66f5,
            green: 0x40a02b,
            yellow: 0xdf8e1d,
            red: 0xd20f39,
            mauve: 0x8839ef,
            teal: 0x179299,
            lavender: 0x7287fd,
            focus: 0xdad9e9,
        }
    }

    fn for_mode(mode: ThemeMode) -> Self {
        if mode.is_dark() {
            Self::dark()
        } else {
            Self::light()
        }
    }

    /// Re-map the pure `status_color` classifier's Mocha constants onto the
    /// active palette so warnings/errors stay legible on the light background.
    fn status_color(&self, status: &str) -> u32 {
        match status_color(status) {
            C_YELLOW => self.yellow,
            C_RED => self.red,
            _ => self.green,
        }
    }

    /// Icon tint for a file kind, from the active palette. The standalone
    /// `kind_color` stays Mocha-based for its pure unit tests; this method is
    /// what the rendered rows and cells use.
    fn kind_color(&self, kind: FileKind) -> u32 {
        match kind {
            FileKind::Folder => self.blue,
            FileKind::Symlink => self.muted,
            FileKind::Image => self.green,
            FileKind::Code => self.lavender,
            FileKind::Archive => self.yellow,
            FileKind::Pdf => self.red,
            FileKind::Text => self.subtext,
            FileKind::Audio => self.mauve,
            FileKind::Video => self.teal,
            FileKind::Unknown => self.muted,
        }
    }
}

const RECONCILE_INTERVAL: Duration = Duration::from_secs(10);
const SELF_OP_SUPPRESS: Duration = Duration::from_millis(1500);
/// Throttle progress-bar updates so a fast copy does not flood the UI.
const PROGRESS_THROTTLE: Duration = Duration::from_millis(80);

/// Layout metrics shared by the list page-step and the icon grid. Keeping
/// them named avoids drift between the row height and the grid cell height.
const ROW_HEIGHT_PX: f64 = 28.0;
const LIST_CHROME_HEIGHT_PX: f64 = 140.0;
/// Single source of truth for the sidebar width: used both when rendering the
/// sidebar and when subtracting it from the viewport for the icon grid.
const SIDEBAR_WIDTH_PX: f32 = 190.0;
const GRID_CELL_WIDTH_PX: f32 = 108.0;
/// `grid_row` adds `.px_3()` (24px total horizontal padding) and `.gap_2()`
/// (8px between cells). The column calculation must budget for both or the
/// last column overflows the viewport (U-17).
const GRID_ROW_PADDING_PX: f32 = 24.0;
const GRID_GAP_PX: f32 = 8.0;

/// Icon-grid column count for the width left after the sidebar, folding the
/// row padding and inter-cell gaps into `grid_columns`.
fn grid_columns_for_width(content_width: f32) -> usize {
    grid_columns(
        content_width - GRID_ROW_PADDING_PX - GRID_GAP_PX,
        GRID_CELL_WIDTH_PX + GRID_GAP_PX,
    )
}

/// Presence check that also sees dangling symlinks.
///
/// `Path::exists` follows the link target, so a broken symlink looks absent
/// even though the directory entry is still there. Undo and reveal must treat
/// such a link as present; the check mirrors `copy_entry` / `move_entry`.
fn path_present(path: &Path) -> bool {
    path.exists() || path.is_symlink()
}

/// Status shown after a search-filter refresh (kind/extension/name/sort).
///
/// The watcher may have flagged the snapshot as stale
/// ([`STATUS_SEARCH_STALE`]); a mere filter tweak must not bury that
/// "再検索が必要です" notice under a plain result count. When the summary is
/// already that same sentinel (the Rescan/Renamed watch branch composes it),
/// keep it as-is instead of duplicating the warning.
fn search_filter_status(current: &str, summary: String) -> String {
    if current.starts_with(STATUS_SEARCH_STALE) && !summary.starts_with(STATUS_SEARCH_STALE) {
        format!("{STATUS_SEARCH_STALE}・{summary}")
    } else {
        summary
    }
}

/// Status for directory watch events that would report `STATUS_CHANGED` (Z02).
///
/// F5 (`Reload`) reaches the keymap even in the Search view, where `reload()`
/// intentionally does nothing to preserve the search listing. In that view we
/// re-run the current query instead, so the advertised recovery key works
/// (AC10). Every other view uses the normal directory reload.
fn reload_reruns_search(view: ViewSource) -> bool {
    matches!(view, ViewSource::Search)
}

/// A failed reconciliation sets `STATUS_RESCAN_FAILED` with its F5 recovery
/// hint and must stay visible until the next *successful* read (strings.rs
/// F6-36). The watch sites all route their "changed" notice through this helper
/// so a bare watch event cannot bury the failure under a green status.
/// `STATUS_SEARCH_STALE` is the Search-view form of the same watch notice (it
/// starts with `STATUS_CHANGED`), so it must be guarded too: otherwise the
/// Rescan/Renamed branch in the Search watch loop would overwrite a held
/// failure (AC01).
/// Any other `next` status (a success path, or a different failure) passes
/// through unchanged, so the guard does not pin the status permanently.
fn reconcile_aware_status(current: &str, next: &str) -> String {
    if current.starts_with(STATUS_RESCAN_FAILED)
        && (next == STATUS_CHANGED || next == STATUS_SEARCH_STALE)
    {
        current.to_string()
    } else {
        next.to_string()
    }
}

/// The successful half of the F6-36 "keep RESCAN_FAILED until the next
/// successful read" contract: once a fresh read lands, clear the stale failure
/// notice so the bar is not stuck red forever (Z14). Other statuses are left
/// untouched.
fn clear_rescan_failure_status(current: &str) -> Option<String> {
    current.starts_with(STATUS_RESCAN_FAILED).then(String::new)
}

/// End the F6-36 RESCAN_FAILED hold after a successful directory read.
///
/// Both fresh-read paths go through here — the F5 `reload` success branch and
/// the 10s `reconcile` success branch (including the no-fingerprint-change
/// case) — so the two cannot drift apart. Only a held failure notice is
/// replaced; every other status passes through untouched. Returns whether a
/// notice was actually cleared, which lets callers/tests observe the change.
fn clear_rescan_failure_after_successful_read(status: &mut String) -> bool {
    if let Some(cleared) = clear_rescan_failure_status(status) {
        *status = cleared;
        true
    } else {
        false
    }
}

/// Keys forwarded to the normal keymap while the search view is open and no
/// search text input owns focus. Text editing keys stay with the query field;
/// once focus moves to the result list, navigation, copy, F4 (terminal) and
/// Ctrl+Q (quit) reach the existing handlers. Alt variants keep their prior
/// (blocked) behaviour. Ctrl+C / Ctrl+Q are unshifted only: Ctrl+Shift+C maps
/// to `CopyPath` (not an allowed Search action), so the hardcoded OR must not
/// forward it regardless of shift (AC02).
fn search_view_key_allowed(key: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
    if alt {
        return false;
    }
    matches!(
        key,
        "up" | "down" | "pageup" | "pagedown" | "home" | "end" | "enter" | "f4"
    ) || (ctrl && !shift && matches!(key, "c" | "q"))
}

/// The actions the Search-view allowlist above is meant to reach. Checking the
/// resolved action (not just the shipped keystroke) means a rebind of one of
/// them still fires while browsing results (finding 4). OpenSettings belongs
/// here too: Ctrl+, must open preferences even while browsing results (L04).
/// Destructive actions such as trash/restore stay blocked because they are not
/// in this set. Reload (F5) belongs here so the "F5 で再読込" recovery hint on
/// `STATUS_RESCAN_FAILED` is advertised by a key that is actually reachable
/// while browsing results; `reload` keeps the search query untouched (AC03).
fn search_view_action_allowed(action: KeyAction) -> bool {
    matches!(
        action,
        KeyAction::MoveUp
            | KeyAction::MoveDown
            | KeyAction::PageUp
            | KeyAction::PageDown
            | KeyAction::SelectFirst
            | KeyAction::SelectLast
            | KeyAction::OpenSelected
            | KeyAction::OpenTerminal
            | KeyAction::OpenSettings
            | KeyAction::Copy
            | KeyAction::Quit
            | KeyAction::Reload
    )
}

/// Whether a keystroke reaches the normal keymap while browsing Search
/// results. Keeps the original hardcoded keys and additionally honours any
/// keymap rebind of an allowed action.
fn search_view_key_reaches_keymap(
    keymap: &KeyMap,
    key: &str,
    ctrl: bool,
    shift: bool,
    alt: bool,
) -> bool {
    // AB01: the keymap-rebind path must not resurrect Alt chords that the
    // allowlist deliberately rejects. Reject Alt before consulting the keymap;
    // otherwise rebinding an allowed action (e.g. Copy) to ctrl+alt+y makes the
    // keymap side return that action and Alt keys leak through in Search view.
    if alt {
        return false;
    }
    search_view_key_allowed(key, ctrl, shift, alt)
        || search_view_action_allowed(keymap.resolve(key, ctrl, shift, alt))
}

/// Final Search-view routing decision, including the text-input focus guard
/// (AE01). `action` is the keystroke's resolved keymap action and
/// `reaches_keymap` is [`search_view_key_reaches_keymap`] for the same
/// keystroke; keeping the whole decision in a pure helper makes the input-
/// focused branch directly testable without a GPUI `Window`.
///
/// While a query text field owns focus every key stays with the input —
/// F4 / Ctrl+Q included, per the documented contract — except `Reload`:
/// F5 (and its Ctrl+R alias, or any rebind) must still re-run the current
/// query, because that is the recovery key advertised by
/// `STATUS_RESCAN_FAILED` and `LOAD_ERROR_HINT_SEARCH` (AC10). Checking the
/// resolved action rather than a hardcoded F5 keeps rebinds working.
fn search_view_key_reaches_dispatch(
    input_focused: bool,
    action: KeyAction,
    reaches_keymap: bool,
) -> bool {
    reaches_keymap && (!input_focused || matches!(action, KeyAction::Reload))
}

/// Icon colour per file kind (folders are blue, everything else is tinted).
///
/// Rendering now uses [`AppPalette::kind_color`]; this Mocha-only reference
/// remains for the pure unit test that checks the kinds stay distinguishable.
#[cfg(test)]
fn kind_color(kind: FileKind) -> u32 {
    match kind {
        FileKind::Folder => C_BLUE,
        // Symlinks reuse the muted tint already applied to them in the list
        // and grid render paths.
        FileKind::Symlink => C_MUTED,
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

/// Sentinels rendered in amber: stale search, cancellation, the read-only
/// settings notice and the `STATUS_*NO_*` "not available here" notices.
/// `STATUS_NO_SELECTION` is an operation refusal, not a success, so it belongs
/// here too (K03). Checked **before** the error markers because several
/// `STATUS_*FAILED` strings (e.g. "変更できません") are substrings of a longer
/// `STATUS_*NO_*` notice; the specific notice wins. An explicit
/// `STATUS_ERROR_PREFIX` still takes priority in [`status_color`], so wrapped
/// errors that happen to contain a refusal suffix stay red.
const STATUS_WARNING_MARKERS: &[&str] = &[
    STATUS_SEARCH_STALE,
    STATUS_CANCELLED,
    STATUS_SETTINGS_READ_ONLY,
    // X02: 未保存設定の破棄はデータ消失の通知で成功ではないが、失敗でもない。
    // 「できません」も持たないため、登録しないと成功 green に落ちていた。
    STATUS_SETTINGS_DISCARDED,
    STATUS_NO_SELECTION,
    STATUS_NO_CLIPBOARD,
    STATUS_DROP_NO_DESTINATION,
    STATUS_REDO_NO_TARGET,
    STATUS_TRASH_NO_COPY,
    STATUS_TRASH_NO_CUT,
    STATUS_TRASH_NO_PASTE,
    STATUS_TRASH_NO_RENAME,
    STATUS_TRASH_NO_PURGE,
    STATUS_TRASH_NO_HIDDEN,
    STATUS_TRASH_NO_SORT,
    // Refusal / empty-target notices are warnings, never success (L06).
    STATUS_TABS_ONLY_DIRECTORY,
    STATUS_BOOKMARKS_ONLY_DIRECTORY,
    STATUS_LAST_TAB,
    // M05: no-op unbind refuses an operation, so it is amber, never success.
    STATUS_KEYMAP_NO_UNBIND_PREFIX,
    STATUS_DROP_ONLY_DIRECTORY,
    STATUS_DROP_EMPTY,
    STATUS_TAB_RESTORE_EMPTY,
    STATUS_UNDO_EMPTY,
    STATUS_REDO_EMPTY,
    // N02: 「開けません」は「できません」を含まないため、登録しないと green になる。
    STATUS_CANNOT_OPEN_SUFFIX,
    // N03: ディレクトリ表示以外での機能拒否（肯定形だが拒否通知）。
    STATUS_PREVIEW_ONLY_DIRECTORY,
    STATUS_LOCATION_ONLY_DIRECTORY,
    STATUS_FUZZY_JUMP_ONLY_DIRECTORY,
    STATUS_PROPERTIES_ONLY_DIRECTORY,
    STATUS_SEARCH_ONLY_DIRECTORY,
    // N08 / N09: 件数上限・空の復元対象・空の検索語も操作拒否なので amber。
    STATUS_MAX_TABS_PREFIX,
    STATUS_RESTORE_EMPTY,
    STATUS_SEARCH_EMPTY_QUERY,
    // P03: 検索を開く前の案内は操作拒否なので amber（放置すると green になる）。
    STATUS_SEARCH_NOT_OPEN,
    // Q03: ブックマーク拒否は「できません」を含むが、具体的な拒否通知として
    // warning(amber) に分類する（error マーカーより先に判定される）。
    STATUS_BOOKMARK_NOT_PERSISTABLE,
    // Y08: 監視の初期化失敗は secondary slot が直接 p.yellow で描画するが、
    // 分類表との一貫性のため warning マーカーにも登録する。
    STATUS_WATCH_INIT_FAILED,
    // AB07: 一部無効 / 無効 も secondary slot が直接 p.yellow で描画する。
    // self.status を通らないためマーカー自体は表示経路に乗らないが、Y08 と
    // 同じく分類表との一貫性のため warning に登録する。
    STATUS_WATCH_PARTIAL,
    STATUS_WATCH_DISABLED,
];

/// Sentinels rendered in red: wrapped errors and the concrete
/// `STATUS_*FAILED` / partial-failure messages.
const STATUS_ERROR_MARKERS: &[&str] = &[
    STATUS_ERROR_PREFIX,
    STATUS_SETTINGS_SAVE_FAILED,
    // X01: 理由別の保存失敗は「できません」部分文字列で偶然赤くなっていただけ
    // なので、明示的なマーカーとして登録して回帰を防ぐ。
    STATUS_SETTINGS_SAVE_TOO_LARGE,
    STATUS_SETTINGS_SAVE_LOCKED,
    STATUS_RESCAN_FAILED,
    STATUS_TRASH_ALL_FAILED,
    STATUS_RESTORE_FAILED,
    STATUS_UNDO_FAILED,
    STATUS_REDO_FAILED,
    STATUS_RENAME_FAILED,
    STATUS_OP_FAILED_SUFFIX,
    STATUS_PARTIAL_FAILURE_SUFFIX,
    STATUS_TRASH_PARTIAL_FAILED,
    // A denied metadata read and a vanished path are both errors even though
    // neither message carries `STATUS_ERROR_PREFIX` (H02/H07).
    STATUS_ACCESS_DENIED,
    STATUS_PATH_GONE,
    // Missing drop/move targets and non-directory paths are concrete failures
    // too, not warnings: the requested operation could not be carried out
    // (I01). Keep them out of `STATUS_WARNING_MARKERS`.
    STATUS_TARGET_GONE,
    STATUS_TARGET_MISSING,
    STATUS_DROP_NOT_FOUND,
    STATUS_NOT_DIRECTORY,
    // M06: both suffixes carry a runtime value (`{name}` / `{n}`), so the
    // fixed part itself is the marker; otherwise these refusals read as success.
    STATUS_NOT_FOUND_SUFFIX,
    STATUS_MISSING_COUNT_SUFFIX,
    // Q02: P08 の「できません」拒否は STATUS_OP_FAILED_SUFFIX に包含的だが、
    // 明示的に登録してマーカー表の完全性をテストでも固定する。
    STATUS_SPECIAL_FILE_NO_COPY,
    STATUS_FOLDER_NO_PREVIEW,
    STATUS_SYMLINK_NO_PREVIEW,
    // R04 / R05: 動的な値を連結する失敗接頭辞も concrete failure として red。
    STATUS_SAVED_SEARCH_SAVE_FAILED_PREFIX,
    STATUS_BOOKMARK_LABEL_SET_FAILED_PREFIX,
    // X03 / X04 / X08: main.rs の format! 内に埋め込まれていた接頭辞 /
    // ローカル定数を strings.rs へ移し、同じマーカー表から色を決める。
    STATUS_BOOKMARK_LABEL_NOT_SET_PREFIX,
    STATUS_TRASH_LOAD_FAILED_PREFIX,
    STATUS_UNDO_MISSING_SUFFIX,
    // X09 / Y01: 起動ディレクトリ拒否は strings.rs の分類表で error だが、
    // 「開けません」(STATUS_CANNOT_OPEN_SUFFIX) を含むため、単体では warning に
    // 負けていた。呼び出し側は STATUS_ERROR_PREFIX を付けるが（下の status_color
    // の早期 return も参照）、分類表との整合のため明示的に error 登録する。
    STATUS_INVALID_CLI_DIR,
];

/// Pure colour decision for the primary status text (U-01). An explicit
/// `STATUS_ERROR_PREFIX` is always red, stale/cancelled/`NO_*` warnings are
/// amber, other errors are red, and everything else stays success green.
/// `contains` also covers composed messages such as "…しました（一部失敗: …）".
fn status_color(status: &str) -> u32 {
    // Warning markers are normally checked first so a specific refusal beats a
    // generic `*_FAILED` phrase. 「開けません」(N02), however, also occurs in the
    // wrapped startup error "エラー: 起動ディレクトリを開けませんでした"; the
    // explicit prefix must win there or that error would turn amber.
    // X09 / Y01: STATUS_INVALID_CLI_DIR は「開けません」を含むため warning
    // マーカーに一致するが、起動ディレクトリの拒否は concrete error なので
    // 明示的なエラー接頭辞と同じく最優先で赤にする（呼び出し側は常に接頭辞を
    // 付けるが、単体で classification を問うテストも正しく通す）。
    if status.contains(STATUS_ERROR_PREFIX) || status.contains(STATUS_INVALID_CLI_DIR) {
        return C_RED;
    }
    if STATUS_WARNING_MARKERS.iter().any(|m| status.contains(m)) {
        C_YELLOW
    } else if STATUS_ERROR_MARKERS.iter().any(|m| status.contains(m)) {
        C_RED
    } else {
        C_GREEN
    }
}

/// Secondary right-side status-bar slots in descending priority (U-10).
/// The watch slot is last because `STATUS_PRIORITY` lists "監視状態" last;
/// it is therefore the first slot dropped on a narrow bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SecondarySlot {
    HiddenCount,
    UndoCount,
    Version,
    Watch,
}

const SECONDARY_SLOT_PRIORITY: &[SecondarySlot] = &[
    SecondarySlot::HiddenCount,
    SecondarySlot::UndoCount,
    SecondarySlot::Version,
    SecondarySlot::Watch,
];

/// Characters of primary status text tolerated per `STATUS_PRIORITY` tier
/// before one secondary slot is hidden.
const STATUS_CHAR_BUDGET_PER_TIER: usize = 16;

/// Pure width approximation: return the secondary slots that still fit beside
/// `status`. The budget scales with `STATUS_PRIORITY.len()`; every extra tier
/// of status text hides the lowest-priority remaining slot first.
fn visible_secondary_slots(status: &str) -> &'static [SecondarySlot] {
    let budget = STATUS_CHAR_BUDGET_PER_TIER * STATUS_PRIORITY.len();
    let over = status.chars().count().saturating_sub(budget) / STATUS_CHAR_BUDGET_PER_TIER;
    let hide = over.min(SECONDARY_SLOT_PRIORITY.len());
    &SECONDARY_SLOT_PRIORITY[..SECONDARY_SLOT_PRIORITY.len() - hide]
}

/// U-22: join several startup notices with a visible separator so none of them
/// is silently replaced by a later assignment to `status`.
fn join_status_notices(notices: &[String]) -> String {
    notices.join(" / ")
}

/// Status-bar text for the current selection (U-11).
///
/// `detail` is the already-formatted text for the single selected entry,
/// without a prefix. `implicit` is true when that entry was auto-selected by a
/// load/reload (the keyboard needs a target) rather than picked by the user:
/// the entry is still a real, actionable selection, but the bar shows
/// "カーソル:" instead of claiming a user-made "選択:".
fn selection_status_text(selection_count: usize, detail: Option<&str>, implicit: bool) -> String {
    match selection_count {
        0 => "選択なし".to_string(),
        1 => {
            let Some(detail) = detail.filter(|text| !text.is_empty()) else {
                return "選択なし".to_string();
            };
            if implicit {
                format!("カーソル: {detail}")
            } else {
                format!("選択: {detail}")
            }
        }
        count => format!("{count} 件選択"),
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

/// Short status label for a theme preference. Both theme entry points (the
/// Alt+Shift+T toggle and the settings overlay Save) use it so a switch is
/// reported identically (G07).
fn theme_choice_label(choice: ThemeChoice) -> &'static str {
    match choice {
        ThemeChoice::Light => THEME_LABEL_LIGHT,
        ThemeChoice::Dark => THEME_LABEL_DARK,
        ThemeChoice::System => THEME_LABEL_SYSTEM,
    }
}

/// Caption of a saved-search chip. The name comes first so the list can be
/// scanned quickly, with the saved query shown for confirmation (F6-49).
fn saved_search_caption(name: &str, query: &str) -> String {
    format!("{name}: {query}")
}

/// Status shown after a saved-search save click. Rejected saves are reported
/// explicitly so a click is never silently ignored (F6-49).
fn saved_search_save_status(name: &str, stored: bool) -> String {
    if stored {
        format!("検索を保存しました: {name}")
    } else {
        // 空の名前／検索語は呼び出し側で別メッセージとして扱われるため、ここ
        // は保存件数の上限に達した場合の案内とする（F6-49 / R04）。
        format!("{STATUS_SAVED_SEARCH_SAVE_FAILED_PREFIX}{name}{STATUS_SAVED_SEARCH_CAP_HINT}")
    }
}

/// Status-bar copy of a settings-card validation error. The card footer already
/// paints the raw message red; the status bar needs the error prefix to get the
/// same treatment from `status_color`. Keeping this composition in one pure
/// function lets the regression test exercise the handler's actual message
/// instead of rebuilding the prefix by hand (G13).
fn settings_validation_status(message: &str) -> String {
    format!("{STATUS_ERROR_PREFIX}{message}")
}

/// Status shown when Esc / backdrop dismissal drops unsaved settings edits.
/// A clean form needs no notice; only a dirty one loses work (H03).
fn settings_discard_status(dirty: bool) -> Option<&'static str> {
    dirty.then_some(STATUS_SETTINGS_DISCARDED)
}

/// Caption for the keymap dialog's `bind` command. Kept pure so the chord in
/// the success notice can be regression-tested against the canonical form that
/// `upsert_keymap_override` persists and F1 help displays (N06).
fn keymap_bind_caption(action: &str, chord: &str) -> String {
    format!(
        "キーマップを設定しました: {action} → {}",
        canonical_chord(chord)
    )
}

/// Caption for the keymap dialog's `unbind` command. Kept pure so the no-op
/// branch (K02) can be regression-tested without a GPUI `Context`.
fn keymap_unbind_caption(removed: bool, action: &str) -> String {
    if removed {
        format!("キーマップを解除しました: {action}")
    } else {
        format!("{STATUS_KEYMAP_NO_UNBIND_PREFIX}{action}")
    }
}

/// Keymap override limit notice (V04). Both the Alt+K dialog and the settings
/// card hit this cap, so the dynamic count is composed in one place only.
fn keymap_limit_status() -> String {
    format!("{STATUS_KEYMAP_LIMIT_PREFIX}{MAX_KEYMAP_OVERRIDES}{STATUS_KEYMAP_LIMIT_SUFFIX}")
}

/// Choose the status to show after a settings write (Bug L01). The caller's
/// success message is used only when the write succeeded; otherwise the
/// dedicated save-failure status wins, even when the UI action itself worked.
fn settings_save_status(saved: bool, success: String) -> String {
    if saved {
        success
    } else {
        STATUS_SETTINGS_SAVE_FAILED.to_string()
    }
}

/// Pure mapping from a save error to an accurate user-facing notice (U03).
fn settings_save_error_status(error: Option<SettingsSaveError>) -> Option<String> {
    match error {
        Some(SettingsSaveError::TooLarge) => Some(STATUS_SETTINGS_SAVE_TOO_LARGE.to_string()),
        Some(SettingsSaveError::Locked) => Some(STATUS_SETTINGS_SAVE_LOCKED.to_string()),
        Some(SettingsSaveError::Io) | None => None,
    }
}

/// Start-up writes call `save_settings()` before any status bar is live, so the
/// failure must be turned into a notice here (V01). Use the concrete reason
/// recorded in `settings.last_save_error` when there is one; only fall back to
/// the generic notice for plain I/O failures / unknown causes.
fn startup_save_failure_status(settings: &Settings) -> String {
    settings_save_error_status(settings.last_save_error.get())
        .unwrap_or_else(|| STATUS_SETTINGS_SAVE_FAILED.to_string())
}

/// Read-only notice shared by startup and every skipped write (W08). When a
/// corrupt/unsupported settings file was rejected, `load_error` holds the
/// concrete reason; showing only the generic notice hides why the file is
/// read-only and what the user can do about it (F6-36: 理由 + 復帰導線).
fn settings_read_only_status(settings: &Settings) -> String {
    match settings.load_error.as_deref() {
        Some(reason) if !reason.is_empty() => {
            format!("{STATUS_SETTINGS_READ_ONLY}: {reason}{STATUS_SETTINGS_READ_ONLY_HINT}")
        }
        _ => STATUS_SETTINGS_READ_ONLY.to_string(),
    }
}

/// Status shown after a settings write, accounting for skipped writes (F04).
/// `save()` returns `true` for read-only / ephemeral sessions even though no
/// file was written, so those sessions must get the dedicated read-only notice
/// instead of the caller's success message — the same behaviour the settings
/// overlay already had. Rollback is still decided by the raw `saved` flag: a
/// skipped write keeps the in-memory preview for the current session.
///
/// `attempted` tells whether `save()` was actually called. No-op callers (an
/// unchanged label, a rejected saved-search add, …) never reach `save()`, so
/// there is nothing to skip: their own rejection message must survive instead
/// of being masked by `STATUS_SETTINGS_READ_ONLY` (I02).
fn settings_write_status(
    settings: &Settings,
    saved: bool,
    attempted: bool,
    success: String,
) -> String {
    if !attempted {
        return success;
    }
    if settings.write_was_skipped() {
        settings_read_only_status(settings)
    } else if !saved {
        // U03: prefer the concrete reason recorded by `save_with_reason`.
        settings_save_error_status(settings.last_save_error.get())
            .unwrap_or_else(|| settings_save_status(saved, success))
    } else {
        settings_save_status(saved, success)
    }
}

/// Status shown after a view switch. Only a real write reports the switch; a
/// skipped (read-only / ephemeral) write shows the read-only notice and a failed
/// write shows the save-failure notice, so `set_view` never silently drops the
/// outcome of `save_settings` (H05/H14/H15).
fn set_view_status(settings: &Settings, saved: bool, mode: ViewLayout) -> String {
    let success = match mode {
        ViewLayout::Details => "詳細リストに切り替えました".to_string(),
        ViewLayout::Icons => "アイコングリッドに切り替えました".to_string(),
    };
    settings_write_status(settings, saved, true, success)
}

/// Pure rollback decision shared by the seven write paths that preview settings
/// in memory before persisting them (the settings overlay Save, the terminal
/// command dialog, the keymap dialog, `toggle_theme`, `set_sort`,
/// `toggle_hidden` and `set_view`). A failed write leaves disk at the old
/// values, so the previewed in-memory values must be discarded to keep both in
/// sync (D13/H01).
fn settings_write_needs_rollback(saved: bool) -> bool {
    !saved
}

/// Status shown after undoing a trash operation. A zero restore count means
/// nothing came back, so it is reported as a failure (and the caller keeps the
/// undo entry) instead of claiming a successful "0 件" restore.
fn trash_undo_status(label: &str, restored: usize, missing: usize) -> String {
    if restored == 0 {
        // T08: 理由フラグメントは strings.rs の定数を共有する。
        format!("{STATUS_RESTORE_FAILED}（項目が{STATUS_TRASH_MISSING_REASON}）")
    } else if missing == 0 {
        format!("{label}を元に戻しました（{restored} 件）")
    } else {
        // M01: 一部の復元失敗は成功ではなく一部失敗として色判定される必要がある。
        // `STATUS_PARTIAL_FAILURE_SUFFIX` の "しました" を動詞の活用にそのまま
        // 使うため、直前に "元に戻" を置く（"元に戻しました（一部失敗: …"）。
        format!("{label}を元に戻{STATUS_PARTIAL_FAILURE_SUFFIX}{restored} 件、{missing} 件は{STATUS_TRASH_MISSING_REASON}）")
    }
}

/// Status after a cancelled trash redo. Items already moved to the trash before
/// the cancellation are reported instead of being hidden behind a bare cancel
/// message, mirroring the normal (non-redo) trash path. A zero count means
/// nothing was moved, so only the cancellation is reported.
fn trash_redo_cancelled_status(trashed: usize) -> String {
    if trashed == 0 {
        STATUS_CANCELLED.to_string()
    } else {
        format!("{STATUS_CANCELLED}（{trashed} 件をゴミ箱へ移動済み）")
    }
}

/// Row name with a bookmark label prefixed. The original name stays visible so
/// the underlying entry can still be recognised (F6-49).
fn labeled_display_name(base: &str, label: Option<&str>) -> String {
    match label {
        Some(label) if !label.trim().is_empty() => format!("[{}] {}", label.trim(), base),
        _ => base.to_owned(),
    }
}

/// Why would `set_bookmark_label` refuse this input? (U04)
///
/// Mirrors the same three checks as the `bookmark_label_is_settable` pre-flight
/// (the path rules of `Settings::sane_path`, `BookmarkLabel::new`, then the
/// `MAX_BOOKMARK_LABELS` cap which does not apply to an existing entry), in the
/// same order. Returning the exact reason instead of a bare `bool` lets the
/// rejection caption tell the user how to recover. Kept pure for regression
/// tests; the wording comes from `strings.rs`.
fn bookmark_label_rejection_reason(
    settings: &Settings,
    path: &std::path::Path,
    label: &str,
) -> Option<&'static str> {
    // `sane_path` is private to `settings.rs`; replicate its contract here so
    // this pre-flight agrees with the real setter on unsanitary paths (E09).
    let bytes = path.as_os_str().as_encoded_bytes();
    if bytes.is_empty()
        || bytes.len() > crate::settings::MAX_PATH_BYTES
        || bytes.contains(&0)
        || path.to_str().is_none()
    {
        return Some(STATUS_BOOKMARK_LABEL_REASON_INVALID_PATH);
    }
    if crate::model::BookmarkLabel::new(path, label).is_none() {
        return Some(STATUS_BOOKMARK_LABEL_REASON_INVALID_LABEL);
    }
    if settings.bookmark_label(path).is_none()
        && settings.bookmark_labels.len() >= crate::model::MAX_BOOKMARK_LABELS
    {
        return Some(STATUS_BOOKMARK_LABEL_REASON_LIMIT);
    }
    None
}

/// Pure pre-flight check for `commit_label_editor`: would `set_bookmark_label`
/// accept this label? Validating before `ensure_bookmark_for_label` keeps a
/// rejected label from leaving behind a bookmark with no label attached.
fn bookmark_label_is_settable(settings: &Settings, path: &std::path::Path, label: &str) -> bool {
    bookmark_label_rejection_reason(settings, path, label).is_none()
}

/// Rejection caption when a bookmark label cannot be set (R05 / U04). Kept pure
/// so the composed wording and its red colour classification can be regression
/// tested against the exact string the editor handler emits. `reason` is `None`
/// only when the setter refused for a cause the pre-flight cannot see.
fn bookmark_label_set_rejection(path: &std::path::Path, reason: Option<&'static str>) -> String {
    let base = format!(
        "{STATUS_BOOKMARK_LABEL_SET_FAILED_PREFIX}{}",
        display_path(path)
    );
    match reason {
        Some(reason) => format!("{base}{reason}"),
        None => base,
    }
}

/// Rejection caption when no bookmark label was set (U05). Both the label
/// editor and the row context menu emit this text; composing it with
/// `STATUS_ERROR_PREFIX` keeps the refusal red instead of letting the inline
/// literal fall through to success green. Kept pure so the exact handler copy
/// can be regression-tested.
fn bookmark_label_not_set_status(path: &std::path::Path) -> String {
    format!(
        "{STATUS_ERROR_PREFIX}{STATUS_BOOKMARK_LABEL_NOT_SET_PREFIX}{}",
        display_path(path)
    )
}

/// Error text for a failed trash listing (U06). Kept pure so the wording and
/// its red colour classification can be regression-tested.
fn trash_load_error_status(error: &str) -> String {
    format!("{STATUS_ERROR_PREFIX}{STATUS_TRASH_LOAD_FAILED_PREFIX}{error}")
}

/// Display text of a load error: add the shared error prefix exactly once
/// (U06). Some load errors already carry it at their source (the trash path),
/// so a blind `format!` at the render site would print "エラー: エラー: …".
fn load_error_display_text(error: &str) -> String {
    if error.starts_with(STATUS_ERROR_PREFIX) {
        error.to_string()
    } else {
        format!("{STATUS_ERROR_PREFIX}{error}")
    }
}

/// Compose the missing-target undo refusal so its wording and red colour can
/// be regression-tested without a live GPUI `Context`.
fn undo_missing_status(path: &std::path::Path) -> String {
    format!(
        "{STATUS_UNDO_FAILED}: {}{STATUS_UNDO_MISSING_SUFFIX}",
        display_path(path)
    )
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

/// Base log level for a finished transfer, derived from its outcome.
///
/// Cancellation is routine user behaviour (debug), a clean run is informational
/// and any recorded error is at least a failure. The caller downgrades `ERROR`
/// to `WARN` when some items were still transferred (partial failure).
fn transfer_log_level(first_error: &Option<OpError>, cancelled: bool) -> Level {
    if cancelled || matches!(first_error, Some(OpError::Cancelled)) {
        Level::DEBUG
    } else if first_error.is_none() {
        Level::INFO
    } else {
        Level::ERROR
    }
}

/// Status text for a byte-progress update during a transfer (U08). Building it
/// here keeps the parenthesised cancel guidance on the shared
/// `STATUS_CANCEL_HINT` constant instead of a hand-copied inline literal.
fn transfer_progress_status(verb: &str, done: u64, total: u64) -> String {
    format!(
        "{verb}中… {} / {} {STATUS_CANCEL_HINT}",
        human_size(done),
        human_size(total)
    )
}

/// Emit an event at a runtime-selected level. `tracing::event!` requires a
/// constant level (its callsite is a `static`), so dispatch to the level macros.
macro_rules! transfer_event {
    ($level:expr, $($fields:tt)*) => {
        if $level == Level::ERROR {
            tracing::error!($($fields)*)
        } else if $level == Level::WARN {
            tracing::warn!($($fields)*)
        } else if $level == Level::INFO {
            tracing::info!($($fields)*)
        } else {
            tracing::debug!($($fields)*)
        }
    };
}

/// Final-status suffix for sources filtered out *before* a transfer starts.
///
/// `run_transfer` replaces `status` immediately with its "…中…" message, so a
/// caller that pre-filters missing/special entries must hand its notice through
/// the transfer instead of assigning it to `status` beforehand.
///
/// T02: every caller hands `(reason, count)` pairs instead of a hand-written
/// sentence, so the wording lives only in `strings.rs`
/// (`STATUS_SKIPPED_PREFIX` / `STATUS_SKIPPED_SUFFIX`). This keeps the paste,
/// move and external-drop paths in sync instead of drifting into three
/// different inline spellings.
fn transfer_skip_notice(parts: &[(&str, usize)]) -> String {
    if parts.is_empty() {
        return String::new();
    }
    let body = parts
        .iter()
        .map(|(reason, count)| format!("{reason} {count}"))
        .collect::<Vec<_>>()
        .join(" / ");
    format!("{STATUS_SKIPPED_PREFIX}{body}{STATUS_SKIPPED_SUFFIX}")
}

// ------------------------------------------------------------- F6-50 terminal

/// Directory in which "open terminal here" starts the child process.
///
/// A selected directory is used as-is; a selected file uses its parent so the
/// shell opens where the file lives. `fallback` is the listing's current
/// directory when nothing is selected (or the path has no parent).
fn terminal_dir_for(path: &Path, is_dir: bool, fallback: &Path) -> PathBuf {
    if is_dir {
        return path.to_path_buf();
    }
    path.parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| fallback.to_path_buf())
}

/// Resolve the terminal working directory from the focused row, falling back
/// to the current directory when the listing is empty.
fn terminal_target_dir(entry: Option<&Entry>, cwd: &Path) -> PathBuf {
    match entry {
        Some(entry) => terminal_dir_for(&entry.path, entry.is_dir, cwd),
        None => cwd.to_path_buf(),
    }
}

/// Target of a middle click on a row: a directory row opens that directory in
/// the new tab; a file row (or any non-directory entry) keeps the current
/// directory. Pure so the rule can be tested without a GPUI window.
/// Version text shown at the right edge of the status bar.
fn version_label() -> String {
    format!("v{}", env!("CARGO_PKG_VERSION"))
}

fn middle_click_tab_dir(entry: &Entry, cwd: &Path) -> PathBuf {
    if entry.is_dir {
        entry.path.clone()
    } else {
        cwd.to_path_buf()
    }
}

/// Build the child process from an already-split argv. The target path never
/// enters the command line: it is passed only via `Command::current_dir`, so a
/// user-controlled path cannot be interpreted as a shell fragment (F6-50).
fn terminal_process(argv: &[String], dir: &Path) -> Option<Command> {
    let (program, args) = argv.split_first()?;
    let mut command = Command::new(program);
    command.args(args).current_dir(dir);
    Some(command)
}

/// Normalize the text typed in the terminal-command settings dialog: trim
/// surrounding whitespace, and map a blank command to `None` (the "disabled"
/// state persisted as an empty string). Pure so the rule is unit-testable
/// without constructing a GPUI input entity.
fn normalize_terminal_command(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

/// Status shown when the configured terminal command is missing or unusable.
/// The reason / recovery hint lives in `strings.rs` so its wording is shared
/// with the colour-classification and wording tests (S07).
fn terminal_disabled_status() -> String {
    format!("ターミナルを{STATUS_CANNOT_OPEN_SUFFIX}{STATUS_TERMINAL_DISABLED_HINT}")
}

/// Status shown after a successful launch (command plus target directory).
fn terminal_started_status(argv: &[String], dir: &Path) -> String {
    format!(
        "ターミナルを起動しました: {}（{}）",
        argv.join(" "),
        display_path(dir)
    )
}

/// Status shown when the spawn itself failed, including the OS error so the
/// user can tell a missing emulator from a missing directory.
fn terminal_failed_status(error: &std::io::Error, dir: &Path) -> String {
    format!(
        "{STATUS_TERMINAL_START_PREFIX}{STATUS_OP_FAILED_SUFFIX}: {error}（{}）",
        display_path(dir)
    )
}

// --------------------------------------------------------------- F6-51 keymap

/// Logical action produced by a keystroke. The keyboard dispatch is a table of
/// (keystroke -> action) pairs rather than a hardcoded `match`, so a binding can
/// be changed without touching the dispatch code (F6-51 / r5-future-14).
///
/// Dialog-ownership keys (`F1`, `Escape`) are deliberately handled *before* the
/// table in `on_key_down`: a bad rebind must never lock the user out of a
/// dialog, so those two keys stay literal and are not listed here.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum KeyAction {
    OpenPreview,
    StartRename,
    Reload,
    OpenSelected,
    GoParent,
    MoveUp,
    MoveDown,
    PageUp,
    PageDown,
    SelectFirst,
    SelectLast,
    TrashSelected,
    RestoreSelected,
    ToggleHidden,
    SelectAll,
    ToggleBookmark,
    OpenSearch,
    FocusSearchName,
    OpenProperties,
    OpenLocation,
    Undo,
    Redo,
    ToggleUndoHistory,
    NewTab,
    RestoreClosedTab,
    CloseTab,
    NextTab,
    PrevTab,
    MoveTabLeft,
    MoveTabRight,
    DetailsView,
    IconsView,
    NewFolder,
    Copy,
    Cut,
    Paste,
    Quit,
    GoBack,
    GoForward,
    /// Ctrl+P: open the fuzzy-jump overlay.
    FuzzyJump,
    /// F4: open a terminal in the selected directory (F6-50).
    OpenTerminal,
    /// Alt+T: edit the terminal-command setting used by F4.
    EditTerminalCommand,
    /// Alt+K: edit keybinding overrides from the keymap dialog.
    EditKeymap,
    /// Ctrl+,: open the full preferences overlay (theme, files, shortcuts, …).
    OpenSettings,
    /// Alt+Shift+T: cycle the persisted component theme (dark / light).
    ToggleTheme,
    /// Ctrl+Shift+L: jump to the first sidebar place (Home). Sidebar rows are
    /// click-only, so this is the minimal keyboard path into the sidebar
    /// (U-07); the other places stay mouse-only.
    FocusSidebar,
    /// Ctrl+Space: toggle the focused row in the non-contiguous selection,
    /// the keyboard equivalent of Ctrl+click (U-08).
    ToggleSelection,
    /// Ctrl+Shift+C: copy the focused row's path, reusing the context-menu
    /// handler. Reveal-in-manager and set-label stay context-menu-only.
    CopyPath,
    /// No binding (either unmapped by default or unbound by the user).
    None,
}

/// One row of the keymap table. `shift: None` matches both shift states, which
/// preserves the pre-existing `Ctrl+H` / `Ctrl+R` / `Ctrl+Q` "either shift"
/// behaviour and the plain Shift+Arrow selection extension.
#[derive(Clone, Debug, PartialEq, Eq)]
struct KeyBinding {
    // Owned so a chord read from settings can be stored directly. `KeyMap`
    // itself is still `Clone`; only the `Copy` derive had to go.
    key: String,
    ctrl: bool,
    alt: bool,
    shift: Option<bool>,
    action: KeyAction,
}

/// Parse a persisted chord like `ctrl+shift+p` into `(key, ctrl, shift, alt)`.
///
/// Modifiers may appear in any order and are case-insensitive; the key is the
/// last token and is lowercased to match GPUI's keystroke names. An omitted
/// `shift` stays `None` (it becomes a concrete `false` when applied through
/// `rebind`), while an explicit `shift+` yields `Some(true)`.
fn parse_chord(chord: &str) -> Option<(String, bool, Option<bool>, bool)> {
    // Persisted entries are `action=chord;` joined by ';'. A chord containing
    // ';' would round-trip as `action=ctrl+;;` and fail to parse on reload, so
    // reject it here as well as at the keymap-input boundary.
    if chord.contains(';') {
        return None;
    }
    let (mut ctrl, mut shift, mut alt) = (false, None, false);
    let mut key = None;
    for token in chord.split('+') {
        let token = token.trim();
        if token.is_empty() {
            return None;
        }
        match token.to_ascii_lowercase().as_str() {
            "ctrl" if key.is_none() => ctrl = true,
            "shift" if key.is_none() => shift = Some(true),
            "alt" if key.is_none() => alt = true,
            _ if key.is_none() => key = Some(token.to_ascii_lowercase()),
            _ => return None,
        }
    }
    Some((key?, ctrl, shift, alt))
}

/// Whether a chord mentions a dialog-ownership key. Delegates to
/// `settings::is_reserved_chord`: the loader drops such entries via
/// `settings::sane_keymap_entry`, so the dialog must reject the same input
/// instead of silently writing a useless override.
fn keymap_chord_reserved(chord: &str) -> bool {
    // Single source of truth shared with settings::is_reserved_chord.
    crate::settings::is_reserved_chord(chord)
}

/// One command accepted by the keymap dialog's single text input.
#[derive(Clone, Debug, PartialEq, Eq)]
enum KeymapInput {
    /// `action_name chord`: add or replace a persisted override.
    Bind { action: String, chord: String },
    /// `unbind action_name`: drop the persisted override for that action.
    Unbind { action: String },
}

/// Parse one line typed into the keymap dialog: `action_name chord` or
/// `unbind action_name`.
///
/// A single text input is used instead of raw key capture: GPUI dispatches
/// `key_down` globally and a modal input cannot safely swallow arbitrary
/// keystrokes without stealing Escape / F1 from the dialog guards. The action
/// is validated through serde against `KeyAction`'s snake_case names and the
/// chord through `parse_chord`, so a bound pair is always applicable by
/// `KeyMap::from_settings`. The chord is lowercased for canonical storage.
/// The `unbind` form needs no chord because it removes an existing override,
/// restoring the default binding for that action.
fn parse_keymap_input(input: &str) -> Option<KeymapInput> {
    // ';' is the persisted entry separator, never part of a chord. Rejecting it
    // before tokenizing keeps every accepted input string round-trippable.
    if input.contains(';') {
        return None;
    }
    let mut fields = input.split_whitespace();
    let first = fields.next()?;
    let second = fields.next()?;
    if fields.next().is_some() {
        return None;
    }
    if first.len() > MAX_CHORD_BYTES || second.len() > MAX_CHORD_BYTES {
        return None;
    }
    // `none` is a serde name but not a bindable action (no table row has it).
    // Reserved actions are rejected as well: moving or unbinding OpenSettings
    // would evict the shipped Ctrl+, and make the static recovery hint false
    // (L01). The check covers both the bind and the unbind form.
    let parse_action = |name: &str| -> Option<String> {
        if crate::settings::is_reserved_action(name) {
            return None;
        }
        let value = serde_json::Value::String(name.to_string());
        let action = serde_json::from_value::<KeyAction>(value).ok()?;
        (action != KeyAction::None).then(|| name.to_string())
    };
    if first == "unbind" {
        return Some(KeymapInput::Unbind {
            action: parse_action(second)?,
        });
    }
    let action = parse_action(first)?;
    let chord = second;
    parse_chord(chord)?;
    if keymap_chord_reserved(chord) {
        return None;
    }
    // `parse_chord` tolerates repeated modifiers; the dialog is stricter so a
    // typo like `ctrl+ctrl+p` is reported instead of being persisted.
    let mut seen = std::collections::HashSet::new();
    for token in chord.split('+') {
        if matches!(
            token.trim().to_ascii_lowercase().as_str(),
            "ctrl" | "shift" | "alt"
        ) && !seen.insert(token.trim().to_ascii_lowercase())
        {
            return None;
        }
    }
    Some(KeymapInput::Bind {
        action,
        chord: chord.to_ascii_lowercase(),
    })
}

/// Normalize a chord into one stable persisted spelling: modifiers in the
/// order used by `format_chord` (`ctrl`, `alt`, `shift`) and a lowercased key.
/// `parse_chord` already treats modifier order and case as irrelevant, so two
/// spellings of the same keystroke (e.g. `ctrl+shift+p` / `shift+ctrl+p`) must
/// compare equal when evicting overrides, just as `KeyMap::rebind` does at
/// runtime. Already-canonical forms such as `ctrl+shift+p` stay unchanged.
fn canonical_chord(chord: &str) -> String {
    let Some((key, ctrl, shift, alt)) = parse_chord(chord) else {
        return chord.trim().to_ascii_lowercase();
    };
    format_chord(&KeyBinding {
        key,
        ctrl,
        alt,
        shift,
        action: KeyAction::None,
    })
}

/// Whether two chord spellings claim the same keystroke under `KeyMap`
/// dispatch. `binding_matches` deliberately ignores `alt` for Ctrl bindings,
/// so `ctrl+p` and `ctrl+alt+p` must be treated as one chord when a persisted
/// override is inserted (S06). Otherwise both survive in settings while only
/// the later one survives in the live keymap after `from_settings` reloads
/// them — the file and the dispatcher then disagree about the owner.
///
/// An omitted shift resolves to `false`, matching `KeyMap::from_settings`, so
/// `ctrl+p` and `ctrl+shift+p` stay distinct just as they are at runtime.
fn chords_collide(a: &str, b: &str) -> bool {
    let (Some((key_a, ctrl_a, shift_a, alt_a)), Some((key_b, ctrl_b, shift_b, alt_b))) =
        (parse_chord(a), parse_chord(b))
    else {
        // An unparseable spelling has no modifier semantics; fall back to the
        // canonical form so deduplication stays deterministic.
        return canonical_chord(a) == canonical_chord(b);
    };
    let binding = KeyBinding {
        key: key_a,
        ctrl: ctrl_a,
        alt: alt_a,
        shift: Some(shift_a.unwrap_or(false)),
        action: KeyAction::None,
    };
    KeyMap::binding_matches(
        &binding,
        &key_b,
        ctrl_b,
        alt_b,
        Some(shift_b.unwrap_or(false)),
    )
}

/// Insert or replace one persisted override, deduplicated by action name.
/// Returns `false` only when the list is full for a new action; the parser has
/// already rejected invalid and reserved pairs before this is called. The
/// chord is stored canonically so eviction is independent of modifier order.
fn upsert_keymap_override(
    overrides: &mut Vec<(String, String)>,
    action: &str,
    chord: &str,
) -> bool {
    let chord = canonical_chord(chord);
    // A chord can only belong to one action. Mirror `KeyMap::rebind`'s eviction
    // in the persisted list so the binding survives a restart unchanged. The
    // comparison is semantic, not textual: `binding_matches` ignores `alt` for
    // Ctrl chords, so `ctrl+p` and `ctrl+alt+p` are the same keystroke and must
    // not both be stored (S06). `canonical_chord` still normalizes modifier
    // order for the value we actually persist.
    overrides.retain(|(name, bound)| name == action || !chords_collide(bound, &chord));
    if let Some(entry) = overrides.iter_mut().find(|(name, _)| name == action) {
        entry.1 = chord;
        return true;
    }
    if overrides.len() >= MAX_KEYMAP_OVERRIDES {
        return false;
    }
    overrides.push((action.to_string(), chord));
    true
}

/// Remove every persisted override for `action`, leaving other entries intact.
/// Returns `true` when at least one entry was removed. Rebuilding the live
/// `KeyMap` afterwards restores the action's default binding.
fn remove_keymap_override(overrides: &mut Vec<(String, String)>, action: &str) -> bool {
    let before = overrides.len();
    overrides.retain(|(name, _)| name != action);
    before != overrides.len()
}

/// Personalized key map. The table is data, so a settings panel (or a test)
/// can rebind an action without editing `on_key_down` (F6-51).
#[derive(Clone, Debug)]
struct KeyMap {
    bindings: Vec<KeyBinding>,
}

impl KeyMap {
    /// Whether a binding claims a keystroke. `None` for `shift` means "any
    /// shift state", so both wildcard-default bindings and concrete rebinds
    /// are compared consistently.
    ///
    /// For Ctrl bindings `alt` is a don't-care: before the F6-51 keymap
    /// migration, `on_key_down` tested `modifiers.control` first and never
    /// inspected `alt`, so `Ctrl+Alt+H` fired the same action as `Ctrl+H`.
    /// Alt navigation (`ctrl == false`) still requires an exact `alt` match,
    /// keeping `Alt+Left` working and `Alt+<other key>` a no-op (Finding 5).
    fn binding_matches(
        binding: &KeyBinding,
        key: &str,
        ctrl: bool,
        alt: bool,
        shift: Option<bool>,
    ) -> bool {
        binding.key.as_str() == key
            && binding.ctrl == ctrl
            && (binding.ctrl || binding.alt == alt)
            && match (binding.shift, shift) {
                (Some(bound), Some(wanted)) => bound == wanted,
                _ => true,
            }
    }

    /// The default table reproduces every shortcut that existed before F6-51
    /// and adds Ctrl+P for fuzzy jump. Binding order is significant: the first
    /// match wins.
    fn default_keymap() -> Self {
        // Import the variants explicitly: a glob would import `KeyAction::None`
        // and shadow `Option::None` used in the wildcard-shift bindings below.
        use KeyAction::{
            CloseTab, Copy, Cut, DetailsView, EditKeymap, EditTerminalCommand, FocusSearchName,
            FuzzyJump, GoBack, GoForward, GoParent, IconsView, MoveDown, MoveTabLeft, MoveTabRight,
            MoveUp, NewFolder, NewTab, NextTab, OpenLocation, OpenPreview, OpenProperties,
            OpenSearch, OpenSelected, OpenSettings, OpenTerminal, PageDown, PageUp, Paste, PrevTab,
            Quit, Redo, Reload, RestoreClosedTab, RestoreSelected, SelectAll, SelectFirst,
            SelectLast, StartRename, ToggleBookmark, ToggleHidden, ToggleTheme, ToggleUndoHistory,
            TrashSelected, Undo,
        };
        let binding = |key: &str, ctrl, alt, shift, action| KeyBinding {
            key: key.to_string(),
            ctrl,
            alt,
            shift,
            action,
        };
        Self {
            bindings: vec![
                // Plain keys.
                binding("up", false, false, None, MoveUp),
                binding("down", false, false, None, MoveDown),
                binding("pageup", false, false, None, PageUp),
                binding("pagedown", false, false, None, PageDown),
                binding("home", false, false, None, SelectFirst),
                binding("end", false, false, None, SelectLast),
                binding("enter", false, false, None, OpenSelected),
                binding("backspace", false, false, None, GoParent),
                binding("f5", false, false, None, Reload),
                binding("space", false, false, None, OpenPreview),
                binding("f3", false, false, None, OpenPreview),
                // F4 follows the KDE/Dolphin convention of opening a terminal
                // in the current folder. It was unused, so no F6-51 binding
                // (notably Ctrl+P fuzzy jump) changes (F6-50).
                binding("f4", false, false, None, OpenTerminal),
                binding("f2", false, false, None, StartRename),
                binding("delete", false, false, None, TrashSelected),
                binding("r", false, false, None, RestoreSelected),
                // Ctrl keys.
                binding("h", true, false, None, ToggleHidden),
                binding("a", true, false, Some(false), SelectAll),
                binding("d", true, false, Some(false), ToggleBookmark),
                binding("f", true, false, Some(false), OpenSearch),
                binding("f", true, false, Some(true), FocusSearchName),
                binding("i", true, false, Some(false), OpenProperties),
                binding("l", true, false, Some(false), OpenLocation),
                // U-07: Ctrl+Shift+L jumps to the first sidebar place (Home).
                // OpenLocation above is shift=Some(false), so the shifted
                // chord cannot collide under `binding_matches`.
                binding("l", true, false, Some(true), KeyAction::FocusSidebar),
                binding("z", true, false, Some(false), Undo),
                binding("z", true, false, Some(true), Redo),
                binding("u", true, false, Some(true), ToggleUndoHistory),
                binding("t", true, false, Some(false), NewTab),
                binding("t", true, false, Some(true), RestoreClosedTab),
                binding("w", true, false, Some(false), CloseTab),
                binding("tab", true, false, Some(false), NextTab),
                binding("tab", true, false, Some(true), PrevTab),
                binding("pageup", true, false, Some(true), MoveTabLeft),
                binding("pagedown", true, false, Some(true), MoveTabRight),
                binding("1", true, false, Some(false), DetailsView),
                binding("2", true, false, Some(false), IconsView),
                binding("r", true, false, None, Reload),
                binding("n", true, false, Some(true), NewFolder),
                binding("c", true, false, Some(false), Copy),
                // Ctrl+Shift+C copies the focused row's path via the
                // context-menu handler; plain Ctrl+C above is shift=Some(false).
                binding("c", true, false, Some(true), KeyAction::CopyPath),
                // Ctrl+Space toggles the focused row in the non-contiguous
                // selection (U-08); plain Space (OpenPreview) is ctrl=false.
                binding(
                    "space",
                    true,
                    false,
                    Some(false),
                    KeyAction::ToggleSelection,
                ),
                binding("x", true, false, Some(false), Cut),
                binding("v", true, false, Some(false), Paste),
                binding("enter", true, false, Some(false), RestoreSelected),
                binding("q", true, false, None, Quit),
                binding("p", true, false, Some(false), FuzzyJump),
                // Ctrl+, opens the preferences overlay. `,` is unused by any
                // other binding; `shift: Some(false)` keeps Ctrl+< unbound.
                // `binding_matches` ignores `alt` for Ctrl bindings, so this
                // deliberately has no Ctrl+Alt counterpart.
                binding(",", true, false, Some(false), OpenSettings),
                // Alt keys.
                binding("left", false, true, None, GoBack),
                binding("right", false, true, None, GoForward),
                binding("up", false, true, None, GoParent),
                // Alt+Shift+T cycles the theme. It must precede the Alt+T
                // binding below: that binding has `shift: None` (matches both
                // shift states) and the table is first-match-wins, so plain
                // Alt+T still reaches EditTerminalCommand unchanged.
                binding("t", false, true, Some(true), ToggleTheme),
                // Alt+T opens the terminal-command dialog. Ctrl+Alt+T is NOT
                // usable here: `binding_matches` ignores `alt` for Ctrl
                // bindings, so it would resolve to Ctrl+T (NewTab) instead.
                binding("t", false, true, None, EditTerminalCommand),
                // Alt+K opens the keymap dialog. A Ctrl+Alt+K variant would be
                // unsafe here: `binding_matches` ignores `alt` for Ctrl, so a
                // user-added Ctrl+K override would also fire it. Plain Alt+K
                // has no Ctrl counterpart in the default table (F6 finding 6).
                binding("k", false, true, None, EditKeymap),
            ],
        }
    }

    /// Rebuild the table from the persisted keymap overrides. Every valid
    /// entry is applied through `rebind`, so eviction and precedence are the
    /// same as for a live rebind. Malformed chords and action names that are
    /// not in the table are ignored; already-validated settings keep a list
    /// small enough that this is cheap at startup.
    fn from_settings(settings: &Settings) -> Self {
        let mut keymap = Self::default_keymap();
        for (action_name, chord) in &settings.keymap {
            let Some((key, ctrl, shift, alt)) = parse_chord(chord) else {
                continue;
            };
            // Deserialize through serde so the persisted spelling cannot drift
            // from the enum's `snake_case` names (e.g. `fuzzy_jump`).
            let name = serde_json::Value::String(action_name.trim().to_string());
            let Ok(action) = serde_json::from_value::<KeyAction>(name) else {
                continue;
            };
            // A chord without `shift+` means "no shift", matching how the
            // settings format spells out every modifier it wants.
            keymap.rebind(action, &key, ctrl, shift.unwrap_or(false), alt);
        }
        keymap
    }

    /// Pure lookup used by `on_key_down`. Unknown keys intentionally resolve to
    /// `None` so the old "unbound key is a no-op" behaviour is preserved.
    fn resolve(&self, key: &str, ctrl: bool, shift: bool, alt: bool) -> KeyAction {
        let found = self
            .bindings
            .iter()
            .find(|binding| Self::binding_matches(binding, key, ctrl, alt, Some(shift)));
        found.map_or(KeyAction::None, |binding| binding.action)
    }

    /// Give `action` a new keystroke, evicting any other binding that would
    /// claim the same keystroke. Returns `false` for an action that is not in
    /// the table, for a reserved dialog key, or for a reserved action: moving
    /// OpenSettings would evict the shipped Ctrl+, that the F1 help and
    /// `STATUS_SETTINGS_DISCARDED` advertise (L01). `from_settings` applies
    /// saved overrides through this method; a future settings panel can do the
    /// same.
    fn rebind(&mut self, action: KeyAction, key: &str, ctrl: bool, shift: bool, alt: bool) -> bool {
        if action == KeyAction::OpenSettings
            || crate::settings::is_reserved_chord(key)
            || !self.bindings.iter().any(|b| b.action == action)
        {
            return false;
        }
        // Drop the action's old binding as well: a rebind moves the action,
        // it does not add a second keystroke for it.
        self.bindings.retain(|binding| binding.action != action);
        self.bindings
            .retain(|binding| !Self::binding_matches(binding, key, ctrl, alt, Some(shift)));
        self.bindings.push(KeyBinding {
            key: key.to_string(),
            ctrl,
            alt,
            shift: Some(shift),
            action,
        });
        true
    }

    /// Restore one action to its shipped keystroke (used by a "reset" button).
    /// See `rebind` for why this is allowed to be unused in the binary today.
    #[allow(dead_code)]
    fn reset(&mut self, action: KeyAction) {
        let defaults = Self::default_keymap();
        if let Some(default) = defaults.bindings.iter().find(|b| b.action == action) {
            let default = default.clone();
            self.bindings.retain(|b| b.action != action);
            self.bindings.retain(|b| {
                !Self::binding_matches(b, &default.key, default.ctrl, default.alt, default.shift)
            });
            self.bindings.push(default);
        }
    }
}

/// Render a binding in the chord spelling accepted by the keymap dialog
/// (e.g. `ctrl+shift+p`, `f4`, `alt+k`). Pure so the help overlay and tests
/// share one representation of a live binding.
fn format_chord(binding: &KeyBinding) -> String {
    let mut parts: Vec<&str> = Vec::with_capacity(4);
    if binding.ctrl {
        parts.push("ctrl");
    }
    if binding.alt {
        parts.push("alt");
    }
    if binding.shift == Some(true) {
        parts.push("shift");
    }
    parts.push(binding.key.as_str());
    parts.join("+")
}

/// Snake-case name of an action. Reuses the serde names the Alt+K dialog
/// accepts, so the help notice cannot drift from the input syntax.
fn keymap_action_name(action: KeyAction) -> String {
    serde_json::to_value(action)
        .ok()
        .and_then(|value| value.as_str().map(str::to_string))
        .unwrap_or_default()
}

/// Reverse lookup: the chord currently bound to `action`, if the action is
/// still present in the table. This is the pure helper used by the F1 help
/// override notice and by its regression test.
fn chord_for_action(keymap: &KeyMap, action: KeyAction) -> Option<String> {
    keymap
        .bindings
        .iter()
        .find(|binding| binding.action == action)
        .map(format_chord)
}

/// `(action, chord)` pairs for bindings that are not exactly a shipped
/// default. `HELP_SECTIONS` is static, so this supplemental list is what keeps
/// F1 help truthful after an Alt+K rebind.
fn active_keymap_overrides(keymap: &KeyMap) -> Vec<(String, String)> {
    let defaults = KeyMap::default_keymap();
    let mut overrides: Vec<(String, String)> = keymap
        .bindings
        .iter()
        .filter(|binding| !defaults.bindings.contains(*binding))
        .filter_map(|binding| {
            chord_for_action(keymap, binding.action)
                .map(|chord| (keymap_action_name(binding.action), chord))
        })
        .collect();
    // P07: compare *default chords*, not merely actions. A rebind moves the
    // action to a new key and can leave one of its shipped chords unclaimed
    // (e.g. rebind `open_terminal` to space: F4 is now unassigned although the
    // action itself is still bound at space). A chord counts as claimed when a
    // current binding matches its key/modifiers, using the same wildcard
    // semantics as key dispatch. Report each lost chord explicitly so F1 help
    // stays truthful about what users can actually press.
    for default in &defaults.bindings {
        let claimed = keymap.bindings.iter().any(|binding| {
            // R01: the chord must still be held by the SAME action. A rebind
            // can move another action onto this key (e.g. open_terminal onto
            // space, evicting open_preview's space binding): the chord is then
            // still claimed, but no longer by its default action, so the
            // displaced default must be reported as unassigned.
            binding.action == default.action
                && KeyMap::binding_matches(
                    binding,
                    &default.key,
                    default.ctrl,
                    default.alt,
                    default.shift,
                )
        });
        if !claimed {
            let chord = format_chord(default);
            overrides.push((
                keymap_action_name(default.action),
                format!("{chord} → 未割り当て"),
            ));
        }
    }
    overrides.sort();
    overrides.dedup();
    overrides
}

/// One-line F1 notice listing live overrides. `None` for the shipped default
/// keymap, so the ordinary help overlay is unchanged in that case.
fn help_overrides_line(keymap: &KeyMap) -> Option<String> {
    let overrides = active_keymap_overrides(keymap);
    if overrides.is_empty() {
        return None;
    }
    let entries = overrides
        .iter()
        .map(|(action, chord)| format!("{action} → {chord}"))
        .collect::<Vec<_>>()
        .join(" / ");
    Some(format!("現在のキー設定（上書き）: {entries}"))
}

// ------------------------------------------------------------- F6-51 fuzzy jump

/// Where a fuzzy-jump candidate came from. The order also acts as the ranking
/// tie-break priority (earlier variants win an equal score).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum FuzzySource {
    Tab,
    Bookmark,
    Recent,
    Ancestor,
    Entry,
}

impl FuzzySource {
    fn label(self) -> &'static str {
        match self {
            Self::Tab => "タブ",
            Self::Bookmark => "ブックマーク",
            Self::Recent => "最近",
            Self::Ancestor => "階層",
            Self::Entry => "ファイル",
        }
    }
}

/// A path offered by the Ctrl+P overlay, together with its display name.
#[derive(Clone, Debug, PartialEq, Eq)]
struct FuzzyCandidate {
    path: PathBuf,
    label: String,
    source: FuzzySource,
}

const FUZZY_MAX_RESULTS: usize = 12;
const FUZZY_MAX_ANCESTORS: usize = 8;

/// Case-insensitive subsequence score. `None` means the query is not a
/// subsequence at all; higher scores mean tighter, earlier matches. An exact
/// substring always outranks a scattered subsequence. GPUI-free by design so
/// the ranking can be regression-tested headlessly (F6-51).
fn fuzzy_score(candidate: &str, query: &str) -> Option<i32> {
    let query = query.trim();
    if query.is_empty() {
        return Some(0);
    }
    let haystack = candidate.to_lowercase();
    let needle = query.to_lowercase();
    if let Some(position) = haystack.find(&needle) {
        return Some(100 + needle.chars().count() as i32 * 4 - (position.min(40) / 4) as i32);
    }
    let chars: Vec<char> = haystack.chars().collect();
    let needle_chars: Vec<char> = needle.chars().collect();
    let mut matched = 0usize;
    let mut last: Option<usize> = None;
    let mut score = 0i32;
    for (index, ch) in chars.iter().enumerate() {
        if matched == needle_chars.len() {
            break;
        }
        if *ch != needle_chars[matched] {
            continue;
        }
        score += 10;
        if last == Some(index.saturating_sub(1)) {
            score += 8; // consecutive characters
        }
        if index == 0 || !chars[index - 1].is_alphanumeric() {
            score += 6; // start of a word or an extension
        }
        score -= (index.min(40) / 4) as i32; // earlier matches are better
        last = Some(index);
        matched += 1;
    }
    if matched == needle_chars.len() {
        Some(score)
    } else {
        None
    }
}

/// Build the candidate list from already-cached state only: tabs, bookmarks,
/// recent directories, ancestors of the current directory and the visible
/// entries. No filesystem I/O, so this is a pure, testable projection. The
/// first occurrence of a path wins, giving the priority order its meaning.
fn fuzzy_candidates(
    cwd: &Path,
    tabs: &[PathBuf],
    bookmarks: &[PathBuf],
    recent: &[PathBuf],
    entries: &[Entry],
    labels: &[(PathBuf, String)],
) -> Vec<FuzzyCandidate> {
    let label_for = |path: &Path| -> Option<&str> {
        labels
            .iter()
            .find(|(candidate, _)| candidate == path)
            .map(|(_, label)| label.as_str())
    };
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    let mut push = |path: PathBuf, source: FuzzySource| {
        if path.as_os_str().is_empty() || !seen.insert(path.clone()) {
            return;
        }
        let base = tab_label(&path);
        let label = labeled_display_name(&base, label_for(&path));
        out.push(FuzzyCandidate {
            path,
            label,
            source,
        });
    };
    for path in tabs {
        push(path.clone(), FuzzySource::Tab);
    }
    for path in bookmarks {
        push(path.clone(), FuzzySource::Bookmark);
    }
    for path in recent {
        push(path.clone(), FuzzySource::Recent);
    }
    let mut ancestor = Some(cwd.to_path_buf());
    for _ in 0..FUZZY_MAX_ANCESTORS {
        let Some(path) = ancestor else { break };
        if path.as_os_str().is_empty() {
            break;
        }
        let next = parent_dir(&path);
        let stop = next.as_ref().is_none_or(|parent| parent == &path);
        push(path, FuzzySource::Ancestor);
        if stop {
            break;
        }
        ancestor = next;
    }
    for entry in entries {
        push(entry.path.clone(), FuzzySource::Entry);
    }
    out
}

/// Indices of the best `limit` candidates for `query`. Empty queries keep the
/// source priority order (tabs first) so the overlay is useful even before the
/// user types anything. Ties are broken by source, then insertion order.
fn rank_fuzzy(candidates: &[FuzzyCandidate], query: &str, limit: usize) -> Vec<usize> {
    let mut scored: Vec<(i32, FuzzySource, usize)> = Vec::new();
    for (index, candidate) in candidates.iter().enumerate() {
        let label = fuzzy_score(&candidate.label, query);
        let path = fuzzy_score(&candidate.path.to_string_lossy(), query);
        let score = match (label, path) {
            (Some(a), Some(b)) => a.max(b),
            (Some(a), None) => a,
            (None, Some(b)) => b,
            (None, None) => continue,
        };
        scored.push((score, candidate.source, index));
    }
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));
    scored
        .into_iter()
        .take(limit)
        .map(|(_, _, index)| index)
        .collect()
}

/// Which listing the window currently shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ViewSource {
    Directory,
    Trash,
    Search,
}

/// Keyboard intent while the trash view owns the key events. Permanent
/// deletion is deliberately never returned: Delete only explains the limit
/// and points at the restore shortcut.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrashKeyAction {
    Restore,
    NoPurge,
    Ignore,
}

/// Map a keystroke to a trash-view action. `Enter` (and `Ctrl+Enter`) restore;
/// `R` is an explicit restore alias; `Delete` reports that permanent deletion
/// is unsupported. Pure so the keyboard wiring can be regression-tested.
fn trash_key_action(key: &str, control: bool) -> TrashKeyAction {
    if control {
        return if key == "enter" {
            TrashKeyAction::Restore
        } else {
            TrashKeyAction::Ignore
        };
    }
    match key {
        "enter" | "r" => TrashKeyAction::Restore,
        "delete" => TrashKeyAction::NoPurge,
        _ => TrashKeyAction::Ignore,
    }
}

/// Status shown for Delete in the trash view: the unsupported action plus the
/// keyboard path that actually works (F6-33).
fn trash_delete_status() -> String {
    format!("{STATUS_TRASH_NO_PURGE}（Enter / R で元の場所へ復元）")
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
        // U-20: an error overlay with no next action leaves the user stuck.
        // AC07: the recovery chord depends on the view — Ctrl+L is only useful
        // in the Directory view, while Trash/Search offer Esc instead.
        let hint = match view {
            ViewSource::Directory => LOAD_ERROR_HINT_DIRECTORY,
            ViewSource::Trash => LOAD_ERROR_HINT_TRASH,
            ViewSource::Search => LOAD_ERROR_HINT_SEARCH,
        };
        // U06: some load errors (notably the trash path) already carry
        // STATUS_ERROR_PREFIX at their source. Route through the shared
        // helper so the overlay never prints "エラー: エラー: …".
        return Some((load_error_display_text(error), Some(hint.to_string())));
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
        return Some(match view {
            ViewSource::Search => (
                EMPTY_FILTERED.0.to_string(),
                Some(EMPTY_FILTERED.1.to_string()),
            ),
            // V13: `toggle_hidden` rejects Ctrl+H in Trash, so the generic
            // hint would suggest a chord that cannot work here.
            ViewSource::Trash => (crate::strings::empty_hidden_only_trash(all_entries), None),
            ViewSource::Directory => (crate::strings::empty_hidden_only(all_entries), None),
        });
    }
    None
}

use crate::strings::{
    DIALOG_FOOTER_HINT, EMPTY_DIRECTORY, EMPTY_FILTERED, EMPTY_SEARCH, EMPTY_TRASH,
    ERR_FOLDER_CREATE, FUZZY_JUMP_LABEL, HELP_SECTIONS, KEYMAP_FORMAT_HINT, KEYMAP_INPUT_HINT,
    LABEL_CANCEL, LOADING, LOAD_ERROR_HINT_DIRECTORY, LOAD_ERROR_HINT_SEARCH,
    LOAD_ERROR_HINT_TRASH, OP_DIRS_SUFFIX, OP_DONE_SUFFIX, OP_FILES_SUFFIX, OP_SKIPPED_SUFFIX,
    PARENT_FOLDER_LABEL, STATUS_ACCESS_DENIED, STATUS_BOOKMARKS_ONLY_DIRECTORY,
    STATUS_BOOKMARK_LABEL_NOT_SET_PREFIX, STATUS_BOOKMARK_LABEL_REASON_INVALID_LABEL,
    STATUS_BOOKMARK_LABEL_REASON_INVALID_PATH, STATUS_BOOKMARK_LABEL_REASON_LIMIT,
    STATUS_BOOKMARK_LABEL_SET_FAILED_PREFIX, STATUS_BOOKMARK_NOT_PERSISTABLE, STATUS_CANCELLED,
    STATUS_CANCELLING, STATUS_CANCEL_HINT, STATUS_CANNOT_OPEN_SUFFIX, STATUS_CHANGED,
    STATUS_DROP_EMPTY, STATUS_DROP_NOT_FOUND, STATUS_DROP_NO_DESTINATION,
    STATUS_DROP_ONLY_DIRECTORY, STATUS_DROP_SAME_DIRECTORY, STATUS_ERROR_PREFIX,
    STATUS_FILE_NO_NEW_TAB_PREFIX, STATUS_FOLDER_CREATED, STATUS_FOLDER_CREATING,
    STATUS_FOLDER_NO_PREVIEW, STATUS_FUZZY_JUMP_ONLY_DIRECTORY, STATUS_INVALID_CLI_DIR,
    STATUS_KEYMAP_FORMAT_INVALID, STATUS_KEYMAP_INVALID_PREFIX, STATUS_KEYMAP_LIMIT_PREFIX,
    STATUS_KEYMAP_LIMIT_SUFFIX, STATUS_KEYMAP_NO_UNBIND_PREFIX, STATUS_LAST_TAB,
    STATUS_LOCATION_ONLY_DIRECTORY, STATUS_MAX_TABS_PREFIX, STATUS_MAX_TABS_SUFFIX,
    STATUS_MISSING_COUNT_SUFFIX, STATUS_NOT_DIRECTORY, STATUS_NOT_FOUND_SUFFIX,
    STATUS_NO_CLIPBOARD, STATUS_NO_SELECTION, STATUS_OPENED_DEFAULT_SUFFIX,
    STATUS_OP_FAILED_SUFFIX, STATUS_OP_IN_PROGRESS_SUFFIX, STATUS_OP_RUNNING,
    STATUS_PARTIAL_FAILURE_SUFFIX, STATUS_PATH_COPIED, STATUS_PATH_GONE, STATUS_PLACE_GONE_HINT,
    STATUS_PREVIEW_ONLY_DIRECTORY, STATUS_PRIORITY, STATUS_PROPERTIES_ONLY_DIRECTORY,
    STATUS_REDO_EMPTY, STATUS_REDO_FAILED, STATUS_REDO_NO_TARGET, STATUS_RENAME_EXISTS_PREFIX,
    STATUS_RENAME_FAILED, STATUS_RENAME_NO_PARENT, STATUS_RESCAN_FAILED, STATUS_RESTORE_EMPTY,
    STATUS_RESTORE_FAILED, STATUS_REVEALED_PREFIX, STATUS_SAME_DIRECTORY_NO_MOVE,
    STATUS_SAVED_SEARCH_CAP_HINT, STATUS_SAVED_SEARCH_NAME_REQUIRED,
    STATUS_SAVED_SEARCH_QUERY_REQUIRED, STATUS_SAVED_SEARCH_SAVE_FAILED_PREFIX,
    STATUS_SEARCH_EMPTY_QUERY, STATUS_SEARCH_NOT_OPEN, STATUS_SEARCH_ONLY_DIRECTORY,
    STATUS_SEARCH_PROMPT, STATUS_SEARCH_STALE, STATUS_SETTINGS_DISCARDED,
    STATUS_SETTINGS_READ_ONLY, STATUS_SETTINGS_READ_ONLY_HINT, STATUS_SETTINGS_SAVE_FAILED,
    STATUS_SETTINGS_SAVE_LOCKED, STATUS_SETTINGS_SAVE_TOO_LARGE, STATUS_SKIPPED_PREFIX,
    STATUS_SKIPPED_SUFFIX, STATUS_SKIP_REASON_MISSING, STATUS_SKIP_REASON_SAME_DIRECTORY,
    STATUS_SKIP_REASON_SPECIAL, STATUS_SPECIAL_FILE_NO_COPY, STATUS_SYMLINK_NO_PREVIEW,
    STATUS_TABS_ONLY_DIRECTORY, STATUS_TAB_MOVED_LEFT, STATUS_TAB_MOVED_RIGHT, STATUS_TAB_RESTORED,
    STATUS_TAB_RESTORE_EMPTY, STATUS_TARGET_GONE, STATUS_TARGET_MISSING,
    STATUS_TERMINAL_COMMAND_INVALID, STATUS_TERMINAL_DISABLED_HINT, STATUS_TERMINAL_START_PREFIX,
    STATUS_THEME_CHANGED_PREFIX, STATUS_TRASH_ALL_FAILED, STATUS_TRASH_LOADING,
    STATUS_TRASH_LOAD_FAILED_PREFIX, STATUS_TRASH_MISSING_REASON, STATUS_TRASH_NO_AUTO_REFRESH,
    STATUS_TRASH_NO_COPY, STATUS_TRASH_NO_CUT, STATUS_TRASH_NO_HIDDEN, STATUS_TRASH_NO_PASTE,
    STATUS_TRASH_NO_PURGE, STATUS_TRASH_NO_RENAME, STATUS_TRASH_NO_SORT,
    STATUS_TRASH_NO_TERMINAL_PREFIX, STATUS_TRASH_PARTIAL_FAILED, STATUS_UNDO_EMPTY,
    STATUS_UNDO_EXISTS_SUFFIX, STATUS_UNDO_FAILED, STATUS_UNDO_HISTORY_HINT,
    STATUS_UNDO_MISSING_SUFFIX, STATUS_WATCH_COUNT_SUFFIX, STATUS_WATCH_DISABLED,
    STATUS_WATCH_INIT_FAILED, STATUS_WATCH_PARTIAL, STATUS_WATCH_PREFIX, THEME_LABEL_DARK,
    THEME_LABEL_LIGHT, THEME_LABEL_SYSTEM,
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

/// Pure: discard a dialog's stale validation error when its input changes.
/// Returns `true` only when an error was present, so callers notify once.
fn clear_dialog_error(error: &mut Option<String>) -> bool {
    error.take().is_some()
}

/// Step one settings page by `delta`, wrapping at both ends (G08). Kept pure
/// so the Ctrl+Tab / Ctrl+Shift+Tab cycle is regression-testable headlessly.
fn settings_tab_step(tab: SettingsTab, delta: isize) -> SettingsTab {
    let count = SettingsTab::ALL.len() as isize;
    let next = (tab.index() as isize + delta).rem_euclid(count) as usize;
    SettingsTab::from_index(next)
}

/// Pure: map an Alt+digit pressed inside the settings overlay straight to its
/// page (K07). `1`..`5` follow [`SettingsTab::ALL`]; every other key returns
/// `None`, so callers can try it unconditionally without swallowing typing.
fn settings_tab_from_digit_key(key: &str) -> Option<SettingsTab> {
    if key.len() != 1 {
        return None;
    }
    let digit = key.as_bytes()[0];
    if !(b'1'..=b'5').contains(&digit) {
        return None;
    }
    SettingsTab::ALL.get((digit - b'1') as usize).copied()
}

/// Pure: next value of the Files-tab sort radio set (K08). The order matches
/// the `sort_options` passed to `render_settings_content`; an unknown value
/// starts at the first entry so Alt+S always makes visible progress.
fn settings_sort_next(choice: &str) -> &'static str {
    const ORDER: [&str; 4] = ["name", "kind", "size", "modified"];
    match ORDER.iter().position(|value| *value == choice) {
        Some(index) => ORDER[(index + 1) % ORDER.len()],
        None => ORDER[0],
    }
}

/// Whether a keystroke is bare, so a settings row shortcut may claim it (N01).
/// `settings_row_shortcut` only sees the trimmed `key` string and documents
/// that modified chords are intentionally not recognised; the host must add
/// this guard or Ctrl+T / Alt+T / Shift+T / Fn+T would also change the theme and
/// clobber global shortcuts. All five `Modifiers` fields are checked, including
/// `function`. Kept pure so every modifier combination is regression-testable
/// without a GPUI window (the raw host event is not).
fn settings_row_key_is_bare(modifiers: &Modifiers) -> bool {
    !modifiers.control
        && !modifiers.alt
        && !modifiers.shift
        && !modifiers.platform
        && !modifiers.function
}

/// Inline rename dialog state. Dropping it closes the dialog and drops the
/// input subscription.
struct RenameState {
    target: PathBuf,
    original: String,
    input: Entity<InputState>,
    /// Keeps the `InputEvent::Change` / `PressEnter` subscription alive while open.
    _subscription: Subscription,
    error: Option<String>,
    /// Op token claimed by `confirm_rename`, if this dialog submitted a rename
    /// that is still running. Esc only cancels an op this dialog actually owns.
    op: Option<OpToken>,
}

/// Inline bookmark-label editor (F6-49). Dropping it closes the dialog.
struct LabelState {
    path: PathBuf,
    input: Entity<InputState>,
    /// Keeps the `InputEvent::PressEnter` subscription alive while open.
    _subscription: Subscription,
}

/// Location bar (Ctrl+L): a path input with validation error.
struct LocationState {
    input: Entity<InputState>,
    /// Keeps the `InputEvent::Change` / `PressEnter` subscription alive while open.
    _subscription: Subscription,
    error: Option<String>,
}

/// Terminal-command settings dialog (Alt+T). Dropping it closes the dialog
/// and drops the input subscription.
struct TerminalCommandState {
    input: Entity<InputState>,
    /// Keeps the `InputEvent::Change` / `PressEnter` subscription alive while open.
    _subscription: Subscription,
    error: Option<String>,
}

/// Keymap settings dialog (Alt+K). Dropping it closes the dialog and drops
/// the input subscription.
struct KeymapEditorState {
    input: Entity<InputState>,
    /// Keeps the `InputEvent::Change` / `PressEnter` subscription alive while open.
    _subscription: Subscription,
    error: Option<String>,
}

/// Fuzzy jump overlay (Ctrl+P): a query input plus the ranked candidate list.
/// Dropping it closes the dialog and drops its input subscriptions.
struct FuzzyState {
    input: Entity<InputState>,
    /// Keeps `InputEvent::Change` / `PressEnter` subscriptions alive.
    _subscription: Subscription,
    candidates: Vec<FuzzyCandidate>,
    /// Indices into `candidates`, ranked for the current query.
    results: Vec<usize>,
    /// Cursor into `results` (not `candidates`), moved by Up / Down.
    selected: usize,
}

/// Read-only text preview (Space / F3).
struct PreviewState {
    /// File currently shown; the source of truth for next/prev navigation.
    path: PathBuf,
    /// Monotonic serial of this preview, used to ignore stale read results.
    serial: u64,
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
    /// Name field of the saved-search bar (F6-49).
    save_name: Option<Entity<InputState>>,
    /// Keeps the save-name field's `PressEnter` subscription alive while open.
    /// It is bound to this input only, so Enter here saves the search while
    /// Enter in the query field still triggers `run_search` (G06).
    save_name_subscription: Option<Subscription>,
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
#[cfg_attr(not(debug_assertions), allow(dead_code))]
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

/// Build an error load state on a **fresh** generation.
///
/// Errors raised outside a reload callback (bad navigation path, mkdir
/// failure, trash failure) must advance `generation` before being stored:
/// the reload callback only applies when its captured generation still equals
/// `load.generation()`. Reusing the current generation lets a reload that was
/// already in flight pass that guard and replace the error with `Ready` plus
/// stale entries (concurrency finding 1). Kept pure so the rule is unit-testable
/// without a live GPUI `Context`.
fn error_state_with_fresh_generation(generation: &mut u64, error: String) -> LoadState {
    *generation = generation.wrapping_add(1);
    LoadState::Error {
        generation: *generation,
        error,
    }
}

/// Whether a successful background reconcile may clear a stale error overlay.
///
/// Only `Error` qualifies. `Loading` must be left alone: an explicit reload
/// may have been dispatched after the reconcile read started, and its
/// in-flight state is newer than the reconcile result.
fn should_clear_error_on_reconcile(load: &LoadState) -> bool {
    matches!(load, LoadState::Error { .. })
}

/// Whether a successful reconcile must (re-)apply the freshly read listing.
///
/// A changed fingerprint obviously needs a reload. A stale error overlay needs
/// one too: the error path clears `entries`, so merely flipping `load` back to
/// `Ready` would leave an empty list on screen (regression this helper guards).
/// Kept pure so the recovery decision is unit-testable — the full state machine
/// needs a live GPUI `Context` and cannot run in a unit test.
fn should_apply_loaded_on_reconcile(changed: bool, clear_error: bool) -> bool {
    changed || clear_error
}

/// Whether an in-flight directory reconcile may apply its freshly read listing.
///
/// `reconcile` is dispatched only from the directory view, but the user can
/// open search (Ctrl+F) while its background read is still in flight. Search
/// completion rewrites `fingerprint`, so the generation/loading guards alone
/// would let a stale reconcile callback pass and replace the search results
/// with directory entries while the search bar stays open. The view must still
/// be `Directory` when the result lands. `Trash` is rejected for the same
/// reason (its listing is owned by the trash view).
fn should_apply_reconcile(view: ViewSource, generation_ok: bool, loading: bool) -> bool {
    view == ViewSource::Directory && generation_ok && !loading
}

/// Whether the 10s timer may dispatch a new directory reconcile.
///
/// A slow `read_entries` can outlive the timer interval. Starting another read
/// would accumulate an extra `Vec<Entry>` snapshot and background task; the
/// generation guard only prevents a stale result from being *applied*, not the
/// wasted concurrent read. `in_flight` is set when a read is dispatched and
/// cleared on every completion path (success or error). Kept pure so the decision
/// is unit-testable without a live GPUI `Context`.
fn should_start_reconcile(in_flight: bool, loading: bool, view: ViewSource) -> bool {
    !in_flight && !loading && view == ViewSource::Directory
}

/// Whether a finished properties metadata read may still be applied.
///
/// `props_serial` is bumped on every open and close, so a callback from a
/// superseded read must be discarded before it writes status, triggers a reload,
/// or (re)installs the dialog with stale content. Kept pure for a focused test.
fn should_apply_properties_read(current_serial: u64, read_serial: u64) -> bool {
    current_serial == read_serial
}

/// Identity of one background operation.
///
/// Every long-running or destructive action (transfer, trash, undo, redo,
/// trash-view restore, rename) claims the shared sentinel through
/// [`Explorer::begin_op`] and keeps a clone of this token. Completion handlers
/// call [`Explorer::finish_op`], which clears the sentinel only while `id`
/// still matches, so a late callback can never cancel a newer operation.
#[derive(Clone, Debug)]
struct OpToken {
    id: u64,
    cancel: Arc<CancelFlag>,
}

#[cfg(test)]
impl OpToken {
    fn is_cancelled(&self) -> bool {
        self.cancel.is_cancelled()
    }

    fn cancel(&self) {
        self.cancel.cancel();
    }
}

/// Install a fresh cancellation flag and bump the *monotonic* operation id.
/// Kept as a free function so the stale-completion rule is unit-testable
/// without a GPUI context.
fn install_op_token(id: &mut u64, cancel: &mut Option<Arc<CancelFlag>>) -> OpToken {
    *id = id.wrapping_add(1);
    let flag = Arc::new(CancelFlag::new());
    *cancel = Some(flag.clone());
    OpToken {
        id: *id,
        cancel: flag,
    }
}

/// Clear the sentinel only when `token` still identifies the current op.
///
/// The id is deliberately **not** reset to zero: a stale completion carrying
/// the old id must stay distinguishable from a freshly started operation.
fn clear_op_token(current: u64, cancel: &mut Option<Arc<CancelFlag>>, token: &OpToken) -> bool {
    if current != token.id {
        return false;
    }
    *cancel = None;
    true
}

/// Whether `token` still identifies an *active* sentinel owner.
///
/// `clear_op_token` deliberately keeps the monotonic id, so the id alone also
/// matches an operation that has already settled. Requiring the live cancel
/// flag stops a queued progress event from repainting "…中…" (and restoring
/// `op_progress`) after the final status was applied. A newly started op
/// installs a fresh id *and* flag, so a legitimate current transfer still
/// passes while an old token never does.
fn op_is_active(current_id: u64, cancel: &Option<Arc<CancelFlag>>, token: &OpToken) -> bool {
    cancel.is_some() && current_id == token.id
}

/// Move `paths` to the system trash one by one, honouring Esc between items.
///
/// Blocking: call from the background executor only. Returns the successfully
/// trashed paths, the failures and whether cancellation was observed.
fn trash_batch(
    paths: Vec<PathBuf>,
    cancel: &CancelFlag,
) -> (Vec<PathBuf>, Vec<(PathBuf, String)>, bool) {
    let mut trashed = Vec::new();
    let mut failures = Vec::new();
    for path in paths {
        if cancel.is_cancelled() {
            return (trashed, failures, true);
        }
        match trash_view::delete(&path) {
            Ok(()) => trashed.push(path),
            Err(error) => failures.push((path, error.to_string())),
        }
    }
    (trashed, failures, cancel.is_cancelled())
}

/// Metadata collected for the read-only properties dialog.
///
/// All fields are plain data so the blocking reads can happen on the background
/// executor and only the finished snapshot is applied on the UI thread.
#[derive(Debug)]
struct PropertiesSnapshot {
    is_symlink: bool,
    is_dir: bool,
    size: Option<u64>,
    modified: Option<SystemTime>,
    permissions: Option<String>,
    symlink_target: Option<PathBuf>,
}

/// Read a file's metadata for the properties dialog.
///
/// Blocking: call from the background executor only, never from render. The
/// order and fallbacks mirror the previous inline implementation exactly.
fn read_properties_snapshot(path: &Path) -> Result<PropertiesSnapshot, FsError> {
    // `symlink_metadata` sees broken links too and never follows them. Keep the
    // io error so the caller can distinguish "gone" from "denied" and other
    // failures instead of always blaming a vanished path.
    let link_metadata = std::fs::symlink_metadata(path)
        .map_err(|err| FsError::from_io(path.to_path_buf(), &err))?;
    let is_symlink = link_metadata.file_type().is_symlink();
    let metadata = std::fs::metadata(path).ok().or(Some(link_metadata.clone()));
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
        std::fs::read_link(path).ok()
    } else {
        None
    };
    Ok(PropertiesSnapshot {
        is_symlink,
        is_dir: link_metadata.is_dir(),
        size,
        modified,
        permissions,
        symlink_target,
    })
}

/// Pure mapping from a failed properties metadata read to the UI reaction.
/// Only `NotFound` reloads the listing; a permission error and every other
/// failure keep the current view and show a distinct status.
#[derive(Debug, PartialEq, Eq)]
enum PropertiesErrorReaction {
    PathGone,
    AccessDenied,
    Other,
}

impl PropertiesErrorReaction {
    fn for_error(error: &FsError) -> Self {
        match error {
            FsError::Fs { kind, .. } | FsError::FsCause { kind, .. } => match kind {
                FsErrorKind::NotFound => Self::PathGone,
                FsErrorKind::PermissionDenied => Self::AccessDenied,
                _ => Self::Other,
            },
            _ => Self::Other,
        }
    }

    /// `(status text, whether to reload)` — pure so it can be tested directly.
    fn apply(self, path: &Path, error: &FsError) -> (String, bool) {
        match self {
            Self::PathGone => (format!("{STATUS_PATH_GONE}: {}", display_path(path)), true),
            // R02: 共有の STATUS_ACCESS_DENIED（strings.rs）を色分類表と同じく参照する。
            Self::AccessDenied => (
                format!("{STATUS_ACCESS_DENIED}: {}", display_path(path)),
                false,
            ),
            Self::Other => (
                format!("{STATUS_ERROR_PREFIX}{}", error.message_ja()),
                false,
            ),
        }
    }
}

/// Pure snapshot-mapping half of [`record_trashed_paths`].
///
/// Kept separate from the blocking trash listing so the byte-exact id mapping
/// used by undo bookkeeping can be tested with synthetic non-UTF-8 paths: the
/// real trash directory cannot be seeded deterministically in a unit test.
fn trashed_ids_from_snapshot(
    items: &[trash_view::TrashItem],
    trashed: &[PathBuf],
) -> Vec<(PathBuf, OsString)> {
    trash_view::record_ids(items, trashed)
}

/// Resolve the trash ids of freshly trashed paths from a previously read
/// listing. Blocking: call from the background executor only. An empty result
/// (including a failed listing) leaves the undo entry empty, as before (F6-15).
fn record_trashed_paths(trashed: &[PathBuf]) -> Vec<(PathBuf, OsString)> {
    trash_view::list()
        .map(|items| trashed_ids_from_snapshot(&items, trashed))
        .unwrap_or_default()
}

/// Paths carried by an internal row drag that may be copied into
/// `destination`. The whole multi-selection is preserved (order-stable), the
/// destination itself and duplicate paths are dropped. Pure so the multi-drop
/// rule can be regression-tested without a running GPUI window.
fn internal_drop_paths(paths: &[PathBuf], destination: &Path) -> Vec<PathBuf> {
    let mut kept: Vec<PathBuf> = Vec::with_capacity(paths.len());
    for path in paths {
        if path == destination || kept.contains(path) {
            continue;
        }
        kept.push(path.clone());
    }
    kept
}

/// Decide where a pending post-creation rename may start (F6-34).
///
/// Pure so the guard can be regression-tested: the listing must be loaded and
/// idle, we must be in the directory view with no dialog already owning the
/// keyboard, and the freshly created folder must actually be visible.
fn pending_rename_index(
    loading: bool,
    renaming: bool,
    directory_view: bool,
    entries: &[Entry],
    path: &Path,
) -> Option<usize> {
    if loading || renaming || !directory_view {
        return None;
    }
    entries.iter().position(|entry| entry.path == path)
}

struct Explorer {
    settings: Settings,
    /// Hand-drawn colors for the effective (resolved) theme.
    palette: AppPalette,
    /// Keeps the OS appearance observer alive; it only has an effect while
    /// `settings.theme_choice == ThemeChoice::System`.
    appearance_subscription: Option<Subscription>,
    cwd: PathBuf,
    /// Every entry read from the current directory.
    all_entries: Vec<Entry>,
    /// Filtered + sorted view rendered by `uniform_list`.
    entries: Vec<Entry>,
    /// Cached resolver for full-colour freedesktop system icons (list/grid).
    system_icon_resolver: SystemIconResolver,
    /// Focused row index (keyboard cursor / single-selection shortcut).
    selected: usize,
    /// Multi-selection keyed by path so it survives reloads and re-sorting.
    selection: Selection,
    /// True when the single selected path was auto-selected by a load/reload
    /// (keyboard ops need a target) rather than picked by the user. Only the
    /// status-bar label changes: the path stays a real selection for actions.
    implicit_selection: bool,
    /// Current listing source (real directory or the system trash).
    view: ViewSource,
    /// Trash items backing the virtual listing (empty in directory view).
    trash_items: Vec<trash_view::TrashItem>,
    /// Recursive search state (active only while `view == Search`).
    search: SearchState,
    /// Content-search line numbers keyed by path (display concern, not Entry).
    search_lines: std::collections::HashMap<PathBuf, u32>,
    history: History,
    focus_handle: FocusHandle,
    scroll_handle: UniformListScrollHandle,
    /// Current icon-grid column count, refreshed each render. Used to map an
    /// entry index to its list row when scrolling (F6-10).
    grid_column_count: usize,
    load: LoadState,
    status: String,
    /// Monotonic id of the latest load request; stale results are dropped.
    generation: u64,
    /// Fingerprint of `all_entries`, used by the 10s reconciliation timer.
    fingerprint: u64,
    /// Path to select once the next load finishes (new folder, etc.).
    pending_select: Option<PathBuf>,
    /// Path to select and immediately rename once the next load finishes
    /// (Ctrl+Shift+N creates a folder already primed for typing its name).
    pending_rename: Option<PathBuf>,
    /// Monotonic serial for previews; stale background reads are discarded.
    preview_serial: u64,
    /// Monotonic serial for the properties metadata read. Bumped on every
    /// open **and** close: a snapshot (or an error) from a superseded read is
    /// discarded before it can resurrect the dialog or trigger a late reload.
    props_serial: u64,
    /// Foreground waiter of the properties metadata read. Replaced on reopen
    /// and dropped on close, cancelling the previous read.
    props_read_task: Option<Task<()>>,
    /// True while a 10s reconciliation `read_entries` is in flight. Prevents
    /// the timer from starting an overlapping read (generation only guards
    /// stale applies, not wasted/concurrent reads). Cleared on every completion
    /// path, including errors.
    reconcile_in_flight: bool,
    /// Foreground waiter of the latest load. Dropping it cancels the wait.
    load_task: Option<Task<()>>,
    /// A watcher event landed while a reload was in flight. Its incremental
    /// edit cannot be applied to `all_entries` (the in-flight read may predate
    /// it and would be overwritten by `apply_loaded`), so the load re-issues a
    /// fresh reload once it completes (finding 2).
    needs_reload_after_load: bool,
    watcher: DirWatcher,
    /// Self-triggered FS operations suppress the watcher for a short window.
    suppress_until: Option<Instant>,
    /// Cached sidebar places (resolved once; see [`Place`]).
    places: Vec<Place>,
    /// Visible mount points (Linux: /proc/mounts). Refreshed on reload only.
    mounts: Vec<MountPoint>,
    /// Open tabs (always at least one); the active tab mirrors `cwd`/`history`.
    tabs: Tabs,
    /// Latest frame of this window, refreshed in `render` and persisted with
    /// the window-scoped session. Session state is per window, never global.
    window_bounds: Option<WindowBoundsState>,
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
    /// Monotonic id of the running operation; stale completions are ignored.
    op_token: u64,
    /// Byte progress of the running copy/move: (done, total). `None` when idle.
    op_progress: Option<(u64, u64)>,
    /// Inline rename dialog (F2 / context menu). `None` when closed.
    renaming: Option<RenameState>,
    /// Inline bookmark-label editor (F6-49). `None` when closed.
    label_editor: Option<LabelState>,
    /// Read-only properties dialog state.
    properties: Option<PropsState>,
    /// Whether the F1 keyboard help overlay is open.
    help_open: bool,
    /// Path input opened with Ctrl+L.
    location: Option<LocationState>,
    /// Terminal-command settings dialog (Alt+T). `None` when closed.
    terminal_command_editor: Option<TerminalCommandState>,
    /// Full preferences overlay (Ctrl+,). `None` when closed.
    settings_ui: Option<SettingsUiState>,
    /// Keymap settings dialog (Alt+K). `None` when closed.
    keymap_editor: Option<KeymapEditorState>,
    /// Personalized key map (F6-51). Defaults to the shipped bindings.
    keymap: KeyMap,
    /// Ctrl+P fuzzy jump overlay state. `None` when closed.
    fuzzy: Option<FuzzyState>,
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

/// The subset of development env hooks that mutate freshly loaded settings.
/// Keeping them in their own value lets the mutations be exercised without a
/// GPUI context and without touching the process environment.
#[derive(Clone, Copy, Default)]
#[cfg(any(debug_assertions, test))]
struct DebugSettingsOverrides {
    sort_kind: bool,
    recent: bool,
    bookmarks: bool,
    search_history: bool,
}

/// Apply the startup debug overrides to `settings`.
///
/// All environment access lives in `Explorer::new` behind
/// `#[cfg(debug_assertions)]`; this function is pure with respect to the
/// environment. Passing the default (every flag false) models the release
/// path exactly: not a single settings field is touched.
#[cfg(any(debug_assertions, test))]
fn apply_debug_settings_overrides(
    settings: &mut Settings,
    start_dir: &Path,
    overrides: DebugSettingsOverrides,
) {
    if overrides.sort_kind {
        settings.sort = SortSpec {
            key: SortKey::Kind,
            dir: crate::model::SortDir::Asc,
            dirs_first: true,
        };
    }
    if overrides.recent {
        settings.record_recent(start_dir);
        if let Some(home) = dirs::home_dir() {
            settings.record_recent(&home);
        }
        settings.record_recent(&PathBuf::from("/"));
    }
    if overrides.bookmarks {
        settings.bookmarks.clear();
        settings.toggle_bookmark(start_dir);
        if let Some(home) = dirs::home_dir() {
            settings.toggle_bookmark(&home);
        }
        settings.toggle_bookmark(&PathBuf::from("/"));
    }
    if overrides.search_history {
        settings.recent_searches = vec!["TODO".to_string(), "rs".to_string(), "config".to_string()];
    }
}

/// Pure guard for the Ctrl+D bookmark toggle. `Some(status)` means the path
/// cannot be persisted and the caller must return before touching
/// `toggle_bookmark` (and its `debug_assert_eq!`); `None` means toggle normally.
///
/// Kept separate from the GPUI method so every unpersistable-path rejection
/// (empty / NUL / non-UTF-8 / oversized) is regression-testable without a
/// running window (T04).
fn bookmark_toggle_rejection(path: &Path) -> Option<String> {
    if Settings::can_persist_path(path) {
        None
    } else {
        Some(STATUS_BOOKMARK_NOT_PERSISTABLE.to_string())
    }
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
        // Resolve the persisted preference once, using the appearance GPUI
        // observes for this window. `theme` stays the resolved mode so older
        // readers of the legacy field see the effective value.
        let effective_theme = resolve_theme(settings.theme_choice, cx.window_appearance().into());
        settings.theme = effective_theme;
        let palette = AppPalette::for_mode(effective_theme);
        // Any active debug hook must never write to the real settings file.
        // In release `debug` is `DebugHooks::default()`, so this remains false
        // and normal persistence behaviour is preserved.
        settings.ephemeral = debug.ephemeral;
        // Session restoration is not a debug hook and must run in every build.
        // Validate the CLI directory first so a missing path or a file argument
        // produces a diagnostic instead of being silently dropped by
        // `restore_session`'s `is_dir` filter.
        let (cli_dir, rejected_cli_dir) = match startup_dir() {
            Some(path) => match validate_cli_dir(path) {
                Ok(canonical) => (Some(canonical), None),
                Err(rejected) => (None, Some(rejected)),
            },
            None => (None, None),
        };
        if let Some(path) = &rejected_cli_dir {
            eprintln!(
                "gpui-file-explorer: {STATUS_INVALID_CLI_DIR}: {}",
                path.display()
            );
        }
        let (saved_tabs, start_dir, restored_active) = restore_session(
            &settings.tabs,
            settings.active_tab,
            settings.last_dir.as_deref(),
            cli_dir,
            dirs::home_dir(),
        );
        // The env-driven startup overrides are development-only. In release
        // the entire block is compiled out, so a leaked GPUI_FE_DEBUG_* value
        // can never clear bookmarks, seed recents or overwrite search history
        // in the user's real settings.json (F6-03).
        #[cfg(debug_assertions)]
        {
            apply_debug_settings_overrides(
                &mut settings,
                &start_dir,
                DebugSettingsOverrides {
                    sort_kind: std::env::var("GPUI_FE_DEBUG_SORT")
                        .is_ok_and(|value| value == "kind"),
                    recent: std::env::var_os("GPUI_FE_DEBUG_RECENT").is_some(),
                    bookmarks: std::env::var_os("GPUI_FE_DEBUG_BOOKMARKS").is_some(),
                    search_history: std::env::var_os("GPUI_FE_DEBUG_SEARCH_HISTORY").is_some(),
                },
            );
        }

        let (watch_tx, watch_rx) = async_channel::unbounded::<Vec<FsEvent>>();
        let watcher = DirWatcher::new(move |events| {
            let _ = watch_tx.try_send(events);
        });

        let places = build_places();
        let tabs = restore_tab_sessions(&settings, &saved_tabs, restored_active, &start_dir);
        let history = tabs.active().history.clone();
        // Seed from the saved frame so an early save (before the first render
        // observes the real window) cannot overwrite it with nothing.
        let restored_bounds = settings.primary_window().and_then(|window| window.bounds);
        // Apply persisted overrides before `settings` moves into `Self`.
        let keymap = KeyMap::from_settings(&settings);
        // Seed the icon theme order from the resolved startup mode so the first
        // render already uses breeze-dark on a dark theme.
        let mut system_icon_resolver = SystemIconResolver::new();
        system_icon_resolver.set_prefer_dark(effective_theme.is_dark());
        let mut this = Self {
            settings,
            palette,
            // The window-level subscription is installed in `open_window`,
            // where a `&mut Window` is available (`Context` alone has none).
            appearance_subscription: None,
            cwd: start_dir,
            all_entries: Vec::new(),
            entries: Vec::new(),
            system_icon_resolver,
            selected: 0,
            selection: Selection::default(),
            implicit_selection: true,
            view: ViewSource::Directory,
            trash_items: Vec::new(),
            search: SearchState::default(),
            search_lines: std::collections::HashMap::new(),
            history,
            focus_handle: cx.focus_handle(),
            scroll_handle: UniformListScrollHandle::new(),
            grid_column_count: 1,
            load: LoadState::Loading { generation: 0 },
            status: String::new(),
            generation: 0,
            fingerprint: 0,
            pending_select: None,
            pending_rename: None,
            preview_serial: 0,
            props_serial: 0,
            props_read_task: None,
            reconcile_in_flight: false,
            load_task: None,
            needs_reload_after_load: false,
            watcher,
            suppress_until: None,
            places,
            // Populated asynchronously by `refresh_mounts` via the initial
            // `reload` below; reading /proc/mounts here would block the UI.
            mounts: Vec::new(),
            tabs,
            window_bounds: restored_bounds,
            recent: Vec::new(),
            bookmarks: Vec::new(),
            clipboard: None,
            op_task: None,
            op_cancel: None,
            op_token: 0,
            op_progress: None,
            renaming: None,
            label_editor: None,
            properties: None,
            help_open: false,
            location: None,
            terminal_command_editor: None,
            settings_ui: None,
            keymap_editor: None,
            keymap,
            fuzzy: None,
            preview: None,
            undo: UndoStack::new(),
            undo_history_open: false,
            focus_list_on_next_render: false,
            debug,
        };

        // Remember the directory we actually started in (also proves the
        // settings file round-trips on the very first launch).
        this.settings.last_dir = Some(this.cwd.clone());
        // A first run has no persisted tabs, so nothing has populated 「最近」
        // yet. Seed it with the directory we actually started in.
        if this.settings.tabs.is_empty() {
            this.settings.record_recent(&this.cwd);
        }
        // U-22: collect every startup notice instead of assigning `this.status`
        // three times (the last assignment used to swallow the earlier ones).
        let mut startup_notices: Vec<String> = Vec::new();
        if !this.save_settings() {
            // V01: `save_settings` records why the write failed; show that
            // reason instead of a generic "check the file" notice.
            startup_notices.push(startup_save_failure_status(&this.settings));
        }
        if this.settings.load_error.is_some() {
            // W08: 起動時と書き込み時で同じ理由・復帰導線付きの通知を使う。
            startup_notices.push(settings_read_only_status(&this.settings));
        }
        if let Some(rejected) = rejected_cli_dir {
            startup_notices.push(format!(
                "{STATUS_ERROR_PREFIX}{STATUS_INVALID_CLI_DIR}: {}",
                display_path(&rejected)
            ));
        }
        this.status = join_status_notices(&startup_notices);

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
        self.refresh_mounts(cx);
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
                        let needs_reload = this.apply_loaded(entries);
                        // AA01(a): F5 is a fresh read as well. End the F6-36
                        // RESCAN_FAILED hold here too, not only on reconcile;
                        // the helper leaves other success notices untouched.
                        clear_rescan_failure_after_successful_read(&mut this.status);
                        if needs_reload {
                            // Re-read after events that landed past this
                            // snapshot (finding 2). Bumps generation so this
                            // callback's own result cannot land twice.
                            this.reload(cx);
                        }
                        // Keep self-operation notifications ("ゴミ箱へ移動しました",
                        // "フォルダを作成しました", "変更を検出しました") visible; they are
                        // replaced by the next action's message.
                    }
                    Err(error) => {
                        this.all_entries.clear();
                        this.entries.clear();
                        // Keep the fingerprint in sync with the cleared listing.
                        // Otherwise it still describes the last successful load,
                        // so a later reconcile whose read matches that load sees
                        // `changed == false` and skips the reload. Defence in
                        // depth alongside the explicit clear-error re-apply.
                        this.fingerprint = fingerprint(&this.all_entries);
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

    /// Set a load error from a path outside the reload callback.
    ///
    /// All such sites go through here so the generation is always advanced and
    /// any reload already in flight is invalidated (finding 1). The error
    /// raised *inside* the reload callback keeps its captured generation and is
    /// deliberately not routed through this helper.
    fn set_load_error(&mut self, error: String) {
        self.load = error_state_with_fresh_generation(&mut self.generation, error);
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
                        // AB06: Trash の F5 再読込も成功読み込みなので、保持中の
                        // RESCAN_FAILED を解除する。ヘルパーは RESCAN_FAILED
                        // 以外のステータスを変更しない。
                        clear_rescan_failure_after_successful_read(&mut this.status);
                    }
                    Err(error) => {
                        this.trash_items.clear();
                        this.all_entries.clear();
                        this.entries.clear();
                        this.selected = 0;
                        this.selection.clear();
                        this.load = LoadState::Error {
                            generation,
                            error: trash_load_error_status(&error),
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
                // Keyboard ops need a target, but the user picked nothing:
                // mark it implicit so the bar says "カーソル:".
                self.implicit_selection = true;
                self.selection.select_single(entry.path.clone());
            }
        }
        if !self.entries.is_empty() {
            self.scroll_to_entry(self.selected);
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
        self.pending_rename = None;
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
    ///
    /// Returns `true` when a watcher event was deferred because this load was
    /// already in flight; the caller must then trigger a fresh reload so those
    /// events are not lost to this snapshot (finding 2).
    fn apply_loaded(&mut self, entries: Vec<Entry>) -> bool {
        let keep = self
            .pending_select
            .take()
            .or_else(|| self.selected_entry().map(|entry| entry.path.clone()));
        self.all_entries = entries;
        self.fingerprint = fingerprint(&self.all_entries);
        self.rebuild_view(keep);
        std::mem::take(&mut self.needs_reload_after_load)
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
                // Keyboard ops need a target, but the user picked nothing:
                // mark it implicit so the bar says "カーソル:".
                self.implicit_selection = true;
                self.selection.select_single(entry.path.clone());
            }
        }
        if !self.entries.is_empty() {
            self.scroll_to_entry(self.selected);
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

    /// Re-check which recent directories still exist for the **display cache**
    /// only. The persisted `settings.recent_dirs` must survive a temporarily
    /// missing directory (e.g. an unmounted drive) so it reappears when back.
    /// Called from `reload` only — never from render.
    fn refresh_recent(&mut self) {
        self.recent = self
            .settings
            .recent_dirs
            .iter()
            .filter(|path| path.is_dir())
            .cloned()
            .collect();
    }

    /// Re-check which bookmarks still exist for the display cache only; keep
    /// `settings.bookmarks` intact (see [`Self::refresh_recent`]). Called from
    /// `reload` only — never from render.
    fn refresh_bookmarks(&mut self) {
        self.bookmarks = self
            .settings
            .bookmarks
            .iter()
            .filter(|path| path.is_dir())
            .cloned()
            .collect();
    }

    /// Toggle a bookmark for the current directory (Ctrl+D).
    fn toggle_current_bookmark(&mut self, cx: &mut Context<Self>) {
        if self.view != ViewSource::Directory {
            self.status = STATUS_BOOKMARKS_ONLY_DIRECTORY.to_string();
            cx.notify();
            return;
        }
        let path = self.cwd.clone();
        // A non-UTF-8 path is rejected by `toggle_bookmark` (sane_path), which
        // would trip the assertion below in debug and show a false "removed"
        // message in release. Refuse it before any state is touched.
        if let Some(status) = bookmark_toggle_rejection(&path) {
            self.status = status;
            cx.notify();
            return;
        }
        let was_bookmarked = self.settings.is_bookmarked(&path);
        let added = self.settings.toggle_bookmark(&path);
        debug_assert_eq!(added, !was_bookmarked);
        let saved = self.save_settings();
        self.refresh_bookmarks();
        self.status = settings_write_status(
            &self.settings,
            saved,
            true,
            if added {
                format!("ブックマークに追加しました: {}", display_path(&path))
            } else {
                format!("ブックマークを解除しました: {}", display_path(&path))
            },
        );
        cx.notify();
    }

    /// Toggle a bookmark for an arbitrary directory row (F6-49). The sidebar
    /// cache is refreshed immediately so the label UI sees the new entry.
    fn toggle_bookmark_path(&mut self, path: &Path, cx: &mut Context<Self>) {
        if path.as_os_str().is_empty() {
            return;
        }
        // Mirror `toggle_current_bookmark`: a non-UTF-8 path would be rejected
        // by `toggle_bookmark` and produce a false "removed" status below.
        if let Some(status) = bookmark_toggle_rejection(path) {
            self.status = status;
            cx.notify();
            return;
        }
        let added = self.settings.toggle_bookmark(path);
        let saved = self.save_settings();
        self.refresh_bookmarks();
        self.status = settings_write_status(
            &self.settings,
            saved,
            true,
            if added {
                format!("ブックマークに追加しました: {}", display_path(path))
            } else {
                format!("ブックマークを解除しました: {}", display_path(path))
            },
        );
        cx.notify();
    }

    /// Open the inline label editor for a directory row (F6-49).
    fn open_label_editor(&mut self, path: PathBuf, window: &mut Window, cx: &mut Context<Self>) {
        if self.label_editor.is_some() {
            return;
        }
        let existing = self
            .settings
            .bookmark_label(&path)
            .unwrap_or("")
            .to_string();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(existing));
        let subscription = cx.subscribe(&input, |this, _input, event: &InputEvent, cx| {
            if matches!(event, InputEvent::PressEnter { .. }) {
                this.commit_label_editor(cx);
            }
        });
        let focus = input.read(cx).focus_handle(cx).clone();
        focus.focus(window, cx);
        self.label_editor = Some(LabelState {
            path,
            input,
            _subscription: subscription,
        });
        cx.notify();
    }

    /// Apply the typed label. A blank value removes the label, matching
    /// `Settings::set_bookmark_label`'s own contract (F6-49).
    fn commit_label_editor(&mut self, cx: &mut Context<Self>) {
        let (path, value) = {
            let Some(editor) = self.label_editor.as_ref() else {
                return;
            };
            (
                editor.path.clone(),
                editor.input.read(cx).value().to_string(),
            )
        };
        let label = value.trim().to_string();
        // A label needs a bookmark owner, otherwise validate() drops it on the
        // next load and the label silently disappears after a restart. The
        // "ラベルを設定…" menu entry is available for any directory, so
        // auto-bookmark a path that is not bookmarked yet. Clearing a label
        // never creates a bookmark.
        //
        // Validate first: an oversized/NUL label or a full label list would
        // otherwise leave a fresh bookmark behind with no label attached.
        let auto_bookmarked =
            if label.is_empty() || !bookmark_label_is_settable(&self.settings, &path, &label) {
                false
            } else {
                self.settings.ensure_bookmark_for_label(&path)
            };
        let changed = if label.is_empty() {
            self.settings.remove_bookmark_label(&path)
        } else {
            self.settings.set_bookmark_label(&path, &label)
        };
        // No label change means no `save()` call: keep the specific "not set" /
        // "could not set" caption instead of letting a read-only session mask it
        // with the generic read-only notice (I02).
        let attempted = changed || auto_bookmarked;
        let saved = if attempted {
            self.save_settings()
        } else {
            true
        };
        if auto_bookmarked {
            self.refresh_bookmarks();
        }
        self.close_label_editor(cx);
        self.status = settings_write_status(
            &self.settings,
            saved,
            attempted,
            if label.is_empty() {
                if changed {
                    format!("ラベルを削除しました: {}", display_path(&path))
                } else {
                    bookmark_label_not_set_status(&path)
                }
            } else if changed {
                format!("ラベルを設定しました: {label}（{}）", display_path(&path))
            } else {
                bookmark_label_set_rejection(
                    &path,
                    bookmark_label_rejection_reason(&self.settings, &path, &label),
                )
            },
        );
        cx.notify();
    }

    /// Remove a label directly from the row context menu (F6-49).
    fn clear_bookmark_label(&mut self, path: &Path, cx: &mut Context<Self>) {
        let removed = self.settings.remove_bookmark_label(path);
        // An unchanged label never reaches `save()`; report the caller's own
        // "not set" caption verbatim (I02).
        let saved = if removed { self.save_settings() } else { true };
        self.status = settings_write_status(
            &self.settings,
            saved,
            removed,
            if removed {
                format!("ラベルを削除しました: {}", display_path(path))
            } else {
                bookmark_label_not_set_status(path)
            },
        );
        cx.notify();
    }

    fn close_label_editor(&mut self, cx: &mut Context<Self>) {
        if self.label_editor.take().is_some() {
            // Dropping the editor also drops its focused input; hand focus back
            // to the list on the next render (commit / Esc / Cancel / delete
            // all funnel through here).
            self.restore_focus_next_frame();
            cx.notify();
        }
    }

    /// Re-read the mount list off the UI thread.
    ///
    /// `/proc/mounts` is a blocking filesystem read; it now runs on the GPUI
    /// background executor and the result is applied only if the reload that
    /// requested it is still current. Called from `reload` only — never render.
    fn refresh_mounts(&mut self, cx: &mut Context<Self>) {
        let generation = self.generation;
        let background = cx.background_spawn(async move { mounts::list_mounts() });
        cx.spawn(async move |this, cx| {
            let mounts = background.await;
            let _ = this.update(cx, |this, cx| {
                if this.generation != generation {
                    return;
                }
                this.mounts = mounts;
                cx.notify();
            });
        })
        .detach();
    }

    /// 10-second reconciliation: re-read the directory in the background and
    /// reload only when the content fingerprint actually changed. This covers
    /// events lost by the OS watcher (queue overflow, unwatchable paths, ...).
    fn reconcile(&mut self, cx: &mut Context<Self>) {
        // Hotplugged volumes must appear even while a non-directory view is
        // active — the directory-only work below is skipped in that case.
        // `refresh_mounts` offloads the `/proc/mounts` read and discards stale
        // results by generation, so refreshing on every 10s tick is cheap and
        // safe (a `reload` may still request one sooner).
        self.refresh_mounts(cx);
        // The trash view has its own listing; the directory reconcile must not
        // overwrite it with the current directory contents. `in_flight` also
        // rejects a tick fired while the previous `read_entries` is still
        // running (a slow read can outlive the 10s interval); otherwise the
        // reads would pile up and accumulate stale snapshots/tasks.
        if !should_start_reconcile(self.reconcile_in_flight, self.load.is_loading(), self.view) {
            return;
        }
        self.reconcile_in_flight = true;
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
                        // Release the in-flight guard on every completion path,
                        // errors included, so a later tick is never blocked.
                        this.reconcile_in_flight = false;
                        if this.generation == generation {
                            this.status = format!("{STATUS_RESCAN_FAILED}: {error}");
                            cx.notify();
                        }
                    });
                    return;
                }
            };
            let _ = this.update(cx, |this, cx| {
                // Release the in-flight guard before any early return so a
                // later tick can always dispatch a fresh reconcile.
                this.reconcile_in_flight = false;
                // The view check is essential: `open_search` does not bump
                // `self.generation`, and search completion overwrites
                // `fingerprint`. Without it a reconcile dispatched before
                // Ctrl+F would land here with generation/loading still valid
                // and replace the live search results with directory entries.
                if !should_apply_reconcile(
                    this.view,
                    this.generation == generation,
                    this.load.is_loading(),
                ) {
                    return;
                }
                // A changed fingerprint, or a stale error overlay that must
                // be dropped, re-applies the freshly read listing. When both
                // flags are false there is nothing to rebuild, but the read
                // itself still ended the RESCAN_FAILED hold (below).
                // `Loading` is never touched here (guarded above), so an
                // explicit reload that started after this read was dispatched
                // still wins.
                let clear_error = should_clear_error_on_reconcile(&this.load);
                let changed = fingerprint(&entries) != this.fingerprint;
                let applied = should_apply_loaded_on_reconcile(changed, clear_error);
                if applied {
                    // `apply_loaded` rebuilds the view and recomputes the
                    // fingerprint itself, but does not touch `load`; the
                    // explicit transition below is still needed to drop the
                    // error overlay. Calling it once here covers both paths
                    // without a double-apply when both flags are true.
                    let needs_reload = this.apply_loaded(entries);
                    if clear_error {
                        this.load = LoadState::Ready;
                    }
                    if needs_reload {
                        this.reload(cx);
                    }
                }
                // AA01(b): this is a successful read even when the fingerprint
                // did not change (changed=false, clear_error=false), so the
                // clear must sit outside the apply guard above. Keeping it here
                // ends the F6-36 hold on every successful reconcile read while
                // leaving `load`/`entries` handling to the guarded branch.
                let status_cleared = clear_rescan_failure_after_successful_read(&mut this.status);
                // A no-change/no-clear tick still has nothing to render; avoid
                // adding a 10s repaint just to run the status check.
                if applied || status_cleared {
                    cx.notify();
                }
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
                            // AC01: a rescan/rename notice is a watch notice,
                            // not a successful read. Compose the same guard as
                            // the removed-entry branch below so a held
                            // RESCAN_FAILED (with its F5 hint) survives.
                            let reconciled =
                                reconcile_aware_status(&this.status, STATUS_SEARCH_STALE);
                            this.status = search_filter_status(&this.status, reconciled);
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
                        // `apply_search_filter` already preserves a stale
                        // warning; do not bury it under a plain "changed"
                        // status when a removed entry pruned the snapshot.
                        // AB02: search_filter_status only preserves
                        // STATUS_SEARCH_STALE, so compose the rescan-failure
                        // guard first. The reconciled status becomes the
                        // summary passed to search_filter_status: a held
                        // RESCAN_FAILED then survives unchanged, while a stale
                        // warning still prefixes the reconciled summary.
                        let reconciled = reconcile_aware_status(&this.status, STATUS_CHANGED);
                        this.status = search_filter_status(&this.status, reconciled);
                        cx.notify();
                        return;
                    }
                    if this.load.is_loading() {
                        // A reload's snapshot may predate these events. Mutating
                        // `all_entries` here would be undone when its callback
                        // replaces the vec with the stale snapshot (finding 2).
                        // Defer: the load will re-issue itself on completion.
                        this.needs_reload_after_load = true;
                        return;
                    }
                    if batch
                        .iter()
                        .any(|event| matches!(event, FsEvent::Rescan | FsEvent::Renamed { .. }))
                    {
                        // Z02: do not overwrite a pending rescan failure here.
                        this.status = reconcile_aware_status(&this.status, STATUS_CHANGED);
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
                                // Z02: keep the F5 rescan-failure hint until a read succeeds.
                                this.status = reconcile_aware_status(&this.status, STATUS_CHANGED);
                                this.reload(cx);
                                return;
                            }
                        }
                    }
                    if changed {
                        this.fingerprint = fingerprint(&this.all_entries);
                        this.rebuild_view(keep);
                        // Z02: an incremental update is not a successful
                        // re-read; preserve a pending rescan failure here too.
                        this.status = reconcile_aware_status(&this.status, STATUS_CHANGED);
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
    /// saves the settings file). The full per-tab navigation history is kept
    /// in the primary window's session, not in the legacy path-only fields.
    fn persist_tabs(&mut self) {
        if self.debug.ephemeral {
            return;
        }
        self.tabs.sync(self.cwd.clone(), self.history.clone());
        self.settings.tabs = self.tabs.tabs().iter().map(|tab| tab.cwd.clone()).collect();
        self.settings.active_tab = self.tabs.active_index();
        let window = self.settings.primary_window_mut();
        window.active_tab = self.tabs.active_index();
        window.tabs = self.tabs.tabs().iter().map(TabSession::from_tab).collect();
    }

    /// Save settings with this window's latest frame attached.
    ///
    /// All session writes go through here so the bounds observed in `render`
    /// (and the should-close hook) are persisted alongside tabs/history.
    fn save_settings(&mut self) -> bool {
        if let Some(bounds) = self.window_bounds {
            self.settings.primary_window_mut().bounds = Some(bounds);
        }
        self.settings.save_with_reason().is_ok()
    }

    /// Load a tab's directory/history into the explorer. Returns whether the
    /// accompanying settings write succeeded so callers that add a success
    /// message can keep it from masking a save failure (Bug L01).
    fn load_tab(&mut self, tab: Tab, cx: &mut Context<Self>) -> bool {
        self.clear_search();
        self.view = ViewSource::Directory;
        self.cwd = tab.cwd;
        self.history = tab.history;
        self.selection.clear();
        self.selected = 0;
        self.pending_select = None;
        self.pending_rename = None;
        self.load = LoadState::Ready;
        self.status.clear();
        self.settings.last_dir = Some(self.cwd.clone());
        self.settings.record_recent(&self.cwd);
        self.persist_tabs();
        let saved = self.save_settings();
        // `load_tab` clears the status above, so no success caption precedes
        // the write. Route through the shared helper anyway: in a read-only /
        // ephemeral session `save()` reports success without writing, and the
        // caller must see the dedicated notice (J01).
        self.status = settings_write_status(&self.settings, saved, true, String::new());
        self.reload(cx);
        saved
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
        if self.tabs.len() >= MAX_TABS {
            self.status = format!("{STATUS_MAX_TABS_PREFIX}{MAX_TABS}{STATUS_MAX_TABS_SUFFIX}");
            cx.notify();
            return;
        }
        self.sync_active_tab();
        self.tabs.new_tab(self.cwd.clone());
        let tab = self.tabs.active().clone();
        self.load_tab(tab, cx);
    }

    /// Open a new tab rooted at `path` (middle click on a row / grid cell).
    fn new_tab_at(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        if self.view != ViewSource::Directory {
            self.status = STATUS_TABS_ONLY_DIRECTORY.to_string();
            cx.notify();
            return;
        }
        if !path.is_dir() {
            self.status = format!("{STATUS_NOT_DIRECTORY}: {}", display_path(&path));
            cx.notify();
            return;
        }
        if self.tabs.len() >= MAX_TABS {
            self.status = format!("{STATUS_MAX_TABS_PREFIX}{MAX_TABS}{STATUS_MAX_TABS_SUFFIX}");
            cx.notify();
            return;
        }
        self.sync_active_tab();
        self.tabs.new_tab(path.clone());
        let tab = self.tabs.active().clone();
        let saved = self.load_tab(tab, cx);
        self.status = settings_write_status(
            &self.settings,
            saved,
            true,
            format!("新しいタブで開きました: {}", display_path(&path)),
        );
        cx.notify();
    }

    /// Shared middle-click handler for list rows and grid cells.
    fn open_row_in_new_tab(this: &mut Self, index: usize, cx: &mut Context<Self>) {
        let Some(target) = this
            .entries
            .get(index)
            .map(|entry| middle_click_tab_dir(entry, &this.cwd))
        else {
            return;
        };
        this.new_tab_at(target, cx);
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
            let saved = self.load_tab(tab, cx);
            self.status = settings_write_status(
                &self.settings,
                saved,
                true,
                format!("{STATUS_TAB_RESTORED}: {}", display_path(&cwd)),
            );
        } else {
            self.status = STATUS_TAB_RESTORE_EMPTY.to_string();
        }
        cx.notify();
    }

    /// Scroll to the list row containing an entry. In icon-grid mode several
    /// entries share a row, so the list index differs from the entry index
    /// (third cross-review F6-10).
    fn scroll_to_entry(&self, entry_index: usize) {
        let row = if self.settings.view == ViewLayout::Icons {
            entry_index / self.grid_column_count.max(1)
        } else {
            entry_index
        };
        self.scroll_handle
            .scroll_to_item(row, ScrollStrategy::Nearest);
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
            let saved = self.save_settings();
            self.status = settings_write_status(
                &self.settings,
                saved,
                true,
                STATUS_TAB_MOVED_LEFT.to_string(),
            );
            cx.notify();
        }
    }

    fn move_tab_right(&mut self, cx: &mut Context<Self>) {
        self.sync_active_tab();
        if self.tabs.move_active_right() {
            self.persist_tabs();
            let saved = self.save_settings();
            self.status = settings_write_status(
                &self.settings,
                saved,
                true,
                STATUS_TAB_MOVED_RIGHT.to_string(),
            );
            cx.notify();
        }
    }

    /// Horizontal tab bar (directory view only).
    fn tab_bar(&self, cx: &mut Context<Self>) -> Option<Div> {
        if self.view != ViewSource::Directory {
            return None;
        }
        let p = self.palette;
        let mut bar = div()
            .flex()
            .items_center()
            .gap_1()
            .px_3()
            .py_1()
            .bg(rgb(p.base))
            .border_b_1()
            .border_color(rgb(p.surface))
            .overflow_hidden();
        for (index, tab) in self.tabs.tabs().iter().enumerate() {
            let active = index == self.tabs.active_index();
            let label = tab_label(&tab.cwd);
            let close_aria = format!("{label} タブを閉じる");
            // U-18: the label keeps only the last component, so same-named
            // directories are ambiguous without the full path on hover.
            let full_path = tab.cwd.display().to_string();
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
                .when(active, |item| item.bg(rgb(p.surface)))
                .text_color(if active { rgb(p.text) } else { rgb(p.subtext) })
                .hover(|style| style.bg(rgb(p.surface)))
                .tooltip(move |window, cx| Tooltip::new(full_path.clone()).build(window, cx))
                .on_click(cx.listener(move |this, _event, window, cx| {
                    this.focus_handle.focus(window, cx);
                    this.switch_tab(index, cx);
                }))
                .child(div().max_w(px(160.0)).truncate().child(label));
            if active {
                item = item.child(
                    div()
                        .id("tab-close")
                        .aria_label(close_aria)
                        .px_1()
                        .rounded_md()
                        .hover(|style| style.bg(rgb(p.overlay)))
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
                .aria_label("新しいタブ")
                .px_2()
                .py_0p5()
                .rounded_md()
                .text_sm()
                .text_color(rgb(p.subtext))
                .cursor_pointer()
                .hover(|style| style.bg(rgb(p.surface)))
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
        self.pending_rename = None;
        self.load = LoadState::Ready;
        self.status.clear();
        self.settings.last_dir = Some(self.cwd.clone());
        self.settings.record_recent(&self.cwd);
        self.persist_tabs();
        let saved = self.save_settings();
        // No success caption is shown by navigation itself; still surface a
        // skipped write (read-only / ephemeral session) instead of staying
        // silent (J01).
        self.status = settings_write_status(&self.settings, saved, true, String::new());
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
            self.set_load_error(format!("{STATUS_NOT_DIRECTORY}: {}", display_path(&path)));
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
        // Peek first: a missing target must leave cwd and history untouched.
        let Some(target) = self.history.peek_back(&self.cwd) else {
            return;
        };
        if !target.is_dir() {
            self.status = format!("{STATUS_TARGET_GONE}: {}", display_path(&target));
            cx.notify();
            return;
        }
        if let Some(committed) = self.history.apply(self.cwd.clone(), target, NavMode::Back) {
            self.set_cwd(committed, cx);
        }
    }

    fn go_forward(&mut self, cx: &mut Context<Self>) {
        if self.view == ViewSource::Trash {
            self.exit_trash(cx);
            return;
        }
        // Peek first: a missing target must leave cwd and history untouched.
        let Some(target) = self.history.peek_forward(&self.cwd) else {
            return;
        };
        if !target.is_dir() {
            self.status = format!("{STATUS_TARGET_GONE}: {}", display_path(&target));
            cx.notify();
            return;
        }
        if let Some(committed) = self
            .history
            .apply(self.cwd.clone(), target, NavMode::Forward)
        {
            self.set_cwd(committed, cx);
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
            self.status = format!("{name} {STATUS_OPENED_DEFAULT_SUFFIX}");
            cx.notify();
        }
    }

    // -------------------------------------------------------------- selection

    /// Select one row (plain click / arrow key) and move the keyboard cursor.
    fn select_single_index(&mut self, index: usize, cx: &mut Context<Self>) {
        let index = clamp_index(index, self.entries.len());
        self.implicit_selection = false;
        self.selected = index;
        if let Some(entry) = self.entries.get(index) {
            self.selection.select_single(entry.path.clone());
        } else {
            self.selection.clear();
        }
        if !self.entries.is_empty() {
            self.scroll_to_entry(index);
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
        self.implicit_selection = false;
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
        self.scroll_to_entry(index);
        cx.notify();
    }

    /// Switch between the details list and the icon grid (persisted).
    fn set_view(&mut self, mode: ViewLayout, cx: &mut Context<Self>) {
        if self.settings.view == mode {
            return;
        }
        // Snapshot before previewing the view switch: a failed write leaves
        // disk at the old mode, so the in-memory value must be rolled back
        // before the next render can observe the preview (D13/H01).
        let snapshot = self.settings.clone();
        self.settings.view = mode;
        let saved = self.save_settings();
        if settings_write_needs_rollback(saved) {
            self.rollback_settings(snapshot, cx);
        }
        self.status = set_view_status(&self.settings, saved, mode);
        cx.notify();
    }

    fn select_all(&mut self, cx: &mut Context<Self>) {
        self.implicit_selection = false;
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

    /// U-07: keyboard access to the sidebar. Sidebar rows are click-only
    /// (`on_click`) and have no per-row `FocusHandle`; making them tab-focusable
    /// would be invasive, so the minimal safe path jumps straight to the first
    /// sidebar place (Home). Limitation: bookmarks, recent, drives and trash
    /// sidebar entries remain mouse-only.
    fn focus_sidebar(&mut self, cx: &mut Context<Self>) {
        let home = self
            .places
            .first()
            .filter(|place| place.exists)
            .and_then(|place| place.path.clone());
        if let Some(path) = home {
            self.navigate_to(path, cx);
        }
    }

    /// U-08: toggle the focused row in the non-contiguous selection from the
    /// keyboard, i.e. the keyboard equivalent of Ctrl+click. Reuses the single
    /// modifier-aware selection path so Shift ranges keep working unchanged.
    fn toggle_selection(&mut self, cx: &mut Context<Self>) {
        let selected = self.selected;
        self.select_with_modifiers(selected, true, false, cx);
    }

    /// Keyboard path for the context-menu "パスをコピー" action (Ctrl+Shift+C).
    /// Limitation (documented): "ファイルマネージャで表示" (reveal) and
    /// "ラベルを設定…" (set label) stay context-menu-only; adding a key action
    /// for each is deferred to keep this change scoped to one high-value one.
    fn copy_path_for_selection(&mut self, cx: &mut Context<Self>) {
        let path = self.selected_entry().map(|entry| entry.path.clone());
        if let Some(path) = path {
            self.copy_path(path, cx);
        }
    }

    // ---------------------------------------------------------------- terminal

    /// Launch the configured terminal emulator with `dir` as its working
    /// directory. A disabled command or a failed spawn is reported on the
    /// status bar instead of failing silently (F6-50).
    fn open_terminal(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
        let Some(argv) = self.settings.terminal_command_argv() else {
            self.status = terminal_disabled_status();
            cx.notify();
            return;
        };
        let Some(mut command) = terminal_process(&argv, &dir) else {
            self.status = terminal_disabled_status();
            cx.notify();
            return;
        };
        match command.spawn() {
            Ok(mut child) => {
                // Reap in a detached thread: dropping `Child` on Unix leaves a
                // zombie until the app exits (finding 3 of the feature review).
                std::thread::spawn(move || {
                    let _ = child.wait();
                });
                self.status = terminal_started_status(&argv, &dir);
            }
            Err(error) => self.status = terminal_failed_status(&error, &dir),
        }
        cx.notify();
    }

    /// F4 entry point: use the focused row, or the current directory when the
    /// listing is empty. Trash rows have no filesystem directory to reuse.
    fn open_terminal_for_selection(&mut self, cx: &mut Context<Self>) {
        if self.view == ViewSource::Trash {
            self.status = format!("{STATUS_TRASH_NO_TERMINAL_PREFIX}{STATUS_CANNOT_OPEN_SUFFIX}");
            cx.notify();
            return;
        }
        let dir = terminal_target_dir(self.selected_entry(), &self.cwd);
        self.open_terminal(dir, cx);
    }

    // ------------------------------------------------ terminal command settings

    /// Alt+T: open the terminal-command settings dialog, prefilled with the
    /// persisted command (or the platform default when none is configured).
    fn open_terminal_command_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.terminal_command_editor.is_some() {
            return;
        }
        let current = self.settings.terminal_command.trim();
        let value = if current.is_empty() {
            Settings::default().terminal_command
        } else {
            current.to_string()
        };
        let input = cx.new(|cx| InputState::new(window, cx).default_value(value));
        let subscription =
            cx.subscribe(&input, |this, _input, event: &InputEvent, cx| match event {
                InputEvent::Change => {
                    if let Some(editor) = this.terminal_command_editor.as_mut() {
                        if clear_dialog_error(&mut editor.error) {
                            cx.notify();
                        }
                    }
                }
                InputEvent::PressEnter { .. } => this.confirm_terminal_command(cx),
                _ => {}
            });
        let focus = input.read(cx).focus_handle(cx).clone();
        focus.focus(window, cx);
        self.terminal_command_editor = Some(TerminalCommandState {
            input,
            _subscription: subscription,
            error: None,
        });
        cx.notify();
    }

    fn close_terminal_command_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.terminal_command_editor.take().is_some() {
            self.focus_handle.focus(window, cx);
            self.focus_list_on_next_render = true;
            cx.notify();
        }
    }

    /// Enter in the dialog: persist the normalized command, save settings,
    /// close the dialog and return focus to the list. Invalid input (oversized
    /// or containing control characters) keeps the dialog open with an error.
    fn confirm_terminal_command(&mut self, cx: &mut Context<Self>) {
        let raw = {
            let Some(editor) = self.terminal_command_editor.as_ref() else {
                return;
            };
            editor.input.read(cx).value().to_string()
        };
        let command = normalize_terminal_command(&raw).unwrap_or_default();
        // Snapshot before previewing the command: a failed write leaves disk at
        // the old value, so rollback restores the persisted command (D13).
        let snapshot = self.settings.clone();
        if !self.settings.set_terminal_command(&command) {
            self.set_terminal_command_error(STATUS_TERMINAL_COMMAND_INVALID, cx);
            return;
        }
        let saved = self.save_settings();
        if settings_write_needs_rollback(saved) {
            self.rollback_settings(snapshot, cx);
        }
        self.terminal_command_editor = None;
        self.focus_list_on_next_render = true;
        self.status = settings_write_status(
            &self.settings,
            saved,
            true,
            if command.is_empty() {
                "ターミナル起動コマンドを無効にしました".to_string()
            } else {
                format!("ターミナル起動コマンドを設定しました: {command}")
            },
        );
        cx.notify();
    }

    fn set_terminal_command_error(&mut self, error: &str, cx: &mut Context<Self>) {
        if let Some(editor) = self.terminal_command_editor.as_mut() {
            editor.error = Some(error.to_string());
        }
        cx.notify();
    }

    /// Modal overlay for the terminal-command settings dialog (Alt+T).
    fn terminal_command_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        let editor = self.terminal_command_editor.as_ref()?;
        let p = self.palette;
        let backdrop = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(gpui::black().opacity(0.45))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    this.close_terminal_command_settings(window, cx);
                }),
            );
        let cancel = cx.listener(|this, _: &ClickEvent, window, cx| {
            this.close_terminal_command_settings(window, cx);
        });
        let confirm = cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.confirm_terminal_command(cx);
        });
        let mut form = div()
            .w(px(480.0))
            .bg(rgb(p.surface))
            .rounded_lg()
            .border_1()
            .border_color(rgb(p.overlay))
            .shadow_lg()
            .p_5()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("ターミナル起動コマンド"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(p.subtext))
                    .child("F4 で開くターミナルの起動コマンド（空白で無効化）"),
            )
            .child(Input::new(&editor.input).w_full());
        if let Some(error) = &editor.error {
            form = form.child(div().text_sm().text_color(rgb(p.red)).child(error.clone()));
        }
        form = form
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("terminal-command-cancel")
                            .label("キャンセル")
                            .on_click(cancel),
                    )
                    .child(
                        Button::new("terminal-command-confirm")
                            .label("設定")
                            .on_click(confirm),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(p.muted))
                    .child(DIALOG_FOOTER_HINT),
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

    // ------------------------------------------------------------ keymap settings

    /// Alt+K: open the keymap dialog, prefilled with an action name. The chord
    /// is typed after a space (`open_preview ctrl+shift+p`); raw key capture is
    /// deliberately avoided because GPUI's `key_down` is global and a modal
    /// input cannot claim arbitrary chords without stealing Escape / F1.
    fn open_keymap_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.keymap_editor.is_some() {
            return;
        }
        let input = cx.new(|cx| InputState::new(window, cx).default_value("open_preview "));
        let subscription =
            cx.subscribe(&input, |this, _input, event: &InputEvent, cx| match event {
                InputEvent::Change => {
                    if let Some(editor) = this.keymap_editor.as_mut() {
                        if clear_dialog_error(&mut editor.error) {
                            cx.notify();
                        }
                    }
                }
                InputEvent::PressEnter { .. } => this.confirm_keymap(cx),
                _ => {}
            });
        let focus = input.read(cx).focus_handle(cx).clone();
        focus.focus(window, cx);
        self.keymap_editor = Some(KeymapEditorState {
            input,
            _subscription: subscription,
            error: None,
        });
        cx.notify();
    }

    fn close_keymap_settings(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.keymap_editor.take().is_some() {
            self.focus_handle.focus(window, cx);
            self.focus_list_on_next_render = true;
            cx.notify();
        }
    }

    /// Enter in the dialog: parse `action chord` (bind) or `unbind action`
    /// (remove), apply it to the persisted overrides, save settings and rebuild
    /// the live table immediately. Invalid input keeps the dialog open with an
    /// error; Esc closes it.
    fn confirm_keymap(&mut self, cx: &mut Context<Self>) {
        let raw = {
            let Some(editor) = self.keymap_editor.as_ref() else {
                return;
            };
            editor.input.read(cx).value().to_string()
        };
        let Some(command) = parse_keymap_input(&raw) else {
            self.set_keymap_error(STATUS_KEYMAP_FORMAT_INVALID, cx);
            return;
        };
        // Snapshot before previewing the keymap change: a failed write leaves
        // disk at the old value, so rollback must restore both settings.keymap
        // and the derived live table (D13).
        let snapshot = self.settings.clone();
        // `attempted` records whether anything reached `save()`. Bind always
        // changes the overrides; unbind only does so when an override was really
        // removed (K02). Mirrors the `clear_bookmark_label` no-op pattern.
        let (status, attempted) = match command {
            KeymapInput::Bind { action, chord } => {
                if !upsert_keymap_override(&mut self.settings.keymap, &action, &chord) {
                    self.set_keymap_error(&keymap_limit_status(), cx);
                    return;
                }
                (keymap_bind_caption(&action, &chord), true)
            }
            KeymapInput::Unbind { action } => {
                let removed = remove_keymap_override(&mut self.settings.keymap, &action);
                (keymap_unbind_caption(removed, &action), removed)
            }
        };
        // Rebuild the live table so the change works without a restart, exactly
        // as a binding read from settings.json at startup would. Unbinding a
        // persisted override restores that action's default binding.
        self.keymap = KeyMap::from_settings(&self.settings);
        // A no-op unbind never reaches `save()`, so there is nothing to skip:
        // its own "解除する設定はありません" caption must survive instead of being
        // masked by STATUS_SETTINGS_READ_ONLY in a read-only session (K02).
        let saved = if attempted {
            self.save_settings()
        } else {
            true
        };
        if settings_write_needs_rollback(saved) {
            self.rollback_settings(snapshot, cx);
        }
        self.keymap_editor = None;
        self.focus_list_on_next_render = true;
        self.status = settings_write_status(&self.settings, saved, attempted, status);
        cx.notify();
    }

    fn set_keymap_error(&mut self, error: &str, cx: &mut Context<Self>) {
        if let Some(editor) = self.keymap_editor.as_mut() {
            editor.error = Some(error.to_string());
        }
        cx.notify();
    }

    /// Modal overlay for the keymap settings dialog (Alt+K).
    fn keymap_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        let editor = self.keymap_editor.as_ref()?;
        let p = self.palette;
        let backdrop = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(gpui::black().opacity(0.45))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, window, cx| {
                    this.close_keymap_settings(window, cx);
                }),
            );
        let cancel = cx.listener(|this, _: &ClickEvent, window, cx| {
            this.close_keymap_settings(window, cx);
        });
        let confirm = cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.confirm_keymap(cx);
        });
        let mut form = div()
            .w(px(480.0))
            .bg(rgb(p.surface))
            .rounded_lg()
            .border_1()
            .border_color(rgb(p.overlay))
            .shadow_lg()
            .p_5()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("キーマップの設定"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(p.subtext))
                    .child(KEYMAP_INPUT_HINT),
            )
            .child(Input::new(&editor.input).w_full());
        if let Some(error) = &editor.error {
            form = form.child(div().text_sm().text_color(rgb(p.red)).child(error.clone()));
        }
        form = form
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("keymap-cancel")
                            .label("キャンセル")
                            .on_click(cancel),
                    )
                    .child(
                        Button::new("keymap-confirm")
                            .label("設定")
                            .on_click(confirm),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(p.muted))
                    .child(DIALOG_FOOTER_HINT),
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

    // --------------------------------------------------------------- shortcuts

    fn on_key_down(&mut self, event: &KeyDownEvent, window: &mut Window, cx: &mut Context<Self>) {
        let key = event.keystroke.key.as_str();
        let modifiers = &event.keystroke.modifiers;
        // While the rename dialog is open the text input owns most keys; only
        // Escape is handled here (Enter arrives as `InputEvent::PressEnter`).
        if self.renaming.is_some() {
            if key == "escape" {
                self.cancel_rename(window, cx);
            }
            return;
        }
        if self.label_editor.is_some() {
            // The label input owns keys while open; only Escape closes it.
            if key == "escape" {
                self.close_label_editor(cx);
            }
            return;
        }
        if self.terminal_command_editor.is_some() {
            // The command input owns keys while open; only Escape closes it.
            if key == "escape" {
                self.close_terminal_command_settings(window, cx);
            }
            return;
        }
        if self.keymap_editor.is_some() {
            // The keymap input owns keys while open; only Escape closes it.
            // Enter arrives as `InputEvent::PressEnter`.
            if key == "escape" {
                self.close_keymap_settings(window, cx);
            }
            return;
        }
        if self.settings_ui.is_some() {
            // The preferences card owns the keyboard; its inputs receive typing.
            // Escape cancels and Enter saves, reusing the same action handler as
            // the footer buttons. This guard sits before F1 so the help overlay
            // can never be stacked on top of the settings dialog.
            //
            // The main keymap binds Ctrl+Tab / Ctrl+Shift+Tab to NextTab /
            // PrevTab, but that dispatch is unreachable while this guard holds.
            // Reuse the chords here to step through the five settings pages
            // instead; no settings action claimed them before (G08).
            if key == "tab" && modifiers.control {
                let delta = if modifiers.shift { -1 } else { 1 };
                self.cycle_settings_tab(delta, cx);
                return;
            }
            // N05: compute input focus once, ahead of Alt+S and the row
            // shortcuts, so neither can fire while a settings text input owns
            // the keystroke. Tab / Escape / Enter below keep their original
            // handling and deliberately ignore this flag.
            let typing = self.settings_ui.as_ref().is_some_and(|state| {
                [
                    Some(&state.search_input),
                    Some(&state.keymap_input),
                    Some(&state.terminal_input),
                ]
                .into_iter()
                .flatten()
                .any(|input| input.read(cx).focus_handle(cx).is_focused(window))
            });
            // I05: bare Tab / Shift+Tab cycles focus through the visible
            // controls (search box, the active tab's text editor when present,
            // then the Cancel/Save buttons). Ctrl+Tab above still switches pages.
            if key == "tab" {
                if let Some(state) = self.settings_ui.as_ref() {
                    state.cycle_focus(window, cx, modifiers.shift);
                }
                return;
            }
            // K07: Alt+1..Alt+5 jump straight to a page. This gives the
            // "matches in another tab" search hint a keyboard action instead
            // of a static pointer. Ctrl remains excluded so Ctrl+Alt+digit is
            // left alone.
            if modifiers.alt && !modifiers.control {
                if let Some(tab) = settings_tab_from_digit_key(key) {
                    self.on_settings_action(SettingsUiAction::Tab(tab), cx);
                    return;
                }
                // K08: RadioGroup/Switch are not in the Tab focus cycle, so
                // the Files-tab sort order gets its own direct chord. N05:
                // a focused text input keeps its `s` instead of sorting.
                if key == "s" && !typing {
                    let next_sort = self
                        .settings_ui
                        .as_ref()
                        .filter(|state| state.tab == SettingsTab::Files)
                        .map(|state| settings_sort_next(&state.sort_choice));
                    if let Some(next_sort) = next_sort {
                        self.on_settings_action(
                            SettingsUiAction::SetSort(next_sort.to_string()),
                            cx,
                        );
                        return;
                    }
                }
            }
            // M08: single-letter shortcuts for settings rows that are outside
            // the Tab focus cycle (theme / view / hidden). Skipped while a
            // text input owns focus, and only for a bare chord: the helper
            // contract rejects modified chords so global shortcuts survive (N01).
            let bare = settings_row_key_is_bare(modifiers);
            let row_shortcut = if bare && !typing {
                self.settings_ui.as_ref().and_then(|state| {
                    let query = state.search_input.read(cx).value().to_string();
                    if !settings_row_is_visible(state.tab, key, &query) {
                        return None;
                    }
                    let values = SettingsRowValues {
                        theme: &state.theme_choice,
                        view: &state.view_choice,
                        hidden: state.show_hidden,
                    };
                    settings_row_shortcut(state.tab, key, values)
                })
            } else {
                None
            };
            if let Some(action) = row_shortcut {
                self.on_settings_action(action, cx);
                return;
            }
            match key {
                "escape" => self.dismiss_settings_ui(cx),
                "enter" => self.on_settings_action(SettingsUiAction::Save, cx),
                _ => {}
            }
            return;
        }
        // F6-51: the fuzzy overlay owns the keyboard while open. Typing goes to
        // its input (Change / PressEnter subscriptions); only navigation and
        // dismissal are handled here. This guard runs before F1 so a second
        // dialog can never be opened on top of the fuzzy overlay.
        if self.fuzzy.is_some() {
            match key {
                "escape" => self.close_fuzzy(cx),
                "up" => self.move_fuzzy_selection(-1, cx),
                "down" => self.move_fuzzy_selection(1, cx),
                _ => {}
            }
            return;
        }
        if self.help_open {
            // Keyboard scrolling is intentionally skipped here: the overlay
            // body is a plain `.overflow_y_scroll()` div and gpui-pre 0.3.12
            // `ScrollHandle` exposes no `scroll_by`, so PageUp/PageDown/
            // Home/End have no scroll action to reach. Mouse wheel works.
            // P02: F1 also closes help here, so the key keeps toggling in both
            // directions after the open-toggle moved below the modal guards.
            if key == "escape" || key == "f1" {
                self.help_open = false;
                cx.notify();
            }
            return;
        }
        if self.location.is_some() {
            if key == "escape" {
                self.close_location(cx);
            }
            return;
        }
        if self.undo_history_open {
            // Same limitation as the F1 overlay: no `ScrollHandle::scroll_by`
            // API exists for its `.overflow_y_scroll()` body, so only Escape
            // is routed here; scrolling stays mouse-wheel only.
            if key == "escape" {
                self.undo_history_open = false;
                cx.notify();
            }
            return;
        }
        if self.preview.is_some() {
            // The preview overlay owns the keyboard: route its keys through a
            // pure mapping so the normal keymap below is left untouched.
            match preview::preview_key_action(key, modifiers.control) {
                preview::PreviewKeyAction::Close => self.close_preview(cx),
                preview::PreviewKeyAction::Next => self.move_preview(1, cx),
                preview::PreviewKeyAction::Prev => self.move_preview(-1, cx),
                preview::PreviewKeyAction::OpenNewTab => self.preview_open_in_new_tab(cx),
                preview::PreviewKeyAction::None => {}
            }
            return;
        }
        if self.properties.is_some() {
            if key == "escape" {
                self.close_properties(cx);
            }
            return;
        }
        // P02: F1 opens help only after every modal dialog has had a chance to
        // consume the key. This toggle used to sit above these guards, so F1 in
        // a dialog stacked help underneath it and the next Escape closed help
        // instead of the dialog (two Escapes needed). Settings and the fuzzy
        // overlay return even earlier; the help guard above handles F1 when
        // help is already open, keeping the toggle symmetric.
        if key == "f1" {
            self.help_open = !self.help_open;
            cx.notify();
            return;
        }
        if self.view == ViewSource::Search {
            if key == "escape" {
                // A running operation owns the first Escape: cancel it before
                // closing the search view. The next Escape then closes search.
                if self.op_cancel.is_some() {
                    self.cancel_op(cx);
                } else {
                    self.close_search(cx);
                }
                return;
            }
            // Ctrl+Shift+F (FocusSearchName) must reach the name filter even
            // while a search text input owns the keyboard. Handle it before
            // the input-focus guard below, which swallows every other key
            // typed into the query field except Reload. Using the keymap keeps
            // rebindings.
            if self
                .keymap
                .resolve(key, modifiers.control, modifiers.shift, modifiers.alt)
                == KeyAction::FocusSearchName
            {
                self.focus_search_name(window, cx);
                return;
            }
            // AE01: the query field still keeps every key while typing
            // (including F4 / Ctrl+Q), except Reload — F5 / Ctrl+R (or a rebind)
            // must reach dispatch so the advertised recovery key re-runs the
            // query. With focus on a result row / the root, the allowlist and
            // any keymap rebind of an allowed action fall through as before.
            let action =
                self.keymap
                    .resolve(key, modifiers.control, modifiers.shift, modifiers.alt);
            let reaches_keymap = search_view_key_reaches_keymap(
                &self.keymap,
                key,
                modifiers.control,
                modifiers.shift,
                modifiers.alt,
            );
            if !search_view_key_reaches_dispatch(
                self.search_input_focused(window, cx),
                action,
                reaches_keymap,
            ) {
                return;
            }
        }
        // Escape is intentionally literal here: no rebind may remove the
        // ability to cancel a running operation from the normal view.
        if key == "escape" {
            self.cancel_op(cx);
            return;
        }
        // Page step: how many 28px rows fit into the window viewport.
        let page_step = {
            let viewport_height = window.viewport_size().height.to_f64();
            ((viewport_height - LIST_CHROME_HEIGHT_PX) / ROW_HEIGHT_PX)
                .floor()
                .max(1.0) as isize
        };
        // F6-51: one table lookup replaces the previous Ctrl / Alt / plain
        // match cascade. Unknown keys resolve to `KeyAction::None` and stay
        // no-ops, exactly as before.
        let action = self
            .keymap
            .resolve(key, modifiers.control, modifiers.shift, modifiers.alt);
        self.dispatch_key_action(action, modifiers.shift, page_step, window, cx);
    }

    /// Execute a keymap action. All context-sensitive guards (trash view,
    /// search field focus) live here so the table itself stays pure and the
    /// behaviour of every pre-existing shortcut is preserved (F6-51).
    fn dispatch_key_action(
        &mut self,
        action: KeyAction,
        shift: bool,
        page_step: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        use KeyAction::*;
        match action {
            MoveUp => self.move_selection(-1, shift, cx),
            MoveDown => self.move_selection(1, shift, cx),
            PageUp => self.move_selection(-page_step, shift, cx),
            PageDown => self.move_selection(page_step, shift, cx),
            SelectFirst => self.select_first(shift, cx),
            SelectLast => self.select_last(shift, cx),
            OpenSelected => self.open_selected(cx),
            GoParent => self.go_parent(cx),
            // AC10: F5 is reachable in the Search view (see
            // `search_view_action_allowed`), but `reload()` deliberately
            // early-returns there to keep the search listing intact. Re-run
            // the current query instead, so the advertised "F5 で再読込"
            // recovery hint actually works while searching.
            Reload => {
                if reload_reruns_search(self.view) {
                    self.run_search(cx);
                } else {
                    self.reload(cx);
                }
            }
            OpenPreview => self.open_preview_selected(cx),
            StartRename => {
                if self.view == ViewSource::Trash {
                    self.status = STATUS_TRASH_NO_RENAME.to_string();
                    cx.notify();
                } else {
                    self.start_rename(window, cx);
                }
            }
            TrashSelected => {
                if trash_key_action("delete", false) == TrashKeyAction::NoPurge
                    && self.view == ViewSource::Trash
                {
                    // Permanent deletion is intentionally not offered; tell
                    // the user which keyboard path restores instead (F6-33).
                    self.status = trash_delete_status();
                    cx.notify();
                } else {
                    self.trash_selected(window, cx);
                }
            }
            RestoreSelected => {
                // Plain `R` and Ctrl+Enter share this alias; it only acts in
                // the trash view and is harmless everywhere else.
                if self.view == ViewSource::Trash
                    && trash_key_action("enter", true) == TrashKeyAction::Restore
                {
                    self.restore_selected(cx);
                }
            }
            ToggleHidden => self.toggle_hidden(cx),
            ToggleTheme => self.toggle_theme(cx),
            FocusSidebar => self.focus_sidebar(cx),
            ToggleSelection => self.toggle_selection(cx),
            CopyPath => self.copy_path_for_selection(cx),
            SelectAll => self.select_all(cx),
            ToggleBookmark => self.toggle_current_bookmark(cx),
            OpenSearch => self.open_search(window, cx),
            FocusSearchName => self.focus_search_name(window, cx),
            OpenProperties => self.open_selected_properties(cx),
            OpenLocation => self.open_location(window, cx),
            Undo => self.undo_last(cx),
            Redo => self.redo_last(cx),
            ToggleUndoHistory => {
                self.undo_history_open = !self.undo_history_open;
                cx.notify();
            }
            NewTab => self.new_tab(cx),
            RestoreClosedTab => self.restore_closed_tab(cx),
            CloseTab => self.close_tab(cx),
            NextTab => self.next_tab(cx),
            PrevTab => self.prev_tab(cx),
            MoveTabLeft => self.move_tab_left(cx),
            MoveTabRight => self.move_tab_right(cx),
            DetailsView => self.set_view(ViewLayout::Details, cx),
            IconsView => self.set_view(ViewLayout::Icons, cx),
            NewFolder => self.new_folder(cx),
            Copy => {
                if self.view == ViewSource::Trash {
                    self.status = STATUS_TRASH_NO_COPY.to_string();
                    cx.notify();
                } else {
                    self.copy_selected(cx);
                }
            }
            Cut => {
                if self.view == ViewSource::Trash {
                    self.status = STATUS_TRASH_NO_CUT.to_string();
                    cx.notify();
                } else {
                    self.cut_selected(cx);
                }
            }
            Paste => {
                if self.view == ViewSource::Trash {
                    self.status = STATUS_TRASH_NO_PASTE.to_string();
                    cx.notify();
                } else {
                    self.paste(cx);
                }
            }
            Quit => {
                // Ctrl+Q bypasses the per-window should_close handler, so save
                // explicitly before quitting (F6-43).
                if let Some(bounds) = WindowBoundsState::from_window(window) {
                    self.window_bounds = Some(bounds);
                }
                self.persist_tabs();
                // No status bar survives `quit`, so surface the failure in the
                // log instead of silently dropping it (Bug L01).
                if !self.save_settings() {
                    // V10: log the reason recorded by `save_with_reason` so a
                    // locked / oversized settings file is diagnosable.
                    tracing::error!(
                        reason = ?self.settings.last_save_error.get(),
                        "failed to save settings before quitting"
                    );
                }
                cx.quit();
            }
            GoBack => self.go_back(cx),
            GoForward => self.go_forward(cx),
            FuzzyJump => self.open_fuzzy_jump(window, cx),
            OpenTerminal => self.open_terminal_for_selection(cx),
            EditTerminalCommand => self.open_terminal_command_settings(window, cx),
            EditKeymap => self.open_keymap_settings(window, cx),
            OpenSettings => self.open_settings_ui(window, cx),
            None => {}
        }
    }

    // ------------------------------------------------------------------ actions

    fn toggle_hidden(&mut self, cx: &mut Context<Self>) {
        if self.view == ViewSource::Trash {
            // Trash pseudo-entries have no hidden flag: toggling here would
            // pollute the persisted directory preference without any effect.
            self.status = STATUS_TRASH_NO_HIDDEN.to_string();
            cx.notify();
            return;
        }
        // Snapshot before previewing the hidden flag: a failed write leaves
        // disk at the old setting, so the in-memory value must be rolled back
        // (D13) before any view refresh can observe the preview.
        let snapshot = self.settings.clone();
        self.settings.show_hidden = !self.settings.show_hidden;
        let saved = self.save_settings();
        if settings_write_needs_rollback(saved) {
            self.rollback_settings(snapshot, cx);
        } else {
            let keep = self.selected_entry().map(|entry| entry.path.clone());
            if self.view == ViewSource::Search {
                // The current hits were collected under the old hidden setting;
                // a filter-only refresh cannot add/remove hidden hits. Re-run
                // the query with the new setting (`run_search` reads it again).
                // `rebuild_view` would replace search results with the directory
                // listing and drop kind/extension/name filters (finding 3).
                self.run_search(cx);
            } else {
                self.rebuild_view(keep);
            }
        }
        self.status = settings_write_status(
            &self.settings,
            saved,
            true,
            if self.settings.show_hidden {
                "隠しファイルを表示しています".to_string()
            } else {
                "隠しファイルを非表示にしました".to_string()
            },
        );
        cx.notify();
    }

    /// Apply an already-resolved mode to the component theme, the legacy
    /// `settings.theme` field and the hand-drawn palette in one place. Every
    /// theme entry point (startup, toggle, OS appearance change) goes through
    /// here so none of the three can drift out of sync.
    fn apply_theme_mode(&mut self, mode: ThemeMode, cx: &mut Context<Self>) {
        self.settings.theme = mode;
        self.palette = AppPalette::for_mode(mode);
        // A dark app theme must resolve Breeze icons from breeze-dark first;
        // this also drops cached paths resolved under the previous mode.
        self.system_icon_resolver.set_prefer_dark(mode.is_dark());
        Theme::change(mode, None, cx);
        cx.notify();
    }

    /// Undo an in-memory settings preview after a failed disk write (D13).
    /// `snapshot` is the full `Settings` captured before any field was changed,
    /// so restoring it also covers `theme`, `theme_choice`, `view`,
    /// `show_hidden`, `sort`, `keymap` and `terminal_command`. The derived
    /// palette / icon resolver / GPUI theme are restored through the same
    /// `apply_theme_mode` entry point every theme change uses.
    fn rollback_settings(&mut self, snapshot: Settings, cx: &mut Context<Self>) {
        let previous_mode = snapshot.theme;
        let was_system = snapshot.theme_choice == ThemeChoice::System;
        self.settings = snapshot;
        self.keymap = KeyMap::from_settings(&self.settings);
        self.apply_theme_mode(previous_mode, cx);
        if was_system {
            // The preview may have pinned the component theme with
            // `Theme::change`; a System preference must keep following the OS.
            Theme::sync_system_appearance(None, cx);
        }
    }

    /// Re-resolve the preference when the OS switches between light and dark.
    /// The observer is always installed (see `open_window`) but this is a
    /// no-op unless the persisted choice is `System`.
    fn on_system_appearance_changed(&mut self, cx: &mut Context<Self>) {
        if self.settings.theme_choice != ThemeChoice::System {
            return;
        }
        let system: ThemeMode = cx.window_appearance().into();
        let resolved = resolve_theme(self.settings.theme_choice, system);
        if self.settings.theme == resolved {
            return;
        }
        self.apply_theme_mode(resolved, cx);
    }

    /// Cycle the persisted preference (Alt+Shift+T): Dark → Light → System →
    /// Dark. `Settings::theme` keeps the *resolved* mode for compatibility;
    /// `theme_choice` is what the next toggle starts from.
    fn toggle_theme(&mut self, cx: &mut Context<Self>) {
        // Snapshot before previewing: if the write fails, disk still holds the
        // old preference and the live UI must be put back to match it (D13).
        let snapshot = self.settings.clone();
        let choice = next_theme_choice(self.settings.theme_choice);
        self.settings.theme_choice = choice;
        let system: ThemeMode = cx.window_appearance().into();
        let resolved = resolve_theme(choice, system);
        if choice == ThemeChoice::System {
            // `None` makes gpui-component read the same window appearance we
            // resolved above (documented in `Theme::sync_system_appearance`).
            Theme::sync_system_appearance(None, cx);
            self.settings.theme = resolved;
            self.palette = AppPalette::for_mode(resolved);
            self.system_icon_resolver
                .set_prefer_dark(resolved.is_dark());
        } else {
            self.apply_theme_mode(resolved, cx);
        }
        let saved = self.save_settings();
        if settings_write_needs_rollback(saved) {
            self.rollback_settings(snapshot, cx);
        }
        self.status = settings_write_status(
            &self.settings,
            saved,
            true,
            format!(
                "{STATUS_THEME_CHANGED_PREFIX}{}",
                theme_choice_label(choice)
            ),
        );
        cx.notify();
    }

    fn set_sort(&mut self, key: SortKey, cx: &mut Context<Self>) {
        if self.view == ViewSource::Trash {
            // The trash list has its own pseudo-entry order: rewriting the
            // directory sort spec from its header would not sort anything.
            self.status = STATUS_TRASH_NO_SORT.to_string();
            cx.notify();
            return;
        }
        // Snapshot before previewing the sort: a failed write leaves disk at
        // the old spec, so both `settings.sort` and the visible order must be
        // put back to match it (D13/F05).
        let snapshot = self.settings.clone();
        let order = self.settings.sort.order().toggled(key);
        self.settings.sort = SortSpec::from_order(order);
        let saved = self.save_settings();
        if settings_write_needs_rollback(saved) {
            self.rollback_settings(snapshot, cx);
        } else {
            let keep = self.selected_entry().map(|entry| entry.path.clone());
            if self.view == ViewSource::Search {
                // Keep the search filters intact; a directory `rebuild_view` here
                // would discard them and `reload()` cannot restore them in Search
                // (finding 3).
                self.apply_search_filter(cx);
            } else {
                self.rebuild_view(keep);
            }
        }
        // `set_sort` has no success caption: keep the current status on a real
        // write, but make a skipped write visible instead of leaving an earlier
        // success status on the bar (F04/J01).
        let success = self.status.clone();
        self.status = settings_write_status(&self.settings, saved, true, success);
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
                            this.status = format!("{STATUS_FOLDER_CREATED}: {name}");
                            this.pending_select = Some(path.clone());
                            // F6-34: creation and naming become one gesture.
                            // The next render starts the inline rename once
                            // the new folder is present in the loaded listing.
                            this.pending_rename = Some(path);
                            this.reload(cx);
                        } else {
                            this.status = format!(
                                "{STATUS_FOLDER_CREATED}: {}（別のフォルダを表示中）",
                                display_path(&path)
                            );
                        }
                    }
                    Err(error) => {
                        this.set_load_error(format!("{ERR_FOLDER_CREATE}: {error}"));
                    }
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn copy_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        cx.write_to_clipboard(ClipboardItem::new_string(display_path(&path)));
        self.status = format!("{STATUS_PATH_COPIED}: {}", display_path(&path));
        cx.notify();
    }

    /// Remember the selected entries for a later paste (Ctrl+C).
    fn copy_selected(&mut self, cx: &mut Context<Self>) {
        let paths = self.action_paths();
        if paths.is_empty() {
            self.status = STATUS_NO_SELECTION.to_string();
            cx.notify();
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
            self.status = STATUS_NO_SELECTION.to_string();
            cx.notify();
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

    /// Claim the shared operation sentinel for a new background operation.
    ///
    /// All long-running actions go through here, so a trash confirmation, an
    /// undo or a rename can no longer bypass the exclusive guard used by paste
    /// and drop. `None` means another operation still owns the sentinel.
    fn begin_op(&mut self, cx: &mut Context<Self>) -> Option<OpToken> {
        if self.op_task.is_some() {
            self.status = STATUS_OP_RUNNING.to_string();
            cx.notify();
            return None;
        }
        Some(install_op_token(&mut self.op_token, &mut self.op_cancel))
    }

    /// Whether `token` still identifies an active operation owning the sentinel.
    fn is_current_op(&self, token: &OpToken) -> bool {
        op_is_active(self.op_token, &self.op_cancel, token)
    }

    /// Clear the sentinel only if `token` still owns it; a stale completion
    /// must not clear a newer operation's flag or drop its task handle.
    /// Returns `true` when the caller may apply its result.
    fn finish_op(&mut self, token: &OpToken, had_progress: bool) -> bool {
        if !clear_op_token(self.op_token, &mut self.op_cancel, token) {
            return false;
        }
        self.op_task = None;
        if had_progress {
            self.op_progress = None;
        }
        true
    }

    /// Cancel the running operation, if any (Esc). Shared by every action on
    /// the `op_task` / `op_cancel` pipeline, not just paste.
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
            self.status = format!("{}{STATUS_MISSING_COUNT_SUFFIX}", missing.len());
            self.reload(cx);
            return;
        }
        if paths.is_empty() {
            self.status = if same_dir > 0 {
                STATUS_SAME_DIRECTORY_NO_MOVE.to_string()
            } else {
                STATUS_NO_CLIPBOARD.to_string()
            };
            cx.notify();
            return;
        }

        // Z03: 混在ケースでは同フォルダのスキップも最終通知に残す。
        // 全件が同フォルダのときは上の専用 STATUS が返るため、ここに来る
        // `same_dir` は必ず「転送する項目がある」場合の一部である。
        let mut parts: Vec<(&str, usize)> = Vec::new();
        if !missing.is_empty() {
            parts.push((STATUS_SKIP_REASON_MISSING, missing.len()));
        }
        if same_dir > 0 {
            parts.push((STATUS_SKIP_REASON_SAME_DIRECTORY, same_dir));
        }
        let skip_notice = transfer_skip_notice(&parts);
        self.run_transfer(cwd.clone(), paths, cut, skip_notice, cx);
    }

    /// Copy (`cut == false`) or move (`cut == true`) `paths` into `destination`
    /// on the background executor.
    ///
    /// `skip_notice` is a pre-formatted final-status suffix (see
    /// [`transfer_skip_notice`]) for entries the caller filtered out before the
    /// transfer. It must be passed here: `status` is replaced by the progress
    /// message immediately below, so setting it at the call site would lose it.
    ///
    /// Safety: destinations always use [`ConflictPolicy::Rename`], so an
    /// existing file is never overwritten; `Esc` cancels cooperatively.
    fn run_transfer(
        &mut self,
        destination: PathBuf,
        paths: Vec<PathBuf>,
        cut: bool,
        skip_notice: String,
        cx: &mut Context<Self>,
    ) {
        let Some(token) = self.begin_op(cx) else {
            return;
        };
        let verb = if cut { "移動" } else { "コピー" };
        let target_prefix = if destination == self.cwd {
            String::new()
        } else {
            format!("「{}」へ ", display_path(&destination))
        };
        self.status = format!("{verb}{STATUS_OP_IN_PROGRESS_SUFFIX}");
        cx.notify();

        // Byte progress from the background copy/move is forwarded through a
        // channel and throttled to ~80 ms so the UI is not flooded with
        // notifications. The pump stops when the operation drops its sender.
        let (progress_tx, progress_rx) = async_channel::unbounded::<(u64, u64)>();
        let progress_verb = verb;
        let progress_token = token.clone();
        let task_cancel = token.cancel.clone();
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
                    // Ignore progress that arrives after this operation settled
                    // or lost the sentinel to a newer one (the final status
                    // message must not be overwritten).
                    if !this.is_current_op(&progress_token) {
                        return;
                    }
                    this.op_progress = Some((done, total));
                    this.status = transfer_progress_status(progress_verb, done, total);
                    cx.notify();
                });
            }
        })
        .detach();

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
                if !this.finish_op(&token, true) {
                    return;
                }
                let cancelled = matches!(first_error, Some(OpError::Cancelled));
                let succeeded = stats.files + stats.dirs > 0;
                let level = transfer_log_level(&first_error, cancelled);
                // An error after at least one item was transferred is only a
                // partial failure; a total failure stays at error level.
                let level = if level == Level::ERROR && succeeded {
                    Level::WARN
                } else {
                    level
                };
                let outcome = if cancelled {
                    "cancelled"
                } else if first_error.is_none() {
                    "success"
                } else if succeeded {
                    "partial_failure"
                } else {
                    "failure"
                };
                let error_text = first_error
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_default();
                transfer_event!(
                    level,
                    op = verb,
                    outcome,
                    success = !cancelled && first_error.is_none(),
                    files = stats.files,
                    dirs = stats.dirs,
                    bytes = stats.bytes,
                    skipped = stats.skipped.len(),
                    error = %error_text,
                    "file operation finished"
                );
                // A cut/move clipboard may only be dropped when every source
                // was moved. `first_error` is consumed by the match below, so
                // snapshot that fact first (see the clipboard handling below).
                let fully_succeeded = first_error.is_none() && stats.skipped.is_empty();
                let status = match first_error {
                    Some(OpError::Cancelled) => STATUS_CANCELLED.to_string(),
                    Some(error) if succeeded => {
                        format!("{verb}{STATUS_PARTIAL_FAILURE_SUFFIX}{error}）")
                    }
                    Some(error) => format!("{verb}{STATUS_OP_FAILED_SUFFIX}: {error}"),
                    None => summarize_op(verb, &stats),
                };
                let status = format!("{status}{skip_notice}");
                this.suppress_for(SELF_OP_SUPPRESS);
                if cut && fully_succeeded {
                    // Only a fully completed move makes every source stale. On
                    // cancel or partial failure some sources still exist; keep
                    // the slot so the user can retry (paste re-checks which
                    // paths are gone). Reporting the exact remaining paths
                    // would need per-item results from the background task;
                    // the conservative full-success rule is the minimal safe
                    // choice and never loses an unmoved source.
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

    /// Copy paths dragged from another row (possibly a whole multi-selection)
    /// into `destination`. Internal drops reuse the OS-drop pipeline: always a
    /// copy with `Rename` conflicts, and Esc cancels the running transfer via
    /// the shared operation sentinel.
    fn drop_internal_into(
        &mut self,
        paths: Vec<PathBuf>,
        destination: PathBuf,
        cx: &mut Context<Self>,
    ) {
        let paths = internal_drop_paths(&paths, &destination);
        self.drop_external_into(destination, paths, cx);
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
                STATUS_SPECIAL_FILE_NO_COPY.to_string()
            } else {
                STATUS_DROP_EMPTY.to_string()
            };
            cx.notify();
            return;
        }
        let mut notices: Vec<(&str, usize)> = Vec::new();
        if missing > 0 {
            notices.push((STATUS_SKIP_REASON_MISSING, missing));
        }
        if special > 0 {
            notices.push((STATUS_SKIP_REASON_SPECIAL, special));
        }
        // Z03: 混在ドロップでは同フォルダのスキップも理由として残す。
        if same > 0 {
            notices.push((STATUS_SKIP_REASON_SAME_DIRECTORY, same));
        }
        // Pass the notices to `run_transfer`: assigning `status` here would be
        // overwritten by its immediate "コピー中…" update, and the skip reasons
        // would never reach the final status.
        let skip_notice = transfer_skip_notice(&notices);
        self.run_transfer(destination, existing, false, skip_notice, cx);
    }

    // ------------------------------------------------------------------ undo

    /// Overlay listing what can still be undone/redone (read-only).
    fn undo_history_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.undo_history_open {
            return None;
        }
        let p = self.palette;
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
            .bg(rgb(p.surface))
            .rounded_lg()
            .border_1()
            .border_color(rgb(p.overlay))
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
                    .text_color(rgb(p.muted))
                    .child(format!("{STATUS_UNDO_EMPTY}{STATUS_UNDO_HISTORY_HINT}")),
            );
        }
        if !undo_actions.is_empty() {
            body = body.child(div().text_xs().text_color(rgb(p.muted)).child(format!(
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
                                .text_color(rgb(p.text))
                                .child(action.label()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(rgb(p.subtext))
                                .child(Self::undo_action_detail(action)),
                        ),
                );
            }
        }
        if !redo_actions.is_empty() {
            body = body.child(div().text_xs().text_color(rgb(p.muted)).child(format!(
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
                                .text_color(rgb(p.text))
                                .child(action.label()),
                        )
                        .child(
                            div()
                                .flex_1()
                                .min_w_0()
                                .truncate()
                                .text_xs()
                                .text_color(rgb(p.subtext))
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
                .text_color(rgb(p.muted))
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
                let report_path = path.clone();
                // Both the deletion and the listing are blocking trash I/O.
                let background = cx.background_spawn(async move {
                    match trash_view::delete(&path) {
                        Ok(()) => Ok(record_trashed_paths(std::slice::from_ref(&path))),
                        Err(error) => Err(error),
                    }
                });
                cx.spawn(async move |this, cx| match background.await {
                    Ok(recorded) => {
                        eprintln!(
                            "undo-report: trashed {} recorded={}",
                            report_path.display(),
                            recorded.len()
                        );
                        if recorded.is_empty() {
                            return;
                        }
                        let _ = this.update(cx, |this, cx| {
                            this.undo.push(UndoAction::Trash { items: recorded });
                            this.undo_last(cx);
                        });
                    }
                    Err(error) => eprintln!("undo-report: trash failed: {error}"),
                })
                .detach();
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
            self.status = STATUS_UNDO_EMPTY.to_string();
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
                if !path_present(&current) {
                    self.status = undo_missing_status(&current);
                    // Keep the action so a later retry or other undo is possible.
                    self.undo.push_undo(retry.clone());
                    cx.notify();
                    return;
                }
                if path_present(&target) {
                    self.status = format!(
                        "{STATUS_UNDO_FAILED}: {}{STATUS_UNDO_EXISTS_SUFFIX}",
                        display_path(&target)
                    );
                    self.undo.push_undo(retry.clone());
                    cx.notify();
                    return;
                }
                let Some(token) = self.begin_op(cx) else {
                    self.undo.push_undo(retry.clone());
                    return;
                };
                self.status = format!("{label}を元に戻しています…{STATUS_CANCEL_HINT}");
                cx.notify();
                let rename_source = current.clone();
                let cancel = token.cancel.clone();
                let background = cx.background_spawn(async move {
                    if cancel.is_cancelled() {
                        None
                    } else {
                        Some(ops::rename_entry(&rename_source, &name))
                    }
                });
                let action_for_redo = UndoAction::Rename { from, to: current };
                let task = cx.spawn(async move |this, cx| {
                    let result = background.await;
                    let _ = this.update(cx, |this, cx| {
                        if !this.finish_op(&token, false) {
                            return;
                        }
                        match result {
                            Some(Ok(restored)) => {
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
                            None => {
                                this.undo.push_undo(action_for_redo);
                                this.status = STATUS_CANCELLED.to_string();
                            }
                            Some(Err(error)) => {
                                this.undo.push_undo(action_for_redo);
                                this.status = format!("{STATUS_UNDO_FAILED}: {error}");
                            }
                        }
                        cx.notify();
                    });
                });
                self.op_task = Some(task);
            }
            UndoAction::Trash { items } => {
                let action_for_redo = UndoAction::Trash {
                    items: items.clone(),
                };
                let Some(token) = self.begin_op(cx) else {
                    self.undo.push_undo(retry);
                    return;
                };
                self.status = format!("{label}を元に戻しています…{STATUS_CANCEL_HINT}");
                cx.notify();
                let cancel = token.cancel.clone();
                let background = cx.background_spawn(async move {
                    if cancel.is_cancelled() {
                        None
                    } else {
                        Some(trash_view::restore_by_id(&items))
                    }
                });
                let task = cx.spawn(async move |this, cx| {
                    let result = background.await;
                    let _ = this.update(cx, |this, cx| {
                        if !this.finish_op(&token, false) {
                            return;
                        }
                        match result {
                            Some(Ok((restored, missing))) => {
                                this.suppress_for(SELF_OP_SUPPRESS);
                                if restored > 0 {
                                    this.undo.push_redo(action_for_redo);
                                } else {
                                    // 何も復元されなかった場合は pop を取り消し、undo
                                    // エントリをスタックへ戻して再試行可能にする。
                                    this.undo.push_undo(action_for_redo);
                                }
                                eprintln!(
                                    "undo-report: trash undone, restored={restored} missing={missing}"
                                );
                                this.status = trash_undo_status(label, restored, missing);
                                this.reload(cx);
                            }
                            None => {
                                this.undo.push_undo(action_for_redo);
                                this.status = STATUS_CANCELLED.to_string();
                            }
                            Some(Err(error)) => {
                                this.undo.push_undo(action_for_redo);
                                this.status = format!("{STATUS_UNDO_FAILED}: {error}");
                            }
                        }
                        cx.notify();
                    });
                });
                self.op_task = Some(task);
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
            self.status = STATUS_REDO_EMPTY.to_string();
            cx.notify();
            return;
        };
        let label = action.label();
        let retry = action.clone();
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
                let Some(token) = self.begin_op(cx) else {
                    self.undo.push_redo(retry);
                    return;
                };
                self.status = format!("{label}をやり直しています…{STATUS_CANCEL_HINT}");
                cx.notify();
                let cancel = token.cancel.clone();
                let background = cx.background_spawn(async move {
                    if cancel.is_cancelled() {
                        None
                    } else {
                        Some(ops::rename_entry(&current, &name))
                    }
                });
                let task = cx.spawn(async move |this, cx| {
                    let result = background.await;
                    let _ = this.update(cx, |this, cx| {
                        if !this.finish_op(&token, false) {
                            return;
                        }
                        match result {
                            Some(Ok(redone)) => {
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
                            None => {
                                this.undo.push_redo(action_for_undo);
                                this.status = STATUS_CANCELLED.to_string();
                            }
                            Some(Err(error)) => {
                                this.undo.push_redo(action_for_undo);
                                this.status = format!("{STATUS_REDO_FAILED}: {error}");
                            }
                        }
                        cx.notify();
                    });
                });
                self.op_task = Some(task);
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
                let Some(token) = self.begin_op(cx) else {
                    self.undo.push_redo(retry);
                    return;
                };
                self.status = format!("{label}をやり直しています…{STATUS_CANCEL_HINT}");
                cx.notify();
                let cancel = token.cancel.clone();
                // Keep the partial result even when cancelled: items already
                // moved to the trash must still be recorded so they can be
                // undone, exactly like the normal (non-redo) trash path.
                let background = cx.background_spawn(async move { trash_batch(paths, &cancel) });
                let task = cx.spawn(async move |this, cx| {
                    let (trashed, failures, cancelled) = background.await;
                    // Resolve the newly created trash ids on the background
                    // executor; the UI thread only consumes the final mapping.
                    let recorded = if trashed.is_empty() {
                        Vec::new()
                    } else {
                        let paths = trashed.clone();
                        cx.background_spawn(async move { record_trashed_paths(&paths) }).await
                    };
                    let _ = this.update(cx, |this, cx| {
                        if !this.finish_op(&token, false) {
                            return;
                        }
                        let failed = trashed.is_empty();
                        if cancelled && failed {
                            // Pure cancellation: nothing was moved, so keep the
                            // redo entry and report only the cancellation.
                            this.undo.push_redo(action_for_undo);
                            this.status = STATUS_CANCELLED.to_string();
                            tracing::debug!(
                                op = "redo",
                                action = "trash",
                                outcome = "cancelled",
                                success = false,
                                "undo/redo operation finished"
                            );
                            cx.notify();
                            return;
                        }
                        let partial = !failed && !failures.is_empty();
                        let level = if cancelled {
                            Level::DEBUG
                        } else if failed {
                            Level::ERROR
                        } else if partial {
                            Level::WARN
                        } else {
                            Level::INFO
                        };
                        transfer_event!(
                            level,
                            op = "redo",
                            action = "trash",
                            outcome = if failed {
                                "failure"
                            } else if cancelled {
                                "cancelled"
                            } else if partial {
                                "partial_failure"
                            } else {
                                "success"
                            },
                            success = !failed && failures.is_empty() && !cancelled,
                            items = trashed.len(),
                            failures = failures.len(),
                            "undo/redo operation finished"
                        );
                        if failed {
                            this.undo.push_redo(action_for_undo);
                            this.status = STATUS_REDO_FAILED.to_string();
                        } else {
                            this.suppress_for(SELF_OP_SUPPRESS);
                            // Always record the redo, even when the trash
                            // listing fails: otherwise the operation silently
                            // vanishes from the undo stack (F6-15).
                            this.undo.push_undo(UndoAction::Trash { items: recorded });
                            this.status = if cancelled {
                                trash_redo_cancelled_status(trashed.len())
                            } else if failures.is_empty() {
                                format!("{label}をやり直しました（{} 件）", trashed.len())
                            } else {
                                let (path, reason) = &failures[0];
                                let name = path
                                    .file_name()
                                    .map(|name| name.to_string_lossy().into_owned())
                                    .unwrap_or_else(|| display_path(path));
                                format!(
                                    "{label}をやり直しました（{} 件、{}{STATUS_TRASH_PARTIAL_FAILED}「{}」: {}）",
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
                });
                self.op_task = Some(task);
            }
        }
        cx.notify();
    }

    // ----------------------------------------------------------------- preview

    fn open_preview_selected(&mut self, cx: &mut Context<Self>) {
        if self.view != ViewSource::Directory {
            self.status = STATUS_PREVIEW_ONLY_DIRECTORY.to_string();
            cx.notify();
            return;
        }
        let Some(entry) = self.entries.get(self.selected) else {
            return;
        };
        if entry.is_dir {
            self.status = STATUS_FOLDER_NO_PREVIEW.to_string();
            cx.notify();
            return;
        }
        if entry.is_symlink {
            self.status = STATUS_SYMLINK_NO_PREVIEW.to_string();
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
        let serial = self.preview_serial.wrapping_add(1);
        self.preview_serial = serial;
        let expected_path = path.clone();
        self.preview = Some(PreviewState {
            path: path.clone(),
            serial,
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
                // Consecutive navigation replaces the overlay quickly. A read
                // from an older file must never paint over the newer one.
                if preview.serial != serial || preview.path != expected_path {
                    return;
                }
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

    /// Move to the next/previous previewable file (F6-34). Folders and symlinks
    /// are skipped; the list selection follows so closing the overlay leaves
    /// the cursor on the file that was last shown.
    fn move_preview(&mut self, delta: isize, cx: &mut Context<Self>) {
        let current = self.preview.as_ref().and_then(|preview| {
            self.entries
                .iter()
                .position(|entry| entry.path == preview.path)
        });
        let Some(index) = preview::step_preview_index(&self.entries, current, delta) else {
            return;
        };
        let entry = &self.entries[index];
        let path = entry.path.clone();
        let title = entry.name_text.clone();
        self.select_single_index(index, cx);
        self.open_preview(path, title, cx);
    }

    /// Open the previewed file in a new tab (F6-34): tabs hold directories, so
    /// the file's parent becomes the new tab and the file is selected there.
    fn preview_open_in_new_tab(&mut self, cx: &mut Context<Self>) {
        let Some(path) = self.preview.as_ref().map(|preview| preview.path.clone()) else {
            return;
        };
        let Some(parent) = preview::preview_parent(&path).map(Path::to_path_buf) else {
            self.status = format!("{STATUS_FILE_NO_NEW_TAB_PREFIX}{STATUS_CANNOT_OPEN_SUFFIX}");
            cx.notify();
            return;
        };
        if !parent.is_dir() {
            self.status = format!("{STATUS_PATH_GONE}: {}", display_path(&parent));
            cx.notify();
            return;
        }
        self.sync_active_tab();
        self.tabs.new_tab(parent.clone());
        let tab = self.tabs.active().clone();
        self.close_preview(cx);
        let saved = self.load_tab(tab, cx);
        // `load_tab` clears pending selection; set it afterwards so the file
        // that was previewed is focused in the freshly opened tab.
        self.pending_select = Some(path);
        self.status = settings_write_status(
            &self.settings,
            saved,
            true,
            format!("新しいタブで開きました: {}", display_path(&parent)),
        );
        cx.notify();
    }

    /// Read-only preview overlay with a scrollable text area.
    fn preview_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        let preview = self.preview.as_ref()?;
        let p = self.palette;
        let close = cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.close_preview(cx);
        });
        let prev = cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.move_preview(-1, cx);
        });
        let next = cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.move_preview(1, cx);
        });
        let new_tab = cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.preview_open_in_new_tab(cx);
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
            LOADING.to_string()
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
            .bg(rgb(p.surface))
            .rounded_lg()
            .border_1()
            .border_color(rgb(p.overlay))
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
                                rgb(p.red)
                            } else {
                                rgb(p.muted)
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
                    .bg(rgb(p.base))
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
                    .bg(rgb(p.base))
                    .rounded_md()
                    .p_3()
                    .font_family("monospace")
                    .text_xs()
                    .text_color(rgb(p.text))
                    .child(body_text),
            );
        }
        let positions = preview::previewable_indices(&self.entries);
        let counter = positions
            .iter()
            .position(|&index| {
                self.entries.get(index).map(|entry| &entry.path) == Some(&preview.path)
            })
            .map(|position| format!("{} / {}", position + 1, positions.len()))
            .unwrap_or_default();
        panel = panel.child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .child(
                    div()
                        .flex()
                        .gap_2()
                        .child(Button::new("preview-prev").label("前へ").on_click(prev))
                        .child(Button::new("preview-next").label("次へ").on_click(next))
                        .child(
                            Button::new("preview-new-tab")
                                .label("新タブで開く")
                                .on_click(new_tab),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(div().text_xs().text_color(rgb(p.muted)).child(counter))
                        .child(Button::new("preview-close").label("閉じる").on_click(close)),
                ),
        );
        panel = panel.child(div().text_xs().text_color(rgb(p.muted)).child(
            "↑/↓ で前後のファイル、Ctrl+Enter で新タブ、Space / F3 / Esc で閉じる（読み取り専用）",
        ));
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
            self.status = STATUS_LOCATION_ONLY_DIRECTORY.to_string();
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
        let subscription =
            cx.subscribe(&input, |this, _input, event: &InputEvent, cx| match event {
                InputEvent::Change => {
                    if let Some(location) = this.location.as_mut() {
                        if clear_dialog_error(&mut location.error) {
                            cx.notify();
                        }
                    }
                }
                InputEvent::PressEnter { .. } => this.confirm_location(cx),
                _ => {}
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
        let p = self.palette;
        let mut bar = div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_1()
            .bg(rgb(p.mantle))
            .border_b_1()
            .border_color(rgb(p.surface))
            .child(div().text_sm().text_color(rgb(p.subtext)).child("パス:"))
            .child(div().flex_1().child(Input::new(&location.input)))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(p.muted))
                    .child("Enter で移動 / Esc で閉じる"),
            );
        if let Some(error) = &location.error {
            bar = bar.child(div().text_xs().text_color(rgb(p.red)).child(error.clone()));
        }
        Some(bar)
    }

    // -------------------------------------------------------------- fuzzy jump

    /// Ctrl+P: build the candidate list from cached state and open the overlay.
    /// No filesystem walk happens here; only confirming a candidate navigates.
    fn open_fuzzy_jump(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.view != ViewSource::Directory {
            self.status = STATUS_FUZZY_JUMP_ONLY_DIRECTORY.to_string();
            cx.notify();
            return;
        }
        if self.fuzzy.is_some() {
            return;
        }
        let candidates = self.build_fuzzy_candidates();
        let results = rank_fuzzy(&candidates, "", FUZZY_MAX_RESULTS);
        let input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder(format!("ファイル名／フォルダ名で{FUZZY_JUMP_LABEL}…"))
        });
        let subscription =
            cx.subscribe(&input, |this, input, event: &InputEvent, cx| match event {
                InputEvent::Change => {
                    let query = input.read(cx).value().to_string();
                    this.apply_fuzzy_query(&query, cx);
                }
                InputEvent::PressEnter { .. } => this.confirm_fuzzy(cx),
                _ => {}
            });
        let focus = input.read(cx).focus_handle(cx).clone();
        focus.focus(window, cx);
        self.fuzzy = Some(FuzzyState {
            input,
            _subscription: subscription,
            candidates,
            results,
            selected: 0,
        });
        cx.notify();
    }

    /// Projection of the cached tabs / bookmarks / recent / ancestors /
    /// entries into ranked-overlay candidates. Pure except for reading the
    /// already-in-memory `Explorer` fields (F6-51).
    fn build_fuzzy_candidates(&self) -> Vec<FuzzyCandidate> {
        let labels: Vec<(PathBuf, String)> = self
            .bookmarks
            .iter()
            .filter_map(|path| {
                self.settings
                    .bookmark_label(path)
                    .map(|label| (path.clone(), label.to_string()))
            })
            .collect();
        let tabs: Vec<PathBuf> = self.tabs.tabs().iter().map(|tab| tab.cwd.clone()).collect();
        fuzzy_candidates(
            &self.cwd,
            &tabs,
            &self.bookmarks,
            &self.recent,
            &self.entries,
            &labels,
        )
    }

    fn close_fuzzy(&mut self, cx: &mut Context<Self>) {
        self.fuzzy = None;
        self.focus_list_on_next_render = true;
        cx.notify();
    }

    fn apply_fuzzy_query(&mut self, query: &str, cx: &mut Context<Self>) {
        if let Some(fuzzy) = self.fuzzy.as_mut() {
            fuzzy.results = rank_fuzzy(&fuzzy.candidates, query, FUZZY_MAX_RESULTS);
            fuzzy.selected = 0;
        }
        cx.notify();
    }

    fn move_fuzzy_selection(&mut self, delta: isize, cx: &mut Context<Self>) {
        if let Some(fuzzy) = self.fuzzy.as_mut() {
            if fuzzy.results.is_empty() {
                return;
            }
            let len = fuzzy.results.len() as isize;
            fuzzy.selected = ((fuzzy.selected as isize + delta).rem_euclid(len)) as usize;
        }
        cx.notify();
    }

    /// Enter: confirm the highlighted candidate. The path is cloned first so
    /// the `self.fuzzy` borrow ends before navigation mutates `self`.
    fn confirm_fuzzy(&mut self, cx: &mut Context<Self>) {
        let path = {
            let Some(fuzzy) = self.fuzzy.as_ref() else {
                return;
            };
            let Some(index) = fuzzy.results.get(fuzzy.selected) else {
                return;
            };
            let Some(candidate) = fuzzy.candidates.get(*index) else {
                return;
            };
            candidate.path.clone()
        };
        self.confirm_fuzzy_path(path, cx);
    }

    /// Navigate to a chosen candidate. Files reveal themselves by selecting
    /// the row in their parent directory (the same mechanism as "new folder"),
    /// so no external application is launched from a jump.
    fn confirm_fuzzy_path(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.fuzzy = None;
        self.focus_list_on_next_render = true;
        if path.is_dir() {
            self.navigate_to(path.clone(), cx);
            self.status = format!("{FUZZY_JUMP_LABEL}: {}", display_path(&path));
        } else if let Some(parent) = path.parent() {
            let parent = parent.to_path_buf();
            if parent.is_dir() {
                self.navigate_to(parent, cx);
                // `set_cwd` clears the pending selection, so set it afterwards.
                self.pending_select = Some(path.clone());
                self.status = format!("{FUZZY_JUMP_LABEL}: {}", display_path(&path));
            } else {
                self.status = format!("{STATUS_PATH_GONE}: {}", display_path(&path));
            }
        } else {
            self.status = format!("{STATUS_PATH_GONE}: {}", display_path(&path));
        }
        cx.notify();
    }

    /// Ctrl+P overlay: input, ranked list and source badges. Clicking a row
    /// confirms that exact path (not the current index), so a stale click
    /// cannot select a different result after the query changed.
    fn fuzzy_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        let fuzzy = self.fuzzy.as_ref()?;
        let p = self.palette;
        let close = cx.listener(|this, _: &ClickEvent, _window, cx| this.close_fuzzy(cx));
        let backdrop = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(gpui::black().opacity(0.45))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _window, cx| this.close_fuzzy(cx)),
            );
        let mut list = div()
            .id("fuzzy-results")
            .flex()
            .flex_col()
            .gap_1()
            .max_h(px(380.0))
            .overflow_y_scroll();
        if fuzzy.results.is_empty() {
            list = list.child(
                div()
                    .px_2()
                    .py_1()
                    .text_sm()
                    .text_color(rgb(p.muted))
                    .child(format!(
                        "{}（タブ・ブックマーク・最近・階層・このフォルダを検索）",
                        EMPTY_SEARCH.0
                    )),
            );
        }
        for (rank, index) in fuzzy.results.iter().enumerate() {
            let Some(candidate) = fuzzy.candidates.get(*index) else {
                continue;
            };
            let path = candidate.path.clone();
            let click = cx.listener(move |this: &mut Self, _: &ClickEvent, _window, cx| {
                this.confirm_fuzzy_path(path.clone(), cx);
            });
            let selected = rank == fuzzy.selected;
            list = list.child(
                div()
                    .id(("fuzzy-result", rank))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .bg(if selected {
                        rgb(p.overlay)
                    } else {
                        rgb(p.mantle)
                    })
                    .on_click(click)
                    .child(
                        div()
                            .w(px(96.0))
                            .text_xs()
                            .text_color(if selected {
                                rgb(p.yellow)
                            } else {
                                rgb(p.muted)
                            })
                            .child(candidate.source.label()),
                    )
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_sm()
                            .text_color(rgb(p.text))
                            .child(candidate.label.clone()),
                    )
                    .child(
                        div()
                            .max_w(px(300.0))
                            .truncate()
                            .text_xs()
                            .text_color(rgb(p.subtext))
                            .child(display_path(&candidate.path)),
                    ),
            );
        }
        let body = div()
            .id("fuzzy-dialog")
            .w(px(760.0))
            .bg(rgb(p.surface))
            .rounded_lg()
            .border_1()
            .border_color(rgb(p.overlay))
            .shadow_lg()
            .p_5()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child(FUZZY_JUMP_LABEL),
            )
            .child(Input::new(&fuzzy.input).w_full())
            .child(list)
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(p.muted))
                            .child("↑/↓ で選択 / Enter で移動 / Esc で閉じる"),
                    )
                    .child(Button::new("fuzzy-close").label("閉じる").on_click(close)),
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

    // -------------------------------------------------------------------- help

    /// F1 keyboard-help overlay (read-only).
    fn help_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        if !self.help_open {
            return None;
        }
        let p = self.palette;
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
            list = list.child(div().text_xs().text_color(rgb(p.muted)).child(*section));
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
                                .text_color(rgb(p.text))
                                .child(*key),
                        )
                        .child(
                            div()
                                .flex_1()
                                .text_sm()
                                .text_color(rgb(p.subtext))
                                .child(*description),
                        ),
                );
            }
        }
        let overrides_line = help_overrides_line(&self.keymap);
        let mut body = div()
            .w(px(720.0))
            .bg(rgb(p.surface))
            .rounded_lg()
            .border_1()
            .border_color(rgb(p.overlay))
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
            );
        // Static HELP_SECTIONS cannot follow a user rebind. Surface the live
        // overrides above the table instead of rewriting every static row.
        if let Some(line) = overrides_line {
            body = body.child(div().text_sm().text_color(rgb(p.yellow)).child(line));
        }
        let body = body.child(list).child(
            div()
                .flex()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(p.muted))
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
    /// The blocking metadata reads run on the background executor; only the
    /// finished snapshot is applied here (never from render).
    fn open_properties(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.open_properties_impl(path, cx, false);
    }

    /// Debug-hook variant: kick off the recursive size scan as soon as the
    /// metadata snapshot lands, preserving the old synchronous hand-off to
    /// `start_dir_size` without blocking the UI thread.
    fn open_properties_with_size(&mut self, path: PathBuf, cx: &mut Context<Self>) {
        self.open_properties_impl(path, cx, true);
    }

    fn open_properties_impl(&mut self, path: PathBuf, cx: &mut Context<Self>, start_size: bool) {
        // Every open and close bumps the serial. A metadata read from a
        // superseded open must not resurrect the dialog, overwrite a newer
        // snapshot, or fire a late status/reload from its error path.
        let serial = self.props_serial.wrapping_add(1);
        self.props_serial = serial;
        // Dropping the previous waiter cancels its pending read as well.
        self.props_read_task = None;

        let read_path = path.clone();
        let background = cx.background_spawn(async move { read_properties_snapshot(&read_path) });
        let task = cx.spawn(async move |this, cx| {
            let result = background.await;
            let _ = this.update(cx, |this, cx| {
                if !should_apply_properties_read(this.props_serial, serial) {
                    return;
                }
                let snapshot = match result {
                    Ok(snapshot) => snapshot,
                    Err(error) => {
                        let reaction = PropertiesErrorReaction::for_error(&error);
                        let (status, reload) = reaction.apply(&path, &error);
                        this.status = status;
                        // A permission failure or any other io error must not
                        // discard the visible listing (only a gone path does).
                        if reload {
                            this.reload(cx);
                        }
                        return;
                    }
                };
                let kind = if snapshot.is_symlink {
                    "シンボリックリンク"
                } else if snapshot.is_dir {
                    "フォルダ"
                } else {
                    "ファイル"
                };
                let name = path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| display_path(&path));
                // Reopening (possibly for a different path) supersedes any
                // recursive size scan started by the previously open dialog:
                // cancel it and drop its task before installing new state.
                if let Some(old) = this.properties.as_mut() {
                    if let Some(cancel) = old.size_cancel.take() {
                        cancel.cancel();
                    }
                    old.size_task = None;
                }
                this.properties = Some(PropsState {
                    name,
                    path,
                    kind,
                    is_dir: snapshot.is_dir,
                    size: snapshot.size,
                    modified: snapshot.modified,
                    permissions: snapshot.permissions,
                    symlink_target: snapshot.symlink_target,
                    size_loading: false,
                    dir_size: None,
                    size_task: None,
                    size_cancel: None,
                });
                if start_size {
                    this.start_dir_size(cx);
                }
                cx.notify();
            });
        });
        // Keep the waiter alive until the read lands; a later open replaces it
        // and a close drops it, cancelling any superseded read.
        self.props_read_task = Some(task);
    }

    fn open_selected_properties(&mut self, cx: &mut Context<Self>) {
        if self.view != ViewSource::Directory {
            self.status = STATUS_PROPERTIES_ONLY_DIRECTORY.to_string();
            cx.notify();
            return;
        }
        let Some(entry) = self.entries.get(self.selected) else {
            return;
        };
        self.open_properties(entry.path.clone(), cx);
    }

    fn close_properties(&mut self, cx: &mut Context<Self>) {
        // Invalidate any in-flight metadata read first: its late completion
        // must not resurrect the dialog or trigger a status/reload after close.
        self.props_serial = self.props_serial.wrapping_add(1);
        self.props_read_task = None;
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
        let check_path = path.clone();
        let cancel = Arc::new(CancelFlag::new());
        let scan_cancel = cancel.clone();
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
                // Only the scan still represented by the current state may
                // write back. If the dialog was closed/reopened for another
                // file (or a fresh scan replaced this one), these stats are
                // stale and must not clobber the current snapshot/task.
                let current = props.path == check_path
                    && props
                        .size_cancel
                        .as_ref()
                        .is_some_and(|active| Arc::ptr_eq(active, &scan_cancel));
                if !current {
                    return;
                }
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
        let p = self.palette;
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
                        .text_color(rgb(p.muted))
                        .child(label),
                )
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .text_sm()
                        .text_color(rgb(p.text))
                        .child(value),
                )
        };
        let mut form = div()
            .w(px(560.0))
            .bg(rgb(p.surface))
            .rounded_lg()
            .border_1()
            .border_color(rgb(p.overlay))
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
                        .text_color(rgb(p.muted))
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
                .text_color(rgb(p.muted))
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
        // A manual F2 (or a debug hook) satisfies any queued post-creation
        // rename too; never fire a second dialog on a later render.
        self.pending_rename = None;
        let Some(entry) = self.entries.get(self.selected) else {
            return;
        };
        let target = entry.path.clone();
        let original = entry.name_text.clone();
        let input = cx.new(|cx| InputState::new(window, cx).default_value(original.clone()));
        let subscription =
            cx.subscribe(&input, |this, _input, event: &InputEvent, cx| match event {
                InputEvent::Change => {
                    if let Some(rename) = this.renaming.as_mut() {
                        if clear_dialog_error(&mut rename.error) {
                            cx.notify();
                        }
                    }
                }
                InputEvent::PressEnter { .. } => this.confirm_rename(cx),
                _ => {}
            });
        let focus = input.read(cx).focus_handle(cx).clone();
        focus.focus(window, cx);
        self.renaming = Some(RenameState {
            target,
            original,
            input,
            _subscription: subscription,
            error: None,
            op: None,
        });
        cx.notify();
    }

    /// Validate the typed name and rename on the background executor.
    ///
    /// Safety: the new name is validated, an existing destination is rejected
    /// before spawning, and the engine refuses to overwrite. On error the
    /// dialog stays open and the original file is untouched.
    fn confirm_rename(&mut self, cx: &mut Context<Self>) {
        if self.op_task.is_some() {
            self.status = STATUS_OP_RUNNING.to_string();
            cx.notify();
            return;
        }
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
            self.set_rename_error(STATUS_RENAME_NO_PARENT, cx);
            return;
        };
        let destination = parent.join(&value);
        if destination.exists() {
            self.set_rename_error(
                &format!("{STATUS_RENAME_EXISTS_PREFIX}{}", destination.display()),
                cx,
            );
            return;
        }

        let Some(token) = self.begin_op(cx) else {
            return;
        };
        // Record ownership so Esc on this dialog cancels only this op, never
        // an unrelated transfer that happened to be running when F2 was hit.
        if let Some(rename) = self.renaming.as_mut() {
            rename.op = Some(token.clone());
        }
        self.status = format!("名前を変更しています…{STATUS_CANCEL_HINT}");
        cx.notify();
        let rename_source = target.clone();
        let cancel = token.cancel.clone();
        let background = cx.background_spawn(async move {
            if cancel.is_cancelled() {
                None
            } else {
                Some(ops::rename_entry(&rename_source, &value))
            }
        });
        let rename_original = target;
        let task = cx.spawn(async move |this, cx| {
            let result = background.await;
            let _ = this.update(cx, |this, cx| {
                if !this.finish_op(&token, false) {
                    return;
                }
                match result {
                    Some(Ok(new_path)) => {
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
                    None => {
                        this.status = STATUS_CANCELLED.to_string();
                        this.close_rename(cx);
                    }
                    Some(Err(error)) => {
                        this.set_rename_error(&format!("{STATUS_RENAME_FAILED}: {error}"), cx);
                        this.status = format!("{STATUS_RENAME_FAILED}: {error}");
                    }
                }
                cx.notify();
            });
        });
        self.op_task = Some(task);
    }

    fn cancel_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        // Esc must also stop a rename that was already submitted and is now
        // running in the background, not merely close the dialog. Only cancel
        // when this dialog still owns the active op token: Esc on a dialog
        // that never submitted (or whose rename already settled) must not
        // abort an unrelated transfer.
        let owns_op = self
            .renaming
            .as_ref()
            .and_then(|rename| rename.op.as_ref())
            .is_some_and(|token| self.is_current_op(token));
        if owns_op {
            self.cancel_op(cx);
        }
        if self.renaming.take().is_some() {
            self.focus_handle.focus(window, cx);
            cx.notify();
        }
    }

    fn close_rename(&mut self, cx: &mut Context<Self>) {
        if self.renaming.take().is_some() {
            // The dialog's input is dropped here; schedule focus restoration
            // so the unchanged-name and cancelled paths don't leave focus on
            // a destroyed input (the success path schedules it as well).
            self.restore_focus_next_frame();
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
        let p = self.palette;
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
            .bg(rgb(p.surface))
            .rounded_lg()
            .border_1()
            .border_color(rgb(p.overlay))
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
                    .text_color(rgb(p.subtext))
                    .truncate()
                    .child(display_path(&rename.target)),
            )
            .child(Input::new(&rename.input).w_full());
        if let Some(error) = &rename.error {
            form = form.child(div().text_sm().text_color(rgb(p.red)).child(error.clone()));
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
                .text_color(rgb(p.muted))
                .child(DIALOG_FOOTER_HINT),
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

    /// Modal overlay for the bookmark-label editor (F6-49).
    fn label_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        let editor = self.label_editor.as_ref()?;
        let p = self.palette;
        let backdrop = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(gpui::black().opacity(0.45))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _window, cx| {
                    this.close_label_editor(cx);
                }),
            );
        let cancel = cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.close_label_editor(cx);
        });
        let confirm = cx.listener(|this, _: &ClickEvent, _window, cx| {
            this.commit_label_editor(cx);
        });
        let remove = {
            let path = editor.path.clone();
            cx.listener(move |this, _: &ClickEvent, _window, cx| {
                this.clear_bookmark_label(&path, cx);
                this.close_label_editor(cx);
            })
        };
        let form = div()
            .w(px(480.0))
            .bg(rgb(p.surface))
            .rounded_lg()
            .border_1()
            .border_color(rgb(p.overlay))
            .shadow_lg()
            .p_5()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_lg()
                    .font_weight(gpui::FontWeight::SEMIBOLD)
                    .child("ブックマークラベル"),
            )
            .child(
                div()
                    .text_sm()
                    .text_color(rgb(p.subtext))
                    .truncate()
                    .child(display_path(&editor.path)),
            )
            .child(Input::new(&editor.input).w_full())
            .child(
                div()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("label-cancel")
                            .label("キャンセル")
                            .on_click(cancel),
                    )
                    .child(Button::new("label-remove").label("削除").on_click(remove))
                    .child(Button::new("label-confirm").label("設定").on_click(confirm)),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(p.muted))
                    .child("Enter で確定 / 空にして確定すると削除 / Esc でキャンセル"),
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
        if !path_present(&path) {
            self.status = format!("{STATUS_PATH_GONE}: {}", display_path(&path));
            self.reload(cx);
            return;
        }
        cx.reveal_path(&path);
        self.status = format!("{STATUS_REVEALED_PREFIX}{}", display_path(&path));
        cx.notify();
    }

    fn trash_selected(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let paths = self.action_paths();
        if paths.is_empty() {
            self.status = STATUS_NO_SELECTION.to_string();
            cx.notify();
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
        if self.op_task.is_some() {
            self.status = STATUS_OP_RUNNING.to_string();
            cx.notify();
            return;
        }
        // The modal prompt and the background move are asynchronous. Remember
        // which listing owned these paths so a completion that lands after the
        // user switched views cannot rewrite the new listing (finding 3).
        let view_at_start = self.view;
        let mut existing: Vec<PathBuf> = Vec::new();
        for path in paths {
            if path.exists() || path.is_symlink() {
                existing.push(path);
            }
        }
        if existing.is_empty() {
            self.status = STATUS_TARGET_MISSING.to_string();
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
            &[LABEL_CANCEL, "ゴミ箱へ移動"],
            cx,
        );
        let background = cx.background_executor().clone();
        cx.spawn(async move |this, cx| {
            let answer = receiver.await.unwrap_or(0);
            if answer != 1 {
                return;
            }
            // Re-check after the modal prompt: another operation may have
            // claimed the sentinel while the user was answering.
            let Some(token) = this.update(cx, |this, cx| this.begin_op(cx)).ok().flatten() else {
                return;
            };
            let targets = existing;
            let cancel = token.cancel.clone();
            let this_for_task = this.clone();
            let task = cx.spawn(async move |cx| {
                let (trashed, failures, cancelled) = background
                    .spawn(async move { trash_batch(targets, &cancel) })
                    .await;
                // Resolve trash ids off the UI thread while the listing is
                // still fresh; the update closure only uses the final mapping.
                let recorded = if trashed.is_empty() {
                    Vec::new()
                } else {
                    let paths = trashed.clone();
                    cx.background_spawn(async move { record_trashed_paths(&paths) })
                        .await
                };
                let _ = this_for_task.update(cx, |this, cx| {
                    if !this.finish_op(&token, false) {
                        return;
                    }
                    // The modal prompt and the background move are async. Do not
                    // rewrite a listing that was not active when the paths were
                    // captured (finding 3): a switch to Search must not be
                    // clobbered by directory entries from this completion.
                    let view_unchanged = this.view == view_at_start;
                    if trashed.is_empty() && !cancelled {
                        let message = format!(
                            "{STATUS_TRASH_ALL_FAILED}: {}",
                            failures
                                .first()
                                .map(|(_, message)| message.clone())
                                .unwrap_or_default()
                        );
                        if view_unchanged && this.view != ViewSource::Search {
                            this.set_load_error(message);
                        } else {
                            this.status = message;
                        }
                    } else {
                        if !trashed.is_empty() {
                            this.suppress_for(SELF_OP_SUPPRESS);
                        }
                        let mut message = if cancelled {
                            if trashed.is_empty() {
                                STATUS_CANCELLED.to_string()
                            } else {
                                format!(
                                    "{STATUS_CANCELLED}（{} 件をゴミ箱へ移動済み）",
                                    trashed.len()
                                )
                            }
                        } else if trashed.len() == 1 {
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
                                "（{}{STATUS_TRASH_PARTIAL_FAILED}「{}」: {}）",
                                failures.len(),
                                name,
                                reason
                            ));
                        }
                        this.status = message;
                        if !recorded.is_empty() {
                            this.undo.push(UndoAction::Trash { items: recorded });
                        }
                        if view_unchanged {
                            this.all_entries
                                .retain(|entry| !trashed.contains(&entry.path));
                            this.fingerprint = fingerprint(&this.all_entries);
                            if this.view == ViewSource::Search {
                                // Preserve kind/extension/name filters: the
                                // generic rebuild would drop them, and
                                // `reload()` is a no-op in Search so it could
                                // never repair the listing (finding 3).
                                this.apply_search_filter(cx);
                            } else {
                                this.rebuild_view(None);
                            }
                            this.reload(cx);
                        } else if this.view == ViewSource::Search {
                            // The user switched to Search while the op ran.
                            // The watch was suppressed for our own change and
                            // `reload()` is a no-op in Search, so the snapshot
                            // cannot be repaired here: flag it stale instead
                            // of silently skipping the listing update.
                            this.status = STATUS_SEARCH_STALE.to_string();
                        }
                    }
                    cx.notify();
                });
            });
            let _ = this.update(cx, |this, cx| {
                this.op_task = Some(task);
                cx.notify();
            });
        })
        .detach();
    }

    // ------------------------------------------------------------------- search

    /// Whether one of the search bar's text fields currently owns the keyboard.
    /// While typing, every key except Escape belongs to the input; once the
    /// result list (or the root) has focus, navigation can reach the keymap.
    fn search_input_focused(&self, window: &Window, cx: &App) -> bool {
        [
            self.search.input.as_ref(),
            self.search.ext.as_ref(),
            self.search.name_filter.as_ref(),
            self.search.save_name.as_ref(),
        ]
        .into_iter()
        .flatten()
        .any(|input| input.read(cx).focus_handle(cx).is_focused(window))
    }

    /// Move keyboard focus to the search bar's optional name filter
    /// (Ctrl+Shift+F / `FocusSearchName`). Shared by normal keymap dispatch and
    /// the early Search-view handler so both paths behave identically.
    fn focus_search_name(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(input) = self.search.name_filter.clone() {
            let focus = input.read(cx).focus_handle(cx).clone();
            focus.focus(window, cx);
            cx.notify();
        } else {
            self.status = STATUS_SEARCH_NOT_OPEN.to_string();
            cx.notify();
        }
    }

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
            self.status = STATUS_SEARCH_ONLY_DIRECTORY.to_string();
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
        // The save-name field is read when "保存" is clicked, but Enter must
        // work there too (G06). The subscription is bound to this input only,
        // so it cannot steal Enter from the query field's `run_search`.
        let save_name = cx.new(|cx| InputState::new(window, cx));
        let save_name_subscription =
            cx.subscribe(&save_name, |this, _input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::PressEnter { .. }) {
                    this.save_current_search(cx);
                }
            });
        self.search.save_name = Some(save_name);
        self.search.save_name_subscription = Some(save_name_subscription);
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
        self.status = STATUS_SEARCH_PROMPT.to_string();
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
            self.status = STATUS_SEARCH_EMPTY_QUERY.to_string();
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
                // `write_was_skipped()` covers read-only and ephemeral sessions,
                // both of which make `save()` a no-op. Log a real failure
                // instead of silently discarding the result (Bug L01).
                if !this.settings.write_was_skipped() {
                    let saved = this.save_settings();
                    if !saved {
                        // V10: keep the recorded reason in the log as well.
                        tracing::error!(
                            reason = ?this.settings.last_save_error.get(),
                            "failed to save search history"
                        );
                    }
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
        let mut filtered = filter_results(
            &self.all_entries,
            self.search.kind,
            extension.as_deref(),
            name_filter.as_deref(),
        );
        // `all_entries` arrives path-sorted from the walker. Reuse the shared
        // comparator so the sort headers actually reorder search results,
        // keeping Directory and Search views consistent.
        sort_entries(&mut filtered, self.settings.sort.order());
        let paths: Vec<PathBuf> = filtered.iter().map(|entry| entry.path.clone()).collect();
        self.selection.retain(&paths);
        self.entries = filtered;
        self.fingerprint = fingerprint(&self.entries);
        if self.selection.is_empty() {
            if let Some(entry) = self.entries.first() {
                // Auto-selected search target, not a user pick.
                self.implicit_selection = true;
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
        self.scroll_to_entry(self.selected);
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
        self.status = search_filter_status(
            &self.status,
            format!(
                "検索{mode_label}{scope_label}: 「{}」 {} 件",
                self.search.query,
                self.entries.len()
            ),
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
        // Invalidate completions already queued on the executor: a late search
        // callback checks only `search.generation`, so without this bump it
        // could overwrite `load = Ready` / `all_entries` while the directory
        // reload triggered by leaving the search view is in flight.
        self.search.generation = self.search.generation.wrapping_add(1);
        self.search.input = None;
        self.search.subscription = None;
        self.search.ext = None;
        self.search.ext_subscription = None;
        self.search.name_filter = None;
        self.search.name_filter_subscription = None;
        self.search.save_name = None;
        self.search.save_name_subscription = None;
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
        // The search input is destroyed by `clear_search`; restore list focus
        // on the next render instead of leaving it on a removed element.
        self.restore_focus_next_frame();
        self.reload(cx);
    }

    fn search_bar(&self, cx: &mut Context<Self>) -> Option<Div> {
        let input = self.search.input.as_ref()?;
        let p = self.palette;
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
                    .when(active, |button| button.bg(rgb(p.surface)))
                    .text_color(if active { rgb(p.text) } else { rgb(p.subtext) })
                    .hover(|style| style.bg(rgb(p.surface)))
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
                    .when(active, |button| button.bg(rgb(p.surface)))
                    .text_color(if active { rgb(p.text) } else { rgb(p.subtext) })
                    .hover(|style| style.bg(rgb(p.surface)))
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
                    .when(active, |button| button.bg(rgb(p.surface)))
                    .text_color(if active { rgb(p.text) } else { rgb(p.subtext) })
                    .hover(|style| style.bg(rgb(p.surface)))
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
                .child(div().text_xs().text_color(rgb(p.muted)).child("拡張子:"))
                .child(div().w(px(110.0)).child(Input::new(ext)))
        });
        let name_input = self.search.name_filter.as_ref().map(|input| {
            div()
                .flex()
                .items_center()
                .gap_1()
                .child(div().text_xs().text_color(rgb(p.muted)).child("名前:"))
                .child(div().w(px(110.0)).child(Input::new(input)))
        });
        Some(
            div()
                .flex()
                .items_center()
                .gap_2()
                .px_3()
                .py_1()
                .bg(rgb(p.mantle))
                .border_b_1()
                .border_color(rgb(p.surface))
                .child(div().text_sm().text_color(rgb(p.subtext)).child("検索:"))
                .child(div().flex_1().child(Input::new(input)))
                .child(modes)
                .child(scopes)
                .children((self.search.mode == SearchMode::Name).then_some(kinds))
                .children(extension_input)
                .children(name_input)
                .child(div().text_xs().text_color(rgb(p.muted)).child(hint)),
        )
    }

    /// Chips with recent search queries, shown while the query field is empty.
    fn search_history_bar(&self, cx: &mut Context<Self>) -> Option<Div> {
        let input = self.search.input.as_ref()?;
        if !input.read(cx).value().is_empty() || self.settings.recent_searches.is_empty() {
            return None;
        }
        let p = self.palette;
        let mut row = div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_1()
            .bg(rgb(p.mantle))
            .border_b_1()
            .border_color(rgb(p.surface))
            .child(
                div()
                    .text_xs()
                    .text_color(rgb(p.muted))
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
                    .bg(rgb(p.base))
                    .text_xs()
                    .text_color(rgb(p.subtext))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(p.surface)).text_color(rgb(p.text)))
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

    /// Save the current query under the typed name (F6-49).
    fn save_current_search(&mut self, cx: &mut Context<Self>) {
        let (Some(name_input), Some(query_input)) =
            (self.search.save_name.clone(), self.search.input.clone())
        else {
            return;
        };
        let name = name_input.read(cx).value().trim().to_string();
        let query = query_input.read(cx).value().trim().to_string();
        let stored = if name.is_empty() || query.is_empty() {
            false
        } else {
            self.settings.save_search(&name, &query)
        };
        let saved = if stored { self.save_settings() } else { true };
        self.status = if name.is_empty() {
            format!("{STATUS_ERROR_PREFIX}{STATUS_SAVED_SEARCH_NAME_REQUIRED}")
        } else if query.is_empty() {
            format!("{STATUS_ERROR_PREFIX}{STATUS_SAVED_SEARCH_QUERY_REQUIRED}")
        } else {
            settings_write_status(
                &self.settings,
                saved,
                // A rejected save (e.g. the max-search cap) never called
                // `save()`: keep the concrete rejection reason (I02).
                stored,
                saved_search_save_status(&name, stored),
            )
        };
        cx.notify();
    }

    /// Recall a saved search: load its query into the search input and run it
    /// with the currently selected mode/scope/filters (F6-49).
    fn recall_saved_search(&mut self, name: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(query) = self
            .settings
            .saved_search(name)
            .map(|search| search.query.clone())
        else {
            self.status = format!("保存検索{STATUS_NOT_FOUND_SUFFIX}: {name}");
            cx.notify();
            return;
        };
        if let Some(input) = self.search.input.clone() {
            input.update(cx, |state, cx| state.set_value(query, window, cx));
        }
        self.run_search(cx);
    }

    /// Delete a saved search by name (F6-49).
    fn delete_saved_search(&mut self, name: &str, cx: &mut Context<Self>) {
        let removed = self.settings.remove_search(name);
        let saved = if removed { self.save_settings() } else { true };
        self.status = if removed {
            settings_write_status(
                &self.settings,
                saved,
                true,
                format!("保存検索を削除しました: {name}"),
            )
        } else {
            format!("保存検索{STATUS_NOT_FOUND_SUFFIX}: {name}")
        };
        cx.notify();
    }

    /// Bar for saving, recalling and deleting named searches. Shown together
    /// with the search bar; chips mirror `settings.saved_searches` (F6-49).
    fn saved_search_bar(&self, cx: &mut Context<Self>) -> Option<Div> {
        // Both fields live only while the search bar is open.
        self.search.input.as_ref()?;
        let name_input = self.search.save_name.as_ref()?;
        let p = self.palette;
        let mut row = div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .py_1()
            .bg(rgb(p.mantle))
            .border_b_1()
            .border_color(rgb(p.surface))
            .child(div().text_xs().text_color(rgb(p.muted)).child("保存検索:"))
            .child(div().w(px(110.0)).child(Input::new(name_input)))
            .child(
                Button::new("save-search")
                    .label("保存")
                    .on_click(cx.listener(|this, _: &ClickEvent, _window, cx| {
                        this.save_current_search(cx);
                    })),
            );
        for (index, search) in self.settings.saved_searches.iter().enumerate() {
            let name = search.name.clone();
            let recall_name = name.clone();
            let delete_name = name.clone();
            let caption = saved_search_caption(&search.name, &search.query);
            row = row
                .child(
                    div()
                        .id(("saved-search-recall", index))
                        .px_2()
                        .py_0p5()
                        .rounded_md()
                        .bg(rgb(p.base))
                        .text_xs()
                        .text_color(rgb(p.subtext))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(p.surface)).text_color(rgb(p.text)))
                        .child(caption)
                        .on_click(cx.listener(move |this, _: &ClickEvent, window, cx| {
                            this.recall_saved_search(&recall_name, window, cx);
                        })),
                )
                .child(
                    div()
                        .id(("saved-search-delete", index))
                        .aria_label(format!("{name} を削除"))
                        .px_1()
                        .py_0p5()
                        .rounded_md()
                        .text_xs()
                        .text_color(rgb(p.muted))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(p.surface)).text_color(rgb(p.red)))
                        .child("×")
                        .on_click(cx.listener(move |this, _: &ClickEvent, _window, cx| {
                            this.delete_saved_search(&delete_name, cx);
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
        let items = trash_view::items_for_paths(&self.trash_items, &paths);
        if items.is_empty() {
            self.status = STATUS_RESTORE_EMPTY.to_string();
            cx.notify();
            return;
        }
        let count = items.len();
        let Some(token) = self.begin_op(cx) else {
            return;
        };
        self.status = format!("{count} 件を復元{STATUS_OP_IN_PROGRESS_SUFFIX}");
        cx.notify();
        let cancel = token.cancel.clone();
        let background = cx.background_spawn(async move {
            if cancel.is_cancelled() {
                None
            } else {
                Some(trash_view::restore(items))
            }
        });
        let task = cx.spawn(async move |this, cx| {
            let result = background.await;
            let _ = this.update(cx, |this, cx| {
                if !this.finish_op(&token, false) {
                    return;
                }
                this.status = match result {
                    None => STATUS_CANCELLED.to_string(),
                    Some(Ok(())) => {
                        this.suppress_for(SELF_OP_SUPPRESS);
                        format!("{count} 件を元の場所へ復元しました")
                    }
                    Some(Err(error)) => format!("{STATUS_RESTORE_FAILED}: {error}"),
                };
                this.reload(cx);
                cx.notify();
            });
        });
        self.op_task = Some(task);
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
            let terminal_weak = weak.clone();
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
            let terminal_path = path.clone();
            let copy_path = path.clone();
            let props_path = path.clone();
            let trash_paths = batch_paths.clone();
            let open_label = if is_dir {
                "開く"
            } else {
                "既定のアプリで開く"
            };

            let mut menu = menu
                .item(
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
                    PopupMenuItem::new("ターミナルで開く")
                        .icon(Icon::new(IconName::Terminal))
                        .on_click(move |_, _window, cx| {
                            let target = terminal_path.clone();
                            let _ = terminal_weak.update(cx, |this, cx| {
                                // The clicked row is resolved by path, so a
                                // re-sort while the menu is open cannot retarget.
                                let dir = terminal_dir_for(&target, is_dir, &this.cwd);
                                this.open_terminal(dir, cx);
                            });
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
                            let _ =
                                props_weak.update(cx, |this, cx| this.open_properties(target, cx));
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
                );
            if is_dir {
                // Personalization entries (F6-49): bookmarks and their labels
                // are resolved by path at click time, just like the actions
                // above, so a re-sort while the menu is open cannot retarget.
                let bookmark_weak = weak.clone();
                let label_weak = weak.clone();
                let clear_label_weak = weak.clone();
                let bookmark_path = path.clone();
                let label_path = path.clone();
                let clear_label_path = path.clone();
                menu = menu
                    .separator()
                    .item(
                        PopupMenuItem::new("ブックマークを追加/解除")
                            .icon(Icon::new(IconName::Star))
                            .on_click(move |_, _window, cx| {
                                let target = bookmark_path.clone();
                                let _ = bookmark_weak
                                    .update(cx, |this, cx| this.toggle_bookmark_path(&target, cx));
                            }),
                    )
                    .item(
                        PopupMenuItem::new("ラベルを設定…")
                            .icon(Icon::new(IconName::Pencil))
                            .on_click(move |_, _window, cx| {
                                let target = label_path.clone();
                                let _ = label_weak.update_in(cx, |this, window, cx| {
                                    this.open_label_editor(target, window, cx);
                                });
                            }),
                    )
                    .item(
                        PopupMenuItem::new("ラベルを削除")
                            .icon(Icon::new(IconName::Trash))
                            .on_click(move |_, _window, cx| {
                                let target = clear_label_path.clone();
                                let _ = clear_label_weak.update(cx, |this, cx| {
                                    this.clear_bookmark_label(&target, cx);
                                });
                            }),
                    );
            }
            menu
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
        // In the trash the timestamp is the deletion time, not the mtime.
        // Compute the displayed label first so the aria label stays in sync.
        let label = if key == SortKey::Modified && self.view == ViewSource::Trash {
            "削除日時"
        } else {
            key.label()
        };
        let p = self.palette;
        let base = div()
            .id(id)
            .aria_label(format!("{} で並び替え", label))
            .h_full()
            .flex()
            .items_center()
            .px_2()
            .cursor_pointer()
            .text_color(if active { rgb(p.text) } else { rgb(p.muted) })
            .hover(|style| style.text_color(rgb(p.text)));
        let base = match width {
            Some(width) => base.w(px(width)).text_right(),
            None => base.flex_1(),
        };
        base.on_click(cx.listener(move |this, _event, window, cx| {
            this.focus_handle.focus(window, cx);
            this.set_sort(key, cx);
        }))
        .child(format!("{arrow} {label}"))
    }

    fn sort_header(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        div()
            .flex()
            .items_center()
            .gap_2()
            .px_3()
            .h(px(26.0))
            .bg(rgb(p.mantle))
            .border_b_1()
            .border_color(rgb(p.surface))
            .text_xs()
            .text_color(rgb(p.muted))
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
        let p = self.palette;
        let color = if self.load.error_message().is_some() {
            p.red
        } else {
            p.muted
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
                            hint.map(|hint| div().text_xs().text_color(rgb(p.muted)).child(hint)),
                        ),
                ),
        )
    }

    fn list_row(this: &mut Self, index: usize, cx: &mut Context<Self>) -> Option<AnyElement> {
        let entry = this.entries.get(index).cloned()?;
        // Trash rows are virtual (path = trash id), so recover the restore
        // destination from the listing snapshot for a per-row tooltip.
        let trash_parent = if this.view == ViewSource::Trash {
            trash_view::original_parent_for_path(&this.trash_items, &entry.path)
        } else {
            None
        };
        let display_name =
            display_name_for(&entry, this.view, &this.cwd, &this.search.active_roots);
        let display_name = match this.search_lines.get(&entry.path) {
            Some(line) if this.view == ViewSource::Search => format!("{display_name}:{line}"),
            _ => display_name,
        };
        // F6-49: bookmark labels prefix the row name; the real name stays
        // visible so the underlying entry can still be identified.
        let bookmark_label = this.settings.bookmark_label(&entry.path).map(str::to_owned);
        let display_name = labeled_display_name(&display_name, bookmark_label.as_deref());
        let selected = this.selection.contains(&entry.path);
        let focused = this.selected == index;
        let is_dir = entry.is_dir;
        let p = this.palette;
        // `drag_over` closures borrow their color by reference; capture plain
        // `u32` copies so they do not outlive the palette local.
        let drag_blue = p.blue;
        let (icon, icon_color) = if this.view == ViewSource::Trash {
            (IconName::Trash, p.muted)
        } else {
            (
                icon_for_entry(&entry),
                if entry.is_symlink {
                    p.muted
                } else {
                    p.kind_color(kind_for_entry(&entry))
                },
            )
        };
        // Resolve the full-colour system icon up front, before any render
        // closure captures `this`. Trash rows keep the Lucide Trash icon.
        let system_icon_path = if this.view == ViewSource::Trash {
            None
        } else {
            this.system_icon_resolver.icon_path_for_entry(&entry, 16)
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
        let middle_click = cx.listener(move |this, _event: &MouseDownEvent, window, cx| {
            this.focus_handle.focus(window, cx);
            Self::open_row_in_new_tab(this, index, cx);
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
        // U-09: the truncated name cell needs a tooltip carrying the full
        // value: the full path in Search, the full name in every other view.
        let name_tooltip_text = if this.view == ViewSource::Search {
            entry.path.display().to_string()
        } else {
            entry.name_text.clone()
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
                    rgb(p.overlay)
                } else if focused {
                    rgb(p.focus)
                } else {
                    rgb(p.base)
                })
                // U-02/U-03: the keyboard cursor gets a distinct high-contrast
                // left border. It is independent of `selected`, so the cursor
                // stays visible inside a multi-selection and differs from hover.
                .when(focused, |row| row.border_l_2().border_color(rgb(p.blue)))
                .cursor_pointer()
                .hover(|style| {
                    style.bg(if selected {
                        rgb(p.overlay)
                    } else {
                        rgb(p.focus)
                    })
                })
                .on_click(click)
                .on_mouse_down(MouseButton::Right, right_click)
                .on_mouse_down(MouseButton::Middle, middle_click)
                .when(drag_out, |row| {
                    row.on_drag(drag_paths, |_paths, _, _, cx| cx.new(|_| Empty))
                        .external_drag_payload(|paths: &Vec<PathBuf>, _window, _cx| {
                            Some(ExternalDragPayload::Files(FileDragPaths::new(
                                paths.iter().map(|path| (path.clone(), path.is_dir())),
                            )))
                        })
                })
                .when_some(drop_target, |row, target| {
                    let external_target = target.clone();
                    row.on_drop::<ExternalPaths>(cx.listener(
                        move |this, paths: &ExternalPaths, _window, cx| {
                            this.drop_external_into(
                                external_target.clone(),
                                paths.paths().to_vec(),
                                cx,
                            );
                        },
                    ))
                    // Accept rows dragged from this window too. `Vec<PathBuf>`
                    // is the value `on_drag` puts on the internal drag, so a
                    // multi-selection arrives here intact (F6-33).
                    .on_drop::<Vec<PathBuf>>(cx.listener(
                        move |this, paths: &Vec<PathBuf>, _window, cx| {
                            this.drop_internal_into(paths.clone(), target.clone(), cx);
                        },
                    ))
                    // Visualise a valid destination while either kind of drag
                    // hovers it, so the drop target is obvious before release.
                    .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(rgb(drag_blue)))
                    .drag_over::<Vec<PathBuf>>(move |style, _, _, _| style.bg(rgb(drag_blue)))
                })
                // `.context_menu` wraps the element in `ContextMenu`, which no
                // longer exposes `tooltip`, so attach the tooltip first.
                .when_some(trash_parent, |row, parent| {
                    row.tooltip(move |window, cx| {
                        let text = format!("復元先: {}", parent.clone());
                        Tooltip::new(text).build(window, cx)
                    })
                })
                .context_menu(context_menu)
                .child(
                    div().w(px(18.0)).flex().justify_center().child(
                        match system_icon_path.as_deref() {
                            Some(path) => gpui_kit::img(path)
                                .size(px(18.0))
                                .with_fallback(move || {
                                    Icon::new(icon)
                                        .text_color(rgb(icon_color))
                                        .into_any_element()
                                })
                                .into_any_element(),
                            None => Icon::new(icon)
                                .text_color(rgb(icon_color))
                                .into_any_element(),
                        },
                    ),
                )
                .child(
                    // `tooltip` needs an interactive (Stateful) element, hence
                    // the explicit id on the truncated name cell.
                    div()
                        .id(("row-name", index))
                        .flex_1()
                        .truncate()
                        .tooltip(move |window, cx| {
                            Tooltip::new(name_tooltip_text.clone()).build(window, cx)
                        })
                        .child(display_name),
                )
                .child(
                    div()
                        .w(px(90.0))
                        .text_xs()
                        .truncate()
                        .text_color(rgb(p.muted))
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
                        .text_color(rgb(p.muted))
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
                        .text_color(rgb(p.muted))
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
        // F6-49: bookmark labels prefix the row name; the real name stays
        // visible so the underlying entry can still be identified.
        let bookmark_label = this.settings.bookmark_label(&entry.path).map(str::to_owned);
        let display_name = labeled_display_name(&display_name, bookmark_label.as_deref());
        let selected = this.selection.contains(&entry.path);
        let focused = this.selected == index;
        let is_dir = entry.is_dir;
        let p = this.palette;
        // `drag_over` closures borrow their color by reference; capture plain
        // `u32` copies so they do not outlive the palette local.
        let drag_blue = p.blue;
        let (icon, icon_color) = if this.view == ViewSource::Trash {
            (IconName::Trash, p.muted)
        } else {
            (
                icon_for_entry(&entry),
                if entry.is_symlink {
                    p.muted
                } else {
                    p.kind_color(kind_for_entry(&entry))
                },
            )
        };
        // Resolve the full-colour system icon up front, before any render
        // closure captures `this`. Trash rows keep the Lucide Trash icon.
        let system_icon_path = if this.view == ViewSource::Trash {
            None
        } else {
            this.system_icon_resolver.icon_path_for_entry(&entry, 48)
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
        let middle_click = cx.listener(move |this, _event: &MouseDownEvent, window, cx| {
            this.focus_handle.focus(window, cx);
            Self::open_row_in_new_tab(this, index, cx);
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
                    rgb(p.overlay)
                } else if focused {
                    rgb(p.focus)
                } else {
                    rgb(p.base)
                })
                // U-02/U-03: same focus indicator as the list row (independent
                // of `selected` so it is visible inside a multi-selection).
                .when(focused, |cell| cell.border_l_2().border_color(rgb(p.blue)))
                .cursor_pointer()
                .hover(|style| {
                    style.bg(if selected {
                        rgb(p.overlay)
                    } else {
                        rgb(p.focus)
                    })
                })
                .on_click(click)
                .on_mouse_down(MouseButton::Right, right_click)
                .on_mouse_down(MouseButton::Middle, middle_click)
                .when(drag_out, |cell| {
                    cell.on_drag(drag_paths, |_paths, _, _, cx| cx.new(|_| Empty))
                        .external_drag_payload(|paths: &Vec<PathBuf>, _window, _cx| {
                            Some(ExternalDragPayload::Files(FileDragPaths::new(
                                paths.iter().map(|path| (path.clone(), path.is_dir())),
                            )))
                        })
                })
                .when_some(drop_target, |cell, target| {
                    let external_target = target.clone();
                    cell.on_drop::<ExternalPaths>(cx.listener(
                        move |this, paths: &ExternalPaths, _window, cx| {
                            this.drop_external_into(
                                external_target.clone(),
                                paths.paths().to_vec(),
                                cx,
                            );
                        },
                    ))
                    .on_drop::<Vec<PathBuf>>(cx.listener(
                        move |this, paths: &Vec<PathBuf>, _window, cx| {
                            this.drop_internal_into(paths.clone(), target.clone(), cx);
                        },
                    ))
                    .drag_over::<ExternalPaths>(move |style, _, _, _| style.bg(rgb(drag_blue)))
                    .drag_over::<Vec<PathBuf>>(move |style, _, _, _| style.bg(rgb(drag_blue)))
                })
                .context_menu(context_menu)
                .child(match system_icon_path.as_deref() {
                    Some(path) => gpui_kit::img(path)
                        .size(px(48.0))
                        .with_fallback(move || {
                            div()
                                .text_2xl()
                                .child(Icon::new(icon).text_color(rgb(icon_color)))
                                .into_any_element()
                        })
                        .into_any_element(),
                    None => div()
                        .text_2xl()
                        .child(Icon::new(icon).text_color(rgb(icon_color)))
                        .into_any_element(),
                })
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
        let p = self.palette;
        let sidebar = div()
            .w(px(SIDEBAR_WIDTH_PX))
            .h_full()
            .flex()
            .flex_col()
            .bg(rgb(p.mantle))
            .border_r_1()
            .border_color(rgb(p.surface))
            .py_2()
            .child(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(rgb(p.muted))
                    .child("場所"),
            );
        // U-16: nine places plus bookmarks/recent/drives can exceed the window
        // height. Keep the header fixed and make only the list area scrollable;
        // `min_h_0` lets this flex child shrink so overflow can take effect.
        let mut scroll_area = div()
            .id("sidebar-scroll")
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .overflow_y_scroll();
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
                .text_color(if existing { rgb(p.subtext) } else { rgb(p.dim) })
                .when(active, |row| row.bg(rgb(p.surface)).text_color(rgb(p.text)))
                .when(!existing, |row| {
                    row.tooltip(move |window, cx| {
                        Tooltip::new(STATUS_PLACE_GONE_HINT).build(window, cx)
                    })
                })
                .child(Icon::new(place.icon))
                .child(place.label);
            if is_trash {
                row = row
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(p.surface)))
                    .on_click(cx.listener(|this, _event, window, cx| {
                        this.focus_handle.focus(window, cx);
                        this.enter_trash(cx);
                    }));
            } else if let Some(target) = place.path.clone().filter(|_| existing) {
                row = row
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(p.surface)))
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.focus_handle.focus(window, cx);
                        this.navigate_to(target.clone(), cx);
                    }));
            }
            scroll_area = scroll_area.child(row);
        }
        if !self.bookmarks.is_empty() {
            scroll_area = scroll_area.child(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(rgb(p.muted))
                    .child("ブックマーク"),
            );
            for (index, path) in self.bookmarks.iter().enumerate() {
                let active = self.view == ViewSource::Directory && path == &self.cwd;
                let label =
                    labeled_display_name(&tab_label(path), self.settings.bookmark_label(path));
                let target = path.clone();
                scroll_area = scroll_area.child(
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
                        .text_color(rgb(p.subtext))
                        .when(active, |row| row.bg(rgb(p.surface)).text_color(rgb(p.text)))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(p.surface)))
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
            scroll_area = scroll_area.child(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(rgb(p.muted))
                    .child("最近"),
            );
            for (index, path) in recent.iter().enumerate() {
                let active = self.view == ViewSource::Directory && path == &self.cwd;
                let label =
                    labeled_display_name(&tab_label(path), self.settings.bookmark_label(path));
                let target = path.clone();
                scroll_area = scroll_area.child(
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
                        .text_color(rgb(p.subtext))
                        .when(active, |row| row.bg(rgb(p.surface)).text_color(rgb(p.text)))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(p.surface)))
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
            scroll_area = scroll_area.child(
                div()
                    .px_3()
                    .py_1()
                    .text_xs()
                    .text_color(rgb(p.muted))
                    .child("ドライブ"),
            );
            for (index, mount) in self.mounts.iter().enumerate() {
                let active = self.view == ViewSource::Directory && mount.path == self.cwd;
                let label = if mount.path == std::path::Path::new("/") {
                    "ルート (/)".to_string()
                } else {
                    mount.path.display().to_string()
                };
                // Surface the previously unused device/fs fields on hover.
                let detail = format!(
                    "{} · {} · {}",
                    mount.path.display(),
                    mount.fs_type,
                    mount.source
                );
                let target = mount.path.clone();
                scroll_area = scroll_area.child(
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
                        .text_color(rgb(p.subtext))
                        .when(active, |row| row.bg(rgb(p.surface)).text_color(rgb(p.text)))
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(p.surface)))
                        .on_click(cx.listener(move |this, _event, window, cx| {
                            this.focus_handle.focus(window, cx);
                            this.navigate_to(target.clone(), cx);
                        }))
                        .tooltip(move |window, cx| Tooltip::new(detail.clone()).build(window, cx))
                        .child(Icon::new(IconName::HardDrive))
                        .child(div().min_w_0().truncate().child(label)),
                );
            }
        }
        sidebar.child(scroll_area)
    }

    fn toolbar(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
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
                breadcrumb = breadcrumb.child(
                    div()
                        .id(("breadcrumb-sep", index))
                        .aria_label("階層の区切り")
                        .text_xs()
                        .text_color(rgb(p.dim))
                        .child("›"),
                );
            }
            let is_last = index == last;
            breadcrumb = breadcrumb.child(
                div()
                    .id(10_000 + index as u64)
                    .aria_label(format!("{label} へ移動"))
                    .px_1()
                    .py_0p5()
                    .rounded_sm()
                    .text_sm()
                    .truncate()
                    .text_color(if is_last { rgb(p.text) } else { rgb(p.subtext) })
                    .when(is_last, |cell| cell.font_weight(FontWeight::SEMIBOLD))
                    .cursor_pointer()
                    .hover(|style| style.bg(rgb(p.surface)))
                    .on_click(cx.listener(move |this, _event, window, cx| {
                        this.focus_handle.focus(window, cx);
                        this.navigate_to(path.clone(), cx);
                    }))
                    .child(label),
            );
        }

        let back = icon_button(
            &p,
            "nav-back",
            "戻る",
            IconName::ArrowLeft,
            self.history.can_go_back() || self.view == ViewSource::Trash,
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.go_back(cx);
            }),
        );
        let forward = icon_button(
            &p,
            "nav-forward",
            "進む",
            IconName::ArrowRight,
            self.history.can_go_forward() || self.view == ViewSource::Trash,
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.go_forward(cx);
            }),
        );
        let up = icon_button(
            &p,
            "nav-up",
            PARENT_FOLDER_LABEL,
            IconName::ArrowUp,
            self.view == ViewSource::Trash || parent_dir(&self.cwd).is_some(),
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.go_parent(cx);
            }),
        );
        let refresh = icon_button(
            &p,
            "refresh",
            "再読込",
            IconName::RefreshCw,
            true,
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.reload(cx);
            }),
        );
        let undo_button = icon_button(
            &p,
            "undo",
            "元に戻す",
            IconName::Undo2,
            self.undo.can_undo(),
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.undo_last(cx);
            }),
        );
        let redo_button = icon_button(
            &p,
            "redo",
            "やり直す",
            IconName::Redo2,
            self.undo.can_redo(),
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.redo_last(cx);
            }),
        );
        let view_details = icon_button_active(
            &p,
            "view-details",
            "詳細リスト",
            IconName::List,
            true,
            self.settings.view == ViewLayout::Details,
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.set_view(ViewLayout::Details, cx);
            }),
        );
        let view_grid = icon_button_active(
            &p,
            "view-grid",
            "アイコングリッド",
            IconName::LayoutGrid,
            true,
            self.settings.view == ViewLayout::Icons,
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.set_view(ViewLayout::Icons, cx);
            }),
        );
        let new_folder = icon_button(
            &p,
            "new-folder",
            "新規フォルダ",
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
            &p,
            "toggle-hidden",
            "隠しファイル切替",
            hidden_icon,
            true,
            cx.listener(|this, _event, window, cx| {
                this.focus_handle.focus(window, cx);
                this.toggle_hidden(cx);
            }),
        );
        let help = icon_button(
            &p,
            "help",
            "ヘルプ (F1)",
            IconName::CircleQuestionMark,
            true,
            cx.listener(|this, _event, _window, cx| {
                this.help_open = true;
                cx.notify();
            }),
        );

        div()
            .flex()
            .items_center()
            .gap_1()
            .px_3()
            .py_1()
            .bg(rgb(p.mantle))
            .border_b_1()
            .border_color(rgb(p.surface))
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
            .child(help)
    }

    fn status_bar(&self, cx: &mut Context<Self>) -> Div {
        let p = self.palette;
        let hidden_count = self.all_entries.len().saturating_sub(self.entries.len());
        // U-10: a long primary status eats the bar; the pure helper drops the
        // lowest-priority secondary slots first (STATUS_PRIORITY tail: 監視状態).
        let visible_slots = visible_secondary_slots(&self.status);
        // U-11: `selected` is the keyboard cursor, not a selection. Build the
        // detail text only when an entry is actually selected.
        let selection_detail = if self.selection.len() == 1 {
            self.selected_entry().and_then(|entry| {
                if self.entries.is_empty() {
                    return None;
                }
                if self.view == ViewSource::Trash {
                    let parent =
                        trash_view::original_parent_for_path(&self.trash_items, &entry.path);
                    Some(match parent {
                        Some(parent) => format!(
                            "{}（削除: {} / 復元先: {}）",
                            entry.name_text,
                            human_datetime(entry.modified),
                            parent
                        ),
                        None => format!(
                            "{}（削除: {}）",
                            entry.name_text,
                            human_datetime(entry.modified)
                        ),
                    })
                } else {
                    Some(format_selected(entry))
                }
            })
        } else {
            None
        };
        let left = div()
            .flex()
            .items_center()
            .gap_2()
            .min_w_0()
            .child(
                div()
                    .max_w(px(520.0))
                    .truncate()
                    .text_color(rgb(p.subtext))
                    .child(if self.view == ViewSource::Trash {
                        "ゴミ箱".to_string()
                    } else {
                        display_path(&self.cwd)
                    }),
            )
            .child(div().text_color(rgb(p.dim)).child("|"))
            .child(format!("{} 件", self.entries.len()))
            .child(div().text_color(rgb(p.muted)).child(selection_status_text(
                self.selection.len(),
                selection_detail.as_deref(),
                self.implicit_selection,
            )));
        let right = div()
            .flex()
            .items_center()
            .gap_2()
            .min_w_0()
            .truncate()
            .child(if let Some(error) = self.load.error_message() {
                div()
                    .text_color(rgb(p.red))
                    .child(load_error_display_text(error))
                    .into_any_element()
            } else if self.load.is_loading() {
                div()
                    .text_color(rgb(p.yellow))
                    .child(LOADING)
                    .into_any_element()
            } else if !self.status.is_empty() {
                // U-05: GPUI 0.3.12 には live region / `aria_live` に相当する API が
                // 無い（`StatefulInteractiveElement` に無く、`AriaProperties` にも
                // live フィールドが無い。AccessKit の `Live` は公開されていない）。
                // そのためスクリーンリーダーへの自動読み上げは実装できず、ここでは
                // 色分け（U-01）と省略（U-10）のみ行う（既知の制限事項）。
                div()
                    .min_w_0()
                    .truncate()
                    .text_color(rgb(self.palette.status_color(&self.status)))
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
                            .bg(rgb(p.surface))
                            .flex()
                            .child(
                                div()
                                    .h_full()
                                    .rounded_full()
                                    .bg(rgb(p.blue))
                                    .w(px(120.0 * fraction as f32)),
                            ),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(p.muted))
                            .child(format!("{:.0}%", fraction * 100.0)),
                    )
                    .into_any_element()
            } else {
                div().into_any_element()
            })
            .child(
                if visible_slots.contains(&SecondarySlot::HiddenCount)
                    && self.view == ViewSource::Search
                    && hidden_count > 0
                {
                    // Search results were narrowed by the kind/extension filter.
                    div()
                        .text_color(rgb(p.subtext))
                        .child(format!("除外 {hidden_count} 件"))
                        .into_any_element()
                } else if visible_slots.contains(&SecondarySlot::HiddenCount)
                    && hidden_count > 0
                    && !self.settings.show_hidden
                {
                    div()
                        .text_color(rgb(p.subtext))
                        .child(format!("隠し {hidden_count} 件"))
                        .into_any_element()
                } else {
                    div().into_any_element()
                },
            )
            .child(
                if !visible_slots.contains(&SecondarySlot::UndoCount)
                    || self.undo.undo_len() + self.undo.redo_len() == 0
                {
                    div().into_any_element()
                } else {
                    div()
                        .id("undo-counter")
                        .text_color(rgb(p.muted))
                        .cursor_pointer()
                        .hover(|style| style.text_color(rgb(p.text)))
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
                },
            )
            .child(if !visible_slots.contains(&SecondarySlot::Watch) {
                div().into_any_element()
            } else if self.view == ViewSource::Trash {
                // AB08: route the Trash watch-slot caption through strings.rs.
                div()
                    .text_color(rgb(p.muted))
                    .child(STATUS_TRASH_NO_AUTO_REFRESH)
                    .into_any_element()
            } else if self.watcher.is_supported() {
                if let Some(reason) = self.watcher.last_error() {
                    div()
                        .text_color(rgb(p.yellow))
                        .child(format!(
                            "{STATUS_WATCH_PREFIX}{STATUS_WATCH_PARTIAL}{reason}"
                        ))
                        .into_any_element()
                } else {
                    div()
                        .text_color(rgb(p.subtext))
                        .child(format!(
                            "{STATUS_WATCH_PREFIX}{}{STATUS_WATCH_COUNT_SUFFIX}",
                            self.watcher.watched_paths().len()
                        ))
                        .into_any_element()
                }
            } else {
                let reason = self
                    .watcher
                    .last_error()
                    .unwrap_or(STATUS_WATCH_INIT_FAILED);
                div()
                    .text_color(rgb(p.yellow))
                    .child(format!(
                        "{STATUS_WATCH_PREFIX}{STATUS_WATCH_DISABLED}{reason}"
                    ))
                    .into_any_element()
            })
            // Right edge: build version (DESIGN.md status-bar requirement).
            // Hidden before the watch slot when the primary status is long.
            .child(if visible_slots.contains(&SecondarySlot::Version) {
                div()
                    .text_color(rgb(p.subtext))
                    .child(version_label())
                    .into_any_element()
            } else {
                div().into_any_element()
            });
        div()
            .flex()
            .items_center()
            .justify_between()
            .gap_3()
            .px_3()
            .h(px(26.0))
            .bg(rgb(p.mantle))
            .border_t_1()
            .border_color(rgb(p.surface))
            .text_xs()
            .text_color(rgb(p.muted))
            .child(left)
            .child(right)
    }
}

/// Icon tint for a toolbar button from the active palette.
///
/// The toolbar buttons set an explicit `text_color`, which overrides any
/// color inherited from the gpui-component theme. They must therefore be
/// tinted from [`AppPalette`] directly: the old hardcoded Mocha constants
/// left light-theme icons at ~1.2:1 against the light toolbar (C03).
/// Pure so the theme mapping can be unit-tested without a GPU window.
fn icon_button_text_color(palette: &AppPalette, enabled: bool) -> u32 {
    if enabled {
        palette.text
    } else {
        palette.dim
    }
}

/// Toolbar button that shows a persistent active state (e.g. the current
/// list/grid layout). Colors come from the active `palette`; the explicit
/// `text_color`/`hover` overrides win over any inherited component theme.
fn icon_button_active(
    palette: &AppPalette,
    id: &'static str,
    label: &'static str,
    icon: IconName,
    enabled: bool,
    active: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let text = icon_button_text_color(palette, enabled);
    let surface = palette.surface;
    div()
        .id(id)
        .aria_label(label)
        .flex()
        .items_center()
        .justify_center()
        .w(px(28.0))
        .h(px(28.0))
        .rounded_md()
        .when(active, |button| button.bg(rgb(surface)))
        .text_color(rgb(text))
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|style| style.bg(rgb(surface)))
                .on_click(on_click)
        })
        .tooltip(move |window, cx| Tooltip::new(label).build(window, cx))
        .child(Icon::new(icon))
}

/// Toolbar button: Lucide icon inside a hoverable square, tinted from the
/// active `palette` (see [`icon_button_active`] for the rationale).
fn icon_button(
    palette: &AppPalette,
    id: &'static str,
    label: &'static str,
    icon: IconName,
    enabled: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> Stateful<Div> {
    let text = icon_button_text_color(palette, enabled);
    let surface = palette.surface;
    div()
        .id(id)
        .aria_label(label)
        .flex()
        .items_center()
        .justify_center()
        .w(px(28.0))
        .h(px(28.0))
        .rounded_md()
        .text_color(rgb(text))
        .when(enabled, |button| {
            button
                .cursor_pointer()
                .hover(|style| style.bg(rgb(surface)))
                .on_click(on_click)
        })
        .tooltip(move |window, cx| Tooltip::new(label).build(window, cx))
        .child(Icon::new(icon))
}

impl Explorer {
    /// Start the inline rename for a freshly created folder once its row has
    /// appeared in the loaded listing. Runs in both debug and release builds:
    /// unlike the screenshot hooks below this is a real workflow feature.
    fn apply_pending_rename(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(path) = self.pending_rename.clone() else {
            return;
        };
        let index = pending_rename_index(
            self.load.is_loading(),
            self.renaming.is_some(),
            self.view == ViewSource::Directory,
            &self.entries,
            &path,
        );
        match index {
            Some(index) => {
                self.pending_rename = None;
                self.select_single_index(index, cx);
                // The rename input must keep focus even if an earlier dialog
                // scheduled a return of focus to the list.
                self.focus_list_on_next_render = false;
                self.start_rename(window, cx);
            }
            None => {
                // The folder disappeared (watcher/undo): never leave a stale
                // rename request queued forever. While another dialog owns the
                // keyboard, keep the request for the next render instead.
                if !self.load.is_loading()
                    && self.view == ViewSource::Directory
                    && self.renaming.is_none()
                {
                    self.pending_rename = None;
                }
            }
        }
    }

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
        if !self.load.is_loading() {
            if self.debug.properties_once && !self.load.is_loading() && !self.entries.is_empty() {
                self.debug.properties_once = false;
                let path = self.entries[0].path.clone();
                if self.debug.properties_size {
                    self.open_properties_with_size(path, cx);
                } else {
                    self.open_properties(path, cx);
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
        #[cfg(debug_assertions)]
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
        #[cfg(debug_assertions)]
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
    /// Restore list focus one frame after a dialog closes. Debug hooks live in
    /// `apply_startup_hooks` (disabled in release), but focus restoration must
    /// work in every build (finding 1 of the feature review).
    fn apply_focus_restore(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if !self.focus_list_on_next_render {
            return;
        }
        self.focus_list_on_next_render = false;
        let weak = cx.entity().downgrade();
        window.on_next_frame(move |window, cx| {
            // A dialog may have opened between scheduling and the next frame;
            // don't steal focus from its input in that case.
            let focus = weak
                .update(cx, |this, _cx| {
                    let busy = this.renaming.is_some()
                        || this.location.is_some()
                        || this.preview.is_some()
                        || this.undo_history_open
                        || this.fuzzy.is_some()
                        || this.terminal_command_editor.is_some()
                        || this.keymap_editor.is_some()
                        || this.label_editor.is_some()
                        || this.properties.is_some()
                        || this.help_open
                        // The settings card owns focus (its search box is
                        // focused on open); a queued restore triggered while
                        // another dialog closed must not steal it (F03).
                        || this.settings_ui.is_some()
                        // A Search view (including one just reopened in the
                        // same frame) owns focus for its query input; a queued
                        // restore from `close_search` must not steal it.
                        || this.view == ViewSource::Search
                        || this.search.input.is_some();
                    (!busy).then(|| this.focus_handle.clone())
                })
                .ok()
                .flatten();
            if let Some(focus) = focus {
                focus.focus(window, cx);
            }
        });
    }

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
        // Cache the current frame cheaply (no disk I/O in render). It is
        // written to settings by the normal save points and the close hook.
        if let Some(bounds) = WindowBoundsState::from_window(window) {
            self.window_bounds = Some(bounds);
        }
        self.apply_startup_hooks(window, cx);
        self.apply_pending_rename(window, cx);
        self.apply_focus_restore(window, cx);
        let tab_bar = self.tab_bar(cx);
        let search_bar = self.search_bar(cx);
        let search_history_bar = self.search_history_bar(cx);
        let saved_search_bar = self.saved_search_bar(cx);
        let location_bar = self.location_bar();
        self.apply_overlay_hooks(window, cx);
        let rename_dialog = self.rename_dialog(cx);
        let properties_dialog = self.properties_dialog(cx);
        let help_dialog = self.help_dialog(cx);
        let preview_dialog = self.preview_dialog(cx);
        let undo_history_dialog = self.undo_history_dialog(cx);
        let label_dialog = self.label_dialog(cx);
        let fuzzy_dialog = self.fuzzy_dialog(cx);
        let terminal_command_dialog = self.terminal_command_dialog(cx);
        let keymap_dialog = self.keymap_dialog(cx);
        let settings_dialog = self.settings_dialog(cx);

        let grid_mode = self.settings.view == ViewLayout::Icons;
        // Column headers only make sense in the details list.
        let header = if grid_mode {
            None
        } else {
            Some(self.sort_header(cx))
        };
        let columns =
            grid_columns_for_width(window.viewport_size().width.to_f64() as f32 - SIDEBAR_WIDTH_PX);
        self.grid_column_count = columns;
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

        let p = self.palette;
        let mut list_area = div()
            .relative()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .bg(rgb(p.base))
            .child(list)
            .on_drop::<ExternalPaths>(cx.listener(|this, paths: &ExternalPaths, _window, cx| {
                this.drop_external(paths.paths().to_vec(), cx);
            }))
            // A row drag released over empty list space (or a file row) falls
            // back to the current directory, matching the OS-drop behaviour.
            .on_drop::<Vec<PathBuf>>(cx.listener(|this, paths: &Vec<PathBuf>, _window, cx| {
                let cwd = this.cwd.clone();
                this.drop_internal_into(paths.clone(), cwd, cx);
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
            .bg(rgb(p.base))
            .text_color(rgb(p.text))
            .child(self.toolbar(cx))
            .children(tab_bar)
            .children(search_bar)
            .children(search_history_bar)
            .children(saved_search_bar)
            .children(location_bar)
            .child(body)
            .child(self.status_bar(cx))
            .children(rename_dialog)
            .children(properties_dialog)
            .children(help_dialog)
            .children(preview_dialog)
            .children(undo_history_dialog)
            .children(label_dialog)
            .children(fuzzy_dialog)
            .children(terminal_command_dialog)
            .children(keymap_dialog)
            .children(settings_dialog)
    }
}

// ---------------------------------------------------------- settings overlay

use crate::settings_ui::{
    render_settings_content, settings_row_is_visible, settings_row_shortcut, SettingsRowValues,
    SettingsTab, SettingsUiAction, SettingsUiHost, SettingsUiPalette, SettingsUiState,
};

impl Explorer {
    /// Step the settings overlay's active page by `delta`, wrapping around.
    ///
    /// Delegates to the same `SettingsUiAction::Tab` handler as the `TabBar`
    /// click, so `select_tab`, stale-error clearing and `cx.notify()` all stay
    /// in one place and the segmented control follows the rendered page (G08).
    fn cycle_settings_tab(&mut self, delta: isize, cx: &mut Context<Self>) {
        let Some(tab) = self.settings_ui.as_ref().map(|state| state.tab) else {
            return;
        };
        self.on_settings_action(SettingsUiAction::Tab(settings_tab_step(tab, delta)), cx);
    }

    /// Dismiss the preferences overlay with Esc or a backdrop click.
    ///
    /// Both routes bypass the footer's explicit `保存せず閉じる` label, so an
    /// edited form must say out loud that its changes were dropped (H03). The
    /// footer Cancel button keeps its own warning and still calls the plain
    /// `SettingsUiAction::Cancel` handler.
    fn dismiss_settings_ui(&mut self, cx: &mut Context<Self>) {
        if let Some(status) =
            settings_discard_status(self.settings_ui.as_ref().is_some_and(|state| state.dirty))
        {
            self.status = status.to_string();
        }
        self.on_settings_action(SettingsUiAction::Cancel, cx);
    }

    /// Ctrl+,: build the preferences overlay from the persisted settings and
    /// subscribe to its search box so typing re-renders the filtered rows.
    fn open_settings_ui(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.settings_ui.is_some() {
            return;
        }
        // A restore queued by a dialog that just closed must not fire later and
        // pull focus off the settings search box (F03). Closing this overlay
        // queues a fresh restore via `SettingsUiAction::Cancel`.
        self.focus_list_on_next_render = false;
        let keymap = self
            .settings
            .keymap
            .iter()
            .map(|(action, chord)| format!("{action}={chord};"))
            .collect::<String>();
        let mut state = SettingsUiState::with_initial_values(
            window,
            cx,
            &self.settings.terminal_command,
            &keymap,
        );
        let theme_choice = match self.settings.theme_choice {
            crate::settings::ThemeChoice::Dark => "dark",
            crate::settings::ThemeChoice::Light => "light",
            crate::settings::ThemeChoice::System => "system",
        };
        state.set_theme_choice(theme_choice);
        state.set_view_choice(if self.settings.view == ViewLayout::Icons {
            "icons"
        } else {
            "details"
        });
        let sort_choice = match self.settings.sort.key {
            SortKey::Name => "name",
            SortKey::Kind => "kind",
            SortKey::Size => "size",
            SortKey::Modified => "modified",
        };
        state.set_sort_choice(sort_choice);
        state.set_show_hidden(self.settings.show_hidden);
        state.observe_inputs(cx);
        // `observe_inputs` only watches the search box. The text editors never
        // emit `SetTerminal`/`SetKeymap`, so watch their `Change` events here to
        // keep the dirty flag (and the footer text) honest.
        for input in [state.terminal_input.clone(), state.keymap_input.clone()] {
            let subscription = cx.subscribe(&input, |this, _input, event: &InputEvent, cx| {
                if matches!(event, InputEvent::Change) {
                    let mut had_error = false;
                    if let Some(state) = this.settings_ui.as_mut() {
                        state.dirty = true;
                        had_error = clear_dialog_error(&mut state.error);
                    }
                    if had_error {
                        // Save mirrors the validation error into the status bar;
                        // drop that stale message once the input is corrected.
                        this.status.clear();
                    }
                    cx.notify();
                }
            });
            state.subscriptions.push(subscription);
        }
        // The global key guard swallows keys while the overlay is open, so the
        // card must own focus. The search box is present on every tab and has
        // no default value, making it the safest representative input.
        let focus = state.search_input.read(cx).focus_handle(cx).clone();
        focus.focus(window, cx);
        self.settings_ui = Some(state);
        cx.notify();
    }

    /// Full-screen dimming layer with the centered preferences card.
    fn settings_dialog(&self, cx: &mut Context<Self>) -> Option<Div> {
        let state = self.settings_ui.as_ref()?;
        // The effective (resolved) theme decides which palette the card uses.
        let palette = if self.settings.theme.is_dark() {
            SettingsUiPalette::dark()
        } else {
            SettingsUiPalette::light()
        };
        let theme_options = [
            ("dark", THEME_LABEL_DARK),
            ("light", THEME_LABEL_LIGHT),
            ("system", THEME_LABEL_SYSTEM),
        ];
        let view_options = [("details", "一覧"), ("icons", "アイコン")];
        let sort_options = [
            ("name", SortKey::Name.label()),
            ("kind", SortKey::Kind.label()),
            ("size", SortKey::Size.label()),
            ("modified", SortKey::Modified.label()),
        ];
        let content = render_settings_content::<Explorer>(
            &palette,
            state,
            &theme_options,
            &view_options,
            &sort_options,
            cx,
        );
        let backdrop = div()
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .bg(gpui::black().opacity(0.45))
            .on_mouse_down(
                MouseButton::Left,
                cx.listener(|this, _: &MouseDownEvent, _window, cx| {
                    this.dismiss_settings_ui(cx);
                }),
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
                .child(content),
        )
    }
}

impl SettingsUiHost for Explorer {
    fn on_settings_action(&mut self, action: SettingsUiAction, cx: &mut Context<Self>) {
        match action {
            SettingsUiAction::None => {}
            SettingsUiAction::Tab(tab) => {
                if let Some(state) = self.settings_ui.as_mut() {
                    state.select_tab(tab);
                    let had_error = clear_dialog_error(&mut state.error);
                    if had_error {
                        // An error raised on another tab no longer applies to
                        // the newly selected page (or its status-bar copy).
                        self.status.clear();
                    }
                    cx.notify();
                }
            }
            SettingsUiAction::SetTheme(value) => {
                if let Some(state) = self.settings_ui.as_mut() {
                    state.set_theme_choice(value);
                    state.dirty = true;
                    cx.notify();
                }
            }
            SettingsUiAction::SetView(value) => {
                if let Some(state) = self.settings_ui.as_mut() {
                    state.set_view_choice(value);
                    state.dirty = true;
                    cx.notify();
                }
            }
            SettingsUiAction::SetSort(value) => {
                if let Some(state) = self.settings_ui.as_mut() {
                    state.set_sort_choice(value);
                    state.dirty = true;
                    cx.notify();
                }
            }
            SettingsUiAction::ToggleHidden(value) => {
                if let Some(state) = self.settings_ui.as_mut() {
                    state.set_show_hidden(value);
                    state.dirty = true;
                    cx.notify();
                }
            }
            // The GPUI text inputs own their value; only the dirty flag is ours.
            SettingsUiAction::SetTerminal(_) | SettingsUiAction::SetKeymap(_) => {
                if let Some(state) = self.settings_ui.as_mut() {
                    state.dirty = true;
                    cx.notify();
                }
            }
            SettingsUiAction::Cancel => {
                self.settings_ui = None;
                self.focus_list_on_next_render = true;
                cx.notify();
            }
            SettingsUiAction::Save => {
                // Snapshot before any preview mutation. A failed write leaves
                // disk at the old values, so the UI must be put back to match
                // what the next launch will load (D13).
                let snapshot = self.settings.clone();
                // Copy the form values out of the card before mutating settings.
                let (terminal_raw, keymap_raw, theme_choice, view_choice, sort_choice, show_hidden) = {
                    let Some(state) = self.settings_ui.as_ref() else {
                        return;
                    };
                    (
                        state.terminal_value(cx),
                        state.keymap_value(cx),
                        state.theme_choice.clone(),
                        state.view_choice.clone(),
                        state.sort_choice.clone(),
                        state.show_hidden,
                    )
                };

                // `action=chord;` entries go through the same validator as the
                // Alt+K dialog, then through the same upsert with eviction.
                let mut overrides: Vec<(String, String)> = Vec::new();
                let mut error: Option<String> = None;
                for entry in keymap_raw
                    .split(';')
                    .map(str::trim)
                    .filter(|entry| !entry.is_empty())
                {
                    let normalized = entry.replacen('=', " ", 1);
                    match parse_keymap_input(&normalized) {
                        Some(KeymapInput::Bind { action, chord }) => {
                            if !upsert_keymap_override(&mut overrides, &action, &chord) {
                                error = Some(keymap_limit_status());
                                break;
                            }
                        }
                        Some(KeymapInput::Unbind { action }) => {
                            // Same semantics as the Alt+K dialog: removing an
                            // override that does not exist is a harmless no-op.
                            remove_keymap_override(&mut overrides, &action);
                        }
                        None => {
                            // AC08: the card's footer mirrors the Alt+K dialog
                            // error, so append the same canonical format hint.
                            error = Some(format!(
                                "{STATUS_KEYMAP_INVALID_PREFIX}{entry}（{KEYMAP_FORMAT_HINT}）"
                            ));
                            break;
                        }
                    }
                }
                if let Some(message) = error {
                    if let Some(state) = self.settings_ui.as_mut() {
                        state.error = Some(message.clone());
                    }
                    // The card now renders `state.error` in a shared footer
                    // visible on every tab, so the failure is no longer hidden
                    // on Shortcuts. Mirror it into the status bar too (outside
                    // the scrollable card), and prefix it so `status_color`
                    // paints it red instead of default success green. Keep the
                    // dialog open for editing.
                    self.status = settings_validation_status(&message);
                    cx.notify();
                    return;
                }

                let command = normalize_terminal_command(&terminal_raw).unwrap_or_default();
                if !self.settings.set_terminal_command(&command) {
                    let message = STATUS_TERMINAL_COMMAND_INVALID.to_string();
                    if let Some(state) = self.settings_ui.as_mut() {
                        state.error = Some(message.clone());
                    }
                    self.status = settings_validation_status(&message);
                    cx.notify();
                    return;
                }

                let theme = match theme_choice.as_str() {
                    "light" => crate::settings::ThemeChoice::Light,
                    "system" => crate::settings::ThemeChoice::System,
                    _ => crate::settings::ThemeChoice::Dark,
                };
                // Compare against the pre-edit snapshot, not the resolved mode:
                // changing the persisted preference is worth reporting even
                // when the effective theme stays the same (e.g. System→Dark on
                // a dark desktop). Mirrors the Alt+Shift+T notice (G07).
                let theme_changed = snapshot.theme_choice != theme;
                // Resolve against the appearance GPUI observes for this window,
                // not the previously persisted mode, then push the result through
                // the single entry point that keeps `settings.theme`, the palette
                // and GPUI Component's Theme in sync.
                let effective_theme = resolve_theme(theme, cx.window_appearance().into());
                self.apply_theme_mode(effective_theme, cx);
                if theme == ThemeChoice::System {
                    // Match the Alt+Shift+T System path: let gpui-component read
                    // the window appearance instead of pinning `Theme::change`.
                    Theme::sync_system_appearance(None, cx);
                }
                self.settings.theme_choice = theme;
                self.settings.view = if view_choice == "icons" {
                    ViewLayout::Icons
                } else {
                    ViewLayout::Details
                };
                self.settings.show_hidden = show_hidden;
                let sort_key = match sort_choice.as_str() {
                    "kind" => SortKey::Kind,
                    "size" => SortKey::Size,
                    "modified" => SortKey::Modified,
                    _ => SortKey::Name,
                };
                // The form only edits the key. Keep the persisted direction
                // (and `dirs_first`) untouched instead of deriving a default
                // from the chosen key.
                self.settings.sort.key = sort_key;
                self.settings.keymap = overrides;
                self.keymap = KeyMap::from_settings(&self.settings);

                // Persist before refreshing the list: on failure the rollback
                // must also leave the visible entries untouched, otherwise the
                // list would keep the previewed show_hidden/sort values (D13).
                let saved = self.save_settings();
                if settings_write_needs_rollback(saved) {
                    self.rollback_settings(snapshot, cx);
                } else {
                    let keep = self.selected_entry().map(|entry| entry.path.clone());
                    if self.view == ViewSource::Search {
                        self.run_search(cx);
                    } else {
                        self.rebuild_view(keep);
                    }
                }
                self.settings_ui = None;
                self.focus_list_on_next_render = true;
                let success = if theme_changed {
                    format!("{STATUS_THEME_CHANGED_PREFIX}{}", theme_choice_label(theme))
                } else {
                    "設定を保存しました".to_string()
                };
                // One write-status path for every settings write: skipped
                // (read-only / ephemeral) and failed writes must not be masked
                // by the success caption (F04/J01).
                self.status = settings_write_status(&self.settings, saved, true, success);
                cx.notify();
            }
        }
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
///
/// `--` terminates option parsing: only arguments before it can be flags, so a
/// file named `--help` passed after `--` is treated as a path, not a request to
/// print help.
fn classify_cli(args: &[String]) -> CliAction {
    let scan_end = args
        .iter()
        .position(|arg| arg == "--")
        .unwrap_or(args.len());
    let scan = &args[..scan_end];
    if scan.iter().any(|arg| arg == "--version" || arg == "-V") {
        CliAction::Version
    } else if scan.iter().any(|arg| arg == "--help" || arg == "-h") {
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
                 使い方: gpui-file-explorer [オプション] [ディレクトリ]\n\n\
                 オプション:\n\
                   -V, --version   バージョンを表示して終了（ウィンドウは開きません）\n\
                   -h, --help      この使い方を表示して終了（ウィンドウは開きません）\n\
                   --              以降の引数をすべてパスとして扱う（`-` で始まる名前も可）\n\n\
                 引数なしの場合は前回のディレクトリ（無ければ $HOME）を開きます。\n\
                 主なキー: F2 名前変更 / Ctrl+C・Ctrl+X・Ctrl+V コピー・切り取り・貼り付け /\n\
                 Ctrl+P {} / F4 ターミナル / Alt+K キー設定 /\n\
                 Ctrl+H 隠しファイル / Ctrl+Shift+N 新規フォルダ / Alt+←→ 戻る・進む / Esc キャンセル\n\n\
                 ライセンス: AGPL-3.0-or-later\n\
                 ソース: https://github.com/Seduce0068/gpui-file-explorer",
                env!("CARGO_PKG_VERSION"),
                FUZZY_JUMP_LABEL
            );
            true
        }
        CliAction::Run => false,
    }
}

/// Resolve the initial directory from raw CLI args (pure, so `--` handling is
/// testable without touching `std::env`).
///
/// `--` is an end-of-options separator: the token itself is skipped and every
/// argument after it is a path, even one starting with `-` or equal to a flag.
/// Before the separator, leading-`-` arguments are ignored as flags.
fn startup_dir_from(args: impl IntoIterator<Item = OsString>) -> Option<PathBuf> {
    let mut after_separator = false;
    args.into_iter().find_map(|arg| {
        if !after_separator {
            if arg.to_string_lossy() == "--" {
                after_separator = true;
                return None;
            }
            let path = PathBuf::from(&arg);
            if path.as_os_str().to_string_lossy().starts_with('-') {
                return None;
            }
            Some(path)
        } else {
            Some(PathBuf::from(arg))
        }
    })
}

/// First path argument, treated as the initial directory.
fn startup_dir() -> Option<PathBuf> {
    // Keep raw bytes: a path may contain non-UTF-8 bytes on Unix (F6-06).
    // Flag matching is ASCII only, so lossy conversion is safe here.
    startup_dir_from(std::env::args_os().skip(1))
}

/// Canonicalize a CLI-supplied directory and detect paths that cannot be used.
///
/// Returns the canonical path on success, or the original argument on failure
/// (missing path, regular file, unreadable path). Callers use the error variant
/// to emit a diagnostic instead of silently falling back to saved tabs/home.
fn validate_cli_dir(path: PathBuf) -> Result<PathBuf, PathBuf> {
    let canonical = std::fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
    if canonical.is_dir() {
        Ok(canonical)
    } else {
        Err(path)
    }
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

    // Capture the saved active tab's *path* as a fallback for an out-of-range
    // index. Missing-directory tabs are kept in `tabs`, so the saved numeric
    // index stays stable and is preferred when it still points into the list
    // (finding 1); the path only helps when the index genuinely went stale.
    let active_path = saved.get(active).cloned();

    // Keep every saved tab, including directories that are temporarily
    // unmounted (finding 4). Dropping them here lets `persist_tabs` write the
    // shrunken list and lose them permanently. Activating such a tab goes
    // through the existing `reload()` path, which surfaces the read error, and
    // the tab becomes usable again once its directory is mounted back. This is
    // the smaller of the two safe options: retaining the tabs only in settings
    // while hiding them from the tab bar would need extra merge logic in
    // `persist_tabs` to survive later tab switches.
    let mut tabs = saved.to_vec();

    // Prefer the saved numeric index: tabs are no longer dropped when their
    // directory is temporarily missing, so `active` remains stable even with
    // duplicate cwds (`position()` would always pick the first tab). Fall back
    // to the path only when the index is out of range (finding 1).
    let resolve_active = |list: &[PathBuf]| -> usize {
        if active < list.len() {
            return active;
        }
        active_path
            .as_ref()
            .and_then(|path| list.iter().position(|candidate| candidate == path))
            .unwrap_or_else(|| active.min(list.len().saturating_sub(1)))
    };

    // An explicit CLI argument (`.desktop %f`, shell invocation) must win over
    // persisted tabs; otherwise passing a folder to the app silently opens a
    // different location (third cross-review F6-12).
    if let Some(cli) = cli_dir {
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
        let index = resolve_active(&tabs);
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

/// Restore tabs **with their back/forward history** from the primary window's
/// session. `restore_session` still decides which directories open (CLI > saved
/// tabs > last dir > home); this only reattaches the per-tab history that used
/// to be lost on restart. Tabs with no matching session get a fresh history.
fn restore_tab_sessions(
    settings: &Settings,
    saved_tabs: &[PathBuf],
    active: usize,
    fallback: &Path,
) -> Tabs {
    let sessions: &[TabSession] = settings
        .primary_window()
        .map(|window| window.tabs.as_slice())
        .unwrap_or(&[]);

    // Match on the persisted cwd string, not a canonicalized path: distinct
    // symlink tabs legitimately normalize to the same target but own separate
    // histories (finding 2). A per-path queue lets duplicate-cwd tabs consume
    // their sessions in order instead of every tab receiving the first match.
    let mut by_path: std::collections::HashMap<
        &std::path::Path,
        std::collections::VecDeque<&TabSession>,
    > = std::collections::HashMap::new();
    for session in sessions {
        by_path
            .entry(session.cwd.as_path())
            .or_default()
            .push_back(session);
    }

    let mut tabs = Tabs::from_paths(saved_tabs.to_vec(), active)
        .unwrap_or_else(|| Tabs::new(fallback.to_path_buf()));
    for index in 0..tabs.len() {
        let cwd = tabs.tabs()[index].cwd.clone();
        let history = by_path
            .get_mut(cwd.as_path())
            .and_then(|queue| queue.pop_front())
            .map(TabSession::to_history)
            .unwrap_or_default();
        // `Tabs` can set a history only on the active tab: visit each index
        // in turn, then restore the index chosen by `restore_session`.
        tabs.activate(index);
        tabs.sync(cwd, history);
    }
    tabs.activate(active.min(tabs.len() - 1));
    tabs
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

/// A saved frame is reused only when its origin still lands on a display.
/// This preserves the previous Linux/Wayland behaviour (a visible window)
/// after a monitor was unplugged or settings came from another machine.
fn bounds_visible(bounds: &Bounds<Pixels>, cx: &App) -> bool {
    let origin = point(bounds.origin.x, bounds.origin.y);
    cx.displays()
        .iter()
        .any(|display| display.visible_bounds().contains(&origin))
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
        // Load once at startup: the persisted *choice* is resolved against the
        // appearance GPUI observes right now, and the resolved mode styles
        // context menus / prompts. The same snapshot seeds the window frame.
        let mut settings = Settings::load();
        let system_appearance: ThemeMode = cx.window_appearance().into();
        let effective_theme = resolve_theme(settings.theme_choice, system_appearance);
        settings.theme = effective_theme;
        Theme::change(effective_theme, None, cx);

        cx.on_window_closed(|cx, _window_id| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();

        // Restore the saved frame of the primary window. The centered fallback
        // remains for a fresh install, unusable saved values and off-screen
        // origins (e.g. after a monitor change).
        let window_bounds = settings
            .primary_window()
            .and_then(|session| session.bounds)
            .and_then(|saved| saved.as_window_bounds())
            .filter(|candidate| bounds_visible(&candidate.get_bounds(), cx));
        let bounds = window_bounds
            .unwrap_or_else(|| WindowBounds::centered(size(px(1100.0), px(700.0)), cx));
        let options = WindowOptions {
            window_bounds: Some(bounds),
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
            // `Explorer::new` only receives a `Context`, which has no window,
            // so the OS appearance observer is installed here (window level).
            // It is inert unless the persisted choice is `System`; keeping the
            // Subscription on the Explorer ties its lifetime to the view.
            let appearance_view = view.downgrade();
            let subscription = window.observe_window_appearance(move |_window, cx| {
                let _ = appearance_view.update(cx, |this, cx| {
                    this.on_system_appearance_changed(cx);
                });
            });
            view.update(cx, |this, _cx| {
                this.appearance_subscription = Some(subscription);
            });
            let focus_handle = view.read(cx).focus_handle.clone();
            let explorer = view.downgrade();
            // The final frame must be captured here: `on_window_closed` runs
            // after the window is gone and can no longer read its bounds.
            window.on_window_should_close(cx, move |window, cx| {
                // Frame capture is optional (finding 3): a headless or already
                // hidden window can report no bounds, but its tabs/history must
                // still be saved. Only the bounds update is conditional.
                if let Some(bounds) = WindowBoundsState::from_window(window) {
                    let _ = explorer.update(cx, |this, cx| {
                        this.window_bounds = Some(bounds);
                        cx.notify();
                    });
                }
                let _ = explorer.update(cx, |this, _cx| {
                    this.persist_tabs();
                    if !this.save_settings() {
                        // No status bar survives the close; mirror the Ctrl+Q
                        // path and surface the failure in the log (Bug L01).
                        // V10: include why the write failed.
                        tracing::error!(
                            reason = ?this.settings.last_save_error.get(),
                            "failed to save settings before closing window"
                        );
                    }
                });
                true
            });
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
        active_keymap_overrides, apply_debug_settings_overrides, bookmark_label_is_settable,
        bookmark_label_not_set_status, bookmark_label_rejection_reason,
        bookmark_label_set_rejection, bookmark_toggle_rejection, build_places, canonical_chord,
        chord_for_action, chords_collide, classify_cli, clear_dialog_error, clear_op_token,
        clear_rescan_failure_after_successful_read, clear_rescan_failure_status, format_chord,
        format_selected, fuzzy_candidates, fuzzy_score, grid_columns_for_width,
        help_overrides_line, icon_button_text_color, install_op_token, internal_drop_paths,
        join_status_notices, keymap_action_name, keymap_bind_caption, keymap_chord_reserved,
        keymap_limit_status, keymap_unbind_caption, kind_color, labeled_display_name,
        load_error_display_text, middle_click_tab_dir, normalize_terminal_command, op_is_active,
        parse_chord, parse_keymap_input, pending_rename_index, rank_fuzzy,
        read_properties_snapshot, reconcile_aware_status, remove_keymap_override,
        saved_search_caption, saved_search_save_status, search_filter_status, set_view_status,
        settings_discard_status, settings_read_only_status, settings_row_key_is_bare,
        settings_save_error_status, settings_save_status, settings_sort_next,
        settings_tab_from_digit_key, settings_tab_step, settings_validation_status,
        settings_write_needs_rollback, settings_write_status, startup_dir_from,
        startup_save_failure_status, status_color, summarize_op, tab_label, terminal_dir_for,
        terminal_disabled_status, terminal_failed_status, terminal_process,
        terminal_started_status, terminal_target_dir, theme_choice_label, transfer_log_level,
        transfer_progress_status, transfer_skip_notice, trash_batch, trash_delete_status,
        trash_key_action, trash_load_error_status, trash_redo_cancelled_status, trash_undo_status,
        trashed_ids_from_snapshot, undo_missing_status, upsert_keymap_override, validate_cli_dir,
        version_label, visible_secondary_slots, AppPalette, CliAction, DebugSettingsOverrides,
        Explorer, FuzzyCandidate, FuzzySource, KeyAction, KeyBinding, KeyMap, KeymapInput,
        Modifiers, PropertiesErrorReaction, SecondarySlot, SettingsTab, SortKey, TrashKeyAction,
        UndoAction, ViewLayout, C_BLUE, C_DIM, C_GREEN, C_MUTED, C_RED, C_TEXT, C_YELLOW,
        GRID_CELL_WIDTH_PX, GRID_GAP_PX, GRID_ROW_PADDING_PX, KEYMAP_FORMAT_HINT,
        SECONDARY_SLOT_PRIORITY, SIDEBAR_WIDTH_PX, STATUS_ACCESS_DENIED,
        STATUS_BOOKMARKS_ONLY_DIRECTORY, STATUS_BOOKMARK_LABEL_REASON_INVALID_LABEL,
        STATUS_BOOKMARK_LABEL_REASON_INVALID_PATH, STATUS_BOOKMARK_LABEL_REASON_LIMIT,
        STATUS_BOOKMARK_NOT_PERSISTABLE, STATUS_CANCELLED, STATUS_CANCELLING, STATUS_CANCEL_HINT,
        STATUS_CANNOT_OPEN_SUFFIX, STATUS_CHANGED, STATUS_CHAR_BUDGET_PER_TIER, STATUS_DROP_EMPTY,
        STATUS_DROP_NOT_FOUND, STATUS_DROP_ONLY_DIRECTORY, STATUS_DROP_SAME_DIRECTORY,
        STATUS_ERROR_MARKERS, STATUS_ERROR_PREFIX, STATUS_FILE_NO_NEW_TAB_PREFIX,
        STATUS_FOLDER_NO_PREVIEW, STATUS_FUZZY_JUMP_ONLY_DIRECTORY, STATUS_KEYMAP_INVALID_PREFIX,
        STATUS_KEYMAP_NO_UNBIND_PREFIX, STATUS_LAST_TAB, STATUS_LOCATION_ONLY_DIRECTORY,
        STATUS_MAX_TABS_PREFIX, STATUS_MISSING_COUNT_SUFFIX, STATUS_NOT_DIRECTORY,
        STATUS_NOT_FOUND_SUFFIX, STATUS_NO_SELECTION, STATUS_OP_FAILED_SUFFIX,
        STATUS_PARTIAL_FAILURE_SUFFIX, STATUS_PATH_GONE, STATUS_PREVIEW_ONLY_DIRECTORY,
        STATUS_PRIORITY, STATUS_PROPERTIES_ONLY_DIRECTORY, STATUS_REDO_EMPTY, STATUS_REDO_FAILED,
        STATUS_REDO_NO_TARGET, STATUS_RESCAN_FAILED, STATUS_RESTORE_EMPTY, STATUS_RESTORE_FAILED,
        STATUS_SAME_DIRECTORY_NO_MOVE, STATUS_SEARCH_EMPTY_QUERY, STATUS_SEARCH_NOT_OPEN,
        STATUS_SEARCH_ONLY_DIRECTORY, STATUS_SEARCH_STALE, STATUS_SETTINGS_DISCARDED,
        STATUS_SETTINGS_READ_ONLY, STATUS_SETTINGS_READ_ONLY_HINT, STATUS_SETTINGS_SAVE_FAILED,
        STATUS_SETTINGS_SAVE_LOCKED, STATUS_SETTINGS_SAVE_TOO_LARGE, STATUS_SPECIAL_FILE_NO_COPY,
        STATUS_SYMLINK_NO_PREVIEW, STATUS_TABS_ONLY_DIRECTORY, STATUS_TAB_RESTORE_EMPTY,
        STATUS_TARGET_GONE, STATUS_TARGET_MISSING, STATUS_TERMINAL_COMMAND_INVALID,
        STATUS_TERMINAL_DISABLED_HINT, STATUS_TERMINAL_START_PREFIX, STATUS_THEME_CHANGED_PREFIX,
        STATUS_TRASH_MISSING_REASON, STATUS_TRASH_NO_COPY, STATUS_TRASH_NO_TERMINAL_PREFIX,
        STATUS_UNDO_EMPTY, STATUS_UNDO_FAILED, STATUS_WARNING_MARKERS, THEME_LABEL_DARK,
        THEME_LABEL_LIGHT, THEME_LABEL_SYSTEM,
    };
    use crate::icons::FileKind;
    use crate::model::{Entry, FsError};
    use crate::ops::{OpError, OpStats};
    use crate::settings::{Settings, SettingsSaveError, ThemeChoice, MAX_KEYMAP_OVERRIDES};
    use crate::strings::{
        STATUS_BOOKMARK_LABEL_NOT_SET_PREFIX, STATUS_BOOKMARK_LABEL_SET_FAILED_PREFIX,
        STATUS_DROP_NO_DESTINATION, STATUS_INVALID_CLI_DIR, STATUS_MAX_TABS_SUFFIX,
        STATUS_NO_CLIPBOARD, STATUS_RENAME_FAILED, STATUS_SAVED_SEARCH_SAVE_FAILED_PREFIX,
        STATUS_SKIP_REASON_MISSING, STATUS_SKIP_REASON_SAME_DIRECTORY, STATUS_SKIP_REASON_SPECIAL,
        STATUS_TRASH_ALL_FAILED, STATUS_TRASH_LOAD_FAILED_PREFIX, STATUS_TRASH_NO_CUT,
        STATUS_TRASH_NO_HIDDEN, STATUS_TRASH_NO_PASTE, STATUS_TRASH_NO_PURGE,
        STATUS_TRASH_NO_RENAME, STATUS_TRASH_NO_SORT, STATUS_TRASH_PARTIAL_FAILED,
        STATUS_UNDO_MISSING_SUFFIX, STATUS_WATCH_DISABLED, STATUS_WATCH_INIT_FAILED,
        STATUS_WATCH_PARTIAL,
    };
    use crate::trash_view;
    use std::ffi::OsString;
    use std::path::PathBuf;
    use std::time::SystemTime;

    #[test]
    fn status_color_classifies_errors_warnings_and_success() {
        let wrapped_error = format!("{STATUS_ERROR_PREFIX}ファイルを読み取れません");
        // (status, expected colour): a table so every branch of the helper has
        // a concrete regression case, including composed messages.
        let cases = [
            (wrapped_error.as_str(), C_RED),
            (STATUS_SETTINGS_SAVE_FAILED, C_RED),
            // X01: 理由別の保存失敗も明示的な error マーカーとして赤を固定する。
            (STATUS_SETTINGS_SAVE_TOO_LARGE, C_RED),
            (STATUS_SETTINGS_SAVE_LOCKED, C_RED),
            // W06: 再スキャン失敗は ERROR_MARKERS 登録済みで red（表示は正しいが
            // 回帰表に欠けていたため、ここで明示的に固定する）。
            (STATUS_RESCAN_FAILED, C_RED),
            // Y01: 回帰表に欠けていた concrete error マーカーを全て固定する。
            (STATUS_TRASH_ALL_FAILED, C_RED),
            (STATUS_RENAME_FAILED, C_RED),
            (STATUS_TRASH_PARTIAL_FAILED, C_RED),
            (STATUS_OP_FAILED_SUFFIX, C_RED),
            (STATUS_PARTIAL_FAILURE_SUFFIX, C_RED),
            (STATUS_NOT_FOUND_SUFFIX, C_RED),
            (STATUS_MISSING_COUNT_SUFFIX, C_RED),
            (STATUS_SAVED_SEARCH_SAVE_FAILED_PREFIX, C_RED),
            (STATUS_BOOKMARK_LABEL_SET_FAILED_PREFIX, C_RED),
            (STATUS_BOOKMARK_LABEL_NOT_SET_PREFIX, C_RED),
            (STATUS_TRASH_LOAD_FAILED_PREFIX, C_RED),
            (STATUS_UNDO_MISSING_SUFFIX, C_RED),
            // X09 / Y01: 「開けません」を含むが concrete error なので赤。
            (STATUS_INVALID_CLI_DIR, C_RED),
            (STATUS_RESTORE_FAILED, C_RED),
            (STATUS_REDO_FAILED, C_RED),
            (STATUS_ACCESS_DENIED, C_RED),
            (STATUS_PATH_GONE, C_RED),
            // I01: a vanished drop/move target and a non-directory path are
            // failures, so they must not fall through to success green.
            (STATUS_TARGET_GONE, C_RED),
            (STATUS_TARGET_MISSING, C_RED),
            (STATUS_DROP_NOT_FOUND, C_RED),
            (STATUS_NOT_DIRECTORY, C_RED),
            // Q02: P08 の「できません」拒否は concrete failure として error(red)。
            (STATUS_SPECIAL_FILE_NO_COPY, C_RED),
            (STATUS_FOLDER_NO_PREVIEW, C_RED),
            (STATUS_SYMLINK_NO_PREVIEW, C_RED),
            ("コピーできません: 権限がありません", C_RED),
            ("3 件コピーしました（一部失敗: 権限がありません）", C_RED),
            (STATUS_SEARCH_STALE, C_YELLOW),
            (STATUS_CANCELLED, C_YELLOW),
            (STATUS_SETTINGS_READ_ONLY, C_YELLOW),
            // X02: 未保存設定の破棄通知は成功ではなく warning(amber)。
            (STATUS_SETTINGS_DISCARDED, C_YELLOW),
            // K03: "選択項目がありません" refuses an operation, so it is amber
            // rather than success green even without a *_FAILED phrase.
            (STATUS_NO_SELECTION, C_YELLOW),
            (STATUS_TRASH_NO_COPY, C_YELLOW),
            // Y01: 従来の回帰表になかった NO_* / TRASH_NO_* マーカーを固定する。
            (STATUS_NO_CLIPBOARD, C_YELLOW),
            (STATUS_DROP_NO_DESTINATION, C_YELLOW),
            (STATUS_TRASH_NO_CUT, C_YELLOW),
            (STATUS_TRASH_NO_PASTE, C_YELLOW),
            (STATUS_TRASH_NO_RENAME, C_YELLOW),
            (STATUS_TRASH_NO_PURGE, C_YELLOW),
            (STATUS_TRASH_NO_HIDDEN, C_YELLOW),
            (STATUS_TRASH_NO_SORT, C_YELLOW),
            (STATUS_KEYMAP_NO_UNBIND_PREFIX, C_YELLOW),
            // NO_* notices must win over the *_FAILED phrase they contain.
            (STATUS_REDO_NO_TARGET, C_YELLOW),
            // L06: refusal / empty-target notices are warnings, not success.
            (STATUS_TABS_ONLY_DIRECTORY, C_YELLOW),
            (STATUS_BOOKMARKS_ONLY_DIRECTORY, C_YELLOW),
            (STATUS_LAST_TAB, C_YELLOW),
            (STATUS_DROP_ONLY_DIRECTORY, C_YELLOW),
            (STATUS_DROP_EMPTY, C_YELLOW),
            (STATUS_TAB_RESTORE_EMPTY, C_YELLOW),
            (STATUS_UNDO_EMPTY, C_YELLOW),
            (STATUS_REDO_EMPTY, C_YELLOW),
            // N02: 「開けません」は「できません」を含まない拒否接尾。
            (STATUS_CANNOT_OPEN_SUFFIX, C_YELLOW),
            // N03: ディレクトリ表示でのみ使える機能の拒否（肯定形だが amber）。
            (STATUS_PREVIEW_ONLY_DIRECTORY, C_YELLOW),
            (STATUS_LOCATION_ONLY_DIRECTORY, C_YELLOW),
            (STATUS_FUZZY_JUMP_ONLY_DIRECTORY, C_YELLOW),
            (STATUS_PROPERTIES_ONLY_DIRECTORY, C_YELLOW),
            (STATUS_SEARCH_ONLY_DIRECTORY, C_YELLOW),
            // N08 / N09: 上限・空対象・空検索語の拒否。
            (STATUS_MAX_TABS_PREFIX, C_YELLOW),
            (STATUS_RESTORE_EMPTY, C_YELLOW),
            (STATUS_SEARCH_EMPTY_QUERY, C_YELLOW),
            // P03: 検索未オープンの案内も拒否通知として amber。
            (STATUS_SEARCH_NOT_OPEN, C_YELLOW),
            // Q03: 「できません」を含むが具体的な拒否通知なので warning(amber)。
            (STATUS_BOOKMARK_NOT_PERSISTABLE, C_YELLOW),
            // Y08: 監視の初期化失敗は secondary slot で直接黄色に描画されるが、
            // 色分類でも warning マーカーとして固定する。
            (STATUS_WATCH_INIT_FAILED, C_YELLOW),
            // AC04: 部分的/無効な監視も STATUS_WARNING_MARKERS 登録済みなので、
            // 回帰表にも固定して成功 green への落下を防ぐ。
            (STATUS_WATCH_PARTIAL, C_YELLOW),
            (STATUS_WATCH_DISABLED, C_YELLOW),
            // W10: 「キャンセルしています…」は進行中の一時通知で拒否ではない。
            // 完了通知 STATUS_CANCELLED（黄色）と意図的に色を分け、成功 green
            // のままとする。strings.rs の分類表と同じ判断をここでも固定する。
            (STATUS_CANCELLING, C_GREEN),
            (STATUS_CHANGED, C_GREEN),
            // Q02: 同一フォルダへの操作は拒否ではなく不要な操作を行わない
            // 情報通知なので、従来どおり成功 green のまま。
            (STATUS_SAME_DIRECTORY_NO_MOVE, C_GREEN),
            (STATUS_DROP_SAME_DIRECTORY, C_GREEN),
            ("3 件をコピーしました", C_GREEN),
            ("", C_GREEN),
        ];
        for (status, expected) in cases {
            assert_eq!(status_color(status), expected, "status = {status:?}");
        }
        // The partial-failure suffix alone must also be classified as an error.
        assert_eq!(
            status_color(&format!(
                "移動しました{STATUS_PARTIAL_FAILURE_SUFFIX}理由）"
            )),
            C_RED
        );
        // Composed properties statuses carry a path after the sentinel.
        assert_eq!(
            status_color(&format!("{STATUS_ACCESS_DENIED}: /tmp/props")),
            C_RED
        );
        assert_eq!(
            status_color(&format!("{STATUS_PATH_GONE}: /tmp/props")),
            C_RED
        );
        // The same sentinels appear with a path appended at their call sites.
        for marker in [
            STATUS_TARGET_GONE,
            STATUS_TARGET_MISSING,
            STATUS_DROP_NOT_FOUND,
            STATUS_NOT_DIRECTORY,
        ] {
            assert_eq!(
                status_color(&format!("{marker}: /tmp/target")),
                C_RED,
                "composed status for {marker:?} must be red"
            );
        }
        // N02: 実際の 3 箇所の呼び出し文も warning になることを確認する。
        let disabled = terminal_disabled_status();
        assert!(disabled.contains(STATUS_CANNOT_OPEN_SUFFIX), "{disabled}");
        // S07: 文言は strings.rs の接尾 + 理由/復帰導線から組み立てる
        // （完全一致の文言テストは strings.rs 側に置く）。
        assert_eq!(
            disabled,
            format!("ターミナルを{STATUS_CANNOT_OPEN_SUFFIX}{STATUS_TERMINAL_DISABLED_HINT}")
        );
        assert_eq!(status_color(&disabled), C_YELLOW, "{disabled}");
        // W01: 2 箇所の拒否接頭辞も strings.rs の定数から組み立てる。
        for status in [
            format!("{STATUS_TRASH_NO_TERMINAL_PREFIX}{STATUS_CANNOT_OPEN_SUFFIX}"),
            format!("{STATUS_FILE_NO_NEW_TAB_PREFIX}{STATUS_CANNOT_OPEN_SUFFIX}"),
        ] {
            assert_eq!(status_color(&status), C_YELLOW, "{status}");
        }
        // W10: 一時進行通知は警告・エラーのどちらのマーカーにも登録しない。
        assert!(!STATUS_WARNING_MARKERS.contains(&STATUS_CANCELLING));
        assert!(!STATUS_ERROR_MARKERS.contains(&STATUS_CANCELLING));
        // N02 のマーカー追加で、明示的なエラー接頭辞付き起動エラーが
        // amber に落ちないこと（接頭辞優先）も同時に検証する。
        let cli_error = format!("{STATUS_ERROR_PREFIX}{STATUS_INVALID_CLI_DIR}");
        assert_eq!(status_color(&cli_error), C_RED, "{cli_error}");
    }

    #[test]
    fn rescan_failed_survives_watch_changed_until_success() {
        // Z02: 再スキャン失敗は次回の成功読み込みまで保持する（strings.rs F6-36）。
        let failed = format!("{STATUS_RESCAN_FAILED}: permission denied");
        assert_eq!(status_color(&failed), C_RED);
        assert!(failed.contains("（F5 で再読込）"), "F5 導線が残ること");

        // 監視イベントの "変更を検出しました" は失敗を上書きしない。
        let after_watch = reconcile_aware_status(&failed, STATUS_CHANGED);
        assert_eq!(after_watch, failed, "watch event must not bury the failure");
        assert_eq!(status_color(&after_watch), C_RED);

        // AC01: the Search-view form of the same watch notice
        // (STATUS_SEARCH_STALE) must be guarded identically; otherwise the
        // Search rescan/rename branch buries the held failure.
        assert_eq!(reconcile_aware_status(&failed, STATUS_SEARCH_STALE), failed);

        // 成功側の通知は同じヘルパーを素通りし、赤を解除できる（固定されない）。
        let success = "再読込しました";
        let cleared = reconcile_aware_status(&after_watch, success);
        assert_eq!(cleared, success);
        assert_eq!(status_color(&cleared), C_GREEN);

        // 失敗中でなければ "変更を検出しました" は従来どおり green。
        let ordinary = reconcile_aware_status("3 件をコピーしました", STATUS_CHANGED);
        assert_eq!(ordinary, STATUS_CHANGED);
        assert_eq!(status_color(&ordinary), C_GREEN);
    }

    // Z14: the successful read must actually clear the held RESCAN_FAILED
    // notice (F6-36 "until the next successful read"), not merely let a later
    // status overwrite it by accident.
    #[test]
    fn successful_reconcile_clears_the_held_rescan_failure() {
        let failed = format!("{STATUS_RESCAN_FAILED}: permission denied");
        assert_eq!(
            clear_rescan_failure_status(&failed),
            Some(String::new()),
            "a fresh successful read clears the stale failure"
        );
        assert_eq!(
            clear_rescan_failure_status(STATUS_CHANGED),
            None,
            "unrelated statuses must be left untouched"
        );
        assert_eq!(clear_rescan_failure_status(""), None);
    }

    /// Slice of `include_str!("main.rs")` spanning a unique item marker up to
    /// the next item marker; keeps source assertions away from the test text.
    fn source_region<'a>(source: &'a str, start: &str, end: &str) -> &'a str {
        let from = source
            .find(start)
            .unwrap_or_else(|| panic!("missing source marker: {start}"));
        let to = source[from..]
            .find(end)
            .map(|offset| from + offset)
            .unwrap_or(source.len());
        &source[from..to]
    }

    /// Byte index just past the brace closing the block opened at `open`.
    fn braced_block_end(source: &str, open: usize) -> usize {
        assert_eq!(source.as_bytes()[open], b'{');
        let mut depth = 0usize;
        for (offset, ch) in source[open..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth = depth.checked_sub(1).expect("unbalanced braces");
                    if depth == 0 {
                        return open + offset + 1;
                    }
                }
                _ => {}
            }
        }
        panic!("unterminated braced block");
    }

    // AA01: the shared helper mutates the real status string, not just returns
    // a flag, and leaves every non-failure notice untouched.
    #[test]
    fn successful_read_helper_clears_only_held_rescan_failure() {
        let mut failed = format!("{STATUS_RESCAN_FAILED}: permission denied");
        assert!(clear_rescan_failure_after_successful_read(&mut failed));
        assert_eq!(failed, "", "F6-36 hold ends at the next successful read");
        assert!(!clear_rescan_failure_after_successful_read(&mut failed));

        let mut ordinary = STATUS_CHANGED.to_string();
        assert!(!clear_rescan_failure_after_successful_read(&mut ordinary));
        assert_eq!(ordinary, STATUS_CHANGED);

        let mut empty = String::new();
        assert!(!clear_rescan_failure_after_successful_read(&mut empty));
        assert_eq!(empty, "");
    }

    // AA01(a): F5 success is a fresh read too. `reload` cannot be driven
    // headlessly (it needs a live GPUI Context), so pin the call site in source
    // and exercise the exact status transition with the shared helper above.
    #[test]
    fn f5_reload_success_clears_the_held_rescan_failure() {
        let source = include_str!("main.rs");
        let reload = source_region(
            source,
            "    fn reload(&mut self, cx: &mut Context<Self>) {",
            "    fn set_load_error(",
        );
        let ok = reload
            .find("Ok(entries) => {")
            .expect("reload success branch missing");
        let err = reload[ok..]
            .find("Err(error) => {")
            .map(|offset| ok + offset)
            .expect("reload error branch missing");
        let ok_branch = &reload[ok..err];
        assert!(
            ok_branch.contains(concat!("clear_rescan_failure_", "after_successful_read")),
            "F5 success must clear the held failure; Ok branch was:\n{ok_branch}"
        );

        let mut status = format!("{STATUS_RESCAN_FAILED}: permission denied");
        assert!(clear_rescan_failure_after_successful_read(&mut status));
        assert_eq!(status, "");
    }

    // AA01(b): a successful reconcile with an unchanged fingerprint has
    // changed=false AND clear_error=false, so it never enters the
    // `should_apply_loaded_on_reconcile` branch. The clear must therefore sit
    // after that branch, not inside it.
    #[test]
    fn no_change_reconcile_success_clears_the_held_rescan_failure() {
        use super::should_apply_loaded_on_reconcile;

        let source = include_str!("main.rs");
        let reconcile = source_region(
            source,
            "    fn reconcile(&mut self, cx: &mut Context<Self>) {",
            "    fn spawn_watch_loop(",
        );
        let guard = reconcile
            .find("if applied {")
            .expect("reconcile apply guard missing");
        let open = guard
            + reconcile[guard..]
                .find('{')
                .expect("reconcile apply guard body missing");
        let after_guard = &reconcile[braced_block_end(reconcile, open)..];
        assert!(
            after_guard.contains(concat!("clear_rescan_failure_", "after_successful_read")),
            "a no-change successful reconcile must clear outside the apply guard; tail was:\n{after_guard}"
        );

        // The exact flag combination that used to skip the clear.
        assert!(!should_apply_loaded_on_reconcile(false, false));
        let mut status = format!("{STATUS_RESCAN_FAILED}: read failed");
        assert!(clear_rescan_failure_after_successful_read(&mut status));
        assert_eq!(status, "");
    }

    #[test]
    fn dynamic_refusal_statuses_are_not_green() {
        // M05: 「解除する設定はありません」は操作拒否なので warning(amber)。
        let noop = keymap_unbind_caption(false, "open_preview");
        assert!(noop.contains(STATUS_KEYMAP_NO_UNBIND_PREFIX), "{noop}");
        assert_eq!(status_color(&noop), C_YELLOW, "no-op unbind: {noop}");
        // 実際に解除できた場合の成功通知は従来どおり green。
        let removed = keymap_unbind_caption(true, "open_preview");
        assert_eq!(
            status_color(&removed),
            C_GREEN,
            "successful unbind: {removed}"
        );

        // M06: 動的な名前・件数を含む拒否も concrete failure として error(red)。
        let missing_search = format!("保存検索{STATUS_NOT_FOUND_SUFFIX}: ghost");
        assert_eq!(status_color(&missing_search), C_RED, "{missing_search}");
        let missing_count = format!("3{STATUS_MISSING_COUNT_SUFFIX}");
        assert_eq!(status_color(&missing_count), C_RED, "{missing_count}");

        // N08: 動的な上限値を含む「タブは最大 {n} 個までです」もwarning(amber)。
        let max_tabs = format!("{STATUS_MAX_TABS_PREFIX}8{STATUS_MAX_TABS_SUFFIX}");
        assert!(max_tabs.contains(STATUS_MAX_TABS_PREFIX), "{max_tabs}");
        assert_eq!(status_color(&max_tabs), C_YELLOW, "{max_tabs}");
    }

    #[test]
    fn settings_validation_error_status_is_red() {
        // The settings Save path mirrors validation errors from the card footer
        // into `self.status`. Without the prefix the pure classifier reads those
        // messages as success green even though the footer paints them red.
        // Use the same helper the Save handler calls, so a future refactor that
        // drops the prefix fails here instead of silently turning errors green.
        // AC08: the settings-card invalid-keymap error must carry the same
        // canonical format hint as the Alt+K dialog. The Save handler needs a
        // live GPUI context, so pin its wiring in source and check that the
        // composed message still classifies as red when mirrored.
        let source = include_str!("main.rs");
        let handler = source_region(source, "    fn on_settings_action(", "fn classify_cli(");
        assert!(handler.contains("STATUS_KEYMAP_INVALID_PREFIX"));
        assert!(
            handler.contains("KEYMAP_FORMAT_HINT"),
            "settings-card invalid-keymap error must include the shared format hint"
        );
        let invalid_keymap =
            format!("{STATUS_KEYMAP_INVALID_PREFIX}ctrl+x（{KEYMAP_FORMAT_HINT}）");
        assert!(invalid_keymap.contains(KEYMAP_FORMAT_HINT));

        let messages = [
            invalid_keymap,
            keymap_limit_status(),
            STATUS_TERMINAL_COMMAND_INVALID.to_string(),
        ];
        for message in messages {
            assert_eq!(status_color(&message), C_GREEN, "unprefixed: {message}");
            let mirrored = settings_validation_status(&message);
            assert!(
                mirrored.starts_with(STATUS_ERROR_PREFIX),
                "handler composition lost the prefix: {mirrored}"
            );
            assert_eq!(mirrored, format!("{STATUS_ERROR_PREFIX}{message}"));
            assert_eq!(status_color(&mirrored), C_RED, "mirrored: {mirrored}");
        }
    }

    #[test]
    fn settings_discard_status_only_reports_a_dirty_form() {
        // H03: Esc / backdrop dismissal is silent for a clean form, but must
        // name the loss when the edited form is thrown away. The footer Cancel
        // button keeps its own `保存せず閉じる` label and does not use this.
        assert_eq!(settings_discard_status(false), None);
        let discarded = settings_discard_status(true).expect("dirty form must report the discard");
        assert!(
            discarded.starts_with("未保存の変更を破棄しました"),
            "{discarded}"
        );
        // I06: the notice also tells the user how to reopen the settings, and
        // the status bar mirrors the canonical constant verbatim.
        assert!(
            discarded.contains("Ctrl+, で設定を開き直せます"),
            "{discarded}"
        );
        assert_eq!(discarded, STATUS_SETTINGS_DISCARDED);
        // X02: データ消失の通知を成功色で表示してはならない。
        assert_eq!(status_color(STATUS_SETTINGS_DISCARDED), C_YELLOW);
    }

    #[test]
    fn theme_labels_are_shared_by_status_and_settings_options() {
        // H08: both entry points must report the same localized name.
        assert_eq!(theme_choice_label(ThemeChoice::Dark), THEME_LABEL_DARK);
        assert_eq!(theme_choice_label(ThemeChoice::Light), THEME_LABEL_LIGHT);
        assert_eq!(theme_choice_label(ThemeChoice::System), THEME_LABEL_SYSTEM);
        assert_eq!(
            format!(
                "{STATUS_THEME_CHANGED_PREFIX}{}",
                theme_choice_label(ThemeChoice::Light)
            ),
            "テーマ: ライト"
        );
    }

    #[test]
    fn palette_status_color_uses_active_theme_colors() {
        let dark = AppPalette::dark();
        let light = AppPalette::light();
        // (status, Mocha expectation, Latte expectation). The pure classifier
        // returns Mocha constants, so the palette must remap them per theme.
        let cases = [
            (STATUS_SEARCH_STALE, dark.yellow, light.yellow),
            (STATUS_CANCELLED, dark.yellow, light.yellow),
            (STATUS_SETTINGS_READ_ONLY, dark.yellow, light.yellow),
            // X02: 破棄通知もテーマごとの warning 色へマップされる。
            (STATUS_SETTINGS_DISCARDED, dark.yellow, light.yellow),
            (STATUS_NO_SELECTION, dark.yellow, light.yellow),
            // Y01: 回帰表に欠けていた NO_* / keymap / bookmark の warning。
            (STATUS_NO_CLIPBOARD, dark.yellow, light.yellow),
            (STATUS_DROP_NO_DESTINATION, dark.yellow, light.yellow),
            (STATUS_TRASH_NO_CUT, dark.yellow, light.yellow),
            (STATUS_TRASH_NO_PASTE, dark.yellow, light.yellow),
            (STATUS_TRASH_NO_RENAME, dark.yellow, light.yellow),
            (STATUS_TRASH_NO_PURGE, dark.yellow, light.yellow),
            (STATUS_TRASH_NO_HIDDEN, dark.yellow, light.yellow),
            (STATUS_TRASH_NO_SORT, dark.yellow, light.yellow),
            (STATUS_KEYMAP_NO_UNBIND_PREFIX, dark.yellow, light.yellow),
            (STATUS_BOOKMARK_NOT_PERSISTABLE, dark.yellow, light.yellow),
            // Y08: 監視の初期化失敗も warning 色へマップされる。
            (STATUS_WATCH_INIT_FAILED, dark.yellow, light.yellow),
            // AC04: 部分的/無効な監視の警告もテーマ色へマップされる。
            (STATUS_WATCH_PARTIAL, dark.yellow, light.yellow),
            (STATUS_WATCH_DISABLED, dark.yellow, light.yellow),
            // L06: refusal / empty-target notices use the warning color.
            (STATUS_TABS_ONLY_DIRECTORY, dark.yellow, light.yellow),
            (STATUS_BOOKMARKS_ONLY_DIRECTORY, dark.yellow, light.yellow),
            (STATUS_LAST_TAB, dark.yellow, light.yellow),
            (STATUS_DROP_ONLY_DIRECTORY, dark.yellow, light.yellow),
            (STATUS_DROP_EMPTY, dark.yellow, light.yellow),
            (STATUS_TAB_RESTORE_EMPTY, dark.yellow, light.yellow),
            (STATUS_UNDO_EMPTY, dark.yellow, light.yellow),
            (STATUS_REDO_EMPTY, dark.yellow, light.yellow),
            (STATUS_TRASH_NO_COPY, dark.yellow, light.yellow),
            // N02 / N03 / N08 / N09: 新規の拒否通知もテーマ色へマップされる。
            (STATUS_CANNOT_OPEN_SUFFIX, dark.yellow, light.yellow),
            (STATUS_PREVIEW_ONLY_DIRECTORY, dark.yellow, light.yellow),
            (STATUS_LOCATION_ONLY_DIRECTORY, dark.yellow, light.yellow),
            (STATUS_FUZZY_JUMP_ONLY_DIRECTORY, dark.yellow, light.yellow),
            (STATUS_PROPERTIES_ONLY_DIRECTORY, dark.yellow, light.yellow),
            (STATUS_SEARCH_ONLY_DIRECTORY, dark.yellow, light.yellow),
            (STATUS_MAX_TABS_PREFIX, dark.yellow, light.yellow),
            (STATUS_RESTORE_EMPTY, dark.yellow, light.yellow),
            (STATUS_SEARCH_EMPTY_QUERY, dark.yellow, light.yellow),
            (STATUS_SEARCH_NOT_OPEN, dark.yellow, light.yellow),
            (STATUS_ERROR_PREFIX, dark.red, light.red),
            (STATUS_SETTINGS_SAVE_FAILED, dark.red, light.red),
            // X01: 理由別の保存失敗もテーマごとの error 色へマップされる。
            (STATUS_SETTINGS_SAVE_TOO_LARGE, dark.red, light.red),
            (STATUS_SETTINGS_SAVE_LOCKED, dark.red, light.red),
            (STATUS_ACCESS_DENIED, dark.red, light.red),
            (STATUS_PATH_GONE, dark.red, light.red),
            (STATUS_TARGET_GONE, dark.red, light.red),
            (STATUS_TARGET_MISSING, dark.red, light.red),
            (STATUS_DROP_NOT_FOUND, dark.red, light.red),
            (STATUS_NOT_DIRECTORY, dark.red, light.red),
            // Y01: 回帰表に欠けていた concrete error マーカーもテーマ色へ。
            (STATUS_RESCAN_FAILED, dark.red, light.red),
            (STATUS_TRASH_ALL_FAILED, dark.red, light.red),
            (STATUS_RESTORE_FAILED, dark.red, light.red),
            (STATUS_UNDO_FAILED, dark.red, light.red),
            (STATUS_RENAME_FAILED, dark.red, light.red),
            (STATUS_OP_FAILED_SUFFIX, dark.red, light.red),
            (STATUS_PARTIAL_FAILURE_SUFFIX, dark.red, light.red),
            (STATUS_NOT_FOUND_SUFFIX, dark.red, light.red),
            (STATUS_MISSING_COUNT_SUFFIX, dark.red, light.red),
            (STATUS_SPECIAL_FILE_NO_COPY, dark.red, light.red),
            (STATUS_FOLDER_NO_PREVIEW, dark.red, light.red),
            (STATUS_SYMLINK_NO_PREVIEW, dark.red, light.red),
            (STATUS_SAVED_SEARCH_SAVE_FAILED_PREFIX, dark.red, light.red),
            (STATUS_BOOKMARK_LABEL_SET_FAILED_PREFIX, dark.red, light.red),
            (STATUS_BOOKMARK_LABEL_NOT_SET_PREFIX, dark.red, light.red),
            (STATUS_TRASH_LOAD_FAILED_PREFIX, dark.red, light.red),
            (STATUS_UNDO_MISSING_SUFFIX, dark.red, light.red),
            // X09 / Y01: 接頭辞付きでなくても concrete error として赤。
            (STATUS_INVALID_CLI_DIR, dark.red, light.red),
            ("3 件をコピーしました", dark.green, light.green),
            ("", dark.green, light.green),
        ];
        for (status, expected_dark, expected_light) in cases {
            assert_eq!(dark.status_color(status), expected_dark, "dark: {status:?}");
            assert_eq!(
                light.status_color(status),
                expected_light,
                "light: {status:?}"
            );
        }
        // Light results must not be the Mocha constants the classifier emits.
        assert_ne!(light.yellow, C_YELLOW);
        assert_ne!(light.red, C_RED);
        assert_ne!(light.status_color(STATUS_SEARCH_STALE), C_YELLOW);
        assert_ne!(light.status_color(STATUS_ERROR_PREFIX), C_RED);
    }

    #[test]
    fn palette_kind_color_uses_active_theme_colors() {
        let dark = AppPalette::dark();
        let light = AppPalette::light();
        let cases = [
            (FileKind::Folder, dark.blue, light.blue),
            (FileKind::Symlink, dark.muted, light.muted),
            (FileKind::Image, dark.green, light.green),
            (FileKind::Code, dark.lavender, light.lavender),
            (FileKind::Archive, dark.yellow, light.yellow),
            (FileKind::Pdf, dark.red, light.red),
            (FileKind::Text, dark.subtext, light.subtext),
            (FileKind::Audio, dark.mauve, light.mauve),
            (FileKind::Video, dark.teal, light.teal),
            (FileKind::Unknown, dark.muted, light.muted),
        ];
        for (kind, expected_dark, expected_light) in cases {
            assert_eq!(dark.kind_color(kind), expected_dark, "dark: {kind:?}");
            assert_eq!(light.kind_color(kind), expected_light, "light: {kind:?}");
            assert_ne!(
                expected_light, expected_dark,
                "light kind {kind:?} must not reuse the Mocha tint"
            );
        }
        // Explicit Mocha-constant checks: the light palette must not fall back.
        assert_ne!(light.kind_color(FileKind::Folder), C_BLUE);
        assert_ne!(light.kind_color(FileKind::Pdf), C_RED);
        assert_ne!(light.kind_color(FileKind::Archive), C_YELLOW);
    }

    #[test]
    fn palette_status_color_covers_all_warning_and_error_markers() {
        // Y01: マーカーの重複は strings.rs の分類表との相互照合を曖昧にし、
        // 削除漏れを見逃す原因になるため、両配列で一意性を固定する。
        for (markers, label) in [
            (STATUS_WARNING_MARKERS, "warning"),
            (STATUS_ERROR_MARKERS, "error"),
        ] {
            let mut seen = std::collections::HashSet::new();
            for marker in markers {
                assert!(seen.insert(*marker), "duplicate {label} marker: {marker}");
            }
        }
        for palette in [AppPalette::dark(), AppPalette::light()] {
            for marker in STATUS_WARNING_MARKERS {
                assert_eq!(
                    palette.status_color(marker),
                    palette.yellow,
                    "warning marker {marker:?} must be amber"
                );
            }
            for marker in STATUS_ERROR_MARKERS {
                assert_eq!(
                    palette.status_color(marker),
                    palette.red,
                    "error marker {marker:?} must be red"
                );
            }
        }
    }

    #[test]
    fn status_priority_anchors_secondary_slot_order() {
        assert_eq!(STATUS_PRIORITY.last(), Some(&"監視状態"));
        assert_eq!(SECONDARY_SLOT_PRIORITY.len(), 4);
        assert_eq!(SECONDARY_SLOT_PRIORITY.last(), Some(&SecondarySlot::Watch));
    }

    #[test]
    fn visible_secondary_slots_hide_lowest_priority_first() {
        let budget = STATUS_CHAR_BUDGET_PER_TIER * STATUS_PRIORITY.len();
        assert_eq!(visible_secondary_slots(""), SECONDARY_SLOT_PRIORITY);
        let short_status = "a".repeat(budget - 1);
        assert_eq!(
            visible_secondary_slots(&short_status),
            SECONDARY_SLOT_PRIORITY
        );

        // Each extra full tier of status text hides one slot: 96/112/128/144 chars.
        let at_budget = "a".repeat(budget);
        assert_eq!(visible_secondary_slots(&at_budget), SECONDARY_SLOT_PRIORITY);
        let one = "a".repeat(budget + STATUS_CHAR_BUDGET_PER_TIER);
        assert_eq!(visible_secondary_slots(&one), &SECONDARY_SLOT_PRIORITY[..3]);
        let two = "a".repeat(budget + STATUS_CHAR_BUDGET_PER_TIER * 2);
        assert_eq!(visible_secondary_slots(&two), &SECONDARY_SLOT_PRIORITY[..2]);
        let three = "a".repeat(budget + STATUS_CHAR_BUDGET_PER_TIER * 3);
        assert_eq!(
            visible_secondary_slots(&three),
            &SECONDARY_SLOT_PRIORITY[..1]
        );
        let all = "a".repeat(budget + STATUS_CHAR_BUDGET_PER_TIER * 4);
        assert!(visible_secondary_slots(&all).is_empty());
        // Never panic or wrap even for a pathologically long status.
        let huge = "a".repeat(budget + 10_000);
        assert!(visible_secondary_slots(&huge).is_empty());
    }

    #[test]
    fn join_status_notices_keeps_every_startup_message() {
        let notices = vec![
            "設定を保存できませんでした".to_string(),
            "設定を読み取り専用で開きました: 理由".to_string(),
            "エラー: 起動ディレクトリを開けませんでした".to_string(),
        ];
        assert_eq!(
            join_status_notices(&notices),
            "設定を保存できませんでした / 設定を読み取り専用で開きました: 理由 / エラー: 起動ディレクトリを開けませんでした"
        );
        assert_eq!(join_status_notices(&[]), "");
        let single = vec!["唯一の通知".to_string()];
        assert_eq!(join_status_notices(&single), "唯一の通知");
    }

    #[test]
    fn debug_settings_overrides_defaults_leave_settings_untouched() {
        // Release builds never reach the override path: `debug_hooks_from_env`
        // returns `DebugHooks::default()` and `Explorer::new` compiles the whole
        // block out. An all-false override set models that state exactly, so
        // the user's real settings must not change at all.
        let mut settings = Settings::default();
        let expected = settings.clone();
        apply_debug_settings_overrides(
            &mut settings,
            &PathBuf::from("/tmp/gpui-fe-no-debug"),
            DebugSettingsOverrides::default(),
        );
        assert_eq!(settings, expected);
    }

    #[test]
    fn debug_settings_overrides_apply_when_enabled() {
        // Guard against a vacuous regression test: with the hooks enabled the
        // very same function really does mutate the four settings fields.
        let start_dir = PathBuf::from("/tmp/gpui-fe-debug-start");
        let mut settings = Settings::default();
        apply_debug_settings_overrides(
            &mut settings,
            &start_dir,
            DebugSettingsOverrides {
                sort_kind: true,
                recent: true,
                bookmarks: true,
                search_history: true,
            },
        );
        assert_eq!(settings.sort.key, SortKey::Kind);
        assert!(settings.recent_dirs.contains(&start_dir));
        assert!(settings.bookmarks.contains(&start_dir));
        assert_eq!(
            settings.recent_searches,
            vec!["TODO".to_string(), "rs".to_string(), "config".to_string()]
        );
    }

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
        // Symlinks intentionally reuse the muted tint already applied to them
        // in the list and grid render paths, so they are excluded above.
        assert_eq!(kind_color(FileKind::Symlink), C_MUTED);
    }

    #[test]
    fn toolbar_icon_tint_comes_from_the_active_palette() {
        // C03: the toolbar buttons used to hardcode Mocha `C_TEXT`/`C_DIM`,
        // so in the light theme the icons stayed dark-on-light and were
        // nearly invisible. The tint must follow the active palette.
        let dark = AppPalette::dark();
        let light = AppPalette::light();

        assert_eq!(icon_button_text_color(&dark, true), C_TEXT);
        assert_eq!(icon_button_text_color(&dark, false), C_DIM);
        assert_eq!(icon_button_text_color(&light, true), light.text);
        assert_eq!(icon_button_text_color(&light, false), light.dim);

        // The enabled light tint must differ from the old hardcoded Mocha one.
        assert_ne!(light.text, C_TEXT);
        assert_ne!(
            icon_button_text_color(&light, true),
            icon_button_text_color(&dark, true)
        );
    }

    #[test]
    fn sidebar_width_is_shared_by_rendering_and_grid_layout() {
        // The sidebar is rendered with `px(SIDEBAR_WIDTH_PX)` and the icon grid
        // is laid out in `viewport - SIDEBAR_WIDTH_PX`. Lock the shared value so
        // the two sites cannot drift apart again (regression: 214 vs 190).
        assert_eq!(SIDEBAR_WIDTH_PX, 190.0);
        let viewport_width = 1000.0f32;
        let content_width = viewport_width - SIDEBAR_WIDTH_PX;
        assert_eq!(content_width, 810.0);
        assert_eq!(grid_columns_for_width(content_width), 6);
    }

    #[test]
    fn grid_columns_budget_padding_and_gaps_within_viewport() {
        // U-17: each `grid_row` costs `.px_3()` (24px horizontal) plus
        // `.gap_2()` (8px × (n-1)) on top of the 108px cells. The computed
        // column count must fit the entire row inside the content area.
        for viewport_width in [800.0, 1000.0, 1100.0, 1280.0, 1920.0] {
            let content_width = viewport_width - SIDEBAR_WIDTH_PX;
            let columns = grid_columns_for_width(content_width);
            let required = GRID_ROW_PADDING_PX
                + columns as f32 * GRID_CELL_WIDTH_PX
                + columns.saturating_sub(1) as f32 * GRID_GAP_PX;
            assert!(
                required <= content_width,
                "viewport {viewport_width}: {columns} columns need {required}px > {content_width}px"
            );
        }
        // Default 1100x728 window: plain division gave 8 columns (944px needed
        // for 910px of content); the padded calculation must drop to 7.
        let content_width = 1100.0 - SIDEBAR_WIDTH_PX;
        assert_eq!(grid_columns_for_width(content_width), 7);
        assert_eq!(
            GRID_ROW_PADDING_PX + 7.0 * GRID_CELL_WIDTH_PX + 6.0 * GRID_GAP_PX,
            828.0
        );
    }

    #[test]
    fn tab_label_uses_last_component_or_root() {
        use std::path::Path;
        assert_eq!(tab_label(Path::new("/")), "ルート");
        assert_eq!(tab_label(Path::new("/home/koppe")), "koppe");
        assert_eq!(tab_label(Path::new("relative/dir")), "dir");
        // AE06: `.` / `..` have no `file_name()`, so the previously untested
        // fallback must keep the literal path via `display_path` instead of
        // yielding an empty tab label.
        assert_eq!(tab_label(Path::new(".")), ".");
        assert_eq!(tab_label(Path::new("..")), "..");
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
    fn transfer_log_level_reflects_outcome() {
        use tracing::Level;
        assert_eq!(transfer_log_level(&None, false), Level::INFO);
        assert_eq!(transfer_log_level(&None, true), Level::DEBUG);
        assert_eq!(
            transfer_log_level(&Some(OpError::Cancelled), false),
            Level::DEBUG
        );
        let failure = Some(OpError::InvalidTarget("理由".to_string()));
        assert_eq!(transfer_log_level(&failure, false), Level::ERROR);
        assert_eq!(transfer_log_level(&failure, true), Level::DEBUG);
    }

    #[test]
    fn transfer_skip_notice_preserves_drop_reasons() {
        // T02: paste / move / drop now share one wording template, so only the
        // reason label changes while the "N 件をスキップ" suffix stays common.
        assert_eq!(transfer_skip_notice(&[]), "");
        // Y02: 理由ラベルは strings.rs の定数経由で組み立てる。
        assert_eq!(
            transfer_skip_notice(&[(STATUS_SKIP_REASON_MISSING, 3)]),
            "（存在しない項目 3 件をスキップ）"
        );
        let notice = transfer_skip_notice(&[
            (STATUS_SKIP_REASON_MISSING, 2),
            (STATUS_SKIP_REASON_SPECIAL, 1),
        ]);
        assert_eq!(notice, "（存在しない項目 2 / 特殊ファイル 1 件をスキップ）");
        // Z03: 混在ケースで same_dir / same が消えないことを固定する。
        let three = transfer_skip_notice(&[
            (STATUS_SKIP_REASON_MISSING, 2),
            (STATUS_SKIP_REASON_SPECIAL, 1),
            (STATUS_SKIP_REASON_SAME_DIRECTORY, 3),
        ]);
        assert_eq!(
            three,
            "（存在しない項目 2 / 特殊ファイル 1 / 同じフォルダ 3 件をスキップ）"
        );
    }

    #[test]
    fn trash_keyboard_routes_restore_and_no_purge() {
        assert_eq!(trash_key_action("enter", false), TrashKeyAction::Restore);
        assert_eq!(trash_key_action("r", false), TrashKeyAction::Restore);
        assert_eq!(trash_key_action("enter", true), TrashKeyAction::Restore);
        assert_eq!(trash_key_action("delete", false), TrashKeyAction::NoPurge);
        assert_eq!(trash_key_action("delete", true), TrashKeyAction::Ignore);
        assert_eq!(trash_key_action("x", false), TrashKeyAction::Ignore);
        let status = trash_delete_status();
        assert!(status.contains("Enter"), "{status}");
        assert!(status.contains('R'), "{status}");
        assert!(status.contains("復元"), "{status}");
    }

    #[test]
    fn internal_drop_keeps_multi_selection_and_drops_self_and_duplicates() {
        let destination = PathBuf::from("/work/target");
        let dragged = vec![
            PathBuf::from("/work/a.txt"),
            destination.clone(),
            PathBuf::from("/work/b.txt"),
            PathBuf::from("/work/a.txt"),
        ];
        assert_eq!(
            internal_drop_paths(&dragged, &destination),
            vec![PathBuf::from("/work/a.txt"), PathBuf::from("/work/b.txt")]
        );
        assert!(internal_drop_paths(&[], &destination).is_empty());
    }

    // AA07: `record_trashed_paths` wraps a blocking trash listing, so exercise
    // its pure snapshot-mapping half with raw (invalid UTF-8) bytes and pin the
    // wrapper's delegation in source. Losing bytes here would silently break
    // undo: ids are compared as OsString, not lossy Strings.
    #[cfg(unix)]
    #[test]
    fn record_trashed_paths_preserves_non_utf8_bytes_and_order() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};

        let id_a = OsString::from_vec(b"/trash/info/a-\xff.trashinfo".to_vec());
        let id_b = OsString::from_vec(b"/trash/info/b-\xfe.trashinfo".to_vec());
        let path_a = PathBuf::from(OsString::from_vec(b"/tmp/\xff-a.txt".to_vec()));
        let path_b = PathBuf::from(OsString::from_vec(b"/tmp/\xfe-b.txt".to_vec()));
        let items = vec![
            trash_view::TrashItem {
                id: id_a.clone(),
                name: OsString::from_vec(b"\xff-a.txt".to_vec()),
                original_parent: PathBuf::from("/tmp"),
                time_deleted: 100,
            },
            trash_view::TrashItem {
                id: id_b.clone(),
                name: OsString::from_vec(b"\xfe-b.txt".to_vec()),
                original_parent: PathBuf::from("/tmp"),
                time_deleted: 200,
            },
        ];

        // Request b before a: caller order (not snapshot order) is preserved,
        // and each returned id must be the exact original byte sequence.
        let recorded = trashed_ids_from_snapshot(&items, &[path_b.clone(), path_a.clone()]);
        assert_eq!(recorded.len(), 2);
        assert_eq!(
            recorded[0].0.as_os_str().as_bytes(),
            path_b.as_os_str().as_bytes()
        );
        assert_eq!(
            recorded[0].1.as_os_str().as_bytes(),
            id_b.as_os_str().as_bytes()
        );
        assert_eq!(
            recorded[1].0.as_os_str().as_bytes(),
            path_a.as_os_str().as_bytes()
        );
        assert_eq!(
            recorded[1].1.as_os_str().as_bytes(),
            id_a.as_os_str().as_bytes()
        );
        assert!(trashed_ids_from_snapshot(&items, &[PathBuf::from("/tmp/missing")]).is_empty());

        // The blocking wrapper must use the exact mapper exercised above.
        let source = include_str!("main.rs");
        let wrapper = source_region(
            source,
            "fn record_trashed_paths(trashed: &[PathBuf])",
            "fn internal_drop_paths(",
        );
        assert!(
            wrapper.contains(concat!("trashed_ids_from_", "snapshot")),
            "record_trashed_paths must route through the tested mapper: {wrapper}"
        );
    }

    #[test]
    fn pending_rename_waits_for_a_visible_loaded_row() {
        let entries = vec![
            Entry {
                name: OsString::from("a.txt"),
                name_text: "a.txt".to_string(),
                sort_key: "a.txt".to_string(),
                path: PathBuf::from("/work/a.txt"),
                is_dir: false,
                is_symlink: false,
                size: 1,
                modified: None,
            },
            Entry {
                name: OsString::from("新しいフォルダ"),
                name_text: "新しいフォルダ".to_string(),
                sort_key: "新しいフォルダ".to_string(),
                path: PathBuf::from("/work/新しいフォルダ"),
                is_dir: true,
                is_symlink: false,
                size: 0,
                modified: None,
            },
        ];
        let target = &entries[1].path;
        assert_eq!(
            pending_rename_index(false, false, true, &entries, target),
            Some(1)
        );
        // Do not steal the keyboard while loading or while another dialog is
        // open, and never rename from a non-directory view.
        assert_eq!(
            pending_rename_index(true, false, true, &entries, target),
            None
        );
        assert_eq!(
            pending_rename_index(false, true, true, &entries, target),
            None
        );
        assert_eq!(
            pending_rename_index(false, false, false, &entries, target),
            None
        );
        assert_eq!(
            pending_rename_index(false, false, true, &entries, &PathBuf::from("/work/gone")),
            None
        );
    }

    // P02: there is no headless GPUI window to press F1 in, so verify the guard
    // order in the compiled source (same technique as the rollback call-site
    // test). The toggle must run after every modal that used to let help stack
    // underneath it; needles are split so this test's own text cannot match.
    #[test]
    fn f1_help_toggle_runs_after_modal_guards() {
        let source = include_str!("main.rs");
        let toggle = source
            .find(concat!("self", ".help_open = !", "self", ".help_open"))
            .expect("F1 help toggle missing");
        let guards = [
            concat!("if self.fuzzy.is_some() {", "\n            match key {"),
            concat!("if self.help_open {", "\n            // Keyboard scrolling"),
            concat!(
                "if self.location.is_some() {",
                "\n            if key == \"escape\" {"
            ),
            "if self.undo_history_open {",
            "if self.preview.is_some() {",
            "if self.properties.is_some() {",
        ];
        for guard in guards {
            let pos = source.find(guard).expect("modal guard missing");
            assert!(
                pos < toggle,
                "{guard} must consume F1 before the help toggle opens help"
            );
        }
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
    fn help_reports_evicted_default_bindings_as_unassigned() {
        // Direct-state regression: a keymap that drops the shipped single f4
        // binding for open_terminal must make F1 help stop claiming F4 works
        // (P07). The chord, not just the action, is now reported.
        let mut keymap = KeyMap::default_keymap();
        keymap
            .bindings
            .retain(|b| b.action != KeyAction::OpenTerminal);
        let overrides = active_keymap_overrides(&keymap);
        assert!(
            overrides.contains(&("open_terminal".to_string(), "f4 → 未割り当て".to_string())),
            "evicted default chord must be reported: {overrides:?}"
        );
        let line = help_overrides_line(&keymap).expect("override notice");
        assert!(line.contains("open_terminal → f4 → 未割り当て"), "{line}");
    }

    #[test]
    fn help_reports_unassigned_default_chord_after_real_rebind() {
        // P07/R01: a real `rebind` moves the action, leaving its old chord
        // unclaimed while the action stays bound elsewhere. Rebinding
        // open_terminal to space also evicts open_preview's default space
        // binding, so space is still claimed -- but by a different action.
        // R01 requires the SAME action to hold a default chord for it to count
        // as claimed, so open_preview's displaced space must be reported as
        // unassigned even though open_terminal now owns space.
        let mut keymap = KeyMap::default_keymap();
        assert!(keymap.rebind(KeyAction::OpenTerminal, "space", false, false, false));
        let overrides = active_keymap_overrides(&keymap);
        assert!(
            overrides.contains(&("open_terminal".to_string(), "space".to_string())),
            "live rebind must still be reported: {overrides:?}"
        );
        assert!(
            overrides.contains(&("open_terminal".to_string(), "f4 → 未割り当て".to_string())),
            "f4 must be reported as unassigned: {overrides:?}"
        );
        assert!(
            overrides.contains(&("open_preview".to_string(), "space → 未割り当て".to_string())),
            "open_preview's space default is displaced by open_terminal: {overrides:?}"
        );
        assert!(
            !overrides
                .iter()
                .any(|(_, chord)| chord == "f3 → 未割り当て"),
            "open_preview keeps its f3 default: {overrides:?}"
        );
        let line = help_overrides_line(&keymap).expect("override notice");
        assert!(line.contains("open_terminal → f4 → 未割り当て"), "{line}");
        assert!(line.contains("open_preview → space → 未割り当て"), "{line}");
    }

    #[test]
    fn help_reports_every_unassigned_default_chord_for_one_action() {
        // P07: open_preview ships two default chords (space / f3). Rebinding it
        // must report both; sort/dedup must not collapse them into one line.
        let mut keymap = KeyMap::default_keymap();
        assert!(keymap.rebind(KeyAction::OpenPreview, "f9", false, false, false));
        let overrides = active_keymap_overrides(&keymap);
        for chord in ["f3", "space"] {
            assert!(
                overrides.contains(&("open_preview".to_string(), format!("{chord} → 未割り当て"))),
                "{chord} must be reported: {overrides:?}"
            );
        }
        let line = help_overrides_line(&keymap).expect("override notice");
        assert!(line.contains("open_preview → f3 → 未割り当て"), "{line}");
        assert!(line.contains("open_preview → space → 未割り当て"), "{line}");
    }

    #[test]
    fn help_lists_active_overrides() {
        let default = KeyMap::default_keymap();
        assert!(active_keymap_overrides(&default).is_empty());
        assert!(help_overrides_line(&default).is_none());
        assert_eq!(keymap_action_name(KeyAction::OpenPreview), "open_preview");
        assert_eq!(
            chord_for_action(&default, KeyAction::OpenTerminal).as_deref(),
            Some("f4")
        );

        let mut keymap = KeyMap::default_keymap();
        assert!(keymap.rebind(KeyAction::OpenPreview, "p", true, true, false));
        // P07: the live override stays, and both shipped open_preview chords
        // (f3 / space) are now unassigned. Sorted by (action, chord).
        assert_eq!(
            active_keymap_overrides(&keymap),
            vec![
                ("open_preview".to_string(), "ctrl+shift+p".to_string()),
                ("open_preview".to_string(), "f3 → 未割り当て".to_string()),
                ("open_preview".to_string(), "space → 未割り当て".to_string()),
            ]
        );
        assert_eq!(
            chord_for_action(&keymap, KeyAction::OpenPreview).as_deref(),
            Some("ctrl+shift+p")
        );
        // Static HELP_SECTIONS still says "space / F3"; the supplemental
        // notice must carry the live binding after the Alt+K rebind.
        let line = help_overrides_line(&keymap).expect("override notice");
        assert!(line.contains("open_preview → ctrl+shift+p"), "{line}");

        assert!(keymap.rebind(KeyAction::OpenTerminal, "k", true, false, false));
        let line = help_overrides_line(&keymap).expect("override notice");
        assert!(line.contains("open_terminal → ctrl+k"), "{line}");
        let terminal_binding = keymap
            .bindings
            .iter()
            .find(|binding| binding.action == KeyAction::OpenTerminal)
            .expect("terminal binding");
        assert_eq!(format_chord(terminal_binding), "ctrl+k");
    }

    #[test]
    fn dialog_error_clears_on_input_change() {
        let mut error = Some("既に存在します".to_string());
        assert!(clear_dialog_error(&mut error));
        assert!(error.is_none());
        assert!(
            !clear_dialog_error(&mut error),
            "clearing an empty error needs no repaint"
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

        // V13: Trash rejects Ctrl+H, so it must not suggest the chord
        // unconditionally; the recovery route is to leave Trash first.
        let (title, _) = empty_state_message(ViewSource::Trash, false, None, 5, 0).unwrap();
        assert_eq!(
            title,
            "隠しファイル 5 件が非表示です（ゴミ箱を出て Ctrl+H で表示）"
        );
        assert!(title.contains("ゴミ箱を出て Ctrl+H"));
        assert!(!title.contains("（Ctrl+H で表示）"));

        let (title, _) = empty_state_message(ViewSource::Directory, true, None, 0, 0).unwrap();
        assert_eq!(title, "読み込み中…");

        // AC07: the load-error recovery hint is view-specific. Ctrl+L only
        // applies to the Directory view; Trash advertises the working exit
        // chords (Alt+← / Backspace) and Search advertises Esc.
        let (title, hint) =
            empty_state_message(ViewSource::Directory, false, Some("読めません"), 0, 0).unwrap();
        assert_eq!(title, "エラー: 読めません");
        assert_eq!(
            hint.as_deref(),
            Some(crate::strings::LOAD_ERROR_HINT_DIRECTORY)
        );
        assert!(hint.unwrap().contains("Ctrl+L"));

        let (_, hint) =
            empty_state_message(ViewSource::Trash, false, Some("読めません"), 0, 0).unwrap();
        assert_eq!(hint.as_deref(), Some(crate::strings::LOAD_ERROR_HINT_TRASH));
        let trash_hint = hint.unwrap();
        assert!(trash_hint.contains("F5"));
        // AD02: Esc does not leave Trash, so the hint must advertise the
        // actual exit chords instead of promising a key that only cancels ops.
        assert!(trash_hint.contains("Alt+←"), "{trash_hint}");
        assert!(trash_hint.contains("Backspace"), "{trash_hint}");
        assert!(!trash_hint.contains("Esc"), "{trash_hint}");
        assert!(!trash_hint.contains("Ctrl+L"));

        let (_, hint) =
            empty_state_message(ViewSource::Search, false, Some("読めません"), 0, 0).unwrap();
        assert_eq!(
            hint.as_deref(),
            Some(crate::strings::LOAD_ERROR_HINT_SEARCH)
        );
        let search_hint = hint.unwrap();
        assert!(search_hint.contains("F5"));
        assert!(search_hint.contains("Esc"));
        assert!(!search_hint.contains("Ctrl+L"));

        assert!(empty_state_message(ViewSource::Directory, false, None, 3, 2).is_none());
    }

    #[test]
    fn empty_state_error_overlay_prefixed_exactly_once() {
        use super::{empty_state_message, ViewSource};

        // AD01: `reload_trash` stores a status that already carries
        // STATUS_ERROR_PREFIX, so the overlay must not add a second one.
        let error = trash_load_error_status("permission denied");
        let (title, hint) =
            empty_state_message(ViewSource::Trash, false, Some(&error), 0, 0).unwrap();

        assert_eq!(title, error);
        assert_eq!(
            title.matches(STATUS_ERROR_PREFIX).count(),
            1,
            "overlay must show the error prefix exactly once: {title}"
        );
        let doubled = format!("{STATUS_ERROR_PREFIX}{STATUS_ERROR_PREFIX}");
        assert!(!title.contains(&doubled), "doubled prefix in: {title}");
        assert_eq!(hint.as_deref(), Some(crate::strings::LOAD_ERROR_HINT_TRASH));

        // A plain (unprefixed) error still gets exactly one prefix.
        let (plain_title, _) =
            empty_state_message(ViewSource::Trash, false, Some("読めません"), 0, 0).unwrap();
        assert_eq!(plain_title, format!("{STATUS_ERROR_PREFIX}読めません"));
        assert_eq!(plain_title.matches(STATUS_ERROR_PREFIX).count(), 1);
    }

    #[test]
    fn selection_status_text_distinguishes_cursor_from_selection() {
        use super::selection_status_text;

        // U-11: with no selection the cursor must not be reported as a choice.
        for implicit in [true, false] {
            assert_eq!(
                selection_status_text(0, Some("first.txt（フォルダ）"), implicit),
                "選択なし"
            );
        }

        // A load/reload auto-target is shown as a cursor, not a user pick.
        assert_eq!(
            selection_status_text(1, Some("__pycache__（フォルダ）"), true),
            "カーソル: __pycache__（フォルダ）"
        );
        // An explicit click / keyboard selection keeps the "選択" label.
        assert_eq!(
            selection_status_text(1, Some("first.txt（フォルダ）"), false),
            "選択: first.txt（フォルダ）"
        );

        // Missing or empty detail never claims a selection.
        assert_eq!(selection_status_text(1, None, true), "選択なし");
        assert_eq!(selection_status_text(1, None, false), "選択なし");
        assert_eq!(selection_status_text(1, Some(""), false), "選択なし");

        // Multi-selection count is independent of the implicit flag.
        for implicit in [true, false] {
            assert_eq!(
                selection_status_text(3, Some("first.txt（フォルダ）"), implicit),
                "3 件選択"
            );
        }
    }

    #[test]
    fn implicit_selection_only_changes_the_single_entry_label() {
        use super::selection_status_text;

        // Dogfooding regression: a fresh load auto-selects the first row and
        // must report it as a cursor, never as a user selection.
        let detail = Some("__pycache__（フォルダ）");
        assert_eq!(
            selection_status_text(1, detail, true),
            "カーソル: __pycache__（フォルダ）"
        );
        assert_eq!(
            selection_status_text(1, detail, false),
            "選択: __pycache__（フォルダ）"
        );

        // Operations still use `selection.len()`; an implicit path remains a
        // real selection internally, so only the 1-entry text differs.
        assert_eq!(selection_status_text(2, detail, true), "2 件選択");
        assert_eq!(selection_status_text(2, detail, false), "2 件選択");
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
        let nested_roots = vec![cwd.clone(), nested_parent.clone(), nested_child.clone()];
        assert_eq!(
            display_name_for(&in_other_root, ViewSource::Search, &cwd, &nested_roots),
            "docs/notes.txt"
        );

        // AC05: the "keep current best when a later root is longer" branch.
        // With roots in short→long order the first relative name is already
        // the shortest, so the later (longer) relative name must not replace it.
        let short_first_roots = vec![nested_child, nested_parent];
        assert_eq!(
            display_name_for(&in_other_root, ViewSource::Search, &cwd, &short_first_roots),
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
    fn load_error_invalidates_in_flight_reload_generation() {
        use super::error_state_with_fresh_generation;

        // Regression (finding 1): an error raised outside a reload callback
        // must carry a *new* generation. Otherwise an in-flight callback whose
        // captured generation still matches `load.generation()` passes its
        // guard and overwrites the error with `Ready` + stale entries.
        let mut generation = 5u64;
        let in_flight_generation = generation;
        let error = error_state_with_fresh_generation(&mut generation, "移動失敗".to_string());
        assert_eq!(generation, in_flight_generation.wrapping_add(1));
        assert_eq!(error.generation(), Some(generation));
        assert_ne!(error.generation(), Some(in_flight_generation));
        assert_eq!(error.error_message(), Some("移動失敗"));

        // The generation check is the exact guard used by the reload callback.
        let callback_generation = Some(in_flight_generation);
        let stale = callback_generation != error.generation();
        assert!(stale, "古いリロードはエラーを上書きできないこと");

        // Wrapping at the u64 boundary must not panic (release build) or
        // accidentally reuse the previous generation.
        let mut generation = u64::MAX;
        let error = error_state_with_fresh_generation(&mut generation, "迂回".to_string());
        assert_eq!(generation, 0);
        assert_eq!(error.generation(), Some(0));
    }

    #[test]
    fn reconcile_success_clears_only_error_load_state() {
        use super::{should_clear_error_on_reconcile, LoadState};

        assert!(should_clear_error_on_reconcile(&LoadState::Error {
            generation: 3,
            error: "読み込み失敗".to_string(),
        }));
        // An explicit reload may be in flight: its Loading state must win.
        assert!(!should_clear_error_on_reconcile(&LoadState::Loading {
            generation: 4
        }));
        assert!(!should_clear_error_on_reconcile(&LoadState::Ready));
    }

    #[test]
    fn reconcile_error_recovery_restores_entries() {
        use super::should_apply_loaded_on_reconcile;

        // Regression: after an error cleared `entries`, the fingerprint still
        // matched the last successful read (`changed == false`). A stale Error
        // overlay must nonetheless re-apply the fresh listing so the list is
        // repopulated when the overlay is cleared.
        assert!(should_apply_loaded_on_reconcile(false, true));
        // Normal change and the combined case must still re-apply exactly once.
        assert!(should_apply_loaded_on_reconcile(true, false));
        assert!(should_apply_loaded_on_reconcile(true, true));
        // No change and no error: skip rebuild/notify.
        assert!(!should_apply_loaded_on_reconcile(false, false));

        // A full state-machine test would need a live GPUI `Context<Explorer>`
        // plus the 10s timer to drive `cx.spawn`; `reconcile` cannot be invoked
        // from a unit test. The decision that caused the regression is captured
        // in the pure predicate above instead.
    }

    #[test]
    fn reconcile_result_is_applied_only_in_directory_view() {
        use super::{should_apply_reconcile, ViewSource};

        // The reported bug: search was opened while a reconcile was in flight,
        // generation still matched and no load was in progress, yet applying
        // the directory entries would overwrite the search results.
        assert!(should_apply_reconcile(ViewSource::Directory, true, false));
        assert!(!should_apply_reconcile(ViewSource::Search, true, false));
        // Trash owns its own listing too, so it must be rejected as well.
        assert!(!should_apply_reconcile(ViewSource::Trash, true, false));
        // The pre-existing generation/loading guards remain in force.
        assert!(!should_apply_reconcile(ViewSource::Directory, false, false));
        assert!(!should_apply_reconcile(ViewSource::Directory, true, true));
        assert!(!should_apply_reconcile(ViewSource::Directory, false, true));
    }

    #[test]
    fn reconcile_is_not_dispatched_while_a_read_is_in_flight() {
        use super::{should_start_reconcile, ViewSource};

        // Normal directory tick: dispatch allowed.
        assert!(should_start_reconcile(false, false, ViewSource::Directory));
        // The reported bug: a slow `read_entries` outlives the 10s timer; the
        // overlapping tick must be skipped so snapshots/tasks do not pile up.
        assert!(!should_start_reconcile(true, false, ViewSource::Directory));
        // Pre-existing guards remain in force.
        assert!(!should_start_reconcile(false, true, ViewSource::Directory));
        assert!(!should_start_reconcile(false, false, ViewSource::Search));
        assert!(!should_start_reconcile(false, false, ViewSource::Trash));
        assert!(!should_start_reconcile(true, true, ViewSource::Directory));
    }

    #[test]
    fn stale_properties_read_is_ignored_after_close_or_reopen() {
        use super::should_apply_properties_read;

        // Matching serial: the read still belongs to the current dialog.
        assert!(should_apply_properties_read(7, 7));
        // A superseded read (older serial) must never apply.
        assert!(!should_apply_properties_read(7, 6));
        // Close or reopen bumps the serial: the old callback is now stale even
        // though its path might be identical.
        assert!(!should_apply_properties_read(8, 7));
        // Wrapping must not accidentally let an old serial match again.
        assert!(!should_apply_properties_read(0, u64::MAX));
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

    #[test]
    fn cli_double_dash_terminates_option_scanning() {
        let args = |values: &[&str]| values.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let os_args = |values: &[&str]| values.iter().map(OsString::from).collect::<Vec<_>>();

        // Anything after `--` is a path, never a flag: `-- --help` is Run.
        assert_eq!(classify_cli(&args(&["--", "--help"])), CliAction::Run);
        assert_eq!(classify_cli(&args(&["--", "--version"])), CliAction::Run);
        assert_eq!(classify_cli(&args(&["--", "-h", "-V"])), CliAction::Run);
        // Flags before `--` still classify normally (precedence unchanged).
        assert_eq!(classify_cli(&args(&["--help", "--"])), CliAction::Help);
        assert_eq!(
            classify_cli(&args(&["--version", "--"])),
            CliAction::Version
        );

        // `--` itself is skipped and a following `-dir` is accepted as a path.
        assert_eq!(
            startup_dir_from(os_args(&["--", "-dir"])),
            Some(PathBuf::from("-dir"))
        );
        // A flag-like token after `--` is a path too.
        assert_eq!(
            startup_dir_from(os_args(&["--", "--help"])),
            Some(PathBuf::from("--help"))
        );
        // Before `--`, a leading `-` argument is still ignored as a flag.
        assert_eq!(
            startup_dir_from(os_args(&["--flag", "normal-dir"])),
            Some(PathBuf::from("normal-dir"))
        );
        // With no `--`, the first non-flag argument still wins.
        assert_eq!(
            startup_dir_from(os_args(&["-h", "/tmp/dir"])),
            Some(PathBuf::from("/tmp/dir"))
        );
        assert_eq!(startup_dir_from(os_args(&["-h", "-V"])), None);
    }

    #[test]
    fn invalid_cli_dir_is_reported_instead_of_silently_ignored() {
        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-cli-dir-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("a-file.txt");
        std::fs::write(&file, b"x").unwrap();
        let missing = dir.join("missing");

        // A real directory is accepted; a file and a missing path are rejected
        // with the original argument so the caller can print it in a warning.
        assert!(validate_cli_dir(dir.clone()).is_ok());
        assert_eq!(validate_cli_dir(file.clone()), Err(file));
        assert_eq!(validate_cli_dir(missing.clone()), Err(missing));

        let _ = std::fs::remove_dir_all(&dir);
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
        assert_eq!(
            tabs, saved,
            "temporarily missing tabs are kept so persist_tabs cannot drop them (finding 4)"
        );
        assert_eq!(
            start, dir,
            "out-of-range active index is clamped to the last tab"
        );
        assert_eq!(index, 1);

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

    #[test]
    fn restore_session_resolves_active_by_path_when_earlier_tab_vanishes() {
        use super::restore_session;
        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-active-path-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let missing = dir.join("A-missing");
        let b = dir.join("B");
        let c = dir.join("C");
        std::fs::create_dir_all(&b).unwrap();
        std::fs::create_dir_all(&c).unwrap();

        // saved = [A(missing), B, C], active = 1 (B). A numeric clamp applied
        // after A vanished would open C (index 1 of the shrunken list); the
        // path-based resolution must keep B active instead (finding 1).
        let saved = vec![missing, b.clone(), c];
        let (tabs, start, index) = restore_session(&saved, 1, None, None, None);
        assert_eq!(tabs, saved, "all saved tabs are retained (finding 4)");
        assert_eq!(index, 1, "active index must still point at B");
        assert_eq!(start, b, "B must be the tab opened after restore");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn restore_tab_sessions_reattaches_history_and_keeps_active_index() {
        use super::restore_tab_sessions;
        use crate::settings::{Settings, TabSession};

        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-tab-session-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut settings = Settings::default();
        settings.primary_window_mut().tabs = vec![
            TabSession {
                cwd: dir.clone(),
                back: vec![PathBuf::from("/alpha-parent")],
                forward: vec![],
            },
            TabSession {
                cwd: PathBuf::from("/beta"),
                back: vec![PathBuf::from("/beta-parent")],
                forward: vec![],
            },
        ];
        let saved_tabs = vec![dir.clone(), PathBuf::from("/beta")];
        let tabs = restore_tab_sessions(&settings, &saved_tabs, 1, &dir);
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs.active_index(), 1, "saved active tab must be restored");
        let mut first = tabs.tabs()[0].history.clone();
        assert_eq!(
            first.go_back(PathBuf::from("/probe")),
            Some(PathBuf::from("/alpha-parent")),
            "per-tab back history must survive restart"
        );
        let mut second = tabs.tabs()[1].history.clone();
        assert_eq!(
            second.go_back(PathBuf::from("/probe")),
            Some(PathBuf::from("/beta-parent"))
        );

        // A tab added by a CLI argument has no saved history and stays empty.
        let tabs = restore_tab_sessions(
            &settings,
            &[dir.clone(), PathBuf::from("/cli-only")],
            1,
            &dir,
        );
        assert!(!tabs.tabs()[1].history.can_go_back());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn restore_tab_sessions_keeps_distinct_histories_for_duplicate_cwd() {
        use super::restore_tab_sessions;
        use crate::settings::{Settings, TabSession};

        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-dup-cwd-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();

        // Two tabs share the exact same cwd but own different histories. A
        // single `find()` would hand both tabs session[0]'s history; matching
        // by the saved cwd string through a per-path queue fixes that
        // (finding 2). The queue also preserves order for duplicate paths.
        let mut settings = Settings::default();
        settings.primary_window_mut().tabs = vec![
            TabSession {
                cwd: dir.clone(),
                back: vec![PathBuf::from("/first-parent")],
                forward: vec![PathBuf::from("/first-forward")],
            },
            TabSession {
                cwd: dir.clone(),
                back: vec![PathBuf::from("/second-parent")],
                forward: vec![PathBuf::from("/second-forward")],
            },
        ];
        let saved_tabs = vec![dir.clone(), dir.clone()];
        let tabs = restore_tab_sessions(&settings, &saved_tabs, 1, &dir);
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs.active_index(), 1);

        // `go_back` pushes `current` onto the forward stack, so assert the
        // forward entry on a fresh clone of the restored history.
        let mut first = tabs.tabs()[0].history.clone();
        assert_eq!(
            first.go_back(PathBuf::from("/probe")),
            Some(PathBuf::from("/first-parent"))
        );
        let mut first_forward = tabs.tabs()[0].history.clone();
        assert_eq!(
            first_forward.go_forward(PathBuf::from("/probe")),
            Some(PathBuf::from("/first-forward"))
        );
        let mut second = tabs.tabs()[1].history.clone();
        assert_eq!(
            second.go_back(PathBuf::from("/probe")),
            Some(PathBuf::from("/second-parent"))
        );
        let mut second_forward = tabs.tabs()[1].history.clone();
        assert_eq!(
            second_forward.go_forward(PathBuf::from("/probe")),
            Some(PathBuf::from("/second-forward"))
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn restore_session_keeps_numeric_active_with_duplicate_cwd_and_history() {
        use super::{restore_session, restore_tab_sessions};
        use crate::settings::{Settings, TabSession};

        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-active-dup-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let a = dir.join("a");
        std::fs::create_dir_all(&a).unwrap();

        // Both saved tabs have the same cwd but distinct histories; the saved
        // active tab is the *second* one. A path-based `position()` resolves to
        // the first tab and would silently activate the wrong history.
        let mut settings = Settings::default();
        settings.primary_window_mut().tabs = vec![
            TabSession {
                cwd: a.clone(),
                back: vec![PathBuf::from("/first-parent")],
                forward: vec![PathBuf::from("/first-forward")],
            },
            TabSession {
                cwd: a.clone(),
                back: vec![PathBuf::from("/second-parent")],
                forward: vec![PathBuf::from("/second-forward")],
            },
        ];
        let saved_tabs = vec![a.clone(), a.clone()];

        let (tabs, start, active) = restore_session(&saved_tabs, 1, None, None, None);
        assert_eq!(tabs, saved_tabs, "duplicate-cwd tabs are preserved");
        assert_eq!(start, a);
        assert_eq!(active, 1, "numeric active index must win over first match");

        let tabs = restore_tab_sessions(&settings, &tabs, active, &start);
        assert_eq!(tabs.len(), 2);
        assert_eq!(tabs.active_index(), 1);
        let mut active_history = tabs.tabs()[tabs.active_index()].history.clone();
        assert_eq!(
            active_history.go_back(PathBuf::from("/probe")),
            Some(PathBuf::from("/second-parent")),
            "the restored active tab must own the second tab's history"
        );
        let mut active_forward = tabs.tabs()[tabs.active_index()].history.clone();
        assert_eq!(
            active_forward.go_forward(PathBuf::from("/probe")),
            Some(PathBuf::from("/second-forward"))
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    // F6-09: the shared operation sentinel must survive overlapping
    // completion handlers and must be observable by every action, not only
    // paste/copy (trash, undo, redo, restore and rename share it now).

    #[test]
    fn stale_completion_keeps_a_newer_operation_sentinel() {
        let mut id = 0;
        let mut cancel = None;
        let first = install_op_token(&mut id, &mut cancel);
        assert_eq!(first.id, 1);
        assert!(cancel.is_some());

        // The UI guard normally prevents this, but a late callback from the
        // first operation must still leave the newer operation cancellable.
        let second = install_op_token(&mut id, &mut cancel);
        assert_eq!(second.id, 2);
        assert!(!clear_op_token(id, &mut cancel, &first));
        assert!(cancel.is_some(), "stale callback cleared a live flag");

        assert!(clear_op_token(id, &mut cancel, &second));
        assert!(cancel.is_none());
    }

    #[test]
    fn operation_ids_are_never_reused_after_completion() {
        let mut id = 0;
        let mut cancel = None;
        let first = install_op_token(&mut id, &mut cancel);
        assert!(clear_op_token(id, &mut cancel, &first));

        let second = install_op_token(&mut id, &mut cancel);
        assert_eq!(second.id, 2, "monotonic ids prevent an ABA mix-up");
        assert!(!clear_op_token(id, &mut cancel, &first));
        assert!(clear_op_token(id, &mut cancel, &second));
    }

    #[test]
    fn stale_progress_cannot_claim_a_settled_operation() {
        let mut id = 0;
        let mut cancel = None;
        let token = install_op_token(&mut id, &mut cancel);
        assert!(
            op_is_active(id, &cancel, &token),
            "live op accepts progress"
        );

        // `finish_op` clears the flag but deliberately keeps the monotonic id.
        assert!(clear_op_token(id, &mut cancel, &token));
        assert!(
            !op_is_active(id, &cancel, &token),
            "queued progress must not repaint the final status"
        );

        // A newer overlapping op gets a fresh id and flag; it must keep working
        // while the old token stays rejected.
        let newer = install_op_token(&mut id, &mut cancel);
        assert!(!op_is_active(id, &cancel, &token));
        assert!(op_is_active(id, &cancel, &newer));
    }

    #[test]
    fn esc_flag_is_shared_between_token_holders() {
        let mut id = 0;
        let mut cancel = None;
        let token = install_op_token(&mut id, &mut cancel);
        let background_view = token.cancel.clone();
        assert!(!token.is_cancelled());
        token.cancel(); // what Esc does through the sentinel
        assert!(background_view.is_cancelled());
        assert!(token.is_cancelled());
    }

    #[test]
    fn cancelled_trash_batch_does_not_delete_files() {
        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-f6-09-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("keep.txt");
        std::fs::write(&file, b"keep").unwrap();

        let mut id = 0;
        let mut cancel = None;
        let token = install_op_token(&mut id, &mut cancel);
        token.cancel();

        let (trashed, failures, cancelled) = trash_batch(vec![file.clone()], &token.cancel);
        assert!(cancelled);
        assert!(trashed.is_empty());
        assert!(failures.is_empty());
        assert!(file.exists(), "Esc must stop before the first deletion");

        let _ = std::fs::remove_dir_all(&dir);
    }

    // F6-18: the properties metadata read now runs on the background executor.
    // These tests exercise the exact snapshot function that got moved, so the
    // dialog keeps showing the same values after the offload.

    fn f618_temp_dir(tag: &str) -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "gpui-fe-f6-18-{tag}-{}-{stamp}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[cfg(unix)]
    #[test]
    fn path_present_sees_broken_symlinks() {
        use super::path_present;

        let dir = f618_temp_dir("present");
        let missing = dir.join("missing");
        assert!(!missing.exists());
        assert!(!path_present(&missing));

        let file = dir.join("file.txt");
        std::fs::write(&file, b"x").unwrap();
        assert!(path_present(&file));

        let link = dir.join("broken-link");
        std::os::unix::fs::symlink(dir.join("no-such-target"), &link).unwrap();
        assert!(!link.exists(), "a dangling link fails Path::exists");
        assert!(
            path_present(&link),
            "the link entry itself still exists and must be undoable/revealable"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn search_view_key_routing_is_limited_to_navigation_and_copy() {
        use super::{search_view_key_allowed, search_view_key_reaches_keymap};

        for key in ["up", "down", "pageup", "pagedown", "home", "end", "enter"] {
            assert!(
                search_view_key_allowed(key, false, false, false),
                "blocked {key}"
            );
            assert!(
                !search_view_key_allowed(key, false, false, true),
                "Alt+{key} must keep its old behaviour"
            );
        }
        assert!(search_view_key_allowed("c", true, false, false));
        // F4 (terminal) and Ctrl+Q (quit) are global actions: they must reach
        // normal dispatch while browsing results, but stay blocked in the input.
        // The input-focus guard lives in `on_key_down`, not in this pure helper.
        assert!(search_view_key_allowed("f4", false, false, false));
        assert!(!search_view_key_allowed("f4", false, false, true));
        assert!(search_view_key_allowed("q", true, false, false));
        assert!(!search_view_key_allowed("q", false, false, false));
        assert!(!search_view_key_allowed("q", true, true, false));
        for (key, ctrl) in [
            ("c", false),
            ("a", true),
            ("escape", false),
            ("f2", false),
            ("delete", false),
        ] {
            assert!(
                !search_view_key_allowed(key, ctrl, false, false),
                "unexpectedly forwarded {key} (ctrl={ctrl})"
            );
        }

        // AC02: the hardcoded Ctrl+C / Ctrl+Q shortcuts must be unshifted.
        // Ctrl+Shift+C resolves to `CopyPath`, which is not an allowed Search
        // action, so the old shift-blind hardcoded OR leaked it to the keymap.
        assert!(!search_view_key_allowed("c", true, true, false));
        assert!(search_view_key_allowed("c", true, false, false));
        assert!(!search_view_key_allowed("q", true, true, false));
        let default = KeyMap::default_keymap();
        assert_eq!(default.resolve("c", true, true, false), KeyAction::CopyPath);
        assert!(!search_view_key_reaches_keymap(
            &default, "c", true, true, false
        ));
        // Contrast: plain Ctrl+C still reaches normal dispatch.
        assert!(search_view_key_reaches_keymap(
            &default, "c", true, false, false
        ));
        // Ctrl+Shift+Q resolves to Quit (its binding is shift-agnostic), which
        // is allowed in Search just like in the normal view; only the
        // hardcoded shortcut is now shift-sensitive.
        assert_eq!(default.resolve("q", true, true, false), KeyAction::Quit);
    }

    #[test]
    fn search_view_key_reaches_keymap_honours_allowed_action_rebinds() {
        use super::{search_view_key_allowed, search_view_key_reaches_keymap};

        let default = KeyMap::default_keymap();
        // The shipped keystrokes still work.
        assert!(search_view_key_reaches_keymap(
            &default, "up", false, false, false
        ));
        assert!(search_view_key_reaches_keymap(
            &default, "c", true, false, false
        ));
        // A destructive action is never forwarded, even if bound to a new key.
        assert!(!search_view_key_reaches_keymap(
            &default, "delete", false, false, false
        ));

        // Rebinding an allowed action to an otherwise unlisted key must make
        // that key reach normal dispatch in the Search view (finding 4).
        let mut rebound = KeyMap::default_keymap();
        assert!(rebound.rebind(KeyAction::Copy, "y", true, false, false));
        assert!(!search_view_key_allowed("y", true, false, false));
        assert!(search_view_key_reaches_keymap(
            &rebound, "y", true, false, false
        ));
        // The action's old keystroke is gone, but the hardcoded allowlist still
        // forwards it (harmless no-op), matching the pre-rebind behaviour.
        assert!(search_view_key_reaches_keymap(
            &rebound, "c", true, false, false
        ));

        // Rebinding a disallowed action stays blocked.
        let mut rebound = KeyMap::default_keymap();
        assert!(rebound.rebind(KeyAction::TrashSelected, "y", true, false, false));
        assert!(!search_view_key_reaches_keymap(
            &rebound, "y", true, false, false
        ));
    }

    // AA04: every action in `search_view_action_allowed` must keep working
    // while browsing results after a rebind to a key the hardcoded allowlist
    // does not know. The earlier test only covered Copy (and OpenSettings via
    // its default chord), leaving nine actions untested.
    #[test]
    fn search_view_every_allowed_action_reaches_keymap_when_rebound() {
        use super::{
            search_view_action_allowed, search_view_key_allowed, search_view_key_reaches_keymap,
        };

        // Unlisted single-letter chords. `rebind` refuses OpenSettings, so that
        // allowed action is verified via its shipped Ctrl+, below. AC03 adds
        // Reload (F5) to the set; its default chord is checked separately.
        let cases: &[(KeyAction, &str)] = &[
            (KeyAction::MoveUp, "y"),
            (KeyAction::MoveDown, "u"),
            (KeyAction::PageUp, "i"),
            (KeyAction::PageDown, "o"),
            (KeyAction::SelectFirst, "p"),
            (KeyAction::SelectLast, "n"),
            (KeyAction::OpenSelected, "m"),
            (KeyAction::OpenTerminal, "j"),
            (KeyAction::Copy, "k"),
            (KeyAction::Quit, "l"),
            (KeyAction::Reload, "b"),
        ];
        assert_eq!(cases.len() + 1, 12, "all allowed actions must be covered");
        for &(action, key) in cases {
            assert!(
                search_view_action_allowed(action),
                "test table must only contain allowed actions: {action:?}"
            );
            let mut keymap = KeyMap::default_keymap();
            assert!(
                keymap.rebind(action, key, false, false, false),
                "rebind {action:?} to {key} failed"
            );
            assert!(
                !search_view_key_allowed(key, false, false, false),
                "{key} must be unlisted so only the action path can forward it"
            );
            assert_eq!(keymap.resolve(key, false, false, false), action);
            assert!(
                search_view_key_reaches_keymap(&keymap, key, false, false, false),
                "rebinding {action:?} to {key} must reach normal dispatch"
            );
        }

        // The eleventh allowed action is deliberately not rebindable; its
        // shipped chord must still reach normal dispatch (L04).
        assert!(search_view_action_allowed(KeyAction::OpenSettings));
        let mut settings_keymap = KeyMap::default_keymap();
        assert!(!settings_keymap.rebind(KeyAction::OpenSettings, "y", false, false, false));
        assert!(search_view_key_reaches_keymap(
            &settings_keymap,
            ",",
            true,
            false,
            false
        ));

        // Contrast: a destructive action stays blocked even when rebound to
        // the same unlisted key.
        let mut rejected = KeyMap::default_keymap();
        assert!(rejected.rebind(KeyAction::TrashSelected, "y", false, false, false));
        assert!(!search_view_action_allowed(KeyAction::TrashSelected));
        assert!(!search_view_key_reaches_keymap(
            &rejected, "y", false, false, false
        ));

        // AB01: an Alt chord stays blocked even when an allowed action is
        // rebound to it. The keymap side alone would return `Copy` for the
        // ctrl+alt chord, so `reaches_keymap` must reject Alt before consulting
        // the keymap (matching the "Alt variants keep their prior blocked
        // behaviour" contract).
        let mut alt_rebound = KeyMap::default_keymap();
        assert!(alt_rebound.rebind(KeyAction::Copy, "y", true, false, true));
        assert_eq!(alt_rebound.resolve("y", true, false, true), KeyAction::Copy);
        assert!(!search_view_key_reaches_keymap(
            &alt_rebound,
            "y",
            true,
            false,
            true
        ));
    }

    // AC03: `STATUS_RESCAN_FAILED` advertises "F5 で再読込" and the status is
    // visible while browsing Search results, so the advertised key must be
    // reachable in that view. F5 is not in the hardcoded key allowlist; it
    // reaches normal dispatch through the allowed `Reload` action instead.
    #[test]
    fn search_view_advertised_f5_reload_key_reaches_keymap() {
        use super::{
            search_view_action_allowed, search_view_key_allowed, search_view_key_reaches_keymap,
        };

        let default = KeyMap::default_keymap();
        assert!(search_view_action_allowed(KeyAction::Reload));
        assert!(!search_view_key_allowed("f5", false, false, false));
        assert_eq!(
            default.resolve("f5", false, false, false),
            KeyAction::Reload
        );
        assert!(search_view_key_reaches_keymap(
            &default, "f5", false, false, false
        ));
        // AC10: reaching the keymap must have an effect — Search re-runs the
        // query instead of calling reload(), which early-returns in that view.
        assert!(super::reload_reruns_search(super::ViewSource::Search));
        assert!(!super::reload_reruns_search(super::ViewSource::Directory));
        assert!(!super::reload_reruns_search(super::ViewSource::Trash));
    }

    // AE01: the previous input-focus guard swallowed every key while the query
    // field owned focus, so F5 never reached `Reload` → `run_search` even
    // though STATUS_RESCAN_FAILED / LOAD_ERROR_HINT_SEARCH advertise it. The
    // pure helper is exercised with input_focused = true to lock the fix.
    #[test]
    fn search_input_focused_forwards_reload_but_keeps_other_keys() {
        use super::search_view_key_reaches_dispatch;

        let dispatch =
            |keymap: &KeyMap, key: &str, ctrl: bool, shift: bool, input_focused: bool| {
                let action = keymap.resolve(key, ctrl, shift, false);
                let reaches =
                    super::search_view_key_reaches_keymap(keymap, key, ctrl, shift, false);
                search_view_key_reaches_dispatch(input_focused, action, reaches)
            };

        let default = KeyMap::default_keymap();
        // Both shipped Reload chords pass the input-focus guard.
        assert_eq!(
            default.resolve("f5", false, false, false),
            KeyAction::Reload
        );
        assert_eq!(default.resolve("r", true, false, false), KeyAction::Reload);
        assert!(
            dispatch(&default, "f5", false, false, true),
            "F5 must re-run the query while typing"
        );
        assert!(
            dispatch(&default, "r", true, false, true),
            "Ctrl+R must re-run the query while typing"
        );

        // A rebind of Reload keeps working while typing.
        let mut rebound = KeyMap::default_keymap();
        assert!(rebound.rebind(KeyAction::Reload, "b", false, false, false));
        assert!(
            dispatch(&rebound, "b", false, false, true),
            "rebound Reload must pass the input-focus guard too"
        );

        // The documented contract still holds for every other key: the input
        // keeps F4 / Ctrl+Q (and navigation / Ctrl+,) while typing, even though
        // those actions are allowed when a result row owns focus.
        assert!(
            !dispatch(&default, "f4", false, false, true),
            "F4 stays in input"
        );
        assert!(
            !dispatch(&default, "q", true, false, true),
            "Ctrl+Q stays in input"
        );
        assert!(
            !dispatch(&default, "up", false, false, true),
            "arrows stay in input"
        );
        assert!(
            !dispatch(&default, ",", true, false, true),
            "Ctrl+, stays in input"
        );
        assert!(!dispatch(&default, "escape", false, false, true));

        // Without input focus the allowlist still forwards them all.
        assert!(dispatch(&default, "f4", false, false, false));
        assert!(dispatch(&default, "q", true, false, false));
        assert!(dispatch(&default, "up", false, false, false));
    }

    #[test]
    fn search_view_forwards_ctrl_comma_to_open_settings() {
        use super::{search_view_key_allowed, search_view_key_reaches_keymap};

        // L04: while browsing results, Ctrl+, must reach the normal keymap so
        // the preferences overlay can be opened without leaving the Search view.
        let keymap = KeyMap::default_keymap();
        assert!(!search_view_key_allowed(",", true, false, false));
        assert_eq!(
            keymap.resolve(",", true, false, false),
            KeyAction::OpenSettings
        );
        assert!(search_view_key_reaches_keymap(
            &keymap, ",", true, false, false
        ));
        // Unshifted comma and Ctrl+Shift+, stay harmless no-ops, as in the
        // normal view.
        assert!(!search_view_key_reaches_keymap(
            &keymap, ",", false, false, false
        ));
        assert!(!search_view_key_reaches_keymap(
            &keymap, ",", true, true, false
        ));
    }

    #[test]
    fn search_filter_status_keeps_freshness_warning() {
        let summary = "検索: 「rs」 3 件".to_string();

        // A freshness warning must survive a filter tweak and stay first.
        // AE08: build on the shared constant so its F5 recovery hint cannot
        // drift from the assertion.
        let stale = search_filter_status(STATUS_SEARCH_STALE, summary.clone());
        assert!(stale.starts_with(STATUS_SEARCH_STALE));
        assert!(stale.contains("3 件"));
        // Repeated tweaks must not duplicate the warning.
        let twice = search_filter_status(&stale, summary.clone());
        assert_eq!(twice.matches(STATUS_SEARCH_STALE).count(), 1);
        assert_eq!(twice.matches("F5 で再検索").count(), 1);

        // Ordinary statuses still get replaced by the fresh result count.
        assert_eq!(
            search_filter_status("検索中…「rs」", summary.clone()),
            summary
        );
    }

    #[test]
    fn search_view_removed_status_keeps_rescan_failure_and_stale_warning() {
        // AB02: the Search-view removed-entry watch site must compose
        // `reconcile_aware_status` with `search_filter_status` in that order:
        // the reconciled status becomes the summary, so a held RESCAN_FAILED
        // survives and a SEARCH_STALE warning still prefixes the summary (the
        // same expression the production watch site now uses).
        let compose = |current: &str| {
            let reconciled = reconcile_aware_status(current, STATUS_CHANGED);
            search_filter_status(current, reconciled)
        };

        assert_eq!(compose(STATUS_RESCAN_FAILED), STATUS_RESCAN_FAILED);

        let stale = compose(STATUS_SEARCH_STALE);
        assert!(stale.starts_with(STATUS_SEARCH_STALE));
        assert!(stale.contains(STATUS_CHANGED));
        assert_eq!(stale.matches(STATUS_SEARCH_STALE).count(), 1);

        assert_eq!(compose("検索中…「rs」"), STATUS_CHANGED);

        // AC01: the Rescan/Renamed branch uses the same composition with the
        // Search-view stale notice as `next`. A held RESCAN_FAILED must survive
        // it too (the old code assigned STATUS_SEARCH_STALE directly), and a
        // repeated stale notice must not be duplicated.
        let compose_rescan = |current: &str| {
            let reconciled = reconcile_aware_status(current, STATUS_SEARCH_STALE);
            search_filter_status(current, reconciled)
        };

        let failed_with_reason = format!("{STATUS_RESCAN_FAILED}: permission denied");
        assert_eq!(compose_rescan(&failed_with_reason), failed_with_reason);
        assert_eq!(compose_rescan(STATUS_RESCAN_FAILED), STATUS_RESCAN_FAILED);
        assert_eq!(compose_rescan(STATUS_SEARCH_STALE), STATUS_SEARCH_STALE);
        assert_eq!(compose_rescan("検索中…「rs」"), STATUS_SEARCH_STALE);
    }

    #[test]
    fn properties_snapshot_reads_file_metadata() {
        let dir = f618_temp_dir("file");
        let file = dir.join("sample.txt");
        std::fs::write(&file, b"abcd").unwrap();

        let snapshot = read_properties_snapshot(&file).unwrap();
        assert!(!snapshot.is_symlink);
        assert!(!snapshot.is_dir);
        assert_eq!(snapshot.size, Some(4));
        assert!(snapshot.modified.is_some());
        assert_eq!(snapshot.symlink_target, None);
        #[cfg(unix)]
        assert!(snapshot.permissions.is_some());
        #[cfg(not(unix))]
        assert!(snapshot.permissions.is_none());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn properties_snapshot_handles_dirs_and_missing_paths() {
        let dir = f618_temp_dir("dir");
        let snapshot = read_properties_snapshot(&dir).unwrap();
        assert!(snapshot.is_dir);
        assert!(!snapshot.is_symlink);
        assert_eq!(snapshot.size, None, "directory size is computed separately");

        assert!(read_properties_snapshot(&dir.join("missing")).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn properties_snapshot_follows_symlink_target_for_size() {
        use std::os::unix::fs::symlink;

        let dir = f618_temp_dir("link");
        let file = dir.join("target.txt");
        std::fs::write(&file, b"12345").unwrap();
        let link = dir.join("link.txt");
        symlink(&file, &link).unwrap();

        let snapshot = read_properties_snapshot(&link).unwrap();
        assert!(snapshot.is_symlink);
        assert!(!snapshot.is_dir);
        assert_eq!(snapshot.symlink_target, Some(file));
        assert_eq!(
            snapshot.size,
            Some(5),
            "metadata follows the symlink target"
        );

        let broken = dir.join("broken");
        symlink(dir.join("nope"), &broken).unwrap();
        let broken_snapshot = read_properties_snapshot(&broken).unwrap();
        assert!(broken_snapshot.is_symlink);
        // Broken target: `metadata()` fails, so the preserved fallback is the
        // symlink's own metadata (its len is the target path length).
        assert!(broken_snapshot.size.is_some());
        assert!(
            broken_snapshot.symlink_target.is_some(),
            "read_link works on broken links"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[cfg(unix)]
    #[test]
    fn bookmark_toggle_rejects_non_utf8_directory() {
        use std::os::unix::ffi::OsStringExt;

        // A cwd reached through the OS dialog can contain invalid UTF-8; the
        // Ctrl+D guard must refuse it before `toggle_bookmark` and its assert.
        let bad = PathBuf::from(OsString::from_vec(vec![b'/', b't', b'm', b'p', b'/', 0xff]));
        assert_eq!(
            bookmark_toggle_rejection(&bad),
            Some(STATUS_BOOKMARK_NOT_PERSISTABLE.to_string())
        );
        assert_eq!(
            bookmark_toggle_rejection(&PathBuf::from("/tmp/valid")),
            None
        );
    }

    #[test]
    fn bookmark_toggle_rejects_empty_nul_and_oversized_paths() {
        // `can_persist_path` is `sane_path`: non-empty, byte-bounded, NUL-free
        // and UTF-8. Every violation must be rejected before the toggle touches
        // bookmarks, not just the non-UTF-8 case (G14).
        let rejected = Some(STATUS_BOOKMARK_NOT_PERSISTABLE.to_string());
        // T04: the reason must stay generic — it is shown for all four
        // violations, not only for non-UTF-8 paths.
        let reason = rejected.as_ref().expect("a rejection reason");
        assert!(!reason.contains("非UTF-8"), "{reason}");
        assert!(reason.contains("保存できない形式"), "{reason}");
        assert_eq!(bookmark_toggle_rejection(&PathBuf::new()), rejected);
        assert_eq!(
            bookmark_toggle_rejection(&PathBuf::from("tmp\0dir")),
            rejected
        );
        assert_eq!(
            bookmark_toggle_rejection(&PathBuf::from(
                "x".repeat(crate::settings::MAX_PATH_BYTES + 1)
            )),
            rejected
        );
        assert_eq!(
            bookmark_toggle_rejection(&PathBuf::from("/tmp/valid")),
            None
        );
    }

    #[test]
    fn properties_error_reaction_distinguishes_not_found_denied_and_other() {
        let path = PathBuf::from("/tmp/props");
        let io_error =
            |kind: std::io::ErrorKind| FsError::from_io(path.clone(), &std::io::Error::from(kind));

        let missing = io_error(std::io::ErrorKind::NotFound);
        assert_eq!(
            PropertiesErrorReaction::for_error(&missing),
            PropertiesErrorReaction::PathGone
        );
        let (status, reload) = PropertiesErrorReaction::for_error(&missing).apply(&path, &missing);
        assert!(status.contains(STATUS_PATH_GONE), "{status}");
        assert!(reload, "only a gone path may trigger a reload");

        let denied = io_error(std::io::ErrorKind::PermissionDenied);
        assert_eq!(
            PropertiesErrorReaction::for_error(&denied),
            PropertiesErrorReaction::AccessDenied
        );
        let (status, reload) = PropertiesErrorReaction::for_error(&denied).apply(&path, &denied);
        assert!(status.contains(STATUS_ACCESS_DENIED), "{status}");
        assert!(!reload, "a denied read must keep the current listing");

        let other = io_error(std::io::ErrorKind::Other);
        assert_eq!(
            PropertiesErrorReaction::for_error(&other),
            PropertiesErrorReaction::Other
        );
        let (status, reload) = PropertiesErrorReaction::for_error(&other).apply(&path, &other);
        assert!(status.contains(STATUS_ERROR_PREFIX), "{status}");
        assert!(
            !reload,
            "an unrelated io error must keep the current listing"
        );
    }

    // U03: distinguish oversized serialization from lock contention instead
    // of showing a generic permissions/disk-space hint for every failure.
    #[test]
    fn save_failure_status_uses_the_recorded_reason() {
        assert_eq!(
            settings_save_error_status(Some(SettingsSaveError::TooLarge)),
            Some(STATUS_SETTINGS_SAVE_TOO_LARGE.to_string())
        );
        assert_eq!(
            settings_save_error_status(Some(SettingsSaveError::Locked)),
            Some(STATUS_SETTINGS_SAVE_LOCKED.to_string())
        );
        assert_eq!(
            settings_save_error_status(Some(SettingsSaveError::Io)),
            None
        );
        assert_eq!(settings_save_error_status(None), None);
        // Reason-specific notices must be classified as errors in the bar.
        for reason in [SettingsSaveError::TooLarge, SettingsSaveError::Locked] {
            let status = settings_save_error_status(Some(reason)).expect("reason notice");
            assert_eq!(status_color(&status), C_RED);
        }
    }

    // V01: the start-up notice is built before the UI exists, but it must still
    // use the reason `save_settings()` recorded instead of the generic message.
    #[test]
    fn startup_save_failure_status_uses_the_recorded_reason() {
        let too_large = Settings {
            last_save_error: std::cell::Cell::new(Some(SettingsSaveError::TooLarge)),
            ..Settings::default()
        };
        assert_eq!(
            startup_save_failure_status(&too_large),
            STATUS_SETTINGS_SAVE_TOO_LARGE
        );
        let locked = Settings {
            last_save_error: std::cell::Cell::new(Some(SettingsSaveError::Locked)),
            ..Settings::default()
        };
        assert_eq!(
            startup_save_failure_status(&locked),
            STATUS_SETTINGS_SAVE_LOCKED
        );
        // Unknown / plain I/O failures keep the previous generic wording.
        let io = Settings {
            last_save_error: std::cell::Cell::new(Some(SettingsSaveError::Io)),
            ..Settings::default()
        };
        assert_eq!(
            startup_save_failure_status(&io),
            STATUS_SETTINGS_SAVE_FAILED
        );
        assert_eq!(
            startup_save_failure_status(&Settings::default()),
            STATUS_SETTINGS_SAVE_FAILED
        );
    }

    // L01: a discarded settings-write result must never be reported as a
    // successful UI action. The pure selector is shared by every fixed path.
    #[test]
    fn settings_save_status_replaces_success_on_failed_write() {
        let success = "ブックマークに追加しました".to_string();
        assert_eq!(settings_save_status(true, success.clone()), success);
        assert_eq!(
            settings_save_status(false, success.clone()),
            STATUS_SETTINGS_SAVE_FAILED
        );
        // The failure sentinel must actually look like an error in the bar.
        assert_eq!(status_color(&settings_save_status(false, success)), C_RED);
    }

    // F04: `save()` reports success without writing for read-only / ephemeral
    // sessions. Every write path must then show the read-only notice instead of
    // its success caption, just as the settings overlay already did.
    // V06: when a real write failed, `settings_write_status` must prefer the
    // concrete reason recorded by `save_with_reason` over the caller's success
    // caption and over the generic failure notice.
    #[test]
    fn settings_write_status_prefers_the_recorded_failure_reason() {
        let success = "ブックマークに追加しました".to_string();
        let settings = Settings::default();
        for (reason, expected) in [
            (
                crate::settings::SettingsSaveError::TooLarge,
                STATUS_SETTINGS_SAVE_TOO_LARGE,
            ),
            (
                crate::settings::SettingsSaveError::Locked,
                STATUS_SETTINGS_SAVE_LOCKED,
            ),
        ] {
            settings.last_save_error.set(Some(reason));
            let status = settings_write_status(&settings, false, true, success.clone());
            assert_eq!(status, expected, "reason must beat the success caption");
            assert_ne!(status, STATUS_SETTINGS_SAVE_FAILED);
        }
        // No recorded reason falls back to the generic failure notice.
        settings.last_save_error.set(None);
        assert_eq!(
            settings_write_status(&settings, false, true, success.clone()),
            STATUS_SETTINGS_SAVE_FAILED
        );
    }

    #[test]
    fn settings_write_status_reports_skipped_writes() {
        let success = "ブックマークに追加しました".to_string();
        let writable = Settings::default();
        assert!(!writable.write_was_skipped());
        assert_eq!(
            settings_write_status(&writable, true, true, success.clone()),
            success
        );
        assert_eq!(
            settings_write_status(&writable, false, true, success.clone()),
            STATUS_SETTINGS_SAVE_FAILED
        );

        for skipped in [
            Settings {
                read_only: true,
                ..Settings::default()
            },
            Settings {
                ephemeral: true,
                ..Settings::default()
            },
            Settings {
                read_only: true,
                ephemeral: true,
                ..Settings::default()
            },
        ] {
            assert!(skipped.write_was_skipped());
            assert_eq!(
                settings_write_status(&skipped, true, true, success.clone()),
                STATUS_SETTINGS_READ_ONLY
            );
            // A skipped write must win over a failed one: reporting
            // `STATUS_SETTINGS_SAVE_FAILED` here would wrongly tell a read-only
            // user a file write was attempted and failed (G12).
            assert_eq!(
                settings_write_status(&skipped, false, true, success.clone()),
                STATUS_SETTINGS_READ_ONLY,
                "skipped writes take precedence over saved=false"
            );
        }
    }

    // W08: 読み取り専用の理由が settings.load_error に残っているのに表示が
    // 「設定を読み取り専用で開きました」だけだと、なぜ書けないのか分からない。
    // 起動時（V01）と同じ書式で理由と回復導線を併記する。
    #[test]
    fn settings_write_status_reports_read_only_reason_and_recovery_hint() {
        let reason = "設定ファイルが壊れています";
        let settings = Settings {
            read_only: true,
            load_error: Some(reason.to_string()),
            ..Settings::default()
        };
        assert!(settings.write_was_skipped());
        let status = settings_write_status(
            &settings,
            true,
            true,
            "ブックマークに追加しました".to_string(),
        );
        assert_eq!(
            status,
            format!("{STATUS_SETTINGS_READ_ONLY}: {reason}{STATUS_SETTINGS_READ_ONLY_HINT}")
        );
        assert!(status.contains(reason), "{status}");
        assert!(status.contains(STATUS_SETTINGS_READ_ONLY_HINT), "{status}");
        assert!(!status.contains("ブックマークに追加しました"), "{status}");
        assert_eq!(status_color(&status), C_YELLOW, "{status}");
        // 起動時の通知と書き込み時の通知は同じヘルパーから組み立てる。
        assert_eq!(settings_read_only_status(&settings), status);

        // 理由が空／未設定なら汎用通知に戻し、空の ": " を出さない。
        for load_error in [Some(String::new()), None] {
            let plain = Settings {
                load_error,
                ..settings.clone()
            };
            assert_eq!(settings_read_only_status(&plain), STATUS_SETTINGS_READ_ONLY);
        }
    }

    // I02: callers that never invoked `save()` have nothing to skip. Their own
    // rejection caption must survive even in a read-only / ephemeral session,
    // otherwise the concrete reason ("ラベルは設定されていません", the max-search
    // cap, …) is lost behind the generic read-only notice.
    #[test]
    fn settings_write_status_noop_keeps_caller_rejection_message() {
        let rejection = bookmark_label_set_rejection(std::path::Path::new("/tmp/target"), None);
        let skipped = Settings {
            read_only: true,
            ..Settings::default()
        };
        let ephemeral = Settings {
            ephemeral: true,
            ..Settings::default()
        };
        for settings in [Settings::default(), skipped, ephemeral] {
            // `saved` is `true` on the no-op branches, and `attempted` is false.
            assert_eq!(
                settings_write_status(&settings, true, false, rejection.clone()),
                rejection,
                "a no-op must report its own rejection verbatim"
            );
            // The generic read-only notice must not leak into the no-op path.
            assert_ne!(
                settings_write_status(&settings, true, false, rejection.clone()),
                STATUS_SETTINGS_READ_ONLY
            );
            // The same caption is still masked when a write really was skipped.
            if settings.write_was_skipped() {
                assert_eq!(
                    settings_write_status(&settings, true, true, rejection.clone()),
                    STATUS_SETTINGS_READ_ONLY
                );
            }
        }

        // The concrete saved-search rejection reason must also survive a no-op.
        let cap_reason = saved_search_save_status("仕事", false);
        assert!(
            cap_reason.contains("上限に達したか") && cap_reason.contains("無効"),
            "{cap_reason}"
        );
        assert_eq!(
            settings_write_status(
                &Settings {
                    read_only: true,
                    ..Settings::default()
                },
                true,
                false,
                cap_reason.clone()
            ),
            cap_reason
        );
    }

    // K02: an unbind that removes no override changes nothing, so it must not
    // reach `save()`. Its own "nothing to unbind" caption has to survive even
    // in a read-only session instead of becoming STATUS_SETTINGS_READ_ONLY.
    #[test]
    fn keymap_unbind_noop_keeps_rejection_caption_in_read_only_session() {
        let noop = keymap_unbind_caption(false, "open_preview");
        assert_eq!(noop, "解除する設定はありません: open_preview");
        let removed = keymap_unbind_caption(true, "open_preview");
        assert_eq!(removed, "キーマップを解除しました: open_preview");
        assert_ne!(noop, removed);

        for settings in [
            Settings::default(),
            Settings {
                read_only: true,
                ..Settings::default()
            },
            Settings {
                ephemeral: true,
                ..Settings::default()
            },
        ] {
            // No-op path in `confirm_keymap`: `attempted = false`, `saved = true`.
            assert_eq!(
                settings_write_status(&settings, true, false, noop.clone()),
                noop,
                "no-op unbind must report its own rejection verbatim"
            );
            assert_ne!(
                settings_write_status(&settings, true, false, noop.clone()),
                STATUS_SETTINGS_READ_ONLY
            );
        }

        // The masked behaviour this fixes: an actually attempted unbind in a
        // read-only session *does* show the generic notice, which is why the
        // no-op must skip `save()` entirely.
        let read_only = Settings {
            read_only: true,
            ..Settings::default()
        };
        assert_eq!(
            settings_write_status(
                &read_only,
                true,
                true,
                keymap_unbind_caption(true, "open_preview")
            ),
            STATUS_SETTINGS_READ_ONLY
        );
    }

    // G08: Ctrl+Tab / Ctrl+Shift+Tab step the settings TabBar with wrap-around.
    #[test]
    fn settings_tab_step_wraps_in_both_directions() {
        assert_eq!(
            settings_tab_step(SettingsTab::Appearance, 1),
            SettingsTab::Files
        );
        assert_eq!(
            settings_tab_step(SettingsTab::Personalize, 1),
            SettingsTab::Appearance
        );
        assert_eq!(
            settings_tab_step(SettingsTab::Appearance, -1),
            SettingsTab::Personalize
        );
        assert_eq!(
            settings_tab_step(SettingsTab::Files, -1),
            SettingsTab::Appearance
        );
        for tab in SettingsTab::ALL {
            assert_eq!(settings_tab_step(tab, SettingsTab::ALL.len() as isize), tab);
        }
    }

    // K07: Alt+1..Alt+5 jump directly to a page, so the digit→page map must
    // cover all five displayed tabs and reject every other key.
    #[test]
    fn settings_digit_key_maps_to_displayed_tabs_only() {
        for (index, tab) in SettingsTab::ALL.iter().enumerate() {
            let key = char::from_digit(index as u32 + 1, 10).unwrap().to_string();
            assert_eq!(settings_tab_from_digit_key(&key), Some(*tab));
        }
        assert_eq!(settings_tab_from_digit_key("0"), None);
        assert_eq!(settings_tab_from_digit_key("6"), None);
        assert_eq!(settings_tab_from_digit_key("a"), None);
        assert_eq!(settings_tab_from_digit_key(""), None);
        assert_eq!(settings_tab_from_digit_key("12"), None);
    }

    // K08: RadioGroup is skipped by the Tab focus cycle, so Alt+S must always
    // advance the sort choice and wrap after the last radio option.
    #[test]
    fn settings_sort_next_cycles_and_wraps() {
        assert_eq!(settings_sort_next("name"), "kind");
        assert_eq!(settings_sort_next("kind"), "size");
        assert_eq!(settings_sort_next("size"), "modified");
        assert_eq!(settings_sort_next("modified"), "name");
        assert_eq!(settings_sort_next(""), "name");
        assert_eq!(settings_sort_next("unknown"), "name");
    }

    // N01: `settings_row_shortcut` only inspects the key string, so the host
    // must reject modified chords before calling it. Otherwise Ctrl+T / Alt+T /
    // Shift+T would change the theme and swallow the global shortcut.
    #[test]
    fn settings_row_shortcut_requires_bare_modifiers() {
        assert!(settings_row_key_is_bare(&Modifiers::default()));
        for modified in [
            Modifiers {
                control: true,
                ..Modifiers::default()
            },
            Modifiers {
                alt: true,
                ..Modifiers::default()
            },
            Modifiers {
                shift: true,
                ..Modifiers::default()
            },
            Modifiers {
                platform: true,
                ..Modifiers::default()
            },
            Modifiers {
                function: true,
                ..Modifiers::default()
            },
            Modifiers {
                control: true,
                shift: true,
                ..Modifiers::default()
            },
            Modifiers {
                alt: true,
                platform: true,
                ..Modifiers::default()
            },
        ] {
            assert!(
                !settings_row_key_is_bare(&modified),
                "modified chord must not claim a settings row: {modified:?}"
            );
        }
    }

    // D13: every preview-then-write path must discard its in-memory changes
    // exactly when the disk write fails; a successful (or skipped) write keeps
    // the preview for the current session.
    #[test]
    fn failed_settings_write_triggers_rollback() {
        assert!(settings_write_needs_rollback(false));
        assert!(!settings_write_needs_rollback(true));
    }

    // H06: the pure decision helper above cannot prove that a call site exists.
    // Check the source itself so a newly added or regressed write path cannot
    // silently omit rollback (the `main.rs` include is compiled into the test).
    // The seven preview-then-write paths are: settings overlay Save, terminal
    // command dialog, keymap dialog, toggle_theme, set_sort, toggle_hidden and
    // set_view. Build the needle from fragments so this test's own source does
    // not contribute a match.
    #[test]
    fn rollback_is_called_by_all_seven_preview_then_write_paths() {
        let source = include_str!("main.rs");
        let needle = concat!("self", ".rollback_settings(");
        let call_sites = source.matches(needle).count();
        assert_eq!(
            call_sites, 7,
            "each preview-then-write path must call rollback_settings"
        );
    }

    // H05/H14/H15: set_view used to ignore the write result entirely, so a
    // read-only / ephemeral session showed no notice at all. Exercise the pure
    // helper the handler now uses for both skipped and failed writes.
    #[test]
    fn set_view_status_reports_skipped_and_failed_writes() {
        let success = set_view_status(&Settings::default(), true, ViewLayout::Details);
        assert!(success.contains("詳細リスト"), "{success}");
        assert_eq!(status_color(&success), C_GREEN);
        assert!(
            set_view_status(&Settings::default(), true, ViewLayout::Icons)
                .contains("アイコングリッド")
        );

        for settings in [
            Settings {
                read_only: true,
                ..Settings::default()
            },
            Settings {
                ephemeral: true,
                ..Settings::default()
            },
        ] {
            assert!(settings.write_was_skipped());
            for mode in [ViewLayout::Details, ViewLayout::Icons] {
                assert_eq!(
                    set_view_status(&settings, true, mode),
                    STATUS_SETTINGS_READ_ONLY,
                    "a skipped view write must show the read-only notice"
                );
            }
        }

        let failed = set_view_status(&Settings::default(), false, ViewLayout::Icons);
        assert_eq!(failed, STATUS_SETTINGS_SAVE_FAILED);
        assert_eq!(status_color(&failed), C_RED);
    }

    // F6-49: the personalization wiring uses these pure helpers plus the real
    // Settings API from the panels, so the whole path is covered headlessly.

    #[test]
    fn saved_search_ui_caption_and_status_are_explicit() {
        assert_eq!(
            saved_search_caption("Todos", "TODO name:rs"),
            "Todos: TODO name:rs"
        );
        let ok = saved_search_save_status("Todos", true);
        let rejected = saved_search_save_status("Todos", false);
        assert!(ok.contains("Todos") && ok.contains("保存しました"), "{ok}");
        // 失敗時は上限到達か入力無効の両方があり得ることを正直に案内する（U02）。
        assert!(
            rejected.contains("Todos")
                && rejected.contains("できませんでした")
                && rejected.contains("上限に達したか")
                && rejected.contains("無効です")
                && rejected.contains("削除"),
            "{rejected}"
        );
        assert!(
            !rejected.contains("上限に達しました。"),
            "上限だけを断言してはならない: {rejected}"
        );
    }

    #[test]
    fn trash_undo_status_reports_zero_restore_as_failure() {
        let empty = trash_undo_status("ゴミ箱", 0, 0);
        assert_eq!(
            empty,
            format!("{STATUS_RESTORE_FAILED}（項目が{STATUS_TRASH_MISSING_REASON}）")
        );
        assert!(
            !empty.contains("元に戻しました"),
            "0 件復元を成功と報告しない: {empty}"
        );
        let ok = trash_undo_status("ゴミ箱", 2, 0);
        assert!(ok.contains("ゴミ箱") && ok.contains("2 件"), "{ok}");
        assert!(ok.contains("元に戻しました"), "{ok}");
        let partial = trash_undo_status("ゴミ箱", 1, 1);
        assert!(
            partial.contains("1 件")
                && partial.contains(&format!("1 件は{STATUS_TRASH_MISSING_REASON}")),
            "{partial}"
        );
        // T08: both failure branches share one reason fragment from strings.rs.
        assert!(empty.contains(STATUS_TRASH_MISSING_REASON), "{empty}");
        assert!(partial.contains(STATUS_TRASH_MISSING_REASON), "{partial}");
        // M01: 一部の復元失敗は成功 green ではなく error(red) に分類される。
        assert!(partial.contains(STATUS_PARTIAL_FAILURE_SUFFIX), "{partial}");
        assert_eq!(status_color(&partial), C_RED, "partial restore: {partial}");
        // 全件成功は従来どおり green のまま。
        assert_eq!(status_color(&ok), C_GREEN, "full restore: {ok}");
    }

    #[test]
    fn trash_redo_cancelled_status_reports_partial_move() {
        // 何も移動していない純粋なキャンセルはキャンセル文言のみ。
        let none = trash_redo_cancelled_status(0);
        assert_eq!(none, STATUS_CANCELLED);
        assert!(!none.contains("件"), "0 件を報告しない: {none}");
        // キャンセル前に移動済みの項目数を報告し、部分的な結果を隠さない。
        let partial = trash_redo_cancelled_status(3);
        assert_eq!(
            partial,
            format!("{STATUS_CANCELLED}（3 件をゴミ箱へ移動済み）")
        );
        assert!(partial.contains("3 件"), "{partial}");
    }

    #[test]
    fn saved_search_wiring_roundtrips_through_settings() {
        use crate::settings::Settings;

        let mut settings = Settings::default();
        assert!(settings.save_search("Todos", "TODO"));
        let found = settings
            .saved_search("todos")
            .expect("lookup is case-insensitive");
        assert_eq!(found.query, "TODO");
        assert_eq!(
            saved_search_caption(&found.name, &found.query),
            "Todos: TODO"
        );
        // Saving the same name again updates the entry instead of duplicating it.
        assert!(settings.save_search("TODOS", "name:rs"));
        assert_eq!(settings.saved_searches.len(), 1);
        assert_eq!(settings.saved_search("todos").unwrap().query, "name:rs");
        assert!(settings.remove_search("todos"));
        assert!(settings.saved_search("Todos").is_none());
        assert!(!settings.remove_search("Todos"));
    }

    #[test]
    fn bookmark_label_wiring_reaches_row_names() {
        use crate::settings::Settings;
        use std::path::Path;

        let mut settings = Settings::default();
        let path = Path::new("/work/project");
        assert_eq!(
            labeled_display_name("project", settings.bookmark_label(path)),
            "project"
        );
        assert!(settings.set_bookmark_label(path, " work "));
        assert_eq!(settings.bookmark_label(path), Some("work"));
        // Sidebar rows use the tab label as their base; list rows use the
        // entry/relative-search name. Both keep the original text visible.
        assert_eq!(
            labeled_display_name(&tab_label(path), settings.bookmark_label(path)),
            "[work] project"
        );
        assert_eq!(
            labeled_display_name("src/main.rs", settings.bookmark_label(path)),
            "[work] src/main.rs"
        );
        // A blank label clears the value, so rows fall back to their base name.
        assert!(settings.set_bookmark_label(path, "  "));
        assert!(settings.bookmark_label(path).is_none());
        assert_eq!(
            labeled_display_name("project", settings.bookmark_label(path)),
            "project"
        );
        assert!(settings.set_bookmark_label(path, "work"));
        assert!(settings.remove_bookmark_label(path));
        assert!(!settings.remove_bookmark_label(path));
    }

    #[test]
    fn bookmark_label_is_settable_matches_setter_contract() {
        use crate::model::{BookmarkLabel, MAX_BOOKMARK_LABELS, MAX_BOOKMARK_LABEL_BYTES};
        use std::path::Path;

        let mut settings = Settings::default();
        let path = Path::new("/work/project");
        assert!(bookmark_label_is_settable(&settings, path, " work "));
        assert!(!bookmark_label_is_settable(&settings, path, ""));
        assert!(!bookmark_label_is_settable(&settings, path, "   "));
        let too_long = "x".repeat(MAX_BOOKMARK_LABEL_BYTES + 1);
        assert!(!bookmark_label_is_settable(&settings, path, &too_long));
        assert!(!bookmark_label_is_settable(&settings, path, "a\0b"));
        assert!(!bookmark_label_is_settable(
            &settings,
            Path::new(""),
            "work"
        ));

        // `sane_path` rejects these paths even though `BookmarkLabel::new`
        // accepts them. Pair every pre-flight assertion with the real setter
        // so the two contracts cannot drift apart again (E09).
        let mut setter = Settings::default();
        let nul_path = PathBuf::from("/work/pro\0ject");
        assert!(!bookmark_label_is_settable(&settings, &nul_path, "work"));
        assert!(!setter.set_bookmark_label(&nul_path, "work"));
        let too_long_path = PathBuf::from(format!(
            "/{}",
            "x".repeat(crate::settings::MAX_PATH_BYTES + 1)
        ));
        assert!(!bookmark_label_is_settable(
            &settings,
            &too_long_path,
            "work"
        ));
        assert!(!setter.set_bookmark_label(&too_long_path, "work"));
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let non_utf8 = PathBuf::from(OsString::from_vec(vec![b'/', 0xff]));
            assert!(!bookmark_label_is_settable(&settings, &non_utf8, "work"));
            assert!(!setter.set_bookmark_label(&non_utf8, "work"));
        }

        // Fill to the cap: a brand-new entry is refused, while the existing
        // entry for `path` is still replaced in place by the real setter.
        assert!(settings.set_bookmark_label(path, "seed"));
        for index in 0..MAX_BOOKMARK_LABELS - 1 {
            settings.bookmark_labels.push(BookmarkLabel {
                path: PathBuf::from(format!("/other/{index}")),
                label: format!("label {index}"),
            });
        }
        assert_eq!(settings.bookmark_labels.len(), MAX_BOOKMARK_LABELS);
        assert!(bookmark_label_is_settable(&settings, path, "new"));
        assert!(settings.set_bookmark_label(path, "new"));
        let fresh = Path::new("/fresh");
        assert!(!bookmark_label_is_settable(&settings, fresh, "work"));
        assert!(!settings.set_bookmark_label(fresh, "work"));
    }

    #[test]
    fn bookmark_label_is_settable_accepts_exact_byte_limits() {
        // Both guards reject only lengths *greater* than the cap, so a value
        // sitting exactly on each limit must be accepted by the pre-flight and
        // by the real setter (E09: keep the two contracts in lockstep).
        let mut settings = Settings::default();

        // One '/' plus (MAX_PATH_BYTES - 1) ASCII bytes hits the path cap.
        let path_cap = crate::settings::MAX_PATH_BYTES;
        let boundary_path = PathBuf::from(format!("/{}", "x".repeat(path_cap - 1)));
        assert_eq!(
            boundary_path.as_os_str().as_encoded_bytes().len(),
            path_cap,
            "test fixture must sit exactly on the path byte limit"
        );

        let label_cap = crate::model::MAX_BOOKMARK_LABEL_BYTES;
        let boundary_label = "L".repeat(label_cap);
        assert_eq!(
            boundary_label.len(),
            label_cap,
            "test fixture must sit exactly on the label byte limit"
        );

        assert!(bookmark_label_is_settable(
            &settings,
            &boundary_path,
            &boundary_label
        ));
        assert!(settings.set_bookmark_label(&boundary_path, &boundary_label));
        assert_eq!(
            settings.bookmark_label(&boundary_path),
            Some(boundary_label.as_str())
        );
    }

    #[test]
    fn bookmark_label_helper_ignores_blank_values() {
        assert_eq!(labeled_display_name("item", None), "item");
        assert_eq!(labeled_display_name("item", Some("  ")), "item");
        assert_eq!(labeled_display_name("item", Some(" home ")), "[home] item");
    }

    // ------------------------------------------------------------------ F6-50
    // The terminal launcher's pure pieces are GPUI-free: the target directory,
    // the child-process argv/cwd contract and the status text are testable
    // headlessly. Spawning is deliberately not faked here; the Command builder
    // assertions below prove the path is never embedded in argv.

    #[test]
    fn terminal_dir_for_uses_selected_dir_or_file_parent() {
        use std::path::Path;
        let cwd = Path::new("/work/project");
        assert_eq!(
            terminal_dir_for(Path::new("/work/project/docs"), true, cwd),
            PathBuf::from("/work/project/docs")
        );
        assert_eq!(
            terminal_dir_for(Path::new("/work/project/src/main.rs"), false, cwd),
            PathBuf::from("/work/project/src")
        );
        // A root path has no parent, so fall back to the listing's cwd.
        assert_eq!(
            terminal_dir_for(Path::new("/"), false, cwd),
            PathBuf::from("/work/project")
        );
    }

    #[test]
    fn terminal_target_dir_prefers_focused_row_then_cwd() {
        let cwd = PathBuf::from("/work/project");
        let base = Entry {
            name: OsString::from("item"),
            name_text: "item".to_string(),
            sort_key: "item".to_string(),
            path: PathBuf::from("/work/project/item"),
            is_dir: false,
            is_symlink: false,
            size: 0,
            modified: None,
        };
        assert_eq!(terminal_target_dir(None, &cwd), cwd);
        assert_eq!(
            terminal_target_dir(Some(&base), &cwd),
            PathBuf::from("/work/project")
        );
        let directory = Entry {
            is_dir: true,
            ..base
        };
        assert_eq!(
            terminal_target_dir(Some(&directory), &cwd),
            PathBuf::from("/work/project/item")
        );
    }

    #[test]
    fn version_label_matches_cargo_version() {
        let label = version_label();
        assert!(label.starts_with('v'));
        assert_eq!(&label[1..], env!("CARGO_PKG_VERSION"));
    }

    #[test]
    fn middle_click_tab_dir_uses_directory_row_or_cwd() {
        let cwd = PathBuf::from("/work/project");
        let file = Entry {
            name: OsString::from("item"),
            name_text: "item".to_string(),
            sort_key: "item".to_string(),
            path: PathBuf::from("/work/project/item"),
            is_dir: false,
            is_symlink: false,
            size: 0,
            modified: None,
        };
        // A file row keeps the listing's directory (not the file's parent).
        assert_eq!(middle_click_tab_dir(&file, &cwd), cwd);
        let directory = Entry {
            is_dir: true,
            ..file
        };
        assert_eq!(
            middle_click_tab_dir(&directory, &cwd),
            PathBuf::from("/work/project/item")
        );
    }

    #[test]
    fn terminal_process_passes_path_only_as_current_dir() {
        use std::ffi::OsStr;
        // A hostile-looking directory must never be spliced into argv.
        let dir = PathBuf::from("/work/project/x'; touch /tmp/pwned #");
        let argv = vec![
            "konsole".to_string(),
            "--title".to_string(),
            "build".to_string(),
        ];
        let command = terminal_process(&argv, &dir).expect("non-empty argv builds a Command");
        assert_eq!(command.get_program(), "konsole");
        assert_eq!(
            command.get_args().collect::<Vec<_>>(),
            vec![OsStr::new("--title"), OsStr::new("build")]
        );
        assert_eq!(command.get_current_dir(), Some(dir.as_path()));
        let args: String = command
            .get_args()
            .map(|arg| arg.to_string_lossy())
            .collect::<Vec<_>>()
            .join(" ");
        assert!(!args.contains("touch"), "path leaked into argv: {args}");
        // Empty argv is rejected instead of panicking on argv[0].
        assert!(terminal_process(&[], &dir).is_none());
    }

    #[test]
    fn terminal_statuses_report_reason_and_target() {
        let argv = vec!["konsole".to_string(), "--title".to_string()];
        let dir = PathBuf::from("/work/project");
        let disabled = terminal_disabled_status();
        assert!(disabled.contains("未設定"), "{disabled}");
        assert!(disabled.contains("terminal_command"), "{disabled}");
        let started = terminal_started_status(&argv, &dir);
        assert!(started.contains("konsole --title"), "{started}");
        assert!(started.contains("/work/project"), "{started}");
        let error = std::io::Error::new(std::io::ErrorKind::NotFound, "no emulator");
        let failed = terminal_failed_status(&error, &dir);
        assert!(failed.contains("no emulator"), "{failed}");
        assert!(failed.contains("/work/project"), "{failed}");
        // R03: 文言は strings.rs の定数から組み立てられ、赤に分類される。
        assert_eq!(
            failed,
            format!(
                "{STATUS_TERMINAL_START_PREFIX}{STATUS_OP_FAILED_SUFFIX}: {error}（/work/project）"
            ),
            "{failed}"
        );
        assert_eq!(
            failed,
            format!("ターミナルを起動できません: {error}（/work/project）")
        );
        assert_eq!(status_color(&failed), C_RED, "{failed}");
    }

    #[test]
    fn saved_search_cap_rejection_keeps_wording_and_is_red() {
        // R04: 上限拒否は strings.rs の接頭辞 + 理由から組み立てられ、赤に分類される。
        let rejected = saved_search_save_status("仕事", false);
        assert_eq!(
            rejected,
            "検索を保存できませんでした: 仕事（保存できる検索の上限に達したか、名前または検索語が無効です。既存の保存検索を削除するか、入力を見直してください）",
            "{rejected}"
        );
        assert_eq!(status_color(&rejected), C_RED, "{rejected}");
    }

    #[test]
    fn bookmark_label_rejection_keeps_wording_and_is_red() {
        // R05: ラベル設定拒否は strings.rs の接頭辞から組み立てられ、赤に分類される。
        let rejection = bookmark_label_set_rejection(std::path::Path::new("/tmp/target"), None);
        assert_eq!(rejection, "ラベルを設定できませんでした: /tmp/target");
        assert_eq!(status_color(&rejection), C_RED, "{rejection}");
    }

    #[test]
    fn bookmark_label_not_set_rejection_keeps_wording_and_is_red() {
        // U05 / X03: 未設定拒否は strings.rs の共通接頭辞から組み立てられ、
        // 緑に落ちない。期待値も同じ定数を使い、文言の乖離を検出できるようにする。
        let status = bookmark_label_not_set_status(std::path::Path::new("/tmp/target"));
        assert_eq!(
            status,
            format!("{STATUS_ERROR_PREFIX}{STATUS_BOOKMARK_LABEL_NOT_SET_PREFIX}/tmp/target")
        );
        assert!(status.contains(STATUS_ERROR_PREFIX), "{status}");
        assert_eq!(status_color(&status), C_RED, "{status}");
    }

    #[test]
    fn bookmark_label_rejection_reports_recoverable_reason() {
        // U04: 拒否理由を添えて、ユーザーが対処できるようにする。
        use crate::model::{BookmarkLabel, MAX_BOOKMARK_LABELS, MAX_BOOKMARK_LABEL_BYTES};

        let settings = Settings::default();
        let path = std::path::Path::new("/tmp/target");
        assert_eq!(
            bookmark_label_rejection_reason(&settings, path, "work"),
            None
        );
        assert_eq!(
            bookmark_label_set_rejection(path, None),
            "ラベルを設定できませんでした: /tmp/target"
        );
        assert_eq!(
            bookmark_label_rejection_reason(&settings, std::path::Path::new(""), "work"),
            Some(STATUS_BOOKMARK_LABEL_REASON_INVALID_PATH)
        );
        let too_long = "x".repeat(MAX_BOOKMARK_LABEL_BYTES + 1);
        assert_eq!(
            bookmark_label_rejection_reason(&settings, path, &too_long),
            Some(STATUS_BOOKMARK_LABEL_REASON_INVALID_LABEL)
        );

        // 上限超過の新規ラベルだけが LIMIT。既存ラベルの置換は許可されたまま。
        let mut full = Settings::default();
        assert!(full.set_bookmark_label(path, "seed"));
        for index in 0..MAX_BOOKMARK_LABELS - 1 {
            full.bookmark_labels.push(BookmarkLabel {
                path: PathBuf::from(format!("/other/{index}")),
                label: format!("label {index}"),
            });
        }
        assert_eq!(
            bookmark_label_rejection_reason(&full, std::path::Path::new("/fresh"), "work"),
            Some(STATUS_BOOKMARK_LABEL_REASON_LIMIT)
        );
        assert_eq!(bookmark_label_rejection_reason(&full, path, "new"), None);
        let rejection =
            bookmark_label_set_rejection(path, Some(STATUS_BOOKMARK_LABEL_REASON_LIMIT));
        assert_eq!(
            rejection,
            "ラベルを設定できませんでした: /tmp/target（ラベルの保存件数が上限に達しました。不要なラベルを削除してから再試行してください）"
        );
        assert_eq!(status_color(&rejection), C_RED, "{rejection}");
    }

    #[test]
    fn trash_load_error_is_prefixed_once_and_red() {
        // U06 / X04: ゴミ箱読み込み失敗は strings.rs の接頭辞付きで組み立てられ、
        // ステータスバー描画側での二重付与も起きない。
        let error = trash_load_error_status("permission denied");
        assert_eq!(
            error,
            format!("{STATUS_ERROR_PREFIX}{STATUS_TRASH_LOAD_FAILED_PREFIX}permission denied")
        );
        assert_eq!(status_color(&error), C_RED, "{error}");

        let plain = "読み込み失敗";
        assert_eq!(
            load_error_display_text(plain),
            format!("{STATUS_ERROR_PREFIX}{plain}")
        );
        assert_eq!(load_error_display_text(&error), error);
        let doubled = format!("{STATUS_ERROR_PREFIX}{STATUS_ERROR_PREFIX}");
        assert!(!load_error_display_text(&error).starts_with(&doubled));
    }

    #[test]
    fn undo_missing_status_keeps_wording_and_is_red() {
        // U07 / X08: 接尾辞は strings.rs の共有定数経由で組み立てられ、赤に分類される。
        assert!(STATUS_UNDO_MISSING_SUFFIX.contains("存在しません"));
        let status = undo_missing_status(std::path::Path::new("/tmp/current"));
        assert_eq!(
            status,
            format!("{STATUS_UNDO_FAILED}: /tmp/current{STATUS_UNDO_MISSING_SUFFIX}")
        );
        assert!(status.contains(STATUS_UNDO_FAILED), "{status}");
        assert_eq!(status_color(&status), C_RED, "{status}");
    }

    #[test]
    fn transfer_progress_status_shares_the_cancel_hint() {
        // U08: 転送進捗のキャンセル案内は STATUS_CANCEL_HINT を共有する。
        let status = transfer_progress_status("コピー", 1024, 2048);
        assert_eq!(status, "コピー中… 1.0 K / 2.0 K （Esc でキャンセル）");
        assert!(status.ends_with(STATUS_CANCEL_HINT), "{status}");
        assert_eq!(STATUS_CANCEL_HINT, "（Esc でキャンセル）");
    }

    #[test]
    fn undo_action_detail_covers_all_four_branches() {
        // U09: Rename（file_name あり／なし）と Trash（単件／複数件）を網羅する。
        let rename = UndoAction::Rename {
            from: PathBuf::from("/tmp/old/old.txt"),
            to: PathBuf::from("/tmp/new/new.txt"),
        };
        assert_eq!(Explorer::undo_action_detail(&rename), "old.txt → new.txt");

        // file_name() が無い側は display_path() にフォールバックする。
        let rename_from_root = UndoAction::Rename {
            from: PathBuf::from("/"),
            to: PathBuf::from("/tmp/new.txt"),
        };
        assert_eq!(
            Explorer::undo_action_detail(&rename_from_root),
            "/ → new.txt"
        );

        let rename_to_root = UndoAction::Rename {
            from: PathBuf::from("/tmp/old.txt"),
            to: PathBuf::from("/"),
        };
        assert_eq!(Explorer::undo_action_detail(&rename_to_root), "old.txt → /");

        let trash_single = UndoAction::Trash {
            items: vec![(PathBuf::from("/tmp/a.txt"), OsString::from("id-a"))],
        };
        assert_eq!(Explorer::undo_action_detail(&trash_single), "/tmp/a.txt");

        let trash_multiple = UndoAction::Trash {
            items: vec![
                (PathBuf::from("/tmp/a.txt"), OsString::from("id-a")),
                (PathBuf::from("/tmp/b.txt"), OsString::from("id-b")),
            ],
        };
        assert_eq!(Explorer::undo_action_detail(&trash_multiple), "2 件");
    }

    #[test]
    fn f4_terminal_binding_leaves_f6_51_bindings_intact() {
        let keymap = KeyMap::default_keymap();
        assert_eq!(
            keymap.resolve("f4", false, false, false),
            KeyAction::OpenTerminal
        );
        assert_eq!(keymap.resolve("f4", true, false, false), KeyAction::None);
        // The shortcuts shipped by F6-51 keep their exact keystrokes.
        assert_eq!(
            keymap.resolve("p", true, false, false),
            KeyAction::FuzzyJump
        );
        assert_eq!(keymap.resolve("p", true, true, false), KeyAction::None);
        assert_eq!(
            keymap.resolve("f3", false, false, false),
            KeyAction::OpenPreview
        );
        assert_eq!(keymap.resolve("f5", false, false, false), KeyAction::Reload);
    }

    #[test]
    fn ctrl_alt_shortcuts_still_match_ctrl_bindings() {
        // Finding 5: before the keymap migration `on_key_down` checked
        // `modifiers.control` first and ignored `alt`, so Ctrl+Alt+H fired
        // the same action as Ctrl+H. Test the pure matcher directly.
        let ctrl_h = KeyBinding {
            key: "h".to_string(),
            ctrl: true,
            alt: false,
            shift: None,
            action: KeyAction::ToggleHidden,
        };
        assert!(KeyMap::binding_matches(&ctrl_h, "h", true, false, None));
        assert!(KeyMap::binding_matches(&ctrl_h, "h", true, true, None));
        assert!(!KeyMap::binding_matches(&ctrl_h, "h", false, false, None));

        // Alt navigation has `ctrl == false`, so its `alt` stays significant.
        let alt_left = KeyBinding {
            key: "left".to_string(),
            ctrl: false,
            alt: true,
            shift: None,
            action: KeyAction::GoBack,
        };
        assert!(KeyMap::binding_matches(
            &alt_left, "left", false, true, None
        ));
        assert!(!KeyMap::binding_matches(
            &alt_left, "left", false, false, None
        ));
        assert!(!KeyMap::binding_matches(
            &alt_left, "left", true, true, None
        ));

        // The default table resolves Ctrl+Alt shortcuts to their Ctrl action,
        // while Alt alone on a plain key remains a no-op as before.
        let keymap = KeyMap::default_keymap();
        assert_eq!(
            keymap.resolve("h", true, false, true),
            KeyAction::ToggleHidden
        );
        assert_eq!(keymap.resolve("q", true, true, true), KeyAction::Quit);
        assert_eq!(keymap.resolve("h", false, false, true), KeyAction::None);
    }

    // F6-51: the keymap table and the fuzzy ranking are GPUI-free, so the
    // regression surface (including every pre-existing shortcut) is testable
    // headlessly.

    #[test]
    fn default_keymap_preserves_every_existing_shortcut() {
        let keymap = KeyMap::default_keymap();
        use KeyAction::*;

        // Dialog-ownership keys stay outside the table so no rebind can take
        // away the ability to close a dialog or cancel an operation.
        assert_eq!(
            keymap.resolve("escape", false, false, false),
            KeyAction::None
        );
        assert_eq!(keymap.resolve("f1", false, false, false), KeyAction::None);

        // Plain keys: Space / F3 / Esc / F1 / F2 / F5 are the regression
        // critical ones named in F6-51.
        assert_eq!(keymap.resolve("space", false, false, false), OpenPreview);
        assert_eq!(keymap.resolve("f3", false, false, false), OpenPreview);
        assert_eq!(keymap.resolve("f2", false, false, false), StartRename);
        assert_eq!(keymap.resolve("f5", false, false, false), Reload);
        assert_eq!(keymap.resolve("enter", false, false, false), OpenSelected);
        assert_eq!(keymap.resolve("backspace", false, false, false), GoParent);
        assert_eq!(keymap.resolve("delete", false, false, false), TrashSelected);
        assert_eq!(keymap.resolve("r", false, false, false), RestoreSelected);
        assert_eq!(keymap.resolve("up", false, true, false), MoveUp);
        assert_eq!(keymap.resolve("pageup", false, false, false), PageUp);
        assert_eq!(keymap.resolve("home", false, false, false), SelectFirst);
        assert_eq!(keymap.resolve("end", false, false, false), SelectLast);

        // Ctrl keys, including every shift-sensitive pair and the wildcards.
        assert_eq!(keymap.resolve("h", true, false, false), ToggleHidden);
        assert_eq!(keymap.resolve("h", true, true, false), ToggleHidden);
        assert_eq!(keymap.resolve("a", true, false, false), SelectAll);
        assert_eq!(keymap.resolve("d", true, false, false), ToggleBookmark);
        assert_eq!(keymap.resolve("f", true, false, false), OpenSearch);
        assert_eq!(keymap.resolve("f", true, true, false), FocusSearchName);
        assert_eq!(keymap.resolve("i", true, false, false), OpenProperties);
        assert_eq!(keymap.resolve("l", true, false, false), OpenLocation);
        assert_eq!(keymap.resolve("z", true, false, false), Undo);
        assert_eq!(keymap.resolve("z", true, true, false), Redo);
        assert_eq!(keymap.resolve("u", true, true, false), ToggleUndoHistory);
        assert_eq!(keymap.resolve("t", true, false, false), NewTab);
        assert_eq!(keymap.resolve("t", true, true, false), RestoreClosedTab);
        assert_eq!(keymap.resolve("w", true, false, false), CloseTab);
        assert_eq!(keymap.resolve("tab", true, false, false), NextTab);
        assert_eq!(keymap.resolve("tab", true, true, false), PrevTab);
        assert_eq!(keymap.resolve("pageup", true, true, false), MoveTabLeft);
        assert_eq!(keymap.resolve("pagedown", true, true, false), MoveTabRight);
        assert_eq!(keymap.resolve("1", true, false, false), DetailsView);
        assert_eq!(keymap.resolve("2", true, false, false), IconsView);
        assert_eq!(keymap.resolve("r", true, false, false), Reload);
        assert_eq!(keymap.resolve("n", true, true, false), NewFolder);
        assert_eq!(keymap.resolve("c", true, false, false), Copy);
        assert_eq!(keymap.resolve("x", true, false, false), Cut);
        assert_eq!(keymap.resolve("v", true, false, false), Paste);
        assert_eq!(keymap.resolve("enter", true, false, false), RestoreSelected);
        assert_eq!(keymap.resolve("q", true, true, false), Quit);

        // Alt navigation.
        assert_eq!(keymap.resolve("left", false, false, true), GoBack);
        assert_eq!(keymap.resolve("right", false, false, true), GoForward);
        assert_eq!(keymap.resolve("up", false, false, true), GoParent);

        // The new binding and unknown keys.
        assert_eq!(keymap.resolve("p", true, false, false), FuzzyJump);
        assert_eq!(keymap.resolve("p", true, true, false), KeyAction::None);
        assert_eq!(keymap.resolve("x", false, false, false), KeyAction::None);
    }

    #[test]
    fn ctrl_comma_resolves_to_open_settings() {
        // `STATUS_SETTINGS_DISCARDED` tells the user to reopen settings with
        // Ctrl+,; the shipped keymap must actually bind that chord (J06).
        let keymap = KeyMap::default_keymap();
        assert_eq!(
            keymap.resolve(",", true, false, false),
            KeyAction::OpenSettings
        );
        // The hint promises the unshifted chord, and a bare comma stays a
        // no-op so ordinary text input is not hijacked.
        assert_eq!(keymap.resolve(",", true, true, false), KeyAction::None);
        assert_eq!(keymap.resolve(",", false, false, false), KeyAction::None);
    }

    #[test]
    fn rebind_and_from_settings_cannot_move_open_settings() {
        // L01: a live rebind must not evict the shipped Ctrl+, binding that the
        // static F1 help and STATUS_SETTINGS_DISCARDED recovery hint advertise.
        let mut keymap = KeyMap::default_keymap();
        assert!(!keymap.rebind(KeyAction::OpenSettings, "o", true, false, false));
        assert!(!keymap.rebind(KeyAction::OpenSettings, "o", true, true, false));
        assert_eq!(
            keymap
                .bindings
                .iter()
                .filter(|b| b.action == KeyAction::OpenSettings)
                .count(),
            1
        );
        assert_eq!(
            keymap.resolve(",", true, false, false),
            KeyAction::OpenSettings
        );
        assert_eq!(keymap.resolve("o", true, false, false), KeyAction::None);

        // Defense in depth: a persisted override injected without going through
        // Settings::validate is ignored by from_settings for the same reason.
        let settings = Settings {
            keymap: vec![("open_settings".to_string(), "ctrl+o".to_string())],
            ..Settings::default()
        };
        let restored = KeyMap::from_settings(&settings);
        assert_eq!(
            restored.resolve(",", true, false, false),
            KeyAction::OpenSettings
        );
        assert_eq!(restored.resolve("o", true, false, false), KeyAction::None);
    }

    #[test]
    fn default_keymap_has_no_ambiguous_duplicate_keystrokes() {
        let keymap = KeyMap::default_keymap();
        let mut seen = std::collections::HashSet::new();
        for binding in &keymap.bindings {
            let spec = (
                binding.key.as_str(),
                binding.ctrl,
                binding.alt,
                binding.shift,
            );
            assert!(
                seen.insert(spec),
                "duplicate keystroke in default keymap: {spec:?}"
            );
        }
    }

    #[test]
    fn keyboard_accessibility_actions_resolve_without_collisions() {
        let keymap = KeyMap::default_keymap();

        // U-07 / U-08 / context-menu copy-path keyboard paths.
        assert_eq!(
            keymap.resolve("l", true, true, false),
            KeyAction::FocusSidebar
        );
        assert_eq!(
            keymap.resolve("space", true, false, false),
            KeyAction::ToggleSelection
        );
        assert_eq!(keymap.resolve("c", true, true, false), KeyAction::CopyPath);

        // The original shifted-off bindings keep their meaning.
        assert_eq!(
            keymap.resolve("l", true, false, false),
            KeyAction::OpenLocation
        );
        assert_eq!(
            keymap.resolve("space", false, false, false),
            KeyAction::OpenPreview
        );
        assert_eq!(keymap.resolve("c", true, false, false), KeyAction::Copy);

        // Ctrl bindings treat alt as a don't-care, so the new chords also
        // resolve with alt held; they must not steal a plain-Ctrl shortcut.
        assert_eq!(keymap.resolve("c", true, true, true), KeyAction::CopyPath);
        assert_eq!(keymap.resolve("c", true, false, true), KeyAction::Copy);
    }

    #[test]
    fn keymap_rebind_is_personal_and_reversible() {
        let mut keymap = KeyMap::default_keymap();
        assert!(keymap.rebind(KeyAction::FuzzyJump, "p", true, true, false));
        assert_eq!(keymap.resolve("p", true, true, false), KeyAction::FuzzyJump);
        assert_eq!(keymap.resolve("p", true, false, false), KeyAction::None);

        // Rebinding onto an occupied keystroke evicts the old owner so a
        // keystroke never resolves to two actions.
        assert!(keymap.rebind(KeyAction::FuzzyJump, "l", true, false, false));
        assert_eq!(
            keymap.resolve("l", true, false, false),
            KeyAction::FuzzyJump
        );
        assert_eq!(keymap.resolve("p", true, true, false), KeyAction::None);

        // Reserved dialog keys, the Ctrl+, settings shortcut (K01) and unmapped
        // actions are rejected. A rebind must not evict OpenSettings: the
        // STATUS_SETTINGS_DISCARDED recovery hint depends on it.
        assert!(!keymap.rebind(KeyAction::FuzzyJump, "escape", false, false, false));
        assert!(!keymap.rebind(KeyAction::FuzzyJump, ",", true, false, false));
        assert_eq!(
            keymap.resolve(",", true, false, false),
            KeyAction::OpenSettings,
            "Ctrl+, must survive a rejected rebind"
        );
        assert!(!keymap.rebind(KeyAction::None, "p", true, false, false));

        keymap.reset(KeyAction::FuzzyJump);
        assert_eq!(
            keymap.resolve("p", true, false, false),
            KeyAction::FuzzyJump
        );
        // A displaced binding stays unbound until that action is reset too.
        assert_eq!(keymap.resolve("l", true, false, false), KeyAction::None);
        keymap.reset(KeyAction::OpenLocation);
        assert_eq!(
            keymap.resolve("l", true, false, false),
            KeyAction::OpenLocation
        );
    }

    #[test]
    fn parse_chord_accepts_modifiers_in_any_order_and_rejects_garbage() {
        assert_eq!(
            parse_chord("ctrl+shift+p"),
            Some(("p".to_string(), true, Some(true), false))
        );
        assert_eq!(
            parse_chord("SHIFT+ALT+ctrl+PageUp"),
            Some(("pageup".to_string(), true, Some(true), true))
        );
        assert_eq!(
            parse_chord("f4"),
            Some(("f4".to_string(), false, None, false))
        );
        assert_eq!(
            parse_chord(" alt + t "),
            Some(("t".to_string(), false, None, true))
        );
        // L02: internal whitespace is trimmed per token, so the parser sees the
        // same key as the reserved-chord validator must.
        assert_eq!(
            parse_chord("ctrl + shift + p"),
            Some(("p".to_string(), true, Some(true), false))
        );
        assert_eq!(
            parse_chord(" ctrl + , "),
            Some((",".to_string(), true, None, false))
        );
        assert!(keymap_chord_reserved("ctrl + ,"));
        // A missing key, an empty token, or a modifier after the key is invalid.
        for bad in ["", "ctrl+", "+ctrl+p", "ctrl++p", "p+ctrl", "ctrl+shift+"] {
            assert_eq!(parse_chord(bad), None, "accepted {bad:?}");
        }
    }

    #[test]
    fn semicolon_is_rejected_so_keymap_entries_round_trip() {
        // Overrides are persisted as `action=chord;` and split on ';' when
        // loaded. A chord containing ';' would be written as `action=ctrl+;;`
        // and could never be parsed back, so both layers must reject it.
        for chord in [";", "ctrl+;", ";+p", "ctrl+;+p"] {
            assert_eq!(parse_chord(chord), None, "accepted chord {chord:?}");
            let input = format!("open_preview {chord}");
            assert_eq!(
                parse_keymap_input(&input),
                None,
                "accepted keymap input {input:?}"
            );
        }
        // A stray separator anywhere else in the command is invalid too.
        assert_eq!(parse_keymap_input("unbind open_preview;"), None);
        // Normal chords remain accepted.
        assert_eq!(
            parse_chord("ctrl+p"),
            Some(("p".to_string(), true, None, false))
        );
        assert!(parse_keymap_input("open_preview ctrl+p").is_some());
    }

    #[test]
    fn keymap_from_settings_applies_overrides_through_rebind() {
        let mut settings = Settings::default();
        settings.keymap.extend([
            ("open_preview".to_string(), "ctrl+shift+p".to_string()),
            ("fuzzy_jump".to_string(), "ctrl+shift+z".to_string()),
        ]);
        let keymap = KeyMap::from_settings(&settings);

        // A `snake_case` action name selects the matching enum variant, and a
        // modifier prefix in any order produces the expected keystroke.
        assert_eq!(
            keymap.resolve("p", true, true, false),
            KeyAction::OpenPreview
        );
        assert_eq!(keymap.resolve("z", true, true, false), KeyAction::FuzzyJump);
        // `rebind` moves the action: all of its old chords are released and an
        // existing owner of the new chord (Ctrl+Shift+Z / Redo) is evicted.
        assert_eq!(
            keymap.resolve("space", false, false, false),
            KeyAction::None
        );
        assert_eq!(keymap.resolve("f3", false, false, false), KeyAction::None);
        assert_eq!(keymap.resolve("p", true, false, false), KeyAction::None);
        assert_eq!(keymap.resolve("z", true, false, false), KeyAction::Undo);
        // Unrelated shortcuts stay untouched.
        assert_eq!(
            keymap.resolve("h", true, false, false),
            KeyAction::ToggleHidden
        );
    }

    #[test]
    fn keymap_from_settings_ignores_unknown_actions_and_bad_chords() {
        let mut settings = Settings::default();
        settings.keymap.extend([
            ("not_an_action".to_string(), "ctrl+z".to_string()),
            ("open_preview".to_string(), "ctrl+".to_string()),
            ("reload".to_string(), "escape".to_string()),
            ("quit".to_string(), "f1".to_string()),
        ]);
        let keymap = KeyMap::from_settings(&settings);
        assert_eq!(keymap.resolve("z", true, false, false), KeyAction::Undo);
        assert_eq!(
            keymap.resolve("space", false, false, false),
            KeyAction::OpenPreview
        );
        assert_eq!(keymap.resolve("f5", false, false, false), KeyAction::Reload);
        assert_eq!(keymap.resolve("q", true, false, false), KeyAction::Quit);
    }

    #[test]
    fn parse_keymap_input_accepts_action_and_chord() {
        let bound = |action: &str, chord: &str| {
            Some(KeymapInput::Bind {
                action: action.to_string(),
                chord: chord.to_string(),
            })
        };
        assert_eq!(
            parse_keymap_input("open_preview ctrl+shift+p"),
            bound("open_preview", "ctrl+shift+p")
        );
        // Modifier order is irrelevant and the stored chord is canonical.
        assert_eq!(
            parse_keymap_input("  fuzzy_jump   SHIFT+ALT+ctrl+PageUp  "),
            bound("fuzzy_jump", "shift+alt+ctrl+pageup")
        );
        assert_eq!(
            parse_keymap_input("open_terminal f4"),
            bound("open_terminal", "f4")
        );
    }

    #[test]
    fn parse_keymap_input_accepts_unbind_action() {
        let unbound = |action: &str| {
            Some(KeymapInput::Unbind {
                action: action.to_string(),
            })
        };
        assert_eq!(
            parse_keymap_input("unbind open_preview"),
            unbound("open_preview")
        );
        assert_eq!(
            parse_keymap_input("  unbind   fuzzy_jump \n"),
            unbound("fuzzy_jump")
        );
        // `none` is a serde name, not a bindable action: it must not become an
        // unbind target, and a chord is never accepted for the unbind form.
        assert_eq!(parse_keymap_input("unbind none"), None);
        assert_eq!(parse_keymap_input("unbind unknown_action"), None);
        assert_eq!(parse_keymap_input("unbind open_preview ctrl+p"), None);
        assert_eq!(parse_keymap_input("unbind"), None);
    }

    #[test]
    fn remove_keymap_override_drops_only_the_named_action() {
        let mut overrides = vec![
            ("open_preview".to_string(), "space".to_string()),
            ("reload".to_string(), "f5".to_string()),
            ("open_preview".to_string(), "ctrl+p".to_string()),
        ];
        assert!(remove_keymap_override(&mut overrides, "open_preview"));
        assert_eq!(overrides, vec![("reload".to_string(), "f5".to_string())]);
        // A second attempt is a no-op and reports that nothing was removed.
        assert!(!remove_keymap_override(&mut overrides, "open_preview"));
        assert!(!remove_keymap_override(&mut overrides, "quit"));
        assert_eq!(overrides.len(), 1);

        // Removing an override restores the default through `from_settings`.
        let settings = Settings {
            keymap: overrides,
            ..Settings::default()
        };
        let keymap = KeyMap::from_settings(&settings);
        assert_eq!(keymap.resolve("f5", false, false, false), KeyAction::Reload);
    }

    #[test]
    fn parse_keymap_input_rejects_unknown_and_unbindable_actions() {
        for input in [
            "not_an_action ctrl+p",
            "OPEN_PREVIEW ctrl+p",
            "none ctrl+p",
            "edit_keymap",
        ] {
            assert_eq!(parse_keymap_input(input), None, "accepted {input:?}");
        }
        // `edit_keymap` is a real serde name, so it parses when a chord follows.
        assert!(parse_keymap_input("edit_keymap ctrl+k").is_some());
    }

    #[test]
    fn parse_keymap_input_rejects_invalid_chords() {
        for input in [
            "open_preview ctrl+",
            "open_preview +p",
            "open_preview p+ctrl",
            "open_preview ctrl+ctrl+p",
        ] {
            assert_eq!(parse_keymap_input(input), None, "accepted {input:?}");
        }
    }

    #[test]
    fn parse_keymap_input_rejects_reserved_keys() {
        // K01: Ctrl+, backs the STATUS_SETTINGS_DISCARDED recovery hint, so a
        // keymap override must not be able to claim it.
        for chord in ["escape", "f1", "ctrl+f1", "shift+ESCAPE", ",", "ctrl+,"] {
            assert!(keymap_chord_reserved(chord), "not reserved: {chord}");
            let input = format!("open_preview {chord}");
            assert_eq!(parse_keymap_input(&input), None, "accepted {input:?}");
        }
    }

    #[test]
    fn parse_keymap_input_rejects_reserved_open_settings_action() {
        // L01: the keymap dialog (and the Settings overlay, which shares this
        // parser) must not be able to move or unbind Ctrl+,. Both forms are
        // rejected before any persisted override is touched.
        for input in [
            "open_settings ctrl+o",
            "open_settings shift+ctrl+o",
            "unbind open_settings",
            "  unbind   open_settings \n",
        ] {
            assert_eq!(parse_keymap_input(input), None, "accepted {input:?}");
        }
        // Other actions still parse, and the reservation is action-specific.
        assert!(parse_keymap_input("open_preview ctrl+o").is_some());
        assert!(parse_keymap_input("unbind open_preview").is_some());
    }

    #[test]
    fn keymap_chord_reserved_matches_only_the_last_segment() {
        // `f10` / `f12` merely contain the substring "f1" and must remain
        // bindable, matching `settings::sane_keymap_entry`.
        for chord in ["f10", "ctrl+f10", "f11", "f12", "ctrl+f12"] {
            assert!(!keymap_chord_reserved(chord), "rejected {chord}");
            let input = format!("open_preview {chord}");
            assert!(parse_keymap_input(&input).is_some(), "rejected {input:?}");
        }
        for chord in ["f1", "ctrl+f1", "escape", "alt+escape"] {
            assert!(keymap_chord_reserved(chord), "not reserved: {chord}");
        }
    }

    #[test]
    fn parse_keymap_input_whitespace_rules() {
        assert_eq!(
            parse_keymap_input("\t open_preview \t ctrl+shift+p \n"),
            Some(KeymapInput::Bind {
                action: "open_preview".to_string(),
                chord: "ctrl+shift+p".to_string(),
            })
        );
        // Exactly two whitespace-separated fields are required: no missing
        // chord, no extra action words, no empty input.
        for input in [
            "",
            "   ",
            "open_preview",
            "open preview ctrl+p",
            "open_preview ctrl+p extra",
        ] {
            assert_eq!(parse_keymap_input(input), None, "accepted {input:?}");
        }
    }

    #[test]
    fn upsert_keymap_override_evicts_other_action_on_same_chord() {
        let mut overrides = vec![
            ("open_preview".to_string(), "alt+p".to_string()),
            ("other".to_string(), "f5".to_string()),
        ];
        assert!(upsert_keymap_override(
            &mut overrides,
            "open_terminal",
            "alt+p"
        ));
        assert!(
            overrides.contains(&("open_terminal".to_string(), "alt+p".to_string())),
            "the new binding must be present"
        );
        assert!(
            !overrides.contains(&("open_preview".to_string(), "alt+p".to_string())),
            "the previous owner of the chord must be evicted"
        );
        assert_eq!(overrides.len(), 2, "unrelated bindings are preserved");
    }

    #[test]
    fn upsert_keymap_override_dedupes_ctrl_chord_alt_variant() {
        // S06: 文字列の正規化だけでは `ctrl+p` と `ctrl+alt+p` は別扱いになるが、
        // `binding_matches` は Ctrl バインドの `alt` を無視するのでdispatch 上は
        // 同じキー。保存済みオーバーライドも同じ規則で排除しなければ、ファイルに
        // 2 件残るのに再読み込み後は後勝ちの 1 件しか効かない。
        assert!(chords_collide("ctrl+p", "ctrl+alt+p"));
        assert!(chords_collide("ctrl+alt+p", "ctrl+p"));
        // shift は runtime と同じく区別する（from_settings は省略時 false）。
        assert!(!chords_collide("ctrl+p", "ctrl+shift+p"));
        assert!(chords_collide("shift+ctrl+p", "ctrl+shift+p"));

        let mut overrides = vec![("fuzzy_jump".to_string(), "ctrl+p".to_string())];
        assert!(upsert_keymap_override(
            &mut overrides,
            "open_preview",
            "ctrl+alt+p"
        ));
        assert_eq!(
            overrides,
            vec![("open_preview".to_string(), "ctrl+alt+p".to_string())],
            "the alt variant must evict the plain Ctrl binding"
        );

        // 残った 1 件から組み立てた live keymap も同じ解決結果になる。
        let settings = Settings {
            keymap: overrides.clone(),
            ..Settings::default()
        };
        let keymap = KeyMap::from_settings(&settings);
        for alt in [false, true] {
            assert_eq!(
                keymap.resolve("p", true, false, alt),
                KeyAction::OpenPreview,
                "alt={alt} must resolve to the sole persisted owner"
            );
        }

        // 逆の挿入順でも 1 件だけ生存し、保存される綴りは後勝ちのものになる。
        let mut reversed = vec![("fuzzy_jump".to_string(), "ctrl+alt+p".to_string())];
        assert!(upsert_keymap_override(
            &mut reversed,
            "open_preview",
            "ctrl+p"
        ));
        assert_eq!(
            reversed,
            vec![("open_preview".to_string(), "ctrl+p".to_string())]
        );
    }

    #[test]
    fn validated_settings_load_one_owner_for_ctrl_alt_variant() {
        // T01 / S06 load-time path: `Settings::validate` must collapse the two
        // spellings before `KeyMap::from_settings` sees them. Otherwise the
        // file claims two owners while `rebind`'s `binding_matches` eviction
        // keeps only the later one at runtime.
        let mut settings = Settings {
            keymap: vec![
                ("fuzzy_jump".to_string(), "ctrl+p".to_string()),
                ("open_preview".to_string(), "ctrl+alt+p".to_string()),
            ],
            ..Settings::default()
        };
        settings.validate();
        assert_eq!(
            settings.keymap,
            vec![("fuzzy_jump".to_string(), "ctrl+p".to_string())],
            "the Alt variant must be dropped while loading settings"
        );
        let keymap = KeyMap::from_settings(&settings);
        for alt in [false, true] {
            assert_eq!(
                keymap.resolve("p", true, false, alt),
                KeyAction::FuzzyJump,
                "alt={alt}: the surviving entry must own both spellings"
            );
        }
    }

    #[test]
    fn chords_collide_falls_back_to_the_canonical_form_for_bad_spellings() {
        // T11: an unparseable spelling has no modifier semantics, so the
        // fallback compares canonical (trimmed, lowercased) forms. Keep the
        // comparison symmetric and make sure a valid chord never collides
        // with an unrelated fallback.
        assert!(chords_collide(" CTRL+ ", "ctrl+"));
        assert!(chords_collide("ctrl+", " CTRL+ "));
        assert!(!chords_collide("ctrl+p", "p+ctrl"));
        assert!(!chords_collide("p+ctrl", "ctrl+p"));
    }

    #[test]
    fn canonical_chord_normalizes_modifier_order_and_upsert_evicts_across_spellings() {
        // Already-canonical persisted spellings must stay byte-identical.
        assert_eq!(canonical_chord("ctrl+shift+p"), "ctrl+shift+p");
        assert_eq!(canonical_chord("ctrl+p"), "ctrl+p");
        assert_eq!(canonical_chord("alt+k"), "alt+k");
        assert_eq!(canonical_chord("f4"), "f4");
        // Modifier order and case are irrelevant, key is lowercased.
        assert_eq!(canonical_chord("shift+ctrl+p"), "ctrl+shift+p");
        assert_eq!(
            canonical_chord("SHIFT+Ctrl+P"),
            canonical_chord("ctrl+shift+p")
        );

        // A non-canonical spelling of the same chord evicts the stored entry.
        let mut overrides = vec![("other".to_string(), "shift+ctrl+p".to_string())];
        assert!(upsert_keymap_override(
            &mut overrides,
            "open_preview",
            "ctrl+shift+p"
        ));
        assert_eq!(
            overrides,
            vec![("open_preview".to_string(), "ctrl+shift+p".to_string())],
            "the previous owner of the same chord must be evicted"
        );

        // Conversely, a newly added non-canonical spelling replaces a
        // canonical entry and is itself stored canonically.
        let mut overrides = vec![("open_preview".to_string(), "ctrl+shift+p".to_string())];
        assert!(upsert_keymap_override(
            &mut overrides,
            "other",
            "shift+ctrl+p"
        ));
        assert_eq!(
            overrides,
            vec![("other".to_string(), "ctrl+shift+p".to_string())]
        );
    }

    #[test]
    fn unparseable_chords_use_the_trimmed_lowercased_fallback() {
        // `parse_chord` は空トークン（末尾の `+`）とキー後の修飾子を拒否する。
        // その場合 `canonical_chord` は trim + 小文字化した入力を正規形とする。
        assert_eq!(canonical_chord("ctrl+"), "ctrl+");
        assert_eq!(canonical_chord(" p+ctrl "), "p+ctrl");

        // 両方が unparseable なら正規形の一致で衝突と判定される。
        assert!(chords_collide("ctrl+", "ctrl+"));
        // 片方だけ unparseable なら、もう片方の正規形と一致しない限り衝突しない。
        assert!(!chords_collide("ctrl+", "ctrl+p"));
    }

    // N06: the Alt+K success notice, the persisted override and the F1 help
    // line must all show the same canonical chord, whatever spelling was typed.
    #[test]
    fn keymap_bind_caption_uses_the_canonical_chord() {
        let notice = keymap_bind_caption("open_preview", "shift+ctrl+p");
        assert_eq!(
            notice,
            "キーマップを設定しました: open_preview → ctrl+shift+p"
        );

        let mut overrides = Vec::new();
        assert!(upsert_keymap_override(
            &mut overrides,
            "open_preview",
            "shift+ctrl+p"
        ));
        assert_eq!(overrides[0].1, "ctrl+shift+p");
        assert!(
            notice.ends_with(overrides[0].1.as_str()),
            "notice must match the stored chord: {notice}"
        );

        // Already-canonical and bare chords are unchanged.
        assert_eq!(
            keymap_bind_caption("open_preview", "ctrl+shift+p"),
            "キーマップを設定しました: open_preview → ctrl+shift+p"
        );
        assert_eq!(
            keymap_bind_caption("reload", "f5"),
            "キーマップを設定しました: reload → f5"
        );
    }

    #[test]
    fn upsert_keymap_override_dedupes_by_action_and_caps_count() {
        let mut overrides = vec![
            ("open_preview".to_string(), "space".to_string()),
            ("reload".to_string(), "f5".to_string()),
        ];
        assert!(upsert_keymap_override(
            &mut overrides,
            "open_preview",
            "ctrl+shift+p"
        ));
        assert_eq!(
            overrides,
            vec![
                ("open_preview".to_string(), "ctrl+shift+p".to_string()),
                ("reload".to_string(), "f5".to_string()),
            ]
        );

        // A full list accepts replacement of an existing action but no new one.
        let mut full: Vec<(String, String)> = (0..MAX_KEYMAP_OVERRIDES)
            .map(|index| (format!("action-{index}"), "f4".to_string()))
            .collect();
        assert!(!upsert_keymap_override(&mut full, "open_preview", "ctrl+p"));
        assert!(upsert_keymap_override(&mut full, "action-0", "ctrl+p"));
        assert_eq!(full.len(), MAX_KEYMAP_OVERRIDES);
        assert_eq!(full[0].1, "ctrl+p");
    }

    #[test]
    fn edit_keymap_shortcut_does_not_collide_with_ctrl_k() {
        let mut keymap = KeyMap::default_keymap();
        assert_eq!(keymap.resolve("k", false, false, false), KeyAction::None);
        assert_eq!(
            keymap.resolve("k", false, false, true),
            KeyAction::EditKeymap
        );
        assert_eq!(keymap.resolve("k", true, false, false), KeyAction::None);

        // Even after a user adds Ctrl+K, Alt+K stays distinct: `Ctrl` bindings
        // ignore `alt`, so Ctrl+Alt+K would follow Ctrl+K instead.
        assert!(keymap.rebind(KeyAction::Reload, "k", true, false, false));
        assert_eq!(keymap.resolve("k", true, false, false), KeyAction::Reload);
        assert_eq!(
            keymap.resolve("k", false, false, true),
            KeyAction::EditKeymap
        );
        assert_eq!(keymap.resolve("k", true, false, true), KeyAction::Reload);
    }

    #[test]
    fn fuzzy_score_is_case_insensitive_subsequence_with_ranking() {
        assert_eq!(fuzzy_score("main.rs", ""), Some(0));
        assert!(fuzzy_score("README.md", "readme").is_some());
        assert!(fuzzy_score("src/main.rs", "mn").is_some());
        assert_eq!(fuzzy_score("main.rs", "zz"), None);
        // An exact substring outranks a scattered subsequence of the same
        // candidate, and word-boundary matches are accepted.
        let exact = fuzzy_score("main.rs", "main").unwrap();
        let loose = fuzzy_score("main.rs", "mns").unwrap();
        assert!(exact > loose, "exact={exact} loose={loose}");
        assert!(fuzzy_score("src/main.rs", "mr").is_some());
    }

    #[test]
    fn fuzzy_source_label_covers_every_variant() {
        // Q10: all five `FuzzySource` arms are asserted in one table so a new
        // variant without a label (or a changed label) fails this test.
        let cases = [
            (FuzzySource::Tab, "タブ"),
            (FuzzySource::Bookmark, "ブックマーク"),
            (FuzzySource::Recent, "最近"),
            (FuzzySource::Ancestor, "階層"),
            (FuzzySource::Entry, "ファイル"),
        ];
        for (source, expected) in cases {
            assert_eq!(source.label(), expected, "{source:?}");
        }
    }

    #[test]
    fn fuzzy_candidates_dedupe_by_path_and_keep_source_priority() {
        let cwd = PathBuf::from("/work/project");
        let tabs = vec![cwd.clone(), PathBuf::from("/other/tab")];
        let bookmarks = vec![cwd.clone(), PathBuf::from("/home/user")];
        let recent = vec![PathBuf::from("/tmp/recent")];
        let entry = Entry {
            name: std::ffi::OsString::from("main.rs"),
            name_text: "main.rs".to_string(),
            sort_key: "main.rs".to_string(),
            path: PathBuf::from("/work/project/main.rs"),
            is_dir: false,
            is_symlink: false,
            size: 1,
            modified: None,
        };
        let labels = vec![(PathBuf::from("/home/user"), "home".to_string())];
        let candidates = fuzzy_candidates(
            &cwd,
            &tabs,
            &bookmarks,
            &recent,
            std::slice::from_ref(&entry),
            &labels,
        );

        // The current directory appears as a tab, bookmark and ancestor, but
        // only the highest-priority source survives.
        let count = candidates.iter().filter(|c| c.path == cwd).count();
        assert_eq!(count, 1, "duplicate paths must be collapsed");
        assert_eq!(
            candidates.iter().find(|c| c.path == cwd).unwrap().source,
            FuzzySource::Tab
        );
        let home = candidates
            .iter()
            .find(|c| c.path.as_path() == std::path::Path::new("/home/user"))
            .unwrap();
        assert_eq!(home.source, FuzzySource::Bookmark);
        assert_eq!(home.label, "[home] user");
        assert!(candidates
            .iter()
            .any(|c| c.path == entry.path && c.source == FuzzySource::Entry));
        assert!(candidates.iter().any(|c| {
            c.source == FuzzySource::Ancestor && c.path.as_path() == std::path::Path::new("/work")
        }));
    }

    #[test]
    fn rank_fuzzy_prefers_better_matches_and_bounds_results() {
        let candidates = vec![
            FuzzyCandidate {
                path: PathBuf::from("/a/notes.txt"),
                label: "notes.txt".to_string(),
                source: FuzzySource::Entry,
            },
            FuzzyCandidate {
                path: PathBuf::from("/b/notes.txt"),
                label: "notes.txt".to_string(),
                source: FuzzySource::Bookmark,
            },
            FuzzyCandidate {
                path: PathBuf::from("/c/main.rs"),
                label: "main.rs".to_string(),
                source: FuzzySource::Tab,
            },
            FuzzyCandidate {
                path: PathBuf::from("/d/zzz"),
                label: "zzz".to_string(),
                source: FuzzySource::Recent,
            },
        ];
        let ranked = rank_fuzzy(&candidates, "notes", 10);
        assert_eq!(ranked.len(), 2, "non-matching candidates are dropped");
        assert_eq!(ranked[0], 1, "Bookmark beats Entry on an equal score");
        assert_eq!(ranked[1], 0);
        assert!(!ranked.contains(&3));

        // Empty queries keep source priority (Tab first) and respect the cap.
        let all = rank_fuzzy(&candidates, "", 2);
        assert_eq!(all, vec![2, 1]);
        assert!(rank_fuzzy(&candidates, "", 0).is_empty());
    }

    #[test]
    fn normalize_terminal_command_trims_and_maps_blank_to_none() {
        assert_eq!(
            normalize_terminal_command("  konsole --title build  ").as_deref(),
            Some("konsole --title build")
        );
        assert_eq!(
            normalize_terminal_command("\t konsole \n").as_deref(),
            Some("konsole")
        );
        assert_eq!(normalize_terminal_command(""), None);
        assert_eq!(normalize_terminal_command("   \t\n "), None);
        // Non-ASCII commands survive trimming unchanged.
        assert_eq!(
            normalize_terminal_command("  端末 --x  ").as_deref(),
            Some("端末 --x")
        );
    }

    #[test]
    fn alt_t_terminal_command_shortcut_does_not_collide_with_ctrl_t() {
        let keymap = KeyMap::default_keymap();
        // Alt+T is the chosen shortcut for the terminal-command dialog.
        assert_eq!(
            keymap.resolve("t", false, false, true),
            KeyAction::EditTerminalCommand
        );
        // Plain "t" stays a no-op as before.
        assert_eq!(keymap.resolve("t", false, false, false), KeyAction::None);
        // Existing Ctrl+T / Ctrl+Shift+T are unchanged. Ctrl+Alt+T still
        // resolves to NewTab because `binding_matches` ignores `alt` for Ctrl
        // bindings -- the reason Alt+T was chosen instead.
        assert_eq!(keymap.resolve("t", true, false, false), KeyAction::NewTab);
        assert_eq!(
            keymap.resolve("t", true, true, false),
            KeyAction::RestoreClosedTab
        );
        assert_eq!(keymap.resolve("t", true, false, true), KeyAction::NewTab);
    }

    #[test]
    fn alt_shift_t_toggles_theme_without_stealing_alt_t() {
        let keymap = KeyMap::default_keymap();
        // Alt+Shift+T is the new theme cycle; it precedes the Alt+T wildcard
        // binding so plain Alt+T (and the Ctrl variants) still work.
        assert_eq!(
            keymap.resolve("t", false, true, true),
            KeyAction::ToggleTheme
        );
        assert_eq!(
            keymap.resolve("t", false, false, true),
            KeyAction::EditTerminalCommand
        );
        assert_eq!(keymap.resolve("t", true, false, false), KeyAction::NewTab);
        assert_eq!(
            keymap.resolve("t", true, true, false),
            KeyAction::RestoreClosedTab
        );
        // Ctrl+Alt+T remains NewTab: `binding_matches` ignores alt for Ctrl.
        assert_eq!(keymap.resolve("t", true, false, true), KeyAction::NewTab);
    }
}
