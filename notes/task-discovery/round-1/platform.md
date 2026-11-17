---
type: Reference
title: 残タスク発掘 round 1 — プラットフォーム／配布／エコシステム ギャップスカウト
description: >-
  gpui-file-explorer のクロスプラットフォーム対応・配布・供給網に関する PASS 1 の発掘結果。
  ローカル文書とソースのファイル:行を根拠に残タスクと機能追加案を列挙する。
tags:
  - task-discovery
  - platform
  - distribution
  - ecosystem
  - gpui-file-explorer
generated: 2026-11-17T19:34:28+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 調査範囲と根拠

ベースライン（README.md、app/README.md 既知の制限、DESIGN.md §7/§9/§10.8、
notes/fs-cross-platform-notes.md、notes/os-file-clipboard-notes.md、
notes/wasm-browser-notes.md、notes/ecosystem-notes.md、ci.yml、build-deb.sh）を
全読したうえで、app/src の `#[cfg]` 分岐・監視・操作エンジン・設定を実読して差分を探した。
Web 検索は試行したが検索エンドポイントが応答せず、上流事実はローカル一次ノートに
記載された URL（untrusted なポインタ）に依存する。すべての `file:line` は実読で確認済み。

### r1-plat-01 Windows 実機ビルド・起動検証の不在

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-plat-01 | Windows 実機ビルド・起動検証の不在 | 残タスク | README.md:84-86 / DESIGN.md:35 / DESIGN.md:1100 | Windows は `cargo check` 成功のみで、起動・D&D・ゴミ箱・IME・隠し属性の実機検証が皆無。gpui-kit 側 CI は windows-latest を通すが自アプリは未検証。閉じるには: windows-latest で `cargo check/test --locked` に加え、VM/実機で起動・右クリック・Recycle Bin 復元・日本語 IME を手動検証し記録する。 | Windows 利用者へ「動く」と保証できる。Phase2 受入の前段 | L | 既知(ドキュメント記載あり) | 高 |

### r1-plat-02 Windows 長パス（MAX_PATH / `\\?\` / longPathAware）未対応

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-plat-02 | Windows 長パス未対応 | 残タスク | app/Cargo.toml:13-27 / app/build.rs:27-29 / DESIGN.md:808 / notes/fs-cross-platform-notes.md:267 | `dunce` も longPathAware マニフェストも `\\?\` 接頭辞も無く、build.rs は非 Linux で早期 return。260 文字超のパスで copy/rename/trash が失敗し得る。閉じるには: アプリ manifest に longPathAware を追加し I/O パスを `\\?\` 化、表示は dunce で短縮、300 文字超のツリーでコピー・ゴミ箱を実機検証する。 | 深い階層のプロジェクトでデータ損失・操作失敗を防ぐ | M | 既知(ドキュメント記載あり) | 高 |

### r1-plat-03 Windows の symlink/junction コピーが Unsupported

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-plat-03 | Windows の symlink/junction コピーが Unsupported | 残タスク | app/src/ops.rs:375-392 / notes/fs-cross-platform-notes.md:246 | `copy_symlink` は非 Unix で即 `OpError::Unsupported` を返す。Windows では 1 個でも symlink/junction を含むディレクトリのコピー**全体**が失敗する（設計ノートは「リンク先をコピー」フォールバック案に留め、実装も既知の制限にも無い）。閉じるには: `std::os::windows::fs::{symlink_file,symlink_dir}`＋権限/開発者モード判定、失敗時は実体コピーへフォールバックし junction のループ検出を追加する。 | Windows で実用上のブロッカーを解消。サイレントな部分失敗を防ぐ | M | 新規 | 高 |

### r1-plat-04 Windows/macOS の隠しファイル属性判定が未実装

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-plat-04 | Windows/macOS の隠しファイル属性判定が未実装 | 残タスク | app/src/model.rs:404-405 / app/src/model.rs:449 / DESIGN.md:809 / notes/fs-cross-platform-notes.md:273 | `is_hidden_name` は先頭 `.` のみを見るため、Windows の `FILE_ATTRIBUTE_HIDDEN` / `SYSTEM`（`desktop.ini` 等）と macOS の `UF_HIDDEN` が隠されない。DESIGN §7 では P2 扱い。閉じるには: Entry に OS 属性フラグを保持し `#[cfg]` で判定を切替え、3 OS のフィクスチャで `Ctrl+H` の挙動をテストする。 | 各 OS のエクスプローラと同等の「隠し」挙動を実現 | M | 既知(ドキュメント記載あり) | 高 |

