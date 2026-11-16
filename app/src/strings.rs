// SPDX-License-Identifier: AGPL-3.0-or-later
//! Centralized Japanese UI strings (DESIGN.md §8.5): first step toward i18n.
//!
//! Only self-contained literals live here. Messages that embed runtime values
//! stay at their call sites with `format!`.

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
            ("Ctrl+I", "プロパティ"),
            ("Space / F3", "テキストプレビュー"),
        ],
    ),
    (
        "タブ・ブックマーク",
        &[
            ("Ctrl+T / Ctrl+W", "タブを開く / 閉じる"),
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

/// Frequently used fixed status-bar messages.
pub const STATUS_CHANGED: &str = "変更を検出しました";
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
