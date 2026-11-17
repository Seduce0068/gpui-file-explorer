---
type: Reference
title: "第4ラウンド ファイルシステム意味論エッジケースのタスク発見"
description: "gpui-file-explorer の copy/move/rename/trash/stat 経路で失われるメタデータと特殊ファイル扱いの調査"
tags:
  - task-discovery
  - filesystem
  - metadata
  - rust
  - gpui
generated:
  by: "r4-fs"
  at: "2026-11-17T21:01:08+09:00"
status: complete
---

## 範囲と出典

`app/src/ops.rs`（モジュール決定 2-21 行、copy 256-478 行、remove/total_bytes 480-577 行、
move/rename 579-690 行）、`app/src/model.rs`（Entry 77-127 行、entry_for_path 144-171 行、
kind_label 419-443 行）、`app/src/main.rs`（プロパティ 2977-3207 行）、
`notes/fs-cross-platform-notes.md` §5（241-289 行）・§10（417-440 行）・§14（530-548 行）を
実読して根拠とした。全項目でソース改変はしていない。`std::fs` の既定挙動に依存し
ローカル実行で検証できなかった箇所は「未確認」と明記し、調査項目として提起する。

## 検出結果

### r4-fs-hardlink — ハードリンクが独立ファイル化し inode/リンク数が見えない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r4-fs-hardlink | ハードリンク独立化 | 機能追加案 | ops.rs:292-296; model.rs:79-91 | 新 inode 化、nlink 非表示 → 保持/表示 | 中 | 中 | 高 | 高 |

根拠: `copy_file_atomic` は `.partial` を `create_new` で作り `rename` するため
（ops.rs:292-296, 331-350）、元が同一 inode のハードリンクでもコピー先は別 inode になる。
保存するのは mode のみ（ops.rs:362-368）で、`nlink`/`ino` は `Entry`（model.rs:79-91）にも
プロパティ（main.rs:2983-3204）にも存在しない。Unix で `nlink > 1` を検出して
ハードリンク関係を再構成するか、最低限 inode/リンク数を表示するタスク。
ディスク使用量が倍増する点は VM/バックアップ用途で影響が大きい。

### r4-fs-sparse — スパースファイルのホールがバイト単位コピーで実体化する

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r4-fs-sparse | スパース展開 | 調査項目 | ops.rs:297-312; fs-notes:252 | 0 埋め read/write → 保持手法の調査と実測 | 中 | 高 | 高 | 中（未確認） |

根拠: コピーは 64 KiB のゼロ初期化バッファで `read`→`write_all` を繰り返す
（ops.rs:297-312, COPY_CHUNK_SIZE=29 行）。ホールから読んだ 0 が実ブロックとして
書き出される可能性が高く、進捗分母は `metadata.len()`（ops.rs:506-577）なので
実 I/O 量とずれる。設計ノートもスパース保持を「要検証」（fs-notes:252）、
Windows の `std::fs::copy` 保持を未確認（fs-notes:541）とする。
`SEEK_HOLE`/`copy_file_range`/`FICLONE` 等の既存クレートを事前調査し、
スパース fixture で確保ブロック数を比較するタスク。実挙動は未確認。

### r4-fs-xattr — xattr / ACL / セキュリティコンテキストがコピーで失われる

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r4-fs-xattr | xattr/ACL 喪失 | 機能追加案 | ops.rs:362-368 | mode のみ、xattr/ACL なし → best-effort 複製 | 高 | 高 | 高 | 高（未確認） |

根拠: ファイルもディレクトリも `PermissionsExt::mode()` → `set_permissions` のみ
（ops.rs:362-368, 426-431）。`setxattr`/ACL/`chown` を扱うコードも依存も存在しない
（`app/Cargo.toml` と `app/src/` 全文を grep で確認）。
`user.*` xattr、POSIX ACL、SELinux/AppArmor ラベル、Windows ACL/ADS は
FS 既定値に置き換わる（具体的な継承結果は未確認）。プラットフォーム別モジュールで
best-effort 複製し、部分失敗を明示警告にするタスク。バックアップ/復元用途で価値が高い。