### r1-plat-05 macOS クロスビルド制約

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-plat-05 | macOS クロスビルド制約 | 残タスク | README.md:85-86 / DESIGN.md:36 / notes/ecosystem-notes.md:255-256 | 上流 `gpui-pre-apple` の制約で Linux から `cargo check --target aarch64-apple-darwin` が不可。閉じるには: CI に macos-latest（aarch64）job を追加して `cargo check/test --locked` を回す。まず AGENTS.md §5 に従い共通ワークフロー `Seduce0068/workflows` 側で対応できるか検討し、不可なら本リポジトリへ matrix job を追加する。 | 「コンパイル可能性を壊さない」という Phase2 目標を自動ゲート化 | M | 既知(ドキュメント記載あり) | 高 |

### r1-plat-06 macOS NFD・大小文字 rename・Finder reveal の実機未検証

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-plat-06 | macOS NFD・大小文字 rename・Finder reveal の実機未検証 | 残タスク | DESIGN.md:808 / notes/fs-cross-platform-notes.md:268 / app/src/ops.rs:663-686 / app/README.md:353-357 | NFD→NFC の表示正規化は未実装（I/O は元 `PathBuf` 維持が必須）。大小文字のみ rename の 2 段階処理は実装済みだが APFS での実機確認が無い。`cx.reveal_path` は戻り値が無く Finder への reveal 成否を検知できない。閉じるには: unicode-normalization を表示レイヤのみに導入し、macOS 実機で NFD ファイル名の検索/リネームと `open -R` 相当の reveal を確認する。 | 日本語ファイル名が壊れず、Finder 連携も信頼できる | L | 既知(ドキュメント記載あり) | 中 |

### r1-plat-07 ドライブ列挙が Linux のみ

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-plat-07 | ドライブ列挙が Linux のみ | 残タスク | app/src/mounts.rs:138-151 / app/README.md:431-433 / DESIGN.md:810 | `list_mounts()` は非 Linux で空 Vec を返し、Windows/macOS ではサイドバーに「ドライブ」セクションが出ない。空き容量・ホットプラグも未対応。閉じるには: `sysinfo::Disks` で 3 OS 共通の列挙＋空き容量を追加し、Windows は GetLogicalDrives/WM_DEVICECHANGE、macOS は NSFileManager/NSWorkspace 通知を `#[cfg]` で実装する。 | クロスプラットフォームの基本ナビゲーションが成立する | L | 既知(ドキュメント記載あり) | 高 |

### r1-plat-08 OS ファイルクリップボード（text/uri-list）未対応

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-plat-08 | OS ファイルクリップボード未対応 | 残タスク | notes/os-file-clipboard-notes.md:6-8 / notes/os-file-clipboard-notes.md:63-72 / README.md:90-94 / app/README.md:381-384 | GPUI 0.3.12 の Linux バックエンドが `text/uri-list` を read/write せず、Dolphin 等とのファイルのコピー&貼り付けが不可（D&D は別経路で済み）。閉じるには: gpui-pre-linux へ URI list の read/write を upstream 提案するか、arboard 3.7 `file_list()` を直接経路で追加し、`ClipboardEntry::ExternalPaths` を既存 `drop_external_into`（常にコピー・Rename）へ流す。cut/copy 区別は `x-special/gnome-copied-files` 先頭行を解釈する。 | 他アプリとのファイル受け渡しが双方向にできる | L | 既知(ドキュメント記載あり) | 高 |

