---
type: Reference
title: Round 2 — 状態機械 / 永続化 レビュー
description: 読み取り専用エージェント r2-state の所見（15 件）。
tags: [task-discovery, cross-review, state, gpui-file-explorer]
generated: 2026-11-17T23:50:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-state-01 | Undo(Rename) の事前チェック失敗で pop 済み項目が消滅 | 残タスク | `main.rs:2347`, `:2362-2373` | `pop_undo()` 後に早期 return し `push_undo()` で戻さない。round1 r1-correct-01 と同根。 | 履歴整合性 | S | 前回と重複(R6-B-02) | 高 |
| r2-state-02 | undo/redo が `op_task` ガード外で並走し redo の LIFO 順序が反転 | 残タスク | `main.rs:2342-2346`, `:2378-2404`, `:2450-2502`, `:2033` | 排他チェックは paste のみ。Ctrl+Z 連打で完了順に `push_redo` され順序が壊れ、新規操作の redo クリアも in-flight 完了で復活する。 | 履歴順序の保証 | M | 前回と重複(R6-B-02) | 高 |
| r2-state-03 | Redo(Trash) で `list()` 失敗時に redo 項目が両スタックから消滅 | 残タスク | `main.rs:2456`, `:2543-2563` | 再ゴミ箱化は成功しているが `trash_view::list()` が Err のとき else が無く push がされない。 | 成功操作の undo 保証 | S | 新規 | 高 |
| r2-state-04 | Trash 操作の undo 記録が `list()` 失敗／ID 不一致で黙って欠落 | 残タスク | `main.rs:3622-3634` | 移動済みでも list Err または不一致なら `push(UndoAction::Trash)` が実行されない。 | 破壊的操作の undo | S | 既知だが未対応 | 高 |
| r2-state-05 | 戻る／進むの失敗時に履歴スタックだけが遷移する | 残タスク | `main.rs:1342-1351`, `:1360-1369`; `history.rs:51-62`。**親が現行コードで確認済み** | `apply(Back/Forward)` が pop/push を先に実行し、その後 `is_dir()` 判定。消えたターゲットでも back が失われ表示と履歴が乖離。 | 履歴の永続的破損防止 | S | 新規 | 高 |
| r2-state-06 | `navigate_with_mode` が Back/Forward の戻り値を無視 | 構造改善 | `main.rs:1316-1335`; `history.rs:64-76` | `apply` は Back/Forward で引数 target を無視してスタックから地点を返すが、呼び出し側は引数 path で `set_cwd` する。現状は未発火だが API 契約違反。 | 将来のバグ予防 | S | 新規 | 中 |
| r2-state-07 | 検索バー展開中に進行中のロードが検索ビューを上書き | 残タスク | `main.rs:3650-3708`, `:660-662`, `:680-707` | `open_search` が `load_task` をキャンセルせず view 切替 only。飛んでいる旧ロードが generation 一致で `apply_loaded` し検索画面にディレクトリ一覧が出る。 | 状態遷移の整合性 | S | 新規 | 高 |
| r2-state-08 | 検索の後段フィルタが `selected` と `Selection` の不変式を破る | 残タスク | `main.rs:3854-3862`, `:1385-1390`, `:1505-1513` | `selection.retain()` で古い選択を残したまま `selected = 0`。Enter は先頭を開くが Ctrl+C は旧選択を対象にする。 | 誤操作防止 | M | 新規 | 高 |
| r2-state-09 | 新規フォルダ作成の `pending_select` がフォーカスと選択を分離する | 残タスク | `main.rs:811-844`, `:1733`, `:1505-1513` | 新規フォルダ行にフォーカスがあるのにアクション対象は旧選択ファイルという矛盾状態。 | 誤操作防止 | M | 新規 | 高 |
| r2-state-10 | デバッグ／ephemeral セッションで設定が実ファイルへ永続化される | 残タスク | `main.rs:510-543`, `:640`, `:910`, `:1308`, `:1476`, `:1692`, `:1706`; `settings.rs:237-242`。**親が :510-543/:640 を確認済み** | `debug.ephemeral` を参照するのは `persist_tabs()` と検索履歴保存のみ。起動時保存(640)・ブックマーク・ナビ・表示/ソートは無条件 `save()` で debug 注入値が settings.json を汚染。 | 実設定のデータロス防止 | M | 新規 | 高 |
| r2-state-11 | `validate()` が `last_dir` のみで他パス配列を検証しない | 残タスク | `settings.rs:197-200`, `:226-233`; `main.rs:888-897`。**親が確認済み** | tabs/recent_dirs/bookmarks はメモリ上は prune されるが設定ファイルは更新されず、次の無関係な `save()` まで残存。 | 設定の整合性 | S | round1と重複(r1-correct-13) | 高 |
| r2-state-12 | 抑制窓・操作実行中の監視イベントが破棄され再取得されない | 構造改善 | `main.rs:981-983`, `:1104-1111`; `README.md:415-416` | watch loop が `is_suppressed()` / `op_cancel.is_some()` のときバッチを `return` で捨てる。長い paste 中は全イベントを落とし最大 10 秒の乖離。 | 監視の完全性 | S | 既知(ドキュメント記載あり) | 高 |
| r2-state-13 | ディレクトリ切替後のキュー残留イベントに cwd/世代ガードが無い | 残タスク | `main.rs:974-980`, `:1028-1034`, `:1040-1045`, `:854-871` | 旧ディレクトリの `Removed` が親検査なしで新 cwd に適用、`Rescan/Renamed` は無条件 reload。Created/Modified だけ parent 検査があり不均一。 | 誤更新防止 | M | 前回と重複(R6-A-05) | 高 |
| r2-state-14 | cwd/history/tabs の三重保持と手動 `sync_active_tab()` | 構造改善 | `main.rs:437`, `:454`, `:474-475`, `:1116-1129` ほか;`tabs.rs:11-14` | 同じ論理状態がExplorerとtabsに二重保持され8箇所で手動同期。各タブの history/選択/スクロールは復元されず同期漏れで表示と設定が食い違う。 | 構造改善 | M | 既知(ドキュメント記載あり) | 高 |
| r2-state-15 | undo スタックがメモリのみで再起動時に消える | 残タスク | `undo.rs:51-56`, `:64-93`; `main.rs:498-499`, `:590` | `UndoStack` は Vec 2本だけで Settings にフィールドが無く、終了時保存も無い。 | 履歴の永続化 | M | 前回と重複(R6-B-14) | 高 |

## レンズ要約

状態遷移の多くは「pop/更新を先に、検証・記録を後で」という順序のため失敗経路でスタックと表示が回復不能に乖離する。並走ガードが paste の `op_task` に偏り undo/redo・検索・監視の in-flight 処理が状態を上書きする。ephemeral フラグも保存経路の一部にしか効いていない。
