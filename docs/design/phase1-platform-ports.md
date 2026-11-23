---
type: Spec
title: "Phase 1 プラットフォーム境界（ポート／アダプタ）設計"
description: >-
  Rust コア抽出の OS 境界ポート草案、GPUI / Android(Compose) / PWA(Web)
  アダプタ、1画面ビューア受け入れ条件、工数とライセンス境界の再見積もり
tags:
  - gpui
  - ports-adapters
  - mobile
  - phase1
  - licensing
  - pwa
generated:
  by: "p1-design-ports"
  at: "2026-11-23T14:35:08+09:00"
status: draft
---

## 開始

本設計は、承認済みモバイル ADR の選択肢B を Phase 1 で検証するための、
プラットフォーム境界（ポート／アダプタ）の設計案である。
新方針（2026-11-23、AGENTS.md「クロスプラットフォーム対応方針」）により、
選択肢B は「Rust コア共有 + Jetpack Compose（Android アプリ）/
Web PWA（iOS および Android PWA）」を指す。SwiftUI アプリ版は提供しない。
iOS を PWA のみとする主因は、iOS アプリ配布に Apple Developer Program の
年会費が必ずかかるというコスト理由である。技術的理由（gpui-kit の wasm
安定パスなし）と併せて決定根拠として記録する。
対象は設計のみであり、Cargo.toml やソースの変更は含まない。
成果物は本ファイル 1 つで、`docs/design/phase1-core-extraction.md` とは担当を分離する。

## 確定事実と設計前提

R1 で確認済みの事実を、本設計の前提として引用する。

- 進捗とキャンセルは同期 API である。`ops.rs` は
   `progress: &mut dyn FnMut(u64)`（`app/src/ops.rs:409,564,736`）と
   `cancel: &CancelFlag`（同 `:407,562`）を受け取る。
   `CancelFlag` は `AtomicBool` のラッパ（同 `:47,56`）で Send / Sync を持つ。
- GPUI 側は既に同期 → 非同期の橋渡しを実装済みである。
   `app/src/main.rs:6215` で `async_channel::unbounded::<(u64, u64)>()` を作り、
   `cx.spawn` のループで受けて `this.update` へ渡す（同 `:6215-6242`）。
   約 80 ms のスロットルも同じ箇所にある。
- `settings.rs` は `gpui_kit` に直依存する
   （`ThemeMode` / `Window` / `WindowBounds`、`app/src/settings.rs:71-72`）。
   コア側は素の enum / struct + JSON 互換値とし、変換はアダプタが担う。
- `trash` 5.3 は iOS / Android を cfg 除外し、
   `app/src/trash_view.rs:31-39` がゲートをミラーし、同 `:78-125` が no-op fallback を
   提供する。リスト取得はブロッキングで、
   UI は `cx.background_spawn` で退避するよう同 `:46-48` に明記されている。
- `mounts.rs` は Linux の `/proc/mounts` 解析に加え、Windows は
   `gfe-win32::logical_drives()` によるネイティブ列挙へ接続済み
   （`app/src/mounts.rs:220-227`、`win32/src/lib.rs:154,206`）。
   **as-built（2026-11-23）**: 旧本文の「Windows は文書化されたスタブ」
   （`app/src/mounts.rs:234-237`）は解消済みである。
   OS アイコンの既定プロバイダは Linux が `FreedesktopIconProvider`、
   Windows / モバイルが `BundledIconProvider`
   （`app/src/system_icons.rs:416,427-433`、`win32/src/lib.rs:306,355`）。
   `gfe-win32` の `file_icon_index` / `file_icon_handle` は FFI 側で得られるが、
   名前ベースの `OsIconProvider` は HICON を描画側へ渡せないため、
   Windows のネイティブアイコン描画への配線は未実施である。
- `gpui-pre-windows` 0.3.12 は `prompt_for_paths`（`:782`）、`reveal_path`（`:816`）、
   `open_with_system`（`:830`）を安全な `Platform` メソッドとして提供する。
   内部の `unsafe` 88 箇所は依存クレート内に閉じこもる。
- モバイル ADR は `status: approved`、選択肢B 採用、
   Phase 1 は「1画面の薄いビューア」と「A との工数差・ライセンス境界の再見積もり」
   を要求する（`docs/adr/adr-20261122-mobile-android-ios-support.md:191-195,213-219`）。
   モバイル範囲は 2026-11-23 の新方針（開始節）で更新されている。
