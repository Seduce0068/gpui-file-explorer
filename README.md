<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
<!-- SPDX-FileCopyrightText: 2026 Developer <developer@example.com> -->

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
cargo test           # app クレートのテストのみ（データ層・監視・P1 機能を含む。件数は実行結果を参照）
./target/debug/gpui-file-explorer [ディレクトリ]
```

全体確認はリポジトリ直下で `cargo test --workspace` を実行してください
（`core` / `app` / `poc` / `win32` を含む workspace 全体が対象。上の `cargo test` は
app クレートのみ）。

前提: Rust 1.92+、Vulkan 対応 GPU ドライバ、Wayland または X11。
Rust の 1.92 は [`app/Cargo.toml`](app/Cargo.toml) の `rust-version` に宣言した
**MSRV（下限）** です。ただし、実検証したところ
`cargo +1.92.0 check --locked --workspace --all-targets` は依存クレート
`gpui-pre-util 0.3.12` の不安定機能 `slice_as_array`（E0658）で失敗します。
そのため `rust-toolchain.toml` で 1.92 へピン留めすると CI が壊れるため、
**意図的にピン留めしていません**。実機で検証済みのツールチェーンは
rustc / cargo 1.99.1（[app/README.md](app/README.md) の「検証環境」を参照）。
CI は共通ワークフローの `rust_toolchain` 入力（既定 `stable`）を使い、
fmt / clippy は同ワークフローが `rustfmt` / `clippy` コンポーネントを明示導入するため
バージョンのばらつきは抑えられています。必要パッケージは
[app/README.md](app/README.md) の「ビルド」を参照してください
（`libxkbcommon-x11-dev` が無い環境向けの回避スクリプトを同梱）。

`Cargo.lock` はリポジトリ直下・`app/`・`poc/` の 3 箇所に存在しますが、
virtual workspace で実際に解決に使われるのは**ルートの `Cargo.lock` のみ**です。
`app/Cargo.lock` と `poc/Cargo.lock` は workspace 化前の残骸で**未使用**
（方針により削除せず、ビルド入力には使いません。`build-deb.sh` の
`--skip-build` 時は古い残骸が更新されていないか警告のみ行います）。

## キーマップの上書き

キーバインドは `~/.config/gpui-file-explorer/settings.json` の `keymap` に
`(アクション名, コード)` の組で永続化されます。**`Alt+K` でキーマップ設定
ダイアログを開き、`アクション名 コード` の形式で入力して Enter を押すと
登録できます**（例: `open_preview ctrl+shift+p`。Esc はキャンセル）。
**`unbind アクション名`** を入力して Enter を押すと、そのアクションの上書きが
削除され、出荷時の既定バインドに戻ります（例: `unbind open_preview`）。
生のキー入力を記録する方式ではありません。手編集する場合はアプリを終了した
状態で設定ファイルを編集してください。

### 設定関連のキーバインド

| キー | 動作 |
| --- | --- |
| `Ctrl+,` | 設定を開く（5 タブ。保存 / キャンセル。Alt+K / Alt+T の個別ダイアログを包含・補完） |
| `b` | 設定ダイアログのファイルタブで画像サムネイル表示を切替（修飾キーなし・入力中は無効） |
| `Alt+K` | キーマップ設定ダイアログ（個別バインドの登録・解除） |
| `Alt+T` | ターミナル起動コマンドの設定ダイアログ |

設定画面は「外観 / ファイル / ショートカット / ターミナル / 個人化」の
5 タブ構成で、「保存」で反映、「キャンセル」で破棄します。
ラジオボタン・スイッチは Tab 巡回に含まれないため、外観タブは `t`（テーマ）/
`v`（ビュー）、ファイルタブは `h`（隠しファイル）と `b`（画像サムネイル）で
直接切替できます（検索欄等の入力中は無効）。各設定行の説明にも同じキーを
記載しています。

```json
{ "version": 1, "keymap": [["open_preview", "ctrl+shift+p"]] }
```

設定画面（「ショートカット」タブ）の入力欄は **`action=chord;`** 形式です。
`;` 区切りで複数指定し（例: `open_preview=ctrl+shift+p;quit=ctrl+q;`）、
保存時に入力欄の内容が上書き一覧を**すべて置き換え**ます。削除する場合は
該当項目を入力欄から消すか、この欄でも **`unbind アクション名`** を入力して
保存します（`Alt+K` ダイアログと同じ構文で 1 件ずつ解除できます）。

- アクション名は `KeyAction` の snake_case 名（例: `open_preview` /
  `fuzzy_jump`）、コードは `ctrl` / `shift` / `alt` を `+` で連結した修飾キーと
  最後のキー名（例: `ctrl+shift+p`、`alt+left`、`f4`）。修飾キーの順番は任意です。
- 同じアクション名は UI での登録時に置き換えられ最後の入力が優先されます
  （手編集で重複させた場合は最初の 1 件が有効。最大 64 件）。
  `escape` / `f1` / `,`（カンマ）は予約キーで上書き不可（`open_settings` は
  `Ctrl+,` 固定、`unbind` も不可）。不正なコードや未知のアクション名は
  無視されます。詳細は [app/README.md](app/README.md) の
  「キーマップの上書き」を参照してください。
- 全キーバインドの一覧は [app/README.md](app/README.md) の「キーバインド」、
  またはアプリ内の `F1` ヘルプで確認できます。
- `unbind アクション名` で上書きを解除し、既定のキーバインドに戻せます。
- 一括リセットのボタンはありません（`KeyMap::reset` は UI に未配線）。
  すべて既定へ戻す場合は手編集で `keymap` を `[]` にするか削除し、個別には
  `unbind アクション名` で既定バインドへ戻します。

## リポジトリ構成

| パス | 内容 |
|---|---|
| [app/](app/) | **動く試作**（AGPL-3.0-or-later）。ツールバー / ブレッドクラム / サイドバー / 仮想リスト / 非同期ロード / 自然順ソート / 隠しファイル切替 / 履歴 / 右クリックメニュー / ゴミ箱（確認付き）/ 設定永続化 / notify 監視 |
| [core/](core/) | `gfe_core`。GPUI 非依存の純ロジック（model / search / ops / error / ports / trash_view） |
| [win32/](win32/) | `gfe-win32`。Win32 FFI を隔離（unsafe はここのみ。実機未検証） |
| [pwa/](pwa/) | iOS / Android 共通の 1 画面 PWA。core wasm API 経由の一覧・検索は受入達成（実ファイル走査は Phase 2） |
| [android/](android/) | Compose 1 画面ビューアの scaffold。SDK/JDK 不在のためビルド未検証 |
| [DESIGN.md](DESIGN.md) | 設計書。機能マトリクス P0〜P3、ASCII ワイヤーフレーム、型契約、watch→debounce→reconcile 状態機械、trash-first 安全則、AGPL 運用、受入条件 AC-1〜AC-13、ADR 12 件（変更履歴は DESIGN.md 末尾を参照） |
| [docs/adr/](docs/adr/) | [ADR 索引](docs/adr/index.md): 新要件の個別 ADR 5 件。モバイルのみ **approved（2026-11-23 承認）**、他4件は **draft・未承認**。DESIGN.md の ADR-001〜012 とは別系列 |
| [RESEARCH.md](RESEARCH.md) | 調査レポート。要点 12 項目 / 推奨スタック / クロスプラットフォームの罠 / MVP 計画 / PoC 検証結果 |
| [poc/](poc/) | スタック検証用の最小 PoC（`gpui-kit` のみ、実機でビルド・起動確認済み） |
| [notes/](notes/) | 一次調査ノート（GPUI API 棚卸し・エコシステム・OS/FS 差分）と独立検証レポート |
| [scripts/dev-setup.sh](scripts/dev-setup.sh) | `libxkbcommon-x11` 開発ファイル欠如時のローカル symlink 作成（システム変更なし） |
| [scripts/setup-e2e-input.sh](scripts/setup-e2e-input.sh) | E2E 入力ツール（`wtype` / `xdotool` / `ydotool`）を sudo なしでユーザー空間へ導入 |
| [packaging/linux/](packaging/linux/) | Linux DEB ビルド（`build-deb.sh`、`.desktop`、hicolor アイコン）。root 不要・ホストへインストールしない |
| [.github/workflows/ci.yml](.github/workflows/ci.yml) | 個人リポジトリ共通の Rust ゲート（`Seduce0068/workflows@v1`）へ委譲。`app/` に加え `extra_gates` で workspace member の `core/` / `win32/` / `poc/` とクロスターゲット（wasm32 / windows-gnu）の `cargo check` を検証 |

> `notes/` は元の開発ワークスペース（`work/gpui-file-explorer/`）で作成した調査記録です。
> 一部に `work/gpui-file-explorer/...` や `security-review/Argus`（リポジトリ外）への参照が残っています。

## 試作の検証結果（2026-11-11 実機 / テスト数は 2026-11-21 更新）

- `cargo build` 警告 0 / `cargo test` **全件 passed / 0 failed**（件数は実行結果を参照）
  （2026-11-21 時点。初回実機検証時は 35 件（M-1/2/3 修正後 36 件））
- Wayland ネイティブ起動（stderr 0 行）。KWin D-Bus で
  `caption="GPUI File Explorer"`, `resourceClass="gpui-file-explorer"`, 1100x728 を確認
- 起動中に外部からファイル作成 → 一覧が自動更新（14→15 件、出典: Lead 再検証）
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
  `README.md` / `copyright` (DEP-5) / `LICENSE` / `NOTICE.md`（アプリの詳細版） /
  `NOTICE-repo.md`（リポジトリ版の要約） / `NOTICE-app.md` / `app/NOTICE.md` / `app/LICENSE`
  （後者 2 つは `NOTICE-repo.md` の相対リンクをパッケージ内で解決するための複製）と
  `DEBIAN/postinst` / `DEBIAN/postrm`（`.desktop` の MimeType を
  `update-desktop-database` で更新する maintainer script）。
  ビルド時および `--verify` で両スクリプトの存在・**0755**・ソースとの一致を検証します。
  `--verify` はさらに `.deb` の control tarfile から `./postinst` / `./postrm` を
  取り出してステージとのバイト照合と tar 上のモード **0755** の確認も行います
  （BK18）
- `Depends` は `ldd` + `dpkg -S` から自動生成（Vulkan ローダは dlopen のため `libvulkan1` を明示追加）。
  `Recommends` には `libfontconfig1` と `desktop-file-utils`（maintainer script の
  `update-desktop-database` 用）を含みます
- `dpkg-deb --root-owner-group` を使うため **sudo 不要**。ホストへのインストールも行いません
- 再現性: `SOURCE_DATE_EPOCH` を `dpkg-deb` へ渡します（明示値を優先。未設定なら HEAD のコミット時刻）。
  これにより ar のタイムスタンプと tar の mtime が固定され、同一ソースなら `.deb` がバイト単位で再現します
- 検証済み: `dpkg-deb -x` で展開したバイナリが `--version` / GUI 起動（Wayland）で動作、
  `apt-get -s install ./...deb` の依存解決が通ること
- 署名・リポジトリ公開・タグ付けは未実施（リリースはユーザー指示があるまで行いません）

### インストール / 検証 / アンインストール

ビルドした DEB は次の 3 段で扱います（Kubuntu 向け）。

```bash
deb="packaging/linux/build/gpui-file-explorer_0.1.0-1_amd64.deb"

