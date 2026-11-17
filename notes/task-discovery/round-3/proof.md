---
type: Reference
title: "証跡品質・受入基準トレーサビリティ監査（PASS 3）"
description: >-
  gpui-file-explorer の主張を検証可能性の観点から監査し、テスト件数・CI run・
  スクショ来歴・受入条件・再現手順の不整合を修正タスクとして列挙する。
tags:
  - task-discovery
  - round-3
  - evidence-quality
  - acceptance-criteria
  - reproducibility
generated: "2026-11-17T20:38:29+09:00"
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 監査の範囲と方法

本レンズは機能の有無を扱わず、既存ドキュメントが提示する「証拠」が第三者にも
再現可能かどうかのみを監査する。PASS 1 / PASS 2 の機能発見は重複扱いとし、
ここでは文書間の矛盾・証跡の出所・受入条件の更新状態を対象にした。

検証は読み取り専用の安価なチェックに限定した（ファイル read、grep、
sha256sum / cmp、パス存在確認、`git log`）。`cargo test` の再実行は行っていないため、
実際の pass/fail は文書上の主張として扱い、実再実行結果は未確認である。

### r3-proof-01

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-proof-01 | `cargo test` 件数が 6 通りに分岐し単一の真実源が無い | 構造改善 | `README.md:52,135`（167 passed）、`notes/verify-report.md:17,48,93`（35）、`notes/lead-verify/SUMMARY.md:13,51`（35→36）、`review-report.md:659`（167＝164 passed＋3 failed）、`:716`（168）、`app/PROGRESS.md:78,109`、`packaging/linux/build/.../README.md:289`（132）。Runtime Proof: `grep -rh '#\[test\]' app/src --include='*.rs' \| wc -l` = 174 | 同じ `cargo test` が文書ごとに 35 / 36 / 132 / 167 / 168 / 174 件。特に「167」は review-report では 3 failed を含む値で、README の「167 passed / 0 failed」と同時点で矛盾する。AC-2 の合格判定を遡及できない。修正: `cargo test -- --list` の出力を `notes/test-inventory.md` へコミットし、全ドキュメントをそこにリンク。 | 受入判定の再現性、配布 README の誤記防止 | S | 新規 | 高 |

### r3-proof-02

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-proof-02 | README の CI run 参照が最終承認より 5 run 以上古い | 残タスク | `README.md:5,121`（run 43734742786／2026-11-16）、`review-report.md:66,458,722,746,763,792,798`（43967966780 → 44063705679／2026-11-17T19:27）、`git log` 最新 `d75b3aa`。Runtime Proof: README に run への URL は無く badge は workflow のみ | 「CI の検証範囲（2026-11-16 追記）」が据え置きで、tracing 導入後の最終承認 run 44063705679 が反映されていない。数字だけの run ID はクリックで検証できず、commit SHA との対応も無い。修正: run permalink・commit SHA・完了時刻を一箇所へ集約し、CI 成功時に更新する運用を追加。 | ビルド証跡の鮮度、監査時の追跡コスト削減 | S | 新規 | 高 |

### r3-proof-03

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-proof-03 | `check.sh` の exit 0 は `cargo audit` 実施を証明しない | 構造改善 | `scripts/check.sh:90-96`（`cargo-audit` 未導入ならスキップして継続）、`README.md:129-135`（完全ループ exit 0・「既知 8 件の allowed warnings」）、`review-report.md:651-660,714-716`（記録された実績は `--fast`）。Runtime Proof: `find . -iname '*audit*' -o -iname 'deny.toml'` = 0 件（ホスト側 config の有無は未確認） | README の「完全ループ exit 0 / audit 8 allowed warnings」に対し、実際に記録されたローカル実行は audit を省く `--fast` のみ。audit 不在が黙ってスキップされるため exit 0 を証拠にできない。修正: スキップ時に `SKIPPED` を明示して非 0 にするか `--require-audit` を追加し、allowlist をリポジトリ内 config へ移す。 | 検証ゲートの信頼性、依存脆弱性の見落とし防止 | S | 新規 | 高 |

