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

- [ ] **F6-01** Undo/Trash 項目の黙っての消失 — `pop_undo()` 後の早期 return と `trash_view::list()` 成功依存
- [ ] **F6-02** move の TOCTOU 上書きと symlink 攻撃連鎖 — `ops.rs:604,612` に予約機構なし
- [ ] **F6-03** リリース版デバッグフック（29 種）による実 FS 操作 — `cfg(debug_assertions)` 0 件
- [ ] **F6-04** 設定読取失敗が既定値で上書き／退避成功が無通知 — `settings.rs:200-202` → `main.rs:640`
- [ ] **F6-05** デバッグ値が起動時 save で実設定を汚染 — `ephemeral` ガードが一部の保存経路のみ
- [ ] **F6-06** 非 UTF-8 名・引数の lossy 化 — undo/trash ID/CLI/serde
- [ ] **F6-07** 検索の一致判定・上限・意味論 — NUL 判定、per-root cap 500、sort 前 cap
- [ ] **F6-08** 選択・フォーカスの不変式破壊 — `selected` / `Selection` / `path_set` の 3 層ミラー
- [ ] **F6-09** 操作排他・キャンセル・背圧の偏り — `op_task`/`op_cancel` が transfer のみ
- [ ] **F6-10** グリッドの scroll index 誤り — 行 index に entry index を渡す（描画上の軽微な問題）
- [ ] **F6-11** 設定一時ファイルのリークとパーミッション — tmp 残存・0600→0644
- [ ] **F6-12** 保存タブが CLI 引数／`.desktop %f` を上書き — `restore_session()` の早期 return
- [ ] **F6-13** 操作成否・Undo 確認文と実保証の乖離 — open/reveal 戻り値破棄、貼付全中止
- [ ] **F6-14** macOS/iOS/Android で trash ビューがビルド不能 — `trash 5.3.0` の `os_limited` 除外

### 優先度: 中

- [ ] **F6-15** 履歴ナビ破損・trash redo の誤報告
- [ ] **F6-16** 検索結果の右クリックが `self.cwd` に適用される
- [ ] **F6-17** 監視の世代/cwd ガード欠如・抑制窓のイベント破棄
- [ ] **F6-18** UI スレッド上の同期ファイル I/O
- [ ] **F6-19** エラー型の平坦化と分断（`FsError`/`OpError`/`String`）
- [ ] **F6-20** 設定スキーマ検証・上限・マイグレーションの不足
- [ ] **F6-21** 設定の並列更新と保存失敗の非通知
- [ ] **F6-22** 検索／プレビューの鮮度・性能・形式判定
- [ ] **F6-23** ゴミ箱 undo/redo の部分成功報告
- [ ] **F6-24** データモデルの型不変条件退化（Entry/Selection/History/path）
- [ ] **F6-25** 色覚アクセシビリティと text alternative（ツールバー 8 ボタン）
- [ ] **F6-26** i18n・日本語表記の散在とロケール並び順
- [ ] **F6-27** 配布物のゲート・運用（`.desktop` MimeType 欠落、DEB 無検証）
- [ ] **F6-28** ビルド再現性と CI 透明性（配布のみ `--locked` なし）
- [ ] **F6-29** サプライチェーン監査とライセンス
- [ ] **F6-30** ドキュメント数値の真実源欠落（132/167/168、判定 4 回反転）
- [ ] **F6-31** テストの実質欠如（空振り assert、trivially pass）
- [ ] **F6-32** セッション復元とウィンドウスコープ
- [ ] **F6-33** ドラッグ&ドロップとゴミ箱ビュー操作の不足
- [ ] **F6-34** ワークフロー改善（プレビュー連続移動、新タブで開く、即命名）
- [ ] **F6-35** キーボード／ダイアログの細部
- [ ] **F6-36** エラー・ステータスメッセージの情報量と競合
- [ ] **F6-37** 作業効率ロードマップ（一括リネーム、転送後検証、ターミナル起動）
- [ ] **F6-38** 個人化ロードマップ（保存検索、ブックマークラベル、Fuzzy jump、キーマップ）

### 優先度: 低（調査タスク）

- [ ] **F6-39〜F6-48** 未確認項目（攻撃連鎖の実機再現、100k 実測、シャットダウン、WASM/AGPL、Windows/macOS 意味論、第三者ライセンス）— 詳細は[最終レポート](notes/task-discovery-2/round-6/consolidated.md)の「未確認」節を参照

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
