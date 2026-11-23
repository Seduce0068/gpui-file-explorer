---
type: Todo
title: "gpui-file-explorer プロジェクトTODO"
description: "gpui-file-explorer 固有のタスク。横断タスクはグローバルTODOへ。"
tags:
  - todo
  - tasks
  - "gpui-file-explorer"
generated:
  by: opencode
  at: "2026-11-17T20:11:47+09:00"
status: current
---

## 進行中

（なし）

## 未着手

> 出典: [第2次クロスレビュー最終統合レポート](notes/task-discovery-2/round-6/consolidated.md)
> （5 回の独立レビュー・367 件 → 54 項目。ID はレポートの F6-* に対応）

### 優先度: 高

> 出典: 第3回タスク発掘（9 エージェント並列・生 81 件 → 親集約）。
> ID は `R1-*` / `R2-*` / `R3-*` / `R4-*` の発掘担当に対応。

- [x] **R1-ST01** キーマップ設定で同一 chord が複数アクションに衝突しても検証されない — 対応: dbfd1d4
  — 根拠 `app/src/settings.rs:876-889` / confirmed / 要解决
- [x] **R1-ST02** F1 ヘルプが静的 HELP_SECTIONS でユーザー設定キーマップを反映しない — 対応: d3eba3f
  — 根拠 `app/src/main.rs:4974` / confirmed / 要解决
- [x] **R1-ST02** ダイアログ検証エラーが入力変更で消えず張り付く — 対応: d3eba3f
  — 根拠 `app/src/main.rs:5312-5316` 他 / confirmed / 要解决
- [x] **R2-FG01** status 正規定数が未使用で main.rs が監視イベントに上書きされる — 対応: d3eba3f
  — 根拠 `app/src/strings.rs:240-246` / confirmed / 要解决

- [x] **F6-01** Undo/Trash 項目の黙っての消失 — `pop_undo()` 後の早期 return と
  `trash_view::list()` 成功依存 — 対応:
  a2375e8
- [x] **F6-02** move の TOCTOU 上書きと symlink 攻撃連鎖 — `ops.rs:604,612` に予約機構なし — 対応:
  70682a2
- [x] **F6-03** リリース版デバッグフック（29 種）による実 FS 操作 — `cfg(debug_assertions)` 0 件 — 対応:
  ef278ff
- [x] **F6-04** 設定読取失敗が既定値で上書き／退避成功が無通知 — `settings.rs:200-202` →
  `main.rs:640` — 対応:
  a275846
- [x] **F6-05** デバッグ値が起動時 save で実設定を汚染 — `ephemeral` ガードが一部の保存経路のみ — 対応: 8ba7c0c
- [x] **F6-06** 非 UTF-8 名・引数の lossy 化 — undo/trash ID/CLI/serde — 対応:
  26ccdbd (CLI 引数)。undo/trash ID は OsString 化して解消 (aeb687c)
- [x] **F6-07** 検索の一致判定・上限・意味論 — NUL 判定、per-root cap 500、sort 前 cap — 対応:
  95104ad
- [x] **F6-08** 選択・フォーカスの不変式破壊 — `selected` / `Selection` / `path_set` の 3
  層ミラー — 対応:
  36ce436
- [x] **F6-09** 操作排他・キャンセル・背圧の偏り — 対応: da143b0
- [x] **F6-10** グリッドの scroll index 誤り — 行 index に entry index を渡す（描画上の軽微な問題） —
  対応:
  5efa268
- [x] **F6-11** 設定一時ファイルのリークとパーミッション — tmp 残存・0600→0644 — 対応: 3a95d84
- [x] **F6-12** 保存タブが CLI 引数／`.desktop %f` を上書き — `restore_session()` の早期
  return — 対応:
  4ce6067
- [x] **F6-13** 操作成否・Undo 確認文と実保証の乖離 — open/reveal 戻り値破棄、貼付全中止 — 対応: e2bf3d0
- [x] **F6-14** macOS/iOS/Android で trash ビューがビルド不能 — 対応: c7849e2
  （trash 依存を cfg 付き target 別依存へ、非対応 OS は no-op フォールバック）

### 優先度: 中

- [x] **F6-15** 履歴ナビ破損・trash redo の誤報告 — 対応: 98042a9
- [x] **F6-16** 検索結果の右クリックが `self.cwd` に適用される — 対応: 7286eb6 (文書化、設計判断で現状維持)
- [x] **F6-17** 監視の世代/cwd ガード欠如・抑制窓のイベント破棄 — 対応: 現行コードで確認済み (cwd ガードあり)
- [x] **F6-18** UI スレッド上の同期ファイル I/O — 対応: 作業ツリー（background_spawn 化）
- [x] **F6-19** エラー型の平坦化と分断（`FsError`/`OpError`/`String`）
  対応: 作業ツリー（共通 AppError）
- [x] **F6-20** 設定スキーマ検証・上限・マイグレーションの不足 — 対応: 4268167
- [x] **F6-21** 設定の並列更新と保存失敗の非通知 — 対応: 2bace24
- [x] **F6-22** 検索／プレビューの鮮度・性能・形式判定 — 対応: 現行コードで確認済み (二重サイズガード)
- [x] **F6-23** ゴミ箱 undo/redo の部分成功報告 — 対応: 現行コードで確認済み (failures 保持)
- [x] **F6-24** データモデルの型不変条件退化（Entry/Selection/History/path） — 対応:
  現行コードで確認済み (不変条件保持)
- [x] **F6-25** 色覚アクセシビリティと text alternative（ツールバー 8 ボタン） — 対応: d78ae16
- [x] **F6-26** i18n・日本語表記の散在とロケール並び順 — 対応: c7849e2（文言定義の集約と照合契約の明記）
- [x] **F6-27** 配布物のゲート・運用（`.desktop` MimeType 欠落、DEB 無検証） — 対応: 2ab59ce
- [x] **F6-28** ビルド再現性と CI 透明性（配布のみ `--locked` なし） — 対応: e3b661d
- [x] **F6-29** サプライチェーン監査とライセンス — 対応: c7849e2（deny.toml / CI / check.sh）
- [x] **F6-30** ドキュメント数値の真実源欠落（132/167/168、判定 4 回反転） — 対応: 6ff4abc
- [x] **F6-31** テストの実質欠如（空振り assert、trivially pass） — 対応:
  trivially pass / ignored test 無しを確認
- [x] **F6-32** セッション復元とウィンドウスコープ — 対応: ad5949c
- [x] **F6-33** ドラッグ&ドロップとゴミ箱ビュー操作の不足 — 対応: 作業ツリー（内部 DnD / trash キー導線）
- [x] **F6-34** ワークフロー改善（プレビュー連続移動、新タブで開く、即命名） — 対応: 作業ツリー
- [x] **F6-35** キーボード／ダイアログの細部 — 対応: 既にトグル実装済みを確認
- [x] **F6-36** エラー・ステータスメッセージの情報量と競合
  対応: ad5949c（定数と共存ルール、呼び出し側は 44 箇所を定数へ移行済み（aeb687c））
- [x] **F6-37** 作業効率ロードマップ — 転送後検証と一括リネーム API を実装。ターミナル起動は F6-50 へ分離。
- [x] **F6-38** 個人化ロードマップ — データモデル完了（保存検索 / ブックマークラベル、11テスト）。UI 配線は F6-49 へ分離。

- [x] **F6-49** 個人化 UI 配線 — 保存検索とブックマークラベルを接続。Fuzzy jump / キーマップは F6-51 へ分離。

- [x] **F6-50** ターミナル起動 — 設定可能コマンド（Linux 既定 konsole）と F4 / コンテキストメニューを追加
- [x] **F6-51** Fuzzy jump と個人化キーマップ — Ctrl+P オーバーレイとテーブル駆動キーマップを追加（再割当はメモリ上のみ）

### 優先度: 中（第3回発掘・confirmed）

- [x] **R1-ST01** rename_entry が壊れた symlink を拒否（copy/move と不一致）
  — 対応: 779b327
  — `app/src/ops.rs:775` / R3-TQ01 と重複統合
- [x] **R1-ST01** ブックマーク削除時にラベルが孤立し再追加で復活する — 対応: cb7f98a
  — `app/src/settings.rs:503-505` / R3-TQ01 と重複統合
- [x] **R3-TQ01** keymap 検証が `contains("f1")` で F10〜F19 を誤拒否 — 対応: dbfd1d4
  — `app/src/settings.rs:185-188`
- [x] **R3-TQ01** merge_search_results が上限到達時に最終ソートをスキップ — 対応: 8ec8969
  — `app/src/search.rs:259-261`
- [x] **R3-TQ01** 内容検索のバイナリ判定が先頭行一致で回避される — 対応: 8ec8969
  — `app/src/search.rs:315-327`
- [x] **R1-ST02** SIDEBAR_WIDTH_PX=214 が実サイドバー 190px と乖離 — 対応: f03c653
  — `app/src/main.rs:87` vs `:7146`
- [x] **R1-ST02** icon_button_active に aria_label が無い — 対応: f03c653
  — `app/src/main.rs:7690-7714`
- [x] **R3-TQ02** kind_for_entry が symlink を無視し icon_for_entry と分類が食い違う — 対応: f03c653
  — `app/src/icons.rs:26-34`
- [x] **R2-FG01** Ctrl+P / F4 / Alt+K が F1 ヘルプ・CLI help に未掲載 — 対応: d3eba3f
  — `app/src/strings.rs:76-159`

### 優先度: 高（第4回発掘・即対応）

- [x] **R2-TQ01** move_entry のディレクトリ移動が Unix で ENOTDIR になる
  — `app/src/ops.rs:711-720` / 対応: (本ラウンド)
- [x] **R2-TQ01** keymap 検証の `&&` 短絡で chord 重複除外がバイパスされる
  — `app/src/settings.rs:906` / 対応: (本ラウンド)

### 優先度: 中（第4回発掘・confirmed）

- [x] **R1-FG01** 通知トースト未実装（ステータス行で代替）
  — 対応: 設計判断で現状維持。`app/README.md`「設計（DESIGN.md）からの
  差分」に記録済み（2026-11-18 確認）
  — `DESIGN.md:807` / `app/src/main.rs:2586`
- [x] **R2-TQ02** 破損 symlink の Undo / reveal が `exists()` で誤判定 — 対応: 5b1b1fa
  — `app/src/main.rs:4047`
- [x] **R5-UX01** Alt+K ダイアログが F10〜F19 を予約キーと誤判定（settings 側のみ修正済み） — 対応: 5b1b1fa
  — `app/src/main.rs:389-392`
- [x] **R5-UX01** 検索結果がキーボードで操作できない（検索ビューでキーが破棄される） — 対応: 5b1b1fa
  — `app/src/main.rs:3158-3163`
- [x] **R4-DP01** CI / check.sh が release プロファイルを検証しない — 対応: f8fef38
  — `scripts/check.sh:151-197` / `#[cfg(not(debug_assertions))]` 分岐が未検証
- [x] **R4-DP01** check.sh が監査ツール未導入時に黙って成功する — 対応: f8fef38
  — `scripts/check.sh:101-142`
- [x] **R4-DP01** gen-perf-fixture.sh が既存ディレクトリをマーカーで武装化 — 対応: 19234b3
  — `scripts/gen-perf-fixture.sh:120-127`

### 優先度: 中（第5回発掘・confirmed）

- [x] **R1-INT01** Alt+K 再バインドが同一 chord の他アクションを永続化から除去しない
  — 対応: 9772725
- [x] **R1-INT01** keymap UI にバインド解除の導線が無い — 対応: 9b5aae2
  — `app/src/main.rs:3031-3041` / `parse_keymap_input` が None を拒否
- [x] **R1-INT01** 検索ビューで F4 / Ctrl+Q が塞がりヘルプと挙動が食い違う — 対応: 9b5aae2
  — `app/src/main.rs:3236-3249`
- [x] **R4-PLAT01** Unix で合法なバックスラッシュ入りファイル名が拒否される — 対応: 8abeb7c
  — `app/src/ops.rs:124-128`
- [x] **R5-QUAL01** icons テストが戻り値を捨てて割当を検証していない — 対応: 1bbfce2
  — `app/src/icons.rs:189-193`
- [x] **R5-QUAL01** pre-epoch fingerprint テストが結果を破棄している — 対応: 1bbfce2
  — `app/src/model.rs:1126-1129`
- [x] **R3-DOC01** DESIGN.md の §8.5 参照が存在しない節を指す — 対応: b1f9263
  — `DESIGN.md:102,260,305`
- [x] **R3-DOC01** Ctrl+P が README のキーバインド表に欠落 — 対応: b1f9263
  — `app/src/strings.rs:86` / `README.md:48`

### 優先度: 中（第6回発掘・confirmed）

- [x] **R1-SCH01** version 欠落が SCHEMA_VERSION で埋まり legacy 判定が失效する
  — 対応: 1bc2374
- [x] **R1-SCH01** 配列重複排除が truncate 後で手編集設定の末尾項目が消える
  — 対応: 1bc2374
- [x] **R2-SAF01** copy_recursive 中Ok/エラー時にコピー先ディレクトリが残存する — 対応: a37c10c
  — `app/src/ops.rs:569`
- [x] **R2-SAF01** rename_entry が壊れた symlink 宛先を黙って上書きする — 対応: a37c10c
  — `app/src/ops.rs:797`
- [x] **R4-INT01** Alt+Shift+T のテーマ保存失敗が通知されない — 対応: 3103d44
  — `app/src/main.rs:3464-3473`
- [x] **R4-INT01** README に unbind 構文の記載が無い — 対応: 3103d44
  — `app/src/strings.rs:168` / `app/README.md:99`
- [x] **R5-DOC01** app/README の version 挙動が古い — 対応: 0d664d5
  — 対応: 0d664d5

### 優先度: 中（第7回発掘・判断記録）

- [x] **H-1/H-7/H-8** Explorer の God Object / 状態単一化 — `ui/help.rs`
  抽出は cosmetic なため見送り。真の解消には 46 箇所の `cwd`/`history`
  参照と borrow 対応を伴う 150-250 行の再設計が必要で、現状維持が妥当。
  詳細は assoc-refactor の調査ログを参照。
- [x] ステータスバー右端のバージョン表示を純粋関数化して回帰テスト追加
  — 対応: (本ラウンド)

### 優先度: 中（第8回発掘）

- [x] **R4-DATA01** 非UTF-8パスが serde 保存を全て失敗させる
  — 対応: 0bd0679
- [x] **R4-DATA01** settings.json を全量読み出してからサイズ検査する
  — 対応: 0bd0679
- [x] **R2-STAT01** 検索終了時に generation が増えず遅延結果が上書きする
  — 対応: 0bd0679
- [x] **R2-STAT01** reconcile 成功時に Error 状態が残存する
  — 対応: 0bd0679
- [x] **R1-INV01** History::default() が max_back=0 で new() と不整合 — 対応: 996796b
  — `app/src/history.rs:22-34`
- [x] **R1-INV01** go_back/go_forward が妥当性確認前にスタックを更新 — 対応: 996796b
  — `app/src/history.rs:51-62`
- [x] **R1-INV01** UndoStack::push_redo に上限が無い — 対応: 0156b17
  — `app/src/undo.rs:78-80`
- [x] **R1-INV01** select_all(&[]) が空選択＋stale anchor を残す — 対応: 0156b17
  — `app/src/model.rs:216-221`

### 優先度: 中（第9回発掘）

- [x] **R1-LEAK01** case-only rename の中間一時ファイルが 2 回目失敗で残存
  — 対応: e957de5
- [x] **R1-LEAK01** rename_entry に O_EXCL 予約が無く TOCTOU で上書き可能
  — 対応: e957de5
- [x] **R1-LEAK01** remove_entry_safe が再帰中に ReadDir を保持
  — 対応: e957de5
- [x] **R1-LEAK01** watcher 初期化失敗時に debouncer スレッドがリーク
  — 対応: 9abefeb（fail-soft SafeWatcher で依存更新なしに解消）
- [x] **R3-PERF01** search_lines の全件 Vec 化 — 誤検知として却下
  — 現行は `filter_map` で必要な行番号のみ収集（main.rs:6283-6290）

### 優先度: 中（第11回回帰監査）

- [x] **R1-AUDIT01** O_EXCL 予約を rename 直前に削除し競合保証が不成立 — 対応: 8dd37f3
  — `app/src/ops.rs:871-878` / Windows 全般と Unix ディレクトリ移動
- [x] **R2-AUDIT02** keymap 重複排除が棄却エントリでアクション名を予約 — 対応: b31f7c0
  — `app/src/settings.rs:968-983`
- [x] **R3-AUDIT03** 永続 keymap の evict が文字列比較で実効と不一致 — 対応: b31f7c0
  — `app/src/main.rs:513-515`
- [x] **R3-AUDIT03** reconcile 完了コールバックが view を再確認せず検索結果を上書き — 対応: 13ca37b
  — `app/src/main.rs:2176-2194`
- [x] **R4-AUDIT04** 空白のみクエリのテストが空振り — 対応: 4c36de6
  — `app/src/search.rs:111-114,449-452`
