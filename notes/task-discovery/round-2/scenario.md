---
type: Reference
title: "実ワークフロー逆算による残タスク発掘（パス2・シナリオレンズ）"
description: >-
  6 つの具体的ユーザーシナリオを現在の機能セットに照らして step-by-step に
  ウォークスルーし、機能単位のチェックリストでは見えない摩擦点からタスクを
  導出した一覧。全主張をファイル:行 / DESIGN 節に根拠付けた。
tags:
  - task-discovery
  - scenario
  - workflow
  - reverse-engineering
  - gpui-file-explorer
generated: 2026-11-17T20:03:12+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## レンズと基点

本レンズは REAL-WORKFLOW / SCENARIO REVERSE-ENGINEERING を担当する。基点として
`app/README.md:80-247`（キーバインド・実装済み P0/P1）、
`app/README.md:351-468`（既知の制限）、
`app/README.md:249-255`（今後の候補）、
`DESIGN.md:1-60`（目的・非目的・ユーザー・成功基準）、
`DESIGN.md:162-219`（主要フロー A〜D）、
`DESIGN.md:61-105`（機能マトリクス）、および裏取りのため
`app/src/model.rs:349-363,589-601`、`app/src/settings.rs:53-78`、
`app/src/ops.rs`（`pub fn` 一覧・`:523`）、`app/src/preview.rs:24-55`、
`app/src/main.rs:1341-1371,1855-1984` を通読した。

pass-1 の 24 項目（terminal-open / empty file / duplicate / relative path copy /
quick filter / color tags / column customization / selection size / recent files /
batch rename / bookmark reorder / created・accessed time / symlink create /
trash purge / archive / tab session restore / breadcrumb copy・edit / OS clipboard /
search source labels / open・reveal failure / E2E input / settings schema /
100k perf / cross-platform packaging・CI / watcher recovery）は意図的に避けた。
ただし本シナリオ上どうしても交差するものは、切り口を変え `novelty` に
`既知(ドキュメント記載あり)` / `既知だが未対応` と正直に付記した
（例: 写真の EXIF 撮影日時は pass-1 の「created/accessed time＝FS タイムスタンプ」とは別物）。

## シナリオ別ウォークスルーと発見

### シナリオ A: 開発者が 2 つのソースツリーを比較したい

想定手順: (1) `Ctrl+T` で 2 タブを開きそれぞれ `Ctrl+L` でツリー A/B へ移動
(`app/README.md:90,92`、`DESIGN.md:178-191`)。(2) 行を上下して差分箇所を目で追う。
(3) 両方を同時に見ようとしてタブを切り替える。

摩擦点: タブはカレントディレクトリと履歴のみを保持し、**切替時に選択行・スクロール位置が
リセット**される（`app/README.md:445-447`）。再起動時もディレクトリとアクティブタブしか
復元されない（同）。1 ウィンドウ 1 状態の設計（ADR-002、`DESIGN.md:95`）で分割ペインは
機能マトリクスにも無い。「2 ファイルを diff ツールで開く」操作も無く、`Enter`/ダブルクリックは
単一パスを `cx.open_with_system` に渡すだけ（`app/src/main.rs:1341-1371`、
`app/README.md:100,136`）。ソートキーも名前/種類/サイズ/更新日時の 4 種のみ
（`app/src/model.rs:349-363`）。

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-scn-01 | 2 ペイン並列ディレクトリビュー（比較用分割表示） | 機能追加案 | `DESIGN.md:95` (ADR-002 1ウィンドウ1状態), `app/README.md:445-447`, `app/README.md:204-208` | タブは排他的で A/B を同時描画できない。各ペインが独立した cwd・選択・スクロールを持つ分割レイアウトを追加する。新規依存なしだが `ExplorerApp` の状態を 1 状態からペイン配列へ拡張する構造変更を伴う。 | 高（ツリー比較・コードレビューの基本動作） | L | 新規 | 中 |
| r2-scn-02 | ペイン間の同期スクロール／リンク選択 | 機能追加案 | `app/README.md:445-447` (切替でスクロール・選択リセット), `app/README.md:134-135` (scroll_to_item 追従) | 分割表示だけでは左右の対応行がずれる。片方の選択/スクロールを他方へ反映する「リンクモード」を、既存の `scroll_to_item` と選択 reconciliation を再利用して実装する。r2-scn-01 に依存。 | 中高（目視比較の精度向上） | M | 新規 | 中 |
| r2-scn-03 | 選択 2 ファイルを外部 diff ツールで起動 | 機能追加案 | `app/src/main.rs:1341-1371` (open_with_system は単一パス), `app/README.md:114`, `DESIGN.md:82` | 2 ファイル選択→「差分を表示」で `meld` / `diff` 等を起動したい。GPUI API 外の `std::process::Command` と、ツール未インストール時のフォールバック（テキストプレビュー並べ）が必要。既知制限 `app/README.md:353-357`（起動成否を検知できない）の影響も受ける。 | 中（開発者向け・既知制限の回避にも寄与） | M | 新規 | 中 |

