---
type: Reference
title: Round 3 — 設定 / CLI / 起動 レビュー
description: 読み取り専用エージェント r3-config の所見（15 件）。
tags: [task-discovery, cross-review, config, cli, gpui-file-explorer]
generated: 2026-11-17T23:58:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-config-01 | 非 UTF-8 CLI 引数で起動が panic | 残タスク | `main.rs:5631`, `:5655` | `std::env::args()` を handle_cli/startup_dir の2箇所で使用。不正 UTF-8 引数で GPUI 起動前に panic。 | 起動堅牢性 | S | round1/2と重複(r1-port-04) | 高 |
| r3-config-02 | 保存済みタブが CLI ディレクトリ引数を黙って上書き | 残タスク | `main.rs:518-524`, `:5675-5680`; `app/README.md:69`; `gpui-file-explorer.desktop:10`。**親が現行コードで確認済み** | `restore_session()` は保存タブが残っていれば先に return し cli_dir を一切見ない。2回目以降の起動で引数が無視され、`Exec=... %f` 連携も死ぬ。テストもこの優先順位を固定している。 | 主要 UX / 連携 | S | 新規 | 高 |
| r3-config-03 | 未知・余分な引数が無警告で無視、`--` 終端なし | 残タスク | `main.rs:5617-5625`, `:5639-5644`, `:5653-5659` | `--version` を any() で検出し `/dir --version` も version 表示で起動しない。先頭の非 `-` 引数1つだけ採用し残りは破棄。help にフラグ未記載。 | CLI 契約 | M | 前回と重複(R6-B-19) | 高 |
| r3-config-04 | 設定ファイル読取失敗が「不存在」と同一視され既定値で上書き | 残タスク | `settings.rs:176-178`; `main.rs:639-640`。**親が確認済み** | read_to_string 失敗時に read_only なしで Self::default() を返し起動時 save が元を上書き。破損 JSON は read_only になるが読取失敗は例外。 | 設定のデータ損失 | S | round1/2と重複(r1-sec-07) | 高 |
| r3-config-05 | 配列上限が UI 挿入時のみでロード時未適用、validate は last_dir のみ | 残タスク | `settings.rs:35-39`, `:106-147`, `:179-200`, `:226-233` | MAX_* は record_recent 等で truncate されるが load_from では適用されない。細工した settings.json が大量の is_dir() を呼び診断も出ない。 | 起動時 DoS 防止 | S | 前回と重複(R6-A-11) | 高 |
| r3-config-06 | settings.json に明示的パーミッション設定なし | 残タスク | `settings.rs:255-278` | create_new + sync_all + rename のみで set_permissions が無い。umask 依存で 0644 になり得る（絶対パス履歴・検索語を含む）。 | プライバシー | S | 前回と重複(R6-A-11) | 中 |
| r3-config-07 | 複数インスタンスの同時書き込みにロックが無く lost update | 調査項目 | `settings.rs:237-279`; `main.rs:639-640`, `:1122-1128` | 全インスタンスが起動時 save。一意 tmp+rename で破損は避けるが flock/単一インスタンス判定が無く後勝ちで bookmark/tabs/recent が失われる。 | 並列安全性 | M | 新規 | 中 |
| r3-config-08 | デバッグフックが release ビルドでも有効 | 残タスク | `main.rs:594-633`; `app/README.md:456-458` | cfg gate が無く 18 種の GPUI_FE_DEBUG_* を毎回読み取り、設定すれば本番バイナリでも発火。ドキュメントは開示済みで未対応。 | リリースの健全性 | S | 既知(ドキュメント記載あり) | 高 |
| r3-config-09 | ephemeral ガードが起動時保存を通り抜け、対象外 env も多数 | 残タスク | `main.rs:511-543`, `:609-633`, `:639-640`, `:1122-1129` | ephemeral は persist_tabs/検索保存のみを保護。起動時 save は検査せず DEBUG_SORT/RECENT/BOOKMARKS/SEARCH_HISTORY を永続化。DEB 同梱 README の「保存しません」と直接矛盾。 | 実設定の汚染防止 | M | 新規 | 高 |
| r3-config-10 | dirs_first は保存されるがソートで無視される dead setting | 残タスク | `model.rs:528-549`; `app/README.md:369-370` | order() は key/ascending のみ、from_order() は常に true を書き戻す。UI から false にする導線も無い。 | スキーマ整合 | S | 既知(ドキュメント記載あり) | 高 |
| r3-config-11 | マイグレーションがスタブで、将来 version の read_only が doc 未記載 | 残タスク | `settings.rs:203-224`; `app/README.md:363-364` | doc は「警告を出して v1 として読み込み」だが future version は read_only=true + load_error で保存停止。v0/v1 は変換なしで version を正規化するだけ。 | doc 整合 | M | round1/2と重複(r1-verify-05) | 高 |
| r3-config-12 | read_only / 保存失敗の UI フィードバックが一時的・皆無 | 構造改善 | `settings.rs:70-77`, `:235-243`; `main.rs:641-643`, `:699`, `:1141` | load_error は起動時に status へ1回書くだけで後続更新で消える。save() 失敗は error! ログのみで UI に出ない。 | 診断性 | S | 新規 | 中 |
| r3-config-13 | 設定管理 UI が無く、スキーマ文書もコードと不一致 | 機能追加案 | `settings.rs:51-78`; `app/README.md:482`; `DESIGN.md:826-834` | 永続フィールドは9個で DESIGN の5項目より多い。GUI で直接編集できるのは sort/show_hidden/view のみ。 | 設定の利便性 | L | 前回と重複(R6-A-12) | 高 |
| r3-config-14 | DEB 同梱 README が陳腐化（fsync・バックアップ名・テスト数・debug 保存） | 残タスク | `packaging/linux/build/.../README.md:27,146,345,410`; `settings.rs:276`, `:150-159`; `README.md:26` | doc は「fsync なし」「broken-<epoch秒>」「132 tests」「debug 有効時は保存しない」だが実装は sync_all あり・broken-<nanos>-<PID>・167 tests・DEBUG 値を保存。ステージング成果物が古い。 | 配布ドキュメントの正確性 | S | round1/2と重複(r1-verify-06) | 高 |
| r3-config-15 | 終了時保存が無く DESIGN §8.1 と不一致 | 残タスク | `DESIGN.md:840`; `main.rs:5713-5718`, `:1647` | on_window_closed は cx.quit() のみ、Ctrl+Q も save なし。各ミューテーションで同期保存されるため即消失はしないが設計契約と外部終了経路で乖離。 | 契約整合 | S | 前回と重複(R6-C-15) | 高 |

## レンズ要約

CLI は非 UTF-8 panic・引数サイレント破棄・保存タブによる引数上書きが残り、doc との不一致が複数文書に波及。設定層はロード時バウンド未適用・読取失敗時上書き・ephemeral ガード不完全が連鎖し、デバッグ環境変数が起動時保存で実設定を汚染する点が最も新鮮なリスク。