- [x] **R4-AUDIT04** 壊れた symlink で entry_for_path と from_dir_entry が不一致 — 対応: 4c36de6
  — `app/src/model.rs:68-71,113-116`
- [x] **R4-AUDIT04** 検索の拡張子フィルタが extension_of と規則を共有しない — 対応: 4c36de6
  — `app/src/search.rs:52-67`

### 優先度: 中（第11回監査・残存）

- [x] **R1-AUDIT01** Windows の rename は予約解除後の残存 TOCTOU を文書化
  — 対応: 8dd37f3（std::fs では atomic replace-if-absent 不可のため現状維持）

### 優先度: 高（第12回 project-review / 一部対応済み）

- [x] **R4-CI01** CI の extra_gates に release 検証が無い
  — 対応: 3221859
- [x] **R3-DE01** release で GPUI_FE_DEBUG_* が実設定を書き換える
  — 対応: 37d0c24（compile-time gate、strings で ABSENT 確認）
- [x] **R1-CF01** config_dir 不在時に save が成功を返す / size cap 矛盾 / terminal 既定値の永続化
  — 対応: bf7f58e

### 優先度: 中（第12回 project-review / confirmed）

- [x] **R2-ER01** OS ドロップのスキップ通知が run_transfer で上書きされる — main.rs:4040 — 対応: 5cbe508
- [x] **R2-ER01** ブックマーク/ラベル系で設定保存失敗が握りつぶされる — main.rs:2068 — 対応: 5cbe508
- [x] **R2-ER01** カットのキャンセル/失敗時もクリップボードが破棄される — main.rs:3890 — 対応: 5cbe508
- [x] **R2-ER01** 部分コピー削除エラーが無視され残骸が残る — ops.rs:585 — 対応: 3261129
- [x] **R2-BV01** 古い進捗コールバックが最終ステータスを上書き — main.rs:3694 — 対応: 5cbe508
- [x] **R2-BV01** エラー状態が進行中 reload コールバックに上書きされる
  — 対応: d9da623
- [x] **R2-BV01** reload 中の watcher 増分が古いスナップショットで消える
  — 対応: d9da623
- [x] **R2-BV01** Search 表示中に rebuild_view が呼ばれフィルタが失われる
  — 対応: d9da623
- [x] **R1-AR01** chord 検証ルールが settings/main に三重分散
  — 対応: 129d0a1
- [x] **R1-AR01** TabSession が History の公開 API を逆算利用 — settings.rs:327 — 対応: 129d0a1
- [x] **R5-TS01** 非 Unix で symlink 非追跡テストが空振り — search.rs:554 — 対応: 94b1e54
- [x] **R5-TS01** クロスデバイス移動フォールバックが実質未テスト — ops.rs:855 — 対応: 3261129
- [x] **R5-DC01** review-report の行数/構造化ログ記述がドリフト
  — 対応: 28a030e

### 優先度: 中（第14回 UX 発掘・confirmed）

- [x] **R1-UX01** F1 ヘルプに Up/Down・R/Ctrl+Enter・ゴミ箱復元が未掲載 — 対応: b273132
  — `app/src/strings.rs:76-163` / 2 発見を統合
- [x] **R3-DATA01** ゴミ箱表示で sort_header/Ctrl+H がディレクトリ用フィルタを適用 — 対応: b273132
  — `app/src/main.rs:3718-3730,3681-3691`
- [x] **R2-UX02** フォーカス復帰漏れ（rename / label / search / keymap）
  — 対応: b273132
  — `app/src/main.rs:5876,2269,6650,8525` / 4 発見を統合
- [x] **R2-UX02** リネームキャンセルが進行中の別操作まで cancel_op する — 対応: b273132
  — `app/src/main.rs:5955-5962`
- [x] **R2-UX02** プロパティの dir_size コールバックに path/serial 照合が無い — 対応: b273132
  — `app/src/main.rs:5671-5683`
- [x] **R3-DATA01** ドライブ一覧がホットプラグ未更新・source/fs_type 非表示 — 対応: b273132
  — `app/src/main.rs:2311-2325,7875-7904`
- [x] **R3-DATA01** ゴミ箱行が復元先 original_parent を表示しない — 対応: b273132
  — `app/src/trash_view.rs:140-163` / `app/src/main.rs:7507-7523`
- [x] **R4-ACC01** クリック可能 div の aria_label 欠落（タブ×/保存検索×/見出し/パンくず等） — 対応: b273132
  — `app/src/main.rs:2771-2798,6939-6953,7271-7289,7956`
- [x] **R4-ACC01** F1 ヘルプ/操作履歴がキーボードでスクロールできない
  — 対応: 制限を文書化して見送り（gpui-pre 0.3.12 の ScrollHandle に
  `scroll_by` が無く、プレーン div + overflow_y_scroll では実装不可）

### 優先度: 高（第15回シナリオ発掘・confirmed）

- [x] **R4-FLOW04** 同一 cwd の複数タブが再起動で1つに圧縮され履歴が失われる — 対応: 9cca8a4
  — `app/src/settings.rs:927,940` / `app/src/main.rs:8986`
- [x] **R4-FLOW04** 32 個超のオープンタブが再起動で黙って切り詰められる — 対応: 9cca8a4
  — `app/src/main.rs:2582` / `app/src/settings.rs:929`
- [x] **R4-FLOW04** 一時的に存在しないブックマーク/最近が永続設定から削除される — 対応: 24d6f0c
  — `app/src/main.rs:2176,2183`
- [x] **R4-FLOW04** 未知 enum 値1つで settings.json 全体が退避され全失 — 対応: 9cca8a4
  — `app/src/settings.rs:851,271` / `app/src/model.rs:313`

### 優先度: 中（第15回シナリオ発掘・confirmed）

- [x] **R2-FLOW02** Ctrl+Shift+F が検索ビューで発火しない — 対応: 24d6f0c
  — `app/src/main.rs:109-117,3542`
- [x] **R2-FLOW02** 検索ビューで Esc が操作キャンセルを奪う — 対応: 24d6f0c
  — `app/src/main.rs:3542-3560`
- [x] **R2-FLOW02** 検索結果の並び替えヘッダが設定だけ変え表示順は不変 — 対応: 24d6f0c
  — `app/src/main.rs:3750` / `app/src/search.rs:166`
- [x] **R2-FLOW02** 隠し切替が検索結果に反映されずステータスだけ偽装 — 対応: 24d6f0c
  — `app/src/main.rs:3706` / `app/src/search.rs:142`
- [x] **R2-FLOW02** フィルタ変更後にビューポートが先頭へスクロールされる — 対応: 24d6f0c
  — `app/src/main.rs:6657-6674`
- [x] **R2-FLOW02** 鮮度警告がフィルタ変更で上書きされる — 対応: 24d6f0c
  — `app/src/main.rs:2435,6483`
- [x] **R3-FLOW04** terminal_command の明示設定が再起動で無効化される — 対応: 9cca8a4
  — `app/src/settings.rs:223,971`
- [x] **R1-FLOW01** 初回起動で record_recent が呼ばれず「最近」が空 — 対応: 24d6f0c
  — `app/src/main.rs:1899`

### 優先度: 中（第17回テスト/文言発掘・対応済み）

- [x] **R4-DOC02** リリース版デバッグフック無効化の記述がコードと逆転
  — 対応: c369275
- [x] **R4-DOC02** タブ履歴が再起動で復元されないという README 記述が陳腐化
  — 対応: c369275
- [x] **R4-DOC02** テスト件数 353→365 / ops 43件・EXDEV 実テスト記載
  — 対応: c369275
- [x] **R3-ERR01** 保存失敗の stderr 導線 / 保存検索上限の誤説明 / 0件復元の成功報告
  — 対応: 00a7bbe
- [x] **R1-MUT01/R2-MUT02** 弱いオラクル7件（inode分岐/toggled/datetime/age/fingerprint/undo/tabs）
  — 対応: 4f8b4dd

### 優先度: 高（第18回シナリオ発掘・confirmed）

- [x] **R1-MW01** 保存タブの消失で active インデックスが別タブを指す — 対応: e2480b6
  — `app/src/main.rs:9015,9032`
- [x] **R1-MW01** 同一 cwd タブの復元で履歴を first-match が取り違える — 対応: e2480b6
  — `app/src/main.rs:9073-9076`

### 優先度: 中（第18回シナリオ発掘）

- [x] **R1-MW01** ウィンドウ終了時の bounds 失敗でタブ保存がスキップされる — 対応: e2480b6
  — `app/src/main.rs:9157-9166`
- [x] **R1-MW01** 一時的に存在しないタブが起動時に破棄され履歴も失われる — 対応: e2480b6
  — `app/src/main.rs:9015`
- [x] **R2-RES01** プロパティのメタデータ読み取りタスクに serial ガードが無い — 対応: e2480b6
  — `app/src/main.rs:5692-5741`
- [x] **R2-RES01** reconcile に実行中フラグが無く並走しスナップショットが蓄積 — 対応: e2480b6
  — `app/src/main.rs:2387-2401`
- [x] **R3-IO01** exists()/is_dir() が EACCES を NotFound に誤分類 — 対応: 45ebca4
  — `app/src/ops.rs:581,831,925`
- [x] **R3-IO01** 長いファイル名で .partial-* が NAME_MAX 超過 — 対応: 45ebca4
  — `app/src/ops.rs:287-315`
- [x] **R3-IO01** create_unique_folder が dangling symlink で100回失敗 — 対応: 45ebca4
  — `app/src/model.rs:766-777`

### 優先度: 高（第19回相互作用監査）

- [x] **R3-INT03** copy失敗時 cleanup が並行作成者の宛先を再帰削除する — 対応: 552098f
  — `app/src/ops.rs:635-648,911-917`
- [x] **R4-INT04** 未知 enum フォールバック値が次回 save で書き戻される — 対応: 4ecab1b
  — `app/src/settings.rs:200-210,924-936`

### 優先度: 中（第19回相互作用監査）

- [x] **R1-INT01** active-by-path が同一 cwd タブの先頭一致を選び履歴と active が食い違う — 対応: 194a3af
  — `app/src/main.rs:9095-9100`
- [x] **R2-INT02** 操作完了後の Search でスナップショットが無警告で古くなる — 対応: 194a3af
  — `app/src/main.rs:6404,6478`
- [x] **R2-INT02** watcher Removed が STATUS_SEARCH_STALE を消す — 対応: 194a3af
  — `app/src/main.rs:2562`
- [x] **R2-INT02** Search のキーホワイトリストが keymap 再バインドと矛盾 — 対応: 194a3af
  — `app/src/main.rs:122-130`
- [x] **R3-INT03** 短縮 .partial-* 名に nonce が無く並行コピーで衝突 — 対応: 552098f
  — `app/src/ops.rs:329-337`
- [x] **R4-INT04** sanitize_paths_for_save に合計バイト上限が無い — 対応: 4ecab1b
  — `app/src/settings.rs:1147-1178`
- [x] **R4-INT04** terminal_command のコメントが現行実装と矛盾 — 対応: 4ecab1b
  — `app/src/settings.rs:1444,2809`

### 優先度: 中（第20回保守性発掘）

- [x] **R1-MAINT01/R2-MAINT02** stale dead_code と古いコメント — 対応: 7ef58e6
  — settings 個人化 API / model コンストラクタ / strings F6-36
- [x] **R3-E2E01** app/README 内で E2E ツールの導入状態が矛盾（未導入 vs 導入済み） — 対応: f62a6df
  — `app/README.md:467` vs `:489`
- [x] **R3-E2E01** setup-e2e-input.sh が libxdo 依存を検証せず誤報告する
  — 対応: f62a6df
  — `scripts/setup-e2e-input.sh:55`
- [x] **R3-E2E01** E2E probe の実行証跡が未記録（ローカル完結で可能な準備） — 対応: f62a6df
  — `README.md:69,107`
- [x] **R4-WASM01** gpui-kit feature 調整による stable wasm 経路の可否が未検証 — 対応: 539292a
  — `notes/wasm-browser-notes.md:28-38`
- [x] **R4-WASM01** wasm 失敗ログが現行 Cargo.toml の trash ゲートと矛盾 — 対応: 539292a
  — `notes/wasm-browser-notes.md:43`

### 優先度: 中（第21回発掘）

- [x] **R1-CLI01** `--` 終端区切り未対応（区切り後のフラグも解釈される） — 対応: c6279be
  — `app/src/main.rs:9066,9111`
- [x] **R1-CLI01** 存在しない/ファイルの CLI 引数が診断なしでフォールバック — 対応: c6279be
  — `app/src/main.rs:9131,9184`
- [x] **R2-I1801** Ctrl+P の名称が「あいまいジャンプ」と「クイックジャンプ」で不一致 — 対応: 7043ce5
  — `app/src/strings.rs:86` / `app/src/main.rs:5400`
- [x] **R2-I1801** 用語規約「ディレクトリはエラー詳細のみ」が strings.rs 自身で違反 — 対応: 7043ce5
  — `app/src/strings.rs:9,83,280`
- [x] **R3-MIG01** 保存検索/ラベルの検証がトリム前で余白超過時に失われる — 対応: 7ddc803
  — `app/src/model.rs:592` / `app/src/settings.rs:1099`
- [x] **R3-MIG01** record_search が sane_query を通さない — 対応: 7ddc803
  — `app/src/settings.rs:674`
- [x] **R4-LOG01** 失敗/キャンセル転送が INFO「completed」として記録される — 対応: 7ddc803
  — `app/src/main.rs:4266`

### 優先度: 中（第22回発掘・confirmed）

- [x] **R2-IDEM01** ブックマーク上限追い出しでラベルが孤立し再追加で復活 — 対応: 2b1f302
  — `app/src/settings.rs:668-670`
- [x] **R2-IDEM01** 未ブックマーク行に設定したラベルが再起動で失われる — 対応: 2b1f302
  — `app/src/main.rs:7562` / `app/src/settings.rs:745`
- [x] **R3-CANCEL01** ゴミ箱 redo キャンセル時に部分完了結果が破棄される — 対応: 58b0179
  — `app/src/main.rs:4966-5001`

### 優先度: 高（ユーザビリティレビュー・confirmed）

- [x] **U-01** エラー/警告が成功と同じ緑色で表示される — main.rs:8617 — 対応: 4d36433
- [x] **U-02** フォーカスとホバーが同一色で枠線なし（コントラスト 1.09:1） — main.rs:7876,8104 — 対応: 4d36433
- [x] **U-03** 複数選択中にカーソル行が判別不能 — main.rs:7876-7882 — 対応: 4d36433
- [x] **U-04** F1 ヘルプへの UI 導線が皆無 — main.rs:8360-8546 — 対応: 4d36433
- [x] **U-05** ステータスバーにライブリージョンなし
  — 見送り: gpui-pre 0.3.12 に aria_live 公開 API が無い（コードコメントに明記）
- [x] **U-06** ゴミ箱確認ダイアログのキャンセルが英語 `cancel` — main.rs:6616 — 対応: 4d36433
- [x] **U-07** サイドバーがマウス専用（キーボードで移動不可） — main.rs:8198-8354 — 対応: 4d36433
- [x] **U-08** 非連続複数選択のキーボード手段なし — main.rs:3198-3257 — 対応: 4d36433
- [x] **U-09** 名前セルに Tooltip なし（検索の相対パスが切れる） — main.rs:7942 — 対応: 4d36433
- [x] **U-10** STATUS_PRIORITY が規約のみで描画に未実装
  — 対応: 4d36433

### 優先度: 中（ユーザビリティレビュー）

- [x] **U-11** 起動直後に未選択なのに「選択: 先頭項目」と表示 — main.rs:8596 — 対応: 4d36433
- [x] **U-12** 空フォルダのヒントがドロップのみで新規作成/貼り付けを案内しない — strings.rs:196 — 対応: 4d36433
- [x] **U-13** ツールバーアイコンにホバー Tooltip なし — main.rs:8783-8807 — 対応: 4d36433
- [x] **U-14** Ctrl+Shift+F が検索ビュー外で無音 — main.rs:6766 — 対応: 4d36433
- [x] **U-15** 選択なしのコピー/切取/ゴミ箱移動が無音 — main.rs:4074,4094,6564 — 対応: 4d36433
- [x] **U-16** サイドバーにスクロールが無く下位ドライブが到達不能 — main.rs:8167-8358 — 対応: 5b745fa
- [x] **U-17** グリッド列数計算が gap/padding を無視 — main.rs:9091,8001 — 対応: 5b745fa
- [x] **U-18** タブ名が最終コンポーネントのみで同名タブを区別不能 — main.rs:287 — 対応: 4d36433
- [x] **U-19** ステータスバーの小さな文字が低コントラスト
  — 対応: 4d36433 (C_SUBTEXT 適用)
- [x] **U-20** ディレクトリ読み込みエラーに復帰導線なし — main.rs:1165 — 対応: 4d36433
- [x] **U-21** 存在しない「場所」が薄色で並ぶだけで理由不明 — main.rs:8208 — 対応: 4d36433
- [x] **U-22** 複数起動通知が最後の1件で上書きされる — main.rs:2066-2079 — 対応: 4d36433

