---
type: Spec
title: "Phase 1: GPUI 非依存コア crate の抽出境界と workspace 構成"
description: >-
  Rust コア共有 + Compose（Android アプリ）/ PWA の Phase 1 spike に向け、
  model/ops/search 等を GPUI 非依存 crate へ抽出する設計案
tags:
  - gpui
  - core-extraction
  - workspace
  - phase1
  - mobile
  - pwa
generated:
  by: "p1-design-core"
  at: "2026-11-23T14:34:25+09:00"
status: draft
---

## 目的と範囲

承認済みモバイルADR（`docs/adr/adr-20261122-mobile-android-ios-support.md の「Phase 1: モバイルアーキテクチャspike」節`）の
Phase 1 合格条件のうち **(1) 抽出境界定義** を本設計で満たす。対象は設計のみであり、
`Cargo.toml` / ソースの直接変更は行わない。R1 の実読事実（親エージェント確認済み）を前提に、
第1スコープ・逆参照の解消・ポート定義・段階移行を具体的に定める。

新方針（2026-11-23、AGENTS.md「クロスプラットフォーム対応方針」）では、
選択肢Bは「Rust コア共有 + Jetpack Compose（Android アプリ）/
Web PWA（iOS および Android PWA）」を指す。SwiftUI アプリ版は提供しない。
iOS を PWA のみとする主因は、iOS アプリ配布に Apple Developer Program の
年会費が必ずかかるというコスト理由である。技術的理由（gpui-kit の wasm
安定パスなし、後述 §技術的制約）と併せて決定根拠とする。

## 1. クレート名・配置と workspace 構成

### クレート名

- ディレクトリ: `core/`
- package 名: `gpui-file-explorer-core`
- `[lib] name = "gfe_core"`

Rust の sysroot に `core` クレートが存在するため、package/lib を単に `core` とすると
名前解決が衝突する。app 側では `use gfe_core as core;` でローカル別名を張り、
`core::{model, ops, search}` という参照形を維持する。旧 `app/src/model.rs` の
「crate ルートが `gpui_kit::*` を glob import する」問題からも分離できる
（as-built: `app/src/model.rs` は `gfe_core::model` の re-export シェルで、
実体は `core/src/model.rs`）。

### workspace Cargo.toml（as-built 2026-11-23）

旧本文は「ルート新規追加案」だったが、仮想 workspace は**実装済み**である。
現行の正本はルート `Cargo.toml`。下記スニペットは現行の正本と同じ構成
（win32 / `gfe-win32` の member 追加を反映）を抜粋したものであり、
正確な内容は正本を参照する。未採用の `[workspace.dependencies]` は含めない。

```toml
[workspace]
resolver = "2"
members = ["core", "app", "poc", "win32"]
default-members = ["core", "app"]

[workspace.package]
edition = "2021"
rust-version = "1.92"
license = "AGPL-3.0-or-later"

[profile.dev]
debug = "line-tables-only"

[profile.dev.package."*"]
debug = false

[profile.release]
strip = true
```

設計上の注意点:

- `[profile.*]` は workspace ルート `Cargo.toml` へ集約済み（as-built）。
   `profile.dev`（`debug = "line-tables-only"`）/ `profile.dev.package."*"`
   （`debug = false`）/ `profile.release`（`strip = true`）を定義する。
   member 側（app / poc）の `[profile.*]` は削除済みで、残ると警告付きで無視される。
- virtual workspace の resolver 既定は 1 のため `resolver = "2"` を明示する。
- `default-members` から `poc` を外し、ルートでの `cargo test` がコアと app を対象にする。
- core は AGPL-3.0-or-later を維持する。GPUI スタック（Apache-2.0）は app アダプタ側に
  閉じ込め、ADR 選択肢B のライセンス境界を保つ
  （`docs/adr/adr-20261122-mobile-android-ios-support.md:133`）。
- `core/src/lib.rs` 先頭に `#![forbid(unsafe_code)]` を明記する
   （`app/src/main.rs:8` の crate 単位規約と同準拠）。

## 2. 第1スコープのファイル一覧と根拠

| 移動元 | 移動先 | 根拠 | 注意 |
| --- | --- | --- | --- |
| `error.rs` | `core/error.rs` | `:16-20`: 依存 `std` のみ | 移動可 |
| `model.rs` | `core/model.rs` | `:7`: GPUI 一致は doc/test | 逆参照 §3 |
| `strings.rs` | `core/strings.rs` | `:1-5`: 定数のみ | 移動可 |
| `ops.rs` | `core/ops.rs` | `:41`: GPUI-free | 移動可 |
| `search.rs` | `core/search.rs` | `:16`: GPUI-free | 移動可 |
| `trash_view.rs` 純関数 | `core/trash_view.rs` | FS 非アクセス | platform は app |

