---
type: Reference
title: "残タスク発掘 round 3 — OS 深層統合 API 機会の調査"
description: >-
  GPUI ファイルエクスプローラーの D-Bus / XDG / FreeDesktop / Windows Shell /
  macOS AppKit といった OS 統合 API の未着手な接続点を、ローカルの調査ノートと
  ソースのファイル:行を根拠に列挙する（PASS 3・OSINT レンズ）。
tags:
  - task-discovery
  - os-integration
  - dbus
  - freedesktop
  - xdg
  - gpui-file-explorer
generated:
  by: "r3-osint"
  at: "2026-11-17T20:26:40+09:00"
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 調査範囲と根拠

PASS 3 の DEEP OS-INTEGRATION API OPPORTUNITIES レンズ。優先根拠は
`notes/fs-cross-platform-notes.md`（§2 監視 / §3 アイコン / §5 操作 / §6 統合 /
§7 クリップボード / §9 マトリクス / §10 落とし穴 / §13 一次資料 / §14 未確認）、
`notes/api-notes.md`（§4.5 ネイティブダイアログ / §8.1 URL・ファイル）、
`DESIGN.md:791-818`（OS 統合マトリクス）および `app/src/` の現行コードである。

既に他パスで扱われた項目（OS ファイルクリップボード、open/reveal の失敗検知、
サムネイル/OS アイコンの機能自体、ドライブ列挙、PollWatcher、Windows の長パス等、
macOS NFD、DEB パッケージング、アプリ識別子、AGPL 配布、trash ポータル機能、
WASM ポート、一般クロスプラットフォーム検証、Open With の UI 自体）は重複を避け、
各項目は「より深い API 接続点・配管」に絞った。

外部仕様の確認は DeepSeek web_search / web_fetch がネットワークエラー
（`fetch failed`）で失敗し、代替の RustXNG 検索も一次資料を返さなかった。
したがって仕様のメソッド名・定数・URL 詳細はすべて **未確認（外部仕様・未検証）**
とし、ローカルノートを権威として扱う。ローカルノート内の一次資料リンクは
`notes/fs-cross-platform-notes.md:516-526` に集約されている。

## 発見一覧

### r3-osint-01: FileManager1 の D-Bus メソッドフォールバック

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r3-osint-01 | D-Bus `org.freedesktop.FileManager1` の ShowFolders/ShowItems フォールバック | 残タスク | `app/src/main.rs:3449-3457`; `DESIGN.md:799-800`; `notes/fs-cross-platform-notes.md:303-309`; `notes/api-notes.md:301`; `app/Cargo.toml:13-26` | `reveal()` は `cx.reveal_path` 一発のみ。DESIGN は Linux で `ShowItems`（Dolphin 対応）を期待するが「KDE 実機 ⚠」。opener は D-Bus 第一候補 + `OpenURI` フォールバックを持つが本プロジェクトは未依存。次の一手: zbus proxy で ShowFolders/ShowItems → OpenURI の明示フォールバックを 1 モジュール化。※ メソッド名の対応範囲は外部仕様未確認。 | DE 差による「ファイルマネージャで表示」失敗を低減 | M | 既知(ドキュメント記載あり) | 高 |

### r3-osint-02: FileChooser ポータルの sandbox 対応

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r3-osint-02 | `org.freedesktop.portal.FileChooser` による sandbox 下のファイル選択 | 調査項目 | `notes/api-notes.md:298-305`; `DESIGN.md:805`; `notes/fs-cross-platform-notes.md:494-495`; `app/Cargo.toml:13-26` | `prompt_for_paths` は「Linux ではファイルピッカーを開けない場合エラー」と doc にあり、DESIGN も「XDG ポータル必須 ⚠」とする。Flatpak 等では `FileChooser` ポータルが前提。次の一手: ashpd 0.14 でポータルを呼ぶ経路を PoC し、GPUI の `prompt_for_paths` と挙動差を比較。※ ポータル API 詳細は外部仕様未確認。 | sandbox 配布でもフォルダ/ファイル選択を保証 | M | 既知(ドキュメント記載あり) | 中 |

### r3-osint-03: 最近使った項目の XBEL 書き出し

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r3-osint-03 | freedesktop recent files (XBEL) への `recent_dirs` 書き出し | 機能追加案 | `app/src/settings.rs:61,102-113`; `app/src/main.rs:884-885`; `notes/fs-cross-platform-notes.md:519` | 最近使ったディレクトリは `settings.json` の `recent_dirs`（最大 10）だけで他アプリから不可視。freedesktop の recent files 形式（`recently-used.xbel`）へ読み書きするアダプタを追加し、既存 `record_recent` と双方向同期する。※ XBEL スキーマ詳細は外部仕様未確認。 | GTK アプリ等の「最近使った項目」と相互可視化 | M | 新規 | 中 |

### r3-osint-04: freedesktop サムネイルキャッシュの相互運用

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r3-osint-04 | freedesktop thumbnail spec の共有キャッシュに準拠 | 機能追加案 | `notes/fs-cross-platform-notes.md:180-182`; `notes/task-discovery/round-1/gap.md:63` | サムネイル機能自体は r1-gap-07 で扱済み。残るはディスクキャッシュが `~/.cache/<app>/thumbnails/` の独自パスで共有キャッシュと非互換な点。URI ハッシュ + 正規化サイズで読み書きし、Nautilus/Dolphin の生成済みサムネイルを再利用する。※ ハッシュ算法・サイズ詳細は外部仕様未確認。 | 重複生成を避け、デスクトップ全体でサムネイル共有 | M | 新規 | 中 |