# 1. インストール（root 権限が必要）
sudo dpkg -i "${deb}"

# 2. 検証（ビルド成果物とステージの照合は読み取り専用）
bash packaging/linux/build-deb.sh --verify
dpkg-deb --info "${deb}"
dpkg-deb --contents "${deb}"
gpui-file-explorer --version

# 3. アンインストール
sudo dpkg -r gpui-file-explorer
```

`dpkg -r` はパッケージが配布した `/usr` 配下のファイルだけを削除します。
設定などの **`$HOME` データは残ります**
（`~/.config/gpui-file-explorer/settings.json`）。完全に消す場合は必要に応じて
手動で削除してください。再インストール時はこの設定が再利用されます。

## 既知の制限

- キーボード / 右クリック / 削除確認ダイアログの**実押下 E2E は未検証**。
  `scripts/setup-e2e-input.sh` で `wtype` / `xdotool` / `ydotool` は
  sudo なしで導入できるが、当環境では KWin が
  `zwp_virtual_keyboard_v1` 非対応、XTEST は
  「Control input devices」ポータル許可待ちで Wayland ネイティブの
  アプリへ届かない。`ydotool` を使うには root による udev ルール
  （`/etc/udev/rules.d/99-uinput.rules` に
  `KERNEL=="uinput", SUBSYSTEM=="misc", TAG+="uaccess"`）と再ログインが必要。
- 実キー入力が届かない環境でも、`GPUI_FE_DEBUG_*` フック（debug ビルドのみ）と
  XDG Desktop Portal のスクリーンショットを組み合わせれば UI 状態のローカル検証は可能
  （詳細は [app/README.md](app/README.md) の「既知の制限」）。
- P1 実装済み: 安全なファイル操作（`app/src/ops.rs`: 上書き禁止・symlink非追跡・キャンセル対応）、コピー/切り取り/貼り付け（`Ctrl+C`/`Ctrl+X`/`Ctrl+V`）、リネームダイアログ（F2、上書き拒否・失敗時は原名維持）、コピー/移動のバイト進捗表示（ステータスバー、80msスロットル）、複数選択（Ctrl/Shift/Ctrl+A）とバッチ操作（コピー/切り取り/ゴミ箱、件数付き確認）、ゴミ箱ビュー（一覧・復元）、視覚的な進捗バー（総バイト事前計算 + パーセント表示）、アイコングリッド表示（列数自動・設定永続化）、再帰検索（Ctrl+F、symlink非追跡・上限付き・キャンセル対応）、ボリューム（ドライブ）一覧（Linux /proc/mounts と Windows gfe-win32 経由のネイティブ列挙。Windows は実機未検証、macOS 未対応）、タブ（Ctrl+T/W/Tab、履歴保持）、最近使った場所（サイドバー、最大10件）、ドラッグ&ドロップ（OSからのコピー受信・フォルダ単位のドロップ先・OSへのドラッグアウト）、検索結果の絞り込み（種別・拡張子）、内容検索（行番号表示・バイナリ除外・上限付き）、ブックマーク（Ctrl+D、最大50件）、プロパティ表示（Ctrl+I、読み取り専用）、空状態・ローディング表示、キーボードヘルプ（F1）、タブ構成の永続化、プロパティのディレクトリサイズ計算（バックグラウンド・キャンセル可能）、種類（拡張子）でのソートと「種類」列、タブの並べ替え、検索履歴（チップから再利用）、パス直接入力（Ctrl+L）、テキスト/画像プレビュー（Space/F3、読み取り専用・上限付き・magic number 判定）、元に戻す/やり直す（Ctrl+Z、名前変更・ゴミ箱のみ）、検索結果の名前部分一致絞り込み（Ctrl+Shift+F）、閉じたタブの復元（Ctrl+Shift+T）、サイドバーのホームへ移動（Ctrl+Shift+L）、画像サムネイル表示（NW09、一覧/グリッドに実画像を表示。`show_thumbnails` 設定で切替可能、gpui-kit の非同期読込/キャッシュを利用、8 MiB ガード・symlink 拒否・ゴミ箱ビュー除外を適用。カスタム LRU・ローダ側バイト上限・ローディング表示は未対応）を実装・テスト済み。`#![forbid(unsafe_code)]` を app 本体に適用。未実装: 検索結果のソース表示強化、OSファイルクリップボード連携: Linux バックエンドで text/uri-list の読み書きが未対応（Windows/macOS は読み取りのみ GPUI 側に実装済み、書き込みは全プラットフォーム未対応）。空き領域の右クリックメニューは実装済み（行外の右クリックで新規フォルダ/再読込）
  - as-built（F6-38/F6-49）: 保存検索（最大50件・チップから呼び出し/削除）とブックマークラベル（右クリックで設定/削除）も実装済み。