- WinUI 3 は draft ADR で不採用。GPUI UI を XAML に置換えるため
   B と同じ UI 倍増が生じる
   （`docs/adr/adr-20261123-windows-native-api-strategy.md:236-250,252-265`）。

## ポート定義（トレイト草案）

### 共通原則: 同期コア / 非同期アダプタ

コアは `async` を用いず、既存の `ops.rs` / `trash_view.rs` と同じ同期シグネチャを維持する。
非同期性はすべてアダプタ側に閉じ込める。具体的な橋渡し先は次の 3 つに限定する。

- GPUI（デスクトップ）: `cx.background_spawn` / `async_channel` /
  `cx.spawn` + `this.update`。
- Android（Compose アプリ）: `Dispatchers.IO` と `MainIoDispatcher` /
  `Dispatchers.Main`。
- PWA（iOS / Android 共通 Web）: Web Worker + `postMessage`。
  進捗は `postMessage` を受けて React state を更新する（trait は未採用）。

ブロッキングport（Trash / Volume / Image）はコアを同期のまま呼び、
アダプタがバックグラウンド実行系へ退避してから結果をコアへ戻す。
ユーザー操作を待つ port（FileDialog）は「同期サブミット + 完了コールバック」とし、
Clipboard は同期 Result を返す。コアに `.await` を導入しない。
コールバックはどのスレッドから呼ばれてもよいが、
UI 更新はアダプタがメインスレッドへ切り替えて行う。

```rust
// すべてのportは Send + Sync を要求する（ops.rs の CancelFlag と同じ契約）。
pub struct PortError(pub String);

pub type Completion<T> = Box<dyn FnOnce(Result<T, PortError>) + Send + 'static>;
```

### TrashBackend

既存の `trash_view.rs` の `platform` モジュールをそのまま port 化する。
シグネチャは同期・ブロッキングのままで、Android / PWA は no-op 実装を提供する。

```rust
pub struct TrashId(pub std::ffi::OsString);

pub struct TrashEntry {
    pub id: TrashId,
    pub name: std::ffi::OsString,
    pub original_parent: std::path::PathBuf,
    pub time_deleted: i64,
}

pub trait TrashBackend: Send + Sync {
    fn list(&self) -> Result<Vec<TrashEntry>, PortError>;
    fn restore(&self, ids: &[TrashId]) -> Result<(usize, usize), PortError>;
    fn delete(&self, path: &std::path::Path) -> Result<(), PortError>;
}
```

`restore` の戻り値 `(restored, missing)` は既存の `restore_by_id`
（`app/src/trash_view.rs:66,122`）と同じ意味を持つ。
no-op 実装は `list -> Ok(vec![])`、`restore -> Ok((0, ids.len()))`、
`delete -> Err(...)` とする（同 `:110-122` と同一方針）。

### VolumeSource

`mounts::list_mounts()` を port 化する。戻り値の `MountPoint` はコア側の素の struct で、
GPUI 固有の型を含まない。

```rust
pub trait VolumeSource: Send + Sync {
    fn list_mounts(&self) -> Vec<crate::mounts::MountPoint>;
}
```

Linux は `/proc/mounts` 解析、Windows は `gfe-win32::logical_drives()`
（as-built 2026-11-23: 旧スタブ `app/src/mounts.rs:234-237` は置換済み）、
Android / PWA は空ベクトルを返す。
サイドバーの「ドライブ」節は空配列で自然に非表示になる
（同 `:239-240` のコメントと整合）。

### ImageDecoder

`preview.rs` は現在バイト列の取得と形式判定までを行い、
`ImageBytes { format, bytes }`（`app/src/preview.rs:79`）を返す。
デコードは表示側の関心なので port として分離する。上限は既存定数を使う。

```rust
pub enum ImageFormat { Png, Jpeg, Gif, WebP }

pub struct DecodedImage {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

pub trait ImageDecoder: Send + Sync {
    fn decode(
        &self,
        bytes: &[u8],
        format: ImageFormat,
        max_pixels: u64,
    ) -> Result<DecodedImage, PortError>;
}
```

