---
type: Reference
title: アプリケーションライフサイクルとウィンドウ管理のタスク発見（PASS 4）
description: >-
  DESIGN.md の OS マトリクス・ADR-002、app/src/main.rs の起動・終了経路、
  settings.rs の永続化、watcher.rs / ops.rs の初期化を実読し、
  プロセス／ウィンドウ単位のライフサイクルに固有の残タスク・調査項目を
  file:line 根拠付きで列挙したもの。
tags:
  - task-discovery
  - gpui-file-explorer
  - lifecycle
  - window-management
  - round-4
generated:
  by: "r4-lifecycle"
  at: "2026-11-17T21:01:33+09:00"
status: complete
---

## 発見一覧

PASS 3 の非同期競合・入力フォーカス復帰、DirWatcher の Drop join、E2E・配布・a11y・セキュリティ・open/reveal 失敗は意図的に除外した。
本表は「プロセス開始〜終了」と「ウィンドウ生成〜破棄」のプロセス／ウィンドウ単位の経路のみを対象とし、全項目を実読した `file:line` に基づく。
設計で明示されているが実装に無いものは「残タスク」、コード上は不在だが意図・実機挙動が証明できないものは「調査項目」とした。

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-life-01 | ウィンドウ終了時に settings.save() が呼ばれず設計 §8.1 と乖離している | 残タスク | `DESIGN.md:840`, `app/src/main.rs:5619-5624`, `app/src/main.rs:1585`, `app/src/settings.rs:237-243` | 設計は保存タイミングに「ウィンドウ終了時」を明記するが、`on_window_closed` は `cx.windows().is_empty()` を見て `cx.quit()` するだけで `settings.save()` を呼ばない。現状は全永続フィールドがミューテーション時に同期保存されるため即時のデータ損失は起きにくいが、今後「終了時に初めて確定する状態」や保存を忘れた経路が出た瞬間に失われる。`on_window_closed` と Ctrl+Q の両方を通る共通の shutdown 関数を用意し、保存とキャンセルを集約するのが妥当。 | 中 | 小 | 高 | 高 |
| r4-life-02 | Ctrl+Q が on_window_closed を迂回して即 quit する | 構造改善 | `app/src/main.rs:1537`, `app/src/main.rs:1585`, `app/src/main.rs:5619-5624` | `modifiers.control` 分岐の `("q", _) => cx.quit()` はウィンドウ閉鎖イベントを経由せずプロセスを終了する。現状は両経路とも単に quit するだけなので挙動は等価だが、r4-life-01 のような終了時処理を `on_window_closed` に追加しても Ctrl+Q はそれをすり抜ける。終了処理を一箇所に集約し、キーハンドラからは共通の shutdown を呼ぶ設計へ変更する。 | 中 | 小 | 高 | 高 |
| r4-life-03 | コピー・ゴミ箱・検索の最中にウィンドウが閉じられた際の挙動が未定義 | 調査項目 | `app/src/main.rs:5619-5624`, `app/src/main.rs:580-582`, `app/src/main.rs:3682`, `app/src/ops.rs:254-357`, `app/src/ops.rs:924-941` | 終了フックは quit のみで、進行中操作への cancel通知も完了待ちも行わない。`ops::copy_file` は `.partial-*` を書き、キャンセル・エラー時に `remove_file` で片付ける（ops.rs:322/343-357、テスト 924-941）が、これは Rust の制御フローが動く場合の保証であり、プロセス終了や強制終了で途中の `.partial-*` が残る挙動はコードからは証明できない（未確認）。検索は `cancel.store(true)`（main.rs:3682）で止まるが quit 時に呼ばれる箇所はない。終了時に in-flight 操作をキャンセルして完了を待つポリシー、あるいは起動時に残留 `.partial-*` を検出して除去する回收処理を定義する。 | 中 | 中 | 高 | 中 |
| r4-life-04 | マルチウィンドウは ADR-002 で明示的に先送り、実装も単一 open_window | 調査項目 | `DESIGN.md:26`, `DESIGN.md:95`, `DESIGN.md:357`, `DESIGN.md:1056`, `app/src/main.rs:5619-5624`, `app/src/main.rs:5639` | OS マトリクスは「タブ / 複数ウィンドウ」を P0/P1 × とし「まず 1 ウィンドウ 1 状態（ADR-002）」と明記、ADR-002 は単一 Explorer Entity を採用する。実際 `open_window` の呼び出しは main.rs:5639 の1箇所だけだが、`on_window_closed` は `cx.windows().is_empty()` を見ており複数ウィンドウの終端判定だけは先回りしている。対応に必要なものは、ウィンドウごとの Explorer Entity と settings/tabs の分離、ウィンドウ registry と閉鎖時の状態マージ、watcher のウィンドウごとの成否管理、ジオメトリの個別永続化、 session restore の複数ウィンドウ化。まずは「何を共有し何を分けるか」の ADR 追加が先決。 | 中 | 大 | 高 | 高 |
| r4-life-05 | ウィンドウ位置・サイズが毎回 1100x700 中央固定で永続化されない | 機能追加案 | `app/src/main.rs:5626-5637`, `app/src/settings.rs:53-78`, `DESIGN.md:839-840` | `Bounds::centered(None, size(px(1100.0), px(700.0)), cx)` がハードコードされ、`Settings` 構造体には bounds / x / y / width / height 相当のフィールドが無い。設計の永続化リスト（§8.1）も last_dir・sort・show_hidden・view 等のみでジオメトリは対象外。設定にジオメトリを追加しウィンドウ移動・リサイズ時に保存、起動時に復元する必要がある。復元時はモニタ構成変更で画面外に出るケースの検証（表示可能なワークエリアへのクランプ）も必須。 | 中 | 中 | 高 | 高 |
| r4-life-06 | セカンド起動・単一インスタンス方針が未確認（ロックファイル/IPC の実装なし） | 調査項目 | `app/src/main.rs:5606-5654`, `app/src/settings.rs:237-279`, `app/src/settings.rs:149-159` ほか `app/src/` 全体を検索しても lockfile / single-instance / IPC 関連の識別子はヒットせず（未確認） | `main()` は CLI 判定後すぐ `app.run` し、既存インスタンスの検出や引数引き渡しがない。2プロセス起動すれば両者が独立に watcher を張り、`settings.json` は atomic rename による last-writer-wins で上書き合い、片方の Exit が保存した tabs/last_dir が另一方で消えうる。設計書にも単一インスタンスに関する記述は無く、意図的な非対応か単なる未検討かは未確認。「2回目の起動で既存ウィンドウを前面化して引数ディレクトリを開く」等のポリシーを決定し、ロックファイル + IPC（または既存サービスの利用）を選定する。 | 中 | 中 | 高 | 中 |
| r4-life-07 | 起動時の設定ロード→watch 開始→初回ディレクトリ読込に順序依存がある | 残タスク | `app/src/main.rs:543-546`, `app/src/main.rs:638-645`, `app/src/main.rs:654-708`, `app/src/main.rs:862-868`, `app/src/main.rs:814`, `app/src/watcher.rs:89-110`, `app/src/watcher.rs:128-157` | `Explorer::new` は Settings::load(508) → restore_session(516) → DirWatcher::new(543、この時点では未 watch) → save(638) → watch ループ起動(643) → reconcile 起動(644) → reload(645) の順。`reload` が `update_watcher`(668→868) でやっと OS watch を登録し、その直後に `read_entries` を背景開始する。watch ループは初回ロード完了を待たず `all_entries` へ差分を適用でき（979-1039）、完了時に `apply_loaded` が `all_entries` を無条件置換する（814）ため、この隙間に作成されたファイルが一覧から消えうる。起動中は watch 差分をキューに溜めて初回ロード完了後に適用する、あるいは初回ロード完了まで watch イベントを破棄して reconcile に委ねる等の順序規定が必要。 | 中 | 小 | 高 | 中高 |
| r4-life-08 | 最小化・復元・ウィンドウフォーカス変化のライフサイクル処理が存在しない | 調査項目 | `app/src/main.rs:5619-5624`, `app/src/main.rs:74`, `app/src/main.rs:1049-1057` ほか minimize / restore / on_focus / set_bounds 等のハンドラは `app/src/` にヒットせず（未確認） | コード上のウィンドウイベント購読は `on_window_closed` のみで、最小化・復元時の再描画や一覧鮮度確認は行わない。現在は 10 秒ごとの reconcile（main.rs:74,1049-1057）と watch で結果的に更新されるが、長時間バックグラウンド後に復元した直後の一覧が古いままになる可能性があり、Wayland/KWin の実際のイベント配送挙動は未確認。復元時に `reload` を差し込むか、最低限「復元直後は最大 10 秒古いことがある」を設計書の既知制約として明記する。 | 低中 | 中 | 高 | 中 |
| r4-life-09 | 起動失敗・パニック時のシャットダウン／復帰方針が無い | 調査項目 | `app/src/main.rs:5596-5604`, `app/src/main.rs:5645-5649`, `app/src/settings.rs:245-247`, `app/src/settings.rs:263-279` ほか panic hook / クラッシュセンチネルの実装は見当たらず（未確認） | `init_logging` は best-effort で、`open_window` 失敗はログ + `cx.quit()` で通告する（5645-5649）。一方 `gpui_kit::init` や `Explorer::new` 内の panic に対するフックや、次回起動で異常終了を検出して設定・セッションを復旧する仕組みは無い。設定保存は tmp + rename の atomic write だがディレクトリ fsync は省略されており（settings.rs:245-247）、保存直後のクラッシュ durability は best-effort と明記されている。異常終了時のログ確実化とセッション復旧ポリシー（最後に正常保存したセッションへのフォールバック等）を定義する。 | 低中 | 中 | 高 | 中 |
| r4-life-10 | 保存済みタブが存在すると CLI 引数のディレクトリが無視される | 調査項目 | `app/src/main.rs:5574-5594`, `app/src/main.rs:516-522`, `app/src/main.rs:5546-5547`, `app/src/main.rs:5938-5954` | `restore_session` は保存済みタブが1つ以上残っていれば CLI 引数を見ずにそのアクティブタブを返す（5581-5585）。ヘルプ文は「引数なしの場合は前回のディレクトリ」と説明的に読めるため、引数を付けたのに保存済みセッションが開く挙動は利用者を混乱させうる。テスト 5938 はこの優先順位を意図的に固定しているので単なるバグではないが、起動セッションの仕様として「CLI 引数が最優先か、保存セッションが最優先か」を設計書で明示し、必要なら引数を新規タブとして追加する挙動へ変更する。 | 中 | 小 | 中 | 高 |

