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
  at: "2026-11-23T14:14:46+09:00"
status: approved
---

## コンテキスト

本プロジェクトは `gpui-kit = "=0.9.1"`（`app/Cargo.toml:14`）を唯一のUI基盤として、
クロスプラットフォームのファイルエクスプローラを実装している。
これまでデスクトップ（Linux / Windows / macOS）を主対象としてきたが、
新たにモバイル対応を含む要件追加が発生した。

本ADRは、ローカルに存在する依存クレートのソースとアプリコードを実読して確認した事実に
基づき、モバイル対応の選択肢を比較して推奨方針を記録する。
調査は既検証の知見を引用し、ゼロからやり直してはいない。

## 要件

- 既存要件: GPUI によるファイルエクスプローラ（一覧、検索、操作、設定、ゴミ箱ビュー）。
- 追加要件（本ADRの対象）: **「Android版、iOS版も要件に入れて」**。
- 範囲: Android と iOS のモバイルアプリとしての対応を指す。
- 除外: モバイルブラウザ経由の対応は選択肢Cとして検討し、別途評価する。

## 検証済み事実

### GPUIスタックのモバイル除外設計

- `app/Cargo.toml:14`: `gpui-kit = "=0.9.1"`（exact pin）。他UIフレームワークは依存なし。
- `gpui-kit-0.9.1/Cargo.toml:326-334`: `gpui-pre-platform` 依存は
  `cfg(not(any(target_os = "ios", target_os = "android")))` でゲートされている。
- `gpui-kit-0.9.1/src/lib.rs:131-144`: `platform` / `web` の再エクスポートは
  デスクトップとwasmのみ。iOS / Android 向けの再エクスポートは存在しない。
- `gpui-kit-0.9.1/src/lib.rs:198-200`: `application()` も同一cfgでゲート。
  コメントに "Mobile applications provide their platform with `Application::with_platform`"。

### プラットフォーム実装の有無

- `app/Cargo.lock:2416-2429`: `gpui-pre-platform` 0.3.12 の依存は
  ios / linux / macos / web / windows のみ。Android backend は存在しない。
- `gpui-pre-platform-0.3.12/src/gpui_platform.rs:63-`: `cfg_select!` の分岐は
  ios / macos / windows / linux のみで、android 分岐はない。
- `gpui-pre-0.3.12/src/app.rs:242`: `Application::with_platform` が存在し、
  呼び出し側による `Platform` 注入が可能。

### iOSの実装状況

- `gpui-pre-ios-0.3.12/src/ios/platform.rs:309`: `prompt_for_paths` は
  "File picker not yet implemented for iOS" を返すスタブ。
- 同 `:323`: `prompt_for_new_path` は "Save dialog not yet implemented for iOS" のスタブ。
- 同 `:342`: `reveal_path` は no-op。同 `:346`: `open_with_system` はコメントのみで、
  `UIDocumentInteractionController` / `UIActivityViewController` の利用が想定されている。
- `gpui-pre-ios-0.3.12/Cargo.toml:37-43`: `crate-type = ["staticlib", "rlib"]`。
- `gpui-pre-ios-0.3.12/examples/ios.rs`: UIKit のスモークループ（tapカウンタ）のみ。

### Windowsの実装状況

- `gpui-pre-windows-0.3.12/src/platform.rs:782`: `prompt_for_paths` 実装済み。
- 同 `:795`: `prompt_for_new_path` 実装済み。
- 同 `:816`: `reveal_path` 実装済み。同 `:830`: `open_with_system` 実装済み。
- 同 `:1524`: `ShellExecuteW`、同 `:1572`: `SHOpenFolderAndSelectItems` を使用。
- 現スタックのまま Windows は実用的に動作可能。

### ゴミ箱とライセンス

- `trash` 5.3.0 は ios / android をcfgで除外（`trash-5.3.0/Cargo.toml:108-193`）。
- `app/Cargo.toml:28-34` で対象外ターゲットを除外し、
  `app/src/trash_view.rs:12-19` がno-op fallbackを提供する。