### 優先度: 高（制限プロファイル R1・confirmed）

- [x] **L01** 設定保存失敗が10経路で無視され成功表示される
  — 対応: limited-l01
- [x] **L02** 非UTF-8パスのブックマーク切替で panic / 誤報告
  — 対応: 8993961
- [x] **L03** プロパティ読み取りエラーが「パスがありません」と誤通知
  — 対応: 8993961
- [x] **L04** --skip-build の陳腐バイナリがバージョン文字列では検出不可
  build-deb.sh:112-120 — 対応: limited-l04
- [x] **L05** ゴミ箱ソートヘッダの aria_label が表示文言と不一致 — main.rs:7919 — 対応: limited-docs
- [x] **L06** app/README キーバインド表が F1 ヘルプと乖離（5キー未掲載）
  app/README.md:90-133 — 対応: limited-docs
- [x] **L07** Ctrl+P 名称が README と UI で不一致
  app/README.md:111, main.rs:5762 — 対応: limited-docs
- [x] **L08** 読み込み中/0件表示が strings 定数を迂回して直書き
  main.rs:5507,8896,5907 — 対応: limited-docs
- [x] **L09** strings 正規定数
  （KIND_*/AGE_*/VALUE_UNKNOWN）が model で未適用 — model.rs:412,737
  対応: limited-docs
- [x] **L10** DESIGN の非目的が実装済み機能（内容検索/タブ）と矛盾
  DESIGN.md:26,109 — 対応: limited-docs

### 優先度: 高（ユーザー要望・設定/外観）

- [x] **UX-A** 統合設定画面（TabBar 5タブ: 外観/ファイル/ショートカット/ターミナル/個人化）を追加 — 対応: 436fe1d/3e4caaf/4a3b0df/1cdbc0c
  — 新規 `app/src/settings_ui.rs` + `main.rs` 配線
- [x] **UX-B** ファイルアイコンを Breeze のシステム多色アイコンに変更（`gpui_kit::img(&Path)`） — 対応: 436fe1d/3e4caaf/4a3b0df/1cdbc0c
  — 新規 `app/src/system_icons.rs` + `icons.rs` + `main.rs` 描画
- [x] **UX-C** テーマ選択を Light / Dark / System の3値に拡張しライトテーマを実現 — 対応: 436fe1d/3e4caaf/4a3b0df/1cdbc0c
  — `settings.rs` スキーマ + `main.rs` パレット192箇所

### 優先度: 高（継続ループ R1・confirmed）

- [x] **C01** 設定画面のテーマ保存が apply_theme_mode を通らず本体に反映されない — 対応: loop-c01
  — main.rs:10102-10110 / 複数エージェントが一致指摘
- [x] **C02** キーマップ入力エラーが Shortcuts タブに表示されない — 対応: loop-c01
  — settings_ui.rs:602 / main.rs:10078
- [x] **C03** ライトテーマでツールバーアイコンがほぼ不可視（Mocha 固定色） — 対応: loop-c03
  — main.rs:9464-9500
- [x] **C04** 設定画面 Save が sort.dir を黙って上書きする — 対応: loop-c01
  — main.rs:10117-10126
- [x] **C05** 3値テーマに対して README/F1/DESIGN が2値のまま誤案内 — 対応: loop-docs
  — app/README.md / DESIGN.md

### 優先度: 中（継続ループ R1）

- [x] **C06** terminal/keymap 入力編集で dirty が立たない — settings_ui.rs:273 — 対応: loop-c01
- [x] **C07** --skip-build の鮮度ガード missing build.rs
  — 対応: loop-docs
- [x] **C08** stale allow(dead_code) と「配線待ち」コメント群 — 対応: 親
  — settings.rs / settings_ui.rs / system_icons.rs
- [x] **C09** Ctrl+, が両 README のキーバインド表に未掲載 — README.md / app/README.md — 対応: loop-docs
- [x] **C10** 設定画面のキーマップ構文 `action=chord;` が無説明 — settings_ui.rs:192 — 対応: loop-docs

### 優先度: 高（ループ再開 R1・confirmed）

- [x] **D01** 設定オーバーレイが入力へフォーカスせずキーボード操作が届かない — 対応: loop2-core
  — main.rs:9931-9972 / DESIGN.md:258 規約違反
- [x] **D02** 設定画面の検証エラーが入力修正後も消えない — 対応: loop2-core
  — main.rs:9960-9969 / 他ダイアログは clear_dialog_error 済み
- [x] **D03** chord にセミコロンを含むと設定画面の往復で Save が永久失敗 — 対応: loop2-core
  — main.rs:9925,10101 / parse_chord がセミコロンを許容

### 優先度: 中（ループ再開 R1）

- [x] **D04** ラベル設定失敗時に自動追加したブックマークだけが残る — 対応: loop2-icons
  — main.rs:2860-2874
- [x] **D05** Breeze ダーク用アイコンがテーマと連動せずライト用が優先される — 対応: loop2-icons
  — system_icons.rs:157-161
- [x] **D06** app/README が Breeze 実装を反映せず「Lucide のみ」と記載 — 対応: loop2-docs
  — app/README.md:358,551
- [x] **D07** DESIGN のアイコン戦略が P2 未実装のまま as-built と矛盾 — 対応: loop2-docs
  — DESIGN.md:98,875,1086
- [x] **D08** 設定画面の keymap が unbind 構文を扱えない — 対応: loop2-core
  — main.rs:10107-10118
- [x] **D09** 設定検索が現在タブのみで0件案内なし — 対応: loop2-search
  — settings_ui.rs:468-506
- [x] **D10** AppPalette の status_color / kind_color に直接テストなし — 対応: loop2-docs
  — main.rs:169-195
- [x] **D11** DESIGN 行数クレームが再ドリフト（1,138 vs 1,157） — README.md:83 — 対応: loop2-docs
- [x] **D12** アーキテクチャ表が settings_ui/system_icons を欠落 — app/README.md:364 — 対応: loop2-docs
- [x] **D13** 保存失敗時にメモリ上の設定・テーマがロールバックされない — main.rs:10186 — 対応: loop2-rollback

### 優先度: 高（ループ R1・confirmed）

- [x] **E01** 設定カード Shortcuts タブに検証エラーが表示されない — 対応: loop3-error
  — settings_ui.rs:591-618 / error は Terminal タブのみ
- [x] **E02** Alt+K 保存失敗時に keymap がロールバックされない — 対応: loop3-rollback
  — main.rs:4044-4074
- [x] **E03** Alt+T 保存失敗時に terminal_command がロールバックされない — 対応: loop3-rollback
  — main.rs:3873-3887
- [x] **E04** ウィンドウクローズ時の保存失敗が完全に無視される — 対応: loop3-rollback
  — main.rs:10641-10644 / Ctrl+Q はログ出力済み

### 優先度: 中（ループ R1）

- [x] **E05** toggle_bookmark_path が非UTF-8事前拒否を通らない — 対応: loop3-rollback
  — main.rs:2836-2850
- [x] **E06** DESIGN §2 テーマ行が2値・ダーク固定のまま — 対応: loop3-docs
  — DESIGN.md:108
- [x] **E07** review-report の「todo.md 参照」が空振り / 167 tests 断言 — 対応: loop3-docs
  — review-report.md:30,86,606,659
- [x] **E08** DESIGN as-built 差分表の見出し日付が初版のまま — 対応: loop3-docs
  — DESIGN.md:336
- [x] **E09** bookmark_label_is_settable が sane_path 条件を反映せず偽陽性 — 対応: loop3-docs
  — main.rs:565-571 / settings.rs:882
- [x] **E10** read_only セッションで「設定を保存しました」と誤表示 — 対応: loop3-rollback
  — settings.rs:1346 / main.rs:10294
- [x] **E11** 設定画面のテーマ案内が「自動更新」で実際は保存時 — 対応: loop3-error
  — settings_ui.rs:700-703
- [x] **E12** 未保存編集のキャンセル/Esc/背景クリックに確認なし — 対応: loop3-error
  — main.rs:10165 / settings_ui.rs:785
- [x] **E13** 設定UI文言が strings.rs 正規定数を迂回（LABEL_CANCEL 等） — 対応: loop3-error
  — settings_ui.rs:792 / main.rs:10294

### 優先度: 高（ループ R2・confirmed）

- [x] **F01** 設定検証エラーがステータスバーで緑（成功色）表示 — main.rs:10257 — 対応: loop4-core
- [x] **F02** 設定ダイアログに Enter 保存のキーボード経路なし — settings_ui.rs:272 — 対応: loop4-core
- [x] **F03** 設定再オープン時に保留フォーカス復元が検索欄から焦点を奪う — main.rs:9818 — 対応: loop4-core
- [x] **F04** read_only が設定画面以外の書き込みパスで成功通知になる — main.rs:509 — 対応: loop4-save

### 優先度: 中（ループ R2）

- [x] **F05** set_sort 保存失敗時に並び順がロールバックされない — main.rs:4628 — 対応: loop4-save
- [x] **F06** サイドバー幅が DESIGN/review-report で 200/214px のまま
  DESIGN.md:152, review-report.md:774 — 対応: loop4-docs
- [x] **F07** README CI run が古い（43734 vs 44063）— README.md:175 — 対応: loop4-docs
- [x] **F08** Windows cargo check 警告数が README 0 / review-report 2 で矛盾
  README.md:139 — 対応: loop4-docs
- [x] **F09** 初回実機検証 36 件は一次記録 35 件と矛盾 — README.md:97 — 対応: loop4-docs
- [x] **F10** DESIGN §8.2 配色契約が独自パレット実装と乖離 — DESIGN.md:869 — 対応: loop4-docs
- [x] **F11** 古いコメント群（共通フッタ/ロールバック2経路） — main.rs:517,10254 — 対応: loop4-core
- [x] **F12** set_prefer_dark の早期リターン/非入れ替え分岐が無テスト
  system_icons.rs:198,398 — 対応: loop4-docs
- [x] **F13** bookmark_label_is_settable の上限ちょうどが無テスト
  main.rs:566 — 対応: loop4-docs

### 優先度: 高（ループ R3・confirmed）

- [x] **G01** toggle_hidden 保存失敗時に show_hidden がロールバックされない — 対応: loop5-core
  — main.rs:4529 / stale コメントあり
- [x] **G02** read_only 通知が成功色（緑）表示 — main.rs:323-352 — 対応: loop5-core
- [x] **G03** ブックマーク/ラベル4経路が settings_write_status を迂回
  main.rs:2850,2953 — 対応: loop5-core

### 優先度: 中（ループ R3）

- [x] **G04** 検索履歴保存の失敗が完全握りつぶし — main.rs:7649 — 対応: loop5-core
- [x] **G05** 保存検索の入力エラーが緑表示 — main.rs:7958 — 対応: loop5-core
- [x] **G06** 保存検索の名前入力で Enter 保存が効かない — main.rs:7513 — 対応: loop5-ux
- [x] **G07** 設定UIテーマ変更保存時に切替通知なし — main.rs:10342 — 対応: loop5-core
- [x] **G08** 設定UIタブ切替にキーボード導線なし — settings_ui.rs:771 — 対応: loop5-ux
- [x] **G09** review-report の OKF verified が本文実測より古い
  review-report.md:13 — 対応: loop5-docs
- [x] **G10** DESIGN ops.rs テスト数 11 vs 実 47 — DESIGN.md:359 — 対応: loop5-docs
- [x] **G11** CI run「最新」が 11-17 のまま as-built 11-21
  README.md:179 — 対応: loop5-docs
- [x] **G12** settings_write_status の skipped+saved=false 未テスト
  main.rs:523 — 対応: loop5-ux
- [x] **G13** 設定エラー prefix がハンドラ側で未検証 — main.rs:10300 — 対応: loop5-ux
- [x] **G14** bookmark_toggle_rejection の NUL/長さが未テスト
  main.rs:2303 — 対応: loop5-ux
- [x] **G15** icon の text-x-rust 逆方向 / size=0 クランプ未テスト — system_icons.rs:326,228
  対応: loop5-docs
- [x] **G16** 設定UI内キー操作が F1/README に未記載
  strings.rs:81, app/README.md:114 — 対応: loop5-ux

### 優先度: 高（ループ R4・confirmed）

- [x] **H01** set_view 保存失敗時に view がロールバックされない（7経路目の漏れ） — 対応: loop6-core
  — main.rs:3769
- [x] **H02** STATUS_ACCESS_DENIED が成功色（緑）表示 — main.rs:363 — 対応: loop6-core
- [x] **H03** dirty な設定の Esc/背景クリックが無警告で破棄される — 対応: loop6-ux
  — settings_ui.rs:785 / main.rs:10174
- [x] **H04** 設定キーマップ欄が全置換・unbind 不可を説明していない — 対応: loop6-ux
  — settings_ui.rs:608

### 優先度: 中（ループ R4）

- [x] **H05** set_view が write_was_skipped を検査せず read_only 未通知
  main.rs:3774 — 対応: loop6-core
- [x] **H06** 6経路ロールバックテストが純関数のみで呼び出し漏れを検出不可 — main.rs:12570 — 対応: loop6-core
- [x] **H07** プロパティ STATUS_PATH_GONE の分類が未定義 — main.rs:2099 — 対応: loop6-core
- [x] **H08** テーマ表示名が strings.rs 定数ではなくハードコード — main.rs:10342 — 対応: loop6-ux
- [x] **H09** 設定検索がキー名 (Alt+K 等) にマッチしない — settings_ui.rs:345 — 対応: loop6-ux
- [x] **H10** XDG Portal readiness 判定の log 時刻が期限切れ
  main.rs:9071 — 対応: loop6-docs
- [x] **H11** OFFLINE/headless 検証手順が README にない — README.md:157 — 対応: loop6-docs
- [x] **H12** テスト数 450 がドキュメント未同期 — README.md:26 — 対応: loop6-docs
- [x] **H13** review-report markdownlint 4件が SNAPSHOT 注記と矛盾
  review-report.md:656 — 対応: loop6-docs
- [x] **H14** 全切替操作で settings 保存失敗時にエラー表示なし — main.rs:3774 — 対応: loop6-core
- [x] **H15** set_view の read_only/ephemeral テストなし
  main.rs:12570 — 対応: loop6-core

### 優先度: 高（ループ R5・confirmed）

- [x] **I01** 対象なし系の失敗通知が成功色（緑）で表示される — 対応: loop7-core
  — main.rs:342-358 / strings.rs の TARGET_GONE 等
- [x] **I02** settings_write_status が no-op 分岐で拒否理由をマスク — 対応: loop7-core
  — main.rs:555 / commit_label_editor, save_current_search

### 優先度: 中（ループ R5）

- [x] **I03** review-report の debug hook「release 残存」記述が矛盾 — 対応: loop7-docs
  — review-report.md:300-303
- [x] **I04** DESIGN の新規フォルダ名リテラルが実装と乖離 — 対応: loop7-docs
  — DESIGN.md:266,802 / model.rs:805
- [x] **I05** 設定ダイアログ内 Tab ナビゲーション不在 — 対応: loop7-docs
  — settings_ui.rs:771
- [x] **I06** 未保存破棄通知に復帰導線なし（strings 通知規約違反） — 対応: loop7-core
  — strings.rs:316

### 優先度: 中（ループ R6・confirmed）

- [x] **J01** load_tab/navigate_with_mode が settings_write_status を迂回
  対応: loop8-core
  — main.rs:3404,3667
- [x] **J02** status_message_constants_are_nonempty が定数を網羅していない — 対応: loop8-core
  — strings.rs:617
- [x] **J03** DESIGN §10.1 のテスト数 450 が実 461 と乖離 — DESIGN.md:996 — 対応: loop8-docs
- [x] **J04** settings_focus_order の Personalize/Alt+T が未テスト
  settings_ui.rs:1164 — 対応: loop8-ux
- [x] **J05** next_settings_focus の未知 current+後方が未テスト
  settings_ui.rs:510 — 対応: loop8-ux
- [x] **J06** 破棄通知の Ctrl+, が既定キーマップで未固定 — main.rs:1204 — 対応: loop8-core
- [x] **J07** F1 ヘルプに Tab/Shift+Tab 巡回が未記載 — strings.rs:165 — 対応: loop8-ux
- [x] **J08** 未保存案内文言が2通りに分かれ未集約 — settings_ui.rs:793,939 — 対応: loop8-ux
- [x] **J09** review-report の「DESIGN 履歴 v0.1」が解消済みで陳腐化
  review-report.md:636 — 対応: loop8-docs

### 優先度: 高（ループ R7・confirmed）

- [x] **K01** Ctrl+, が keymap override で evict され案内が壊れる — 対応: loop9-core
  — main.rs:1265 / settings.rs:381
- [x] **K02** confirm_keymap の unbind no-op が拒否理由をマスク — 対応: loop9-core
  — main.rs:4222-4240
- [x] **K03** STATUS_NO_SELECTION が警告色から漏れる — main.rs:324 — 対応: loop9-core

### 優先度: 中（ループ R7）

- [x] **K04** keymap ヒントが unbind 非対応と案内するが実装は受理 — 対応: loop9-ux
  — settings_ui.rs:429
