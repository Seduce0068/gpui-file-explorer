---
type: Reference
title: Round 5 — 将来機能 / ロードマップ レビュー
description: 読み取り専用エージェント r5-future の所見（15 件・優先順位と設計スケッチ付き）。
tags: [task-discovery, cross-review, roadmap, gpui-file-explorer]
generated: 2026-11-18T00:24:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-future-01 | 分割（デュアルペイン）ビューとペイン間転送 | 構造改善 | `DESIGN.md:95`, `:357`, `:1056`; `app/README.md:88`, `:204-211` | L の本体は後回しでもペイン構造だけは比較・転送系の前に ADR 化すべき。設計スケッチ: `Vec<Pane{...}>` + active_pane、watch はペインごと gen 付き。 | 高 | L | 前回と重複(R6-C-01) | 高 |
| r5-future-02 | 一括リネーム（連番・パターン・dry-run） | 機能追加案 | `app/README.md:170-173`; `ops.rs:648-689` | F2 は1件のみ。トークン置換＋「旧名→新名」テーブルと衝突件数を実行前に表示。既存 rename_entry を表示順に呼ぶだけでエンジン変更不要。 | 中高 | M | round1-4と重複(r1-ux-10) | 高 |
| r5-future-03 | 転送後検証・チェックサム・重複検出の優先順序付け | 機能追加案 | `ops.rs:446-480`; `model.rs:589-601` | 転送後検証→チェックサム表示→重複検出の順。コピーループで BLAKE3 等を更新し完了時比較。DefaultHasher は fingerprint 用で内容同一性に使えない。 | 高 | M | 前回と重複(R6-C-02) | 高 |
| r5-future-04 | undo の再起動後永続化 | 残タスク | `undo.rs:18-19`, `:51-56`; `app/README.md:411-412` | UndoAction を serde 化し state へ追記、起動時に陳腐化項目を検証して捨てる。まず R6-B-02 を直すのが先決。 | 中 | M | 前回と重複(R6-B-14) | 高 |
| r5-future-05 | 衝突解決 UI（上書き／別名／スキップの明示選択） | 機能追加案 | `ops.rs:8`, `:51-61`; `app/README.md:167-169` | 貼り付けは常に ConflictPolicy::Rename。第一段として競合件数と結果の事前通知だけを出す段階実装が妥当。 | 中高 | M | round1-4と重複(r2-scn-12) | 高 |
| r5-future-06 | 名前付き保存検索 | 機能追加案 | `settings.rs:69`, `:138-146`; `app/README.md:201-202` | recent_searches の履歴チップのみ。SavedSearch{name,query,scope,...} を settings に追加しサイドバーから再実行。設定 UI を待たず前倒し可能。 | 中 | M | 前回と重複(R6-A-18) | 高 |
| r5-future-07 | ブックマークの別名とディレクトリのタグ／色分け | 機能追加案 | `settings.rs:63`, `:116-133`; `main.rs:4856-4858` | Vec<PathBuf> を Bookmark{path,label,color} へ。serde untagged で旧形式を読む後方互換が必須。 | 中 | M | round1-4と重複(r1-ux-06) | 高 |
| r5-future-08 | Fuzzy jump／クイックオープン（Ctrl+P） | 機能追加案 | `app/README.md:80-119`, `:242-243`; `DESIGN.md:43` | 新規。bookmarks/recent/tabs/祖先を横断するあいまい検索オーバーレイ。新規 I/O ほぼ不要でキーボード中心ユーザーに費用対効果が高い。 | 中高 | M | 新規 | 中 |
| r5-future-09 | 選択フォルダをターミナルで開く | 機能追加案 | `app/README.md:116`; `main.rs:4207-4340` | $TERMINAL → konsole/kgx/xterm を引数で起動。シェル文字列を組まずインジェクション回避。新規依存なしで統合群の最優先。 | 高 | S | round1-4と重複(r1-ux-01) | 高 |
| r5-future-10 | 「他のアプリで開く」アプリ選択 UI | 調査項目 | `main.rs:1373-1375`; `DESIGN.md:799` | GPUI にアプリ指定 API が無く実現性調査が先。xdg-mime + .desktop 列挙のスケッチ。D-Bus/プロセス委譲の spike を ADR 化。 | 高 | M | 前回と重複(R6-A-02) | 中 |
| r5-future-11 | クイックルックの拡張（まずシンタックスハイライト） | 機能追加案 | `app/README.md:438-441`; `preview.rs:24-38`, `:56-75` | テキスト延長で依存追加が比較的小さく開発者に直接価値。PDF はラスタライザ能力と AGPL 互換依存が未調査で後回し。 | 中 | M | 前回と重複(R6-B-11) | 中 |
| r5-future-12 | アーカイブの閲覧と展開 | 機能追加案 | `DESIGN.md:24`, `:1075`; `icons.rs:52` | まず中身を読み取り専用一覧、展開は次段。zip-slip 対策と暗号化の扱いを最初から設計。 | 中高 | L | 既知(ドキュメント記載あり) | 中 |
| r5-future-13 | リモート／ネットワーク場所の OS マウント委譲 | 機能追加案 | `DESIGN.md:24`, `:775`; `mounts.rs:1-10` | プロトコル自作せず gvfs-mount/sshfs へ Command 委譲。マウント後は通常ローカルパスとして既存コードが動き L を M に圧縮。 | 中 | M | 前回と重複(R6-C-05) | 中 |
| r5-future-14 | キーバインド再割当と個人化設定の集約 | 構造改善 | `DESIGN.md:732`, `:847`; `settings.rs:53-69` | on_key_down ハードコードで keymap 皆無。まず key→action 名マッピングを1フィールド追加しテーブル駆動化。 | 中 | L | 既知だが未対応 | 中 |
| r5-future-15 | 読み取り専用 Web デモ（別 crate の wasm 化） | 調査項目 | `README.md:96-104`; `notes/wasm-browser-notes.md:28-50` | アプリ本体の wasm 化は後回し。バンドルしたサンプルツリーを仮想 FS として検索・グリッド・プレビューのみ再実装する別 crate を推奨。 | 中低 | L | 既知(ドキュメント記載あり) | 高 |

## レンズ要約

既存実装は「安全なローカル操作」の完成度が高い一方、作業効率（分割ペイン、一括リネーム、あいまいジャンプ、転送検証）と個人化（ラベル、保存検索、キーマップ）が薄い。着手順は低工数即効の 09 → 02 → 03 を先にし、構造を要する 01 はその前にペイン設計だけ ADR 化するのが妥当。大半は DESIGN P2/P3 または R6 項目の再提示だが、実行順序と設計スケッチを付与した。
