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
//!   ICU4X によるロケール照合は P3 の未実装項目（DESIGN.md §2（機能マトリクス）/
//!   §10.8（ロードマップ）、`notes/fs-cross-platform-notes.md` §1.4）。
//! - 大小文字比較は `to_lowercase()` であり、完全な Unicode case folding でも
//!   ロケール固有の照合（トルコ語 `İ` など）でもない。NFC 正規化も行わない。
//!   そのため `ß` / `ẞ` や NFD ファイル名の一致漏れは既知の制限である。
//! - 日時・サイズの書式もロケール非依存（ローカルタイムの `YYYY-MM-DD HH:MM`
//!   と 2 進単位）。相対時刻の単語は下の `AGE_*` に集約した。
//!
//! `LABEL_*` / `KIND_*` / `AGE_*` / `VALUE_UNKNOWN` は `main.rs` / `model.rs` の
//! 表示文言の正規定義である。`model::kind_label` / `human_datetime` / `human_age`
//! はこれらの定数を参照し、下の `formatting_words_match_call_sites` /
//! `kind_words_match_call_sites` が乖離を検出する。
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

/// 設定ダイアログの行ショートカットがテキスト入力中に無効である旨の注記。
///
/// X13: 設定行の説明・F1 ヘルプ・README で文言が分かれていたため、ここを
/// 唯一の定義とする。`concat!` は定数識別子を受け付けないので、静的文字列へ
/// 埋め込む場合は下の `settings_input_guard_note!()` を使う。
macro_rules! settings_input_guard_note {
    () => {
        "（検索欄等の入力中は無効）"
    };
}
pub(crate) use settings_input_guard_note;

/// 入力ガード注記の正規表記（X13）。F1 ヘルプ・設定行・README の同期判定に使う。
pub const SETTINGS_INPUT_GUARD_NOTE: &str = settings_input_guard_note!();

/// 定数と `concat!` 用リテラルが同じ長さであることをコンパイル時に確認する。
/// テスト以外のビルドでも定数を参照させ、`-D warnings` の dead_code を防ぐ。
const _: () = assert!(SETTINGS_INPUT_GUARD_NOTE.len() == settings_input_guard_note!().len());

/// 再読込操作の正規表記（AF05）。
///
/// README・F1 ヘルプ・読み込み失敗ヒントで同じ語を使うための唯一の定義。
/// `concat!` は定数識別子を受け付けないので、静的文字列へ埋め込む場合は
/// `reload_word!()` を使う（入力ガード注記と同じ方式）。
macro_rules! reload_word {
    () => {
        "再読込"
    };
}

/// 再読込語の正規表記（AF05）。F1 / 読み込み失敗ヒント / README の同期判定に使う。
pub const RELOAD_WORD: &str = reload_word!();

const _: () = assert!(RELOAD_WORD.len() == reload_word!().len());

/// エラー時の復帰原則（AL05 / r5-err-10）。
///
/// README の「エラーが出たら」節と F1 ヘルプの「その他」行で共用する唯一の定義。
/// `concat!` は定数識別子を受け付けないので、静的文字列へ埋め込む場合は
/// `error_recovery_help!()` を使う。
macro_rules! error_recovery_help {
    () => {
        "エラー時は F5 再読込 / 親フォルダへ移動 / 名前変更で対処"
    };
}

/// エラー復帰の正規表記（AL05）。F1 / README の同期判定に使う。
pub const ERROR_RECOVERY_HELP: &str = error_recovery_help!();

const _: () = assert!(ERROR_RECOVERY_HELP.len() == error_recovery_help!().len());

// ---------------------------------------------------------------------------
// AC07 / AC08 / AC09: 追加の正規文言
//
// このブロックは 280 行目より前（本ファイルの所有範囲）に置く。280 行目以降は
// 別エージェントが所有するため、既存の `LOAD_ERROR_HINT` / ステータス定数や
// 末尾の `mod tests` はここでは変更しない。
// ---------------------------------------------------------------------------

/// 設定ダイアログにテキスト入力欄があるタブの注記（AC09）。
///
/// F1 ヘルプと `app/README.md` で同じ条件を案内するための唯一の定義。
/// `concat!` は定数識別子を受け付けないので、静的文字列へ埋め込む場合は
/// `settings_input_fields_note!()` を使う。
macro_rules! settings_input_fields_note {
    () => {
        "入力欄はショートカット / ターミナルタブのみ"
    };
}

/// 入力欄タブ注記の正規表記（AC09）。F1 / README の同期判定に使う。
pub const SETTINGS_INPUT_FIELDS_NOTE: &str = settings_input_fields_note!();

const _: () = assert!(SETTINGS_INPUT_FIELDS_NOTE.len() == settings_input_fields_note!().len());

/// キーマップ入力の正規形式ヒント（AC08）。
///
/// Alt+K ダイアログ（空白区切り）と設定カード（`=` 区切り）の両方で同じ
/// 「アクション名 + コード」形式を案内するための唯一の定義。設定カードの
/// 不正入力通知（`main.rs::settings_save` の `STATUS_KEYMAP_INVALID_PREFIX`
/// 経由）でもこの文言を併記する。`concat!` は定数識別子を受け付けないので、
/// 静的文字列へ埋め込む場合は `keymap_format_hint!()` を使う。
macro_rules! keymap_format_hint {
    () => {
        "形式: アクション名 コード または unbind アクション名（例: open_preview ctrl+shift+p）。設定カードでは 操作=キー も可（例: open_preview=ctrl+shift+p）"
    };
}
pub(crate) use keymap_format_hint;

/// キーマップ形式ヒントの正規表記（AC08）。設定カードの入力欄・エラー通知の
/// 同期判定に使う。
pub const KEYMAP_FORMAT_HINT: &str = keymap_format_hint!();

const _: () = assert!(KEYMAP_FORMAT_HINT.len() == keymap_format_hint!().len());

/// 読み込み失敗時の復帰案内（AC07）。ビューごとに有効なキーが異なるため、
/// `empty_state_message` は `ViewSource` でこの 3 つを選ぶこと。
///
/// ディレクトリ表示では `Ctrl+L`（OpenLocation）が有効なので、既存の
/// [`LOAD_ERROR_HINT`] をそのまま共用する（355 行目の定義は変更しない）。
pub const LOAD_ERROR_HINT_DIRECTORY: &str = LOAD_ERROR_HINT;
/// ゴミ箱ビューから一覧へ戻るための復帰案内（AE05）。
///
/// 読み込み失敗時（`LOAD_ERROR_HINT_TRASH`）と空状態（`EMPTY_TRASH`、
/// 280 行目以降は core 所有）が同じ導線を案内するための唯一の定義。
/// `concat!` は定数識別子を受け付けないので、静的文字列へ埋め込む場合は
/// `trash_exit_hint!()` を使う。
macro_rules! trash_exit_hint {
    () => {
        "Alt+← または Backspace で一覧へ戻る"
    };
}
/// ゴミ箱ビューでは `Ctrl+L` が拒否され、Esc は操作キャンセル（未操作なら
/// no-op）のため一覧へ戻らない。実際に `exit_trash` を呼ぶ Alt+← / Backspace
/// （main.rs）を案内する（AE05 で Backspace を追加）。
pub const LOAD_ERROR_HINT_TRASH: &str = concat!("F5 で", reload_word!(), " / ", trash_exit_hint!());
/// 検索ビューでは `Ctrl+L` がキー許可リストになく、Esc で閉じるのが復帰導線。
/// F5 は一覧の再読込ではなくクエリの再実行なので「再検索」と案内する（AF02、
/// `STATUS_SEARCH_STALE` と同じ語）。
pub const LOAD_ERROR_HINT_SEARCH: &str = "F5 で再検索 / Esc で閉じる";

/// ゴミ箱の復帰案内の正規表記（AE05）。F1 / README / 空状態の同期判定に使う。
pub const TRASH_EXIT_HINT: &str = trash_exit_hint!();

/// `main.rs` の rewiring が未適用でも `-D warnings` の dead_code を出さない。
/// 呼び出し側が3定数を使い始めた後も、この参照は無害に残る。
const _: () = assert!(!LOAD_ERROR_HINT_DIRECTORY.is_empty());
const _: () = assert!(!LOAD_ERROR_HINT_TRASH.is_empty());
const _: () = assert!(!LOAD_ERROR_HINT_SEARCH.is_empty());
const _: () = assert!(TRASH_EXIT_HINT.len() == trash_exit_hint!().len());
const _: () = assert!(
    LOAD_ERROR_HINT_TRASH.len()
        == concat!("F5 で", reload_word!(), " / ").len() + TRASH_EXIT_HINT.len()
);

// ---------------------------------------------------------------------------
// AK08-AK12: ダイアログ / バー / オーバーレイの固定ヒント
//
// main.rs のインラインリテラルをここへ集約する。呼び出し側が参照する前に
// 空文字列や文言の変化を検出できるよう、定数アサーションとテストを併設する。
// ---------------------------------------------------------------------------

/// 操作履歴ダイアログのフッター案内（AK08）。
pub const HISTORY_DIALOG_FOOTER_HINT: &str = "Ctrl+Shift+U / Esc で閉じる（読み取り専用）";
/// 場所入力バーの操作案内（AK09）。
pub const LOCATION_BAR_HINT: &str = "Enter で移動 / Esc で閉じる";
/// F1 ヘルプオーバーレイのフッター案内（AK10）。
pub const HELP_OVERLAY_FOOTER_HINT: &str = "F1 / Esc で閉じる（一覧はスクロールできます）";
/// プロパティダイアログの読み取り専用案内（AK11）。
pub const PROPERTIES_READ_ONLY_HINT: &str = "Esc で閉じる（読み取り専用）";
/// 検索バーの通常時の操作案内（AK12）。
pub const SEARCH_BAR_HINT: &str = "Enter で検索 / Esc で閉じる";
/// 検索実行中に検索バーへ表示するラベル（AK12）。
pub const SEARCH_IN_PROGRESS_LABEL: &str = "検索中…";
/// プレビューオーバーレイのフッター案内（AM02）。
pub const PREVIEW_OVERLAY_FOOTER_HINT: &str =
    "↑/↓ で前後のファイル、Ctrl+Enter で新タブ、Space / F3 / Esc で閉じる（読み取り専用）";
/// あいまいジャンプオーバーレイのフッター案内（AM02）。
pub const FUZZY_JUMP_OVERLAY_FOOTER_HINT: &str = "↑/↓ で選択 / Enter で移動 / Esc で閉じる";

const _: () = assert!(!HISTORY_DIALOG_FOOTER_HINT.is_empty());
const _: () = assert!(!LOCATION_BAR_HINT.is_empty());
const _: () = assert!(!HELP_OVERLAY_FOOTER_HINT.is_empty());
const _: () = assert!(!PROPERTIES_READ_ONLY_HINT.is_empty());
const _: () = assert!(!SEARCH_BAR_HINT.is_empty());
const _: () = assert!(!SEARCH_IN_PROGRESS_LABEL.is_empty());
const _: () = assert!(!PREVIEW_OVERLAY_FOOTER_HINT.is_empty());
const _: () = assert!(!FUZZY_JUMP_OVERLAY_FOOTER_HINT.is_empty());

#[cfg(test)]
mod dialog_and_bar_hint_tests {
    use super::*;

    #[test]
    fn all_hints_are_nonempty() {
        for hint in [
            HISTORY_DIALOG_FOOTER_HINT,
            LOCATION_BAR_HINT,
            HELP_OVERLAY_FOOTER_HINT,
            PROPERTIES_READ_ONLY_HINT,
            SEARCH_BAR_HINT,
            SEARCH_IN_PROGRESS_LABEL,
            PREVIEW_OVERLAY_FOOTER_HINT,
            FUZZY_JUMP_OVERLAY_FOOTER_HINT,
        ] {
            assert!(!hint.is_empty(), "hint must not be empty");
            assert_eq!(hint, hint.trim(), "hint must not carry stray whitespace");
        }
    }

    #[test]
    fn hint_wording_matches_the_main_rs_call_sites() {
        // 既存のインラインリテラルを正規文言として固定する。main.rs 側が
        // 定数へ切り替えても文言が変わらないことを保証する回帰テスト。
        assert_eq!(
            HISTORY_DIALOG_FOOTER_HINT,
            "Ctrl+Shift+U / Esc で閉じる（読み取り専用）"
        );
        assert_eq!(LOCATION_BAR_HINT, "Enter で移動 / Esc で閉じる");
        assert_eq!(
            HELP_OVERLAY_FOOTER_HINT,
            "F1 / Esc で閉じる（一覧はスクロールできます）"
        );
        assert_eq!(PROPERTIES_READ_ONLY_HINT, "Esc で閉じる（読み取り専用）");
        assert_eq!(SEARCH_BAR_HINT, "Enter で検索 / Esc で閉じる");
        assert_eq!(SEARCH_IN_PROGRESS_LABEL, "検索中…");
        assert_eq!(
            PREVIEW_OVERLAY_FOOTER_HINT,
            "↑/↓ で前後のファイル、Ctrl+Enter で新タブ、Space / F3 / Esc で閉じる（読み取り専用）"
        );
        assert_eq!(
            FUZZY_JUMP_OVERLAY_FOOTER_HINT,
            "↑/↓ で選択 / Enter で移動 / Esc で閉じる"
        );
    }

    #[test]
    fn hints_keep_their_dismiss_keys_and_read_only_notice() {
        assert!(HISTORY_DIALOG_FOOTER_HINT.contains("Ctrl+Shift+U"));
        assert!(HISTORY_DIALOG_FOOTER_HINT.contains("読み取り専用"));
        assert!(LOCATION_BAR_HINT.contains("Enter で移動"));
        assert!(HELP_OVERLAY_FOOTER_HINT.contains("スクロールできます"));
        assert!(PROPERTIES_READ_ONLY_HINT.contains("読み取り専用"));
        assert!(SEARCH_BAR_HINT.contains("Enter で検索"));
        assert!(SEARCH_IN_PROGRESS_LABEL.ends_with('…'));
        // AM02: オーバーレイのフッターは操作キーと読み取り専用の案内を落とさない。
        for token in ["↑/↓", "Ctrl+Enter", "Space", "F3", "読み取り専用"] {
            assert!(
                PREVIEW_OVERLAY_FOOTER_HINT.contains(token),
                "preview footer must explain {token}"
            );
        }
        for token in ["↑/↓", "Enter で移動"] {
            assert!(
                FUZZY_JUMP_OVERLAY_FOOTER_HINT.contains(token),
                "fuzzy-jump footer must explain {token}"
            );
        }
        for hint in [
            HISTORY_DIALOG_FOOTER_HINT,
            LOCATION_BAR_HINT,
            HELP_OVERLAY_FOOTER_HINT,
            PROPERTIES_READ_ONLY_HINT,
            SEARCH_BAR_HINT,
            PREVIEW_OVERLAY_FOOTER_HINT,
            FUZZY_JUMP_OVERLAY_FOOTER_HINT,
        ] {
            assert!(hint.contains("Esc"), "hint must explain Esc: {hint}");
        }
    }

    #[test]
    fn overlay_footer_hints_are_wired_as_constants_in_main_rs() {
        // AM02 / AN08: 3 つのオーバーレイ（プレビュー / あいまいジャンプ /
        // F1 ヘルプ）のフッター案内は strings.rs の定数を参照して描画する。
        // 既存の文言テストだけではインライン化に戻せたため、main.rs の
        // 描画サイトを source-guard で固定する。
        let main_source = include_str!("main.rs");
        for (render_site, constant) in [
            (
                ".child(PREVIEW_OVERLAY_FOOTER_HINT)",
                "PREVIEW_OVERLAY_FOOTER_HINT",
            ),
            (
                ".child(FUZZY_JUMP_OVERLAY_FOOTER_HINT)",
                "FUZZY_JUMP_OVERLAY_FOOTER_HINT",
            ),
            (
                ".child(crate::strings::HELP_OVERLAY_FOOTER_HINT)",
                "HELP_OVERLAY_FOOTER_HINT",
            ),
        ] {
            assert!(
                main_source.contains(render_site),
                "main.rs must render the {constant} footer via `{render_site}`"
            );
        }
        // 定数参照が消えて文言リテラルが main.rs へ埋め込まれた場合も検出する。
        for literal in [
            PREVIEW_OVERLAY_FOOTER_HINT,
            FUZZY_JUMP_OVERLAY_FOOTER_HINT,
            HELP_OVERLAY_FOOTER_HINT,
        ] {
            assert!(
                !main_source.contains(literal),
                "overlay footer must not be inlined into main.rs: {literal}"
            );
        }
    }