- Windows / macOS の実機ビルド・実行は未検証（設計は `std::fs` / `dirs` / `trash` でポータブル）。
  Windows は `cargo check` レベルでは成功（警告は cfg 分岐由来の 2 件のみ。
  [review-report.md](review-report.md) の検証記録に記載）。macOS は
  上流 `gpui-pre-apple` のクロスビルド制約で check 不可。
- 配布パッケージは Linux amd64 の DEB のみ（署名・AppImage/Flatpak・Windows/macOS は未対応）
- ネイティブフォルダ選択（`prompt_for_paths`、XDG ポータル）は API 調査と
  ポータル要件の確認が必要なため未配線です。P1 の設計項目ではあるものの
  今回は実装せず、既知制限として記録のみとします
  （[DESIGN.md](DESIGN.md) §7 の OS 統合マトリクスも参照）
- 詳細は [app/README.md](app/README.md) の「既知の制限」

## 新要件（2026-11-22）

- Windows Explorer / Double Commander 並みの**見やすさ・使いやすさ**
  （二画面・列表示・コンテキストメニュー・サムネイル）を目標とする。
  Windows は第一級ターゲット（Phase1/MVP）へ昇格。ただし権限編集・
  拡張メニュー・アーカイブ・クラウド・ネットワークブラウズの
  完全互換は非目的のまま（[DESIGN.md](DESIGN.md) §1.2）。