`max_pixels` には `preview::MAX_IMAGE_PIXELS`（`app/src/preview.rs:27`）を渡す。
表示可否の判定はコア側の `thumbnail_eligible`（`app/src/main.rs:2125`）が
`preview::is_thumbnailable`（`app/src/preview.rs:72`）と
`MAX_IMAGE_BYTES`（同 `:24`）を使って行うため、port 側には重複させない。

### FileDialogPort（prompt / reveal / open_with）

GPUI では `Platform` の安全メソッドとして既に提供されるが、
Android は `gpui-pre-platform` にターゲットも `cfg_select!` 分岐もない
（モバイル ADR `:115-117`）。PWA では File System Access API の
`showDirectoryPicker` をユーザー操作起点で使う（対応ブラウザに依存）。
`reveal` / `open_with` はブラウザで不可のため no-op とする。
iOS の picker 等がスタブであることは ADR `:62-66` の事実だが、
アプリ版を作らないためブロッカーではない。
よって port 化して各フロントエンドの第一級 API へ逃がす。

```rust
pub struct PromptRequest {
    pub title: String,
    pub directory: Option<std::path::PathBuf>,
    pub multiple: bool,
    pub directories_only: bool,
}

pub trait FileDialogPort: Send + Sync {
    /// 同期サブミット。完了はコールバックで通知する（コアに await を導入しない）。
    fn prompt_paths(
        &self,
        req: PromptRequest,
        done: Completion<Vec<std::path::PathBuf>>,
    );
    fn reveal(&self, path: &std::path::Path) -> Result<(), PortError>;
    fn open_with(&self, path: &std::path::Path) -> Result<(), PortError>;
}
```

`prompt_paths` はユーザー操作を伴うため完了まで時間がかかる。
キャンセルは `Ok(vec![])` ではなく `Err(PortError("cancelled".into()))` で表現し、
呼び出し側が.status を汚さないようにする。

### ClipboardPort

現状の cut / copy はアプリ内部の `ClipboardSlot`（`app/src/main.rs:2481,2962`）だけで、
OS クリップボードへはパス文字列 1 本を書き込むのみである
（`docs/adr/adr-20261123-os-file-clipboard.md` の「検証済み事実」）。
Phase 1 の読み取り専用ビューアでは no-op でよいが、将来の相互運用のため境界だけ定義する。

```rust
pub struct ClipboardFile {
    pub path: std::path::PathBuf,
    pub cut: bool,
}

pub trait ClipboardPort: Send + Sync {
    fn write_files(&self, files: &[ClipboardFile]) -> Result<(), PortError>;
    fn read_files(&self) -> Result<Vec<ClipboardFile>, PortError>;
}
```

Windows の CF_HDROP、POSIX の `text/uri-list`、Android の共有 Intent と
PWA の Clipboard API の対応状況は ADR に譲り、port の形のみを固定する。

### WatchSource

`DirWatcher` + `watch_rx` の `async_channel`（`app/src/main.rs:2945,3503-3504`）と
`spawn_watch_loop`（同 `:4226`）を port 化する。登録は同期、通知はコールバックである。

```rust
pub struct WatchHandle { /* Drop で監視解除 */ }

pub trait WatchSource: Send + Sync {
    fn watch(
        &self,
        roots: &[std::path::PathBuf],
        on_event: Box<dyn Fn(crate::watcher::FsEvent) + Send>,
    ) -> Result<WatchHandle, PortError>;
}
```

Phase 1 のビューアは手動リロードで要件を満たすため、no-op 実装でよい。
デスクトップは既存の `notify` / `notify-debouncer-full` を維持する。

### ProgressSink（as-built 2026-11-23: 未採用）

当初は `ops.rs` の `FnMut(u64)` + `CancelFlag` を trait にまとめる案を
検討したが、**採用しなかった**。未配線の trait が残ると二重契約になるため、
`core/src/ports.rs` の `ProgressSink` は削除済み。現行契約は
`ops::copy_entry` / `move_entry` の `impl FnMut(u64)`（累積バイトのみ）と
`&CancelFlag` のままで、`total_bytes` は呼び出し側が別途計算する。
スロットル（約 80 ms）は `main.rs` 側に残し、コアへ持ち込まない。