### trash_view の 3 層パターン

`app/src/trash_view.rs:10` の GPUI-free 先行例をそのまま境界として使う。

1. 第1層（core）: 純関数 `partition_restore_ids` / `entries_from_items` /
   `original_parent_for_path` / `items_for_paths` / `record_ids` と、
   `TrashItem` 相当のデータ契約（§4 の `TrashBackend` 関連型）のみ。
2. 第2層（app アダプタ）: ブロッキングな platform API（`:27-76`）と
   cfg no-op fallback（`:78-125`）。`trash` クレート自体は `app/Cargo.toml:33-34` に残す。
3. 第3層（UI）: `cx.background_spawn` で第2層を呼ぶ（`:46-47` の契約コメントを維持）。

これにより Android / PWA では第2層が no-op のままでも、コアの単体テストは全ターゲットで通る。

### settings の扱い（完全移行は別スコープ）

`Settings` 本体・`load_from`（`settings.rs:1275`）/ `save_to`（`:1750`）は app に残す。
理由は `settings.rs:71-72` の `gpui_kit::ThemeMode / Window / WindowBounds` 直依存と、
`:74,76` の `history` / `tabs::Tab` 型依存で、そのまま入れると GPUI-free が壊れるため。

Phase 1 で core へ切り出すのは純粋な判定部分に限定する。

- `SCHEMA_VERSION` / `LEGACY_SCHEMA_VERSION`（`:92,95`）と `schema_status` 相当の
   バージョン判定。
- `deserialize_theme_choice`（`:266-280`）/ `deserialize_theme`（`:296-310`）の
   文字列分類を、GPUI 型を受け取らない `classify_theme_*(&str) -> ThemeValue` として抽出。
- `migrate`（`:1410-1421`）のバージョン判定を
   `migrate_version(loaded: u32, current: u32) -> MigrationStatus` として抽出。

承認済みADR:215の「settings を抽出対象とする」は、型・判定を core へ置き、
I/O と gpui_kit 型変換を app アダプタへ残す形で充足する（条件 (1) の充足形）。
置き先は `core/src/settings_persistence.rs` とし、
`MigrationReport` を返す純関数だけを持たせる
（フィールドは `read_only: bool` と `unknown_fields: Vec<String>`）。
`std::fs`・パス注入（`load_from` / `save_to`）・UI 型は core へ持ち込まない。

## 3. model → settings 逆参照の解消

現状: `model.rs:348-363` の `deserialize_sort_key` と
`:507-520` の `deserialize_sort_dir` が
`crate::settings::note_unknown_persisted_enum()`（`settings.rs:222`）を呼び、
thread_local `UNKNOWN_ENUM_SEEN`（`:205,213`）を立てる。
`load_from_with_backup`（`:1281`）が `:1382-1390` でこれを消費して
`read_only` を決定する。model を core へ移すと依存が逆転し、
core が app（ひいては GPUI）を参照するため解消が必須。

| 案 | 概要 | 長所 | 短所 |
| --- | --- | --- | --- |
| 1. thread_local | core にシグナルを置き app が take | 差分最小 | スレッドアフィンで async に弱い |
| 2. load 結果に bool | 純粋 validate → `LoadReport` 返却 | 隠れ状態なし・ LTS安全 | テスト改修が大きい |
| 3. hook を app へ | core は素の serde のみ | settings 非依存・挙動維持 | 文字列定義が分散 |

### 推奨

**Phase 1 は案3、Phase 2 で案2へ収束する** ことを推奨する。

- 案3の具体: `model.rs:542,544` の `#[serde(deserialize_with = ...)]` と `:348,507` の
   2関数を app の `PersistedSortSpec` へ移し、core の `SortKey` / `SortDir` は
   `rename_all = "snake_case"` と安全なデフォルトだけを持つ。thread_local は app 内に残る。
- 理由: 第1スコープの目的は「core を GPUI/settings 非依存にして一覧・検索を試作で出す」こと。
   案3なら2フックを消すだけで逆参照が完全に断てる。
- 案1は短期安全だが隠れ状態を core に持ち込むため採用しない。
- 案2は最終形として正しいが、settings の大規模テスト改修を Phase 1 に載せるのはリスクが高い。
- 案3の乖離リスクは `PersistedSortSpec` に `TryFrom<&str>` / `From<SortSpec>` を実装し、
   対応表を一箇所へ集約して緩和する。

### as-built: Phase 2 案2へ収束済み（2026-11-23）

