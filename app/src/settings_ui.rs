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
        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("設定を検索…"));
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

// Search keyword sets, one per visible row. They are shared by the per-row
// filter in `render_settings_content` and by `tabs_matching_query`, so a row and
// the cross-tab hint derived from it can never drift apart.
// M09: each array also carries the words actually rendered in that row's
// caption, description and section title — users search what they can see, not
// the internal option value. Keep the lists in sync with the row markup below.
const APPEARANCE_THEME_KEYWORDS: &[&str] = &[
    "テーマ",
    "theme",
    "ダーク",
    "ライト",
    "システム",
    "配色",
    "外観",
    // "アプリ全体の明るさを切り替えます"
    "明るさ",
    "切り替え",
];
const APPEARANCE_VIEW_KEYWORDS: &[&str] = &[
    "レイアウト",
    "view",
    "一覧",
    "アイコン",
    "表示",
    "外観",
    // "既定のビュー" / "詳細一覧またはアイコン一覧を選択します"
    "ビュー",
    "既定のビュー",
    "詳細一覧",
    "選択",
];
const FILES_HIDDEN_KEYWORDS: &[&str] = &[
    "隠しファイル",
    "hidden",
    "ファイル",
    "ドットファイル",
    // "名前が . で始まるファイルとフォルダーを一覧に含めます"
    "ドット",
    "フォルダー",
    "一覧",
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
];
const SHORTCUTS_DEFAULTS_KEYWORDS: &[&str] = &[
    "既定",
    "default",
    "ショートカット",
    "キー",
    "Alt+K",
    "Alt+T",
    "Alt+S",
    // Settings-dialog tab navigation, listed in F1 help and the default-shortcut
    // list below (L03/L07). Keep the array in sync with SHORTCUTS_DEFAULT_LINES.
    "Alt+1",
    "Alt+2",
    "Alt+3",
    "Alt+4",
    "Alt+5",
    "Alt+数字",
    "Ctrl+,",
    "Ctrl+F",
    "Ctrl+L",
    "F1",
    "F4",
    "Ctrl+Shift+L",
    "Ctrl+Space",
    "Ctrl+Shift+C",
    "Ctrl+Tab",
    "Ctrl+Shift+Tab",
    // "主な既定のショートカット" and the rendered default list. The display
    // form "Alt+1〜5" is searchable verbatim in addition to each digit (M09).
    "主な",
    "タブ",
    "直接移動",
    "Alt+1〜5",
    "ヘルプ",
    "キー一覧",
    "場所",
    "設定を開く",
];

/// Shortcuts-tab keymap hint. Unlike the Alt+K dialog this field replaces the
/// whole override list on Save, so that consequence must be spelled out where
/// the user is typing (H04). Per-entry `unbind アクション名` is accepted here
/// too (the Save handler parses it through `parse_keymap_input`), so the hint
/// must not send users to Alt+K for something this field can do itself (K04).
/// M04: like `strings::KEYMAP_INPUT_HINT`, it also names `open_settings` as a
/// reserved action — `Ctrl+,` is fixed and cannot be rebound (or unbound)
/// from either editor, so promising otherwise here would mislead the user.
const SETTINGS_KEYMAP_INPUT_HINT: &str = "「操作=キー」形式で入力します。複数指定は ; で区切ります（例: open_preview=ctrl+shift+p; quit=ctrl+q）。保存すると一覧は入力内容で置き換わります（一部追加ではありません）。この欄でも `unbind アクション名` で解除できます。open_settings は Ctrl+, 固定で変更できません（unbind も不可）。";
/// Rows of the "主な既定のショートカット" list, kept as one constant so the
/// Shortcuts tab, F1 help (`strings::HELP_SECTIONS`) and README cannot drift
/// apart. The chord names are also searchable via `SHORTCUTS_DEFAULTS_KEYWORDS`
/// (L03/L07).
const SHORTCUTS_DEFAULT_LINES: &[&str] = &[
    "F1 … ヘルプ / キー一覧",
    "Ctrl+L … 場所を入力",
    "Ctrl+F … ファイルを検索",
    "F4 … ターミナルを開く",
    "Alt+T … ターミナル起動コマンドの設定",
    "Ctrl+, … 設定を開く",
    "Alt+1〜5 … 設定ダイアログのタブへ直接移動",
    "Alt+S … ファイルタブの並び順を切替",
];
const TERMINAL_COMMAND_KEYWORDS: &[&str] = &[
    "ターミナル",
    "terminal",
    "コマンド",
    "起動",
    // "選択中のフォルダーを作業ディレクトリとして起動するコマンド（空欄で無効化）"
    "作業ディレクトリ",
    "無効化",
    "フォルダー",
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
                        "アプリ全体の明るさを切り替えます",
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
                        "詳細一覧またはアイコン一覧を選択します",
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
                        "名前が . で始まるファイルとフォルダーを一覧に含めます",
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
                        "ファイル一覧を開いたときの基準となる項目（Alt+S でキーボードから切替）",
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
                            .child("選択中のフォルダーを作業ディレクトリとして起動するコマンド（空欄で無効化）"),
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
                    .child(section_title(palette, "このアプリについて"))
                    .child(info_line(
                        palette,
                        "gpui-file-explorer — GPUI 製のファイルマネージャー",
                    ))
                    .child(info_line(
                        palette,
                        "設定は ~/.config/gpui-file-explorer/settings.json に保存されます。",
                    ))
                    .child(info_line(
                        palette,
                        "配色はテーマ設定に連動し、変更は保存時に反映されます。",
                    ));
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
    let footer = footer.child(actions);

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
        selected_index, settings_focus_order, settings_hidden_next, settings_row_is_visible,
        settings_row_shortcut, settings_theme_next, settings_view_next, tabs_matching_query,
        SettingsFocusTarget, SettingsRowValues, SettingsTab, SettingsUiAction, SettingsUiPalette,
        SETTINGS_KEYMAP_INPUT_HINT, SHORTCUTS_DEFAULTS_KEYWORDS, SHORTCUTS_DEFAULT_LINES,
    };

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
    fn tabs_matching_query_finds_shortcut_key_names() {
        // H09: chord names are the way users refer to shortcuts, so every one
        // displayed on the Shortcuts tab must surface that tab on its own.
        for key in [
            "Alt+K",
            "Alt+T",
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
        for key in ["F1", "Ctrl+L", "Ctrl+F", "F4", "Alt+T", "Ctrl+,", "Alt+S"] {
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