## GPUI アダプタの接続先

既存コードのどの関数／型を呼ぶかを、根拠となる行とともに列挙する。
抽出後もデスクトップの挙動は変えない。

- VolumeSource: `Explorer::refresh_mounts` → `mounts::list_mounts()` を
   `cx.background_spawn`（`app/src/main.rs:4111-4113`）。
- TrashBackend: `Explorer::reload_trash` → `trash_view::list()` を
   `cx.background_spawn`（同 `:3709-3713`、`app/src/trash_view.rs:46-48`）。
- 一覧: `Explorer::reload` → `model::read_entries(&dir)`
   （同 `:3625,3648`、`app/src/model.rs:92`）。
- WatchSource: `DirWatcher::new` → `watch_tx` → `Explorer::spawn_watch_loop`
   （同 `:3503-3504,4226`）。
- FileDialogPort: `Explorer::open_path` → `cx.open_with_system(&path)`
   （同 `:4813,4826`）と `cx.reveal_path(&path)`（同 `:8555`）。
- 進捗: `Explorer::begin_op` → `async_channel` → `cx.spawn` →
   `this.update`（同 `:6200-6240`、trait 未採用）。
- ImageDecoder: `thumbnail_eligible` →
   `preview::read_image_preview` / `ImageBytes`（同 `:2125`、
   `app/src/preview.rs:79,150`）。
- ClipboardPort: `ClipboardSlot` / `Explorer::clipboard`（no-op で維持、
   同 `:2481,2962`）。
- 設定変換: `settings.rs` の gpui_kit 型をアダプタで受け、コアは JSON 互換値
   （`app/src/settings.rs:71-72`）。

検索はコア API の `search::search_by_name`、`search::filter_results`、
`search::search_contents`（`app/src/search.rs:104,46,343`）をそのまま使う。
GPUI アダプタはこれらを `cx.background_spawn` で呼ぶだけであり、
port を新たに追加しない。

## Android / PWA アダプタの最小実装方針

### 共通: Rust コアとの橋渡し

Android はコアを staticlib としてリンクし、Kotlin 側は薄い JNI シム経由で呼ぶ。
C ABI の文字列受け渡しには所有権を伴うため、Rust 側の `unsafe` はコアへ置かず
専用の `core-ffi` shim crate へ隔離し、不変条件のコメントと
`clippy::undocumented_unsafe_blocks` の CI ゲートを必須とする
（Windows ADR の A2 と同じ考え方、
`docs/adr/adr-20261123-windows-native-api-strategy.md:145-153`）。
PWA は GPUI 非依存コアを wasm 化し、`wasm-bindgen` の生成バインディング経由で呼ぶ。
UniFFI のようなバインディング生成は選択肢として残すが、依存グラフに存在しない
（モバイル ADR `:85-86`）ため、採用時はライセンスと保守性の調査が別途必要である。
本設計はこの選択を確定させない。

### PWA（iOS / Android 共通 Web フロントエンド）

- UI: React 等の Web UI で一覧・検索の1画面のみ。GPUI UI は再利用しない。
- 一覧 / 検索: wasm 化したコア API を Web Worker で呼び、結果を React state へ反映する。
- FileDialogPort: File System Access API の `showDirectoryPicker` を
  ユーザー操作起点で呼ぶ（対応ブラウザに依存）。`reveal` / `open_with` は no-op。
- 進捗: Worker からの `postMessage` を受けて React state を更新する。
  キャンセルは Worker 側の `CancelFlag` を UI 操作で設定する。
- Service Worker: 静的資産とシェルのキャッシュを担う。Phase 1 では最小構成でよい。
- TrashBackend / ClipboardPort / WatchSource: no-op 実装
  （`app/src/trash_view.rs:78-125` と同じ方針）。監視は手動リロードで代替する。
- iOS アプリ版は提供しないため、SwiftUI / UIKit のアダプタは対象外。

### Android

- UI: Jetpack Compose の `LazyColumn` + `ViewModel` 1 画面のみ。
- 一覧 / 検索: `Dispatchers.IO` でコア API を呼び、`StateFlow` で Compose へ公開する。
- FileDialogPort: SAF の `ACTION_OPEN_DOCUMENT` / `ACTION_OPEN_DOCUMENT_TREE` を
   Activity Result API で起動し、Uri をコアの `PathBuf` 相当へ変換する。
   SAF の永続 URI 権限とスコープ付きストレージの扱いは Phase 3 に委ねる
   （モバイル ADR `:220-229`）。`open_with` は `Intent.ACTION_VIEW` +
   `FLAG_GRANT_READ_URI_PERMISSION` を使う。
