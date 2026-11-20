// SPDX-License-Identifier: AGPL-3.0-or-later
//! Centralized Japanese UI strings (DESIGN.md §8.4): first step toward i18n.
//!
//! Only self-contained literals live here. Messages that embed runtime values
//! stay at their call sites with `format!`.
//!
//! # 文言規約 (DESIGN.md §8.4)
//!
//! - 用語は「フォルダ / ファイル」を基本とするが、ヘルプ・ステータス・監視表示では
//!   「ディレクトリ」も許容する（エラー詳細でも従来どおり使用）。表記ゆれは
//!   `strings.rs` の定数へ集約する目標とする。
//! - 敬体は「です / ます」、ボタンは体言止め（「ゴミ箱へ移動」「キャンセル」）。
//! - キー名は i18n 前提の SCREAMING_SNAKE_CASE を維持し、P2 の `rust-i18n`
//!   移行では値だけを翻訳リソースへ差し替える（DESIGN.md §8.4）。
//!
//! # ロケール依存の並び順・大小文字比較（現状契約の明示）
//!
//! 一覧の並び順と名前の一致判定は **ロケール非依存** である。OS のロケールに
//! 暗黙依存すると表示が環境ごとに変わるため、P0 では次の契約を採る。
//!
//! - 並び順: `model::sort_entries` / `model::compare_names` が
//!   `alphanumeric_sort::compare_str` で比較する。比較キーは
//!   `str::to_lowercase()` の事前計算値で、大小文字を無視する自然順
//!   （"file2" < "file10"）。同値は大小文字を区別する比較で安定化する。
//! - 日本語の名前は Unicode スカラ値順になり **五十音順にはならない**。
//!   ICU4X によるロケール照合は P3 の未実装項目（DESIGN.md:103, 1075、
//!   `notes/fs-cross-platform-notes.md` §1.4）。
//! - 大小文字比較は `to_lowercase()` であり、完全な Unicode case folding でも
//!   ロケール固有の照合（トルコ語 `İ` など）でもない。NFC 正規化も行わない。
//!   そのため `ß` / `ẞ` や NFD ファイル名の一致漏れは既知の制限である。
//! - 日時・サイズの書式もロケール非依存（ローカルタイムの `YYYY-MM-DD HH:MM`
//!   と 2 進単位）。相対時刻の単語は下の `AGE_*` に集約した。
//!
//! `LABEL_*` / `KIND_*` / `AGE_*` / `VALUE_UNKNOWN` は `main.rs` / `model.rs` に
//! 直書きされている同義リテラルの正規定義である。F6-26 の書き込み範囲はこの
//! ファイルのみのため呼び出し側の差し替えは別タスクだが、下の
//! `formatting_words_match_call_sites` / `kind_words_match_call_sites` が乖離を検出する。
//!
//! # エラー・ステータスメッセージの情報量と共存ルール (F6-36)
//!
//! ## 情報量の段階
//!
//! 診断できる通知は次の 4 段で構成し、上から順に欠落しないようにする。
//!
//! 1. **種別**: `STATUS_ERROR_PREFIX` などの接頭辞で成否・種別を明示する。
//! 2. **対象**: パス / 名前 / 件数（`「{name}」`、`{n} 件`）。部分失敗では
//!    対象名を残すのに全件失敗で捨てるという非対称を作らない。
//! 3. **理由**: `OpError` / `error` の文言をそのまま連結する。
//! 4. **復帰導線**: `（Esc でキャンセル）`、`（再検索が必要です）` など次に
//!    何をすべきかを示す（既存の `STATUS_SEARCH_STALE` がこの形）。
//!
//! 実行時値は従来どおり呼び出し側の `format!` に残し、ここには自足した
//! 接頭辞・接尾辞だけを正規定義として置く。
//!
//! ## 共存ルール（競合時の優先順位）
//!
//! 表示層の優先順位は `STATUS_PRIORITY` のとおり
//! **エラー > 操作通知 > 進捗 > 選択情報 > 監視状態** で、狭い領域では
//! 後ろから省略する。空状態ではエラーがローディングより優先される。
//!
//! 1 回の操作完了内の分岐は `OP_RESULT_PRIORITY` のとおり
//! **キャンセル > 一部失敗 > 失敗のみ > 成功** である。成功と失敗が
//! 同時に起きたときは成功を消さず `STATUS_PARTIAL_FAILURE_SUFFIX` で
//! 失敗を併記し、成功項目が皆無の場合だけ `STATUS_OP_FAILED_SUFFIX` に
//! 落とす。再スキャン失敗（`STATUS_RESCAN_FAILED`）は後続の
//! `STATUS_CHANGED` で上書きせず、次回成功まで保持する。
//!
//! ステータス文言の定数化は aeb687c で呼び出し側（`main.rs`）まで移行済みで、
//! 下の定数が正規の値を提供する。文言を変更するときは呼び出し側の表示も
//! 併せて確認する。