- [x] **K05** 進行中ステータスが STATUS_OP_IN_PROGRESS_SUFFIX を迂回 — 対応: loop9-core
  — main.rs:5597,5732,7104
- [x] **K06** DESIGN §10.1 クロスリファレンスが 461 のまま — DESIGN.md:997 — 対応: loop9-tests
- [x] **K07** 設定検索の他タブ一致ヒントにジャンプ手段なし — settings_ui.rs:860 — 対応: loop9-ux
- [x] **K08** ラジオ/スイッチが Tab 巡回から除外されキーボード不可 — settings_ui.rs:491 — 対応: loop9-ux
- [x] **K09** preview.rs 画像判定の未テスト分岐（webp/gif/MM TIFF 等） — 対応: loop9-tests
- [x] **K10** error.rs BoxedCause::clone が無テスト — 対応: loop9-tests
- [x] **K11** mounts.rs unescape_mount_field の不正オクタル分岐が無テスト — 対応: loop9-tests
- [x] **K12** system_icons.rs image-x- 生成分岐が無テスト — 対応: loop9-tests
- [x] **K13** Terminal タブで未保存ヒントが重複表示 — settings_ui.rs:790,936 — 対応: loop9-ux

### 優先度: 高（ループ R7・実バグ修正済み）

- [x] **K14** unescape_mount_field がマルチバイト文字で panic — 対応: 親（bytes スライスに修正、回帰テスト追加）

### 優先度: 高（ループ R8・confirmed）

- [x] **L01** open_settings 自身を rebind すると Ctrl+, が消え案内が矛盾 — 対応: loop10-core
  — main.rs:1004 / settings.rs:396
- [x] **L02** is_reserved_chord が内部空白入り chord をすり抜ける — 対応: loop10-core
  — settings.rs:384
- [x] **L03** 設定検索が Alt+S / Alt+1〜5 にマッチしない — 対応: loop10-ux
  — settings_ui.rs:406

### 優先度: 中（ループ R8）

- [x] **L04** Search ビューで Ctrl+, が keymap に到達しない — 対応: loop10-core
  — main.rs:267
- [x] **L05** 他タブ一致ヒントが Alt+数字の具体的番号を案内しない — 対応: loop10-ux
  — settings_ui.rs:481
- [x] **L06** 対象なし系ステータスが成功色のまま — main.rs:325 — 対応: loop10-ux + 親（main.rs マーカー追加）
- [x] **L07** 既定ショートカット一覧に Ctrl+, / Alt+数字 / Alt+S 未掲載 — 対応: loop10-ux
  — settings_ui.rs:793
- [x] **L08** generic_icon_for_kind の 5 分岐が未テスト — system_icons.rs:294 — 対応: loop10-tests
- [x] **L09** text-x- 二重プレフィックスガード未テスト — system_icons.rs:336 — 対応: loop10-tests
- [x] **L10** normalize_events の空 paths→Rescan 分岐が未テスト — watcher.rs:56 — 対応: loop10-tests
- [x] **L11** human_size の T/P 単位分岐が未テスト — model.rs:726 — 対応: loop10-tests
- [x] **L12** DESIGN 471 件の Root Proof が存在しない README:463 を参照 — 対応: loop10-tests
  — DESIGN.md:997
- [x] **L13** review-report 却下根拠の README:133 が現行と不一致 — 対応: loop10-tests
  — review-report.md:344

### 優先度: 高（ループ R9・confirmed）

- [x] **M01** ゴミ箱 Undo の部分復元通知が成功色（緑） — main.rs:632 — 対応: loop11-core
- [x] **M02** settings.rs の keymap 重複排除が chord 未正規化
  settings.rs:1331 — 対応: loop11-core
- [x] **M03** sane_keymap_entry が重複修飾子を受理 — settings.rs:417 — 対応: loop11-core

### 優先度: 中（ループ R9）

- [x] **M04** 設定ダイアログ keymap ヒントに open_settings 予約の説明欠落
  settings_ui.rs:455 — 対応: loop11-ux（M08 配線は親）
- [x] **M05** keymap unbind no-op 通知が警告色から漏れる — main.rs:561 — 対応: loop11-core
- [x] **M06** 「見つかりません」系の動的拒否通知が緑 — main.rs:8244,5091 — 対応: loop11-core
- [x] **M07** README の CI/検証範囲が check.sh・ci.yml より古い
  README.md:172 — 対応: loop11-tests
- [x] **M08** 設定のテーマ/ビュー/隠しファイル行がキーボード不可
  settings_ui.rs:534 — 対応: loop11-ux（M08 配線は親）
- [x] **M09** 設定検索が画面表示文言を索引せず0件になる
  settings_ui.rs:357 — 対応: loop11-ux（M08 配線は親）
- [x] **M10** preview.rs step_preview_index 前方ラップ未テスト
  preview.rs:241 — 対応: loop11-tests
- [x] **M11** png_dimensions の IHDR 不一致ガード未テスト
  preview.rs:30 — 対応: loop11-tests
- [x] **M12** error.rs with_cause no-op 分岐未テスト — error.rs:177 — 対応: loop11-tests
- [x] **M13** grid_columns の非有限幅ガード未テスト — model.rs:303 — 対応: loop11-tests
- [x] **M14** 設定検索0件状態に復帰導線なし — settings_ui.rs:508 — 対応: loop11-ux（M08 配線は親）

### 優先度: 高（ループ R10・confirmed）

- [x] **N01** t/v/h 行ショートカットが修飾キー付きでも発火 — main.rs:4502 — 対応: loop12-core
- [x] **N02** 「開けません」系拒否がエラーマーカー未登録で緑 — main.rs:838,4033 — 対応: loop12-status
- [x] **N03** 「ディレクトリ表示でのみ」系5文言が定数化・分類漏れ — main.rs:6040 — 対応: loop12-status

### 優先度: 中（ループ R10）

- [x] **N04** t/v/h が F1 ヘルプと設定行に未案内 — strings.rs:160 — 対応: loop12-docs
- [x] **N05** Alt+S に入力欄フォーカスガードなし — main.rs:4485 — 対応: loop12-core
- [x] **N06** Alt+K 成功通知が正規化前 chord を表示 — main.rs:4293 — 対応: loop12-core
- [x] **N07** 検索キーワード「表示」欠落 — settings_ui.rs:421 — 対応: loop12-docs
- [x] **N08** 空対象・上限超過の動的拒否が緑 — main.rs:8393,3500 — 対応: loop12-status
- [x] **N09** 検索語が空の拒否が緑 — main.rs:7840 — 対応: loop12-status
- [x] **N10** README の unbind 説明が app/README と矛盾 — README.md:62 — 対応: loop12-docs
- [x] **N11** DESIGN 未実装「のみ」が他ドキュメントと矛盾 — DESIGN.md:123 — 対応: loop12-docs
- [x] **N12** 予約キー説明に open_settings / Ctrl+, 欠落 — README.md:71 — 対応: loop12-docs
- [x] **N13** DESIGN §11 性能「未計測」が実測記録と矛盾 — DESIGN.md:1176 — 対応: loop12-docs
- [x] **N14** README 空フォルダ例が現行文言と乖離 — app/README.md:351 — 対応: loop12-docs
- [x] **N15** ops.rs verify_entry の symlink/種別/キャンセル分岐が無テスト — 対応: loop12-docs
- [x] **N16** settings.rs canonical_chord_key のフォールバック3分岐が無テスト — 対応: loop12-core
- [x] **N17** preview.rs 不正 UTF-8 ヘッダ / search.rs search_roots の分岐が無テスト — 対応: loop12-docs

### 優先度: 中（ループ R11・confirmed）

- [x] **P01** settings_row_key_is_bare が function 修飾子を見落とす — 対応: loop13-core
  — main.rs:1820
- [x] **P02** F1 が preview/location ガードより前に処理されヘルプが重なる — 対応: loop13-core
  — main.rs:4603
- [x] **P03** 検索名フィルタ未オープン時の拒否案内が緑 — main.rs:7791 — 対応: loop13-core
- [x] **P04** 内容検索の行読み取り中キャンセル分岐が未テスト — search.rs:311 — 対応: loop13-tests
- [x] **P05** 検索0件ヒントに名前フィルタ(Ctrl+Shift+F)導線なし — 対応: loop13-ux（P08 配線は親）
  — strings.rs:233
- [x] **P06** STATUS_SETTINGS_UNSAVED_HINT コメントが K13 実装と矛盾 — 対応: loop13-core
  — strings.rs:361

### 優先度: 低（ループ R11）

- [x] **P07** F1 上書き通知が evict された既定バインドを表示しない — 対応: 親（前ラウンド保留分）
  — main.rs:1414
- [x] **P08** 移動/プレビュー/特殊ファイルの拒否文言が定数化漏れ — 対応: loop13-ux（P08 配線は親）
  — main.rs:5190,6099,5493
- [x] **P09** DESIGN 変更履歴 11-21 行が 507 同期ほかを記録していない — 対応: loop13-ux（P08 配線は親）
  — DESIGN.md:1213
- [x] **P10** 設定ダイアログ内にキーボード凡例なし（F1 は塞がれる） — 対応: loop13-ux（P08 配線は親）
  — settings_ui.rs:1165
- [x] **P11** create_unique_folder の 100 回枯渇エラーが未テスト — 対応: loop13-tests
  — model.rs:813
- [x] **P12** normalize_events Both 単一パス/ SafeWatcher init_error=None が無テスト
  — 対応: loop13-tests
  — watcher.rs:64,110
- [x] **P13** sniff_image_format の 1024 バイト走査上限が未テスト — 対応: loop13-tests
  — preview.rs:95

### 優先度: 中（ループ R12・confirmed）

- [x] **Q01** F1 通知が空になった既定キーを検出しない（P07 不完全） — 対応: loop14-core
  — main.rs:1419
- [x] **Q02** P08 拒否定数が色分類規約テストに未登録 — strings.rs:294 — 対応: loop14-core
- [x] **Q03** STATUS_BOOKMARK_NOT_PERSISTABLE が main.rs ローカルのまま — 対応: loop14-core
  — main.rs:406
- [x] **Q04** 516 tests の内訳説明が旧 P0 期のまま — app/README.md:27 — 対応: loop14-tests
- [x] **Q05** 設定フッター凡例が逆方向キーを省略 — settings_ui.rs:395 — 対応: loop14-ux
- [x] **Q06** README に設定内 F1 塞がれと凡例代替が未説明 — app/README.md:115 — 対応: loop14-ux
- [x] **Q07** EMPTY_TRASH に復帰導線なし・LOAD_ERROR_HINT 未検証 — 対応: loop14-ux
  — strings.rs:221
- [x] **Q08** バージョン検索キーワードが表示内容と不一致 — settings_ui.rs:588 — 対応: loop14-ux

### 優先度: 低（ループ R12）

- [x] **Q09** search_contents の max_depth 分岐が無テスト — search.rs:393 — 対応: loop14-tests
- [x] **Q10** FuzzySource::label() の 5 分岐が無テスト — main.rs:1477 — 対応: loop14-core
- [x] **Q11** step_preview_index の delta=0 が無テスト — preview.rs:236 — 対応: loop14-tests
- [x] **Q12** sniff_image_format の PNM 境界が無テスト — preview.rs:94 — 対応: loop14-tests
- [x] **Q13** reorder_breeze_themes 片方欠落が無テスト — system_icons.rs:399 — 対応: loop14-tests
- [x] **Q14** LOAD_ERROR_HINT が README 空状態一覧になし — app/README.md:351 — 対応: loop14-ux

### 優先度: 中（ループ R13・confirmed）

- [x] **R01** F1 通知が別アクションに奪われた既定 chord を未報告 — 対応: loop15-core
  — main.rs:1441
- [x] **R02** STATUS_ACCESS_DENIED が main.rs ローカルで分類表から漏れ — 対応: loop15-core
  — main.rs:414
- [x] **R03** terminal_failed_status の起動拒否がインラインリテラル — 対応: loop15-core
  — main.rs:889
- [x] **R04** saved_search_save_status の上限拒否がインラインリテラル — 対応: loop15-core
  — main.rs:565
- [x] **R05** ラベル設定拒否文言が strings.rs 正規定義から漏れ — 対応: loop15-core
  — main.rs:3196
- [x] **R06** Alt+K ヒントに open_settings の unbind 不可が未記載 — 対応: loop15-ux
  — strings.rs:213
- [x] **R07** 設定検索のキー名検索が UI/F1 に未案内 — 対応: loop15-ux
  — settings_ui.rs:222

### 優先度: 低（ループ R13）

- [x] **R08** settings_ui.rs コメントが F1 ヘルプを未実装と誤記 — 対応: loop15-ux
  — settings_ui.rs:532
- [x] **R09** 起動エラー色テストが STATUS_INVALID_CLI_DIR を迂回 — 対応: loop15-core
  — main.rs:11349
- [x] **R10** 設定行説明の「フォルダー」表記ゆれ — settings_ui.rs:541 — 対応: loop15-ux
- [x] **R11** F1 ヘルプの Esc が未保存破棄を反映していない — strings.rs:167 — 対応: loop15-ux
- [x] **R12** 既定ショートカット一覧に Alt+Shift+T 欠落 — settings_ui.rs:545 — 対応: loop15-ux

### 優先度: 中（ループ R14・confirmed）

- [x] **S01** 保存検索の入力必須エラーがインライン拒否リテラル — 対応: loop16-core
  — main.rs:8374
- [x] **S02** review-report 総評の「132 件のヘッドレス テスト」が現在形・誤分類 — 対応: loop16-tests
  — review-report.md:21
- [x] **S03** read_image_preview の後段サイズガード未到達＋誤コメント — 対応: loop16-tests
  — preview.rs:134,472
- [x] **S04** acquire_save_lock の古いロック回収/タイムアウト分岐が無テスト — 対応: loop16-core
  — settings.rs:1464
- [x] **S05** 既定ショートカット一覧に Ctrl+Shift+F 欠落 — 対応: loop16-ux
  — settings_ui.rs:554

### 優先度: 低（ループ R14）

- [x] **S06** 永続 keymap eviction が Ctrl の alt don't-care と不一致 — 対応: loop16-core（実バグ修正）
  — main.rs:1141
- [x] **S07** ターミナル無効化の理由・復帰導線がインライン — 対応: loop16-core
  — main.rs:880
- [x] **S08** ops.rs 固定テスト数「50 件」が動的方針と不統一 — 対応: loop16-tests
  — app/README.md:293 / DESIGN.md:366
- [x] **S09** README キーバインド表に F4 行なし — app/README.md:92 — 対応: loop16-ux
- [x] **S10** 設定 About にライセンス/ソース導線なし — settings_ui.rs:1161 — 対応: loop16-ux

### 優先度: 中（ループ R15・confirmed）

- [x] **T01** settings.rs validate が Ctrl alt don't-care に未追従（S06 ロード時） — 対応: loop17-core（実バグ修正）
  — settings.rs:431
- [x] **T02** スキップ通知文言が STATUS_SKIPPED_* を迂回し揺れている — 対応: loop17-core
  — main.rs:5303,5604
- [x] **T03** 名前変更ダイアログの拒否リテラル未定数化 — main.rs:7375 — 対応: loop17-core
- [x] **T04** ブックマーク拒否が原因を「非UTF-8」と誤表示 — strings.rs:308 — 対応: loop17-core（実バグ修正）
- [x] **T05** ops.rs 件数の grep 計上が review-report 訂正と矛盾 — 対応: loop17-tests
  — app/README.md:294 / DESIGN.md:366
- [x] **T06** ショートカット一覧が F1/README と乖離 — settings_ui.rs:557 — 対応: loop17-ux
- [x] **T07** 設定検索キーワードが表示されないキー名を広告 — 対応: loop17-ux
  — settings_ui.rs:485

### 優先度: 低（ループ R15）

- [x] **T08** trash_undo_status の失敗理由がインライン断片 — main.rs:673 — 対応: loop17-core
- [x] **T09** 履歴空状態/パスツールチップの拒否文言残存 — main.rs:5666 — 対応: loop17-core
- [x] **T10** strings.rs 色分類表に STATUS_PATH_GONE 未登録 — 対応: loop17-core
  — strings.rs:914
- [x] **T11** chords_collide / canonical_chord の解析不能フォールバック未テスト — 対応: loop17-core
  — main.rs:1139,1117
- [x] **T12** History::restore の forward truncation 分岐未テスト（実バグ修正）
  — history.rs:80 / 対応: loop17-tests
- [x] **T13** 隠しファイルのみ空状態が復帰キーテスト対象外 — main.rs:1773 — 対応: loop17-ux

### 優先度: 高（ループ R16・confirmed）

- [x] **U01** 設定ロード時の back スタック truncate が直近履歴を失う（実バグ） — 対応: loop18-history（実バグ修正）
  — settings.rs:1366,686
- [x] **U02** 保存検索の上限以外拒否が「上限に達しました」と誤通知 — 対応: loop18-history
  — main.rs:8419 / settings.rs:913
- [x] **U03** 設定保存失敗の汎用メッセージが原因を誤案内 — 対応: 親（save_with_reason で理由判別）
  — strings.rs:399 / settings.rs:1585

