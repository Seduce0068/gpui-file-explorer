---
type: Reference
title: タスク発見ラウンド1 -- ロバストネス・性能・テストカバレッジレンズ
description: >-
  gpui-file-explorer の未対応タスク・機能追加提案の発見（5並列パスの1つ）。
  合成入力代替、10万件性能、キャンセル/EXDEV、watcher 劣化、
  設定前方互換、tracing、エラー UX、テストギャップを file:line 根拠付きで列挙する。
tags:
  - task-discovery
  - robustness
  - performance
  - test-coverage
  - gpui-file-explorer
generated: 2026-11-17T19:31:57+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 調査範囲と表の読み方

ベースライン（README.md / review-report.md / notes/verify-report.md /
notes/lead-verify/SUMMARY.md / DESIGN.md §10.5・§10.6・§11 /
app/README.md「テスト」「既知の制限」）を読み、現行ソースを直接読んで
file:line を検証した。表内のパスはリポジトリルートからの相対パス。
novelty は「新規」= ベースラインに未記載、「既知（ドキュメント記載あり）」=
制限として明記済みで追加対応不要、「既知だが未対応」= 記載済みだが
具体策・検証が残っているものを指す。ここではソースを変更していない。

### r1-qual-01 10万件ディレクトリの実測が無く、O(n) ホットスポットも未検証

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-qual-01 | 10万件性能の実測と O(n) 更新パスの計測 | performance | DESIGN.md:1031,1049,1101; DESIGN.md:939; app/src/main.rs:999,1013,1033,953 | DESIGN §11 項目10・§10.5（10万件 200MB 以下）が未計測のまま。加えて watcher 1イベントごとに `all_entries.retain`(main.rs:999)・`iter_mut().find`(1013)・`fingerprint(&all_entries)`(1033) が全件走査し、reconcile も 953 で全件ハッシュする。10万件ではバースト時に事実上 O(n²) となり、目標の「監視イベント反映 1フレーム」(DESIGN.md:1030) と矛盾しうる。1k/10k 生成スクリプトと `VmRSS`/フレーム時間の記録を追加し、必要ならパス→インデックス HashMap 化を判断すべき。 | 高（P2 設計判断の前提条件が欠落） | 中（計測スクリプト + 計測、構造変更は別） | 既知だが未対応 | 高（コードと設計書を直接確認） |

### r1-qual-02 キー入力 E2E の代用として `on_key_down` の純粋ロジック抽出とテストを追加

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-qual-02 | 合成入力不能環境でのテスト代用（キー→アクション pure 化） | test-coverage | app/src/main.rs:1477; app/src/main.rs:507-629; app/README.md:368-373; notes/verify-report.md:99,219-220 | 実押下は Wayland 合成入力ツール不在で NOT TESTABLE と明記済み。しかし現状の代替は `GPUI_FE_DEBUG_*` フック（main.rs:507-629）が UI 状態を直接書き換えるだけで、`on_key_down`(1477) の分岐そのものは一切通らない。キー→アクション判定を GPUI 非依存の純関数へ切り出し、代表キー（Ctrl+C/X/V/Z、F2、F1、Esc、矢印、Backspace、Alt+←→）のマッピングをデータ駆動で検証すれば、実機なしで回帰を捕捉できる。DESIGN §10.1 の UI ヘッドレス（P2)より低コストな現実解。 | 高（既知制限への実行可能な代替） | 中（巨大 if チェーンの抽出リファクタ） | 既知だが未対応（具体的代用案は新規） | 高（コード・ドキュメント双方を確認） |