// ---------------------------------------------------------------------------
// F1 ヘルプオーバーレイ
// ---------------------------------------------------------------------------

/// Ctrl+P 機能のユーザー向け名称。ヘルプ・ステータス・オーバーレイ・CLI ヘルプで共用する。
pub const FUZZY_JUMP_LABEL: &str = "クイックジャンプ";
/// 親フォルダを指す共通ラベル。ヘルプとナビゲーションボタンで共用する。
pub const PARENT_FOLDER_LABEL: &str = "親フォルダ";

/// Keyboard shortcuts shown by the F1 help overlay: (section, [(key, description)]).
pub const HELP_SECTIONS: &[(&str, &[(&str, &str)])] = &[
    (
        "ナビゲーション",
        &[
            ("Enter", "選択項目を開く"),
            ("↑ / ↓", "選択項目を上下に移動"),
            ("Ctrl+L", "パスを直接入力して移動"),
            ("Alt+← / Alt+→", "戻る / 進む"),
            ("Alt+↑ / Backspace", PARENT_FOLDER_LABEL),
            ("Home / End", "先頭 / 末尾へ"),
            ("PageUp / PageDown", "ページ単位で移動"),
            ("Ctrl+P", FUZZY_JUMP_LABEL),
            (
                "Ctrl+Shift+L",
                "サイドバーのホームへ移動（他の場所はマウスのみ）",
            ),
        ],
    ),
    (
        "選択",
        &[
            ("Ctrl+A", "全選択"),
            ("Ctrl+Space", "フォーカス行の選択を切替（非連続選択）"),
            ("Shift+↑↓", "範囲選択"),
            ("Ctrl+クリック / Shift+クリック", "追加選択 / 範囲選択"),
        ],
    ),
    (
        "ファイル操作",
        &[
            (
                "Ctrl+C / Ctrl+X / Ctrl+V",
                "コピー / 切り取り / 貼り付け（常に別名保存）",
            ),
            ("Ctrl+Shift+C", "フォーカス行のパスをコピー"),
            ("F2", "名前変更"),
            ("Delete", "ゴミ箱へ移動（完全削除なし）"),
            ("R / Ctrl+Enter", "選択項目を元の場所へ復元（ゴミ箱ビュー）"),
            (
                "ゴミ箱ビュー",
                "Enter / R で復元、Delete は完全削除しません",
            ),
            ("Ctrl+Shift+N", "新規フォルダ"),
            (
                "ドラッグ&ドロップ",
                "OS からコピー / フォルダ行へコピー / OS へドラッグ",
            ),
            (
                "Ctrl+Z / Ctrl+Shift+Z",
                "元に戻す / やり直す（名前変更・ゴミ箱のみ）",
            ),
            ("Ctrl+Shift+U", "操作の履歴を表示"),
        ],
    ),
    (
        "表示",
        &[
            ("Ctrl+1 / Ctrl+2", "詳細リスト / アイコングリッド"),
            ("Ctrl+H", "隠しファイルの表示切替"),
            ("Ctrl+R / F5", "再読込"),
            ("Ctrl+F", "検索（名前 / 内容）"),
            ("Ctrl+Shift+F", "検索結果を名前で絞り込み"),
            ("Ctrl+I", "プロパティ"),
            ("Space / F3", "テキストプレビュー"),
            ("Alt+Shift+T", "テーマをダーク / ライトで切替"),
        ],
    ),
    (
        "タブ・ブックマーク",
        &[
            ("Ctrl+T / Ctrl+W", "タブを開く / 閉じる"),
            ("Ctrl+Shift+T", "閉じたタブを復元"),
            ("Ctrl+Tab / Ctrl+Shift+Tab", "次のタブ / 前のタブ"),
            ("Ctrl+Shift+PageUp / PageDown", "タブを左 / 右へ移動"),
            ("ミドルクリック", "行のフォルダを新しいタブで開く"),
            ("Ctrl+D", "現在のディレクトリをブックマーク"),
        ],
    ),
    (
        "その他",
        &[
            ("F1", "このヘルプ"),
            ("Ctrl+,", "設定を開く"),
            ("F4", "選択中のフォルダでターミナルを開く"),
            ("Alt+T", "F4 で開くターミナルの起動コマンドを設定"),
            (
                "Alt+K",
                "キーボードショートカットを設定（アクション名 コード）",
            ),
            ("Esc", "キャンセル / ダイアログを閉じる"),
            ("Ctrl+Q", "終了"),
        ],
    ),
    (
        "ライセンス / ソース",
        &[(
            "AGPL-3.0-or-later",
            "ソース: https://github.com/Seduce0068/gpui-file-explorer",
        )],
    ),
];