### r4-fs-timestamp — コピー時に mtime/atime/birthtime が保存されない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r4-fs-timestamp | 時刻未保存 | 残タスク | ops.rs:264,362-368; fs-notes:246 | mode にのみ利用 → atime/mtime 保存 | 高 | 中 | 中 | 高 |

根拠: Unix では `fs::metadata(src)` を取得するが（ops.rs:264）、
利用箇所は mode の複製（ops.rs:362-368）だけ。`filetime` 依存は無く、
設計ノートは「任意で `filetime` による mtime 維持（未検証）」とだけ記す（fs-notes:246）。
結果としてコピー先は新規の atime/mtime/birthtime を持ち、同一 FS の `rename` 移動
（ops.rs:612-618）では保持されるという非対称が生じる。ここで扱うのは**表示追加ではなく
コピー時の保存**であり、pass-1 の作成/アクセス時刻表示提案とは別。
既存クレート調査の上で atime/mtime 保存を実装し、birthtime は std の制約を調査する。

### r4-fs-special — FIFO/socket/デバイスファイルの扱いが経路ごとに矛盾

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r4-fs-special | 特殊ファイル不整合 | 構造改善 | ops.rs:437-440; model.rs:420-433 | 一覧はファイル 0 B、操作は失敗 → 種別共通化 | 中 | 中 | 高 | 高 |

根拠: `copy_recursive`（ops.rs:437-440）と `remove_entry_safe`（ops.rs:495-498）は
特殊ファイルを `OpError::Unsupported` で拒否するが、`dir_stats` は
symlink/dir/file 以外を数えず（ops.rs:541-551）、`total_bytes_of` は 0 を返す
（ops.rs:557-576）。一覧側は `fs::metadata`（model.rs:112-115, 157-160）の
`is_file() == false` を区別せず `Entry` を作り、`kind_label` は「—」を返す
（model.rs:420-433）。FIFO/socket/ブロック・キャラクタデバイスを
一般ファイル 0 バイトとして見せた後に操作だけ失敗するのは利用者を惑わす。
ファイル種別を `Entry`/`DirStats` に追加し、エラー文に種別を出すタスク。

### r4-fs-size-change — コピー中のサイズ変化を検出せず暗黙に取り込む

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r4-fs-size-change | サイズ変化未検出 | 残タスク | ops.rs:299-312 | 前後 len 未照合 → 比較と検証を追加 | 中 | 中 | 中 | 中（未確認） |

根拠: 読込ループは `read == 0` で終わるだけで（ops.rs:299-312）、
`metadata.len()` と実コピー bytes を照合しない（ops.rs:264 の metadata は mode 専用）。
成長中ファイルは余分なバイトまでコピーされ、縮小中ファイルは静かに切り詰められる。
進捗上限は別途 `total_bytes` のスナップショット（ops.rs:506-508）なので
バーが 100% を超える/届かない可能性がある。コピー前後の `len()` 比較と
専用エラー／警告、再加算した `stats.bytes` の検証を追加するタスク。
実際の変化挙動は OS/FS依存のため未確認として扱う。

### r4-fs-dir-perms — 再帰コピーのディレクトリ権限が最後に best-effort で復元

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r4-fs-dir-perms | 権限復元遅延 | 構造改善 | ops.rs:411,420-431 | 子コピー後に best-effort → 即時復元と警告 | 高 | 中 | 高 | 高 |

根拠: `create_dir`（ops.rs:411）は既定モードで作られ、子要素を `?` で全部コピーした後
（ops.rs:420-425）にようやく `set_permissions` が走る（ops.rs:426-431）。
途中でキャンセル/失敗すると一部ディレクトリは既定権限のまま残り、
元が 0700 でも 0755 として露出しうる。さらに `let _ =` で失敗が黙殺され、
Windows には permission 復元経路が無い。作成直後にもモードを適用し、
失敗を非致命警告として報告、Windows 属性の扱いを決めるタスク。

### r4-fs-ownership — Unix の所有者/グループがコピーで保持されない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r4-fs-ownership | 所有者未保持 | 調査項目 | ops.rs:362-368; fs-notes:245-246 | chown 経路なし → 保持可否と通知を調査 | 中 | 高 | 高 | 高（未確認） |

