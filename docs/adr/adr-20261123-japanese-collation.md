---
type: ADR
title: "日本語ファイル名の五十音順ソート（ICU4X collation）の選定"
description: "ロケール照合を icu_collator で実現する段階的導入方針と後方互換要件の記録"
tags:
  - collation
  - icu4x
  - japanese
  - sorting
  - licensing
generated:
  by: "research-icu"
  at: "2026-11-23T01:25:17+09:00"
verified:
  by: "research-icu"
  at: "2026-11-23T01:25:17+09:00"
status: draft
---

## コンテキスト

ファイル一覧の並び順は現在「ロケール非依存の自然順」である。
一方、日本語のファイル名を五十音順に並べたいという要件が P3（差別化）として
設計文書に載っており、ICU4X の利用が前提とされている。

本 ADR は、ローカルのコード・設計文書・依存ロックファイルと、
crates.io / docs.rs / GitHub の一次情報だけを根拠に、
実現選択肢を比較して**推奨方針**を記録する。
結論は確定決定ではなく、ユーザーの承認まで `status: draft` を維持する。

## 検証済み事実（ローカル）

### 現行の並び順契約

- `app/src/model.rs` は `alphanumeric_sort::compare_str` を import する。
- `app/src/model.rs` の `Entry` は `name_text` と、
  その小文字事前計算値 `sort_key` をフィールドとして持つ。
- `sort_key` は `Entry::from_dir_entry` と `entry_for_path` で
  `name_text.to_lowercase()` から作られる。
- `compare_names` / `compare_precomputed` は、
  小文字キーの自然順比較の後に元文字列の比較で同値を解決する。
- `sort_entries` はディレクトリを先に出し、
  選択キーの比較の後に自然順を tie-break として使う。
  「Use the precomputed lowercase key so sorting never allocates.」の
  コメントどおり比較中のアロケーションはない。
- `app/src/strings.rs` のモジュールドキュメントは「日本語の名前は Unicode スカラ値順になり
  **五十音順にはならない**」「ICU4X によるロケール照合は P3 の未実装項目」と明記する。
- `app/src/strings.rs` の `sort_and_case_contract_is_explicit` は
  `compare_names("あ", "ア") == Less`、`compare_names("ア", "一") == Less` を固定する。
- `SortSpec` が永続化対象で、フィールドは `key` / `dir` / `dirs_first` のみ。`Entry` 自体は
  `#[derive(Clone, Debug)]` のみで serde 対象外であり、永続化されない。

注: 依頼文の「DESIGN.md:112」および `strings.rs` のモジュールドキュメントが参照する
「DESIGN.md:103, 1075」は、現在のファイルの行番号と乖離している。
該当箇所は DESIGN §2「機能マトリクス」の「ロケール照合（五十音順）/ ICU4X」行と
§10.8「ロードマップ」の P3 差別化行（ロケール照合を収録）である。
行番号は並行変更で変わるため、今後は章節名で参照する。

### 既存の調査メモとライセンス許可

- `notes/fs-cross-platform-notes.md` §1（推奨表）は候補を
  「`icu_collator` / OS API、2.4.0」と記載する。
- 同ファイル §1.4（ロケール対応照合）は Linux で `icu_collator` 2.4.0（Unicode-3.0）を挙げつつ、
  「MVP ではロケール照合しない」と結論する。
- `deny.toml` の `[licenses].allow` には既に `Unicode-3.0` が含まれる。
- `NOTICE.md` は本リポジトリを `AGPL-3.0-or-later` とし、
  依存クレートは各自のライセンスを保持すると明記する。
- `app/Cargo.toml:1-7` は edition 2021 / `rust-version = "1.92"` /
  `license = "AGPL-3.0-or-later"` である。

### ICU4X の依存状況

- virtual workspace の正本はルート `Cargo.lock`。`app/Cargo.lock` と
  `poc/Cargo.lock` は workspace 化前の未使用残骸（README.md の「Cargo.lock」節を参照）。
- ルート `Cargo.lock` には ICU4X の推移依存が既に入っている。
  すべてバージョン `2.4.0` で、
  `icu_collections`（:2930）、`icu_locale_core`（:2944）、
  `icu_locale_fallback`（:2958）、`icu_locale_fallback_data`（:2972）、
  `icu_normalizer`（:2978）、`icu_normalizer_data`（:2992）、
  `icu_properties`（:2998）、`icu_properties_data`（:3013）、
  `icu_provider`（:3019）、`icu_segmenter`（:3036）、
  `icu_segmenter_data`（:3053）。
  `icu_segmenter` は `app/Cargo.lock:2175` で GPUI 系クレートの依存として引かれている。
