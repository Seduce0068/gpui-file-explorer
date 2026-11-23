---
type: ADR
title: "Windows ネイティブAPI戦略: unsafe FFI と WinUI 3 の扱い"
description: "Windows FFI で unsafe が要る理由と WinUI 3 採用の是非を事実ベースで記録する"
tags:
  - gpui
  - windows
  - ffi
  - unsafe
  - winui3
  - adr
generated:
  by: "research-winui"
  at: "2026-11-23T14:16:14+09:00"
verified:
  by: "research-winui"
  at: "2026-11-23T14:16:14+09:00"
status: draft
---

## コンテキスト

ユーザーから2つの質問を受けた。

1. なぜ Windows ネイティブAPIの FFI には `unsafe` が必要か。
2. WinUI 3 はどうか。

本ADRはローカルソースの実読と公式ドキュメントのみを根拠に、
両質問へ答え、具体的な推奨方針を記録する。
外部情報は `web_fetch` 経由の一次情報に限定した。

## 検証済み事実（ローカル）

### アプリクレートは unsafe を禁止している

- `app/src/main.rs:8`: `#![forbid(unsafe_code)]`。
  アプリクレート全体で `unsafe` を禁止している。
- `app/src/mounts.rs:234-237`: `#[cfg(target_os = "windows")]`
  の `list_mounts()` は
  `mount_points_from_drive_mask(0, ...)` を返す文書化されたスタブ。
  同 `:229-233` に `windows-sys 0.61.2` の署名
  （`GetLogicalDrives() -> u32` と `GetVolumeInformationW(...)`）
  の検証記録がある。
- `app/src/system_icons.rs:15-20`: `OsIconProvider` は FFI 不使用。
  Windows の `SHGetFileInfo` は NF02 承認待ちとして明示的に先送り。
  同 `:348-350`: 現状は `BundledIconProvider` が使われる。
- `app/Cargo.toml:13-34`: 直接依存に `windows-sys` / `windows` はない。
  ルート `Cargo.lock` の `windows` 系クレートはすべて推移依存。

### forbid はモジュールの allow で上書きできない

`rustc` で確認した（Runtime Proof）。

```rust
#![forbid(unsafe_code)]
#[allow(unsafe_code)]
mod ffi { pub unsafe fn f() {} }
```

結果は
`error[E0453]: allow(unsafe_code) incompatible with previous forbid`。
したがって「クレート直下に `#[allow(unsafe_code)]` の FFI モジュールを置く」
案は、現状の `#![forbid(unsafe_code)]` を維持したままでは実装できない。

### gpui-pre-windows 0.3.12 は安全なラッパーを提供する

ローカルキャッシュの `gpui-pre-windows-0.3.12/src/platform.rs` を実読した。

- `:522`: `impl Platform for WindowsPlatform`。
- `:782` `prompt_for_paths`、`:795` `prompt_for_new_path`、
  `:816` `reveal_path`、`:830` `open_with_system` は
  いずれも **`unsafe` ではない公開メソッド**。
- 内部では `:1524` `ShellExecuteW`、`:1572`
  `SHOpenFolderAndSelectItems`、`:1824`
  `unsafe extern "system" fn window_procedure` を使用。
  同ファイルの `unsafe` 出現数は 88（ローカル確認）。
- これらは依存クレート内に閉じ込められており、
  アプリは `Platform` トレイト経由で安全に呼び出す。

## 質問1: なぜ FFI に unsafe が必要か

### Rust 側の理由

`extern "system"` / `extern "C"` の宣言は、
呼び出し先が Rust の型・所有権・安全性の契約を守ることを
コンパイラが検証できない。
[NomiconFFI] は "Foreign functions are assumed to be unsafe" と述べ、
生ポインタやスレッド安全性を呼び出し側の責任とする。
よって生の FFI 呼び出しは `unsafe` ブロックで包む必要がある。
逆にライブラリ側が不変条件を担保して `unsafe` を隠せば、
利用者は安全な API だけを呼べる（同ドキュメントの
"Creating a safe interface"）。

### 呼び出しごとの必要性（Windows の実例）

ローカルキャッシュの `windows-sys-0.61.2` と
`windows-0.62.2` のソースで署名を確認した。

