---
type: Reference
title: Round 1 サマリ — 広域・基盤レビュー
description: 5 レンズ並列の第1回クロスレビュー結果（生 finding 71 件）の集計とハイライト。
tags: [task-discovery, cross-review, summary, gpui-file-explorer]
generated: 2026-11-17T23:26:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## レビュー対象の基点

- レビュー開始時点の HEAD: `5e660f1`。未コミットの `app/src/main.rs` / `app/src/search.rs`
  （名前部分一致フィルタ追加）をワーキングツリーごと読み取って評価した。
- ラウンド中に同一変更が `55a803e`（`feat(search): add name substring filter with
  Ctrl+Shift+F focus`）としてコミットされた。行番号はレビュー時点のもので、
  commit 後に数行ずれている可能性がある（第6回の妥当性再検証で確認する）。

## 集計

| レンズ | エージェント | finding 数 | ファイル |
|---|---|---|---|
| アーキテクチャ / 構造 | `r1-arch` | 15 | [arch.md](arch.md) |
| 正確性 / ロジック | `r1-correct` | 13 | [correct.md](correct.md) |
| セキュリティ / プライバシー | `r1-sec` | 15 | [sec.md](sec.md) |
| 移植性 / クロスプラットフォーム | `r1-port` | 15 | [port.md](port.md) |
| 検証 / CI / ドキュメント真理 | `r1-verify` | 13 | [verify.md](verify.md) |
| **合計** | 5 エージェント | **71** | — |

内訳（速報・第6回で確定）: 新規 約30 件 / 前回重複・既知 約41 件。

## 親による事実確認（Runtime Proof）

エージェント出力を仮説として扱い、以下を現行コードで直接確認した:

| 主張 | 確認結果 |
|---|---|
| `r1-sec-01` move の rename TOCTOU | `ops.rs:604-612` で存在確認後 `fs::rename`。確認 |
| `r1-correct-08` 上限適用がソート前 | `search.rs:257-268` で `return` 後に `sort_by`。確認 |
| `r1-sec-08` PoC に unsafe 禁止属性なし | `poc/src/main.rs:1-14` に属性なし。確認 |
| `r1-verify-05` 設定スキーマ説明の矛盾 | `settings.rs:209-224` が read_only を設定。README と矛盾。確認 |
| `r1-correct-13` validate が last_dir のみ | `settings.rs:226-233`。確認 |
| 前回の高優先事项の残存 | `trash = "5.3"`、`GPUI_FE_DEBUG_*` 47 件、`cfg(debug_assertions)` 0 件、unbounded チャンネル 2 箇所 — いずれも現行コードに残存 |

## ハイライト（第6回の統合候補）

1. **Undo 項目消失**（`r1-correct-01/02/03` = `r1-sec-10` = 前回 R6-B-02）。複数レンズから重複検出、最も確度が高い functional bug。
2. **move の上書き禁止保証崩壊**（`r1-sec-01`、新規・高）。`ops.rs:604-612` の rename 競合。
3. **起動時設定の連鎖リスク**（`r1-sec-04/05/07`、`r1-correct-13`）。デバッグフック・配列上限・読取失敗・validate 漏れ。
4. **リリースビルドのデバッグフック**（`r1-arch-08` = `r1-sec-03`、前回 R6-C-10）。47 環境変数・0 cfg gate。
5. **検証ゲートの真実源不在**（`r1-verify-01/02/03/06`、前回 R6-B-03）。132/167/168 の分岐、audit スキップ、DEB 同梱 README の陳腐化。
6. **非 UTF-8 引数 panic**（`r1-port-04` = `r1-sec-13`、新規・高）。`args()` → `args_os()` で低工数解決。

## 次ラウンド方針

Round 2 では観点を切り替え、**シナリオ / 状態 / 非同期 / 入力 / アクセシビリティ** の 5 レンズで独立発掘する。本ラウンドの生 ID は重複判定の基点として round-6 で参照する。
