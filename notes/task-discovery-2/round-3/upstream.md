---
type: Reference
title: Round 3 — 上流 / 依存 / ライセンス / ビルド再現性 レビュー
description: 読み取り専用エージェント r3-upstream の所見（15 件・ローカル情報のみ、外部検索なし）。
tags: [task-discovery, cross-review, upstream, license, gpui-file-explorer]
generated: 2026-11-17T23:58:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-upstream-01 | DEB リリースビルドが `--locked` を通さず依存が再解決されうる | 残タスク | `packaging/linux/build-deb.sh:108`; `scripts/check.sh:66-73`; `ci.yml:39-43`; `Cargo.toml:15-27` | check.sh と CI の poc ゲートは --locked を付けるが build-deb.sh の cargo build --release は無し。キャレート範囲のため配布バイナリだけ版が漂移しうる。 | 再現性 | S | 新規 | 高 |
| r3-upstream-02 | CI が外部再利用ワークフローでツールチェーン固定をローカル確認できない | 調査項目 | `ci.yml:24-43`; `Cargo.toml:6` | `uses: Seduce0068/workflows/.../rust-gates.yml@v1` に Rust バージョン/--locked 指定が無く rust-toolchain* も無い。v1 は差し替え可能なタグ。**提供側 yml の toolchain 入力と cargo コマンド列を確認**。 | CI 信頼性 | S | 既知(ドキュメント記載あり) | 低 |
| r3-upstream-03 | 宣言 MSRV 1.92 と実環境 1.99.1 の乖離・下限ビルド未検証 | 残タスク | `Cargo.toml:6`; `README.md:30`; `notes/ecosystem-notes.md:226` | oo7 0.6.0 が 1.92+ を要求するのが実質下限。1.92 での cargo check --locked は未実施。**Rust 1.92 でのビルド確認**。 | 互換性 | S | round1/2と重複(r2-up-07) | 中 |
| r3-upstream-04 | 再公開クレート gpui-pre-reqwest が依存グラフに混入 | 調査項目 | `Cargo.lock:2141,2441-2444`; `RESEARCH.md:669-679` | reqwest 0.12.15 の別名再公開 crate がロックに存在。上流アドバイザリ監視で名前が一致せず deny.toml/audit.toml も無い。元 crate との差分と owner 状態を確認。 | サプライチェーン | M | round1/2と重複(r1-plat-14) | 中 |
| r3-upstream-05 | デスクトップ専用ビルドに wasm 系クレートと nightly 前提 feature が潜在 | 調査項目 | `Cargo.lock:2130,2503,2566,7056-7058`; `notes/wasm-browser-notes.md:28-38` | gpui-kit が gpui-pre-web を無条件依存、multithreaded 既定が wasm_thread 0.3.3（#![feature]）をロック。実リンク対象とバイナリサイズ影響を cargo tree で確認。 | 依存面の整理 | M | 新規 | 中 |
| r3-upstream-06 | trash 5.3.0 の Portal backend 未使用で sandbox 環境のゴミ箱が未検証 | 機能追加案 | `Cargo.toml:18`; `RESEARCH.md:477,484` | 既定 feature は freedesktop 系で portal(zbus/rustix) は無効。Flatpak 等でゴミ箱が動かない可能性。本番配布は DEB のみ。 | 配布先の拡張 | S | round1/2と重複(r2-up-04) | 中 |
| r3-upstream-07 | notify のネットワーク FS／Windows バッファ溢れに対する代替監視が未実装 | 残タスク | `RESEARCH.md:455-459`; `app/README.md:145-148,415-417` | 10秒 reconcile と抑制 1.5秒はあるが推奨の PollWatcher フォールバックと「自動更新オフ」表示が無い。Windows ERROR_NOTIFY_ENUM_DIR も未確認。 | 監視の堅牢性 | M | 既知(ドキュメント記載あり) | 高 |
| r3-upstream-08 | chrono::Local が wasm 非対応でコード側に cfg／抽象化が無い | 残タスク | `app/README.md:468-471`; `notes/wasm-browser-notes.md:49-50` | 日時表示とバックアップ名に chrono を直使い。README 自身が js-sys 差し替え必要性を明記するが対策なし。 | 移植性 | M | round1/2と重複(r1-port-15) | 高 |
| r3-upstream-09 | dirs が 5.0.1/6.0.0 併存し設計書・NOTICE の記述も 7.0.0 と不一致 | 調査項目 | `Cargo.lock:1221-1236`; `Cargo.toml:21`; `DESIGN.md:923` | 直接 6.0.0 と推移 5.0.1（zed-font-kit 経由）が混在。設計書想定 7.0.0 と実装・通知が食い違う。 | 記述の整合 | S | round1/2と重複(r2-up-10) | 高 |
| r3-upstream-10 | cargo audit が不完全なまま再実行されず cargo-deny/vet のゲートも無い | 残タスク | `RESEARCH.md:678`; `DESIGN.md:925`; deny.toml/audit.toml 未検出 | 先行監査は脆弱性0件だが未整備8件＋ネットワーク失敗367件で不完全と明記。DESIGN は cargo deny と audit を求めるが設定も自動化も無い。 | サプライチェーン | M | 既知だが未対応 | 中 |
| r3-upstream-11 | 第三者ライセンス本文（Apache-2.0 / Lucide ISC）がリポジトリと DEB に無い | 残タスク | `DESIGN.md:915-917`; `app/NOTICE.md:27-32`; `build-deb.sh:134-138` | NOTICE は帰属と URL のみで全文を同梱せず、DEB の copyright は AGPL 全文のみ。法的十分性の確認が必要。 | ライセンス遵守 | S | 既知だが未対応(r4-lic-08) | 中 |
| r3-upstream-12 | DEB control に License フィールドが無く cross-platform 表記と検証範囲が不一致 | 調査項目 | `build-deb.sh:172-187`; `DEBIAN/control:8-14`; `README.md:84-87` | control は Version/Homepage/Description を生成するが License が無く copyright も DEP-5 ヘッダなし。Description は cross-platform をうたうが Windows/macOS 未検証。 | 配布メタデータ | S | 既知だが未対応(r4-lic-06) | 低 |
| r3-upstream-13 | poc/Cargo.toml に license/description/repository メタデータが無い | 残タスク | `poc/Cargo.toml:2-8`; `app/Cargo.toml:2-11` | app は license/description/repository/homepage/readme を揃えるが poc は name/version/edition のみ。SPDX ヘッダはあるが機械可読でない。 | メタデータ一貫性 | S | 新規 | 高 |
| r3-upstream-14 | wasm デモ実装なら AGPL §13 のソース提供導線が未設計 | 調査項目 | `LICENSE:540-551`; `DESIGN.md:888-893`; `notes/wasm-browser-notes.md:83-89` | 現状はデスクトップ専用で network 利用は無いが、Web デモを作るとリモート利用者への Corresponding Source 提供が必要。ブラウザ対応前にライセンス設計を先決めすべき。 | 法的準備 | M | 新規 | 中 |
| r3-upstream-15 | scripts/check.sh が audit 未導入時に成功表示し poc 依存は audit 対象外 | 構造改善 | `scripts/check.sh:90-99`; `ci.yml:35,39-43`。**親が確認済み** | command -v cargo-audit 失敗時に1行注意で :99 の All checks passed. に到達し exit 0。CI extra_gates は poc の fmt/check/test/clippy のみ。 | ゲートの信頼性 | S | round1/2と重複(r1-verify-02) | 高 |

## レンズ要約

依存は gpui-kit = "=0.9.1" の完全ピン＋両 Cargo.lock コミットで「動くが更新停止」状態。上流修正の取り込み経路が前回指摘のまま閉じている。今回の固有の追加点は配布スクリプトだけが --locked を外す再現性の穴、外部ワークフローのブラックボックス化、wasm/wasm_thread の潜在依存、poc メタデータ、WASM 化時の AGPL §13。法的判断が必要な項目はすべて「要確認」として留めた。