### 優先度: 中（ループ R16）

- [x] **U04** ブックマークラベル設定拒否に理由なし — main.rs:734 — 対応: loop18-history + loop18-status
- [x] **U05** ラベル未設定拒否がインライン字面で緑 — main.rs:3269 — 対応: loop18-status（実バグ修正）
- [x] **U06** ゴミ箱読み込み失敗リテラル残存 — main.rs:2944 — 対応: loop18-status
- [x] **U07** Undo「が存在しません」断片未定数化 — main.rs:5876 — 対応: loop18-status
- [x] **U08** 転送進捗が STATUS_CANCEL_HINT を迂回 — main.rs:5379 — 対応: loop18-status

### 優先度: 低（ループ R16）

- [x] **U09** undo_action_detail の4分岐が無テスト — main.rs:5769 — 対応: loop18-status

### 優先度: 中（ループ R17・confirmed）

- [x] **V01** 起動時設定保存失敗が具体的理由を表示しない — main.rs:2904 — 対応: loop19-core（実バグ修正）
- [x] **V02** Alt+K ヒントが予約キー「,」を説明していない — strings.rs:213 — 対応: loop19-core
- [x] **V03** ターミナル入力拒否文言が二重インライン — main.rs:4344,10898 — 対応: loop19-core
- [x] **V04** キーマップ拒否・上限文言が散在（上限は二重定義） — 対応: loop19-core
  — main.rs:4514,4530,10866,10876
- [x] **V05** save_with_reason のエラー種別写像が無テスト — settings.rs:1503 — 対応: loop19-tests
- [x] **V06** settings_write_status の理由優先分岐が無テスト — main.rs:651 — 対応: 親
- [x] **V07** 設定検索が永続キー名(terminal_command等)を検索できない — 対応: loop19-ux
  — settings_ui.rs:672

### 優先度: 低（ループ R17）

- [x] **V08** 古い allow(dead_code)/コメントが理由判別配線後も残存 — 対応: loop19-core
  — strings.rs:404,main.rs:620
- [x] **V09** History::max_back と MAX_HISTORY_ENTRIES の一致が無検証 — 対応: loop19-tests
  — history.rs:8
- [x] **V10** ステータスバー無い保存経路が理由をログに残さない — 対応: loop19-core
  — main.rs:5033
- [x] **V11** review-report の DESIGN 行番号参照が drift — review-report.md:649 — 対応: loop19-ux
- [x] **V12** 「README に現在値」案内が動的件数化後も空参照 — 対応: loop19-ux
  — review-report.md:26
- [x] **V13** 隠しファイル空状態の Ctrl+H がゴミ箱で無効 — 対応: loop19-core（実バグ修正）
  — main.rs:1871
- [x] **V14** STATUS_RESCAN_FAILED に復帰導線なし — strings.rs:380 — 対応: loop19-core

### 優先度: 中（ループ R18・confirmed）

- [x] **W01** ターミナル/新タブ拒否文の前半が main.rs インライン残存 — 対応: loop20-core
  — main.rs:4298,6453
- [x] **W02** 検索開始案内が未定数化 — main.rs:8089 — 対応: loop20-core
- [x] **W03** DESIGN §8.4 が存在しない EMPTY_DIR を例示 — DESIGN.md:916 — 対応: loop20-docs
- [x] **W04** KeyMap::reset「データ層のみ」記述が配置誤り — 対応: loop20-docs
  — app/README.md:530
- [x] **W05** review-report Windows3件フォローアップが解消後も未処理 — 対応: loop20-docs
  — review-report.md:686
- [x] **W06** STATUS_RESCAN_FAILED が色分類テーブルから漏れ — 対応: loop20-core
  — strings.rs:393
- [x] **W07** 行ショートカットの入力中無効がダイアログ内で案内なし — 対応: loop20-ux
  — settings_ui.rs:606
- [x] **W08** read_only 通知に理由と復帰導線なし — main.rs:654 — 対応: loop20-core（実バグ修正）
- [x] **W09** F1 がディレクトリ表示限定キーの制約を説明せず — 対応: loop20-ux
  — strings.rs:88

### 優先度: 低（ループ R18）

- [x] **W10** STATUS_CANCELLING が色分類から漏れ（進行中は緑の意図か） — 対応: loop20-core
  — main.rs:330
- [x] **W11** DESIGN §8.4「すべて集約・散在禁止」が as-built と未同期 — 対応: loop20-docs
  — DESIGN.md:916

### 優先度: 中（ループ R19・confirmed）

- [x] **X01** save 失敗理由定数が error マーカー未登録 — main.rs:377 — 対応: loop21-core
- [x] **X02** 未保存破棄通知が成功緑（データ損失通知） — main.rs:11645 — 対応: loop21-core（実バグ修正）
- [x] **X03** ラベル未設定拒否が main.rs インライン — main.rs:808 — 対応: loop21-core
- [x] **X04** ゴミ箱読込失敗が main.rs インライン — main.rs:817 — 対応: loop21-core
- [x] **X05** ops.rs validate_file_name の Windows 分岐が無テスト — 対応: loop21-tests
- [x] **X06** ops.rs with_cleanup_failure の NotFound 分岐が無テスト — 対応: loop21-tests

### 優先度: 低（ループ R19）

- [x] **X07** typing ガード注記が Alt+S 一覧行に未同期 — settings_ui.rs:682 — 対応: loop21-ux
- [x] **X08** Undo「が存在しません」が main.rs ローカル — main.rs:832 — 対応: loop21-core
- [x] **X09** CLI 起動ディレクトリ拒否が main.rs ローカル — main.rs:11026 — 対応: loop21-core
- [x] **X10** review-report の DESIGN 行番号が drift — review-report.md:651 — 対応: loop21-ux
- [x] **X11** error.rs fs_message 空 OS メッセージ分岐が無テスト — 対応: loop21-tests
- [x] **X12** system_icons push_unique_path 重複分岐が無テスト — 対応: loop21-tests
- [x] **X13** 入力ガード注記が3通りに分岐 — strings.rs:183 / README:154 — 対応: loop21-ux
- [x] **X14** Personalize キーワード「情報」「about」が表示されず — 対応: loop21-ux
  — settings_ui.rs:713
- [x] **X15** README キー表と F1 の自動同期テストなし — 対応: loop21-ux

### 優先度: 中（ループ R20・confirmed）

- [x] **Y01** 色分類の strings.rs↔main.rs 相互検証テストなし — 対応: loop22-core（実バグ修正）
  — strings.rs:1126 / main.rs:330
- [x] **Y02** スキップ理由語が main.rs インライン残存 — main.rs:5452,5749 — 対応: loop22-core
- [x] **Y03** mounts.rs path_is_visible が非vacuousに検証されず — 対応: loop22-tests
  — mounts.rs:59
- [x] **Y04** 設定検索が並び順選択肢名と昇順/降順をキーワードに欠く — 対応: loop22-ux
  — settings_ui.rs:458
- [x] **Y05** キー凡例と README/F1 の同期保証なし — settings_ui.rs:396 — 対応: loop22-ux
- [x] **Y06** ゴミ箱空状態テストがローカル副本を検証 — settings_ui.rs:1722 — 対応: loop22-ux

### 優先度: 低（ループ R20）

- [x] **Y07** タブ上限拒否の接尾辞「 個までです」がインライン — 対応: loop22-core
  — main.rs:3788
- [x] **Y08** STATUS_WATCH_INIT_FAILED が分類表・マーカーから漏れ — 対応: loop22-core
  — strings.rs:440
- [x] **Y09** watcher.rs 旧監視解除失敗の集約が未検証 — watcher.rs:244 — 対応: loop22-tests
- [x] **Y10** search_roots の canonicalize 異表記重複排除が未検証 — 対応: loop22-tests
  — search.rs:227
- [x] **Y11** F1「設定ダイアログ」行に他タブ一致案内なし — strings.rs:186 — 対応: loop22-ux
- [x] **Y12** F1 Alt+K 行に unbind 構文と Enter/Esc の説明なし — 対応: loop22-ux
  — strings.rs:217

### 優先度: 高（ループ R21・confirmed）

- [x] **Z01** 検索0件ヒントの Ctrl+F が検索ビューで無効（V13 同型） — 対応: loop23-ux（実バグ修正）
  — strings.rs:282 / main.rs:253
- [x] **Z02** STATUS_RESCAN_FAILED が STATUS_CHANGED で上書きされる — 対応: loop23-core（実バグ修正）
  — main.rs:3639
- [x] **Z03** 混在スキップ時に same_dir 理由が最終通知から消える — 対応: loop23-core（実バグ修正）
  — main.rs:5452

### 優先度: 中（ループ R21）

- [x] **Z04** ops.rs partial 名の create_new リトライ/枯渇が無テスト — 対応: loop23-tests
- [x] **Z05** watcher.rs 同一セット+last_error ありの再監視が無テスト — 対応: loop23-tests
- [x] **Z06** system_icons.rs new() の XDG テーマ順序が無テスト — 対応: loop23-tests
- [x] **Z07** 予約キー一覧が README/F1/設定ヒントに伝播せず — 対応: loop23-ux
  — strings.rs:268
- [x] **Z08** 設定検索0件の復帰導線が F1 に未反映 — strings.rs:203 — 対応: loop23-ux
- [x] **Z09** ダイアログ脚文の Enter/Esc 表記が F1/README と不一致 — 対応: loop23-core
  — main.rs:4501

### 優先度: 低（ループ R21）

- [x] **Z10** ops.rs unique_path の10,000回枯渇が無テスト — 対応: loop23-tests
- [x] **Z11** ops.rs with_cleanup_failure の Io 元パス分岐が無テスト — 対応: loop23-tests
- [x] **Z12** キーマップ上限64件が入力前ヒントに未記載 — 対応: loop23-ux
  — strings.rs:270
- [x] **Z13** KeyMap::reset が設定 UI から操作できない — 対応: loop23-ux

### 優先度: 中（ループ R21 フォローアップ・confirmed）

- [x] **Z14** 再スキャン成功時に STATUS_RESCAN_FAILED がクリアされない — 対応: 親（clear_rescan_failure_status）

### 優先度: 中（ループ R22・confirmed）

- [x] **AA01** F5/変更なし reconcile 成功で再スキャン失敗がクリアされない — 対応: loop24-core（実バグ修正）
  — main.rs:3044,3585
- [x] **AA02** app/README ホットプラグ「未対応」が実装と矛盾 — 対応: loop24-ux
  — app/README.md:630
- [x] **AA03** 保存検索・ブックマークラベルが4文書に未反映 — 対応: loop24-ux
  — app/README.md:143
- [x] **AA04** search_view_action_allowed の9分岐が未テスト — 対応: loop24-core
  — main.rs:293
- [x] **AA05** 設定検索キーマップ例（open_preview 等）が検索不能 — 対応: loop24-ux
  — settings_ui.rs:616

### 優先度: 低（ループ R22）

- [x] **AA06** 監視更新件数 14→15 vs 12→13 が別 runs と未明示 — 対応: loop24-ux
  — README.md:108 / app/README.md:473
- [x] **AA07** record_trashed_paths の非UTF-8分岐が未テスト — main.rs:2548 — 対応: loop24-core

### 優先度: 中（ループ R23・confirmed）

- [x] **AB01** Alt+再割当が Search ビューのキー許可をバイパス（実バグ） — 対応: loop25-core（実バグ修正）
  — main.rs:329
- [x] **AB02** Search の watch 削除が RESCAN_FAILED 保持を破る — 対応: loop25-core
  — main.rs:3695
- [x] **AB03** STATUS_SKIP_REASON_SAME_DIRECTORY が非空インベントリ未登録 — 対応: loop25-core
  — strings.rs:1148
- [x] **AB04** review-report verified が 2026-11-22 追記より前 — 対応: loop25-docs
  — review-report.md:13
- [x] **AB05** DESIGN 変更履歴に 2026-11-22 追記未反映 — 対応: loop25-docs
  — DESIGN.md:1220

### 優先度: 低（ループ R23）

- [x] **AB06** ゴミ箱 F5 成功が RESCAN_FAILED をクリアしない — main.rs:3124 — 対応: loop25-core
- [x] **AB07** STATUS_WATCH_PARTIAL/DISABLED が分類表から漏れ — 対応: loop25-core
  — strings.rs:513
- [x] **AB08** 監視スロット「ゴミ箱（自動更新なし）」がインライン — 対応: loop25-core
  — main.rs:10231
- [x] **AB09** README 未実装列挙が他文書と不一致 — README.md:145 — 対応: loop25-docs
- [x] **AB10** ターミナル無効通知に Ctrl+, 導線なし — strings.rs:685 — 対応: loop25-core
- [x] **AB11** ゴミ箱 Ctrl+H 拒否に「出る」導線なし — strings.rs:384 — 対応: loop25-core
- [x] **AB12** 検索0件の Esc 推奨が未保存破棄リスクを警告せず — 対応: loop25-ux
  — settings_ui.rs:806
- [x] **AB13** 色検索が表示専用パレット行へ案内される — 対応: loop25-ux
  — settings_ui.rs:740
- [x] **AB14** 昇順/降順キーワードが設定行に存在しない — 対応: loop25-ux
  — settings_ui.rs:474

### 優先度: 高（ループ R24・confirmed）

- [x] **AC01** Search の Rescan/Renamed が RESCAN_FAILED を上書き（実バグ） — 対応: loop26-core（実バグ修正）
  — main.rs:3685
- [x] **AC02** Search の Ctrl+Shift+C が allowlist をすり抜け（実バグ） — 対応: loop26-core（実バグ修正）
  — main.rs:293
- [x] **AC03** Search で RESCAN_FAILED の F5 ヒントが届かない（実バグ） — 対応: loop26-core（実バグ修正）
  — main.rs:293 / strings.rs:533

### 優先度: 中（ループ R24）

- [x] **AC04** WATCH_PARTIAL/DISABLED が status_color 回帰表に未登録 — 対応: loop26-core
  — main.rs:11588
- [x] **AC05** display_name_for の best 維持分岐が未テスト — main.rs:604 — 対応: loop26-core
- [x] **AC06** 設定検索が Files タブの見出し/ラベルを検索できない — 対応: loop26-ux
  — settings_ui.rs:1292
- [x] **AC07** 読込失敗ヒントが Trash/Search で無効な Ctrl+L を案内 — 対応: loop26-ux
  — strings.rs:355
- [x] **AC08** キーマップ不正通知に入力形式の案内なし — strings.rs:562 — 対応: loop26-ux

### 優先度: 低（ループ R24）

- [x] **AC09** F1 の Tab 移動行が全タブに入力欄があるかのように案内 — 対応: loop26-ux
  — strings.rs:271

### 優先度: 中（ループ R24 フォローアップ・confirmed）

- [x] **AC10** Search で F5 が keymap に到達しても reload() が early-return し実質デッド
  — 対応: 親（reload_reruns_search で run_search へ振り分け）

### 優先度: 中（ループ R25・confirmed）

- [x] **AD01** ゴミ箱読込エラーのオーバーレイで接頭辞が二重付与 — 対応: loop27-core（実バグ修正）
  — main.rs:1974
- [x] **AD02** LOAD_ERROR_HINT_TRASH の「Esc で一覧へ戻る」が実動作と不一致 — 対応: loop27-ux + loop27-core（実バグ修正）
  — strings.rs:150
- [x] **AD03** キーマップ形式説明が3定義に散在（AC08 の片側配線） — 対応: loop27-ux + loop27-core
  — strings.rs:126,670,443
- [x] **AD04** F1/設定一覧のタブ移動キーが PageDown 単体と誤記 — 対応: loop27-ux
  — strings.rs:338

### 優先度: 低（ループ R25）

- [x] **AD05** F1 設定ダイアログ行に未保存破棄警告が2通り重複 — 対応: loop27-ux + loop27-core
  — strings.rs:351
- [x] **AD06** 隠しファイル/並び順行にディレクトリ表示専用の案内なし — 対応: loop27-ux
  — settings_ui.rs:664

### 優先度: 中（ループ R26・confirmed）

- [x] **AE01** Search 入力フォーカス中は F5 が届かず再検索しない（実バグ） — 対応: loop28-core（実バグ修正）
  — main.rs:5066
- [x] **AE02** app/README の LOAD_ERROR_HINT_TRASH が旧文言（Esc 案内） — 対応: loop28-ux
  — app/README.md:373
- [x] **AE03** Rust MSRV 表記がドキュメント間で不一致（1.92 vs 1.99.1） — 対応: loop28-ux
  — README.md:30 / Cargo.toml:6
- [x] **AE04** 色分類表とマーカー表の相互照合が片方向のみ — 対応: loop28-core
  — strings.rs:1806

### 優先度: 低（ループ R26）

- [x] **AE05** EMPTY_TRASH が Backspace 復帰を省く — strings.rs:543 — 対応: loop28-ux
- [x] **AE06** tab_label の file_name 不在分岐が無テスト — main.rs:939 — 対応: loop28-core
- [x] **AE07** ゴミ箱並び替え拒否に復帰導線なし — strings.rs:612 — 対応: loop28-ux
- [x] **AE08** STATUS_SEARCH_STALE が F5 再検索を案内せず — 対応: loop28-ux
  — strings.rs:590

