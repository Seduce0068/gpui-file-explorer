---
type: Reference
title: PASS 1〜5 の finding 重複インデックス（PASS 6 統合の下地）
description: >-
  5 パスで発掘した生 finding をテーマ別に束ねた作業索引。
  最終判定は PASS 6 で行うため、ここでは「同じテーマの候補を磁力で寄せる」のみを行う。
tags:
  - task-discovery
  - dedup
  - pass-6-input
generated: 2026-11-17T21:30:00+09:00
status: in-progress
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 使い方

PASS 6 では各クラスタを 1 つの統合項目にまとめ、
`採用 / 却下 / 未確認 / 既知(優先度低)` のいずれかを付与する。
同じ finding が複数クラスタに跨る場合は、主担当クラスタにのみ計上する。

## クラスタ一覧

### A. OS ファイル連携・クリップボード
`r1-gap-01` `r1-plat-08` `r3-osint-03` `r1-gap-03` `r1-code-11`
`r3-osint-01` `r3-osint-05` `r2-cmp-02` `r4-cli-10`

### B. 監視・リコンサイル
`r1-gap-10` `r1-qual-05` `r1-qual-06` `r1-code-15` `r1-qual-09`
`r3-state-05` `r3-state-06` `r3-state-07` `r3-state-10` `r3-async-01`
`r3-input-03` `r3-osint-06`

### C. 性能・スケール
`r1-gap-08` `r1-qual-01` `r3-proof-08` `r1-code-12` `r1-qual-11`
`r3-async-02` `r4-fs-sparse`

### D. 設定・永続化
`r1-code-07` `r1-code-08` `r1-gap-06` `r1-qual-07` `r2-sec-04`
`r2-sec-12` `r3-osint-09` `r4-cfg-01`〜`r4-cfg-11` `r4-life-01` `r4-life-02`

### E. 検索
`r1-code-13` `r1-gap-02` `r2-cmp-03` `r2-scn-09` `r4-cfg-05` `r4-cfg-06`
`r3-input-01` `r5-search-*`（PASS 5 の成果を統合時に追加）

### F. アンドゥ・履歴・タブ復元
`r1-code-02` `r1-code-09` `r1-ux-16` `r2-cmp-09` `r2-scn-08`
`r3-state-01` `r3-state-02` `r3-state-03` `r3-state-04` `r3-state-08`
`r3-state-09` `r3-async-05` `r4-cfg-08`

### G. ゴミ箱
`r1-gap-05` `r1-ux-14` `r2-sec-03` `r3-async-04` `r5-trash-*`

### H. プレビュー・サムネイル・アイコン
`r1-gap-07` `r1-code-14` `r2-sec-10` `r2-cmp-10` `r2-cmp-11`
`r3-osint-04` `r3-osint-10` `r4-cfg-10` `r4-cli-04` `r5-render-*`

### I. クロスプラットフォーム・配布・供給網
`r1-plat-01`〜`r1-plat-15` `r1-qual-14` `r2-up-01`〜`r2-up-11`
`r3-osint-07` `r3-osint-08` `r4-lic-01`〜`r4-lic-11`

### J. ドキュメント・証跡・検証可能性
`r1-code-01` `r1-code-03` `r4-cli-01` `r4-cli-02` `r4-life-05`
`r3-proof-01`〜`r3-proof-11` `r4-lic-02` `r4-lic-05` `r4-lic-07`

### K. 機能追加（UX 列挙）
`r1-ux-01`〜`r1-ux-17`（うち既知: 14/15/16）
`r2-cmp-01`〜`r2-cmp-15` `r2-scn-01`〜`r2-scn-12`

### L. 入力・操作モデル
`r3-input-01`〜`r3-input-12` `r3-state-11` `r3-async-06`〜`r3-async-10`

### M. ファイル操作の安全性・FS 意味論
`r1-code-05` `r1-code-06` `r1-qual-03` `r1-qual-04` `r1-qual-10` `r1-qual-12`
`r2-sec-01`〜`r2-sec-12` `r4-fs-*`（11 件） `r4-life-03` `r5-error-*`

### N. ライフサイクル・CLI
`r4-cli-03`〜`r4-cli-12` `r4-life-04` `r4-life-06`〜`r4-life-10`

### O. a11y / i18n
`r2-a11y-01`〜`r2-a11y-12`（うち `r2-a11y-09` は親が却下済み）

## 親が既に判定済みの項目（PASS 6 で再評価不要）

| finding | 判定 | 根拠 |
|---|---|---|
| `r2-a11y-09` | **却下** | テキストラベルが併記されており「色のみ」ではない（`round-2/VALIDATION.md`） |
| `r1-code-10` | 数値訂正済み | ALLOWED_FS は 22 件ではなく 20 件（`round-1/code.md` を修正済み） |
| 「README の引数仕様が乖離」疑い | **却下** | `main.rs:5537` / `:5560` で正しく処理（`round-4/VALIDATION.md`） |
| `r4-cfg-02` | 既知・低優先 | `app/README.md:366-367` に明記済み |
| `r5-err-06` = `r3-state-03` | **重複（PASS 5 で判明）** | どちらも「undo の事前チェック失敗で `pop_undo()` 後に `push_undo` が無い」経路。親は `round-3/VALIDATION.md` で既に確認済み。PASS 6 では `r3-state-03` を主担当とし `r5-err-06` は取り込まない。 |