- レスポンシブサイドバー（NW04）: ビューポート幅が 900px 以上なら 190px、
  520px 以下なら 0px、その間は線形補間。`Ctrl+B` で表示を強制した場合は
  狭い画面でも 190px（[DESIGN.md](DESIGN.md) §3.1 の as-built 記載）。
- Android は Compose アプリ＋PWA、iOS は PWA のみ（Apple Developer Program 年会費のため）。
  [モバイル対応 ADR](docs/adr/adr-20261122-mobile-android-ios-support.md) は
  approved（2026-11-23 承認、選択肢B）。[pwa/](pwa/) は core wasm API 経由の
  1 画面ビューアとして受入達成、[android/](android/) は scaffold（ビルド未検証）。
- 配布は実装されるまで従来どおり **Linux amd64 の DEB のみ**。
  Windows / macOS / モバイル向けパッケージは未対応。

## OS ファイルクリップボード

- アプリ内クリップボード（`Ctrl+C/X/V`）と D&D は対応済みですが、OS のファイルクリップボード
  （`text/uri-list`）は GPUI 0.3.12 の Linux バックエンドが読み書きしないため未対応です。
- 調査と将来対応: [notes/os-file-clipboard-notes.md](notes/os-file-clipboard-notes.md)

## WASM / ブラウザ対応