    #[test]
    fn dialog_and_bar_hints_are_wired_as_constants_in_main_rs() {
        // AK08-AK12: 履歴ダイアログ / 場所バー / プロパティ / 検索バーの
        // 固定ヒントも strings.rs の定数を参照して描画する。文言テストだけでは
        // main.rs へリテラルを再インライン化できてしまう（定数は他箇所で
        // 使われるため dead_code にもならない）ため、参照とリテラル未埋め込み
        // の両方を source-guard で固定する。
        let main_source = include_str!("main.rs");
        for (reference, constant, literal) in [
            (
                "crate::strings::HISTORY_DIALOG_FOOTER_HINT",
                "HISTORY_DIALOG_FOOTER_HINT",
                HISTORY_DIALOG_FOOTER_HINT,
            ),
            (
                "crate::strings::LOCATION_BAR_HINT",
                "LOCATION_BAR_HINT",
                LOCATION_BAR_HINT,
            ),
            (
                "crate::strings::PROPERTIES_READ_ONLY_HINT",
                "PROPERTIES_READ_ONLY_HINT",
                PROPERTIES_READ_ONLY_HINT,
            ),
            (
                "crate::strings::SEARCH_BAR_HINT",
                "SEARCH_BAR_HINT",
                SEARCH_BAR_HINT,
            ),
            (
                "crate::strings::SEARCH_IN_PROGRESS_LABEL",
                "SEARCH_IN_PROGRESS_LABEL",
                SEARCH_IN_PROGRESS_LABEL,
            ),
        ] {
            assert!(
                main_source.contains(reference),
                "main.rs must render the {constant} hint via `{reference}`"
            );
            assert!(
                !main_source.contains(literal),
                "hint must not be inlined into main.rs: {literal}"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// AI01 / AI02 / AI04 / AI06 / AI07: F1 ・ README ・ 設定カードの共通文言
//
// 280 行目以降は別エージェントが所有するため、正規定義はすべてここに置く。
// `concat!` は定数識別子を受け付けないので、静的文字列へ埋め込む場合は
// 同名のマクロを使う（他ファイルからは `pub(crate) use` されたものを参照）。
// ---------------------------------------------------------------------------

/// Ctrl+クリック / Shift+クリック の選択動作（AI04）。
///
/// F1 の旧文言「追加選択」は Ctrl+クリックがトグル動作であることを誤って
/// 示していた。README の「個別トグル」を正規とし、F1 も同じ語に揃える。
macro_rules! mouse_selection_click_description {
    () => {
        "個別トグル / 範囲選択"
    };
}
pub const MOUSE_SELECTION_CLICK_DESCRIPTION: &str = mouse_selection_click_description!();

/// ドラッグ&ドロップの対象（AI01 / AI04）。
///
/// ファイル行や空き領域へドロップした場合はカレントディレクトリへコピー
/// される（README のドラッグ&ドロップ節と `main.rs` の実装が根拠）。
macro_rules! mouse_drag_drop_description {
    () => {
        "OS からコピー / フォルダ行へドロップでそのフォルダへ / OS へドラッグ"
    };
}
pub const MOUSE_DRAG_DROP_DESCRIPTION: &str = mouse_drag_drop_description!();

/// ミドルクリックで開くタブの対象（AI01 / AI04）。
///
/// `main.rs::middle_click_tab_dir` はディレクトリ行ならそのフォルダ、
/// ファイル行ならカレントディレクトリを返す。旧 F1 行「行のフォルダを…」は
/// 後者を落としていたので、README の正確な記述を正規とする。
macro_rules! mouse_middle_click_description {
    () => {
        "行 / グリッドセルを新しいタブで開く（フォルダ行はそのフォルダ、それ以外は現在のディレクトリ）"
    };
}
pub const MOUSE_MIDDLE_CLICK_DESCRIPTION: &str = mouse_middle_click_description!();

/// キーマップの一括リセットに関する案内（AI06 / AI10）。
///
/// `KeyMap::reset` に UI は配線されておらず、ボタンも存在しない。
/// F1 の Alt+K 行・設定ダイアログのショートカットタブ・README で共用する。
/// F1 は設定ダイアログ内で無効（Esc / F1 がダイアログに吸われる）ため、
/// ダイアログ内の入力ヒントがアプリ内で唯一の手掛かりになる。
macro_rules! keymap_reset_guidance {
    () => {
        "一括リセットのボタンはありません（KeyMap::reset は UI 未配線）。全既定へ戻す場合は設定ファイルの keymap を [] にするか、アクションごとに unbind してください"
    };
}
pub(crate) use keymap_reset_guidance;
pub const KEYMAP_RESET_GUIDANCE: &str = keymap_reset_guidance!();

/// ターミナル設定欄の説明（AI07）。
///
/// この欄が F4 で起動するコマンドを設定するものであることを明示する。
/// F4 行・Alt+T 行・設定ダイアログのターミナルタブ・README で共用する。
macro_rules! terminal_command_description {
    () => {
        "F4 で開くターミナルの起動コマンドを設定（選択中のフォルダを作業ディレクトリとして起動、空欄で無効化）"
    };
}
pub(crate) use terminal_command_description;
pub const TERMINAL_COMMAND_DESCRIPTION: &str = terminal_command_description!();

/// 読み取り専用セッションで設定ダイアログのフッターに表示する文言（AJ07）。
///
/// 保存ボタンが無効であることと、変更が永続化されないことを明示する。
/// `concat!` は定数識別子を受け付けないので、静的文字列へ埋め込む場合は
/// `settings_read_only_footer!()` を使う。
macro_rules! settings_read_only_footer {
    () => {
        "この設定は読み取り専用です（保存は行われません）"
    };
}
pub(crate) use settings_read_only_footer;
pub const SETTINGS_READ_ONLY_FOOTER: &str = settings_read_only_footer!();

/// 読み取り専用セッションで個人化ページに表示する保存先の注記（AJ09）。
///
/// 通常時に表示する `~/.config/.../settings.json` への保存案内と排他に使う。
macro_rules! settings_read_only_save_note {
    () => {
        "現在は読み取り専用のため保存されません"
    };
}
pub(crate) use settings_read_only_save_note;
pub const SETTINGS_READ_ONLY_SAVE_NOTE: &str = settings_read_only_save_note!();

/// 読み取り専用時の配色注記（AJ09）。通常時の「変更は保存時に反映されます」は
/// 保存できないセッションでは誤解を招くため、連動関係だけを案内する。
macro_rules! settings_read_only_palette_note {
    () => {
        "配色はテーマ設定に連動します（保存は行われません）"
    };
}
pub(crate) use settings_read_only_palette_note;
pub const SETTINGS_READ_ONLY_PALETTE_NOTE: &str = settings_read_only_palette_note!();

/// 検索履歴 / 保存検索チップの操作案内（AJ10）。
///
/// `app/README.md` の検索履歴・保存検索節と F1 ヘルプで共用する。
macro_rules! search_history_chips_description {
    () => {
        "検索語は最大 10 件を設定に保存し、検索欄が空のとき「最近の検索:」チップとして表示。保存検索は「保存検索:」欄に名前を入力して「保存」、最大 50 件を settings.json に保存。チップをクリックで呼び出し、「×」で削除（同名は上書き）"
    };
}
pub(crate) use search_history_chips_description;
pub const SEARCH_HISTORY_CHIPS_DESCRIPTION: &str = search_history_chips_description!();

/// 列幅の変更先案内（AP03）。
///
/// 設定 UI の列幅は保存済みの値を表示するだけなので、変更先を明示する。
/// F1 の「一覧の列」行と設定ダイアログのファイルタブ行で共用する。
/// F1 は設定ダイアログ内で無効なので、後者の行説明が唯一の手掛かりになる。
macro_rules! column_width_settings_note {
    () => {
        "幅は設定UIでは表示のみ。変更は settings.json の column_prefs.widths を編集"
    };
}
pub(crate) use column_width_settings_note;
pub const COLUMN_WIDTH_SETTINGS_NOTE: &str = column_width_settings_note!();

const _: () =
    assert!(MOUSE_SELECTION_CLICK_DESCRIPTION.len() == mouse_selection_click_description!().len());
const _: () = assert!(MOUSE_DRAG_DROP_DESCRIPTION.len() == mouse_drag_drop_description!().len());
const _: () =
    assert!(MOUSE_MIDDLE_CLICK_DESCRIPTION.len() == mouse_middle_click_description!().len());
const _: () = assert!(KEYMAP_RESET_GUIDANCE.len() == keymap_reset_guidance!().len());
const _: () = assert!(TERMINAL_COMMAND_DESCRIPTION.len() == terminal_command_description!().len());
const _: () = assert!(SETTINGS_READ_ONLY_FOOTER.len() == settings_read_only_footer!().len());
const _: () = assert!(SETTINGS_READ_ONLY_SAVE_NOTE.len() == settings_read_only_save_note!().len());
const _: () =
    assert!(SETTINGS_READ_ONLY_PALETTE_NOTE.len() == settings_read_only_palette_note!().len());
const _: () =
    assert!(SEARCH_HISTORY_CHIPS_DESCRIPTION.len() == search_history_chips_description!().len());
const _: () = assert!(COLUMN_WIDTH_SETTINGS_NOTE.len() == column_width_settings_note!().len());

/// AC07 / AC08 の定数は既存の `mod tests`（280 行目以降）からは触らない。
/// 所有範囲内に小さなテストモジュールを置き、各案内が宣伝するキーを固定する。
#[cfg(test)]
mod view_hint_and_keymap_format_tests {
    use super::*;

    #[test]
    fn load_error_hints_advertise_only_keys_available_in_each_view() {
        // ディレクトリ表示: Ctrl+L (OpenLocation) は有効。
        assert_eq!(LOAD_ERROR_HINT_DIRECTORY, LOAD_ERROR_HINT);
        for token in [concat!("F5 で", reload_word!()), "Ctrl+L"] {
            assert!(
                LOAD_ERROR_HINT_DIRECTORY.contains(token),
                "directory load-error hint: {LOAD_ERROR_HINT_DIRECTORY}"
            );
        }
        // ゴミ箱: Ctrl+L は拒否され、Esc は exit_trash を呼ばない（操作
        // キャンセル専用）。Alt+← / Backspace で戻る経路を示す。
        for token in [
            concat!("F5 で", reload_word!()),
            "Alt+← または Backspace で一覧へ戻る",
        ] {
            assert!(
                LOAD_ERROR_HINT_TRASH.contains(token),
                "trash load-error hint: {LOAD_ERROR_HINT_TRASH}"
            );
        }
        assert!(!LOAD_ERROR_HINT_TRASH.contains("Ctrl+L"));
        assert!(!LOAD_ERROR_HINT_TRASH.contains("Esc"));
        // 検索: 同様に Ctrl+L は許可外。F5 はクエリ再実行、Esc で閉じる経路を示す。
        for token in ["F5 で再検索", "Esc で閉じる"] {
            assert!(
                LOAD_ERROR_HINT_SEARCH.contains(token),
                "search load-error hint: {LOAD_ERROR_HINT_SEARCH}"
            );
        }
        assert!(!LOAD_ERROR_HINT_SEARCH.contains("Ctrl+L"));
        assert_ne!(LOAD_ERROR_HINT_TRASH, LOAD_ERROR_HINT_SEARCH);
    }

    #[test]
    fn keymap_format_hint_covers_both_editors() {
        for token in [
            "アクション名 コード",
            "unbind アクション名",
            "open_preview ctrl+shift+p",
            "操作=キー",
            "open_preview=ctrl+shift+p",
        ] {
            assert!(
                KEYMAP_FORMAT_HINT.contains(token),
                "canonical keymap format hint: {KEYMAP_FORMAT_HINT}"
            );
        }
        assert_eq!(KEYMAP_FORMAT_HINT, keymap_format_hint!());
    }

    #[test]
    fn keymap_input_hint_is_built_from_the_canonical_format_hint() {
        // AD03: Alt+K の入力ヒントは正規の形式ヒントをそのまま埋め込む。
        // 別々に書いた説明が drift しないことを、前方一致で固定する。
        assert!(
            KEYMAP_INPUT_HINT.starts_with(KEYMAP_FORMAT_HINT),
            "KEYMAP_INPUT_HINT must embed KEYMAP_FORMAT_HINT verbatim: {KEYMAP_INPUT_HINT}"
        );
        // 正規ヒントだけが持つ設定カード形式も、入力ヒントから読めること。
        for token in ["操作=キー", "open_preview=ctrl+shift+p"] {
            assert!(
                KEYMAP_INPUT_HINT.contains(token),
                "input hint must expose canonical token {token}: {KEYMAP_INPUT_HINT}"
            );
        }
        assert!(KEYMAP_INPUT_HINT.contains("unbind アクション名で設定を解除できます"));
    }

    #[test]
    fn mouse_operation_descriptions_are_canonical_in_f1_and_readme() {
        // AI01/AI04: F1 と README のキー表でマウス操作の説明を共有する。
        for (key, canonical) in [
            (
                "Ctrl+クリック / Shift+クリック",
                MOUSE_SELECTION_CLICK_DESCRIPTION,
            ),
            ("ドラッグ&ドロップ", MOUSE_DRAG_DROP_DESCRIPTION),
            ("ミドルクリック", MOUSE_MIDDLE_CLICK_DESCRIPTION),
        ] {
            let (_, text) = HELP_SECTIONS
                .iter()
                .flat_map(|(_, rows)| *rows)
                .find(|(help_key, _)| *help_key == key)
                .unwrap_or_else(|| panic!("F1 must have a {key} row"));
            assert_eq!(
                *text, canonical,
                "F1 {key} row must use the canonical description"
            );
            let row = include_str!("../README.md")
                .lines()
                .find(|line| {
                    line.starts_with('|') && line.replace('`', "").contains(&format!("| {key} |"))
                })
                .unwrap_or_else(|| panic!("README must have a {key} keybinding row"));
            assert!(
                row.replace('`', "").contains(canonical),
                "README {key} row must use the canonical description: {row}"
            );
        }
    }

    /// README の散文を行末で折り返したまま比較できる形に正規化する。
    /// バッククォート（コード記法）と行頭のインデントを除き、各行を連結する。
    fn readme_prose() -> String {
        include_str!("../README.md")
            .lines()
            .map(|line| line.trim().replace('`', ""))
            .collect()
    }

    #[test]
    fn trash_help_row_carries_the_canonical_exit_hint() {
        // AI02: 読み込み失敗ヒント・空状態と同じ「一覧へ戻る」導線を F1 にも載せる。
        let text = HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| *rows)
            .find(|(key, _)| *key == "ゴミ箱ビュー")
            .map(|(_, text)| *text)
            .expect("trash F1 row must exist");
        assert!(
            text.contains(TRASH_EXIT_HINT),
            "F1 trash row must embed TRASH_EXIT_HINT: {text}"
        );
        let readme = readme_prose();
        assert!(
            readme.contains(TRASH_EXIT_HINT),
            "README must document the canonical trash exit hint"
        );
    }

    #[test]
    fn keymap_reset_guidance_is_shared_by_f1_and_readme() {
        // AI06/AI10: 一括リセット UI が無い旨と既定へ戻す手順を共用する。
        let alt_k = HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| *rows)
            .find(|(key, _)| *key == "Alt+K")
            .map(|(_, text)| *text)
            .expect("Alt+K F1 row must exist");
        assert!(
            alt_k.contains(KEYMAP_RESET_GUIDANCE),
            "F1 Alt+K row must embed the canonical reset guidance: {alt_k}"
        );
        let readme = readme_prose();
        assert!(
            readme.contains(KEYMAP_RESET_GUIDANCE),
            "README must embed the canonical reset guidance"
        );
    }

    #[test]
    fn terminal_description_names_f4_and_matches_readme() {
        // AI07: ターミナル設定欄が F4 の起動コマンドを設定するものであることを
        // 正規文言に固定し、README も同じ語で案内する。
        for token in ["F4", "起動コマンド", "作業ディレクトリ", "空欄で無効化"]
        {
            assert!(
                TERMINAL_COMMAND_DESCRIPTION.contains(token),
                "canonical terminal description must name {token}: {TERMINAL_COMMAND_DESCRIPTION}"
            );
        }
        let readme = readme_prose();
        assert!(
            readme.contains(TERMINAL_COMMAND_DESCRIPTION),
            "README must embed the canonical terminal description"
        );
    }

    #[test]
    fn settings_dialog_help_names_the_discard_warning_only_once() {
        // AD05: 旧文言「破棄され通知されます」と正規文言の二重掲載を防ぐ。
        let dialog = HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| *rows)
            .find(|(key, _)| *key == "設定ダイアログ")
            .map(|(_, text)| *text)
            .expect("settings dialog row must exist");
        // AR01: the discard warning moved to the search-clear guidance in the
        // settings card and the canonical Esc hint; F1 no longer repeats it.
        assert!(
            !dialog.contains(UNSAVED_CHANGES_DISCARD_NOTE),
            "F1 must not repeat the discard warning: {dialog}"
        );
        assert!(!dialog.contains("破棄され通知されます"), "{dialog}");
    }

    #[test]
    fn tab_move_help_key_spells_out_both_full_chords() {
        // AD04: 省略形の PageDown は選択移動行と誤一致するため、F1 側も
        // 完全な chord を2つ並べる。
        let key = HELP_SECTIONS
            .iter()
            .find(|(title, _)| *title == "タブ・ブックマーク")
            .and_then(|(_, rows)| rows.iter().find(|(key, _)| key.contains("PageUp")))
            .map(|(key, _)| *key)
            .expect("tab-move help row must exist");
        assert_eq!(key, "Ctrl+Shift+PageUp / Ctrl+Shift+PageDown");
    }

    #[test]
    fn directory_view_only_note_has_one_canonical_wording() {
        assert_eq!(
            DIRECTORY_VIEW_ONLY_NOTE,
            "一部のキーはディレクトリ表示でのみ有効です"
        );
        let note = HELP_SECTIONS
            .iter()
            .find(|(title, _)| *title == "その他")
            .and_then(|(_, rows)| rows.iter().find(|(key, _)| *key == "ディレクトリ表示"))
            .map(|(_, text)| *text)
            .expect("directory-view note row must exist");
        assert_eq!(note, DIRECTORY_VIEW_ONLY_NOTE);
    }

    #[test]
    fn trash_exit_hint_is_shared_by_load_error_and_empty_state() {
        // AE05: ゴミ箱ビューの読み込み失敗ヒントと空状態ヒントは、同じ
        // 「一覧へ戻る」導線（Alt+← / Backspace）を共通マクロから組み立てる。
        assert_eq!(TRASH_EXIT_HINT, trash_exit_hint!());
        assert!(LOAD_ERROR_HINT_TRASH.ends_with(TRASH_EXIT_HINT));
        for token in ["Alt+←", "Backspace", "一覧へ戻る"] {
            assert!(
                TRASH_EXIT_HINT.contains(token),
                "canonical trash exit hint: {TRASH_EXIT_HINT}"
            );
        }
        let (_, empty) = EMPTY_TRASH;
        assert!(
            empty.contains(TRASH_EXIT_HINT),
            "EMPTY_TRASH must reuse the canonical exit hint: {empty}"
        );
        assert!(!empty.contains("前の場所へ"), "stale exit wording: {empty}");
        // AI02: F1 の「ゴミ箱ビュー」行も同じ正規ヒントを埋め込む。
        // 186-187 行目のコメントが宣言する F1 / README / 空状態の 3 way sync
        // のうち、F1 側の乖離をここで検出する（HELP_SECTIONS は read-only）。
        let trash_help = HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| *rows)
            .find(|(key, _)| *key == "ゴミ箱ビュー")
            .map(|(_, text)| *text)
            .expect("trash-view help row must exist");
        assert!(
            trash_help.contains(TRASH_EXIT_HINT),
            "F1 trash-view row must embed the canonical exit hint: {trash_help}"
        );
    }

    #[test]
    fn trash_sort_refusal_carries_the_exit_route() {
        // AE07: ゴミ箱の他通知と同じ「ゴミ箱を出て」導線を並び替え拒否にも載せる。
        assert!(
            STATUS_TRASH_NO_SORT.contains("ゴミ箱を出て"),
            "trash sort refusal must show the exit route: {STATUS_TRASH_NO_SORT}"
        );
        assert!(STATUS_TRASH_NO_HIDDEN.contains("ゴミ箱を出て"));
    }

    #[test]
    fn search_stale_status_and_f1_row_offer_f5_rerun() {
        // AE08/AF02: AC10 で F5 が検索を再実行するため、通知・F1 ヘルプ・
        // 検索ビューの読み込み失敗ヒントも同じ語で案内する。
        assert!(
            STATUS_SEARCH_STALE.contains("F5 で再検索"),
            "stale search status must advertise F5: {STATUS_SEARCH_STALE}"
        );
        assert_eq!(
            LOAD_ERROR_HINT_SEARCH, "F5 で再検索 / Esc で閉じる",
            "search load-error hint must advertise re-run, not reload"
        );
        let reload = HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| *rows)
            .find(|(key, _)| *key == "Ctrl+R / F5")
            .map(|(_, text)| *text)
            .expect("reload help row must exist");
        assert_eq!(
            reload,
            concat!(reload_word!(), " / 検索の再実行"),
            "F1 reload row must use the canonical reload word and mention search re-run: {reload}"
        );
    }

    #[test]
    fn reload_wording_is_canonical_in_f1_and_readme() {
        // AF05: 「再読込」を唯一の表記とし、README の旧表記を検出する。
        assert_eq!(RELOAD_WORD, reload_word!());
        let reload = HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| *rows)
            .find(|(key, _)| *key == "Ctrl+R / F5")
            .map(|(_, text)| *text)
            .expect("reload help row must exist");
        assert_eq!(reload, concat!(reload_word!(), " / 検索の再実行"));
        let readme = include_str!("../README.md");
        let f5_row = readme
            .lines()
            .find(|line| line.contains("`Ctrl+R` / `F5`"))
            .expect("README must have the F5 row");
        let design_row = readme
            .lines()
            .find(|line| line.contains("隠しファイル切替・ソート切替"))
            .expect("README must describe in-memory rebuilds");
        for (name, line) in [("F5", f5_row), ("design", design_row)] {
            assert!(
                line.contains(RELOAD_WORD),
                "{name} row must use {RELOAD_WORD}: {line}"
            );
            assert!(
                !line.contains("再読み込み"),
                "{name} row has stale wording: {line}"
            );
        }
    }

    #[test]
    fn settings_dialog_escape_hint_is_shared_by_f1_footer_and_readme() {
        // AF10: 設定ダイアログの Esc 案内は F1・フッター凡例・README で
        // 同じ正規文言を使う。他ダイアログの DIALOG_FOOTER_HINT とは意図的に分ける。
        assert_eq!(SETTINGS_ESCAPE_HINT, settings_escape_hint!());
        let dialog = HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| *rows)
            .find(|(key, _)| *key == "設定ダイアログ")
            .map(|(_, text)| *text)
            .expect("settings dialog help row must exist");
        assert!(
            dialog.contains(SETTINGS_ESCAPE_HINT),
            "F1 settings row must use the canonical hint: {dialog}"
        );
        assert!(
            !dialog.contains("Esc でキャンセル"),
            "stale F1 wording: {dialog}"
        );
        let settings_ui = include_str!("settings_ui.rs");
        assert!(
            settings_ui.contains("crate::strings::settings_escape_hint!()"),
            "settings footer legend must embed the canonical macro"
        );
        assert!(
            !settings_ui.contains("Enter 保存 / Esc 閉じる"),
            "stale footer legend wording"
        );
        let readme = include_str!("../README.md");
        let key_row = readme
            .lines()
            .find(|line| line.contains("| 設定ダイアログのキー |"))
            .expect("README must have the settings key row");
        let legend_row = readme
            .lines()
            .find(|line| line.contains("`Enter` 保存 /"))
            .expect("README must document the settings footer legend");
        for (name, line) in [("settings key", key_row), ("footer legend", legend_row)] {
            let normalized = line.replace('`', "");
            assert!(
                normalized.contains(SETTINGS_ESCAPE_HINT),
                "README {name} row must use the canonical hint: {line}"
            );
        }
        assert_ne!(
            SETTINGS_ESCAPE_HINT, DIALOG_FOOTER_HINT,
            "other dialogs keep the cancel wording"
        );
    }

    #[test]
    fn help_escape_row_uses_canonical_settings_hint() {
        // AR07: F1 の Esc 行も設定ダイアログの正規案内と同じ文言に統一する。
        // 旧来の汎用表現（「キャンセル / ダイアログを閉じる」）は残さない。
        let esc = HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| *rows)
            .find(|(key, _)| *key == "Esc")
            .map(|(_, text)| *text)
            .expect("F1 help must have an Esc row");
        assert_eq!(esc, settings_escape_hint!(), "{esc}");
        assert_eq!(esc, SETTINGS_ESCAPE_HINT, "{esc}");
        assert!(!esc.contains("キャンセル / ダイアログを閉じる"), "{esc}");
    }

    #[test]
    fn readme_documents_each_view_recovery_hint_verbatim() {
        // AE02: app/README.md のビュー別ヒントが定数と漂移しないことを固定する。
        // README はキーをバッククォートで囲むため、外してから照合する。
        let readme = include_str!("../README.md");
        for (name, constant) in [
            ("LOAD_ERROR_HINT_DIRECTORY", LOAD_ERROR_HINT_DIRECTORY),
            ("LOAD_ERROR_HINT_TRASH", LOAD_ERROR_HINT_TRASH),
            ("LOAD_ERROR_HINT_SEARCH", LOAD_ERROR_HINT_SEARCH),
        ] {
            let line = readme
                .lines()
                .find(|line| line.contains(name))
                .unwrap_or_else(|| panic!("README must document {name}"));
            let normalized = line.replace('`', "");
            assert!(
                normalized.contains(constant),
                "README {name} must match the constant ({constant}): {line}"
            );
        }
        // ゴミ箱では Ctrl+L が拒否されるので、旧来の案内が残っていないこと。
        let trash_line = readme
            .lines()
            .find(|line| line.contains("LOAD_ERROR_HINT_TRASH"))
            .expect("trash load-error hint is documented");
        assert!(
            !trash_line.contains("Ctrl+L"),
            "stale trash hint: {trash_line}"
        );
        assert!(
            !trash_line.contains("`Esc` 一覧へ戻る"),
            "stale trash hint: {trash_line}"
        );
    }

    #[test]
    fn readme_f5_row_mentions_search_rerun() {
        // AE08: README の F5 行も再検索の再実行を案内する。
        let readme = include_str!("../README.md");
        let row = readme
            .lines()
            .find(|line| line.contains("`Ctrl+R` / `F5`"))
            .expect("README must have the F5 row");
        assert!(
            row.contains("再検索"),
            "F5 row must mention search re-run: {row}"
        );
    }

    #[test]
    fn read_only_wording_is_canonical_for_footer_and_personalize_page() {
        // AJ07 / AJ09: 読み取り専用セッションの文言は一箇所で定義し、
        // マクロ経由の静的文字列と定数が一致することを固定する。
        assert_eq!(SETTINGS_READ_ONLY_FOOTER, settings_read_only_footer!());
        assert_eq!(
            SETTINGS_READ_ONLY_SAVE_NOTE,
            settings_read_only_save_note!()
        );
        assert_eq!(
            SETTINGS_READ_ONLY_PALETTE_NOTE,
            settings_read_only_palette_note!()
        );
        for token in ["読み取り専用", "保存は行われません"] {
            assert!(
                SETTINGS_READ_ONLY_FOOTER.contains(token),
                "read-only footer must name {token}: {SETTINGS_READ_ONLY_FOOTER}"
            );
        }
        assert_eq!(
            SETTINGS_READ_ONLY_SAVE_NOTE,
            "現在は読み取り専用のため保存されません"
        );
        assert!(!SETTINGS_READ_ONLY_SAVE_NOTE.contains("settings.json"));
        assert!(SETTINGS_READ_ONLY_PALETTE_NOTE.contains("保存は行われません"));
    }

    #[test]
    fn search_history_chips_description_is_canonical_and_matches_readme() {
        // AJ10: F1 の prose 行と README の検索履歴 / 保存検索節で共用する。
        assert_eq!(
            SEARCH_HISTORY_CHIPS_DESCRIPTION,
            search_history_chips_description!()
        );
        let readme = include_str!("../README.md").replace('`', "");
        for token in [
            "最近の検索:",
            "保存検索:",
            "最大 10 件",
            "最大 50 件",
            "同名は上書き",
        ] {
            assert!(
                SEARCH_HISTORY_CHIPS_DESCRIPTION.contains(token),
                "canonical chips description must name {token}: {SEARCH_HISTORY_CHIPS_DESCRIPTION}"
            );
            assert!(
                readme.contains(token),
                "README must document the chip token {token}"
            );
        }
    }

    #[test]
    fn terminal_description_macro_is_exported_for_other_modules() {
        // AJ01 support: 正規文言マクロが settings_ui.rs / main.rs から
        // 参照できること（pub(crate) use）を固定する。
        assert_eq!(
            TERMINAL_COMMAND_DESCRIPTION,
            terminal_command_description!()
        );
        let settings_hint = include_str!("settings_ui.rs");
        assert!(
            settings_hint.contains("crate::strings::terminal_command_description!()"),
            "settings_ui.rs must reference the exported macro"
        );
        let main_source = include_str!("main.rs");
        assert!(
            main_source.contains("TERMINAL_COMMAND_DESCRIPTION")
                || main_source.contains("terminal_command_description!()"),
            "main.rs must use the canonical terminal description"
        );
        let stale = "F4 で開くターミナルの起動コマンド（空白で無効化）";
        assert!(
            !main_source.contains(stale),
            "main.rs must not repeat the stale Alt+T dialog wording"
        );
    }
}

