---
type: Reference
title: Round 2 — 非同期 / スレッド / キャンセル レビュー
description: 読み取り専用エージェント r2-async の所見（14 件）。
tags: [task-discovery, cross-review, async, gpui-file-explorer]
generated: 2026-11-17T23:50:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-async-01 | ゴミ箱完了コールバックが UI スレッドで `trash::list()` を同期実行 | 残タスク | `main.rs:3622`, `:2548`, `trash_view.rs:17-19`, `DESIGN.md:394` | 完了ハンドラが `this.update(...)` 内（前景）でブロッキング I/O を呼び、DESIGN §4.4 に明示的に反する。 | フリーズ防止 | S | 新規 | 高 |
| r2-async-02 | reload / open_properties が UI スレッドで metadata と `/proc/mounts` を同期読み | 残タスク | `main.rs:666-670`, `:875-897`, `:921-923`, `:3033-3075`, `mounts.rs:140` | `refresh_places/mounts/recent/bookmarks` が同期 `is_dir()` と `read_to_string("/proc/mounts")`。NFS/FUSE でフレーム停止。 | スレッド契約の順守 | M | 新規 | 高 |
| r2-async-03 | 監視ポンプがロード進行中に `all_entries` を直接更新し完了スナップショットが上書き | 残タスク | `main.rs:974-982`, `:1036-1067`, `:679-689`, `:811-818`, `:954` | watch ループは `load.is_loading()` を見ない（reconcile は 954 で見る）。作成行が read_entries 完了で消え 10 秒 reconcile まで復帰しない。 | 監視の完全性 | S | 新規 | 中高 |
| r2-async-04 | reconcile が Task ハンドルを保持せず並走し古いスナップショットが採用されうる | 残タスク | `main.rs:928-965`, `:1091-1099`, `:954-960` | 10 秒 tick で前回読み込みが残ったまま次回が起動し、到着順が逆なら古い結果が新状態を抹消。 | 結果の鮮度保証 | S | 既知(ドキュメント記載あり) | 中高 |
| r2-async-05 | `total_bytes` へのキャンセル未伝播と進捗チャネルの背圧ゼロ | 残タスク | `main.rs:1931`, `:1893`, `:1932`, `:1943-1946`, `ops.rs:506-508`, `:557-577` | `total_bytes` は CancelFlag を引かず、`dir_stats` は確認しているのに非対称。unbounded チャンネルに try_send が無制限蓄積。 | キャンセル応答性 | M | 前回と重複(R6-A-08) | 高 |
| r2-async-06 | Esc が paste 以外の長時間操作をキャンセルできない | 残タスク | `main.rs:1802-1809`, `:1887-1888`, `:3511-3645`, `:2341-2589` | `op_cancel` は `run_transfer` のみ。trash/undo/redo/rename は flag も Task も持たず Esc 後も完了まで走る。 | キャンセル一貫性 | M | 既知(ドキュメント記載あり) | 高 |
| r2-async-07 | trash 確認経路が `op_task` / `op_cancel` ガードを迂回 | 残タスク | `main.rs:3564-3644`, `:1823`, `:2060`, `:2342`, `:981`, `:3597` | 削除中に Ctrl+V で並走し両完了コールバックが all_entries/reload/generation を競更新。watch 抑制も削除完了後まで効かない。 | 競合状態の排除 | M | 既知(ドキュメント記載あり) | 中高 |
| r2-async-08 | undo/redo が `op_task` を設定せず「操作中」ガードが空転 | 構造改善 | `main.rs:2342`, `:2451`, `:2378`, `:2412`, `:2033` | 開始時に `op_task.is_some()` を確認するが自分で設定しない。Ctrl+Z 連打で背景処理が並走しスタック順序が壊れる。 | 排他制御の正常化 | M | 既知(ドキュメント記載あり) | 高 |
| r2-async-09 | 完了ハンドラが `op_task` / `op_cancel` / `op_progress` を無条件クリア | 残タスク | `main.rs:1991-1993`, `:1888`, `:2033` | paste 完了が自操作判定なしで3状態を None にし、trash と並走するとセンチネルを消す。 | 完了判定の正確性 | M | 前回と重複(R6-C-09) | 中 |
| r2-async-10 | 失敗コールバックに cwd/世代ガードがなく別ディレクトリの LoadState を汚す | 残タスク | `main.rs:1742-1746`, `:3585-3595`, `:1325-1330`, `:683` | `new_folder` は成功時のみ cwd 確認、失敗時は無条件 Error を書く。A で操作開始→B へ移動→失敗コールバックが B をエラー固定に。 | エラー状態の正確性 | S | 既知(ドキュメント記載あり) | 高 |
| r2-async-11 | 進捗が単調性/100% を保証せず完了前に残メッセージが捨てられる | 残タスク | `main.rs:1898-1907`, `:1911`, `:1942-1946`, `:1966`, `:1991-1993` | offset は成功 bytes のみ加算し部分失敗後に done が後退しうる。完了ハンドラが op_cancel を先にクリアし最終進捗も捨てる。 | 進捗表示の正確性 | S | 前回と重複(R6-A-08) | 中 |
| r2-async-12 | リネーム完了コールバックにダイアログ同一性がなく別ファイルのダイアログを閉じる | 残タスク | `main.rs:3343-3382`, `:3352`, `:3371`, `:3376`, `:3385-3396` | A のリネーム確定→応答待ちに Esc→B を開く→A のコールバックが B を閉じる/エラー表示。同一性トークンと Task 保持が必要。 | 誤クローズ防止 | S | 新規 | 高 |
| r2-async-13 | ディレクトリサイズ結果にダイアログ同一性トークンがない | 調査項目 | `main.rs:3119-3150`, `:3137-3145`, `:3076-3089` | コールバックが現在の properties に無条件書き込み。GPUI の背景 Task drop セマンティクスを確認し、不足なら生成 ID を捕捉する。 | 結果の混入防止 | S | 既知(ドキュメント記載あり) | 低中 |
| r2-async-14 | 監視スレッドの終了待ちがなく teardown 時に残イベントがサイレント破棄 | 調査項目 | `watcher.rs:172-178`, `main.rs:545-548`, `:974-975`, `:1083-1085` | `DirWatcher::drop` は debouncer を drop するだけで stop/join せず、unbounded try_send 失敗も無視。 | シャットダウン順序の検証 | M | 前回と重複(R6-A-06) | 中 |

## レンズ要約

検索の3層ガード（共同 cancel / Task drop / 世代照合）は模範的だが、reconcile・監視・ゴミ箱・undo/redo・リネーム・新規フォルダには波及していない。未ガードの detach コールバックが all_entries / LoadState / undo スタックを競更新する。DESIGN §4.4 の UI スレッド I/O 禁止が trash::list / metadata / /proc/mounts で直接破られており、ここが最も確度の高い改修点。