- デスクトップ試作は `std::fs` / `notify` / `trash` / OS ダイアログを使用する。
  PWA は `gfe_core` を wasm 化し in-memory JSON アダプタで一覧・検索を動かす（実ファイルは未対応）。
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

## CI の検証範囲（2026-11-23 更新）

`.github/workflows/ci.yml` は
`Seduce0068/workflows/.github/workflows/rust-gates.yml@v1` に委譲し、`app/` で
fmt → check → test → clippy → `cargo audit` を実行する。リポジトリは
`core` / `app` / `poc` / `win32` を member とする virtual workspace であり、
共有ワークフローの固定ゲートは `app/` のみなので、残りは `extra_gates` で
明示的に回す。

`extra_gates` の内容（2026-11-23 時点）:

- `app/` の release check/test（`#[cfg(not(debug_assertions))]` 分岐の検証）。
- workspace member の `core` / `win32` の test と clippy。
  `win32` は FFI 境界なので `-D clippy::undocumented_unsafe_blocks` を付与。
- クロスターゲットの `cargo check`（リンカ不要）:
  `gpui-file-explorer-core` を `wasm32-unknown-unknown`、
  `gfe-win32` を `x86_64-pc-windows-gnu` に対して検査する。
  ターゲットは共有ワークフローの `wasm_targets` 入力で `rustup target add` する。
