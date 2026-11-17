---
type: Reference
title: PASS 6 兼任クラスタ K-O の重複吸収と妥当性判定
description: >-
  クラスタ K（機能追加 UX）・L（入力モデル）・M（ファイル操作安全性）・N（ライフサイクル／CLI）・
  O（a11y／i18n）の生 finding をルート因ごとに統合し、妥当性・優先度・工数・次の一手を付与した
  PASS 6 最終統合表。
tags:
  - task-discovery
  - pass-6
  - dedup
  - cluster-c
generated: 2026-11-17T21:40:28+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 統合項目一覧

| ID | title | category | sources | evidence | validity | priority | effort | next-action |
|---|---|---|---|---|---|---|---|---|
| R6-C-01 | 分割（デュアルペイン）ビューとペイン間リンク | 構造改善 | r2-cmp-01, r2-scn-01, r2-scn-02 | app/README.md:88,204-208; DESIGN.md:95（ADR-002 は1ウィンドウ1状態）; r2-scn-02 は r2-scn-01 に依存 | 確定 | 中 | L | ペイン配列の状態モデル／watcher 管理を含む ADR-002 再設計方針を先に決定する |
| R6-C-02 | ハッシュ基盤（チェックサム表示・重複検出・転送後検証） | 機能追加案 | r2-cmp-05, r2-scn-05, r2-scn-10 | app/README.md:229-232,428-430; ops.rs:506-556; 既存 DefaultHasher は変更検知用で内容同一性判定に不可 | 確定 | 中 | M | ストリーミング・ダイジェストの共通モジュール API を設計し3機能で共有する |
| R6-C-03 | 比較・容量可視化・プロパティ／メタデータ拡張 | 機能追加案 | r2-cmp-06, r2-scn-03, r2-cmp-07, r2-scn-06, r2-cmp-08, r2-cmp-13, r2-scn-04, r1-ux-12 | main.rs:1341-1371（open_with_system は単一パス）; app/README.md:175-177,229-232,441-442; model.rs:349-363（SortKey 4種）; ops.rs:523 | 確定 | 中 | L | 外部ツール未インストール時の代替を仕様化し、EXIF は読み取り専用表示から段階導入する |
| R6-C-04 | 日常操作・一覧カスタマイズ・パスコピー | 機能追加案 | r1-ux-01, r1-ux-02, r1-ux-03, r1-ux-04, r1-ux-05, r1-ux-06, r1-ux-07, r1-ux-08, r1-ux-09, r1-ux-10, r1-ux-11, r1-ux-13, r1-ux-15, r1-ux-17, r2-cmp-14, r2-cmp-15 | app/README.md:108-128,137-142,158-167,189-203,241-244,377-378; main.rs:1664-1708; r1-ux-15 は DESIGN P3 既知 | 確定 | 中 | M | create_unique_folder／ConflictPolicy::Rename を流用し、低コスト・高頻度操作から実装する |
| R6-C-05 | 操作発見性とリモート連携（方針確認が必要な拡張） | 調査項目 | r2-cmp-04, r2-scn-11, r2-cmp-12 | strings.rs:7-77（HELP_SECTIONS 約30件）; app/README.md:478（Action 化は保留）; model.rs:525-536; mounts.rs:1-7 | 未確認 | 低 | M | ADR で公式に保証する操作面とリモート VFS の是非を決定してから着手する |
| R6-C-06 | 検索ビューのキーボード操作不能（Enter が選択結果を開かない） | 残タスク | r3-input-01 | main.rs:1528-1533（Search は Escape 以外を early-return）, :1613（enter→open_selected に到達不可）, app/README.md:100; 親確認 round-3/VALIDATION.md | 確定 | 高 | S | Search でも open_path 系を許可し、検索完了時にリストへ focus を戻す再現テストを先に追加する |
| R6-C-07 | 選択・フォーカス復帰の不整合 | 残タスク | r3-input-02, r3-input-04, r3-state-11 | main.rs:5434-5455（on_click なし）, :825-835（unwrap_or(0)）, :5287-5290; DESIGN.md:244,593 | 確定 | 高 | S〜M | 全クローズ経路と rebuild_view のフォーカス決定を単一ヘルパへ集約する |
| R6-C-08 | 入力意味の細部と既知の入力拡張 | 残タスク／既知 | r3-input-05, r3-input-06, r3-input-07, r3-input-08, r3-input-09, r3-input-10, r3-input-11, r3-input-12 | model.rs:222-235; main.rs:1413,1480-1531,4364-4370,4421-4435,4438-4445; DESIGN.md:140,265; r3-input-08/09/12 は既知 P1/P2 | 確定 | 中 | M | Escape 階層と修飾キーの意味をテスト仕様化し、省略名ツールチップを DESIGN P1 着手候補にする |
| R6-C-09 | 非同期コールバックの世代・ガード不整合 | 調査項目 | r3-async-06, r3-async-07, r3-async-08, r3-async-09, r3-async-10 | main.rs:1664-1701,3599-3646,1937-1942,3066-3099,1844-1873; r3-async-04/05 は cluster B 主担当 | 未確認 | 中 | M | operation_id と完了フラグを導入し、cluster B のガード基盤と共通化する |
| R6-C-10 | デバッグフックがリリースビルドでも作動 | セキュリティ／構造改善 | r2-sec-01, r4-cli-12 | main.rs:509,523,530,538（GPUI_FE_DEBUG_* を無条件読取り）, :5301-5314,5326-5368; cfg(debug_assertions) は app/src 全体に皆無（R6 独立再検証済み） | 確定 | 高 | S | フック全体を cfg(debug_assertions) で囲み、リリース設定で無効なことを統合テストで検証する |
| R6-C-11 | 設定 JSON の配列が読み込み時に上限適用されない | 残タスク | r2-sec-04 | settings.rs:169-200（migrate/validate のみ、validate は last_dir のみ検査）, :113/132/146 の truncate はミューテータ側; 親確認 round-3/VALIDATION.md（R6 再検証済み） | 確定 | 高 | S | デシリアライズ後に各 Vec を truncate し、tabs 上限・active_tab clamp・文件大小上限を追加する |
| R6-C-12 | ファイル操作の完全性・堅牢性・FS 意味論 | 残タスク／調査項目 | r1-code-05, r1-code-06, r1-qual-03, r1-qual-04, r1-qual-10, r1-qual-12, r2-sec-05, r2-sec-08, r4-fs-hardlink, r4-fs-xattr, r4-fs-timestamp, r4-fs-special, r4-fs-size-change, r4-fs-dir-perms, r4-fs-ownership, r4-fs-netfs, r4-fs-case-collision, r4-fs-move-metadata | ops.rs:313-315（sync_all は best-effort）, :344,362-368（rename 後に権限 best-effort）, :375-391（非 Unix symlink は Unsupported）, :395-441,482-499,557-577（再帰に深度上限なし）, :674-685; r4-fs-sparse は cluster C 主担当 | 確定 | 中 | L | 一時ファイル生成時に mode を先設定し、再帰を反復スタック化してクロスデバイス移動の劣化を通知する |
| R6-C-13 | TOCTOU・Windows 名検証・情報露出 | 調査項目 | r2-sec-02, r2-sec-06, r2-sec-07, r2-sec-09, r2-sec-11 | preview.rs:101-158（symlink 検査後に File::open）; ops.rs:240-252,152-169,76-129; search.rs:196-233; main.rs:1704-1707,3558-3568 | 未確認 | 中 | M | canonicalize した ID を保持して open 直前／削除直前に inode を再照合し、Windows 実機で予約名テストを追加する |
| R6-C-14 | エラー UX（集約・回復導線・記録の非対称） | 残タスク | r4-life-03, r2-scn-07, r2-scn-12, r5-err-01, r5-err-02, r5-err-03, r5-err-04, r5-err-05, r5-err-07, r5-err-08, r5-err-09, r5-err-10 | main.rs:1885（first_error、R6 再検証）, :1926-1966, :1693-1698, :3536-3546; DESIGN.md:742,253-255,767-769; r5-err-06 は r3-state-03 に吸収（dedup-index 注記） | 確定 | 中 | S〜M | 失敗項目一覧と「F5／親へ」導線を status へ集約し、各エラーへ tracing::warn! を1行追加する |
| R6-C-15 | 終了処理の集約（終了時保存と Ctrl+Q 迂回） | 残タスク／構造改善 | r4-life-01, r4-life-02 | main.rs:5619-5624（on_window_closed は quit のみで save なし）, :1585（Ctrl+Q が cx.quit() を直接呼ぶ）, :638 ほかでミューテータ保存; DESIGN.md:840; 親確認 round-4/VALIDATION.md（R6 再検証済み） | 確定 | 高 | S | 共通 shutdown 関数を用意し、保存・キャンセル・進行中操作の完了待ちを一箇所へ集約する |
| R6-C-16 | 起動順序・マルチウィンドウ・ライフサイクルの未確認事項 | 調査項目 | r4-life-04, r4-life-06, r4-life-07, r4-life-08, r4-life-09, r4-life-10 | main.rs:543-546,638-645,654-708,814,5639（open_window は1箇所）; settings.rs:53-78; ADR-002, DESIGN.md:26,95; lockfile/IPC 識別子なし | 未確認 | 中 | M | 初回ロード完了前の watch 差分キューイングを再現テスト化し、単一インスタンス方針を ADR で決定する |
| R6-C-17 | CLI 契約（引数・診断・終了コード） | 残タスク／機能追加案 | r4-cli-03, r4-cli-05, r4-cli-06, r4-cli-07, r4-cli-08, r4-cli-09, r4-cli-11 | main.rs:5523-5530,5559-5565,5574-5594,5606-5654; ExitCode／process::exit の使用なし; r4-cli-01 の ADR 決定が前提 | 確定 | 中 | S〜M | fn main() -> ExitCode 化し、不正引数 2／起動失敗 1 を定義して未知フラグは usage を stderr へ出す |
| R6-C-18 | アクセシビリティ基盤の調査項目 | 調査項目 | r2-a11y-01, r2-a11y-03, r2-a11y-04, r2-a11y-05, r2-a11y-10, r2-a11y-11 | main.rs:4412-4459（ラベル API 不使用）, :1532-1634（素の tab 処理なし）, :5624（1100x700 固定）, :5083-5114; DESIGN.md:1099,1104,848 | 未確認 | 中 | M | fcitx5/mozc と CJK フォントの実機検証を先に実施し、Tab 巡回と accessible name の設計を決定する |
| R6-C-19 | 既知の a11y／i18n 対応項目 | 既知 | r2-a11y-02, r2-a11y-06, r2-a11y-07, r2-a11y-08, r2-a11y-12 | main.rs:4421-4427（フォーカス=背景色差、親確認）, :58-72（ThemeMode::Dark 固定）; model.rs:103,460; DESIGN.md:847,859-861,101,103 | 既知(低優先) | 低 | M〜L | フォーカス枠線化と strings.rs 集約を先に実施し、テーマ切替・五十音照合はロードマップどおり後回しにする |
| R6-C-20 | 色覚多様性への配慮［親が却下］ | 却下 | r2-a11y-09 | main.rs:5067-5081 は「エラー: {error}」「読み込み中…」等テキストを併記（R6 再検証）, :5156-5179 も「監視: 一部無効 — {reason}」等を併記; DESIGN.md:848 の「色だけで伝えない」は充足; 親判定 round-2/VALIDATION.md | 却下 | — | — | 採用しない。コントラスト比の定量検証は別途調査項目として扱う（本 finding の主張ではない） |

