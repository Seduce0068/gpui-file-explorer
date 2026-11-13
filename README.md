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

## 既知の制限

- キーボード / 右クリック / 削除確認ダイアログの**実押下 E2E は未検証**
  （Wayland ネイティブへ合成入力を送るツールが検証機に無いため）
- P1 進行中: 安全なファイル操作（`app/src/ops.rs`: 上書き禁止・symlink非追跡・キャンセル対応）と、その UI 接続（`Ctrl+C`/`Ctrl+X`/`Ctrl+V`、右クリックメニュー、Esc キャンセル）を実装・テスト済み。未実装: リネームUI、進捗バー、グリッド表示、タブ、検索、ボリューム一覧、複数選択
- Windows / macOS の実機ビルド・実行は未検証（設計は `std::fs` / `dirs` / `trash` でポータブル）
- 詳細は [app/README.md](app/README.md) の「既知の制限」

## ライセンス

本リポジトリのコード・文書は **GNU Affero General Public License v3.0 or later**
（`AGPL-3.0-or-later`）です。バイナリ配布時は対応するソースの提供が必要です
（本リポジトリがそのソースです）。依存クレートはそれぞれのライセンスに従います
（gpui 系 Apache-2.0、Lucide ISC、notify CC0-1.0 ほか）。詳細は
[NOTICE.md](NOTICE.md) と [app/NOTICE.md](app/NOTICE.md) を参照してください。
