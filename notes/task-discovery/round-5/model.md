---
type: Reference
title: "PASS 5 タスク発見：データモデル／ソート／フィルタ／人間化 Internals"
description: "model.rs と DESIGN.md §5 の型契約を精読し、単体テスト可能な不変条件の観点から残タスク・調査項目を抽出したレンズ報告"
tags:
  - task-discovery
  - pass-5
  - model
  - sorting
  - humanization
generated:
  by: "r5-model"
  at: "2026-11-17T21:26:15+09:00"
status: complete
---

## 概要

本レンズは PASS 5（最終調査パス）のうち、データモデル / ソート / フィルタ / 人間化の内部実装を担当した。
精読対象は `app/src/model.rs`（全 1426 行）および `DESIGN.md:474-645`（§5 データモデルと型 / 実装契約）。
呼び出し側の挙動確認のため `app/src/main.rs` の選択・描画・再読込パスも参照したが、変更は行っていない。

重複回避のため、以下の既出トピックは本報告で扱わない：プロパティの作成/アクセス時刻、ステータスバーの選択サイズ、
列カスタマイズ、kind ソートの実装自体、隠しファイル切替 UI、ロケーション入力の補完/展開、非 UTF-8 の lossy 変換、
ハードリンク/inode 表示、`sort.dirs_first` の無視、タブセッション復元、history 200 定数、検索上限。
なお `r4-fs-special`（FIFO/socket 等特殊ファイルが「—」で表示される問題）と重複する範囲も除外し、
本レンズでは複合拡張子とシンボリックリンクのラベルに絞った。

各 finding は「現在の挙動」→「具体的タスク」→「単体テスト可能な不変条件」の順で記載する。

## 検出結果

### r5-model-01 — 自然順の Unicode／先頭ゼロと tie-break 契約の逸脱

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r5-model-01 | 自然順の Unicode/先頭ゼロと tie-break 契約逸脱 | 調査項目 | app/src/model.rs:103, 464-495; DESIGN.md:533; app/Cargo.lock:159（alphanumeric-sort 1.5.8） | `sort_key` は `to_lowercase()` で事前計算（103）。比較は `compare_str(小文字).then_with(compare_str(原文))`（464-466, 484）。依存クレートは非ロケール照合で、非数字を Unicode スカラ値順に比較し U+00FF をまたぐと順序を反転する特則を持つ（crate ドキュメント）。DESIGN §5.1 は「同値は path で安定化」と規定するが（DESIGN.md:533）、実装の tie-break は `name_text`（465, 484）で異なる。タスク：`file01`/`file1`、`第1章`/`第1-2章`、`あ`/`ア`/`一`、異ディレクトリ同名の不変条件テストを追加し、path tie-break に揃えるか設計を改訂する。 | 中（名前順の予測可能性と決定的順序） | 中 | 中（実装自体は対象外だが不変条件と契約逸脱は未扱い） | 高（コード・crate ドキュメント直読） |

### r5-model-02 — kind_label の複合拡張子とシンボリックリンク種別

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r5-model-02 | kind_label の複合拡張子とリンク種別 | 残タスク | app/src/model.rs:104-115, 408-433, 148-160; DESIGN.md:487-505; tests:990-1001 | `extension_of` は `rsplit_once('.')`（411）のため `archive.tar.gz` → `gz` となり、複合拡張子の意味が失われる。`kind_label` は `is_dir` を先に判定（421）し、`is_dir` は `metadata()` が symlink を追跡するため（112-115）リンク先ディレクトリは「フォルダ」表示で「リンク」にならない。DESIGN §5.1 の `EntryKind` 5 区分（SymlinkToDir/SymlinkToFile 含む）が bool 2 つに潰れ、区別が失われた。タスク：複合拡張子ポリシー（既知拡張子の優先 etc.）とリンク種別ラベルを定義し単体テストを追加。特殊ファイルの「—」は r4-fs-special と重複し対象外。 | 中高（表示の正確さ、設計契約整合） | 低〜中 | 高（特殊ファイルと複合拡張子・リンクは別） | 高（コード直読） |

### r5-model-03 — human_size の単位記号・境界・丸めポリシー

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r5-model-03 | human_size の単位記号・境界・丸めポリシー | 残タスク | app/src/model.rs:611-625; tests:1018-1026 | 計算は 2 進（1024）なのに単位記号は SI（`B,K,M,G,T,P`、613）。`1023` は `1023 B`、`1024` は `1.0 K` と境界で表現が飛ぶ。P で打ち止めのため EB 以上は `16384.0 P` のような巨大数になる。`format!("{value:.1}")`（623）は f64 の round-half-to-even に依存し、10 進の期待とずれる場合がある。タスク：IEC 表記（KiB 等）採用か現状維持かを決定し、0/1023/1024/1536/1024^5/`u64::MAX` の境界テストを追加。 | 中（サイズ表示の一貫性） | 低 | 中（ステータスバーの選択サイズ集計とは別） | 高（コード直読） |

