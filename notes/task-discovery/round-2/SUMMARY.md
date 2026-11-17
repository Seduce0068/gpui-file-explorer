---
type: Reference
title: 残タスク発掘 PASS 2 統合サマリ（5 レンズ・観点変更）
description: >-
  PASS 1 とは異なる 5 レンズ（上流依存・他 FM 比較・a11y/i18n・セキュリティ・
  シナリオ逆算）による第 2 パスの発掘結果を統合した中間成果物。
tags:
  - task-discovery
  - agent-team
  - round-2
  - gpui-file-explorer
generated: 2026-11-17T20:24:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 実行結果

PASS 1 の機能列挙的レンズと重複しないよう、観点を意図的にずらした 5 レンズを並列起動した。

| レンズ | 成果物 | 件数 | エージェント状態 |
|---|---|---|---|
| 上流／依存バージョンドリフト | [upstream.md](upstream.md) | 11 | 完了 |
| 他ファイルマネージャ機能比較 | [competitor.md](competitor.md) | 15 | 完了 |
| アクセシビリティ／i18n | [a11y.md](a11y.md) | 12 | 完了（markdownlint 0 issues を自己検証） |
| セキュリティ脅威モデル再評価 | [security.md](security.md) | 12 | 完了 |
| ユーザーシナリオ逆算 | [scenario.md](scenario.md) | 12 | 完了 |
| **合計** | | **62** | 全レンズ完了（PASS 1 の再委譲失敗は今回なし） |

## PASS 1 との補完関係

- PASS 1 が「ドキュメント／コードに既に書かれた未実装」を拾ったのに対し、
  PASS 2 は「書かれていない観点」を狙った。結果、新規性 `新規` の比率が高い。
- 機能列挙では出てこなかった構造課題: `r2-up-09`（build.rs のホスト/ターゲット混同）、
  `r2-up-07`（宣言 MSRV 1.92 vs 実 rustc 1.100.0）、`r2-sec-01`（デバッグフックの
  リリースビルド残存）、`r2-sec-04`（設定 JSON 配列の上限未適用）。
- シナリオレンズからは「ハッシュ基盤」（r2-scn-05 重複検出 + r2-scn-10 コピー検証）
  や「2 ペイン」（r2-scn-01 + r2-cmp-01）といった、複数 findings を束ねる
  上位テーマが浮かび、第 6 パスの統合単位として有望。

## 親による事実確認（[VALIDATION.md](VALIDATION.md) に詳細）

- **採用**: 上流の版本存在（`gpui-kit 0.10.0` / `notify 9.0.0-rc.9`）は
  ローカル crates.io インデックスを `strings` で直接確認。trash の Portal backend は
  同梱 CHANGELOG:27 を確認。`rustc 1.100.0`、build.rs:27 の
  `cfg!(target_os = "linux")`、`GPUI_FE_DEBUG` 47 件／`debug_assertions` 0 件も確認。
- **却下**: `r2-a11y-09`（「色のみで状態表現」）は根拠行に
  `エラー: {error}` / `監視: 無効 — {reason}` などのテキストが併記されており事実と異なる。
  `a11y.md` に直接却下注記を插入し、第 6 パスが誤採用しないようにした。

## 第 6 パスへの引き継ぎ

- 累計 finding 数: PASS 1 = 73、PASS 2 = 62、**合計 135**（生 finding）。
- 第 6 パスでは「重複吸収」に加え、PASS 1 の予備マップと本パスの
  上位テーマ（ハッシュ基盤・2 ペイン・上流ピン更新・a11y 直接未達）を
  突き合わせ、妥当性（事実確認済み／未確認／却下）を明示する必要がある。
- 未検証のまま残る外部事実（上流 0.10.0 の修正内容、notify 安定版時期、
  他 FM の現行機能）は「未確認」として扱い、断定しない。
