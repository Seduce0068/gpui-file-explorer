---
type: Reference
title: Round 3 — 検索 / フィルタエンジン レビュー
description: 読み取り専用エージェント r3-search の所見（15 件）。
tags: [task-discovery, cross-review, search, gpui-file-explorer]
generated: 2026-11-17T23:58:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-search-01 | バイナリ判定が「最初の一致行まで」しか見ていない | 残タスク | `search.rs:296-297`, `:315-327`, `:773-788` | doc は read range 内の NUL でバイナリとするが一致行で即 return し後続を読まない。`b"TARGET\nsecond\0line"` がテキスト扱い。テストは NUL を一致行と同じ行に置いている。 | 検索結果の正当性 | S | 新規 | 高 |
| r3-search-02 | 500 クランプで 2〜3 ルートの合計上限が 2000 に達しない | 残タスク | `main.rs:3729-3735`, `:3784`, `:3792`, `:3818`; `app/README.md:425` | `per_root_cap = global_cap.div_ceil(root_count).clamp(1, 500)`。2ルート=1000、3ルート=1500 が実効上限で doc の「全体 2000」と矛盾。テストは 500 を固定し欠陥を固定化。 | 検索完全性と doc 整合 | S | 新規 | 高 |
| r3-search-03 | マージの cap 適用がソート前で先行ルートに偏る | 残タスク | `search.rs:257-268` | `return` が `sort_by` より前。上限超過時はパス最小ではなく先に処理されたバッチが残る。 | 完全性 | S | round1/2と重複(r1-correct-08) | 高 |
| r3-search-04 | search API が trim せず空クエリテストが trivially pass | 調査項目 | `search.rs:111-114`, `:347-350`, `:446-450`; `main.rs:3742` | `"  "` が空白2文字の検索になる。UI と filter_results は trim しており API 間で意味論が不一致。 | API 一貫性 | S | 新規 | 高 |
| r3-search-05 | 名前フィルタの Unicode case folding 不足 | 残タスク | `search.rs:55-57`, `:68-70`, `:651-682` | trim/lowercase は正しいが `strasse` が `Straße` に一致せず、ς/σ も統合されない。テストは ASCII のみ。 | 国際化 | S | 新規 | 中 |
| r3-search-06 | Ctrl+Shift+F が early return で到達不能 | 残タスク | `main.rs:1582-1587`, `:1597-1603`, `:3698-3708` | name_filter が Some になるのは view==Search の時だけだがキーハンドラが先に return。検索外では None で no-op。 | 新規機能の到達性 | S | round1/2と重複(r2-input-01) | 高 |
| r3-search-07 | 後段フィルタが precomputed sort_key を再利用しない | 構造改善 | `search.rs:61-71`; `model.rs:154`, `:463-466` | エントリごとに `to_lowercase()` と `format!` を生成。sort_key は既に小写化済みでソート側は再利用している。 | 連続入力の効率 | S | 新規 | 高 |
| r3-search-08 | 絞り込みのたびに選択行とスクロールが先頭へリセット | 残タスク | `main.rs:3871`, `:3877-3878`, `:822-848` | `apply_search_filter` が無条件 `selected=0` と `scroll_to_item(0)`。1文字変更や削除イベントのたびにカーソルが先頭へ飛ぶ。 | キーボード操作の一貫性 | S | 新規 | 高 |
| r3-search-09 | 検索の stale 警告が後続の削除イベントで上書きされる | 残タスク | `main.rs:996-1003`, `:1004-1025`, `:3899-3903` | Rescan/Renamed で STATUS_SEARCH_STALE を設定しても直後の Removed が通常ステータスで上書き。 | 警告の保持 | S | 前回と重複(R6-A-04) | 中 |
| r3-search-10 | スナップショットが作成・変更を無視し README と矛盾 | 残タスク | `main.rs:984-1027`; `watcher.rs:20-29`; `app/README.md:427-428` | Removed と Rescan/Renamed のみ処理。Created/Modified は即 return で再検索合図も出ない。README は「作成・変更・リネームで再検索が必要と表示」と明記。 | doc 整合と鮮度表示 | M | 新規 | 高 |
| r3-search-11 | 内容検索の読み取り失敗が黙ってスキップされ部分失敗が不可視 | 残タスク | `search.rs:399-411` | `if let Ok(Some(...))` で Err も None も無視。権限/IO エラーの件数も理由も残らず由来も失われる。 | 部分失敗の可視化 | M | 前回と重複(R6-A-17) | 高 |
| r3-search-12 | LIFO 走査と上限でアルファベット末尾の部分集合が残る | 調査項目 | `search.rs:117`, `:126-130`, `:167`, `:533-559` | children.sort 後に stack.push→pop で降順走査。max_results=3 で末尾側3件が残り、最後のソートは部分集合を整えるだけ。 | 結果の代表性 | S | 前回と重複(r1-correct-08) | 高 |
| r3-search-13 | 拡張子後段フィルタがディレクトリとドットファイルにも一致 | 残タスク | `search.rs:61-67`; `model.rs:408-417`, `:427-432` | `ends_with(".rs")` なので `src.rs` や `.rs` も通過。extension_of は空 stem を除外しており「種類」列と意味論が食い違う。 | フィルタ意味論 | S | 新規 | 高 |
| r3-search-14 | キャンセル・Unicode・CRLF のテストが実質不在 | 調査項目 | `search.rs:561-569`, `:841-849`, `:651-682`, `:773-788` | cancellation テストは開始前に true を渡すだけで中途分岐を通らない。名前フィルタは ASCII、バイナリは同一行 NUL のみ。 | 検証の実効性 | M | 新規 | 中 |
| r3-search-15 | SearchScope::Current のコメントが再帰実装と矛盾 | 調査項目 | `search.rs:181-189`, `:161-163`; `app/README.md:191-195` | enum doc は "only current directory" だが max_depth:24 で再帰。README は正しく「再帰検索」と記載。 | 可読性 | S | 既知(ドキュメント記載あり) | 中 |

## レンズ要約

walker 自体の symlink/隠し/上限/キャンセルは概ね健全だが、実装の意味論と doc・UI の約束の乖離（バイナリ判定、ルート上限、作成・変更の stale 表示）に集中。追加済み名前フィルタは trim・大小文字無視は正しいが Unicode folding が弱く到達不能（round2 既知）。テストは件数・開始前キャンセル・ASCII に偏り実振る舞いを証明できていない。