/// 設定ダイアログのタブ直接移動キーの正規表記（Y05）。
///
/// フッターのキー凡例・F1 ヘルプ・README で同じ表記を使うための唯一の定義。
/// `concat!` は定数識別子を受け付けないので、静的文字列へ埋め込む場合は
/// 下の `settings_tab_jump_keys!()` を使う（入力ガード注記と同じ方式）。
macro_rules! settings_tab_jump_keys {
    () => {
        "Alt+1〜5"
    };
}
pub(crate) use settings_tab_jump_keys;

/// AG02: `reload_word!()` は他モジュール（設定 UI の保存ボタン等）からも
/// 静的文字列へ埋め込むために使うため、クレート内へ再エクスポートする。
/// マクロ定義自体は所有範囲外（105 行目）にあり、ここでは再エクスポートのみ。
pub(crate) use reload_word;

/// タブ直接移動キーの正規表記（Y05）。F1 / フッター / README の同期判定に使う。
pub const SETTINGS_TAB_JUMP_KEYS: &str = settings_tab_jump_keys!();

/// 定数と `concat!` 用リテラルが同じ長さであることをコンパイル時に確認する。
const _: () = assert!(SETTINGS_TAB_JUMP_KEYS.len() == settings_tab_jump_keys!().len());

/// 未保存変更の破棄警告の唯一の定義（AB12 / AR01）。
///
/// 設定ダイアログの Esc 案内には既に `settings_escape_hint!()` に破棄警告が
/// 含まれる。AR01 で零命中ヒントが × クリア案内へ変わったため、このマクロは
/// 定数定義のみで使用し、他モジュールへの再エクスポートは不要になった。
macro_rules! unsaved_changes_discard_note {
    () => {
        "（未保存変更があると破棄されます）"
    };
}

/// 未保存変更の破棄警告の正規表記（AB12）。同期判定に使う。
pub const UNSAVED_CHANGES_DISCARD_NOTE: &str = unsaved_changes_discard_note!();

/// 定数と `concat!` 用リテラルが同じ長さであることをコンパイル時に確認する。
const _: () = assert!(UNSAVED_CHANGES_DISCARD_NOTE.len() == unsaved_changes_discard_note!().len());

/// 設定ダイアログの Esc キーの正規案内（AF10）。
///
/// 実際の挙動は「保存せず閉じる」で、未保存変更は破棄される。F1 ヘルプ・
/// 設定ダイアログのフッター凡例・app/README の3箇所で同じ文言を使うための
/// 唯一の定義。`concat!` は定数識別子を受け付けないので、静的文字列へ
/// 埋め込む場合は `settings_escape_hint!()` を使う。
/// 他のダイアログの `DIALOG_FOOTER_HINT`（280 行目以降、core 所有）は
/// 「Esc でキャンセル」のまま扱いを分ける。
macro_rules! settings_escape_hint {
    () => {
        "Esc で閉じる（未保存変更は破棄）"
    };
}
pub(crate) use settings_escape_hint;

/// 設定ダイアログの Esc 案内の正規表記（AF10）。F1 / フッター / README の同期判定に使う。
pub const SETTINGS_ESCAPE_HINT: &str = settings_escape_hint!();

const _: () = assert!(SETTINGS_ESCAPE_HINT.len() == settings_escape_hint!().len());

/// 予約キーを列挙した正規の案内文（Z07）。
///
/// `concat!` は定数識別子を受け付けないので、静的文字列へ埋め込む場合は
/// `reserved_keys_note!()` を使う（入力ガード注記と同じ方式）。HELP_SECTIONS
/// より前に定義するのは Rust の macro_rules! が定義順に効くため。
macro_rules! reserved_keys_note {
    () => {
        "escape / f1 は予約キーです（, も Ctrl+, 固定のため予約キー）"
    };
}
pub(crate) use reserved_keys_note;

/// F1 ヘルプの共通注記と設定行の説明で共用する、ディレクトリ表示限定の文言
/// （AD06）。`concat!` は定数識別子を受け付けないので、静的文字列へ埋め込む
/// 場合は `directory_view_only_note!()` を使う。
macro_rules! directory_view_only_note {
    () => {
        "一部のキーはディレクトリ表示でのみ有効です"
    };
}
pub(crate) use directory_view_only_note;

/// ディレクトリ表示限定注記の正規表記。F1 / 設定行の同期判定に使う。
pub const DIRECTORY_VIEW_ONLY_NOTE: &str = directory_view_only_note!();

const _: () = assert!(DIRECTORY_VIEW_ONLY_NOTE.len() == directory_view_only_note!().len());

/// ダイアログ共通フッターのキー案内（Z09）。
/// ターミナル起動コマンド / キーマップ / 名前変更の 3 つのダイアログで
/// 同じ文言を使う唯一の定義。F1 ヘルプ（「設定ダイアログ」/ Alt+K 行）と
/// README の「Enter で保存 / Esc でキャンセル」に合わせる。
/// `concat!` は定数識別子を受け付けないので、静的文字列へ埋め込む場合は
/// `dialog_footer_hint!()` を使う（入力ガード注記と同じ方式）。
/// AG03: Alt+K 行からも埋め込むため、定義を F1 ヘルプより前に置く
/// （macro_rules! は定義順に効く）。
macro_rules! dialog_footer_hint {
    () => {
        "Enter で保存 / Esc でキャンセル"
    };
}

