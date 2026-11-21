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

// Search keyword sets, one per visible row. They are shared by the per-row
// filter in `render_settings_content` and by `tabs_matching_query`, so a row and
// the cross-tab hint derived from it can never drift apart.
const APPEARANCE_THEME_KEYWORDS: &[&str] = &[
    "テーマ",
    "theme",
    "ダーク",
    "ライト",
    "システム",
    "配色",
    "外観",
];
const APPEARANCE_VIEW_KEYWORDS: &[&str] =
    &["レイアウト", "view", "一覧", "アイコン", "表示", "外観"];
const FILES_HIDDEN_KEYWORDS: &[&str] = &["隠しファイル", "hidden", "ファイル", "ドットファイル"];
const FILES_SORT_KEYWORDS: &[&str] = &["並び", "sort", "ファイル", "順序"];
const SHORTCUTS_KEYMAP_KEYWORDS: &[&str] = &[
    "ショートカット",
    "キー",
    "keymap",
    "キーバインド",
    "割り当て",
];
const SHORTCUTS_DEFAULTS_KEYWORDS: &[&str] = &["既定", "default", "ショートカット", "キー"];
const TERMINAL_COMMAND_KEYWORDS: &[&str] = &["ターミナル", "terminal", "コマンド", "起動"];
const PERSONALIZE_PALETTE_KEYWORDS: &[&str] = &["個人化", "パレット", "色", "カラー", "theme"];
const PERSONALIZE_INFO_KEYWORDS: &[&str] = &["個人化", "情報", "about", "設定", "バージョン"];

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
                        "ファイル一覧を開いたときの基準となる項目",
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
                            .child(
                                "「操作=キー」形式で入力します。複数指定は ; で区切ります（例: open_preview=ctrl+shift+p; quit=ctrl+q）",
                            ),
                    )
                    .child(Input::new(&state.keymap_input).w_full());
            }
            if show_shortcuts_defaults {
                column = column
                    .child(divider(palette))
                    .child(section_title(palette, "主な既定のショートカット"))
                    .child(info_line(palette, "F1 … ヘルプ / キー一覧"))
                    .child(info_line(palette, "Ctrl+L … 場所を入力"))
                    .child(info_line(palette, "Ctrl+F … ファイルを検索"))
                    .child(info_line(palette, "F4 … ターミナルを開く"))
                    .child(info_line(palette, "Alt+T … ターミナル起動コマンドの設定"));
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
            // Validation errors are rendered once in the shared footer below,
            // so they stay visible on every tab (including Shortcuts).
            if show_terminal_command {
                column = column.child(info_line(
                    palette,
                    "変更は「保存」を押すまで反映されません。",
                ));
            }
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
        let message = if matching_tabs.is_empty() {
            "該当する設定がありません".to_string()
        } else {
            let labels: Vec<&'static str> = matching_tabs.iter().map(|tab| tab.label()).collect();
            format!(
                "該当する設定がありません。{} タブに一致があります",
                labels.join("、")
            )
        };
        div()
            .w_full()
            .py_6()
            .flex()
            .items_center()
            .justify_center()
            .text_sm()
            .text_color(rgb(palette.subtext))
            .child(message)
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
                .child(if state.dirty {
                    "未保存の変更があります"
                } else {
                    "変更は保存するまで反映されません"
                }),
        )
        .child(
            Button::new("settings-cancel")
                .label(cancel_label(state.dirty))
                .on_click(cancel_click),
        )
        .child(
            Button::new("settings-save")
                .label("保存")
                .primary()
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
        cancel_label, matches_query, selected_index, tabs_matching_query, SettingsTab,
        SettingsUiAction, SettingsUiPalette,
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
    fn tabs_matching_query_is_empty_when_no_tab_matches() {
        assert!(tabs_matching_query("zzzz-存在しない設定").is_empty());
    }

    #[test]
    fn tabs_matching_query_returns_every_tab_for_an_empty_query() {
        // Mirrors `matches_query`: an empty query makes every row visible.
        assert_eq!(tabs_matching_query(""), SettingsTab::ALL.to_vec());
        assert_eq!(tabs_matching_query("   "), SettingsTab::ALL.to_vec());
    }
}
