---
type: ADR
title: "モバイル対応（Android/iOS）のアーキテクチャ選定"
description: "要件追加「Android版、iOS版も要件に入れて」に対するGPUIスタック適合性の評価と段階的対応方針"
tags:
  - gpui
  - mobile
  - android
  - ios
  - architecture
  - licensing
generated:
  by: "adr-mobile"
  at: "2026-11-22T22:45:26+09:00"
verified:
  by: "lead"
  at: "2026-11-23T15:27:53+09:00"
status: approved
---

## コンテキスト

本プロジェクトは `gpui-kit = "=0.9.1"`（`app/Cargo.toml:17`）を唯一のUI基盤として、
クロスプラットフォームのファイルエクスプローラを実装している。
これまでデスクトップ（Linux / Windows / macOS）を主対象としてきたが、
新たにモバイル対応を含む要件追加が発生した。
2026-11-23 に AGENTS.md §10 のクロスプラットフォーム方針が改訂され、
iOS はアプリ版ではなく PWA を対象とすることになった。

本ADRは、ローカルに存在する依存クレートのソースとアプリコードを実読して確認した事実に
基づき、モバイル対応の選択肢を比較して推奨方針を記録する。
調査は既検証の知見を引用し、ゼロからやり直してはいない。

## 要件

- 既存要件: GPUI によるファイルエクスプローラ（一覧、検索、操作、設定、ゴミ箱ビュー）。
- 新方針（2026-11-23）: `~/.agents/AGENTS.md` §10「クロスプラットフォーム対応方針」
  （`:228-236`）に根拠を持つ。以下を対応範囲とする。
- Kubuntu（Linux）: メイン開発環境。実機動作検証あり。
- Android: アプリ版とPWAの両方に対応する。
- iOS: アプリ版は提供せず、PWAに対応する。
- Windows: 評価版Windowsで動作検証する。
- macOS: 実機動作検証は行わない。
- 旧要件「Android版、iOS版も要件に入れて」（アプリ版のみを想定）は上記へ置換し、
  iOS アプリ版は対象外とする。
- iOS を PWA のみとする主因はコストである。ユーザー明示の理由として、
  iOS アプリの配布には Apple Developer Program の年会費が必ずかかり、
  これを避ける。iOS のファイルAPIがスタブである点は副次的な技術理由に留まる。
- 対象ブラウザは Chrome・Edge・Brave・Firefox・Safari を基本とする
  （同 AGENTS.md §10「ブラウザ対応対象」`:238-240`）。

## 検証済み事実

### GPUIスタックのモバイル除外設計

- `app/Cargo.toml:17`: `gpui-kit = "=0.9.1"`（exact pin）。他UIフレームワークは依存なし。
- `gpui-kit-0.9.1/Cargo.toml:326-334`: `gpui-pre-platform` 依存は
  `cfg(not(any(target_os = "ios", target_os = "android")))` でゲートされている。
- `gpui-kit-0.9.1/src/lib.rs:131-144`: `platform` / `web` の再エクスポートは
  デスクトップとwasmのみ。iOS / Android 向けの再エクスポートは存在しない。
- `gpui-kit-0.9.1/src/lib.rs:198-200`: `application()` も同一cfgでゲート。
  コメントに "Mobile applications provide their platform with
  `Application::with_platform`"。

### プラットフォーム実装の有無

- `Cargo.lock:2443-2456`（virtual workspace の正本。旧 `app/Cargo.lock` は未使用残骸）:
  `gpui-pre-platform` 0.3.12 の依存は
  ios / linux / macos / web / windows のみ。Android backend は存在しない。
- `gpui-pre-platform-0.3.12/src/gpui_platform.rs:63-`: `cfg_select!` の分岐は
  ios / macos / windows / linux のみで、android 分岐はない。
- `gpui-pre-0.3.12/src/app.rs:242`: `Application::with_platform` が存在し、
  呼び出し側による `Platform` 注入が可能。

### iOSの実装状況

- `gpui-pre-ios-0.3.12/src/ios/platform.rs:309`: `prompt_for_paths` は
  "File picker not yet implemented for iOS" を返すスタブ。
- 同 `:323`: `prompt_for_new_path` は
  "Save dialog not yet implemented for iOS" のスタブ。
- 同 `:342`: `reveal_path` は no-op。同 `:346`: `open_with_system` はコメントのみで、
  `UIDocumentInteractionController` / `UIActivityViewController` の利用が想定されている。
- `gpui-pre-ios-0.3.12/Cargo.toml:37-43`: `crate-type = ["staticlib", "rlib"]`。
- `gpui-pre-ios-0.3.12/examples/ios.rs`: UIKit のスモークループ（tapカウンタ）のみ。

### Windowsの実装状況