### 優先度: 中（ループ R27・confirmed）

- [x] **AF01** status_color が警告をエラーより優先し結合通知が琥珀色に — 対応: loop29-core（実バグ修正）
  — main.rs:540
- [x] **AF02** LOAD_ERROR_HINT_SEARCH が「再読込」（実動作は再検索） — 対応: loop29-ux
  — strings.rs:166
- [x] **AF03** ラベル編集ダイアログの脚文が DIALOG_FOOTER_HINT 迂回 — 対応: loop29-core
  — main.rs:7992
- [x] **AF04** 設定カード検索が settings.json の選択値にヒットしない — 対応: loop29-ux
  — settings_ui.rs:413
- [x] **AF05** 「再読込/再読み込み」の表記ゆれ未集約 — app/README.md:136 — 対応: loop29-ux

### 優先度: 低（ループ R27）

- [x] **AF06** デバッグ進捗フックが STATUS_CANCEL_HINT 迂回
  — 対応: 親（transfer_progress_status を共用）
- [x] **AF07** ops.rs require_directory の symlink 追跡分岐が無テスト — 対応: loop29-tests
- [x] **AF08** preview.rs の post-read サイズガード分岐が無テスト — 対応: loop29-tests
- [x] **AF09** watcher.rs DirWatcher::new の初期化失敗分岐が無テスト — 対応: loop29-tests
- [x] **AF10** 設定ダイアログの Esc 表記が凡例と F1/README で不一致 — 対応: loop29-ux
  — settings_ui.rs:398

### 優先度: 中（ループ R28・confirmed）

- [x] **AG01** 設定検索で open_settings が零命中（キーワード未登録） — 対応: loop30-ux
  — settings_ui.rs:515
- [x] **AG02** reload_word! が未公開で「再読込」リテラルが重複 — 対応: loop30-core（実バグ修正）
  — strings.rs:105 / settings_ui.rs:751
- [x] **AG03** Alt+K ヘルプ行が DIALOG_FOOTER_HINT を迂回 — 対応: loop30-core
  — strings.rs:735
- [x] **AG04** ターミナル無効化成功通知が未定数化 — main.rs:4582 — 対応: loop30-core
- [x] **AG05** F1 ヘルプが主要マウス操作4種を欠く — strings.rs:592 — 対応: loop30-ux（実バグ修正）
- [x] **AG06** 設定保存成功の通知文言が未定義（破棄通知と非対称） — 対応: loop30-core
  — strings.rs:1033

### 優先度: 低（ループ R28）

- [x] **AG07** OP_RESULT_PRIORITY[2]「失敗のみ」が未検証 — strings.rs:1182 — 対応: loop30-core
- [x] **AG08** 設定ダイアログ内で F1 無効の理由が示されない — 対応: loop30-ux
  — settings_ui.rs:393
- [x] **AG09** README の F4 だけバッククォート表記でない — app/README.md:114 — 対応: loop30-ux
- [x] **AG10** サイドバー空セクションの文言が未定義（speculative） — 対応: loop30-ux
  — strings.rs:791

### 優先度: 低（ループ R29・confirmed）

- [x] **AH01** DESIGN as-built 表が P1 完了後も旧主張のまま — DESIGN.md:358 — 対応: loop31-docs
- [x] **AH02** app/README 差分表が「明記なし」と誤記 — app/README.md:702 — 対応: loop31-docs
- [x] **AH03** 予約キー注記の「escape」が検索キーワードに欠落 — 対応: loop31-ux
  — settings_ui.rs:668
- [x] **AH04** 「ディレクトリ」がファイル設定行のキーワードに欠落 — 対応: loop31-ux
  — settings_ui.rs:696
- [x] **AH05** DESIGN クローム高と実装定数 140px の基準差 — 対応: loop31-docs
  — DESIGN.md:140 / main.rs:206（speculative）

### 優先度: 中（ループ R30・confirmed）

- [x] **AI01** マウス操作の双方向同期テストが F1 の3行を漏らす — 対応: loop32-ux
  — settings_ui.rs:3075
- [x] **AI02** TRASH_EXIT_HINT が F1 ヘルプで共用されていない — 対応: loop32-ux + loop32-core
  — strings.rs:186,655
- [x] **AI03** DESIGN のボリューム行に sysinfo 根拠が残存 — 対応: loop32-docs（親が lint 修正）
  — DESIGN.md:103,863

### 優先度: 低（ループ R30）

- [x] **AI04** マウス操作の説明文が F1 と README で漂移 — strings.rs:640 — 対応: loop32-ux
- [x] **AI05** サイドバー空セクションの文言が未表示 — strings.rs:843 — 対応: loop32-docs
- [x] **AI06** 設定カードに一括リセット非配線の案内なし — 対応: loop32-ux
  — settings_ui.rs:678
- [x] **AI07** ターミナル設定タブが F4 関連を案内せず — 対応: loop32-ux
  — settings_ui.rs:1391
- [x] **AI08** キーマップ unbind 未ヒット通知に復帰導線なし — 対応: loop32-core
  — strings.rs:1060
- [x] **AI09** やり直し空通知に記録対象ヒントなし — strings.rs:1139 — 対応: loop32-core
  — strings.rs:769

### 優先度: 中（ループ R31・confirmed）

- [x] **AJ01** Alt+T ダイアログが TERMINAL_COMMAND_DESCRIPTION を迂回 — 対応: loop33-core
  — main.rs:4640
- [x] **AJ02** F2 リネームの入力検証拒否が未定数化（P08 残件） — 対応: loop33-core
  — ops.rs:193
- [x] **AJ03** ダイアログ拒否が self.status にミラーされず色分類対象外 — 対応: loop33-core（実バグ修正）
  — main.rs:4736,4567
- [x] **AJ04** ADR-011「モジュール分割」に as-built 注記なし — 対応: loop33-docs
  — DESIGN.md:1170
