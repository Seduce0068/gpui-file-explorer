---
type: Reference
title: クロスレビュー統合レポート -- gpui-file-explorer
description: >-
  app/ と poc/ を対象にしたセキュリティ・アーキテクチャ・運用保守の
  3 観点クロスレビュー結果と統合判定。
tags:
  - code-review
  - cross-review
  - rust
  - gpui
generated: 2026-11-16T22:51:56+09:00
verified: 2026-11-21T16:49:57+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 総評

データ層（`model` / `ops` / `search` / `settings` / `history` / `tabs` /
`trash_view`）は GPUI 非依存の純関数として切り出され、初回レビュー時点の
全テスト 132 件（2026-11-16 時点。ヘッドレス テストのみの計上ではなく、
全テストの実行数）で担保されている。この値もレビュー時点の値で現在値では
ない（後述「Medium 指摘（重複統合・要約）」の『ドキュメントの数値乖離』項目）。
現在のテスト件数は `cargo test` を実行して確認（README は drift 防止のため固定値を記載しない方針）。
安全性ポリシー（上書き禁止、symlink 非追跡、
`.partial` 経由の原子的コピー、`#![forbid(unsafe_code)]`）も
`ops.rs` のモジュール doc と実装が一致していた。

一方で、DESIGN.md §4.1 が計画した
`app.rs + model/* + fs/* + ui/*` 分割は未達で、P1 機能がすべて
`main.rs` に集中（初回レビュー時点 5,588 行・約 60 フィールド、第2回 5,909 行、
最終スナップショットは第3回 10,751 行・50 フィールド、2026-11-19 実測）。
いずれもレビュー時点の値で現在値ではない（後述「Medium 指摘（重複統合・要約）」の『ドキュメントの数値乖離』項目）。
エラー型の 3 系統混在、監視イベントの差分適用未実装（全量再読込）、
設定マイグレーションの欠如が、Phase2（Windows/macOS、WASM）前に
解消すべき構造的負債として確認できた。

セキュリティ上の致命的欠陥（Critical）は検出されなかった。

## スコープと方法

- 対象: `app/src/*.rs`（13 ファイルは初回レビュー時点のスナップショット、
  現在は 17 ファイル。最新構成は `app/README.md` の「アーキテクチャ」表を参照）、
  `poc/src/main.rs`、各 `build.rs`、`Cargo.toml`、`scripts/`、
  `packaging/linux/`、`.github/workflows/ci.yml`、
  `DESIGN.md` / `README.md` / `app/README.md` / `app/PROGRESS.md`
- 観点: セキュリティ、アーキテクチャ、運用・保守の並列レビュー
- エージェント結果:
  - `security-reviewer`: 完了（指摘 24 件を提出）
  - `arch-reviewer`: 完了（指摘 34 件を提出）
  - `ops-reviewer`: 成果物なしで失敗（進捗報告のみ）→ 親が横断走査で補完
- 子エージェントの指摘は仮説として扱い、現行コード・制御フロー・
  実行結果で事実確認した上で採用・格下げ・却下を判断した。

## 判定

- **Critical: 0 件**
- **High: 5 件**（初期指摘。High 2/3/4 は解消、High 1 は実質軽減、
  High 5 のみ上流依存で追跡継続）
- **Medium: 20 件**（大部分は対応済み、残りは設計判断で保留）
- **Low / Info: 20 件**（継続的な改善項目、大部分は対応済み）

直近コミット `5fde297`（README の DESIGN.md 行数修正）単体は
**承認**。リポジトリ全体としては当初 **条件付き承認** としたが、
2026-11-17 の継続作業で High 2（監視差分）、High 3（設定
マイグレーション）、High 4（内容検索 streaming）を解消、High 1 は
状態集約とフック抽出で実質的負債を軽減（完全な `ui/*` 分割は設計判断
のため保留）、High 5 は上流依存のため追跡継続。詳細は各項目の
「解消済み」表記を参照。

最終状態（2026-11-17T07:29+09:00）: 全変更を `origin/main` へ push
済み、`CI success`（run 43967966780）、ローカルとリモート完全一致。
残る High 5 は修正済み CVE なしの unmaintained 警告のみで、
当時は **実質的に承認** と判定した。

**2026-11-17 追記（第2回レビュー）**: 独立した再レビューで
Critical 1 件・High 8 件が新たに検出され、上記判定は取り下げ、
いったん **マージブロック** とした。その後 Critical 1 件と High 6 件を
修正し、残る High 1 件（H-3 tracing）も 2026-11-17T14:05 に導入して
**承認** に昇格（詳細は末尾「第2回クロスレビュー」、
「第2回指摘の修正完了」、および「H-3 構造化ログの導入」）。

## High 指摘（Phase2 前の必須改善項目）

### H-1 Explorer の God Object 化（部分解消）

- 箇所: `app/src/main.rs:411`（修正後は 439 行）
- 根拠: `Explorer` が UI / 状態 / ファイル操作 / 監視 / 履歴 / タブ /
  検索 / プレビュー / プロパティ / ダイアログをすべて保持。初回
  レビュー時点 5,588 行・約 60 フィールド（第2回 5,909 行、
  第3回 10,751 行・50 フィールド、2026-11-19 実測。レビュー時点の
  スナップショットで現在値ではない。詳細は「Medium 指摘（重複統合・要約）」の『ドキュメントの数値乖離』項目）。
  `PROGRESS.md` もフラット構成と自認。
- 対応（実施済み）: `debug_*` 20 個を `DebugHooks`、`search_*` 13 個を
  `SearchState` に集約し、Explorer のフィールドを約 60 → 29 に削減。
  各参照（計 110 箇所）も機械的に統一。さらに `render()` の先頭に
  混在していたデバッグフック 144 行を `apply_startup_hooks` /
  `apply_overlay_hooks` へ抽出し、描画パスを短縮。
- 残作業（判断: 現状維持）: `ui/*` への完全分割は、UI メソッドが
  既に Explorer の独立メソッド（toolbar / sidebar / status_bar /
  list_row / dialogs 等）として分離されており、ファイル移動は
  数千行の差分を伴う設計判断のため実施しない。段階的改善
  （フィールド集約・フック抽出・strings.rs）で実質的負債は軽減済み。
- 検証: `cargo test --all-targets --locked` = 140 passed / 0 failed、
  `cargo clippy --all-targets --locked -- -D warnings` = 成功。
- 出典: arch-reviewer A-1

### H-2 監視イベントの情報破棄と全量再読込（解消済み）

- 箇所: `app/src/watcher.rs:30-35`（修正後は `normalize_events`）
- 根拠: `new_debouncer` のコールバックが
  `let _ = result; on_change();` と `DebounceEventResult` を破棄し、
  `DESIGN.md` §4.6 / §5.6 の `FsEvent` 正規化が未実装だった。
