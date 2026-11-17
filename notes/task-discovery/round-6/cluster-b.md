---
type: Reference
title: "PASS 6 クラスタ F〜J 最終統合（アンドゥ・ゴミ箱・プレビュー・配布・証跡）"
description: >-
  クラスタ F（アンドゥ・履歴・タブ）、G（ゴミ箱）、H（プレビュー・サムネイル）、
  I（クロスプラットフォーム・配布・供給網）、J（ドキュメント・証跡）の生 finding を
  ルート原因ごとに統合し、妥当性・優先度・次の一手を付与した最終判定表。
tags:
  - task-discovery
  - pass-6
  - dedup
  - cluster-f-j
  - gpui-file-explorer
generated: 2026-11-17T21:41:13+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

<!-- markdownlint-disable MD013 -->

## 最終統合判定表

20 件の統合項目。sources 列にクラスタ F〜J の全 raw ID をちょうど 1 回ずつ計上している。
妥当性は `確定` / `未確認` / `却下` / `既知(低優先)`、優先度は 高/中/低、工数は S/M/L。
高優先 7 件のうち 7 件について現行ソースを直接読み直して再検証した（下記「検証のけん引」参照）。

| ID | title | category | sources | evidence | validity | priority | effort | next-action |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| R6-B-01 | macOS でゴミ箱ビューがビルド不能（os_limited 不在） | 残タスク | r5-trash-10 | 確定。trash-5.3.0/src/lib.rs:426-430 の cfg は windows または all(unix, not(macos), not(ios), not(android), feature="freedesktop") に限定され macOS は明示除外。app/src/trash_view.rs:19 (list) / :26 (restore) / :49 (restore_by_id 内の restore_all) が無条件に trash::os_limited を呼ぶ。親は round-5/VALIDATION.md:22 でローカル crate ソース直接確認済み。macOS ターゲットはコンパイル必定失敗。README.md:84-86 の「上流 gpui-pre-apple 制約」説明では直接原因を説明しきれていない。 | 確定 | 高 | M | 着手前に ADR を残し、macOS は #[cfg] でゴミ箱ビューを除外して「未提供」と明示するか、os_limited 相当 API（代替クレート含む）を選定する。round-5 指摘どおり設計判断の文書化が先決。 |
| R6-B-02 | undo/redo スタックがエラー経路と並走で静かに壊れる | 残タスク | r3-state-03, r3-state-04, r3-state-02, r3-async-05 | 確定（r3-state-03 は親確認済み: round-3/VALIDATION.md:26、r5-err-06 は重複で cluster M 担当）。main.rs:2311-2322 の !current.exists() / target.exists() は pop_undo() 後に早期 return し push_undo しない。:2346 / :2385 の非同期 Err 経路は push_undo するため同期失敗だけ不整合。:2291-2295 / :2399-2404 の op_task ガードは undo/redo 自身が op_task を設定せず（設定は :1982 の paste のみ）、Ctrl+Z 連打でスタック順序が反転しうる。r3-state-04 は :2497 の if let Ok(list()) で redo 成功時に undo が記録されない経路。 | 確定 | 高 | M | (a) Rename の 2 つの早期 return と :2497 の Err 側で push_undo を呼ぶ。(b) undo/redo 専用 in-flight フラグを導入し :2291 / :2400 のガードと操作ライフタイムを対応させる。2 連続 Ctrl+Z で push_redo 順が逆になる回帰テストを追加。 |
| R6-B-03 | 検証ゲートの真実源不在（test 件数・CI run・audit スキップ） | 構造改善 | r3-proof-01, r3-proof-02, r3-proof-03, r3-proof-06, r3-proof-07 | 確定。README.md:26/52/135 は 167 passed、notes/verify-report.md:17/47-48/93 は 35 passed、review-report.md:601-602/664-665 は 164 passed+3 failed=167、:723/748 は 168、静的 grep は 174（本エージェント再確認: grep -rh '#[test]' app/src = 174）。scripts/check.sh:32-33 は --fast で run_audit=false、:90-96 は cargo-audit 不在時に skipping を表示して継続し All checks passed へ到達（本エージェント再確認）。親は round-3/VALIDATION.md:24-25 で r3-proof-01/03 を確認済み。 | 確定 | 高 | S | cargo test -- --list の出力を notes/test-inventory.md へコミットし全ドキュメントをリンク。check.sh は audit スキップ時に SKIPPED を明示して非 0 化するか --require-audit を追加。CI run permalink・commit SHA・最終確認日を 1 表へ集約し、解消済み表記には commit+テスト ID+検証日時を必須化する。 |
| R6-B-04 | 証跡画像・再現手順が as-built と乖離 | 残タスク | r3-proof-04, r3-proof-05, r3-proof-08, r3-proof-09, r3-proof-10 | 確定。本エージェント再確認: sha256 が app/screenshot/watcher-before.png = window-crop.png = window.png = 041cc9b8…、watcher-after.png = watcher-refresh-crop.png = 0cd6f279… でバイト同一（新規撮影でない）。app/src/main.rs の GPUI_FE_DEBUG_* は 47 個（grep -c = 47）、P1 画像は入力 E2E でなく状態デモ。ls app/src は 14 個のフラット .rs で model/ と fs/ は存在せず、DESIGN.md:943-956 のテスト一覧パスが実在しない。r3-proof-05 の旧パス参照は親が round-3 で確認。r3-proof-08 は主担当が cluster C（dedup-index:35）だが J の範囲にも含まれるため参照として計上。 | 確定 | 高 | M | 画像ごとに sha256・取得コマンド・env・時刻を manifest 化し、重複は「複製」と明記して AC-7 ペアを再撮影。フック由来画像には hook 名を付け「状態デモであり入力 E2E でない」と README に記載。DESIGN.md:943-956 と付録コマンドを実パスへ更新。 |
| R6-B-05 | クロスプラットフォーム実機検証と OS 連携の未整備 | 残タスク | r1-plat-01, r1-plat-02, r1-plat-03, r1-plat-04, r1-plat-05, r1-plat-06, r1-plat-07, r1-plat-08, r1-plat-10, r1-qual-14, r3-osint-07, r3-osint-08 | 確定（大半は既知の制限として README/DESIGN に記載）。親は round-1/VALIDATION.md:25 で r1-plat-03 の非 Unix symlink Unsupported（ops.rs:375-391）を確認。r1-qual-14 は review-report.md:599-602 で Windows test が MSVC linker 不在で link 不可、CI は Linux のみ。r3-osint-07 は ops.rs 自前実装と MAX_UNDO=20 への IFileOperation 委譲提案、r3-osint-08 は open -R 依存への NSWorkspace 比較 PoC（外部仕様未確認）。 | 確定 | 高 | L | windows-latest / macos-latest の matrix job を共通ワークフロー Seduce0068/workflows 側で対応検討のうえ追加（AGENTS.md §5）。Windows は longPathAware+symlink フォールバック、3 OS の隠し属性・ドライブ列挙・OS クリップボードを実機検証し記録。r3-osint-07/08 は外部 API 仕様を一次情報で確認してから PoC 化。 |
| R6-B-06 | ゴミ箱復元の失敗・衝突・部分成功が正確に扱われない | 残タスク | r5-trash-01, r5-trash-05, r5-trash-06, r5-trash-07, r5-trash-12 | 確定。trash 5.3.0 は Error::RestoreTwins（lib.rs:563-573 等）と RestoreCollision { path, remaining_items }（lib.rs:244-261,589-600）を提供するが、app/src/trash_view.rs:25-27 と main.rs:4052-4055 が to_string() で詳細を捨てる。同名 2 行同時選択で 1 件も復元されない（r5-trash-01）。:38-45 の restore_by_id は item.id.to_string_lossy() で比較し非 UTF-8 で誤選出（r5-trash-12、本エージェントが当該コードを直接確認）。成功 count は選択件数のまま（r5-trash-07）。 | 確定 | 高 | M | trash_view.rs の戻り値を RestoreOutcome { restored, collision, twins, other } に変更し、main.rs 側で「名前変更して復元／スキップして残りを復元／中止」を提示。RestoreTwins は選択時警告、親ディレクトリ消失（r5-trash-05）は再作成/別場所/中止を提示。ID 比較は OsString のまま保持。r5-trash-06/07 は同一の戻り値変更を共用。 |
| R6-B-07 | レイアウト定数の三重ドリフトと行・ヘッダの整列崩れ | 構造改善 | r5-render-01, r5-render-02, r5-render-03, r5-render-06, r5-render-09, r5-render-11 | 確定（r5-render-01 は親が round-5/VALIDATION.md:26 で確認）。main.rs:83 の SIDEBAR_WIDTH_PX=214 に対し実描画 :4669 は 190、DESIGN.md:134 は 200、:5428 のグリッド列数計算は 214 を控除。LIST_CHROME_HEIGHT_PX=140（:82）に対し実 chrome は約 88px。ヘッダ cell のみ :4266 が px_2 を持ち :4464-4495 の行側と約 8px ずれる。grid_columns（model.rs:329-336）は gap/padding を無視。ステータスバー右側（:5063-5182）とヘルプキー列（:2916-2928）に truncate が無い。 | 確定 | 中 | S | SIDEBAR_WIDTH_PX を sidebar() の .w() にも適用し 214/190/200 を 1 値へ統一。LIST_CHROME_HEIGHT_PX を実 chrome に合わせるか実測導出へ。ヘッダの px_2 を外すか行側へ同量付与。grid_columns に gap/padding を渡し境界テスト（available=756 なら 6 列）を追加。右側テキストとヘルプキー列に truncate を付与。 |
| R6-B-08 | 履歴ナビゲーションと検索ビューの反映不整合・定数陳腐化 | 残タスク | r3-state-01, r3-state-08, r1-code-09, r1-code-02 | 確定（コード由来）。main.rs:1298-1307 / :1316-1325 は history.apply() の後で target.is_dir() を判定し、失敗してもスタックが変化したまま。:656-658 の reload() は view==Search で早期 return する一方、undo/redo 成功時（:2342-2343 / :2440-2441）は pending_select を設定してから reload を呼ぶため検索一覧が古くなり :807-815 まで持ち越される。history.rs:31-36,44-46 の max_back=200 はリテラル。main.rs:496 のコメントは Ctrl+Shift+H だが実装/ヘルプは Ctrl+Shift+U。 | 確定 | 中 | S | target.is_dir() を History::apply 前に検証し成功時のみスタック更新。検索ビューでも reload が効くようにするか pending_select を検索結果へ適用。max_back を定数化し設定化を検討。main.rs:496 のコメントを Ctrl+Shift+U に修正。 |
| R6-B-09 | ゴミ箱の表示情報とエラー診断が不足 | 残タスク | r5-trash-02, r5-trash-03, r5-trash-04, r5-trash-08, r5-trash-09, r5-trash-11 | 確定。trash_view.rs:65-74 の entries_from_items は original_parent を破棄し :60-64 は DeletionDate 欠損を None→「-」に写すが main.rs:4494 の見出しは「更新日時」。:77 の newer-first 固定が apply_trash_loaded で直接適用され sort_header クリック（main.rs:5422-5426）が効かない。:729-739 の list() は error.to_string() で原因別案内不可。:72 のサイズは「-」固定だが crate の metadata()（lib.rs:392-424）が TrashItemSize を提供。r5-trash-08 は手動再読込のみで「ゴミ箱（自動更新なし）」と明記済み。 | 確定 | 中 | M | Entry に元パスと size を保持し詳細行/プロパティへ表示（metadata はバックグラウンド一括取得、失敗時のみ「-」）。見出しを「削除日時」へ切替え欠損時は「日時不明」。sort_header を無効化するか SortSpec を適用。list エラーを構造化し「再読み込み」ボタンを追加。自動更新は trash_folders() + notify/ポーリングで既存 generation ガードを再利用。 |
| R6-B-10 | ゴミ箱削除の TOCTOU と完全削除の未実装 | 調査項目 | r1-gap-05, r1-ux-14, r2-sec-03, r3-async-04 | 未確認（エクスプロイト未証明）。main.rs:3474-3479 の exists/is_symlink 確認と :3524 の trash::delete は別フェーズで、確認後の差し替えが可能（r2-sec-03、親は率低〜中と記載）。:3460-3594 の trash_paths は window.prompt の receiver.await を含むのに op_task/op_cancel を設定せず、:3532-3593 の完了コールバックが reload() で世代を進め他操作と競合（r3-async-04）。完全削除は DESIGN.md:100 で P0/MVP 禁止・P2 で二段確認付き限定解禁（r1-gap-05 / r1-ux-14）。 | 未確認 | 中 | M | trash_paths を op_task/op_cancel パイプラインへ載せるか専用ビジーフラグ＋世代スナップショットを導入（r3-async-08/10 のセンチネル問題も同時解消）。削除直前に canonicalize した inode を再確認し不一致ならスキップ通知。完全削除は DESIGN §6.7 の二段確認・既定無効・赤警告・フルパス表示を必須化して P2 で実装。 |
| R6-B-11 | プレビューの機能拡張・形式判定・サイズ上限 | 残タスク | r1-code-14, r2-sec-10, r2-cmp-10, r2-cmp-11, r4-cfg-10, r4-cli-04 | 未確認（SVG 外部参照の実際の解決は未検証）。preview.rs:81-88 は先頭 1 KiB で `<svg>` を探すため XML 宣言が長いと判定不能（r1-code-14）。:81-88 の sniff 結果を :127-134 で renderer へ渡すが gpui_kit の SVG 実装が外部参照を解決するか未確認（r2-sec-10）。README.md:434 はシンタックスハイライト/PDF 非対応を明記（r2-cmp-10/11）。preview.rs:19-22 の MAX_PREVIEW_BYTES/MAX_IMAGE_BYTES は定数で再ビルド必要（r4-cfg-10）。main.rs:5559-5565 はファイル引数を is_dir() でふるいにかけ捨てるが open_preview(path,...) は実装済み（r4-cli-04）。 | 未確認 | 中 | M | SVG は gpui_kit のラスタライザを一次情報で確認し、外部参照を解決するなら image/foreignObject/href を含む SVG を拒否。sniff 範囲を拡大。シンタックスハイライトは syntect 等既存クレートの採用可否を調査（自作回避）。PDF は外部ビューワ起動か軽量レンダラ依存を比較。ファイル引数は親ディレクトリへ移動して open_preview を呼ぶ経路を追加。上限はプレビュー UI からの「もっと読む」で設定化。 |
| R6-B-12 | サムネイルと OS ネイティブアイコン（共有キャッシュ未準拠） | 機能追加案 | r1-gap-07, r3-osint-04, r3-osint-10 | 既知(低優先)。DESIGN.md:91-92 と app/README.md:416 でロードマップ P2「OS アイコン、サムネイル」として明記、現状は Lucide 静的アイコンのみ（r1-gap-07）。r3-osint-04 はサムネイル自体は r1-gap-07 で扱済みとし残る独自キャッシュ `~/.cache/<app>/thumbnails/` の非互換を指摘。r3-osint-10 は org.freedesktop.thumbnailer D-Bus 問い合わせ提案。いずれもハッシュ算法・インターフェース名は外部仕様未確認。 | 既知(低優先) | 低 | L | P2 ロードマップ項目として既存 crate（freedesktop-icons 等）と共有キャッシュ仕様を一次情報で調査してから着手。D-Bus thumbnailer は zbus でサービス有無を問い合わせる PoC を先に作り、動画/PDF は MVP 外を維持。自作サムネイラは調査結果選定後に判断。 |
| R6-B-13 | 描画の未検証点と UI 機能拡張 | 調査項目 | r5-render-04, r5-render-05, r5-render-07, r5-render-08, r5-render-10, r5-render-12 | 未確認（性能影響は未測定、GPUI 内部挙動は未確認）。main.rs:4339-4411 と :4534-4604 は可視行ごとに Entry::clone・String・Vec・listener 2 個・MenuBuilder Box を生成（r5-render-04）。:4353/:4357 と :4549/:4553 で icon_for_entry と kind_for_entry が別々に kind_for_name を呼び小文字化 String を二重確保（r5-render-05、icons.rs:28-41,84-92）。:4851-4897 のパンくずは全セグメント描画後 clipping（r5-render-07）、:4667-4676 のサイドバーに overflow_y_scroll が無い（r5-render-08）。:5436-5457 の uniform_list 固定高前提は mode 切替時の再計測タイミング未検証（r5-render-12）。ダイアログ z-order は既知（r5-render-10）。 | 未確認 | 中 | M | FileKind を Entry 生成時に 1 回求めて保持するか icon_and_color() へ統合（純粋関数でテスト済みのため安全）。アロケーション削減は 100k シナリオのプロファイリング結果を待ってから判断し cluster C の性能測定項目と紐付ける。パンくずは末尾優先 elision、サイドバーは overflow_y_scroll＋セクション折りたたみを追加。z-order は active_dialog 列挙で描画を 1 つに制限するか仕様化。 |
| R6-B-14 | undo スタックの再起動後永続化 | 構造改善 | r2-cmp-09, r2-scn-08 | 確定（両者は同一根因の重複）。app/README.md:218-224,407-408 と app/src/undo.rs:12-19 で undo は名前変更とゴミ箱のみ・最大 20 件・再起動で消える。UndoAction は PathBuf と trash id のみで serde 化可能。r2-cmp-09（competitor）と r2-scn-08（scenario）は「操作履歴 journal を永続化し起動時復元」で一致。novelty は r2-cmp-09 が新規、r2-scn-08 が既知（ドキュメント記載あり）で食い違うが主張は同一。 | 確定 | 中 | M | settings.rs とは別の op-log JSON を追加し、ゴミ箱 ID の失効・元パス再利用・対象消滅時の拒否ルールを設計してからシリアライズ。再起動後に「削除を取り消す」が可能であることを回帰テストで固定。 |
| R6-B-15 | タブ状態の復元と表示設定のスコープ | 残タスク | r1-ux-16, r3-state-09, r4-cfg-08 | 確定（うち r1-ux-16/r4-cfg-08 は既知の制限として文書化済み）。app/README.md:204-208,445-447 と settings.rs:106 でタブのディレクトリと active index のみ復元、戻る/進む履歴・選択・スクロールは失われる。main.rs:5579-5583 は存在しないパスを filter 後 active.min(tabs.len()-1) で clamp するが除外分の index を引き直さず [A,B,C] active=1 で A 消滅なら [B,C] active=1（C が開く）。settings.rs:57-59 / tabs.rs:11-14 / main.rs:1085-1086 で sort/show_hidden/view は全タブ共通。親は round-4/VALIDATION.md:30-31 で r4-cfg-08 と r1-ux-16 を「別軸」と指摘。 | 確定 | 中 | M | restore_session で filter 前の index を対応付け直し、保存済み active が指すタブを追跡する。history/Selection を settings へシリアライズして完全復元（r1-ux-16）。表示設定のタブ単位化はスキーマ v2 と移行を伴うため ADR で可否判断後に対応（r4-cfg-08、親の別軸指摘を尊重し独立 next-action として残す）。 |
| R6-B-16 | 配布・CI・アプリ識別子と WASM 展開のライセンス表示 | 残タスク | r1-plat-09, r1-plat-11, r1-plat-12, r1-plat-13, r1-plat-15 | 確定（文書/CI 定義由来）。main.rs:5633 に Wayland app_id はあるが cx.set_app_identity 呼び出しなし（r1-plat-11）。README.md:60-77 で DEB amd64 のみ、署名・apt repo・タグ・tar.gz/AppImage/Flatpak なし、AGPL §4 の source tarball+SHA-256 同行も未実施（r1-plat-12）。.github/workflows/ci.yml:27-43 は ubuntu-latest のみ（r1-plat-13）。r1-plat-09 は std::fs/notify/trash 直依存でブラウザ不可。DESIGN.md:888-894 で現状は §13 非発動だが Web デモ公開で発動しうる（r1-plat-15）。 | 確定 | 中 | L | 起動時に set_app_identity を追加。GitHub Releases へ source tarball+タグ+SHA-256+GPG 署名を添付し apt repo を公開、build-deb.sh の stage から非 Debian 向け tar.gz を生成。CI matrix は R6-B-05 と同じ共通ワークフロー検討に統合。WASM は default-features=false の読み取り専用デモを別 crate で先行させ、公開前に About/初回画面へソース URL を表示して §13 対応を明記。 |
| R6-B-17 | 依存バージョン追跡とビルド再現性 | 調査項目 | r1-plat-14, r2-up-01, r2-up-02, r2-up-03, r2-up-04, r2-up-05, r2-up-06, r2-up-07, r2-up-08, r2-up-09, r2-up-10, r2-up-11 | 未確認（上流 0.10.0/0.3.13/notify 9 の修正内容・安定化時期は未確認だが版本存在は親確認済み: round-2/VALIDATION.md:22-23）。ローカルインデックスに gpui-kit 0.10.0 と notify 9.0.0-rc.9 が存在。app/Cargo.toml:6 の rust-version=1.92 に対し実行環境 rustc 1.100.0（親確認）。app/build.rs:27 の cfg!(target_os="linux") はホスト評価、:50 に絶対パスの -L native=（親確認: round-2/VALIDATION.md:26）。完全ピン =0.9.1 と Cargo.lock コミットは遵守済みだが unmaintained 8 件の監視と cargo-deny/vet ゲートが未運用（r1-plat-14）。 | 未確認 | 中 | M | update 候補（gpui-kit 0.10.0、notify 9、dirs 7）は上流 changelog を一次情報で確認してから ADR 化。build.rs は CARGO_CFG_TARGET_OS 判定と相対 -L native= へ修正。MSRV 1.92 の根拠を文書化し rust-toolchain を追加して下限ビルドをゲート化。app/poc のバージョン文字列一致を CI extra_gates に追加。cargo-deny で unmaintained/GPL 混入/重複版を監視。 |
| R6-B-18 | ライセンス・NOTICE の完全性と自動ゲート化 | 残タスク | r4-lic-01, r4-lic-02, r4-lic-03, r4-lic-04, r4-lic-05, r4-lic-06, r4-lic-07, r4-lic-08, r4-lic-09, r4-lic-10, r4-lic-11 | 確定（r4-lic-09/02 は親が round-4/VALIDATION.md:26 で cargo metadata により確認）。19 ファイルすべてに SPDX 付与済みだが check.sh に検証ステップなし（r4-lic-01）。NOTICE 類に著作権者/年号なし（r4-lic-02、親確認）。cargo metadata の distinct license に MPL-2.0（cbindgen/dwrote/option-ext）、Unicode-3.0、0BSD、BSL-1.0、NCSA、Apache-2.0 OR GPL-2.0-only 等が含まれ DESIGN.md:925 の allow 一覧と不一致（r4-lic-09、親確認）。通知同梱（build-deb.sh:134-138）と Zed GPL コード不在（git 依存 0 件）は充足済み（r4-lic-06/10）。J クラスタの r4-lic-02/05/07 もここに計上（重複回避）。 | 確定 | 中 | M | check.sh に SPDX 網羅チェックを追加し新規ファイルの欠落を防止。著作権者/年号の holder と記載範囲を設計判断で決定し NOTICE/README へ集約（r4-lic-02/05）。DESIGN.md:925 の allow 一覧と NOTICE 要約を cargo metadata 結果へ合わせ、MPL-2.0 等の扱いを deny.toml でゲート化。desktop Comment へのソース入手案内（r4-lic-07）は P2 として追加。法的結論は避け調査項目として残す親の姿勢を継承。 |
| R6-B-19 | CLI 契約とドキュメントの陳腐化 | 残タスク | r1-code-01, r1-code-03, r3-proof-11, r4-cli-01, r4-cli-02 | 確定。GPUI_FE_DEBUG_PROGRESS/UNDO_HISTORY/TAB_MOVE が main.rs:594/603/618 と ephemeral 配列 :605-626 に存在するが README:390-405 に無い（r1-code-01）。preview.rs:8-9 の doc は「first 8 KiB」だが実装 :163 は bytes.contains(&0) で全範囲（r1-code-03）。DESIGN.md:14-28/47-57 に CLI 方針が無く CliAction のみ存在（r4-cli-01）。main.rs:5523-5530 で --version が --help より優先され DEB ビルドが出力書式に依存するが README に載らない（r4-cli-02）。review-report.md:718-719 が求めた Windows 失敗 3 件のドキュメント痕跡が 0 件（r3-proof-11、本エージェント grep 再確認）。 | 確定 | 中 | S | ADR で公式 CLI 面（起動ディレクトリ/help/version のみ）と非目的を決定し、--help/--version をクイックスタートへ追記（後方互換を壊さない）。デバッグフック 3 種を README へ追記。preview.rs:8-9 のコメントを全範囲走査へ修正。Windows 失敗 3 件・修正 commit 0551a0d・検証 run を既知の制限へ追記してフォローアップ #6 を close。 |
| R6-B-20 | ウィンドウジオメトリが永続化されない | 機能追加案 | r4-life-05 | 確定。main.rs:5626-5637 の Bounds::centered(None, size(px(1100.0), px(700.0)), cx) がハードコード、settings.rs:53-78 に bounds/x/y/width/height 相当フィールドなし、DESIGN.md:839-840 の永続化リストもジオメトリ対象外。親は round-4/VALIDATION.md:25 で r4-life-01/02（終了時 save と Ctrl+Q 迂回）を確認済みで、本件は同じ保存タイミング設計と隣接。 | 確定 | 中 | M | Settings にジオメトリを追加しウィンドウ移動/リサイズ時に保存、起動時に復元。モニタ構成変更で画面外に出るケースは表示可能なワークエリアへクランプする検証を必須化。r4-life-01/02 の終了時 save 設計と併せて 1 つの保存方針に整理（cluster D と調整）。 |