### r5-model-04 — human_age の未来時刻・うるう年・境界値

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r5-model-04 | human_age の未来時刻・うるう年・境界値 | 残タスク | app/src/model.rs:636-657; tests:1038-1050 | 未来時刻は `duration_since` の Err を無条件に「未来」へ変換（642-644）。ネットワークマウントの mtime や時計ズレで数秒先の通常ファイルが「未来」と表示される。年は 365 日固定（652-656）でうるう年を考量しない。タスク：数秒程度の未来を「たった今」とみなす許容窓を定義し、59/60、3599/3600、86399/86400、364/365/366 日の境界を単体テストで固定する。 | 中（相対時刻表示の頑健性） | 低 | 高（既存テストは典型値のみで境界未カバー） | 高（コード直読） |

### r5-model-05 — human_datetime のローカルタイムゾーン暗黙前提

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r5-model-05 | human_datetime のローカルTZ暗黙前提 | 調査項目 | app/src/model.rs:627-634; tests:1028-1036; app/Cargo.toml:23（chrono 0.4） | `chrono::Local` のローカル TZ を暗黙に前提とし、固定書式 `%Y-%m-%d %H:%M`（632-633）。秒・UTC オフセット・12/24h 設定が無いため同一分内の更新が同一文字列になり、DST の曖昧時刻も区別できない。リモート FS / NTFS の UTC 時刻がマシン TZ でシフトして表示される点も未文書化。既存テストは長さ 16 文字のみ（1033）で書式変更に脆い。タスク：TZ/書式方針を設計化し、オフセット付き表示と境界テストを追加。 | 中（日時解釈の透明性） | 低〜中 | 高（DATA MODEL レンズでは未扱い） | 高（コード直読） |

### r5-model-06 — fingerprint の陳腐化ポリシーと衝突許容

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r5-model-06 | fingerprint の陳腐化と衝突ポリシー | 構造改善 | app/src/model.rs:586-609; tests:1052-1069; app/src/main.rs:924-960 | `DefaultHasher` で `len/name/is_dir/size/modified(秒)` のみをハッシュ（592-601）。コメント自身が同一秒・同一サイズの内容変更を検出できないと認める（589-591）。さらに `is_symlink`（リンク先差し替え）とサブ秒時刻をハッシュせず、pre-epoch は全て `None` に畳まれる（604-609）。10 秒 reconcile（main.rs:924-960）の検知漏れに直結する。タスク：ハッシュ対象を明示した上で 1 秒ブラインドスポットの再現テストと、衝突時の挙動（再読込に倒す等）の文書化を追加。 | 中高（変更検知の信頼性） | 中 | 中（r1-qual-05 は watcher 再試行、r2-scn-05 は内容ハッシュで、fingerprint 自体は未扱い） | 高（コード直読） |

### r5-model-07 — read_entries の部分読み取り失敗が黙って捨てられる

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r5-model-07 | read_entries の部分エラーが非表示 | 残タスク | app/src/model.rs:93-142, 148-171; DESIGN.md:502; tests:779-815 | `read_dir` の各 item は `let Ok(item) = item else { continue }`（136）で失敗を黙って破棄。`from_dir_entry` も metadata 失敗時に `size=0/modified=None/is_dir=false` を返し（107-115）、部分不能ディレクトリが「成功」かつ誤サイズの一覧になる。DESIGN §5.1 は `size: Option<u64>`（502）を規定するが実装は `u64` + 0 フォールバック。`entry_for_path` は `link_metadata` へフォールバック（157）し挙動も不一致。タスク：警告または `FsError` の返却経路と、破損 symlink / 権限否定子のテストを追加。 | 中高（一覧の完全性と信頼性） | 中 | 高（部分不能ディレクトリの扱いは未扱い） | 高（コード直読） |

### r5-model-08 — フィルタ→ソートは正しいが索引ビュー契約と乖離

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r5-model-08 | 索引ビュー契約との乖離 | 構造改善 | app/src/model.rs:445-454; DESIGN.md:513-534; app/src/main.rs:819-839 | `visible_entries` は filter → sort の順（447-452）で、順序自体は正しい。しかし全 `Entry` を clone して新しい Vec を作るため、隠し切替・ソート変更のたびに全件コピーが発生する。DESIGN §5.1 が規定する `visible: Vec<usize>` インデックスビュー（518-520, 533-534）・「再読込せずに表示のみ作り直す」（534）という契約と乖離し、`rebuild_view`（main.rs:819-839）も毎回 Vec を再構築する。タスク：索引ビューへ移行し、フィルタがソート前に適用されることと I/O を伴わないことを不変条件テスト化。 | 中（10 万件時の不要なクローン回避、設計整合） | 中〜高 | 中（r1-qual-01 は O(n) 一般で、§5.1 構造逸脱は未扱い） | 高（コード・設計書直読） |

