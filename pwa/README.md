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

- `set_panic_hook()` — panic を `console.error` に流す（初期化時に呼ぶ）
- `listEntriesFromJson(json)` — JSON 化したエントリ配列をコアの並び順で整列し、
  camelCase の JSON 文字列を返す（**Phase 1 の受け入れ経路**）
- `searchInJson(query, json)` — 同じ配列へコアの名前検索をかけ、
  `{ entry, matchLine }[]` の JSON 文字列を返す（受け入れ経路）
- `CoreViewer.new()` — 引数なし（wasm-bindgen の private constructor のため
  `new CoreViewer()` は不可）
- `CoreViewer.list_entries(dir)` / `CoreViewer.search(query, dir)` — `std::fs` を
  呼ぶレガシー API。wasm32 では実ファイルを読めずエラー文言の文字列が返るため、
  in-memory API が無いときのフォールバックとしてのみ使う。

**主経路は in-memory API**: `main.js` は `listEntriesFromJson` / `searchInJson` に
JS 側のサンプル JSON（`demoJson()`）を渡し、コアの `visible_entries` / 名前検索を
実際に通す。一覧は `core API 経由`、検索は `core API の検索で N 件` の
ステータスで表示され、これが Phase 1 の受け入れ条件を満たす。
`std::fs` 経路が失敗したときの読み取り専用デモへの退避はフォールバックであり
受け入れではない。実ファイルの走査には OPFS / FileSystem ポートが必要
（Phase 2 以降）。

## ビルドと起動

Kubuntu 上でリポジトリルートから実行する（外部送信なし、`npm install` 不要）:

```bash
echo '--- wasm ビルド ---'
wasm-pack build core --target web --out-dir pwa/pkg --release

echo '--- 生成物の確認 ---'
ls -la pwa/pkg
# gfe_core.js / gfe_core_bg.wasm / gfe_core.d.ts が並ぶこと

echo '--- ローカル配信 ---'
python3 -m http.server 8765 --directory pwa
# ブラウザで http://127.0.0.1:8765/index.html を開く
```

`--out-dir` は相対パス指定。配置先が `pwa/pkg` 以外になった場合は
`core/` から `../pwa/pkg` を指定し、`ls` で実際の配置を確認すること。

`file://` 直接開きは ES module と wasm の MIME で失敗するため、必ず HTTP 配信を使う。

## オフライン起動（service worker）

`sw.js` が静的 5 ファイル（`/`、`index.html`、`manifest.webmanifest`、
`main.js`、`style.css`）と `pkg/gfe_core.js` / `pkg/gfe_core_bg.wasm` /
`pkg/snippets/<hash>/inline0.js` を precache する。`main.js` の `boot()` が
`./sw.js` を登録する。静的シェルの更新を永続反映するため、install では
`skipWaiting()` を呼ばず、activate で `clients.claim()` により制御を取る。

precache は 1 ファイルでも失敗したら install を失敗させ、旧 SW / 旧キャッシュを
維持する（部分的なキャッシュでオフラインが黙って死ぬのを防ぐため）。安全な
シェル更新は次の手順で行う。

1. 静的シェルを変更したら `sw.js` の `CACHE_NAME` をインクリメントする
   （現在 `gfe-viewer-v2`）。SW のバイト列が変わるため `updatefound` が発火する。
2. 更新された SW が `installed` になると、`main.js` が `#status`（aria-live）に
   更新通知を出し、「新しい版を読み込む」ボタンを表示する。
3. ボタンは待機中の SW へ `SKIP_WAITING` を送り、`controllerchange` で再読込する。
   通知を無視しても、次回ページを開いたときに待機 SW を検出して再通知する。

`activate` が削除するのは `gfe-viewer-` で始まる自アプリのキャッシュだけ。
他 SW のキャッシュは残す。ナビゲーション要求は network-first（失敗時は
`ignoreSearch` 付きで `index.html` へフォールバック）のため、オンラインでは
常に最新のシェルを取得する。静的アセットは cache-first だが、runtime 保存は
既知の拡張子に限定し、クエリ付き URL は保存しない。さらに runtime キャッシュは
件数上限（32 件）と TTL（24 時間）を設けて無限増殖と stale 化を防ぐ。外部送信は行わない。

