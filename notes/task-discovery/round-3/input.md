---
type: Reference
title: "gpui-file-explorer 第3パス タスク発見: 入力・操作モデルの精緻化"
description: "DESIGN.md §3.6 / app/README.md キーバインド / app/src/main.rs のマウス・キーボードハンドラを精読し、入力操作モデルの細部に関する具体的な残タスク・機能追加案・調査項目を12件記録したレンズ別レポート。"
tags:
  - gpui-file-explorer
  - task-discovery
  - round-3
  - input
  - interaction-model
  - mouse
  - keyboard
generated: 2026-11-17T20:39:05+09:00
status: complete
---

## 概要

対象: `DESIGN.md:262-272`（マウス・その他）、`app/README.md:80-116`（キーバインド）、
`app/src/main.rs` の `on_key_down` / row click / `right_click` / `on_drag` / selection /
dialog close / focus 関連コード、`app/src/model.rs:174-280`（`Selection`）。
すべての file:line は実ファイルを読んで確認済み。既知の制限は「既知」と明記した。

### r3-input-01 検索ビューで Enter が選択結果を開かない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-input-01 | 検索ビューで Enter が結果を開かない（早期 return とフォーカス未移動） | 残タスク | `app/src/main.rs:1526-1531`, `:1611`, `:3603`, `:3629`, `:3745-3768`, `app/README.md:100` | `on_key_down` は `view == Search` のとき Escape 以外を無条件 `return` で握り潰すため、`:1611` の `enter → open_selected` に到達しない。検索完了後もフォーカスは検索入力のままで `focus_list_on_next_render` が立たない（立つのは `:2645/:2812/:2837/:3359` のみ）。結果行のダブルクリックは `:4364-4370` で開けるが、キーボードでは Enter/F2/Delete/Ctrl+C がすべて死ぬ。Search ビューでも `open_path` 系を許可し、検索完了時にリストへフォーカスを戻す必要がある。 | 高 | 小 | 高 | 高 |

### r3-input-02 空き領域クリックが選択解除とフォーカス取得を伴わない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-input-02 | 一覧の空き領域クリックで選択解除・フォーカス移動が起きない | 残タスク | `app/src/main.rs:5434-5455`, `:5457-5467`, `:4360-4371`, `:4555-4566`, `DESIGN.md:244`, `app/README.md:489`, `:113` | `uniform_list` と `list_area` には `on_drop` のみがあり `on_click` が無い。フォーカス取得と選択は行の `on_click`（`:4361`, `:4556`）でのみ行われるため、空き領域をクリックしても複数選択が残り、ルートへのフォーカス移動も起きない。空き領域のダブルクリックも「開く」動作を持たない（DESIGN §3.6 のダブルクリックは行対象）。空き領域の**右**クリックメニュー未対応は既知（`app/README.md:489`）だが、左クリックの選択解除は未記載のギャップ。空き領域クリックで `selection.clear()` + `focus_handle.focus()` を行うハンドラを追加する。 | 中 | 小 | 高 | 高 |

### r3-input-03 外部変更でフォーカス行が消えると先頭へ飛ぶ

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-input-03 | 監視による再読込で選択行消滅時のフォーカスが先頭に退避する | 残タスク | `app/src/main.rs:810-845`, `app/src/model.rs:267-279`, `DESIGN.md:593` | `rebuild_view` は `selection.retain(&paths)` で消えたパスを落とし、フォーカスは `keep` → 残存選択の先頭 → `unwrap_or(0)` の順で決定する（`:825-835`）。フォーカス行が外部で削除されると旧可視 index に近い前後ではなく常に index 0 へ移動する。DESIGN §5.3 の `reconcile` 契約「旧可視 index に近い前後のエントリへ移す」と不一致。近傍フォーカス復元と、選択が消えた旨のステータス通知を追加する。 | 中 | 中 | 中 | 高 |

### r3-input-04 ダイアログ・ヘルプ・履歴を閉じた際のフォーカス復帰が不統一

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-input-04 | オーバーレイ閉鎖時のリストへのフォーカス復帰が一部経路で欠落 | 構造改善 | `app/src/main.rs:2643-2646`, `:2810-2813`, `:2837`, `:3334-3337`, `:5287-5290`, `:1486-1497`, `:1504-1510`, `:2075-2090`, `:3054-3059`, `:3842-3854`, `DESIGN.md:244` | preview/location/rename は `focus_list_on_next_render = true` を立て、`:5287-5290` の `on_next_frame` でリストへ戻す。一方 properties（`:3054`）、search（`:3842`）、help（F1 トグル）、undo 履歴（閉じるボタン/背景クリック）はフラグを立てない。DESIGN §3.4 の「閉じたらリストへ戻す」が一部しか満たされず、矢印キー等が効かない状態が残りうる。全クローズ経路を同一ヘルパに集約する。 | 中 | 小 | 中 | 高 |