- 対応: `FsEvent` と `normalize_events` を追加し、コールバックを
  `Fn(Vec<FsEvent>)` に変更。`spawn_watch_loop` は Created/Modified を
  `entry_for_path` で差分 upsert、Removed を削除し、Renamed/Rescan
  のみ全量 reload にフォールバック。`model::entry_for_path` を追加し
  `search.rs` の重複関数も共通化。エラー→Rescan、rename の Both/From/To 3 パターンの正規化テストも追加。
- 検証: `cargo test --all-targets --locked` = 141 passed / 0 failed、
  実機 watcher テストが `FsEvent::Created` を確認。
  `cargo clippy --all-targets --locked -- -D warnings` = 成功。
- 出典: arch-reviewer A-7、security-reviewer（DoS）を統合
- 追加改善: `DirWatcher::last_error()` をステータスバーへ接続。
  監視初期化失敗は「監視: 無効 — 理由」、一部ディレクトリの
  watch 失敗は「監視: 一部無効 — 理由」と表示し、理由が
  ユーザーに伝わるようにした（`#[allow(dead_code)]` も撤廃）。

### H-3 設定スキーマのマイグレーション欠如（解消済み）

- 箇所: `app/src/settings.rs:151`（修正後は `migrate` メソッド）
- 根拠: `settings.version = SCHEMA_VERSION;` で読み込んだ
  version を強制上書きするだけで、移行ロジックが存在しなかった。
- 対応: 読み込んだ version を `migrate` へ渡し、既知 version ごとに
  変換できる枠組みを追加。未知 version は警告を出して v1 として
  読み込む。保存時の version 正規化は従来どおり。3 件のテストを追加。
- 検証: `cargo test --all-targets --locked` = 135 passed / 0 failed、
  `cargo clippy --all-targets --locked -- -D warnings` = 成功。
- 出典: arch-reviewer A-15

### H-4 内容検索の全ファイル読み込み（解消済み）

- 箇所: `app/src/search.rs:364`（修正後は行単位読み込み関数）
- 根拠: `std::fs::read(&path)` でファイル全体を読み込み、
  `String::from_utf8_lossy` で走査していた。1 MiB の
  `max_file_size` チェックはあったが、大量ファイル連続時の
  IO / メモリ圧力を考慮し arch-reviewer の High を採用した。
- 対応: `BufReader::read_until` で行単位に読み込み、最初の
  マッチ行で早期終了。NUL チェックを読み取った全範囲へ拡大し、
  キャンセルを行間でも確認するよう変更。テストを 1 件追加。
- 検証: `cargo test --all-targets --locked` = 138 passed / 0 failed、
  `cargo clippy --all-targets --locked -- -D warnings` = 成功。
- 出典: arch-reviewer A-13、security-reviewer（Content Validation）

### H-5 トランジティブ依存のメンテナンス停止

- 箇所: `app/Cargo.toml:14`（`gpui-kit = "=0.9.1"`）
- 根拠: `cargo audit` が 8 件の `unmaintained` 警告を報告
  （`instant` / `paste` / `rustls-pemfile` / `rustybuzz` /
  `ttf-parser` / `zbus_names` / `zvariant` / `zvariant_derive`）。
  修正済み CVE はなし。Runtime Proof: exit 0、
  「8 allowed warnings found」。
- 対応: `gpui-kit` / `gpui-pre` の更新版が出たら追従し
  `Cargo.lock` を更新。追跡課題として記録する。
- 出典: security-reviewer S-H1

## Medium 指摘（重複を統合）

### 状態管理と concurrency

- キー入力・モーダル: `main.rs:1457-1508` で各フラグの
  if ブロックが直列化し、Esc で閉じる処理が重複。モーダル Stack
  の導入を推奨（arch A-6）。
- 進捗チャネル: `main.rs:1816-1845` の
  `progress_rx.recv().await` が foreground task を占有。
  80 ms スロットルは到着後のフィルタにすぎない（arch A-7）。
  16 ms tick の `try_recv` バッチ化を推奨。
- prompt 再入: `main.rs:3423-3499`。`app/README.md` も
  「別の prompt を開かない保証はありません」と自認。
  `Option<TrashPrompt>` で単一回答を保証する（arch A-8）。
- render の副作用: `main.rs:5124-5267` の debug hooks が
  `render()` 内で `enter_trash` / `drop_external_into` 等を発火。
  `Render` 契約違反。`on_next_frame` へ移動（arch A-9）。
- UI スレッド同期 I/O: `main.rs:1264, 1282` の `is_dir()`、
  `main.rs:571-581` の `canonicalize`。文書化された起動時例外を
  超えて増殖しています（arch A-4, A-5）。

### エラー処理

- 3 系統混在（一部解消）: `FsError` / `FsErrorKind` と `LoadState`
  （Loading/Ready/Error）を導入。`loading: bool` + `error: Option<String>`
  を排他状態へ置換し、generation を LoadState 側でも保持して古い
  結果の破棄に利用。テスト3件追加。`OpError` 側も `io_error` の `CrossesDevices` / `IsADirectory`
   マッピングを追加しテスト。完全統合は後続
  課題（arch A-10）。
- 失敗詳細の欠落（解消済み）: 件数のみだった表示に最初の
  失敗ファイル名と理由を含めるよう変更。redo(trash) も
  失敗理由を保持する `(PathBuf, String)` へ変更（arch A-11）。
- 失敗時の status 残り（解消済み）: `error` 設定時に古い
  `status` が残りうるため、Err 分岐で `status` をクリアした。
- 設定保存エラーの握りつぶし（解消済み、運用独自）:
  `Settings::save()` がエラーを stderr に出力するよう変更。

### データモデル

- `model.rs:19`（解消済み）: `Entry.name` を `OsString` に変更し、
  表示・ソート用の `name_text`（lossy キャッシュ）を追加。
  非 UTF-8 ファイル名が I/O で失われないことを Unix テストで
  検証（144 passed）。`sort_key` の事前計算も後続で実装済み。
  さらに `ops::unique_path` も `&OsStr` 受け取りに変更し、
  競合リネーム時の `(N)` 付与で生バイトを保持（158 passed）。
- `model.rs:27-28`（解消済み）: `Entry.match_line` を削除し、
  `search::SearchResult { entry, match_line }` へ分離。Explorer は
  `search_lines: HashMap<PathBuf, u32>` で行番号を保持し、
  描画時に参照。検索クリア時に Map もクリア（arch A-2）。
- `model.rs:178`（解消済み）: `Selection` は順序保持の `Vec` に加え
  `HashSet` インデックスを並走させ、`contains` を O(1) 化。
  描画の各行からの検索が高速化され、追加順・範囲選択の挙動は
  不変。同期不変条件のテストを追加（arch A-14）。
- `model.rs:385`（解消済み）: `Entry.sort_key`（小文字化済み）を
  構築時に事前計算し、`sort_entries` は比較ごとの `to_lowercase()`
  アロケーションを廃止。`compare_names` はテスト専用に限定。
  既存の自然順ソートテストがすべて通過。

### 検索