wasm-pack の再ビルドで snippet のハッシュが変わった場合は `sw.js` の
precache パスも更新してください（`ls pwa/pkg/snippets` で確認）。
マニフェスト名は `GFE Viewer` に統一し、`index.html` の `<link rel="icon">` が
同じ SVG data URI を流用するため favicon の 404 は出ない。

オフラインの受け入れは Playwright で確認する。オンラインで一覧 4 件を読み、
`navigator.serviceWorker.ready` を待ってから `setOffline(true)` → reload し、
一覧 4 件が再描画されることを見る。更新通知は `sw.js` を書き換えて
`CACHE_NAME` を進めた後にリロードし、`#status` の通知と
`#update-reload` の表示を確認する。`sw.js` の precache 一覧と登録呼び出しは
`node --test pwa/main.test.js` で固定している。

## ファイル選択とブラウザ対応

| 機能 | 対応ブラウザ | 動作 |
| --- | --- | --- |
| 一覧・検索 | すべて | core API（`pkg/` がある場合） |
| フォルダ選択 | FSA API 対応ブラウザ | `showDirectoryPicker()` の結果を利用 |
| フォルダ選択 | 非対応（**iOS Safari** を含む） | ボタンを非表示にし、内蔵サンプルの読み取り専用デモ |

**iOS Safari は File System Access API に非対応。** Apple Developer Program の
年会費が必ずかかるため iOS はアプリ版を出さず PWA のみ対応する方針に沿い、
ホーム画面への追加（PWA インストール）までは可能だが、実ファイルの選択はできない。
現状は選択したフォルダ名を `state.rootName` に保持するだけで、一覧・検索は
`demoJson()` の in-memory API 経路で表示する（ルート名は core へ渡さない）。
実ツリーの走査には OPFS 等の FileSystem ポートが必要
（`notes/wasm-browser-notes.md` §2 を参照）。

## キーボード操作とアクセシビリティ

一覧は `role="listbox"` と `aria-multiselectable="true"` を持ち、
`aria-labelledby="list-heading"` で見出しと紐づく。
ポインタ操作には必ずキーボードの代替経路を用意している。

| キー | 動作 |
| --- | --- |
| `↑` / `↓` | フォーカス行を上下へ移動（通常モードでは選択も追従） |
| `Home` / `End` | 先頭 / 末尾の行へ移動 |
| `Enter` | 選択を確定（選択モード中は当該行をトグル / 範囲選択） |
| `Space` / `i` | フォーカス行の詳細（`path` / `size` / `isDir`）をステータスへ表示 |
| `Escape` | 範囲選択の arm → 選択モード → 選択状態の順に一段ずつ解除 |

詳細表示はタッチの長押しから `Space` / `i`（マウスは右クリック）へ退避した。
長押しは「選択モードへ突入 + 当該行をトグル」として扱う。
`Ctrl` / `Cmd` / `Alt` との同時押しはブラウザ / OS のショートカットへ譲り、
一覧のハンドラは横取りしない。`Shift` はナビゲーションキー（`↑` / `↓` /
`Home` / `End`）だけを譲り、`Shift+I`（大文字入力）は詳細表示として受理する。
デスクトップ版の Ctrl トグル / Shift 範囲 / Ctrl+A の代わりに、下記の専用ボタンを使う。
キーボードで移動したときだけ `scrollIntoView({ block: 'nearest' })` で viewport 内へ
追従する。ポインタのタップでは `preventScroll` のままスクロール位置を動かさない。

一覧は検索やソートで描き直されるが、再描画前に一覧内へフォーカスがあった場合は
同じ行（空状態では検索欄）へフォーカスを戻すため、`activeElement` が `body` へ
落ちない。検索0件の「検索をクリア」を `Enter` で押した後も同様に復元される。

一覧の 📁 / 📄 / ↗ は `aria-hidden="true"` の span に分離しているため、
`role="option"` のアクセシブル名にはファイル名とメタ情報だけが含まれる。
長押し行では `user-select` / `-webkit-touch-callout` を無効化し、検索 input の
テキスト選択には影響しない。

## 複数選択（選択モード）

`#select-mode` トグルを押すと選択モードへ入り、`aria-pressed` と表示が切り替わる。
選択モード中は次の導線が使える（いずれも修飾キーを必要としない）。

