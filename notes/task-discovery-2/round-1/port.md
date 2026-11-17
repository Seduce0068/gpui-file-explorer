---
type: Reference
title: Round 1 — 移植性 / クロスプラットフォーム レビュー
description: 読み取り専用エージェント r1-port の所見（15 件）。
tags: [task-discovery, cross-review, portability, gpui-file-explorer]
generated: 2026-11-17T23:25:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-port-01 | `trash` / `notify` / `dirs` が WASM 非対応なのに無条件依存 | 残タスク | `Cargo.toml:16-18`, `trash_view.rs:18-27`, `main.rs:449`, `watcher.rs:11-13`, `notes/wasm-browser-notes.md:43-50` | `trash::TrashItem` が `Explorer` のフィールドとして残り、`trash::os_limited::list/restore_all` と `notify` の Debouncer が cfg gate なしでリンクされる。`cargo check --target wasm32-unknown-unknown` は `trash` で失敗した Runtime Proof あり。 | 高 | L | 前回と重複(R6-B-01) | 高 |
| r1-port-02 | CI ゲートが Linux のみで Windows/macOS のコンパイル可能性が未検証 | 構造改善 | `ci.yml:25-43`, `README.md:84-86`, `DESIGN.md:39` | `apt_packages` は Linux 開発パッケージのみ。`uses:` 先に target matrix は無く、DESIGN §1.3 の目標を CI が担保していない。 | 高 | M | 前回と重複(R6-B-16) | 高 |
| r1-port-03 | ルート `/` がハードコードされ Windows のドライブ概念と矛盾 | 残タスク | `main.rs:150,267,530,538,5666`, `model.rs:699-714` | `tab_label` は `Path::new("/")` のときだけ「ルート」を返し、サイドバー/起動フォールバックも `/` 固定。Windows の `C:\` は `Component::Prefix` が捨てられラベルが `/` になる。 | 中 | M | 新規 | 低 |
| r1-port-04 | 非 UTF-8 の CLI 引数で `std::env::args()` がパニック | 残タスク | `main.rs:5611,5635-5638`, `notes/fs-cross-platform-notes.md:269,412` | 起動ディレクトリ引数を `args()`（String）で読むため非 UTF-8 パスを渡すとパニック。データ層は `OsString` を保持する設計と矛盾。`args_os()` で閉じられる。 | 中 | S | 新規 | 高 |
| r1-port-05 | 大小文字非区別 FS で選択・一意名判定が生 `PathBuf` キー | 構造改善 | `model.rs:180-189`, `ops.rs:225-236,659-669` | `Selection` は `HashSet<PathBuf>`、`resolve_destination` は `candidate.exists()`、`unique_path` もバイト単位。case-insensitive FS で重複選択・不必要な `(1)` 採番が起きる。 | 中 | M | 既知(ドキュメント記載あり) | 低 |
| r1-port-06 | macOS の NFD ファイル名を NFC 化せず検索・比較 | 残タスク | `model.rs:102-103,153-154`, `search.rs:315-327,347` | 表示・sort_key・内容検索の needle が `to_string_lossy()/to_lowercase()` のみ。HFS+ 由来の NFD 名を NFC クエリで検索すると一致しない。 | 中 | M | 既知(ドキュメント記載あり) | 低 |
| r1-port-07 | Windows 長パスの `\\?\` が表示・設定に漏れる | 残タスク | `main.rs:5662`, `ops.rs:244-245,666-669` | CLI 起動時と case-only rename 判定で `fs::canonicalize` を使う。Windows では `\\?\C:\...` が返り cwd/settings に保存・UI 表示され得る。 | 中 | S | 既知(ドキュメント記載あり) | 低 |
| r1-port-08 | Windows 予約名・禁止文字検証が末尾ドット/スペースと制御文字を漏らす | 残タスク | `ops.rs:133-169` | `#[cfg(windows)]` ブロックは `CON/LPT9` 等と `< > : " \| ? *` のみを拒否。`foo.`、`foo `、0x01–0x1F は通る。 | 中 | S | 前回と重複(R6-C-13) | 中 |
| r1-port-09 | 隠しファイル判定が Unix の dot 前置のみ | 残タスク | `model.rs:404-406`, `README.md:103` | `is_hidden_name` は `name.starts_with('.')` 一択。Windows の `FILE_ATTRIBUTE_HIDDEN` と macOS の `UF_HIDDEN` を読まない。 | 中 | M | 既知(ドキュメント記載あり) | 高 |
| r1-port-10 | シンボリックリンクのコピーが Windows で `Unsupported` | 機能追加案 | `ops.rs:375-391` | `copy_symlink` は `#[cfg(unix)]` のみ実装し、非 Unix は常に `OpError::Unsupported`。「リンク先をコピー」へのフォールバック案があるが未実装。 | 中 | M | 既知(ドキュメント記載あり) | 高 |
| r1-port-11 | クロスデバイス移動の判定が `ErrorKind::CrossesDevices` に依存 | 調査項目 | `ops.rs:116-119,612-643` | Windows の別ボリューム間 `MoveFileExW` が std でどうマップされるか実機未検証。 | 中 | M | 新規 | 低 |
| r1-port-12 | Windows の O_EXCL 予約を削除してから rename する非原子性と設定保存の矛盾 | 調査項目 | `ops.rs:331-350`, `settings.rs:278` | `copy_file_atomic` は Windows で予約ファイルを `remove_file` してから `rename`。しかし `settings.rs:278` は同じ `fs::rename` で既存設定を置換しており矛盾。 | 中 | M | 前回と重複(R6-C-13) | 低 |
| r1-port-13 | CRLF 行末が内容検索とテキストプレビューに残る | 調査項目 | `search.rs:315-327`, `preview.rs:158-170` | 内容検索は `read_until(b'\n')` で `\r` を落とさず、プレビューも生文字列を描画。Unix の LF のみでテストしている。 | 低 | S | 新規 | 低 |
| r1-port-14 | 非 UTF-8 パスが `serde_json` 設定保存で失敗しうる | 調査項目 | `settings.rs:258,271-278`, `model.rs:80-82` | `Entry::name` は OsString を保持するが Settings の PathBuf フィールドは JSON 文字列化できず `save()` が `error!` ログのみで失敗する。 | 中 | M | 新規 | 中 |
| r1-port-15 | `chrono::Local`/`Utc::now()` が WASM で動かない | 残タスク | `app/README.md:463-468`, `notes/wasm-browser-notes.md:49-50,67-79` | 日時表示とバックアップ名に `chrono = "0.4"` を使う。README 自身が WASM 対応時に `js-sys` ベースへ差し替えが必要と明記するがコード側に cfg も抽象化も無い。 | 低 | M | 既知(ドキュメント記載あり) | 高 |

## レンズ要約

ポータビリティの大部分は「設計ノートに落とし穴として記載済みだが実装に反映されていない」項目（大小文字・NFD・長パス・隠し属性・Windows 名検証）で、実機がないと確定できない。CI が Linux のみで WASM/Windows/macOS をゲートしていないため、README の「クロスプラットフォーム設計」という主張とコードの乖離が続く。次に閉じるべきは Windows target の `cargo check` と wasm feature 分離、非 UTF-8 パスの CLI／設定まわり。