R2-SIGNAL の並列ロード検証で、案3の thread_local `Cell` + process-global
`AtomicBool` の二段シグナルは N 個の独立ロードを1ビットで表現しようとしており
構造的に破綻していることが確定した。T_A の note 後に T_B の事前クリア
`swap(false)` が走ると T_A のシグナルが失われ未知 enum の元値が
フォールバック値で上書きされる。逆に T_B が T_A の note を拾うと正常ファイルが
read_only になる。memory order や `AtomicBool` 延命では解消しないため、
計画より前倒しで案2（load 結果に bool / LoadReport）へ収束済み。

- `core/src/model.rs` の `UNKNOWN_PERSISTED_ENUM_LOCAL` / `_SEEN`、
  `note_unknown_persisted_enum` / `take_unknown_persisted_enum` と
  `persisted_enum_signal_tests` を削除。`deserialize_sort_key` /
  `deserialize_sort_dir` は素の fallback のみ（シグナルは立てない）。
- `app/src/settings.rs` の純関数 `detect_unknown_persisted_enum(&Value)` が、
  既にパース済みの生 JSON の `sort.key` / `sort.dir` / `view` / `theme` /
  `theme_choice` / `windows[].bounds.mode` / `column_prefs.visible|widths` を照合し、
  `load_from_with_backup` が当該ロード単位で `read_only` を決定する。
  既知値は各 enum の `Deserialize` 実装と `COLUMN_KEYS` /
  `is_known_column_key` を再利用し、重複定義しない。
- 回帰テスト `parallel_loads_keep_read_only_scoped_to_each_file` が
  32スレッド × 4ラウンドの正常／未知 enum 混在ロードで、
  各ロードの `read_only` が自ファイル入力と一致することを検証する。

## 4. ポート（トレイト）定義草案

置き先は `core/src/ports.rs`（トレイトとデータ型のみ、GPUI も platform crate も依存しない）。
実装は app が `app/src/ports_gpui.rs` に、モバイルは各ネイティブ層に置く。

| トレイト | 役割 | GPUI アダプタ | Compose / PWA アダプタ |
| --- | --- | --- | --- |
| `TrashBackend` | 一覧・復元・削除 | `trash_view.rs` platform | no-op → Phase2 |
| `VolumeSource` | ボリューム一覧 | `app/src/mounts.rs` | 外部ストレージ / 空 |
| `ImageDecoder` | アイコン解析 | `icons.rs` 等 | `Bitmap` / Web 画像 |
| `OpenRevealPrompt` | picker / reveal | gpui-pre API | SAF / FSA API |
| `Clipboard` | uri-list 読み書き | OS clipboard | CM / Web Clipboard |
| `WatchSource` | ディレクトリ監視 | `notify` | `FileObserver` / ポーリング |

gpui-pre の prompt は iOS がスタブ（モバイル ADR `:62-66`）、Windows が実装済み
（同 `:72-75`）という ADR 事実を境界の根拠とする。iOS アプリ版を作らないため、
iOS スタブは PWA 方針ではブロッカーではない。なお FSA API は File System
Access API、CM は Android の ClipboardManager を指す。

```rust
// core/src/ports.rs
pub trait TrashBackend {
    fn list(&self) -> Result<Vec<TrashEntry>, PortError>;
    fn restore(&self, ids: &[OsString]) -> Result<RestoreReport, PortError>;
    fn delete(&self, path: &Path) -> Result<(), PortError>;
}

pub trait VolumeSource {
    fn volumes(&self) -> Vec<VolumeInfo>;
}

pub trait ImageDecoder {
    fn decode(&self, bytes: &[u8]) -> Result<ImageInfo, PortError>;
}

pub trait OpenRevealPrompt {
    fn pick_paths(&self) -> Result<Vec<PathBuf>, PortError>;
    fn reveal(&self, path: &Path) -> Result<(), PortError>;
}

pub trait Clipboard {
    fn read_uri_list(&self) -> Result<Vec<String>, PortError>;
    fn write_uri_list(&self, uris: &[String]) -> Result<(), PortError>;
}

pub trait WatchSource {
    fn watch(&self, path: &Path) -> Result<WatchHandle, PortError>;
}
```

呼び出し形は `Explorer::new(...)` のような依存注入を想定する
（引数は trash, volumes, images, prompt, clipboard, watcher, progress）。
同期メソッドのブロッキング契約は `trash_view.rs:46-47` を踏襲し、
UI 側がバックグラウンド実行を担当する（Phase 1 で async 化しない）。

Compose 側は Rust コアを staticlib としてビルドし、JNI 経由で
`Explorer::list(path)` と `Explorer::search(query)` を呼ぶ。
ポート実装だけを Kotlin の第一級 API（SAF 等）で書き、GPUI はバイナリに含めない。
PWA 側は GPUI 非依存コアを wasm 化し、Web フロントエンドから同じ API を呼ぶ。
SwiftUI アプリ版は提供しない（コスト理由と wasm 制約は §技術的制約を参照）。