## 兼任クラスタ総評

### カバレッジ

- クラスタ F（13 ID）: R6-B-02 / 08 / 14 / 15 に計上。
- クラスタ G（16 ID）: R6-B-01 / 06 / 09 / 10 に計上。`r5-trash-*` 12 件を 4 項目へ吸収。
- クラスタ H（21 ID）: R6-B-07 / 11 / 12 / 13 に計上。`r5-render-*` 12 件を 3 項目へ吸収。
- クラスタ I（40 ID）: R6-B-05 / 16 / 17 / 18 に計上。`r1-plat-*` 15 件と `r2-up-*` 11 件、`r4-lic-*` 11 件を各 1〜2 項目へ吸収。
- クラスタ J（18 ID + 重複 3 ID）: R6-B-03 / 04 / 19 / 20 に計上。`r3-proof-*` は r3-proof-08 を除く 10 件を 2 項目へ吸収。

合計 20 項目。全 raw ID がちょうど 1 項目に含まれる（下記の例外・重複を除く）。

### 主要な統合判断

- **同一根因の合流**: undo 永続化は r2-cmp-09 + r2-scn-08（R6-B-14）。undo スタック破壊は r3-state-03 + r3-state-04（エラー経路）と r3-state-02 + r3-async-05（ガード空転）を 1 行にまとめ、next-action を (a)(b) に分離（R6-B-02）。
- **別解の保持**: ゴミ箱削除では TOCTOU 緩和（inode 再確認）と完全削除の P2 実装を別 next-action として残した（R6-B-10）。プレビューでは SVG サニタイズ・シンタックスハイライト・PDF・ファイル引数・上限設定を個別 next-action で保持（R6-B-11）。
- **タブ軸の分離**: r4-cfg-08（設定スコープ）と r1-ux-16（履歴・選択の復元）は親の「別軸」指摘（round-4/VALIDATION.md:30-31）を尊重し、同一行内で独立 next-action とした（R6-B-15）。