## レンズ総評

起動・終了の骨格は PoC から踏襲された最小構成で、`main()` は `handle_cli()` → `init_logging()` → `app.run()` → `on_window_closed` 登録 → 単一 `open_window()` → `cx.activate(true)` という直線的な流れを持つ（`poc/src/main.rs:374-410` も同型）。
ウィンドウ終端判定に `cx.windows().is_empty()` を使う点だけが複数ウィンドウを想定した先回りだが、ADR-002 が示すとおり「1 ウィンドウ 1 状態」が現在の契約である。

最も確度が高く修正コストも小さいのは r4-life-01 と r4-life-02 で、設計が要求する「終了時保存」が実装になく、さらに Ctrl+Q がその終端を迂回している。
終了処理を一つの shutdown 経路に集約すれば、進行中操作のキャンセル（r4-life-03）や設定保存も同じ場所から呼べるようになり、後のマルチウィンドウ化（r4-life-04）でも効く。

次点で r4-life-07 の起動時順序依存は、初回ロード完了前の watch 差分が `apply_loaded` で上書きされる点までコード上追えるため、再現テストを先に作ってから修正するのが妥当。
r4-life-05 のジオメトリ永続化は明確な機能追加、r4-life-10 は CLI と session restore の仕様確認、r4-life-08 / r4-life-09 は Wayland/KWin の実機挙動と異常終了ポリシーが未確認のため「調査項目」とした。

単一インスタンス（r4-life-06）はコード・設計書の両方で根拠が得られず「未確認」を付けた。2プロセスが設定ファイルを last-writer-wins で上書きし合う構造はコード上明らかなため、ポリシー決定の優先度は中程度と判断する。
いずれもプロセス／ウィンドウレベルのライフサイクルに限定しており、pass 3 の非同期競合やダイアログのフォーカス復帰とは重複しない。
