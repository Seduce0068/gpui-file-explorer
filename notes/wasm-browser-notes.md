---
type: Report
title: "WASM / ブラウザ対応の調査"
description: >-
  GPUI ファイルエクスプローラー試作の wasm32 対応可否を、ローカル crate
  マニフェストと現行ツリーの cargo check で再検証した記録。
tags:
  - wasm
  - browser
  - gpui-kit
  - feasibility
generated:
  by: "wasm-browser-notes"
  at: "2026-11-14T16:48:57+09:00"
verified:
  by: "wasm-investigation"
  at: "2026-11-19T07:09:43+09:00"
status: "review"
---

## 概要

- 対象: 本リポジトリの GPUI ファイルエクスプローラー試作をブラウザ（wasm32）で動かせるか
- 検証環境: Kubuntu 2026.04 / rustc 1.100.0（2026-11-19 再検証時点）/
  wasm32-unknown-unknown ターゲット導入済み
- 結論: **GPUI 自体は wasm ビルド可能（条件付き）だが、現試作はそのままではブラウザで動かない。**
  移植にはファイルシステム抽象化の作り直しが必要で、P1/P2 の機能追加とは別プロジェクト相当。
- 2026-11-19: gpui-kit 0.9.1 の feature 表を実読し、現行ツリーで wasm check を再実行
  （§1-1 / §1-2）。旧 §1-2 の `trash` 起因という記録は現在のツリーでは再現しない。

## 1. 検証結果（Runtime Proof）

### 1-1. GPUI スタックの wasm コンパイル

`gpui-pre` + `gpui-pre-platform` を `default-features = false`（`multithreaded` を無効）で
依存した最小 crate は **stable ツールチェーンで wasm32-unknown-unknown にコンパイル成功**:

```toml
[dependencies]
gpui = { package = "gpui-pre", version = "=0.3.12", default-features = false }
gpui_platform = { package = "gpui-pre-platform", version = "=0.3.12", default-features = false }
```

```text
$ cargo check --target wasm32-unknown-unknown
    Checking gpui-pre-web v0.3.12
    Checking gpui-pre-platform v0.3.12
    Finished `dev` profile ... in 55.12s
```

### 1-1-b. gpui-kit 0.9.1 に安定パスはあるか（2026-11-19 調査）

結論: **`gpui-kit` を使う安定パスは存在しない。** feature フラグで
`multithreaded` / `wasm_thread` を無効化できない（ローカル crate の実読）。

- `gpui-kit-0.9.1/Cargo.toml:39-235` の `[features]` 表に
  `multithreaded` / `wasm_thread` は存在しない。既定は `:46-49` の
  `default = ["component", "assets"]`。
- 同 `:326-334`（`default-features = false` **不在**）:

  ```toml
  [target.'cfg(not(any(target_os = "ios", target_os = "android")))'.dependencies.gpui_platform]
  version = "=0.3.12"
  features = ["font-kit", "x11", "wayland", "runtime_shaders"]
  package = "gpui-pre-platform"
  ```

- 同 `:341-343`（こちらも `default-features = false` **不在**）:

  ```toml
  [target.'cfg(target_family = "wasm")'.dependencies.gpui_web]
  version = "=0.3.12"
  package = "gpui-pre-web"
  ```

- `gpui-pre-platform-0.3.12/Cargo.toml:40,43`:
  `default = ["multithreaded"]` /
  `multithreaded = ["gpui_web/multithreaded"]`。
- `gpui-pre-web-0.3.12/Cargo.toml:38-42,120-124`:
  `default = ["multithreaded"]` /
  `multithreaded = ["dep:wasm_thread", "scheduler/wasm-threads"]`。
  `wasm_thread = "0.3"` は `optional = true` の wasm 専用依存。

よって `gpui-kit = { version = "=0.9.1", default-features = false }` は
`component` / `assets` を外すだけで、上述 2 つの既定 feature は残る。
Cargo の feature は依存グラフ全体で和集合のため、アプリ側に
`default-features = false` の依存を追加しても `gpui-kit` 側の有効化は打ち消せない。

