---
type: Reference
title: Round 5 サマリ — 敵対的 / スケール / 将来 / 保守 / メタ
description: 第5回（最終独立発掘）クロスレビューの集計、反証、第6回への引き継ぎ。
tags: [task-discovery, cross-review, summary, gpui-file-explorer]
generated: 2026-11-18T00:25:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 集計

| レンズ | エージェント | finding 数 | ファイル |
|---|---|---|---|
| 敵対的 / 攻撃連鎖 | `r5-adversarial` | 13 | [adversarial.md](adversarial.md) |
| スケール / パフォーマンス | `r5-scale` | 15 | [scale.md](scale.md) |
| 将来機能 / ロードマップ | `r5-future` | 15 | [future.md](future.md) |
| 保守性 / 負債トリアージ | `r5-maint` | 15 | [maint.md](maint.md) |
| メタ / カバレッジギャップ | `r5-meta` | 15 | [meta.md](meta.md) |
| **合計** | 5 エージェント | **73** | — |

Round 1〜5 の累計: **367 件**の生 finding。

## 第6回へ渡す反証・訂正（メタレンズより）

| 旧主張 | 訂正 | 根拠 |
|---|---|---|
| 「名前フィルタが完全デッドコード」 | **誇張**。親が `main.rs:4037-4044` の「名前:」Input 描画を確認。正しくは「ショートカット不到達＋Tab トラバース無しでキーボードのみ不可」 | r5-meta-03 + 親の確認 |
| 「GPUI_FE_DEBUG_* 47 件」 | **29 種類**に確定（第6回妥当性再検証で精密計測。最初の再計測 30 は doc コメント `GPUI_FE_DEBUG_*` のワイルドカードを誤計上していた） | r5-meta-04 + c6-valid + 親の精密再計測 |
| 「ALLOWED_FS 20 種」 | 親が `mounts.rs:28-50` を実数え: **21 種**（前回の「修正値」自体が off-by-one） | r5-meta-05 + 親の再計測 |
| 「NOTICE タイポ未対応」 | **修正済み**（親が `app/NOTICE.md:29` の `gpui_kit::assets::AllAssets` を確認） | r5-meta-06 + 親の確認 |
| 「r5-render-05 未対応」 | **部分対応済み**（ヘルパ統合は入ったが二重呼び出しが残存） | r5-meta-02 |
| 「trash が macOS 除外のみ」 | macOS・iOS・Android を除外し freedesktop feature 依存 | r5-meta-11 |

## 第6回が優先すべき重複クラスタ

1. **undo 項目消失**: `r1-correct-01/02/03` = `r1-sec-10` = `r2-state-01..04` = `r2-async-08` = `r3-fs-06` = `r4-model-09` = `r4-error-08` = 前回 `R6-B-02`
2. **検証ゲート**: `r1-verify-02` = `r3-upstream-15` = `r4-dist-05` = `r4-proof-07` = `r5-maint-14` = 前回 `R6-B-03`
3. **名前フィルタ到達性**: `r2-input-01/02` = `r2-scenario-02/03` = `r3-search-06` = `r5-meta-03`（反証反映）
4. **グリッド scroll index**: `r3-render-03` = `r5-scale-15`
5. **設定読取失敗時上書き**: `r1-sec-07` = `r2-state-10` 系 = `r3-config-04` = `r4-life-06` = `r4-error-04`
6. **ephemeral/デバッグ汚染**: `r1-sec-04` = `r2-state-10` = `r3-config-09` = `r4-life-07` = `r5-adv-03/04`
7. **todo.md 残タスク零**: `r4-proof-02` = `r5-maint-11` — 本次ゴールの最終操作の直接根拠

## ハイライト

- **攻撃連鎖の昇格**（`r5-adv-01/03/04`）。既知 TOCTOU・デバッグフックが「任意ディレクトリ削除」「起動時のみで実ファイルのゴミ箱化/コピー」へ。リリース版の `GPUI_FE_DEBUG_*` が主要原因。
- **O(n²) / 大量 clone**（`r5-scale-01/02/03/05/09`）。10万件で選択・監視・ソートが破綻する構造的事実。ただし 100k 実測は皆無（`r5-scale-14`）で「理論的」扱い。
- **メタな自己訂正**（`r5-meta-04/05/06/03`）。レビュー記録自体の数値・修正状態が誤っており、第6回はこれを吸収してから最終列表を作る。

## 次ラウンド（最終）

Round 6 では 367 件を重複排除・妥当性評価し、優先順位付き最終列表を `round-6/consolidated.md` にまとめ、**`todo.md` へ反映**する。全調査は読み取り専用で、ソースコードは変更しない。
