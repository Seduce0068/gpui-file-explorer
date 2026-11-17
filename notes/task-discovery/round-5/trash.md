---
type: Reference
title: "ゴミ箱サブシステム内部監査（pass 5）"
description: >-
  trash_view.rs と main.rs / trash 5.3.0 のゴミ箱 UI 統合を実コード行単位で監査し、
  残タスク・機能追加案・調査項目を抽出した Final Pass レンズ結果。
tags:
  - task-discovery
  - round-5
  - trash
  - gpui-file-explorer
  - rust
generated:
  by: "r5-trash"
  at: "2026-11-17T21:26:18+09:00"
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 発見一覧

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-trash-01 | 同一ファイルを二重にゴミ箱へ入れると復元が全件失敗する | 残タスク | `trash_view.rs:56-79,69`; trash 5.3.0 `lib.rs:563-573,622-628` | 行は trash ID を `path` として保持するので選択は ID 単位で正しい。しかし同名・同 `original_path` の 2 行を同時選択して復元すると、`restore_all` が `Error::RestoreTwins` を返し**1 件も復元されない**。UI は両行を同じ名前で表示するため利用者は区別できず、理由も文字列化で不明。Twins を検出して片方のみ復元するか、選択時に警告するタスク。 | データ消失感の回避・誤操作防止 | S | 新規 | 高 |
| r5-trash-02 | 元の場所（original_parent）が一覧・ステータスのどこにも出ない | 機能追加案 | `trash_view.rs:65-74`; `main.rs:4463-4495,5050-5054`; trash `lib.rs:359-365,376-378` | `entries_from_items` は `original_parent` を破棄し、`Entry` に元パス格納欄もない。行は名前・「—」・「-」・削除日時のみ、ステータスも「選択: 名前（削除: 日時）」。復元先が不明なため、同名ファイル偶然一致や二重削除の判別が不能。元パスを保持して詳細行/プロパティに表示するタスク。 | 復元先の透明化・誤復元防止 | M | 新規 | 高 |
| r5-trash-03 | 削除日時の欠損表現と列見出し「更新日時」の意味不一致 | 残タスク | `trash_view.rs:60-64`; `model.rs:628-634,363`; `main.rs:4494,5051`; freedesktop `freedesktop.rs:212` | `time_deleted < 0` を `None` に写し、`human_datetime(None)` は `-`。Linux で `.trashinfo` に `DeletionDate` が無いと `-1` が入るため「削除: -」と表示される。一方ヘッダは「更新日時」で、値は削除時刻。欠損時は「日時不明」と明示し、ゴミ箱ビューでは見出しを「削除日時」に切替えるタスク。 | 表示の正確性・誤解防止 | S | 新規 | 高 |
| r5-trash-04 | ゴミ箱ビューでソートヘッダが無効（設定だけが変わる） | 残タスク | `main.rs:5422-5426,4274-4277,1654-1660,4281-4298,750-780`; `trash_view.rs:77` | 詳細表示では常に `sort_header` を描画し、クリックは `set_sort` → `rebuild_view` を呼ぶ。しかしゴミ箱は `apply_trash_loaded` が `entries_from_items` の newer-first 固定を直接適用し `rebuild_view` を通らないため、並びは変わらず矢印と保存設定だけが変わる。検索/隠しフィルタも同経路でバイパスされる。ヘッダを無効化するか SortSpec を適用するタスク。 | UI の嘘を除去・設定の整合 | S | 新規 | 高 |
| r5-trash-05 | 復元先の親ディレクトリが削除済みの場合の代替先が無い | 残タスク | `trash_view.rs:25-27`; `main.rs:4034-4054`; trash `lib.rs:376-378` | `restore_all` は `original_parent` へ戻すため、親が消えていれば失敗する。現状は `Err` を `to_string()` して「復元できません: …」と出すのみで、元ディレクトリを再作成するか別の場所へ戻す手段が無い。事前に親の存在を確認し「元ディレクトリを再作成 / 別の場所を選ぶ / 中止」を提示するタスク。 | 復元成功率の向上 | M | 新規 | 高 |
| r5-trash-06 | 復元衝突の structured error を捨てており、 precise next action が実装できない | 既知だが未対応 | `trash_view.rs:25-27`; `main.rs:4052-4055`; trash `lib.rs:244-261,589-600`; `app/README.md:419` | 既知「衝突時は失敗し項目はゴミ箱に残る」のクローズアクション。crate は `Error::RestoreCollision { path, remaining_items }` と再試行例を提供するが、`to_string()` で詳細を失う。戻り値を `RestoreOutcome { restored, collision, twins, other }` 等に変え、衝突パス名と「名前変更して復元 / スキップして残りを復元 / 中止」を提示し `remaining_items` で再試行する実装が正確な完了条件。 | 既知制限の実質解消 | M | 既知だが未対応 | 高 |
| r5-trash-07 | バッチ復元の部分成功が集計されず成功件数も精度が無い | 残タスク | `main.rs:4045,4046,4048,4052-4056`; trash `lib.rs:244-247` | 成功メッセージの `count` は復元前の選択件数。crate 仕様上は衝突で途中停止し「復元できた件数 / 残った件数」が得られるのに、Err 時はエラー文字列のみで部分成功が隠れる。`RestoreOutcome` を集計し「3 件復元、1 件衝突で残留」と表示するタスク（r5-trash-06 と同じ戻り値変更を共用）。 | 操作結果の正確な通知 | S | 新規 | 高 |
| r5-trash-08 | 他アプリのゴミ箱変更が自動反映されない（現状は手動再読込のみ） | 既知だが未対応 | `main.rs:5153-5157,711-746`; `fs-cross-platform-notes.md:257` | ステータスに「ゴミ箱（自動更新なし）」と明記済み。クローズアクション: `trash::os_limited::trash_folders()` で実ゴミ箱ディレクトリを取得し、`notify` 監視または 10 秒ポーリングで `reload_trash` を発火（既存の generation ガードと `LoadState::Loading` を再利用、自己操作は抑制）。 | 他アプリ連携・表示の鮮度 | M | 既知(UI文言)だが未対応 | 高 |
| r5-trash-09 | `trash::list` エラーが文字列化され、原因別の案内と再試行が無い | 構造改善 | `trash_view.rs:17-20`; `main.rs:729-739,4300-4335,5067-5071`; trash `lib.rs:275-278` | `list()` は `error.to_string()`、crate の Display は `Error during a \`trash\` operation: {self:?}`。ゴミ箱ディレクトリ不在・権限不足・portal 無効等が区別できず、「ゴミ箱を読み込めません: …」と一括り。構造化エラー + 「再読み込み」ボタン + 既知 Final 一覧の保持を追加するタスク。 | 診断性・回復手段 | M | 新規 | 高 |
| r5-trash-10 | macOS では `trash::os_limited` が存在せずビルド不能 | 残タスク | `trash_view.rs:19,26,49`; trash `lib.rs:426-430`; `app/Cargo.toml:18`; `README.md:84`; `fs-cross-platform-notes.md:259` | crate 5.3.0 の `os_limited` は Windows と freedesktop 系 Unix のみ cfg 公開。macOS モジュールには `list`/`restore_all` が無く、現状の 3 呼び出しは macOS ターゲットでコンパイルできない（Windows/macOS 実機未検証と README に記載）。`#[cfg]` で除外して「macOS ではゴミ箱ビュー未提供」と明示するか、代替 API/クレート選定の ADR を残すタスク。 | クロスプラットフォーム整合 | M | 新規 | 高 |
| r5-trash-11 | サイズ「-」固定を解消する具体的クローズアクション（メタデータ API 未活用） | 既知だが未対応 | `trash_view.rs:72`; `main.rs:4482-4486`; `app/README.md:418`; trash `lib.rs:392-424,512`; `fs-cross-platform-notes.md:257` | 既知「サイズは '-' 固定」のクローズアクション: `trash::os_limited::metadata(&item)` の `TrashItemSize::Bytes(n)` を `Entry.size` に設定し `human_size` 表示、`Entries(n)` は「n 項目」と表示。同期 I/O になるため一覧取得時にバックグラウンドで一括取得し、失敗時のみ '-' にフォールバックする。 | 情報量の向上・既知制限の解消 | S | 既知だが未対応 | 高 |
| r5-trash-12 | trash ID を lossy 文字列化して照合しており非 UTF-8 で誤选出の恐れ | 調査項目 | `trash_view.rs:38-45,69`; trash `lib.rs:346-353` | 選択は `PathBuf::from(&item.id)` で OsString を保持するが、`restore_by_id` は `item.id.to_string_lossy() == id.as_str()` で比較。Linux の非 UTF-8 `.trashinfo` パスでは異なる ID が同じ U+FFFD 文字列に潰れ得る。ID を `OsString` のまま保持/比較するよう型を見直す（Undo 記録形式との整合も要確認）タスク。 | 非 UTF-8 環境での正確性 | M | 新規 | 中 |

