---
type: Reference
title: 状態機械・型契約の整合性監査（PASS 3）
description: >-
  DESIGN.md §4.6/§5 の契約と model.rs / history.rs / tabs.rs / undo.rs /
  trash_view.rs / watcher.rs / main.rs の実装を突き合わせ、状態遷移・世代ガード・
  選択キー・履歴スタック・タブ状態の具体的な不整合をタスク化した調査。
tags:
  - task-discovery
  - state-machine
  - type-contract
  - round-3
  - gpui-file-explorer
generated:
  by: "r3-state"
  at: "2026-11-17T20:39:38+09:00"
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 調査範囲と判定方針

- 契約の根拠: `DESIGN.md:436-470`（watch → debounce → reconcile 状態機械）、
  `DESIGN.md:474-718`（Entry / Selection / History / LoadState / FsError /
  FsEvent / WatchState / Settings）。
- 実装の根拠: `app/src/model.rs`、`app/src/history.rs`、`app/src/tabs.rs`、
  `app/src/undo.rs`、`app/src/trash_view.rs`、`app/src/watcher.rs`、および
  これらの状態を実際に遷移させる `app/src/main.rs`（型定義 355-502、
  reload 652-744、reconcile 924-961、watch loop 965-1045、
  undo/redo 2290-2537、session restore 5572-5592）。
- 全件をファイル:行で読み直して確認した。「設計上の選択」と「契約違反」を分け、
  観測が OS のイベント配送に依存するものは `調査項目` として confidence を下げた。
- PASS 1/2 で既に扱われた undo 永続化・watch 再試行・tab session restore 自体は
  再掲せず、本レンズ固有の「スタックと実 FS の乖離」「世代トークンの欠落」
  「インデックスとパスキーの不整合」に絞った。

<!-- markdownlint-disable MD013 MD060 -->

### r3-state-01: Back/Forward が移動先検証前にスタックを書き換える

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-state-01 | 失敗した戻る/進むで履歴スタックが実位置と乖離 | 残タスク | main.rs:1298-1307, 1316-1325; history.rs:51-62 | `go_back`/`go_forward` は `history.apply(...)` で back を pop し current を forward へ push した後で `target.is_dir()` を判定している。対象が消えていてもスタックは変化したまま「移動できません」表示になる。 | 消えたディレクトリが履歴から消え、forward に現在地が積まれ続ける問題を防ぎ、戻る操作の信頼性を保つ | S | 新規 | 高 |

次のアクション: `target.is_dir()` を `History::apply` の前に検証し、移動成功時のみ
スタックを更新する（または `apply` に検証クロージャを渡す）。history.rs:51-62 は
現在「妥当性を一切見ずに pop/push する」契約なので、呼び出し側順序の修正が必要。

### r3-state-02: undo/redo タスクが直列化されずスタック順序が壊れうる

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-state-02 | Ctrl+Z 連打で undo/redo が並走し順序が反転しうる | 構造改善 | main.rs:2291-2295, 2327, 2361, 2405, 2425, 2481; main.rs:1982 | `undo_last`/`redo_last` は `op_task.is_some()` を検査するが自分で `op_task` を設定せず、`cx.spawn(...).detach()` で非同期実行する。`op_task` を set するのは paste 系の 1982 のみ。 | 連続 undo の完了順がスタック順序と逆になりうる。Undo 履歴オーバーレイと実 FS の対応が崩れる | M | 新規 | 高 |

次のアクション: undo/redo 専用の in-flight フラグ（または `op_task` と同じゲート）を
追加し、完了まで次の `pop_undo`/`pop_redo` を止める。テストは 2 連続 Ctrl+Z で
`push_redo` の順序が undo 順の逆になることを検証する。

### r3-state-03: undo の事前条件失敗時にエントリが失われる

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-state-03 | rename の undo が失敗すると undo スタックから消える | 残タスク | main.rs:2296, 2311-2322, 2346, 2385 | `pop_undo()` で取り出した後、`!current.exists()` / `target.exists()` の早期 return が 2 箇所あり、いずれも `push_undo` で戻していない。非同期の `Err` 経路（2346/2385）は戻しているため、同期失敗だけ不整合。 | 「元に戻せません」と表示された後に同じ操作を再度 undo できず、履歴が静かに消える | S | 新規 | 高 |

次のアクション: 2296 の pop を「検証後」へずらすか、2311/2317 の return 前に
`this.undo.push_undo(action_for_redo)` 相当を戻す。`UndoStack::pop_undo` は
破壊的に取り出す（undo.rs:73-75）ため、呼び出し側の全失敗経路が戻す責任を持つ契約を
テストで固定すべき。