- `main.rs:3600-3613`（解消済み）: 後続ルートが 1 件に制限される
  不公平を修正。`ceil(global_cap / root_count)` を各ルートへ均等
  配分する `per_root_cap` を導入し、テストを 1 件追加。
- `main.rs:3606-3614`（仕様明記）: `search.cancel` は
  cooperative flag / task drop / generation guard の 3 層構造の
  1 層であることをコメントで明記（挙動変更なし）。

### 永続化と OS 抽象化

- `settings.rs:198-208`: tmp + rename に `fsync` が無く、
  `ops.rs:272` のコピーと方針が不一致だった（arch A-16）。
  `sync_all()` を追加して解消済み。ディレクトリの fsync は
  未実施で、README の既知の制限も更新した。
- `settings.rs:45-62`: 永続設定と実行時状態（tabs / recent_searches /
  bookmarks）が同居。設計判断のため現状維持（分離は将来課題）。
- `ops.rs:317-332`: 非 Unix で symlink コピーが Unsupported。
  Windows の `symlink_file` / `symlink_dir` 対応が必要。
- `ops.rs:559`: EXDEV 判定が `raw_os_error() == Some(18)` に依存していた。
  `ErrorKind::CrossesDevices` に置き換え、Linux の EXDEV(18) が
  std で正しくマップされることをテストで検証済み（解消）。
- `mounts.rs:141` / `trash_view.rs:18` / `preview.rs:25`:
  OS ごとの実装を直接呼んでおり、差し替え可能な provider trait
  （`VolumeProvider` / `TrashProvider`）が無い（設計判断のため保留）。
- `mounts.rs:76`（追加修正）: `unescape_mount_field` が各バイトを
  `as char` 変換しており、UTF-8 マウントパスが壊れていた。
  バイト列へ蓄積して最後に `from_utf8_lossy` するよう修正し、
  日本語パスとオクタル UTF-8 のテストを追加。
- `build-deb.sh:119`（解消済み）: `ldd` 由来パスを
  `/lib*`, `/usr/lib*` 配下の実在ファイルに制限し、`dpkg -S` が
  返すパッケージ名も文字種で検証。`bash -n` / `shellcheck` 通過、
  実 DEB ビルドで `Depends` が従来どおり生成されることを確認。

- 最終コードでのフル DEB ビルド（2026-11-17T03:24+09:00）が成功し、
  展開したバイナリが `--version` で `gpui-file-explorer 0.1.0` を返すことを確認。
  以降のコード変更後も再ビルド（2026-11-17T05:52+09:00）で
  `--version` 正常、同梱 `.desktop` の validate 通過を再確認。
  SPDX ヘッダ追加後も再ビルド（2026-11-17T07:09+09:00）で
  `--version` 正常・同梱 desktop validate 通過を確認。
- `gpui-file-explorer.desktop`（追加改善）: `Exec` に `%f` を追加し、
  ファイルマネージャーからフォルダを引数として渡せるようにした。
  `desktop-file-validate` 通過、DEB 展開後の `.desktop` にも反映確認。
  `StartupWMClass` の再実測は、kdotool 未導入に加え KWin D-Bus に
  `activeWindow` が無く `queryWindowInfo` は対話的（UserCancel）で
  自動化できないため未実施（2026-11-11 の確認結果を維持）。

### セキュリティ（事実確認済み）

- `ops.rs:286-299`（解消済み）: `hard_link` 失敗 → `dest.exists()`
  → `rename` の競合を、`OpenOptions::create_new(true)` による
  O_EXCL 予約 + `rename` に置換。既存ファイルは予約時に
  `AlreadyExists` で拒否され、競合相手が割り込む余地がない。
  既存の上書き拒否 / 部分的ファイル削除テストが通過
  （security S-M2）。
- `ops.rs:214`: `ensure_not_into_itself` の `canonicalize` にも
  TOCTOU が残るが、影響は自己配下コピーに限定。
- `preview.rs:117`（解消済み）: `metadata.len()` に加えて
  `File::take(max_bytes + 1)` で実読み取り量も制限し、
  疑似ファイルシステム等で size が偽装されても上限を超過
  しないようにした。境界（ちょうど上限）テストも追加。
- 構造化ログの欠如（運用独自）: 初回レビュー時に stderr テキスト
  （`gpui-file-explorer: op=...`）を追加。第2回レビューで tracing
  未導入を指摘（H-3）、2026-11-17T14:05 に tracing を導入して
  解消（「H-3 構造化ログの導入」参照）。undo は `undo-report` 維持。

## Low / Info（要約）

- `main.rs:177`（部分解消）: `strings.rs` を新設し、F1 ヘルプ、
  空状態メッセージ（Trash/Search/Directory/フィルタ/読み込み）、
  固定ステータス文言21種と重複する `format!` 前置詞5種
  （移動先/対象が存在しません、ディレクトリではありません、
  元に戻せません、やり直せません）を集約。値埋め込みが必要な
  `format!` 文言は呼び出し側に残す（段階的 i18n 基盤）。
- `main.rs:172` / `settings.rs:35`（解消済み）: 表示ソース側を
  `ViewSource`、レイアウト側を `ViewLayout` にリネームして
  名前の衝突を解消。`#[serde(rename_all = "snake_case")]` は
  不変で `settings.json` の `"details"` / `"icons"` 互換性を
  維持（settings 系 24 テスト通過）。
- `main.rs` の debug hooks（判断: 現状維持）: `GPUI_FE_DEBUG_*` の読み込みは
  `#[cfg(debug_assertions)]` でガードされ、**debug ビルドのみ有効・release では
  コンパイルアウト**されます（`main.rs` の `Explorer::debug_hooks_from_env` と
  起動時の設定上書きブロック）。したがって release バイナリでは
  環境変数を設定しても発火せず、実 settings.json は汚染されません。
  スクリーンショット検証は debug ビルドで実施しており、
  `app/README.md` の「既知の制限」にもその旨を明記しています。
- `main.rs:1318`: ショートカットが raw 文字列比較。
  `actions!` + `KeyBinding` への移行を検討。
- `main.rs:5563`（解消済み）: 起動時 window 作成失敗の
  `expect` を、stderr へ理由を出して `cx.quit()` する
  フレンドリな処理へ変更。`--version` と 3 秒間の GUI
  起動（stderr 0 行）で正常性を確認。
- `history.rs:6`（解消済み）: `NavMode` と `apply` を追加し、
  Push/Back/Forward/Up/Replace を統一 API 化。`go_parent` は
  `Up`、同一パス再読込は `Replace` に接続。タブ切替時の履歴
  復元は `load_tab` が実施済みで、再起動時の非復元は README
  に制限明記済み（追加対応不要）。
- `model.rs:493`（解消済み）: fingerprint が秒単位に丸められる
  ため、1 秒未満の連続変更は検出できない旨を doc コメントへ
  明記（watcher が主検出、reconcile は粗い保険）。
