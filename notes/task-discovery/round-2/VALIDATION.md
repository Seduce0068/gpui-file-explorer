---
type: Reference
title: PASS 2 の親による事実確認と却下記録
description: >-
  第 2 パスのエージェント出力のうち、外部事実・高影響・過大主張の疑いがあるものを
  親が現行コード・ローカル一次情報で検証した記録。
tags:
  - task-discovery
  - validation
  - runtime-proof
  - round-2
generated: 2026-11-17T20:20:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 採用（事実確認）

| ID | 主張 | 確認方法 | 結果 |
|---|---|---|---|
| r2-up-01 | ローカル crates.io インデックスに `gpui-kit 0.10.0` が存在 | `strings ~/.cargo/registry/index/*/.cache/gp/ui/gpui-kit \| grep '"vers"'` | **確認**。0.9.1 と 0.10.0 の両方が存在。修正内容自体は未確認の旨も妥当。 |
| r2-up-03 | `notify 9.0.0-rc.9` がインデックスに存在 | 同上（`.cache/no/ti/notify`） | **確認**。rc.2〜rc.9 が存在。安定版公開時期は未確認の旨も妥当。 |
| r2-up-04 | trash 5.3.0 の CHANGELOG に Portal backend 追加 | `grep -in portal trash-5.3.0/CHANGELOG.md` | **確認**。27 行目に "add Portal backend"。 |
| r2-up-07 | 実行環境の rustc が 1.100.0、宣言 MSRV は 1.92 | `rustc --version` / `app/Cargo.toml:6` | **確認**。乖離は実在。 |
| r2-up-09 | `build.rs` が `cfg!(target_os = "linux")` でホスト評価している | `sed -n '20,55p' app/build.rs` | **確認**。27 行目に完全一致。絶対パスの `-L native=` も 50 行目に存在。 |
| r2-a11y-02 / 09 の設計根拠 | DESIGN.md:848 が「フォーカス=枠線」「色だけで状態を伝えない」を要求 | `sed -n '843,865p' DESIGN.md` | **確認**。設計要件は実在。フォーカスが背景色差分 (`main.rs:4421-4427`) である点も確認。 |

## 却下（子の主張が事実と異なる）

| ID | 主張 | 却下根拠 |
|---|---|---|
| r2-a11y-09 | 「エラー=赤・読込=黄・正常=緑・監視異常=黄と**色のみ**で状態を表現」 | `main.rs:5065-5079` は `エラー: {error}` / `読み込み中…` / ステータス文本を、`5156-5179` は `監視: 一部無効 — {reason}` / `監視: 無効 — {reason}` / `ゴミ箱（自動更新なし）` をそれぞれ子要素として描画しており、**テキストが併記されている**。DESIGN.md:848 の「色だけで状態を伝えない（アイコン/文字も併用）」は満たされている。よって「未達」という主張は却下。コントラスト比の定量的検証は独立した調査項目として残す余地があるが、それは本 finding の主張ではない。 |

## 補足

- 却下した r2-a11y-09 には `a11y.md` 本文に直接「親による事実確認（却下）」の注記を插入済みで、
  第 6 パスが誤って採用しないようにした。
- 本確認は読み取りのみで、ソースコードは変更していない。