### r3-state-04: Trash の redo 成功時に undo が記録されない経路

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-state-04 | list() 失敗時に redo だけが消費され元に戻せない | 残タスク | main.rs:2453-2460, 2470-2480, 2497-2512 | Trash の redo は `trash::delete` 成功後、`trash_view::list()` が `Ok` のときだけ `push_undo` する（2497 の `if let Ok`）。`Err` のときは redo スタックからpop 済みで何も積まず、成功ステータスを表示する。 | 「やり直しました」直後に undo 履歴が空になり、戻せない操作が発生する | S | 新規 | 高 |

次のアクション: `list()` 失敗時も元の `items`（trash id を含む）で `push_undo` するか、
`restore_by_id` と同じ id 保持方針を redo にも適用する。id が取れない場合のみ
「undo 不可」を明示する分岐にする。

### r3-state-05: 親ディレクトリ由来イベントが「再読込トリガ」として扱われない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-state-05 | 現行ディレクトリの削除を監視が再読込へ反映しない | 残タスク | DESIGN.md:454; main.rs:996-1002, 1003-1008, 984-990 | 設計は「親由来は再読込トリガ」と明記するが、`Created|Modified` は `path.parent() != cwd` で裸の `continue`（再読込なし）。`Removed` には親判定が無く `all_entries.retain` するだけなので、cwd 自体が消えても `Renamed` 以外では再読込されない。 | 外部 FM で現在地が削除/作成されても 10 秒 reconcile まで古い一覧が残る | M | 新規 | 中 |

次のアクション: 親由来の `Created/Removed/Modified` のうち cwd 自身を指すものを検出したら
`Reload`（design の `Rescan` 相当）を発行する。`Removed` にも親由来フィルタを追加し、
現在地の消滅はステータス「見つかりません」まで導く。**未確認**: notify が cwd 削除を
`Removed(cwd)` と `Renamed{from:cwd}` のどちらで届けるかはプラットフォーム依存。

### r3-state-06: watch の差分適用に generation / load 状態のガードが無い

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-state-06 | 古いディレクトリのイベントが新規一覧を直接書き換える | 構造改善 | main.rs:970-1038（`this.generation` 参照なし）; main.rs:679, 950; DESIGN.md:447 | `reload` と `reconcile` は世代ガードを持つが、watch loop の差分適用（996-1034）には `generation` / `is_loading()` チェックが無い。`Removed` は親パス判定も無いため、ナビゲーション前にキューされたイベントが新 cwd の `all_entries` に適用されうる。 | 監視スレッドの遅延イベントによる表示の壊れを防ぎ、設計の「gen ガード」を全経路で統一 | M | 新規 | 中 |

次のアクション: watch loop の `this.update` 冒頭で `let gen = this.generation;` を取り、
適用直前に再照合する。バッチ適用中に navigation が起きたら破棄して `Reload` に倒す。

### r3-state-07: 自操作抑制に対象パス集合が無く全イベントを飲んでいる

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-state-07 | suppress_until にパス集合が無く外部変更も破棄される | 構造改善 | DESIGN.md:467; main.rs:464-465, 1060-1066, 977-981; watcher.rs:139-156 | 設計は `suppress_until = now + 1.5s`（**+ 対象パス集合**）を要求するが、実装は `Option<Instant>` のみ。抑制窓内は全 `FsEvent` が return され、`op_cancel.is_some()` も同様に全イベントを止める。 | 自操作直後の他プロセス変更が最大 1.5 秒（debug フックの 30 秒も含む）見えなくなる | M | 既知(ドキュメント記載あり) | 高 |

次のアクション: `suppress: Option<(Instant, HashSet<PathBuf>)>` とし、対象パスに一致する
イベントだけを破棄する。10 秒 reconcile は現状どおり常時有効を維持する。

### r3-state-08: Search 表示中は reload が no-op になり pending_select が漏れる

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-state-08 | 検索結果表示中に undo しても一覧が更新されない | 残タスク | main.rs:656-658, 2342-2343, 2440-2441, 807-815 | `reload()` は `view == Search` で早期 return するが、undo/redo 成功時は `pending_select = Some(...)` を設定してから `reload(cx)` を呼ぶ。結果、検索一覧は古いままで `pending_select` が残り、次回の別ディレクトリ読込（807-815）まで持ち越される。 | 検索結果画面での undo/redo が見かけ上効かず、後で無関係な一覧の選択を汚す | S | 新規 | 高 |

次のアクション: Search 表示中に FS 変更を伴う操作が成功したら検索を再実行するか、
`pending_select` を設定しない（クリアしてから退出時に reload）経路を明示する。

### r3-state-09: 保存済み active_tab がタブ除外後に差し替わる

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-state-09 | 消えたタブの分だけアクティブ位置がずれる | 残タスク | main.rs:5579-5583, 547-549; settings.rs:65-67 | `restore_session` は `saved` から存在しないパスを filter してから `active.min(tabs.len()-1)` で clamps するが、除外された分の index を引き直さない。例: `[A,B,C]` active=1 で A が消えると `[B,C]` active=1 となり C が開く。 | 起動時に意図しないタブがアクティブになり、シリアライズ済み `active_tab` が意味を失う | S | 新規 | 高 |