- ProgressSink: `MainIoDispatcher` / `Dispatchers.Main` で
   `MutableStateFlow` を更新する。
- TrashBackend / WatchSource: no-op。`ClipboardPort` も共有 Intent 対応までは no-op。
- 上記は Compose アプリ系統。Android では加えて共通 PWA（前節）も対象とする。

## 1画面ビューアの受け入れ条件

Phase 1 の goal はアーキテクチャ検証であり、UI の完成ではない。次を満たしたとき完了とする。

1. コア API（`model::read_entries` と `search::search_by_name` / `filter_results`）を
    経由して、ディレクトリ一覧と検索結果を 1 画面に表示できること。
2. Android Compose アプリと iOS / Android 共通 PWA で、ビルドが通り
   上記 1 画面が起動すること。iOS アプリ版ビルドは要求しない。
3. UI の外観（テーマ、レイアウト、アイコン、アニメーション、ローカライズ）の完成は要求しない。
4. 読み取り専用でよいため、ProgressSink / TrashBackend / ClipboardPort / WatchSource は
    no-op 実装で受け入れる。
5. コア抽出後もデスクトップ側の `cargo test` / `cargo clippy` が失敗しないこと。
6. 新 crate とモバイルシムがライセンス境界のゲート（後述）を満たすこと。

## 技術的制約と検証環境

- `notes/wasm-browser-notes.md:51-91` に記載のとおり、gpui-kit 0.9.1 には
   wasm の安定パスが存在しない（feature 調整では解消不可、nightly + COOP/COEP
   または fork が必要）。よって PWA は GPUI UI を再利用せず、
   Web フロントエンドを新規構築する。
- UI 系統は **desktop(GPUI) / Android(Compose) / PWA(Web) の3系統** になる。
- iOS アプリ版は提供しない。主因は Apple Developer Program の年会費が必ずかかる
   というコスト理由（ユーザー明示、2026-11-23）で、上記の技術的理由と併せて
   決定根拠とする。
- 検証環境（verified）: Kubuntu（Linux）はメイン開発環境かつ実機動作検証あり。
   Windows は評価版 Windows で動作検証する。macOS は実機動作検証を行わない。
   Android はアプリ版と PWA の両方、iOS は PWA のみを対象とする。

## 工数再見積もり

まず文書上の既存見積もりを引用する。これらは**いずれも実測値ではない**。

- モバイル ADR の比較表: A = 極大、B = 中〜大、C = 大、D = 小
   （`docs/adr/adr-20261122-mobile-android-ios-support.md:178-183`）。
- DESIGN.md の凡例とロードマップ: P1 MVP = 2〜4 週、P2 = 3〜6 週
   （`DESIGN.md:95,1222`）。

本設計による再見積もりも、計測に基づかない設計者の概算である。
UI 系統は **desktop(GPUI) / Android(Compose) / PWA(Web) の3系統** とする
（iOS アプリ版は作らない）。
Phase 1 spike を、DESIGN の P1（2〜4 週）と同規模の「2〜4 週」と見積もる。
内訳の目安は、port 定義とコア境界の確定が数日、
GPUI アダプタの配線替えが 1〜2 週、
Android Compose ビューアが数日〜1 週、
共通 PWA ビューアが数日〜1 週、
ライセンス・ゲート整備が数日である。
実装後に実績工数を記録し、この見積もりを更新すること。

選択肢Aを不採用とした理由を再確認する。

- `gpui-kit` が iOS / Android を意図的にゲートしており、コアを外れて低レイヤを
   直接構成する（`gpui-kit-0.9.1/Cargo.toml:326`、`src/lib.rs:131-144,198-200`）。
- iOS のファイル API は picker / save / reveal / open_with すべてがスタブで、
   fork して実装する必要がある（`gpui-pre-ios-0.3.12/src/ios/platform.rs:309,323,342,346`）。