- **`icu_collator` は `app/Cargo.lock` / `poc/Cargo.lock` の双方に存在しない。**
  いずれの `Cargo.toml` にも直接依存として載っていない。
- `~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f` には
  `icu_*` クレートが 1.5.0〜2.4.0 の複数バージョンキャッシュされているが、
  `icu_collator` のディレクトリはない。
  ローカルの crates.io インデックスキャッシュにも `icu_collator` のエントリはない。
  すなわち新規ダウンロードと `Cargo.lock` 更新が必要である。

## 外部調査（一次情報）

ICU4X 公式リポジトリは `unicode-org/icu4x`（GitHub 検索で確認）。
2026-11-22 時点で更新されており、保守が継続している。

- クレート名: `icu_collator`（[crates.io](https://crates.io/crates/icu_collator)）。
- 現在の安定版: `2.4.0`。公開日 2026-10-22、非 yanked。
- ライセンス: `Unicode-3.0`（crates.io メタデータおよび docs.rs の両方で確認）。
- MSRV: Rust 1.88 / edition 2024。アプリの MSRV 1.92 を下回り互換。
- 既定フィーチャは `compiled_data`（ baked データを同梱）。
  代替として `serde` + `BufferProvider` による外部データも可能。
- クレートアーカイブのサイズは `icu_collator` が 1,288,909 バイト、
  `icu_collator_data` 2.4.0 が 480,488 バイト（crates.io メタデータ）。
  実バイナリへの影響は未計測のため、Phase 0 で実測する。
- API（[docs.rs 2.4.0](https://docs.rs/icu_collator/2.4.0/icu_collator/)）:

```rust
use icu::collator::{options::CollatorOptions, Collator};
use icu::locale::locale;

let prefs = locale!("ja").into();
let options = CollatorOptions::default();
let collator = Collator::try_new(prefs, options).unwrap();
assert_eq!(collator.compare("あ", "い"), std::cmp::Ordering::Less);
```

- `Collator::try_new` の第 1 引数は 2.x では `CollatorPreferences` であり、
  生の `Locale` ではない（`CollatorPreferences: From<Locale>` により
  `locale!("ja").into()` で渡せる）。1.5.0 では `&DataLocale` であったため、
  旧資料の API 記述はそのまま使えない。
- 比較メソッドは 1.5.0 / 2.4.0 のいずれの公式ドキュメントでも
  `compare(left, right) -> Ordering` であり、`compare_to` は存在しない。
  依頼文の API 想定（`locale` 直接渡し・`compare_to`）は訂正が必要である。
- 数値順序は `CollatorPreferences.numeric_ordering =
  Some(CollationNumericOrdering::True)`（BCP-47 `kn`）で有効化できる。
  かなの大小は `case_first`（`kf`）が担当する。
- `write_sort_key_to` でソートキーバイト列を生成できるが、公式ドキュメントは
  「CLDR 更新・Unicode 更新・ICU4X 更新で無効になり得るため、
  保存するなら再計算に備えるべし」と警告している。

## 選択肢

### 案 A: ICU4X collator を opt-in 設定で導入（推奨）

`icu_collator` 2.4.0 を直接依存に追加し、日本語ロケールの照合を
独立した pure ヘルパーに閉じ込める。初期状態は無効で、
設定の `locale_collation = true` を選んだときだけ使う。

- 利点: CLDR の照合規則に追従し、五十音順・濁点/半濁点・全角/半角の
  扱いを自前で定義しなくてよい。ライセンスは既に `deny.toml` が許可済み。
- 利点: 既存の `alphanumeric_sort` を温存できるため、
  数字・ASCII の自然順と既定の並び順を壊さない。
- 難点: 新規依存とデータ同梱によるバイナリ増加。比較はアロケーションを伴い得るため、
  `sort_key` の事前計算設計との整合を Phase 0 で検証する必要がある。

### 案 B: 自作のかな表で五十音順を実装（却下）

- 五十音順の表（あいうえお…）を自前の `match` / 配列で定義する案。
- 却下理由 1: 濁点・半濁点（は<ば<ぱ）、小書き仮名、拗音、
  全角/半角、漢字の読み（同音異字）を正しく扱うには CLDR 相当の規則が必要で、
  表だけでは要件を満たさない。
- 却下理由 2: Unicode / CLDR の更新追従と回帰テストの維持コストが高く、
  車輪の再発明になる（`AGENTS.md` §2 の事前調査方針にも反する）。
- 却下理由 3: 既存の `notes/fs-cross-platform-notes.md` §1.4（ロケール対応照合）が
  既製候補として `icu_collator` を特定済みで、採用障害が見当たらない。

### 案 C: std のみで現状維持（既定値としては採用、単独の解決策としては却下）

- `str::cmp` と `to_lowercase()` だけを使う現在の方式。
- 五十音順要件を満たさず、`strings.rs` のモジュールドキュメントの
  「五十音順にはならない」という契約を維持するだけであるため、要件対応策としては却下する。
- ただし**既定の挙動としては維持**する（後方互換要件を参照）。

## 推奨決定

**案 A を推奨する。** 五十音順は CLDR の照合規則そのものであり、
自作表（案 B）で再現すべきではない。
また案 C の挙動を既定として残したまま、opt-in で段階導入する。

## 実装計画（3 フェーズ）

### Phase 0: pure ヘルパーとテストを feature flag の背後に置く

- 新規モジュール（例 `app/src/collation.rs`）に GPUI 非依存の
  `compare_names_ja(&str, &str) -> Ordering` を置く。
- Cargo feature（例 `locale-collation = ["dep:icu_collator"]`）を追加し、
  feature 無効時は現行の `compare_precomputed` とバイト単位で同じ結果にする。
- 依存宣言は `icu_collator = { version = "2.4.0", default-features = false,
  features = ["compiled_data"] }` を想定するが、バージョン固定と
  feature 設計は実装タスクで確定する。本 ADR の作成タスクでは
  `Cargo.toml` / `Cargo.lock` を変更しない。
- `locale!("ja")` の `CollatorPreferences` と `CollatorOptions::default()`
  で collator を一度だけ生成し、比較ごとの生成を避ける。
- 受入テスト（下記）を feature 有効時のみ実行する。
- 10,000 件の名前に対するソート時間を計測し、現行比を記録する。

### Phase 1: 設定の opt-in `locale_collation`

- `SortSpec` に `#[serde(default)] pub locale_collation: bool` を追加し、
  既定値を `false` にする。旧設定ファイルは変更なしで読める。
- 設定 UI に「五十音順（日本語）」のトグルを追加し、
  `true` のときだけ `compare_names_ja` を呼ぶ。
- 設定変更時に `Entry.sort_key` を再計算し、古いキーが残らないようにする。
- 数字・ASCII のみの名前は現行の自然順と同じ結果になることを
  互換テストで固定する（必要なら `numeric_ordering = True` と
  `alphanumeric_sort` の併用方針をこのフェーズで決定する）。

### Phase 2: 検証後の既定切り替え

- Phase 0/1 のテスト・実測・ユーザー評価が揃った後に限り、
  新規インストールの既定を `true` へ変更する。
- 既存インストールは `false` を維持し、設定から opt-out できるようにする。
- 切り替えはリリースノートで明示し、五十音順の範囲と制限を記載する。

## データサイズとライセンス

- `compiled_data` 既定フィーチャは照合データを静的に同梱する。
  クレートアーカイブは `icu_collator` 約 1.29 MB、
  `icu_collator_data` 約 0.48 MB（いずれも 2.4.0）。
  実際のバイナリ増加量は未検証で、Phase 0 の実測で確定する。
- 必要なら `default-features = false` + `serde` + `BufferProvider` で
  データを外部化できるが、GUI アプリでは運用の複雑度が増すため初期採用は見送る。
- `icu_collator` 2.4.0 は `Unicode-3.0` であり、AGPL-3.0-or-later と
  両立する（非 copyleft）。`deny.toml` は既に `Unicode-3.0` を許可しており、
  新たなライセンス例外は不要である。
- 依存追加時は `app/NOTICE.md` に ICU4X（`unicode-org/icu4x`,
  `icu_collator` / `icu_collator_data`, Unicode-3.0）を追記する。
- `cargo deny check` の licenses / advisories / bans / sources を
  依存追加後に再実行し、既存 baseline との差分を確認する。

## 後方互換要件

- 既定の並び順（`locale_collation = false`）は現行と完全に同一でなければならない。
  既存の契約テスト（`app/src/strings.rs` の `sort_and_case_contract_is_explicit`、
  `app/src/model.rs` の `sort_dirs_first_then_natural_name_order` など）は
  すべて green を維持する。
- `SortSpec` に追加するフィールドは `#[serde(default)]` とし、
  フィールドが無い旧 JSON も読み込めるようにする。
- 現在 `Entry.sort_key` はメモリ上のみ（`Entry` は serde 対象外）で
  ディスクに永続化されていない。したがって今回の設定追加で
  ディスク上のソートキー移行は不要である。
- 将来ソートキーを永続化する場合は、バージョン番号と
  ICU4X / CLDR / Unicode のバージョンをメタデータとして付与し、
  設定変更時とデータ更新時に必ず再計算する。
  これは `write_sort_key_to` の公式警告（更新で無効になり得る）に従う要件である。
- `true` → `false` への切り替え時も再計算を行い、
  五十音順キーが残って既定の自然順を汚さないようにする。

## 受入基準とテスト戦略

### 受入基準

1. feature 無効時、既存の自然順・大小文字無視・同値 tie-break が変化しない。
2. `locale_collation = true` のとき、五十音順の基本序列が成立する。
3. 濁点・半濁点、かな種別、全角/半角の扱いは ICU4X の既定に従い、
   アプリ側で独自に上書きしない。
4. 数字と ASCII の並び順は現行と同じ（`file2 < file10`、`alpha < Zebra`）。
5. 既定値は `false` のままで、旧設定ファイルが読み込める。
6. 設定変更時にソートキーが再計算され、古い順序が残らない。
7. `cargo fmt` / `cargo check` / `cargo test` / `cargo clippy` と
   `cargo deny check` が既存 baseline を壊さない。

### テスト戦略

- 単体テスト（pure ヘルパー、feature 有効時）:

```rust
assert_eq!(ja("あ", "い"), Ordering::Less);
assert_eq!(ja("い", "う"), Ordering::Less);
assert_eq!(ja("う", "え"), Ordering::Less);
assert_eq!(ja("え", "お"), Ordering::Less);
assert_eq!(ja("は", "ば"), Ordering::Less);
assert_eq!(ja("ば", "ぱ"), Ordering::Less);
// あ < ア < ｱ の前後関係は ICU4X 既定の判定をゴールデン値として固定する。
assert_eq!(ja("file2", "file10"), Ordering::Less);
assert_eq!(ja("alpha", "Zebra"), Ordering::Less);
assert_eq!(ja("ABC", "abc").is_eq(), false);
```

- 回帰テスト: `app/src/strings.rs` の `sort_and_case_contract_is_explicit`
  を feature 無効時にそのまま通す。
- 設定テスト: `locale_collation` が無い旧 JSON、明示 `false`、
  明示 `true` の 3 パターンを `serde_json` で往復させる。
- プロパティテスト: 10,000 件のランダムな日本語混在名で
  比較が推移的かつ決定论的であること、パニックしないことを確認する。
- 実測: 10,000 件のソート時間とバイナリサイズを
  feature 前後で比較し、結果を `notes/` に記録する（本 ADR では未計測）。
- 文書検証: 本 ADR は OKF フロントマターと markdownlint を満たすこと。

## 結果（予想される影響）

- 正面: 日本語ユーザーが期待する五十音順を opt-in で提供できる。
- 正面: ICU4X は実績のある CLDR ベース実装で、保守が継続している。
- 负面: 新規依存とデータ同梱によるバイナリ増加（値は未計測）。
- 负面: 設定・テスト・リリースノート・NOTICE の更新範囲が増える。
- リスク: ICU4X のマイナー更新で照合結果やソートキーが変わり得る。
  バージョン固定と再計算手順で吸収する。

## 参照

- [ICU4X リポジトリ](https://github.com/unicode-org/icu4x)
- [icu_collator（crates.io）](https://crates.io/crates/icu_collator)
- [icu_collator 2.4.0 ドキュメント](https://docs.rs/icu_collator/2.4.0/icu_collator/)
- [Collator](https://docs.rs/icu_collator/2.4.0/icu_collator/struct.Collator.html)
- [CollatorBorrowed](https://docs.rs/icu_collator/2.4.0/icu_collator/struct.CollatorBorrowed.html)
- [CollatorPreferences](https://docs.rs/icu_collator/2.4.0/icu_collator/struct.CollatorPreferences.html)
- [Unicode ライセンス（SPDX: Unicode-3.0）](https://spdx.org/licenses/Unicode-3.0.html)
