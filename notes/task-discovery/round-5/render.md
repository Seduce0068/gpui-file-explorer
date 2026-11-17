---
type: Reference
title: 描画・レイアウト内部のタスク発見（PASS 5）
description: >-
  DESIGN.md の画面レイアウト節と app/README.md の実装済み P0 UI 節、
  app/src/main.rs の render 系関数（list_row / grid_row / grid_cell / sidebar /
  status_bar / toolbar / location_bar / sort_header / overlay / 各ダイアログ）、
  app/src/icons.rs と app/src/strings.rs を実読し、
  描画パスの Mechanics（定数と実描画の対応、行ごとのアロケーション、
  可視範囲計算、アイコン探索、省略表示、オーバーレイ、パンくず、幅競合、
  サイドバー、レイアウト定数の導出）に固有のタスクを file:line 根拠付きで列挙したもの。
  100k 性能測定・watch ホットスポット・進捗スロットル・a11y・列カスタマイズ・
  グリッド機能自体・モード切替スクロール・ホバーツールチップ・DPI/
  フォントサイズ・デバッグフックスクショは他レンズが担当するため再掲しない。
tags:
  - task-discovery
  - gpui-file-explorer
  - rendering
  - layout
  - round-5
generated:
  by: "r5-render"
  at: "2026-11-17T21:32:06+09:00"
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 発見一覧

