---
type: Reference
title: Round 4 サマリ — ライフサイクル / 配布 / エラーUX / モデル / 証跡
description: 第4回クロスレビュー（運用・保守観点5レンズ）の集計とハイライト。
tags: [task-discovery, cross-review, summary, gpui-file-explorer]
generated: 2026-11-18T00:13:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 集計

| レンズ | エージェント | finding 数 | ファイル |
|---|---|---|---|
| ライフサイクル / 起動・終了 | `r4-life` | 15 | [life.md](life.md) |
| 配布 / パッケージング / CI | `r4-dist` | 15 | [dist.md](dist.md) |
| エラー UX / 診断 / 回復 | `r4-error` | 15 | [error.md](error.md) |
| データモデル / 不変条件 | `r4-model` | 14 | [model.md](model.md) |
| 証跡 / 検証ゲート / as-built | `r4-proof` | 15 | [proof.md](proof.md) |
| **合計** | 5 エージェント | **74** | — |

Round 1〜4 の累計: **294 件**の生 finding。

## 親による事実確認（Runtime Proof）

| 主張 | 確認結果 |
|---|---|
| `r2-async-01` / `r1-arch-09` trash::list が UI スレッド | `main.rs:3636` が `this.update(...)` 内で `trash_view::list()`。`r4-error-08` も同じ箇所を指摘。**確認**。 |
| `r3-render-03` グリッド scroll index | `main.rs:1445-1447` が `scroll_to_item(index, ...)` に entry index を渡す。グリッドは行単位なので不整合。**確認**。 |
| `r4-proof-02` todo.md が残タスク零と表示 | `todo.md:15-33` は 進行中/未着手 が「なし」。前回 discovery の 55 項目が未反映。**確認**。本次ゴールの最終操作（todo 反映）の直接根拠。 |

## ハイライト（第6回の統合候補）

1. **todo.md が残タスク零を宣言**（`r4-proof-02`、新規・高）。55 件の既知残タスクが未反映。第6回で必ず反映する。
2. **ファイル関連付けが実質死んでいる**（`r4-dist-01/02/09`）。`.desktop` に `MimeType=` が無く、さらに保存タブが CLI 引数 (`%f`) を上書き。
3. **成功・確認文言が保証と乖離**（`r4-error-01/08`）。 open/reveal 失敗を捨てて成功表示、「あとで元に戻せます」が list() 失敗で裏付けられない。
4. **status の単一スロット競合**（`r4-error-03/05` = `r6-A-04`）。成功・警告・再スキャン失敗が reload/watcher に消される。
5. **配布物の検証ゲート皆無**（`r4-dist-06`、新規）。CI green は DEB の正当性を何も保証しない。
6. **設計型の不変条件退化**（`r4-model-01/03/04/14`）。Entry/Selection/History/path が pub フィールド＋手作業同期に退化。
7. **前回指摘の範囲修正**（`r4-error-13`）。「件数も捨てる」は部分成功には当てはまらず、全件失敗時のみ。第6回の妥当性判定に反映する。

## 次ラウンド方針

Round 5 では **敵対的 / スケール / 将来機能 / 保守 / メタ** の5レンズで独立発掘する。Round 1〜4 の 294 件は重複判定の基点として round-6 で参照する。