- `GetLogicalDrives`: `windows-sys` の
  `Storage/FileSystem/mod.rs:212` は
  `link!("kernel32.dll" "system" fn GetLogicalDrives() -> u32)`。
  引数もポインタもなく、それ自体は危険ではないが、
  `extern` 宣言由来で呼び出しは `unsafe` を要求される。
  `windows` 0.62.2 の `:1313` も `pub unsafe fn` のまま。
- `GetVolumeInformationW`: `windows-sys` の同 `:237` は
  `PCWSTR` / `PWSTR` / `*mut u32` を受け取り、
  呼び出し先がバッファへ書き込む。NUL 終端、容量、長さ引数、
  書き込み先の有効性はすべて呼び出し側の責任で、
  本質的に `unsafe` が必要。
- `SHGetFileInfoW`: `windows-sys` の
  `UI/Shell/mod.rs:553` は `*mut SHFILEINFOW` を受け取る。
  戻り値の `HICON` は呼び出し側の所有物になり、
  `DestroyIcon`（同 `UI/WindowsAndMessaging/mod.rs:86`）で
  解放する必要がある。所有権と寿命の契約が FFI を跨ぐ。
- `ShellExecuteW` / `SHOpenFolderAndSelectItems`: `HWND` や
  COM の PIDL ポインタを扱い、ウィンドウのスレッド親和性と
  COM の初期化状態に依存する。本質的に `unsafe`。
- `window_procedure`: OS から呼ばれるコールバックで、
  `LPARAM` を `CREATESTRUCTW` へキャストする（`:1831`）。
  生ポインタの参照外しを含むため `unsafe`。

### 重要な nuance: アプリは unsafe を足す必要がない

- ファイルダイアログ、reveal、open-with は
  `gpui-pre-windows` が `Platform` の安全メソッドとして提供済み。
  アプリが自前の FFI を書く理由はない。
- 残るギャップは2つだけ。ドライブ列挙
  （`GetLogicalDrives` + `GetVolumeInformationW`）と
  OS アイコン抽出（`SHGetFileInfo`）。
  現状は前者が空ベクトル、後者が bundled アイコンで代替中。
- つまり「Windows 対応に unsafe が要る」のは事実だが、
  「アプリ自身に unsafe を持ち込む必要がある」のは
  この2機能に限定される。

## 選択肢

### A: windows-sys を直接依存に追加し、狭い FFI 層を作る

`windows-sys 0.61.2` は既にロックファイル内にあり、
新しいバージョンを導入するわけではない（ルート `Cargo.lock:7812-7813`）。

- A1: クレート直下を `forbid` から `deny` に変更し、
  モジュールへ `#[allow(unsafe_code)]` を付与する。
  上記 E0453 のとおり `forbid` のままでは不可。
- A2（推奨）: unsafe を含む小さな helper crate を別に作り、
  安全な API だけを app クレートへ公開する。
  app の `#![forbid(unsafe_code)]` は維持できる。

不変条件の文書化が必須である。
UTF-16 の NUL 終端とバッファ長、`HICON` の RAII 解放、
COM 初期化と STA、`HWND` の有効性とスレッド親和性。
AGENTS.md の Rust 規約に従い、レビュー合意と
`clippy::undocumented_unsafe_blocks` の CI ゲートを伴う。

### B: 既存の安全ラッパーを利用する

- B1（`windows` crate）: ローカル確認の結果、
  `GetLogicalDrives` は `windows` 0.57.0 / 0.58.0 / 0.61.3 /
  0.62.2 のすべてで `pub unsafe fn` のまま。
  `GetVolumeInformationW` も 0.62.2 の `:1487` で
  `pub unsafe fn`、`SHGetFileInfoW` も同 0.62.2 の
  `UI/Shell/mod.rs:3169` で `pub unsafe fn`。
  したがって「`windows` crate なら安全」はこの3APIについては
  成立しない（バッファを `&mut [u16]` にする改善はあるが
  `unsafe` は残る）。
