---
type: Reference
title: 設定サーフェスとユーザー制御の残タスク調査（PASS 4）
description: >-
  gpui-file-explorer の Settings 構造体・DESIGN.md 設定スキーマ・main.rs の設定消費箇所を
  照合し、UI から変更できない設定・ハードコード定数・設定管理機能の欠如を発掘した
  レンズ別調査。全所見はファイル:行で検証済み。
tags:
  - task-discovery
  - gpui-file-explorer
  - settings
  - pass-4
generated: 2026-11-17T20:54:33+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 調査範囲と前提

`app/src/settings.rs:51-78`（Settings 構造体）、`DESIGN.md:819-843`（設定スキーマ）、
`app/README.md:149-152,188,198-207`（設定の説明）、`app/src/main.rs` の `settings.` 参照
（57 箇所）を網羅的に照合した。重複回避のため、スキーマ移行・永続化の耐久性、
JSON 配列上限、設定パス、将来スキーマの read-only 化、デバッグフック、テーマ、
i18n、配布、ライセンスは対象外とする。

## 発見事項

### r4-cfg-01 設定ダイアログが存在しない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-cfg-01 | 設定ダイアログ／設定 UI が存在しない | 機能追加案 | `app/src/main.rs:2073`（undo-history）, `:2652`（preview）, `:2879`（help）, `:3105`（properties）, `:3365`（rename）がダイアログの全種；`:4998-5018` ツールバーに設定ボタンなし；`DESIGN.md:117` には `[設定?]` と計画だけが残る；`app/src/settings.rs:53-78` | 永続フィールドは 10 個あるが、UI から直接編集できるのは `sort`・`show_hidden`・`view`・`tabs` 等一部のみ。残りは JSON を手編集するしかなく、GUI アプリとしての発見性が低い。DESIGN のモックアップに設定 affordance が描かれていたが未実装。 | キーボードショートカットを覚えない利用者でも全設定に到達可能。初回設定とトラブルシュートの敷居が下がる。 | M | 新規 | 高 |

### r4-cfg-02 dirs_first は保存されるが並び順に一切効かない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-cfg-02 | `sort.dirs_first` が保存・シリアライズされるだけで動作に反映されない | 残タスク | `app/src/settings.rs:57`；`app/src/model.rs:528-530`（スキーマに保持）, `:544-548`（`order()` は key/ascending のみ）, `:470-473`（`sort_entries` は常に dirs/files を分割）, `:551-556`（`from_order` が常に `true` を再設定）；`app/src/main.rs:821-822,1654-1657` | JSON で `dirs_first: false` と書いても `rebuild_view` が `SortSpec::order()` に変換する時点で情報が失われ、ディレクトリは常に先頭に来る。並び順変更のたびに `true` へ上書きされるため、ファイルの永続的な改変もできない。スキーマ側コメントは「将来の mix オプション用」と説明しているが、現状は死にフィールド。 | スキーマの嘘を解消。ファイルとフォルダを混ぜて並べたいという明確な要求を実装可能にする（設計済みの拡張ポイントを活用）。 | M | 新規 | 高 |

### r4-cfg-03 検索履歴の保存数と表示数が不一致

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-cfg-03 | 検索履歴は 10 件保存されるが 8 件しか表示されない | 残タスク | `app/src/settings.rs:39`（`MAX_RECENT_SEARCHES: usize = 10`）, `:144-146`（truncate(10)）；`app/src/main.rs:3992`（`.take(8)`）；`app/README.md:198-199`（「最大 10 件まで保存」と明記） | 9 件目・10 件目は設定ファイルに残るが UI に現れず、再利用も削除もできない。ドキュメントの「最大 10 件」と体験が矛盾する。 | ドキュメントと挙動の一致。保存済みデータが silent に隠れないようにする（片方へ揃えるだけ）。 | S | 新規 | 高 |

### r4-cfg-04 永続コレクションに個別削除・クリアの UI がない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-cfg-04 | ブックマーク／最近の場所／検索履歴を UI から削除できない | 機能追加案 | `app/src/main.rs:4740-4766`（ブックマーク行は click で移動のみ）, `:4778-4804`（最近も click のみ）, `:3992-4014`（検索チップは click で再検索のみ）, `:897-915`（ブックマーク解除は現在ディレクトリの Ctrl+D のみ）；`app/src/settings.rs:107-147`（record/toggle API はあるが remove/clear の UI 呼び出しなし） | 古いブックマークはそのディレクトリへ移動して Ctrl+D する必要があり、存在しないパスは `refresh_bookmarks` で消えるまで残る。検索語の履歴は機微な文字列になり得るがクリア手段がない。 | プライバシー（検索語の除去）とリストの衛生状態。誤登録の手数が大幅に減る。 | M | 新規 | 高 |