下表はすべて実読した `file:line` に基づく。設計で意図しているのに実描画と食い違うもの・
整合性が取れていないものを「残タスク」、新たな表示能力の提案を「機能追加案」、
コードから挙動を証明できず計測・方針決定が必要なものを「調査項目」、
定数や分類ロジックの単一情報源化を「構造改善」とした。
パフォーマンス劣化を断定的に述べる項目は設けず、アロケーション回数や呼び出し回数の
事実だけを記し、影響量は「調査項目」として分離した（r5-render-04、r5-render-12）。

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-render-01 | サイドバー幅が「定数 214 / 実描画 190 / 設計 200」の三重ドリフトでグリッド列数計算が 24px 過大控除 | 構造改善 | `app/src/main.rs:83`, `app/src/main.rs:4669`, `app/src/main.rs:5427-5430`, `DESIGN.md:134` | `SIDEBAR_WIDTH_PX = 214.0` が定義され render の列数計算 `(viewport.width - SIDEBAR_WIDTH_PX) / GRID_CELL_WIDTH_PX` に使われる一方、実際の `sidebar()` は `.w(px(190.0))` を返し、`DESIGN.md:134` は「サイドバー 200px 固定」と書く。つまりグリッドの可用幅を実描画より 24px 多く差し引いており、閾値ちょうどのウィンドウ幅で 1 列落ちる（例: 実可用 756px なら 7 列入るのに 732px として 6 列と判定）。定数コメント `app/src/main.rs:79-80` は「drift を避けるために名前を付けた」と宣言しているが、sidebar だけ値が不一致で宣言と逆になっている。単一情報源 (`SIDEBAR_WIDTH_PX`) を `sidebar()` の `.w()` にも適用し、DESIGN の 200px か実描画の 190px かを仕様決定して三者を揃えるタスク。設定でサイドバー幅可変化する将来にも効く。 | 中高 | 小 | 中 | 高 |
| r5-render-02 | 列ヘッダとデータ行の列位置が約 8px ずれている（ヘッダcell のみ内側 px_2） | 残タスク | `app/src/main.rs:4261-4273`, `app/src/main.rs:4281-4298`, `app/src/main.rs:4464-4495` | `sort_header_cell` は `.px_2()` (line 4266) を持ち、固定幅列ではさらに `.w(px(width)).text_right()` (line 4271) を重ねるため、ヘッダ文字の右端は列境界より内側へ寄る。一方 `list_row` の種別・サイズ・更新日時 div (line 4464-4495) は `.w(px(90/90/150))` と `.text_right()` のみで内部 padding が無く、値の右端が列境界に来る。両者は同じ gap_2 / px_3 / アイコン 18px / 90-90-150 の列定義を共有しているのに、内側 padding の非対称だけでヘッダと値が縦方向にそろわない。視覚的な整列崩れで、特に「サイズ」「更新日時」の右揃え列で顕著。ヘッダ cell の `px_2` を外すか、行側にも同量の padding を入れて揃える（狙いは GPUI の padding を両側で同じにすること）。 | 中 | 小 | 中 | 高 |
| r5-render-03 | PageUp/PageDown のページステップ定数 140px が実描画 chrome 高（約 88px 〜）と一致しない | 残タスク | `app/src/main.rs:82`, `app/src/main.rs:1599-1605`, `app/src/main.rs:4287`, `app/src/main.rs:4998-5018`, `app/src/main.rs:5183-5196` | `LIST_CHROME_HEIGHT_PX = 140.0` を使い `((viewport_height - 140) / 28).floor().max(1)` でページ移動行数を算出する。しかし常時表示される chrome は toolbar 高 `(28px ボタン + py_1×2 = 36px)` + sort_header `h(px(26))` + status bar `h(px(26))` = 約 88px で、tab bar / search bar / location bar が出る時だけ増える。最小構成でも 52px 多く差し引くため 1 ページの移動行数が 2 行ほど過小になり、タブ等を開くとさらに差额が広がる。定数を実際の chrome に合わせるか、`window.viewport_size()` とリスト領域の実測 high から導出する（GPUI のレイアウト後計測が必要なら調査項目へ格上げ）。キーボード操作の体感に直結する小さな不具合。 | 中 | 小〜中 | 中 | 高 |
| r5-render-04 | 可視行の構築で Entry::clone・表示名 String・選択パス Vec・リスナ 2 個・MenuBuilder Box が毎フレーム生成される | 調査項目 | `app/src/main.rs:4339`, `app/src/main.rs:4340-4345`, `app/src/main.rs:4362-4388`, `app/src/main.rs:4389-4411`, `app/src/main.rs:5441-5452` | `list_row` は `entries.get(index).cloned()` で Entry を複製し、`display_name_for` (line 118-146) が `name_text.clone()` を返し、search ビューではさらに `format!` で文字列を連結する (line 4342-4345)。加えて `selection.paths().to_vec()` / `vec![entry.path.clone()]` (line 4390-4394)、`batch_paths.clone()` (line 4395)、drop 先用 `entry.path.clone()` (line 4398)、`click` / `right_click` の 2 つの `cx.listener` (line 4362-4388)、`Box<MenuBuilder>` (line 4402-4411) が行ごとに作られる。`grid_cell` も同型 (line 4534-4604)。`uniform_list` の range は可視範囲に限定されるため全件×毎フレームではないが、行数に比例する回数で生成されることはコード上明確。ここでは「アロケーションが存在する」ことだけを確定事項とし、 actual frame time への影響は 100k 件シナリオのプロファイリングが必要なため 調査項目 とする（重複排除した PERF レンズの測定項目と紐付ける）。 | 中 | 中 | 中 | 高（発生箇所）／低（影響量は未測定） |
| r5-render-05 | 1 行につきアイコン分類が 2 回走り、拡張子の小文字化 String も 2 回確保される | 構造改善 | `app/src/main.rs:4353`, `app/src/main.rs:4357`, `app/src/main.rs:4549-4554`, `app/src/icons.rs:28-41`, `app/src/icons.rs:84-92` | list_row は `icon_for_entry(&entry)` (line 4353) と `kind_color(kind_for_entry(&entry))` (line 4357) を別々に呼び、grid_cell も同じ (line 4549, 4553)。`icon_for_entry` は非 dir/非 symlink で `kind_for_name` を呼び (icons.rs:90)、`kind_for_entry` も `kind_for_name` を呼ぶ (icons.rs:32-33)。`kind_for_name` は `rsplit_once('.')` 後に `extension.to_ascii_lowercase()` で String を確保する (icons.rs:38-42) ため、同一ファイル名の分類と小文字化が行ごとに二重実行される。`Entry` 生成時に `FileKind` を 1 回だけ求めて保持するか、`(icon, icon_color)` をまとめるヘルパ（`icon_and_color(&entry, view)`）に統合して呼び出しを 1 回にする。純粋関数でテストも揃っている (icons.rs:117-172) ため安全にリファクタ可能。 | 中 | 小 | 中 | 高 |
| r5-render-06 | grid_columns が gap と padding を計算に含めず、列数×108px が可用幅をはみ出す可能性 | 残タスク | `app/src/model.rs:329-336`, `app/src/main.rs:4526-4527`, `app/src/main.rs:4606-4609`, `app/src/main.rs:5427-5430` | `grid_columns` は `floor(available_width / min_cell_width)` だけで列数を決め、cell 自身の `.w(px(108.0))` 以外の水平コストを無視する。実際の `grid_row` は `.gap_2()` (8px) と `.px_3()` (左右 12px) を持ち (line 4526-4527)、cell も `.px_1()` を持つ (line 4611)。N 列の実幅は概ね `24 + N*108 + (N-1)*8` で、available_width が 108 のちょうど倍数のとき N 列と判定されても実際は 8(N-1)+24 px はみ出し、水平スクロール／クリップを招く。モデル側のテスト (model.rs:1339-1343) は丸めだけを検証し、gap/padding を考虑していない。`grid_columns(available, cell, gap, padding)` のように牌題を渡すか、cell 幅に実効値（108+8）を渡して `/` する形へ変更し、テストに「available=756 なら 6 列（7 列はみ出す）」のような境界ケースを追加する。グリッド機能自体ではなく幅算式の内部精度の話。 | 中 | 小 | 中 | 高 |
| r5-render-07 | 深いパスのパンくずが途中セグメントを省略せず、末尾の現ディレクトリまで潰れる | 機能追加案 | `app/src/main.rs:4851-4858`, `app/src/main.rs:4874-4897`, `app/src/model.rs:699-715` | toolbar の breadcrumb コンテナは `.overflow_hidden()` で全セグメントを並べ (line 4856)、各 segment に `.truncate()` を付けるだけ (line 4887)。`breadcrumb_segments` はルートから全 component を展開して Vec を返す (model.rs:700-715) ため、深いパスでは先頭側から clipping され、利用者が最も見たい末尾（現在ディレクトリ名）も flex 収縮で省略される危険がある。DOM 的には「全部描いてから隠す」方式で、先頭を `…` にまとめる middle-elision や「末尾を優先して常に全表示」制御が無い。加えてセグメント生成時に accumulated.clone() を毎回呼び、render ごとに String+PathBuf の Vec を確保する。末尾 1〜2 セグメントを必ず確保し、残りを `…` のクリックで展開する（あるいは先頭を折り畳む）表示へ変更するタスク。 | 中 | 中 | 高 | 高 |
| r5-render-08 | サイドバーが縦スクロールもセクション折りたたみも持たず、項目増加で下部が到達不能 | 機能追加案 | `app/src/main.rs:4667-4676`, `app/src/main.rs:4731-4846`, `app/src/main.rs:2900-2906` | `sidebar()` は `div().w(px(190)).h_full().flex().flex_col()` で、`.overflow_y_scroll()` を付けていない。場所の後にブックマーク・最近・ドライブを条件付きで連結する (line 4731-4846) ため、これらが多いとウィンドウ高を超えて下部セクションが描画されず、マウスでもスクロールできない。同じコードベースの help dialog は `.max_h(px(500)).overflow_y_scroll()` (line 2905-2906) を使っており、パターンは既知。まず外側コンテナにスクロールを付与し、次に各セクション見出しをクリックで折りたためるようにして情報量を制御する拡張が可能。ブックマーク/最近の件数上限設定とも組み合わせられる。 | 中 | 中 | 高 | 高 |
| r5-render-09 | ステータスバー右側の可変文字列に truncate / min_w_0 が無く、狭いウィンドウで左のパス表示と競合 | 残タスク | `app/src/main.rs:5022-5062`, `app/src/main.rs:5063-5182`, `app/src/main.rs:5183-5196` | 左ブロックは `.min_w_0()` とパスの `.max_w(px(520)).truncate()` (line 5026, 5029-5030) で縮小可能だが、右ブロックには `flex_1` も `min_w_0` も無く、`self.status` / `エラー: {error}` / `監視: 一部無効 — {reason}` などの可変テキスト (line 5067-5084, 5159-5182) に `.truncate()` が無い。外側は `.justify_between().gap_3()` (line 5186-5187) なので、長いエラー文やステータス文が右ブロックを押し広げ、左のパスが 520px まで縮んだ後は右側自身がクリップ／はみ出す。進捗バー 120px と % 表示 (line 5096-5115) も固定幅で競合に加わる。右側の各テキストに truncate と上限幅を付け、優先度（エラー > 進捗 > 選択 > 監視）に応じて省略順を決めるタスク。 | 中 | 小〜中 | 中 | 高 |
| r5-render-10 | 複数ダイアログが同時にオープンし得るが z-order は子の追加順依存（既知） | 調査項目 | `app/src/main.rs:5506-5510`, `app/src/main.rs:1479-1533`, `app/src/main.rs:2887-2899`, `app/src/main.rs:3105-3121` | 5 つのダイアログが root の `.children()` として rename → properties → help → preview → undo_history の順に積まれ、各々が `.absolute().size_full()` の backdrop と `.bg(black().opacity(0.45))` を持つ。キーボードは `on_key_down` の優先 chain (line 1482-1527) で 1 つだけ閉じる挙動だが、マウスや debug フックで複数 flag が立つと backdrop の重なり順＝子順になり、意図しないダイアログが前面に残る／背後の backdrop がクリックを吸う可能性がある。ダイアログ re-entrancy 自体は先行パスで既知とされているため再掲は最小限にとどめ、ここでは「描画順と backdrop の hit-test が仕様化されているか」のみを 調査項目 として記録する。必要なら単一の `active_dialog` 列挙で描画を 1 つに制限し、z-index を明示する。 | 中 | 中 | 低（既知） | 中（同時オープンの実機再現は未実施） |
| r5-render-11 | F1 ヘルプのキー列が固定 230px で、最長ショートカット文字列が説明列へはみ出す | 残タスク | `app/src/main.rs:2916-2928`, `app/src/main.rs:2932-2939`, `app/src/strings.rs:61-67`, `app/src/strings.rs:139-150` | help_dialog はキー列を `.w(px(230.0))` 固定・`.truncate()` 無しで描き、説明列を `.flex_1()` として並べる (line 2916-2928)。一方 `HELP_SECTIONS` には `"Ctrl+Shift+PageUp / PageDown"` (strings.rs:65) や `"Ctrl+C / Ctrl+X / Ctrl+V"` (strings.rs:32) など 230px を超え得る文字列が含まれる。ダイアログ本体は `.w(px(720))` 固定 (line 2933) なので、はみ出しは説明列を圧迫する。`help_sections_are_well_formed` は空文字列しか検証せず (strings.rs:139-150)、幅収まりはテストされていない。キー列に `.truncate()` を付けるか、キーと説明を 2 行配置に変える／幅を可変にして折り返すかの表示設計が必要。ヘルプは初回ユーザーが最初に触れる画面なので整列崩れの体感が大きい。 | 中 | 小 | 中 | 高 |
| r5-render-12 | uniform_list の固定行高前提と grid 行の実高・可視範囲の整合が未検証 | 調査項目 | `app/src/main.rs:5436-5457`, `app/src/main.rs:4413-4422`, `app/src/main.rs:4501-4531`, `app/src/main.rs:4605-4663`, `app/src/model.rs:338-344` | `uniform_list("explorer-list", item_count, ...)` は行高を先頭アイテムから測る API で、全アイテムが同一高であることを前提に可視 range を計算する。list モードの行は `.h(px(28))` (line 4421) で一定、grid モードの行は cell `.h(px(104))` + row `.py_1()` (line 4528, 4609) で一定に見える。終端の部分行も `grid_row` が `checked_mul`/`min(len)` で cell 数を減らすだけ (line 4507-4517, model.rs:339-344) なので高さは変わらないはずだが、GPUI の `uniform_list` がアイテム高を再計測するタイミング（mode 切替・font scale 変更時）と `track_scroll` (line 5457) の組み合わせで range がズレるかどうかはコードから証明できない。表示崩れが出るかは実機／GPUI ソース確認が必要なため 調査項目 とし、ここでは「固定高の根拠」と「未検証点」を切り分けて記録する（モード切替スクロール自体は他レンズ担当のため重複しない）。 | 中 | 中 | 中 | 中（コード上の固定高は高、GPUI 内部挙動は未確認） |

