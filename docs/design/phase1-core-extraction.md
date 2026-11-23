---
type: Spec
title: "Phase 1: GPUI 非依存コア crate の抽出境界と workspace 構成"
description: "モバイルADR選択肢Bの Phase 1 spike に向け、model/ops/search 等を GPUI 非依存 crate へ抽出する設計案"
tags:
  - gpui
  - core-extraction
  - workspace
  - phase1
  - mobile
generated:
  by: "p1-design-core"
  at: "2026-11-23T14:34:25+09:00"
status: draft
---

## 目的と範囲

承認済みモバイルADR（`docs/adr/adr-20261122-mobile-android-ios-support.md:191-195`）の
Phase 1 合格条件のうち **(1) 抽出境界定義** を本設計で満たす。対象は設計のみであり、
`Cargo.toml` / ソースの直接変更は行わない。R1 の実読事実（親エージェント確認済み）を前提に、
第1スコープ・逆参照の解消・ポート定義・段階移行を具体的に定める。

## 1. クレート名・配置と workspace 構成

### クレート名

- ディレクトリ: `core/`
- package 名: `gpui-file-explorer-core`
- `[lib] name = "gfe_core"`

Rust の sysroot に `core` クレートが存在するため、package/lib を単に `core` とすると
名前解決が衝突する。app 側では `use gfe_core as core;` でローカル別名を張り、
`core::{model, ops, search}` という参照形を維持する（`app/src/model.rs:897-898` の
「crate ルートが `gpui_kit::*` を glob import する」問題からも分離できる）。

### workspace Cargo.toml（ルート新規追加案）

```toml
[workspace]
resolver = "2"
members = ["core", "app", "poc"]
default-members = ["core", "app"]

[workspace.package]
edition = "2021"
rust-version = "1.92"
license = "AGPL-3.0-or-later"

[workspace.dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"
gpui-kit = "=0.9.1"

[profile.dev]
debug = "line-tables-only"

[profile.dev.package."*"]
debug = false

[profile.release]
strip = true
```

設計上の注意点:

- 現在 `app/Cargo.toml:36-45` と `poc/Cargo.toml:14-23` にある `[profile.*]` は
  workspace ルートへ集約する。member 側の profile は警告付きで無視される。
- virtual workspace の resolver 既定は 1 のため `resolver = "2"` を明示する。
- `default-members` から `poc` を外し、ルートでの `cargo test` がコアと app を対象にする。
- core は AGPL-3.0-or-later を維持する。GPUI スタック（Apache-2.0）は app アダプタ側に
  閉じ込め、ADR 選択肢B のライセンス境界（`docs/adr/adr-20261122-mobile-android-ios-support.md:133`）
  を保つ。
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
   `original_parent_for_path` / `items_for_paths` / `record_ids` と、`TrashItem` 相当の
   データ契約（§4 の `TrashBackend` 関連型）のみ。
2. 第2層（app アダプタ）: ブロッキングな platform API（`:27-76`）と
   cfg no-op fallback（`:78-125`）。`trash` クレート自体は `app/Cargo.toml:33-34` に残す。
3. 第3層（UI）: `cx.background_spawn` で第2層を呼ぶ（`:46-47` の契約コメントを維持）。

これにより iOS / Android では第2層が no-op のままでも、コアの単体テストは全ターゲットで通る。

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
`MigrationReport { read_only: bool, unknown_fields: Vec<String> }` を返す純関数だけを持たせる。
`std::fs`・パス注入（`load_from` / `save_to`）・UI 型は core へ持ち込まない。

## 3. model → settings 逆参照の解消

現状: `model.rs:348-363` の `deserialize_sort_key` と
`:507-520` の `deserialize_sort_dir` が
`crate::settings::note_unknown_persisted_enum()`（`settings.rs:222`）を呼び、
thread_local `UNKNOWN_ENUM_SEEN`（`:205,213`）を立てる。`load_from_with_backup`（`:1281`）が
`:1382-1390` でこれを消費して `read_only` を決定する。model を core へ移すと依存が逆転し、
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

## 4. ポート（トレイト）定義草案

置き先は `core/src/ports.rs`（トレイトとデータ型のみ、GPUI も platform crate も依存しない）。
実装は app が `app/src/ports_gpui.rs` に、モバイルは各ネイティブ層に置く。

