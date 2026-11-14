# WASM / ブラウザ対応の調査（2026-11-14）

- 対象: 本リポジトリの GPUI ファイルエクスプローラー試作をブラウザ（wasm32）で動かせるか
- 検証環境: Kubuntu 2026.04 / rustc 1.99.1 / wasm32-unknown-unknown ターゲット導入済み
- 結論: **GPUI 自体は wasm ビルド可能（条件付き）だが、現試作はそのままではブラウザで動かない。**
  移植にはファイルシステム抽象化の作り直しが必要で、P1/P2 の機能追加とは別プロジェクト相当。

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

一方、`gpui-kit = "=0.9.1"` をそのまま wasm ターゲットでビルドすると **stable では失敗**する:

```text
error[E0554]: `#![feature]` may not be used on the stable release channel
 --> wasm_thread-0.3.3/src/lib.rs:1:37
1 | #![cfg_attr(target_arch = "wasm32", feature(stdarch_wasm_atomic_wait))]
```

`gpui-pre-platform` の既定 feature `multithreaded` が `wasm_thread`（nightly 必須）を有効にするため。
回避策は (a) nightly ツールチェーンを使う、(b) `gpui-kit` を使わず
`gpui-pre` + `gpui-pre-platform` を `default-features = false` で直接使う、のいずれか。

### 1-2. 現試作アプリの wasm コンパイル

```text
$ cd app && cargo check --target wasm32-unknown-unknown
error[E0433]: cannot find module or crate `platform` in this scope   (trash crate)
error[E0599]: no method named `delete_all_canonicalized` ...          (trash crate)
error: could not compile `trash` due to 3 previous errors
```

`trash` が wasm 非対応で即失敗する。他にも `notify`（OS 通知）、`dirs`、`std::fs` の実ファイル操作、
`std::process::id`、`std::thread`、OS ダイアログなど、デスクトップ前提の依存・API が多い。

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
- gpui-kit README: WebAssembly 対応をうたうが、stable で使うには feature 調整が必要（本ノート §1-1）
- 検証コマンドは §1 のとおり。`wasm32-unknown-unknown` ターゲットは導入済み