## 親判定の継承と独立再検証

親が既に判定済みの 4 項目はその verdict を継承した。うち `r2-a11y-09` は却下行（R6-C-20）として残し、
`r2-sec-04`／`r3-input-01`／`r4-life-01,02` は親が本クラスタへ再割当てしたため採用行に計上した。
dedup-index 上はそれぞれ cluster D／E／D にも_cross-list_されるが、親の直接指示を優先している。

独立再検証は上位 8 項目を現行コードで実施した（いずれも読み取りのみ・ソース未変更）。

- `r3-input-01`: main.rs:1528-1533 の early-return と :1613 の到達不能を確認（R6-C-06）。
- `r2-sec-01`: main.rs:509/523/530/538 の env 直読みと cfg 無効化の皆無を確認（R6-C-10）。
- `r2-sec-04`: settings.rs:169-200 が migrate/validate のみで truncate を持たないことを確認（R6-C-11）。
- `r4-life-01,02`: main.rs:5619-5624 の save 欠落と :1585 の Ctrl+Q 迂回を確認（R6-C-15）。
- `r1-code-05` / `r2-sec-08`: ops.rs:375-391 の非 Unix Unsupported と :362-368 の権限 best-effort を確認（R6-C-12）。
- `r5-err-01`: main.rs:1885 の `first_error` 単一保持を確認（R6-C-14）。
- `r2-a11y-09`: main.rs:5067-5081 のテキスト併記を確認し、親の却下を支持（R6-C-20）。