- `gpui-pre-windows-0.3.12/src/platform.rs:782`: `prompt_for_paths` 実装済み。
- 同 `:795`: `prompt_for_new_path` 実装済み。
- 同 `:816`: `reveal_path` 実装済み。同 `:830`: `open_with_system` 実装済み。
- 同 `:1524`: `ShellExecuteW`、同 `:1572`: `SHOpenFolderAndSelectItems` を使用。
- 現スタックのまま Windows は実用的に動作可能。評価版Windowsで検証する。

### ゴミ箱とライセンス

- `trash` 5.3.0 は ios / android をcfgで除外（`trash-5.3.0/Cargo.toml:108-193`）。
- `app/Cargo.toml:36-37` で対象外ターゲットを除外し、
  `app/src/trash_view.rs:12-19` がno-op fallbackを提供する。
- アプリは AGPL-3.0-or-later（`app/Cargo.toml:1,7`）。
- GPUIスタック（gpui-kit / gpui-pre / gpui-pre-platform / gpui-pre-ios）は Apache-2.0。
- `deny.toml:55` は Unicode-3.0 を許可（`DESIGN.md:1052` の allow 一覧と一致）。
- Tauri / uniFFI / KMP のラッパーは依存グラフに存在しない。
  `tauri-winrt-notification` はWindows通知ヘルパーであり、Tauriラッパーではない。

### Web/WASMの先行知見

- `notes/wasm-browser-notes.md:51-91`: "gpui-kit を使う安定パスは存在しない"。
  feature調整では解消できず、nightly + COOP/COEP/SharedArrayBuffer または
  gpui-kit のforkが必要と実証済み。
- 同 `:198-206`: 現時点の推奨はデスクトップ専用のまま。
- 新方針（2026-11-23）では、上記の実証により PWA で GPUI UI を再利用できない。
  Web フロントエンド（React 等）を新規構築する必要がある（選択肢Bの短所を参照）。
- ライセンス境界は変わらない。AGPL-3.0-or-later（アプリ）、Apache-2.0（GPUI）、
  Unicode-3.0（ICU 系依存、`deny.toml:55`）の3区分は維持する。

## 選択肢の比較

### 選択肢A: GPUI UIを維持し、iOSはPlatform注入、Androidは自作

注（2026-11-23）: 新方針では iOS アプリ版を提供しないため、以下の iOS の
Platform 注入 / SwiftUI に関する記述は「新方針では対象外」とする。
Android の Platform 自作の問題は新方針でも変わらず、選択肢Aは不採用のまま。

内容: `Application::with_platform` により、iOS は `gpui-pre-ios` と
`gpui-pre-platform`、`gpui-base` / `component` を直接利用し、
Android は JNI / Activity ベースの `Platform` をゼロから実装する。

長所:

- UIコードをモバイルでも再利用できる。
- platform注入API（`gpui-pre/src/app.rs:242`）と、iOS の staticlib +
  UIKit スモークループが足がかりになる（新方針では対象外）。

短所:

- `gpui-kit` 自身が iOS / Android を意図的にゲートしており
  （`Cargo.toml:326`、`lib.rs:198-200`）、事実上 gpui-kit を外れて低レイヤを直接構成する。
- iOS のファイルAPIは未実装スタブ（picker / save / reveal / open_with_system）で、
  forkして実装する必要がある（新方針では対象外）。
- Android は `gpui-pre-platform` にターゲット依存も `cfg_select!` 分岐もなく、
  ウィンドウ、入力、IME、SAF ファイル選択など `Platform` 全体を自作する。
  工数が極めて大きい。
- Apache-2.0 のGPUIをforkして AGPL アプリと結合する際は、
  帰属表示とAGPLの及ぶ範囲をライセンスレビューで確定させる必要がある。
- `trash` にモバイルbackendがなく、ゴミ箱ビューは引き続き利用不可。

### 選択肢B: Rustコア共有 + Jetpack Compose（Androidアプリ）/ Web PWA（iOS・Android）

内容: model / ops / settings / search などGPUI非依存のロジックをライブラリcrateへ分離し、
Android アプリは Jetpack Compose、iOS と Android の PWA は Web フロントエンド
（React 等）を新規構築する。SwiftUI アプリ版は提供しない。

長所:

- `app/src/trash_view.rs:10` が "This module is GPUI-free" と明言しており、
  コア分離の先行例が既にある。
- ファイル選択・共有は Android の SAF と PWA の Web File System Access API を
  使える範囲で利用し、iOS アプリ向けの未実装スタブを回避できる。
- GPUI のforkが不要で、AGPLアプリと Apache-2.0 依存の境界を保ちやすい。
- コアを共有するため、Compose と PWA で同一の一覧・検索結果を検証できる。

短所:

- モバイルでGPUI UIの再利用を放棄し、UI実装を Compose と PWA の2系統保守する。
- PWA は gpui-kit 0.9.1 の安定 WASM パスが無いため
  （`notes/wasm-browser-notes.md:51-91`）、GPUI UI を再利用できず、
  Web フロントエンド（React 等）を新規構築する必要がある。
  これが「UI 系統が増える」短所の実体であり、ライセンス境界
  （AGPL-3.0-or-later / Apache-2.0 のGPUI / Unicode-3.0）は変わらない。
- `Application` 相当の機能をコアAPIとして定義し、
  既存アプリをlibrary crateへリファクタする作業が必要。
- KMPを選ぶ場合も、新方針では iOS アプリ版が無いため共有範囲は限定される。

### 選択肢C: WASM/Webラッパーでモバイルブラウザを対象

内容: gpui-kit を wasm 化し、モバイルブラウザで動作させる。
新方針の PWA は選択肢Bの Web フロントエンドを指し、本選択肢とは異なる。

長所:

- 理論上は単一コードベース。

短所:

- `notes/wasm-browser-notes.md` で gpui-kit 0.9.1 の安定パスなしと実証済み。
  nightly + COOP/COEP または fork が必要。
- モバイルブラウザは直接ファイルシステムへアクセスできず（OPFS等に限定）、
  エクスプローラの要件と相性が悪い。
- 現時点で non-viable。

### 選択肢D: モバイルを保留し、Windows先行のデスクトップパリティを優先

内容: モバイル要件をバックログに留め、まず Windows 対応とレスポンシブレイアウトを完成させる。

長所:

- `gpui-pre-windows` 0.3.12 にファイルAPIと
  `ShellExecuteW` / `SHOpenFolderAndSelectItems` が実装済みで、
  現スタックのまま Windows が実用的。
- `trash` にWindows backendがあり、既存機能をそのまま活かせる。
- 工数が低く、モバイルアーキテクチャを決定する時間を確保できる。

短所:

- 単体では今回の要件を満たさない。モバイル利用者は未対応のまま。
- 暫定措置として位置づけを明示しないと、対応先送り常態化のリスクがある。

### 比較の要点

| 選択肢 | UI再利用 | 工数 | ライセンス | 要件充足 |
| --- | --- | --- | --- | --- |
| A | 高 | 極大 | forkとレビュー要 | 新方針では対象外 |
| B | 低 | 中〜大 | 境界が明確 | 可能 |
| C | 高（理論） | 大 | 変更なし | 現時点不可 |
| D | 高 | 小 | 変更なし | 一部のみ |

注: B の UI 系統は 2026-11-23 方針で Compose（Androidアプリ）+
PWA（iOS / Android）となった。承認済みの B 自体は維持している。

## 決定（承認済み 2026-11-23）

現時点の推奨は、**D（Windows先行）を直ちに実行し、
モバイルの目標アーキテクチャとして B を Phase 1 のspike（検証）で確認する**
という二段構えである。

**ユーザーにより 2026-11-23 に推奨が承認され、status を `approved` に変更した。**
承認内容: D（Windows先行）を直ちに実行し、モバイルの目標アーキテクチャは
B（Rustコア共有＋SwiftUI / Jetpack Compose）として Phase 1 の spike で検証する
（当時のUI表記）。
選択肢A（GPUI fork）は採用しないため、そのライセンス方針の承認は不要。
Phase 1 の具体的な着手は、この ADR の承認をもってゲートを満たしたものとする。

**2026-11-23 の方針変更追記（承認済みを維持）:** `~/.agents/AGENTS.md` §10 の
新方針反映に伴い、目標アーキテクチャ B は維持したまま、UI 系統のみが
「SwiftUI / Jetpack Compose」から
「Jetpack Compose（Androidアプリ）+ Web PWA（iOS および Android）」へ変わった。
iOS アプリ版は提供しない。
決定の主因はコストである。ユーザーが明示した理由として、iOS アプリの配布には
Apple Developer Program の年会費が必ずかかり、これを避けて PWA を選ぶ。
GPUI の iOS 実装がスタブであることは副次的な技術理由に留まる。
Kubuntu をメイン開発環境かつ実機検証環境とし、Windows は評価版で動作検証、
macOS は実機動作検証を行わない。

理由:

- A は gpui-kit の設計意図（モバイル除外）に逆らい、
  Android の `Platform` をゼロから作る必要があり、継続的なfork保守が避けられない。
- C はローカル検証で既に不成立。
- B はコア資産を残しつつOS標準APIを活かせ、最もリスクが低い。
  PWA の Web フロントエンドは新規構築だが、ライセンス境界は変わらない。
- D は判断を遅らせつつ、今の成果を出せる。

## 実装フェーズ

### Phase 0: Windowsパリティとレスポンシブレイアウト

