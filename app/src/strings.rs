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
            ("Alt+Shift+T", "テーマをダーク / ライト / システムで切替"),
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
            (
                "設定ダイアログ",
                "Enter で保存 / Esc でキャンセル（未保存変更時は破棄され通知されます）/ 上部の検索欄で設定項目を絞り込み（キー名でも検索可）。F1 はダイアログ内で無効なので、下部のキー凡例に必要なキーを表示",
            ),
            (
                "設定ダイアログのタブ",
                "Ctrl+Tab / Ctrl+Shift+Tab で次 / 前のタブへ移動",
            ),
            (
                "設定ダイアログのタブ（直接）",
                "Alt+1〜5 で外観 / ファイル / ショートカット / ターミナル / 個人化へ直接移動",
            ),
            (
                "設定ダイアログの並び順",
                "ファイルタブで Alt+S により「既定の並び順」を切替",
            ),
            (
                "設定ダイアログの行ショートカット",
                "外観タブで t=配色 / v=既定のビュー、ファイルタブで h=隠しファイルを切替（修飾キーなし・入力欄フォーカス中は無効）",
            ),
            (
                "設定ダイアログの操作",
                "Tab / Shift+Tab で検索欄 → 入力欄 → キャンセル / 保存へフォーカス移動（循環）",
            ),
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
/// dialog-ownership role (see `app/src/main.rs` keymap helpers). The
/// `open_settings` notice explains why the shipped Ctrl+, cannot be moved: the
/// F1 help and the discard-recovery hint promise that chord (L01). V02: the
/// comma key itself is reserved too (`settings::is_reserved_chord` rejects
/// `escape` / `f1` / `,`), so the hint must name all three.
pub const KEYMAP_INPUT_HINT: &str =
    "形式: アクション名 コード（例: open_preview ctrl+shift+p）。unbind アクション名で設定を解除。escape / f1 は予約キーです（, も Ctrl+, 固定のため予約キー）。open_settings は Ctrl+, 固定で変更できません（unbind も不可）。";

// ---------------------------------------------------------------------------
// 空状態・読み込み
// ---------------------------------------------------------------------------

/// Empty-state title + optional hint per listing source.
pub const EMPTY_TRASH: (&str, &str) = (
    "ゴミ箱は空です",
    "削除した項目はここに表示されます（完全削除は未対応）。Alt+← で前の場所へ",
);
pub const EMPTY_SEARCH: (&str, &str) = (
    "一致する項目がありません",
    "検索語や絞り込みを変更してください（Ctrl+F で再検索）",
);
pub const EMPTY_DIRECTORY: (&str, &str) = (
    "このフォルダは空です",
    "ここにファイルをドロップするか、Ctrl+Shift+N で新規フォルダ / Ctrl+V で貼り付け",
);
pub const EMPTY_FILTERED: (&str, &str) = (
    "絞り込みで全て除外されました",
    "種別や拡張子、名前（Ctrl+Shift+F）の条件を変更してください",
);
pub const LOADING: &str = "読み込み中…";
/// Recovery hint shown under a load error in the empty-state overlay (U-20).
pub const LOAD_ERROR_HINT: &str = "F5 で再読込 / Ctrl+L で別のパスへ移動";
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
pub const STATUS_SEARCH_STALE: &str = "変更を検出しました（再検索が必要です）";
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
/// スキップ件数通知の開き括弧。`main.rs::transfer_skip_notice` が
/// 「{理由} {N} 件をスキップ」を囲むために使う（T02）。
pub const STATUS_SKIPPED_PREFIX: &str = "（";
/// スキップ件数通知の閉じ括弧。件数に続く「件をスキップ」と一体化させ、
/// コピー／移動／ドロップの 3 箇所で同じ文言になるよう共有する（T02）。
pub const STATUS_SKIPPED_SUFFIX: &str = " 件をスキップ）";
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

