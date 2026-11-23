<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# pwa/ — core API ビューア（iOS / Android 共通）

`@gpui-file-explorer/core`（クレート名 `gpui-file-explorer-core`、lib 名 `gfe_core`）を
wasm 化し、**core API 経由**で一覧と検索を表示する 1 画面 PWA の scaffold。
React 等の新規依存は使わず、Vanilla JS の静的ファイルのみで構成する。

## 受け入れ基準

**Phase 1 spike の受け入れは外観ではなくコア API 経由の一覧・検索表示。**
2026-11-23 時点で達成済み: `wasm-pack build` 後に Playwright で
`http://127.0.0.1:8765/index.html` を開き、一覧 4 件と検索 1 件が
`core API 経由` ステータスで表示されることを確認した（Real Proof）。
`pkg/` が無いときの内蔵サンプルは scaffold 確認専用の読み取り専用デモ。

## in-memory API と実ファイルの違い

wasm32 の `std::fs` はブラウザで実ファイルを読めないため、
`listEntriesFromJson` / `searchInJson` に JSON 化したサンプルを渡し、
コアの `sort_entries` / `visible_entries` / 名前検索を実際に通している。
実ファイルの走査には OPFS / FileSystem ポートが必要（Phase 2 以降）。

## 前提（core 側の実装）

別エージェントが `core/` に wasm-bindgen API を実装する前提で配線している。
ビルド前に以下が揃っていること:

- `core/Cargo.toml` の `[lib]` に `crate-type = ["cdylib", "rlib"]`
  （wasm-pack 0.13.1 は `crate-type must be cdylib` を要求する）
- `wasm-bindgen`（必要なら `serde_wasm_bindgen`）依存の追加
- `CoreViewer` の公開と `list_entries` / `search` メソッド

lib 名は `gfe_core` なので、生成される JS は `pkg/gfe_core.js`、
wasm は `pkg/gfe_core_bg.wasm` になる（`main.js` の `WASM_MODULE` で指定）。

実装済みの API は次のとおり（`core/src/wasm.rs` を実読）:

- `CoreViewer.new()` — 引数なし（wasm-bindgen の private constructor のため `new CoreViewer()` は不可）
- `list_entries(dir: string)` — JSON 文字列（camelCase の配列）を返す
- `search(query: string, dir: string)` — `{ entry, matchLine }[]` の JSON 文字列を返す
- `set_panic_hook()` — panic を `console.error` に流す（初期化時に呼ぶ）

**既知の制約**: 両メソッドは `std::fs` を呼ぶため、ブラウザの wasm では実ファイルを
読めずエラー文言の文字列が返る。`main.js` はその文言をステータスに表示し、
scaffold 確認用の読み取り専用デモへ退避する。コア API 経由の実データ表示には
FileSystem ポート（OPFS 等）が必要で、デモ表示は受け入れではない。

## ビルドと起動

Kubuntu 上でリポジトリルートから実行する（外部送信なし、`npm install` 不要）:

```bash
echo '--- wasm ビルド ---'
wasm-pack build core --target web --out-dir pwa/pkg --release

echo '--- 生成物の確認 ---'
ls -la pwa/pkg
# gfe_core.js / gfe_core_bg.wasm / gfe_core.d.ts が並ぶこと

echo '--- ローカル配信 ---'
python3 -m http.server 8000 --directory pwa
# ブラウザで http://localhost:8000/ を開く
```

`--out-dir` は相対パス指定。配置先が `pwa/pkg` 以外になった場合は
`core/` から `../pwa/pkg` を指定し、`ls` で実際の配置を確認すること。

`file://` 直接開きは ES module と wasm の MIME で失敗するため、必ず HTTP 配信を使う。

## ファイル選択とブラウザ対応

| 機能 | 対応ブラウザ | 動作 |
| --- | --- | --- |
| 一覧・検索 | すべて | core API（`pkg/` がある場合） |
| フォルダ選択 | FSA API 対応ブラウザ | `showDirectoryPicker()` の結果を利用 |
| フォルダ選択 | 非対応（**iOS Safari** を含む） | ボタンを非表示にし、内蔵サンプルの読み取り専用デモ |

