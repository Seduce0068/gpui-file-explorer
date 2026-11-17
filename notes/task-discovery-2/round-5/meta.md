---
type: Reference
title: Round 5 — メタ / カバレッジギャップ / 反証 レビュー
description: 読み取り専用エージェント r5-meta の所見（15 件）。第6回の重複・妥当性判定の直接入力。
tags: [task-discovery, cross-review, meta, verification, gpui-file-explorer]
generated: 2026-11-18T00:24:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r5-meta-01 | 全ラウンドの行番号が現行 HEAD と系统的に乖離 | 調査項目 | `notes/task-discovery/round-4/cli.md`(main.rs:5523) / `round-3/SUMMARY.md:37`(restore_session :5668) / 現行 `main.rs:5625,5662,5676,5721` | ファイル末尾付近で +100〜126 行、先頭付近は +1行。round-6 が注記の行番号をそのまま使うと別関数を指す。現行行への再解決が必要。 | 出典精度 | S | 新規 | 高 |
| r5-meta-02 | r5-render-05 は「部分修正済み」で記述が陳腐化 | 残タスク | `main.rs:4444-4455`, `:4640-4651`; `icons.rs:90`, `:32` | ヘルパ統合は入ったが kind_for_name の二重呼び出しと小文字化 String の二重確保は残存。「未対応」ではなく「部分的対応・コア未解決」と再分類すべき。 | 二重分類の解消 | S | 既知だが未対応 | 高 |
| r5-meta-03 | 「名前フィルタが完全デッドコード」は誇張（マウス到達可） | 調査項目 | `main.rs:4037-4044`（検索ツールバーに「名前:」Input を描画）, `:1583-1588`, `:1598-1604`; `round-2/SUMMARY.md:38` | early return で Ctrl+Shift+F が届かないのは事実だが入力欄は描画されている。正しくは「ショートカット不到達＋Tab トラバース無しでキーボードのみ不可」。 | 過大主張の是正 | S | 前回と重複(r2-input-01) | 高 |
| r5-meta-04 | 「GPUI_FE_DEBUG_* 47件」は数え方が未定義 | 調査項目 | `main.rs:511,525,532,540,594-608,610-634,5430-5459` | 実体は重複を除き **29種類**。文字列出現行数は doc 込みで約49。最終報告は 29 種類へ訂正すべき。 | 指標の定義と再計測 | S | 前回と重複(r1-arch-08) | 中 |
| r5-meta-05 | ALLOWED_FS は「20種」ではなく実数え 21種 | 残タスク | `mounts.rs:28-50`（29〜49行目が21エントリ）; `notes/task-discovery/dedup-index.md:88` | レビュー側の「修正値」自体が off-by-one。確定扱いになっており round 6 が採用すると誤数値が残留する。 | 確定事項の訂正 | S | 新規 | 高 |
| r5-meta-06 | NOTICE タイポは修正済みなのに旧レポートが未対応のまま | 残タスク | `app/NOTICE.md:29`（現在は AllAssets と正しい）; `notes/lead-verify/SUMMARY.md:35`; `notes/verify-report.md:119,211,238` | 「一方は直した、他方は直っていない」というラウンド間矛盾。verify-report の当該行を「解消済み」へ吸収し再修正タスクとして残さない。 | 重複作業の排除 | S | 新規 | 高 |
| r5-meta-07 | DEB 同梱ドキュメントの相対リンクがインストール後に切れる | 残タスク | `build-deb.sh:134-138`; `app/README.md:10,499` と `NOTICE.md:6,9` | ライセンス全文は copyright というファイル名で入るため LICENSE リンクが切れ、app/NOTICE.md も NOTICE-app.md へリネされ切れる。 | 配布物の案内精度 | S | 新規 | 高 |
| r5-meta-08 | icons.rs の「全種別アイコンをカバー」テストは assert が無い | 構造改善 | `icons.rs:153-172`（let _ = format!(...) のみ）; 対比 `:117-150` | パニック以外では決して失敗しない空振りテスト。Pdf/Text が同じ IconName に潰れても検出不能。 | 回帰網の実質化 | S | 新規 | 高 |
| r5-meta-09 | 非 Linux 向けテストが Linux CI では空振りする | 構造改善 | `mounts.rs:210-215`; `preview.rs:302-307`, `:388-395`; `ci.yml` は ubuntu-latest のみ | list_mounts の non-linux 実装も Windows symlink 拒否経路も Linux CI で1回も実行されない。 | 未検証分岐の可視化 | S | 新規 | 高 |
| r5-meta-10 | Windows では symlink コピーが全面 Unsupported なのに README は無条件に約束 | 残タスク | `ops.rs:376-391`; `app/README.md:162`, `:466` | std::os::windows::fs::symlink_file が存在するのに non-unix 一括拒否。README は「未検証」留めで実挙動と矛盾。 | クロスプラットフォーム整合 | M | 新規 | 中 |
| r5-meta-11 | trash の cfg は macOS 以外も除外（検証は維持、範囲は過小） | 調査項目 | `trash-5.3.0/src/lib.rs:426-429`; `trash_view.rs:19,26,49`; `Cargo.toml:18` | os_limited は windows または all(unix, not(macos), not(ios), not(android), feature="freedesktop")。macOS に加え iOS/Android も除外で freedesktop feature 依存。既定 feature 変更で Linux も壊れ得る。「macOS のみ」と書かず範囲を正す。 | 影響範囲の正確化 | S | 前回と重複(r5-trash-10) | 高 |
| r5-meta-12 | F1 ヘルプとキーハンドラの対応を検査するテストが存在しない | 構造改善 | `strings.rs:139-154`; `main.rs:1583-1604` | Ctrl+Shift+F がヘルプに載っているのに Search view で到達不能、という不整合が3ラウンド続けて検出された根因。HELP_SECTIONS と dispatch を結ぶ検証が無い。 | 同類不整合の予防 | S | 新規 | 中 |
| r5-meta-13 | ConflictPolicy の expect(dead_code) は lint 期待が外れる脆い書き方 | 調査項目 | `ops.rs:53`, `:233-235`, `:741-975` | non-test で Fail/Skip が match されるだけ。main.rs で Fail を使うと期待が外れ unfulfilled_lint_expectations が -D warnings でビルドを落とす可能性。 | CI 突然死の回避 | S | 新規 | 低 |
| r5-meta-14 | build-deb.sh のバージョン抽出が最初の version 行に依存 | 構造改善 | `build-deb.sh:69-70`, `:114-120`; `Cargo.toml:2-11` | sed|head -n1 は Cargo.toml 書式変更に弱い。空文字になると stale バイナリ検知まで壊れる。 | 配布スクリプトの堅牢化 | S | 新規 | 中 |
| r5-meta-15 | 並列ラウンド間で同じテーマが別 ID として多数計上 | 構造改善 | `notes/task-discovery/dedup-index.md:46-75`; 各 round SUMMARY; `round-6/cluster-b.md` | 最大クラスタ: ①undo 消失（r1-correct-01/02/03=r1-sec-10=r2-state-01..04=r2-async-08=r3-state-01..04=r5-err-06=R6-B-02）、②検証ゲート（r1-verify-02=r3-proof-03/07=r3-upstream-15=r4-dist-05=r4-proof-07=R6-B-03）、③build.rs ホスト評価（r2-up-09=R6-B-17）、④名前フィルタ到達性（r2-input-01/02=r2-scenario-02/03=r3-search-06）。 | 集計の圧縮 | S | 新規 | 高 |

## レンズ要約（第6回への直接入力）

未開拓領域で見落としを発見: テストの assert 非空性（08/09）、DEB 内相対リンク解決（07）、cfg(not(unix)) 本文（10）、レビュー記録自身の数値・修正状態（04/05/06）。第6回が先に解消すべき重複クラスタは undo 消失・検証ゲート・build.rs ホスト評価・名前フィルタ到達性の4組。反証: 「名前フィルタ完全デッドコード」は却下、「47環境変数」は29種へ、「ALLOWED_FS 20種」は21種へ、「NOTICE タイポ未対応」は修正済みへ、r5-render-05 は部分対応済みへ上書きすべき。