根拠: 保存されるのは mode のみで（ops.rs:362-368, 426-431）、
`chown` に相当する呼び出しはコードベースに存在しない。設計ノートの copy/move 節
（fs-notes:245-246）も所有者に言及しない。よって root 所有や共有グループのファイルは
実行ユーザの所有になる。std には chown API が無く特権も必要なため、
「特権時にのみ保持」「非特権時は明示通知」の方針を決め、プラットフォーム API
（例: `nix` 等）の採用を事前調査するタスク。

### r4-fs-netfs — NFS/SMB で rename/trash の前提が検証されていない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r4-fs-netfs | ネットワークFS前提 | 調査項目 | ops.rs:331-350,612-644 | EXDEV 以外未処理 → 検出と fallback 調査 | 中 | 高 | 中 | 中（未確認） |

根拠: `move_entry` は `fs::rename` を試し `CrossesDevices` のときだけ
copy+delete に落とす（ops.rs:612-644）。`copy_file_atomic` は O_EXCL 予約と
`rename` による原子的置換を前提とする（ops.rs:331-350）が、NFS/SMB での
原子的置換や `EOPNOTSUPP`/`ETIMEDOUT` の扱いは未検証。設計ノートも
ネットワーク FS での trash 失敗（fs-notes:259）、監視/容量取得の遅延
（fs-notes:432）、判定手法が未定（fs-notes:543）とする。`statfs` による判定と
非 EXDEV 失敗時のフォールバック／明確なエラーを決める調査タスク。
実挙動は未確認。

### r4-fs-case-collision — 大小文字衝突検出が `exists()` に依存

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r4-fs-case-collision | 大小文字衝突 | 残タスク | ops.rs:177-216,220-237 | case-fold なし → 明示比較とテスト | 中 | 中 | 中 | 中（未確認） |

根拠: `unique_path`（ops.rs:177-216）も `resolve_destination`（ops.rs:220-237）も
`Path::exists()` のみで明示的な case-fold 比較を行わない。設計ノートは
「既存名の検出は大小文字を無視した比較」（fs-notes:248）と
「同一名判定は case-fold」（fs-notes:424）を要求しており、実装と乖離している。
さらに候補が 10000 個尽きると元のパスへ退回し（ops.rs:213-215）、
`AlreadyExists` で失敗する。Windows/macOS および大小非区別マウントを対象に
明示比較とテストを追加するタスク。`exists()` が各 FS で十分かは未確認。

### r4-fs-move-metadata — クロスデバイス移動でメタデータ保持が黙って劣化

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r4-fs-move-metadata | 異デバイス移動劣化 | 構造改善 | ops.rs:612-643 | EXDEV は copy+delete 化 → 劣化を通知 | 高 | 中 | 高 | 高 |

根拠: 同一 FS の移動は `fs::rename`（ops.rs:612-618）で inode/時刻/xattr/所有者を
そのまま保つが、`CrossesDevices` 時の fallback は `copy_recursive`
（ops.rs:619-643）へ入るため、上記 r4-fs-hardlink / sparse / xattr / timestamp /
ownership の全喪失を一度に引き起こす。利用者には進捗バイト数しか見えず
劣化が通知されない。「メタデータ保持移動が不可」であることを警告するか
異デバイス移動を設定で禁止し、両経路のメタデータ比較テストを追加するタスク。

## レンズ総評

本レンズでは、コピーが「バイト列の再生成」であるがゆえに失われる情報
（inode/ハードリンク、スパース性、xattr/ACL、時刻、所有者）と、
集計・一覧・操作の間で特殊ファイルの判定が食い違う問題を中心に 11 件を挙げた。
特に r4-fs-dir-perms（権限露出）と r4-fs-move-metadata（移動の黙った劣化）は
既存の安全設計（ops.rs:2-21）の趣旨と直接衝突し、優先度が高い。
`std::fs` の既定に依存する r4-fs-sparse / xattr / netfs / case-collision は
いずれも実挙動を未確認としており、まず小さな fixture と実機 NFS/SMB での
Runtime Proof を取ることから着手すべきである。pass 1-3 の UX 改良や
除外指定済みの symlink ポリシー・EXDEV フォールバック機構自体・O_EXCL 等とは重複させず、
EXDEV 経路での「メタデータ劣化」という意味論だけを扱った。
