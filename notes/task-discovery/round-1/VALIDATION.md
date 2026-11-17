---
type: Reference
title: PASS 1 高影響指摘の親による事実確認
description: >-
  エージェントが挙げた PASS 1 の指摘のうち、影響が大きいもの・確度が中程度のものを
  親エージェントが現行コードで直接確認した Runtime Proof の記録。
tags:
  - task-discovery
  - validation
  - runtime-proof
  - round-1
generated: 2026-11-17T20:05:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 確認結果一覧

| ID | 主張 | 確認した行 | 判定 |
|---|---|---|---|
| r1-qual-09 | reconcile 失敗 status が後続 watcher イベントで上書きされる | `main.rs:934-947`, `986-990`, `1024-1036` | **事実確認**。失敗時に `this.status = "再スキャンに失敗しました: …"` を設定するが、`FsEvent::Rescan/Renamed` を処理する分岐が無条件に `STATUS_CHANGED` を代入する。妥当性「高」。 |
| r1-code-15 | DirWatcher の Drop がスレッド終了を待たない | `watcher.rs:159-166` | **事実確認**。`impl Drop` の本体はコメントのみで明示的な join/停止呼び出しなし。ただし實害（競合）は未観測のため、指摘は「調査項目」として妥当。 |
| r1-code-06 | コピーの `sync_all` 失敗が握りつぶされる | `ops.rs:313-315` | **事実確認**。`let _ = output.sync_all();` と best-effort コメントあり。設定保存 (`settings.rs`) は `sync_all()?` の方針と異なる点も確認。 |
| r1-plat-03 / r1-code-05 | 非 Unix の symlink コピーが Unsupported | `ops.rs:375-391` | **事実確認**。`#[cfg(not(unix))]` で `OpError::Unsupported` を返す。Windows で symlink を含むツリー全体が失敗する経路は実在。 |
| r1-qual-01 | watcher 1 イベントあたり O(n) 走査が複数ある | `main.rs:997-1005`, `1013-1016`, `1032-1034` | **事実確認**。`all_entries.retain()`、`iter_mut().find()`、`fingerprint(&all_entries)` がイベント処理内で全件走査。10 万件想定での O(n) 呼び出しは実在（実測自体は未実施のまま）。 |
| r1-qual-12 | case-only rename の中継名に O_EXCL 予約が無い | `ops.rs:674-685` | **事実確認**。中継名は PID + SystemTime で予測不能化済みだが、`fs::rename(src, &temp)` は宛先を無条件置換。`create_new` 予約はコピー経路 (`ops.rs:331-335`) のみ。硬化候補として妥当。 |

## 補足

- 上記 6 件はいずれも子エージェントの記載どおりであり、却下した指摘はない。
- `ALLOWED_FS` 件数（`r1-code-10`）のみ子の記載が 22 件で誤りで、親が
  `mounts.rs:28-50` を実数えして 20 件に訂正済み（`code.md` を修正）。
- 本確認は読み取りのみで、ソースコードは変更していない。