**iOS Safari は File System Access API に非対応。** Apple Developer Program の
年会費が必ずかかるため iOS はアプリ版を出さず PWA のみ対応する方針に沿い、
ホーム画面への追加（PWA インストール）までは可能だが、実ファイルの選択はできない。
現状はルート名だけを core へ渡しており、実ツリーの走査には OPFS 等の
FileSystem ポートが必要（`notes/wasm-browser-notes.md` §2 を参照）。

## キーボード操作とアクセシビリティ

一覧は `role="listbox"` を持ち、`aria-labelledby="list-heading"` で見出しと紐づく。
ポインタ操作には必ずキーボードの代替経路を用意している。

| キー | 動作 |
| --- | --- |
| `↑` / `↓` | 選択行を上下へ移動 |
| `Home` / `End` | 先頭 / 末尾の行を選択 |
| `Enter` | 選択を確定 |
| `Space` / `i` | 選択行の詳細（`path` / `size` / `isDir`）をステータスへ表示（長押し・右クリックと同じ） |

`Ctrl` / `Cmd` / `Alt` との同時押しはブラウザ / OS のショートカットへ譲り、
一覧のハンドラは横取りしない。キーボードで選択を移動したときだけ
`scrollIntoView({ block: 'nearest' })` で viewport 内へ追従する。ポインタの
タップでは `preventScroll` のままスクロール位置を動かさない。

一覧の 📁 / 📄 / ↗ は `aria-hidden="true"` の span に分離しているため、
`role="option"` のアクセシブル名にはファイル名とメタ情報だけが含まれる。
長押し行では `user-select` / `-webkit-touch-callout` を無効化し、検索 input の
テキスト選択には影響しない。

右方向へ 80px 超スワイプすると検索をクリアして一覧へ戻る。左端 24px から
始まるジェスチャは OS の戻る操作へ譲る。`touchmove` の 10px 超過で立つ `moved`
は長押し / クリック抑止にのみ使い、スワイプ判定は `touchend` の移動量で行う。

エラー文言の色は `--error` トークンに集約し、ライト / ダーク双方の背景で
WCAG AA（コントラスト比 4.5:1 以上）を満たす値を指定する。

## 空状態と検索クリアの導線

一覧が 0 件のときの文言は pure helper `emptyStateText(query, mode)` が状態ごとに決める。

| 状態 | 文言 | 「検索をクリア」ボタン |
| --- | --- | --- |
| クエリありの検索0件 | `「<クエリ>」に一致する項目はありません` | 表示（クエリをエコー） |
| クエリなしの検索0件 | `検索に一致する項目はありません` | 表示 |
| 一覧そのものが空 | `項目がありません` | 非表示 |

文言は `textContent` で設定するため、クエリに HTML が入っていてもタグとして解釈されず
XSS にならない。「検索をクリア」ボタンは検索0件のときだけ描画し、押すと
`clearSearch()` が検索欄・選択状態をクリアして一覧へ戻る。右方向スワイプの
`onSwipeBack()` も同じ `clearSearch()` を共有し、スワイプ時だけ直後の click を抑止する。

デスクトップ版は 0 件ヒントにキーボード導線（検索欄のクリア操作）を添えるが、
PWA はタップ操作を前提にボタンで復帰できるようにしている。ボタンのタップ領域は
44px 以上で、`focus-visible` 時に outline が表示される（一覧の行と同じ規則）。

## 構成

| ファイル | 役割 |
| --- | --- |
| `index.html` | 検索ボックス + 一覧の 1 画面 |
| `main.js` | `pkg/` を動的 import し `CoreViewer` を初期化、一覧・検索を DOM へ反映 |
| `style.css` | ダーク / ライト自動切替（既定は `prefers-color-scheme`）+ iOS セーフエリア対応 |
| `manifest.webmanifest` | PWA マニフェスト（standalone / SVG 内蔵） |
| `.gitignore` | 生成物 `pkg/` を除外 |

`pwa/` と `android/` はルート `Cargo.toml` の workspace members には入れない。
