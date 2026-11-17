---
type: Reference
title: Round 3 サマリ — FS / 検索 / 描画 / 設定CLI / 上流
description: 第3回クロスレビュー（データ・外部観点5レンズ）の集計とハイライト。
tags: [task-discovery, cross-review, summary, gpui-file-explorer]
generated: 2026-11-17T23:59:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 集計

| レンズ | エージェント | finding 数 | ファイル |
|---|---|---|---|
| FS 意味論 / データ完全性 | `r3-fs` | 15 | [fs.md](fs.md) |
| 検索 / フィルタエンジン | `r3-search` | 15 | [search.md](search.md) |
| 描画 / レイアウト / プレビュー | `r3-render` | 16 | [render.md](render.md) |
| 設定 / CLI / 起動 | `r3-config` | 15 | [config.md](config.md) |
| 上流 / 依存 / ライセンス | `r3-upstream` | 15 | [upstream.md](upstream.md) |
| **合計** | 5 エージェント | **76** | — |

Round 1〜3 の累計: **220 件**の生 finding。

## 基点の変化

- ラウンド開始時に HEAD が `55a803e` → `ee03854`（`feat(tabs): restore last closed tab with Ctrl+Shift+T`）へ進んだ。
  各エージェントは現行ワーキングツリーを読んでおり、新コミットの内容もレビュー対象に含まれる。
- ラウンド中に `app/src/main.rs` へ未コミットの 8 行追加（`GPUI_FE_DEBUG_SEARCH_NAME` フック）が発生。
  これは名前フィルタを環境変数から注入するデバッグ経路の追加であり、キーハンドラの early return
  （`r2-input-01` / `r3-search-06`）を修正するものではない。第6回の妥当性再検証では現行行番号を基準にする。

## 親による事実確認（Runtime Proof）

| 主張 | 確認結果 |
|---|---|
| `r3-config-02` 保存タブが CLI 引数を上書き | `restore_session()`（`:5668-5687`）が保存タブを先に return。テスト `restore_session_prefers_saved_tabs_then_cli...` がその優先順位を固定。**確認**。README の「引数で開始ディレクトリを指定」と矛盾。 |
| `r3-search-06` Ctrl+Shift+F 到達不能 | Round 2 に続き独立レンズから再検出。**確認**。 |
| `r3-upstream-15` audit スキップ | `scripts/check.sh:90-99`。Round 1 に続き再検出。**確認**。 |
| `r3-config-04` 読取失敗時に既定値上書き | `settings.rs:176-178` + `main.rs:640`。Round 1/2 に続き再検出。**確認**。 |

## ハイライト（第6回の統合候補）

1. **CLI 引数が保存タブに黙って負ける**（`r3-config-02`、新規・高）。2回目以降の起動で引数も `.desktop %f` も死ぬ。
2. **float 検索の上限と意味論の乖離**（`r3-search-01/02/10`、新規）。NUL 判定が一致行で終了、2〜3ルートで実効上限 1000/1500、Created/Modified の stale 表示が README と矛盾。
3. **グリッドのスクロール index 誤り**（`r3-render-03`、新規・高）。uniform_list の行 index に entry index を渡す。
4. **非 UTF-8 / ダングリング symlink の data integrity**（`r3-fs-02/04/05`、新規）。`exists()` と `to_string_lossy()` が「上書き禁止」を崩す。
5. **ephemeral ガードの不完全さ**（`r3-config-09` = `r2-state-10` = `r1-sec-04`）。debug 値が起動時 save で実設定を汚染。
6. **配布の再現性の穴**（`r3-upstream-01`、新規）。build-deb.sh だけ `--locked` なし。

## 次ラウンド方針

Round 4 では **ライフサイクル / 配布 / エラーUX / モデル / 証跡** の5レンズで独立発掘する。Round 1〜3 の 220 件は重複判定の基点として round-6 で参照する。
