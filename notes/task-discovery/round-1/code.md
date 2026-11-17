---
type: Reference
title: ソースコード潜在作業の発掘（ラウンド1・コードレンズ）
description: >-
  app/src/*.rs と build.rs / Cargo.toml を精読し、未実装・握りつぶし・
  陳腐化コメント・ハードコード制限などを file:line 根拠付きで列挙した
  残タスク・機能追加案の候補一覧。
tags:
  - task-discovery
  - gpui-file-explorer
  - source-audit
  - latent-work
generated:
  by: "r1-code"
  at: "2026-11-17T19:37:08+09:00"
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 発掘結果

全findingsは `app/src/` を実読して確認した file:line に基づく。`review-report.md` /
`notes/verify-report.md` に既出の項目は `novelty` にその旨を明示した。
なお全件が「ドキュメントに既知の制限として記載があるが、コード着手には至っていない」
傾向をもち、6回目の統合では「新規」5件（r1-code-01/02/03/09/10/15）を優先して
重複排除することを推奨する。

### 発見一覧

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-code-01 | デバッグフック3種が README に未記載 | 残タスク | main.rs:594, 603, 618 / app/README.md:390-405 | `GPUI_FE_DEBUG_PROGRESS`(594) / `GPUI_FE_DEBUG_UNDO_HISTORY`(603) / `GPUI_FE_DEBUG_TAB_MOVE`(618) が `ephemeral` 判定とフック初期化に存在するが、README のデバッグフック一覧に無い。SEARCH_QUERY や DROP_TARGET は記載済み。 | スクリーンショット検証の再現性向上。隠し機能の発見性 | S | 新規 | 高 |
| r1-code-02 | undo 履歴オーバーレイのキー表記が陳腐化 | 構造改善 | main.rs:496 vs strings.rs:46, main.rs:2191 | フィールド コメントが「Ctrl+Shift+H」と記載するが、実装のショートカットと F1 ヘルプは「Ctrl+Shift+U」。コード内だけで完結するドキュメント不整合。 | 将来の保守者を惑わす誤情報の除去 | S | 新規 | 高 |
| r1-code-03 | preview.rs のモジュール doc が NUL 判定範囲の修正前のまま | 構造改善 | preview.rs:8-9 vs preview.rs:163 | 冒頭コメントは「NUL byte in the first 8 KiB」と書くが、実装は `bytes.contains(&0)` で読み取った全範囲を走査するよう修正済み（review-report 326-327 行）。コメントだけが旧仕様。 | セキュリティ上重要な判定仕様の記述正确性 | S | 新規 | 高 |
| r1-code-04 | 非 Linux のボリューム一覧が空実装 | 残タスク | mounts.rs:6-7, 146-151 | `#[cfg(not(target_os = "linux"))] list_mounts()` が常に `Vec::new()` を返す。「for now」「can be added later」と明記された未完成スタブ。 | Windows/macOS で「ドライブ」セクションが機能する | L | 既知(ドキュメント記載あり) | 高 |
| r1-code-05 | 非 Unix のシンボリックリンク・コピーが Unsupported | 残タスク | ops.rs:375-391 | `copy_symlink` は `#[cfg(not(unix))]` で `let _ = (src, dest, stats);` の後に Unsupported を返す。Windows の `symlink_file` / `symlink_dir` 未実装。 | Windows で symlink を含むコピーが成功する | M | 既知だが未対応 | 高 |
| r1-code-06 | コピーの sync_all 失敗が握りつぶされている | 調査項目 | ops.rs:314-315 | `let _ = output.sync_all();` は best-effort とコメントにあるが、設定保存(settings.rs:276) は `sync_all()?` で失敗を返す。コピーは完全性を保証しない方針で統一されているか要確認。 | コピー済みファイルの永続性保証の明確化 | S | 新規 | 中 |
| r1-code-07 | 設定保存のディレクトリ fsync 未実施 | 残タスク | settings.rs:245-247, 271-278 | tmp ファイルは `sync_all()?` するが、rename 後の親ディレクトリは fsync しない（best-effort と明記）。電源断時の完全な耐性は保証外。 | クラッシュ時の設定消失リスク低減 | S | 既知(ドキュメント記載あり) | 高 |
| r1-code-08 | 設定スキーマの v2 以降マイグレーション枠が空 | 残タスク | settings.rs:203-224 | `migrate()` は `0 | 1 => {}` と `future =>` 読み取り専用のみ。将来 v2 を入れる `2 => { ... }` アームがコメントで予約されているだけで、ダウングレード検出も未実装。 | 将来のスキーマ変更で設定を失わない | M | 既知(ドキュメント記載あり) | 高 |
| r1-code-09 | 戻る履歴上限 200 がハードコード | 調査項目 | history.rs:31-36, 44-46 | `max_back: 200` が `new()` 内リテラルで、定数化も設定化もされていない。undo は `MAX_UNDO`、recent/bookmarks/search は `MAX_*` 定数化済みで不揃い。 | チューニング可能性と一貫性 | S | 新規 | 中 |
| r1-code-10 | マウント FS ホワイトリストが固定 | 調査項目 | mounts.rs:24-55 | `ALLOWED_FS` が 20 種のベタ書き配列（mounts.rs:28-50 を実数え）。新規 FS（例: apfs / refs / fuse バリエーション）は自動的に除外され、サイドバーに出ない。 | 新ファイルシステム表示の追随性 | S | 新規 | 中 |
| r1-code-11 | ファイルを開く / リビールが常に成功文言を返す | 残タスク | main.rs:1371-1373, 3455-3457 | `cx.open_with_system` / `cx.reveal_path` は戻り値が無く、呼び出し側は成功文字列をセットするだけ（AC-9）。xdg-open 失敗を検知できない。 | 失敗時の誤案内を防ぐ（D-Bus/終了コード監視） | M | 既知(ドキュメント記載あり) | 高 |
| r1-code-12 | 進捗チャネルが foreground task を占有 | 構造改善 | main.rs:1842-1873 | `progress_rx.recv().await` が常駐タスクを専有。80ms スロットルは「到着後フィルタ」であり、連続送信で notify が発生しうる（レビュー A-7 既出）。16ms tick の try_recv バッチ化候補。 | UI スレッド負荷の低減、進捗表示の安定化 | M | 既知(レビュー記載あり) | 高 |
| r1-code-13 | 複数ルート検索の由来ラベル強化が未着手 | 機能追加案 | main.rs:113-144, 3695-3696, 3802-3807 | `display_name_for` は検索ルート相対パスで表示するが、どのルート由来かは表示しない。ステータスは「N 箇所」のみ。README「今後の候補」の検索結果ソース表示に相当。 | 同名ファイルが複数ルートに散在する際の判別 | M | 既知(ドキュメント記載あり) | 中 |
| r1-code-14 | SVG プレビューの形式判定が先頭 1 KiB のみ | 残タスク | preview.rs:81-88 | `sniff_image_format` は `<svg` を先頭 1024 バイトでしか探さない。XML 宣言やコメントが長い SVG は判定不能でエラー表示になる。 | SVG プレビューの互換性向上 | S | 既知(ドキュメント記載あり) | 高 |
| r1-code-15 | DirWatcher の Drop が監視スレッド終了を待たない | 調査項目 | watcher.rs:160-166 | `Drop` は debouncer の Drop による停止フラグ設定に依存し、`stop(self)` を `&mut self` から呼べないため明示的な join を行わない。終了直後のイベント取りこぼし/競合の可能性は未検証。 | シャットダウン時の監視スレッド競合リスク低減 | M | 新規 | 中 |

### 各項の補足

- r1-code-01: `ephemeral` 配列（main.rs:605-626）は TAB_MOVE を含む一方、README の
  デバッグフック説明（390-405 行）には無い。UNDO_HISTORY フックは main.rs:5381 で
  オーバーレイを開くために使われる。
- r1-code-03: review-report はコード修正を「解消済み」とするが、モジュール冒頭の
  doc は更新されていない。純粋なコメント修正でリスクなし。
- r1-code-06: 設定は `sync_all()?` で失敗を上位へ返すが、コピーは `let _ =` で無視。
  方針の不一致なのか意図的な棲み分けなのかを設計ドキュメントで明示するのが先決。
- r1-code-09: `MAX_UNDO`(undo.rs:19) / `MAX_RECENT_DIRS` 等(settings.rs:35-39) は
  定数化済み。`max_back` だけがリテラルで、設定ファイルからも変更不可。
- r1-code-11: 既知の制限として README / review-report の双方に載るが、ソース側の
  成功文言ハードコード（main.rs:1372 / 3456）がその直接根拠。
- r1-code-12: 進捗ポンプは `while let Ok(...) = recv().await` で開始され、
  送信側ドロップで終了する。80ms 判定は受け取った後のスキップ。

## レンズ総評

本レンズは「未完成のロジック」そのものより、**実装は進んだが周辺（コメント・
ドキュメント・定数・方針）が追従していない「軽微な遅れ」**を中心に拾った。
具体的には r1-code-01/02/03 の3件は数行の修正で解消でき、費用対効果が高い。
r1-code-04/05/11/13/14 は既知の制限をソース側で再確認したもので、6回目の
統合では他レンズの同趣旨 findings と吸収してよい。r1-code-06/09/10/15 は
「設計判断か残作業か」の切り分けが必要な調査項目で、特に sync_all の方針
(r1-code-06) はデータ完全性に関わるため優先的に合意形成すべきである。
全体としてブロッカーは無く、既存レビューが示した「承認」判定を覆すものは
見つからなかった。
