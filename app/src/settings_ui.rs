// SPDX-License-Identifier: AGPL-3.0-or-later
//! Self-contained preferences overlay for the file explorer.
//!
//! The module deliberately avoids `gpui_component::setting::{Settings, ...}`:
//! those helpers mutate their own state and cannot write back into the
//! explorer's persisted `Settings`. Instead this module only owns the *visual*
//! form and reports user intent through [`SettingsUiAction`]. The host view
//! (normally `Explorer`) implements [`SettingsUiHost`] and decides how to apply
//! each action to its own state.
//!
//! All colors are supplied through [`SettingsUiPalette`] so the same form works
//! for the dark (Catppuccin Mocha) and light (Catppuccin Latte) themes. The
//! parent builds the overlay backdrop and centres the element returned by
//! [`render_settings_content`].

// `main.rs` consumes this module's public API. A few optional helpers remain
// unused, so the allow is scoped to this file; remove it when they are wired.
#![allow(dead_code)]

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::radio::RadioGroup;
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::TabBar;
use gpui_kit::prelude::*;
use gpui_kit::*;

// ---------------------------------------------------------------------------
// Tabs
// ---------------------------------------------------------------------------

/// The five preferences pages, in the order shown in the segmented [`TabBar`].
/// Mirrors the tab layout of Dolphin/Nautilus Preferences.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SettingsTab {
    #[default]
    Appearance,
    Files,
    Shortcuts,
    Terminal,
    Personalize,
}

impl SettingsTab {
    /// Display order of the tabs.
    pub const ALL: [Self; 5] = [
        Self::Appearance,
        Self::Files,
        Self::Shortcuts,
        Self::Terminal,
        Self::Personalize,
    ];

    /// Convert a [`TabBar`] click index into a tab. Out-of-range indices fall
    /// back to the first page so a stale click can never panic the host.
    pub fn from_index(index: usize) -> Self {
        Self::ALL.get(index).copied().unwrap_or(Self::Appearance)
    }

    /// Index of this tab, matching [`SettingsTab::from_index`].
    pub fn index(self) -> usize {
        match self {
            Self::Appearance => 0,
            Self::Files => 1,
            Self::Shortcuts => 2,
            Self::Terminal => 3,
            Self::Personalize => 4,
        }
    }

    /// Localized tab caption.
    pub fn label(self) -> &'static str {
        match self {
            Self::Appearance => "外観",
            Self::Files => "ファイル",
            Self::Shortcuts => "ショートカット",
            Self::Terminal => "ターミナル",
            Self::Personalize => "個人化",
        }
    }
}

// ---------------------------------------------------------------------------
// Palette
// ---------------------------------------------------------------------------

/// Colors used by the settings card. The host passes a palette matching the
/// active application theme; the module never reads private `main.rs` colors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingsUiPalette {
    /// Card background (Catppuccin `surface`).
    pub surface: u32,
    /// Borders, separators and segmented-control trough (Catppuccin `overlay`).
    pub overlay: u32,
    /// Primary row captions.
    pub text: u32,
    /// Row descriptions.
    pub subtext: u32,
    /// De-emphasized notes.
    pub muted: u32,
    /// Section titles and selection accents (Catppuccin `blue`).
    pub accent: u32,
    /// Success / enabled hints.
    pub green: u32,
    /// Validation errors.
    pub red: u32,
}

impl SettingsUiPalette {
    /// Catppuccin Mocha values used by the application's dark theme.
    pub const fn dark() -> Self {
        Self {
            surface: 0x313244,
            overlay: 0x45475a,
            text: 0xcdd6f4,
            subtext: 0xa6adc8,
            muted: 0x6c7086,
            accent: 0x89b4fa,
            green: 0xa6e3a1,
            red: 0xf38ba8,
        }
    }

    /// Catppuccin Latte values used by the application's light theme.
    pub const fn light() -> Self {
        Self {
            surface: 0xccd0da,
            overlay: 0xbcc0cc,
            text: 0x4c4f69,
            subtext: 0x5c5f77,
            muted: 0x6c6f85,
            accent: 0x1e66f5,
            green: 0x40a02b,
            red: 0xd20f39,
        }
    }
}

// ---------------------------------------------------------------------------
// Actions
// ---------------------------------------------------------------------------

/// User intent emitted by the form. The host applies it to its own settings.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettingsUiAction {
    /// Nothing to do (used as a default / no-op callback result).
    None,
    /// Persist the edited values and close the overlay.
    Save,
    /// Discard edits and close the overlay.
    Cancel,
    /// Switch to the given preferences page.
    Tab(SettingsTab),
    /// Change the theme choice (`dark` / `light` / `system`).
    SetTheme(String),
    /// Change the view layout choice (`details` / `icons`).
    SetView(String),
    /// Show or hide dot-files.
    ToggleHidden(bool),
    /// Change the sort key.
    SetSort(String),
    /// Replace the terminal startup command.
    SetTerminal(String),
    /// Replace the keymap overrides.
    SetKeymap(String),
}

// ---------------------------------------------------------------------------
// State
// ---------------------------------------------------------------------------

/// Mutable state of one open settings overlay. The host owns it as an
/// `Option<SettingsUiState>`: `None` means closed.
///
/// Text fields are real GPUI inputs so the user gets normal editing, selection
/// and IME behavior. The host reads them back on [`SettingsUiAction::Save`]
/// with [`SettingsUiState::terminal_value`] / [`SettingsUiState::keymap_value`].
pub struct SettingsUiState {
    /// Currently shown tab.
    pub tab: SettingsTab,
    /// Selected theme option value, filled by the host when opening.
    pub theme_choice: String,
    /// Selected view option value, filled by the host when opening.
    pub view_choice: String,
    /// Selected sort option value, filled by the host when opening.
    pub sort_choice: String,
    /// Current "show hidden files" flag.
    pub show_hidden: bool,
    /// Terminal startup command editor.
    pub terminal_input: Entity<InputState>,
    /// Keymap override editor (`action=chord` entries separated by `;`).
    pub keymap_input: Entity<InputState>,
    /// Settings search box in the card header; filters rows by caption.
    pub search_input: Entity<InputState>,
    /// Caller-owned focus handle for the footer Cancel button (I05).
    pub cancel_focus: FocusHandle,
    /// Caller-owned focus handle for the footer Save button (I05).
    pub save_focus: FocusHandle,
    /// Validation / save error shown on the active tab.
    pub error: Option<String>,
    /// Whether any control has been edited since opening.
    pub dirty: bool,
    /// Reserved for hosts that want to attach their own input subscriptions.
    pub subscriptions: Vec<Subscription>,
}

impl SettingsUiState {
    /// Create an overlay with empty text fields.
    pub fn new(window: &mut Window, cx: &mut App) -> Self {
        Self::with_initial_values(window, cx, "", "")
    }

    /// Create an overlay prefilled from the host's persisted settings.
    pub fn with_initial_values(
        window: &mut Window,
        cx: &mut App,
        terminal: &str,
        keymap: &str,
    ) -> Self {
        let terminal_input = cx.new(|cx| InputState::new(window, cx).default_value(terminal));
        let keymap_input = cx.new(|cx| InputState::new(window, cx).default_value(keymap));
        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder(SETTINGS_SEARCH_PLACEHOLDER));
        Self {
            tab: SettingsTab::Appearance,
            theme_choice: String::new(),
            view_choice: String::new(),
            sort_choice: String::new(),
            show_hidden: false,
            terminal_input,
            keymap_input,
            search_input,
            cancel_focus: cx.focus_handle(),
            save_focus: cx.focus_handle(),
            error: None,
            dirty: false,
            subscriptions: Vec::new(),
        }
    }

    /// Switch the visible page (also used by tests for tab state changes).
    pub fn select_tab(&mut self, tab: SettingsTab) {
        self.tab = tab;
    }

    /// Set the selected theme value.
    pub fn set_theme_choice(&mut self, value: impl Into<String>) {
        self.theme_choice = value.into();
    }

    /// Set the selected view-layout value.
    pub fn set_view_choice(&mut self, value: impl Into<String>) {
        self.view_choice = value.into();
    }

    /// Set the selected sort-key value.
    pub fn set_sort_choice(&mut self, value: impl Into<String>) {
        self.sort_choice = value.into();
    }

    /// Set the "show hidden files" flag.
    pub fn set_show_hidden(&mut self, value: bool) {
        self.show_hidden = value;
    }

    /// Current terminal command text.
    pub fn terminal_value(&self, cx: &App) -> String {
        self.terminal_input.read(cx).value().to_string()
    }

    /// Current keymap override text.
    pub fn keymap_value(&self, cx: &App) -> String {
        self.keymap_input.read(cx).value().to_string()
    }

    /// Subscribe to the search box so typing re-renders the host and refreshes
    /// the row filter in [`render_settings_content`]. Call once right after the
    /// state is constructed; the subscription lives as long as this state.
    pub fn observe_inputs<V: SettingsUiHost>(&mut self, cx: &mut Context<V>) {
        let search = self.search_input.clone();
        let subscription =
            cx.subscribe(&search, |_host: &mut V, _input, _event: &InputEvent, cx| {
                cx.notify();
            });
        self.subscriptions.push(subscription);
    }

    /// Move keyboard focus one step through the visible controls. `backwards`
    /// implements Shift+Tab. Returns `true` when a control was focused.
    ///
    /// The host calls this from its settings key guard for bare Tab; Ctrl+Tab
    /// keeps its page-switching role (I05).
    pub fn cycle_focus(&self, window: &mut Window, cx: &mut App, backwards: bool) -> bool {
        // Copy the filter first: the read guard borrows `cx` immutably while
        // `focus()` below needs it mutably.
        let query = self.search_input.read(cx).value().to_string();
        let order = settings_focus_order(self.tab, &query);
        let current = order.iter().copied().find(|target| match target {
            SettingsFocusTarget::Search => self
                .search_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window),
            SettingsFocusTarget::Keymap => self
                .keymap_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window),
            SettingsFocusTarget::Terminal => self
                .terminal_input
                .read(cx)
                .focus_handle(cx)
                .is_focused(window),
            SettingsFocusTarget::Cancel => self.cancel_focus.is_focused(window),
            SettingsFocusTarget::Save => self.save_focus.is_focused(window),
        });
        let Some(target) = next_settings_focus(&order, current, backwards) else {
            return false;
        };
        let focus = match target {
            SettingsFocusTarget::Search => self.search_input.read(cx).focus_handle(cx),
            SettingsFocusTarget::Keymap => self.keymap_input.read(cx).focus_handle(cx),
            SettingsFocusTarget::Terminal => self.terminal_input.read(cx).focus_handle(cx),
            SettingsFocusTarget::Cancel => self.cancel_focus.clone(),
            SettingsFocusTarget::Save => self.save_focus.clone(),
        };
        focus.focus(window, cx);
        true
    }
}

// ---------------------------------------------------------------------------
// Host trait
// ---------------------------------------------------------------------------

/// Implemented by the view that owns the settings overlay. Keeping the
/// callback in a trait means this module never mentions `Explorer` and the
/// parent only needs a short `impl` block plus an overlay layer in `render`.
pub trait SettingsUiHost: Render {
    /// Handle a user action emitted by the settings form.
    fn on_settings_action(&mut self, action: SettingsUiAction, cx: &mut Context<Self>);
}

// ---------------------------------------------------------------------------
// Pure helpers
// ---------------------------------------------------------------------------

/// Index of `choice` in an option list, defaulting to the first entry.
fn selected_index(options: &[(&str, &str)], choice: &str) -> usize {
    options
        .iter()
        .position(|(value, _)| *value == choice)
        .unwrap_or(0)
}

/// Case-insensitive substring filter used by the header search box. An empty
/// query matches everything.
fn matches_query(query: &str, keywords: &[&str]) -> bool {
    let needle = query.trim().to_lowercase();
    if needle.is_empty() {
        return true;
    }
    keywords
        .iter()
        .any(|candidate| candidate.to_lowercase().contains(&needle))
}

/// Caption of the dismiss button. With unsaved edits the neutral `キャンセル`
/// would hide the fact that closing discards them, so the label names the
/// consequence instead (`E12`).
fn cancel_label(dirty: bool) -> &'static str {
    if dirty {
        "保存せず閉じる"
    } else {
        crate::strings::LABEL_CANCEL
    }
}

/// Text of the single, shared save-status line in the footer.
///
/// K13: this is the only place the "changes apply on Save" hint is rendered;
/// the Terminal tab used to repeat it in its body when the form was clean.
fn footer_status_text(dirty: bool) -> &'static str {
    if dirty {
        "未保存の変更があります"
    } else {
        crate::strings::STATUS_SETTINGS_UNSAVED_HINT
    }
}

/// Always-visible key legend of the settings dialog footer (P10).
///
/// F1 is deliberately blocked inside the dialog, so the keys users would look
/// up in the help overlay have to be shown here. Kept to one compact line to
/// fit the 760px card next to the save/cancel buttons.
fn settings_footer_legend() -> &'static str {
    concat!(
        "Tab/Shift+Tab 移動 / Ctrl+Tab・Ctrl+Shift+Tab タブ / Enter 保存 / Esc 閉じる / ",
        crate::strings::settings_tab_jump_keys!(),
        " 直接移動"
    )
}

