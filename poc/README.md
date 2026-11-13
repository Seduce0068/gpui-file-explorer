# GPUI File Explorer PoC

`gpui-kit` 0.9.1（内部で `gpui-pre` 0.3.12 = Zed GPUI snapshot + `gpui-pre-platform` 0.3.12）で
作った最小限のファイルエクスプローラ。クロスプラットフォーム GPUI スタックが
このマシン（Kubuntu 2026.04 / KDE Plasma / Wayland / NVIDIA GTX 1660 SUPER）で
ビルド・実行できることの検証用。

## 機能

- ウィンドウ表示（760x520、`app_id = gpui-file-explorer-poc`）
- ディレクトリ読み込みは **GPUI の background executor**（`cx.background_spawn`）で非同期実行。
  render 内ではブロッキング I/O をしない。
- `uniform_list` による仮想化リスト（`UniformListScrollHandle` + `track_scroll` で
  キーボード選択に追従してスクロール）
- 表示: 名前 / サイズ / 更新からの経過時間、ディレクトリは `DIR`
- キー操作:
  - `Up` / `Down`: 選択移動
  - `Enter`: ディレクトリに入る（ファイルならステータス表示のみ）
  - `Backspace`: 親ディレクトリへ
  - `Esc`: 終了
- ダブルクリックでディレクトリに入る
- ステータス行: 件数 / 選択項目 / 読み込み状態

## ビルド

```bash
cd poc
cargo build            # debug
cargo test             # データ層のユニットテスト（4件 / 約2秒）
cargo build --release  # 最適化ビルド（約3分 / 51 MiB）
```

### 前提（このマシンでの注意）

`gpui-kit` は `gpui-pre-platform` の `x11` feature を固定で有効にするため、
リンク時に `-lxkbcommon-x11` が必要になる。本機には実行時ライブラリ
`libxkbcommon-x11.so.0`（`libxkbcommon-x11-0`）はあるが、
`libxkbcommon-x11-dev` が無くバージョン無し `libxkbcommon-x11.so` が存在しない。

リポジトリ直下の `scripts/dev-setup.sh` を一度実行すると
`poc/native-libs/libxkbcommon-x11.so`（システムの `.so.0` への symlink）が作成され、
`poc/build.rs` がそのディレクトリを **絶対パスで** リンカ検索パスへ追加する
（CWD に依存しない）。sudo やシステムパッケージ追加はしていない。
実行時はシステムの `.so.0` が使われる。

```bash
cd gpui-file-explorer   # リポジトリ直下
bash scripts/dev-setup.sh   # app/ と poc/ の両方に symlink を作成
cd poc && cargo build
```

別解: `gpui-kit` を使わず `gpui-pre-platform` を `features = ["wayland"]` のみで直接使えば
X11 リンク要件自体を避けられる（未検証・フォールバックB）。

## 実行

```bash
# 引数なし: $HOME を開く / 引数あり: そのディレクトリを開く
./target/debug/gpui-file-explorer-poc
./target/debug/gpui-file-explorer-poc /usr/share
```

Wayland セッションでネイティブ Wayland ウィンドウとして表示される
（`DISPLAY` があっても `WAYLAND_DISPLAY` があれば Wayland が選ばれる）。

## 検証結果（2026-11-11）

- `cargo build`（依存含む初回）: 1m31s / 警告 0 / debug バイナリ 745 MiB
- `cargo build --release`: 3m05s / 51 MiB（strip 後 37 MiB）
- 実行: stderr 出力 0 行（EGL/Vulkan/Wayland エラーなし）
- ウィンドウ: KWin D-Bus で `caption="GPUI File Explorer PoC"`,
  `resourceClass="gpui-file-explorer-poc"`, 760x548, ネイティブ Wayland を確認
- スクリーンショット: `screenshot/window-crop.png`（非同期ロードで 167 entries 表示）

詳細は `../notes/poc-report.md` を参照。