/// OS 連携の成功 notification。戻り値を確認せずに出す現状の文言（r4-error-01）。
pub const STATUS_OPENED_DEFAULT_SUFFIX: &str = "を既定のアプリで開きました";
pub const STATUS_REVEALED_PREFIX: &str = "ファイルマネージャで表示: ";
/// キーマップの `unbind` で解除するオーバーライドがなかったときの通知接頭辞。
/// 動的な `{action}` を連結するため、接頭辞自体を色判定のマーカーとして登録する。
pub const STATUS_KEYMAP_NO_UNBIND_PREFIX: &str = "解除する設定はありません: ";
/// キーマップ入力の形式が不正なときの通知（V04）。
/// Alt+K ダイアログと設定カードの Shortcuts 欄で同じ文言を共有する。
pub const STATUS_KEYMAP_FORMAT_INVALID: &str =
    "形式は「アクション名 コード」または「unbind アクション名」です（例: open_preview ctrl+shift+p）";
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
/// Esc / 背景クリックで未保存の設定変更を破棄したときの通知（H03）。
/// フッターの「保存せず閉じる」ボタンと違い、これらの操作は事前の警告がない。
/// 情報量ルール (F6-36) の「復帰導線」に従い、設定を開き直す操作を併記する（I06）。
pub const STATUS_SETTINGS_DISCARDED: &str =
    "未保存の変更を破棄しました（Ctrl+, で設定を開き直せます）";
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

/// OS がプロパティのメタデータ読み取りを拒否したときの通知（R02）。
/// 以前は main.rs ローカルだったが、色分類表と一元化するためここへ移動した。
pub const STATUS_ACCESS_DENIED: &str = "アクセスが拒否されました";
/// ターミナル起動失敗の接頭辞（R03）。`STATUS_OP_FAILED_SUFFIX` と連結して
/// 「ターミナルを起動できません」を組み立てる。
pub const STATUS_TERMINAL_START_PREFIX: &str = "ターミナルを起動";
/// ターミナル起動コマンドが長すぎる／制御文字を含むときの入力エラー（V03）。
/// ターミナルコマンドダイアログと設定カードの両方で共有する唯一の定義。
pub const STATUS_TERMINAL_COMMAND_INVALID: &str = "コマンドが長すぎるか、制御文字を含んでいます";
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
    ": 起動コマンドが未設定です（設定の terminal_command を確認）";

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