- [x] **AJ05** ui/* 分割を「残課題」と記載するドキュメントが決定と矛盾 — 対応: loop33-docs
  — DESIGN.md:376 / app/README.md:701
- [x] **AJ06** WindowBoundsState::is_valid の上限分岐が無テスト — 対応: loop33-docs
  — settings.rs:611
- [x] **AJ07** 設定ダイアログ内に read-only 表示なし — settings_ui.rs:383
  — 対応: loop33-ux + 親（read_only 配線）

### 優先度: 低（ループ R31）

- [x] **AJ08** fuzzy_score の min(40) キャップ分岐が無テスト
  — 対応: 親（相対スコアで固定）
- [x] **AJ09** 個人化タブが read-only でも保存されると表示 — 対応: loop33-ux + 親（read_only 配線）
  — settings_ui.rs:1218
- [x] **AJ10** F1 に検索履歴/保存検索の導線なし — strings.rs:878 — 対応: loop33-core

## 新要件（Windows Explorer / Double Commander 級 UX + Android / iOS）

> ユーザー要件: 「Windowsのfiles、Windowsのデフォルトファイルエクスプローラー、
> ダブルコマンダー並みの見やすさ、使いやすさにしてね。あとAndroid版、ios版も
> 要件に入れて。」（2026-11-22）

### 優先度: 高（新要件・設計判断が必要）

- [x] **NW01** DESIGN の対象OS/非目的を改訂: Windows を第一級、Android/iOS を要件化 — 対応: docs-target-os
  — DESIGN.md:26,37-54 / R1-DOC01
- [x] **NW02** デュアルペイン設計 ADR（draft・承認待ち） — 対応: adr-dualpane
  — docs/adr/adr-20261122-dual-pane-ui.md
- [x] **NW03** Android/iOS の実現可能性 ADR — 対応: adr-mobile（draft・ユーザー承認待ち）
  — docs/adr/adr-20261122-mobile-android-ios-support.md
- [x] **NW04** レスポンシブレイアウト — 対応: impl-responsive
  — サイドバー幅/ダイアログ幅/グリッド列を viewport 適応化

### 優先度: 中（新要件・実装）

- [x] **NW05** Windows ドライブ列挙とルート表現 OS 別化 — 対応: impl-windrives
  — 実 FFI は unsafe 禁止のため documented stub（要承認）
  — mounts.rs:155 / main.rs:2122 / R1-ARCH01
- [x] **NW06** OS アイコン抽象化（FFI 非依存部分） — 対応: impl-osicons
  — Win FFI 実装は NF02 承認後 / provider は未配線
  — system_icons.rs:160 / R1-ARCH01
- [x] **NW07** 列のカスタマイズ — 対応: impl-columns
  — ヘッダーは対応、行との整列はフォローアップ
  — main.rs:9246 / R1-UX01
- [x] **NW08** コンテキストメニューをキーボードから開く — 対応: impl-ctxkey
  — Shift+F10 / Menu をバインド（E2E 実機確認は未実施）
- [x] **NW09** 一覧の画像サムネイル — 対応: impl-thumbs
  — gpui-kit の async 読込＋キャッシュを再利用（独自 LRU はフォローアップ）
  — preview.rs:1 / R1-PRD01
- [x] **NW10** OS クリップボード調査と ADR — 対応: research-clipboard
  — Win/mac は読み取り実装済み・未接続、Linux は未対応
  — main.rs:2300 / R1-PRD01
- [x] **NW11** タッチ入力層（tap 選択 / long-press 詳細 / swipe 戻る）— 対応: nw11-pwa + nw11-android
  — PWA は Playwright で実動作確認（選択・長押し・スワイプ・キーボード）
  — Android は Compose combinedClickable + Snackbar（SDK/JDK 不在でビルド検証不可）
  — platform 修飾子（Cmd）は macOS 実機検証なしの方針により対象外
- [x] **NW12** ICU4X 五十音順調査と ADR（draft） — 対応: research-icu
  — icu_collator 2.4.0 未取得、実装は Phase 0 以降
  — strings.rs:25 / R1-PRD01

### 優先度: 高（モバイル ADR Phase 1 実装計画・設計完了）

> 設計: [phase1-core-extraction.md](docs/design/phase1-core-extraction.md) /
> [phase1-platform-ports.md](docs/design/phase1-platform-ports.md)
> モバイル ADR は approved（2026-11-23、選択肢B）。
> iOS は PWA のみ（Apple Developer Program の年会費が必ずかかるため）、
> Android は Compose アプリ＋PWA、Windows は評価版で検証、macOS は実機検証なし。

- [x] **P1-00** workspace 準備: ルート Cargo.toml に [workspace] と members を追加
  — 対応: p1-core-move + 親
  — app/poc の profile を集約。挙動不変
- [x] **P1-01** core 骨格: `core/`（gpui-file-explorer-core / gfe_core）を作成
  — 対応: p1-core-move + 親
  — `#![forbid(unsafe_code)]`、error/strings/ports を配置
- [x] **P1-02** 純粋ロジック移動: model/search/ops/trash_view 純関数群を core へ
  — 対応: p1-core-move + 親
  — model→settings 逆参照は core 内のシグナル port で解消（案3の代替）
- [x] **P1-03** app 側参照を gfe_core へ一本化 + GPUI アダプタ土台
  — 対応: 親（main.rs import 直接化、ports_gpui.rs 追加）
  — デスクトップ全テスト green を維持
- [x] **P1-04** 1画面ビューア試作: PWA は core API 経由で達成（Real Proof）
  — Android scaffold は作成、SDK/JDK 不在でビルド検証不可
  — コア API 経由で一覧・検索を表示（外観完成は要求しない）
- [x] **P1-05** 工数・ライセンス再見積もりの実績反映
  — wasm ビルド 1.3s / PWA 4件一覧・1件検索確認。ライセンス境界不変

### 優先度: 中（新要件フォローアップ）

- [x] **NF01** 列カスタマイズ時にヘッダと行の列を整列させる（行描画を列ベースへ） — 対応: fix-columns
  — NW07 の残作業 / impl-columns 報告
- [x] **NF02** Windows 実ドライブ列挙を gfe-win32 へ隔離実装
  — 対応: nf02-win32（2026-11-23 ユーザー承認）
  — unsafe は win32 crate のみ、app/core は forbid 維持。Windows target check/clippy pass
  — NW05 の documented stub を実装へ / AGENTS.md §15
- [x] **NF03** サムネイル設定の Switch を設定UIへ配線 + docs 更新
  — 対応: fix-thumbsettings + 親（main.rs 配線）
  — NW09 のフォローアップ / README.md:148 / DESIGN.md:101
- [x] **NF04** icons.rs::kind_for_name に PNM 拡張子（pbm/ppm/pgm/pnm）を追加 — 対応: fix-icons-pnm
  — NW09 で発見したギャップ

### 優先度: 中（ループ R37・confirmed）

- [x] **AK01** 狭い幅で折りたたんだサイドバーを再表示する手段がない — 対応: impl-r37-core
  — main.rs:10275
- [x] **AK02** サムネイル/列 Switch にキーボード操作経路がない（t/v/h のみ） — 対応: impl-r37-ux
  — settings_ui.rs:1040
- [x] **AK03** モバイル ADR「作成中」表記が実態と乖離（作成済み・draft） — 対応: impl-r37-ux
  — README.md:165 / DESIGN.md:58
- [x] **AK04** デュアルペイン ADR の main.rs 行アンカーが drift — 対応: impl-r37-ux
  — docs/adr/adr-20261122-dual-pane-ui.md:33
- [x] **AK05** サムネイル表示条件が一覧/グリッドで重複し無テスト — 対応: impl-r37-core
  — main.rs:9847,10136
- [x] **AK06** icon_path_for_entry の分類分岐が無テスト — system_icons.rs:233 — 対応: impl-r37-tests
- [x] **AK07** アイコンパス探索優先順位のフィクスチャテストなし — 対応: impl-r37-tests
  — system_icons.rs:259

### 優先度: 低（ループ R37）

- [x] **AK08** 操作履歴ダイアログのキーヒントが未集約 — main.rs:6520 — 対応: impl-r37-core + impl-r37-ux
- [x] **AK09** パス入力バーのキーヒントが未集約 — main.rs:7399 — 対応: impl-r37-core + impl-r37-ux
- [x] **AK10** F1 ヘルプのフッターヒントが未集約 — main.rs:7766 — 対応: impl-r37-core + impl-r37-ux
- [x] **AK11** プロパティダイアログのキーヒントが未集約 — main.rs:8070 — 対応: impl-r37-core + impl-r37-ux
- [x] **AK12** 検索バーの待機/検索中ヒントが未集約 — main.rs:9009 — 対応: impl-r37-core + impl-r37-ux
- [x] **AK13** クリップボード ADR の todo アンカーが 1〜3 行ずれ — 対応: impl-r37-ux
  — docs/adr/adr-20261123-os-file-clipboard.md:22
- [x] **AK14** README 構成表に docs/adr/ が未掲載 — README.md:88 — 対応: impl-r37-ux
- [x] **AK15** column_label/column_ids の全バリアントが無テスト — 対応: impl-r37-ux
  — settings_ui.rs:1150
- [x] **AK16** 列 visibility/reorder の no-op 分岐が無テスト — 対応: impl-r37-ux
  — settings_ui.rs:1197

### 優先度: 中（ループ R38・confirmed）

- [x] **AL01** 検索中ステータスが SEARCH_IN_PROGRESS_LABEL を迂回 — 対応: impl-r38-core
  — main.rs:8849
- [x] **AL02** OS クリップボード「GPUI側未実装」表記が ADR と矛盾 — 対応: impl-r38-docs
  — README.md:151 / app/README.md:415
- [x] **AL03** DESIGN 変更履歴/参照に draft ADR 3件が未反映 — DESIGN.md:1252 — 対応: impl-r38-docs
- [x] **AL04** 設定検索から Shift+F10 / Menu に到達できない — 対応: impl-r38-core
  — settings_ui.rs:680
- [x] **AL05** エラー→対処の対応表 — 対応: impl-r39-errorhelp
  — README に4行の対応表、F1 に正規文言を追加
  — app/README.md:532

### 優先度: 低（ループ R38）

- [x] **AL06** DESIGN の dirs バージョン差「要確認」が解消済み — DESIGN.md:1245 — 対応: impl-r38-docs

### 優先度: 中（ループ R40・confirmed）

- [x] **AM01** F2 リネーム拒否が self.status にミラーされず（AJ03 残件） — 対応: impl-r40-core（実バグ修正）
  — main.rs:8309
- [x] **AM02** プレビュー/クイックジャンプのフッターヒント未集約 — 対応: impl-r40-core
  — main.rs:7336,7707
- [x] **AM03** エラー対応表の「次の操作」列に回帰ガードなし — 対応: impl-r40-core
  — strings.rs:1853
- [x] **AM04** trash_view::restore_by_id のテストが空振り — trash_view.rs:374 — 対応: impl-r40-tests
- [x] **AM05** ヘッダクリック/Alt+S のキー案内に「種類」列・順序が欠落 — 対応: impl-r40-docs + 親（strings.rs）
  — strings.rs:1108,1174

### 優先度: 低（ループ R40）

- [x] **AM06** 五十音順/クリップボード ADR の行アンカーが drift — 対応: impl-r40-docs
  — docs/adr/adr-20261123-*.md
- [x] **AM07** DESIGN §3.1 の PageUp/Down 行アンカーが 5134 のまま — 対応: impl-r40-docs
  — DESIGN.md:189
- [x] **AM08** AppError::source() の3バリアントが未検証 — error.rs:245 — 対応: impl-r40-tests
- [x] **AM09** sniff_image_format の非 WEBP RIFF 分岐が無テスト — 対応: impl-r40-tests
  — preview.rs:100

### 優先度: 中（ループ R41・confirmed）

- [x] **AN01** エラー対応表のターミナル文言が実表示と不一致 — 対応: impl-r41-docs
  — app/README.md:544
- [x] **AN02** DESIGN のサイドバー幅「固定」表記がレスポンシブ実装と矛盾 — 対応: impl-r41-docs
  — DESIGN.md:197
- [x] **AN03** 列カスタマイズが README/F1 に未記載 — app/README.md:118
  — 対応: impl-r41-docs + 親（例外登録）

### 優先度: 低（ループ R41）

- [x] **AN04** 五十音順 ADR の DESIGN 行アンカーが 1 行ずれ — 対応: impl-r41-docs
  — docs/adr/adr-20261123-japanese-collation.md:56
- [x] **AN05** モバイル ADR の wasm ノートへのリンク切れ — 対応: impl-r41-docs
  — docs/adr/adr-20261122-mobile-android-ios-support.md:256
- [x] **AN06** context_menu_anchor のガード失敗分岐が無テスト — main.rs:461 — 対応: impl-r41-tests
- [x] **AN07** status_color 回帰表に7定数が欠落 — main.rs:12348 — 対応: impl-r41-tests
- [x] **AN08** オーバーレイヒント定数の配線ガードなし — strings.rs:235 — 対応: impl-r41-tests
- [x] **AN09** 列幅の settings.json キー文書化 — 対応: 親
  — column_prefs.widths のキーと手編集手順を追記

### 優先度: 中（ループ R42・confirmed）

- [x] **AO01** AN08 の配線ガードがオーバーレイ3種のみ（残り5ヒント無防備） — 対応: impl-r42-core
  — strings.rs:333

### 優先度: 低（ループ R42）

- [x] **AO02** SettingsUiPalette の green/red 等が回帰検出外 — 対応: impl-r42-core
  — settings_ui.rs:2065
- [x] **AO03** DESIGN 変更履歴の「190px 固定」が現行仕様と矛盾 — 対応: 親
  — DESIGN.md:1297
- [x] **AO04** column_width の下限棄却分岐が無テスト — main.rs:3237 — 対応: impl-r42-tests
- [x] **AO05** AppError::from_io の NotADirectory/Io 分岐が無テスト — 対応: impl-r42-tests
  — error.rs:132

### 優先度: 低（ループ R43・confirmed）

- [x] **AP01** 6検証拒否定数が色分類表/マーカー配列から漏れている — 対応: impl-r43-core
  — strings.rs:1650,1653,1658,1760,1781,1784
- [x] **AP02** クリップボード ADR の #L アンカーが +4 行再ドリフト — 対応: impl-r43-core
  — docs/adr/adr-20261123-os-file-clipboard.md:44
- [x] **AP03** 列幅の手編集手順が F1/設定行に案内されず — 対応: impl-r43-ux
  — settings_ui.rs:837
- [x] **AP04** 設定検索「ヘッダ」がキーワードに欠落 — settings_ui.rs:594 — 対応: impl-r43-ux
- [x] **AP05** 設定検索「一括リセット」「予約キー」が欠落 — 対応: impl-r43-ux
  — settings_ui.rs:633

### 優先度: 中（ループ R44・confirmed）

- [x] **AQ01** DESIGN の create_unique_folder 参照が 805（実 803） — 対応: impl-r44-docs
  — DESIGN.md:315,866
- [x] **AQ02** app/README「ソートはヘッダクリックのみ」が Alt+S と矛盾 — 対応: impl-r44-docs
  — app/README.md:660

### 優先度: 低（ループ R44）

- [x] **AQ03** 設定検索 `reset` がゼロヒット — settings_ui.rs:641 — 対応: impl-r44-ux
- [x] **AQ04** `settings.json` がファイル列タブから検索できない — 対応: impl-r44-ux
  — settings_ui.rs:598
- [x] **AQ05** column_width_label の不正保存値分岐が無テスト — 対応: impl-r44-ux（実バグ修正）
  — settings_ui.rs:1300
- [x] **AQ06** ADR/DESIGN の行アンカー方形針自己矛盾（4箇所） — 対応: impl-r44-docs
  — docs/adr/*.md / DESIGN.md:190

### 優先度: 中（ループ R45・confirmed）

- [x] **AR01** 設定検索欄にクリア手段が無い（零命中案内と矛盾） — 対応: impl-r45-ux + 親（strings.rs）
  — settings_ui.rs:243
- [x] **AR02** read-only 時に「Enter 保存」案内と無効 Save が残る — 対応: impl-r45-ux
  — settings_ui.rs:461,1363

### 優先度: 低（ループ R45）

- [x] **AR03** strings.rs モジュール doc の DESIGN 行アンカーが陳腐化 — 対応: impl-r45-tests
  — strings.rs:26
- [x] **AR04** 設定検索 `column_prefs.widths` / `widths` が零命中 — 対応: impl-r45-ux
  — settings_ui.rs:600
- [x] **AR05** default_column_width の未知キー分岐が無テスト — settings.rs:744 — 対応: impl-r45-tests
- [x] **AR06** image_format_for_name の "tif" 別名が無テスト — preview.rs:50 — 対応: impl-r45-tests
- [x] **AR07** Shortcuts タブの Esc 行に破棄警告が無い — settings_ui.rs:954
  — 対応: impl-r45-tests + impl-r45-ux

### 優先度: 中（ループ R46・confirmed）

- [x] **AS01** 設定検索「破棄」「未保存」が零命中 — settings_ui.rs:666 — 対応: impl-r46

### 優先度: 低（ループ R46）

- [x] **AS02** 設定検索「アクション」が零命中 — settings_ui.rs:666 — 対応: impl-r46
- [x] **AS03** STATUS_UNDO_EXISTS_SUFFIX がマーカー表に未登録 — 対応: impl-r46
  — main.rs:614

### 優先度: 中（ループ R47・confirmed）

- [x] **AT01** read-only で Alt+K/Alt+T ダイアログが「Enter で保存」案内 — 対応: impl-r47-core
  — main.rs:5157,5351
- [x] **AT02** DESIGN §7 clipboard「Wayland ✅」が ADR 実読結果と矛盾 — 対応: impl-r47-docs
  — DESIGN.md:890

### 優先度: 低（ループ R47）

- [x] **AT03** undo EXISTS 側が純粋ヘルパ化されていない — main.rs:6721 — 対応: impl-r47-core
- [x] **AT04** 設定検索「グリッド」が零命中 — settings_ui.rs:646 — 対応: impl-r47-core
- [x] **AT05** DESIGN §8.1 dirs「未確認」が §11「確認済み」と矛盾 — 対応: impl-r47-docs
  — DESIGN.md:933
- [x] **AT06** Tabs::close_active の非末尾分岐が無テスト — tabs.rs:103 — 対応: impl-r47-docs
- [x] **AT07** F1 行ショートカット要約が b を省略 — strings.rs:1337 — 対応: impl-r47-core
- [x] **AT08** Shift+Home/End の範囲選択がヘルプに未案内 — 対応: impl-r47-core
  — strings.rs:1204

### 優先度: 低（ループ R48・confirmed）

- [x] **AU01** 非空在庫に4つの STATUS_* 定数が未登録 — strings.rs:2413 — 対応: impl-r48（誤検知・登録済みを確認、変更不要）
- [x] **AU02** DESIGN D&D 行に P1 実装済みの as-built 注記なし — 対応: impl-r48
  — DESIGN.md:128,897
- [x] **AU03** review-report chrono 参照が 1 行ずれ — review-report.md:326 — 対応: impl-r48
- [x] **AU04** DESIGN PoC 行アンカーが 6 行ずれ — DESIGN.md:1086 — 対応: impl-r48

### 優先度: 中（ループ R49・confirmed）

- [x] **AV01** ラベル編集ダイアログが read-only でも「Enter で保存」 — 対応: impl-r49-core（実バグ修正）
  — main.rs:8531
- [x] **AV02** app/README の設定スキーマ「一致」主張が as-built と乖離 — 対応: impl-r49-docs
  — app/README.md:749
- [x] **AV03** 設定カード全体がスクロールし操作ボタンが画面外へ — 対応: impl-r49-core（実バグ修正）
  — settings_ui.rs:2068

### 優先度: 低（ループ R49）

- [x] **AV04** NW04 レスポンシブが README/app/README に未記載 — 対応: impl-r49-docs
- [x] **AV05** AppError::with_cause の Cancelled no-op 分岐が無テスト — 対応: impl-r49-docs
  — error.rs:177
- [x] **AV06** app/README が新要件（Windows 第一級）を未反映 — 対応: impl-r49-docs
  — app/README.md:5

### 優先度: 中（ループ R50・confirmed）

- [x] **AW01** キーマップ入力ヒントの「形式/コード/解除」が検索零命中 — 対応: impl-r50
  — settings_ui.rs:668

### 優先度: 低（ループ R50）

- [x] **AW02** read-only 表示語「読み取り専用」が検索キーワードに欠落 — 対応: impl-r50
  — settings_ui.rs:1038
- [x] **AW03** 検索履歴「最大10件」とチップ8件描画が矛盾 — 対応: impl-r50
  — main.rs:9256 / app/README.md:370

### 優先度: 低（調査タスク）

- [x] **F6-39〜F6-48** 未確認項目を調査し、事実／未確認／必要環境を整理 — 成果物:
  [unconfirmed-followup.md](notes/task-discovery-2/unconfirmed-followup.md)。性能実測は
  [perf-baseline.md](notes/perf-baseline.md) にあり（200 MB 目標は未達）。
  実機再現・フレーム時間計測は未実施で、必要手順を同ファイルに記録。
  AGPL §13 導線と DEP-5 copyright は対応済み。Debian 登録は現時点では
  行わない（ユーザー判断、将来配布方針決定時に再評価）。

## 完了

- [x] `todo.md` をプロジェクト内へ配置（2026-11-17）
- [x] 第2回クロスレビューの全指摘（Critical 1 / High 8）を解消（2026-11-17）
- [x] `tracing` による構造化ログを導入（2026-11-17）
- [x] Windows テスト期待値と Packaging の安全ガードを修正（2026-11-17）
- [x] `app/` / `poc/` のビルドキャッシュを clean（約 36 GiB 回収、2026-11-17）
- [x] 第2次クロスレビュー 5 パス実施（25 レンズ・生 367 件、2026-11-18）
- [x] 第6パスで重複・妥当性を吸収し最終 54 項目へ統合（2026-11-18）
- [x] 最終項目を本 `todo.md` へ反映（2026-11-18）

## 進行中（ループ5 バックログ）

### 優先度: 高（confirmed）

- [x] **AQ01** CI/check.sh が core / win32 の単体テストを実行していない — 対応: loop5
- [x] **AQ02** 設定検索 zero-hit: クイックジャンプ / 親フォルダ / サイドバー — 対応: loop5
- [x] **AQ03** wasm searchInJson が core の検索規約と不一致（空クエリ） — 対応: loop5
- [x] **AQ04** PWA CoreViewer 生成が wasm-bindgen API と不一致 — 対応: loop5
- [x] **AQ05** ADR 件数が4件のまま（実5件・モバイルのみ approved） — 対応: loop5

### 優先度: 中（confirmed）

- [x] **AQ06** status 文言のインライン残り（名前変更成功 / 別フォルダ / 未保存） — 対応: loop5
- [x] **AQ07** 「最後の1列は非表示にできません」が設定行に案内されない — 対応: loop5
- [x] **AQ08** モバイル ADR 見出しが「未確定」のまま（approved と矛盾） — 対応: loop5
- [x] **AQ09** app/Cargo.toml 行番号参照が gfe-core 抽出後に陳腐化 — 対応: loop5
- [x] **AQ10** DESIGN §11 未確認事項が as-built で更新されていない — 対応: loop5
- [x] **AQ11** wasm DTO 変換の境界値が無テスト — 対応: loop5
- [x] **AQ12** PWA タッチ防御分岐（多指 / 左端 / 斜め）が無テスト — 対応: loop5
- [x] **AQ13** poc licenses 監査除外理由が失効（license フィールド存在） — 対応: loop5
- [x] **AQ14** workspace 導入後に「ルート Cargo.toml が無い」記述が残存 — 対応: loop5

### 優先度: 低（confirmed）

- [x] **AQ15** STATUS_RESCAN_FAILED が reload_word! を通していない — 対応: loop5
- [x] **AQ16** docs/adr/index.md が無く ADR 索引規約を満たさない — 対応: loop5
- [x] **AQ17** gfe-win32 の非 Windows Drop / 空文字 NUL が無テスト — 対応: loop5
- [x] **AQ18** workspace 化後にメンバ配下の旧 Cargo.lock が残存 — 対応: loop5

## 進行中（ループ6 バックログ）

### 優先度: 高（ループ6・confirmed）

- [x] **AR01** Search フィルタ変更が RESCAN_FAILED を上書き（実バグ） — 対応: loop6
- [x] **AR02** 公開シグナル port が thread_local（跨线程で信号喪失） — 対応: loop6
- [x] **AR03** PWA ダークテーマ error 文字が WCAG AA 未満 — 対応: loop6
- [x] **AR04** 設定UI muted 色が両テーマで 4.5:1 未満 — 対応: loop6
- [x] **AR05** app/README アーキテクチャ表が core 抽出後も旧構成 — 対応: loop6
- [x] **AR06** Windows ドライブ列挙が未実装とする as-built 記述が残存 — 対応: loop6

### 優先度: 中（ループ6・confirmed）

- [x] **AR07** AppError::from_io が raw_os_error を失う — 対応: loop6
- [x] **AR08** AppError::from_io が IsADirectory / EROFS / ENOSPC を誤分類 — 対応: loop6
- [x] **AR09** wasm 境界エラーが非構造文字列で種別判別不可 — 対応: loop6
- [x] **AR10** ScopedIcon のハンドルアクセス手段が無い — 対応: loop6
- [x] **AR11** undocumented_unsafe_blocks ゲートが宣言のみ — 対応: loop6
- [x] **AR12** Windows ADR が gfe-win32 実装後も未更新 — 対応: loop6
- [x] **AR13** DESIGN §4.1 が workspace 構成を反映していない — 対応: loop6
- [x] **AR14** Debian 同梱 NOTICE-app.md が旧スナップショット — 対応: loop6
- [x] **AR15** core::error 文言が status 色分類表に未登録 — 対応: loop6
- [x] **AR16** PWA listbox にアクセシブルネームが無い — 対応: loop6
- [x] **AR17** PWA キーボードでパス詳細を取得できない — 対応: loop6
- [x] **AR18** cargo test を「全テスト」とする記述が workspace 化後も残存 — 対応: loop6

### 優先度: 低（ループ6・confirmed）

- [x] **AR19** ProgressSink が未配線（ops と二重契約） — 対応: loop6
- [x] **AR20** PortError::source() に直接テストが無い — 対応: loop6
- [x] **AR21** NOTICE.md の app/NOTICE.md リンクが deb 内で切れる — 対応: loop6
- [x] **AR22** PWA リストボックスに Home/End が無い — 対応: loop6

### 既知リスク（ループ6で記録、ループ7で解消）

- 旧: persisted enum シグナルの AtomicBool + thread_local 併用によるレース。
  AS01 で生 JSON クロスチェックの純関数へ収束し完全解消（32 スレッド × 4 ラウンド回帰 green）。

## 進行中（ループ7 バックログ）

### 優先度: 高（ループ7・confirmed）

- [x] **AS01** persisted enum シグナルを案2（LoadReport）へ収束（根本解消） — 対応: loop7
- [x] **AS02** reconcile の古いスナップショットが watch 増分を上書き — 対応: loop7
- [x] **AS03** reconcile エラー経路に view ガードが無い — 対応: loop7
- [x] **AS04** run_search 成功が RESCAN_FAILED ホールドを解除しない — 対応: loop7
- [x] **AS05** DEB ビルドが workspace 化後の target パス変更に未追従 — 対応: loop7
- [x] **AS06** win32 の SAFETY 注記ゲートが Linux で空振り（cross-target 必要） — 対応: loop7
- [x] **AS07** 設定「一覧の列」エディタがキーボード到達不能＋「↑↓」表記が誤り — 対応: loop7

### 優先度: 中（ループ7・confirmed）

- [x] **AS08** wasm32 バインディングがどのゲートでもコンパイルされていない — 対応: loop7
- [x] **AS09** poc licenses 監査が陳腐化した前提でスキップされている — 対応: loop7
- [x] **AS10** docs/design の Windows スタブ / アイコン provider 記述が古い — 対応: loop7
- [x] **AS11** README の CI 検証範囲が core/win32 ゲート追加を未反映 — 対応: loop7
- [x] **AS12** dual-pane ADR のキーボード代替不在記述が古い — 対応: loop7
- [x] **AS13** phase1-core-extraction の workspace「案」が実装済みを未反映 — 対応: loop7
- [x] **AS14** 設定検索プレースホルダが chord 検索を案内しない — 対応: loop7
- [x] **AS15** 零命中ヒントにキーボード代替が無い — 対応: loop7
- [x] **AS16** PWA listbox の aria-live が重複通知を誘発 — 対応: loop7

### 優先度: 低（ループ7・confirmed）

- [x] **AS17** needs_reload_after_load が発生元 generation と紐付かない — 対応: loop7
- [x] **AS18** 3つの Cargo.lock と古い workspace 記述が並存 — 対応: loop7
- [x] **AS19** PWA 空状態 li が option セマンティクスを持たない — 対応: loop7
- [x] **AS20** DeserializeSeed 代替案の却下理由を記録（案2採用時） — 対応: loop7

## 進行中（ループ8 バックログ）

### 優先度: 高（ループ8・confirmed）

- [x] **AU01** 設定ロック回収の TOCTOU と無条件解放で排他が無効化 — 対応: loop8
- [x] **AU02** ロック mtime 未更新で存命中の保存ロックが60秒で奪われる — 対応: loop8
- [x] **AU03** 読み込み前 metadata サイズ検査が競合し正常ファイルを退避 — 対応: loop8
- [x] **AU04** 一時ファイルの 0600 設定失敗が無視され world-readable — 対応: loop8
- [x] **AU05** Search で Created/Modified 監視イベントが無通知で破棄 — 対応: loop8
- [x] **AU06** --skip-build 鮮度ガードが core/ の変更を検出しない — 対応: loop8
- [x] **AU07** ビルド失敗時に前回の .deb が残り誤配布される — 対応: loop8

### 優先度: 中（ループ8・confirmed）

- [x] **AU08** reconcile_in_flight が generation / cwd に紐付かない — 対応: loop8
- [x] **AU09** suppress_until / op_cancel 中の監視バッチが失われる — 対応: loop8
- [x] **AU10** DESIGN §9.1 サプライチェーン as-built が CI と食い違う — 対応: loop8
- [x] **AU11** DESIGN 監視 pump の120ms tick 記述が実装と不一致 — 対応: loop8
- [x] **AU12** detect_unknown_persisted_enum に直接テストが無い — 対応: loop8
- [x] **AU13** windows[].bounds.mode 全走査が未テスト — 対応: loop8
- [x] **AU14** pipefail 下の head -n がビルドを非ゼロ終了させうる — 対応: loop8
- [x] **AU15** doc ゲートがコミット済み成果物の陳腐化を検出できない — 対応: loop8

### 優先度: 低（ループ8・confirmed）

- [x] **AU16** 保存不能セッションで設定ダイアログが writable 表示 — 対応: loop8
- [x] **AU17** normalize_events が3パス以上の Rename を捨てる — 対応: loop8
- [x] **AU18** unwatch 失敗後に古いパスが監視集合から消える — 対応: loop8
- [x] **AU19** phase1-platform-ports が core 抽出前のパスを引用 — 対応: loop8
- [x] **AU20** u64 ラップ境界・語彙一致・ソース-pin テスト不足 — 対応: loop8
- [x] **AU21** app/README DEB 節が --skip-build を説明していない — 対応: loop8

## 進行中（ループ9 バックログ）

### 優先度: 高（ループ9・confirmed）

- [x] **AV01** 破損設定が無通知で退避され起動直後に初期値上書き — 対応: loop9
- [x] **AV02** 読み込み時の truncate が無警告でウィンドウ/履歴を失う — 対応: loop9
- [x] **AV03** 単一フィールドの型不一致で全設定が初期化される — 対応: loop9
- [x] **AV04** errno 保持 API が Fs/Op/Port で非対称 — 対応: loop9
- [x] **AV05** human_age 境界テストが実時刻に依存（フレーク） — 対応: loop9
- [x] **AV06** watcher 統合テストが is_supported() を必須化 — 対応: loop9

### 優先度: 中（ループ9・confirmed）

- [x] **AV07** 未知 enum 1つでセッション全体が書込不能 — 対応: loop9
- [x] **AV08** core 由来の失敗フラグメント4種が色分類に未登録 — 対応: loop9
- [x] **AV09** BoxedCause::clone が errno を失う — 対応: loop9
- [x] **AV10** OS エラー理由が Fs は表示 / Op は cause に閉じ込め — 対応: loop9
- [x] **AV11** ErrorKind::Other の分類が Fs=Unknown / Op=Io — 対応: loop9
- [x] **AV12** DESIGN §9.6 の cargo vet スコープ誤記 — 対応: loop9
- [x] **AV13** README の --poc 説明が実動作と食い違う — 対応: loop9
- [x] **AV14** コミット済み DEB ステージの .desktop/README が古い — 対応: loop9
- [x] **AV15** LockHeartbeat テストが固定 sleep に依存 — 対応: loop9
- [x] **AV16** PWA クリック抑止が performance.now 直結で決定性不可 — 対応: loop9
- [x] **AV17** EXDEV テストが /dev/shm 構成で黙ってスキップ — 対応: loop9
- [x] **AV18** DEB にアンインストール手順と残存データの明示なし — 対応: loop9
- [x] **AV19** build-deb.sh に --dry-run が無い — 対応: loop9
- [x] **AV20** --verify が control/.deb メタデータを照合しない — 対応: loop9

### 優先度: 低（ループ9・confirmed）

- [x] **AV21** 未知フィールドが read_only 判定対象外 — 対応: loop9
- [x] **AV22** phase1-platform-ports の main.rs 行番号が drift — 対応: loop9
- [x] **AV23** --verify が出力ディレクトリを作成する — 対応: loop9
- [x] **AV24** .desktop に TryExec が無い — 対応: loop9
- [x] **AV25** 未知フィールド保全テストが保存時を検証していない — 対応: loop9

## 進行中（ループ10 バックログ）

### 優先度: 高（ループ10・confirmed）

- [x] **AW01** Alt+K 上限到達で既存 override が削除されたまま残る — 対応: loop10
- [x] **AW02** rollback_settings が last_save_error を上書きし理由が退化 — 対応: loop10
- [x] **AW03** ダイアログ脚文が ephemeral / load_error を無視 — 対応: loop10
- [x] **AW04** undo の Close が全ペイン状態を復元しない — 対応: loop10（dual-pane ADR の将来事項として分離）
- [x] **AW05** undo が undo スタックを更新しない（再 undo で旧状態へ） — 対応: loop10（現行コードで充足・ガード追加のみ）
- [x] **AW06** save 完了を待たずに undo が Close を再適用し得る — 対応: loop10（現行コードで充足・ガード追加のみ）
- [x] **AW07** PWA searchInJson に 8 MiB ガードが無い — 対応: loop10

### 優先度: 中（ループ10・confirmed）

- [x] **AW08** 検索履歴チップの era 切替が undo 履歴を汚す — 対応: loop10（dual-pane ADR の将来事項として分離）
- [x] **AW09** op_cancel が Keymap / Settings ダイアログを閉じない — 対応: loop10
- [x] **AW10** Esc がダイアログを閉じた後に command palette を残す — 対応: loop10（現行コードで充足・ガード追加のみ）
- [x] **AW11** settings.json 退避先のパス/件数が UI で確認できない — 対応: loop10
- [x] **AW12** 設定カードの shortcuts タブが横スクロール不可 — 対応: loop10
- [x] **AW13** help_dialog_footer が strings.rs に未集約 — 対応: loop10（現行コードで充足・ガード追加のみ）
- [x] **AW14** error_log が strings.rs に未集約 — 対応: loop10（現行コードで充足・ガード追加のみ）
- [x] **AW15** group_by_directory が strings.rs に未集約 — 対応: loop10（現行コードで充足・ガード追加のみ）
- [x] **AW16** salvage_settings の型別回帰が一部のみ — 対応: loop10
- [x] **AW17** isClickSuppressed の境界が until=0/MAX のみ — 対応: loop10
- [x] **AW18** raw_os_error が BoxedCause/PortError で未テスト — 対応: loop10
- [x] **AW19** human_age_at の 365d/LOCALE 境界テスト追加 — 対応: loop10
- [x] **AW20** double_click が一覧キーボードヘルプに無い — 対応: loop10

### 優先度: 低（ループ10・confirmed）

- [x] **AW21** 設定検索キーワード grep 0件の見出し3つ — 対応: loop10
- [x] **AW22** .deb が SOURCE_DATE_EPOCH 未設定で非再現 — 対応: loop10
- [x] **AW23** --verify が control の Package/Version 値を検証しない — 対応: loop10
- [x] **AW24** PWA FSA ボタンが wasm 非ロード時も有効 — 対応: loop10
- [x] **AW25** undo ダイアログに「元に戻す」ボタンが無い — 対応: loop10

## 進行中（ループ11 バックログ）

### 優先度: 高（ループ11・confirmed）

- [x] **AX01** redo Trash の空 undo がスタックを永久に詰まらせる（実バグ） — 対応: loop11（実バグ修正）
- [x] **AX02** PWA スワイプ戻りが moved ゲートで実質発火しない（実バグ） — 対応: loop11（実バグ修正）
- [x] **AX03** 8 MiB 上限が app/core で独立定義・一致性未検証 — 対応: loop11
- [x] **AX04** source_region() の終端欠落暗黙許容でガードが沈黙 — 対応: loop11
- [x] **AX05** redo Trash の全失敗が OS エラー理由を落とす — 対応: loop11

### 優先度: 中（ループ11・confirmed）

- [x] **AX06** redo Rename に事前チェックが無い — 対応: loop11
- [x] **AX07** 履歴オーバーレイが redo 項目を表示するが実行不能 — 対応: loop11
- [x] **AX08** テーマ System 切替だけが apply_theme_mode を通らない — 対応: loop11
- [x] **AX09** 設定 Save 書き込み失敗でフォームが閉じ再試行不可 — 対応: loop11
- [x] **AX10** era 切替ロジックが3箇所に重複 — 対応: loop11
- [x] **AX11** app/README の P1/テスト節が core 抽出前の帰属 — 対応: loop11
- [x] **AX12** source-guard が戻る値を検証していない — 対応: loop11
- [x] **AX13** PWA FSA ボタン DOM 反映経路が未テスト — 対応: loop11
- [x] **AX14** oversized kind が PWA で消費されない — 対応: loop11
- [x] **AX15** --verify がステージ/.deb のバイナリを照合しない — 対応: loop11
- [x] **AX16** キーボード選択が画面外へ移動し追従しない — 対応: loop11
- [x] **AX17** 修飾キー併用 Space/i/Enter を横取り — 対応: loop11

### 優先度: 低（ループ11・confirmed）

- [x] **AX18** 履歴オーバーレイ Esc が op をキャンセルしない — 対応: loop11
- [x] **AX19** theme 変更だけでも Search を再実行し2次 save が黙る — 対応: loop11
- [x] **AX20** phase1-core-extraction の as-built に未採用セクション — 対応: loop11
- [x] **AX21** 絵文字アイコンが option のアクセシブル名に混入 — 対応: loop11
- [x] **AX22** 長押し行に user-select 抑止が無い — 対応: loop11

## 進行中（ループ12 バックログ）

### 優先度: 中（ループ12・confirmed）

- [x] **AY01** EXDEV 移動で元削除失敗時に dest が残り再試行で重複累積 — 対応: loop12
- [x] **AY02** Trash Undo の部分復元が RestoreCollision で状態不一致 — 対応: loop12
- [x] **AY03** PWA 0件ヒントにクエリエコー/クリア導線が無い — 対応: loop12
- [x] **AY04** Redo Trash が元パス存在で復元済み判定し誤 trash する — 対応: loop12

### 却下（ループ12・事実確認）

- TrashSelected リバインドで .trashinfo を trash する指摘:
  `trash_key_action("delete", false)` は常に NoPurge のため
  Trash ビューではキー種別に依らず拒否される。誤検知。

## 進行中（ループ13 バックログ）

### 優先度: 高（ループ13・confirmed）

- [x] **AZ01** Search で削除ディレクトリの子孫結果が放置され無通知 — 対応: loop13
- [x] **AZ02** OS watch 消失後に張り直す経路が無い — 対応: loop13
- [x] **AZ03** normalize_events の From/To が複数パス 2個目以降を破棄 — 対応: loop13
- [x] **AZ04** 設定 Save が全オーバーレイを閉じエラーが消える — 対応: loop13

### 優先度: 中（ループ13・confirmed）

- [x] **AZ05** fingerprint が秒丸め＋is_symlink 省略で変更を見逃す — 対応: loop13
- [x] **AZ06** reconcile の partially_failed が全件成功としてマークされる — 対応: loop13
- [x] **AZ07** undo Trash の既存パス判定が他人のファイルを陳腐化 — 対応: loop13
- [x] **AZ08** 一括リセット footer が keymap dialog に欠落 — 対応: loop13
- [x] **AZ09** PWA 検索 input にモバイル自動補正の抑止属性を追加 — 対応: 親
- [x] **AZ10** PWA スクロール位置の設計判断（変更不要）
  — 対応: 親（r13 で判断）
  — render() の replaceChildren で結果変更時に先頭へ戻るのは検索 UI として望ましい。
  — キーボード選択時は r11 の scrollIntoView({block:'nearest'}) が viewport 追従する。
  — 両者は競合せず、現状を仕様どおりと確定。

### 優先度: 低（ループ13・confirmed）

- [x] **AZ11** 展開ディレクトリ状態が undo/redo で復元されない — 対応: loop13
- [x] **AZ12** 検索スナップショットが part なキーしか保持しない — 対応: loop13

## 進行中（ループ14 バックログ）

### 優先度: 高（ループ14・confirmed）

- [x] **BA01** コピー検証がサイズのみで内容バイトを照合しない — 対応: loop14
- [x] **BA02** DEB 同梱 app/NOTICE.md の LICENSE 相対リンクが切れる — 対応: loop14
- [x] **BA03** PWA に複数選択・範囲選択の手段が無い — 対応: loop14

### 優先度: 中（ループ14・confirmed）

- [x] **BA04** sync_all() 失敗が無視され永続化されないコピーが成功報告 — 対応: loop14
- [x] **BA05** 空予約ファイルが最終宛先に一時生成されクラッシュで残る — 対応: loop14
- [x] **BA06** check.sh --app で core/win32 テストとクロスチェックが黙スキップ — 対応: loop14
- [x] **BA07** PWA が再描画のたびに選択を破棄 — 対応: loop14
- [x] **BA08** NOTICE のライセンス要約が deny.toml 許可集合と不一致 — 対応: loop14
- [x] **BA09** 同梱 README の NOTICE.md リンクが要約版を指す — 対応: loop14
- [x] **BA10** source_region のマーカー一意性が未検証 — 対応: loop14
- [x] **BA11** 本番/テスト分割失敗時に全ソースへフォールバック — 対応: loop14
- [x] **BA12** グリッド表示にポインタの並び替え手段が無い — 対応: loop14（F1 案内に設定 UI 導線を追記）
- [x] **BA13** 内容検索の symlink 判定と File::open の間に TOCTOU — 対応: loop14

### 優先度: 低（ループ14・confirmed）

- [x] **BA14** NOTICE/SPDX に著作権者・年号が無い — 対応: loop14
- [x] **BA15** README の DEB 収録物一覧に app/NOTICE.md が無い — 対応: loop14
- [x] **BA16** ワークスペース直下とメンバー配下に Cargo.lock が3つ — 対応: loop14
- [x] **BA17** source-guard のマジックナンバー固定が誤検知しやすい — 対応: loop14
- [x] **BA18** PWA にソートキー／方向を指定する手段が無い — 対応: loop14
- [x] **BA19** PWA タッチ長押しに複数選択の導線が無い — 対応: loop14
- [x] **BA20** case-only rename の中間一時名が O_EXCL なし — 対応: loop14
- [x] **BA21** gates ジョブに timeout-minutes が無い — 対応: loop14
- [x] **BA22** ツールチェーン未ピン留めで fmt/clippy が drift に晒される
  — 対応: loop14（1.92 ピンは実検証で不採用・理由記録）
- [x] **BA23** deny.toml の unused-ignored-advisory = allow が失効 ignore を隠す
  — 対応: loop14

## メモ

- **既知の制限（実装しないと判断済み）**: キーマップ一括リセット UI は未配線。
  F1 / README / 設定カードで `keymap` を `[]` にする手順を案内済み（R30 AI10）。

- グローバルTODO: `~/.config/opencode/todo.md`（複数プロジェクト横断のタスク用）。
- 前回実行の 55 項目は `notes/task-discovery/round-6/consolidated.md` に存在したが未反映だった。
  今回の 54 項目がそれを吸収・更新している（[最終レポート](notes/task-discovery-2/round-6/consolidated.md)）。
- 全レビューは読み取り専用で実施。ソースコードの修正は行っていない。
