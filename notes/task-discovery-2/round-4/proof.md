---
type: Reference
title: Round 4 — 証跡 / 検証ゲート / as-built 真理 レビュー
description: 読み取り専用エージェント r4-proof の所見（15 件・テスト実行なしの文書監査）。
tags: [task-discovery, cross-review, proof, gpui-file-explorer]
generated: 2026-11-18T00:12:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-proof-01 | AC 判定の内訳が根拠表と一致せず AC-4 が行方不明 | 構造改善 | `review-report.md:356-359`, `:365-370`; `notes/verify-report.md:90-104` | PASS8+NOT TESTABLE4+未実装1=13 は合うが根拠表は PASS が9件で AC-4 がどの区分にも入らない。現判定を確定できない。 | 受入判定の再現性 | S | 新規 | 中 |
| r4-proof-02 | 正本 todo.md が「未着手なし」だが55件の残タスクが存在 | 構造改善 | `todo.md:15-33`; `notes/task-discovery/round-6/consolidated.md:36-60,146-159` | 共通指示が正本と定める todo.md は 進行中/未着手 が「なし」で完了5件のみ。受け手は「残タスク零」と誤読する。**本次ゴールの最終操作（todo 反映）の直接根拠**。 | 優先順位の正確な伝達 | M | 新規 | 高 |
| r4-proof-03 | 「初回実機検証時は36件」は初回35件の誤記 | 残タスク | `README.md:52`; `app/README.md:304`; `notes/verify-report.md:17,46-51` | 初回は35 passed、36 は M-1/2/3 修正後。時系列と出典を明記すべき。 | テスト履歴の正確性 | S | 新規 | 高 |
| r4-proof-04 | README の Windows check「警告なし」が review-report の2件と矛盾 | 残タスク | `README.md:85-86`; `review-report.md:724-725` | 同じ check で「警告なし」と「cfg 分岐由来の2件」が反転。 | クロスプラットフォーム宣言の裏取り | S | 新規 | 高 |
| r4-proof-05 | テスト件数の単一真実源が無く最終値168が配布READMEに反映されない | 構造改善 | `README.md:26,52,135`; `review-report.md:22,420,659,716,793` | 配布 README は167/132、review-report 最終は168、総評は132のまま。 | 受入・配布の数値整合 | S | 前回と重複(r3-proof-01) | 高 |
| r4-proof-06 | README の CI 証跡が最終承認 run より5 run 古い | 残タスク | `README.md:121`; `review-report.md:798-804` | README は run 43734742786、最終は 44063705679。commit SHA/permalink が無くクリック検証不能。 | ビルド証跡の鮮度 | S | 前回と重複(r3-proof-02) | 高 |
| r4-proof-07 | README の「完全ループ exit 0」は audit 実施を証明しない | 構造改善 | `scripts/check.sh:90-99`; `README.md:135` | audit 未導入ならスキップして exit 0。ローカル記録はすべて --fast。 | 検証ゲートの信頼性 | S | 前回と重複(r3-proof-03) | 高 |
| r4-proof-08 | AC-7 前後画像が既存画像のバイト同一コピーで出所も未記録 | 残タスク | `notes/verify-report.md:79-84,208`; `app/README.md:12` | watcher-before=window-crop=window.png、watcher-after=watcher-refresh-crop.png。新規撮影でない。 | 証跡画像の真正性 | M | 前回と重複(r3-proof-04) | 高 |
| r4-proof-09 | 「訂正済み」と書かれた総評の数値が未訂正でフィールド数は3通り | 構造改善 | `review-report.md:29,84,87,547,600,634,653` | 5,588→5,909 の訂正主張に対し総評は 5,588 のまま。フィールド数も60/39/29。 | レビュー正本の信頼性 | S | round1/2/3と重複(r1-verify-06) | 高 |
| r4-proof-10 | 現判定は本文末尾にしか無く冒頭は旧判定のまま | 構造改善 | `review-report.md:3-14,57-75,484-489,713-718,791` | 判定が4回反転し frontmatter は status: complete のみ。受け手は旧「条件付き承認」を現判定と誤読しうる。 | 受入状態の単一化 | S | 前回と重複(r3-proof-07) | 高 |
| r4-proof-11 | DESIGN §11 の未確認17項目が as-built 更新後も更新されない | 構造改善 | `DESIGN.md:1090-1108,1132`; `app/README.md:483-490` | #13 CJK は確認済み、#14 dirs は決着済み、#17 は実装済みなのに ⚠ のまま。一方実機未検証項目は「新たな未対応なし」と結論。 | 設計書と実装の整合 | S | round1/2/3と重複(r1-verify-11) | 中 |
| r4-proof-12 | AC-1〜13 判定表が2026-11-11で凍結しP1/tracing後の再判定が無い | 構造改善 | `review-report.md:356-359,709-716`; `notes/verify-report.md:88-106` | 変更後に AC-2/4/13 の再判定記録が無い。 | 受入基準のトレーサビリティ | M | 前回と重複(r3-proof-06) | 高 |
| r4-proof-13 | Windows テスト3件失敗の文書化フォローアップが未実施 | 残タスク | `review-report.md:653-655,659-680`; `README.md:79-88` | フォローアップ#6 が既知の制限への追記を要求するが文書側に痕跡なし。 | クロスプラットフォーム履歴の保存 | S | 前回と重複(r3-proof-11) | 高 |
| r4-proof-14 | PROGRESS.md 先頭の状態表が132 passed・P1未着手のまま | 残タスク | `app/PROGRESS.md:8,13,19,107-113` | 先頭は132/P1未着手、同一ファイル後半は167/P1実装済み。 | 進捗ドキュメントの鮮度 | S | 既知(ドキュメント記載あり) | 高 |
| r4-proof-15 | 検証レポート付録の再現コマンドが現在の checkout で失敗 | 残タスク | `notes/verify-report.md:7,26,246,251,262-270` | 付録は存在しないパスを前提。Runtime Proof と銘打ったコマンドが再実行不能。 | 監査コマンドの再利用性 | S | 前回と重複(r3-proof-05) | 高 |

## レンズ要約

受入・検証の主張は「時系列の異なる値が同じ真実として並ぶ」ことが最大の弱点で、テスト数・判定・AC内訳・行数のすべてで「訂正済み」表示と本文が食い違う。加えて正本 todo.md が残タスク零を宣言し、証跡画像・CI run・audit の exit 0 が真正性を証明しないため受け手は現状を過大評価する。優先は新規の 01/02/03/04 と未是正の 05/07/08。