### シナリオ B: 写真家が USB の 500 枚を取り込む（日付ソート・重複検出・EXIF リネーム）

想定手順: (1) USB SSD を挿し「ドライブ」から開く。(2) 撮影日時順に並べる。
(3) 既に取り込んだ枚と重複するものを除外。(4) `YYYYMMDD_連番.jpg` にリネーム。

摩擦点: ボリューム一覧は Linux `/proc/mounts` の**再読込時のみ**で、リムーバブルの
ホットプラグ検知・空き容量は未対応（`app/README.md:245-247,431-433`）。ソートは
FS の `modified` のみで EXIF `DateTimeOriginal` を読む経路が無い
（`app/src/model.rs:349-363,478-484`）。プレビューは画像を先頭バイトで判定するだけで
メタデータ抽出はしない（`app/src/preview.rs:24-55`）。内容検索はバイナリを除外するため
画像の重複は検出できない（`app/README.md:428-430`）。リネームは**フォーカス行 1 件のみ**
（`app/README.md:377-378`）。事前バイト走査も「ベストエフォート」で symlink 非追跡
（`app/README.md:387-388`）。

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-scn-04 | EXIF 撮影日時の抽出と撮影日ソート／グルーピング | 機能追加案 | `app/src/model.rs:349-363` (SortKey 4種のみ), `app/src/preview.rs:49-55` (magic number 判定のみ), `app/README.md:409-410` | `DateTimeOriginal` を読み `Entry` に保持して第 5 のソートキー/日付グループを追加する。新規依存 `exif`（`kamadak-exif`）が必要で、jpg/heic の対応範囲を決める調査も要る。pass-1 の created/accessed time は FS タイムスタンプであり撮影日時ではない点で別タスク。 | 高（写真取り込みの要） | L | 新規 | 高 |
| r2-scn-05 | 内容ハッシュによる重複検出（取り込み前スキップ） | 機能追加案 | `app/README.md:428-430` (内容検索はバイナリ除外), `app/src/model.rs:589-601` (fingerprint は変更検知用), `app/src/ops.rs:506-556` (total_bytes/dir_stats) | 既存 `DefaultHasher` は一覧の変更検知用で内容同一性の判定には使えない。コピー前に sha2/blake3 ダイジェストを計算し既存ファイルと比較してスキップする新規パイプライン。**新規依存（暗号学的ハッシュクレート）が必要**。 | 高（ディスク容量・取り込み時間の節約） | L | 新規 | 高 |

### シナリオ C: ユーザーがディスクを整理したい（使用量ツリーマップ＋安全な一括削除の dry-run）

想定手順: (1) どのフォルダが容量を食っているか可視化して探す。(2) 削除対象を選び、
実行前に「何が・何バイト消えるか」を確認。(3) 実行して間違えたら取り消す。