## レンズ総評

`trash_view.rs` は「ID を行キーにする」設計自体は堅牢で、通常の選択・復元は正しく動く。
問題はその外側に集中している。(1) trash crate が返す構造化エラー
（`RestoreCollision` / `RestoreTwins`）を `to_string()` で捨てるため、既知の衝突制限と
二重ゴミ箱化の Twins 失敗に「次に何をすべきか」が存在しないこと、
(2) `original_parent` とサイズを `Entry` へ写さず、復元先と付加情報を利用者に見せないこと、
(3) ゴミ箱が独自の `apply_trash_loaded` を持つためソート設定・検索が効かないのに
ヘッダだけが操作可能で、UI が実際と矛盾すること、
(4) `os_limited` の cfg 制約で macOS がビルド不能なこと、が特に重要である。

価格対効果が高いのは r5-trash-06 / r5-trash-07 の `RestoreOutcome` 共通化と、
r5-trash-02 / r5-trash-11 のメタデータ保持であり、いずれも既存の
`trash::metadata` / `RestoreCollision.remaining_items` を「繋ぐ」だけの変更で完結する。
r5-trash-10 は設計判断（macOS 方針）を伴うため着手前に ADR 化を推奨する。
既知として扱った 3 件（06 / 08 / 11）はいずれも「現状説明」で終わらせず、
上表に具体的な完了条件を記載した。