### r4-cfg-05 ファイル名検索の上限 2000 件／深さ 24 が固定

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-cfg-05 | 名前検索の `max_results: 2000` / `max_depth: 24` を利用者が調整できない | 機能追加案 | `app/src/search.rs:71-78`（既定値）, `app/src/main.rs:3724-3728`（`..Default::default()` でそのまま利用）, `app/src/main.rs:5883-5888`（`per_root_cap` は 2000 を分配するだけ）, `app/README.md:189-192` | 大きなリポジトリや深いツリーでは 2000 件／深さ 24 で打ち切られるが、設定項目も到達通知もない（`main.rs` に「上限」の表示なし、同語のヒットはテスト行のみ）。 | 対象ディレクトリに応じて深さ・件数を上げたいという明確な要求に対応。打ち切りを可視化するだけでも誤解が減る。 | M | 新規 | 中 |

### r4-cfg-06 内容検索の上限 500 件／1 MiB／深さ 24 が固定

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-cfg-06 | 内容検索の 500 件・1 MiB ファイル上限が設定不可 | 機能追加案 | `app/src/search.rs:268-276`（`max_results: 500`, `max_depth: 24`, `max_file_size: 1024 * 1024`）, `app/src/main.rs:3734-3738`（既定値を利用）, `app/README.md:200-203` | ログや巨大なソースファイルが 1 MiB しきい値で丸ごと検索対象外になる。名前検索（2000 件）との上限差も説明されない。 | 「このファイルだけ検索したい」という用途を救済。1 MiB 制限は安全上の意図があるため、上限を上げる場合の明示的な承認と組み合わせる。 | S | 新規 | 高 |

### r4-cfg-07 設定の export / import / reset 手段が無い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-cfg-07 | 設定の書き出し・読み込み・初期化（reset）ができない | 機能追加案 | `app/src/settings.rs:99-147,161-279`（公開 API は config_path / record_* / toggle_bookmark / load / load_from / save / save_to / validate / migrate のみ）, `app/src/main.rs` に reset/export/import の呼び出しなし（`app/src` 全体を grep しても該当関数なし）, `app/README.md:149-152` | 壊れた JSON は `.broken-*` に退避されるが UI から復元できず、マシン間でブックマークや表示設定を移行する手段もない。初期化したい場合も手動でファイルを消す必要がある。 | 環境移行・バックアップ・「設定をリセットして再起動」という典型的な修復手順を提供。 | M | 新規 | 高 |

### r4-cfg-08 表示設定がタブ単位ではなくグローバル

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-cfg-08 | `sort` / `show_hidden` / `view` が全タブ共通で、タブごとに保持されない | 調査項目 | `app/src/settings.rs:57-59`（3 項目とも Settings 直下）, `app/src/tabs.rs:11-14`（Tab は cwd と history のみ）, `app/src/main.rs:1085-1086`（保存するのはタブのパスのみ）, `:821-822`（描画は常にグローバル値を参照） | 写真フォルダはアイコン、ソースツリーは詳細、という使い分けがタブ切替で維持されない。タブ構成と履歴は永続化されているため、表示設定だけが不揃いなのは設計上も不自然。 | ファイルマネージャとしての定番要求（フォルダごとの表示記憶）に対応しうる。ただし設定スキーマ v2 と移行を伴うため要可否判断。 | L | 新規 | 中 |

### r4-cfg-09 列幅・グリッドセル幅がハードコードで永続化されない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-cfg-09 | 詳細ビューの列幅（90/90/150px）とグリッドのセル幅（108px）が固定 | 機能追加案 | `app/src/main.rs:4293-4297`（`sort_header` の幅指定）, `:4608`（グリッドセル `.w(px(108.0))`）, `:84`（`GRID_CELL_WIDTH_PX = 108.0`）, `:5427-5429`（`grid_columns` への受け渡し）, `app/src/model.rs:331`；`app/README.md:188`（1 セル 108px 基準と明記） | 長いファイル名や更新日時は `.truncate()` で切れるが、列を広げる操作も幅を保存する手段もない。表示モード（`view`）だけが永続化されている。a11y レンズが列幅の rem 化に言及しているが、ユーザー調整・永続化の観点は未扱い。 | 長いファイル名の可読性と、情報量／密度の好みへの対応。ドラッグリサイズ + settings.json 保存で実現可能。 | M | 既知だが未対応 | 中 |