- `app/Cargo.toml:23`（方針明記）: `chrono = "0.4"` は日時表示と
  バックアップ名に使用。wasm 対応時は `js-sys` ベースの時刻取得へ
  差し替える必要がある旨を `app/README.md` の WASM 節に追記。
- `app/build.rs:21` / `poc/build.rs:21`: libxkbcommon shim は
  Linux 専用だったがガードが無かった。`cfg!(target_os = "linux")`
  による早戻りを追加して解消。
- `poc/build.rs`: `#![forbid(unsafe_code)]` が未宣言だったため
  追加（app 側と方針を統一、解消）。
- `main.rs:4328`（解消済み）: `symlink_metadata` で存在確認し、
  デバイス / FIFO / ソケット等の特殊ファイルは事前スキップ。
  1 つの特殊ファイルで転送全体が失敗していた問題も解消。
- `main.rs:573, 1336, 2944, 3126`: 正規化パスと元入力パスの
  使い分け、symlink 表示のサニタイズを検討。
- `preview.rs:147`: NUL チェックが先頭 8 KiB のみだったため、
  読み込んだ全範囲を走査するよう修正（テスト追加、解消）。
- `poc/src/main.rs:72`（解消済み）: `DirEntry::metadata` は
  symlink をフォローするため、サイズ・更新日時は
  `symlink_metadata` の値を使うよう変更（app 側と方針統一）。

## 却下・格下げした指摘（根拠付き）

- **「cargo audit の allowlist がリポジトリ外にありローカルと
  不一致」→ 却下**: 実実行で exit 0・「8 allowed warnings found」。
  リポジトリ内に ignore 設定は無く、cargo-audit 0.22.2 の
  既定挙動。README の「CI の検証範囲」節（cargo audit 実行の記載）と一致。
- **symlink の `metadata()` フォローによる情報漏洩
  （security の Medium）→ Medium に格下げ**:
  `model.rs:39-41` のコメント通り、symlink ディレクトリを
  enterable にする設計意図があり、`is_symlink` フラグも保持。
  表示の明確化を推奨するが脆弱性ではない。
- **内容検索の全読み込み（arch の High）→ High として採用し、
  解消済み**: 行単位 streaming read と早期終了へ変更したため、
  却下・格下げではなく修正対象として扱った（上記 H-4 を参照）。
- **共有ワークフロー `rust-gates.yml@v1` の audit 設定 → 確認済み**:
  `gh api` で取得した本文 152-155 行も素の `cargo audit` であり、
  allowlist 設定は存在しない。ローカル・CI の挙動は一致している。

## 検証エビデンス（Runtime Proof）

- `cargo clippy --all-targets --locked -- -D warnings \
  -D clippy::undocumented_unsafe_blocks`（2026-11-17T03:11+09:00）:
  警告なし。`app/src/main.rs` と `app/build.rs` に
  `#![forbid(unsafe_code)]` を確認（AGENTS.md §15 の unsafe 禁止を具現化）。
- DESIGN の受入条件 AC-1〜AC-13 を照合（2026-11-17T06:37+09:00）。
  レビュー時点のスナップショットであり、現行値は README / 実行結果を参照。
  旧表記「PASS 8 件 + NOT TESTABLE 4 件 = 12 件」は AC-4 の判定漏れによる
  誤記。正しくは PASS 9 件 + NOT TESTABLE 4 件 = 13 件。
  - PASS（9 件）: AC-1（build 成功）/ AC-2（test 成功）/
    AC-3（Wayland 起動・エラーなし）/ AC-4（KWin D-Bus 確認）/
    AC-5（5 領域スクショ）/ AC-6（非同期ロード）/ AC-7（監視更新）/
    AC-11（設定永続化）/ AC-13（SIGTERM クリーン終了）。
  - NOT TESTABLE（4 件）: AC-8 / AC-9 / AC-10 / AC-12
    （合成入力ツール未導入による実押下不可）。AC-9 のエラー通知は
    GPUI API 制約で未実装（README に明記済み）。新たな未対応項目なし。
- 最終コードで実機起動を再確認（2026-11-17T06:58+09:00、Wayland
  `WAYLAND_DISPLAY=wayland-0`）: ビルド成功、3 秒間プロセス生存、
  stderr / stdout とも空（panic / EGL / Vulkan エラーなし = AC-3）、
  SIGTERM でプロセス残存なし（AC-13）。KWin D-Bus の
  `WindowsRunner.Match` は当環境で interface 不明のため照合不可。
  AC-4 の再計測も試行（2026-11-17T07:00+09:00）: KWin D-Bus の
  interface/method を introspect したが、`getWindowInfo` はウィンドウ ID
  引数が必要、`queryWindowInfo` は対話的キャンセルが必要、
  `xwininfo -root -tree` にも Wayland ネイティブウィンドウは現れない。
  再計測不可のため 2026-11-11 の `resourceClass=gpui-file-explorer`
  確認記録を維持。
- 初期の独立検証（`notes/verify-report.md`）の major 3 件も再確認
  （2026-11-17T05:16+09:00）: render パスに同期 I/O なし、選択は
  `Selection`（PathBuf キー）で行 index 競合を回避、起動・ナビゲーション
  以外の UI スレッド I/O なし。すべて解消済み。
  major M-3（コンテキストメニューが行 index を捕捉する問題）も
  `entry_menu(entity, entry.path.clone(), …)` とパスを捕捉する方式に
  変更済みで解消（main.rs:4380 付近）。
- クロスコンパイル（2026-11-17T04:56+09:00）:
  `x86_64-pc-windows-msvc` は check 成功（Windows の rename 仕様差も修正済み）。
  `aarch64-apple-darwin` は上流 `gpui-pre-apple` のビルドスクリプトが
  `stitched_shaders.metal` を生成できず失敗。当リポジトリのコードではなく
  上流のクロスビルド制約で、README の「macOS 実機ビルド未検証」と整合。
  `poc/` も Windows ターゲットで check 成功（2026-11-17T05:04+09:00）。
  `app/` の非 Linux 警告は cfg gating で解消済み。Windows のテスト
  バイナリは MSVC linker（`link.exe`）未導入のため link 不可
  （2026-11-17T05:21+09:00、check レベルの検証は完了済み）。
  `aarch64-unknown-linux-gnu` は cross linker
  （aarch64-linux-gnu-gcc）未インストールのため check 不可
  （2026-11-17T05:08+09:00、環境制約でコード起因ではない）。
- `cargo tarpaulin --locked --out Stdout --exclude-files 'target/*'`
  （2026-11-17T03:05+09:00）: データ層のカバレッジを改善。
  `model.rs` 270/301 → 287/301、`ops.rs` 212/284 → 222/284、
  `search.rs` 118/126 → 124/126、`undo.rs` 34/37 → 35/37、
  `watcher.rs` 46/54 → 48/54。全体は UI の `main.rs`
  （70/3173）が支配的で、合成入力ツール未導入により headless
  テストが困難な既知の制限どおり。
