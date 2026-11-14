<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# gpui-file-explorer

[![CI](https://github.com/Seduce0068/gpui-file-explorer/actions/workflows/ci.yml/badge.svg)](https://github.com/Seduce0068/gpui-file-explorer/actions/workflows/ci.yml)
[![License: AGPL-3.0-or-later](https://img.shields.io/badge/license-AGPL--3.0--or--later-blue)](LICENSE)

GPUI（Zed の UI フレームワーク）でクロスプラットフォームなファイルエクスプローラーを
自作するための**調査レポート・設計書・動く試作**をまとめたリポジトリです。

- 対象スタック: `gpui-kit = "=0.9.1"`（内部で `gpui-pre 0.3.12` = Zed GPUI snapshot + `gpui-pre-platform` + `gpui-component` + Lucide assets）
- 第一ターゲット: Linux / KDE Plasma / Wayland（Kubuntu 2026.04 で実機検証）。コードは Windows/macOS を見据えた設計
- ライセンス: **AGPL-3.0-or-later**（[LICENSE](LICENSE)、第三者表示は [app/NOTICE.md](app/NOTICE.md)）

## クイックスタート

```bash
git clone https://github.com/Seduce0068/gpui-file-explorer.git
cd gpui-file-explorer

# Linux で libxkbcommon-x11-dev が無い場合のみ（sudo 不要のローカル回避）
bash scripts/dev-setup.sh

cd app
cargo build          # 初回は依存込みで約 1m30s
cargo test           # 36 tests
./target/debug/gpui-file-explorer [ディレクトリ]
```

前提: Rust 1.92+、Vulkan 対応 GPU ドライバ、Wayland または X11。
必要パッケージは [app/README.md](app/README.md) の「ビルド」を参照してください
（`libxkbcommon-x11-dev` が無い環境向けの回避スクリプトを同梱）。

## リポジトリ構成

| パス | 内容 |
|---|---|
| [app/](app/) | **動く試作**（AGPL-3.0-or-later）。ツールバー / ブレッドクラム / サイドバー / 仮想リスト / 非同期ロード / 自然順ソート / 隠しファイル切替 / 履歴 / 右クリックメニュー / ゴミ箱（確認付き）/ 設定永続化 / notify 監視 |
| [DESIGN.md](DESIGN.md) | 設計書（1,121 行）。機能マトリクス P0〜P3、ASCII ワイヤーフレーム、型契約、watch→debounce→reconcile 状態機械、trash-first 安全則、AGPL 運用、受入条件 AC-1〜AC-13、ADR 12 件 |
| [RESEARCH.md](RESEARCH.md) | 調査レポート。要点 12 項目 / 推奨スタック / クロスプラットフォームの罠 / MVP 計画 / PoC 検証結果 |
| [poc/](poc/) | スタック検証用の最小 PoC（`gpui-kit` のみ、実機でビルド・起動確認済み） |
| [notes/](notes/) | 一次調査ノート（GPUI API 棚卸し・エコシステム・OS/FS 差分）と独立検証レポート |
| [scripts/dev-setup.sh](scripts/dev-setup.sh) | `libxkbcommon-x11` 開発ファイル欠如時のローカル symlink 作成（システム変更なし） |
| [packaging/linux/](packaging/linux/) | Linux DEB ビルド（`build-deb.sh`、`.desktop`、hicolor アイコン）。root 不要・ホストへインストールしない |
| [.github/workflows/ci.yml](.github/workflows/ci.yml) | 個人リポジトリ共通の Rust ゲート（`Seduce0068/workflows@v1`）へ委譲 |

> `notes/` は元の開発ワークスペース（`work/gpui-file-explorer/`）で作成した調査記録です。
> 一部に `work/gpui-file-explorer/...` や `security-review/Argus`（リポジトリ外）への参照が残っています。

## 試作の検証結果（2026-11-11 実機）

- `cargo build` 警告 0 / `cargo test` **36 passed / 0 failed**
- Wayland ネイティブ起動（stderr 0 行）。KWin D-Bus で
  `caption="GPUI File Explorer"`, `resourceClass="gpui-file-explorer"`, 1100x728 を確認
- 起動中に外部からファイル作成 → 一覧が自動更新（14→15 件）
- 独立検証レポート: [notes/verify-report.md](notes/verify-report.md)（blocker なし。major 3 は修正済み）
- Lead 再検証: [notes/lead-verify/SUMMARY.md](notes/lead-verify/SUMMARY.md)
- 証拠画像: [postfix-after.png](notes/lead-verify/postfix-after.png)

## 配布ビルド（Linux DEB）

```bash
# リポジトリ直下で実行（root では実行できません）
bash packaging/linux/build-deb.sh
# 出力: packaging/linux/build/gpui-file-explorer_0.1.0-1_amd64.deb

# release バイナリを再ビルドせずパッケージだけ作り直す場合
bash packaging/linux/build-deb.sh --skip-build
```

- 収録物: `/usr/bin/gpui-file-explorer`、`.desktop`、hicolor アイコン（16〜512px + scalable）、
  `README.md` / `copyright` / `NOTICE`
- `Depends` は `ldd` + `dpkg -S` から自動生成（Vulkan ローダは dlopen のため `libvulkan1` を明示追加）
- `dpkg-deb --root-owner-group` を使うため **sudo 不要**。ホストへのインストールも行いません
- 検証済み: `dpkg-deb -x` で展開したバイナリが `--version` / GUI 起動（Wayland）で動作、
  `apt-get -s install ./...deb` の依存解決が通ること
- 署名・リポジトリ公開・タグ付けは未実施（リリースはユーザー指示があるまで行いません）

## 既知の制限

- キーボード / 右クリック / 削除確認ダイアログの**実押下 E2E は未検証**
  （Wayland ネイティブへ合成入力を送るツールが検証機に無いため）
- P1 進行中: 安全なファイル操作（`app/src/ops.rs`: 上書き禁止・symlink非追跡・キャンセル対応）、コピー/切り取り/貼り付け（`Ctrl+C`/`Ctrl+X`/`Ctrl+V`）、リネームダイアログ（F2、上書き拒否・失敗時は原名維持）、コピー/移動のバイト進捗表示（ステータスバー、80msスロットル）、複数選択（Ctrl/Shift/Ctrl+A）とバッチ操作（コピー/切り取り/ゴミ箱、件数付き確認）、ゴミ箱ビュー（一覧・復元）、視覚的な進捗バー（総バイト事前計算 + パーセント表示）、アイコングリッド表示（列数自動・設定永続化）、再帰検索（Ctrl+F、symlink非追跡・上限付き・キャンセル対応）、ボリューム（ドライブ）一覧（Linux /proc/mounts）、タブ（Ctrl+T/W/Tab、履歴保持）、最近使った場所（サイドバー、最大10件）、ドラッグ&ドロップ（OSからのコピー受信・フォルダ単位のドロップ先・OSへのドラッグアウト）、検索結果の絞り込み（種別・拡張子）、内容検索（行番号表示・バイナリ除外・上限付き）、ブックマーク（Ctrl+D、最大50件）、プロパティ表示（Ctrl+I、読み取り専用）、空状態・ローディング表示、キーボードヘルプ（F1）、タブ構成の永続化、プロパティのディレクトリサイズ計算（バックグラウンド・キャンセル可能）、種類（拡張子）でのソートと「種類」列、タブの並べ替えを実装・テスト済み。`#![forbid(unsafe_code)]` を app 本体に適用。未実装: 内容検索、複数ルート検索、OSファイルクリップボード連携（GPUI側未実装）
- Windows / macOS の実機ビルド・実行は未検証（設計は `std::fs` / `dirs` / `trash` でポータブル）
- 配布パッケージは Linux amd64 の DEB のみ（署名・AppImage/Flatpak・Windows/macOS は未対応）
- 詳細は [app/README.md](app/README.md) の「既知の制限」

## OS ファイルクリップボード

- アプリ内クリップボード（`Ctrl+C/X/V`）と D&D は対応済みですが、OS のファイルクリップボード
  （`text/uri-list`）は GPUI 0.3.12 の Linux バックエンドが読み書きしないため未対応です。
- 調査と将来対応: [notes/os-file-clipboard-notes.md](notes/os-file-clipboard-notes.md)

## WASM / ブラウザ対応

- 現試作は**デスクトップ専用**です（`std::fs` / `notify` / `trash` / OS ダイアログを使用）。
- GPUI 自体には Web バックエンド（`gpui-pre-web`）があり、`gpui-pre` + `gpui-pre-platform` を
  `default-features = false` にすれば **stable で wasm32 にコンパイル可能**（検証済み）。
  ただし `gpui-kit` の既定 feature は nightly 必須の `wasm_thread` を有効にします。
- ブラウザで動かすには File System Access API / OPFS への FS 抽象化、監視のポーリング化、
  ゴミ箱・OS 統合の代替が必要で、別フロントエンド相当の作業です。
- 詳細と検証ログ: [notes/wasm-browser-notes.md](notes/wasm-browser-notes.md)

## ライセンス

本リポジトリのコード・文書は **GNU Affero General Public License v3.0 or later**
（`AGPL-3.0-or-later`）です。バイナリ配布時は対応するソースの提供が必要です
（本リポジトリがそのソースです）。依存クレートはそれぞれのライセンスに従います
（gpui 系 Apache-2.0、Lucide ISC、notify CC0-1.0 ほか）。詳細は
[NOTICE.md](NOTICE.md) と [app/NOTICE.md](app/NOTICE.md) を参照してください。