`r4-cfg-02`（既知低優先）・`r5-render-01`・`r4-lic-09` は他クラスタ担当のため本表では扱わない。

## 他クラスタへのクロスリファレンス（主担当ではない ID）

| 主担当 | ID | 本表での扱い |
|---|---|---|
| A | r2-cmp-02, r4-cli-10 | Open With／D-Bus 起動は A で統合。本表では所有しない |
| B | r3-input-03, r1-qual-05, r1-qual-06, r1-qual-09 | フォーカス行退避・監視監査は B 担当 |
| C | r4-fs-sparse, r1-qual-01, r1-qual-11 | スパース展開・O(n) 走査は C 担当 |
| D | r2-sec-12, r4-cfg-02 | umask／sort.dirs_first は D 担当。r2-sec-04 と r4-life-01,02 は親指示で本表が採用 |
| E | r2-cmp-03, r2-scn-09 | 保存検索／名前付きビューは E 担当。r3-input-01 は親指示で本表が採用 |
| F | r1-ux-16, r2-cmp-09, r2-scn-08, r3-state-03 | undo スタック永続化・事前チェック失敗は F が主担当 |
| G | r1-ux-14, r2-sec-03 | ゴミ箱一括消去・ゴミ箱 TOCTOU は G 担当 |
| H | r2-cmp-10, r2-cmp-11, r2-sec-10, r4-cli-04, r5-render-01 | プレビュー／ファイル引数／サイドバー幅は H 担当 |
| I | r4-lic-09 ほか | 推移的ライセンスは I 担当 |
| J | r1-code-01, r4-cli-01, r4-cli-02, r4-life-05 | ドキュメント／CLI 方針／ジオメトリは J 担当。r4-cli-12 はデバッグフックとして R6-C-10 に統合 |
| 吸収済み | r5-err-06 | r3-state-03 と同一根因のため dedup-index 注記どおり取り込まない（R6-C-14 に注記のみ） |

