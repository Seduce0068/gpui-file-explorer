<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# PROGRESS — GPUI file explorer prototype

作業ログ（2026-11-11）。ユーザー承認: AGPL-3.0-or-later / 設計と試作。
担当: fe-dev（task-6）。最終更新: 2026-11-17（末尾の追記セクションを参照）。

## 状態: P0 完成・実行検証済み / P1 未着手（2026-11-11 時点）

| 項目 | 状態 |
|---|---|
| `cargo build` | ✅ 警告 0 |
| `cargo test` | ✅ 132 passed / 0 failed（約 0.4s、2026-11-16 時点） |
| Wayland 実行 + KWin D-Bus 確認 | ✅ ネイティブ Wayland、stderr 0 行 |
| スクリーンショット（XDG portal） | ✅ `screenshot/window-crop.png` |
| ファイル監視の実証 | ✅ 起動中に bash で作成 → 約2秒で一覧反映（`screenshot/watcher-refresh-crop.png`） |
| 終了 | ✅ `kill -TERM` → KWin ウィンドウ一覧から消滅、追加 stderr なし |
| README / NOTICE / LICENSE / SPDX | ✅ |
| P1（grid / rename / copy-move / tabs / search / volumes / multi-select） | ⏭ 未着手（README に明記） |

## タイムライン

- 14:34 `app/` 骨組み、Cargo.toml / `.cargo/config.toml` / `native-libs` symlink 作成
  （PoC の libxkbcommon-x11 リンク回避を継承）
- 14:38 依存ビルド成功（1m19s、プレースホルダ main）。API 調査:
  `ContextMenuExt` / `Window::prompt` / `Icon` / `IconName`（Lucide 全種）/ notify-debouncer-full 0.7 /
  async-channel / `UniformListScrollHandle` / `unstable` 周辺を実ソースで確認
- 14:50–15:14 データ層 + UI 実装
  - `model.rs`: Entry/read_dir、自然順（大小無視）ソート、隠しファイル、
    サイズ/日時、選択計算、パンくず、fingerprint、新規フォルダ名
  - `history.rs`: 戻る/進む履歴
  - `settings.rs`: serde_json 永続化（アトミック書き込み）
  - `icons.rs`: 拡張子 → Lucide アイコン分類
  - `watcher.rs`: notify + debouncer、カレント+親の非再帰 watch、400ms
  - `main.rs`: ツールバー/サイドバー/パンくず/仮想リスト/ステータスバー、
    非同期ロード + 世代番号 + Task drop、キーバインド、右クリックメニュー、
    ゴミ箱（確認 prompt → background trash）、新規フォルダ、10s reconcile
- 15:16 初回ビルド: `on_click` は `Stateful<Div>` のみ（`id()` 必須）などの
  コンパイルエラー 7 件を修正 → ビルド成功、テスト 31 件作成
- 15:22 初回 Wayland 実行・KWin D-Bus・スクショ取得。**行の列が右端に伸びない**
  レイアウト不具合を発見（uniform_list の root item は幅 auto のため）
- 15:27 行に `w_full()` を追加 → 列がヘッダと整列。再ビルド・再実行
- 15:29 `/tmp/gpui-fe-demo`（ディレクトリ2 + ファイル10 + 隠し1）で
  アイコン・自然順・隠し件数・監視表示を確認、スクショ保存
- 15:31 起動中に bash から `from-bash-while-running.txt` 作成 →
  12→13 件に自動更新、行追加をスクショで確認（監視の実証）
- 15:34 `kill -TERM`、D-Bus からウィンドウ消滅を確認。portal スクショと demo ディレクトリを掃除
- 15:36– README.md / NOTICE.md / LICENSE 作成、README に検証記録と既知の制限を記載
- 15:40 起動時に `last_dir` を保存するよう変更 → 「引数あり起動→終了→引数なし再起動」で
  パンくずに保存先が復元されることを実行検証（`screenshot/settings-restore-crop.png`）。
  検証用設定ファイル・一時ディレクトリ・ポータルスクショを削除
- 15:43 最終 `cargo build`（警告0）/ `cargo test`（当時31 passed）を再確認
- 16:20–16:40 fe-verifier の独立検証（blocker 0 / major 3 / minor 13）を受け、major を修正:
  - **M-3**: コンテキストメニューの各行操作を「行 index 捕捉」から「`PathBuf` 捕捉 +
    クリック時パス再解決」に変更。対象が消えていれば何もせずステータス通知
  - **M-1**: サイドバーの場所（`dirs::*_dir()` + `is_dir()`）を起動時に `Vec<Place>` として
    キャッシュ。存在確認は reload 時のみで、render パスから同期 I/O を排除
  - **M-2**: 新規フォルダの連番プローブ + `create_dir` を `cx.background_spawn` へ移動
    （`model::create_unique_folder`、`AlreadyExists` はリトライ）。完了後に
    `pending_select` + reload（画面が別ディレクトリなら作成パスを通知のみ）
  - 文書修正: テスト数 36、AC-9（open/reveal 失敗時は成功文言のまま）を既知の制限に追記、
    設定破損時 `.broken-<epoch>` 退避を README に追記、検証後クリーンアップの記述を修正
