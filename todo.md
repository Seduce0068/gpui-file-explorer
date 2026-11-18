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
  26ccdbd (CLI 引数 )。undo/trash ID は残課題
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
- [x] **F6-36** エラー・ステータスメッセージの情報量と競合 — 対応: ad5949c（定数と共存ルール、呼び出し側移行は残課題）
- [x] **F6-37** 作業効率ロードマップ — 転送後検証と一括リネーム API を実装。ターミナル起動は F6-50 へ分離。
- [x] **F6-38** 個人化ロードマップ — データモデル完了（保存検索 / ブックマークラベル、11テスト）。UI 配線は F6-49 へ分離。

- [x] **F6-49** 個人化 UI 配線 — 保存検索とブックマークラベルを接続。Fuzzy jump / キーマップは F6-51 へ分離。

- [x] **F6-50** ターミナル起動 — 設定可能コマンド（Linux 既定 konsole）と F4 / コンテキストメニューを追加
- [x] **F6-51** Fuzzy jump と個人化キーマップ — Ctrl+P オーバーレイとテーブル駆動キーマップを追加（再割当はメモリ上のみ）

### 優先度: 中（第3回発掘・confirmed）

- [x] **R1-ST01** rename_entry が壊れた symlink を NotFound で拒否（copy/move と不一致） — 対応: 779b327
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

### 優先度: 低（調査タスク）

- [x] **F6-39〜F6-48** 未確認項目を調査し、事実／未確認／必要環境を整理 — 成果物:
  [unconfirmed-followup.md](notes/task-discovery-2/unconfirmed-followup.md)。実機再現・
  性能実測・法的判断は未実施で、必要手順を同ファイルに記録。

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