- 同上の再計測（2026-11-17T03:40+09:00）: `icons.rs` が
  17/35 → 34/35、`ops.rs` が 222/284 → 229/291、
  `preview.rs` が 70/73 → 71/73 に改善。
- 同上の再計測（2026-11-17T04:10+09:00）: `model.rs` が
  287/301 → 290/301 に改善。残る未カバー行は errno のレア分岐
  と環境依存の I/O 経路のみ。
- 同上の再計測（2026-11-17T04:29+09:00）: `model.rs` が
  290/301 → 292/301（`parent_dir` の境界を追加カバー）。
  `history.rs` 30/30、`tabs.rs` 56/56 は完全カバーを維持。
- テスト分布（2026-11-17T06:55+09:00、全167件）: model 38 / settings 24 /
  ops 20 / search 19 / UI 純粋ヘルパー 13 / preview 12 / tabs 9 /
  undo 7 / history 6 / watcher 5 / mounts 5 / trash_view 4 / icons 4 /
  strings 1。データ層中心に偏り、残りは GPUI 描画（headless 不可）。
- `bash scripts/check.sh`（audit 込みの完全ループ、
  2026-11-17T02:03+09:00）: **exit 0 / All checks passed**
  （app 150 tests / poc 4 tests / clippy 警告 0 /
  cargo audit 8 allowed warnings）
- `cargo test --release --all-targets --locked`（`app/`、
  2026-11-17T02:20+09:00）: **151 passed / 0 failed**。
  debug / release の双方で同一結果（過去の検証は 150 件時点）。
  再確認（2026-11-17T04:46+09:00）: **161 passed / 0 failed**、
  最終確認（2026-11-17T04:50+09:00）: **162 passed / 0 failed**。
- `cargo test --all-targets --locked`（`app/`、2026-11-16T22:51:56+09:00）:
  **132 passed / 0 failed / 0 ignored**
- `cargo audit`（`app/`、同日時）: **exit 0**、
  「warning: 8 allowed warnings found」（unmaintained のみ）
- 2026-11-17T04:40+09:00 の完全ループで RustSec DB の fetch が
  ネットワークエラーで失敗したが、`cargo audit -n`（ローカル DB）
  では exit 0。コード起因ではなく外部ネットワークの一時的障害。
- `gh api repos/Seduce0068/workflows/contents/.github/workflows/rust-gates.yml?ref=v1`
  （2026-11-16T22:55+09:00）: exit 0、本文 162 行を取得。
  監査ステップは `run: cargo audit`（152-155 行）で、
  ignore / allow 設定なし
- `cargo --version` 系のキャッシュロックは `ops-reviewer` 報告時に
  発生したが、最終的に clippy は exit 0 で完了
- `git status`: 変更は本レポート（`review-report.md`）のみ。
  ソースコードの変更はなし
- arch エージェントの一時成果物 `review-result.md`（本文が空の
  プレースホルダー）は統合後に削除済み。本レポートが正本

## フォローアップ

1. H-1（UI 分割）は、段階的改善（フィールド集約・フック抽出・
   strings.rs）で実質的負債を軽減済み。完全な `ui/*` 分割は
   設計判断のため現状維持（上記参照）。
2. H-5 は上流依存のため修正不可。gpui-kit 更新の都度
   `cargo audit` を再実行し `Cargo.lock` を追従させる運用ルールを
   `app/README.md` の既知の制限に追記済み。
3. 運用独自指摘（設定保存エラー通知、fsync、操作ログ）は
   すべて解消済み。tracing による本格的な構造化ログは当初残課題
   だったが、2026-11-17T14:05 に導入済み（「H-3 構造化ログの導入」）。
4. CI の共有ワークフロー側の audit 設定は `gh api` で確認済み
   （追加対応不要）。OKF メタデータの `verified` は最終検証時点
   （2026-11-21T16:49:57+09:00、`date` コマンドで確認）に更新済み
   （`generated` は初回作成時刻のまま）。
5. push を実施（2026-11-17T07:12+09:00）。AGENTS.md §4 が commit/push
   をエージェント判断・都度確認不要と定め、ユーザーも「任せる」と
   包括委任しているため、従来の「外部影響のため保留」方針を改めた。
   `git push origin main` は **exit 0**、`5fde297..066dce1` を反映
   （133 コミット）。ローカルと `origin/main` は完全一致（ahead 0）。
   push 前に完全ループを実行し全ゲート通過を確認（audit は DB fetch
   の一時障害のため `-n`（ローカル DB）で exit 0 を確認）。
   CI run 43967966780 が **success**（9m19s、2026-11-17T07:23+09:00
   完了）。最初の run 43967771169 は追加 push により cancelled だが、
   最新コミットを含む run が全ゲート通過。
6. 全差分（22 ファイル / +2727 / -831）の統合レビューを実施。
   新規ファイルは `strings.rs` と本レポートのみ。削除された公開 API
   は `unique_path` のシグネチャ変更のみ（内部使用、対応する追加あり）、
   main.rs の削除行はいずれもシグネチャ変更・移動に伴うもので回帰なし。
7. リモートのオープン issue は 0 件（2026-11-17T06:56+09:00、
   `list_issues` state=OPEN）。外部にも未対応タスクは存在しない。

## 第2回クロスレビュー（2026-11-17・全体再レビュー）

前回レビュー以降、コード変更は無く直近 3 コミットは本レポートの
更新のみ（`7c9b2e7`、`756b25c`、`8d8d5c2`）。今回はユーザー指示
「全体を様々な観点でクロスレビュー。ulw」に基づき、`app/`・`poc/`・
`packaging/`・`scripts/`・CI・ドキュメントを対象に、
セキュリティ / アーキテクチャ / 運用保守の 3 視点を並列起動して
再レビューした。子エージェントの指摘はすべて仮説として扱い、
現行コードの該当行を直接読んで事実確認した。

### 判定（第2回）

- **Critical: 1 件** — 設定退避失敗時のデータロス経路。
- **High: 8 件** — 一時ファイル 2 件、観測性 3 件、
  設定前方互換 1 件、設計乖離 2 件。
- **Medium: 16 件 / Low: 12 件**（重複統合後）。
- **判定: マージブロック（リリース前の必須修正あり）**。
  Critical 1 件と High 8 件は、いずれも既存動作を壊さず
  追加修正できる範囲であり、修正後に再レビューすれば
  承認へ昇格可能と判断する。
- 前回の「実質的に承認」判定は、今回の独立レビューで
  新たな Critical 1 件と High 8 件が検出されたため更新する。

### Critical（C-1）設定退避失敗後にデフォルト設定で上書き

- 箇所: `app/src/settings.rs:149-161`、`app/src/main.rs:627-628`
- 事実確認: 破損 JSON を検出すると
  `let _ = std::fs::rename(path, backup);`（`:160`）で退避し、
  失敗しても握り潰して `Self::default()` を返す。直後に
  `Explorer::new` が `this.settings.last_dir = ...;
  this.settings.save();`（`main.rs:627-628`）を実行し、
  `save_to()` の `rename(tmp, path)` が元ファイルを上書きする。