- アプリは AGPL-3.0-or-later（`app/Cargo.toml:1,7`）。
- GPUIスタック（gpui-kit / gpui-pre / gpui-pre-platform / gpui-pre-ios）は Apache-2.0。
- Tauri / uniFFI / KMP のラッパーは依存グラフに存在しない。
  `tauri-winrt-notification` はWindows通知ヘルパーであり、Tauriラッパーではない。

### Web/WASMの先行知見

- `notes/wasm-browser-notes.md:51-91`: "gpui-kit を使う安定パスは存在しない"。
  feature調整では解消できず、nightly + COOP/COEP/SharedArrayBuffer または
  gpui-kit のforkが必要と実証済み。
- 同 `:198-206`: 現時点の推奨はデスクトップ専用のまま。

## 選択肢の比較

### 選択肢A: GPUI UIを維持し、iOSはPlatform注入、Androidは自作

内容: `Application::with_platform` により、iOS は `gpui-pre-ios` と
`gpui-pre-platform`、`gpui-base` / `component` を直接利用し、
Android は JNI / Activity ベースの `Platform` をゼロから実装する。

長所:

- UIコードをモバイルでも再利用できる。
- platform注入API（`gpui-pre/src/app.rs:242`）と、
  iOS の staticlib + UIKit スモークループが足がかりになる。

短所:

- `gpui-kit` 自身が iOS / Android を意図的にゲートしており
  （`Cargo.toml:326`、`lib.rs:198-200`）、事実上 gpui-kit を外れて低レイヤを直接構成する。
- iOS のファイルAPIは未実装スタブ（picker / save / reveal / open_with_system）で、
  forkして実装する必要がある。
- Android は `gpui-pre-platform` にターゲット依存も `cfg_select!` 分岐もなく、
  ウィンドウ、入力、IME、SAF ファイル選択など `Platform` 全体を自作する。
  工数が極めて大きい。
- Apache-2.0 のGPUIをforkして AGPL アプリと結合する際は、
  帰属表示とAGPLの及ぶ範囲をライセンスレビューで確定させる必要がある。
- `trash` にモバイルbackendがなく、ゴミ箱ビューは引き続き利用不可。

### 選択肢B: Rustコアをライブラリ化し、ネイティブUIを構築

内容: model / ops / settings / search などGPUI非依存のロジックをライブラリcrateへ分離し、
iOS は SwiftUI、Android は Jetpack Compose（または KMP で共有）のUIを新規構築する。

長所:

- `app/src/trash_view.rs:10` が "This module is GPUI-free" と明言しており、
  コア分離の先行例が既にある。
- ファイル選択、reveal、共有は各OSの第一級API
  （`UIDocumentPickerViewController` / Android SAF）を使え、スタブ実装が不要。
- GPUI のforkが不要で、AGPLアプリと Apache-2.0 依存の境界を保ちやすい。
- 将来のWeb / CLIフロントエンドにも同じコアを再利用できる。

短所:

- モバイルでGPUI UIの再利用を放棄し、UI実装を2〜3系統保守する。
- `Application` 相当の機能をコアAPIとして定義し、
  既存アプリをlibrary crateへリファクタする作業が必要。
- KMPを選ぶ場合は Kotlin ツールチェーンが追加で必要。

### 選択肢C: WASM/Webラッパーでモバイルブラウザを対象

内容: gpui-kit を wasm 化し、モバイルブラウザで動作させる。

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
| A | 高 | 極大 | forkとレビュー要 | 可能だが長期 |
| B | 低 | 中〜大 | 境界が明確 | 可能 |
| C | 高（理論） | 大 | 変更なし | 現時点不可 |
| D | 高 | 小 | 変更なし | 一部のみ |

## 推奨決定（未確定）

現時点の推奨は、**D（Windows先行）を直ちに実行し、
モバイルの目標アーキテクチャとして B を Phase 1 のspike（検証）で確認する**
という二段構えである。

