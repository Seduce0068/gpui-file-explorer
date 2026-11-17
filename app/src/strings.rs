// SPDX-License-Identifier: AGPL-3.0-or-later
//! Centralized Japanese UI strings (DESIGN.md §8.4): first step toward i18n.
//!
//! Only self-contained literals live here. Messages that embed runtime values
//! stay at their call sites with `format!`.
//!
//! # 文言規約 (DESIGN.md §8.4)
//!
//! - 「フォルダ / ファイル」を使い、「ディレクトリ」はエラー詳細のみに限定する。
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

// ---------------------------------------------------------------------------
// F1 ヘルプオーバーレイ
// ---------------------------------------------------------------------------

/// Keyboard shortcuts shown by the F1 help overlay: (section, [(key, description)]).
pub const HELP_SECTIONS: &[(&str, &[(&str, &str)])] = &[
    (
        "ナビゲーション",
        &[
            ("Enter", "選択項目を開く"),
            ("Ctrl+L", "パスを直接入力して移動"),
            ("Alt+← / Alt+→", "戻る / 進む"),
            ("Alt+↑ / Backspace", "親ディレクトリへ"),
            ("Home / End", "先頭 / 末尾へ"),
            ("PageUp / PageDown", "ページ単位で移動"),
        ],
    ),
    (
        "選択",
        &[
            ("Ctrl+A", "全選択"),
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
            ("F2", "名前変更"),
            ("Delete", "ゴミ箱へ移動（完全削除なし）"),
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
        ],
    ),
    (
        "タブ・ブックマーク",
        &[
            ("Ctrl+T / Ctrl+W", "タブを開く / 閉じる"),
            ("Ctrl+Shift+T", "閉じたタブを復元"),
            ("Ctrl+Tab / Ctrl+Shift+Tab", "次のタブ / 前のタブ"),
            ("Ctrl+Shift+PageUp / PageDown", "タブを左 / 右へ移動"),
            ("Ctrl+D", "現在のディレクトリをブックマーク"),
        ],
    ),
    (
        "その他",
        &[
            ("F1", "このヘルプ"),
            ("Esc", "キャンセル / ダイアログを閉じる"),
            ("Ctrl+Q", "終了"),
        ],
    ),
];

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
    "ここにファイルをドロップするとコピーできます",
);
pub const EMPTY_FILTERED: (&str, &str) = (
    "絞り込みで全て除外されました",
    "種別や拡張子の条件を変更してください",
);
pub const LOADING: &str = "読み込み中…";

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
pub const STATUS_CANCELLING: &str = "キャンセルしています…";
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
// 実行時値と組み合わせる正規の単語（呼び出し側は未移行、下のテストで整合性を検査）
// ---------------------------------------------------------------------------

// 呼び出し側（main.rs / model.rs）への適用は F6-26 の書き込み範囲外のため別タスク。
// 適用完了まで非テストビルドでは未使用となるので dead_code を明示的に許可し、
// 下の整合性テストで値の乖離を検出する。
/// Placeholder for unavailable metadata (`model::human_age` / `human_datetime`).
#[allow(dead_code)]
pub const VALUE_UNKNOWN: &str = "-";
/// Canonical cancel label. Do not use the English `cancel` in prompt buttons.
#[allow(dead_code)]
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
}