### r3-input-05 Ctrl+Shift+クリックが範囲置換になりアンカーも直感と異なる

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-input-05 | 修飾キー组合せ（Ctrl+Shift+クリック）の選択意味が Shift と同一 | 残タスク | `app/src/main.rs:1396-1424`, `app/src/model.rs:222-235`, `:238-258`, `app/README.md:85-86` | `select_with_modifiers` は `shift` を先に判定するため（`:1413`）Ctrl+Shift+クリックは Ctrl を無視して `select_range` を呼ぶ。`select_range` は `set_paths`（model.rs:257）で既存選択を置換するので「選択に範囲を追加」にならない。また `toggle` は解除時もアンカーを `paths.last()` に動かす（model.rs:228-234）ため、Ctrl+トグル後の Shift 範囲起点が最後に押した行へ移る。修飾キーごとの意味を明示化し、Ctrl+Shift は加算範囲にする。 | 中 | 中 | 中 | 中 |

### r3-input-06 Escape / F1 の階層がオーバーレイごとに異なる

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-input-06 | キー握り潰しの優先順が renaming と他オーバーレイで不一致 | 調査項目 | `app/src/main.rs:1480-1485`, `:1486-1497`, `:1498-1531`, `:1632`, `:4453`, `:4647`, `:5490` | renaming 中は `:1480-1485` が全キーを return するため F1 も無視されるが、location/undo/preview/properties/search 表示中は F1 判定（`:1486`）が先にあるためヘルプを開ける。ヘルプ表示中は Escape 以外が握り潰され、F1/ヘルプ→location 等の重なりで Escape が意図しない層を閉じる。さらに PopupMenu（`:4453`, `:4647`）表示中もルートの `on_key_down`（`:5490`）が残り、Delete 等（`:1632` は cancel_op）が同時発火しうる。Escape の優先順を単一の階層判定へ集約する。 | 中 | 中 | 中 | 中 |

### r3-input-07 click_count >= 2 とキーリピートで「開く」が再発火

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-input-07 | 三連クリック・Enter 押下継続で open_with_system / 移動が複数回走る | 残タスク | `app/src/main.rs:4364-4370`, `:4559-4565`, `:1611`, `app/src/main.rs` 全体に `is_repeat` 無し（grep 確認） | ダブルクリック判定が `event.click_count() >= 2` なので三連クリック以上でも `open_index → open_path` が再実行され、ファイルなら `cx.open_with_system`（`:1371`）が複数回呼ばれる。キー側も `event.is_repeat` のガードが無く、Enter 押下継続で `open_selected` が繰り返し走り、フォルダ移動が連鎖する。`click_count() == 2` への固定とリピート入力の無視が必要。 | 中 | 小 | 高 | 中 |

### r3-input-08 行ホバーの視覚変化と省略名ツールチップ

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-input-08 | 選択行ホバーが無変化／フォーカスとホバーが同色／省略名にツールチップ無し | 残タスク（ツールチップは既知 P1） | `app/src/main.rs:4421-4435`, `:4461`, `:4615-4629`, `:4658`, `DESIGN.md:140`, `:848`, GPUI `gpui-pre-0.3.12/src/elements/div.rs:53` | 行背景は selected=`C_OVERLAY`、focused=`0x252537`、hover=`selected ? C_OVERLAY : 0x252537`。選択行にホバーしても変化が無く、フォーカス行とホバー行は同色で区別できない（DESIGN §8.4 の「選択=背景・フォーカス=枠線」とも不一致）。さらに行名/グリッド名は `.truncate()` のみで `.tooltip()` が無く、省略された末尾を読めない。ツールチップは DESIGN:140 で P1 と明記された既知の未実装で、GPUI 側に 500 ms の表示遅延定数があるため省略時のみ完全名を出せる。 | 中 | 小 | 中 | 高 |

### r3-input-09 中クリックが未配線

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-input-09 | 中クリックの新タブ動作が未実装（既知: 何もしない / P2） | 機能追加案（既知） | `DESIGN.md:265`, `app/src/main.rs` に `MouseButton::Middle` 無し（grep 確認。使用は `:2086` Left と `:4437`/`:4631` Right のみ） | 中クリックは DESIGN 上「何もしない（P2 で新タブ）」と既知。アプリ側に Middle ハンドラが無いため、行・空き領域を問わず初期状態のままになる。P2 を実装するなら行の中クリックで `entry.path` の親ディレクトリを `new_tab` する配線が必要（現在の `new_tab` は現在のディレクトリのみ）。 | 中 | 小 | 低 | 高 |