## 兼任クラスタ総評

K-O の生 finding を **20 項目**に統合した。内訳は K=5、L=4、M=5、N=3、O=3 で、
各生 ID は所有項目または上記クロスリファレンスのいずれかに重複なく配置した。
妥当性の内訳は 確定 13／未確認 5／既知(低優先) 1／却下 1 である。

最優先で着手すべき 5 項目は以下のとおり。

1. **R6-C-06** 検索ビューのキーボード操作不能（確定・高・S）— README と矛盾する機能バグで再現テストが先決。
2. **R6-C-07** 選択・フォーカス復帰の不整合（確定・高・S〜M）— DESIGN §3.4/§5.3 との差分が明確。
3. **R6-C-10** デバッグフックがリリースビルドでも作動（確定・高・S）— 攻撃面の除去コストが最も低い。
4. **R6-C-11** 設定配列の読み込み時上限未適用（確定・高・S）— 細工された設定による起動時 OOM を防ぐ。
5. **R6-C-15** 終了処理の集約（確定・高・S）— 将来の終了時状態追加に対する共通基盤となる。

未確認 5 項目（R6-C-05/09/13/16/18）は設計依存の推論または実機挙動に依存するため、
いずれも「まず再現テスト／一次情報の確認」を next-action に置き、確定項目の実装を優先する。
R6-C-20 は親判定を支持する却下行として残し、誤って採用されないようにした。
