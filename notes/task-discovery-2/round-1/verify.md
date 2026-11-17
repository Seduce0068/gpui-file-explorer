---
type: Reference
title: Round 1 — 検証 / CI / ドキュメント真理 レビュー
description: 読み取り専用エージェント r1-verify の所見（13 件）。
tags: [task-discovery, cross-review, verification, gpui-file-explorer]
generated: 2026-11-17T23:25:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-verify-01 | テスト件数の真実源が不在（「132」が現行値として残存） | 構造改善 | `review-report.md:22` / `app/PROGRESS.md:13` vs `README.md:52,135` / `app/README.md:27,288,301` | 総評とPROGRESSは「132 件」と記載するが README/アプリREADMEは 167（修正後 168）を記載。proof.md は 35/36/132/167/168/174 の6通りに分岐と記録済み。 | 受入判定の再現性。cargo test -- --list を単一真実源にする | S | 前回と重複(R6-B-03) | 高 |
| r1-verify-02 | check.sh が cargo-audit 未導入時に黙ってスキップし成功表示 | 構造改善 | `scripts/check.sh:90-99` | `command -v cargo-audit` が失敗するとメッセージ1行でスキップし `:99` の "All checks passed." に到達して exit 0。README の「audit 込み」は保証しない。 | push前ゲートの信頼性 | S | 前回と重複(R6-B-03) | 高 |
| r1-verify-03 | DEB 同梱 README が「132 tests」のまま配布 | 残タスク | `packaging/linux/build/.../usr/share/doc/gpui-file-explorer/README.md:27,276,289` | ビルド成果物内の README は 3 箇所で 132 と記載。リポジトリ直下は 167 に更新済みで生成元との同期が欠落。 | 利用者への古い検証結果配布を解消 | S | 前回と重複(r3-proof-01) | 高 |
| r1-verify-04 | Windows/macOS の CI ゲートが存在しないのに「CI ビルド」を示唆 | 構造改善 | `ci.yml:21-43`, `README.md:84-86`, `DESIGN.md:1100` | ci.yml は Linux の1ジョブのみ。DESIGN §11 #9 は「CI ビルド以外未検証」と書くが該当 CI ビルドは存在しない。 | クロスプラットフォーム宣言の裏取り | M | 新規 | 高 |
| r1-verify-05 | app/README の設定スキーマ説明がコードと矛盾（未知 version） | 残タスク | `app/README.md:360-361` vs `settings.rs:197-223,466-483` | README は「未知の version は警告を出して v1 として読み込みます（ダウングレード検出は未実装）」と記載。実際は `migrate()` が read_only/load_error を設定し v1 書き戻しを防ぎ、テストがそれを検証。**親が現行コードで確認済み。** | 利用者の誤解と「解消済み」判定の信頼性低下を防ぐ | S | 新規 | 高 |
| r1-verify-06 | review-report 内の訂正が総評へ反映されず数値が自己矛盾 | 構造改善 | `review-report.md:29,84` vs `:600,634,653` | 総評は「5,588 行・約 60 フィールド」のまま。後半は実測 5,909 行・39 フィールドと訂正し、フォローアップ :653 で「総評を訂正」と todo 化するも未実行。 | レビュー正本の数値信頼性 | S | 新規 | 高 |
| r1-verify-07 | 判定が同一レポート内で「実質承認→マージブロック→承認」と反転 | 構造改善 | `review-report.md:57-75,480-489,713-718,791` | 冒頭「実質的に承認」、第2回「マージブロック」、修正後「承認」、tracing 導入で再び「承認」と 4 回反転。各判定に commit/テスト ID の明示が無い。 | 受入状態の単一化 | S | 前回と重複(r3-proof-07) | 高 |
| r1-verify-08 | undo の事前チェック早期 return で項目が消失（回帰テストなし） | 残タスク | `main.rs:2340,2355-2366,2390,2414,2429` / `undo.rs:138-246` | `pop_undo` 後、早期 return が popped action を戻さない（Err/trash 経路は戻す）。undo.rs の7テストはスタック構造のみで当該経路を捕捉しない。 | 実際の undo 項目ロス防止と回帰検知 | S | 前回と重複(R6-B-02) | 高 |
| r1-verify-09 | ゴミ箱復元テストが Err 時に trivially pass | 残タスク | `trash_view.rs:124-145` | `unknown_ids_are_reported_as_missing` / `empty_input_is_a_noop` は `if let Ok(...)` で包み、Err なら何も検証せず合格。 | trash 復元の実回帰を検出可能に | S | 新規 | 高 |
| r1-verify-10 | 正証拠スクリーンショットが P0 時代のままで as-built UI と乖離 | 残タスク | `app/README.md:12` / `README.md:58` / `notes/verify-report.md:79-80,208` | 主画像 window-crop.png はタブバー・ゴミ箱/ドライブ・ビュー切替が無い P0 UI。as-built の tabs.png と乖離。AC-7 の before/after もバイト同一コピーと記録済み。 | 受入証跡の as-built 反映 | S | 前回と重複(R6-B-04) | 高 |
| r1-verify-11 | DESIGN §11 の未確認事項が as-built 更新後も更新されない | 構造改善 | `DESIGN.md:1090-1108,1132` / `README.md:84-86` / `model.rs:1123` | 変更履歴は v0.1-as-built を記録するが §11 の17項目は ⚠ のまま。#9 は「CI ビルド以外未検証」（CI ビルド自体なし）、#13 は CJK 未確認（日本語パス表示・テストあり）など陳腐化。 | 設計書と実装の乖離を解消し残リスクを優先付け | S | 既知(ドキュメント記載あり) | 中 |
| r1-verify-12 | watcher 差分適用・検索スナップショット更新にヘッドレステストなし | 残タスク | `main.rs:969-1081` / `watcher.rs:194-223` / `strings.rs:101` | watcher.rs の6テストは normalize_events / removed_paths / inotify 検知のみ。spawn_watch_loop の upsert、親由来パス除外、検索結果 removed 除去、STATUS_SEARCH_STALE 設定は GPUI async 内で未検証。 | 監視取りこぼし・検索結果の誤操作を防ぐ回帰網 | M | 新規 | 中 |
| r1-verify-13 | テストが UI 文言リテラルを直接アサートし実装詳細に結合 | 構造改善 | `main.rs:5818-5823,5840-5843,5851-5857` | `summarize_op` は "2.0 K" / ": dest" / 日本語文言、`help_sections_are_well_formed` は total>=20、`empty_state_covers_all_views` は完全一致タイトルを検証。 | 文言変更と機能回帰を分離 | S | 新規 | 中 |

## レンズ要約

テスト件数（132/167/168）とゲート成功定義に単一の真実源がなく、配布 README や review-report 総評に訂正未反映の陳腐化が集中している。コード側では undo 早期 return の項目消失と trash 復元テストの空 Through、監視差分適用の未検証が残り、「解消済み」表記を支えるエビデンスの再現性が弱点。