### r1-plat-09 WASM/ブラウザ移植の FS 抽象化

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-plat-09 | WASM/ブラウザ移植の FS 抽象化 | 機能追加案 | notes/wasm-browser-notes.md:67-79 / notes/wasm-browser-notes.md:28-38 / README.md:96-104 / app/README.md:463-468 | `std::fs`/`notify`/`trash`/OS ダイアログ直依存でブラウザ不可。gpui-kit 既定 feature は nightly 必須の wasm_thread を要求する。閉じるには: `FileSystem` trait を導入し OPFS/File System Access API 実装、監視はポーリング、ゴミ箱はアプリ内退避、`prompt_for_paths` は `showDirectoryPicker`、chrono は js-sys へ変更。まず `gpui-pre`+`gpui-pre-platform` を `default-features = false` で読み取り専用 Web デモを別 crate で先行させる。 | ブラウザでも試作を触れる。配布チャネルの拡大 | L | 既知(ドキュメント記載あり) | 高 |

### r1-plat-10 PollWatcher フォールバックが未実装

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-plat-10 | PollWatcher フォールバックが未実装 | 残タスク | app/src/watcher.rs:16 / app/src/watcher.rs:89-100 / notes/fs-cross-platform-notes.md:111 / notes/fs-cross-platform-notes.md:126 | 調査ノートは `MaxFilesWatch`／ネットワーク FS で `PollWatcher` への切替を推奨するが、実装は `RecommendedWatcher` 一択でエラー時は `Rescan` に落とすのみ。閉じるには: `notify::PollWatcher`（2 秒）を生成し、watch エラー時に切替＋ステータスに「自動更新が制限されています」を表示、ネットワーク FS のフィクスチャで検証する。 | NFS/WSL/巨大ツリーでも一覧が更新され続ける | M | 既知だが未対応 | 高 |

### r1-plat-11 アプリ識別子（set_app_identity）未設定

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-plat-11 | アプリ識別子未設定 | 構造改善 | app/src/main.rs:5633 / notes/ecosystem-notes.md:266-267 | Wayland `app_id` は設定済みだが `cx.set_app_identity(...)` の呼び出しが無い。gpui-kit 配布ドキュメントが推奨する通知・設定のアプリ紐付けから逸脱している。閉じるには: 起動時に `set_app_identity("com.seduce0068.gpui-file-explorer", "gpui-file-explorer")` を追加し、3 OS で通知識別と設定パスが衝突しないことを確認する。 | macOS/Windows での通知・設定の誤紐付けを防ぐ | S | 新規 | 中 |

### r1-plat-12 配布: 署名・リポジトリ公開・タグ・非 Debian 形式

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-plat-12 | 署名・リポジトリ公開・タグ・非 Debian 形式の欠如 | 残タスク | README.md:60-77 / DESIGN.md:1088 / DESIGN.md:890-893 / notes/ecosystem-notes.md:280-282 / notes/ecosystem-notes.md:485 | DEB amd64 のみで署名・apt repo 公開・タグ付けが無く、tar.gz/AppImage/Flatpak も無い。AGPL §4 のソース tarball＋SHA-256 のリリース同行も未実施。閉じるには: GitHub Releases に source tarball＋タグ＋SHA-256＋GPG 署名を添付し apt repo を公開、非 Debian 向け tar.gz を build-deb.sh の stage から生成。Flatpak/AppImage は gpui-kit 公式手順が無く Vulkan サンドボックス権限の調査が先。 | インストール可能で検証可能な配布物になる。AGPL 遵守も明確化 | L | 既知(ドキュメント記載あり) | 高 |