次のアクション: filter 前の index に対応するパスを保持し、filter 後の同等パスの位置を
再計算する。あるいは filter 時に「除外した index より前の個数」だけ active を減らす。

### r3-state-10: WatchState / 状態機械が型として存在しない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-state-10 | 設計の WatchState が ad-hoc な last_error に置き換わっている | 構造改善 | DESIGN.md:436-452, 663-668; watcher.rs:83-84, 139-156; main.rs:5151-5180 | `WatchState::{Off, Active, Degraded}` も Idle/Armed/Dirty/Applying/Reconcile も実装に型として存在せず、`DirWatcher.last_error: Option<String>` と status 文字列で代用。`unwatch` 失敗（watcher.rs:143）も同じ `last_error` を汚し「監視: 一部無効」と表示される。設計の文言「自動更新: 無効」＋トースト＋F5 案内も無い。 | watch 状態を一意に判定できる型を作り、誤表示と復帰漏れを減らす | M | 既知(ドキュメント記載あり) | 高 |

次のアクション: `WatchState` enum を導入し `watch()` の結果（init 失敗 / watch 失敗 /
unwatch 失敗 / 正常）を区別してから status を組立てる。表示文言とトーストは
DESIGN §4.6:469 に揃える。**設計 choice との境界**: 状態を文字列で持つこと自体は
実装の自由だが、「unwatch 失敗が監視正常なのに無効表示になる」点は契約違反。

### r3-state-11: reconcile のフォーカス復元が「旧 index に近い位置」ではない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-state-11 | フォーカス行を消すと先頭へジャンプする | 残タスク | DESIGN.md:593; main.rs:825-835; model.rs:268-279 | 設計は「focused が消えたら旧可視 index に近い前後のエントリへ移す」と要求するが、`rebuild_view` は keep が見つからないと `selection.paths().first()`、それも無ければ `unwrap_or(0)` で先頭へ移す。旧 `selected` index は使われない。 | 削除後のキーボード操作位置が飛び、大量一覧での連続削除が困難 | S | 新規 | 高 |

次のアクション: `rebuild_view(keep, prev_visible)` のように旧 index を渡し、
消えた場合は `clamp(prev_visible, new_len)` を新フォーカスにする（DESIGN §5.3 の
`reconcile(&mut self, listing, prev_visible)` シグネチャに揃える）。

### r3-state-12: DESIGN §5 の型シグネチャとのドリフト（調査項目）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-state-12 | Ready の loaded_at / Entry の size Option / History::default が契約と違う | 調査項目 | DESIGN.md:487-505, 623-627; main.rs:403-407; model.rs:79-91, 107-115; history.rs:22-36 | `LoadState::Ready` は設計の `{ loaded_at: Instant }` に対して単位型。`Entry.size: u64`（設計は `Option<u64>`）のためメタデータ取得失敗が 0 バイト通常ファイルと区別できない。`#[derive(Default)]` の `History` は `max_back: 0` になり、将来 `History::default()` が使われると全履歴が即座に捨てられる（現状 production 未使用は history.rs:29-36 と tabs.rs:20 で確認）。 | 型で表現すべき「不明」を値 0 に潰さないようにし、潜在的な履歴喪失を塞ぐ | M | 既知(ドキュメント記載あり) | 中 |

次のアクション: まず `History` の `Default` を手実装して `new()` と同じ 200 を設定する
（S で即対応可）。`Entry.size` / `LoadState::Ready` は表示側への影響範囲を洗い出し、
「設計上の選択として README に記録する」か「Option に戻す」かを判断する。

## レンズ総評

本レンズでは、設計側と実装側の「どちらが妥当か」ではなく、**状態を変更する順序**
と**状態を保持する型**が設計とずれている箇所を 12 件抽出した。特に優先すべきは
r3-state-03（undo エントリの消失）、r3-state-02（undo/redo の並走）、
r3-state-01（履歴の事前検証漏れ）の 3 件で、いずれも「ユーザー操作は成功表示なのに
内部スタックが実 FS と乖離する」という同型の問題である。次に r3-state-05/06/07 の
監視まわりのガード不足は 10 秒 reconcile で最終的に回復するものの、
「設計済みの gen ガード・パス集合が配線されていない」点で契約違反が明確である。

r3-state-09/11/12 は一見小さな不整合だが、シリアライズ済みフィールド
（`active_tab`）や「旧 index に近い位置」という設計約束が実装で意味を失っている点で、
型契約の健全性という観点から重要である。r3-state-10 は設計上の選択と違反の境界を
明示した: 文字列での状態表現自体は許容だが、unwatch 失敗による誤「無効」表示は
`WatchState` の導入で解消すべき実害である。全件ともソースは未変更で、
ファイル:行は read/grep で実読したもののみを記載した。