/// Keymap settings dialog (Alt+K): the input syntax and the reserved keys.
/// Text entry is used instead of raw key capture so Escape / F1 keep their
/// dialog-ownership role (see `app/src/main.rs` keymap helpers).
pub const KEYMAP_INPUT_HINT: &str =
    "形式: アクション名 コード（例: open_preview ctrl+shift+p）。unbind アクション名で設定を解除。escape / f1 は予約キーです";

// ---------------------------------------------------------------------------
// 空状態・読み込み
// ---------------------------------------------------------------------------

/// Empty-state title + optional hint per listing source.
pub const EMPTY_TRASH: (&str, &str) = (
    "ゴミ箱は空です",
    "削除した項目はここに表示されます（完全削除は未対応）",
);
pub const EMPTY_SEARCH: (&str, &str) = (
    "一致する項目がありません",
    "検索語や絞り込みを変更してください",
);
pub const EMPTY_DIRECTORY: (&str, &str) = (
    "このフォルダは空です",
    "ここにファイルをドロップするか、Ctrl+Shift+N で新規フォルダ / Ctrl+V で貼り付け",
);
pub const EMPTY_FILTERED: (&str, &str) = (
    "絞り込みで全て除外されました",
    "種別や拡張子の条件を変更してください",
);
pub const LOADING: &str = "読み込み中…";
/// Recovery hint shown under a load error in the empty-state overlay (U-20).
pub const LOAD_ERROR_HINT: &str = "F5 で再読込 / Ctrl+L で別のパスへ移動";

// ---------------------------------------------------------------------------
// ステータスバー（共通・タブ・ゴミ箱）
// ---------------------------------------------------------------------------

/// Frequently used fixed status-bar messages.
pub const STATUS_CHANGED: &str = "変更を検出しました";
/// Shown when a search snapshot cannot be refreshed automatically.
pub const STATUS_SEARCH_STALE: &str = "変更を検出しました（再検索が必要です）";
pub const STATUS_TABS_ONLY_DIRECTORY: &str = "タブはディレクトリ表示でのみ操作できます";
pub const STATUS_BOOKMARKS_ONLY_DIRECTORY: &str =
    "ブックマークはディレクトリ表示でのみ操作できます";