### r1-plat-13 CI マトリクスとリリース自動化の欠如

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-plat-13 | CI マトリクスとリリース自動化の欠如 | 残タスク | .github/workflows/ci.yml:27-43 / README.md:114-122 / notes/ecosystem-notes.md:303-323 | CI は ubuntu-latest の apt 依存と `app/`＋`poc/` のゲートのみで、Windows/macOS job や tag→build→release の自動化が無い。gpui-kit は Linux/macOS/Windows の 3 OS マトリクスを運用している。閉じるには: まず共通ワークフロー側で windows/macos job を追加できるか検討し、不可なら本リポジトリへ matrix job を追加。リリース自動化は署名・AGPL ソース同梱（r1-plat-12）とセットで設計する。 | クロスプラットフォーム回帰を push 時に検出できる | L | 既知(ドキュメント記載あり) | 高 |

### r1-plat-14 供給網: gpui-kit 完全ピンと unmaintained 警告の監視

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-plat-14 | 供給網: gpui-kit 完全ピンと unmaintained 警告の監視 | 構造改善 | app/Cargo.toml:14 / app/README.md:456-461 / notes/ecosystem-notes.md:74-79 / notes/ecosystem-notes.md:376-383 / notes/ecosystem-notes.md:422-426 | `=0.9.1` 完全ピン＋Cargo.lock コミットは遵守済み。だが bus factor=1 の第三者ミラー依存で、unmaintained 8 件（instant/paste/rustls-pemfile/rustybuzz/ttf-parser/zbus_names/zvariant/zvariant_derive）の監視と cargo-deny/vet によるライセンス・重複版チェックが未運用。閉じるには: 更新時に `zed-rev` 来歴を検証し `cargo audit` を再実行、cargo-deny で GPL 混入/重複版をゲート、同名再公開の `gpui-pre-reqwest` 差分をウォッチする。 | 週次破壊変更とライセンス感染のリスクを低減 | M | 既知(ドキュメント記載あり) | 高 |

### r1-plat-15 AGPL §13 と WASM Web デモの衝突

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-plat-15 | AGPL §13 と WASM Web デモの衝突 | 調査項目 | DESIGN.md:888-894 / DESIGN.md:892 / README.md:96-104 | 現状はデスクトップ専用のため §13（ネットワーク越し利用者へのソース提供表示）は発動しない。しかし r1-plat-09 の Web デモを公開すると、改変版をネットワーク経由で使わせる扱いになり表示義務が生じ得る。閉じるには: Web デモの About/初回画面にソース URL を表示し、README に §13 運用を明記してから公開する。 | ブラウザ展開時にライセンス違反を回避できる | S | 新規 | 中 |

## レンズ総評

優先度の高い順に、① Windows 長パスと symlink/junction コピー（r1-plat-02/03、
実データのコピーが失敗し得る点で最も深刻で、かつ r1-plat-01 の実機検証とセットで潰す）、
② Windows/macOS の CI マトリクス追加（r1-plat-05/13、共通ワークフロー対応が先）、
③ 配布の署名・タグ・ソース tarball 同梱（r1-plat-12、AGPL 遵守に直結）を推奨する。

新規発見は r1-plat-03（ops.rs の非 Unix で即 Unsupported になる symlink コピー）、
r1-plat-11（`set_app_identity` 未設定）、r1-plat-15（WASM 公開時の AGPL §13）の 3 件。
特に r1-plat-03 は「Windows は `cargo check` が通る」という現状認識と乖離しており、
コンパイル成功では見つからない実行時ブロッカーとして第 6 パスの検証で優先確認してほしい。

既知項目については、単に再掲せず各 `description` に「閉じるには:」で具体的な次動作を記した。
Web 検索は本次でエンドポイント障害により失敗しており、上流事実はローカル一次ノート
（untrusted な URL ポインタ）に基づく。第 6 パスでは必要に応じて公式ドキュメントの
直接確認を推奨する。
