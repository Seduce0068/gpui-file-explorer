---
type: Reference
title: "gpui-file-explorer 上流・依存バージョンドリフト調査 (PASS 2)"
description: "Cargo.toml / Cargo.lock / build.rs / ローカル crates.io インデックスを根拠に、依存バージョン固定・未使用 feature・build script 前提から生じるタスクを列挙する"
tags:
  - gpui-file-explorer
  - task-discovery
  - upstream
  - dependency
  - cargo
generated:
  by: "r2-upstream"
  at: "2026-11-17T20:02:59+09:00"
status: complete
---

## 概要

PASS 2 の UPSTREAM / DEPENDENCY-VERSION DRIFT レンズ。対象は `app/Cargo.toml`、
`app/Cargo.lock`、`app/build.rs`、`poc/Cargo.toml`、および
`~/.cargo/registry/src/` と `~/.cargo/registry/index/` のローカル crates.io データ。
外部へ未接続でも検証できるローカル一次情報のみを根拠とし、上流の変更内容が
ローカルソースで確認できない場合は「未確認」と明記した。

確認済みの中心事実:

- `gpui-kit` は `=0.9.1` の完全一致固定で、ローカルインデックスには `0.10.0` が存在する。
- `gpui-kit 0.9.1` は `gpui-pre` 系を `=0.3.12` で完全一致固定し、`0.3.13` が存在する。
- `trash 5.3.0` の CHANGELOG に Portal backend 追加が記載されているが、既定 feature では無効。
- `build.rs` の `cfg!(target_os = "linux")` はビルドスクリプトのホスト側評価である。

## 検出タスク一覧

### r2-up-01 gpui-kit の完全一致ピンが 0.10.0 をブロック

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r2-up-01 | `gpui-kit =0.9.1` の完全固定を `0.10.0` へ更新検討 | 残タスク | `app/Cargo.toml:14`; `poc/Cargo.toml:8`; `app/Cargo.lock:2119`; index `.cache/gp/ui/gpui-kit` | ローカルインデックスに 0.10.0 が存在。依存は gpui-base/gpui-component/assets が 0.10.0、gpui-fast が 0.1.16 へ上がる。0.10.0 の具体的なバグ修正内容は未確認。 | 高 | 中 | 中 | 高（版本存在）/ 未確認（修正内容） |

### r2-up-02 gpui-pre の推移的完全一致ピン

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r2-up-02 | `gpui-pre =0.3.12` の完全固定と 0.3.13 の差分調査 | 調査項目 | `gpui-kit-0.9.1/Cargo.toml:271`; `gpui-kit-0.9.1/Cargo.toml:326`; `gpui-kit-0.9.1/Cargo.toml:342`; index `.cache/gp/ui/gpui-pre` | gpui-kit 0.9.1 は gpui / gpui_platform / gpui_web を `=0.3.12` で固定。インデックスに 0.3.13 があるが、gpui-kit 0.10.0 でも gpui-pre 側は `=0.3.12` のまま。0.3.13 の修正内容は未確認。 | 中 | 中 | 中 | 高（ピンと版本存在） |

### r2-up-03 notify 9 系 RC と debouncer 0.8 RC の追跡

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r2-up-03 | `notify 9` / `notify-debouncer-full 0.8` の安定化を追跡し移行可否を評価 | 調査項目 | `app/Cargo.toml:16`; `app/Cargo.toml:17`; `app/Cargo.lock:3877`; `app/Cargo.lock:3896`; index `.cache/no/ti/notify` | 現行は 8.2.2 / 0.7.0（index の MSRV は 1.88 / 1.85）。インデックスに 9.0.0-rc.9 / 0.8.0-rc.4（MSRV 1.91）と `freebsd_inotify` 等の feature 差を確認。RC の安定版公開・API 互換性は未確認。 | 中 | 中 | 中 | 高（版本・feature 差）/ 未確認（安定化時期） |

### r2-up-04 trash 5.3.0 の Portal backend が未使用

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r2-up-04 | trash の `portal` feature を有効化し sandbox 環境のごみ箱を検証 | 機能追加案 | `trash-5.3.0/CHANGELOG.md` (5.3.0 / New Features: "add Portal backend"); `trash-5.3.0/Cargo.toml:65`; `trash-5.3.0/Cargo.toml:75`; `app/Cargo.toml:18` | CHANGELOG の 5.3.0 New Features に Portal backend 追加、Bug Fixes にパスデコード系修正を確認。既定 feature は `coinit_apartmentthreaded` + `chrono` + `freedesktop` で `portal`（zbus/rustix 依存）は無効。有効化による効果は未検証。 | 中 | 低 | 高 | 高（CHANGELOG と feature 定義） |

### r2-up-05 gpui-kit の未使用 feature

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r2-up-05 | `inspector` / `profiler` / `test-support` / `gpui-fast` feature の採用可否を評価 | 機能追加案 | `gpui-kit-0.9.1/Cargo.toml:39`; `gpui-kit-0.9.1/Cargo.toml:46`; `app/Cargo.toml:14` | 既定 feature は `component` + `assets` のみ。`inspector` と `profiler` は UI レイアウト/描画デバッグ、`test-support` はコンポーネントテスト、`gpui-fast` は別バックエンド選択を提供。バイナリサイズと能力のトレードオフを測定して決定する。 | 中 | 中 | 中 | 高（feature 定義） |