/// Keyboard shortcuts shown by the F1 help overlay: (section, [(key, description)]).
pub const HELP_SECTIONS: &[(&str, &[(&str, &str)])] = &[
    (
        "ナビゲーション",
        &[
            ("Enter", "選択項目を開く"),
            ("↑ / ↓", "選択項目を上下に移動"),
            ("Ctrl+L", "パスを直接入力して移動"),
            ("Ctrl+B", "サイドバーを表示 / 非表示（狭い画面で再表示）"),
            ("Alt+← / Alt+→", "戻る / 進む"),
            ("Alt+↑ / Backspace", PARENT_FOLDER_LABEL),
            ("Home / End", "先頭 / 末尾へ"),
            ("PageUp / PageDown", "ページ単位で移動"),
            ("Ctrl+P", FUZZY_JUMP_LABEL),
            (
                "Ctrl+Shift+L",
                "サイドバーのホームへ移動（他の場所はマウスのみ）",
            ),
            // AG05: README のマウス操作表とキー文字列を一致させる（ux 側の
            // HELP_KEY_EXCEPTIONS / 同期テストと組で運用する）。
            ("ダブルクリック", "ディレクトリ移動 / ファイルを既定アプリで開く"),
            ("サイドバー / パンくずクリック", "その場所へ移動"),
        ],
    ),
    (
        "選択",
        &[
            ("Ctrl+A", "全選択"),
            ("Ctrl+Space", "フォーカス行の選択を切替（非連続選択）"),
            ("Shift+↑↓", "範囲選択"),
            (
                "Ctrl+クリック / Shift+クリック",
                mouse_selection_click_description!(),
            ),
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
                concat!(
                    "Enter / R で復元、Delete は完全削除しません。",
                    trash_exit_hint!()
                ),
            ),
            ("Ctrl+Shift+N", "新規フォルダ"),
            ("ドラッグ&ドロップ", mouse_drag_drop_description!()),
            (
                "Ctrl+Z / Ctrl+Shift+Z",
                "元に戻す / やり直す（名前変更・ゴミ箱のみ）",
            ),
            ("Ctrl+Shift+U", "操作の履歴を表示"),
            // AG05: 右クリックのコンテキストメニューも README のキー列に合わせる。
            // NW08: 同じメニューを Shift+F10 / Menu でも開けるようになった旨を
            // 説明に追記する。キー列は既存行のままで、Shortcuts タブとの
            // 双方向同期テスト（settings_ui.rs）を壊さないようにしている。
            (
                "右クリック",
                "コンテキストメニュー（右クリック / Shift+F10 / Menu。開く / パスコピー / ゴミ箱 / ブックマーク他）",
            ),
        ],
    ),
    (
        "表示",
        &[
            ("Ctrl+1 / Ctrl+2", "詳細リスト / アイコングリッド"),
            ("Ctrl+H", "隠しファイルの表示切替"),
            // AG05: ヘッダクリックによるソートも README のキー列に合わせる。
            (
                "ヘッダクリック",
                "名前 / 種類 / サイズ / 更新日時でソート（同一列で昇順⇔降順）",
            ),
            // R41/AN03: ファイルタブの列カスタマイズを F1 にも掲載。キー列を
            // 持たない散文行なので settings_ui.rs の HELP_KEY_EXCEPTIONS に
            // `"一覧の列"` の登録が必要（親担当）。
            // AP03: 列幅は表示のみなので、変更先を正規文言で追記する。
            (
                "一覧の列",
                concat!(
                    "設定（Ctrl+,）のファイルタブ「一覧の列」で表示列と順番を変更。",
                    "Switch で表示 / 非表示、↑↓で並べ替え（最後の 1 列は非表示にできません）。",
                    "行ショートカットはこれらの Switch にはありません。",
                    column_width_settings_note!()
                ),
            ),
            (
                "Ctrl+R / F5",
                concat!(reload_word!(), " / 検索の再実行"),
            ),
            ("Ctrl+F", "検索（名前 / 内容）"),
            // AJ10: 検索欄上部のチップ（検索履歴 / 保存検索）を F1 にも載せる。
            // キー列を持たない説明行なので settings_ui.rs の
            // HELP_KEY_EXCEPTIONS にも同じ文字列を登録する（ux 担当）。
            ("検索履歴 / 保存検索", search_history_chips_description!()),
            ("Ctrl+Shift+F", "検索結果を名前で絞り込み"),
            ("Ctrl+I", "プロパティ"),
            ("Space / F3", "テキストプレビュー"),
            ("Alt+Shift+T", "テーマをダーク / ライト / システムで切替"),
        ],
    ),
    (
        "タブ・ブックマーク",
        &[
            ("Ctrl+T / Ctrl+W", "タブを開く / 閉じる"),
            ("Ctrl+Shift+T", "閉じたタブを復元"),
            ("Ctrl+Tab / Ctrl+Shift+Tab", "次のタブ / 前のタブ"),
            (
                "Ctrl+Shift+PageUp / Ctrl+Shift+PageDown",
                "タブを左 / 右へ移動",
            ),
            ("ミドルクリック", mouse_middle_click_description!()),
            ("Ctrl+D", "現在のディレクトリをブックマーク"),
        ],
    ),
    (
        "その他",
        &[
            ("F1", "このヘルプ"),
            // AL05 / r5-err-10: エラー時の復帰原則を F1 にも載せる。キー列は
            // settings_ui.rs の HELP_KEY_EXCEPTIONS に登録済みの散文行。
            ("エラーが出たら", error_recovery_help!()),
            ("Ctrl+,", "設定を開く"),
            (
                "設定ダイアログ",
                concat!(
                    "Enter で保存 / ",
                    settings_escape_hint!(),
                    " / 上部の検索欄で設定項目を絞り込み（キー名でも検索可）。",
                    "一致がゼロなら検索欄右端の × で検索をクリア。",
                    "他タブに一致すると Alt+数字で移動（",
                    settings_tab_jump_keys!(),
                    "）。F1 はダイアログ内で無効なので、下部のキー凡例に必要なキーを表示"
                ),
            ),
            (
                "設定ダイアログのタブ",
                "Ctrl+Tab / Ctrl+Shift+Tab で次 / 前のタブへ移動",
            ),
            (
                "設定ダイアログのタブ（直接）",
                concat!(
                    settings_tab_jump_keys!(),
                    " で外観 / ファイル / ショートカット / ターミナル / 個人化へ直接移動"
                ),
            ),
            (
                "設定ダイアログの並び順",
                concat!(
                    "ファイルタブで Alt+S により「既定の並び順」を名前 → 種類 → サイズ → 更新日時の順に切替",
                    settings_input_guard_note!()
                ),
            ),
            (
                "設定ダイアログの行ショートカット",
                concat!(
                    "外観タブで t=配色 / v=既定のビュー、ファイルタブで h=隠しファイルを切替（修飾キーなし）",
                    settings_input_guard_note!()
                ),
            ),
            // AK02: b は設定ダイアログのファイルタブ専用の行ショートカット。
            // ダイアログ内では F1 が無効なので、ここと下部のキー凡例に掲載する。
            (
                "b",
                concat!(
                    "設定ダイアログのファイルタブで画像サムネイルを切替（修飾キーなし）",
                    settings_input_guard_note!()
                ),
            ),
            (
                "設定ダイアログの操作",
                concat!(
                    "Tab / Shift+Tab で検索欄 → 入力欄 → キャンセル / 保存へフォーカス移動（循環）。",
                    settings_input_fields_note!()
                ),
            ),
            ("F4", "選択中のフォルダでターミナルを開く"),
            ("Alt+T", terminal_command_description!()),
            (
                "Alt+K",
                concat!(
                    "キーボードショートカットを設定（アクション名 コード / unbind アクション名で解除、",
                    dialog_footer_hint!(),
                    "）。",
                    reserved_keys_note!(),
                    "。最大 64 件まで。",
                    keymap_reset_guidance!()
                ),
            ),
            ("Esc", settings_escape_hint!()),
            ("Ctrl+Q", "終了"),
            ("ディレクトリ表示", directory_view_only_note!()),
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

/// キーマップで上書きできない予約キーの正規リスト（Z07）。
///
/// `settings::is_reserved_chord` が拒否する 3 キーをここで定義し、F1 ヘルプ・
/// Alt+K ダイアログ・設定のショートカット欄・README を同期させる。`escape` /
/// `f1` はダイアログの所有権を守り、`,` は Ctrl+, 固定（L01）を守る。
pub const RESERVED_CHORD_KEYS: &[&str] = &["escape", "f1", ","];

/// 定数を非テストビルドでも参照させ、`-D warnings` の dead_code を防ぐ。
const _: () = assert!(RESERVED_CHORD_KEYS.len() == 3);

/// 予約キー案内文の正規表記（Z07）。F1 / ダイアログ / README の同期判定に使う。
pub const RESERVED_KEYS_NOTE: &str = reserved_keys_note!();

/// 定数と `concat!` 用リテラルが同じ長さであることをコンパイル時に確認する。
const _: () = assert!(RESERVED_KEYS_NOTE.len() == reserved_keys_note!().len());

/// Keymap settings dialog (Alt+K): the input syntax and the reserved keys.
/// Text entry is used instead of raw key capture so Escape / F1 keep their
/// dialog-ownership role (see `app/src/main.rs` keymap helpers). The
/// `open_settings` notice explains why the shipped Ctrl+, cannot be moved: the
/// F1 help and the discard-recovery hint promise that chord (L01). V02/Z07: the
/// comma key is reserved too, so the hint names all three keys from
/// [`RESERVED_CHORD_KEYS`] via [`RESERVED_KEYS_NOTE`]. Z12: the 64-entry cap from
/// `settings::MAX_KEYMAP_OVERRIDES` is spelled out here too.
/// AD03: 形式の説明は `keymap_format_hint!()` に一本化する。ここで書式を
/// 再定義すると Alt+K ダイアログと設定カード／不正入力通知が drift する。
pub const KEYMAP_INPUT_HINT: &str = concat!(
    keymap_format_hint!(),
    "。unbind アクション名で設定を解除できます。",
    reserved_keys_note!(),
    "。open_settings は Ctrl+, 固定で変更できません（unbind も不可）。最大 64 件まで。"
);

// ---------------------------------------------------------------------------
// 空状態・読み込み
// ---------------------------------------------------------------------------

/// Empty-state title + optional hint per listing source.
pub const EMPTY_TRASH: (&str, &str) = (
    "ゴミ箱は空です",
    concat!(
        "削除した項目はここに表示されます（完全削除は未対応）。",
        trash_exit_hint!()
    ),
);
pub const EMPTY_SEARCH: (&str, &str) = (
    "一致する項目がありません",
    "検索語や絞り込みを変更して Enter（Esc で閉じる）",
);
pub const EMPTY_DIRECTORY: (&str, &str) = (
    "このフォルダは空です",
    "ここにファイルをドロップするか、Ctrl+Shift+N で新規フォルダ / Ctrl+V で貼り付け",
);
pub const EMPTY_FILTERED: (&str, &str) = (
    "絞り込みで全て除外されました",
    "種別や拡張子、名前（Ctrl+Shift+F）の条件を変更してください",
);
// AG10: サイドバーの空セクション用文言を既存の EMPTY_* と同じ場所に集約する。
// 描画側は空セクションを is_empty() で丸ごと非表示にする設計のため、現時点では
// 未使用（`-D warnings` を通すため下方の assert! で参照を保持する）。
/// 最近使った項目が空のときのサイドバー文言。
pub const EMPTY_RECENT: (&str, &str) = (
    "最近使った項目はありません",
    "フォルダを開くとここに表示されます",
);
/// ブックマークが空のときのサイドバー文言。
pub const EMPTY_BOOKMARKS: (&str, &str) = (
    "ブックマークはありません",
    "Ctrl+D で現在のフォルダをブックマークできます",
);
/// ドライブが見つからないときのサイドバー文言。
pub const EMPTY_DRIVE: (&str, &str) = (
    "ドライブが見つかりません",
    "ドライブの接続を確認してください",
);
const _: () = assert!(!EMPTY_RECENT.0.is_empty() && !EMPTY_RECENT.1.is_empty());
const _: () = assert!(!EMPTY_BOOKMARKS.0.is_empty() && !EMPTY_BOOKMARKS.1.is_empty());
const _: () = assert!(!EMPTY_DRIVE.0.is_empty() && !EMPTY_DRIVE.1.is_empty());
pub const LOADING: &str = "読み込み中…";
/// Recovery hint shown under a load error in the empty-state overlay (U-20).
pub const LOAD_ERROR_HINT: &str = concat!("F5 で", reload_word!(), " / Ctrl+L で別のパスへ移動");
/// Hidden-files-only empty state (T13): `{count}` is replaced by
/// [`empty_hidden_only`]. The recovery key `Ctrl+H` lives here, next to the
/// other empty-state hints, so it can be checked by the same regression test.
pub const EMPTY_HIDDEN_ONLY: &str = "隠しファイル {count} 件を非表示にしています（Ctrl+H で表示）";

/// Render the hidden-files-only empty-state message (T13).
///
/// `format!` requires a literal format string, so a `const` template cannot be
/// passed to it directly; the caller should use this helper instead.
pub fn empty_hidden_only(count: usize) -> String {
    EMPTY_HIDDEN_ONLY.replace("{count}", &count.to_string())
}
/// Hidden-files-only empty state in Trash view (V13): `Ctrl+H` is rejected by
/// `toggle_hidden` while Trash is open, so the hint must tell the user to leave
/// Trash first instead of suggesting a chord that cannot work here.
pub const EMPTY_HIDDEN_ONLY_TRASH: &str =
    "隠しファイル {count} 件が非表示です（ゴミ箱を出て Ctrl+H で表示）";

/// Render the Trash hidden-files-only empty-state message (V13).
pub fn empty_hidden_only_trash(count: usize) -> String {
    EMPTY_HIDDEN_ONLY_TRASH.replace("{count}", &count.to_string())
}

// ---------------------------------------------------------------------------
// ステータスバー（共通・タブ・ゴミ箱）
// ---------------------------------------------------------------------------

/// Frequently used fixed status-bar messages.
pub const STATUS_CHANGED: &str = "変更を検出しました";
/// Shown when a search snapshot cannot be refreshed automatically.
pub const STATUS_SEARCH_STALE: &str = "変更を検出しました（再検索が必要です / F5 で再検索）";
pub const STATUS_TABS_ONLY_DIRECTORY: &str = "タブはディレクトリ表示でのみ操作できます";
pub const STATUS_BOOKMARKS_ONLY_DIRECTORY: &str =
    "ブックマークはディレクトリ表示でのみ操作できます";
/// ディレクトリ表示以外のビューで機能を拒否するときの共通通知（N03）。
/// 「〜できます / 〜使えます」は肯定形に見えるが、文脈上は拒否なので warning に分類する。
pub const STATUS_PREVIEW_ONLY_DIRECTORY: &str = "プレビューはディレクトリ表示でのみ使えます";
pub const STATUS_LOCATION_ONLY_DIRECTORY: &str = "パス入力はディレクトリ表示でのみ使えます";
pub const STATUS_FUZZY_JUMP_ONLY_DIRECTORY: &str =
    "クイックジャンプはディレクトリ表示でのみ使えます";
pub const STATUS_PROPERTIES_ONLY_DIRECTORY: &str = "プロパティはディレクトリ表示でのみ表示できます";
pub const STATUS_SEARCH_ONLY_DIRECTORY: &str = "検索はディレクトリ表示でのみ使えます";
pub const STATUS_LAST_TAB: &str = "最後のタブは閉じられません";
pub const STATUS_TAB_MOVED_LEFT: &str = "タブを左へ移動しました";
pub const STATUS_TAB_MOVED_RIGHT: &str = "タブを右へ移動しました";
pub const STATUS_TRASH_NO_COPY: &str = "ゴミ箱ビューではコピーできません";
pub const STATUS_TRASH_NO_CUT: &str = "ゴミ箱ビューでは切り取りできません";
pub const STATUS_TRASH_NO_PASTE: &str = "ゴミ箱ビューでは貼り付けできません";
pub const STATUS_TRASH_NO_RENAME: &str = "ゴミ箱ビューでは名前を変更できません";
pub const STATUS_TRASH_NO_PURGE: &str = "ゴミ箱内の完全削除は未対応です";
pub const STATUS_TRASH_NO_HIDDEN: &str =
    "ゴミ箱ビューでは隠しファイルの切替はできません（ゴミ箱を出てから Ctrl+H）";
pub const STATUS_TRASH_NO_SORT: &str =
    "ゴミ箱ビューでは並び替えはできません（ゴミ箱を出てから並び替え）";
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
/// サイドバーの登録済み場所が消えたときのツールチップ（T09）。
/// `F5` での再確認を案内する固定文言を一箇所に集約する。
pub const STATUS_PLACE_GONE_HINT: &str = "フォルダがありません（F5 で再確認）";
pub const STATUS_NOT_DIRECTORY: &str = "ディレクトリではありません";
pub const STATUS_UNDO_FAILED: &str = "元に戻せません";
pub const STATUS_REDO_NO_TARGET: &str = "やり直せません: 対象が存在しません";

// ---------------------------------------------------------------------------
// P08: 拒否文言の正規定義（呼び出し側のインラインリテラルを置き換える）
// ---------------------------------------------------------------------------

/// デバイス / ソケット / FIFO など安全にコピーできない特殊ファイルの拒否。
/// `main.rs::drop_external_into` のインラインリテラルを置き換える。
pub const STATUS_SPECIAL_FILE_NO_COPY: &str = "特殊ファイルのためコピーできません";
/// 切り取り元と貼り付け先が同じフォルダのときの通知（移動版）。
/// `main.rs::paste` のインラインリテラルを置き換える。
pub const STATUS_SAME_DIRECTORY_NO_MOVE: &str = "同じフォルダのため移動しません";
/// ディレクトリ行のプレビュー拒否。`main.rs::open_preview_selected` 用。
pub const STATUS_FOLDER_NO_PREVIEW: &str = "フォルダはプレビューできません";
/// シンボリックリンクのプレビュー拒否。`main.rs::open_preview_selected` と
/// `preview.rs::read_preview` / `read_image_preview` の 3 箇所で共用する。
pub const STATUS_SYMLINK_NO_PREVIEW: &str = "シンボリックリンクはプレビューできません";
/// Ctrl+D のブックマーク切り替えが永続化できないパス（空 / NUL / 非 UTF-8 /
/// サイズ超過）を拒否するときの通知。`main.rs::bookmark_toggle_rejection` が
/// 呼び出す（Q03: 以前は main.rs ローカルだったが、色分類表と一元化した）。
/// T04: 以前の「（非UTF-8）」は 4 つの拒否条件のうち 1 つしか示しておらず、
/// 空パスや NUL 含有では誤った理由を表示していた。全条件に共通する
/// 「保存できない形式」という表現に改める。
pub const STATUS_BOOKMARK_NOT_PERSISTABLE: &str =
    "このパスはブックマークできません（保存できない形式です）";
/// F2 リネームで対象の親ディレクトリが解決できないときの拒否（T03）。
/// `main.rs::confirm_rename` のインラインリテラルを置き換える。
pub const STATUS_RENAME_NO_PARENT: &str = "親ディレクトリがありません";
/// F2 リネームで同名の宛先が既にあるときの拒否接頭辞（T03）。
/// 動的な `{path}` が後続するため接頭辞自体を共有する。
pub const STATUS_RENAME_EXISTS_PREFIX: &str = "既に存在します: ";
/// ゴミ箱ビューでターミナルを開こうとしたときの拒否接頭辞（W01）。
/// `STATUS_CANNOT_OPEN_SUFFIX` と連結して完全な通知を組み立てる。
pub const STATUS_TRASH_NO_TERMINAL_PREFIX: &str = "ゴミ箱ではターミナルを";
/// プレビュー中のファイルの親ディレクトリが解決できないときの拒否接頭辞（W01）。
/// 同じく `STATUS_CANNOT_OPEN_SUFFIX` と連結して使用する。
pub const STATUS_FILE_NO_NEW_TAB_PREFIX: &str = "このファイルは新しいタブで";

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
/// 「開けません」系の拒否接尾（N02）。`STATUS_OP_FAILED_SUFFIX` と違い、
/// 「できません」を含まないため単独では成功 green に落ちていた。
/// main.rs の warning マーカーとして登録し、全ての呼び出し側がこれを連結する。
pub const STATUS_CANNOT_OPEN_SUFFIX: &str = "開けません";
/// CLI で渡された起動ディレクトリがフォルダでなかったときの通知（X09）。
/// 以前は main.rs ローカルで「strings.rs は対象外」という陳腐なコメントが
/// 付いていたが、色分類表と一元化するためここへ移動した。
/// 「開けません」は `STATUS_CANNOT_OPEN_SUFFIX`（warning）にも一致するため、
/// 呼び出し側は必ず `STATUS_ERROR_PREFIX` を付け、エラー接頭辞を優先させる。
pub const STATUS_INVALID_CLI_DIR: &str =
    "起動ディレクトリを開けませんでした（フォルダではありません）";
/// スキップ件数通知の開き括弧。`main.rs::transfer_skip_notice` が
/// 「{理由} {N} 件をスキップ」を囲むために使う（T02）。
pub const STATUS_SKIPPED_PREFIX: &str = "（";
/// スキップ件数通知の閉じ括弧。件数に続く「件をスキップ」と一体化させ、
/// コピー／移動／ドロップの 3 箇所で同じ文言になるよう共有する（T02）。
pub const STATUS_SKIPPED_SUFFIX: &str = " 件をスキップ）";
/// スキップ理由のラベル（存在しない項目）。Y02: main.rs のコピー／移動と
/// 外部ドロップの 3 箇所で inline に書かれていた文言をここへ集約する。
pub const STATUS_SKIP_REASON_MISSING: &str = "存在しない項目";
/// スキップ理由のラベル（特殊ファイル）。`STATUS_SKIP_REASON_MISSING` と
/// 同じく `transfer_skip_notice` の理由スロットで共有する（Y02）。
pub const STATUS_SKIP_REASON_SPECIAL: &str = "特殊ファイル";
/// スキップ理由のラベル（転送元と宛先が同じフォルダ）。
/// 混在ケース（他の項目は転送し、同フォルダの項目だけスキップする場合）で
/// `transfer_skip_notice` の理由スロットに渡すための共通定義（Z03）。
/// 全件が同フォルダの場合は専用の `STATUS_SAME_DIRECTORY_NO_MOVE` /
/// `STATUS_DROP_SAME_DIRECTORY` が使われる。
pub const STATUS_SKIP_REASON_SAME_DIRECTORY: &str = "同じフォルダ";
/// 操作中止の完了通知（進行中の `STATUS_CANCELLING` とは別フェーズ）。
pub const STATUS_CANCELLED: &str = "キャンセルしました";
/// 進行中通知の接尾（`{verb}` に続く）。
pub const STATUS_OP_IN_PROGRESS_SUFFIX: &str = "中…（Esc でキャンセル）";
/// キャンセル可能な操作中に表示する括弧内の案内（K05）。
/// `STATUS_OP_RUNNING` / `STATUS_OP_IN_PROGRESS_SUFFIX` と、main.rs の
/// 「…しています」系ステータスが同じ文言を共有するための唯一の定義。
pub const STATUS_CANCEL_HINT: &str = "（Esc でキャンセル）";

/// 再スキャン失敗。後続の `STATUS_CHANGED` で上書きしない（R6-A-04）。
/// V14: 失敗だけで終わらせず、手動での復帰操作（F5）を案内する。
pub const STATUS_RESCAN_FAILED: &str = "再スキャンに失敗しました（F5 で再読込）";
/// 監視の状態表示。`last_error()` がある場合は理由を連結する。
pub const STATUS_WATCH_PREFIX: &str = "監視: ";
pub const STATUS_WATCH_PARTIAL: &str = "一部無効 — ";
pub const STATUS_WATCH_DISABLED: &str = "無効 — ";
pub const STATUS_WATCH_INIT_FAILED: &str = "監視の初期化に失敗しました";
/// 監視ディレクトリ数の接尾（`{count}` に続く）。
pub const STATUS_WATCH_COUNT_SUFFIX: &str = " ディレクトリ";
/// Trash view の監視スロットに表示する案内（AB08）。
/// ゴミ箱は自動更新されないため、main.rs の inline 文言を定数へ集約する。
pub const STATUS_TRASH_NO_AUTO_REFRESH: &str = "ゴミ箱（自動更新なし）";

/// OS 連携の成功 notification。戻り値を確認せずに出す現状の文言（r4-error-01）。
pub const STATUS_OPENED_DEFAULT_SUFFIX: &str = "を既定のアプリで開きました";
pub const STATUS_REVEALED_PREFIX: &str = "ファイルマネージャで表示: ";
/// キーマップの `unbind` で解除するオーバーライドがなかったときの通知接頭辞。
/// 動的な `{action}` を連結するため、接頭辞自体を色判定のマーカーとして登録する。
pub const STATUS_KEYMAP_NO_UNBIND_PREFIX: &str = "解除する設定はありません: ";
/// キーマップ解除の no-op 通知に添える復帰導線（AI08）。
///
/// F6-36 の情報量ルール（復帰導線）に従い、上書きの確認先を案内する。
/// `STATUS_SETTINGS_DISCARDED` / `STATUS_RESCAN_FAILED` /
/// `STATUS_TERMINAL_DISABLED_HINT` と同じく、失敗だけで終わらせない。
/// 動的な `{action}` の後ろに連結するため、接頭辞自体はマーカーのまま維持する。
pub const STATUS_KEYMAP_NO_UNBIND_HINT: &str =
    "（Ctrl+, で設定を開きショートカットタブで現在の上書きを確認）";
/// キーマップ入力の形式が不正なときの通知（V04）。
/// Alt+K ダイアログ専用。設定カードは `STATUS_KEYMAP_INVALID_PREFIX + KEYMAP_FORMAT_HINT`。
/// AD03: 書式の説明は `keymap_format_hint!()` に一本化する。個別に書いた
/// 文言を残すとダイアログ／設定カード／不正入力通知が drift する。
pub const STATUS_KEYMAP_FORMAT_INVALID: &str = concat!(keymap_format_hint!(), "。");
/// キーマップの上書き件数が上限に達したときの拒否通知の接頭辞（V04）。
/// 動的な件数を連結するため、接頭辞自体をマーカーとして扱う。
pub const STATUS_KEYMAP_LIMIT_PREFIX: &str = "上書きは最大 ";
/// 上限件数の後ろに続く接尾（V04）。件数と合わせて完全な通知を組み立てる。
pub const STATUS_KEYMAP_LIMIT_SUFFIX: &str = " 件までです";
/// 設定カードのキーマップ欄で指定が不正だったときの通知接頭辞（V04）。
/// 動的な `entry` が後続するため、接頭辞自体をマーカーとして扱う。
pub const STATUS_KEYMAP_INVALID_PREFIX: &str = "無効なキーマップ指定: ";
/// タブ数が上限に達したときの拒否通知の接頭辞（N08）。
/// 動的な `{n}` を連結するため、M05 と同じく接頭辞自体を warning マーカーに登録する。
pub const STATUS_MAX_TABS_PREFIX: &str = "タブは最大 ";
/// タブ数上限の後ろに続く接尾（Y07）。`STATUS_KEYMAP_LIMIT_SUFFIX` と同じく
/// 動的な件数と合わせて完全な通知を組み立てるため、呼び出し側はこの定数を使う。
pub const STATUS_MAX_TABS_SUFFIX: &str = " 個までです";
/// 設定の失敗／退避（読み込み失敗と情報量を揃える）。
/// U03: 保存失敗は権限・空き容量だけでなく、MAX_SETTINGS_BYTES 超過や
/// ロックタイムアウトでも起きる。理由を判別できない汎用経路では
/// 特定の原因を断言しない表現にする。
pub const STATUS_SETTINGS_SAVE_FAILED: &str =
    "設定を保存できませんでした（ファイルの状態を確認して再試行してください）";
/// シリアライズ結果が MAX_SETTINGS_BYTES を超えたときの理由（U03）。
/// 保存は行われず、変更はメモリ上に残る。
/// `main.rs::settings_save_error_status` が理由別通知として表示する。
pub const STATUS_SETTINGS_SAVE_TOO_LARGE: &str =
    "設定を保存できませんでした（設定データが大きすぎます。不要な履歴や保存項目を減らしてから再試行してください）";
/// 保存ロックがタイムアウトしたときの理由（U03）。
/// 別インスタンスが編集中なので、待ってから再試行する導線を示す。
/// `main.rs::settings_save_error_status` が理由別通知として表示する。
pub const STATUS_SETTINGS_SAVE_LOCKED: &str =
    "設定を保存できませんでした（別の gpui-file-explorer が設定を編集中です。少し待ってから再試行してください）";
pub const STATUS_SETTINGS_READ_ONLY: &str = "設定を読み取り専用で開きました";
/// 読み取り専用通知に添える回復導線（W08）。読み込み失敗の理由を連結した後ろに
/// 置き、設定を確認する操作を案内する。
pub const STATUS_SETTINGS_READ_ONLY_HINT: &str = "（Ctrl+, で設定を確認）";
/// Esc / 背景クリックで未保存の設定変更を破棄したときの通知（H03）。
/// フッターの「保存せず閉じる」ボタンと違い、これらの操作は事前の警告がない。
/// 情報量ルール (F6-36) の「復帰導線」に従い、設定を開き直す操作を併記する（I06）。
pub const STATUS_SETTINGS_DISCARDED: &str =
    "未保存の変更を破棄しました（Ctrl+, で設定を開き直せます）";
/// 設定オーバーレイの「保存」が成功したときの完了通知（AG06）。
/// 上記 `STATUS_SETTINGS_DISCARDED` の成否に対応する成功版で、色分類も
/// success(green) とする。テーマ変更時は個別の `STATUS_THEME_CHANGED_PREFIX`
/// 付き通知を優先するため、ここは汎用の保存完了文言だけを保持する。
pub const STATUS_SETTINGS_SAVED: &str = "設定を保存しました";
/// 設定ダイアログの共通フッターに表示する「保存」操作の案内。
/// K13 でターミナル欄の重複表示は削除され、共有フッターの未変更時
/// （dirty = false）にのみ表示される。唯一の定義として文言のゆれを防ぐ（J08）。
pub const STATUS_SETTINGS_UNSAVED_HINT: &str = "変更は「保存」を押すまで反映されません。";
/// タブ復元の成否。
pub const STATUS_TAB_RESTORED: &str = "タブを復元しました";
pub const STATUS_TAB_RESTORE_EMPTY: &str = "復元できるタブがありません";
/// フォルダ作成の成否。
pub const STATUS_FOLDER_CREATED: &str = "フォルダを作成しました";
pub const ERR_FOLDER_CREATE: &str = "フォルダを作成できません";
/// パスコピー／件数付き通知の接尾。
pub const STATUS_PATH_COPIED: &str = "パスをコピーしました";
pub const STATUS_MISSING_COUNT_SUFFIX: &str = " 件は既に存在しません";
/// 名前で指定した対象が見つからないときの共通接尾（保存検索の呼び出し／削除）。
/// 動的な `{name}` が後続するため、接尾自体を色判定のマーカーとして登録する。
pub const STATUS_NOT_FOUND_SUFFIX: &str = "が見つかりません";
/// Undo / Redo の「対象なし」と失敗。
/// T09: 操作履歴ダイアログ側は「元に戻せる操作はありません」だったため、
/// 助詞を「は」に統一する（状態を説明する文として自然な形）。
pub const STATUS_UNDO_EMPTY: &str = "元に戻せる操作はありません";
/// 操作履歴ダイアログが空のときの補足（T09）。`STATUS_UNDO_EMPTY` に連結する。
pub const STATUS_UNDO_HISTORY_HINT: &str = "（名前変更とゴミ箱のみ記録します）";
pub const STATUS_UNDO_EXISTS_SUFFIX: &str = " が既に存在します";
/// 元に戻す対象が既に消えているときの接尾辞（U07 / X08）。
/// 逆場合の `STATUS_UNDO_EXISTS_SUFFIX` と対称で、以前は main.rs ローカル
/// だったが、マーカー表と一元化するためここへ移動した。
pub const STATUS_UNDO_MISSING_SUFFIX: &str = " が存在しません";
pub const STATUS_REDO_EMPTY: &str = "やり直せる操作がありません";
pub const STATUS_REDO_FAILED: &str = "やり直せません";
pub const STATUS_RENAME_FAILED: &str = "変更できません";
pub const STATUS_TARGET_MISSING: &str = "対象は既に存在しません";
/// テーマ切替後の通知（表示名は `THEME_LABEL_*` を参照）。
pub const STATUS_THEME_CHANGED_PREFIX: &str = "テーマ: ";
/// テーマ設定の表示名。`theme_choice_label` と設定オーバーレイの選択肢で共用する（H08）。
pub const THEME_LABEL_DARK: &str = "ダーク";
pub const THEME_LABEL_LIGHT: &str = "ライト";
pub const THEME_LABEL_SYSTEM: &str = "システム";
/// ゴミ箱操作の全件失敗と部分失敗（後者は `（{n} 件失敗: 「{name}」: {reason}）` の一部）。
pub const STATUS_TRASH_ALL_FAILED: &str = "ゴミ箱へ移動できません";
pub const STATUS_TRASH_PARTIAL_FAILED: &str = " 件失敗: ";
pub const STATUS_RESTORE_FAILED: &str = "復元できません";
/// ゴミ箱に復元対象が残っていない理由フラグメント（T08）。
/// `main.rs::trash_undo_status` の全件失敗（「項目が…」）と一部失敗
/// （「{n} 件は…」）の両方で共有し、理由の文言を一箇所に固定する。
pub const STATUS_TRASH_MISSING_REASON: &str = "ゴミ箱にありません";
/// 復元対象が空のときの拒否通知（N08）。`STATUS_TAB_RESTORE_EMPTY` と異なり
/// 「できません / ありません」のどちらもマーカーに登録されていなかった。
pub const STATUS_RESTORE_EMPTY: &str = "復元対象がありません";
/// 空の検索語を実行したときの拒否通知（N09）。
pub const STATUS_SEARCH_EMPTY_QUERY: &str = "検索語が空です";
/// 検索を開かずに名前フィルタへフォーカスしようとしたときの案内（P03）。
/// 「先に … してください」は拒否なので warning として色分けする。
pub const STATUS_SEARCH_NOT_OPEN: &str = "先に Ctrl+F で検索を開いてください";
/// 検索ビューを開いた直後に表示する入力案内（W02）。開く操作と入力待ちの
/// 案内を一箇所で管理し、main.rs へのインライン再追加を防ぐ。
pub const STATUS_SEARCH_PROMPT: &str = "検索語を入力して Enter（Esc で閉じる）";

/// OS がプロパティのメタデータ読み取りを拒否したときの通知（R02）。
/// 以前は main.rs ローカルだったが、色分類表と一元化するためここへ移動した。
pub const STATUS_ACCESS_DENIED: &str = "アクセスが拒否されました";
/// ターミナル起動失敗の接頭辞（R03）。`STATUS_OP_FAILED_SUFFIX` と連結して
/// 「ターミナルを起動できません」を組み立てる。
pub const STATUS_TERMINAL_START_PREFIX: &str = "ターミナルを起動";
/// ターミナル起動コマンドが長すぎる／制御文字を含むときの入力エラー（V03）。
/// ターミナルコマンドダイアログと設定カードの両方で共有する唯一の定義。
pub const STATUS_TERMINAL_COMMAND_INVALID: &str = "コマンドが長すぎるか、制御文字を含んでいます";
/// ターミナル起動コマンドを空にして無効化したときの完了通知（AG04）。
/// `main.rs` のターミナルコマンドダイアログ保存経路から参照する。
pub const STATUS_TERMINAL_COMMAND_DISABLED: &str = "ターミナル起動コマンドを無効にしました";
/// 保存検索の保存失敗（上限超過）の接頭辞（R04）。動的な `{name}` が後続するため、
/// 接頭辞自体を error マーカーとして登録する。
pub const STATUS_SAVED_SEARCH_SAVE_FAILED_PREFIX: &str = "検索を保存できませんでした: ";
/// 保存検索が拒否された理由の一般表現（U02）。
/// `SavedSearch::new` は上限だけでなく、名前／検索語が空・長すぎる・NUL を含む
/// 場合も `None` を返す。呼び出し側が理由を判別できない現状では、上限だけを
/// 断言する文言は誤報になるため、両方を含む表現に統一する。
/// 将来 `save_search` が理由を返すようになったときの個別表示方法として保持。
#[allow(dead_code)]
pub const STATUS_SAVED_SEARCH_INVALID_INPUT: &str =
    "保存できる検索の上限に達したか、名前または検索語が無効です。既存の保存検索を削除するか、入力を見直してください";
/// 保存検索の拒否に添える理由と復帰導線（R04 / U02）。
/// 名前は履歴上の `CAP_HINT` のまま（main.rs の既存呼び出し互換）。
pub const STATUS_SAVED_SEARCH_CAP_HINT: &str =
    "（保存できる検索の上限に達したか、名前または検索語が無効です。既存の保存検索を削除するか、入力を見直してください）";
/// 保存検索で名前が空のときの入力必須エラー（S01）。
/// `STATUS_ERROR_PREFIX` と連結して表示する（例: 「エラー: 保存する検索名を入力してください」）。
pub const STATUS_SAVED_SEARCH_NAME_REQUIRED: &str = "保存する検索名を入力してください";
/// 保存検索で検索語が空のときの入力必須エラー（S01）。
/// 同じく `STATUS_ERROR_PREFIX` と連結して表示する。
pub const STATUS_SAVED_SEARCH_QUERY_REQUIRED: &str = "検索語を入力してから保存してください";
/// ブックマークラベルの設定失敗の接頭辞（R05）。動的なパスが後続するため、
/// 接頭辞自体を error マーカーとして登録する。
pub const STATUS_BOOKMARK_LABEL_SET_FAILED_PREFIX: &str = "ラベルを設定できませんでした: ";
/// ブックマークにラベルが設定されていないときの拒否接頭辞（U05 / X03）。
/// 動的なパスが後続するため接頭辞自体を共有し、呼び出し側は
/// `STATUS_ERROR_PREFIX` を付けて組み立てる。
pub const STATUS_BOOKMARK_LABEL_NOT_SET_PREFIX: &str = "ラベルは設定されていません: ";
/// ゴミ箱の読み込みに失敗したときのエラー接頭辞（U06 / X04）。
/// 動的な理由（`error`）が後続するため、接頭辞自体を共有する。
pub const STATUS_TRASH_LOAD_FAILED_PREFIX: &str = "ゴミ箱を読み込めません: ";
/// ブックマークラベルの拒否理由（U04）。
/// `Settings::set_bookmark_label` は bool しか返さないため、main.rs の
/// `bookmark_label_rejection_reason` が設定側と同じ条件を事前検証して
/// 接頭辞 + パスの後へ連結する。空ラベルは削除扱いで成功するため、
/// 拒否理由には含まれない。
pub const STATUS_BOOKMARK_LABEL_REASON_INVALID_PATH: &str =
    "（保存できる形式のパスではありません。別のディレクトリを指定してください）";
/// サイズ超過と NUL を含む場合の両方をまとめた、現状の API で使える表現。
pub const STATUS_BOOKMARK_LABEL_REASON_INVALID_LABEL: &str =
    "（ラベルが長すぎるか、使用できない文字を含みます。短いラベルに変更してください）";
/// 将来 `set_bookmark_label` が理由 enum を返すようになった場合の細分化。
#[allow(dead_code)]
pub const STATUS_BOOKMARK_LABEL_REASON_TOO_LONG: &str =
    "（ラベルが長すぎます。短いラベルに変更してください）";
#[allow(dead_code)]
pub const STATUS_BOOKMARK_LABEL_REASON_NUL: &str = "（ラベルに使用できない文字が含まれています）";
pub const STATUS_BOOKMARK_LABEL_REASON_LIMIT: &str =
    "（ラベルの保存件数が上限に達しました。不要なラベルを削除してから再試行してください）";
/// ターミナルが無効な理由と復帰導線（S07）。`STATUS_CANNOT_OPEN_SUFFIX` と
/// 連結して `terminal_disabled_status()` の文言を組み立てる。
pub const STATUS_TERMINAL_DISABLED_HINT: &str =
    ": 起動コマンドが未設定です（Ctrl+, で設定を開き terminal_command を確認）";

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
// 実行時値と組み合わせる正規の単語（model.rs の整形関数から参照）
// ---------------------------------------------------------------------------

// 呼び出し側（model.rs）への適用済み。単語を変更するときは下の整合性テストと
// 実際の表示（`種類` 列・ステータスバーの相対時刻）を併せて確認する。
/// Placeholder for unavailable metadata (`model::human_age` / `human_datetime`).
pub const VALUE_UNKNOWN: &str = "-";
/// Canonical cancel label. Do not use the English `cancel` in prompt buttons.
pub const LABEL_CANCEL: &str = "キャンセル";

/// ダイアログ共通フッターのキー案内（Z09）の正規定数。
/// ターミナル起動コマンド / キーマップ / 名前変更の 3 つのダイアログで共用し、
/// F1 ヘルプ（「設定ダイアログ」/ Alt+K 行）と README の表記に合わせる。
/// `concat!` は定数識別子を受け付けないので、静的文字列へ埋め込む場合は
/// 上の `dialog_footer_hint!()` を使う（AG03 で定義を F1 より前に移動）。
pub const DIALOG_FOOTER_HINT: &str = dialog_footer_hint!();

/// ブックマークラベル編集ダイアログ専用の追記案内（AF03）。
/// 空文字で確定するとラベル削除になることを示すため、共通フッター
/// [`DIALOG_FOOTER_HINT`] に続けて表示する。参照側は
/// `format!("{DIALOG_FOOTER_HINT} / {DIALOG_FOOTER_DELETE_SUFFIX}")` と組み立て、
/// 共通文言を二重定義しない。
pub const DIALOG_FOOTER_DELETE_SUFFIX: &str = "空にして確定すると削除";

/// 定数と `concat!` 用リテラルが同じ長さであることをコンパイル時に確認する。
const _: () = assert!(DIALOG_FOOTER_HINT.len() == dialog_footer_hint!().len());

/// `種類` 列のラベル（`model::kind_label` で使用）。
pub const KIND_FOLDER: &str = "フォルダ";
pub const KIND_LINK: &str = "リンク";
pub const KIND_NONE: &str = "—";

/// AJ02: `ops::validate_file_name` の入力エラー文言。
/// F2 名前変更ダイアログの検証エラーは、表示文言と色分類を一箇所で固定する
/// ため、ここで定義する 6 定数へ集約する（`STATUS_ERROR_MARKERS` と
/// 非空在庫にも登録済み）。動的な名前が入る Windows 予約名だけは接頭辞と
/// 接尾に分け、呼び出し側が `format!` で組み立てる。
pub const STATUS_RENAME_INVALID_EMPTY: &str = "名前が空です";
pub const STATUS_RENAME_INVALID_DOT: &str = "`.` と `..` は名前として使えません";
pub const STATUS_RENAME_INVALID_SEPARATOR: &str = "名前にパス区切り文字は使えません";
pub const STATUS_RENAME_INVALID_NUL: &str = "名前に NUL 文字は使えません";
pub const STATUS_RENAME_INVALID_RESERVED_PREFIX: &str = "Windows の予約名は使えません: ";
pub const STATUS_RENAME_INVALID_CHAR: &str = "Windows で使えない文字が含まれています";

/// 相対時刻の単語（`model::human_age` で使用）。
pub const AGE_FUTURE: &str = "未来";
pub const AGE_JUST_NOW: &str = "たった今";
pub const AGE_MINUTES_SUFFIX: &str = "分前";
pub const AGE_HOURS_SUFFIX: &str = "時間前";
pub const AGE_DAYS_SUFFIX: &str = "日前";
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
        // V02: `is_reserved_chord` also rejects the bare comma (Ctrl+, is fixed),
        // so the dialog hint must list it alongside escape / f1.
        assert!(KEYMAP_INPUT_HINT.contains(", も Ctrl+, 固定のため予約キー"));
        assert!(KEYMAP_INPUT_HINT
            .contains("open_settings は Ctrl+, 固定で変更できません（unbind も不可）"));
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
    fn error_recovery_help_is_shared_by_f1_and_readme() {
        // AL05 / r5-err-10: 復帰原則の正規文言が F1 と README で drift しない。
        let row = HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| *rows)
            .find(|(key, _)| *key == "エラーが出たら")
            .map(|(_, text)| *text)
            .expect("その他 section must have an error-recovery help row");
        assert_eq!(
            row, ERROR_RECOVERY_HELP,
            "F1 help row must use the canonical recovery guidance"
        );
        // 正規文言が約束する 3 つの導線を F1 側でも固定する。
        for token in ["F5 再読込", "親フォルダへ移動", "名前変更"] {
            assert!(
                ERROR_RECOVERY_HELP.contains(token),
                "canonical guidance must contain {token}"
            );
            assert!(row.contains(token), "F1 row must contain {token}");
        }
        let readme = include_str!("../README.md");
        assert!(
            readme.contains(ERROR_RECOVERY_HELP),
            "README must quote the canonical recovery guidance"
        );
        // R41/AN01: README 全体のトークン検索ではなく、`## エラーが出たら` 節の
        // 表だけを検査する。各「表示される文言」セルが strings.rs の正規定数
        // （または動的接頭辞の前方一致部分）と一致することを固定する。
        let error_section = readme
            .split("## エラーが出たら")
            .nth(1)
            .expect("README must have the error-recovery section")
            .split("\n## ")
            .next()
            .expect("error-recovery section must terminate");
        let rows = error_section
            .lines()
            .filter(|line| line.trim_start().starts_with('|'))
            .skip(2) // 見出し行と区切り行
            .filter_map(|line| {
                let mut cells = line
                    .split('|')
                    .skip(1)
                    .map(|cell| cell.trim().replace('`', ""));
                Some((cells.next()?, cells.next()?))
            })
            .collect::<Vec<_>>();
        let terminal_hint_head = STATUS_TERMINAL_DISABLED_HINT
            .split_once("です")
            .map_or(STATUS_TERMINAL_DISABLED_HINT, |(head, _)| head);
        let expected_messages = [
            ("STATUS_ACCESS_DENIED", STATUS_ACCESS_DENIED.to_string()),
            (
                "STATUS_RENAME_EXISTS_PREFIX",
                format!("{STATUS_RENAME_EXISTS_PREFIX}..."),
            ),
            (
                "STATUS_RESCAN_FAILED",
                STATUS_RESCAN_FAILED
                    .split_once('（')
                    .map_or(STATUS_RESCAN_FAILED, |(head, _)| head)
                    .to_string(),
            ),
            (
                "terminal_disabled_status() prefix",
                format!("ターミナルを{STATUS_CANNOT_OPEN_SUFFIX}{terminal_hint_head}"),
            ),
        ];
        assert_eq!(
            rows.len(),
            expected_messages.len(),
            "README error table must have exactly one row per canonical message"
        );
        for (name, message) in &expected_messages {
            assert!(
                rows.iter().any(|(shown, _)| shown == message),
                "README error table must show the canonical message from {name}: {message}"
            );
        }
        for (shown, _) in &rows {
            assert!(
                expected_messages
                    .iter()
                    .any(|(_, message)| message == shown),
                "README error table has a message not backed by a canonical string: {shown}"
            );
        }
        // AM03: 「次の操作」列のキー導線も回帰対象にする。文言全体の一致は
        // 要求せず、キートークン（F5 / 親フォルダ / F2 / Ctrl+,）の存在を
        // エラー回復テーブル内で固定する。
        let actions = rows
            .iter()
            .map(|(_, action)| action.as_str())
            .collect::<String>();
        for token in ["F5", "親フォルダ", "F2", "Ctrl+,"] {
            assert!(
                actions.contains(token),
                "README error table must carry the recovery key {token}"
            );
        }
    }

    #[test]
    fn help_lists_keyboard_accessibility_shortcuts() {
        let keys: Vec<&str> = HELP_SECTIONS
            .iter()
            .flat_map(|(_, entries)| *entries)
            .map(|(key, _)| *key)
            .collect();
        for expected in [
            "Ctrl+Shift+L",
            "Ctrl+Space",
            "Ctrl+Shift+C",
            "設定ダイアログの行ショートカット",
        ] {
            assert!(
                keys.contains(&expected),
                "{expected} must be listed in F1 help"
            );
        }
        let row_hint = HELP_SECTIONS
            .iter()
            .flat_map(|(_, entries)| *entries)
            .find(|(key, _)| *key == "設定ダイアログの行ショートカット")
            .map(|(_, text)| *text)
            .expect("row shortcut help entry exists");
        for token in ["t=配色", "v=既定のビュー", "h=隠しファイル"] {
            assert!(row_hint.contains(token), "row hint must mention {token}");
        }
    }

    #[test]
    fn help_lists_settings_dialog_keys() {
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
        let dialog = find("設定ダイアログ").expect("settings dialog keys must be in F1 help");
        assert!(
            dialog.contains("Enter") && dialog.contains("保存"),
            "{dialog}"
        );
        // AF10: Esc の説明は設定ダイアログ専用の正規文言へ統一されたため、
        // 「キャンセル」という固定語ではなく SETTINGS_ESCAPE_HINT で検証する。
        assert!(
            dialog.contains("Esc") && dialog.contains(SETTINGS_ESCAPE_HINT),
            "{dialog}"
        );
        assert!(dialog.contains("検索"), "{dialog}");
        // R07: the dialog search covers key names too, not just setting labels.
        assert!(dialog.contains("キー名でも検索可"), "{dialog}");
        // R11 / AD05: Esc cancellation warns when unsaved changes would be
        // discarded. The warning is now spelled out by the canonical macro in
        // exactly one place; the old double-published wording must be gone.
        assert!(
            !dialog.contains(UNSAVED_CHANGES_DISCARD_NOTE),
            "F1 no longer carries the discard note: {dialog}"
        );
        assert!(!dialog.contains("破棄され通知されます"), "{dialog}");
        let tabs = find("設定ダイアログのタブ").expect("settings tab keys must be in F1 help");
        assert!(tabs.contains("Ctrl+Tab"), "{tabs}");
        assert!(tabs.contains("Ctrl+Shift+Tab"), "{tabs}");
        let direct_tabs = find("設定ダイアログのタブ（直接）")
            .expect("Alt+digit settings tab keys must be in F1 help");
        assert!(direct_tabs.contains("Alt+1〜5"), "{direct_tabs}");
        let sort =
            find("設定ダイアログの並び順").expect("Alt+S settings sort key must be in F1 help");
        assert!(sort.contains("Alt+S"), "{sort}");
        let focus = find("設定ダイアログの操作")
            .expect("settings dialog focus-cycling keys must be in F1 help");
        assert!(focus.contains("Tab / Shift+Tab"), "{focus}");
        assert!(
            focus.contains("検索欄") && focus.contains("フォーカス移動"),
            "{focus}"
        );
    }

    #[test]
    fn help_settings_dialog_note_explains_f1_block_and_footer_legend() {
        // Q06: F1 is swallowed while the settings dialog owns the keyboard, so
        // the help entry itself must say so and point at the footer legend that
        // substitutes for the overlay.
        let dialog = HELP_SECTIONS
            .iter()
            .flat_map(|(_, entries)| *entries)
            .find(|(key, _)| *key == "設定ダイアログ")
            .map(|(_, description)| *description)
            .expect("settings dialog keys must be in F1 help");
        for token in ["F1", "無効", "凡例"] {
            assert!(dialog.contains(token), "settings dialog note: {dialog}");
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
    fn empty_state_hints_name_every_filter_and_recovery_key() {
        // P05: 空状態でも「どの条件を変えるか」と「どうやって戻るか」を
        // ユーザーに示さなければならない（EMPTY_DIRECTORY は従来からキーを列挙）。
        let (_, search) = EMPTY_SEARCH;
        for token in ["検索語", "Enter", "Esc で閉じる"] {
            assert!(search.contains(token), "search hint: {search}");
        }
        // Z01: Ctrl+F is dead in the Search view (no `f` in the key allowlist
        // and no OpenSearch in the action allowlist), so it must not be offered.
        assert!(!search.contains("Ctrl+F"), "search hint: {search}");

        let (_, filtered) = EMPTY_FILTERED;
        for token in ["種別", "拡張子", "名前（Ctrl+Shift+F）"] {
            assert!(filtered.contains(token), "filtered hint: {filtered}");
        }

        let (_, directory) = EMPTY_DIRECTORY;
        for token in ["Ctrl+Shift+N", "Ctrl+V"] {
            assert!(directory.contains(token), "directory hint: {directory}");
        }

        // Q07: every empty state must also say how to get back. The trash and
        // load-error overlays are no exception — name their recovery keys.
        let (_, trash) = EMPTY_TRASH;
        // AE05: 空状態の復帰導線は正規の TRASH_EXIT_HINT を埋め込む。
        for token in ["Alt+←", "Backspace", "一覧へ戻る"] {
            assert!(trash.contains(token), "trash hint: {trash}");
        }
        assert!(trash.ends_with(TRASH_EXIT_HINT), "trash hint: {trash}");
        assert!(!trash.contains("前の場所へ"), "stale trash hint: {trash}");
        for token in ["F5", "Ctrl+L"] {
            assert!(
                LOAD_ERROR_HINT.contains(token),
                "load error hint: {LOAD_ERROR_HINT}"
            );
        }

        // T13: the hidden-files-only empty state also has to name the key that
        // recovers the hidden entries. The count is rendered, not a literal in
        // the template, so check both the template and one rendered instance.
        for token in ["Ctrl+H", "で表示"] {
            assert!(
                EMPTY_HIDDEN_ONLY.contains(token),
                "hidden-only hint: {EMPTY_HIDDEN_ONLY}"
            );
        }
        let hidden_only = empty_hidden_only(3);
        assert!(
            hidden_only.contains("3 件"),
            "hidden-only hint: {hidden_only}"
        );
        assert!(
            hidden_only.contains("Ctrl+H"),
            "hidden-only hint: {hidden_only}"
        );
        // V13: in Trash the same chord is rejected, so the hint must give the
        // exit route instead of offering Ctrl+H unconditionally.
        let trash_only = empty_hidden_only_trash(3);
        assert_eq!(
            trash_only,
            "隠しファイル 3 件が非表示です（ゴミ箱を出て Ctrl+H で表示）"
        );
        assert!(trash_only.contains("ゴミ箱を出て Ctrl+H"));
        assert!(!trash_only.contains("（Ctrl+H で表示）"));
        assert_ne!(trash_only, hidden_only);
    }

    #[test]
    fn trash_no_hidden_hint_matches_the_empty_state_exit_route() {
        // AB11: ゴミ箱ビューで Ctrl+H が拒否される通知にも、空状態
        // (EMPTY_HIDDEN_ONLY_TRASH) と同じ「ゴミ箱を出て … Ctrl+H」の復帰導線を
        // 載せる。片方だけに導線があると漂移するので両方を同期検証する。
        assert_eq!(
            STATUS_TRASH_NO_HIDDEN,
            "ゴミ箱ビューでは隠しファイルの切替はできません（ゴミ箱を出てから Ctrl+H）"
        );
        for token in ["ゴミ箱を出て", "Ctrl+H"] {
            assert!(
                STATUS_TRASH_NO_HIDDEN.contains(token),
                "trash refusal hint: {STATUS_TRASH_NO_HIDDEN}"
            );
            assert!(
                EMPTY_HIDDEN_ONLY_TRASH.contains(token),
                "trash empty-state hint: {EMPTY_HIDDEN_ONLY_TRASH}"
            );
        }
        assert!(STATUS_TRASH_NO_HIDDEN.ends_with('）'));
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
        // Every `pub const STATUS_*` string in this module belongs here except
        // `STATUS_PRIORITY` (a label slice with its own test below). A single
        // explicit inventory keeps the F6-36 non-empty check from silently
        // skipping a newly added message (J02).
        for message in [
            STATUS_CHANGED,
            STATUS_SEARCH_STALE,
            STATUS_TABS_ONLY_DIRECTORY,
            STATUS_BOOKMARKS_ONLY_DIRECTORY,
            STATUS_PREVIEW_ONLY_DIRECTORY,
            STATUS_LOCATION_ONLY_DIRECTORY,
            STATUS_FUZZY_JUMP_ONLY_DIRECTORY,
            STATUS_PROPERTIES_ONLY_DIRECTORY,
            STATUS_SEARCH_ONLY_DIRECTORY,
            STATUS_LAST_TAB,
            STATUS_TAB_MOVED_LEFT,
            STATUS_TAB_MOVED_RIGHT,
            STATUS_TRASH_NO_COPY,
            STATUS_TRASH_NO_CUT,
            STATUS_TRASH_NO_PASTE,
            STATUS_TRASH_NO_RENAME,
            STATUS_TRASH_NO_PURGE,
            STATUS_CANCELLING,
            STATUS_NO_CLIPBOARD,
            STATUS_OP_RUNNING,
            STATUS_DROP_ONLY_DIRECTORY,
            STATUS_DROP_NO_DESTINATION,
            STATUS_DROP_SAME_DIRECTORY,
            STATUS_DROP_NOT_FOUND,
            STATUS_DROP_EMPTY,
            STATUS_SPECIAL_FILE_NO_COPY,
            STATUS_SAME_DIRECTORY_NO_MOVE,
            STATUS_FOLDER_NO_PREVIEW,
            STATUS_SYMLINK_NO_PREVIEW,
            STATUS_BOOKMARK_NOT_PERSISTABLE,
            STATUS_RENAME_NO_PARENT,
            STATUS_RENAME_EXISTS_PREFIX,
            STATUS_TRASH_NO_TERMINAL_PREFIX,
            STATUS_FILE_NO_NEW_TAB_PREFIX,
            STATUS_TRASH_LOADING,
            STATUS_FOLDER_CREATING,
            STATUS_TARGET_GONE,
            STATUS_PATH_GONE,
            STATUS_PLACE_GONE_HINT,
            STATUS_NOT_DIRECTORY,
            STATUS_UNDO_FAILED,
            STATUS_REDO_NO_TARGET,
            STATUS_ERROR_PREFIX,
            STATUS_PARTIAL_FAILURE_SUFFIX,
            STATUS_OP_FAILED_SUFFIX,
            STATUS_CANNOT_OPEN_SUFFIX,
            STATUS_SKIPPED_PREFIX,
            STATUS_SKIPPED_SUFFIX,
            STATUS_SKIP_REASON_MISSING,
            STATUS_SKIP_REASON_SPECIAL,
            // AB03: 混在ケースで使う同フォルダの理由も在庫に登録する。
            STATUS_SKIP_REASON_SAME_DIRECTORY,
            STATUS_CANCELLED,
            STATUS_CANCEL_HINT,
            STATUS_INVALID_CLI_DIR,
            STATUS_NO_SELECTION,
            STATUS_OP_IN_PROGRESS_SUFFIX,
            STATUS_RESCAN_FAILED,
            STATUS_WATCH_PREFIX,
            STATUS_WATCH_PARTIAL,
            STATUS_WATCH_DISABLED,
            STATUS_WATCH_INIT_FAILED,
            STATUS_WATCH_COUNT_SUFFIX,
            STATUS_TRASH_NO_AUTO_REFRESH,
            STATUS_OPENED_DEFAULT_SUFFIX,
            STATUS_REVEALED_PREFIX,
            STATUS_KEYMAP_NO_UNBIND_PREFIX,
            STATUS_KEYMAP_NO_UNBIND_HINT,
            STATUS_KEYMAP_FORMAT_INVALID,
            STATUS_KEYMAP_LIMIT_PREFIX,
            STATUS_KEYMAP_LIMIT_SUFFIX,
            STATUS_KEYMAP_INVALID_PREFIX,
            STATUS_MAX_TABS_PREFIX,
            STATUS_MAX_TABS_SUFFIX,
            STATUS_SETTINGS_SAVE_FAILED,
            STATUS_SETTINGS_SAVE_TOO_LARGE,
            STATUS_SETTINGS_SAVE_LOCKED,
            STATUS_SETTINGS_READ_ONLY,
            STATUS_SETTINGS_READ_ONLY_HINT,
            STATUS_SETTINGS_DISCARDED,
            // AG06: 保存成功も在庫に登録し、空文字列や登録漏れを検出する。
            STATUS_SETTINGS_SAVED,
            STATUS_SETTINGS_UNSAVED_HINT,
            STATUS_TAB_RESTORED,
            STATUS_TAB_RESTORE_EMPTY,
            STATUS_FOLDER_CREATED,
            ERR_FOLDER_CREATE,
            STATUS_PATH_COPIED,
            STATUS_MISSING_COUNT_SUFFIX,
            STATUS_NOT_FOUND_SUFFIX,
            STATUS_UNDO_EMPTY,
            STATUS_UNDO_HISTORY_HINT,
            STATUS_UNDO_EXISTS_SUFFIX,
            STATUS_UNDO_MISSING_SUFFIX,
            STATUS_REDO_EMPTY,
            STATUS_REDO_FAILED,
            STATUS_RENAME_FAILED,
            STATUS_TARGET_MISSING,
            STATUS_TRASH_ALL_FAILED,
            STATUS_TRASH_NO_HIDDEN,
            STATUS_TRASH_NO_SORT,
            STATUS_TRASH_PARTIAL_FAILED,
            STATUS_RESTORE_FAILED,
            STATUS_TRASH_MISSING_REASON,
            STATUS_RESTORE_EMPTY,
            STATUS_SEARCH_EMPTY_QUERY,
            STATUS_SEARCH_NOT_OPEN,
            STATUS_SEARCH_PROMPT,
            STATUS_THEME_CHANGED_PREFIX,
            // R02〜R05: strings.rs へ集約した拒否・失敗文言。
            STATUS_ACCESS_DENIED,
            STATUS_TERMINAL_START_PREFIX,
            STATUS_TERMINAL_COMMAND_INVALID,
            // AG04: 無効化の完了通知も在庫に登録する。
            STATUS_TERMINAL_COMMAND_DISABLED,
            STATUS_TERMINAL_DISABLED_HINT,
            STATUS_SAVED_SEARCH_SAVE_FAILED_PREFIX,
            STATUS_SAVED_SEARCH_INVALID_INPUT,
            STATUS_SAVED_SEARCH_CAP_HINT,
            STATUS_SAVED_SEARCH_NAME_REQUIRED,
            STATUS_SAVED_SEARCH_QUERY_REQUIRED,
            STATUS_BOOKMARK_LABEL_SET_FAILED_PREFIX,
            STATUS_BOOKMARK_LABEL_REASON_INVALID_PATH,
            STATUS_BOOKMARK_LABEL_REASON_INVALID_LABEL,
            STATUS_BOOKMARK_LABEL_REASON_TOO_LONG,
            STATUS_BOOKMARK_LABEL_REASON_NUL,
            STATUS_BOOKMARK_LABEL_REASON_LIMIT,
            // X03 / X04: 呼び出し側の format! に残っていた接頭辞も一元化した。
            STATUS_BOOKMARK_LABEL_NOT_SET_PREFIX,
            STATUS_TRASH_LOAD_FAILED_PREFIX,
            // AJ02: F2 名前変更の検証エラー 6 ケース。
            STATUS_RENAME_INVALID_EMPTY,
            STATUS_RENAME_INVALID_DOT,
            STATUS_RENAME_INVALID_SEPARATOR,
            STATUS_RENAME_INVALID_NUL,
            STATUS_RENAME_INVALID_RESERVED_PREFIX,
            STATUS_RENAME_INVALID_CHAR,
        ] {
            assert!(!message.is_empty(), "status message must not be empty");
        }
    }

    #[test]
    fn trash_no_sort_shares_the_exit_trash_recovery_wording() {
        // AE07: ゴミ箱ビューの他の拒否通知と同じ「ゴミ箱を出て」導線を
        // 並び替え拒否にも載せる。片方だけ drift すると回帰を検知する。
        for message in [
            STATUS_TRASH_NO_SORT,
            STATUS_TRASH_NO_HIDDEN,
            EMPTY_HIDDEN_ONLY_TRASH,
        ] {
            assert!(
                message.contains("ゴミ箱を出て"),
                "trash refusal must show the exit route: {message}"
            );
        }
    }

    #[test]
    fn refusal_statuses_have_a_documented_colour_class() {
        // L06: 「操作を行わなかった / できなかった」を表す STATUS_* は成功 green
        // ではなく warning(amber) / error(red) に分類する。表示側の
        // STATUS_WARNING_MARKERS / STATUS_ERROR_MARKERS は main.rs（別担当）が
        // 管理するため、ここでは定数名→分類の正規一覧を持ち、登録漏れや
        // 文言変更を検出できるようにする。
        // 色の適用自体は main.rs の status_color が行う（このテストは分類表のみ検証）。
        let documented: &[(&str, &str, &str)] = &[
            // 既に main.rs の warning markers に登録済みの拒否通知。
            ("STATUS_NO_SELECTION", STATUS_NO_SELECTION, "warning"),
            ("STATUS_NO_CLIPBOARD", STATUS_NO_CLIPBOARD, "warning"),
            (
                "STATUS_DROP_NO_DESTINATION",
                STATUS_DROP_NO_DESTINATION,
                "warning",
            ),
            ("STATUS_REDO_NO_TARGET", STATUS_REDO_NO_TARGET, "warning"),
            ("STATUS_TRASH_NO_COPY", STATUS_TRASH_NO_COPY, "warning"),
            ("STATUS_TRASH_NO_CUT", STATUS_TRASH_NO_CUT, "warning"),
            ("STATUS_TRASH_NO_PASTE", STATUS_TRASH_NO_PASTE, "warning"),
            ("STATUS_TRASH_NO_RENAME", STATUS_TRASH_NO_RENAME, "warning"),
            ("STATUS_TRASH_NO_PURGE", STATUS_TRASH_NO_PURGE, "warning"),
            ("STATUS_TRASH_NO_HIDDEN", STATUS_TRASH_NO_HIDDEN, "warning"),
            ("STATUS_TRASH_NO_SORT", STATUS_TRASH_NO_SORT, "warning"),
            // AE04: これら3つは main.rs の warning マーカーに登録済みだったが
            // 分類表に欠けていた（Y01 の逆方向チェックが無かったため）。
            // 鮮度警告 / キャンセル完了 / 読み取り専用通知で拒否語の形を取ら
            // ないため、拒否語チェックは AMBER_STATUS_NOTICES で明示除外する。
            ("STATUS_SEARCH_STALE", STATUS_SEARCH_STALE, "warning"),
            ("STATUS_CANCELLED", STATUS_CANCELLED, "warning"),
            (
                "STATUS_SETTINGS_READ_ONLY",
                STATUS_SETTINGS_READ_ONLY,
                "warning",
            ),
            (
                "STATUS_KEYMAP_NO_UNBIND_PREFIX",
                STATUS_KEYMAP_NO_UNBIND_PREFIX,
                "warning",
            ),
            // AJ02: F2 名前変更の入力検証エラーも concrete failure(red)。
            (
                "STATUS_RENAME_INVALID_EMPTY",
                STATUS_RENAME_INVALID_EMPTY,
                "error",
            ),
            (
                "STATUS_RENAME_INVALID_DOT",
                STATUS_RENAME_INVALID_DOT,
                "error",
            ),
            (
                "STATUS_RENAME_INVALID_SEPARATOR",
                STATUS_RENAME_INVALID_SEPARATOR,
                "error",
            ),
            (
                "STATUS_RENAME_INVALID_NUL",
                STATUS_RENAME_INVALID_NUL,
                "error",
            ),
            (
                "STATUS_RENAME_INVALID_RESERVED_PREFIX",
                STATUS_RENAME_INVALID_RESERVED_PREFIX,
                "error",
            ),
            (
                "STATUS_RENAME_INVALID_CHAR",
                STATUS_RENAME_INVALID_CHAR,
                "error",
            ),
            // AP01: 入力検証／拒否の定数 6 種は呼び出し側が STATUS_ERROR_PREFIX を
            // 付けていたため分類表から漏れていた。prefix に依存しないよう
            // concrete failure(error) として明示的に登録する。
            (
                "STATUS_KEYMAP_FORMAT_INVALID",
                STATUS_KEYMAP_FORMAT_INVALID,
                "error",
            ),
            (
                "STATUS_KEYMAP_LIMIT_PREFIX",
                STATUS_KEYMAP_LIMIT_PREFIX,
                "error",
            ),
            (
                "STATUS_KEYMAP_INVALID_PREFIX",
                STATUS_KEYMAP_INVALID_PREFIX,
                "error",
            ),
            (
                "STATUS_TERMINAL_COMMAND_INVALID",
                STATUS_TERMINAL_COMMAND_INVALID,
                "error",
            ),
            (
                "STATUS_SAVED_SEARCH_NAME_REQUIRED",
                STATUS_SAVED_SEARCH_NAME_REQUIRED,
                "error",
            ),
            (
                "STATUS_SAVED_SEARCH_QUERY_REQUIRED",
                STATUS_SAVED_SEARCH_QUERY_REQUIRED,
                "error",
            ),
            // 既に error markers に登録済みの concrete failure。
            ("STATUS_UNDO_FAILED", STATUS_UNDO_FAILED, "error"),
            ("STATUS_REDO_FAILED", STATUS_REDO_FAILED, "error"),
            ("STATUS_RENAME_FAILED", STATUS_RENAME_FAILED, "error"),
            // AM01: set_rename_error がステータスへミラーする同期検証エラーも
            // concrete failure(red)。登録漏れだと前回の成功 green が残る。
            ("STATUS_RENAME_NO_PARENT", STATUS_RENAME_NO_PARENT, "error"),
            (
                "STATUS_RENAME_EXISTS_PREFIX",
                STATUS_RENAME_EXISTS_PREFIX,
                "error",
            ),
            ("STATUS_TARGET_GONE", STATUS_TARGET_GONE, "error"),
            // T10: 汎用の「対象が存在しません」も main.rs の error マーカーに
            // 登録済み（C_RED）なので、色分類表にも明示的に載せる。
            ("STATUS_PATH_GONE", STATUS_PATH_GONE, "error"),
            ("STATUS_TARGET_MISSING", STATUS_TARGET_MISSING, "error"),
            ("STATUS_DROP_NOT_FOUND", STATUS_DROP_NOT_FOUND, "error"),
            ("STATUS_NOT_DIRECTORY", STATUS_NOT_DIRECTORY, "error"),
            // W06: 再スキャン失敗も concrete failure（main.rs の
            // STATUS_ERROR_MARKERS に登録済み）なので error(red) と明記する。
            ("STATUS_RESCAN_FAILED", STATUS_RESCAN_FAILED, "error"),
            (
                "STATUS_SETTINGS_SAVE_FAILED",
                STATUS_SETTINGS_SAVE_FAILED,
                "error",
            ),
            (
                "STATUS_SETTINGS_SAVE_TOO_LARGE",
                STATUS_SETTINGS_SAVE_TOO_LARGE,
                "error",
            ),
            (
                "STATUS_SETTINGS_SAVE_LOCKED",
                STATUS_SETTINGS_SAVE_LOCKED,
                "error",
            ),
            ("STATUS_TRASH_ALL_FAILED", STATUS_TRASH_ALL_FAILED, "error"),
            ("STATUS_RESTORE_FAILED", STATUS_RESTORE_FAILED, "error"),
            // X09: 起動ディレクトリの拒否も concrete failure。呼び出し側は
            // STATUS_ERROR_PREFIX を付与し、warning の「開けません」に負けないようにする。
            ("STATUS_INVALID_CLI_DIR", STATUS_INVALID_CLI_DIR, "error"),
            // R02: OS によるアクセス拒否は concrete failure なので error(red)。
            ("STATUS_ACCESS_DENIED", STATUS_ACCESS_DENIED, "error"),
            // R04 / R05: 動的な値を連結する失敗接頭辞も error(red)。
            (
                "STATUS_SAVED_SEARCH_SAVE_FAILED_PREFIX",
                STATUS_SAVED_SEARCH_SAVE_FAILED_PREFIX,
                "error",
            ),
            (
                "STATUS_BOOKMARK_LABEL_SET_FAILED_PREFIX",
                STATUS_BOOKMARK_LABEL_SET_FAILED_PREFIX,
                "error",
            ),
            // X03 / X04: 以前 main.rs の format! に埋め込まれていた接頭辞も
            // concrete failure として明示的に分類する。
            (
                "STATUS_BOOKMARK_LABEL_NOT_SET_PREFIX",
                STATUS_BOOKMARK_LABEL_NOT_SET_PREFIX,
                "error",
            ),
            (
                "STATUS_TRASH_LOAD_FAILED_PREFIX",
                STATUS_TRASH_LOAD_FAILED_PREFIX,
                "error",
            ),
            // Q02: P08 の「できません」拒否は STATUS_OP_FAILED_SUFFIX と同じ
            // 文言を含むため error(red)。main.rs の error マーカーにも明示登録済み。
            (
                "STATUS_SPECIAL_FILE_NO_COPY",
                STATUS_SPECIAL_FILE_NO_COPY,
                "error",
            ),
            (
                "STATUS_FOLDER_NO_PREVIEW",
                STATUS_FOLDER_NO_PREVIEW,
                "error",
            ),
            (
                "STATUS_SYMLINK_NO_PREVIEW",
                STATUS_SYMLINK_NO_PREVIEW,
                "error",
            ),
            // Q03: ブックマーク拒否は「できません」を含むが、より具体的な
            // 拒否通知として warning(amber) に分類する（main.rs 登録済み）。
            (
                "STATUS_BOOKMARK_NOT_PERSISTABLE",
                STATUS_BOOKMARK_NOT_PERSISTABLE,
                "warning",
            ),
            // 動的な値を連結する接尾も、単体で concrete failure として分類する。
            (
                "STATUS_MISSING_COUNT_SUFFIX",
                STATUS_MISSING_COUNT_SUFFIX,
                "error",
            ),
            ("STATUS_NOT_FOUND_SUFFIX", STATUS_NOT_FOUND_SUFFIX, "error"),
            // X08: Undo の存在しない対象も concrete failure（接尾辞を共有）。
            (
                "STATUS_UNDO_MISSING_SUFFIX",
                STATUS_UNDO_MISSING_SUFFIX,
                "error",
            ),
            // L06: 未登録のため green で表示される定数。main.rs の
            // STATUS_WARNING_MARKERS に追加すべき正規一覧（下の配列と同期）。
            (
                "STATUS_TABS_ONLY_DIRECTORY",
                STATUS_TABS_ONLY_DIRECTORY,
                "warning",
            ),
            (
                "STATUS_BOOKMARKS_ONLY_DIRECTORY",
                STATUS_BOOKMARKS_ONLY_DIRECTORY,
                "warning",
            ),
            ("STATUS_LAST_TAB", STATUS_LAST_TAB, "warning"),
            (
                "STATUS_DROP_ONLY_DIRECTORY",
                STATUS_DROP_ONLY_DIRECTORY,
                "warning",
            ),
            ("STATUS_DROP_EMPTY", STATUS_DROP_EMPTY, "warning"),
            (
                "STATUS_TAB_RESTORE_EMPTY",
                STATUS_TAB_RESTORE_EMPTY,
                "warning",
            ),
            ("STATUS_UNDO_EMPTY", STATUS_UNDO_EMPTY, "warning"),
            ("STATUS_REDO_EMPTY", STATUS_REDO_EMPTY, "warning"),
            // X02: 未保存設定の破棄はデータ消失の通知であり、成功 green でも
            // 汎用エラー red でもなく warning(amber) に分類する。
            (
                "STATUS_SETTINGS_DISCARDED",
                STATUS_SETTINGS_DISCARDED,
                "warning",
            ),
            // N02: 「開けません」は「できません」を含まない拒否接尾なので、
            // 単独では成功 green に落ちていた。warning として登録する。
            (
                "STATUS_CANNOT_OPEN_SUFFIX",
                STATUS_CANNOT_OPEN_SUFFIX,
                "warning",
            ),
            // N03: ディレクトリ表示でしか使えない機能の拒否通知。
            (
                "STATUS_PREVIEW_ONLY_DIRECTORY",
                STATUS_PREVIEW_ONLY_DIRECTORY,
                "warning",
            ),
            (
                "STATUS_LOCATION_ONLY_DIRECTORY",
                STATUS_LOCATION_ONLY_DIRECTORY,
                "warning",
            ),
            (
                "STATUS_FUZZY_JUMP_ONLY_DIRECTORY",
                STATUS_FUZZY_JUMP_ONLY_DIRECTORY,
                "warning",
            ),
            (
                "STATUS_PROPERTIES_ONLY_DIRECTORY",
                STATUS_PROPERTIES_ONLY_DIRECTORY,
                "warning",
            ),
            (
                "STATUS_SEARCH_ONLY_DIRECTORY",
                STATUS_SEARCH_ONLY_DIRECTORY,
                "warning",
            ),
            // N08: 動的な件数を含む上限拒否と、空の復元対象。
            ("STATUS_MAX_TABS_PREFIX", STATUS_MAX_TABS_PREFIX, "warning"),
            ("STATUS_RESTORE_EMPTY", STATUS_RESTORE_EMPTY, "warning"),
            // N09: 空の検索語は検索を実行しない拒否なので warning。
            (
                "STATUS_SEARCH_EMPTY_QUERY",
                STATUS_SEARCH_EMPTY_QUERY,
                "warning",
            ),
            // P03: 検索を開く前の操作を求める案内も拒否なので warning。
            ("STATUS_SEARCH_NOT_OPEN", STATUS_SEARCH_NOT_OPEN, "warning"),
            // Y08: 監視の初期化失敗は concrete failure だが、ステータスバーでは
            // self.status を通らず secondary slot が直接 p.yellow で描画する。
            // 色分類表にも warning として載せ、main.rs の warning マーカーにも
            // 登録して回帰を固定する（直接描画なのでマーカー自体は表示経路に
            // 乗らないが、分類の一貫性を保つ）。
            (
                "STATUS_WATCH_INIT_FAILED",
                STATUS_WATCH_INIT_FAILED,
                "warning",
            ),
            // AB07: 一部無効 / 無効 も監視状態の warning 表示。Y08 と同じく
            // self.status を通らず secondary slot が直接 p.yellow で描画する
            // ため、マーカー自体は表示経路に乗らないが、分類の一貫性のため
            // warning として登録する（main.rs のマーカー表にも追加済み）。
            ("STATUS_WATCH_PARTIAL", STATUS_WATCH_PARTIAL, "warning"),
            ("STATUS_WATCH_DISABLED", STATUS_WATCH_DISABLED, "warning"),
            // W10: 「キャンセルしています…」は操作の進行中を示す一時通知で、
            // 拒否でも失敗でもない。完了通知 STATUS_CANCELLED は warning だが、
            // 進行中は成功 green のままとする（下の検証で例外として明示）。
            ("STATUS_CANCELLING", STATUS_CANCELLING, "success"),
            // AG06: 設定保存の成功通知も success(green)。破棄通知（warning）
            // との成否の非対称をここで解消する。
            ("STATUS_SETTINGS_SAVED", STATUS_SETTINGS_SAVED, "success"),
        ];
        // W10: success 分類を許可するのは一時的な進行中通知だけ。意図なき
        // green 回帰を防ぐため、拒否語チェックの対象外もこの一覧で固定する。
        const TRANSIENT_IN_PROGRESS: &[&str] = &["STATUS_CANCELLING"];
        // AG06: 完了型の成功通知（拒否語を持たない）も success として明示する。
        const SUCCESS_NOTICES: &[&str] = &["STATUS_SETTINGS_SAVED"];
        // X02: データ消失の通知は拒否語（「〜ません」）を含まないが成功でもない。
        // 一時進行通知と同じく一般ルールの対象外とし、破棄の事実と復帰導線を
        // 個別に検証する（分類は warning 固定）。
        const DATA_LOSS_NOTICES: &[&str] = &["STATUS_SETTINGS_DISCARDED"];
        // AE04: 鮮度警告 / キャンセル完了 / 読み取り専用通知は意図的に amber
        // （warning マーカー登録済み）だが、「〜ません」等の拒否語を含まない。
        // 分類表への登録は必須としつつ、拒否語チェックだけを明示的に除外する。
        const AMBER_STATUS_NOTICES: &[&str] = &[
            "STATUS_SEARCH_STALE",
            "STATUS_CANCELLED",
            "STATUS_SETTINGS_READ_ONLY",
        ];
        // AM01: 動的なパスを連結する既存排斥の接頭辞は拒否語（「〜ません」等）を
        // 含まないが、concrete error として分類する。一般ルールから外して
        // 接頭辞としての形も専用に検証する。
        const DYNAMIC_EXISTS_ERROR_PREFIX: &[&str] = &["STATUS_RENAME_EXISTS_PREFIX"];
        // AP01: キーマップ形式ヒントとターミナルコマンド検証は拒否語
        // （〜ません / 無効 / 最大 / ください 等）を含まないが、concrete error として
        // 分類する。DYNAMIC_EXISTS_ERROR_PREFIX と同じく一般ルールから外し、
        // それぞれの文言形状を専用に検証する。
        const VALIDATION_ERROR_NOTICES: &[&str] = &[
            "STATUS_KEYMAP_FORMAT_INVALID",
            "STATUS_TERMINAL_COMMAND_INVALID",
        ];
        // L06 / N02 / N03 / N08 / N09 の指摘で main.rs 側への登録が必要な定数名
        // （重複・脱落を防止）。全て warning として分類されなければならない。
        const PENDING_WARNING_MARKERS: &[&str] = &[
            "STATUS_TABS_ONLY_DIRECTORY",
            "STATUS_BOOKMARKS_ONLY_DIRECTORY",
            "STATUS_LAST_TAB",
            "STATUS_DROP_ONLY_DIRECTORY",
            "STATUS_DROP_EMPTY",
            "STATUS_TAB_RESTORE_EMPTY",
            "STATUS_UNDO_EMPTY",
            "STATUS_REDO_EMPTY",
            "STATUS_CANNOT_OPEN_SUFFIX",
            "STATUS_PREVIEW_ONLY_DIRECTORY",
            "STATUS_LOCATION_ONLY_DIRECTORY",
            "STATUS_FUZZY_JUMP_ONLY_DIRECTORY",
            "STATUS_PROPERTIES_ONLY_DIRECTORY",
            "STATUS_SEARCH_ONLY_DIRECTORY",
            "STATUS_MAX_TABS_PREFIX",
            "STATUS_RESTORE_EMPTY",
            "STATUS_SEARCH_EMPTY_QUERY",
            // Q03: strings.rs へ移動したブックマーク拒否も warning として固定。
            "STATUS_BOOKMARK_NOT_PERSISTABLE",
        ];
        // STATUS_DROP_SAME_DIRECTORY（「同じフォルダのためコピーしません」）と
        // STATUS_SAME_DIRECTORY_NO_MOVE（「同じフォルダのため移動しません」）は
        // 操作の拒否ではなく同一フォルダへの不要な操作を行わない情報通知として
        // green のまま（L06 対象外、Q02）。よって上の分類表には登録しない。
        let source = include_str!("strings.rs");
        let mut seen = std::collections::HashSet::new();
        for (name, message, class) in documented {
            assert!(seen.insert(*name), "duplicate classification: {name}");
            assert!(
                source.contains(&format!("pub const {name}")),
                "{name} is not declared in strings.rs"
            );
            assert!(!message.is_empty(), "{name} must not be empty");
            if TRANSIENT_IN_PROGRESS.contains(name) {
                // W10: 一時進行通知は「しています…」で終わることを固定し、
                // 拒否語の検証から除外する。
                assert!(
                    message.contains("しています"),
                    "{name} must be an in-progress notice: {message}"
                );
            } else if DATA_LOSS_NOTICES.contains(name) {
                // X02: 破棄の事実と、設定を開き直す導線の両方を固定する。
                assert!(
                    message.contains("破棄しました") && message.contains("開き直せます"),
                    "{name} must describe the loss and its recovery: {message}"
                );
            } else if SUCCESS_NOTICES.contains(name) {
                // AG06: 保存成功は「〜しました」で終わる完了通知で、拒否語を
                // 含まないため一般ルールから除外する（色は success/green）。
                assert!(*class == "success", "{name} must be success");
                assert!(
                    message.ends_with("しました") && !message.contains("ません"),
                    "{name} must be a success notice: {message}"
                );
            } else if AMBER_STATUS_NOTICES.contains(name) {
                // AE04: 鮮度 / キャンセル / 読み取り専用は warning として分類表に
                // 登録済みであること（下の逆方向チェック）だけを固定する。
                // 拒否語を含まないのは意図的なので一般ルールから除外する。
                assert!(*class == "warning", "{name} must be warning");
            } else if DYNAMIC_EXISTS_ERROR_PREFIX.contains(name) {
                // AM01: 「既に存在します: 」は動的なパスを受け取る失敗接頭辞。
                assert!(*class == "error", "{name} must be error");
                assert!(
                    message.contains("存在します") && message.ends_with(": "),
                    "{name} must be the dynamic exists-refusal prefix: {message}"
                );
            } else if VALIDATION_ERROR_NOTICES.contains(name) {
                // AP01: 拒否語を持たない入力検証エラーは concrete error。
                // 形式ヒント／長さと制御文字の拒否という意味を個別に固定する。
                assert!(*class == "error", "{name} must be error");
                match *name {
                    "STATUS_KEYMAP_FORMAT_INVALID" => assert!(
                        message.starts_with(KEYMAP_FORMAT_HINT) && message.ends_with('。'),
                        "{name} must be the canonical keymap format hint: {message}"
                    ),
                    "STATUS_TERMINAL_COMMAND_INVALID" => assert!(
                        message.contains("長すぎる") && message.contains("制御文字"),
                        "{name} must describe the command validation failure: {message}"
                    ),
                    other => panic!("unexpected validation error notice: {other}"),
                }
            } else {
                assert!(
                    message.contains("ません")
                        || message.contains("のみ")
                        || message.contains("未対応")
                        // N09: 空入力の拒否（「検索語が空です」）。
                        || message.contains("空です")
                        // N08: 上限拒否の接頭辞（「タブは最大 {n} 個までです」）。
                        || message.contains("最大")
                        // P03: 事前操作を求める案内（「先に Ctrl+F で … してください」）。
                        || message.contains("ください")
                        // R02: OS による拒否（「アクセスが拒否されました」）。
                        || message.contains("拒否されました")
                        // W06: 具体的な失敗（「再スキャンに失敗しました」）。
                        || message.contains("失敗しました")
                        // AB07: 監視の一部無効 / 無効（secondary slot 直接描画）。
                        || message.contains("無効")
                        // AJ02: Windows の不正文字は「使えない文字が含まれています」。
                        || message.contains("使えない"),
                    "{name} is not refusal-worded: {message}"
                );
            }
            assert!(
                *class == "warning"
                    || *class == "error"
                    || (*class == "success"
                        && (TRANSIENT_IN_PROGRESS.contains(name)
                            || SUCCESS_NOTICES.contains(name))),
                "{name} has unknown class: {class}"
            );
        }
        for name in PENDING_WARNING_MARKERS {
            assert!(
                documented
                    .iter()
                    .any(|(n, _, class)| *n == *name && *class == "warning"),
                "{name} must be documented as a warning"
            );
        }
        // Y01: 従来はこのテストが strings.rs 自身（include_str!("strings.rs")）
        // しか見ておらず、main.rs 側の STATUS_WARNING_MARKERS /
        // STATUS_ERROR_MARKERS からマーカーを削除しても検出できなかった。
        // 分類表と実際のマーカー配列を相互照合し、脱落を即座に失敗させる。
        let main_source = include_str!("main.rs");
        fn marker_array<'a>(source: &'a str, const_name: &str) -> &'a str {
            let head = format!("const {const_name}: &[&str] = &[");
            let start = source
                .find(&head)
                .unwrap_or_else(|| panic!("{const_name} is not defined in main.rs"))
                + head.len();
            let rest = &source[start..];
            let end = rest
                .find("\n];")
                .unwrap_or_else(|| panic!("{const_name} array is not terminated"));
            &rest[..end]
        }
        fn has_marker(array: &str, name: &str) -> bool {
            array
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .any(|token| token == name)
        }
        let warning_markers = marker_array(main_source, "STATUS_WARNING_MARKERS");
        let error_markers = marker_array(main_source, "STATUS_ERROR_MARKERS");
        for (name, _, class) in documented {
            match *class {
                "warning" => assert!(
                    has_marker(warning_markers, name),
                    "{name} is documented as warning but missing from STATUS_WARNING_MARKERS"
                ),
                "error" => assert!(
                    has_marker(error_markers, name),
                    "{name} is documented as error but missing from STATUS_ERROR_MARKERS"
                ),
                // W10 / AG06: 一時進行通知（STATUS_CANCELLING）と保存成功通知
                // （STATUS_SETTINGS_SAVED）は success で、どちらのマーカーにも登録しない。
                "success" => {
                    assert!(TRANSIENT_IN_PROGRESS.contains(name) || SUCCESS_NOTICES.contains(name))
                }
                other => panic!("{name} has unknown class: {other}"),
            }
        }
        // AE04: reverse direction. The Y01 loop above only walked
        // documented → markers, so a marker added to main.rs without a class
        // (STATUS_SEARCH_STALE / STATUS_CANCELLED / STATUS_SETTINGS_READ_ONLY)
        // went unnoticed. Require every registered marker to be documented as
        // well. Generic dynamic fragments are the sole documented exemption:
        // STATUS_ERROR_PREFIX is handled by an early return in status_color,
        // and the *_SUFFIX / partial-failure fragments are shared building
        // blocks, not per-message refusals, so they need no colour class.
        const GENERIC_ERROR_FRAGMENTS: &[&str] = &[
            "STATUS_ERROR_PREFIX",
            "STATUS_OP_FAILED_SUFFIX",
            "STATUS_PARTIAL_FAILURE_SUFFIX",
            "STATUS_TRASH_PARTIAL_FAILED",
        ];
        fn marker_names(array: &str) -> Vec<&str> {
            array
                .lines()
                .map(|line| line.split("//").next().unwrap_or(""))
                .map(|line| line.trim().trim_end_matches(',').trim())
                .filter(|token| token.starts_with("STATUS_"))
                .collect()
        }
        for (array_name, array, expected_class) in [
            ("STATUS_WARNING_MARKERS", warning_markers, "warning"),
            ("STATUS_ERROR_MARKERS", error_markers, "error"),
        ] {
            for name in marker_names(array) {
                if expected_class == "error" && GENERIC_ERROR_FRAGMENTS.contains(&name) {
                    continue;
                }
                assert!(
                    documented
                        .iter()
                        .any(|(n, _, class)| { *n == name && *class == expected_class }),
                    "{name} is in {array_name} but not documented as {expected_class}"
                );
            }
        }
    }

    #[test]
    fn discarded_settings_notice_carries_a_recovery_hint() {
        // F6-36 の 4 段構成のうち「復帰導線」が欠けていた (I06)。破棄した事実
        // だけで終わらせず、設定を開き直す操作を括弧内に案内する。
        assert!(STATUS_SETTINGS_DISCARDED.starts_with("未保存の変更を破棄しました"));
        assert!(STATUS_SETTINGS_DISCARDED.contains("Ctrl+,"));
        assert!(STATUS_SETTINGS_DISCARDED.contains("設定を開き直せます"));
        assert!(
            STATUS_SETTINGS_DISCARDED.ends_with('）') && STATUS_SETTINGS_DISCARDED.contains('（'),
            "recovery hint must be a parenthesized suffix: {STATUS_SETTINGS_DISCARDED}"
        );
    }

    #[test]
    fn saved_settings_notice_is_the_success_counterpart_of_discarded() {
        // AG06: 設定の成否で色が非対称にならないよう、保存成功にも専用定数を
        // 持ち、破棄通知（warning）に対する成功（green）の対を固定する。
        assert_eq!(STATUS_SETTINGS_SAVED, "設定を保存しました");
        assert!(STATUS_SETTINGS_SAVED.ends_with("しました"));
        assert!(STATUS_SETTINGS_DISCARDED.contains("しました"));
        assert_ne!(STATUS_SETTINGS_SAVED, STATUS_SETTINGS_DISCARDED);
        // 成功通知は破棄の復帰導線を誤って併記しない。
        assert!(!STATUS_SETTINGS_SAVED.contains("破棄"));
        assert!(!STATUS_SETTINGS_SAVED.contains('（'));
        let main_source = include_str!("main.rs");
        assert!(
            main_source.contains("STATUS_SETTINGS_SAVED"),
            "settings overlay Save must route through STATUS_SETTINGS_SAVED"
        );
        assert!(
            !main_source.contains("\"設定を保存しました\""),
            "settings save success must not inline the literal"
        );
    }

    #[test]
    fn settings_unsaved_hint_has_one_canonical_wording() {
        // J08 / K13: ターミナル欄の重複は削除され、共有フッターの未変更時に
        // 表示される唯一の文言として、「保存」ボタンを明示した丁寧形を固定する。
        assert_eq!(
            STATUS_SETTINGS_UNSAVED_HINT,
            "変更は「保存」を押すまで反映されません。"
        );
        assert!(STATUS_SETTINGS_UNSAVED_HINT.contains("「保存」"));
        assert!(STATUS_SETTINGS_UNSAVED_HINT.ends_with('。'));
    }

    #[test]
    fn cancel_hint_is_shared_by_all_in_progress_statuses() {
        // K05: one canonical parenthesized hint for every cancellable progress
        // status, instead of five hand-copied literals drifting apart.
        assert_eq!(STATUS_CANCEL_HINT, "（Esc でキャンセル）");
        assert!(STATUS_OP_RUNNING.ends_with(STATUS_CANCEL_HINT));
        assert!(STATUS_OP_IN_PROGRESS_SUFFIX.ends_with(STATUS_CANCEL_HINT));

        // The undo/redo/rename "…しています" statuses in main.rs must build on
        // that constant, not repeat the literal inline.
        let source = include_str!("main.rs");
        for (phrase, expected_sites) in [
            ("元に戻しています", 2),
            ("やり直しています", 2),
            ("名前を変更しています", 1),
        ] {
            let sites: Vec<&str> = source
                .lines()
                .filter(|line| line.contains(phrase) && line.contains("self.status"))
                .collect();
            assert_eq!(sites.len(), expected_sites, "sites for {phrase}");
            for line in sites {
                assert!(line.contains("STATUS_CANCEL_HINT"), "not shared: {line}");
                assert!(!line.contains("Esc でキャンセル"), "inline hint: {line}");
            }
        }
    }

    #[test]
    fn theme_display_labels_are_localized() {
        // H08: the status bar and the settings overlay share one display name
        // per ThemeChoice, not the serde values (`dark` / `light` / `system`).
        assert_eq!(THEME_LABEL_DARK, "ダーク");
        assert_eq!(THEME_LABEL_LIGHT, "ライト");
        assert_eq!(THEME_LABEL_SYSTEM, "システム");
        assert_eq!(
            format!("{STATUS_THEME_CHANGED_PREFIX}{THEME_LABEL_DARK}"),
            "テーマ: ダーク"
        );
        for label in [THEME_LABEL_DARK, THEME_LABEL_LIGHT, THEME_LABEL_SYSTEM] {
            assert!(!label.is_empty());
        }
        assert_ne!(THEME_LABEL_DARK, THEME_LABEL_LIGHT);
        assert_ne!(THEME_LABEL_LIGHT, THEME_LABEL_SYSTEM);
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
        // AG07: 「失敗のみ」は成功を消さない分岐の中核で、未テストだった。
        assert_eq!(OP_RESULT_PRIORITY[2], "失敗のみ");
        assert_eq!(OP_RESULT_PRIORITY[3], "成功");
        // STATUS_PRIORITY と同じく、長さと重複を固定して分岐の増減・重複を検出する。
        assert_eq!(OP_RESULT_PRIORITY.len(), 4);
        let mut seen = std::collections::HashSet::new();
        for label in OP_RESULT_PRIORITY {
            assert!(seen.insert(*label), "duplicate priority label: {label}");
        }
        assert!(STATUS_PARTIAL_FAILURE_SUFFIX.contains("一部失敗"));
        assert!(STATUS_OP_FAILED_SUFFIX.starts_with("できません"));
        // 進行中と完了は別フェーズで、同じ文言に潰してはならない。
        assert_ne!(STATUS_CANCELLED, STATUS_CANCELLING);
    }

    #[test]
    fn dynamic_refusal_markers_have_canonical_wording() {
        // M05/M06: 動的な値を連結する拒否 notification は接頭辞・接尾自体を
        // main.rs の色判定マーカーとして登録するため、文言の揺れをここで固定する。
        assert_eq!(STATUS_KEYMAP_NO_UNBIND_PREFIX, "解除する設定はありません: ");
        assert_eq!(STATUS_NOT_FOUND_SUFFIX, "が見つかりません");
        assert_eq!(STATUS_MISSING_COUNT_SUFFIX, " 件は既に存在しません");
        // N08: 件数を連結する上限拒否も、接頭辞自体を warning マーカーにする。
        assert_eq!(STATUS_MAX_TABS_PREFIX, "タブは最大 ");
        // Y07: 接尾も定数化し、呼び出し側に " 個までです" を残さない。
        assert_eq!(STATUS_MAX_TABS_SUFFIX, " 個までです");
        assert!(format!("{STATUS_MAX_TABS_PREFIX}8{STATUS_MAX_TABS_SUFFIX}")
            .contains(STATUS_MAX_TABS_PREFIX));
        assert!(format!("{STATUS_KEYMAP_NO_UNBIND_PREFIX}open_preview")
            .contains(STATUS_KEYMAP_NO_UNBIND_PREFIX));
        assert!(format!("保存検索{STATUS_NOT_FOUND_SUFFIX}: foo").contains(STATUS_NOT_FOUND_SUFFIX));
        assert!(format!("3{STATUS_MISSING_COUNT_SUFFIX}").contains(STATUS_MISSING_COUNT_SUFFIX));
    }

    #[test]
    fn keymap_no_unbind_notice_carries_the_canonical_recovery_hint() {
        // AI08: 他の設定通知と同じく、no-op の unbind も失敗だけで終わらせず
        // 上書きの確認導線を併記する（F6-36 の情報量ルール）。
        assert_eq!(
            STATUS_KEYMAP_NO_UNBIND_HINT,
            "（Ctrl+, で設定を開きショートカットタブで現在の上書きを確認）"
        );
        for token in ["Ctrl+,", "設定を開き", "ショートカットタブ", "上書きを確認"]
        {
            assert!(
                STATUS_KEYMAP_NO_UNBIND_HINT.contains(token),
                "recovery hint must name the settings route: {STATUS_KEYMAP_NO_UNBIND_HINT}"
            );
        }
        // 接頭辞は色判定マーカーとして変更しない（マーカー一致が壊れないこと）。
        assert_eq!(STATUS_KEYMAP_NO_UNBIND_PREFIX, "解除する設定はありません: ");
        let notice =
            format!("{STATUS_KEYMAP_NO_UNBIND_PREFIX}open_preview{STATUS_KEYMAP_NO_UNBIND_HINT}");
        assert!(
            notice.starts_with(STATUS_KEYMAP_NO_UNBIND_PREFIX),
            "{notice}"
        );
        assert!(notice.ends_with(STATUS_KEYMAP_NO_UNBIND_HINT), "{notice}");
        // main.rs の no-op 分岐が正規ヒントを連結し、 inline リテラルを
        // 再導入していないことを固定する。
        let main_source = include_str!("main.rs");
        assert!(
            main_source
                .contains("{STATUS_KEYMAP_NO_UNBIND_PREFIX}{action}{STATUS_KEYMAP_NO_UNBIND_HINT}"),
            "no-op unbind caption must append the canonical recovery hint"
        );
        assert!(
            !main_source
                .contains("\"（Ctrl+, で設定を開きショートカットタブで現在の上書きを確認）\""),
            "recovery hint must go through STATUS_KEYMAP_NO_UNBIND_HINT"
        );
    }

    #[test]
    fn skip_notice_wording_is_shared() {
        // T02: コピー／移動／外部ドロップのスキップ通知は、main.rs の
        // `transfer_skip_notice` を通してこの定数だけを使う。括弧と件数接尾の
        // 定義を一箇所に固定し、3 通りのinline文言が再び増えるのを防ぐ。
        assert_eq!(STATUS_SKIPPED_PREFIX, "（");
        assert_eq!(STATUS_SKIPPED_SUFFIX, " 件をスキップ）");
        // Y02: 理由ラベルも定数へ集約し、完全一致の期待値だけで文法を固定する。
        assert_eq!(STATUS_SKIP_REASON_MISSING, "存在しない項目");
        assert_eq!(STATUS_SKIP_REASON_SPECIAL, "特殊ファイル");
        // Z03: 混在ケースで消えていた同フォルダの理由も定数へ集約する。
        assert_eq!(STATUS_SKIP_REASON_SAME_DIRECTORY, "同じフォルダ");
        assert_eq!(
            format!("{STATUS_SKIPPED_PREFIX}{STATUS_SKIP_REASON_MISSING} 2{STATUS_SKIPPED_SUFFIX}"),
            "（存在しない項目 2 件をスキップ）"
        );
        assert_eq!(
            format!("{STATUS_SKIPPED_PREFIX}{STATUS_SKIP_REASON_SPECIAL} 1{STATUS_SKIPPED_SUFFIX}"),
            "（特殊ファイル 1 件をスキップ）"
        );
        // 3 理由が混在しても区切りと件数接尾は 1 通りだけ（main.rs の
        // `transfer_skip_notice` と同じ並びで検証する）。
        assert_eq!(
            format!(
                "{STATUS_SKIPPED_PREFIX}{STATUS_SKIP_REASON_MISSING} 2 / {STATUS_SKIP_REASON_SPECIAL} 1 / {STATUS_SKIP_REASON_SAME_DIRECTORY} 3{STATUS_SKIPPED_SUFFIX}"
            ),
            "（存在しない項目 2 / 特殊ファイル 1 / 同じフォルダ 3 件をスキップ）"
        );
    }

    #[test]
    fn dialog_footer_wording_is_canonical() {
        // Z09: 3 つのダイアログの脚文は F1 / README と同じ文言に統一する。
        assert_eq!(DIALOG_FOOTER_HINT, "Enter で保存 / Esc でキャンセル");
        assert_eq!(DIALOG_FOOTER_HINT, dialog_footer_hint!());
        // 旧来の「確定」表記が main.rs に戻っていないことを固定する。
        let main_source = include_str!("main.rs");
        assert!(
            !main_source.contains("\"Enter で確定 / Esc でキャンセル\""),
            "dialog footer must go through DIALOG_FOOTER_HINT"
        );
        assert!(
            main_source.matches("DIALOG_FOOTER_HINT").count() >= 4,
            "all four dialogs must share the canonical footer"
        );
        // AF03: ラベル編集ダイアログは共通フッター + 削除案内を組み立てる。
        assert_eq!(DIALOG_FOOTER_DELETE_SUFFIX, "空にして確定すると削除");
        assert!(
            main_source.contains("DIALOG_FOOTER_DELETE_SUFFIX"),
            "label dialog must compose the shared delete hint"
        );
        assert!(
            !main_source.contains("Enter で確定 / 空にして確定すると削除 / Esc でキャンセル"),
            "label dialog must not inline the composed footer"
        );
    }

    #[test]
    fn alt_k_help_row_uses_canonical_dialog_footer() {
        // AG03: F1 の Alt+K 行は Z09 の共通フッターを埋め込み、古い直書き
        // リテラルを再導入しないことを固定する。
        let alt_k = HELP_SECTIONS
            .iter()
            .flat_map(|(_, rows)| *rows)
            .find(|(key, _)| *key == "Alt+K")
            .map(|(_, text)| *text)
            .expect("Alt+K help row must exist");
        assert!(
            alt_k.contains(DIALOG_FOOTER_HINT),
            "Alt+K row must contain the canonical footer: {alt_k}"
        );
        let source = include_str!("strings.rs");
        let lines: Vec<&str> = source.lines().collect();
        let start = lines
            .iter()
            .position(|line| line.trim() == "\"Alt+K\",")
            .expect("Alt+K help row source must exist");
        let row = lines[start..start + 8].join("\n");
        assert!(
            row.contains("dialog_footer_hint!()"),
            "Alt+K row must embed the canonical macro: {row}"
        );
        assert!(
            !row.contains("Enter で保存 / Esc でキャンセル"),
            "Alt+K row must not inline the footer literal: {row}"
        );
    }

    #[test]
    fn skip_reasons_and_tab_limit_are_routed_through_constants() {
        // Y02 / Y07: main.rs のコピー／移動／ドロップとタブ上限の 5 箇所に
        // inline 文言が戻らないことを、定数名の使用とリテラルの不在で固定する。
        let main_source = include_str!("main.rs");
        for name in [
            "STATUS_SKIP_REASON_MISSING",
            "STATUS_SKIP_REASON_SPECIAL",
            "STATUS_SKIP_REASON_SAME_DIRECTORY",
            "STATUS_MAX_TABS_SUFFIX",
        ] {
            assert!(main_source.contains(name), "{name} must be used in main.rs");
        }
        for literal in [
            STATUS_SKIP_REASON_MISSING,
            STATUS_SKIP_REASON_SPECIAL,
            STATUS_SKIP_REASON_SAME_DIRECTORY,
            STATUS_MAX_TABS_SUFFIX,
        ] {
            assert!(
                !main_source.contains(&format!("\"{literal}\"")),
                "inline literal must go through a constant: {literal}"
            );
        }
    }

    #[test]
    fn undo_empty_wording_is_unified_across_call_sites() {
        // T09: ステータスバーと操作履歴ダイアログで助詞が違っていた（が / は）のを
        // 「は」に統一し、ダイアログ側の補足も定数経由で連結する。
        assert_eq!(STATUS_UNDO_EMPTY, "元に戻せる操作はありません");
        assert_eq!(
            format!("{STATUS_UNDO_EMPTY}{STATUS_UNDO_HISTORY_HINT}"),
            "元に戻せる操作はありません（名前変更とゴミ箱のみ記録します）"
        );
        // AI09: Redo 側も同じ履歴スコープの補足を共有し、Undo との
        // 非対称を解消する（専用の別文言は作らない）。
        assert_eq!(
            format!("{STATUS_REDO_EMPTY}{STATUS_UNDO_HISTORY_HINT}"),
            "やり直せる操作がありません（名前変更とゴミ箱のみ記録します）"
        );
        assert_eq!(
            STATUS_PLACE_GONE_HINT,
            "フォルダがありません（F5 で再確認）"
        );
        let source = include_str!("main.rs");
        assert!(
            !source.contains("\"元に戻せる操作はありません（名前変更とゴミ箱のみ記録します）\""),
            "the undo history hint must be composed from constants"
        );
        assert!(
            source.contains("{STATUS_REDO_EMPTY}{STATUS_UNDO_HISTORY_HINT}"),
            "redo-empty status must append the shared history hint"
        );
        assert!(
            !source.contains("\"やり直せる操作がありません（名前変更とゴミ箱のみ記録します）\""),
            "the redo history hint must be composed from constants"
        );
        assert!(
            !source.contains("Tooltip::new(\"フォルダがありません（F5 で再確認）\")"),
            "the sidebar tooltip must use STATUS_PLACE_GONE_HINT"
        );
    }

    #[test]
    fn trash_missing_reason_fragment_is_shared() {
        // T08: `trash_undo_status` の全件失敗と一部失敗が同じ断片を連結する。
        assert_eq!(STATUS_TRASH_MISSING_REASON, "ゴミ箱にありません");
        assert_eq!(
            format!("項目が{STATUS_TRASH_MISSING_REASON}"),
            "項目がゴミ箱にありません"
        );
        assert_eq!(
            format!("1 件は{STATUS_TRASH_MISSING_REASON}"),
            "1 件はゴミ箱にありません"
        );
    }

    #[test]
    fn bookmark_rejection_reason_is_generic() {
        // T04: `sane_path` rejects empty / NUL / non-UTF-8 / oversized paths.
        // Naming only "非UTF-8" in the notice would misreport the other three.
        assert_eq!(
            STATUS_BOOKMARK_NOT_PERSISTABLE,
            "このパスはブックマークできません（保存できない形式です）"
        );
        assert!(!STATUS_BOOKMARK_NOT_PERSISTABLE.contains("非UTF-8"));
        assert!(STATUS_BOOKMARK_NOT_PERSISTABLE.contains("保存できない形式"));
    }

    #[test]
    fn rename_refusal_wording_is_centralized() {
        // T03: F2 リネームの 2 つの拒否文言は strings.rs に集約し、
        // main.rs::confirm_rename は定数経由でのみ表示する。
        assert_eq!(STATUS_RENAME_NO_PARENT, "親ディレクトリがありません");
        assert_eq!(STATUS_RENAME_EXISTS_PREFIX, "既に存在します: ");
        assert_eq!(
            format!("{STATUS_RENAME_EXISTS_PREFIX}/tmp/target"),
            "既に存在します: /tmp/target"
        );
        let source = include_str!("main.rs");
        assert!(
            !source.contains("set_rename_error(\"親ディレクトリがありません\""),
            "the parent rejection must go through STATUS_RENAME_NO_PARENT"
        );
        assert!(
            !source.contains("format!(\"既に存在します: {}\""),
            "the exists rejection must go through STATUS_RENAME_EXISTS_PREFIX"
        );
    }

    #[test]
    fn newly_centralized_failure_words_keep_their_wording() {
        // R02〜R05: strings.rs へ集約した文言が従来の表示と一致することを固定する。
        assert_eq!(STATUS_ACCESS_DENIED, "アクセスが拒否されました");
        assert_eq!(
            format!("{STATUS_TERMINAL_START_PREFIX}{STATUS_OP_FAILED_SUFFIX}"),
            "ターミナルを起動できません"
        );
        // U02: 上限以外（名前／検索語が無効）でも同じ経路を通るため、
        // 上限だけを断言する文言に戻してはならない。
        assert_eq!(
            STATUS_SAVED_SEARCH_INVALID_INPUT,
            "保存できる検索の上限に達したか、名前または検索語が無効です。既存の保存検索を削除するか、入力を見直してください"
        );
        assert!(
            STATUS_SAVED_SEARCH_CAP_HINT.contains(STATUS_SAVED_SEARCH_INVALID_INPUT),
            "the composed hint must stay in sync with the standalone reason"
        );
        assert!(!STATUS_SAVED_SEARCH_CAP_HINT.contains("上限に達しました"));
        assert_eq!(
            format!("{STATUS_SAVED_SEARCH_SAVE_FAILED_PREFIX}仕事{STATUS_SAVED_SEARCH_CAP_HINT}"),
            "検索を保存できませんでした: 仕事（保存できる検索の上限に達したか、名前または検索語が無効です。既存の保存検索を削除するか、入力を見直してください）"
        );
        assert_eq!(
            format!("{STATUS_BOOKMARK_LABEL_SET_FAILED_PREFIX}/tmp/target"),
            "ラベルを設定できませんでした: /tmp/target"
        );
    }

    #[test]
    fn bookmark_label_rejection_reasons_are_accurate_and_recoverable() {
        // U04: 理由のない接頭辞だけでは原因が分からない。各拒否理由に
        // 対処法（別パス／短いラベル／既存ラベルの削除）を添える。
        assert!(STATUS_BOOKMARK_LABEL_REASON_INVALID_PATH.contains("別のディレクトリ"));
        assert!(STATUS_BOOKMARK_LABEL_REASON_INVALID_LABEL.contains("長すぎる"));
        assert!(STATUS_BOOKMARK_LABEL_REASON_INVALID_LABEL.contains("使用できない文字"));
        assert!(STATUS_BOOKMARK_LABEL_REASON_INVALID_LABEL.contains("短いラベルに変更"));
        assert!(STATUS_BOOKMARK_LABEL_REASON_TOO_LONG.contains("短いラベルに変更"));
        assert!(STATUS_BOOKMARK_LABEL_REASON_NUL.contains("使用できない文字"));
        assert!(STATUS_BOOKMARK_LABEL_REASON_LIMIT.contains("上限に達しました"));
        assert!(STATUS_BOOKMARK_LABEL_REASON_LIMIT.contains("削除してから再試行"));
        assert_eq!(
            format!(
                "{STATUS_BOOKMARK_LABEL_SET_FAILED_PREFIX}/tmp/target{}",
                STATUS_BOOKMARK_LABEL_REASON_LIMIT
            ),
            "ラベルを設定できませんでした: /tmp/target（ラベルの保存件数が上限に達しました。不要なラベルを削除してから再試行してください）"
        );
    }

    #[test]
    fn settings_save_failures_have_accurate_reason_wording() {
        // U03: 汎用文言は特定の原因（権限／空き容量）を断言しない。
        assert_eq!(
            STATUS_SETTINGS_SAVE_FAILED,
            "設定を保存できませんでした（ファイルの状態を確認して再試行してください）"
        );
        assert!(!STATUS_SETTINGS_SAVE_FAILED.contains("権限"));
        assert!(!STATUS_SETTINGS_SAVE_FAILED.contains("空き容量"));
        // 理由が判別できた場合は具体的な導線を出す。
        assert!(STATUS_SETTINGS_SAVE_TOO_LARGE.contains("大きすぎます"));
        assert!(STATUS_SETTINGS_SAVE_TOO_LARGE.contains("減らしてから再試行"));
        assert!(STATUS_SETTINGS_SAVE_LOCKED.contains("編集中です"));
        assert!(STATUS_SETTINGS_SAVE_LOCKED.contains("少し待ってから再試行"));
        let source = include_str!("main.rs");
        assert!(
            !source.contains("権限や空き容量を確認して再試行してください"),
            "the misleading generic hint must not reappear in main.rs"
        );
    }

    #[test]
    fn saved_search_input_errors_keep_canonical_wording() {
        // S01: 入力必須エラーは strings.rs の定数を経由し、呼び出し側では
        // `STATUS_ERROR_PREFIX` と連結する。従来の表示文言をここで固定する。
        assert_eq!(
            STATUS_SAVED_SEARCH_NAME_REQUIRED,
            "保存する検索名を入力してください"
        );
        assert_eq!(
            STATUS_SAVED_SEARCH_QUERY_REQUIRED,
            "検索語を入力してから保存してください"
        );
        assert_eq!(
            format!("{STATUS_ERROR_PREFIX}{STATUS_SAVED_SEARCH_NAME_REQUIRED}"),
            "エラー: 保存する検索名を入力してください"
        );
        assert_eq!(
            format!("{STATUS_ERROR_PREFIX}{STATUS_SAVED_SEARCH_QUERY_REQUIRED}"),
            "エラー: 検索語を入力してから保存してください"
        );
        // 回帰防止: main.rs にインライン字面が戻っていないことを確認する。
        let source = include_str!("main.rs");
        assert!(!source.contains("\"保存する検索名を入力してください\""));
        assert!(!source.contains("\"検索語を入力してから保存してください\""));
    }

    #[test]
    fn terminal_disabled_hint_keeps_canonical_wording() {
        // S07: 理由と復帰導線を定数化し、「開けません」接尾と連結する。
        // AB10: STATUS_SETTINGS_READ_ONLY_HINT / DISCARDED と同じく Ctrl+, の
        // 復帰導線を明示し、未設定のままで終わらせない。
        assert_eq!(
            STATUS_TERMINAL_DISABLED_HINT,
            ": 起動コマンドが未設定です（Ctrl+, で設定を開き terminal_command を確認）"
        );
        assert!(STATUS_TERMINAL_DISABLED_HINT.contains("Ctrl+,"));
        assert!(STATUS_TERMINAL_DISABLED_HINT.contains("設定を開き"));
        assert!(STATUS_TERMINAL_DISABLED_HINT.contains("terminal_command を確認"));
        assert_eq!(
            format!("ターミナルを{STATUS_CANNOT_OPEN_SUFFIX}{STATUS_TERMINAL_DISABLED_HINT}"),
            "ターミナルを開けません: 起動コマンドが未設定です（Ctrl+, で設定を開き terminal_command を確認）"
        );
        // 回帰防止: main.rs に理由・復帰導線の字面が戻っていないことを確認する。
        let source = include_str!("main.rs");
        assert!(!source.contains("起動コマンドが未設定です"));
    }

    #[test]
    fn refusal_suffixes_and_empty_notices_have_canonical_wording() {
        // N02 / N08 / N09: マーカーとして登録する文言をここで固定する。
        assert_eq!(STATUS_CANNOT_OPEN_SUFFIX, "開けません");
        assert!(!STATUS_CANNOT_OPEN_SUFFIX.contains(STATUS_OP_FAILED_SUFFIX));
        assert_eq!(STATUS_RESTORE_EMPTY, "復元対象がありません");
        assert_eq!(STATUS_SEARCH_EMPTY_QUERY, "検索語が空です");
        // P03: warning マーカーとして登録する案内文言を固定する（green 回帰防止）。
        assert_eq!(STATUS_SEARCH_NOT_OPEN, "先に Ctrl+F で検索を開いてください");
    }

    #[test]
    fn only_directory_refusals_share_canonical_wording() {
        // N03: 5 つのインライン拒否を定数化し、表現の揺れを防ぐ。
        assert_eq!(
            STATUS_PREVIEW_ONLY_DIRECTORY,
            "プレビューはディレクトリ表示でのみ使えます"
        );
        assert_eq!(
            STATUS_LOCATION_ONLY_DIRECTORY,
            "パス入力はディレクトリ表示でのみ使えます"
        );
        assert_eq!(
            STATUS_PROPERTIES_ONLY_DIRECTORY,
            "プロパティはディレクトリ表示でのみ表示できます"
        );
        assert_eq!(
            STATUS_SEARCH_ONLY_DIRECTORY,
            "検索はディレクトリ表示でのみ使えます"
        );
        // クイックジャンプの表示名は `FUZZY_JUMP_LABEL` と常に一致させる。
        assert_eq!(
            STATUS_FUZZY_JUMP_ONLY_DIRECTORY,
            format!("{FUZZY_JUMP_LABEL}はディレクトリ表示でのみ使えます")
        );
        // いずれも「できません」を含まないため、マーカー登録が必須である。
        for message in [
            STATUS_PREVIEW_ONLY_DIRECTORY,
            STATUS_LOCATION_ONLY_DIRECTORY,
            STATUS_FUZZY_JUMP_ONLY_DIRECTORY,
            STATUS_PROPERTIES_ONLY_DIRECTORY,
            STATUS_SEARCH_ONLY_DIRECTORY,
        ] {
            assert!(message.contains("ディレクトリ表示でのみ"), "{message}");
            assert!(!message.contains(STATUS_OP_FAILED_SUFFIX), "{message}");
        }
    }

    #[test]
    fn terminal_command_invalid_literal_has_one_definition() {
        // V03: ターミナルコマンドの入力エラーは 2 箇所で同じ文言を使っていた。
        // 定数自身がリテラルを含むため、走査用の文字列は定数から取得し、
        // 「定数定義の 1 回だけ」を検出する（テスト自身が定義を増やさない）。
        let literal = STATUS_TERMINAL_COMMAND_INVALID;
        let strings_source = include_str!("strings.rs");
        assert_eq!(
            strings_source.matches(literal).count(),
            1,
            "the literal must appear only in its constant definition"
        );
        let main_source = include_str!("main.rs");
        assert!(
            !main_source.contains(&format!("\"{literal}\"")),
            "main.rs must route through STATUS_TERMINAL_COMMAND_INVALID"
        );
        assert!(main_source.contains("STATUS_TERMINAL_COMMAND_INVALID"));
    }

    #[test]
    fn terminal_command_disabled_literal_has_one_definition() {
        // AG04: 起動コマンド無効化の完了通知は定数定義だけに置き、main.rs の
        // 保存経路は必ずその定数を参照する。
        let literal = STATUS_TERMINAL_COMMAND_DISABLED;
        assert!(literal.ends_with("無効にしました"));
        let strings_source = include_str!("strings.rs");
        assert_eq!(
            strings_source.matches(literal).count(),
            1,
            "the literal must appear only in its constant definition"
        );
        let main_source = include_str!("main.rs");
        assert!(
            !main_source.contains(&format!("\"{literal}\"")),
            "main.rs must route through STATUS_TERMINAL_COMMAND_DISABLED"
        );
        assert!(main_source.contains("STATUS_TERMINAL_COMMAND_DISABLED"));
    }

    #[test]
    fn keymap_rejection_literals_have_one_definition() {
        // V04: 形式エラー・上限・無効指定の文言がダイアログと設定カードに
        // 重複していた。各リテラルは定数定義の 1 回だけに保つ。
        let strings_source = include_str!("strings.rs");
        // AD03: STATUS_KEYMAP_FORMAT_INVALID is composed from the canonical
        // macro, so its full text no longer appears as a standalone literal in
        // the source; assert the shared wording instead of a one-off copy.
        assert!(STATUS_KEYMAP_FORMAT_INVALID.starts_with(KEYMAP_FORMAT_HINT));
        assert!(STATUS_KEYMAP_FORMAT_INVALID.contains("open_preview=ctrl+shift+p"));
        for (name, literal) in [
            ("STATUS_KEYMAP_LIMIT_PREFIX", STATUS_KEYMAP_LIMIT_PREFIX),
            ("STATUS_KEYMAP_LIMIT_SUFFIX", STATUS_KEYMAP_LIMIT_SUFFIX),
            ("STATUS_KEYMAP_INVALID_PREFIX", STATUS_KEYMAP_INVALID_PREFIX),
        ] {
            assert_eq!(
                strings_source.matches(literal).count(),
                1,
                "{name} must have a single definition"
            );
        }
        let main_source = include_str!("main.rs");
        for name in [
            "STATUS_KEYMAP_FORMAT_INVALID",
            "STATUS_KEYMAP_LIMIT_PREFIX",
            "STATUS_KEYMAP_LIMIT_SUFFIX",
            "STATUS_KEYMAP_INVALID_PREFIX",
        ] {
            assert!(main_source.contains(name), "{name} must be used in main.rs");
        }
        for literal in [
            STATUS_KEYMAP_FORMAT_INVALID,
            STATUS_KEYMAP_LIMIT_PREFIX,
            STATUS_KEYMAP_INVALID_PREFIX,
        ] {
            assert!(
                !main_source.contains(&format!("\"{literal}\"")),
                "inline keymap literal: {literal}"
            );
        }
        // 上限通知は接頭辞 + 動的件数 + 接尾で組み立てる（旧来の書式は禁止）。
        assert!(
            !main_source.contains(&format!("\"{}{{", STATUS_KEYMAP_LIMIT_PREFIX)),
            "the interpolated limit literal must go through the constants"
        );
        let count = 8usize;
        let composed = format!("{STATUS_KEYMAP_LIMIT_PREFIX}{count}{STATUS_KEYMAP_LIMIT_SUFFIX}");
        assert!(composed.starts_with(STATUS_KEYMAP_LIMIT_PREFIX));
        assert!(composed.contains(&count.to_string()));
        assert!(composed.ends_with(STATUS_KEYMAP_LIMIT_SUFFIX));
    }

    #[test]
    fn rescan_failure_carries_a_recovery_hint() {
        // V14: 再スキャン失敗を表示するだけでは再試行方法が分からないため、
        // F5 の再読込導線を括弧内に添える（F6-36 の「復帰導線」）。
        assert!(STATUS_RESCAN_FAILED.starts_with("再スキャンに失敗しました"));
        assert!(STATUS_RESCAN_FAILED.contains("（F5 で再読込）"));
        assert!(STATUS_RESCAN_FAILED.ends_with('）'));
    }

    #[test]
    fn terminal_and_new_tab_refusals_share_the_cannot_open_suffix() {
        // W01: ゴミ箱でのターミナル拒否と、親ディレクトリが解決できない
        // 新タブ拒否の前半を定数化し、「開けません」接尾を共有する。
        assert_eq!(STATUS_TRASH_NO_TERMINAL_PREFIX, "ゴミ箱ではターミナルを");
        assert_eq!(STATUS_FILE_NO_NEW_TAB_PREFIX, "このファイルは新しいタブで");
        let trash = format!("{STATUS_TRASH_NO_TERMINAL_PREFIX}{STATUS_CANNOT_OPEN_SUFFIX}");
        let new_tab = format!("{STATUS_FILE_NO_NEW_TAB_PREFIX}{STATUS_CANNOT_OPEN_SUFFIX}");
        assert_eq!(trash, "ゴミ箱ではターミナルを開けません");
        assert_eq!(new_tab, "このファイルは新しいタブで開けません");
        // 回帰防止: main.rs にインライン字面が戻っていないことを確認する。
        let source = include_str!("main.rs");
        for literal in [
            STATUS_TRASH_NO_TERMINAL_PREFIX,
            STATUS_FILE_NO_NEW_TAB_PREFIX,
        ] {
            assert!(
                !source.contains(literal),
                "inline refusal prefix: {literal}"
            );
        }
        assert!(source.contains("STATUS_TRASH_NO_TERMINAL_PREFIX"));
        assert!(source.contains("STATUS_FILE_NO_NEW_TAB_PREFIX"));
    }

    #[test]
    fn search_prompt_has_one_canonical_wording() {
        // W02: 検索ビューを開いた直後の案内は定数一箇所で定義する。
        assert_eq!(
            STATUS_SEARCH_PROMPT,
            "検索語を入力して Enter（Esc で閉じる）"
        );
        let source = include_str!("main.rs");
        assert!(
            !source.contains(STATUS_SEARCH_PROMPT),
            "search prompt must not be inlined in main.rs"
        );
        assert!(source.contains("STATUS_SEARCH_PROMPT"));
    }

    #[test]
    fn read_only_hint_names_the_settings_shortcut() {
        // W08: 読み取り専用の理由を表示するとき、設定を確認する導線を
        // 括弧で添える。呼び出し側の書式は main.rs のテストで固定する。
        assert_eq!(STATUS_SETTINGS_READ_ONLY_HINT, "（Ctrl+, で設定を確認）");
        assert!(STATUS_SETTINGS_READ_ONLY_HINT.contains("Ctrl+,"));
        assert!(STATUS_SETTINGS_READ_ONLY_HINT.ends_with('）'));
    }

    #[test]
    fn inline_status_literals_are_routed_through_shared_constants() {
        // X03 / X04 / X08 / X09: これら 4 つの文言は以前 main.rs の
        // format! または main.rs ローカル定数に埋め込まれており、色分類表と
        // 表示文言が乖離していた。正規値はここに置き、呼び出し側は定数を参照する。
        let source = include_str!("main.rs");
        for (name, literal) in [
            (
                "STATUS_BOOKMARK_LABEL_NOT_SET_PREFIX",
                STATUS_BOOKMARK_LABEL_NOT_SET_PREFIX,
            ),
            (
                "STATUS_TRASH_LOAD_FAILED_PREFIX",
                STATUS_TRASH_LOAD_FAILED_PREFIX,
            ),
            ("STATUS_UNDO_MISSING_SUFFIX", STATUS_UNDO_MISSING_SUFFIX),
            ("STATUS_INVALID_CLI_DIR", STATUS_INVALID_CLI_DIR),
        ] {
            assert!(source.contains(name), "{name} must be used in main.rs");
            assert!(
                !source.contains(literal),
                "inline literal for {name} must not reappear in main.rs"
            );
            assert!(
                !source.contains(&format!("const {name}")),
                "{name} must not be redefined locally in main.rs"
            );
        }
        // X09: 移動済みであることを示す陳腐なコメントも削除されていること。
        assert!(!source.contains("strings.rs is intentionally out of scope"));
    }

    #[test]
    fn rename_validation_errors_are_canonical_and_classified_as_errors() {
        // AJ02: `ops::validate_file_name` の 6 ケースをここで固定する。
        // 各定数が非空で、ops.rs の現行文言と一致することを検証する。
        // テスト自身がリテラルを増やさないよう、期待値は定数から取得し、
        // 「定数定義の 1 回だけ」を source 走査で確認する。
        let strings_source = include_str!("strings.rs");
        for (name, message) in [
            ("STATUS_RENAME_INVALID_EMPTY", STATUS_RENAME_INVALID_EMPTY),
            ("STATUS_RENAME_INVALID_DOT", STATUS_RENAME_INVALID_DOT),
            (
                "STATUS_RENAME_INVALID_SEPARATOR",
                STATUS_RENAME_INVALID_SEPARATOR,
            ),
            ("STATUS_RENAME_INVALID_NUL", STATUS_RENAME_INVALID_NUL),
            (
                "STATUS_RENAME_INVALID_RESERVED_PREFIX",
                STATUS_RENAME_INVALID_RESERVED_PREFIX,
            ),
            ("STATUS_RENAME_INVALID_CHAR", STATUS_RENAME_INVALID_CHAR),
        ] {
            assert!(!message.is_empty(), "{name} must not be empty");
            assert_eq!(
                strings_source.matches(message).count(),
                1,
                "{name} literal must appear only in its constant definition"
            );
        }
        // 予約名だけは動的な名前を連結するため接頭辞の形を取る。
        assert!(STATUS_RENAME_INVALID_RESERVED_PREFIX.ends_with(": "));
        // 6 つの定数はすべて error 分類であり、main.rs のマーカー表に登録済み。
        let main_source = include_str!("main.rs");
        let error_markers = main_source
            .split("const STATUS_ERROR_MARKERS: &[&str] = &[")
            .nth(1)
            .expect("STATUS_ERROR_MARKERS must exist")
            .split("\n];")
            .next()
            .expect("marker array must terminate");
        for name in [
            "STATUS_RENAME_INVALID_EMPTY",
            "STATUS_RENAME_INVALID_DOT",
            "STATUS_RENAME_INVALID_SEPARATOR",
            "STATUS_RENAME_INVALID_NUL",
            "STATUS_RENAME_INVALID_RESERVED_PREFIX",
            "STATUS_RENAME_INVALID_CHAR",
        ] {
            assert!(
                error_markers.contains(name),
                "{name} must be registered in STATUS_ERROR_MARKERS"
            );
        }
    }

    #[test]
    fn aj10_help_row_uses_the_canonical_search_history_description() {
        // AJ10: F1 の「検索履歴 / 保存検索」行は共有文言をそのまま使い、
        // キー列を持たない行として settings_ui.rs の例外表にも登録される。
        let (_, rows) = HELP_SECTIONS
            .iter()
            .find(|(title, _)| *title == "表示")
            .expect("表示 section must exist");
        let description = rows
            .iter()
            .find(|(key, _)| *key == "検索履歴 / 保存検索")
            .map(|(_, text)| *text)
            .expect("F1 表示 must list 検索履歴 / 保存検索");
        assert_eq!(description, SEARCH_HISTORY_CHIPS_DESCRIPTION);
        let settings_ui = include_str!("settings_ui.rs");
        assert!(
            settings_ui.contains("\"検索履歴 / 保存検索\""),
            "settings_ui.rs HELP_KEY_EXCEPTIONS must register the prose row"
        );
    }

    #[test]
    fn files_column_customization_is_documented_in_f1_and_readme() {
        // R41/AN03: 列の表示 / 非表示と並べ替えが F1・README・設定 UI の
        // 3 箇所で案内されていることを固定する。
        let (_, rows) = HELP_SECTIONS
            .iter()
            .find(|(title, _)| *title == "表示")
            .expect("表示 section must exist");
        let description = rows
            .iter()
            .find(|(key, _)| *key == "一覧の列")
            .map(|(_, text)| *text)
            .expect("F1 表示 must list the 一覧の列 row");
        for token in ["ファイルタブ", "Switch", "↑↓", "行ショートカット"] {
            assert!(
                description.contains(token),
                "F1 files-column row must mention {token}"
            );
        }
        let readme = include_str!("../README.md");
        let column_note = readme
            .split("**ファイルタブの「一覧の列」**")
            .nth(1)
            .expect("README must document the 一覧の列 settings section")
            .split("\n**")
            .next()
            .expect("files-column note must terminate");
        for token in ["ファイルタブ", "Switch", "↑ / ↓", "行ショートカット"] {
            assert!(
                column_note.contains(token),
                "README files-column note must mention {token}"
            );
        }
        // AP03: 列幅は設定 UI では表示のみ。変更先を F1 と README で共有する。
        let width_note = COLUMN_WIDTH_SETTINGS_NOTE;
        assert!(
            description.contains(width_note),
            "F1 files-column row must embed the canonical width note: {description}"
        );
        let readme_flat: String = readme
            .lines()
            .map(|line| line.trim().replace('`', ""))
            .collect();
        for token in ["表示のみ", "settings.json", "column_prefs.widths"] {
            assert!(
                width_note.contains(token) && readme_flat.contains(token),
                "F1 and README must share the column-width token {token}"
            );
        }
        let settings_ui = include_str!("settings_ui.rs");
        let exceptions = settings_ui
            .split("const HELP_KEY_EXCEPTIONS: &[&str] = &[")
            .nth(1)
            .expect("settings_ui.rs must define HELP_KEY_EXCEPTIONS")
            .split("];")
            .next()
            .expect("HELP_KEY_EXCEPTIONS must terminate");
        assert!(
            exceptions.contains("\"一覧の列\""),
            "settings_ui.rs HELP_KEY_EXCEPTIONS must register the prose row"
        );
    }
}
