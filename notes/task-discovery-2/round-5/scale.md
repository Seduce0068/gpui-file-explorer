---
type: Reference
title: Round 5 — スケール / パフォーマンス レビュー
description: 読み取り専用エージェント r5-scale の所見（15 件・ベンチマーク実行なしの静的解析）。
tags: [task-discovery, cross-review, scale, performance, gpui-file-explorer]
generated: 2026-11-18T00:24:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-scale-01 | 選択 retain が O(選択数×件数) の二次走査 | 構造改善 | `model.rs:268-270`; `main.rs:760`, `:829`, `:1023`, `:3869` | entries.contains(path) を選択件数分線形探索し path_set を全件 clone 再構築。全選択10万件で n²。HashSet membership 化で S 修正可。 | 大規模選択の応答性 | S | 新規 | 高 |
| r5-scale-02 | 可視行ごとに選択パス全体を clone | 構造改善 | `main.rs:4485-4490`, `:4629-4636` | list_row/grid_cell が選択中行で selection.paths().to_vec() を毎フレーム。全選択10万件・可視40行で約400万 PathBuf clone/フレーム。 | フレーム時間 | S | 前回と重複(R6-B-13) | 高 |
| r5-scale-03 | 矢印キー/修飾クリックが毎回全パスを clone | 残タスク | `main.rs:1465-1469`, `:1494-1498`, `:1503-1506`; `model.rs:245-250` | select_with_modifiers/select_all が全件 Vec を生成。矢印1押下で10万 PathBuf 確保。 | キー操作の応答性 | S | 新規 | 高 |
| r5-scale-04 | 検索フィルタがキー入力ごとに O(n) 文字列確保 | 残タスク | `search.rs:46-74`; `main.rs:3688`, `:3703`, `:4025`, `:3862` | 各 entry で to_lowercase が2回、format! がループ内、全件 clone().collect()。sort_key 未利用。 | フィルタ入力の応答性 | S | 新規 | 高 |
| r5-scale-05 | Kind ソート比較が比較ごとに String を確保 | 残タスク | `model.rs:410-417`, `:437-443`, `:470-485` | kind_sort_key の to_lowercase が比較関数内で毎回。10万件で約170万回の一時 String。 | ソート性能 | S | 新規 | 高 |
| r5-scale-06 | visible_entries が全件 clone で毎回 Vec を作り直し | 構造改善 | `model.rs:446-454`; `main.rs:825` | フィルタ＋ソートのたびに全 Entry を複製（4確保/件）。 | 再読込の効率 | M | 新規 | 高 |
| r5-scale-07 | エントリ1件あたり2回の stat と2回の文字列確保 | 構造改善 | `model.rs:107-115`, `:155-160`; `ops.rs:538`, `:557-574` | symlink_metadata + metadata の2 syscall/件、lossy+lowercase の2確保/件。10万件で20万 stat。 | 初期ロード時間 | M | 既知(ドキュメント記載あり) | 高 |
| r5-scale-08 | 10秒リコンサイルが全件再読込＋全件ハッシュ | 残タスク | `main.rs:926-965`, `:1092-1101`; `model.rs:592-602` | 10秒ごとに全件 read_entries + 前景 fingerprint O(n)。DESIGN の「アイドル CPU ほぼ0%」と緊張。 | アイドル負荷 | M | 前回と重複(R6-A-07) | 中 |
| r5-scale-09 | 監視バーストがイベントごとに O(n) 走査 | 構造改善 | `main.rs:977-980`, `:1039-1079`, `:1010-1023` | 各イベントに retain/find が O(n)、加えて fingerprint と rebuild_view も O(n)。k イベントで O(k·n)。 | 監視バースト処理 | M | 前回と重複(R6-A-07) | 高 |
| r5-scale-10 | 監視・進捗の2つの unbounded チャンネル | 残タスク | `main.rs:545-548`, `:1907`, `:1913-1918`, `:1943-1959` | try_send が無制限。64KiB チャンクごとの進捗が蓄積しスロットルがキューを縮めない。 | メモリ増 | M | 前回と重複(R6-A-08) | 高 |
| r5-scale-11 | コピー前に全選択を再帰 stat する total_bytes | 残タスク | `main.rs:1945`; `ops.rs:506-508`, `:557-577` | 選択全ツリーを再帰走査（CancelFlag 非対応）後、copy が同じツリーを再走査。 | コピー開始遅延 | M | 前回と重複(R6-A-08) | 高 |
| r5-scale-12 | コピー1ファイルごとの fsync と 64KiB buffer 確保 | 調査項目 | `ops.rs:297`, `:311`, `:313-315` | ファイルごとに vec![0u8;64KiB] と sync_all。10万小ファイルで fsync がスループット支配。未計測。 | スループット | S | 新規 | 中 |
| r5-scale-13 | undo スタックと閉じタブが件数以外にメモリも無制限 | 構造改善 | `undo.rs:19`, `:30`, `:64-69`, `:96-98`; `tabs.rs:31`, `:107-108` | MAX_UNDO=20 は項目数のみ。全選択ゴミ箱1件に10万ペア、20回で200万ペア。undo_actions() は表示のたびに全 clone。closed は上限なし。 | メモリ増 | M | 新規 | 中 |
| r5-scale-14 | 10万件の実測が無く性能目標を判定できない | 調査項目 | `DESIGN.md:939`, `:1021-1032`, `:1049`, `:1101` | §10.5 の8目標に計測成果物が無く性能テストは1kのみ。1k/10k/100k 生成＋VmRSS/フレーム時間の記録が先決。 | 目標の検証可能性 | M | 前回と重複(R6-A-07) | 高 |
| r5-scale-15 | グリッドで行 index と entry index が混在 | 残タスク | `main.rs:1446-1447`, `:1478-1479`, `:4596-4608`, `:5533-5555` | item_count は行数なのに scroll_to_item/index.id は entry index。10万件10列なら entry5万が行5万へスクロールし終端に張り付く。 | グリッド操作 | S | round1-4と重複(r3-render-03) | 高 |

## レンズ要約

仮想リスト自体は可視 range のみ構築しており健全だが、選択・監視・ソート・検索の全件走査とアロケーションが10万件での主要ホットスポット候補。構造的事実は高確度だが100k実測が皆無でほぼ全件が「理論的」。まず 14 の計測を行い 01/02/03/09 を優先検証すべき。