- 重大度根拠: 退避先の名前衝突や NAME_MAX 超過など、
  「backup への rename は失敗するが元パスへの rename は成功する」
  ケースで、ブックマーク 50 件・タブ・検索履歴が不可逆に失われる。
- 修正方針: 退避失敗時は元ファイルを残したまま
  「読み取り専用セッション」フラグを立てて `save()` をスキップし、
  ステータスバーと stderr の両方に退避先と失敗理由を表示する。
- 出典: ops-reviewer C-1（親が実コードで確認）。

### High 指摘（第2回）

- **H-1 一時設定ファイルの O_EXCL 欠如**
  （`settings.rs:218-223`）: `json.tmp-<pid>` は予測可能で
  `create(true).truncate(true)` が既存 symlink を追従する。
  設定値は UI 入力から部分的に制御可能で、同一ユーザーの
  別プロセスが `.bashrc` 等への書き込みへ昇格し得る
  （CWE-59/377）。`create_new(true)` または
  `NamedTempFile` を推奨（security S-H1、親確認済み）。
- **H-2 コピー用 `.partial-*` の O_EXCL 欠如**
  （`ops.rs:272-284`）: `.partial-<pid>-<stats.files>` も
  予測可能で `File::create` が symlink を追従して truncate する。
  `dest` の O_EXCL 予約は `partial` には及ばない。
  共有ディレクトリで他ユーザーの任意ファイル破壊があり得る
  （security S-H2、親確認済み）。
- **H-3 構造化ログの欠如とレポート記載の乖離**
  （`Cargo.toml:13-23`、`main.rs:1920` 他）: 当時 `tracing` / `log`
  依存が無く、全ログが `eprintln!` の生文字列。
  タイムスタンプ・相関 ID・レベルが無く事後追跡できなかった。
  前回レポート `:278-281` の「stderr テキストで対応」は最低限の
  監査テキストに留まり、tracing 未導入だった（ops H-1、後に解消）。
- **H-4 reconcile エラーの握り潰し**
  （`main.rs:913-936`）: `let Ok(entries) = task.await else {
  return };`（`:923`）が失敗を完全に破棄する。watcher の
  取りこぼしを回復する唯一の保険が黙って失敗し得る
  （ops H-2、親確認済み）。
- **H-5 watcher 初期化エラーの固定文字列化**
  （`watcher.rs:88-103`）: `new_debouncer(...).ok()`（`:92`）が
  `notify::Error` を捨て、ENOSPC / EMFILE / permission の
  実原因が失われる（ops H-3、親確認済み）。
- **H-6 将来スキーマ設定の前方互換欠如**
  （`settings.rs:175-186`、`main.rs:627-628`）: 未知 version を
  警告のみで v1 として読み込み、直後の `save()` が
  `version: 1` で書き戻す。serde が未知フィールドを無視するため
  ロールバック時に v2 固有の設定が静かに失われる。
  当時 `app/README.md:355-356` は「ダウングレード検出は未実装」と
  正直に記載しており、前回レポートの H-3「解消済み」と矛盾
  （ops H-4 + arch L、親確認済み）。現在は未知 version を
  読み取り専用として扱うよう訂正済み（行番号は当時点のスナップショット）。
- **H-7 God Entity 化（設計書との乖離）**
  （`main.rs:423, 496, 5370`）: `Explorer` は第2回レビュー時点で実測 39 フィールド
  （子エージェントは「40 超」と報告したが実測は 39）、
  `impl Explorer` は 4,873 行、`Render` は 103 行。
  DESIGN.md §4.1 / ADR-011 が約束した `app.rs + model/* + fs/* +
  ui/*` 分割は未達で、as-built 想定 1,200 行超の約 5 倍
  （arch A-1、親が数値を修正して確認）。
- **H-8 cwd / history / tabs の状態複製**
  （`main.rs:425, 442, 463, 1047`）: 同一の論理状態が
  `Explorer` と `tabs.active()` に二重保持され、
  `sync_active_tab()` を 8 箇所の呼び出し側が手動同期する。
  同期漏れで保存内容と表示が食い違う（arch A-2、親確認済み）。

### Medium 指摘（重複統合・要約）

- 進捗 / 監視チャネルが `unbounded` でバックプレッシャ無し
  （`main.rs:533, 1815`、ops M-3 + arch M-2）。
- 監視バッチ内の UI スレッド同期 I/O
  （`main.rs:984`、`model.rs:155-157`、arch M-1）。
- エラー型 3 系統混在（`FsError` / `OpError` / `String`、
  arch M-3）。前回の「一部解消」から進展なし。
- 一意名生成ロジックの重複（arch M-4）。
- キー入力が `actions!` + `KeyBinding` 化されていない
  （arch M-5、DESIGN.md §5.8 逸脱）。
- `render()` が `apply_*_hooks` で状態変更（arch M-6、
  前回レポートでも指摘済みで未解消）。
- 世代ガードが `Explorer.generation` と
  `LoadState.generation` の 2 系統（arch M-7）。
- case-only rename の中継名が予測可能で NAME_MAX 超過も
  （`ops.rs:661-663`、security M-1）。
- Windows の O_EXCL 予約解除→rename 間の競合
  （`ops.rs:326-338`、security M-2・要確認）。
- `open_with_system` / `reveal_path` のシェル経由有無
  （`main.rs:1346, 3421`、security M-3）: **確認済み・リスクなし**。
  `gpui-pre-linux 0.3.12` の `run_open_commands`（platform.rs:1182）は
  `open::commands()` が返す `std::process::Command` を
  `smol::process::Command::spawn()` で直接実行しており、`sh -c` 等の
  シェルを経由しない（2026-11-17T21:08+09:00、ローカルレジストリ確認）。
- SVG プレビューの外部エンティティ設定
  （`preview.rs:83-89`、security M-4）: **確認済み・リスクなし**。
  `gpui-pre-0.3.12` の `svg_renderer.render_single_frame(&bytes, 1.0)`
  （platform.rs:3504）は SVG バイトのみを受け取り、ベース URL / ファイル
  パスを渡さない。`usvg` は `default-features = false` で外部エンティティ
  解決機能が無効（2026-11-17T21:19+09:00、ローカルレジストリ確認）。
- `copy_recursive` / `remove_entry_safe` / `total_bytes_of` に
  深さ上限が無く再帰スタック overflow の余地
  （`ops.rs:382-429` 他、security M-5）。
- 異常終了時の `.partial-*` 残存と起動時クリーンアップ欠如
  （`ops.rs:272`、`main.rs` に掃除処理無し、ops M-4・親確認済み）。
- `build-deb.sh --skip-build` が既存バイナリの `--version` を
  検証せず誤配布の余地（ops M-5）。
- `read_to_string` の EACCES が NotFound と同扱いで無通知
  （`settings.rs:145-148`、ops M-2）。