### r1-qual-03 コピー途中のキャンセル（64KiB チャンク間）にテストが無い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-qual-03 | `copy_entry` 途中キャンセルのテスト追加 | test-coverage / robustness | app/src/ops.rs:299-302,308-322; app/src/ops.rs:924-941; app/README.md:379-389 | .cancel は 64KiB チャンク間で確認し、`.partial-*` を削除する設計だが、既存テスト `cancellation_leaves_no_partial_file`(924) は**開始前に cancel 済み**のケースのみ。1チャンク以上書き込んだ後のキャンセル、複数ファイル中の途中キャンセル、および取消時に `stats.created` が歪まないことのテストが無い。UI は「処理済み項目は元に戻らない」と明記しており(README:380)、ユーザーが最も遭遇する経路が未検証。 | 高（データ整合性の主要経路） | 低（Large file を一時生成するテスト1〜2件） | 新規 | 高（実装とテスト本体を読了） |

### r1-qual-04 EXDEV フォールバック（copy→delete）の実経路が未テスト

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-qual-04 | `move_entry` の CrossesDevices 経路のテスト追加 | test-coverage / robustness | app/src/ops.rs:619-643; app/src/ops.rs:1024-1038; review-report.md:235-237 | `io_error` の EXDEV→`ErrorKind::CrossesDevices` 変換テストはあるが、`move_entry` が実際に copy_recursive→remove_entry_safe へフォールバックし、コピー失敗時に元を残す(625-626)/削除失敗時に理由を返す(633-640)経路はテストされていない。別デバイス（/dev/shm 等）を使う統合テスト、またはエラー注入用の内部関数抽出が必要。 | 中〜高（移動のデータ損失 conditional 経路） | 中（クロスデバイス一時領域 or 注入リファクタ） | 新規 | 高（ops.rs とレビュー記述を照合） |

### r1-qual-05 監視失敗後の自動復旧トリガーが無い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-qual-05 | 監視エラーからの自動リトライ契機を追加 | robustness | app/src/watcher.rs:128-134; app/src/main.rs:860-867,924-960,1047-1055; notes/verify-report.md:146 | `watch()` は `last_error` があるとき再試行するが、`update_watcher()` は `reload()` 経由でしか呼ばれない。10秒 reconcile(1047) は fingerprint 不変なら `apply_loaded` も `reload` も呼ばず(953)、結果として「権限回復/一時消失からの復帰」を監視が自動検知できない。reconcile 成功時に `watcher.watch()` を再試行する（失敗時は status を維持）などの復旧経路と、その再試行テストを追加すべき。 | 高（「監視: 一部無効」が永続化しうる） | 低〜中 | 既知だが未対応（verify-report m-2 は watch 再試行自体の修正済みだが復旧トリガー未記載） | 高（呼び出しグラフを直接追跡） |

### r1-qual-06 watcher の実行時エラー内容が捨てられ、テストも空配列のみ

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-qual-06 | `normalize_events` の実エラー保持と、unwatch 失敗のテスト追加 | test-coverage / observability | app/src/watcher.rs:33-36,98-108,139-156; app/src/watcher.rs:213-216,318-328; DESIGN.md:1043,1097 | キュー溢れを含む `DebounceEventResult::Err(Vec<Error>)` は Rescan に丸められ、実エラーはログにも `last_error` にも残らない（初期化失敗 98-108 と unwatch 失敗 139-156 は保持済み）。既存テスト `watcher_errors_become_rescan`(213-216) は `Err(vec![])` のみで、エラー内容の保持・溢出時の Rescan 伝播・unwatch 失敗集約のテストが無い。`error!(...)` を記録し status/トレースへ渡す配線を追加すべき。 | 中〜高（DESIGN R6/§11-6 の保険が機能しているか観測不能） | 低 | 既知だが未対応（Degraded 未実装は verify-report m-1 記載、実エラー保持の具体策は新規） | 高（watcher.rs 全行を読了） |

### r1-qual-07 設定の前方互換に関する README 記述が実装と矛盾

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-qual-07 | 未知スキーマ時の read-only 動作を README に反映 | docs / settings | app/README.md:358-361; app/src/settings.rs:205-224,246-253; app/src/settings.rs:466-481; review-report.md:702-703 | コードは未知 version を警告して `read_only=true` にし `save()` を no-op 化する（settings.rs:218-220,249-253）ことに加え、テスト `migration_normalizes_future_version_without_dropping_values`(466) も存在する。一方 README:360-361 は「警告を出して v1 として読み込みます」と旧挙動のまま（v1 書き戻しは実際には起きない）。利用者が設定が保存されない原因を誤解する。1行修正で解消可能。 | 中（誤解を招く回復 UX） | 低（ドキュメント1行） | 新規 | 高（コード・テスト・README を突合） |