- 16:35 時間内に直せる minor も追加対応: m-7（reload 成功時にステータス通知を消さない）、
  m-2（watcher の watch 失敗時に reload で再試行）、m-1（`DirWatcher::drop` の
  コメントと実装の不一致を修正）。m-4/m-5/m-9/m-12 は README「既知の制限」に追記。
  残りの minor（Degraded 状態、UI 層テスト、fsync、lossy 名の衝突など）は P1 候補。
- 16:37 最終確認: `cargo build` 警告 0 / `cargo test` 36 passed / 起動→KWin
  caption・resourceClass・1100x728・stderr 0 行 / 起動中に外部ファイル作成しても
  stderr 0 行 / SIGTERM でクリーン終了・ウィンドウ消滅。検証用設定・一時物は削除済み
- 15:45–16:05 DESIGN.md（fe-designer 並行執筆）と突き合わせ、差分を解消:
  - 設定スキーマを DESIGN §5.7 / §8.1 に一致（`version` / `sort{key,dir,dirs_first}` /
    `show_hidden` / `view`）。壊れた JSON は `settings.json.broken-<epoch>` に退避
  - キー追加: `PageUp`/`PageDown`（可視行数）、`Delete`（確認付きゴミ箱）、
    `Ctrl+Shift+N`（新規フォルダ）、`Ctrl+Q`（終了）
  - README に「設計からの差分」表を追加（モジュール構成・on_key_down 採用・
    テーマ色直書き・strings.rs 未作成など）。AC-5/AC-7 用に
    `screenshot/window.png` / `watcher-before.png` / `watcher-after.png` を追加
  - `cargo test` 35 passed（設定スキーマの JSON shape テスト + SortSpec テスト追加。M-1/2/3 修正後は 36）。
    実行して `settings.json` が DESIGN の JSON 形状で保存されることを確認（stderr 0 行、終了確認済み）

## 設計上の判断・フォールバック

1. **アイコン**: `gpui_kit::assets::Assets`（デフォルト約100種）では
   image/code/archive/audio/video が揃わないため `AllAssets`（Lucide 全種）を登録。
2. **右クリックメニュー**: 自作 overlay ではなく gpui-component の
   `ContextMenuExt` を採用（右クリック位置に表示、外側クリック/Esc で閉じる、
   フォーカス復帰を内蔵）。空き領域右クリックは未対応（仮想リストが全面を占有）。
3. **削除確認**: `Window::prompt`（Linux では GPUI 内蔵フォールバックダイアログ）を
   使用。`trash::delete` は background executor で実行。
4. **監視 → UI 通知**: watcher スレッドから直接 Entity を触れない（GPUI の Entity は
   スレッド非依存）ため、`async-channel` でフォアグラウンドの GPUI タスクへ通知。
5. **10秒 reconcile**: background で再 read_dir し fingerprint 比較、
   変化時のみ UI に反映（選択位置はパスで維持）。
6. **Test マクロ shadow 回避**: `#[cfg(test)] mod tests` では glob import を使わず
   明示 import（PoC の知見どおり）。全モジュールで実践。
7. **ビルド容量対策**: `[profile.dev] debug = 1` + 依存 `debug = 0` で
   debug バイナリ 745MiB → 165MiB に削減。

## 既知の制約（詳細は README）

- ネイティブ Wayland への合成入力手段が無い（kdotool/ydotool/wtype 未導入、
  xdotool は XWayland 専用）ため、キーボード・右クリック・確認ダイアログの
  実押下 E2E は未実施。コードはコンパイル検証 + データ層テストのみ。
- 空き領域右クリックメニュー、複数選択、グリッド、リネーム等は未実装。
- Windows/macOS ビルドは未検証（設計は `std::fs` / `dirs` / `trash` でポータブル）。

## 2026-11-17 追記（P1 完了・クロスレビュー対応）

- `cargo test`: **160 passed / 0 failed**。`cargo audit`: exit 0
  （allowed warnings 8 件、修正済み CVE なし）。
- P1 機能（grid / rename / copy-move / tabs / search / volumes /
  multi-select / preview / bookmark / undo ほか）は実装済み。
  上記「未着手」「未実装」の記述は 2026-11-11 時点の履歴。
- 全体クロスレビュー（[../review-report.md](../review-report.md)）の指摘を
  反映: 監視イベントの FsEvent 正規化と差分適用、設定スキーマ
  マイグレーション、内容検索の streaming 化、`FsError` /
  `LoadState` の型化、`OsString` / `sort_key` / `SearchResult`
  へのデータモデル整理、O_EXCL による上書き競合の解消など。
- UI 文言は `strings.rs` に段階的に集約済み。完全な `ui/*`
  モジュール分割とショートカットの Action 化は設計判断のため保留。
