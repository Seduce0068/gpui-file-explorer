---
type: Reference
title: PASS 4 の親による事実確認（随時追記）
description: >-
  第 4 パスのエージェント出力のうち、親が現行コード・ドキュメントで直接確認した
  主張の記録。
tags:
  - task-discovery
  - validation
  - runtime-proof
  - round-4
generated: 2026-11-17T21:10:00+09:00
status: in-progress
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 確認済み（採用）

| ID | 主張 | 確認方法 | 結果 |
|---|---|---|---|
| r4-cfg-03 | 検索履歴は 10 件保存されるが UI は 8 件しか表示しない | `sed -n '140,148p' app/src/settings.rs`, `sed -n '3988,3996p' app/src/main.rs`, `grep -n 検索履歴 app/README.md` | **事実確認**。`MAX_RECENT_SEARCHES=10` で truncate し、README:198/425 も「最大 10 件」と記載する一方、`main.rs:3992` は `.iter().take(8)` で描画。低コストの不整合。 |
| r4-cfg-02 | `sort.dirs_first` が保存されるだけで動作に反映されない | `app/README.md:366-367`（既知の制限に明記） | **確認だが novelty=既知**。ドキュメント上「将来互換のため保存、`false` は無視」と明記済み。第 6 パスでは「既知・低優先」として扱う。 |
| r4-cli（親の事前確認） | README が乖離した引数仕様を載せているのではないかという疑い | `grep -n env::args app/src/main.rs`, `sed -n '5537,5565p'` | **否定**。最初の grep は `head -20` で切れていただけで、`main.rs:5537` と `startup_dir()`（`:5560`）で `std::env::args()` を処理している。README の「引数で開始ディレクトリを指定」は正しい。子の出力を待つが、この疑いに基づく finding は採用しない。 |
| r4-life-01 / 02 | ウィンドウ終了時に `settings.save()` が呼ばれず、Ctrl+Q がフックを迂回する | `sed -n '5614,5645p;1580,1590p' app/src/main.rs`, `DESIGN.md:840`, `grep -n '\.save()'` | **事実確認**。`on_window_closed` は `cx.windows().is_empty()` で `cx.quit()` するだけで save しない。DESIGN.md:840 は保存タイミングに「ウィンドウ終了時」を明示。Ctrl+Q（1585）は `cx.quit()` を直接呼び、`on_window_closed` を通らない。なお `settings.save()` は他 10 箇所以上（638/908/1103/…）で呼ばれており、即時保存と終了時保存の併用が設計意図と思われる。 |
| r4-lic-09 / 02 | 推移的依存に DESIGN 想定外のライセンス expression が含まれる／NOTICE に著作権者が無い | `cargo metadata --offline --format-version 1 \| jq` で distinct license と MPL-2.0 パッケージを列挙、`grep -rnE 'Copyright\|©\|authors' NOTICE.md app/NOTICE.md ...` | **事実確認**。distinct license に `MPL-2.0`（cbindgen / dwrote / option-ext）、`Unicode-3.0`、`0BSD`、`BSL-1.0`、`NCSA`、`Apache-2.0 OR GPL-2.0-only`、`MIT OR Apache-2.0 OR LGPL-2.1-or-later` 等が存在。NOTICE 類に Copyright / © / authors の一致なし。子エージェントは法的判断を避け「調査項目」としており、その姿勢も妥当。 |

## 重複の兆し（第 6 パスで吸収）

- `r4-cfg-08`（タブ単位設定の欠如）は PASS 1 の `r1-ux-16`（タブ完全セッション復元）と
  関連するが、前者は「設定のスコープ」、後者は「履歴・選択の復元」で別軸。
- `r4-cfg-11` の時間定数（debounce / reconcile）は PASS 1 の
  `r1-code-12`（進捗スロットル）や `r1-qual-11` と同じ「ハードコード定数」群として
  まとめられるかどうかを第 6 パスで判断する。