- B2（サードパーティ）: ローカルキャッシュの `winsafe 0.0.19` には
  安全なラッパーが存在する。
  `GetLogicalDrives`（`kernel/funcs.rs:609`）、
  `GetVolumeInformation`（同 `:1050`）、
  `SHGetFileInfo`（`shell/funcs.rs:232`、
  `DestroyIconShfiGuard` で RAII 解放）。
  [winsafe] は 0.0.29 が 2026-09-01 公開、MIT、保守継続中。
  MSRV 1.87 は本プロジェクトの 1.92 を下回る。
  ただし 0.x の新規依存であり、API と feature、
  ライセンス確認の PoC が必要。

### C: 現状維持（スタブと bundled アイコン）

- 新規 unsafe も新規依存もゼロ。
- ドライブ一覧と OS 由来のフルカラーアイコンは得られない。
- NF02 の承認が先なら、暫定策として明記すれば許容できる。

### 比較

| 選択肢 | 概要 | 新規依存 | 評価 |
| --- | --- | --- | --- |
| A1 | forbid を deny に変更 | windows-sys | 方針変更要 |
| A2 | unsafe を別 crate へ隔離 | windows-sys | 推奨 |
| B1 | windows crate の安全 API | 既存 | 安全化ならず |
| B2 | winsafe の安全 API | winsafe 0.0.29 | PoC 次第 |
| C | 現状維持 | なし | 暫定 |

## 質問2: WinUI 3 はどうか

### WinUI 3 の定義（公式情報）

- [WinUI3] は新規 Windows デスクトップアプリ向けの推奨
  ネイティブUIフレームワーク。XAML ベースで C# と C++ を
  サポートし、Fluent Design を提供する。
- [WASDK] の一部として配布され、Windows 10 1809 以降で動作する。
  Windows App SDK は WPF / WinForms / Win32 にも機能を提供するが、
  UI フレームワーク自体は WinUI 3 である。
- 導入には NuGet（Microsoft.WindowsAppSDK）と
  Windows App SDK Runtime の展開が必要（[WASDK-existing]）。

### 混同されやすい3者

- WinUI 3: Windows App SDK に含まれる新規デスクトップ向け
  XAML UI。C# / C++。
- WinUI 2: UWP 向け（および XAML Islands 経由のデスクトップ）。
  Windows SDK と緊密結合（[WinUI2]）。
- Win32 common controls: `Comctl32.dll` の子ウィンドウ群。
  いわゆるクラシック Win32 コントロール（[CommonControls]）。

本プロジェクトが使う `gpui-pre-windows` は Win32 / WinRT API
のラッパーであり、WinUI 3 ではない。

### Rust バインディングの現状（正直な評価）

- `windows` / `windows-sys` は Win32 / WinRT の API バインディング
  であり、XAML UI フレームワークそのものは提供しない。
- windows-rs の issue #4654（2026-06-29）でメンテナは、
  WinUI の生バインディングは同梱せず `windows-bindgen` で
  必要に応じて生成する旨を回答した（[Issue4654]）。
  `windows-reactor` は内部で生成したバインディングを隠す設計。
- 一方 `windows-reactor` には 2026年に WinUI バックエンドの
  開発が進んでいる（[PR5180] 2026-10、[PR5392] 2026-11）。
  ただしネイティブ WinUI テストは対話的デスクトップを要し、
  本プロジェクトの依存にはない。
- 結論: 「成熟した Rust 製 WinUI 3 バインディングを
  そのまま採用する」とは現状言えない。
  実験的・開発中と評価するのが正確である。

### 本プロジェクトへの意味

WinUI 3 を採択するなら、Windows の GPUI UI を
XAML UI へ置き換えることになる。

- [mobile-adr] の選択肢B（Rust コア + ネイティブUI）と
  同じ構造になり、UI コードベースが
  Linux/macOS の GPUI、Windows の WinUI、
  モバイルの SwiftUI/Compose と倍増する。
- Windows App SDK Runtime の配布と、
  C++/WinRT または C# のツールチェーンも新たに必要。
- XAML Islands は既存デスクトップアプリへ WinUI コントロールを
  埋め込む手段として文書化されているが、
  GPUI のウィンドウ構造と統合するには大規模作業が必要で、
  2つ目の UI システムを保守する点は変わらない。
- これは狭い FFI ギャップの解法ではなく、
  UI アーキテクチャの方向転換である。

## 推奨決定