### r1-qual-08 構造化ログに相関 ID が無く、ログ出力のテストも 0 件

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-qual-08 | 操作相関 ID (op_id) の導入と tracing イベントの検証 | observability / test-coverage | review-report.md:642; app/src/main.rs:1947-1955,2429-2437,2482-2490,3300-3306; app/src/main.rs:5594-5600; app/src/watcher.rs:103 | `tracing` 導入自体は完了したが、フォローアップで「H-3 導入と同時に op_id を設計」(review-report:642) とされた相関 ID は未実装。copy/move 完了ログ(1947) と watcher/設定系ログは `op=` や `path=` だけで横断追跡できない。また全ソースで logging を検証するテストは 0 件（grep で `#[test]` 内の tracing 参照なし）。`tracing-subscriber` のカスタムレイヤーを 테스트で差し替え、 Major な操作で op_id が一致することをassert する lightweight テストを追加すべき。 | 中（事後追跡・監査） | 中（ID 設計 + 伝播 + テスト） | 既知だが未対応 | 高（レビュー·コード·テスト棚卸しで確認） |

### r1-qual-09 再スキャン失敗ステータスが後続の watcher イベントで即座に上書きされる

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-qual-09 | エラー status の保持／優先度ルールを導入 | error-UX / robustness | app/src/main.rs:934-947; app/src/main.rs:988,1026,1035; app/src/main.rs:1943-1964 | reconcile 失敗時に `status="再スキャンに失敗しました"` を設定する(942)が、その直後に届く watcher イベントが `STATUS_CHANGED` で上書きする(988/1035)。操作完了時の「一部失敗」(1960)も同様に短命になりうる。重大エラーを `LoadState::Error` や専用フィールドで保持し、次回成功まで status に復元する（あるいは上書き禁止）ルールと、その遷移テストが必要。 | 中（監視保険の失敗がユーザーに見えない） | 低〜中 | 新規 | 中（コード上は明確だが、実際の発頻度は未計測） |

### r1-qual-10 再帰的なサイズ走査に深さ上限が無く、深いツリーのテストも無い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-qual-10 | `total_bytes_of` / `copy_recursive` / `remove_entry_safe` の反復化と深さテスト | robustness / test-coverage | app/src/ops.rs:557-577,395-441,482-499; app/src/ops.rs:885-901; review-report.md:584-586 | `total_bytes_of` はディレクトリごとに再帰し、`copy_recursive`・`remove_entry_safe` も再帰で深さ上限が無い（レビュー Medium M-5 記載のまま）。既存テストは浅い一時ツリーのみで、数百〜千段の深さでスタックが尽きるケースが未検証。加えて `total_bytes` は `dir_stats` と異なり CancelFlag を受け取らず（ops.rs:506-508 vs 523-554）、巨大ツリーの事前走査（main.rs:1880）を Esc で止められない（README:387-388 は時間のみ明記）。反復スタック化 + cancel 受け取りへ統一すべき。 | 中（クラッシュ経路・キャンセル UX） | 中 | 既知だが未対応 | 高（ops.rs とレビューを確認） |

### r1-qual-11 進捗チャネルが unbounded で、スロットルの実効性が未検証

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-qual-11 | 進捗 Channel の bounded 化と 80ms スロットルのテスト | performance / test-coverage | app/src/main.rs:1842-1873,1852,1893-1895; review-report.md:561-562; app/README.md:386 | `async_channel::unbounded` に小ファイル単位で `try_send` し、80ms スロットルは受信側到着後のフィルタ（レビュー M-3 で指摘済み・未対応）。数万ファイルのコピーでキューが膨らみ、cancel 後も pump が残留メッセージを捨てるまで回る。bounded channel + 送信側の最新値上書き、および「N件の進捗が高々 M 回の notify に集約される」テストを追加すべき。 | 中（大規模コピー時の安定性） | 中 | 既知だが未対応 | 高（コードとレビューを照合） |