摩擦点: 再帰サイズは**プロパティダイアログの「サイズを計算」で 1 フォルダずつ任意に**
計算するだけ（`app/README.md:229-232,440-442`）。全体の使用量を俯瞰する可視化は無い。
削除はゴミ箱移動のみで完全削除は非目的（`DESIGN.md:25`、`app/README.md:406`）。
ゴミ箱ビューはサイズ「-」固定で他アプリのごみも混在（`app/README.md:417-419`）。
dry-run 機能は `ops` に無く（`ConflictPolicy` は Fail/Skip/Rename のみ、
`app/src/ops.rs:55,233-234`）、undo は名前変更とゴミ箱のみ・最大 20 件・再起動で消える
（`app/README.md:407-408`、`app/src/undo.rs:2-19`）。

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-scn-06 | ディスク使用量ツリーマップ／階層別サイズ可視化 | 機能追加案 | `app/README.md:229-232` (再帰サイズは 1 フォルダずつ), `app/README.md:440-442`, `app/src/ops.rs:523` (dir_stats), `DESIGN.md:91-92` (P2 アイコン/サムネイルのみ) | `ops::dir_stats` の再帰結果を再利用し、ディレクトリ階層を面積で表すビューを追加。新規依存なし（GPUI 自描画）だが、巨大ツリーの計算を background 化しキャンセル可能にする必要がある。機能マトリクスに無い可視化軸。 | 高（「どこを消せばよいか」が一発で分かる） | L | 新規 | 中 |
| r2-scn-07 | 一括ゴミ箱移動の dry-run／影響プレビュー | 機能追加案 | `app/src/ops.rs:55,233-234` (ConflictPolicy に simulate 無し), `app/README.md:177-179` (件数付き確認ダイアログ), `DESIGN.md:204-217` (確認→実行フロー) | 既存の件数ダイアログを拡張し、対象一覧・合計バイト・競合予定（Rename で別名になるファイル）を実行前に提示する「確認のみ」モードを追加。undo が限定的な分、事前の誤操作防止が効く。 | 高（破壊的操作の不安低減） | M | 新規 | 高 |
| r2-scn-08 | 破壊的操作の undo 履歴を再起動後も保持 | 構造改善 | `app/README.md:407-408` (スタック 20 件・再起動で消える), `app/src/undo.rs:18-19` (MAX_UNDO), `app/README.md:362-365` (設定保存は tmp+rename だが undo は未永続化) | `UndoAction` は `PathBuf` と trash id のみで serde 化可能。設定ファイルとは別の journal に永続化し起動時に復元する。pass-1 の「settings schema」は設定側の話で、undo スタック永続化は未カバー。 | 中（「あとで取り消したい」を実現） | M | 既知(ドキュメント記載あり) | 高 |

### シナリオ D: ライターが「focus フォルダ」を決まったビューで起動時に開きたい

想定手順: (1) 原稿フォルダに「名前順・隠しファイル非表示・フィルタ `*.md`」のビューを
紐付けて保存。(2) アプリを再起動したらそのフォルダが同じビューで開く。

摩擦点: `Settings` が保存するのは `last_dir` / `sort` / `show_hidden` / `view` /
`recent_dirs` / `bookmarks` / `tabs` / `active_tab` / `recent_searches` のみ
（`app/src/settings.rs:53-78`）。**名前付きビューやフィルタ状態の保存フィールドが無い**。
ブックマークはパスの List だけでビュー情報を持たない（`app/src/settings.rs:63`、
`app/README.md:241-242`）。検索のスコープ・種別・拡張子フィルタも再検索のたびに
指定し直しで、保存されるのは検索語文字列のみ（`app/README.md:194-199`、
`app/src/settings.rs:68-69`）。起動フローは `Settings::load()` → `last_dir` 一択
（`DESIGN.md:168-175`）。pass-1 の quick filter / tab session restore はあるが、
「ビューをフォルダに結び付けて起動復元する」発想は無い。

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-scn-09 | 名前付き保存ビュー（filter+sort+scope）をフォルダ／ブックマークに結び付け起動復元 | 機能追加案 | `app/src/settings.rs:53-78` (ビュー/フィルタ保存フィールド無し), `app/src/settings.rs:63` (bookmarks はパスのみ), `DESIGN.md:168-175` (起動は last_dir 一択), `app/README.md:194-199` | ブックマークを `Bookmark { path, view: Option<SavedView> }` に拡張し、起動時に指定ブックマークを cwd＋sort＋filter ごと復元する。設定スキーマを v2 に上げる移行が必要（現状 v1 固定・`app/README.md:360-361`）。 | 中高（定型作業の立ち上がりが速い） | M | 新規 | 中 |

### シナリオ E: OS からフォルダをドロップしてコピーし、チェックサムで検証したい

想定手順: (1) Dolphin からフォルダをウィンドウへドラッグ（全体をコピー）。
(2) コピー先と元のダイジェストを比較して整合性を確認。

摩擦点: ドロップは OS→アプリのコピーが実装済みで、常に `ConflictPolicy::Rename`、
上書き不可、`Esc` キャンセル（`app/README.md:209-213`、向上フローは
`app/src/main.rs:1855-1984`）。しかし**チェックサム計算・検証機能が `ops` に存在しない**
（`app/src/ops.rs` の `pub fn` 一覧に hash 系なし）。事前バイト走査は総量を見積もるだけ
（`app/README.md:184-186,387-388`）。さらに D&D の**実操作 E2E は未検証**で、
デバッグフックは release でも有効（`app/README.md:450-455`）。

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-scn-10 | コピー／ドロップ後のチェックサム検証と整合性レポート | 機能追加案 | `app/src/main.rs:1879-1935` (バイト転送のみ), `app/README.md:184-186` (進捗は総バイトのみ), `app/src/ops.rs:446-480` (copy_entry に検証段階無し), `app/README.md:450-455` (D&D E2E 未検証) | 転送と同じ 64 KiB チャンクで sha2/blake3 を更新し、完了時に元/先ダイジェストを比較してステータスへ表示する。**新規依存（ハッシュクレート）が必要**。r2-scn-05 とハッシュ基盤を共有できる。 | 高（USB 取り込み・バックアップの信頼性） | M | 新規 | 高 |