pub const STATUS_LAST_TAB: &str = "最後のタブは閉じられません";
pub const STATUS_TAB_MOVED_LEFT: &str = "タブを左へ移動しました";
pub const STATUS_TAB_MOVED_RIGHT: &str = "タブを右へ移動しました";
pub const STATUS_TRASH_NO_COPY: &str = "ゴミ箱ビューではコピーできません";
pub const STATUS_TRASH_NO_CUT: &str = "ゴミ箱ビューでは切り取りできません";
pub const STATUS_TRASH_NO_PASTE: &str = "ゴミ箱ビューでは貼り付けできません";
pub const STATUS_TRASH_NO_RENAME: &str = "ゴミ箱ビューでは名前を変更できません";
pub const STATUS_TRASH_NO_PURGE: &str = "ゴミ箱内の完全削除は未対応です";
pub const STATUS_TRASH_NO_HIDDEN: &str = "ゴミ箱ビューでは隠しファイルの切替はできません";
pub const STATUS_TRASH_NO_SORT: &str = "ゴミ箱ビューでは並び替えはできません";
pub const STATUS_CANCELLING: &str = "キャンセルしています…";
/// コピー / 切り取り / ゴミ箱移動で選択項目がないときの共通通知。
pub const STATUS_NO_SELECTION: &str = "選択項目がありません";
pub const STATUS_NO_CLIPBOARD: &str = "コピー/切り取りした項目がありません";
pub const STATUS_OP_RUNNING: &str = "ファイル操作を実行中です（Esc でキャンセル）";
pub const STATUS_DROP_ONLY_DIRECTORY: &str = "ドロップはディレクトリ表示でのみ使えます";
pub const STATUS_DROP_NO_DESTINATION: &str = "ドロップ先のフォルダが見つかりません";
pub const STATUS_DROP_SAME_DIRECTORY: &str = "同じフォルダのためコピーしません";
pub const STATUS_DROP_NOT_FOUND: &str = "ドロップされた項目が見つかりません";
pub const STATUS_DROP_EMPTY: &str = "ドロップされた項目がありません";
pub const STATUS_TRASH_LOADING: &str = "ゴミ箱を読み込んでいます…";
pub const STATUS_FOLDER_CREATING: &str = "フォルダを作成中…";

/// Repeated format-string prefixes (the dynamic suffix stays at the call site).
pub const STATUS_TARGET_GONE: &str = "移動先が存在しません";
pub const STATUS_PATH_GONE: &str = "対象が存在しません";
pub const STATUS_NOT_DIRECTORY: &str = "ディレクトリではありません";
pub const STATUS_UNDO_FAILED: &str = "元に戻せません";
pub const STATUS_REDO_NO_TARGET: &str = "やり直せません: 対象が存在しません";

// ---------------------------------------------------------------------------
// 操作完了サマリ
// ---------------------------------------------------------------------------

/// Suffixes used to summarize a completed file operation.
pub const OP_FILES_SUFFIX: &str = "ファイル";
pub const OP_DIRS_SUFFIX: &str = "フォルダ";
pub const OP_SKIPPED_SUFFIX: &str = "件スキップ";
pub const OP_DONE_SUFFIX: &str = "しました";

// ---------------------------------------------------------------------------
// F6-36 エラー・ステータスメッセージ（main.rs の status 通知で使用）
// ---------------------------------------------------------------------------

// aeb687c で main.rs のステータスリテラルは本定数へ移行済み。呼び出し側が
// 参照するため、値を変更すると表示文言に直接反映される。
// 一部の定数は規約データ・予備としてのみ保持され本番コードから参照されないため、
// 個別に dead_code を許可する（下の整合性テストで値の乖離を検出）。

/// エラー表示の共通プレフィクス。空状態ではローディングより優先される。
pub const STATUS_ERROR_PREFIX: &str = "エラー: ";
/// `{verb}しました` に続く一部失敗の括弧。成功を消さず失敗を併記する。
pub const STATUS_PARTIAL_FAILURE_SUFFIX: &str = "しました（一部失敗: ";
/// 成功項目が皆無のときの失敗接尾（`{verb}` に続く）。
pub const STATUS_OP_FAILED_SUFFIX: &str = "できません";
/// スキップ件数の括弧（`{status}` に続く）。
#[allow(dead_code)]
pub const STATUS_SKIPPED_PREFIX: &str = "（存在しない項目 ";
#[allow(dead_code)]
pub const STATUS_SKIPPED_SUFFIX: &str = " 件をスキップ）";
/// 操作中止の完了通知（進行中の `STATUS_CANCELLING` とは別フェーズ）。
pub const STATUS_CANCELLED: &str = "キャンセルしました";
/// 進行中通知の接尾（`{verb}` に続く）。
pub const STATUS_OP_IN_PROGRESS_SUFFIX: &str = "中…（Esc でキャンセル）";

