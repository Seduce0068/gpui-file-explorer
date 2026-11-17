---
type: Reference
title: Round 4 — データモデル / 不変条件 / 設計との drift レビュー
description: 読み取り専用エージェント r4-model の所見（14 件）。
tags: [task-discovery, cross-review, model, gpui-file-explorer]
generated: 2026-11-18T00:12:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-model-01 | Entry 三表現の不変条件が型で守られない | 構造改善 | `model.rs:77-91`, `:96-126`, `:148-171`; `trash_view.rs:56-78`; `DESIGN.md:496-500` | name/name_text/sort_key がすべて pub で単一コンストラクタが無く3経路＋テストが直接初期化。sort_key != lowercase(name_text) な Entry を作れてソート決定性も破綻。 | 不変条件とソート決定性 | M | round1/2/3と重複(r1-arch-14) | 高 |
| r4-model-02 | 2 コンストラクタで dangling symlink の size/modified が食い違う | 残タスク | `model.rs:107-115`, `:155-160`, `:592-600`; `DESIGN.md:502` | from_dir_entry は metadata().ok() のみ、entry_for_path は link_metadata フォールバック。同じ symlink が一覧と watcher で別 Entry になり fingerprint も揺れる。 | 一貫性 | M | 前回と重複(r5-model-07) | 高 |
| r4-model-03 | Selection の3層表現と同期ロジックの拡散 | 構造改善 | `model.rs:177-280`; `main.rs:442-445`, `:823-845`, `:1437-1525`; `DESIGN.md:559-593` | selected: usize + paths: Vec + path_set: HashSet を各 mutator が手作業ミラー。contains() と paths() が別ソースで rebuild_view が reconcile。 | 誤操作防止 | M | round1/2/3と重複(r1-arch-06) | 高 |
| r4-model-04 | History に current が無く DESIGN §5.4 の状態形状と drift | 構造改善 | `history.rs:21-36`, `:51-76`; `tabs.rs:11-21`; `DESIGN.md:599-613` | DESIGN は History{back,forward,current}/apply(target,mode)、実装は current 無しで apply(current,target,mode)。cwd の3コピーがずれ、古い current を渡すと履歴が汚染。 | 型契約の整合 | M | 新規 | 高 |
| r4-model-05 | `#[derive(Default)]` の History が max_back=0 を生む | 構造改善 | `history.rs:22-36`; `tabs.rs:16-22`; `DESIGN.md:600` | 上限200は new() でしか設定せず default() は max_back=0 で visit() が全履歴を即捨てる。production は未使用だが derive 追加時の罠。 | 型不変条件 | S | 既知だが未対応 | 高 |
| r4-model-06 | Tabs::closed が PathBuf のみで履歴を失い無上限 | 構造改善 | `tabs.rs:10-14`, `:25-32`, `:103-121`; `DESIGN.md:595-613` | close_active が cwd しか残さず restore_last_closed が空 History を生成。closed は上限も永続化も無く無限増加。 | セッション復元とメモリ | M | 既知(ドキュメント記載あり) | 高 |
| r4-model-07 | UndoAction が新規フォルダ作成を網羅していない | 機能追加案 | `undo.rs:2-31`; `model.rs:683-697`; `strings.rs:37` | enum は Rename/Trash のみで、空フォルダ作成は安全に逆せるのに Ctrl+Z が効かない。copy/move の除外理由は新規作成に当てはまらない。 | 操作性 | M | 新規 | 中 |
| r4-model-08 | UndoAction::Rename の parent 情報が undo で捨てられる | 構造改善 | `undo.rs:25`; `main.rs:2369-2391`; `ops.rs:651-660` | フル PathBuf を保持するが undo は file_name() だけ取り src.parent() へ join。異なる親を許す型なので将来 move 系を記録すると誤復元。 | 将来の誤復元防止 | M | 新規 | 中 |
| r4-model-09 | Trash redo がゴミ箱再取得失敗時にアクションを失う | 残タスク | `main.rs:2518-2534`, `:2562-2577`; `undo.rs:30` | redo が list() 失敗/ID 不在で push を丸ごとスキップ。ファイルは既にゴミ箱へ移っているのに undo スタックに残らない。 | Undo 完全性 | S | 新規 | 高 |
| r4-model-10 | Settings の型名・シグネチャが DESIGN §5.7 と drift | 構造改善 | `settings.rs:43-49`, `:102-104`, `:237-243`; `DESIGN.md:697-718` | ViewMode vs ViewLayout、config_path()->Option<PathBuf>、save() が () でログのみ。保存失敗を呼び出し側が検出できない。 | 設計との整合 | S | 新規 | 高 |
| r4-model-11 | Settings::validate が last_dir のみで配列・active_tab を検証しない | 残タスク | `settings.rs:51-78`, `:226-233`; `tabs.rs:46-57`; `DESIGN.md:8.1` | recent_dirs/bookmarks/recent_searches は mutator で守るが load 時はバイパス、tabs 空＋active_tab>0 も検証しない。 | ロード時不変条件 | S | round1/2/3と重複(r1-correct-13) | 高 |
| r4-model-12 | ユーザー向け日本語が model.rs に散在（§8.4 集約契約違反） | 構造改善 | `model.rs:53-68`, `:288-314`, `:357-366`, `:643-657`, `:683-697`; `DESIGN.md:857-862` | FsError::message_ja / resolve_location_input / SortKey::label / human_age / 新規フォルダ名が直書き。strings.rs 自身も存在しない「DESIGN.md §8.5」を参照。 | i18n 準備 | M | 新規 | 高 |
| r4-model-13 | SearchResult.match_line と search_lines HashMap の二重保持 | 構造改善 | `search.rs:94-101`; `main.rs:453`, `:1013`, `:3830-3834`, `:3862-3872` | SearchResult を捨てて path→u32 Map へ移しフィルタ後は Map だけが真実。型の「Entry を汚さない」意図が活きていない。 | 単一真実源 | M | 新規 | 高 |
| r4-model-14 | trash view の Entry.path が trash id で「唯一の真実」契約を破る | 構造改善 | `trash_view.rs:54-75`; `model.rs:77-91`; `DESIGN.md:496-497` | path = PathBuf::from(&item.id) で .trashinfo ID を入れ is_dir=false/size=0 固定。ゴミ箱内ディレクトリがファイル表示で open_path の exists() を view 別ガードで誤魔化している。 | 型の意味の明確化 | M | 新規 | 高 |

## レンズ要約

DESIGN §5 が型で表現しようとした不変条件（Entry の triple、History.current、Selection の単一表現、path の意味）が、すべて pub フィールド＋複数コンストラクタ＋呼び出し側の手作業同期に退化している。新規候補は History.current 欠落、Undo の親情報/redo 損失、SearchResult の二重保持、trash id の path 拡張に集中。