| 操作 | 相当するデスクトップ操作 | 動作 |
| --- | --- | --- |
| 行をタップ / `Enter` | Ctrl+クリック | pure helper `selectionToggle` で当該行をトグル |
| 「範囲選択」→ 開始行 → 終端行 | Shift+クリック | `selectionRange(entries, anchor, target)` で両端込みの範囲を選択 |
| 「すべて選択」 | Ctrl+A | `selectionSelectAll` で表示中の全行を選択 |
| 「選択を解除」 | 選択クリア | `selectionClear` で全解除 |
| 行を長押し | — | 選択モードへ突入し、当該行をトグル |
| `Escape` | Esc | 範囲 arm → 選択モード → 選択の順に脱出 |

「範囲選択」はトグルで arm するが、**1 回の範囲選択で必ず解除**される
（pure helper `nextRangeArmed(current, { completed: true })`）。完了時にボタンの
`aria-pressed` を更新し、ステータスへ終了を通知する。arm 中に `Escape` を押しても解除できる。

選択は object 参照ではなく **path** で保持する（core の `Selection` と同じ不変条件）。
再描画時に `retainSelectedPaths(prev, next)` が新一覧へ残った path だけを再選択するため、
検索欄へ 1 文字入力しても一致が残る行の選択は消えない。検索で消えた行の選択だけが外れる。
選択モードを終了すると選択状態もクリアする。

選択件数は `#select-count` の `aria-live="polite"` が通知する。`#status`（`role="status"`
のライブリージョン）は操作内容だけを読み上げ、件数は重複して含めないため、
同じ変化が 2 回読み上げられることはない。

## 並び替え

core wasm API（`list_entries` / `search`）は既定の並び順固定で、ソート条件を
JS から指定できない（BA18）。PWA 単体の pure helper `sortEntries(entries, key, dir)`
で表示時に並べ替える。コントロールは一覧上部の「並び替え」セレクトと方向セレクト。

- `key`: `name`（名前）/ `kind`（種類 = 小文字化した拡張子）/ `size` / `modified`
- `dir`: `asc` / `desc`
- ディレクトリはキー・方向に関わらず常に先頭（core の `SortOrder` と同挙動）
- 名前は大小文字無視の自然順（`file2` < `file10`）、同値なら大小文字区別でタイブレーク
- 入力配列は変更せず新しい配列を返す。ソート変更後も選択は path で残る

core API 側のソート条件指定は後続タスク（core wasm API 拡張）で対応する。
なおデスクトップ版のグリッド表示には列ヘッダ非表示のため並び替え導線が無いが、
これは `app/` 側の対応事項で、この PWA コントロールはその代替ではない。

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

## 非同期検索の世代ガードと長押しの再解決

検索入力のデバウンス（150ms）をすり抜けて `await` 中のリクエストが残ると、
連続入力・「検索をクリア」・右スワイプの後に古い結果が新しい一覧を上書きする。
`state.searchGeneration` を単調に進め、各リクエストが開始時に世代を捕捉し、
`await` 完了後に `isCurrentSearchRequest(current, captured)` で照合する。
現役でない結果やエラーは描画もステータス更新もしない。新しい入力の時点でも
世代を進めるため、デバウンス待ちの 150ms の間に古い結果が出ることもない。

長押しは `touchstart` 時点の行 index ではなく `entry.path` を捕捉する。
500ms のタイマー発火時に `longPressTargetIndex(state.renderedEntries, path)`
で現在の一覧から行を探し直し、見つからなければトグルをスキップする。
検索完了で行が入れ替わっても、別行ではなくタップした行が選ばれる。

いずれも pure helper として `node --test pwa/main.test.js` で回帰を固定している。

## 構成

| ファイル | 役割 |
| --- | --- |
| `index.html` | 検索ボックス + 一覧の 1 画面 |
| `main.js` | `pkg/` を動的 import し `CoreViewer` を初期化、一覧・検索を DOM へ反映 |
| `style.css` | ダーク / ライト自動切替（既定は `prefers-color-scheme`）+ iOS セーフエリア対応 |
| `manifest.webmanifest` | PWA マニフェスト（standalone / SVG 内蔵、name は `GFE Viewer`） |
| `sw.js` | service worker（静的シェル + wasm を precache、オフライン起動） |
| `main.test.js` | pure helper と SW / マニフェストの回帰テスト（`node --test`） |
| `.gitignore` | 生成物 `pkg/` を除外 |

`pwa/` と `android/` はルート `Cargo.toml` の workspace members には入れない。
