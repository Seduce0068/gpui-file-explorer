---
type: Reference
title: Round 3 — FS 意味論 / データ完全性 レビュー
description: 読み取り専用エージェント r3-fs の所見（15 件）。
tags: [task-discovery, cross-review, filesystem, gpui-file-explorer]
generated: 2026-11-17T23:58:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-fs-01 | ディレクトリコピーの部分失敗が宛先ツリーを残す | 残タスク | `ops.rs:402-441`, `:474`; `notes/fs-cross-platform-notes.md:250` | `copy_recursive` は作成済みの子を所有せず `copy_entry` の `?` がエラーを返すだけ。キャンセル/権限エラー後に宛先ツリーが残り、再試行は `(1)` を量産。cross-device move だけは片付けている。 | 中途半端な宛先の防止 | M | 既知(ドキュメント記載あり) | 高 |
| r3-fs-02 | ダングリング symlink が `exists()` 判定をすり抜け move がリンクを黙って破壊 | 残タスク | `ops.rs:179`, `:209`, `:229`, `:331-335`, `:612` | `Path::exists()` は symlink を追うため切れた symlink を「空き」と判定。copy は create_new が EEXIST で誤診断、move は POSIX rename で無音置換し「上書き禁止」を破る。 | 上書き禁止保証 | S | 新規 | 高 |
| r3-fs-03 | 大小文字のみ rename の 2 段階途中失敗で `.rename-*` に取り残される | 残タスク | `ops.rs:676-686` | 一時名へ移動後に target への rename が失敗すると源文件が隠し名のまま残り、ロールバックも所在通知も無い。case-insensitive FS 固有。 | ファイルの消失防止 | S | 新規 | 高 |
| r3-fs-04 | undo の rename が非 UTF-8 名を lossy 化して復元する | 残タスク | `main.rs:2371-2374`, `:2330`; `ops.rs:651` | `from.file_name()` を `to_string_lossy()` で String に落とすため、非 UTF-8 名を undo すると U+FFFD 混入名に改名され復旧不能。 | データ完全性 | S | 新規 | 高 |
| r3-fs-05 | trash ID を lossy 文字列で照合し誤復元しうる | 残タスク | `trash_view.rs:38-45`; `main.rs:2311`, `:2571`, `:3640` | 保存時も照合時も `id.to_string_lossy()` で `.find()`。非 UTF-8 ID が同じ U+FFFD に畳み込まれ無関係な項目を復元。 | ID 照合の正確性 | M | 新規 | 中 |
| r3-fs-06 | `restore_all` の部分成功がカウントと undo/redo 状態から乖離 | 残タスク | `trash_view.rs:47-51`; `main.rs:2429-2451` | `restored = to_restore.len()` は試行数。一部復元後に Err が返ると画面と実 FS が静かにずれる。 | 部分成功の正しい報告 | M | 前回と重複(R6-B-06) | 中 |
| r3-fs-07 | move の rename が TOCTOU で既存宛先を無音上書き | 残タスク | `ops.rs:604`, `:612` | 存在確認後の `fs::rename` が競合相手のファイルを原子置換し被害ファイルを unlink。 | 「never overwrite」保証 | M | round1/2と重複(r1-sec-01) | 高 |
| r3-fs-08 | バッチコピーが先頭エラー後も継続し失敗表示と実データが不一致 | 残タスク | `main.rs:1944`, `:1955`, `:1979`, `:1988-1996` | `first_error` を記録してもループは継続。offset は成功時のみ加算で進捗が逆戻りし、ユーザーが再試行して重複ツリーを作る。 | 失敗時挙動の一貫性 | S | 新規 | 高 |
| r3-fs-09 | コピー時のメタデータ劣化（時刻/xattr/ACL/owner） | 残タスク | `ops.rs:264`, `:362-368`, `:426-431`, `:612-618` | Unix で mode のみ保持し atime/mtime/xattr/ACL/owner は既定値。同一 FS move は全部保持するため「copy と cut で結果が違う」。 | メタデータ完全性 | M | 前回と重複(R6-C-12) | 高 |
| r3-fs-10 | スパースファイルのホールが実体化し進捗分母もずれる | 調査項目 | `ops.rs:264`, `:297-312`; `main.rs:1944` | ゼロ初期化バッファで read→write するためホールが実ブロック化。分母は論理サイズで実 I/O と進捗が乖離。copy_file_range/FICLONE の実測が必要。 | 効率と進捗の正確性 | L | 前回と重複(R6-A-09) | 中 |
| r3-fs-11 | 特殊ファイルが一覧では 0 B ファイル、操作では拒否 | 構造改善 | `ops.rs:437-440`, `:495-498`; `model.rs:112-115`, `:157-160` | copy/remove は FIFO/socket/デバイスを拒否するが Entry は size=0 の通常行として表示。 | 表示と操作の整合 | M | 前回と重複(R6-C-12) | 高 |
| r3-fs-12 | symlink ディレクトリの sizes/stats が一覧と矛盾 | 構造改善 | `model.rs:107-115`, `:157-160`; `ops.rs:541-544`, `:561-563` | Entry は metadata（追跡）で symlink-to-dir を「入れるフォルダ」表示、dir_stats は symlink_metadata で 0 バイトと数える。同じツリーのサイズが矛盾。 | 表示の整合性 | M | 新規 | 高 |
| r3-fs-13 | Windows の予約確保→rename 失敗時に他者のファイルを削除 | 残タスク | `ops.rs:342-350` | Windows で reservation を remove_file してから rename し、失敗時のクリーンアップが dest を無条件削除。隙間で他プロセスが作ったファイルを消す。 | 誤削除防止 | S | 新規 | 中 |
| r3-fs-14 | 設定の原子的保存で一時ファイルがリークし既存パーミッションも喪失 | 残タスク | `settings.rs:248-279`; `ops.rs:322` | `write_all`/`sync_all` の `?` で早期 return しても tmp を片付けない。create_new の tmp は既定モードで rename が既存設定を置換し 0600 が 0644 に戻る。 | データ残留とプライバシー | S | 新規 | 高 |
| r3-fs-15 | コピー中のサイズ変化を検出せず暗黙に取り込む | 残タスク | `ops.rs:264`, `:299-312`; `main.rs:1944` | read==0 で終わるだけで事前 len と実バイトを照合しない。縮小なら切詰、成長なら途中経過を完成としてコピー。 | 完全性の保証 | M | 前回と重複(R6-C-12) | 中 |

## レンズ要約

「原子的コピー」の設計主張がディレクトリ・rename intermediate・undo 経路で成立していない点（01/03/04）と、`exists()` / lossy 変換が非 UTF-8 名・ダングリング symlink でデータ破壊に直結する点（02/04/05）が中心。copy はバイト再生、move は inode 保持という非対称が全体的な弱点。