### r4-cfg-10 プレビュー上限（テキスト 256 KiB／画像 8 MiB）が固定

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-cfg-10 | テキストプレビュー 256 KiB・画像プレビュー 8 MiB の上限を変更できない | 調査項目 | `app/src/preview.rs:19-22`（`MAX_PREVIEW_BYTES` / `MAX_IMAGE_BYTES`）, `app/src/main.rs:2587,2591`（読み込み時に定数を利用）, `:2673-2679`（`truncated` 表示は既に実装） | 切り詰め自体は UI に表示されるため誤解は少ないが、大きなログや高解像度画像を全体見したい場合は設定ファイルの手編集も効かない（定数のため再ビルドが必要）。 | 中程度。上限到達表示がある分、優先度は検索上限より低い。設定化するならプレビューダイアログからの「もっと読む」の方が自然。 | S | 新規 | 中 |

### r4-cfg-11 監視・操作の時間定数は選択的な設定化にとどめるべき

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-cfg-11 | debounce 400ms / reconcile 10s / 自己操作抑制 1.5s は設定候補、undo 20・履歴 200・進捗 80ms は据え置き推奨 | 調査項目 | `app/src/watcher.rs:76-77`（`DEBOUNCE = 400ms`）, `app/src/main.rs:74-77`（`RECONCILE_INTERVAL = 10s`, `SELF_OP_SUPPRESS = 1500ms`, `PROGRESS_THROTTLE = 80ms`）, `app/src/undo.rs:18-19`（`MAX_UNDO = 20`）, `app/src/history.rs:29-35`（`max_back = 200`）, `app/README.md:143-146` | ネットワーク共有や遅い FS では 400ms debounce / 10s reconcile が実感に合わないことがあるため、この 2 つは設定候補。一方、進捗スロットルは内部チューニング、undo 20・履歴 200 は公開しても効果が薄くメモリ上のリスクだけが増えるため、むやみに定数を露出しない。 | 共有ドライブ利用者の「更新が遅い／反映されない」不満を解消しうる。設定項目を増やしすぎない取舍も含めた判断が必要。 | S | 新規 | 中 |

## レンズ総評

設定サーフェスの最大の問題は「値が足りないこと」ではなく「値に到達する導線がないこと」である。
`Settings` は 10 フィールドを持つが、実装された UI はツールバー 3 種（表示切替・隠し切替・
列ヘッダのソート）とコンテキスト操作のみで、`DESIGN.md:117` が描いた設定 affordance は
実装されていない（r4-cfg-01）。これを基盤として整備すれば、検索上限（r4-cfg-05/06）、
列幅（r4-cfg-09）、監視間隔（r4-cfg-11）の設定化も同じ枠組みに載せられる。

即座に潰すべきは低コストの不整合である。`dirs_first` は保存されているのに
`SortSpec::order()` で捨てられ、並び順に一切影響しない（r4-cfg-02）。検索履歴は
10 件保存して 8 件しか表示しない（r4-cfg-03）。この 2 件は effort S〜M で
確度が高く、PASS 6 の統合では最優先で採用すべき候補だ。

次点は「保存されたデータのユーザー支配」である。ブックマーク・最近の場所・検索履歴に
個別削除がなく（r4-cfg-04）、export/import/reset もない（r4-cfg-07）。検索語は機微な
文字列になり得るため、クリア手段の欠如は利便性以上の問題を含み、設定ダイアログと
同一の作業で解消できる。

逆に、内部定数を全部設定に出すのは得策ではない。進捗スロットル 80ms や undo 20 件
などは利用者の判断が入る余地が小さく、設定面を肥大させるだけである（r4-cfg-11）。
タブ単位の表示設定（r4-cfg-08）はユーザー価値が高い一方でスキーマ変更を伴うため、
PASS 6 では「設定ダイアログの実装」と「スキーマ v2」をどのタイミングで結び付けるかという
設計判断として扱うのが妥当だ。