### r1-qual-12 case-only rename の中継パスに O_EXCL 予約が無い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-qual-12 | リネーム中継名の `create_new` 予約 | security / robustness | app/src/ops.rs:676-685; app/src/ops.rs:331-335; review-report.md:736-737 | case-only rename の中継名は短い予測不能名へ変更済みだが、`fs::rename(src,&temp)` は宛先を無条件に置換する。copy の `.partial-*` / `dest` が `create_new(true)` で予約される方針(331-335)と不一致で、事前配置された symlink を置換しうる。中継名も `create_new` で予約（予約ファイルを `rename` が置換）するか、同等の O_EXCL 保証をコメントで明記すべき。 | 低〜中（競合は稀だが安全則の整合性） | 低 | 新規 | 中（予測不能化は修正済みで実害は限定的） |

### r1-qual-13 `RUST_LOG` の不正値が黙って info にフォールバックする

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-qual-13 | ログ設定の失敗を利用者へ通知 | error-UX / observability | app/src/main.rs:5594-5600,5608-5609; app/README.md:77-78 | `EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))` により、typo や不正ディレクティブは警告なしで info に戻る。`try_init` も `_ =` で握り潰す。利用者は「RUST_LOG=debug を設定したのにログが出ない」原因を自己診断できない。突破方法: フォールバック時に `eprintln!`/tracing で理由を表示し、README に有効な例を1つ追加する。 | 低（開発者 UX） | 低 | 新規 | 高（init_logging を直接読了） |

### r1-qual-14 Windows 向けテストゲートが Linux CI に無い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-qual-14 | Windows ターゲットの test 実行ゲート（別ジョブ）を追加 | CI / test-coverage | review-report.md:599-602,718-719; README.md:84-86; .github/workflows/ci.yml:1-40 | Windows の `cargo check --target x86_64-pc-windows-msvc` は成功するが、test バイナリは MSVC linker 不在で link 不可。第2回レビューで発見された Windows 固有テスト失敗3件は期待値修正済みだが、CI は Linux のみで再発を検出できない。Windows runner（link.exe 利用可）を別ジョブで追加するか、最低限 `cargo check --all-targets --target ...` をゲート化し、README の「Windows 対応」記述の根拠を更新すべき。 | 中（クロスプラットフォーム宣言の信頼性） | 中（CI 設定 + 外部 runner） | 既知だが未対応 | 高（レビューと CI 定義を確認） |

## レンズ総評

167 件のテストはデータ層に偏り（review-report.md:406-409 の分布）、
UI 層は純粋ヘルパーのみで、合成入力が無い環境での実押下代替が最大の
構造的ギャップだった。**新規の実装バグ**としては、
(1) 監視失敗後の自動復旧トリガー欠如（r1-qual-05）、
(2) 再スキャンエラー status の上書き（r1-qual-09）、
(3) 設定 read-only 化と README の矛盾（r1-qual-07）、
(4) case-only rename 中継の O_EXCL 欠如（r1-qual-12）が
特に低コストで対応可能と判断した。テストギャップでは
**途中キャンセル（r1-qual-03）** と **EXDEV フォールバック（r1-qual-04）**
が「安全側の分岐なのに未検証」という点で優先度が高い。

性能面は 10万件実測（r1-qual-01）が DESIGN 上の未確認事項のままで、
コード側の O(n) 更新パスと合わせて計測→判断のサイクルを回すべきである。
既知の制限として明記されている項目（合成入力 E2E・unmaintained 警告・
Windows/macOS 実機・デバッグフック残存）は、本レンズでは
「既知（ドキュメント記載あり）」として重複提案していない。
なお本パスではソースを一切変更していない（読み取り専用調査）。