- Android は `gpui-pre-platform` にターゲット依存も `cfg_select!` 分岐もなく、
   `Platform` 全体を自作するため見積もりが困難である（モバイル ADR `:115-117,238`）。
- Apache-2.0 の GPUI を fork して AGPL アプリへ結合する際は、
   帰属表示と AGPL の及ぶ範囲のレビューが別途必要になる（同 `:118-119`）。
- ユーザーは 2026-11-23 に B を承認し、A は不採用、
   そのライセンス方針の承認は不要と明記した（同 `:191-195,243-245`）。

WinUI 3 も draft ADR で不採用であり、GPUI UI を XAML へ置換える点で
B と同じ UI 倍増が生じるため、本見積もりには含めない
（`docs/adr/adr-20261123-windows-native-api-strategy.md:236-250,252-265`）。

## ライセンス境界

- 新規 crate（コア、各アダプタ、`core-ffi` shim、モバイルシム）のすべての先頭に
   `// SPDX-License-Identifier: AGPL-3.0-or-later` を付ける。
   既存の例は `app/src/trash_view.rs:1`、`app/Cargo.toml:1`、
   `deny.toml:1`、`NOTICE.md:1`。
- `NOTICE.md` の「Upstream dependencies keep their own licenses」の方針を維持し、
   新規依存を追加した場合は `app/NOTICE.md` の third-party 一覧へ追記する
   （`NOTICE.md` の Application source / Upstream dependencies の節）。
- 依存追加時は `deny.toml` の確認ゲートを必須とする。
   `licenses.allow` は明示列挙で、`Apache-2.0` / `ISC` / `Unicode-3.0` / `MPL-2.0` 等は
   既に許可されているが、新しいライセンスは明示的に追加する（`deny.toml:39-60`）。
   `sources` は `unknown-registry = "deny"` / `unknown-git = "deny"` で、
   `bans.wildcards = "deny"` も維持する（同 `:29-36`）。
   `advisories.ignore` への追加は理由を明記した場合のみ認める（同 `:16-24`）。
- Zed の GPL コードはコピー禁止とする。GPUI スタックは Apache-2.0 の依存として
   使うだけであり、GPUI fork も行わない（モバイル ADR の承認済み方針、同 `:194,199-200`）。
   これにより Apache-2.0 と AGPL-3.0-or-later の境界は「別クレートの依存」として保たれる。

## 未確認事項

- PWA の File System Access API は対応ブラウザに依存し、Safari 等での挙動は未検証である。
- Android SAF の永続 URI 権限と `FileObserver` の実用性は未検証である。
- `core-ffi` shim を自作するか UniFFI を採用するかは未確定で、
   UniFFI のライセンス・保守性調査が必要である。
- 本セッションは Linux ホストのため、Windows / Android のビルドと実機動作は未確認である。
   新方針では Linux は実機検証あり、Windows は評価版で検証、
   macOS は実機検証なし、iOS は PWA のみ（アプリ版なし）とする。

## 参照

- [モバイル対応 ADR](../adr/adr-20261122-mobile-android-ios-support.md)
- [Windows ネイティブAPI戦略 ADR](../adr/adr-20261123-windows-native-api-strategy.md)
- [OS ファイルクリップボード ADR](../adr/adr-20261123-os-file-clipboard.md)
- [wasm 調査ノート](../../notes/wasm-browser-notes.md)（:51-91、gpui-kit wasm 制約）
- `AGENTS.md`（クロスプラットフォーム対応方針、2026-11-23）
- `app/src/ops.rs:47,56,407-409,825`
- `app/src/main.rs`: 2481, 2962, 3503-3504, 3625, 3709, 4111, 4226,
  4813, 4826, 6200-6240, 8555
- `app/src/settings.rs:71-72`、`app/src/trash_view.rs:1,31-39,49-53,81-125`
- `app/src/mounts.rs:220-227`、`app/src/system_icons.rs:416,427-433`、
  `win32/src/lib.rs:154,206,306,355`
- `app/src/model.rs:92`、`app/src/search.rs:46,104,343`
- `app/src/preview.rs:24,27,72,79,150`
- `app/Cargo.toml:1,28-34`、`deny.toml:1,16-24,29-36,39-60`
- `NOTICE.md:1`、`DESIGN.md:95,1222`