- ドキュメントの数値乖離（親の独自発見・訂正済み）:
  行数・フィールド数はレビュー時点のスナップショットであり現在値
  ではない。初回 5,588 行・約 60 フィールド → 第2回 5,909 行 →
  第3回 10,751 行・50 フィールド（2026-11-19 に `wc -l` と
  `Explorer` 構造体で実測）。以降の変更は追記していない。
  テスト数は第2回レビュー時点のスナップショット（実実行 167 件 =
  164 passed + 3 failed、2026-11-17）。現在のテスト数は `cargo test`
  を実行して確認（README は drift 防止のため固定値を記載しない方針）。
  当初 `#[test]` を grep して 173 件と数えたのは誤りで、
  実行数へ訂正した。
  ops M-7 と統合。
- Windows 実行時のテスト 3 件失敗（Runtime Proof で判明、
  詳細は検証エビデンス）: symlink  tests 2 件とセッション
  復元 test 1 件。いずれも Windows 固有のテスト期待値問題で
  実装の回帰ではないが、クロスプラットフォーム宣言と整合しない。

### Low 指摘（要約）

- `unwatch` 失敗の握り潰し（`watcher.rs:130-132`）。
- DEBOUNCE / RECONCILE_INTERVAL 等の定数ハードコード。
- `scripts/check.sh` が `--app --poc` 同時指定で誤成功
  （`run_app` / `run_poc` が両方 false になる）。
- `build-deb.sh:149` の Maintainer が個人アドレス固定。
- `GPUI_FE_DEBUG_*` フックは debug ビルドのみ有効・release では
  コンパイルアウト（前回判断を as-built 反映し、README に明記済み）。
- `ConflictPolicy::Fail` / `Skip` が本番未使用。
- UI 文言リテラルが `strings.rs` に集約しきれていない。
- DESIGN.md の変更履歴が v0.1 のまま（as-built 差分未反映）→ 解消済み:
  DESIGN.md「変更履歴」表の `0.1-as-built` 行を追加（2026-11-17 の P1 機能、
  2026-11-21 の OS アイコン反映）。当時は `DESIGN.md:1198-1199` にあったが、
  現在は `DESIGN.md:1214-1215`。詳細は DESIGN.md「変更履歴」を参照。
- UI ヘッドレステスト不在（設計上 P2 先送りで合意済み）。
- `build-deb.sh` の `out_dir="/"` 指定を拒否していない。
- 設定の平文永続化（参考情報）。
- CI の mutable タグ参照（AGENTS.md 方針に沿うため現状維持）。

### 却下・格下げ（根拠付き）

- **poc の `fs::metadata` による symlink 追従（security L-1）
  → 却下**: `poc/src/main.rs:73-79` のコメント通り、
  「リンク先ディレクトリへ遷移可能にする」設計意図であり、
  app 側も `model.rs:39-41` で同じ方針。脆弱性ではない。
- **arch の「Explorer は 40 超フィールド」→ 数値修正**:
  第2回レビュー時点の実測は 39 フィールド。趣旨（責務集中）は維持。
- **前回 H-3「設定マイグレーション解消済み」→ 格下げ**:
  移行の「枠組み」のみで実変換アームは存在しない。
  前方互換（H-6）として High に再分類した。
- **前回「構造化ログ解消済み」→ 格下げ**: 最低限の stderr
  テキストであり、当時 tracing 導入は残課題（H-3、2026-11-17 解消）。

### フォローアップ（第2回）

1. C-1 を最優先で修正する（データロス防止）。
2. H-1 / H-2 は `create_new(true)` または `tempfile` で
   一時ファイルを安全化する。
3. H-4 / H-5 はエラーを `LoadState::Error` または
   `last_error` の実理由へ反映し、H-3 の tracing 導入と
   同時に `op_id` を設計する。
4. H-6 は未知 version で読み取り専用モードにし、
   `review-report.md` の H-3 表記を「枠組みのみ」に修正する。
5. H-7 / H-8 は次機能追加前に描画層抽出と単一真実源化へ
   着手する（前回から未解消の設計負債）。
6. 本レポート総評の行数（初回 5,588 → 第2回 5,909 → 第3回 10,751 行、
   2026-11-19 実測）は訂正済み。いずれもレビュー時点のスナップショットで
   現在値ではない（詳細は「Medium 指摘（重複統合・要約）」の『ドキュメントの数値乖離』項目）。
   Windows 実行で判明した 3 件のテスト失敗は `0551a0d` で解消済み
   （2026-11-19、「Windows テスト 3 件」項目を参照）。そのため
   `README.md` / `app/README.md` の既知の制限への追記は不要。

### 検証エビデンス（第2回）

- `bash scripts/check.sh --fast`（2026-11-17T10:20+09:00 完了、exit 101）:
  - `app: cargo fmt --all -- --check` — 通過。
  - `app: cargo check --all-targets --locked` — 通過
    （`Finished dev profile in 8m 00s`）。
  - `app: cargo test --all-targets --locked` — **失敗**。
    `running 164 tests` の表示は成功分のみで、
    実際は 167 件（164 passed / **3 failed** / 0 ignored、
    `test result: FAILED`）。`set -e` のため以降の
    `app` clippy / `poc` ゲートは未実行。
  - 失敗 3 件:
    1. `ops::dir_stats_tests::symlinks_are_counted_but_not_followed`
       （`ops.rs:1064`、left 2 / right 4）。symlink 生成が
       `#[cfg(unix)]`（`:1056-1060`）配下のため Windows では
       2 が正しく、アサーションが Unix 前提。
    2. `ops::tests::copy_directory_recursively_and_keep_symlink`
       （`ops.rs:805`、left 2 / right 3）。同上
       （`:792-793` が `#[cfg(unix)]`）。
    3. `tests::restore_session_prefers_saved_tabs_then_cli_then_last_then_home`
       （`main.rs:5897`、left `\\?\C:\...` / right `C:\...`）。
       Windows のロングパス正規化（`\\?\` プレフィックス）を
       テスト期待値が考慮していない。
  - 判定: 3 件ともテスト側の Windows 非対応であり、
    `dir_stats` / `copy_entry` / `restore_session` の実装
    自体の回帰ではない（コードは `#[cfg(unix)]` で
    期待値を分ける修正が必要）。CI は Linux（共有
    `rust-gates.yml@v1`）のみのため本番ゲートは維持されるが、
    README の「Windows 対応」記述との整合性は要検討。
- `npx --no-install markdownlint-cli2 review-report.md`
  （2026-11-17T10:20+09:00）: **0 issues / exit 0**。
  レビュー時点のスナップショット（現在値は各ドキュメントの lint 実行結果を参照）。
- `git status --short`: 変更は `review-report.md` のみ。
  `.serena/` は untracked（MCP ローカル設定のため対象外）。

## 第2回指摘の修正完了（2026-11-17T12:45+09:00）