### r3-osint-05: xdg-mime / mimeapps.list の問い合わせ配管

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r3-osint-05 | `xdg-mime` 既定アプリ問い合わせの配管を先行実装 | 調査項目 | `notes/fs-cross-platform-notes.md:311-317`; `notes/task-discovery/round-2/competitor.md:43-47` | Open With の UI は r2-cmp-02 で扱済み。その前提となる MIME → 既定アプリ/候補の問い合わせ層（`.desktop` 走査 + `mimeapps.list` + `xdg-mime query`）は未実装。UI を作らずクエリ層だけを差し替え可能なモジュールに切り出す。※ 出力形式は外部未確認。 | Open With 実装時の実現性リスクを先行除去 | S | 既知(ドキュメント記載あり) | 中 |

### r3-osint-06: inotify 上限の事前検知

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r3-osint-06 | inotify watch 上限の起動時検知と sysctl ガイダンス | 残タスク | `notes/fs-cross-platform-notes.md:108,112-116,126,421-422` | `max_user_watches=518185` 等はローカル実測済みで、MaxFilesWatch 時の PollWatcher フォールバックも設計済み（＝他パス covered）。残るは起動時に `/proc/sys/fs/inotify/max_user_watches` を読み、予測 watch 数と比較して「上限近し」をステータス表示 + 設定ガイドを出す事前検知。※ 推奨値は環境依存、外部仕様未確認。 | 巨大ツリーで監視が黙って止まる問題を事前防止 | S | 既知(ドキュメント記載あり) | 高 |

### r3-osint-07: IFileOperation による Windows ネイティブコピー

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r3-osint-07 | Windows Shell `IFileOperation` へのコピー/移動委譲 | 機能追加案 | `app/src/ops.rs:1-21`; `app/src/undo.rs:18-25`; `notes/fs-cross-platform-notes.md:247,521` | コピーは ops.rs の自前 Rust 実装（進捗は chunk 境界、取消は `CancelFlag`）、undo はアプリ内 `MAX_UNDO=20`。Windows で `IFileOperation` に委譲すれば Explorer 標準の進捗ダイアログ・OS レベル undo・シェル準拠の衝突処理が得られる。長パス等は covered なので重複しない。※ COM 初期化/API 詳細は外部仕様未確認。 | ネイティブ進捗・OS undo・シェル互換性 | L | 新規 | 中 |

### r3-osint-08: macOS Finder 選択 API の直接使用

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r3-osint-08 | `NSWorkspace` の Finder 選択 API を `open -R` と比較検証 | 調査項目 | `notes/fs-cross-platform-notes.md:154,308,522`; `DESIGN.md:799`; `notes/api-notes.md:432` | reveal は `open -R` 依存。`NSWorkspace` はアイコン/ボリュームで言及済みだが、Finder 選択（複数選択リビール）の直接呼び出しは未検討。次の一手: objc2-app-kit 0.3.2 で選択系セレクタを呼ぶ PoC を作り `open -R` の挙動差を比較。※ セレクタ名・挙動は外部仕様未確認。 | macOS で確実かつ複数選択の Finder リビール | M | 既知(ドキュメント記載あり) | 中 |

### r3-osint-09: XDG ベースディレクトリの cache/state 準拠

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r3-osint-09 | cache/data/state ディレクトリを `dirs` に統一 | 構造改善 | `app/src/settings.rs:100-104`; `DESIGN.md:823`; `notes/fs-cross-platform-notes.md:181,505`; `app/Cargo.toml:21` | config は `dirs::config_dir()` で XDG 準拠だが、サムネイルキャッシュ等が `~/.cache/<app>` の ad-hoc パスで `XDG_CACHE_HOME` を honors しない。`dirs` の `cache_dir`/`data_dir`/`state_dir` に統一し、環境変数上書きのテストを追加（Cargo.toml は dirs=6、ノートは 7.0.0 で版差に注意）。 | XDG 準拠と非標準環境での配置正确性、権限硬化と相補 | S | 既知(ドキュメント記載あり) | 高 |

### r3-osint-10: thumbnailer D-Bus サービスの探索的 PoC

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r3-osint-10 | `org.freedesktop.thumbnailer` D-Bus サービスの有無を問い合わせ | 調査項目 | `notes/fs-cross-platform-notes.md:180,494`; `notes/task-discovery/round-1/gap.md:63` | 動画/PDF サムネイルは MVP 外で、fs-notes:180 が D-Bus thumbnailer 委譲を提案済み。次の一手: zbus でサービス有無を問い合わせ、利用可能なら ffmpeg 直リンクなしでサムネイル取得する PoC。機能自体は covered なので D-Bus 接続のみを扱う。※ インターフェース名/メソッドは外部仕様未確認。 | 重い依存なしで動画/PDF プレビューを実現 | L | 既知(ドキュメント記載あり) | 中 |

## レンズ総評

OS 深層統合の観点で 10 件を列挙した。ローカルの一次根拠が最も強いのは
r3-osint-01（reveal の D-Bus フォールバック）、r3-osint-06（inotify 上限事前検知）、
r3-osint-09（XDG cache/state 準拠）で、いずれも現行コードとの差分が明確である。
次に着眼度の高いのが r3-osint-03（XBEL）、r3-osint-04（サムネイルキャッシュ共有）、
r3-osint-07（IFileOperation）で、既存機能の「外のエコシステムとの接続」を補う。

全件に共通する制約は、外部仕様のメソッド名・定数・URL が本セッションで
検証できなかったこと（network `fetch failed`）である。実装に着手する前に、
各項目の「未確認」部分を一次資料で確定させる作業を最初のステップに置くこと。
ローカルノートは権威として扱い、外部検索結果は補助情報としてのみ参照した。
ソースファイルは一切変更していない。