### 親判定の継承

- `r2-a11y-09` は cluster O 担当かつ親が却下（round-2/VALIDATION.md:33、テキスト併記で「色のみ」ではない）。本ファイルには計上しないが、同じ「色だけで伝える」系の主張は採用しない方針を継承する。
- `r5-err-06` = `r3-state-03` の重複は cluster M 担当。R6-B-02 では r3-state-03 を主担当として計上し r5-err-06 は含まない。
- `r1-code-10` の ALLOWED_FS 件数は親が 22→20 に訂正済み（round-1/VALIDATION.md:32-33）。本ファイルでは件数を参照しない。
- `r1-qual-07`（README の v1 read 記述と実コードの read-only 不一致）は round-1 で親検証済み。cluster D 担当のため本ファイルには計上しない。
- `r5-trash-10` は親が round-5 でローカル crate ソース直接確認済み。本ファイルでは 確定・高優先・最上位（R6-B-01）として継承した。

### インプット例の取り扱い

タスク文中の「open/reveal 失敗 + FileManager1 フォールバック（r1-code-11 + r1-gap-03 + r3-osint-01）」は dedup-index 上クラスタ A の ID であり、本担当（F〜J）の範囲外のため計上しない。本ファイルでは macOS のFinder reveal に関する r3-osint-08 のみを R6-B-05 に含めた。親がクラスタ A を統合する際に、同例の「解決策が異なる場合は別 next-action として残す」方針を適用する想定である。

