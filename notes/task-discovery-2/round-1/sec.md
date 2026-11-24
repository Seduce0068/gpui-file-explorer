---
type: Reference
title: Round 1 — セキュリティ / プライバシー レビュー
description: 読み取り専用エージェント r1-sec の所見（15 件）。
tags: [task-discovery, cross-review, security, gpui-file-explorer]
generated: 2026-11-17T23:25:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-sec-01 | 同一デバイス移動の TOCTOU で既存ファイルが無音上書きされる | 残タスク | `ops.rs:604-612` | `resolve_destination` の存在確認後に `fs::rename(src,&dest)` を呼ぶため、競合相手が dest を作ると POSIX rename が既存ファイルを置換する。copy の O_EXCL 予約は move 経路に及ばない。**親が現行コードで確認済み。** | 「上書き禁止」保証を同デバイス移動でも維持 | M | 新規 | 高 |
| r1-sec-02 | 再帰削除の symlink 差し替え TOCTOU | 残タスク | `ops.rs:482-493` | `symlink_metadata` でディレクトリ判定後 `read_dir` までに子を symlink へ差し替えると、リンク先ツリーを再帰削除しうる。 | 権限範囲外ツリーの誤削除を遮断 | M | 前回と重複(R6-C-13) | 中 |
| r1-sec-03 | デバッグ環境変数フックがリリースビルドで実ファイル操作を発火 | 残タスク | `main.rs:594-633`, `:5377-5391`, `:5461-5464` | `GPUI_FE_DEBUG_DROP_PATH` は任意パスをコピーし、`DEBUG_TRASH_RESTORE` / `DEBUG_UNDO` は実操作を実行。`cfg(debug_assertions)` は 0 件。 | リリース時の環境変数トリガ操作を排除 | S | 前回と重複(R6-C-10) | 高 |
| r1-sec-04 | デバッグフックが実設定（ブックマーク/検索履歴/ソート）を起動時に上書き | 残タスク | `main.rs:511-543`, `:639-640`, `app/README.md:407` | フック有効時に `settings.bookmarks.clear()` 等を行った後、起動直後の `save()` は `debug.ephemeral` を検査しない（persist_tabs と検索時のみ検査）。実設定が恒久破壊される。 | ブックマーク・検索履歴のデータロス防止 | S | 新規 | 高 |
| r1-sec-05 | 設定 JSON の配列サイズが読み込み時に無制限 | 残タスク | `settings.rs:51-69`, `:176-200`, `:227-233`, `tabs.rs:43-49` | `MAX_*` は UI 追加時のみ適用で `validate()` は last_dir しか見ない。細工した settings.json の巨大配列と無制限 `read_to_string` が起動時メモリ/CPU 枯渇を招く。 | 細工設定による起動時 DoS を防止 | S | 前回と重複(R6-A-11) | 高 |
| r1-sec-06 | 設定一時ファイルが既定パーミッションで生成されパス履歴が漏洩 | 残タスク | `settings.rs:255-278` | `create_new(true)` だがモード指定が無く umask 依存（通常 0644）。`last_dir`/bookmarks/tabs/検索語が同一ホストの他ユーザーから読める。 | プライベートパス履歴のローカル漏洩を防止 | S | 前回と重複(R6-A-11) | 中 |
| r1-sec-07 | 設定読み取りエラーが既定値フォールバック後そのまま上書き | 残タスク | `settings.rs:176-178`, `main.rs:639-640` | EACCES 等で `read_to_string` 失敗時に `Self::default()` を返し、直後の `save()` が rename で元ファイルを破壊する（corrupt-JSON 経路とは別経路）。 | 既存設定の無音消失を防止 | S | 既知だが未対応 | 高 |
| r1-sec-08 | PoC クレートに `forbid(unsafe_code)` が無く CI も unsafe リンター未指定 | 残タスク | `poc/src/main.rs:1-14`, `app/src/main.rs:8`, `poc/build.rs:19`, `ci.yml:43`, `check.sh:87` | app 本体/build.rs と poc/build.rs には宣言があるが PoC バイナリのクレートルートには無い。**親が現行コードで確認済み。** CI は `-D warnings` のみで AGENTS.md §15 の `-D clippy::undocumented_unsafe_blocks` が無い。 | unsafe ポリシーを全クレートで強制 | S | 新規 | 高 |
| r1-sec-09 | プレビュー/内容検索の symlink チェックと open の TOCTOU | 残タスク | `preview.rs:101-118`, `:148-160`, `search.rs:382-400` | `symlink_metadata` で正規ファイル確認後に `File::open` までに FIFO/symlink へ差し替え可能。FIFO なら `read_to_end` が writer 待ちでハング。 | プレビュー/検索経由のハング・リンク先読み取りを防止 | M | 前回と重複(R6-C-13) | 中 |
| r1-sec-10 | undo 事前チェック失敗時にエントリが黙って消失 | 残タスク | `main.rs:2347`, `:2362-2373` | `pop_undo()` 後に対象不在/同名衝突で早期 return し `push_undo` していない。 | undo の再試行可能性を保持 | S | 前回と重複(R6-B-02) | 高 |
| r1-sec-11 | 異常終了時の `.partial-*` 残存と起動時クリーンアップ欠如 | 残タスク | `ops.rs:272-284`, `:319-325` | 通常エラー時は削除するがプロセス強制終了時はコピー済み断片が隠しファイルとして残る。起動時の掃除処理は存在しない。 | データ残留と disk 圧迫を解消 | S | 既知(ドキュメント記載あり) | 高 |
| r1-sec-12 | 監視イベント用 unbounded チャンネルに背圧が無い | 残タスク | `main.rs:545`, `:975-979` | `try_send` は失敗せず、連続 FS 変更で `Vec<FsEvent>` が際限なく蓄積しメモリ枯渇に至る。 | イベント氾濫による DoS を防止 | M | 前回と重複(R6-A-08) | 中 |
| r1-sec-13 | 非 UTF-8 の CLI 引数で起動時に panic | 残タスク | `main.rs:5618`, `:5642` | `std::env::args()` は不正 Unicode 引数で panic する。ファイルマネージャーから非 UTF-8 名を渡すだけで起動不能。 | 悪意あるファイル名での起動拒否を回避 | S | 新規 | 高 |
| r1-sec-14 | ファイル名の制御文字/双方向制御文字が確認ダイアログを偽装 | 残タスク | `main.rs:3537-3554`, `model.rs:102-103` | ゴミ箱確認は `to_string_lossy()` した file_name のみを表示し、U+202E 等の bidi override や改行を含むファイル名で表示名と実体をずらせる。 | 破壊操作の確認なりすましを防止 | S | 新規 | 中 |
| r1-sec-15 | 個人識別子とファイルパスがドキュメント・スクリーンショット・ログに露出 | 残タスク | `settings.rs:9`, `DESIGN.md:119`, `notes/poc-report.md:216-224`, `app/screenshot/properties-dialog.png`, `main.rs:2302-2306` | `/home/testuser/...`、ユーザー名 `testuser` が公開ドキュメント/証跡画像に残存。さらに eprintln と tracing イベントが生パスを journald へ書く。 | 公開リポジトリ・ログ経由のプライバシー漏洩を低減 | S | 新規 | 高 |

## レンズ要約

ファイル操作の「上書き禁止・symlink 非追跡」は設計意図どおりだが、`move` の rename 競合と再帰削除の TOCTOU が保証の実効性を削っている。config 信頼境界（上限・権限・読取失敗）とリリース残存デバッグフックが最大の残リスク。undo 消失・一時ファイル残存・パス露出は低工数で塞げる。