第2回クロスレビューの Critical 1 件と High 6 件を修正し、残る High
1 件（H-3 tracing）は設計判断のため見送った。Windows テスト 3 件の
期待値も修正済み。

- **C-1**: 退避 rename 失敗時に `read_only` / `load_error` を設定し、
  その後の `save()` を no-op 化。理由を stderr とステータスバーへ表示
  （`7c596bc`）。
- **H-1**: `json.tmp-<nanos>-<pid>` を `create_new(true)` で予約し、
  symlink 追従を防止（`7c596bc`）。
- **H-2**: `.partial-<nanos>-<pid>-<n>` を `create_new(true)` で
  予約（`14733ea`）。
- **H-4**: `task.await` の Err を `status` と `cx.notify()` へ反映
  （`3770132`）。
- **H-5**: `new_debouncer` の `notify::Error` を保持し実理由を
  表示（`3770132`）。
- **H-6**: 未知 version を読み取り専用とし、v1 書き戻しによる
  設定消失を防止（`598e7b3`）。
- **Windows テスト 3 件**: symlink 数を `#[cfg(unix)]` で分岐、
  正規化パスを比較用に正規化（`0551a0d`）。2026-11-19 に symlink
  テストの空振りを解消（全プラットフォーム共通の制御ケースを追加）。

### 判定（修正後）

- Critical: **0 件**（C-1 解消）
- High: 残 1 件（当時 H-3 tracing 導入は新規依存・設計判断のため
  保留、2026-11-17T14:05 に導入して High 残 0 件）
- 判定: **承認**（マージブロックを解除）

### 検証

- `bash scripts/check.sh --fast`: **exit 0 / All checks passed**
  （app 168 tests / poc 4 tests / clippy 警告なし）
- `cargo check --target x86_64-pc-windows-msvc --all-targets --locked`:
  成功（警告は cfg 分岐由来の 2 件のみ）
- 新規テスト 1 件追加（退避失敗時に元ファイルが保持され save が no-op になること）
- 全変更を `origin/main` へ push（2026-11-17T12:47+09:00、
  `7c9b2e7..8167ea7`）、CI run 44012616027 が **success**（8m43s）。
  ローカルとリモート完全一致（ahead 0）。

## 第2回 Low/Medium の追加修正（2026-11-17T13:16+09:00）

承認後に残っていた低リスクの指摘も解消した。

- `scripts/check.sh`: `--app` と `--poc` の同時指定を拒否（従来は
  両方 false になり何も検証せず成功していた）。`3eee87f`。
- `build-deb.sh`: `--skip-build` 時に既存バイナリの `--version` を
  Cargo.toml のバージョンと照合し、古いバイナリの誤配布を防止
  （`2087617`）。さらに `out_dir` を絶対パス化してから
  `/` / リポジトリ直下 / 親ディレクトリ / `$HOME` を拒否
  （`.` や `..` による回避も遮断、`a935ea9`）。
- `ops.rs`: case-only rename の中継名を短い予測不能名へ変更
  （NAME_MAX 超過と衝突リスクを解消、`ee100dd`）。
- `watcher.rs`: `unwatch` 失敗を捨てず `last_error` へ反映
  （`a8e0eeb`）。

検証: `bash scripts/check.sh --fast` = **exit 0 / All checks passed**
（app 168 tests / poc 4 tests / clippy 警告なし）。
`cargo check --target x86_64-pc-windows-msvc --all-targets --locked` = 成功。
各スクリプトは `bash -n` / `shellcheck` 通過。

- 全変更を push（`4537022..261fffe`）、CI run 44016121848 が
  **success**（8m12s、2026-11-17T13:27+09:00 完了）。

## 追加リファクタ（2026-11-17T13:35+09:00）

- 一意名生成の `exists()` ループは、同一関数内の初期チェックと
  ポリシー分岐で用途が異なるため統合せず現状維持（判断）。
- `main.rs` のページステップ（行高 28px / クローム 140px）と
  グリッド列計算（サイドバー 190px / セル 108px）を名前付き定数へ
  抽出（`ROW_HEIGHT_PX` / `LIST_CHROME_HEIGHT_PX` /
  `SIDEBAR_WIDTH_PX` / `GRID_CELL_WIDTH_PX`、`2e230a6`）。
  抽出時点の値は 214px だったが、as-built（2026-11-21 現在）は
  `SIDEBAR_WIDTH_PX = 190.0`（`app/src/main.rs`）へ変更済み。
- `watcher.rs` の `DEBOUNCE` は既に定数化済み。テスト内の 5 秒
  タイムアウトはテスト専用のため現状維持。

検証: `bash scripts/check.sh --fast` = **exit 0**（app 168 tests /
poc 4 tests / clippy 警告なし）、Windows ターゲット check 成功。

- 全変更を push（`5217dc9..d40d3bb`）、CI run 44018261819 が
  **success**（10m26s、2026-11-17T13:46+09:00 完了）。

## H-3 構造化ログの導入（2026-11-17T14:05+09:00）

第2回レビューで唯一保留していた High 3（構造化ログ欠如）を解消した。
AGENTS.md §2 の事前調査（crates.io の live status、ローカルキャッシュ、
ライセンス互換、推移依存の有無）を行い、最小構成で導入した。

- `tracing` 0.1 / `tracing-subscriber` 0.3（`fmt` + `env-filter`）を直接
  依存へ追加。`tracing` は gpui 経由で既にロックファイル内に存在し、
  実質的な新規追加は subscriber とその推移依存のみ。
- `main()` で `init_logging()` を呼び、レベル・タイムスタンプ付きの
  構造化イベントを stderr へ出力。`RUST_LOG` で詳細度を変更可能。
- 操作完了（copy/move/rename/redo）とエラー（設定退避失敗、
  設定保存失敗、未知スキーマ、監視初期化失敗、ウィンドウ生成失敗）を
  `tracing` のフィールド付きイベントへ変換。デバッグフックの
  `undo-report` / `preview-report` はスクリーンショット検証用に
  `eprintln!` を維持。
- `app/NOTICE.md` へライセンスを追記、`app/README.md` へ `RUST_LOG`
  の使い方を記載。

判定: High 残 0 件 → **承認**（全指摘対応完了）。

検証: `cargo test --all-targets --locked` = 168 passed / 0 failed、
`cargo clippy --all-targets --locked -- -D warnings` = 成功、
`cargo check --target x86_64-pc-windows-msvc --all-targets --locked`
= 成功。実機起動で stderr 0 行（通常起動時はログなし）を確認。

- 全変更を push（`fe26110..0024cbb`）、CI run 44022098945 が
  **success**（9m10s、2026-11-17T14:18+09:00 完了）。

- 外部セッションが Cargo プロファイルを統一
  （`debug = "line-tables-only"` / `release.strip = true`、
  `19b5f1d`）。tracing 導入と競合なく統合済み。
- 最新コミットに対する CI run 44063705679 が **success**（8m17s、2026-11-17T19:27+09:00 完了）。
