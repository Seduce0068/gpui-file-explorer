---
type: Reference
title: Round 1 — 正確性 / ロジックバグ レビュー
description: 読み取り専用エージェント r1-correct の所見（13 件）。
tags: [task-discovery, cross-review, correctness, gpui-file-explorer]
generated: 2026-11-17T23:25:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-correct-01 | Undo(rename) の事前チェック早期 return で pop 済み Undo 項目が消失 | 残タスク | `main.rs:2347`, `:2362-2372`, `:2377` | `pop_undo()` 後、`!current.exists()` / `target.exists()` の early return で `push_undo` がなく、`action_for_redo` も 2377 でしか生成されない。失敗時に操作がスタックから黙って消え再試行不可。 | Undo の再利用性と履歴整合性が回復 | S | 前回と重複(R6-B-02) | 高 |
| r1-correct-02 | Redo(trash) 成功時の Undo 記録が `trash_view::list()` 成功に依存 | 残タスク | `main.rs:2548-2563`, `:2564-2572` | `restore_all` は成功しているのに `list()` が Err だと `push_undo` が実行されず、ステータスは「やり直しました」のまま。Redo 操作だけが Undo 不能。 | 成功操作の Undo 欠落を防止 | S | 前回と重複(R6-B-02) | 高 |
| r1-correct-03 | ゴミ箱移動成功時の Undo 記録が list()/ID 検索成功に依存 | 残タスク | `main.rs:3621-3634` | `trash::delete` 成功後も `trash_view::list()` 失敗または `original_path()` 不一致なら `undo.push` がスキップされ、移動したファイルを取り消せない。 | データ復帰経路の保証 | S | 前回と重複(R6-B-02) | 中 |
| r1-correct-04 | go_back/go_forward が存在確認前に履歴を破壊し消えた移動先を消費 | 残タスク | `main.rs:1342-1351`, `:1360-1369`; `history.rs:51-62` | `history.apply` が先に back/forward を pop/push し、その後 `is_dir()` 失敗でエラー表示するだけ。履歴スタックが不正な状態のまま再試行できない。 | 履歴ナビの永続的破損を防止 | S | 新規 | 中 |
| r1-correct-05 | 不正パス時の LoadState::Error が generation を上げず後続 reload に上書きされる | 残タスク | `main.rs:1325-1332`, `:683-685`, `:1742-1747` | `navigate_with_mode` の失敗経路は現在 generation で Error をセットするだけで `load_task` も止めない。進行中の reload コールバックが generation 一致で Error を Ready に上書きし古い一覧が復活する。 | エラー状態の誤消去を防止 | S | 新規 | 中 |
| r1-correct-06 | rebuild_view のフォーカス行と selection が不一致し操作対象が食い違う | 残タスク | `main.rs:811-849`, `:1505-1517` | `keep`(pending_select) が `selected` 行を決める一方 `selection.retain` は旧選択を残し、`selection.is_empty()` が false だと new path を選択しない。`action_paths()` は selection を使うためハイライト行と操作対象が別。 | 誤ファイル操作の防止 | M | 前回と重複(R6-C-07) | 中 |
| r1-correct-07 | apply_search_filter が selected を無条件 0 に戻し選択とフォーカスが乖離 | 残タスク | `main.rs:3837-3858` | フィルタ変更時に `selection.retain` は別行を保持するのに `self.selected = 0`。キー操作は先頭行、選択ハイライトは別行という不一致。 | 検索結果操作の一貫性 | S | 前回と重複(R6-C-07) | 中 |
| r1-correct-08 | merge_search_results のグローバル上限が最終ソート前に適用され不公平 | 残タスク | `search.rs:257-268` | 打ち切り判定をバッチ走査中に行ってから `sort_by` するため、複数ルートでは先に処理したルートの結果が優先され、辞書順で小さい後続ルートのヒットがドロップされる。**親が現行コードで確認済み。** | 検索結果の完全性・予測可能性 | S | 新規 | 中 |
| r1-correct-09 | restore_by_id が部分成功を「0 件」と報告し Undo を誤って再積み | 残タスク | `trash_view.rs:47-51`; `main.rs:2409-2436` | `restored = to_restore.len()` を `restore_all` 前に確定し、`restore_all` が一部成功後に Err を返すと呼び出し側は全失敗と扱い Undo を戻す。実際には一部が復元済み。 | 部分成功の正しい報告 | M | 前回と重複(R6-B-06) | 中 |
| r1-correct-10 | Redo(trash) の Undo 項目が list() で見つかった一部のみで再構築される | 残タスク | `main.rs:2549-2562` | `recorded` は `items.iter().find(...)` に失敗した pusht 済みパスを落とす。後で Undo しても一部しか復元されない不完全な項目が積まれる。 | Undo の完全性 | S | 新規 | 中 |
| r1-correct-11 | ops の非 Unix 向けテストが Unsupported を無視して成功を前提 | 残タスク | `ops.rs:377-392`, `:819-831` | `copy_symlink` は非 Unix で `Err(Unsupported)` を返すのにテストは `copy_entry(...).unwrap()` 後に `files == 2` を期待。Windows では panic する誤ったアサーション。 | クロスプラットフォーム検証の正確性 | S | 新規 | 低 |
| r1-correct-12 | search_contents のバイナリ判定が一致行より後を走査しない | 残タスク | `search.rs:298-330` | NUL は現在行のみ検査し、一致が見つかった時点で return するため「1 行目に TARGET、2 行目以降に NUL」はテキスト一致と報告される。doc の「read 範囲内の NUL でバイナリ」と矛盾。 | 検索結果の正当性 | S | 新規 | 低 |
| r1-correct-13 | Settings::validate が last_dir のみ検証し refresh_* の変更が保存されない | 残タスク | `settings.rs:226-233`; `main.rs:668-669`, `:887-897` | `validate` は last_dir しか見ず、reload 内の `recent_dirs`/`bookmarks` retain はメモリのみで `save()` されない。消えたパスが設定ファイルに残り起動毎に繰り返す。**親が現行コードで確認済み（settings.rs:226-233）。** | 設定の整合性 | S | 新規 | 低 |

## レンズ要約

最も確実なのは Undo スタックの早期 return / list() 依存による項目消失（01〜03）で、いずれも前回 R6-B-02 の未対応箇所。次に履歴・エラー世代・フォーカス選択の状態遷移不整合（04〜07）を挙げ、検索マージ上限・部分成功報告・テスト誤記を補足した。中/低置信度のものは再現条件（in-flight load、trash list 失敗、Windows）に依存する。