/// `種類` 列のラベル（`model::kind_label` で使用）。
pub const KIND_FOLDER: &str = "フォルダ";
pub const KIND_LINK: &str = "リンク";
pub const KIND_NONE: &str = "—";

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
        assert!(
            dialog.contains("Esc") && dialog.contains("キャンセル"),
            "{dialog}"
        );
        assert!(dialog.contains("検索"), "{dialog}");
        // R07: the dialog search covers key names too, not just setting labels.
        assert!(dialog.contains("キー名でも検索可"), "{dialog}");
        // R11: Esc cancellation warns when unsaved changes would be discarded.
        assert!(
            dialog.contains("未保存変更時は破棄され通知されます"),
            "{dialog}"
        );
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
        for token in ["検索語", "Ctrl+F", "再検索"] {
            assert!(search.contains(token), "search hint: {search}");
        }

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
        for token in ["Alt+←", "前の場所へ"] {
            assert!(trash.contains(token), "trash hint: {trash}");
        }
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
            STATUS_CANCELLED,
            STATUS_CANCEL_HINT,
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
            STATUS_KEYMAP_NO_UNBIND_PREFIX,
            STATUS_KEYMAP_FORMAT_INVALID,
            STATUS_KEYMAP_LIMIT_PREFIX,
            STATUS_KEYMAP_LIMIT_SUFFIX,
            STATUS_KEYMAP_INVALID_PREFIX,
            STATUS_MAX_TABS_PREFIX,
            STATUS_SETTINGS_SAVE_FAILED,
            STATUS_SETTINGS_SAVE_TOO_LARGE,
            STATUS_SETTINGS_SAVE_LOCKED,
            STATUS_SETTINGS_READ_ONLY,
            STATUS_SETTINGS_DISCARDED,
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
            STATUS_THEME_CHANGED_PREFIX,
            // R02〜R05: strings.rs へ集約した拒否・失敗文言。
            STATUS_ACCESS_DENIED,
            STATUS_TERMINAL_START_PREFIX,
            STATUS_TERMINAL_COMMAND_INVALID,
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
        ] {
            assert!(!message.is_empty(), "status message must not be empty");
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
            (
                "STATUS_KEYMAP_NO_UNBIND_PREFIX",
                STATUS_KEYMAP_NO_UNBIND_PREFIX,
                "warning",
            ),
            // 既に error markers に登録済みの concrete failure。
            ("STATUS_UNDO_FAILED", STATUS_UNDO_FAILED, "error"),
            ("STATUS_REDO_FAILED", STATUS_REDO_FAILED, "error"),
            ("STATUS_RENAME_FAILED", STATUS_RENAME_FAILED, "error"),
            ("STATUS_TARGET_GONE", STATUS_TARGET_GONE, "error"),
            // T10: 汎用の「対象が存在しません」も main.rs の error マーカーに
            // 登録済み（C_RED）なので、色分類表にも明示的に載せる。
            ("STATUS_PATH_GONE", STATUS_PATH_GONE, "error"),
            ("STATUS_TARGET_MISSING", STATUS_TARGET_MISSING, "error"),
            ("STATUS_DROP_NOT_FOUND", STATUS_DROP_NOT_FOUND, "error"),
            ("STATUS_NOT_DIRECTORY", STATUS_NOT_DIRECTORY, "error"),
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
                    || message.contains("拒否されました"),
                "{name} is not refusal-worded: {message}"
            );
            assert!(
                *class == "warning" || *class == "error",
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
        assert_eq!(OP_RESULT_PRIORITY[3], "成功");
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
        assert!(format!("{STATUS_MAX_TABS_PREFIX}8 個までです").contains(STATUS_MAX_TABS_PREFIX));
        assert!(format!("{STATUS_KEYMAP_NO_UNBIND_PREFIX}open_preview")
            .contains(STATUS_KEYMAP_NO_UNBIND_PREFIX));
        assert!(format!("保存検索{STATUS_NOT_FOUND_SUFFIX}: foo").contains(STATUS_NOT_FOUND_SUFFIX));
        assert!(format!("3{STATUS_MISSING_COUNT_SUFFIX}").contains(STATUS_MISSING_COUNT_SUFFIX));
    }

    #[test]
    fn skip_notice_wording_is_shared() {
        // T02: コピー／移動／外部ドロップのスキップ通知は、main.rs の
        // `transfer_skip_notice` を通してこの定数だけを使う。括弧と件数接尾の
        // 定義を一箇所に固定し、3 通りのinline文言が再び増えるのを防ぐ。
        assert_eq!(STATUS_SKIPPED_PREFIX, "（");
        assert_eq!(STATUS_SKIPPED_SUFFIX, " 件をスキップ）");
        assert_eq!(
            format!("{STATUS_SKIPPED_PREFIX}存在しない項目 2{STATUS_SKIPPED_SUFFIX}"),
            "（存在しない項目 2 件をスキップ）"
        );
        assert_eq!(
            format!("{STATUS_SKIPPED_PREFIX}特殊ファイル 1{STATUS_SKIPPED_SUFFIX}"),
            "（特殊ファイル 1 件をスキップ）"
        );
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
        assert_eq!(
            STATUS_TERMINAL_DISABLED_HINT,
            ": 起動コマンドが未設定です（設定の terminal_command を確認）"
        );
        assert_eq!(
            format!("ターミナルを{STATUS_CANNOT_OPEN_SUFFIX}{STATUS_TERMINAL_DISABLED_HINT}"),
            "ターミナルを開けません: 起動コマンドが未設定です（設定の terminal_command を確認）"
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
    fn keymap_rejection_literals_have_one_definition() {
        // V04: 形式エラー・上限・無効指定の文言がダイアログと設定カードに
        // 重複していた。各リテラルは定数定義の 1 回だけに保つ。
        let strings_source = include_str!("strings.rs");
        for (name, literal) in [
            ("STATUS_KEYMAP_FORMAT_INVALID", STATUS_KEYMAP_FORMAT_INVALID),
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
}
