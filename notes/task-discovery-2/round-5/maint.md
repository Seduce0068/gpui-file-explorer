---
type: Reference
title: Round 5 — 保守性 / 開発体験 / 負債トリアージ レビュー
description: 読み取り専用エージェント r5-maint の所見（15 件・上位3件のトリアージ付き）。
tags: [task-discovery, cross-review, maintenance, gpui-file-explorer]
generated: 2026-11-18T00:24:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-maint-01 | 6077行 main.rs の状態抽出が未完（ui/* 分割の先送り） | 構造改善 | `main.rs:435`, `:10-22`; `app/README.md:481`; `DESIGN.md:1065` | Explorer が god object。まず純データを ExplorerState へ抽出しテストを移し、次に render 系を ui/ へ段階移動。状態系バグの修正面を減らす最大レバレッジ。 | 高 | L | 既知(ドキュメント記載あり) | 高 |
| r5-maint-02 | キー判定が未テストの巨大 if チェーン | 構造改善 | `main.rs:1534-1703` | 約170行の分岐が一度もテストされない。classify_key(event, view) -> KeyAction を純関数へ切り出しデータ駆動検証。 | 高 | M | 前回と重複(r1-qual-02) | 高 |
| r5-maint-03 | 47個の GPUI_FE_DEBUG_* がリリースビルドに残存 | 構造改善 | `main.rs:376-402`, `:511-634`, `:5423-5464`, `:5509-5515` | cfg(debug_assertions) は0件。apply_startup_hooks と render 経路から実 FS 操作が発火し得る。**注: 第5回メタレンズで「実体は29種類」と訂正済み（r5-meta-04）**。 | 高 | M | round1-4と重複(r1-sec-03) | 高 |
| r5-maint-04 | 「render の外」というコメントが実コードと矛盾 | 残タスク | `main.rs:5352`, `:5510`, `:5515` | doc comment は "Kept out of render()" と書くが render() から直接呼ばれる。コメント修正またはイベント駆動化。 | 中 | S | 前回と重複(r3-render-05) | 高 |
| r5-maint-05 | unsafe 禁止が全クレート・CI で強制されていない | 構造改善 | `poc/src/main.rs:1-18`; `scripts/check.sh:72-73`, `:86-87`; `ci.yml:39-43` | PoC ルートに forbid が無く clippy は -D warnings のみで undocumented_unsafe_blocks が無い。 | 中 | S | round1-4と重複(r1-sec-08) | 高 |
| r5-maint-06 | エラー型が FsError/OpError/String に分裂 | 構造改善 | `model.rs:19-29`; `ops.rs:66`; `trash_view.rs:25`, `:34`; `preview.rs:100`, `:148` | 共通 AppError を置き From で包み、tracing::warn! を一箇所で保証。 | 中 | M | 既知(ドキュメント記載あり) | 中 |
| r5-maint-07 | 一時ディレクトリ付きテストヘルパーが8箇所重複 | 構造改善 | `model.rs:738-739`; `settings.rs:292`; `ops.rs:699`; `preview.rs:181`; `search.rs:427`; `watcher.rs:191`; `main.rs:6042`; `poc/src/main.rs:422-423` | test_support::scratch_dir(tag) を RAII ガード付きで1つ用意し置換。 | 中 | S | 新規 | 高 |
| r5-maint-08 | strings.rs が存在しない DESIGN §8.5 を参照 | 残タスク | `strings.rs:2`; `DESIGN.md:819-865` | doc comment が §8.5 を指すが第8章は 8.1〜8.4 のみ。 | 低 | S | 新規 | 高 |
| r5-maint-09 | DESIGN の型名 ExplorerApp/DirListing/app.rs が実コードに無い | 構造改善 | `DESIGN.md:356-358`, `:720`, `:1056`; `main.rs:10-22`, `:435` | 設計書からコードへ辿れない。as-built 差分表に「設計名→実装名」対応表を1枚追加。 | 中 | S | 新規 | 高 |
| r5-maint-10 | ダイアログ5種が個別 inline でフォーカス復帰も不均一 | 構造改善 | `main.rs:2136`, `:2715`, `:2942`, `:3168`, `:3428`, `:5509-5517` | active_dialog 列挙と共通 Modal（backdrop・Esc・focus restore）を1つ作り5つの bool を置換。 | 高 | L | 前回と重複(R6-C-07) | 中 |
| r5-maint-11 | 正本 todo.md が「残タスク零」のまま（55件未反映） | 構造改善 | `todo.md:15-31`; `notes/task-discovery-2/round-4/proof.md:17` | 第6回の最終操作で必ず反映する。高6件を優先度:高、中30件を中へ移し R6 ID を添える。 | 高 | M | round1-4と重複(r4-proof-02) | 高 |
| r5-maint-12 | DESIGN §11 の未確認17項目が更新・割当てられず陳腐化 | 構造改善 | `DESIGN.md:1090-1108`, `:1132` | 表に 状態/担当/根拠 列を追加し決着済みを閉じ、未検証のみ todo の調査項目へ移送。 | 中 | S | round1-4と重複(r4-proof-11) | 高 |
| r5-maint-13 | プロジェクトに AGENTS.md が無く規約がローカル外部依存 | 構造改善 | (glob: プロジェクト内に AGENTS.md 無し); `README.md:124-136` | clone だけした貢献者は規約を知れない。最小のプロジェクト AGENTS.md を置く。 | 中 | S | 新規 | 中 |
| r5-maint-14 | cargo audit が app/ のみで poc/ のロックファイルが未監査 | 構造改善 | `scripts/check.sh:90-97`; `ci.yml:35`; `poc/Cargo.toml:8` | 両方独立 Cargo プロジェクトなのに audit は app のみ。check.sh に (cd poc && cargo audit) を追加。 | 中 | S | 新規 | 中 |
| r5-maint-15 | feature flag が無く WASM 向けビルドが手作業手順に依存 | 調査項目 | `app/Cargo.toml:13-38`; `README.md:96-104` | [features] が無く README は手編集を前提。desktop/wasm feature を追加し wasm32 check をゲート化。 | 低 | S | 既知(ドキュメント記載あり) | 中 |

## レンズ要約（負債トリアージ）

ボトルネックは単一ファイルへの状態集約。着順は ①todo.md への55件反映（優先順位の真実源を復旧）→ ②DebugHooks の cfg ゲート化（リリース経路と render を同時に単純化し #4 も解消）→ ③ExplorerState 抽出（#6/#10 を含む状態系バグの修正面を一挙に縮小）。
