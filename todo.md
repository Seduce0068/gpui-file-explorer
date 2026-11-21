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
- [x] **F6-19** エラー型の平坦化と分断（`FsError`/`OpError`/`String`） — 対応: 作業ツリー（共通 AppError）
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
- [x] **F6-36** エラー・ステータスメッセージの情報量と競合 — 対応: ad5949c（定数と共存ルール、呼び出し側は 44 箇所を定数へ移行済み（aeb687c））
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

## メモ

- グローバルTODO: `~/.config/opencode/todo.md`（複数プロジェクト横断のタスク用）。
- 前回実行の 55 項目は `notes/task-discovery/round-6/consolidated.md` に存在したが未反映だった。
  今回の 54 項目がそれを吸収・更新している（[最終レポート](notes/task-discovery-2/round-6/consolidated.md)）。
- 全レビューは読み取り専用で実施。ソースコードの修正は行っていない。