### シナリオ F: キーボードのみで「リネーム→移動→ゴミ箱」の一連を回したい

想定手順: (1) `F2` でリネーム。(2) `Ctrl+X`→移動先へ `Ctrl+V`。(3) `Delete` でゴミ箱。
マウスは一切使わない。

摩擦点: ショートカット自体は揃っている（`app/README.md:94,105,107,109-111`）。
ただしリネームは**フォーカス行 1 件のみ**（`app/README.md:377-378`）、貼り付けは常に
`ConflictPolicy::Rename` で上書き確認なし（`app/src/main.rs:1896-1902`、
`app/README.md:385`）、**移動は undo 対象外**で誤操作の戻し手段が無い
（`app/src/undo.rs:7-10`、`app/README.md:222-223,407-408`）。ソートはヘッダクリックのみで
キーボードから切り替えられない（`app/README.md:409`）。加えてキーボード操作の
**E2E 実押下検証が未実施**（`app/README.md:368-373,390-393`）、確認ダイアログの
再入も保証外（`app/README.md:364-365`）。

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-scn-11 | ソート／表示切替のキーボードショートカット | 機能追加案 | `app/README.md:409` (ソートはヘッダクリックのみ), `app/README.md:88` (表示は Ctrl+1/2 あり), `app/src/model.rs:525-536` (SortSpec/toggled) | 既存 `SortSpec::toggled` をキーへバインドし、例 `Ctrl+Shift+1..4` で名前/種類/サイズ/更新日時ソートを切り替える。表示切替（Ctrl+1/2）との対比で欠けているだけの小さな追加。 | 中（キーボード専用運用の完成度） | S | 新規 | 高 |
| r2-scn-12 | キーボード操作時の競合表示と「移動の安全な取り消し」方針 | 調査項目 | `app/src/main.rs:1896-1912` (cut は move_entry + Rename), `app/src/undo.rs:7-10` (move は意図的に undo 対象外), `app/README.md:364-365` (prompt 再入保証なし), `app/README.md:368-373` (E2E 未検証) | 貼り付けが黙って別名保存するため、キーボードのみでは「意図した場所に意図した名前で置けたか」を確認できない。移動 undo は「勝手に削除しない」原則と緊張関係にあるため、(a) 競合を事前通知する、(b) 元パスを保持する「取り消しではなく元位置へコピー」を用意する、のどちらが原則に合うか設計判断が必要。 | 中高（データ喪失リスクの低減） | M | 既知だが未対応 | 中 |

## レンズ総評

機能チェックリスト（「コピーがある」「undo がある」「タブがある」）では見えなかったのは、
**それらを連鎖させたときに途切れる点**だった。具体的には (1) タブはあっても同時表示・
リンクが無い（A）、(2) 一覧はあってもドメイン固有の属性（EXIF）と内容同一性が無い（B）、
(3) 個別サイズ計算はあっても全体俯瞰と事前確認が無い（C）、(4) 設定保存はあっても
「ビューを場所に結び付ける」概念が無い（D）、(5) 転送はあっても検証が無い（E）、
(6) ショートカットはあってもキーボード専用の安全性（競合可視化・取り消し・E2E 証明）が
無い（F）、である。

新規依存が必要なのは r2-scn-04（`exif`）、r2-scn-05 / r2-scn-10（ハッシュクレート）の
3 件で、いずれも「既存の `DefaultHasher`（変更検知用）を転用しない」点を明記した。
残りは既存の `ops::dir_stats` / `SortSpec::toggled` / `scroll_to_item` / 進捗チャネルを
再利用できる。既知制限の再掲に留まる r2-scn-08 / r2-scn-12 は novelty を
`既知(ドキュメント記載あり)` / `既知だが未対応` として正直に区別した。

信頼度は EXIF・ハッシュ・dry-run・キーボードソート（コードに直接欠落が見える）を「高」、
分割ビュー・ツリーマップ・保存ビュー（設計判断と UI 構造変更を伴う）を「中」とした。
全 12 件のうち 10 件が `新規` で、シナリオレンズが pass-1 の機能列挙と補完的に効いたことを
示している。親エージェントは r2-scn-05 と r2-scn-10 を「ハッシュ基盤」として一本化し、
r2-scn-01→02、r2-scn-06→07 は依存関係を保って優先順位を付けるとよい。
