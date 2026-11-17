---
type: Reference
title: 残タスク発掘 PASS 4 統合サマリ（FS 意味論・CLI・設定・ライフサイクル・ライセンス）
description: >-
  PASS 1〜3 で扱っていなかったファイルシステム意味論・コマンドライン・
  設定到達性・アプリライフサイクル・ライセンス運用の 5 レンズによる第 4 パスの統合結果。
tags:
  - task-discovery
  - agent-team
  - round-4
  - gpui-file-explorer
generated: 2026-11-17T21:22:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 実行結果

| レンズ | 成果物 | 件数 | エージェント状態 |
|---|---|---|---|
| ファイルシステム意味論 | [fs-semantics.md](fs-semantics.md) | 11 | 完了（ID は `r4-fs-<slug>` 形式） |
| CLI／自動化 | [cli.md](cli.md) | 12 | 完了 |
| 設定の表面 | [config-surface.md](config-surface.md) | 11 | 完了 |
| ライフサイクル | [lifecycle.md](lifecycle.md) | 10 | 完了 |
| ライセンス／コンプライアンス | [license.md](license.md) | 11 | 初回失敗 → `r4-license2` で完了 |
| **合計** | | **55** | |

## 今回の特徴

- PASS 1〜3 が「機能・コード・証跡」を中心にしていたのに対し、PASS 4 は
  **「運用と周辺契約」** を狙った。結果、既存レビューや README が
  「知っているが書いていない」状態の項目が多数見つかった。
- 低コスト・高確度の具体的不整合:
  - `r4-cfg-03`: 検索履歴は 10 件保存・README も 10 件と記載するが、
    UI は `main.rs:3992` の `.take(8)` で 8 件しか表示しない（親が確認）。
  - `r4-life-01/02`: DESIGN.md:840 は終了時保存を要求するが
    `on_window_closed`（`main.rs:5619-5624`）は `cx.quit()` のみ。
    Ctrl+Q（`:1585`）はフック自体を迂回（親が確認）。
  - `r4-cli-04`: `.desktop` は `%f` でファイルを渡すが
    `restore_session` が `is_dir()` で除外するため、ファイル引数が黙って捨てられる。
- 新たなリスク領域:
  - `r4-lic-09`: `cargo metadata` の実出力に DESIGN.md:925 の想定を超える
    `MPL-2.0` / `Unicode-3.0` / `NCSA` / `BSL-1.0` / GPL, LGPL 選択肢が存在。
    親も同一コマンドで再確認済み。法的判断はせず調査項目として扱う。
  - `r4-fs-*`: ハードリンク・xattr/ACL・sparse file・特殊ファイル・
    mtime 保持など、`std::fs` 既定に依存する意味論の未整備。

## 親による事実確認（[VALIDATION.md](VALIDATION.md)）

- 採用: `r4-cfg-03`、`r4-life-01/02`、`r4-lic-09/02`、PASS 2 の `r2-sec-04` 再確認。
- **否定（却下）**: 親の事前調査で「README のディレクトリ引数が実装と乖離している」
  疑いを持ったが、`main.rs:5537` / `startup_dir()`（`:5560`）で
  `std::env::args()` を正しく処理しており誤りだった。
  CLI レンズも同様に確認しており、`r4-cli-02` は「実装済みだが未文書化」としている。
- `r4-fs` レンズは指定の連番 ID ではなく slug 形式（`r4-fs-hardlink` 等）で
  出力したが、一意性と 9 列形式は満たすため有効と判定。

## 累計

PASS 1 = 73、PASS 2 = 62、PASS 3 = 55、PASS 4 = 55、**合計 245 件**（生 finding）。
残り PASS 5 の後、PASS 6 で重複排除・妥当性吸収を行う。