| トレイト | 役割 | GPUI アダプタ | SwiftUI / Compose アダプタ |
| --- | --- | --- | --- |
| `TrashBackend` | 一覧・復元・削除 | `trash_view.rs` platform | no-op → Phase2 |
| `VolumeSource` | ボリューム一覧 | `app/src/mounts.rs` | 外部ストレージ |
| `ImageDecoder` | アイコン解析 | `icons.rs` 等 | `UIImage` / `Bitmap` |
| `OpenRevealPrompt` | picker / reveal | gpui-pre API | `UIDocPicker` / SAF |
| `Clipboard` | uri-list 読み書き | OS clipboard | `UIPasteboard` / `CM` |
| `WatchSource` | ディレクトリ監視 | `notify` | FSEvents / `FileObserver` |

gpui-pre の prompt は iOS がスタブ
（モバイル ADR `:62-66`）、
Windows が実装済み（同 `:72-75`）という ADR 事実を境界の根拠とする。

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

呼び出し形は `Explorer::new(trash, volumes, images, prompt, clipboard, watcher, progress)`
のような依存注入を想定する。同期メソッドのブロッキング契約は `trash_view.rs:46-47` を踏襲し、
UI 側がバックグラウンド実行を担当する（Phase 1 で async 化しない）。

SwiftUI / Compose 側は、Rust コアを staticlib としてビルドし（iOS の precedent は ADR `:67`）、
uniFFI / JNI 経由で `Explorer::list(path)` と `Explorer::search(query)` を呼ぶ。
各ポートの実装だけを Swift / Kotlin の第一級 API で書き、GPUI はバイナリに含めない。

## 5. 段階移行手順と検証ゲート

| 段階 | 内容 | 検証ゲート |
| --- | --- | --- |
| 0. workspace | ルート Cargo.toml 追加、members 集約 | fmt/check/test/clippy/deny |
| 1. core 骨格 | Cargo.toml + lib.rs + error/strings/ports | 同上 + core 単体テスト |
| 2. 本体移動 | model/search/ops/trash 純関数。§3 案3 | 同上 + app 全テスト |
| 3. re-export | `pub use gfe_core as core;` で参照を一本化 | 同上 + cargo tree |
| 4. 1画面試作 | core API 経由の一覧・検索ビューアを iOS/Android 双方で作成 | 同上 + デスクトップ回帰なし |

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

### 受け入れ条件

1. 抽出境界を本設計で定義したこと（ADR Phase 1 条件 (1)）。
2. コア API 経由で一覧・検索を出す1画面が iOS / Android 双方で試作できること（同条件 (2)）。
3. 試作後に選択肢Aとの工数差・ライセンス境界を再見積もりしユーザーへ提示すること
   （同条件 (3)、`docs/adr/adr-20261122-mobile-android-ios-support.md:218`）。
4. `core/src/lib.rs` に `#![forbid(unsafe_code)]` があること。
5. `cargo tree -p gpui-file-explorer-core` に `gpui-*` が含まれないこと。
6. `settings.rs:3458,3531,3560` の unknown enum → read-only 回帰が通ること。
7. fmt / check / test / clippy / deny / audit の全ゲートが green であること。

### 未解決点（設計判断として残す項目）

- クレート名 / lib 名の最終決定。`core` 単体は sysroot 衝突で不可。
- 案3の `PersistedSortSpec` を app に置くか、core の `persisted` feature に置くか。
- `TrashBackend` を async トレイトにするか（現状は同期 + background_spawn 契約）。
- `ImageDecoder` の出力形（生ピクセル / サムネイルバイト）と Breeze アイコン資産の
  ライセンス表示先。
- `WatchSource` の通知を `dyn Fn` コールバックにするか channel にするか。
- モバイル adapter の配置（単一 staticlib + uniFFI か、iOS / Android で別 crate か）。
- ライセンス境界: core を AGPL のまま公開するか、Apache-2.0 で再ライセンスして
  採用範囲を広げるかはユーザー判断として残す。

## 参照

- `docs/adr/adr-20261122-mobile-android-ios-support.md`（承認済み、Phase 1 条件 :213-218）
- `app/src/model.rs`, `app/src/ops.rs`, `app/src/search.rs`, `app/src/error.rs`
- `app/src/strings.rs`, `app/src/trash_view.rs`, `app/src/settings.rs`
- `app/Cargo.toml`, `poc/Cargo.toml`
- `README.md:170-172`, `DESIGN.md:57-62,1274-1276`（古い draft・未承認表記は別スコープで修正）