// Search keyword sets, one per visible row. They are shared by the per-row
// filter in `render_settings_content` and by `tabs_matching_query`, so a row and
// the cross-tab hint derived from it can never drift apart.
// M09: each array also carries the words actually rendered in that row's
// caption, description and section title — users search what they can see, not
// the internal option value. V07: the persisted `settings.json` key name is
// included as well, because recovery hints quote that exact key and the
// placeholder advertises "キー名で検索". Keep the lists in sync with the row
// markup below.
const APPEARANCE_THEME_KEYWORDS: &[&str] = &[
    "テーマ",
    "theme",
    // V07: the persisted `settings.json` key name must be searchable too —
    // recovery hints (e.g. strings.rs) name that key, and the placeholder
    // promises "キー名で検索". Without it those hints lead to a zero-hit search.
    "theme_choice",
    "ダーク",
    "ライト",
    "システム",
    "配色",
    "外観",
    // "アプリ全体の明るさを切り替えます（t でキーボードから切替）"
    "明るさ",
    "切り替え",
    "キーボード",
    "切替",
];
const APPEARANCE_VIEW_KEYWORDS: &[&str] = &[
    "レイアウト",
    "view",
    "一覧",
    "アイコン",
    "表示",
    "外観",
    // "既定のビュー" / "詳細一覧またはアイコン一覧を選択します（v でキーボードから切替）"
    "ビュー",
    "既定のビュー",
    "詳細一覧",
    "選択",
    "キーボード",
    "切替",
];
const FILES_HIDDEN_KEYWORDS: &[&str] = &[
    "隠しファイル",
    "hidden",
    // V07: persisted `settings.json` key name (see APPEARANCE_THEME_KEYWORDS).
    "show_hidden",
    "ファイル",
    "ドットファイル",
    // Visible on this row: "ファイルの表示" (section) / "隠しファイルを表示" (label).
    "表示",
    // "名前が . で始まるファイルとフォルダを一覧に含めます（h でキーボードから切替）"
    "ドット",
    "フォルダ",
    "一覧",
    "キーボード",
    "切替",
];
const FILES_SORT_KEYWORDS: &[&str] = &[
    "並び",
    "sort",
    "ファイル",
    "順序",
    "Alt+S",
    // "既定の並び順" / "…基準となる項目（Alt+S でキーボードから切替）"
    "並び順",
    "既定",
    "キーボード",
    "切替",
    "基準",
    // Y04: the radio group shows `SortKey::label()` values (main.rs
    // `sort_options`) — users search the visible choice labels, not the
    // internal `name`/`kind`/`size`/`modified` values. The direction words
    // are the header-click wording documented in README.
    "名前",
    "種類",
    "サイズ",
    "更新日時",
    "昇順",
    "降順",
];
const SHORTCUTS_KEYMAP_KEYWORDS: &[&str] = &[
    "ショートカット",
    "キー",
    "keymap",
    "キーバインド",
    "割り当て",
    // Users search by the chord they remember, not by the row label (H09).
    "Alt+K",
    "Ctrl+,",
    "Ctrl+Tab",
    "Ctrl+Shift+Tab",
    "F1",
    // The no-match hint names these direct-jump chords; searching that wording
    // must land on the Shortcuts page too (L03).
    "Alt+数字",
    // Visible hint + section title ("キーボードショートカット", "操作=キー",
    // "unbind アクション名", "置き換わります", "入力します").
    "キーボード",
    "操作",
    "入力",
    "unbind",
    "置き換え",
    // AA05/M09: SETTINGS_KEYMAP_INPUT_HINT prints the example
    // "open_preview=ctrl+shift+p; quit=ctrl+q". Users copy that wording into
    // this search box, so every token must resolve to the Shortcuts tab
    // instead of dead-ending. Lowercase chords match case-insensitively.
    "open_preview",
    "ctrl+shift+p",
    "quit",
    "ctrl+q",
];
/// Searchable keywords of the "既定のショートカット" row group.
///
/// T07: every *chord* keyword below is a substring of a row rendered from
/// [`SHORTCUTS_DEFAULT_LINES`] (enforced by
/// `tests::every_shortcut_keyword_is_rendered`). Without that invariant a
/// search could report "一致があります" for a tab that never shows the typed
/// chord — the failure mode the `no_match_hint` comment warns about. The
/// leading non-chord aliases are search-only synonyms of visible wording
/// (section title / row text) and are listed in `tests::SEARCH_ALIAS_KEYWORDS`.
const SHORTCUTS_DEFAULTS_KEYWORDS: &[&str] = &[
    // Search-only wording aliases (visible as the section title / row text).
    "既定",
    "default",
    "ショートカット",
    "キー",
    "主な",
    "タブ",
    "直接移動",
    "Alt+数字",
    "ヘルプ",
    "キー一覧",
    "場所",
    "設定を開く",
    // Chords from the F1 help "ナビゲーション" section.
    "Enter",
    "↑",
    "↓",
    "Ctrl+L",
    "Alt+←",
    "Alt+→",
    "Alt+↑",
    "Backspace",
    "Home",
    "End",
    "PageUp",
    "PageDown",
    "Ctrl+P",
    "Ctrl+Shift+L",
    // Chords from the F1 help "選択" section.
    "Ctrl+A",
    "Ctrl+Space",
    "Shift+↑↓",
    // Chords from the F1 help "ファイル操作" section.
    "Ctrl+C",
    "Ctrl+X",
    "Ctrl+V",
    "Ctrl+Shift+C",
    "F2",
    "Delete",
    "R",
    "Ctrl+Enter",
    "Ctrl+Shift+N",
    "Ctrl+Z",
    "Ctrl+Shift+Z",
    "Ctrl+Shift+U",
    // Chords from the F1 help "表示" section.
    "Ctrl+1",
    "Ctrl+2",
    "Ctrl+H",
    "Ctrl+R",
    "F5",
    "Ctrl+F",
    "Ctrl+Shift+F",
    "Ctrl+I",
    "Space",
    "F3",
    "Alt+Shift+T",
    // Chords from the F1 help "タブ・ブックマーク" section.
    "Ctrl+T",
    "Ctrl+W",
    "Ctrl+Shift+T",
    "Ctrl+Tab",
    "Ctrl+Shift+Tab",
    "Ctrl+Shift+PageUp",
    "Ctrl+D",
    // Chords from the F1 help "その他" section, plus the settings-dialog
    // chords rendered on this tab (they live in dialog descriptions in F1).
    "F1",
    "F4",
    "Alt+T",
    "Alt+K",
    "Esc",
    "Ctrl+Q",
    "Ctrl+,",
    "Alt+1",
    "Alt+2",
    "Alt+3",
    "Alt+4",
    "Alt+5",
    "Alt+1〜5",
    "Alt+S",
];

/// Placeholder of the settings-page search field. Chord names such as `Alt+K`
/// are searchable too, so the wording must advertise that (R07).
const SETTINGS_SEARCH_PLACEHOLDER: &str = "設定・キー名で検索…";

/// Shortcuts-tab keymap hint. Unlike the Alt+K dialog this field replaces the
/// whole override list on Save, so that consequence must be spelled out where
/// the user is typing (H04). Per-entry `unbind アクション名` is accepted here
/// too (the Save handler parses it through `parse_keymap_input`), so the hint
/// must not send users to Alt+K for something this field can do itself (K04).
/// M04: like `strings::KEYMAP_INPUT_HINT`, it also names `open_settings` as a
/// reserved action — `Ctrl+,` is fixed and cannot be rebound (or unbound)
/// from either editor, so promising otherwise here would mislead the user.
/// Z07/Z12: the reserved-key note and the `MAX_KEYMAP_OVERRIDES` cap come from
/// the canonical `strings.rs` definitions via the shared macro/literal.
const SETTINGS_KEYMAP_INPUT_HINT: &str = concat!(
    "「操作=キー」形式で入力します。複数指定は ; で区切ります（例: open_preview=ctrl+shift+p; quit=ctrl+q）。保存すると一覧は入力内容で置き換わります（一部追加ではありません）。この欄でも `unbind アクション名` で解除できます。",
    crate::strings::reserved_keys_note!(),
    "。open_settings は Ctrl+, 固定で変更できません（unbind も不可）。最大 64 件まで。"
);

/// Descriptions of the three rows with a bare-letter shortcut, plus the
/// existing Alt+S row. Kept as constants so the visible hint can be regression
/// tested without a GPUI window. F1 is blocked while the settings dialog owns
/// the keyboard, so each row still names its own key inline even though the
/// same t/v/h keys are also listed in `strings::HELP_SECTIONS` (N04). The
/// shared typing-guard suffix (W07/X13) comes from the single canonical
/// `strings::settings_input_guard_note!()` literal and is mirrored by the F1
/// help rows, the Shortcuts-tab Alt+S row and README;
/// `typing_guard_note_is_synced_across_row_hints_help_and_readme` keeps all of
/// them in step.
const APPEARANCE_THEME_DESCRIPTION: &str = concat!(
    "アプリ全体の明るさを切り替えます（t でキーボードから切替）",
    crate::strings::settings_input_guard_note!()
);
const APPEARANCE_VIEW_DESCRIPTION: &str = concat!(
    "詳細一覧またはアイコン一覧を選択します（v でキーボードから切替）",
    crate::strings::settings_input_guard_note!()
);
const FILES_HIDDEN_DESCRIPTION: &str = concat!(
    "名前が . で始まるファイルとフォルダを一覧に含めます（h でキーボードから切替）",
    crate::strings::settings_input_guard_note!()
);
const FILES_SORT_DESCRIPTION: &str = concat!(
    "ファイル一覧を開いたときの基準となる項目（Alt+S でキーボードから切替）",
    crate::strings::settings_input_guard_note!()
);

/// Rows of the "主な既定のショートカット" list, kept as one constant so the
/// Shortcuts tab, F1 help (`strings::HELP_SECTIONS`) and README cannot drift
/// apart. The chord names are also searchable via `SHORTCUTS_DEFAULTS_KEYWORDS`
/// (L03/L07).
///
/// T06: this is a full 1:1 mirror of the keyboard rows in F1 help, not a loose
/// sample — `tests::shortcut_rows_and_f1_help_keys_are_bidirectional` checks
/// both directions. Each row puts the F1 help key verbatim before the ` … `
/// separator so the mirror is mechanical. Two groups are documented exceptions:
///
/// * Prose / mouse / metadata help rows (gestures, "ゴミ箱ビュー", the dialog
///   guidance entries and the license row) have no chord to render.
/// * `Alt+1〜5` and `Alt+S` appear in F1 help *descriptions* of the dialog
///   entries rather than as keys, because F1 is swallowed while the settings
///   dialog owns the keyboard; they are still shown here so the user can find
///   the dialog's own keys before opening it.
const SHORTCUTS_DEFAULT_LINES: &[&str] = &[
    // ナビゲーション
    "F1 … ヘルプ / キー一覧",
    "Enter … 選択項目を開く",
    "↑ / ↓ … 選択項目を上下に移動",
    "Ctrl+L … 場所を入力",
    "Alt+← / Alt+→ … 戻る / 進む",
    "Alt+↑ / Backspace … 親フォルダ",
    "Home / End … 先頭 / 末尾へ",
    "PageUp / PageDown … ページ単位で移動",
    "Ctrl+P … クイックジャンプ",
    "Ctrl+Shift+L … サイドバーのホームへ移動",
    // 選択
    "Ctrl+A … 全選択",
    "Ctrl+Space … フォーカス行の選択を切替（非連続選択）",
    "Shift+↑↓ … 範囲選択",
    // ファイル操作
    "Ctrl+C / Ctrl+X / Ctrl+V … コピー / 切り取り / 貼り付け",
    "Ctrl+Shift+C … フォーカス行のパスをコピー",
    "F2 … 名前変更",
    "Delete … ゴミ箱へ移動（完全削除なし）",
    "R / Ctrl+Enter … 選択項目を元の場所へ復元（ゴミ箱ビュー）",
    "Ctrl+Shift+N … 新規フォルダ",
    "Ctrl+Z / Ctrl+Shift+Z … 元に戻す / やり直す（名前変更・ゴミ箱のみ）",
    "Ctrl+Shift+U … 操作の履歴を表示",
    // 表示
    "Ctrl+1 / Ctrl+2 … 詳細リスト / アイコングリッド",
    "Ctrl+H … 隠しファイルの表示切替",
    "Ctrl+R / F5 … 再読込",
    "Ctrl+F … ファイルを検索",
    "Ctrl+Shift+F … 検索結果を名前で絞り込み",
    "Ctrl+I … プロパティ",
    "Space / F3 … テキストプレビュー",
    "Alt+Shift+T … テーマをダーク / ライト / システムで切替",
    // タブ・ブックマーク
    "Ctrl+T / Ctrl+W … タブを開く / 閉じる",
    "Ctrl+Shift+T … 閉じたタブを復元",
    "Ctrl+Tab / Ctrl+Shift+Tab … 次のタブ / 前のタブ",
    "Ctrl+Shift+PageUp / PageDown … タブを左 / 右へ移動",
    "Ctrl+D … 現在のディレクトリをブックマーク",
    // その他 / 設定ダイアログ（F1 はダイアログ内で無効なのでここに掲載）
    "F4 … ターミナルを開く",
    "Alt+T … ターミナル起動コマンドの設定",
    "Alt+K … キーボードショートカットを設定",
    "Esc … キャンセル / ダイアログを閉じる",
    "Ctrl+Q … 終了",
    "Ctrl+, … 設定を開く",
    "Alt+1〜5 … 設定ダイアログのタブへ直接移動",
    concat!(
        "Alt+S … ファイルタブの並び順を切替",
        crate::strings::settings_input_guard_note!()
    ),
];
const TERMINAL_COMMAND_KEYWORDS: &[&str] = &[
    "ターミナル",
    "terminal",
    // V07: persisted `settings.json` key name; the "起動コマンドが未設定です"
    // recovery hint (strings.rs) points users at it by name.
    "terminal_command",
    "コマンド",
    "起動",
    // "選択中のフォルダを作業ディレクトリとして起動するコマンド（空欄で無効化）"
    "作業ディレクトリ",
    "無効化",
    "フォルダ",
    "選択",
    "空欄",
];
const PERSONALIZE_PALETTE_KEYWORDS: &[&str] = &[
    "個人化",
    "パレット",
    "色",
    "カラー",
    "theme",
    // "カラーパレット" / "アクセント / 成功 / エラー"
    "カラーパレット",
    "アクセント",
    "成功",
    "エラー",
];
const PERSONALIZE_INFO_KEYWORDS: &[&str] = &[
    "個人化",
    "情報",
    "about",
    "設定",
    "バージョン",
    // "このアプリについて" and the read-only info lines.
    "このアプリ",
    "アプリ",
    "GPUI",
    "ファイルマネージャー",
    // S10: license/source rows rendered below (F1 is blocked in the dialog).
    "ライセンス",
    "AGPL",
    "ソース",
    "settings.json",
    "保存",
    "配色",
];