### r3-input-10 サイドバーにホイールスクロール経路が無い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-input-10 | ブックマーク・最近・ドライブが増えるとサイドバー下部が到達不能 | 残タスク | `app/src/main.rs:4665-4674`, `:4729-4765`, `:4766-4803`, `:4804-`, `:5434-5455`, `app/README.md:241` | 一覧/グリッドは `uniform_list` + `scroll_handle` でスクロールされるが、サイドバーは `h_full()` の div に項目を積むだけで overflow / track_scroll / on_scroll 指定が無い（`on_scroll` は app/src 全体で 0 件、`overflow_y_scroll` はダイアログの `:2743`/`:2904` のみ）。ブックマーク最大 50 件 + 最近 10 件 + ドライブで 30 px 行が領域を超えると下部がクリップされ、ホイールでも到達できない。スクロール可能なサイドバーコンテナへの変更が必要。 | 中 | 中 | 中 | 高 |

### r3-input-11 ドラッグ開始閾値が 2 px 固定で誤ドラッグがクリックを相殺

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-input-11 | 3 px の位置ブレで選択が確定せず選択全体のドラッグが始まる | 調査項目 | `app/src/main.rs:4438-4445`, `:4632-4639`, GPUI `gpui-pre-0.3.12/src/elements/div.rs:52` (`DRAG_THRESHOLD: f64 = 2.`), `:3040-3046` (2 px 超で `ElementClickedState::default()` にリセットしてドラッグ開始) | 行の `.on_drag(drag_paths, ...)` にアプリ側の閾値指定は無く、GPUI コアの 2 px 固定閾値で発火する。発火時に clicked_state がリセットされるため、選択のつもりで 3 px 動かすと行の `on_click`（`:4360`/`:4555`）が走らず、複数選択全体を積んだドラッグ画像が出る。DRAG_THRESHOLD は定数で設定手段が無いため、実機再現のうえ受容可能な挙動か判断し、必要ならアプリ側のジェスチャ制御または既知の制限として明記する。 | 中 | 中 | 高 | 高 |

### r3-input-12 クリックでのリネーム開始が無く F2 / メニューのみ

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-input-12 | 行ラベル click-to-rename が無く F2 とコンテキストメニューに依存（既知） | 機能追加案（既知の挙動） | `app/src/main.rs:1615-1622`, `:3235`, `:4177-4183`, `:5285`, `:4360-4371`, `:4555-4566`, `app/README.md:377-378` | `start_rename` の呼び出しは F2（`:1620`）、コンテキストメニュー「名前変更」（`:4181`）、デバッグフック（`:5285`）の 3 経路のみ。行クリックは選択と `click_count >= 2` の「開く」だけで、ゆっくり二重クリック等のクリック操作では改名できない。README 既知の制限「リネームはフォーカス行（最後にクリックした行）にのみ適用」は維持しつつ、ラベル部分のクリックで `start_rename(window, cx)` を呼ぶ導線を追加できるかが検討課題。 | 中 | 小 | 中 | 高 |

## レンズ総評

- 入力モデルそのものの不整合として優先度が高いのは **r3-input-01（検索ビューの Enter 死）**、**r3-input-02（空き領域クリック）**、**r3-input-04（フォーカス復帰の不統一）**。いずれも「キー/クリックが届かない」という具体的な挙動で、DESIGN §3.4 のフォーカス規約との差分が明確。
- **r3-input-03 / r3-input-05 / r3-input-06 / r3-input-07 / r3-input-11** は、既存実装は動くものの状態遷移の細部（消えた行の退避先、修飾キーの意味、Escape の階層、連続入力、ドラッグ閾値）が設計意図とずれる类型。再現手順と期待値をテスト仕様に落とし込む段階にある。
- **r3-input-08 / r3-input-09 / r3-input-12** は DESIGN / README で既知と明記された項目で、新規発見というより P1/P2 の着手候補。ツールチップは GPUI 側に表示遅延定数があり実装コストが低い。
- 確認できた既存動作: 行の右クリックは既選択行なら複数選択を保持しつつ `selected` だけを更新する（`app/src/main.rs:4372-4386`, `:4567-4579`）。DESIGN:266 の「行の右クリックは対象行を選択してからメニュー」は満たされており、今回は残タスクとして計上しない。ただし `select_single_index`（`:1388-1391`）と違い `scroll_to_item` を呼ばないため、キーボードカーソルが画面外に残る点は軽微な洗練余地として記録する。
- 本レンズではドラッグ受信/送信の基本、ラバーバンド、E2E 合成入力の欠如、ショートカット一覧、D&D の E2E ギャップは既パス扱いとして重複を避けた。