### r5-model-09 — Selection の範囲計算と不可視エントリの扱い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r5-model-09 | Selection の範囲計算と不可視扱い | 残タスク | app/src/model.rs:173-280; app/src/main.rs:819-837, 1410-1416; DESIGN.md:559-593; tests:1250-1330 | `select_range` は可視順の paths を受け取り（main.rs:1410-1416）、両端点を含む範囲を選択（252-257）。この点は正しい。一方 `rebuild_view` が可視 path だけで `selection.retain()` を呼ぶため（main.rs:826, model.rs:268-279）、隠し表示を OFF にすると選択中の隠しファイルが黙って解除される。Ctrl 解除時に anchor を末尾へ付け替える挙動（228-230）も直感と異なる。DESIGN §5.3 の `focused` / `paths_for_action`（572, 588）は未実装。タスク：「選択 ⊆ 可視」「anchor ∈ 選択 ∪ {None}」の不変条件テストと、不可視選択の保持方針を決定する。 | 中（選択操作の予測可能性） | 中 | 高（ステータスバーの選択サイズ集計とは別） | 高（コード直読） |

### r5-model-10 — breadcrumb のルート／Windows Prefix 扱い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r5-model-10 | breadcrumb のルート/Prefix 扱い | 残タスク | app/src/model.rs:699-715; tests:1079-1100 | `breadcrumb_segments` は `RootDir` と `Normal` のみラベル化し、それ以外は `_ => {}`（711）でスキップ。Unix の `/` は 1 セグメントとして正しく扱われる（tests:1083）。しかし Windows の `C:\Users\foo` は `Prefix` がラベル化されず、先頭セグメントが `Users`（accumulated=`C:\Users`）となりドライブルートへ戻れない。UNC も同様。相対パスの `..` も正規化されず accumulated に残る（tests:1092-1099）。タスク：Prefix を明示セグメント化し、ドライブ/UNC ルートと `..` の扱いをテストで固定する。 | 中（Windows 対応の同等性） | 低〜中 | 高（既出レンズで未扱い） | 高（コード直読） |

### r5-model-11 — grid_columns が実効セル幅（gap/padding）を考慮しない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r5-model-11 | grid_columns の実効幅未考慮 | 調査項目 | app/src/model.rs:329-344; app/src/main.rs:84, 4521-4531, 5427-5434; tests:1333-1355 | `grid_columns` は `available_width / min_cell_width` のみで算出し（335）、非有限幅・不正な最小幅は 1 列にフォールバック（332-334）。呼び出し側もサイドバー幅を引いた値が負なら 1 列になる（5428）。しかし `grid_row` は `.gap_2().px_3()`（4526-4527）を付けるため実効セル幅は `GRID_CELL_WIDTH_PX=108`（main.rs:84）より広く、計算列数が収容可能列数を超える可能性がある。ゼロ件時に `grid_row_count` の `columns==0` ガード（340-342）は実質デッド。タスク：gap/padding 込みのストライドで算出し、極小・小数・非有限幅のテストを追加。 | 中（グリッドの折返し/はみ出し防止） | 低〜中 | 中（r4-cfg-09 はセル幅固定の設定面で、列数算出式は別） | 中高（算出式は事実、実際のはみ出しは要 UX 確認） |

### r5-model-12 — resolve_location_input の `~` 対応 gaps（既知）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| r5-model-12 | resolve_location_input の ~ 対応 gaps（既知） | 調査項目 | app/src/model.rs:282-314（doc comment 286-287）; tests:1384-1425 | 既知として記録。`~` と `~/...` のみ home へ展開し（297-307）、`~user` はリテラル（tests:1420-1425）、`~+` / `~-` / 環境変数展開は無し。home 未設定時はエラー（300, 304）。Windows の `~\` 区切りも未対応。doc comment に「no shell expansion or `~user` support」と明記済みのため新規調査ではない。将来ユーザー辞書や環境変数展開を入れる場合は、パス正規化（`.` / `..`）と併せて設計する。 | 低（既に文書化済み） | — | 低（既知） | 高（コード直読） |

## レンズ総評

本パスで最も優先度が高いのは **r5-model-07（部分読み取り失敗の黙殺）** と **r5-model-06（fingerprint の 1 秒ブラインドスポット）** である。
いずれも「成功したように見えるのに実態が違う」という性質を持ち、一覧の信頼性と直接結びつく。
次点で **r5-model-02（kind_label の複合拡張子・リンク種別）** と **r5-model-01（自然順の Unicode/先頭ゼロと path tie-break 契約）** を挙げる。
後者は DESIGN §5.1 の明記された契約との逸脱であり、単体テストで容易に固定できる。

中期的には **r5-model-08（索引ビューへの移行）** が 10 万件目標（DESIGN §11）と設計契約の両面で効く。
ただし既存の `Selection` がパスキーへ寄せられているため（r5-model-09）、ビュー移行と選択の不変条件整備は同じ変更セットで扱うのが望ましい。

`human_*` 系（r5-model-03/04/05）はいずれも表示ポリシー未定義が本質で、実装量は小さい。
単位記号（SI vs IEC）、未来時刻の許容、TZ の明示という 3 つの方針を先に決めれば、境界値テストは機械的に追加できる。

全 finding は読み取り専用の調査であり、ソースコードおよび設定の変更は行っていない。
`resolve_location_input`（r5-model-12）は指示に従い「既知」として記録した。