/// Every searchable keyword group of a tab, in visible row order.
fn tab_keyword_groups(tab: SettingsTab) -> &'static [&'static [&'static str]] {
    match tab {
        SettingsTab::Appearance => &[APPEARANCE_THEME_KEYWORDS, APPEARANCE_VIEW_KEYWORDS],
        SettingsTab::Files => &[FILES_HIDDEN_KEYWORDS, FILES_SORT_KEYWORDS],
        SettingsTab::Shortcuts => &[SHORTCUTS_KEYMAP_KEYWORDS, SHORTCUTS_DEFAULTS_KEYWORDS],
        SettingsTab::Terminal => &[TERMINAL_COMMAND_KEYWORDS],
        SettingsTab::Personalize => &[PERSONALIZE_PALETTE_KEYWORDS, PERSONALIZE_INFO_KEYWORDS],
    }
}

/// Tabs that contain at least one row matching `query`, in display order.
///
/// Pure and independent of any GPUI state, so the overlay can offer a hint when
/// the active tab has no matches. Like [`matches_query`], an empty (or
/// whitespace-only) query matches every tab because every row is visible.
pub fn tabs_matching_query(query: &str) -> Vec<SettingsTab> {
    SettingsTab::ALL
        .iter()
        .copied()
        .filter(|tab| {
            tab_keyword_groups(*tab)
                .iter()
                .any(|keywords| matches_query(query, keywords))
        })
        .collect()
}

/// Message shown when the active tab has no rows matching the search query.
///
/// Pure so the wording can be regression-tested without a GPUI window. When
/// another tab holds the matches, the hint names each tab together with its
/// own Alt+digit direct-jump key, giving the user an actionable keyboard route
/// to the reported page instead of a generic "Alt+数字" (K07/L05).
pub fn no_match_hint(matching_tabs: &[SettingsTab]) -> String {
    if matching_tabs.is_empty() {
        // M14: a dead end must still offer a way out. With zero hits there is
        // no tab to jump to, so name the two recovery routes that always work.
        "該当する設定がありません。検索欄を空にするか Esc で閉じてください".to_string()
    } else {
        let labels: Vec<String> = matching_tabs
            .iter()
            .map(|tab| format!("{}（Alt+{}）", tab.label(), tab.index() + 1))
            .collect();
        format!(
            "該当する設定がありません。{}タブに一致があります",
            labels.join("、")
        )
    }
}

// ---------------------------------------------------------------------------
// Row shortcuts for controls without a Tab stop (M08)
// ---------------------------------------------------------------------------

/// Current values needed to turn a row shortcut into a concrete action.
///
/// `SettingsUiAction` deliberately has no "cycle" variant: adding one would
/// break the exhaustive `match` in `main.rs`'s `on_settings_action`, which is
/// outside this module's scope. The existing `SetTheme` / `SetView` /
/// `ToggleHidden` actions are already wired to the dirty-flag and Save path, so
/// the helper resolves the next value here instead. The caller can fill this
/// struct straight from its `SettingsUiState` fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SettingsRowValues<'a> {
    /// Current theme choice (`dark` / `light` / `system`).
    pub theme: &'a str,
    /// Current view choice (`details` / `icons`).
    pub view: &'a str,
    /// Current "show hidden files" switch state.
    pub hidden: bool,
}

/// Map a bare letter pressed on a settings page to the row it controls.
///
/// The settings card's key guard in `main.rs` swallows global shortcuts, and
/// gpui-component 0.9.1 excludes `RadioGroup` / `Switch` from the Tab focus
/// cycle, so three rows are otherwise keyboard-inaccessible while the dialog
/// is open. This mapping is deliberately pure: it takes only the page, the key
/// and the current values, and returns an *existing* action for the host to
/// pass to `SettingsUiHost::on_settings_action`.
///
/// Keys are chosen to be free in the settings guard (only Tab, Enter, Escape,
/// Alt+1..5 and Alt+S are claimed there) and are scoped per page so the same
/// letter never controls two different rows:
///
/// | Page | Key | Row | Returned action |
/// |---|---|---|---|
/// | 外観 (`Appearance`) | `t` | 配色 (テーマ) | [`SettingsUiAction::SetTheme`] with [`settings_theme_next`] |
/// | 外観 (`Appearance`) | `v` | 既定のビュー | [`SettingsUiAction::SetView`] with [`settings_view_next`] |
/// | ファイル (`Files`) | `h` | 隠しファイルを表示 | [`SettingsUiAction::ToggleHidden`] with [`settings_hidden_next`] |
///
/// Host requirements when wiring this in `main.rs`:
/// 1. Ignore the key while a text input owns focus (search / keymap / terminal)
///    — otherwise typing the letter would also change the setting.
/// 2. Only act when [`settings_row_is_visible`] says the row survives the
///    header search filter.
/// 3. Pass the action straight to `on_settings_action`; no new handler needed.
///
/// Key matching is case-insensitive and trimmed; modified chords such as
/// `"alt+t"` are intentionally *not* recognised, leaving Alt+S and Alt+1..5
/// exclusively to the existing tab/sort handlers.
pub fn settings_row_shortcut(
    tab: SettingsTab,
    key: &str,
    current: SettingsRowValues<'_>,
) -> Option<SettingsUiAction> {
    settings_row_keywords(tab, key)?;
    let key = key.trim().to_lowercase();
    match (tab, key.as_str()) {
        (SettingsTab::Appearance, "t") => Some(SettingsUiAction::SetTheme(
            settings_theme_next(current.theme).to_string(),
        )),
        (SettingsTab::Appearance, "v") => Some(SettingsUiAction::SetView(
            settings_view_next(current.view).to_string(),
        )),
        (SettingsTab::Files, "h") => Some(SettingsUiAction::ToggleHidden(settings_hidden_next(
            current.hidden,
        ))),
        _ => None,
    }
}

/// Keyword group of the row addressed by `key` on `tab`, if any. Shared with
/// [`settings_row_is_visible`] so the shortcut and the rendered filter cannot
/// drift apart.
fn settings_row_keywords(tab: SettingsTab, key: &str) -> Option<&'static [&'static str]> {
    match (tab, key.trim().to_lowercase().as_str()) {
        (SettingsTab::Appearance, "t") => Some(APPEARANCE_THEME_KEYWORDS),
        (SettingsTab::Appearance, "v") => Some(APPEARANCE_VIEW_KEYWORDS),
        (SettingsTab::Files, "h") => Some(FILES_HIDDEN_KEYWORDS),
        _ => None,
    }
}

/// Whether the row addressed by `key` on `tab` is rendered under the current
/// header search filter. The host should require this before acting on a row
/// shortcut, so typing `t` cannot silently change a row hidden by the filter.
/// Unknown keys are never visible.
pub fn settings_row_is_visible(tab: SettingsTab, key: &str, query: &str) -> bool {
    settings_row_keywords(tab, key).is_some_and(|keywords| matches_query(query, keywords))
}

/// Next theme value for the 外観 `t` shortcut, matching the global Alt+Shift+T
/// order (Dark → Light → System → Dark). Unknown/empty values start at `dark`,
/// the same fallback as `Settings::theme_choice` deserialization.
pub fn settings_theme_next(choice: &str) -> &'static str {
    match choice {
        "dark" => "light",
        "light" => "system",
        _ => "dark",
    }
}

/// Next view value for the 外観 `v` shortcut, matching the `details` / `icons`
/// persisted values. Unknown/empty values start at `details`, the explorer's
/// default layout.
pub fn settings_view_next(choice: &str) -> &'static str {
    match choice {
        "details" => "icons",
        _ => "details",
    }
}

/// Next switch value for the ファイル `h` shortcut.
pub fn settings_hidden_next(current: bool) -> bool {
    !current
}

// ---------------------------------------------------------------------------
// Focus order (I05)
// ---------------------------------------------------------------------------

/// Keyboard-focusable controls of the settings card, in visual order.
///
/// gpui-component 0.9.1 has per-widget focus handles (including
/// `Button::track_focus`) but no generic `TabIndex` traversal across the
/// mixed controls used here, so the card owns an explicit order instead of
/// relying on the window's global tab stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsFocusTarget {
    /// Header search box (always rendered).
    Search,
    /// Shortcuts-tab keymap editor (visible only when its row matches).
    Keymap,
    /// Terminal-tab command editor (visible only when its row matches).
    Terminal,
    /// Footer discard button (always rendered).
    Cancel,
    /// Footer save button (always rendered).
    Save,
}

/// Focusable controls currently rendered, in tab order.
///
/// Pure so the cycle can be tested without a GPUI window. The active tab's
/// text editor is included only when its row survives the header search
/// filter; an empty query matches every row, mirroring [`matches_query`].
pub fn settings_focus_order(tab: SettingsTab, query: &str) -> Vec<SettingsFocusTarget> {
    let mut order = vec![SettingsFocusTarget::Search];
    let query = query.trim();
    match tab {
        SettingsTab::Shortcuts if matches_query(query, SHORTCUTS_KEYMAP_KEYWORDS) => {
            order.push(SettingsFocusTarget::Keymap);
        }
        SettingsTab::Terminal if matches_query(query, TERMINAL_COMMAND_KEYWORDS) => {
            order.push(SettingsFocusTarget::Terminal);
        }
        _ => {}
    }
    order.push(SettingsFocusTarget::Cancel);
    order.push(SettingsFocusTarget::Save);
    order
}

/// Next target after `current` in `order`, wrapping at both ends.
///
/// An unknown (or `None`) current target starts at the first entry, or the
/// last one when going backwards, so Tab always moves focus somewhere useful.
pub fn next_settings_focus(
    order: &[SettingsFocusTarget],
    current: Option<SettingsFocusTarget>,
    backwards: bool,
) -> Option<SettingsFocusTarget> {
    if order.is_empty() {
        return None;
    }
    let len = order.len();
    let index = current.and_then(|target| order.iter().position(|item| *item == target));
    let next = match index {
        Some(index) if backwards => (index + len - 1) % len,
        Some(index) => (index + 1) % len,
        None if backwards => len - 1,
        None => 0,
    };
    order.get(next).copied()
}

// ---------------------------------------------------------------------------
// Small building blocks
// ---------------------------------------------------------------------------

fn section_title(palette: &SettingsUiPalette, title: &str) -> impl IntoElement {
    div()
        .w_full()
        .pb_1()
        .text_sm()
        .font_weight(gpui::FontWeight::SEMIBOLD)
        .text_color(rgb(palette.accent))
        .child(title.to_string())
}

fn divider(palette: &SettingsUiPalette) -> impl IntoElement {
    div().w_full().h(px(1.0)).bg(rgb(palette.overlay)).my_2()
}

/// A label + description on the left and the control on the right, matching
/// the layout of KDE/GNOME preferences dialogs.
fn setting_row(
    palette: &SettingsUiPalette,
    label: &str,
    description: &str,
    control: impl IntoElement,
) -> impl IntoElement {
    div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .py_2()
        .child(
            div()
                .flex_1()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(palette.text))
                        .child(label.to_string()),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(palette.subtext))
                        .child(description.to_string()),
                ),
        )
        .child(control)
}

