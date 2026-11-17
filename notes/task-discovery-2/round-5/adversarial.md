---
type: Reference
title: Round 5 — 敵対的 / 攻撃連鎖 レビュー
description: 読み取り専用エージェント r5-adversarial の所見（13 件）。
tags: [task-discovery, cross-review, adversarial, security, gpui-file-explorer]
generated: 2026-11-18T00:24:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-adv-01 | symlink 差し替えで `remove_entry_safe` の「辿らない」保証が崩れ再帰削除が暴走 | 残タスク | `ops.rs:482-494` | symlink_metadata 直後に tree を他ディレクトリへの symlink へ付け替えると read_dir が辿り配下の実ファイルを削除。/etc への link なら passwd 等が消える。 | 任意ディレクトリ配下の削除 | S | 既知だが未対応 | 高 |
| r5-adv-02 | preview の symlink 拒否が check→open の TOCTOU で無効化 | 残タスク | `preview.rs:149-158`, `:101-116`; `main.rs:2634-2658` | 通常ファイルを /proc/self/environ 等への symlink に差し替えると内容まで読み最大 256KB/8MiB を表示。 | 任意ファイル内容の露出 | S | 既知だが未対応 | 中 |
| r5-adv-03 | `GPUI_FE_DEBUG_DROP_PATH` / `DROP_TARGET` がリリース版で任意パスをコピー | 残タスク | `main.rs:633-634`, `:5398-5412`, `:2052-2130`; `ops.rs:291-317` | 環境変数細工で起動すると初回 render が drop_external_into を呼び、/proc/self/environ（st_size=0）まで cwd へコピー。ユーザー操作ゼロ。 | 起動時のみの FS 変更と機密複製 | S | 既知だが未対応 | 高 |
| r5-adv-04 | `GPUI_FE_DEBUG_UNDO=trash` が起動時に実ファイルをゴミ箱へ送る | 残タスク | `main.rs:606`, `:5489-5492`, `:2295-2348` | 設定起動で cwd の先頭ファイルを trash::delete し undo を試みるが list 失敗/ID 不一致なら元に戻らない（結果は eprintln のみ）。 | リリース版の無操作破壊 | S | 既知だが未対応 | 高 |
| r5-adv-05 | preview タスクの同一性ガード欠如で別ファイル内容が表示される | 残タスク | `main.rs:2634-2645`, `:2659-2705` | A→B と開くと A の完了コールバックが B の preview を無条件更新。 | UI スプーフィング | S | 新規 | 高 |
| r5-adv-06 | `validate_file_name` が制御文字・双方向上書き・改行を許可 | 残タスク | `ops.rs:133-171`; `main.rs:3336-3355`, `:3367-3383` | U+202E / \n / \r / ANSI エスケープが検証を通過し status と tracing ログへ流れる。ログ偽行注入とファイル名による視覚欺瞞。 | ログ改ざん・欺瞞 | S | 新規 | 高 |
| r5-adv-07 | 悪意ある settings.json の配列が起動時に無制限読み込み＋syscall 嵐 | 残タスク | `settings.rs:51-78`, `:226-233`; `main.rs:5683`, `:669-670` | serde(default) で上限適用なし。tabs に数百万要素を入れると restore_session が各要素に is_dir() を発行。XDG_CONFIG_HOME 操作で起動ハング。 | 起動 DoS | S | 既知だが未対応 | 高 |
| r5-adv-08 | 設定パスと `/proc` を組み合わせた任意ファイル読み取り | 残タスク | `settings.rs:226-233`; `main.rs:640`, `:2634-2658`; `search.rs:210-245`, `:341-417` | last_dir="/proc/self" は is_dir を通過し cwd になる。environ/cmdline/maps が一覧化され preview が len=0 を理由に実内容を表示。 | プロセス環境変数の漏えい | M | 新規 | 中 |
| r5-adv-09 | watcher 増分適用が UI スレッドで同期 metadata を呼び無制限キューで溢れる | 構造改善 | `main.rs:545-548`, `:976-1082`（特に :1054）; `watcher.rs:102-104` | 数千ファイルの高速生成で foreground が各 Created に2 stat を同期実行。遅い FS ならフリーズし溢れたイベントは黙って捨てられる。 | UI ハング＋監視の黙っての消失 | M | 既知だが未対応 | 中 |
| r5-adv-10 | case-only rename の二段階 tmp がファイルを一覧から消す | 残タスク | `ops.rs:663-687` | 予測可能な tmp 名にディレクトリを置く/2回目前に target を生成すると途中失敗で元が隠し名のまま残る。 | ファイルの消失 | S | 新規 | 中 |
| r5-adv-11 | image preview がピクセル次元を検証せずデコーダへ渡す | 調査項目 | `preview.rs:96-135`, `:127-134`; `main.rs:2647-2658` | ファイルサイズ 8MiB のみ検査し PNG/Ico の width/height は見ない。100000x100000 宣言の小さな PNG で OOM/ハング。 | メモリ枯渇 | S〜M | 新規 | 低 |
| r5-adv-12 | `ensure_not_into_itself` 後に dest symlink を差し替え自己コピー無限再帰 | 残タスク | `ops.rs:239-252`, `:446-477`, `:394-441` | canonicalize 確認後に dest_dir を src 配下へ symlink 差し替えでコピー先が元内部になり ENOSPC まで再帰。 | ディスク/スタック枯渇 | M | 既知だが未対応 | 中 |
| r5-adv-13 | content search の symlink_metadata→open 窓で FIFO ブロック／任意ファイル走査 | 調査項目 | `search.rs:382-400`, `:298-330` | writer のいない FIFO への差し替えで read_until がブロックし背景タスクが永久終了しない。/proc への symlink は len=0 で通過し実内容を走査。 | 検索ハングと機密走査 | S | 新規 | 中 |

## レンズ要約

ローカル攻撃者が「ファイル名・symlink・settings.json・env・FS イベント」を武器にした場合、既知の TOCTOU/デバッグフック問題が削除・任意読み取り・起動時破壊操作へ昇格する新連鎖を抽出した。特に再帰削除、DROP_PATH/DEBUG_UNDO のリリース版発動、preview の世代ガード欠如はコード上確定で低工時。OS/GPUI 依存の 4 件は repro 要として信頼度を下げた。