### r2-up-06 assets feature と 0.10.0 更新時のアイコン互換性

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r2-up-06 | gpui-kit-assets 0.10.0 更新時の `IconName` 互換性を検証 | 調査項目 | `gpui-kit-0.9.1/Cargo.toml:287`; `gpui-kit-0.9.1/src/lib.rs:172`; `app/src/icons.rs:8`; `app/src/main.rs:5611` | `assets` feature が gpui-kit-assets を再エクスポートし、アプリは `IconName` と `AllAssets` を直接使用。`assets` を外す選択肢はコード上使用不可。0.10.0 では assets crate も 0.10.0 へ上がるため、アイコン名の変更リスクを確認する。 | 中 | 低 | 中 | 高（使用箇所）/ 未確認（0.10.0 の差分） |

### r2-up-07 宣言 MSRV と実ツールチェーンのドリフト

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r2-up-07 | MSRV 1.92 の根拠を文書化し下限ビルドゲートを追加 | 構造改善 | `app/Cargo.toml:6`; `README.md:30`; index `.cache/no/ti/notify` (8.2.2: 1.88); index `.cache/tr/as/trash` (5.3.0: 1.87) | `rust-version = "1.92"` と README の 1.92+ に対し、実行環境の `rustc` は 1.100.0。直接依存の MSRV は 1.88 / 1.87 以下。`rust-toolchain*` ファイルはリポジトリに存在せず、1.92 での `cargo check --locked` 再現性は未確認。 | 中 | 低 | 中 | 高（宣言と Runtime Proof） |

### r2-up-08 edition 2021 と依存側 edition 2024 の乖離

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r2-up-08 | app / poc の edition 2024 移行計画を策定 | 構造改善 | `app/Cargo.toml:5`; `poc/Cargo.toml:5`; `gpui-kit-0.9.1/Cargo.toml:13`; `gpui-pre-0.3.12/Cargo.toml:13` | 両マニフェストは edition 2021 だが、gpui-kit と gpui-pre は edition 2024。現在ビルドは通っており即時障害ではないが、言語機能と lint 挙動の差が今後拡大する。移行時期と検証手順を決める。 | 低 | 中 | 低 | 高 |

### r2-up-09 build.rs の target_os 判定はホスト評価

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r2-up-09 | `CARGO_CFG_TARGET_OS` に基づく判定へ build script を修正 | 構造改善 | `app/build.rs:26`; `app/build.rs:27`; `app/build.rs:31`; `app/build.rs:43`; `app/build.rs:50` | `cfg!(target_os = "linux")` はビルドスクリプト自身のコンパイル時（ホスト）に評価され、クロスコンパイル時のターゲット OS を表さない。`-L native=` に `CARGO_MANIFEST_DIR` 由来の絶対パスを埋め込む点も再現性を下げる。シム生成は `scripts/dev-setup.sh` と暗黙結合。 | 中 | 低 | 高 | 高 |

### r2-up-10 dirs の重複バージョンと 7.0.0 の存在

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r2-up-10 | `dirs` 5.0.1 / 6.0.0 の混在を解消し 7.0.0 を調査 | 調査項目 | `app/Cargo.lock:1221`; `app/Cargo.lock:1230`; `app/Cargo.lock:2107`; `app/Cargo.lock:8377`; index `.cache/di/rs/dirs` | lock には直接依存の 6.0.0 と zed-font-kit 経由の 5.0.1 が共存。インデックスには 7.0.0 が存在するが、API 互換性と推移的解決への影響は未確認。 | 低 | 中 | 中 | 高（重複と版本存在） |

### r2-up-11 app / poc の gpui-kit 二重ピン

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r2-up-11 | 2 マニフェストの gpui-kit バージョン一致を検証するゲートを追加 | 残タスク | `app/Cargo.toml:14`; `poc/Cargo.toml:8`; `README.md:41`; `.github/workflows/ci.yml` (extra_gates) | ルート `Cargo.toml` がなく、`=0.9.1` が app と poc で個別管理。CI は `extra_gates` で poc も検証するが、両者のバージョン文字列が一致するかの自動検証は見当たらない。片方だけ更新されるドリフトを防ぐ。 | 低 | 低 | 中 | 高（両マニフェストと CI 定義） |

## レンズ総評

依存状態は「動くが更新が止まっている」典型例である。最大のリスクは
`gpui-kit = "=0.9.1"` と、それが推移的に課す `gpui-pre = "=0.3.12"` の二段の
完全一致固定で、上流のバグ修正を取り込む経路が事実上閉じている（r2-up-01/02）。
次に、現行バージョン内で既に利用可能になった能力の取りこぼし
（trash の Portal backend: r2-up-04、gpui-kit の feature: r2-up-05）が挙がる。
これらは新規依存なしで試せるため費用対効果が高い。

notify 9 系と dirs 7 はローカルインデックス上で存在を確認できたが、
API 互換性・安定版公開時期は未確認として扱い、移行ではなく調査タスクに留めた
（r2-up-03/10）。MSRV と edition の乖離（r2-up-07/08）、build script の
ホスト/ターゲット混同（r2-up-09）は、今後のクロス環境ビルドと CI 再現性に
効く構造改善である。外部検索は未使用のため、上流の変更内容に関する客観的
情報は crates.io インデックスキャッシュと同梱 CHANGELOG の範囲に限定される。
