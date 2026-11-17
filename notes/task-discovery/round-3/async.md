---
type: Reference
title: 非同期・並行・キャンセル正確性の再レビュー（ラウンド3・非同期レンズ）
description: >-
  DESIGN.md §4.4-4.5 のスレッドモデルと世代キャンセルを基準に、
  app/src/main.rs / ops.rs / search.rs の非同期経路を実読し、
  古い結果の上書き・キャンセル未伝播・ガード不全を file:line 根拠付きで列挙した調査項目一覧。
tags:
  - task-discovery
  - gpui-file-explorer
  - async
  - concurrency
  - cancellation
  - round-3
generated:
  by: "r3-async"
  at: "2026-11-17T20:38:19+09:00"
status: complete
---

## 発見一覧

対象は PASS 1-2 で既出の項目（100k 性能 / 進捗チャネル制御 / DirWatcher Drop join / watcher 回復・normalize_events / FsEvent 差分適用 / 10 秒 reconcile 自体 / 自己操作抑制 / open・reveal 失敗 / 設定・配布・a11y・セキュリティ / 各種 UX 機能）を意図的に除外し、
**非同期の安全性・キャンセル伝播・結果の新鮮性**に絞った。全項目を実読した `file:line` に基づく。
競合をコード上証明できる項目は「残タスク」、タイミング依存や設計意図の確認が必要な項目は「調査項目」として過大評価を避けた。

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-async-01 | reconcile が並走し古いスナップショットが新しい状態を上書きしうる | 残タスク | `app/src/main.rs:924-961`, `app/src/main.rs:1047-1056`, `app/src/main.rs:807-815`, `app/src/main.rs:74` | `reconcile()` は背景読み込み付き `cx.spawn` を `.detach()` するだけでハンドルを保持せず（960）、世代も新規採番しない（930）。10 秒ごとのループ（1050）から次回が起動したとき前回の `read_entries` がまだ完了していなければ、同一 generation で両方のコールバックが世代チェック（950）を通過する。到着順が逆なら古いスナップショットが `apply_loaded`（812 で `all_entries` を無条件置換）を実行し、新しい内容を抹消する。巨大ディレクトリで読み込みが 10 秒を超えると顕在化する。修正: reconcile 専用 Task ハンドルを保持し採用前に `take()`、または呼び出しごとの単調トークンをコールバックで照合。 | 中高 | 小 | 高 | 中高 |
| r3-async-02 | コピー前の `total_bytes` 再帰ウォークに CancelFlag が伝播しない | 残タスク | `app/src/ops.rs:506-508`, `app/src/ops.rs:557-574`, `app/src/main.rs:1879-1887`, `app/src/main.rs:1836-1837`, `app/src/ops.rs:523-535` | コピー/移動の背景タスクは `ops::total_bytes(&paths)`（1880）をまず実行するが、この関数と `total_bytes_of` は `CancelFlag` を引数に取らず（ops.rs:506/557）、再帰中にキャンセルを確認しない。対照的に `dir_stats` はエントリごとに `cancel.is_cancelled()` を確認する（ops.rs:527/534）。Esc で `op_cancel` を立てても（1836-1837）、ループ先頭の確認（1887）に到達するまで巨大ツリーの全走査が続き、キャンセルレイテンシが最小化されない。修正: `total_bytes`/`total_bytes_of` に `&CancelFlag` を追加し `dir_stats` と同じ間隔で確認、呼び出し側（1880）へ伝播。 | 中高 | 小 | 中 | 高 |
| r3-async-03 | 検索突入直前に進行中だったディレクトリロードが検索結果一覧を上書きする | 残タスク | `app/src/main.rs:3599-3646`, `app/src/main.rs:652-707`, `app/src/main.rs:3762`, `app/src/main.rs:807-815` | `reload()` が背景読み込みを開始した直後に Ctrl+F で `open_search()` すると、`view = Search`（3641）になるだけで `self.generation` も `load_task` も触られない。読み込み完了コールバックは `this.load.generation() == Some(gen)`（679）のみを見るため通過し、`apply_loaded` が `all_entries` をディレクトリ内容で置換する。検索結果も同じ `all_entries`（3762）に格納されるため、検索開始前に一覧が混入する。修正: `open_search` で `load_task.take()` と generation 採番、またはロード完了コールバックに `view != Search` ガードを追加。 | 中 | 小 | 高 | 中高 |
| r3-async-04 | ゴミ箱確認ダイアログ由来の削除が op_task / op_cancel ガードを迂回する | 残タスク | `app/src/main.rs:3460-3594`, `app/src/main.rs:3505-3514`, `app/src/main.rs:3532-3593`, `app/src/main.rs:2009-2013`, `app/src/main.rs:1772-1776`, `app/src/main.rs:979` | `paste`（1772）・外部ドロップ（2009）・undo/redo は `op_task.is_some()` で排他するが、`trash_paths` は確認待ち（`window.prompt` の receiver.await、3505-3514）を含む背景削除なのにガードも `op_task` 設定も `op_cancel` 設定もしない。よって削除の進行中に paste 等を開始でき、完了コールバック（3532-3593）が `all_entries` を更新し `reload()`（3588）で世代を進めるため、並走する他操作の状態と競合する。watch ループは `op_cancel.is_some()`（979）で監視を抑制するがこの経路では抑制されない。修正: 同じ `op_task`/`op_cancel` パイプラインに載せるか、専用のビジーフラグと世代スナップショットを導入。 | 高 | 中 | 高 | 中高 |
| r3-async-05 | undo/redo の「操作中」ガードが実効的に機能しない | 構造改善 | `app/src/main.rs:2290-2295`, `app/src/main.rs:2399-2404`, `app/src/main.rs:2470-2481`, `app/src/main.rs:2481-2534`, `app/src/main.rs:1982` | `undo_last`/`redo_last` は開始時に `op_task.is_some()` を確認する（2291/2400）が、undo/redo 自身は `op_task` を設定しない（設定は paste の完了ハンドラ 1982 のみ）。従って Ctrl+Z を連打すると背景の trash/重命名タスクが並走し、それぞれが undo スタック（2493/2511）・status・`reload()`（2529）を変更する。ガードと操作のライフタイムが対応していない設計不整合。修正: undo/redo 用の専用ビジー `Task` を保持してガードを共通化、または全ミューテーション操作が増減する in-flight カウンタを導入。 | 中高 | 中 | 高 | 高 |
| r3-async-06 | フォルダ作成失敗コールバックに cwd / 世代ガードがなく別ディレクトリに誤表示される | 残タスク | `app/src/main.rs:1664-1701`, `app/src/main.rs:1680-1689`, `app/src/main.rs:1691-1696`, `app/src/main.rs:421-424` | 成功時は `this.cwd == expected_dir`（1680）を確認してから表示・`reload` するが、失敗時は無条件に `LoadState::Error { generation: this.generation }`（1692-1695）を書き込む。A ディレクトリで作成開始後、B へ移動してから失敗すると B の表示状態がエラーに置き換わる。さらに B のロードが進行中だと Error の generation が B の世代と一致し、ロード完了の世代チェック（679）を通過してしまう。修正: 失敗時も `expected_dir` を照合し、操作エラーは status のみに記録して `load` 状態を汚さない。 | 中 | 小 | 中 | 高 |
| r3-async-07 | `open_search` は世代を進めるだけで旧検索タスクをキャンセルしない | 調査項目 | `app/src/main.rs:3599-3646`, `app/src/main.rs:3640`, `app/src/main.rs:3679-3684`, `app/src/main.rs:3673-3675`, `app/src/main.rs:3824-3828` | `run_search` は旧 cancel を立て Task を `take()` する（3679-3684）、`clear_search` も同様（3825-3828）。一方 `open_search` は `search.generation` を進めるだけで（3640）`search.cancel` / `search.task` を触らない。通常は検索ビュー離脱時に `clear_search` を通るため発火経路は限定的だが、何らかの経路で検索実行中に Directory から再オープンされると、旧ウォークが完了まで走り続けコールバックは世代ガード（3748）で捨てられるだけ。古い結果の UI 適用は起きないが CPU とスロットが無駄になる。修正: 世代更新と同時に cancel + take する共通ヘルパーへ集約。 | 低中 | 小 | 中 | 中 |
| r3-async-08 | 完了ハンドラが op_task / op_cancel を無条件にクリアし他操作を巻き込む | 調査項目 | `app/src/main.rs:1937-1942`, `app/src/main.rs:1836-1837`, `app/src/main.rs:1982`, `app/src/main.rs:3505-3514` | paste の完了コールバックは `op_task = None; op_cancel = None; op_progress = None`（1940-1942）を無条件に実行する。ゴミ箱確認経路（r3-async-04）や undo/redo が先に/同時に状態を設定していた場合、paste の完了がそれらを消し去り、進行中操作の Esc キャンセルと watch 抑制が効かなくなる。現行のガードでは paste と trash が排他されていないため到達可能性はあるが、実際の操作手順での再現は未検証。修正: 操作ごとのトークンを保持し、自分のトークンと一致するときだけクリア。 | 中 | 中 | 高 | 中 |
| r3-async-09 | ディレクトリサイズ結果にダイアログ同一性トークンがない | 調査項目 | `app/src/main.rs:3066-3099`, `app/src/main.rs:3086-3095`, `app/src/main.rs:3054-3063`, `app/src/main.rs:2981-3039` | `start_dir_size` の完了コールバックは現在の `this.properties`（3087）へ `dir_size` / `size_loading` / `size_task` を書き込むが、どの path の集計かを照合しない。安全は `PropertiesState` が `size_task` を保持し、ダイアログ差し替え・クローズ（3062）で Task が drop されることに依存している。GPUI の Task drop セマンティクスを確認した上で、もしキャンセルフラグ（3060）だけが効いて Task が残る経路があれば、古い A の統計が新しい B のダイアログへ流入する。まずは Task drop による確実なキャンセルを検証し、不足していれば path/生成 ID をコールバックに捕捉する。 | 中 | 小 | 中 | 低中 |
| r3-async-10 | 進捗ポンプが `op_cancel.is_none()` を「操作決着」の代用にしている | 調査項目 | `app/src/main.rs:1844-1873`, `app/src/main.rs:1857-1869`, `app/src/main.rs:1836-1837`, `app/src/main.rs:1940-1942`, `app/src/main.rs:3473-3514` | 進捗ポンプは `this.op_cancel.is_none()`（1860）で「操作中」を判定し、false なら最終 status を上書きしないよう捨てる。このセンチネルは paste が設定・解除する前提だが、trash や undo/redo は `op_cancel` を設定しない（r3-async-04/05）。そのため他操作が走っている間の進捗が誤採用/誤棄却される可能性がある。生成コード上は「チャネルが閉じればポンプも終わる」ためリークではないが、センチネルと操作ライフタイムの対応は明文化されていない。明示的な `operation_id` と完了フラグによる判定へ置換するか、現行挙動をテストで固定する。 | 中 | 中 | 中 | 中 |