/// One static line of read-only information (used by Personalize / Shortcuts).
fn info_line(palette: &SettingsUiPalette, text: &str) -> impl IntoElement {
    div()
        .w_full()
        .py_1()
        .text_xs()
        .text_color(rgb(palette.subtext))
        .child(text.to_string())
}

/// Read-only version line shown on the Personalize info page (Q08).
///
/// Pure so the wording can be regression-tested without a GPUI window. The
/// `バージョン` keyword in `PERSONALIZE_INFO_KEYWORDS` advertises this row, so
/// the rendered text must actually contain it — otherwise a search for that
/// word highlights a section with nothing matching on screen.
fn personalize_version_info_line() -> String {
    format!(
        "バージョン: gpui-file-explorer v{}",
        env!("CARGO_PKG_VERSION")
    )
}

/// Read-only license line shown on the Personalize info page (S10).
///
/// F1 help is swallowed while the settings dialog owns the keyboard, so the
/// license row that otherwise exists only in `strings::HELP_SECTIONS` is
/// repeated here. Pure like [`personalize_version_info_line`] so a sync test
/// can prove the two copies cannot drift.
fn personalize_license_info_line() -> String {
    "ライセンス: AGPL-3.0-or-later".to_string()
}

/// Read-only source line shown on the Personalize info page (S10).
///
/// Uses the same URL as the F1 help entry in `strings::HELP_SECTIONS`; the sync
/// test compares them verbatim (like Q08 for the version row).
fn personalize_source_info_line() -> String {
    "ソース: https://github.com/Seduce0068/gpui-file-explorer".to_string()
}

/// Read-only application line shown on the Personalize info page (X14).
///
/// `情報` / `about` in [`PERSONALIZE_INFO_KEYWORDS`] previously pointed at a
/// page with no visible match (M09). The keywords now appear verbatim here.
fn personalize_app_info_line() -> &'static str {
    "gpui-file-explorer — GPUI 製のファイルマネージャー（情報 / about）"
}

/// Every read-only line rendered on the Personalize info page, in order.
///
/// Pure so both the renderer and the keyword-visibility test use the exact same
/// strings (X14). Duplicating the literals in the test would let the page drift
/// silently and reintroduce the M09 failure.
fn personalize_info_lines() -> Vec<String> {
    vec![
        personalize_app_info_line().to_string(),
        personalize_version_info_line(),
        personalize_license_info_line(),
        personalize_source_info_line(),
        "設定は ~/.config/gpui-file-explorer/settings.json に保存されます。".to_string(),
        "配色はテーマ設定に連動し、変更は保存時に反映されます。".to_string(),
    ]
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Render the centered settings card **content** only. The host wraps it in
/// the dimming backdrop and centers it in the window.
///
/// `theme_options`, `view_options` and `sort_options` are `(value, label)`
/// pairs; the host owns the concrete lists so it can localize them and map
/// values back to its own enums.
pub fn render_settings_content<V: SettingsUiHost>(
    palette: &SettingsUiPalette,
    state: &SettingsUiState,
    theme_options: &[(&str, &str)],
    view_options: &[(&str, &str)],
    sort_options: &[(&str, &str)],
    cx: &mut Context<V>,
) -> impl IntoElement {
    let query_value = state.search_input.read(cx).value();
    let query = query_value.trim();

    let tab_click = cx.listener(|this: &mut V, index: &usize, _window, cx| {
        this.on_settings_action(SettingsUiAction::Tab(SettingsTab::from_index(*index)), cx);
    });
    let save_click = cx.listener(|this: &mut V, _: &ClickEvent, _window, cx| {
        this.on_settings_action(SettingsUiAction::Save, cx);
    });
    let cancel_click = cx.listener(|this: &mut V, _: &ClickEvent, _window, cx| {
        this.on_settings_action(SettingsUiAction::Cancel, cx);
    });

    // Clone the option values into owned strings: `cx.listener` callbacks must
    // be 'static and cannot borrow the option slices.
    let theme_values: Vec<String> = theme_options
        .iter()
        .map(|(value, _)| (*value).to_string())
        .collect();
    let view_values: Vec<String> = view_options
        .iter()
        .map(|(value, _)| (*value).to_string())
        .collect();
    let sort_values: Vec<String> = sort_options
        .iter()
        .map(|(value, _)| (*value).to_string())
        .collect();

    let theme_radios = RadioGroup::new("settings-theme-group")
        .selected_index(Some(selected_index(theme_options, &state.theme_choice)))
        .w(px(260.0))
        .children(theme_options.iter().map(|(_, label)| (*label).to_string()))
        .on_click(
            cx.listener(move |this: &mut V, index: &usize, _window, cx| {
                if let Some(value) = theme_values.get(*index) {
                    this.on_settings_action(SettingsUiAction::SetTheme(value.clone()), cx);
                }
            }),
        );
    let view_radios = RadioGroup::new("settings-view-group")
        .selected_index(Some(selected_index(view_options, &state.view_choice)))
        .w(px(260.0))
        .children(view_options.iter().map(|(_, label)| (*label).to_string()))
        .on_click(
            cx.listener(move |this: &mut V, index: &usize, _window, cx| {
                if let Some(value) = view_values.get(*index) {
                    this.on_settings_action(SettingsUiAction::SetView(value.clone()), cx);
                }
            }),
        );
    let sort_radios = RadioGroup::new("settings-sort-group")
        .selected_index(Some(selected_index(sort_options, &state.sort_choice)))
        .w(px(260.0))
        .children(sort_options.iter().map(|(_, label)| (*label).to_string()))
        .on_click(
            cx.listener(move |this: &mut V, index: &usize, _window, cx| {
                if let Some(value) = sort_values.get(*index) {
                    this.on_settings_action(SettingsUiAction::SetSort(value.clone()), cx);
                }
            }),
        );
    let hidden_switch = Switch::new("settings-show-hidden")
        .checked(state.show_hidden)
        .on_click(cx.listener(|this: &mut V, checked: &bool, _window, cx| {
            this.on_settings_action(SettingsUiAction::ToggleHidden(*checked), cx);
        }));

    let show_appearance_theme = matches_query(query, APPEARANCE_THEME_KEYWORDS);
    let show_appearance_view = matches_query(query, APPEARANCE_VIEW_KEYWORDS);
    let show_files_hidden = matches_query(query, FILES_HIDDEN_KEYWORDS);
    let show_files_sort = matches_query(query, FILES_SORT_KEYWORDS);
    let show_shortcuts_keymap = matches_query(query, SHORTCUTS_KEYMAP_KEYWORDS);
    let show_shortcuts_defaults = matches_query(query, SHORTCUTS_DEFAULTS_KEYWORDS);
    let show_terminal_command = matches_query(query, TERMINAL_COMMAND_KEYWORDS);
    let show_personalize_palette = matches_query(query, PERSONALIZE_PALETTE_KEYWORDS);
    let show_personalize_info = matches_query(query, PERSONALIZE_INFO_KEYWORDS);

    // With a non-empty query, a tab with no matching rows would otherwise
    // render as a blank column. Work out which tabs do match so we can point
    // the user at them instead.
    let matching_tabs = tabs_matching_query(query);
    let current_tab_has_matches = matching_tabs.contains(&state.tab);

    let body = match state.tab {
        SettingsTab::Appearance => {
            let mut column = div().w_full().flex().flex_col().gap_2();
            if show_appearance_theme {
                column = column
                    .child(section_title(palette, "テーマ"))
                    .child(setting_row(
                        palette,
                        "配色",
                        APPEARANCE_THEME_DESCRIPTION,
                        theme_radios,
                    ));
            }
            if show_appearance_view {
                column = column
                    .child(divider(palette))
                    .child(section_title(palette, "表示レイアウト"))
                    .child(setting_row(
                        palette,
                        "既定のビュー",
                        APPEARANCE_VIEW_DESCRIPTION,
                        view_radios,
                    ));
            }
            column
        }
        SettingsTab::Files => {
            let mut column = div().w_full().flex().flex_col().gap_2();
            if show_files_hidden {
                column = column
                    .child(section_title(palette, "ファイルの表示"))
                    .child(setting_row(
                        palette,
                        "隠しファイルを表示",
                        FILES_HIDDEN_DESCRIPTION,
                        hidden_switch,
                    ));
            }
            if show_files_sort {
                column = column
                    .child(divider(palette))
                    .child(section_title(palette, "並び替え"))
                    .child(setting_row(
                        palette,
                        "既定の並び順",
                        FILES_SORT_DESCRIPTION,
                        sort_radios,
                    ));
            }
            column
        }
        SettingsTab::Shortcuts => {
            let mut column = div().w_full().flex().flex_col().gap_2();
            if show_shortcuts_keymap {
                column = column
                    .child(section_title(palette, "キーボードショートカット"))
                    .child(
                        div()
                            .w_full()
                            .text_xs()
                            .text_color(rgb(palette.subtext))
                            .child(SETTINGS_KEYMAP_INPUT_HINT),
                    )
                    .child(Input::new(&state.keymap_input).w_full());
            }
            if show_shortcuts_defaults {
                column = column
                    .child(divider(palette))
                    .child(section_title(palette, "主な既定のショートカット"));
                for line in SHORTCUTS_DEFAULT_LINES {
                    column = column.child(info_line(palette, line));
                }
            }
            column
        }
        SettingsTab::Terminal => {
            let mut column = div().w_full().flex().flex_col().gap_2();
            if show_terminal_command {
                column = column
                    .child(section_title(palette, "ターミナル"))
                    .child(
                        div()
                            .w_full()
                            .text_xs()
                            .text_color(rgb(palette.subtext))
                            .child("選択中のフォルダを作業ディレクトリとして起動するコマンド（空欄で無効化）"),
                    )
                    .child(Input::new(&state.terminal_input).w_full());
            }
            // K13: the "changes apply on Save" hint lives only in the shared
            // footer below. Rendering it here as well showed it twice on this
            // tab whenever the form was clean.
            column
        }
        SettingsTab::Personalize => {
            let mut column = div().w_full().flex().flex_col().gap_2();
            if show_personalize_palette {
                column = column
                    .child(section_title(palette, "カラーパレット"))
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap_3()
                            .py_2()
                            .child(
                                div()
                                    .w(px(28.0))
                                    .h(px(28.0))
                                    .rounded_md()
                                    .bg(rgb(palette.accent)),
                            )
                            .child(
                                div()
                                    .w(px(28.0))
                                    .h(px(28.0))
                                    .rounded_md()
                                    .bg(rgb(palette.green)),
                            )
                            .child(
                                div()
                                    .w(px(28.0))
                                    .h(px(28.0))
                                    .rounded_md()
                                    .bg(rgb(palette.red)),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(palette.subtext))
                                    .child("アクセント / 成功 / エラー"),
                            ),
                    );
            }
            if show_personalize_info {
                column = column
                    .child(divider(palette))
                    .child(section_title(palette, "このアプリについて"));
                for line in personalize_info_lines() {
                    column = column.child(info_line(palette, &line));
                }
            }
            column
        }
    };

    // Replace the blank column with a hint that names the tabs holding matches.
    let body = if !query.is_empty() && !current_tab_has_matches {
        div()
            .w_full()
            .py_6()
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(rgb(palette.subtext))
            .child(no_match_hint(&matching_tabs))
    } else {
        body
    };

    let header = div()
        .w_full()
        .flex()
        .items_center()
        .justify_between()
        .gap_4()
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xl()
                        .font_weight(gpui::FontWeight::BOLD)
                        .text_color(rgb(palette.text))
                        .child("設定"),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(rgb(palette.subtext))
                        .child("ファイルマネージャーの環境設定"),
                ),
        )
        .child(
            div()
                .w(px(240.0))
                .child(Input::new(&state.search_input).w_full()),
        );

    let tab_bar = TabBar::new("settings-tab-bar")
        .segmented()
        .selected_index(state.tab.index())
        .children(SettingsTab::ALL.iter().map(|tab| tab.label()))
        .on_click(tab_click)
        .w_full();

    let actions = div()
        .w_full()
        .flex()
        .items_center()
        .gap_2()
        .child(
            div()
                .flex_1()
                .text_xs()
                .text_color(rgb(if state.dirty {
                    palette.red
                } else {
                    palette.muted
                }))
                .child(footer_status_text(state.dirty)),
        )
        .child(
            Button::new("settings-cancel")
                .label(cancel_label(state.dirty))
                .track_focus(&state.cancel_focus)
                .on_click(cancel_click),
        )
        .child(
            Button::new("settings-save")
                .label("保存")
                .primary()
                .track_focus(&state.save_focus)
                .on_click(save_click),
        );

    // Validation / save errors live in one shared footer above the buttons so
    // they remain visible on every tab (E01). Rendering them here instead of in
    // a single tab also avoids a duplicate display on Terminal.
    let mut footer = div().w_full().flex().flex_col().gap_2();
    if let Some(error) = &state.error {
        footer = footer.child(
            div()
                .w_full()
                .text_xs()
                .text_color(rgb(palette.red))
                .child(error.clone()),
        );
    }
    // P10: F1 is intentionally swallowed while the dialog owns the keyboard,
    // so the footer always carries its own compact key legend (no search
    // filter or unsaved-state dependency).
    let footer = footer
        .child(
            div()
                .w_full()
                .text_xs()
                .text_color(rgb(palette.muted))
                .child(settings_footer_legend()),
        )
        .child(actions);

    div()
        .id("settings-card-scroll")
        .w(px(760.0))
        .max_h(relative(0.82))
        .overflow_y_scroll()
        .bg(rgb(palette.surface))
        .rounded_lg()
        .border_1()
        .border_color(rgb(palette.overlay))
        .shadow_lg()
        .p_5()
        .flex()
        .flex_col()
        .gap_4()
        .child(header)
        .child(tab_bar)
        .child(divider(palette))
        .child(body)
        .child(divider(palette))
        .child(footer)
}