- `poc/` の fmt / check(`--locked`) / test(`--locked`) / clippy(`-D warnings`)。

サプライチェーン監査は別ジョブ `supply-chain` で実行する（共有ワークフローに
`cargo deny` / `cargo vet` の入力が無いため）。`app/` と `poc/` の両方に
`cargo deny check`（全チェック）を実行する。`poc/` も Cargo.toml に license を
宣言済みであり、licenses を除外する前提は解消された。`cargo vet check` は
`app/supply-chain/config.toml` が存在する場合のみ実行する。

- 検証（Runtime Proof）: 追加ゲートを含む CI が **success**
  （run 43734742786、12分22秒）— 2026-11-16 時点のスナップショット。
- 2026-11-17 時点のスナップショット（現在の as-built ではない）: コミット
  `19b5f1d` に対する CI run 44063705679 が **success**
  （8m17s、2026-11-17T19:27+09:00 完了）。
  [review-report.md](review-report.md) の承認記録に記載。
- ローカルでも同一コマンド列の再現を確認（`app/` を cwd に `bash -e` で実行、
  poc 4 tests pass）。

### ローカル検証ループ

CI と同じ範囲を手元で回すスクリプトを用意しています（差分: ローカルは poc/ にも
`cargo audit` を実行します。`cargo vet check` は各プロジェクトに
`supply-chain/config.toml` が存在する場合のみ実行し、CI の `supply-chain` ジョブも
`app/supply-chain/config.toml` が存在する場合のみ実行します）。
既定では release 検証も
実行します。

`--app` / `--poc` の scope に関わらず、配布成果物のゲートも実行します。

- **PWA**: `node --check pwa/sw.js` と
  `node --input-type=module --check < pwa/main.js`（node が有れば
  `node:test` 非対応でも構文検証）に加え、`sw.js` の `PRECACHE` 配列を `python3` で
  パースして列挙された全アセットの実在を照合し、`node --test pwa/main.test.js`
  を実行します。pure ヘルパーと snippet の形式・一意性、および
  `pkg/snippets/<hash>/inline0.js` の `existsSync` は常に検証し、
  `pwa/pkg/`（wasm-pack の生成物・gitignore 済み）が無い clean checkout では
  生成物のハッシュ一致と pkg 配下のアセット存在確認のみ `SKIPPED` にします。
- **DEB**: `packaging/linux/build-deb.sh --verify`（読み取り専用）。
  `packaging/linux/build/` も gitignore 済みのため、ビルド済み `.deb` が無い
  clean checkout ではゲート列の先頭で `SKIPPED` を表示し、DEB 検証ゲート
  自体は実行しません。ただしコードゲート（Rust / PWA / Android）は通常どおり
  実行されます（先頭の通知は後続ゲートのスキップを意味しません）。
  追跡対象の `.desktop` に対する `desktop-file-validate` は `.deb` の有無に
  依存せず常に実行します。
- **Android**: `python3` 標準ライブラリで AndroidManifest.xml の well-formedness と
  構造（`<manifest>` / `<application>` / MAIN+LAUNCHER intent-filter と
  `android:exported` の明示）を検証します。さらに `@style/...` や
  `@string/` / `@color/` / `@dimen/` / `@bool/` / `@integer/` / `@array/` の
  リソース参照が `res/values*/*.xml` の対応する定義で解決すること
  （修飾子ディレクトリを含む。`@android:` の framework 参照は対象外）、
  相対 Activity 名が `namespace`（無ければ `applicationId`）で解決し、
  `src/main/{java,kotlin}/` の Kotlin ファイルと package 宣言が一致することも
  静的に検証します。`settings.gradle.kts` / `build.gradle.kts` /
  `app/build.gradle.kts` の固定バージョン文字列と Kotlin Android と
  Compose compiler のバージョン一致も確認します（ビルドは行いません）。
  scaffold が見つからない場合は `SKIPPED` です。