- 安定を選ぶなら **§1-1 の `gpui-kit` を使わない構成**が唯一（stanza/コマンドは §1-1）。
- `gpui-kit` を残すなら nightly ツールチェーン（§2-6 の SharedArrayBuffer /
  COOP/COEP も必要）か、gpui-kit 側の改修・fork が必要（本タスクの範囲外）。

### 1-2. 現試作アプリの wasm コンパイル（2026-11-19 再実行）

旧記録の `trash` 起因エラーは現行ツリーでは再現しない。エラーを次の 3 類に分ける。

Stable（素のツールチェーン）での Runtime Proof:

```bash
CARGO_TARGET_DIR=/tmp/wasm-check-r20 \
  cargo check --target wasm32-unknown-unknown \
  --manifest-path app/Cargo.toml --locked
```

- 終了コード: **101**
- 最初のエラー（`/tmp/wasm-check-r20.log:391-398`）:

  ```text
  error[E0554]: `#![feature]` may not be used on the stable release channel
   --> .../wasm_thread-0.3.3/src/lib.rs:1:37
    |
  1 | #![cfg_attr(target_arch = "wasm32", feature(stdarch_wasm_atomic_wait))]
    |                                     ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
  error: could not compile `wasm_thread` (lib) due to 1 previous error
  ```

- ログ中の `trash` 出現数は **0**（`grep -i trash` で確認）。

#### (a) 依存 gated — 現在はエラーにならない

- `app/Cargo.toml:28-34` が `trash = "5.3"` を Windows または
  Freedesktop Unix に限定（cfg 全文は `app/Cargo.toml:33`）。
  `wasm32-unknown-unknown` は `target_os = "unknown"` かつ非 unix のため
  依存グラフから外れ、上記のとおりビルドログにも現れない。
- `app/src/trash_view.rs:31-37` は同じ cfg で `platform` モジュールを切り替え、
  `:82-123` に wasm 向け no-op fallback を用意。旧エラー
  （`cannot find module or crate platform` / `delete_all_canonicalized`）は
  この gating で解消済み。

#### (b) アプリコード由来 — nightly gate を通過させた場合のみ観測

依存エラーで停在するため、リポジトリを変更せず環境変数だけで gate を通す
probe を実施（調査専用。**推奨手順ではない**）:

```bash
CARGO_TARGET_DIR=/tmp/wasm-check-r20 RUSTC_BOOTSTRAP=1 \
  cargo check --target wasm32-unknown-unknown \
  --manifest-path app/Cargo.toml --locked
```

- 終了コード: **101**。全依存（`gpui-kit` / `gpui-pre-web` / `notify` / `dirs`
  等）は checking を通過し、アプリ本体で初エラー:

  ```text
  error[E0423]: cannot find value `AllAssets` in crate `gpui_kit::assets`
      --> src/main.rs:9267:69
       |
  9267 |     let app = gpui_kit::application().with_assets(gpui_kit::assets::AllAssets);
       |                                                                     ^^^^^^^^^
  ```

- 原因: `gpui-kit-assets-0.9.1/src/lib.rs:41` は native で unit struct
  `AllAssets` を公開するが、wasm は `:48` の
  `pub use wasm_assets::Assets as AllAssets;`（`wasm_assets.rs:9-13` の
  非 `pub` フィールド構造体）のため値として参照できない。
  `AllAssets::default()` 等の修正が必要だが、ソース変更は本タスク外。
- `notify` / `dirs` は wasm 向けにもコンパイル成功（ログに
  `Checking notify v8.2.2` / `Checking dirs v6.0.0`）。
  `std::fs` / `std::process::id` / `std::thread` 等のデスクトップ前提 API は
  静的には残るがコンパイラ到達前で**未検証**（推測として扱う）。

#### (c) gpui-kit の nightly feature — 通常の最初のエラー

- 素の stable check の最初のエラーは (c) で、`wasm_thread` 0.3.3 の
  `#![feature(stdarch_wasm_atomic_wait)]` に起因（依存チェーンは §1-1-b）。