## 5. 段階移行手順と検証ゲート

| 段階 | 内容 | 検証ゲート |
| --- | --- | --- |
| 0. workspace | ルート Cargo.toml 追加、members 集約 | fmt/check/test/clippy/deny |
| 1. core 骨格 | Cargo.toml + lib.rs + error/strings/ports | 同上 + core 単体テスト |
| 2. 本体移動 | model/search/ops/trash 純関数。§3 案3 | 同上 + app 全テスト |
| 3. re-export | `pub use gfe_core as core;` で参照を一本化 | 同上 + cargo tree |
| 4. 1画面試作 | core API 経由の一覧・検索を Compose / PWA で作成 | 同上 + デスクトップ回帰なし |

段階2 の import 置換の具体:

```rust
// app/src/main.rs（段階2）
use gfe_core as core;
use core::{model, ops, search, strings};
// 旧: use crate::{model, ops, search, strings};
```

段階3 で `pub use gfe_core as core;` を app の公開面へ移し、app 固有の adapter を
`ports_gpui` として残す。`cargo deny` / `cargo audit` は workspace ルートの
依存グラフを走査対象にしてゲートへ組み込む（依存追加が無い段階も通過を確認する）。

## 6. 受け入れ条件と未解決点

### 技術的制約と検証環境

- `notes/wasm-browser-notes.md:51-91` のとおり、gpui-kit 0.9.1 には wasm の安定パスが
  存在しない。PWA は GPUI UI を再利用せず、Web フロントエンドを新規構築する。
- よって UI 系統は **desktop(GPUI) / Android(Compose) / PWA(Web) の3系統** とする。
- iOS アプリ版は提供しない。主因は Apple Developer Program の年会費が必ずかかる
  というコスト理由（ユーザー明示、2026-11-23）で、技術的理由（上記 wasm 制約）と
  併せて決定根拠とする。
- 検証環境（verified）: Kubuntu（Linux）はメイン開発環境かつ実機動作検証あり。
  Windows は評価版 Windows で動作検証する。macOS は実機動作検証を行わない。
  Android はアプリ版と PWA の両方、iOS は PWA のみを対象とする。

### 受け入れ条件

1. 抽出境界を本設計で定義したこと（ADR Phase 1 条件 (1)）。
2. Android Compose アプリと iOS / Android 共通 PWA で、コア API 経由の一覧・検索
   1画面が起動すること（同条件 (2)）。iOS アプリ版ビルドは要求しない。
3. 試作後に選択肢Aとの工数差・ライセンス境界を再見積もりしユーザーへ提示すること
   （同条件 (3)、`docs/adr/adr-20261122-mobile-android-ios-support.md:218`）。
4. `core/src/lib.rs` に `#![forbid(unsafe_code)]` があること。
5. `cargo tree -p gpui-file-explorer-core` に `gpui-*` が含まれないこと。
6. `app/src/settings.rs` の `detect_unknown_persisted_enum_*` と
   `parallel_loads_keep_read_only_scoped_to_each_file` が通ること（行アンカーは使わない）。
7. fmt / check / test / clippy / deny / audit の全ゲートが green であること。

### 未解決点（設計判断として残す項目）

- クレート名 / lib 名の最終決定。`core` 単体は sysroot 衝突で不可。
- 案3の `PersistedSortSpec` を app に置くか、core の `persisted` feature に置くか。
- `TrashBackend` を async トレイトにするか（現状は同期 + background_spawn 契約）。
- `ImageDecoder` の出力形（生ピクセル / サムネイルバイト）と Breeze アイコン資産の
   ライセンス表示先。
- `WatchSource` の通知を `dyn Fn` コールバックにするか channel にするか。
- モバイル adapter の配置（Android は staticlib + JNI、PWA は wasm バインディング、
  共有方法は未確定）。
- ライセンス境界: core を AGPL のまま公開するか、Apache-2.0 で再ライセンスして
  採用範囲を広げるかはユーザー判断として残す。

## 参照

- `docs/adr/adr-20261122-mobile-android-ios-support.md`
  （承認済み、Phase 1 条件は同 ADR の「Phase 1: モバイルアーキテクチャspike」節）
- [wasm 調査ノート](../../notes/wasm-browser-notes.md)（:51-91、gpui-kit wasm 制約）
- `AGENTS.md`（クロスプラットフォーム対応方針、2026-11-23）
- `app/src/model.rs`, `app/src/ops.rs`, `app/src/search.rs`, `app/src/error.rs`
- `app/src/strings.rs`, `app/src/trash_view.rs`, `app/src/settings.rs`
- `app/Cargo.toml`, `poc/Cargo.toml`
- `README.md:170-172`, `DESIGN.md:57-62,1274-1276`（古い draft・未承認表記は別スコープで修正）