### 重複・主担当の調整が必要な ID

- `r3-proof-08`: dedup-index:35 でクラスタ C が主担当。J の範囲（r3-proof-01〜11）にも含まれるため R6-B-04 に参照として計上した。クラスタ A〜E 統合側と重複する場合は C 側を残し本ファイル側を削除してよい（性能実測なしという主張は同一）。
- `r4-lic-02` / `r4-lic-05` / `r4-lic-07`: I（r4-lic-01〜11）と J の両方に列挙される。主担当を I とし R6-B-18 に 1 回だけ計上した。J 側では重複計上していない。

### 検証のけん引（本エージェントの Runtime Proof）

以下の 7 件について現行ソースを直接読み直し、親判定を補強した。いずれも読み取りのみでソースコードは変更していない。

| 項目 | 直接確認した根拠 |
| --- | --- |
| R6-B-01 | trash-5.3.0/src/lib.rs:426-430 の cfg、trash_view.rs:19/26/49 の無条件呼び出し |
| R6-B-02 | main.rs:2311-2322 の早期 return、:2346/:2385 の Err 側 push_undo、:2291/:1982 のガード |
| R6-B-03 | grep -rh '#[test]' app/src = 174、README=167 / verify-report=35 / review-report=164+3 と 168、check.sh:32-33/90-96 の audit スキップ |
| R6-B-04 | sha256 041cc9b8… と 0cd6f279… のバイト同一、GPUI_FE_DEBUG_* = 47、ls app/src に model/ と fs/ が不在 |
| R6-B-05 | ops.rs:375-391 の非 Unix Unsupported（親 round-1 確認を継承）、review-report.md:599-602 の Windows link 不可 |
| R6-B-06 | trash_view.rs:25-27 の to_string()、:38-45 の to_string_lossy() 比較、trash lib.rs の RestoreCollision/RestoreTwins |
| R6-B-07 | main.rs:83=214 / :4669=190 / :5428 の控除、DESIGN.md:134=200（親 round-5 確認を継承） |

### 残る不確実性

- r3-osint-07/08、r3-osint-04/10、r2-sec-10、R6-B-11 の SVG 挙動は外部仕様（COM API / NSWorkspace / freedesktop ハッシュ / gpui_kit ラスタライザ）に依存し、一次情報未確認のため 未確認 とした。
- R6-B-13 の描画パフォーマンスはアロケーション存在のみ確定で、実フレーム時間への影響は 100k プロファイリング待ち（cluster C の性能測定項目と連携）。
- 法的判断（ライセンス expression の許否、AGPL §13 の発動）は行わず、いずれも調査項目として残した親の姿勢を継承している。