- つまり現行ツリーの wasm ビルドは **(c) → (b) の 2 段で失敗**し、
  `trash` は (a) として gated され失敗要因に含まれない。

### 1-3. Web バックエンド（`gpui-pre-web` 0.3.12）の実装範囲

取得した crate ソース（`gpui-pre-web-0.3.12/src`）で確認:

| API | Web 実装 |
|---|---|
| 描画 | wgpu（WebGPU / WebGL バックエンド選択、`WebBackendPreference`）。CJK/絵文字の Canvas フォールバックあり |
| `open_url` | あり（`window.open`） |
| クリップボード | テキスト・画像の読み書き（async、secure context 前提）。**ファイルパス（ExternalPaths）は対象外** |
| `prompt_for_paths` / `prompt_for_new_path` | **未対応**（`"not supported on the web"` を返す） |
| `reveal_path` / `open_with_system` | **no-op** |
| アプリメニュー | no-op |
| 外部ファイルドラッグ&ドロップ | 専用実装は見当たらず（未確認） |
| マルチスレッド | `multithreaded` feature（nightly + SharedArrayBuffer 等が必要） |

## 2. ブラウザで動かす場合に必要な設計変更

1. **ファイルシステム抽象化**: `std::fs` 直呼びを trait（`FileSystem`）に置き換え、
   - デスクトップ: 既存の `std::fs`
   - ブラウザ: **File System Access API**（`showDirectoryPicker`、ユーザー操作起点）または
     **OPFS**（Origin Private File System）。いずれも非同期 API のため、読み込み経路を async 化
2. **監視の代替**: inotify 等は無いため、ポーリング（例: 2〜5 秒ごとのスナップショット比較）に置換
3. **ゴミ箱**: ブラウザにゴミ箱は無い。削除は「確認 + 完全削除」または「アプリ内ゴミ箱（OPFS 退避）」を自作
4. **OS 統合の削除**: 「既定アプリで開く」「ファイルマネージャで表示」「ゴミ箱」は不可。
   代わりに「ダウンロード」「共有」「コピー」に置き換え
5. **パス概念の違い**: ブラウザのファイルハンドルはパス文字列ではなくハンドル。仮想パス/ID 管理が必要
6. **ビルド/配布**: `wasm-bindgen` + `trunk` または `wasm-pack`、`index.html`、WebGPU/WebGL 対応ブラウザ。
   マルチスレッドを使う場合は nightly と COOP/COEP ヘッダ設定

## 3. 判断

- **現時点の推奨: デスクトップ専用のまま**。ブラウザ対応は「別フロントエンドを1本作る」規模で、
  P1/P2 の機能追加（タブ・ボリューム等）と競合する。
- ブラウザで見せたいだけなら、次の順が現実的:
  1. **読み取り専用の Web デモ**（OPFS/サンプルツリーを表示、検索・グリッドのみ）を
     `gpui-pre-web` + `default-features = false` で別 crate として作る
  2. デスクトップ版をリモート（WebRTC/VNC）で見せる（wasm 化しない）
  3. 本格対応するなら `FileSystem` trait を先に導入し、デスクトップ実装を回帰させない形で進める

## 4. 参考

- `gpui-pre-web` 0.3.12（crates.io、Apache-2.0）: `gpui-pre-web-0.3.12/src/platform.rs`（capabilities）
- `gpui-pre-platform` 0.3.12: wasm ターゲットで `gpui-pre-web` を選択、`multithreaded` feature
- gpui-kit 0.9.1 の `[features]` / wasm 依存: feature 調整では stable 化できない
  （本ノート §1-1-b、マニフェスト引用付き）
- 検証コマンドは §1 のとおり。`wasm32-unknown-unknown` ターゲットは導入済み
