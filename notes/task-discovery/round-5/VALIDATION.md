---
type: Reference
title: PASS 5 の親による事実確認
description: >-
  最終発掘パスのエージェント出力のうち、親が現行コード・ローカル依存ソースで
  直接確認した主張の記録。
tags:
  - task-discovery
  - validation
  - runtime-proof
  - round-5
generated: 2026-11-17T21:40:00+09:00
status: in-progress
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 確認済み（採用）

| ID | 主張 | 確認方法 | 結果 |
|---|---|---|---|
| r5-trash-10 | macOS で `trash::os_limited` が存在せずビルド不能 | `trash-5.3.0/src/lib.rs:426-429` の cfg と `app/src/trash_view.rs:19,26,49` を直接 read | **事実確認（高影響）**。cfg は `target_os = "windows"` または `all(unix, not(target_os = "macos"), not(ios), not(android), feature = "freedesktop")` に限定され、macOS は明示的に除外。一方 `trash_view.rs` は `trash::os_limited::list()` / `restore_all()` を無条件に呼ぶ。macOS ビルドはコード上必定で失敗する。**README.md:84-86 の「macOS は上流 gpui-pre-apple の制約で check 不可」という説明では理由が不充分で、アプリ側の直接原因もある**。PASS 6 では最優先の「クロスプラットフォーム宣言の是正」候補。 |
| r3-state-05（PASS 3 の再確認） | 現行ディレクトリ削除が監視で再読込に反映されない | `main.rs:996-1006`, `DESIGN.md:454` | **事実確認**。`FsEvent::Removed` 分岐には親判定が無く `retain()` のみ。DESIGN.md:454 は「親由来は再読込トリガ」と規定。 |
| r5-err-06 = r3-state-03 | undo 事前チェック失敗で履歴項目が消える | `round-3/VALIDATION.md` の既有確認 | **重複**。親は既に `main.rs:2296-2322` で確認済み。PASS 6 では `r3-state-03` を主担当とし、r5-err-06 は吸収する。 |
| r5-search-03（予備確認） | 拡張子フィルタが `ends_with(".ext")` で model と意味が異なる | `app/src/search.rs` と `app/src/model.rs` の extension 処理を grep | **確認**（PASS 6 で詳細化）。名前に `.` が無い隠しファイルや複数ドットの扱いが食い違う経路が存在。 |
| r5-render-01 | サイドバー幅に 214 / 190 / 200 の三重ドリフト | `grep -n SIDEBAR_WIDTH_PX app/src/main.rs`, `sed -n '4666,4671p;5425,5431p'`, `DESIGN.md:134` | **事実確認**。定数 214（main.rs:83）、実描画 190（:4669）、設計 200（DESIGN.md:134）、グリッド列数計算は 214 を控除（:5428）。新規コミット `2e230a6` で定数を names 化した際に値の摺り合わせが欠けた可能性が高い。低コスト修正候補。 |

## 現時点の累計

PASS 1 = 73、PASS 2 = 62、PASS 3 = 55、PASS 4 = 55、
PASS 5 = 少なくとも 46（search 12 / model 12 / trash 12 / error 10、render は執筆中）。
**合計 292 件以上**（生 finding）。PASS 6 で重複吸収を行う。