## レンズ総評

描画パスの最大の問題は「定数が正しく設定されているのに実描画が追従していない」ことに集約される。
`SIDEBAR_WIDTH_PX=214` に対して実描画は 190px、設計書は 200px（r5-render-01）、
`LIST_CHROME_HEIGHT_PX=140` に対して実 chrome は約 88px 〜（r5-render-03）で、
いずれも「drift を避けるために名前を付けた」というコメント（main.rs:79-80）と矛盾している。
まずこの 2 件を単一情報源化し、列ヘッダと行の padding 非対称（r5-render-02）を直せば、
低コストで見た目とキーボード操作の両方が改善する。

次に手を付けたいのは可視行の無駄な生成（r5-render-04）とアイコンの二重分類（r5-render-05）で、
後者は純粋関数のみを触るため安全に統合できる。ただし前者の frame time への影響は
未測定なので、PERF レンズの 100k 件計測と突き合わせてから優先度を確定する。
超過幅まわりの grid_columns（r5-render-06）/ パンくず（r5-render-07）/ ステータスバー（r5-render-09）
は同じ「利用可能幅を見積もるときに子要素のコストを忘れる」系統で、まとめて境界テストを整備したい。

サイドバー（r5-render-08）とヘルプのキー列（r5-render-11）は表示設計の判断が残るが、
どちらも既存の `overflow_y_scroll` / `truncate` パターンを再利用すれば小さく直せる。
ダイアログの重なり（r5-render-10）と uniform_list の高さ前提（r5-render-12）は
コードだけでは結論が出ないため 調査項目 として残し、実機再現と GPUI ソース確認を次パス以降に委ねる。
本研究ではいずれもソースを変更していない（読み取りのみ）。