// ---------------------------------------------------------------------------
// Tests (pure — no GPUI window required)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    // Explicit imports only: the module-level `use gpui_kit::*` would shadow the
    // built-in `#[test]` attribute if pulled in through `use super::*`.
    use super::{
        cancel_label, footer_status_text, matches_query, next_settings_focus, no_match_hint,
        personalize_info_lines, personalize_license_info_line, personalize_source_info_line,
        personalize_version_info_line, selected_index, settings_focus_order,
        settings_footer_legend, settings_hidden_next, settings_row_is_visible,
        settings_row_shortcut, settings_theme_next, settings_view_next, tabs_matching_query,
        SettingsFocusTarget, SettingsRowValues, SettingsTab, SettingsUiAction, SettingsUiPalette,
        APPEARANCE_THEME_DESCRIPTION, APPEARANCE_THEME_KEYWORDS, APPEARANCE_VIEW_DESCRIPTION,
        FILES_HIDDEN_DESCRIPTION, FILES_HIDDEN_KEYWORDS, FILES_SORT_DESCRIPTION,
        PERSONALIZE_INFO_KEYWORDS, PERSONALIZE_PALETTE_KEYWORDS, SETTINGS_KEYMAP_INPUT_HINT,
        SETTINGS_SEARCH_PLACEHOLDER, SHORTCUTS_DEFAULTS_KEYWORDS, SHORTCUTS_DEFAULT_LINES,
        TERMINAL_COMMAND_KEYWORDS,
    };
    use crate::strings::SETTINGS_INPUT_GUARD_NOTE;

    #[test]
    fn tab_labels_are_stable_and_localized() {
        assert_eq!(SettingsTab::Appearance.label(), "外観");
        assert_eq!(SettingsTab::Files.label(), "ファイル");
        assert_eq!(SettingsTab::Shortcuts.label(), "ショートカット");
        assert_eq!(SettingsTab::Terminal.label(), "ターミナル");
        assert_eq!(SettingsTab::Personalize.label(), "個人化");
    }

    #[test]
    fn tab_index_round_trips_through_from_index() {
        for (index, tab) in SettingsTab::ALL.iter().enumerate() {
            assert_eq!(tab.index(), index);
            assert_eq!(SettingsTab::from_index(index), *tab);
        }
        assert_eq!(SettingsTab::from_index(usize::MAX), SettingsTab::Appearance);
    }

    #[test]
    fn default_tab_is_appearance() {
        assert_eq!(SettingsTab::default(), SettingsTab::Appearance);
    }

    #[test]
    fn palettes_are_distinct_between_dark_and_light() {
        let dark = SettingsUiPalette::dark();
        let light = SettingsUiPalette::light();
        assert_ne!(dark, light);
        assert_ne!(dark.surface, light.surface);
        assert_ne!(dark.text, light.text);
        assert_ne!(dark.accent, light.accent);
        assert_ne!(dark.red, light.red);
    }

    #[test]
    fn palettes_use_the_expected_catppuccin_values() {
        let dark = SettingsUiPalette::dark();
        assert_eq!(dark.surface, 0x313244);
        assert_eq!(dark.overlay, 0x45475a);
        assert_eq!(dark.text, 0xcdd6f4);
        assert_eq!(dark.accent, 0x89b4fa);

        let light = SettingsUiPalette::light();
        assert_eq!(light.surface, 0xccd0da);
        assert_eq!(light.overlay, 0xbcc0cc);
        assert_eq!(light.text, 0x4c4f69);
        assert_eq!(light.muted, 0x6c6f85);
        assert_eq!(light.green, 0x40a02b);
        assert_eq!(light.red, 0xd20f39);
    }

    #[test]
    fn cancel_label_makes_unsaved_changes_explicit() {
        // Clean form keeps the canonical, neutral label (E13).
        assert_eq!(cancel_label(false), crate::strings::LABEL_CANCEL);
        assert_eq!(cancel_label(false), "キャンセル");
        // Dirty form names the consequence so closing is not mistaken for a
        // reversible cancel (E12).
        assert_eq!(cancel_label(true), "保存せず閉じる");
    }

    #[test]
    fn selected_index_falls_back_to_the_first_option() {
        let options = [("details", "一覧"), ("icons", "アイコン")];
        assert_eq!(selected_index(&options, "icons"), 1);
        assert_eq!(selected_index(&options, "details"), 0);
        assert_eq!(selected_index(&options, "unknown"), 0);
    }

    #[test]
    fn search_filter_is_case_insensitive_and_empty_matches_all() {
        assert!(matches_query("", &["テーマ"]));
        assert!(matches_query("  ", &["テーマ"]));
        assert!(matches_query("THEME", &["テーマ", "theme"]));
        assert!(matches_query("隠し", &["隠しファイル", "hidden"]));
        assert!(!matches_query("zzz", &["テーマ", "theme"]));
    }

    #[test]
    fn tab_actions_carry_the_selected_tab() {
        let action = SettingsUiAction::Tab(SettingsTab::Terminal);
        assert_eq!(action, SettingsUiAction::Tab(SettingsTab::Terminal));
        assert_ne!(action, SettingsUiAction::Cancel);
    }

    #[test]
    fn tabs_matching_query_finds_a_keyword_only_in_another_tab() {
        // "ターミナル" is exclusive to the Terminal tab, so a user searching it
        // while on Appearance must be told where the match lives.
        assert_eq!(
            tabs_matching_query("ターミナル"),
            vec![SettingsTab::Terminal]
        );
        // Case-insensitive English keywords behave the same way.
        assert_eq!(tabs_matching_query("TERMINAL"), vec![SettingsTab::Terminal]);
        // One keyword can span several tabs: "theme" is on both Appearance and
        // Personalize, and the result keeps display order.
        assert_eq!(
            tabs_matching_query("theme"),
            vec![SettingsTab::Appearance, SettingsTab::Personalize]
        );
    }

    #[test]
    fn tabs_matching_query_finds_persisted_setting_key_names() {
        // V07: the placeholder promises "キー名で検索", and the recovery hints
        // in strings.rs name the exact `settings.json` key. Searching that key
        // must land on the tab that actually renders it — not a zero-hit dead
        // end that forces the user to guess a Japanese label.
        let cases: &[(&str, SettingsTab)] = &[
            ("terminal_command", SettingsTab::Terminal),
            ("theme_choice", SettingsTab::Appearance),
            ("show_hidden", SettingsTab::Files),
        ];
        for (key, expected) in cases {
            assert_eq!(
                tabs_matching_query(key),
                vec![*expected],
                "persisted key {key} must surface {expected:?}"
            );
            // `matches_query` lowercases both sides, so uppercase input works.
            assert_eq!(
                tabs_matching_query(&key.to_uppercase()),
                vec![*expected],
                "uppercase key {key} must surface {expected:?}"
            );
        }
        // Guard the arrays themselves as well, so a future keyword rewrite
        // cannot drop the key names while the tests above still pass via some
        // unrelated substring match.
        assert!(APPEARANCE_THEME_KEYWORDS.contains(&"theme_choice"));
        assert!(FILES_HIDDEN_KEYWORDS.contains(&"show_hidden"));
        assert!(TERMINAL_COMMAND_KEYWORDS.contains(&"terminal_command"));
    }

    #[test]
    fn no_match_hint_points_at_the_matching_tab_and_alt_digits() {
        // K07/L05: a hit in another tab must be actionable from the keyboard,
        // so the hint names the tab *and its exact Alt+digit* — not a generic
        // "Alt+数字" that leaves the user guessing which key to press.
        assert_eq!(
            no_match_hint(&[SettingsTab::Terminal]),
            "該当する設定がありません。ターミナル（Alt+4）タブに一致があります"
        );
        assert_eq!(
            no_match_hint(&[SettingsTab::Appearance, SettingsTab::Personalize]),
            "該当する設定がありません。外観（Alt+1）、個人化（Alt+5）タブに一致があります"
        );
        // Every tab advertises the same digit the real key handler accepts, so
        // the hint can never drift from the F1 help / README table.
        for tab in SettingsTab::ALL {
            let hint = no_match_hint(&[tab]);
            assert!(
                hint.contains(&format!("{}（Alt+{}）", tab.label(), tab.index() + 1)),
                "hint for {tab:?}: {hint}"
            );
            assert!(!hint.contains("Alt+数字"), "generic hint: {hint}");
        }
        // With no hits at all there is no page to jump to, so M14 requires a
        // recovery route instead: clear the query or close the dialog.
        let none = no_match_hint(&[]);
        assert_eq!(
            none,
            "該当する設定がありません。検索欄を空にするか Esc で閉じてください"
        );
        assert!(none.contains("検索欄を空にする"));
        assert!(none.contains("Esc で閉じてください"));
        assert!(!none.contains("Alt+"));
    }

    #[test]
    fn save_status_hint_is_the_footers_single_source() {
        // K13: the shared footer is the only renderer of the "apply on Save"
        // hint; the Terminal tab body must not repeat it.
        assert_eq!(
            footer_status_text(false),
            crate::strings::STATUS_SETTINGS_UNSAVED_HINT
        );
        assert_eq!(footer_status_text(true), "未保存の変更があります");
        assert_ne!(footer_status_text(true), footer_status_text(false));
    }

    #[test]
    fn settings_footer_legend_lists_dialog_keys_without_f1() {
        // P10: F1 is blocked while the dialog owns the keyboard. The footer
        // legend must therefore name the keys that would otherwise be looked
        // up in the F1 overlay, and stay short enough for the card width.
        let legend = settings_footer_legend();
        for token in [
            "Enter 保存",
            "Esc 閉じる",
            "Tab/Shift+Tab 移動",
            "Ctrl+Tab",
            "Ctrl+Shift+Tab",
            concat!(crate::strings::settings_tab_jump_keys!(), " 直接移動"),
        ] {
            assert!(legend.contains(token), "legend: {legend}");
        }
        assert!(
            !legend.contains("F1"),
            "F1 is blocked in the dialog: {legend}"
        );
        // Q05: naming both directions lengthens the line, but it must still fit
        // the 760px card (one compact line next to the save/cancel buttons).
        assert!(
            legend.chars().count() < 96,
            "legend must stay compact: {legend}"
        );
    }

    #[test]
    fn direct_jump_keys_are_synced_across_legend_f1_and_readme() {
        // Y05: the footer legend, the F1 direct-jump help row and app/README
        // must spell the settings-tab jump keys with one canonical wording.
        let keys = crate::strings::SETTINGS_TAB_JUMP_KEYS;
        assert_eq!(keys, "Alt+1〜5");
        let legend = settings_footer_legend();
        assert!(
            legend.contains(&format!("{keys} 直接移動")),
            "footer legend must use the canonical keys: {legend}"
        );
        let direct = crate::strings::HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| rows.iter().copied())
            .find(|(key, _)| *key == "設定ダイアログのタブ（直接）")
            .map(|(_, text)| text)
            .expect("F1 help must list the direct-jump keys");
        assert!(
            direct.starts_with(keys),
            "F1 direct-jump row must use the canonical keys: {direct}"
        );
        let app_readme = include_str!("../README.md");
        assert!(
            app_readme.contains(keys),
            "app README must use the canonical direct-jump keys {keys}"
        );
        // The Shortcuts tab renders the same notation as a searchable row.
        assert!(
            SHORTCUTS_DEFAULT_LINES
                .iter()
                .any(|line| line.starts_with(keys)),
            "Shortcuts tab must render the canonical direct-jump keys"
        );
    }

    #[test]
    fn f1_dialog_help_names_cross_tab_search_navigation() {
        // Y11: README documents that a no-match hint points at the matching
        // tab and its direct-jump key; the F1 dialog row must say the same.
        let dialog = crate::strings::HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| rows.iter().copied())
            .find(|(key, _)| *key == "設定ダイアログ")
            .map(|(_, text)| text)
            .expect("F1 help must include the settings dialog row");
        assert!(
            dialog.contains("他タブに一致すると Alt+数字で移動"),
            "F1 dialog help must explain cross-tab navigation: {dialog}"
        );
        assert!(
            dialog.contains(crate::strings::SETTINGS_TAB_JUMP_KEYS),
            "F1 dialog help must name the concrete jump keys: {dialog}"
        );
        // Z08: a zero-hit search is a dead end unless the help also names the
        // two recovery routes already used by `no_match_hint` and app/README.
        assert!(
            dialog.contains("検索欄を空にするか Esc で閉じる"),
            "F1 dialog help must explain zero-hit recovery: {dialog}"
        );
        let app_readme = include_str!("../README.md");
        assert!(
            app_readme.contains("検索欄を空にするか `Esc` で閉じるよう案内します"),
            "app README must document the same zero-hit recovery"
        );
    }

    #[test]
    fn f1_keymap_help_explains_unbind_and_dialog_keys() {
        // Y12: the F1 Alt+K row must match README's keymap-dialog contract —
        // unbind syntax plus Enter/Esc — not just the input format.
        let keymap = crate::strings::HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| rows.iter().copied())
            .find(|(key, _)| *key == "Alt+K")
            .map(|(_, text)| text)
            .expect("F1 help must include the Alt+K row");
        for token in [
            "unbind アクション名で解除",
            "Enter で保存",
            "Esc でキャンセル",
        ] {
            assert!(
                keymap.contains(token),
                "F1 Alt+K help must mention {token}: {keymap}"
            );
        }
    }

    #[test]
    fn settings_search_placeholder_advertises_key_name_search() {
        // R07: chord names such as Alt+K are searchable, so the placeholder
        // must say so rather than promising only setting-label search.
        assert_eq!(SETTINGS_SEARCH_PLACEHOLDER, "設定・キー名で検索…");
        assert!(SETTINGS_SEARCH_PLACEHOLDER.contains("キー名"));
    }

    #[test]
    fn trash_hidden_only_hint_must_not_offer_a_rejected_chord() {
        // V13/Y06: use the canonical `strings.rs` constant, not a test-local
        // copy, so this contract cannot silently drift from the rendered hint.
        let generic = crate::strings::EMPTY_HIDDEN_ONLY;
        assert!(
            generic.contains("Ctrl+H で表示"),
            "generic hint must still offer the chord outside Trash: {generic}"
        );
        let trash = crate::strings::EMPTY_HIDDEN_ONLY_TRASH;
        // The unusable chord is only suggested after leaving Trash.
        assert!(
            trash.contains("ゴミ箱を出て"),
            "Trash hint must name the exit route: {trash}"
        );
        assert!(
            !trash.contains("（Ctrl+H で表示）"),
            "Trash hint must not offer the chord unconditionally: {trash}"
        );
        assert!(
            trash.contains("Ctrl+H"),
            "Trash hint must keep the chord: {trash}"
        );
        // Keep the `{count}` substitution contract of `empty_hidden_only`.
        assert!(
            trash.contains("{count}"),
            "Trash hint must keep the count placeholder: {trash}"
        );
        assert_eq!(
            trash.replace("{count}", &3.to_string()),
            "隠しファイル 3 件が非表示です（ゴミ箱を出て Ctrl+H で表示）"
        );
        assert_eq!(
            crate::strings::empty_hidden_only_trash(3),
            "隠しファイル 3 件が非表示です（ゴミ箱を出て Ctrl+H で表示）"
        );
        assert_ne!(trash, generic);
    }

    #[test]
    fn search_empty_hint_must_not_offer_a_rejected_chord() {
        // Z01: the same failure mode as V13 (Trash Ctrl+H), now for Search.
        // Read the real allowlists so a future change to either one breaks the
        // test instead of leaving a hint that points at a dead key.
        assert!(!crate::search_view_key_allowed("f", true, false));
        assert!(!crate::search_view_action_allowed(
            crate::KeyAction::OpenSearch
        ));
        // The keys the rewritten hint promises must actually reach handlers.
        assert!(crate::search_view_key_allowed("enter", false, false));

        let (_, hint) = crate::strings::EMPTY_SEARCH;
        assert!(
            !hint.contains("Ctrl+F") && !hint.contains("ctrl+f"),
            "Search hint must not promise the dead Ctrl+F chord: {hint}"
        );
        for token in ["検索語", "Enter", "Esc"] {
            assert!(
                hint.contains(token),
                "Search hint must name a reachable key: {hint}"
            );
        }
    }

    #[test]
    fn settings_keymap_hint_explains_replace_and_unbind_route() {
        // H04: the shared field looks like an additive editor, so the hint must
        // say Save replaces everything. K04: it must also advertise the unbind
        // syntax this very field accepts, not point users to Alt+K only.
        assert!(SETTINGS_KEYMAP_INPUT_HINT.contains("「操作=キー」形式で入力します"));
        assert!(SETTINGS_KEYMAP_INPUT_HINT.contains("; で区切ります"));
        assert!(SETTINGS_KEYMAP_INPUT_HINT.contains("置き換わります（一部追加ではありません）"));
        assert!(
            SETTINGS_KEYMAP_INPUT_HINT.contains("この欄でも `unbind アクション名` で解除できます")
        );
        assert!(!SETTINGS_KEYMAP_INPUT_HINT.contains("Alt+K の unbind を使用してください"));
        // M04: the reserved-action notice from `strings::KEYMAP_INPUT_HINT`
        // must be present here too, including the fact that unbind cannot
        // release Ctrl+, — otherwise this editor promises something the save
        // handler will reject.
        assert!(SETTINGS_KEYMAP_INPUT_HINT.contains("open_settings は Ctrl+, 固定で変更できません"));
        assert!(SETTINGS_KEYMAP_INPUT_HINT.contains("unbind も不可"));
        assert!(SETTINGS_KEYMAP_INPUT_HINT
            .contains("open_settings は Ctrl+, 固定で変更できません（unbind も不可）"));
    }

    #[test]
    fn keymap_hint_examples_are_searchable() {
        // AA05/M09: the example printed in SETTINGS_KEYMAP_INPUT_HINT must not
        // dead-end in the settings search. Every token has to surface the
        // Shortcuts tab so the wording users copy from the hint finds a row.
        const EXAMPLES: &[&str] = &["open_preview", "ctrl+shift+p", "quit", "ctrl+q"];
        let hint_lower = SETTINGS_KEYMAP_INPUT_HINT.to_lowercase();
        for example in EXAMPLES {
            assert!(
                hint_lower.contains(example),
                "hint must still show the example {example}"
            );
            assert!(
                tabs_matching_query(example).contains(&SettingsTab::Shortcuts),
                "keymap hint example {example} must surface the Shortcuts tab"
            );
        }
    }

    #[test]
    fn reserved_keys_note_and_keymap_cap_are_synced_across_hints_and_readme() {
        // Z07: one canonical reserved-key list drives the F1 Alt+K row, both
        // keymap hints and both READMEs; a drift in any one fails here.
        let keys = crate::strings::RESERVED_CHORD_KEYS;
        let note = crate::strings::RESERVED_KEYS_NOTE;
        assert_eq!(keys, &["escape", "f1", ","]);
        for key in keys {
            assert!(note.contains(key), "reserved note must list {key}: {note}");
        }
        let alt_k = crate::strings::HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| rows.iter().copied())
            .find(|(key, _)| *key == "Alt+K")
            .map(|(_, text)| text)
            .expect("F1 help must include the Alt+K row");
        for (name, text) in [
            ("F1 Alt+K", alt_k),
            ("Alt+K dialog", crate::strings::KEYMAP_INPUT_HINT),
            ("Shortcuts tab", SETTINGS_KEYMAP_INPUT_HINT),
        ] {
            assert!(text.contains(note), "{name} must use the canonical note");
        }
        // Both READMEs must carry the same three keys in one notation.
        let readme_label = format!("`{}`", keys.join("` / `"));
        for (name, text) in [
            ("README.md", include_str!("../../README.md")),
            ("app/README.md", include_str!("../README.md")),
        ] {
            assert!(
                text.contains(&readme_label),
                "{name} must list the canonical reserved keys {readme_label}"
            );
        }

        // Z12: the visible "最大 64 件まで" must track the real runtime cap.
        let cap = format!("最大 {} 件まで", crate::settings::MAX_KEYMAP_OVERRIDES);
        for (name, text) in [
            ("F1 Alt+K", alt_k),
            ("Alt+K dialog", crate::strings::KEYMAP_INPUT_HINT),
            ("Shortcuts tab", SETTINGS_KEYMAP_INPUT_HINT),
        ] {
            assert!(
                text.contains(&cap),
                "{name} must advertise the real keymap cap: {cap}"
            );
        }
    }

    #[test]
    fn tabs_matching_query_finds_shortcut_key_names() {
        // H09: chord names are the way users refer to shortcuts, so every one
        // displayed on the Shortcuts tab must surface that tab on its own.
        for key in [
            "Alt+K",
            "Alt+T",
            // R12: the theme chord is part of the default-shortcut list too.
            "Alt+Shift+T",
            // L03: settings-dialog tab/direct-jump chords and the wording used
            // by the no-match hint must be searchable as well.
            "Alt+1",
            "Alt+2",
            "Alt+3",
            "Alt+4",
            "Alt+5",
            "Alt+数字",
            "Ctrl+,",
            "Ctrl+F",
            "Ctrl+Shift+F",
            "Ctrl+L",
            "F1",
            "F4",
            "Ctrl+Shift+L",
            "Ctrl+Space",
            "Ctrl+Shift+C",
            "Ctrl+Tab",
            "Ctrl+Shift+Tab",
        ] {
            assert_eq!(
                tabs_matching_query(key),
                vec![SettingsTab::Shortcuts],
                "query {key} must surface the Shortcuts tab"
            );
            // Lowercase input is normalized by `matches_query`.
            assert_eq!(
                tabs_matching_query(&key.to_lowercase()),
                vec![SettingsTab::Shortcuts],
                "lowercase query {key} must surface the Shortcuts tab"
            );
        }
        // Alt+S names the Files-tab sort row and is also one of the default
        // shortcuts, so both pages surface in display order (L03/L07).
        assert_eq!(
            tabs_matching_query("Alt+S"),
            vec![SettingsTab::Files, SettingsTab::Shortcuts]
        );
        assert_eq!(
            tabs_matching_query("alt+s"),
            vec![SettingsTab::Files, SettingsTab::Shortcuts]
        );
    }

    #[test]
    fn tabs_matching_query_finds_visible_caption_and_description_words() {
        // M09: searching the wording actually rendered on a row must reveal
        // that row's tab. Each word below is copied from the row markup in
        // `render_settings_content`, not from an internal option value.
        let cases: &[(&str, SettingsTab)] = &[
            ("明るさ", SettingsTab::Appearance),
            ("詳細一覧", SettingsTab::Appearance),
            ("アイコン", SettingsTab::Appearance),
            ("既定のビュー", SettingsTab::Appearance),
            ("ドット", SettingsTab::Files),
            // Y04: the sort radio labels come from `SortKey::label()` in the
            // host's `sort_options`; searching what is visible must not dead-end.
            ("名前", SettingsTab::Files),
            ("種類", SettingsTab::Files),
            ("サイズ", SettingsTab::Files),
            ("更新日時", SettingsTab::Files),
            ("昇順", SettingsTab::Files),
            ("降順", SettingsTab::Files),
            ("作業ディレクトリ", SettingsTab::Terminal),
            ("無効化", SettingsTab::Terminal),
            ("unbind", SettingsTab::Shortcuts),
            ("直接移動", SettingsTab::Shortcuts),
            ("カラーパレット", SettingsTab::Personalize),
            ("settings.json", SettingsTab::Personalize),
        ];
        for (query, expected) in cases {
            assert_eq!(
                tabs_matching_query(query),
                vec![*expected],
                "visible wording {query} must surface {expected:?}"
            );
            assert_eq!(
                tabs_matching_query(&query.to_lowercase()),
                vec![*expected],
                "lowercase wording {query} must surface {expected:?}"
            );
        }
        // The display form shown in the default list is searchable verbatim,
        // alongside each individual Alt+digit chord (L03/M09).
        assert_eq!(
            tabs_matching_query("Alt+1〜5"),
            vec![SettingsTab::Shortcuts]
        );
    }

    #[test]
    fn personalize_version_line_matches_the_version_keyword() {
        // Q08: PERSONALIZE_INFO_KEYWORDS advertises "バージョン", so the info
        // page must actually render a version row carrying that word.
        assert!(PERSONALIZE_INFO_KEYWORDS.contains(&"バージョン"));
        let line = personalize_version_info_line();
        assert!(line.contains("バージョン"), "version line: {line}");
        assert!(
            line.contains(env!("CARGO_PKG_VERSION")),
            "version line must embed CARGO_PKG_VERSION: {line}"
        );
        assert!(matches_query("バージョン", PERSONALIZE_INFO_KEYWORDS));
    }

    #[test]
    fn personalize_license_and_source_lines_match_f1_help() {
        // S10: F1 is swallowed while the dialog owns the keyboard, so the
        // license/source rows that otherwise live only in F1 help must be
        // repeated on the Personalize info page. Compare with HELP_SECTIONS
        // so the two copies cannot drift (same guard as Q08 for the version).
        let license = personalize_license_info_line();
        let source = personalize_source_info_line();
        let help_rows: Vec<(&str, &str)> = crate::strings::HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| rows.iter().copied())
            .collect();
        // The F1 help stores the license as the row key and the source line as
        // its value ("ライセンス / ソース" section).
        let (help_license_key, help_source_line) = help_rows
            .iter()
            .find(|(key, value)| key.starts_with("AGPL") && value.starts_with("ソース: https://"))
            .expect("F1 help must include the license/source row");
        assert_eq!(
            license,
            format!("ライセンス: {help_license_key}"),
            "Personalize license must match F1 help: {help_license_key}"
        );
        assert_eq!(
            source, *help_source_line,
            "Personalize source must match F1 help: {help_source_line}"
        );
        // The visible words are advertised by the Personalize keywords so a
        // search lands on the page that actually renders them (M09/Q08).
        for word in ["ライセンス", "AGPL", "ソース"] {
            assert!(PERSONALIZE_INFO_KEYWORDS.contains(&word));
            assert!(matches_query(word, PERSONALIZE_INFO_KEYWORDS));
        }
        assert!(license.contains("ライセンス") && license.contains("AGPL"));
        assert!(source.contains("ソース") && source.contains("https://"));
    }

    #[test]
    fn personalize_keywords_point_at_visible_text_on_the_page() {
        // X14/M09: a keyword that has no visible match can make the no-match
        // hint point at a tab whose rows show nothing relevant. Build the
        // visible text from the same pure helper the renderer uses so the two
        // cannot drift. `theme` is a search-only alias of the Japanese label
        // 「テーマ」 and is the sole explicit exception.
        const SEARCH_ONLY_ALIASES: &[&str] = &["theme"];
        let visible = [
            SettingsTab::Personalize.label(),
            "カラーパレット",
            "アクセント / 成功 / エラー",
            "このアプリについて",
        ]
        .join("\n");
        let visible = format!("{visible}\n{}", personalize_info_lines().join("\n"));
        let visible_lower = visible.to_lowercase();
        for keyword in PERSONALIZE_INFO_KEYWORDS
            .iter()
            .chain(PERSONALIZE_PALETTE_KEYWORDS.iter())
        {
            if SEARCH_ONLY_ALIASES.contains(keyword) {
                continue;
            }
            assert!(
                visible_lower.contains(&keyword.to_lowercase()),
                "keyword {keyword} must be visible on the Personalize tab: {visible}"
            );
            assert!(
                matches_query(keyword, PERSONALIZE_INFO_KEYWORDS)
                    || matches_query(keyword, PERSONALIZE_PALETTE_KEYWORDS),
                "keyword {keyword} must be searchable"
            );
        }
        // The two keywords that motivated X14 are now rendered verbatim.
        let app_line = personalize_info_lines()[0].clone();
        assert!(app_line.contains("情報") && app_line.contains("about"));
    }

    #[test]
    fn tabs_matching_query_finds_visible_表示_on_the_files_tab() {
        // N07: "ファイルの表示" (section) and "隠しファイルを表示" (label) are
        // rendered on the Files page, so searching that word must surface it.
        // Appearance also matches through its own "表示レイアウト" section title.
        let matched = tabs_matching_query("表示");
        assert!(
            matched.contains(&SettingsTab::Files),
            "Files must surface for its visible 表示 wording: {matched:?}"
        );
        assert_eq!(matched, vec![SettingsTab::Appearance, SettingsTab::Files]);
    }

    #[test]
    fn row_descriptions_advertise_their_bare_letter_shortcuts() {
        // N04: F1 is blocked inside the settings dialog, so each row still
        // names the key that changes it, just like the Alt+S row — even though
        // t/v/h are also listed in the F1 help (strings.rs).
        for (description, key) in [
            (APPEARANCE_THEME_DESCRIPTION, "t"),
            (APPEARANCE_VIEW_DESCRIPTION, "v"),
            (FILES_HIDDEN_DESCRIPTION, "h"),
        ] {
            assert!(
                description.contains(&format!("（{key} でキーボードから切替）")),
                "row description must advertise {key}: {description}"
            );
        }
        assert!(FILES_SORT_DESCRIPTION.contains("Alt+S でキーボードから切替"));
    }

    #[test]
    fn typing_guard_note_is_synced_across_row_hints_help_and_readme() {
        // W07/X13: Alt+S / t / v / h are ignored while a settings text input
        // owns the keystroke. F1 is blocked inside the dialog, so every inline
        // row hint, both related F1 help rows, the Shortcuts-tab Alt+S row and
        // both READMEs must use the one canonical wording.
        let note = SETTINGS_INPUT_GUARD_NOTE;
        for description in [
            APPEARANCE_THEME_DESCRIPTION,
            APPEARANCE_VIEW_DESCRIPTION,
            FILES_HIDDEN_DESCRIPTION,
            FILES_SORT_DESCRIPTION,
        ] {
            assert!(
                description.contains(note),
                "row description must explain the typing guard: {description}"
            );
        }
        let alt_s_row = SHORTCUTS_DEFAULT_LINES
            .iter()
            .find(|line| line.contains("Alt+S …"))
            .expect("Shortcuts tab must render the Alt+S row");
        assert!(
            alt_s_row.contains(note),
            "Shortcuts Alt+S row must explain the typing guard: {alt_s_row}"
        );

        let help_rows: Vec<(&str, &str)> = crate::strings::HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| rows.iter().copied())
            .collect();
        for (key, expected_prefix) in [
            ("設定ダイアログの並び順", "ファイルタブで Alt+S"),
            ("設定ダイアログの行ショートカット", "外観タブで t=配色"),
        ] {
            let text = help_rows
                .iter()
                .find(|(row_key, _)| *row_key == key)
                .map(|(_, text)| *text)
                .unwrap_or_else(|| panic!("F1 help must include {key} row"));
            assert!(
                text.starts_with(expected_prefix) && text.ends_with(note),
                "F1 {key} must end with the canonical typing-guard note: {text}"
            );
        }
        // The row-shortcut note must keep the separate "no modifier" hint.
        let row_shortcut_help = help_rows
            .iter()
            .find(|(key, _)| *key == "設定ダイアログの行ショートカット")
            .map(|(_, text)| *text)
            .expect("row-shortcut help row exists");
        assert!(row_shortcut_help.contains("（修飾キーなし）"));

        let app_readme = include_str!("../README.md");
        let sort_line = app_readme
            .lines()
            .find(|line| line.contains("設定ダイアログの並び順"))
            .expect("README must document the settings sort key");
        assert!(
            sort_line.contains(note),
            "README Alt+S row must explain the typing guard: {sort_line}"
        );
        let body_paragraph = app_readme
            .split("\n\n")
            .find(|paragraph| paragraph.contains("同じ理由で、テーマは外観タブの"))
            .expect("README must explain the bare-letter row shortcuts")
            .replace('\n', "");
        assert!(
            body_paragraph.contains(note),
            "README row-shortcut body must use the canonical note: {body_paragraph}"
        );
        let root_readme = include_str!("../../README.md");
        let root_paragraph = root_readme
            .split("\n\n")
            .find(|paragraph| paragraph.contains("外観タブは `t`（テーマ）"))
            .expect("root README must explain the bare-letter row shortcuts")
            .replace('\n', "");
        assert!(
            root_paragraph.contains(note),
            "root README must use the canonical note: {root_paragraph}"
        );
    }

    #[test]
    fn f1_help_notes_directory_view_only_keys() {
        // W09: F1 help lists directory-only chords without repeating the
        // constraint on each row, so the その他 section carries one short
        // common note. It is a prose row (no chord), hence HELP_KEY_EXCEPTIONS.
        const NOTE: &str = "一部のキーはディレクトリ表示でのみ有効です";
        const KEY: &str = "ディレクトリ表示";
        let other = crate::strings::HELP_SECTIONS
            .iter()
            .find(|(title, _)| *title == "その他")
            .expect("その他 section exists");
        assert!(
            other
                .1
                .iter()
                .any(|(key, text)| *key == KEY && text.contains(NOTE)),
            "その他 must explain the directory-view constraint"
        );
        assert!(
            HELP_KEY_EXCEPTIONS.contains(&KEY),
            "the prose note must be exempt from the chord mirror"
        );
    }

    #[test]
    fn settings_row_shortcut_maps_bare_keys_per_tab() {
        // M08: RadioGroup/Switch rows have no Tab stop and the settings guard
        // swallows the global chords, so each row gets a bare letter scoped to
        // its own page. The helper reuses the ordinary actions already handled
        // by `main.rs`, so no new match arm is required there.
        let current = SettingsRowValues {
            theme: "dark",
            view: "details",
            hidden: false,
        };
        assert_eq!(
            settings_row_shortcut(SettingsTab::Appearance, "t", current),
            Some(SettingsUiAction::SetTheme("light".to_string()))
        );
        assert_eq!(
            settings_row_shortcut(SettingsTab::Appearance, "v", current),
            Some(SettingsUiAction::SetView("icons".to_string()))
        );
        assert_eq!(
            settings_row_shortcut(SettingsTab::Files, "h", current),
            Some(SettingsUiAction::ToggleHidden(true))
        );
        // Upper-case and surrounding whitespace behave like the lower-case key.
        assert_eq!(
            settings_row_shortcut(SettingsTab::Appearance, " T ", current),
            Some(SettingsUiAction::SetTheme("light".to_string()))
        );

        // The returned value follows the current state, not a fixed default.
        let other = SettingsRowValues {
            theme: "system",
            view: "icons",
            hidden: true,
        };
        assert_eq!(
            settings_row_shortcut(SettingsTab::Appearance, "t", other),
            Some(SettingsUiAction::SetTheme("dark".to_string()))
        );
        assert_eq!(
            settings_row_shortcut(SettingsTab::Appearance, "v", other),
            Some(SettingsUiAction::SetView("details".to_string()))
        );
        assert_eq!(
            settings_row_shortcut(SettingsTab::Files, "h", other),
            Some(SettingsUiAction::ToggleHidden(false))
        );
    }

    #[test]
    fn settings_row_shortcut_ignores_other_pages_and_modified_chords() {
        let current = SettingsRowValues {
            theme: "dark",
            view: "details",
            hidden: false,
        };
        for (tab, key) in [
            (SettingsTab::Appearance, "h"),
            (SettingsTab::Files, "t"),
            (SettingsTab::Files, "v"),
            (SettingsTab::Shortcuts, "t"),
            (SettingsTab::Terminal, "h"),
            (SettingsTab::Personalize, "h"),
            // Alt+1..5 and Alt+S belong to the existing tab/sort handlers.
            (SettingsTab::Appearance, "alt+t"),
            (SettingsTab::Files, "alt+h"),
            (SettingsTab::Appearance, "1"),
            (SettingsTab::Files, "s"),
            (SettingsTab::Appearance, ""),
            (SettingsTab::Appearance, "tt"),
        ] {
            assert_eq!(
                settings_row_shortcut(tab, key, current),
                None,
                "{tab:?} + {key:?} must not claim a shortcut"
            );
        }
    }

    #[test]
    fn settings_row_is_visible_tracks_the_search_filter() {
        // An empty query renders every row; a filter that misses the row's
        // keyword group hides it, and the host must ignore the shortcut then.
        assert!(settings_row_is_visible(SettingsTab::Appearance, "t", ""));
        assert!(settings_row_is_visible(SettingsTab::Appearance, "v", "  "));
        assert!(settings_row_is_visible(SettingsTab::Files, "h", "隠し"));
        assert!(!settings_row_is_visible(
            SettingsTab::Appearance,
            "t",
            "隠し"
        ));
        assert!(!settings_row_is_visible(SettingsTab::Files, "h", "テーマ"));
        assert!(!settings_row_is_visible(SettingsTab::Appearance, "s", ""));
        assert!(!settings_row_is_visible(SettingsTab::Files, "T", ""));
    }

    #[test]
    fn settings_next_value_helpers_cycle_and_wrap() {
        // These helpers keep the value resolution pure so the row shortcut can
        // return the existing SetTheme/SetView/ToggleHidden actions directly.
        assert_eq!(settings_theme_next("dark"), "light");
        assert_eq!(settings_theme_next("light"), "system");
        assert_eq!(settings_theme_next("system"), "dark");
        assert_eq!(settings_theme_next(""), "dark");
        assert_eq!(settings_theme_next("unknown"), "dark");

        assert_eq!(settings_view_next("details"), "icons");
        assert_eq!(settings_view_next("icons"), "details");
        assert_eq!(settings_view_next(""), "details");
        assert_eq!(settings_view_next("unknown"), "details");

        assert!(settings_hidden_next(false));
        assert!(!settings_hidden_next(true));
    }

    #[test]
    fn default_shortcut_list_matches_f1_help_and_search_keywords() {
        // L07: the Shortcuts tab must advertise the same keys as F1 help and
        // README. Every literal chord shown here is searchable via the
        // defaults keyword group, and the Alt+1〜5 display form expands to the
        // five individually searchable chords.
        for key in [
            "F1",
            "Ctrl+L",
            "Ctrl+F",
            "Ctrl+Shift+F",
            "F4",
            "Alt+T",
            "Alt+Shift+T",
            "Ctrl+,",
            "Alt+S",
        ] {
            assert!(
                SHORTCUTS_DEFAULT_LINES
                    .iter()
                    .any(|line| line.contains(key)),
                "default shortcut list must show {key}"
            );
            assert!(
                SHORTCUTS_DEFAULTS_KEYWORDS.contains(&key),
                "default shortcut keywords must contain {key}"
            );
        }
        assert!(SHORTCUTS_DEFAULT_LINES
            .iter()
            .any(|line| line.contains("Alt+1〜5")));
        for digit in 1..=5 {
            let key = format!("Alt+{digit}");
            assert!(
                SHORTCUTS_DEFAULTS_KEYWORDS.contains(&key.as_str()),
                "default shortcut keywords must contain {key}"
            );
        }
    }

    /// F1 help rows with no keyboard chord to render on the Shortcuts tab:
    /// mouse gestures, prose notes, dialog-only guidance (F1 is swallowed by
    /// the settings dialog, so those are surfaced in the dialog footer) and
    /// the license/source metadata row (T06).
    const HELP_KEY_EXCEPTIONS: &[&str] = &[
        "Ctrl+クリック / Shift+クリック",
        "ゴミ箱ビュー",
        "ドラッグ&ドロップ",
        "ミドルクリック",
        "設定ダイアログ",
        "設定ダイアログのタブ",
        "設定ダイアログのタブ（直接）",
        "設定ダイアログの並び順",
        "設定ダイアログの行ショートカット",
        "設定ダイアログの操作",
        "ディレクトリ表示",
        "AGPL-3.0-or-later",
    ];

    /// Rows shown on the Shortcuts tab whose chord appears in an F1 help
    /// *description* (the dialog guidance) rather than as a help key (T06).
    const SETTINGS_ONLY_KEYS: &[&str] = &["Alt+1〜5", "Alt+S"];

    /// Search-only synonyms that are not literal chord rows. `default` is the
    /// English alias of "既定"; `Alt+数字` is the generic wording of the
    /// `Alt+1〜5` row. Everything else here is visible in the section title
    /// or in a row description (T07).
    const SEARCH_ALIAS_KEYWORDS: &[&str] = &[
        "既定",
        "default",
        "ショートカット",
        "キー",
        "主な",
        "タブ",
        "直接移動",
        "Alt+数字",
        "ヘルプ",
        "キー一覧",
        "場所",
        "設定を開く",
    ];

    /// The `Alt+1〜5` display row expands to these five searchable chords;
    /// the digits are not literal substrings of that row (T07/M09).
    const ALT_DIGIT_KEYWORDS: &[&str] = &["Alt+1", "Alt+2", "Alt+3", "Alt+4", "Alt+5"];

    /// The part of a rendered row before the ` … ` separator: its F1 help key.
    fn rendered_shortcut_key(line: &str) -> &str {
        line.split_once(" … ").map_or(line, |(key, _)| key).trim()
    }

    /// Every atomic chord from the F1 help keyboard rows (T07).
    fn help_chords() -> Vec<&'static str> {
        crate::strings::HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| rows.iter().map(|(key, _)| *key))
            .filter(|key| !HELP_KEY_EXCEPTIONS.contains(key))
            .flat_map(|key| key.split(" / "))
            .map(str::trim)
            .filter(|chord| !chord.is_empty())
            .collect()
    }

    #[test]
    fn shortcut_rows_and_f1_help_keys_are_bidirectional() {
        // T06: fail on drift in either direction. HELP_SECTIONS is the source
        // of truth for F1 help and README; the Shortcuts tab must mirror every
        // keyboard row, and must not advertise a chord F1 help cannot explain.
        let help_keys: Vec<&str> = crate::strings::HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| rows.iter().map(|(key, _)| *key))
            .filter(|key| !HELP_KEY_EXCEPTIONS.contains(key))
            .collect();
        let rendered: Vec<&str> = SHORTCUTS_DEFAULT_LINES
            .iter()
            .map(|line| rendered_shortcut_key(line))
            .collect();

        for key in &help_keys {
            assert!(
                rendered.contains(key),
                "F1 help key {key} must be rendered on the Shortcuts tab"
            );
        }

        let help_text = crate::strings::HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| rows.iter().flat_map(|(key, text)| [*key, *text]))
            .collect::<Vec<_>>()
            .join("\n");
        for key in &rendered {
            assert!(
                help_text.contains(key),
                "rendered shortcut {key} is undocumented in F1 help"
            );
            assert!(
                help_keys.contains(key) || SETTINGS_ONLY_KEYS.contains(key),
                "rendered shortcut {key} needs a documented exception"
            );
        }

        let mut actual = rendered;
        actual.sort_unstable();
        let mut expected = help_keys;
        expected.extend_from_slice(SETTINGS_ONLY_KEYS);
        expected.sort_unstable();
        assert_eq!(
            actual, expected,
            "Shortcuts rows must be a 1:1 mirror of F1 help keyboard rows"
        );
    }

    /// Explicit notation differences between F1 help keys and the README key
    /// column (X15). README spells arrows as Up/Down/Left/Right; a chord not
    /// listed here must match the README table verbatim.
    const README_KEY_ALIASES: &[(&str, &str)] = &[
        ("↑", "Up"),
        ("↓", "Down"),
        ("Alt+←", "Alt+Left"),
        ("Alt+→", "Alt+Right"),
        ("Alt+↑", "Alt+Up"),
        ("Shift+↑↓", "Shift+Up/Down/Home/End"),
    ];

    /// Key column of the README "キーバインド" table, one normalized cell per
    /// line (backticks removed) so a deleted row can be detected (X15).
    fn readme_key_column() -> String {
        let mut in_section = false;
        let mut cells = Vec::new();
        for line in include_str!("../README.md").lines() {
            if line.starts_with("## キーバインド") {
                in_section = true;
                continue;
            }
            if !in_section {
                continue;
            }
            if line.starts_with('#') {
                break;
            }
            if line.starts_with('|') {
                if let Some(cell) = line.split('|').nth(1) {
                    let cell = cell.trim().replace('`', "");
                    if cell != "キー" && !cell.starts_with('-') {
                        cells.push(cell);
                    }
                }
            }
        }
        assert!(
            !cells.is_empty(),
            "README キーバインド table must have parseable key cells"
        );
        cells.join("\n")
    }

    /// Whole-chord match inside a README key cell: `R` must not pass just
    /// because `Ctrl+R` is listed (X15).
    fn cell_has_chord(cell: &str, chord: &str) -> bool {
        let mut offset = 0;
        while let Some(pos) = cell[offset..].find(chord) {
            let start = offset + pos;
            let end = start + chord.len();
            let is_boundary =
                |c: Option<char>| c.is_none_or(|c| !c.is_alphanumeric() && c != '+' && c != '-');
            if is_boundary(cell[..start].chars().next_back())
                && is_boundary(cell[end..].chars().next())
            {
                return true;
            }
            offset = end;
        }
        false
    }

    #[test]
    fn every_f1_help_key_has_a_readme_keybinding_row() {
        // X15: the README key column and F1 HELP_SECTIONS drifted three times.
        // Reuse the explicit `HELP_KEY_EXCEPTIONS` (prose / mouse / dialog rows
        // have no chord row in the key table) and the explicit alias list
        // above. Every remaining F1 chord must appear in a README key cell.
        let keys = readme_key_column();
        for (section, rows) in crate::strings::HELP_SECTIONS {
            for (key, _) in *rows {
                if HELP_KEY_EXCEPTIONS.contains(key) {
                    continue;
                }
                for chord in key.split(" / ") {
                    let chord = chord.trim();
                    let expected = README_KEY_ALIASES
                        .iter()
                        .find(|(help, _)| *help == chord)
                        .map(|(_, readme)| *readme)
                        .unwrap_or(chord);
                    assert!(
                        keys.lines().any(|cell| cell_has_chord(cell, expected)),
                        "F1 chord {chord} ({section}) must have a README key row ({expected})"
                    );
                }
            }
        }
    }

    #[test]
    fn default_shortcut_keywords_cover_every_f1_help_chord() {
        // T07: a chord shown in F1 help must be searchable from the Shortcuts
        // tab. Bare letters (e.g. "R") may also match another tab's keyword, so
        // only containment in the Shortcuts result is asserted here.
        for chord in help_chords() {
            assert!(
                SHORTCUTS_DEFAULTS_KEYWORDS.contains(&chord),
                "default shortcut keywords must contain F1 help chord {chord}"
            );
            assert!(
                tabs_matching_query(chord).contains(&SettingsTab::Shortcuts),
                "query {chord} must surface the Shortcuts tab"
            );
        }
    }

    #[test]
    fn every_shortcut_keyword_is_rendered() {
        // T07/M09: a chord keyword that is absent from every rendered row
        // makes no_match_hint point at a tab that never shows the typed text.
        for keyword in SHORTCUTS_DEFAULTS_KEYWORDS {
            if SEARCH_ALIAS_KEYWORDS.contains(keyword) {
                continue;
            }
            if ALT_DIGIT_KEYWORDS.contains(keyword) {
                assert!(
                    SHORTCUTS_DEFAULT_LINES
                        .iter()
                        .any(|line| line.contains("Alt+1〜5")),
                    "Alt+digit chords must be covered by the Alt+1〜5 row"
                );
                continue;
            }
            assert!(
                SHORTCUTS_DEFAULT_LINES
                    .iter()
                    .any(|line| line.contains(keyword)),
                "keyword {keyword} must appear in a rendered shortcut row"
            );
        }
    }

    #[test]
    fn tabs_matching_query_is_empty_when_no_tab_matches() {
        assert!(tabs_matching_query("zzzz-存在しない設定").is_empty());
    }

    #[test]
    fn tabs_matching_query_returns_every_tab_for_an_empty_query() {
        // Mirrors `matches_query`: an empty query makes every row visible.
        assert_eq!(tabs_matching_query(""), SettingsTab::ALL.to_vec());
        assert_eq!(tabs_matching_query("   "), SettingsTab::ALL.to_vec());
    }

    #[test]
    fn focus_order_tracks_the_active_tab_and_search_filter() {
        use SettingsFocusTarget::{Cancel, Keymap, Save, Search, Terminal};

        // Tabs without a text editor expose only the header and footer.
        assert_eq!(
            settings_focus_order(SettingsTab::Appearance, ""),
            vec![Search, Cancel, Save]
        );
        assert_eq!(
            settings_focus_order(SettingsTab::Files, ""),
            vec![Search, Cancel, Save]
        );
        // Personalize has only radio/switch rows, never a text editor.
        assert_eq!(
            settings_focus_order(SettingsTab::Personalize, ""),
            vec![Search, Cancel, Save]
        );
        // The Shortcuts/Terminal editor joins the cycle only when its row is
        // actually rendered (empty or matching query).
        assert_eq!(
            settings_focus_order(SettingsTab::Shortcuts, ""),
            vec![Search, Keymap, Cancel, Save]
        );
        assert_eq!(
            settings_focus_order(SettingsTab::Terminal, "  "),
            vec![Search, Terminal, Cancel, Save]
        );
        assert_eq!(
            settings_focus_order(SettingsTab::Shortcuts, "Alt+K"),
            vec![Search, Keymap, Cancel, Save]
        );
        assert_eq!(
            settings_focus_order(SettingsTab::Terminal, "ターミナル"),
            vec![Search, Terminal, Cancel, Save]
        );
        // A filter that hides the editor must remove it from the cycle.
        assert_eq!(
            settings_focus_order(SettingsTab::Shortcuts, "存在しない設定"),
            vec![Search, Cancel, Save]
        );
        assert_eq!(
            settings_focus_order(SettingsTab::Terminal, "zzz"),
            vec![Search, Cancel, Save]
        );
        // "Alt+T" / "既定" only match the default-list row, not the keymap
        // editor row, so the editor must stay out of the focus cycle.
        assert_eq!(
            settings_focus_order(SettingsTab::Shortcuts, "Alt+T"),
            vec![Search, Cancel, Save]
        );
        assert_eq!(
            settings_focus_order(SettingsTab::Shortcuts, "既定"),
            vec![Search, Cancel, Save]
        );
    }

    #[test]
    fn next_focus_wraps_in_both_directions() {
        use SettingsFocusTarget::{Cancel, Keymap, Save, Search};

        let order = [Search, Keymap, Cancel, Save];
        assert_eq!(next_settings_focus(&order, None, false), Some(Search));
        assert_eq!(next_settings_focus(&order, None, true), Some(Save));
        assert_eq!(
            next_settings_focus(&order, Some(Search), false),
            Some(Keymap)
        );
        assert_eq!(
            next_settings_focus(&order, Some(Keymap), true),
            Some(Search)
        );
        assert_eq!(next_settings_focus(&order, Some(Save), false), Some(Search));
        assert_eq!(next_settings_focus(&order, Some(Search), true), Some(Save));
        // A handle outside the current order (e.g. a focused radio button)
        // restarts the cycle instead of stalling.
        assert_eq!(
            next_settings_focus(&order, Some(SettingsFocusTarget::Terminal), false),
            Some(Search)
        );
        // Backwards from an unknown handle starts at the last entry.
        assert_eq!(
            next_settings_focus(&order, Some(SettingsFocusTarget::Terminal), true),
            Some(Save)
        );
        assert_eq!(next_settings_focus(&[], Some(Search), false), None);
        assert_eq!(next_settings_focus(&[], Some(Search), true), None);
    }
}