### r3-proof-04

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-proof-04 | AC-7 の前後スクショと Lead 検証画像の出所が証明できない | 残タスク | `notes/verify-report.md:76-84,208`。Runtime Proof: sha256 が `watcher-before.png` = `window-crop.png` = `window.png` = `041cc9b8…`、`watcher-after.png` = `watcher-refresh-crop.png` = `0cd6f279…`。`notes/lead-verify/*.png` 33 枚中 30 枚が `app/screenshot/` とバイト同一で、`SUMMARY.md` に言及なし（言及は before / after / postfix-after のみ） | AC-7 の「touch 前後」が既存画像のバイト同一コピーで新規撮影ではない（verify-report 自身も指摘済みで未是正）。さらに独立検証ディレクトリの 30 枚に撮影コマンド・時刻・環境変数の記録が無く、独立証跡として引用できない。修正: 画像ごとに sha256・取得コマンド・env・時刻を manifest 化し、重複は削除または「複製」と明記して AC-7 ペアを再撮影。 | 証拠画像の信頼性、AC-7 の真正な再検証 | M | 新規 | 高 |

### r3-proof-05

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-proof-05 | 検証レポート付録の再現コマンドが現在のリポジトリで失敗する | 残タスク | `notes/verify-report.md:7,26,246,251,262-270`（`cd work/gpui-file-explorer/app`、`security-review/Argus/LICENSE`）、`README.md:47-48`（参照残存を自認）。Runtime Proof: repo root から `work/gpui-file-explorer/app` = MISSING、`security-review/Argus/LICENSE` = MISSING | §10 付録の build / test / ライセンス照合コマンドが旧開発ワークスペースのパスを前提とし、現在の checkout ではそのまま実行できない。README は残存を認めるだけで是正していない。修正: `app/`・`notes/` 相対パスへ書き換え、`scripts/verify-artifacts.sh` で参照先存在を機械検査する。 | 新規コントリビュータの再現性、監査コマンドの再利用 | S | 既知(ドキュメント記載あり) | 高 |

### r3-proof-06

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-proof-06 | AC-1〜AC-13 の判定表が 2026-11-11 時点で凍結 | 構造改善 | `DESIGN.md:1005-1017`、`notes/verify-report.md:88-106`（PASS* / NOT TESTABLE）、`README.md:81-82`、`notes/lead-verify/SUMMARY.md:38-40,58`、`review-report.md:709-716` | AC-2 / 4 / 13 は 2026-11-11 のスナップショットのままで、以降の P1 追加・tracing 導入・CI run 変更が反映されていない。AC-6 は静止画証拠未取得、AC-9 は代替条件の実装漏れ、AC-8 / 10 / 12 は NOT TESTABLE のまま。修正: AC ごとに現行テスト ID・画像・run ID・最終確認日を 1 表へ集約し、変更時に再判定を必須化。 | 受入基準のトレーサビリティ、未検証項目の可視化 | M | 新規 | 高 |

### r3-proof-07

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-proof-07 | 「解消済み」表記が同一レポート内で格下げされ、判定も反転している | 構造改善 | `review-report.md:59-63,100,120,132,187-192,527-544,629-632,709-712`。具体例: 前回 H-3 と「構造化ログ解消済み」が `:544` で「枠組みのみ」へ格下げ、判定が「実質承認 → マージブロック → 承認」と反転 | 解消の定義に commit SHA・テスト ID・検証 run のリンクが無いため、同じ事実がバージョン差で「解消」にも「未達」にも読める。修正: 解消済みには commit + テスト ID + 検証日時を必須とし、リンク欠落行を検出する lint を追加。 | レビュー結論の安定性、誤解消の防止 | S | 新規 | 高 |

