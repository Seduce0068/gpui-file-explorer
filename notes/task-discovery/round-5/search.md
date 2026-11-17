---
type: Reference
title: 検索エンジン内部の監査（ラウンド5・-search- レンズ）
description: >-
  app/src/search.rs と main.rs の検索オーケストレーションを実読し、
  並び順・Unicode・拡張子・マルチルート統合などの未達項目を file:line 根拠付きで列挙した調査項目一覧。
tags:
  - task-discovery
  - search
  - search-engine
  - gpui-file-explorer
  - round-5
generated: 2026-11-17T21:26:17+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## レンズ範囲と前提

本レンズは `app/src/search.rs`（全 877 行を通読）と `app/src/main.rs` の検索系
（`display_name_for`: 115-146 / `SearchState`: 356-372 / `per_root_cap`: 3651-3662 /
`run_search`: 3664-3773 / `apply_search_filter`: 3775-3822 / `search_bar`: 3856-3969 /
一覧描画: 4338-4345, 4534-4541）を対象とした。他パスで既に扱われた
クイックフィルタ・検索ソース表示・履歴・自動リフレッシュ・検索範囲・行番号/
バイナリ除外/1MiB 上限・件数/深さ上限・正規表現非対応・Enter バグ・
ハッシュ重複検出・デバッグフック・履歴チップ UI は重複を避けた。

以下は不確実な挙動を「調査項目」として記録した仮説であり、ソース修正は行っていない。

## 所見

### r5-search-01: 検索結果は絶対パス順固定で関連度ランキングが無い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-search-01 | 名前/内容検索とも絶対パス順で関連度が反映されない | 機能追加案 | `app/src/search.rs:91`,`:154`,`:402`,`:237-256`; `app/src/main.rs:3764` | 名前検索・内容検索・マルチルート統合の全てが `entry.path.cmp()` でソートする。部分一致の位置（先頭一致/完全一致）や内容ヒット数で順位付けせず、`children.sort()`(117,353) も最終的に上書きされる。マルチルートでは絶対パスの辞書順になるため、どのルートの結果が先に並ぶかもパス文字列に依存する。 | 高（「`main.rs` と入力したら `main.rs` が先に出る」という利用者期待に沿う。完全一致→前方一致→部分一致の多段ソート等） | M | 新規 | 高 |

### r5-search-02: 名前一致は `to_lowercase()` のみで Unicode 正規化・完全 case fold が無い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-search-02 | `to_lowercase()` 依存で NFD 名・特殊文字の一致が漏れる | 残タスク | `app/src/search.rs:98`,`:137`,`:334`,`:309-311`; `notes/fs-cross-platform-notes.md:268`,`:425` | `needle = query.to_lowercase()` と `name.to_lowercase().contains(&needle)` のみ。Rust の `to_lowercase` は完全 case folding ではない（例: `ß`/`ẞ`、トルコ語 `İ`、ギリシャ語最終シグマ）。また NFD→NFC 正規化が無く、macOS の分解済みファイル名（`@`+combining accent 等）を合成済みクエリで検索できない。設計資料は検索・重複判定での NFC 比較を既に指摘している。 | 高（日本語・欧文ファイル名の取りこぼし防止。Mac 実機での検索精度） | M | 既知(ドキュメント記載あり) | 高 |

### r5-search-03: 拡張子フィルタは末尾文字列一致で真の拡張子と不一致

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-search-03 | `ends_with(".{ext}")` が `model::extension_of` と意味が異なる | 調査項目 | `app/src/search.rs:42-61`（特に 52-58）,`:607-626`; `app/src/model.rs:408-416` | `filter_results` は `name_text.to_lowercase().ends_with(&format!(".{extension}"))` で判定するため、ドットファイル `.rs` や `foo.rs.` のような名前も `rs` に一致しうる。一方 `model::extension_of`（`rsplit_once('.')` で stem/extension 両方の非空白を要求）や `Path::extension()` は真の拡張子を返す。既存テストは `a.TXT`/`b.rs`/`c.rs.bak` のみで、この意味差を検証していない。後段フィルタと一覧の「種類」列で判定が食い違う可能性を確認したい。 | 中（絞り込み結果の説明可能性。非 UTF-8/隠しファイルとの整合） | S | 新規 | 高 |