## レンズ総評

DESIGN.md §4.4-4.5（`:390-436`）が要求する「世代を進める・旧 Task を drop・完了時に世代照合」は `reload`（`:652-707`）と `run_search`（`:3663-3771`）では概ね実装されており、
検索の 3 層ガード（共同キャンセルフラグ / Task drop / 世代照合）は模範的である。
しかしその規律が **reconcile・ゴミ箱確認・undo/redo・フォルダ作成エラー・ディレクトリサイズ** といった他経路へ波及しておらず、
「世代を進めるだけキャンセルしない」「`op_task` が一部操作しか設定しない」「完了ハンドラがグローバル状態を無条件クリア」という 3 つのパターンが競合の温床になっている。

特に優先すべきは r3-async-04（trash の排他バイパス）と r3-async-05（undo/redo ガードの空転）で、いずれも既存の `op_task` という仕組みが「paste 専用」に退化していることが根本原因である。
次点で r3-async-01（reconcile の並走）と r3-async-02（`total_bytes` のキャンセル未伝播）は、巨大ディレクトリ環境で顕在化しやすい。
r3-async-03/06 は「ユーザーが移動した後に古いコールバックが状態を触る」典型で、修正コストが小さく費用対効果が高い。

証明度については、r3-async-01/03/04/05/06/02 はコード上で競合経路を特定できたが、実際のタイミング再現（特に 10 秒を超える読み込みと prompt の待ち挙動）は未実施である。
r3-async-07/08/09/10 は設計依存の推論を含むため「調査項目」とし、まずは再現テストまたは GPUI Task セマンティクスの確認から着手するのが妥当である。
