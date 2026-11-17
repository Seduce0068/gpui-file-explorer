---
type: Reference
title: 残タスク発掘 PASS 5 統合サマリ（最終発掘パス）
description: >-
  検索エンジン・レンダリング・データモデル・ゴミ箱・エラー回復の 5 レンズによる
  最終発掘パスの結果。これで 5 回の繰り返しが完了し、次は PASS 6 の統合となる。
tags:
  - task-discovery
  - agent-team
  - round-5
  - gpui-file-explorer
generated: 2026-11-17T21:45:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 実行結果

| レンズ | 成果物 | 件数 | エージェント状態 |
|---|---|---|---|
| 検索エンジン内部 | [search.md](search.md) | 12 | 完了 |
| レンダリング／レイアウト | [render.md](render.md) | 12 | 完了 |
| データモデル／ソート／人間化 | [model.md](model.md) | 12 | 完了 |
| ゴミ箱サブシステム | [trash.md](trash.md) | 12 | 完了 |
| エラー分類／回復 UX | [error-ux.md](error-ux.md) | 10 | 完了 |
| **合計** | | **58** | 全レンズ完了 |

## 最終パスで判明した高影響項目

- **`r5-trash-10`: macOS ビルドがコード上失敗する**。
  `trash-5.3.0/src/lib.rs:426-429` の `os_limited` は Windows + freedesktop Unix に
  限定され macOS を明示除外。一方 `app/src/trash_view.rs:19,26,49` は無条件に
  `trash::os_limited::list()` / `restore_all()` を呼ぶ。
  親がローカル依存ソースで確認済み。
  README.md:84-86 の「macOS は上流gpui-pre-apple の制約で check 不可」という説明は
  理由として不充分で、**アプリ側の直接原因**が存在する。
- **`r5-render-01`: サイドバー幅の三重ドリフト**。
  定数 214 / 実描画 190 / 設計 200 / グリッド列数計算は 214 を控除。
  親確認済みで、直近の定数名リファクタ（commit `2e230a6`）の値すり合わせ漏れが疑われる。
- `r5-search-06`: 複数ルート検索で読めないルートが黙ってスキップされ部分失敗が非通知。
- `r5-model-02`: `tar.gz` が `gz` になる／symlink 先ディレクトリが「フォルダ」表示。
- `r5-err-01`: バッチ転送が最初のエラーしか保持せず失敗項目一覧を破棄。

## 重複の吸収（[dedup-index.md](../dedup-index.md) に記録）

- `r5-err-06` = `r3-state-03`（undo 事前チェック失敗で履歴項目消失）。
  親は PASS 3 で既に確認済みのため、PASS 6 では `r3-state-03` を主担当とする。
- 新たなクラスタ候補: `r5-render-06`（grid_columns の gap/padding 未考慮）は
  `r5-model-11` と同一関数を指しており、PASS 6 で一つに統合する。

## 親による事実確認（[VALIDATION.md](VALIDATION.md)）

- 採用: `r5-trash-10`、`r5-render-01`、`r3-state-05` 再確認、`r5-search-03` 予備確認。
- 全件が読み取りのみの調査で、ソースコードは未変更。

## 累計と次パス

PASS 1 = 73、PASS 2 = 62、PASS 3 = 55、PASS 4 = 55、PASS 5 = 58、
**合計 303 件**（生 finding）。5 回の独立発掘はこれで完了。
**PASS 6 で重複排除・妥当性吸収・優先順位付けを行い最終列表を完成させる。**