### r5-search-04: 隠しファイル判定がルート自身に適用されず Windows 属性も非対応

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-search-04 | 隠しディレクトリをルートに指定すると中身が検索される | 調査項目 | `app/src/search.rs:104`,`:130-133`,`:366-368`; `app/src/main.rs:3688`,`:3694-3697`; `app/src/model.rs:404-406` | `search_by_name`/`search_contents` はルートを無条件にスタックへ積み（104/340）、隠し判定は子要素の名前に対してのみ実施（130-133/366）。したがってブックマークやタブが `~/.config` のようなドットディレクトリを指す場合、`include_hidden=false`（=`settings.show_hidden=false`）でもその配下が検索される。さらに `is_hidden_name` は `starts_with('.')` のみで、Windows の HIDDEN 属性を持つ非ドットファイルは判定できない（属性判定の有無は要確認）。 | 中（「隠しファイルを表示しない」設定と検索結果の整合） | S〜M | 新規 | 高（ルート未判定）/ 中（Windows 属性） |

### r5-search-05: シンボリックリンク先ディレクトリがフォルダ表示されるが走査されない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-search-05 | 名前一致した symlink が「開けるフォルダ」のように見える | 調査項目 | `app/src/search.rs:134-147`,`:369-382`; `app/src/model.rs:155-166`; `app/src/main.rs:4352-4358` | walker は `symlink_metadata` でリンクを判定するためリンク先へは降りず（134-136/373-376）、無限ループは起きない。しかし `entry_for_path` は `metadata()`（リンク追跡）で `is_dir` を決めるため（model.rs:157-158）、ディレクトリへの symlink は `is_dir=true, is_symlink=true` の Entry になり、アイコンもフォルダ色で描画される。利用者は「フォルダとして検索されたのにその中身がヒットしない」という矛盾に直面しうる。 | 中低（結果の意味論を明示。調査報告にも symlink 非追跡は明記済み） | S | 既知(ドキュメント記載あり) | 高 |

### r5-search-06: 複数ルートのうち読めないルートが黙ってスキップされる

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-search-06 | 権限不足ルートの部分失敗が UI に一切報告されない | 残タスク | `app/src/search.rs:110-116`,`:346-352`; `app/src/main.rs:3714-3745`,`:3816-3820` | `search_roots` は `path.is_dir()` で生存確認するが、read 権限が無いディレクトリは `is_dir()==true` のまま残る。`search_by_name` は `let Ok(reader) = read_dir(&dir) else { continue; }`（110-112）、各エントリも `filter_map(|entry| entry.ok())`（114/350）でエラーを捨てる。背景タスクは `Vec<SearchResult>` しか返さず（3745）、ステータスは単に「N 件」（3816-3820）。あるルートが原因で 0 件になっても利用者は理由を知れない。 | 高（検索結果への信頼。部分的な失敗の可視化） | M | 新規 | 高 |

### r5-search-07: 巨大ディレクトリの列挙・ソート中はキャンセルが効かない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-search-07 | `read_dir` → `collect` → `sort` の区間に cancel 確認が無い | 残タスク | `app/src/search.rs:106-117`,`:342-353`,`:297-315`; `app/src/main.rs:3681-3685`,`:3715-3718` | キャンセルはディレクトリの走査開始前（107/343）と子要素ループ先頭（120/356）、内容検索は行単位（298）で確認される。しかし `read_dir` の全エントリを `collect()` し `children.sort()` する区間（110-117/346-353）には確認が無く、10 万エントリ規模のディレクトリでは Esc を押してもレスポンスが遅れる。内容行の `File::take(max_bytes)` 自体は 1MiB 上限で、行間では確認される。 | 中（大型ツリーでのキャンセルレイテンシ） | S | 新規 | 高 |

### r5-search-08: 全件をメモリに溜めてから表示し、コピーも多段に発生する

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-search-08 | ストリーミング表示が無く Entry の全コピーが完了まで保持される | 構造改善 | `app/src/search.rs:103`,`:243-255`,`:59`; `app/src/main.rs:3710-3746`,`:3756-3764`,`:3783-3786`; `RESEARCH.md:506`,`:706` | 各ルートが `Vec<SearchResult>` を全件構築し、`run_search` が `batches` を保持、`merge_search_results` が HashSet + 別 Vec を生成、`all_entries` へ collect、`apply_search_filter` が `filter_results` の `.cloned()` でさらに `entries` を作る。加えて `search_lines` HashMap も作られる。完了まで UI はスピナーのみで、設計資料が想定した「チャネルで逐次結果を追加表示」にはなっていない。現在は上限 2000 件で有界だが、コピー回数と先行表示の欠如は実在する。 | 中高（巨大ツリーでの体感速度。設計方針との整合） | L | 既知(ドキュメント記載あり) | 高 |