いずれの `SKIPPED` も `--strict` ではハード失敗になります（後述）。
CI は Rust 系ゲートのみで、PWA / DEB / Android は対象外です。

```bash
bash scripts/check.sh              # app/poc: fmt/check/test/clippy +
                                   # core/win32: test/clippy +
                                   # app の release check/test +
                                   # wasm32 / windows-gnu の cross check +
                                   # cargo audit / deny / desktop-file-validate
                                   # + cargo vet（各 supply-chain/config.toml が
                                   # 存在する場合のみ）
bash scripts/check.sh --fast       # audit と vet を省略。deny は advisories を
                                   # 省きオフラインで licenses/bans/sources のみ。
                                   # --no-release も暗示（release 検証を省略）。
bash scripts/check.sh --no-release # app の release check/test のみ省略。
bash scripts/check.sh --strict     # 必要なツール／成果物が無い時に失敗
                                   # （既定は SKIPPED）。ツールは cargo
                                   # audit / deny / vet に加え node と
                                   # node:test、python3、dpkg-deb / dpkg /
                                   # realpath / tar / stat（--verify が使う
                                   # 5 コマンド）と desktop-file-validate、
                                   # 成果物はビルド済み .deb、
                                   # pwa/pkg/snippets/（PRECACHE の生成物）
                                   # と android/ scaffold が対象。追跡済みの
                                   # .desktop は成果物ではなく常に検証します。
bash scripts/check.sh --app        # app/ + core/win32 + cross-target checks（release 検証を含む）。
bash scripts/check.sh --poc        # poc/ に加え、workspace member の core / win32 の
                                   # test+clippy と wasm32 / windows-gnu の
                                   # クロスターゲット check も実行（app/ は省略）。
```

- 検証（Runtime Proof）: 完全ループが **exit 0**（clippy 警告 0 /
  `cargo audit` は既知 8 件の allowed warnings のみ）。テスト数の現在値は
  `cargo test` 結果を参照（固定値を記載すると drift するため）。
- clean checkout では PWA / DEB の成果物ゲートが `SKIPPED` になります
  （`--strict` を付けると両方とも失敗します）。これは生成物が gitignore 済み
  であることによる想定挙動で、コードゲートの失敗ではありません。Android の
  scaffold は追跡対象のため通常は実行されますが、ファイルが欠けた場合は
  同様に `SKIPPED`（`--strict` では失敗）になります。
- CI が正であることに変わりはなく、このスクリプトは push 前の確認用です。

### OFFLINE / headless 検証

headless 環境では `scripts/check.sh --fast` で cargo audit / vet と deny の
advisories（RustSec DB 依存）を省略可能。GUI 検証は
`DISPLAY=:1 WAYLAND_DISPLAY=wayland-0` + spectacle + kdotool で
スクリーンショット取得可（実キー入力は KWin ポータル制約で未検証）。

## ソース・ライセンス

本プロジェクトは **AGPL-3.0-or-later** です。改変版をネットワーク経由で
提供する場合（将来の WASM / Web 版を含む）は、利用者がソースを入手できる
ようこの導線を維持してください。

- ソース: [github.com/Seduce0068/gpui-file-explorer](https://github.com/Seduce0068/gpui-file-explorer)
- ライセンス: [LICENSE](LICENSE)（AGPL-3.0-or-later）
- 第三者クレジット: [NOTICE.md](NOTICE.md) / [app/NOTICE.md](app/NOTICE.md)
- アプリ内にも F1 ヘルプの「ライセンス / ソース」から同じ URL を表示します。