/// 再スキャン失敗。後続の `STATUS_CHANGED` で上書きしない（R6-A-04）。
pub const STATUS_RESCAN_FAILED: &str = "再スキャンに失敗しました";
/// 監視の状態表示。`last_error()` がある場合は理由を連結する。
pub const STATUS_WATCH_PREFIX: &str = "監視: ";
pub const STATUS_WATCH_PARTIAL: &str = "一部無効 — ";
pub const STATUS_WATCH_DISABLED: &str = "無効 — ";
pub const STATUS_WATCH_INIT_FAILED: &str = "監視の初期化に失敗しました";
/// 監視ディレクトリ数の接尾（`{count}` に続く）。
pub const STATUS_WATCH_COUNT_SUFFIX: &str = " ディレクトリ";

/// OS 連携の成功 notification。戻り値を確認せずに出す現状の文言（r4-error-01）。
pub const STATUS_OPENED_DEFAULT_SUFFIX: &str = "を既定のアプリで開きました";
pub const STATUS_REVEALED_PREFIX: &str = "ファイルマネージャで表示: ";
/// 設定の失敗／退避（読み込み失敗と情報量を揃える）。
pub const STATUS_SETTINGS_SAVE_FAILED: &str =
    "設定を保存できませんでした（権限や空き容量を確認して再試行してください）";
pub const STATUS_SETTINGS_READ_ONLY: &str = "設定を読み取り専用で開きました";
/// タブ復元の成否。
pub const STATUS_TAB_RESTORED: &str = "タブを復元しました";
pub const STATUS_TAB_RESTORE_EMPTY: &str = "復元できるタブがありません";
/// フォルダ作成の成否。
pub const STATUS_FOLDER_CREATED: &str = "フォルダを作成しました";
pub const ERR_FOLDER_CREATE: &str = "フォルダを作成できません";
/// パスコピー／件数付き通知の接尾。
pub const STATUS_PATH_COPIED: &str = "パスをコピーしました";
pub const STATUS_MISSING_COUNT_SUFFIX: &str = " 件は既に存在しません";
/// Undo / Redo の「対象なし」と失敗。
pub const STATUS_UNDO_EMPTY: &str = "元に戻せる操作がありません";
pub const STATUS_UNDO_EXISTS_SUFFIX: &str = " が既に存在します";
pub const STATUS_REDO_EMPTY: &str = "やり直せる操作がありません";
pub const STATUS_REDO_FAILED: &str = "やり直せません";
pub const STATUS_RENAME_FAILED: &str = "変更できません";
pub const STATUS_TARGET_MISSING: &str = "対象は既に存在しません";
/// テーマ切替後の通知（値は ThemeMode::name(): `dark` / `light`）。
pub const STATUS_THEME_CHANGED_PREFIX: &str = "テーマ: ";
/// ゴミ箱操作の全件失敗と部分失敗（後者は `（{n} 件失敗: 「{name}」: {reason}）` の一部）。
pub const STATUS_TRASH_ALL_FAILED: &str = "ゴミ箱へ移動できません";
pub const STATUS_TRASH_PARTIAL_FAILED: &str = " 件失敗: ";
pub const STATUS_RESTORE_FAILED: &str = "復元できません";

/// 表示層の優先順位（高い順）。狭いステータスバーでは後ろから省略する。
/// 実行時のスロットではなく、競合時に参照する規約データ（r5-render-09）。
/// `main.rs` の二次スロット省略ヘルパーがこの長さと末尾（"監視状態"）を
/// 参照するため、変更時はそちらの順序定数も同期させること。
pub const STATUS_PRIORITY: &[&str] = &["エラー", "操作通知", "進捗", "選択情報", "監視状態"];
/// 1 回の操作完了内での分岐順（高い順）。成功と失敗が競合しても成功を消さず、
/// 一部失敗として併記するための規約。
#[allow(dead_code)]
pub const OP_RESULT_PRIORITY: &[&str] = &["キャンセル", "一部失敗", "失敗のみ", "成功"];