### r5-search-09: マルチルートの重複排除が生パス文字列依存で親子ルートの不変条件が未検証

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-search-09 | canonicalize はルート dedupe のみに使われ結果 dedupe は生パス | 調査項目 | `app/src/search.rs:209-220`,`:237-256`（特に 249）,`:848-876`; `app/src/main.rs:3694-3697`,`:5882-5889` | `search_roots` は `canonicalize()` でルートを重複排除するが、検索結果は元のルートパスから作られる。`merge_search_results` は `result.entry.path.clone()` をキーに first-wins で dedupe するため、canonicalize が失敗した別表記のルート（シンボリックリンク経由・bind mount 等）や親子で重なるルート（`/data` と `/data/shared`）は同一ファイルでも重複しうる。親子ルートの期待挙動（片方のみ表示／ shortest relative 表示の選択）を定めたテストが無い。 | 中（複数タブ/ブックマーク検索の結果整合） | M | 新規 | 高 |

### r5-search-10: 内容検索は UTF-8 lossy 前提で UTF-16 等はバイナリ扱い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-search-10 | `String::from_utf8_lossy` と NUL 判定がエンコーディングを暗黙固定 | 調査項目 | `app/src/search.rs:279-317`（特に 306-311）,`:334` | 各行は `line.contains(&0)` で NUL を見つけると即「バイナリ」と判定し、`String::from_utf8_lossy(&line).to_lowercase().contains(needle)` で一致させる。このため UTF-16/UTF-32 テキストは NUL を含むほぼ常にスキップされ、Shift_JIS 等の非 UTF-8 テキストは U+FFFD に置換されて一致しない。README はプレビューのテキスト/画像対応のみを明記し、内容検索のエンコーディング前提は明文化されていない（要確認）。 | 中低（前提の明文化、必要ならエンコーディング選択肢） | M | 新規 | 高（挙動）/ 中（仕様未明記の確認） |

### r5-search-11: 内容検索は行番号のみで一致箇所のハイライトもスニペットも無い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-search-11 | `:{line}` を名前に付けるだけで一致文字列を表示しない | 機能追加案 | `app/src/search.rs:81-88`（`match_line` のみ）,`:386-397`; `app/src/main.rs:3756-3763`,`:4342-4345`,`:4538-4541` | `SearchResult` が保持するのは `match_line: Option<u32>` のみで、一致行のテキストやオフセットは破棄される。一覧・グリッドとも `format!("{display_name}:{line}")` を表示するだけで、どの文字が一致したかは分からない。内容検索の目的（該当箇所を確認する）に対して情報が不足している。 | 中高（内容検索の実用性。ファイルを開かずに該当行を確認） | M | 新規 | 高 |

### r5-search-12: マルチルート統合の不変条件を検証するテストが薄い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-search-12 | dedupe/cap のテストが 1 ケースのみで内容結果・親子ルート未カバー | 残タスク | `app/src/search.rs:848-876`,`:811-828`; `app/src/main.rs:5882-5889` | `merge_search_results_dedupes_by_path_and_caps` は `match_line: None` の単純な 2 パス重複と cap=1 のみ。`search_roots` のテストも欠損/重複/フォールバックに留まる。未検証: (a) 親子ルートが重なる場合の global cap と per-root cap の相互作用（main.rs:3656-3660/3719）、(b) 内容検索結果の `match_line` がマージ後も保持されること、(c) 複数ルート間で同一パスの first-wins が決定的であること、(d) `filter_results` の隠しドットファイル挙動（r5-search-03/04 と連動）。 | 中（リファクタ時の回帰防止） | S〜M | 新規 | 高 |

## レンズ総評

検索エンジン自体は「conservative for safety」を徹底しており、symlink 非追跡・
上限付き・協調キャンセル・3 層の世代ガード（main.rs:3678-3685, 3750）は模範的で、
r3-async が指摘した `open_search` のキャンセル欠如以外の並行安全性は高い。
残る課題は主に **「見せ方と意味論」** に集中する。

優先度の高い順に、r5-search-06（読めないルートの黙ってのスキップ）、
r5-search-02（Unicode 正規化/case fold）、r5-search-01（関連度順）を推す。
これらは利用者が「検索が壊れている」と直接感じる経路であり、
いずれも検索結果の信頼性に直結する。次点で r5-search-08（逐次表示）と
r5-search-11（ハイライト/スニペット）が体感価値が高い。
r5-search-03/04/05/09/10 は仕様判断を伴う「調査項目」として記録し、
round-6 の統合では DESIGN/RESEARCH の既存指摘（NFD 対策・逐次表示）と
接続して整理するのが妥当である。
