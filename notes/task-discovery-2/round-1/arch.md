---
type: Reference
title: Round 1 — アーキテクチャ / 構造 レビュー
description: 読み取り専用エージェント r1-arch の所見（15 件）。親が現行コードで一部事実確認済み。
tags: [task-discovery, cross-review, architecture, gpui-file-explorer]
generated: 2026-11-17T23:25:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-arch-01 | `main.rs` モノリスと未実施の `ui/*` 抽出 | 構造改善 | `app/src/main.rs:10-22`, `:435-506`, `:508-5276`, `:5327-5479`, `:5480-5586`; `DESIGN.md:274-317`; `app/README.md:477` | フラットな 13 モジュールのうち実体はほぼ `main.rs` 1 ファイル（約 6000 行・`Explorer` 直接フィールド 39 個＋ネスト状態）。DESIGN §4.1 の `app.rs` / `ui/*` 分割は P0 許容・P1 課題として先送りされたまま機能が 3 倍に増えた。抽出候補は `tab_bar`(1221) / `entry_menu`(4144) / `list_row`(4412) / `grid_cell`(4608) / `sidebar`(4741) / `toolbar`(4924) / `status_bar`(5094) ほかダイアログ群。 | 差分レビュー・並行作業・テスト可能な View 境界を獲得 | L | 既知(ドキュメント記載あり) | 高 |
| r1-arch-02 | `LoadState::Error` が `FsError` を `String` へ平坦化 | 構造改善 | `app/src/main.rs:407-411`, `:694-703`; `app/src/model.rs:19-75`, `:131-141`; `app/README.md:481` | `read_entries` は構造化された `FsError{kind,path,os_message}` を返すが、UI 境界で `error.message_ja()` により即 `String` に変換され保持される。DESIGN §5.5 と README 差分表は `LoadState` が `FsError` を保持すると明記しており as-built と矛盾。 | エラー種別による再試行判定・表示分岐をロジック側でテスト可能に | M | 既知(ドキュメント記載あり) | 高 |
| r1-arch-03 | エラー型の分断（3 体系＋ `Result<_, String>` 乱用） | 構造改善 | `app/src/model.rs:19-33`; `app/src/ops.rs:64-92`; `app/src/trash_view.rs:18-34`; `app/src/preview.rs:100`, `:148`; `app/src/model.rs:292`, `:683`; `app/src/settings.rs:77` | `FsError`/`FsErrorKind`、`OpError`、そして `trash_view` / `preview` / `model::resolve_location_input` / `create_unique_folder` の `Result<_, String>` が併存。`From` 変換は存在せず `.map_err(|e| e.to_string())` に退化し種別を失う。 | エラー UX 集約の前提となる共通型を用意し変換ボイラプレートを削減 | M | 新規 | 高 |
| r1-arch-04 | UI スレッド上での同期ファイル I/O | 構造改善 | `app/src/main.rs:3032-3072`, `:2548`, `:3622`, `:1053`; `app/src/model.rs:148-170`; `DESIGN.md:390-399` | `open_properties` が `symlink_metadata` / `metadata` / `read_link` を同期的に実行。`this.update(...)` 内の `trash_view::list()` と監視イベントの `entry_for_path` も UI スレッドでメタデータを読む。`dir_stats` / `read_entries` は background 化されており対応が不均一。 | 遅い FS でのフリーズ要因を除去しスレッド契約を検証可能に | M | 既知(ドキュメント記載あり) | 高 |
| r1-arch-05 | 設定ミラー状態による真実源の曖昧さ | 構造改善 | `app/src/main.rs:475-479`, `:887-896`, `:1116-1128`, `:1293-1309`; `app/src/settings.rs:60-67` | `Explorer.recent` / `bookmarks` / `tabs` / `cwd` / `history` は `settings` のコピー。`refresh_*` と `persist_tabs` で双方向同期するが、同期漏れ一方で表示と保存がずれる。 | ミラーを集約し「再起動後だけ設定が違う」系バグを構造的に排除 | M | 新規 | 高 |
| r1-arch-06 | 選択状態の二重管理（`selected: usize` と `Selection`） | 構造改善 | `app/src/main.rs:442-445`, `:829-839`, `:1505-1517`; `app/src/model.rs:173-184`; `DESIGN.md:559-593` | DESIGN §5.3 は単一 `Selection` を契約するが、as-built は `selected: usize` と `selection: Selection` を併用し `rebuild_view` が双方を reconcile。`Selection` 自体も `paths: Vec` と `path_set: HashSet` をミラー。 | 3 層の同期ロジックを 1 つに統合しフォーカス復帰（R6-C-07）を単一経路で検証 | M | 既知だが未対応 | 高 |
| r1-arch-07 | `SortSpec.dirs_first` は保存だけされるデッド設定 | 残タスク | `app/src/model.rs:528-530`, `:543-557`, `:370-374`, `:470-495`; `app/README.md:367-368` | `dirs_first` は serde で永続化され `SortSpec::from_order` が常に `true` を書き込むが、`sort_entries` は常に dirs→files に partition するため `false` は無視される。README は「将来互換のため保存」と明記済み。 | 設定項目を削るか効かせるか決めスキーマと実装の齟齬を解消 | S | 既知(ドキュメント記載あり) | 高 |
| r1-arch-08 | デバッグフックと `eprintln!` がリリースビルドに残存 | 残タスク | `app/src/main.rs:376-402`, `:505`, `:511-633`, `:2274-2328`, `:2378-2381`; `cfg(debug_assertions)` は 0 件 | `DebugHooks` が本番 `Explorer` の常設フィールドになり、`GPUI_FE_DEBUG_*` 環境変数と `run_debug_undo`（実 FS 操作）が本番コードパスに存在。 | バイナリ縮小・意図しない操作トリガー除去 | S | 前回と重複(R6-C-10) | 高 |
| r1-arch-09 | ゴミ箱 undo 記録ロジックが 3 箇所に重複 | 構造改善 | `app/src/main.rs:2284-2301`, `:2548-2562`, `:3615-3626`; `app/src/trash_view.rs:34-52` | `trash_view::list()` → `original_path() == path` で ID を引き当てる処理が `run_debug_undo` / `redo_last` / `trash_paths` にほぼ同一で存在（3 回とも UI スレッドで list を呼ぶ）。 | ヘルパー集約で ID 照合ドリフトと同期 I/O を同時に潰す | S | 新規 | 高 |
| r1-arch-10 | crate ルートの glob import が下位モジュールに回避コードを強制 | 構造改善 | `app/src/main.rs:36`; `app/src/history.rs:95-97`; `app/src/model.rs:724`; `app/src/watcher.rs:182`; `app/src/main.rs:5732-5733` | `use gpui_kit::*;` が名前空間を汚し、`#[test]` 属性や標準型が影になるため各テストモジュールに回避コードが散在。 | 明示 import に変えモジュール境界を実際に isolated に | S | 新規 | 高 |
| r1-arch-11 | `icons.rs` / `preview.rs` の「GPUI 非依存」宣言と実態が矛盾 | 構造改善 | `app/src/icons.rs:8`, `:10`, `:28-37`; `app/src/preview.rs:12`, `:17`, `:25`; `DESIGN.md:303-305`, `:319-321` | DESIGN は icons を UI 非依存と定義するが `IconName`（gpui-kit アセット型）を直接返す。preview.rs は「GPUI-free」と書きながら `gpui_kit::ImageFormat` を import。 | 分類ロジックだけを純粋列挙へ切り出し headless テストと再利用を可能に | M | 新規 | 高 |
| r1-arch-12 | 共通ダイアログ部品が無く `render()` が毎フレーム 6 オーバーレイを構築 | 構造改善 | `app/src/main.rs:5481-5585`, `:2115-2160`, `:2694-2774`, `:2921-2990`, `:3147-3200`, `:3407-3442`; `DESIGN.md:306-315` | `render()` が 6 つの `Option<Div>` を毎フレーム生成し、各所で `.bg(black().opacity(0.45))` と `.font_weight(SEMIBOLD)` を繰り返す。設計の `ui/dialogs.rs` は未作成。 | `Dialog` コンポーネント 1 つに統一しスタイルの 6 重ドリフトを停止 | M | 既知だが未対応 | 高 |
| r1-arch-13 | `History` API が設計シグネチャと不一致で旧 API が残存 | 構造改善 | `app/src/history.rs:39-76`; `DESIGN.md:595-613`, `:331-333` | DESIGN §5.4 は `apply(target, mode)` を契約するが実装は `apply(current, target, mode)`。加えて `visit` / `go_back` / `go_forward` が残り `NavMode` の意味が呼び出し側に分散。 | 契約どおりの 1 メソッドに絞り current/target 取り違えを型レベルで防止 | S | 既知(ドキュメント記載あり) | 高 |
| r1-arch-14 | `Entry` 構築ロジックが 3 重複しフィールド知識が拡散 | 構造改善 | `app/src/model.rs:77-91`, `:96-126`, `:148-170`; `app/src/trash_view.rs:56-78` | `from_dir_entry` と `entry_for_path` が同一手順を ~25 行ずつ重複。`entries_from_items` は 7 フィールドを直接初期化し第 3 の経路を作る。名前は `name` / `name_text` / `sort_key` の 3 表現。 | 単一コンストラクタ＋`EntryKind` 化でフィールド追加時の 3 箇所修正を 1 箇所に | M | 新規 | 高 |
| r1-arch-15 | `settings.save()` が 10 箇所のミューテータに分散 | 構造改善 | `app/src/main.rs:640`, `:910`, `:1145`, `:1204`, `:1214`, `:1308`, `:1476`, `:1692`, `:1706`, `:3828`; `:1122-1129` | 保存が各操作ハンドラに直接埋め込まれ、`persist_tabs` だけが `debug.ephemeral` をチェック。 | `mutate()` 集約で ephemeral 规则和保存失敗ハンドリングを一箇所で保証 | M | 前回と重複(R6-C-15) | 中 |

## レンズ要約

データ層は GPUI 非依存でテスト可能な設計が実現できている。しかし境界の外側で `main.rs` が状態・I/O・UI・永続化を吸収し、設計済みの `ui/*` 抽出が先送りされたまま 6000 行に膨張したことが、エラー型の平坦化・ミラー状態・同期 I/O の不統一という複合負債を生んでいる。まずは `LoadState` / `Entry` の型化と `ui/*` の 1 モジュール抽出が費用対効果が高い。