- `gpui-pre-windows` の実装済みファイルAPIをアプリへ接続し、Windowsビルドを検証。
- `trash` のWindows backendを有効化したまま、macOS / iOS / Android のno-op fallbackを維持。
- ウィンドウ幅に応じたレイアウトを整備し、将来のタブレット表示の前提を作る。

### Phase 1: モバイルアーキテクチャspike（本ADR承認後）

- 選択肢Bの検証として、model / ops / settings / search をlibrary crateへ抽出する境界を定義。
- ビューア受け入れは「Android Compose アプリ + iOS/Android 共通 PWA の1画面」とする。
- Android は Jetpack Compose、PWA は React 等の Web フロントエンドで、
  コアAPI経由の一覧・検索を表示し、両者で同じ結果が出ることを確認する。
  as-built（2026-11-24）: PWA は React ではなく Vanilla JS（`pwa/main.js`）で
  実装し、`wasm-bindgen` 生成の `pkg/gfe_core.js` を直接 import してコア API
  を呼ぶ。Web Worker / React state は使用しない（設計案からの逸脱）。
- SwiftUI アプリ版は新方針では対象外につき試作しない。
- 選択肢Aとの工数差とライセンス境界を再見積もりし、最終方針をユーザーへ提示。

### Phase 2: 最小 Android / PWA ビューア

- 読み取り専用: 一覧、検索、ファイル選択（SAF または Web File System Access）、共有。
- iOS はアプリ版を提供しないため、PWA が唯一のアクセス経路となる。
- ゴミ箱ビューは各環境で対応手段が見つかるまで非表示を維持。

### Phase 3: フルパリティ

- 書き込み操作、設定の同期、プラットフォーム固有のUX（権限、SAFの永続URI、
  iOSのサンドボックス）に対応する。
- PWA ではこれらは File System Access API の対応範囲に留まる。
  iOS Safari 等では対応が限られるため、書き込み・永続 URI・サンドボックスに
  依存する機能は提供しないか、対応ブラウザに限定する。
- デスクトップとの回帰テストを共通コア側で担保。

## 結果とリスク

- 良い結果: コアロジックを1か所で保守しつつ、各環境で自然なUXを提供できる。
- 悪い結果: モバイルUIを別系統で持つため、UI改修コストがUI系統数に比例する。
- リスク:
  - ライセンス: GPUI（Apache-2.0）をfork・改変してAGPLアプリに結合する場合の義務範囲
    （選択肢A、新方針では対象外）。
  - 工数: Android `Platform` 自作は見積もりが困難（選択肢A）。
  - コスト: iOS アプリ版を避けたことで PWA のファイルアクセス範囲に制約が残る。
  - 要件: Dのみで完了とみなさず、Phase 1 の承認を必ずゲートにする。

## 確認が必要な事項（ユーザー判断）

以下のチェックは承認済みを維持する。末尾の項目が 2026-11-23 新方針の反映である。

- [x] モバイル対応の目標アーキテクチャを B（コア共有＋ネイティブUI）で承認
  （2026-11-23、ユーザー承認）。
- [x] 選択肢A（GPUI維持・fork）は不採用としたためライセンス方針の承認は不要。
- [x] Phase 0 を先にリリースし、Phase 1 以降をこの ADR 承認のもとで進めることを承認。
- [x] 2026-11-23 のクロスプラットフォーム方針（AGENTS.md §10）の反映を承認。
  B は維持し、UI 系統を Compose（Android アプリ）+ PWA（iOS / Android）へ変更。
  iOS アプリ版は提供しない。主因はコスト（Apple Developer Program の年会費が
  必ずかかる）であり、ユーザーが明示した理由として記録する。

## 参照

- `app/Cargo.toml`, `Cargo.lock`（ルート）, `app/src/trash_view.rs`
- `gpui-kit-0.9.1/Cargo.toml`, `gpui-kit-0.9.1/src/lib.rs`
- `gpui-pre-platform-0.3.12/Cargo.toml`
- `gpui-pre-platform-0.3.12/src/gpui_platform.rs`
- `gpui-pre-0.3.12/src/app.rs`
- `gpui-pre-ios-0.3.12/src/ios/platform.rs`
- `gpui-pre-ios-0.3.12/examples/ios.rs`
- `gpui-pre-windows-0.3.12/src/platform.rs`
- `trash-5.3.0/Cargo.toml`
- `~/.agents/AGENTS.md:228-236`（§10 クロスプラットフォーム対応方針）
- `~/.agents/AGENTS.md:238-240`（§10 ブラウザ対応対象）
- `deny.toml:55`, `DESIGN.md:1052`（Unicode-3.0 のライセンス許可）
- [wasm-browser-notes.md](../../notes/wasm-browser-notes.md)
