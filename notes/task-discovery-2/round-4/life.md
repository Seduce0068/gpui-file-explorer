---
type: Reference
title: Round 4 — ライフサイクル / 起動・終了 レビュー
description: 読み取り専用エージェント r4-life の所見（15 件）。
tags: [task-discovery, cross-review, lifecycle, gpui-file-explorer]
generated: 2026-11-18T00:12:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-life-01 | 終了時保存が DESIGN §8.1 の記載どおりに実装されていない | 構造改善 | `DESIGN.md:840`; `main.rs:5721-5726`, `:1648`, `:641` | on_window_closed は quit するだけ、Ctrl+Q も save なし。全フィールドが変更時に保存されるため即時損失はしないが契約不一致で将来の永続フィールド追加時に漏れる。 | 終了契約の集約 | S | 既知だが未対応(R6-C-15) | 高 |
| r4-life-02 | Ctrl+Q がモーダル/検索ビューで握りつぶされる | 残タスク | `main.rs:1534-1588`, `:1648` | 各 overlay と Search の early return が Escape 以外を破棄し、Ctrl+Q 判定は後ろにある。検索バー/ダイアログ表示中はキーボードで終了できない。 | 終了手段の一貫性 | S | 新規（r2-input-03 と同根だが終了経路への影響は未記載） | 高 |
| r4-life-03 | ウィンドウジオメトリが固定値で再起動時に復元されない | 機能追加案 | `main.rs:5728-5739`; `settings.rs:53-78`; `DESIGN.md:137` | `Bounds::centered(None, 1100×700)` が毎回ハードコード。Settings に bounds が無く resize/move 購読も無い。 | 作業継続性 | M | 既知だが未対応(R6-B-20) | 高 |
| r4-life-04 | 終了時に進行中のコピーがキャンセルされず `.partial-*` が残存しうる | 残タスク | `main.rs:1648`, `:5721-5726`, `:1815-1823`; `ops.rs:276-322` | Esc の cancel_op は op_cancel を立てるが Ctrl+Q/window close は cx.quit() のみ。書き込み途中 terminate で partial 削除に到達しない。 | シャットダウンの FS 整合性 | M | 新規 | 低（GPUI の quit/Task drop 挙動依存） |
| r4-life-05 | 監視スレッドの停止待ちがなく drop のみに依存 | 調査項目 | `watcher.rs:101-123`, `:172-178`; `main.rs:545-548` | DirWatcher::drop は debouncer を drop するだけで join していない。 | クリーン終了とイベント取りこぼし防止 | M | 既知だが未対応(R6-A-06) | 低（実機要検証） |
| r4-life-06 | 設定読取失敗が既定値で上書きされ元ファイルが失われる | 残タスク | `settings.rs:176-178`, `:237-243`; `main.rs:510`, `:640-641` | read_to_string 失敗時に load_error/read_only をセットせず default を返し、直後の save が元を上書き。 | 起動時の設定保護 | S | 前回と重複(r1-sec-07) | 高 |
| r4-life-07 | デバッグ用環境変数が本物の設定ファイルを汚染する | 構造改善 | `main.rs:511-543`, `:609-631`, `:640-641`; `app/README.md:409` | DEBUG_SORT/RECENT/BOOKMARKS/SEARCH_HISTORY が Settings を直接書き換え無条件 save で永続化。ephemeral ガードは tabs のみ。cfg gate も無い。 | 実設定の保護 | M | 前回と重複(r2-state-10) | 高 |
| r4-life-08 | 一時的に消えた保存タブが通常ナビゲーションで永久に失われる | 残タスク | `main.rs:5683-5688`, `:551`, `:1145`, `:1320-1321`; `settings.rs:226-233` | restore_session が is_dir() でタブを絞り、その後の persist_tabs が絞り込み後を書き戻す。再マウントしても復元不能。 | 外部ストレージ耐性 | M | 既知だが未対応(R6-B-15) | 高 |
| r4-life-09 | 保存タブが CLI 引数 / .desktop %f より優先される | 残タスク | `main.rs:5662-5667`, `:5683-5695`; `gpui-file-explorer.desktop:10`; `app/README.md:68-69` | saved tabs が 1 つ以上なら必ず先に return し cli_dir を参照しない。テストがこの優先順位を固定。 | CLI/デスクトップ連携 | S | 前回と重複(r3-config-02) | 高 |
| r4-life-10 | 非 UTF-8 の起動引数で panic し起動しない | 残タスク | `main.rs:5639`, `:5663` | handle_cli/startup_dir が std::env::args() を使用。args_os() で解消可能。 | 起動堅牢性 | S | 前回と重複(r1-port-04) | 高 |
| r4-life-11 | 複数インスタンスが同一 settings.json を無ロックで書き合う | 構造改善 | `settings.rs:102-104`, `:237-279`; `main.rs:510`, `:641` | 単一インスタンスロックも flock も無く、後書き込みが前の tabs/bookmarks/recent を上書き。 | 複数起動時の設定保護 | M | 既知だが未対応(R6-A-10/C-16) | 中 |
| r4-life-12 | 複数ウィンドウ化時に設定/タブのスコープが未定義（ADR-002） | 調査項目 | `main.rs:5721-5726`, `:5728-5746`; `DESIGN.md:95`, `:357` | main() は open_window を1回だけ。第2ウィンドウを作れば別 Explorer が同じ settings を競合更新。ADR-002 の再評価が必要。 | 設計判断の先決 | L | 既知(ドキュメント記載あり) | 低（GPUI の複数 Window サポート要確認） |
| r4-life-13 | タブの戻る/進む履歴が再起動後に失われる | 機能追加案 | `tabs.rs:46-57`; `main.rs:551`, `:1143-1146` | 保存されるのは cwd と active_tab のみ。from_paths が History を空に作る。undo スタックもメモリのみ。 | セッション復元の完全性 | M | 既知だが未対応(R6-B-15/B-14) | 高 |
| r4-life-14 | SIGTERM の「クリーン終了」がアプリ側で保証されていない | 調査項目 | `DESIGN.md:1017`; `main.rs:5708-5756`（シグナルハンドラなし） | シグナルハンドラが無く OS 既定に依存。kill -TERM 時に途中状態が残りうる。 | AC-13 の受入再定義 | M | 新規 | 低（実機要検証） |
| r4-life-15 | 設定保存失敗がログのみでユーザーに通知されない | 残タスク | `settings.rs:237-243`, `:248-279`; `main.rs:641-644` | save() は error! で stderr に出すだけ。disk full でも UI は正常動作を続ける。 | 永続化失敗の可視化 | S | 新規 | 高 |

## レンズ要約

起動は handle_cli → settings load → debug hook → session restore → watcher spawn → 初回 reload の順で、Explorer::new 途中の無条件 save()（main.rs:641）が設定汚染とタブ消失の共通原因。シャットダウンは quit() のみで、DESIGN §8.1 の終了時保存・進行中操作のキャンセル・監視スレッド join がいずれも未実装。低確度の 04/05/12/14 は GPUI/notify 挙動依存で実機確認後に優先度を決めるべき。
