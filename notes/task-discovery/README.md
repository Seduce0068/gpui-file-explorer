---
type: Reference
title: 残タスク・機能追加案の発掘調査（6 パス）
description: >-
  エージェントチームによる 5 回の独立調査パスと、6 回目の重複排除・妥当性吸収を
  行うための戦域ファイル。各ラウンドは round-N/ に所見を永続化する。
tags:
  - task-discovery
  - agent-team
  - gpui-file-explorer
generated: 2026-11-17T19:17:27+09:00
status: in-progress
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 目的

未知の残タスク・機能追加案をエージェントチームでふんだんに調査する。
5 回の独立パスで広く発掘し、6 回目で重複・妥当性を吸収して最終列表を作る。

## ラウンド計画

| ラウンド | 内容 | 成果物 |
|---|---|---|
| 1 | 広域発掘（5 レンズ並列） | `round-1/*.md` |
| 2 | 2 回目の独立発掘（レンズ追加・観点変更） | `round-2/*.md` |
| 3 | 3 回目の独立発掘 | `round-3/*.md` |
| 4 | 4 回目の独立発掘 | `round-4/*.md` |
| 5 | 5 回目の独立発掘 | `round-5/*.md` |
| 6 | 重複排除・妥当性評価・優先順位付け | `round-6/consolidated.md` |

## 進捗

- [x] 戦域ファイル作成（round 1 着手）
- [x] round 1（5 レンズ並列・73 件・[SUMMARY](round-1/SUMMARY.md)）
- [x] round 2（観点変更 5 レンズ・62 件・[SUMMARY](round-2/SUMMARY.md)）
- [x] round 3（状態・非同期・入力・OS・証跡 5 レンズ・55 件・[SUMMARY](round-3/SUMMARY.md)）
- [x] round 4（FS 意味論・CLI・設定・ライフサイクル・ライセンス 5 レンズ・55 件・[SUMMARY](round-4/SUMMARY.md)）
- [x] round 5（検索・描画・モデル・ゴミ箱・エラー UX 5 レンズ・58 件・[SUMMARY](round-5/SUMMARY.md)）
- [x] round 6（3 クラスタ統合＋独立妥当性再検証・303 件→55 項目・[最終レポート](round-6/consolidated.md)）

## 基点（既知の情報。新規発掘の除外対象ではないが重複表示する）

- `README.md` / `app/README.md`: 既知の制限、P1 実装済み一覧
- `DESIGN.md`: 機能マトリクス P0〜P3（§2）、未確認事項 17 件（§11）、ADR 12 件（§10.7）
- `review-report.md`: クロスレビュー指摘（Critical 0 / High / Medium / Low、一部解消済み）
- `notes/verify-report.md`: 独立検証レポート（blocker なし、major 3 は修正済み）

## 各ラウンドの出力形式（必須）

各発見は次のフィールドを持つ Markdown 表行または小節にする:

- `ID`: `<round>-<agent>-<n>`（例 `r1-code-01`）
- `title`: 短い日本語タイトル
- `category`: `残タスク` / `機能追加案` / `調査項目` / `構造改善`
- `evidence`: ファイル:行 または ドキュメント節（Runtime Proof は明示）
- `description`: 何が未達/提案か（1〜3 文）
- `value`: ユーザー価値またはリスク低減
- `effort`: `S` / `M` / `L`
- `novelty`: `新規` / `既知(ドキュメント記載あり)` / `既知だが未対応`
- `confidence`: `高` / `中` / `低`

子エージェントの出力は仮説として扱い、親が現行コード・ドキュメントで事実確認する。