**ユーザーにより 2026-11-23 に推奨が承認され、status を `approved` に変更した。**
承認内容: D（Windows先行）を直ちに実行し、モバイルの目標アーキテクチャは
B（Rustコア共有＋SwiftUI / Jetpack Compose）として Phase 1 の spike で検証する。
選択肢A（GPUI fork）は採用しないため、そのライセンス方針の承認は不要。
Phase 1 の具体的な着手は、この ADR の承認をもってゲートを満たしたものとする。

理由:

- A は gpui-kit の設計意図（モバイル除外）に逆らい、
  Android の `Platform` をゼロから作る必要があり、継続的なfork保守が避けられない。
- C はローカル検証で既に不成立。
- B はコア資産を残しつつOS標準APIを活かせ、最もリスクが低い。
- D は判断を遅らせつつ、今の成果を出せる。

## 実装フェーズ

### Phase 0: Windowsパリティとレスポンシブレイアウト

- `gpui-pre-windows` の実装済みファイルAPIをアプリへ接続し、Windowsビルドを検証。
- `trash` のWindows backendを有効化したまま、macOS / iOS / Android のno-op fallbackを維持。
- ウィンドウ幅に応じたレイアウトを整備し、将来のタブレット表示の前提を作る。

### Phase 1: モバイルアーキテクチャspike（本ADR承認後）

- 選択肢Bの検証として、model / ops / settings / search をlibrary crateへ抽出する境界を定義。
- SwiftUI / Jetpack Compose の薄いビューアを1画面だけ試作し、
  コアAPI経由で一覧・検索を表示。
- 選択肢Aとの工数差とライセンス境界を再見積もりし、最終方針をユーザーへ提示。

### Phase 2: 最小Android/iOSビューア

- 読み取り専用: 一覧、検索、ファイル選択（ネイティブpicker）、システム共有、reveal。
- ゴミ箱ビューは各OSで対応手段が見つかるまで非表示を維持。

### Phase 3: フルパリティ

- 書き込み操作、設定の同期、
  プラットフォーム固有のUX（権限、SAFの永続URI、iOSのサンドボックス）に対応。
- デスクトップとの回帰テストを共通コア側で担保。

## 結果とリスク

- 良い結果: コアロジックを1か所で保守しつつ、各OSで自然なUXを提供できる。
- 悪い結果: モバイルUIを別系統で持つため、UI改修コストがOS数に比例する。
- リスク:
  - ライセンス: GPUI（Apache-2.0）をfork・改変してAGPLアプリに結合する場合の義務範囲
    （選択肢A）。
  - 工数: Android `Platform` 自作は見積もりが困難（選択肢A）。
  - 要件: Dのみで完了とみなさず、Phase 1 の承認を必ずゲートにする。

## 確認が必要な事項（ユーザー判断）

- [x] モバイル対応の目標アーキテクチャを B（コア共有＋ネイティブUI）で承認
  （2026-11-23、ユーザー承認）。
- [x] 選択肢A（GPUI維持・fork）は不採用としたためライセンス方針の承認は不要。
- [x] Phase 0 を先にリリースし、Phase 1 以降をこの ADR 承認のもとで進めることを承認。

## 参照

- `app/Cargo.toml`, `app/Cargo.lock`, `app/src/trash_view.rs`
- `gpui-kit-0.9.1/Cargo.toml`, `gpui-kit-0.9.1/src/lib.rs`
- `gpui-pre-platform-0.3.12/Cargo.toml`, `gpui-pre-platform-0.3.12/src/gpui_platform.rs`
- `gpui-pre-0.3.12/src/app.rs`
- `gpui-pre-ios-0.3.12/src/ios/platform.rs`, `gpui-pre-ios-0.3.12/examples/ios.rs`
- `gpui-pre-windows-0.3.12/src/platform.rs`
- `trash-5.3.0/Cargo.toml`
- [wasm-browser-notes.md](../../notes/wasm-browser-notes.md)
