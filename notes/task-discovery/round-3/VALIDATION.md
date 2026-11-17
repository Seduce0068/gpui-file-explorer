---
type: Reference
title: PASS 3 の親による事実確認（随時追記）
description: >-
  第 3 パスのエージェント出力のうち、親が現行コードで直接確認した主張の記録。
tags:
  - task-discovery
  - validation
  - runtime-proof
  - round-3
generated: 2026-11-17T20:45:00+09:00
status: in-progress
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 確認済み（採用）

| ID | 主張 | 確認方法 | 結果 |
|---|---|---|---|
| r3-input-01 | 検索ビューで Enter が選択結果を開かない | `sed -n '1520,1540p;1605,1625p' app/src/main.rs`、`app/README.md:100` | **事実確認**。`main.rs:1526-1531` は `ViewSource::Search` のキー入力を Escape 以外すべて early-return しており、`1611` の `"enter" => self.open_selected(cx)` に到達しない。README:100 の「Enter=選択項目を開く」と矛盾する。**機能バグとして第 6 パスで高優先度に格上げする**。なお `open_selected` の呼び出し箇所は `1611` と `1541`（Ctrl+I のプロパティ）のみで、検索結果を開く別経路（ダブルクリック等）は確認できていない。 |
| r2-sec-04（PASS 2 の再確認） | 設定 JSON の配列が読み込み時に上限適用されない | `sed -n '160,235p' app/src/settings.rs`、`grep -n MAX_ settings.rs` | **事実確認**。`load_from_with_backup` は `migrate()` と `validate()`（后者は `last_dir` のみ検査）を呼ぶだけで、`MAX_RECENT_DIRS` / `MAX_BOOKMARKS` / `MAX_RECENT_SEARCHES` の truncate はミューテータ（`113/132/146` 行）にしか無い。 |

| r3-proof-01 | `cargo test` 件数の記述がドキュメント間で食い違う | `grep -rnoE '[0-9]+ (tests\|passed\|件)'` と `grep -r '#\[test\]' app/src \| wc -l` | **事実確認**。README:26/52/135 と app/README:27/288/301 は 167、verify-report は 35/36、review-report は 164+3=167/168 と分布。静的な `#[test]` 出現数は **174**（ただし cfg 条件等で実実行数と一致しない可能性があり、実 `cargo test` 再実行は未実施＝ここでは件数の「真実源不在」のみを確認）。 |
| r3-proof-03 | `check.sh` の exit 0 が cargo audit 実施を証明しない | `grep -nE 'audit\|FAST\|exit' scripts/check.sh` | **事実確認**。`--fast` は `run_audit=false`（32-33 行）、`90-96` 行は cargo-audit 不在時に「skipping the advisory check」と表示して続行する。よって exit 0 は audit 実施の証明にならない。 |
| r3-state-03 | rename の undo が失敗すると undo スタックから消える | `sed -n '2286,2330p;2340,2390p' app/src/main.rs` | **事実確認**。`UndoAction::Rename` 分岐の `!current.exists()` / `target.exists()` は `pop_undo()` 後に早期 return し `push_undo` を呼ばない。一方、非同期の `Err(error)` 経路（2346-2347 行）は `this.undo.push_undo(action_for_redo)` で戻している。前提条件失敗時にのみ undo 項目が失われる経路は実在。 |

## 重複の兆し（第 6 パスで吸収）

- `r3-async-02`（コピー前 `total_bytes` に CancelFlag が伝播しない）は
  PASS 1 の `r1-qual-10`（巨大ツリーの事前走査を Esc で止められない）と同一根因。
- `r3-osint-01`（FileManager1 フォールバック）は PASS 1 の
  `r1-code-11` / `r1-gap-03`（open/reveal 失敗検知）と関連するが、提案する
  解決策（D-Bus 経由の reveal）が異なるため、別項目として残すかを第 6 パスで判断する。
- `r3-input-08`（ホバー・省略名ツールチップ）は DESIGN.md:140 の既知 P1、
  `r3-input-09`（中クリック）は DESIGN.md:265 の既知項目で、いずれも novelty=既知が妥当。