// ---------------------------------------------------------------------------
// 実行時値と組み合わせる正規の単語（呼び出し側は未移行、下のテストで整合性を検査）
// ---------------------------------------------------------------------------

// 呼び出し側（main.rs / model.rs）への適用は F6-26 の書き込み範囲外のため別タスク。
// 適用完了まで非テストビルドでは未使用となるので dead_code を明示的に許可し、
// 下の整合性テストで値の乖離を検出する。
/// Placeholder for unavailable metadata (`model::human_age` / `human_datetime`).
#[allow(dead_code)]
pub const VALUE_UNKNOWN: &str = "-";
/// Canonical cancel label. Do not use the English `cancel` in prompt buttons.
pub const LABEL_CANCEL: &str = "キャンセル";

/// `種類` 列のラベル（現在は `model::kind_label` に直書き）。
#[allow(dead_code)]
pub const KIND_FOLDER: &str = "フォルダ";
#[allow(dead_code)]
pub const KIND_LINK: &str = "リンク";
#[allow(dead_code)]
pub const KIND_NONE: &str = "—";

/// 相対時刻の単語（現在は `model::human_age` に直書き）。
#[allow(dead_code)]
pub const AGE_FUTURE: &str = "未来";
#[allow(dead_code)]
pub const AGE_JUST_NOW: &str = "たった今";
#[allow(dead_code)]
pub const AGE_MINUTES_SUFFIX: &str = "分前";
#[allow(dead_code)]
pub const AGE_HOURS_SUFFIX: &str = "時間前";
#[allow(dead_code)]
pub const AGE_DAYS_SUFFIX: &str = "日前";
#[allow(dead_code)]
pub const AGE_YEARS_SUFFIX: &str = "年前";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::compare_names;
    use std::cmp::Ordering;

    #[test]
    fn help_sections_are_well_formed() {
        assert!(!HELP_SECTIONS.is_empty());
        for (title, entries) in HELP_SECTIONS {
            assert!(!title.is_empty());
            assert!(!entries.is_empty());
            for (key, description) in *entries {
                assert!(!key.is_empty());
                assert!(!description.is_empty());
            }
        }
    }

    #[test]
    fn help_sections_keep_all_expected_areas() {
        let titles: Vec<&str> = HELP_SECTIONS.iter().map(|(title, _)| *title).collect();
        for expected in [
            "ナビゲーション",
            "選択",
            "ファイル操作",
            "表示",
            "タブ・ブックマーク",
            "その他",
        ] {
            assert!(
                titles.contains(&expected),
                "missing help section: {expected}"
            );
        }
    }

    #[test]
    fn help_lists_keymap_shortcut_and_hint() {
        let keymap_entry = HELP_SECTIONS
            .iter()
            .flat_map(|(_, entries)| *entries)
            .find(|(key, _)| *key == "Alt+K");
        assert!(keymap_entry.is_some(), "Alt+K must be listed in F1 help");
        assert!(KEYMAP_INPUT_HINT.contains("open_preview ctrl+shift+p"));
        assert!(KEYMAP_INPUT_HINT.contains("unbind アクション名"));
        assert!(KEYMAP_INPUT_HINT.contains("escape / f1"));
    }

    #[test]
    fn help_lists_fuzzy_terminal_and_keymap_shortcuts() {
        let keys: Vec<&str> = HELP_SECTIONS
            .iter()
            .flat_map(|(_, entries)| *entries)
            .map(|(key, _)| *key)
            .collect();
        for expected in ["Ctrl+P", "F4", "Alt+K"] {
            assert!(
                keys.contains(&expected),
                "{expected} must be listed in F1 help"
            );
        }
    }

    #[test]
    fn shared_labels_are_used_in_help_sections() {
        let find = |wanted: &str| {
            HELP_SECTIONS
                .iter()
                .flat_map(|(_, entries)| *entries)
                .find(|(key, _)| *key == wanted)
                .map(|(_, description)| *description)
        };
        assert_eq!(find("Ctrl+P"), Some(FUZZY_JUMP_LABEL));
        assert_eq!(find("Alt+↑ / Backspace"), Some(PARENT_FOLDER_LABEL));
    }

    #[test]
    fn help_lists_keyboard_accessibility_shortcuts() {
        let keys: Vec<&str> = HELP_SECTIONS
            .iter()
            .flat_map(|(_, entries)| *entries)
            .map(|(key, _)| *key)
            .collect();
        for expected in ["Ctrl+Shift+L", "Ctrl+Space", "Ctrl+Shift+C"] {
            assert!(
                keys.contains(&expected),
                "{expected} must be listed in F1 help"
            );
        }
    }

    #[test]
    fn help_lists_navigation_restore_and_trash_notes() {
        let entries: Vec<(&str, &str)> = HELP_SECTIONS
            .iter()
            .flat_map(|(_, entries)| *entries)
            .copied()
            .collect();
        let find = |wanted: &str| {
            entries
                .iter()
                .find(|(key, _)| *key == wanted)
                .map(|(_, description)| *description)
        };
        assert_eq!(find("↑ / ↓"), Some("選択項目を上下に移動"));
        assert_eq!(
            find("R / Ctrl+Enter"),
            Some("選択項目を元の場所へ復元（ゴミ箱ビュー）")
        );
        let trash_note = find("ゴミ箱ビュー").expect("trash note must be listed");
        assert!(trash_note.contains("Enter / R"));
        assert!(trash_note.contains("Delete"));
        assert!(trash_note.contains("完全削除しません"));
    }

    #[test]
    fn canonical_words_are_not_english() {
        for word in [
            LABEL_CANCEL,
            KIND_FOLDER,
            KIND_LINK,
            KIND_NONE,
            AGE_FUTURE,
            AGE_JUST_NOW,
            AGE_MINUTES_SUFFIX,
            AGE_HOURS_SUFFIX,
            AGE_DAYS_SUFFIX,
            AGE_YEARS_SUFFIX,
        ] {
            assert!(!word.is_empty());
            assert!(
                !word.chars().any(|c| c.is_ascii_alphabetic()),
                "canonical word must not contain English letters: {word}"
            );
        }
    }

    #[test]
    fn sort_and_case_contract_is_explicit() {
        // Natural, case-insensitive primary order with a deterministic
        // case-sensitive tie-break (`model::compare_names`).
        assert_eq!(compare_names("alpha", "Zebra"), Ordering::Less);
        assert_eq!(compare_names("file2", "file10"), Ordering::Less);
        assert_eq!(compare_names("ABC", "abc"), Ordering::Less);
        assert_eq!(compare_names("abc", "ABC"), Ordering::Greater);

        // Japanese names follow Unicode scalar order, NOT gojūon
        // (U+3042 'あ' < U+30A2 'ア' < U+4E00 '一').
        assert_eq!(compare_names("あ", "ア"), Ordering::Less);
        assert_eq!(compare_names("ア", "一"), Ordering::Less);

        // `to_lowercase()` is not full case folding and no NFC normalization
        // is applied: these pairs must not collapse into equality.
        assert_ne!(compare_names("ß", "ss"), Ordering::Equal);
    }

    fn entry(name: &str, is_dir: bool, is_symlink: bool) -> crate::model::Entry {
        crate::model::Entry {
            name: name.into(),
            name_text: name.into(),
            sort_key: name.to_lowercase(),
            path: std::path::PathBuf::from(name),
            is_dir,
            is_symlink,
            size: 0,
            modified: None,
        }
    }

    #[test]
    fn kind_words_match_call_sites() {
        assert_eq!(
            crate::model::kind_label(&entry("dir", true, false)),
            KIND_FOLDER
        );
        assert_eq!(
            crate::model::kind_label(&entry("link", false, true)),
            KIND_LINK
        );
        assert_eq!(
            crate::model::kind_label(&entry("README", false, false)),
            KIND_NONE
        );
    }

    #[test]
    fn formatting_words_match_call_sites() {
        use std::time::{Duration, SystemTime};

        let age = |secs: u64| {
            crate::model::human_age(Some(SystemTime::now() - Duration::from_secs(secs)))
        };
        assert_eq!(crate::model::human_age(None), VALUE_UNKNOWN);
        assert_eq!(crate::model::human_datetime(None), VALUE_UNKNOWN);
        assert_eq!(age(1), AGE_JUST_NOW);
        assert_eq!(age(120), format!("2{AGE_MINUTES_SUFFIX}"));
        assert_eq!(age(7200), format!("2{AGE_HOURS_SUFFIX}"));
        assert_eq!(age(172_800), format!("2{AGE_DAYS_SUFFIX}"));
        assert_eq!(age(86_400 * 730), format!("2{AGE_YEARS_SUFFIX}"));
        assert_eq!(
            crate::model::human_age(Some(SystemTime::now() + Duration::from_secs(3600))),
            AGE_FUTURE
        );
    }

    #[test]
    fn status_message_constants_are_nonempty() {
        for message in [
            STATUS_ERROR_PREFIX,
            STATUS_PARTIAL_FAILURE_SUFFIX,
            STATUS_OP_FAILED_SUFFIX,
            STATUS_SKIPPED_PREFIX,
            STATUS_SKIPPED_SUFFIX,
            STATUS_CANCELLED,
            STATUS_NO_SELECTION,
            STATUS_OP_IN_PROGRESS_SUFFIX,
            STATUS_RESCAN_FAILED,
            STATUS_WATCH_PREFIX,
            STATUS_WATCH_PARTIAL,
            STATUS_WATCH_DISABLED,
            STATUS_WATCH_INIT_FAILED,
            STATUS_WATCH_COUNT_SUFFIX,
            STATUS_OPENED_DEFAULT_SUFFIX,
            STATUS_REVEALED_PREFIX,
            STATUS_SETTINGS_SAVE_FAILED,
            STATUS_SETTINGS_READ_ONLY,
            STATUS_TAB_RESTORED,
            STATUS_TAB_RESTORE_EMPTY,
            STATUS_FOLDER_CREATED,
            ERR_FOLDER_CREATE,
            STATUS_PATH_COPIED,
            STATUS_MISSING_COUNT_SUFFIX,
            STATUS_UNDO_EMPTY,
            STATUS_UNDO_EXISTS_SUFFIX,
            STATUS_REDO_EMPTY,
            STATUS_REDO_FAILED,
            STATUS_RENAME_FAILED,
            STATUS_TARGET_MISSING,
            STATUS_TRASH_ALL_FAILED,
            STATUS_TRASH_NO_HIDDEN,
            STATUS_TRASH_NO_SORT,
            STATUS_TRASH_PARTIAL_FAILED,
            STATUS_RESTORE_FAILED,
            STATUS_THEME_CHANGED_PREFIX,
        ] {
            assert!(!message.is_empty(), "status message must not be empty");
        }
    }

    #[test]
    fn status_priority_keeps_errors_above_notifications() {
        assert_eq!(STATUS_PRIORITY.first(), Some(&"エラー"));
        assert_eq!(STATUS_PRIORITY.last(), Some(&"監視状態"));
        let mut seen = std::collections::HashSet::new();
        for label in STATUS_PRIORITY {
            assert!(seen.insert(*label), "duplicate priority label: {label}");
        }
    }

    #[test]
    fn success_and_failure_conflict_keeps_success_visible() {
        // 成功項目がある失敗は「成功 + 一部失敗」の併記に解決し、失敗のみに
        // 落とさない。キャンセルは完了分岐で最も優先される。
        assert_eq!(OP_RESULT_PRIORITY[0], "キャンセル");
        assert_eq!(OP_RESULT_PRIORITY[1], "一部失敗");
        assert_eq!(OP_RESULT_PRIORITY[3], "成功");
        assert!(STATUS_PARTIAL_FAILURE_SUFFIX.contains("一部失敗"));
        assert!(STATUS_OP_FAILED_SUFFIX.starts_with("できません"));
        // 進行中と完了は別フェーズで、同じ文言に潰してはならない。
        assert_ne!(STATUS_CANCELLED, STATUS_CANCELLING);
    }
}