1. GPUI UI は維持する。WinUI 3 は採択しない。
2. ドライブ列挙と OS アイコン抽出の2機能のみを対象に、
   A2（unsafe を別 crate へ隔離し、app は `forbid` を維持）を
   第一候補として実装する。並行して B2（winsafe 0.0.29）を
   小さな PoC で比較し、API と feature、ライセンスを確認する。
3. NF02 の承認が先なら C（現状維持）を継続し、
   「暫定である」ことをコードコメントと todo に明記する。
4. unsafe を書く場合、不変条件のコメント、レビュー署名、
   `clippy::undocumented_unsafe_blocks` の CI ゲートを必須とする。
5. **WinUI 3 の採択はユーザーの明示的な承認が必要な
   アーキテクチャ変更として扱う。** 本ADRはその承認を
   含意しない。status は `draft` のままとする。

### as-built 注記（2026-11-23）

「検証済み事実（ローカル）」の `app/src/mounts.rs` スタブと、
「質問1」の「残るギャップは2つ」「現状は前者が空ベクトル」は、
本ADR 作成時点（2026-11-23T14:16:14+09:00）のスナップショット。
その後、選択肢 A2（unsafe を含む Windows FFI を別クレートへ隔離し、
app の `#![forbid(unsafe_code)]` を維持する方針）が承認され、
ドライブ列挙は `win32/`（`gfe-win32`）クレートへ実装された。
Linux ホストのため Windows 実機動作は未検証。
OS アイコン抽出（`SHGetFileInfo`）は依然として bundled アイコンで
代替中。WinUI 3 は採択しない方針を維持し、status は `draft` のまま
（全体を `approved` へ変更する承認は得ていない）。

## 結果とリスク

- 良い結果: UI は1系統のまま、Windows のドライブ一覧と
  フルカラーアイコンが得られる。
- 悪い結果: FFI の不変条件を誤り、Windows のみで
  クラッシュやリークが発生する。
- 未検証: 本セッションは Linux ホストのため、
  Windows ビルドと実機動作は未確認。
  Windows ターゲットの `cargo check` と実機確認が必要。

## 確認が必要な事項（ユーザー判断）

- [x] A2 の方針（別 crate への FFI 隔離）を承認するか。
      → 承認済み。ドライブ列挙は `gfe-win32` として実装済み（実機未検証）。
- [ ] B2（winsafe 新規依存）の PoC を実施するか。
- [ ] WinUI 3 は本ADRでは採択しないことを確認するか。

## 参照

### ローカル

- `app/src/main.rs:8`, `app/src/mounts.rs:229-237`
- `app/src/system_icons.rs:15-20,348-350`
- `app/Cargo.toml:13-34`, ルート `Cargo.lock:7443-8028`（`windows` 系クレート）
- `gpui-pre-windows-0.3.12/src/platform.rs:522,782,795,816,830`
- 同 `:1524,1572,1824`
- `windows-sys-0.61.2/src/Windows/Win32/Storage/FileSystem/mod.rs`
  の `:212,237`
- `windows-0.62.2/src/Windows/Win32/Storage/FileSystem/mod.rs`
  の `:1313,1487`
- `windows-0.62.2/src/Windows/Win32/UI/Shell/mod.rs:3169`
- `winsafe-0.0.19/src/kernel/funcs.rs:609,1050`
- `winsafe-0.0.19/src/shell/funcs.rs:232`
- [mobile-adr]

### 外部（公式・一次情報）

[NomiconFFI]: https://doc.rust-lang.org/nomicon/ffi.html
[WinUI3]: https://learn.microsoft.com/en-us/windows/apps/winui/winui3/
[WASDK]: https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/
[WASDK-existing]: https://learn.microsoft.com/en-us/windows/apps/windows-app-sdk/use-windows-app-sdk-in-existing-project
[WinUI2]: https://learn.microsoft.com/en-us/windows/uwp/get-started/winui2/
[CommonControls]: https://learn.microsoft.com/en-us/windows/win32/controls/common-controls-intro
[Issue4654]: https://github.com/microsoft/windows-rs/issues/4654
[PR5180]: https://github.com/microsoft/windows-rs/pull/5180
[PR5392]: https://github.com/microsoft/windows-rs/pull/5392
[winsafe]: https://crates.io/crates/winsafe
[mobile-adr]: adr-20261122-mobile-android-ios-support.md