### r3-proof-08

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-proof-08 | §10.5 の性能目標 8 項目に実測成果物が存在しない | 調査項目 | `DESIGN.md:1021-1032`（計測方法付き 8 目標、うち「必須」4 項目）、`README.md:50-58`（性能値なし）、`notes/fs-cross-platform-notes.md:548`（100k 実測ベンチ未実施）。Runtime Proof: notes / README / review-report を VmRSS・fps・計測ログで grep しても計測結果ファイルなし | 計測方法が書かれているにもかかわらず実測値・スクリプト・日時が無く、「必須」とされた目標の達成／未達を判定できない。PASS 1 の r1-qual-01 は 100k に限局。修正: `notes/perf-baseline.md` に 1k / 10k / 100k の実測と実行コマンドを記録し、未測定項目は DESIGN に明示。 | 性能回帰の検知、設計判断の前提整備 | M | 既知だが未対応 | 高 |

### r3-proof-09

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-proof-09 | P1 UI スクショはデバッグフック注入の状態デモで出所ラベルが無い | 構造改善 | `app/src/main.rs:507-629,5326-5368`（47 個の `GPUI_FE_DEBUG_*`）、`app/README.md:392-404`、`notes/lead-verify/SUMMARY.md:38-40`（入力 E2E 未実施）、`app/screenshot/*.png`、`notes/task-discovery/round-1/quality.md:40`（r1-qual-02 は入力テスト代用の別提案） | グリッド・検索・プレビュー等の画像は env フックで状態を直接注入しており、キー入力経路（AC-8）を通っていない。どの画像がどのフック由来か記録が無く、「実装・テスト済み」の証拠として誤読されうる。PASS 1 はテスト代用案、本件は証跡の出所ラベルが対象。修正: 画像 manifest に hook 名を記録し、README で「状態デモであり入力 E2E ではない」と明示。 | 証跡の誤用防止、AC-8 と P1 証拠の分離 | S | 新規 | 高 |

### r3-proof-10

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-proof-10 | AC-2 根拠の §10.2 テスト一覧が as-built と不一致 | 残タスク | `DESIGN.md:943-956`（`model/sort.rs` / `fs/listing.rs` 等）、`notes/verify-report.md:197`（フラット構成を指摘済み）。Runtime Proof: `ls app/src` = 14 個のフラット `.rs` で `model/`・`fs/` ディレクトリは存在しない | 最低 8 件の根拠となるテスト一覧が存在しないモジュール path を指し、どの実テストが AC-2 を満たすかの対応付けが無い。修正: §10.2 を実ファイル名へ更新し、8 件以上のテスト名を列挙して AC-2 から辿れるようにする。 | 設計書と実装の整合、AC-2 の検証可能性 | S | 既知だが未対応 | 高 |

### r3-proof-11

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r3-proof-11 | 第2回レビューのフォローアップ #6（Windows 失敗 3 件の文書化）が未実施 | 残タスク | `review-report.md:718-719`（README / app README へ追記するよう明記）、`:659-680`（失敗 3 件）、`:722,746,763,792`（修正 commit と CI run）。Runtime Proof: `grep -iE '3 件.*失敗|symlinks_are_counted|copy_directory_recursively_and_keep_symlink' README.md app/README.md` = 0 件 | 修正コミット（0551a0d）と CI run は記録されているが、求められたドキュメント側の痕跡が無い。再発時に「Windows で何が壊れていたか」を辿れない。修正: 3 件の失敗内容・修正 commit・検証 run を既知の制限へ追記し、フォローアップを close する。 | クロスプラットフォーム主張の裏付け、履歴の保存 | S | 既知だが未対応 | 高 |

## レンズ総評

11 件を検出した。いずれも機能追加ではなく「主張をどう証明するか」の問題である。
特に優先すべきは r3-proof-01（テスト件数の単一真実源不在）、
r3-proof-03（exit 0 が audit を証明しない）、
r3-proof-04（証拠画像のバイト同一コピー）、
r3-proof-06（AC 判定表の凍結）、r3-proof-02（CI run の陳腐化）の 5 件で、
これらは README の「検証（Runtime Proof）」という文言の信頼性に直結する。

未確認として扱った項目: ホスト側の cargo-audit allowlist 設定の所在、
`cargo test` の実再実行結果、性能実測値、画像撮影時の実環境変数。
本レンズではソースコードを一切変更していない。
