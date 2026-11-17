---
type: Reference
title: Round 2 サマリ — シナリオ / 状態 / 非同期 / 入力 / a11y
description: 第2回クロスレビュー（観点変更5レンズ）の集計とハイライト。
tags: [task-discovery, cross-review, summary, gpui-file-explorer]
generated: 2026-11-17T23:52:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 集計

| レンズ | エージェント | finding 数 | ファイル |
|---|---|---|---|
| ユーザーシナリオ | `r2-scenario` | 14 | [scenario.md](scenario.md) |
| 状態機械 / 永続化 | `r2-state` | 15 | [state.md](state.md) |
| 非同期 / スレッド / キャンセル | `r2-async` | 14 | [async.md](async.md) |
| 入力 / キーボード / フォーカス | `r2-input` | 15 | [input.md](input.md) |
| アクセシビリティ / i18n | `r2-a11y` | 15 | [a11y.md](a11y.md) |
| **合計** | 5 エージェント | **73** | — |

Round 1 との累計: 144 件の生 finding。

## 親による事実確認（Runtime Proof）

| 主張 | 確認結果 |
|---|---|
| `r2-input-01` Ctrl+Shift+F が検索ビューで届かない | `main.rs:1582-1587` の early return が `:1597` の control 分岐より前。**確認**。検索外では `name_filter=None` で no-op（`r2-scenario-02` と同根）。**新規機能が完全にデッドコード**。 |
| `r2-input-03` 検索ビューで Escape 以外が全て破棄 | 同上の early return。確認。 |
| `r2-state-05` 履歴 pop が `is_dir()` 判定より前 | `history.rs:51-62` が pop/push 先行、`main.rs:1346` で判定。確認。 |
| `r2-state-10` debug 値が起動時 save で永続化 | `main.rs:510-543` のフック後に `:640` で無条件 `save()`。確認。 |
| `r2-state-11` validate が last_dir のみ | `settings.rs:226-233`。確認（r1-correct-13 と同一）。 |
| Round 1 の `r1-sec-07` | `settings.rs:176-178` が read 失敗時に `Self::default()`（read_only なし）を返し `main.rs:640` が上書き。確認。 |

## ハイライト（第6回の統合候補）

1. **新規機能の到達性**（`r2-input-01/02` = `r2-scenario-02/03`）。`55a803e` で追加された名前フィルタがキーボードから完全に到達不能。ヘルプ表記・空状態ヒントも未更新。最も新鮮で低工数の functional bug。
2. **Undo/Trash 項目消失の多発**（`r2-state-01/02/03/04` = `r2-async-08` = `r1-correct-01/02/03`）。前回 R6-B-02 の未対応サイトが複数レンズから再検出。redo のパス再用（`r2-scenario-10`）は新規のデータ損失リスク。
3. **DESIGN §4.4 のスレッド契約違反**（`r2-async-01/02` = `r1-arch-04`）。`trash::list()` / `metadata` / `/proc/mounts` が UI スレッドで同期実行。
4. **未ガードの detach コールバック**（`r2-async-03/04/07/09/10/12`、`r2-state-07/13`）。generation/parent/同一性トークンが経路ごとに不均一。
5. **アプリ内 D&D 未実装**（`r2-input-08` = `r2-scenario-08`）。`on_drag` はあるが内部パスを受ける `on_drop` が無い。
6. **検索ビューのキーボード全域無効**（`r2-scenario-09`、`r2-input-03`）。前回の「Enter のみ」より範囲が広い。
7. **反証の記録**（`r2-a11y-15`）。前回の「ステータスは色のみ」却下を裏付けつつ、コントラスト不足として再定義。

## 次ラウンド方針

Round 3 では **FS 意味論 / 検索 / 描画 / 設定・CLI / 上流** の5レンズで独立発掘する。Round 1/2 の 144 件は重複判定の基点として round-6 で参照する。
