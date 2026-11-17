---
type: Reference
title: "PASS 6 高影響主張の独立妥当性再チェック"
description: "PASS 1〜5 で確定とされた高影響 finding 10 件を、現行コード・ローカル依存ソース・実行結果で敵対的に再検証した結果"
tags:
  - task-discovery
  - validity-recheck
  - pass-6
  - runtime-proof
generated: 2026-11-17T21:46:27+09:00
status: complete
---

## 判定表

| finding | 主張 | verdict | evidence | 修正点/却下理由 | 最終 priority |
| --- | --- | --- | --- | --- | --- |
| `r5-trash-10` | macOS で `trash::os_limited` が gated out されビルド不能 | 維持 | `trash-5.3.0/src/lib.rs:426-429` の cfg は `windows` または `all(unix, not(macos), not(ios), not(android), feature="freedesktop")` で macOS を明示除外。`trash_view.rs:19,26,49` が `os_limited::list/restore_all` を無条件呼び出し。app 側に macOS 向け cfg は皆無（`mod trash_view;` は `main.rs:20` で無条件） | 事実。README:84-86 の「上流 gpui-pre-apple のクロスビルド制約」だけでは理由が不足で、アプリ側の直接原因を追記すべき。Linux 実行時の不具合ではなくクロスプラットフォーム宣言の是正案件 | 高 |
| `r3-input-01` | Search ビューで Enter が選択結果を開かない | 修正 | `main.rs:1528-1533` が Search 中は全キーを early return し、`:1606-1615` の `enter => open_selected` に到達不能。`:3616-3618` の Enter は `run_search` へ分配。一方 `:4366-4370` のダブルクリックと `:4108-4115` の右クリック「開く」は `open_index` / `open_path` を呼ぶ | 「Enter 到達不能」自体は確認。ただし検索結果を開く別経路（マウス）が実在するため、最終報告で「検索結果を開く手段がない」と書いてはならない。キーボード操作のみが死ぬという scope に限定し、誇張表現を避ける | 中 |
| `r4-life-01/02` | ウィンドウ終了時に `save()` が無く Ctrl+Q がフックを迂回 | 維持 | `main.rs:5619-5624` の `on_window_closed` は `cx.windows().is_empty()` を見て `cx.quit()` するのみで save なし。`:1585` の `("q", _) => cx.quit()` はフックを経由しない。save は `:638,908,1103,1162,1172,1266,1434,1643,1657,3768` に分散。`DESIGN.md:840` は保存タイミングに「ウィンドウ終了時」を明記 | 両方ともコード上事実。ただし全永続フィールドがミューテーション時に同期保存されるため即時のデータ損失は再現せず、finding 自身もその旨を明記。設計との整合・将来の終了時状態向けの構造改善であり、現時点の functional bug ではない | 中 |
| `r4-cfg-03` | 検索履歴は 10 件保存されるが 8 件しか表示されない | 維持 | `settings.rs:39` で `MAX_RECENT_SEARCHES = 10`、`:146` で `truncate(10)`。対して `main.rs:3992` は `.iter().take(8)`。`app/README.md:198-199` は「最大 10 件まで設定に保存」と明記 | 数値の不一致は事実。隠れるのは 9/10 件目のみでデータ損失ではない。保存側か表示側のどちらかに揃える低コストの表記・UI 整合問題 | 低 |
| `r4-lic-09` | DESIGN 想定外の推移的ライセンス expression が存在 | 維持 | `cd app && cargo metadata --offline --format-version 1 \| jq -r '.packages[].license' \| sort \| uniq -c` の実出力（全 815 package）で `MPL-2.0` ×3（`cbindgen 0.28.0` / `dwrote 0.11.5` / `option-ext 0.2.0`）、`0BSD` ×2、`Apache-2.0 WITH LLVM-exception ...` ×5、`Apache-2.0 OR GPL-2.0-only` ×1（`self_cell 1.3.0`）、`(MIT OR Apache-2.0) AND NCSA` ×1 等を確認。`DESIGN.md:925` の allow 一覧に MPL-2.0 は無く、`app/NOTICE.md:55-58` の要約とも不一致 | メタデータ上の事実。ただし違法性の断定ではなく「調査項目」のまま扱う。`cbindgen` はビルド時、`dwrote` は Windows 向けで配布バイナリへの影響は未確認。`deny.toml` / `audit.toml` はリポジトリに存在せず自動化は未整備 | 中 |
| `r5-render-01` | サイドバー幅に 214 / 190 / 200 の三重ドリフト | 維持 | `main.rs:83` で `SIDEBAR_WIDTH_PX = 214.0`、`:4669` の実描画は `.w(px(190.0))`、`DESIGN.md:134` は「サイドバー 200px 固定（P0）」、`:5428` のグリッド列数計算は `SIDEBAR_WIDTH_PX`（214）を控除 | 数値ドリフトは事実。予約 214 が実描画 190 を上回るため重なり・クリッピングは起きず、グリッド列が 1 列減る程度の描画効率問題。functional bug ではなく低優先のレイアウト整合 | 低 |
| `r3-state-03`（= `r5-err-06`） | undo の事前チェック失敗で履歴項目が消える | 維持 | `main.rs:2298` で `pop_undo()` した後、Rename 分岐の `:2313-2323` は `!current.exists()` / `target.exists()` で早期 return するが `push_undo` が無い。対して非同期失敗時の `:2348` は `this.undo.push_undo(action_for_redo)` で戻している | 事実で、10 件中でも数少ない実 functional bug（undo 項目が黙って失われ再試行不可）。`r5-err-06` と同一経路の重複であり、PASS 6 は `r3-state-03` に 1 回のみ計上する | 中 |
| `r2-sec-04` | 設定 JSON の配列がロード時に上限適用されない | 維持 | `settings.rs:175-201` の `load_from_with_backup` は `from_str` 後に `migrate` + `validate` のみ。`:227-233` の `validate` は `last_dir` の存在確認しかしない。上限は `:113,132-134,146` のミューテータ側だけで、`tabs` には MAX 定数が無い。`main.rs:5581-5585` は `active` を clamp するが要素数は制限せず、`:638` の起動時 save でそのまま書き戻される | コード上事実。ただし脅威モデルはユーザー設定ディレクトリの細工（特権昇格なし・ローカルでの自傷 DoS）。リモート攻撃面として過大評価せず hardening 案件として記録 | 低中 |
| `r3-proof-01/03` | テスト件数が文書ごとに分岐し、`check.sh` の exit 0 が audit 実施を証明しない | 維持 | 件数は `README.md:52` = 167 passed/0 failed、`review-report.md:602,665` = 「167 件（164 passed / 3 failed）」、`notes/verify-report.md:17,48` = 35、`notes/lead-verify/SUMMARY.md:13,51` = 35→36、`app/PROGRESS.md:13` = 132、`:109` = 167 と 6 通りに分岐。`grep -rh '#\[test\]' app/src --include='*.rs' \| wc -l` = 174。`scripts/check.sh:90-97` は `cargo-audit` 未導入時にスキップメッセージを出すだけで、`:99` の `All checks passed.` に到達し exit 0。`test-inventory.md` / `deny.toml` は `find` で 0 件 | 両方とも事実。ただし runtime の functional bug ではなく、受入判定の再現性と検証ゲートの信頼性に関する証跡問題 | 中 |
| `r2-up-01/09` | gpui-kit 0.10.0 がローカル index に存在し、`build.rs` の OS 判定がホスト評価 | 維持 | ローカル index キャッシュ `.cargo/registry/index/.../.cache/gp/ui/gpui-kit` に exact token `0.10.0` と `"vers":"0.10.0"` の JSON が存在。`app/Cargo.toml:14` と `poc/Cargo.toml:8` は `=0.9.1` で固定。`app/build.rs:26` の `cfg!(target_os = "linux")` はビルドスクリプト自身のコンパイル時（ホスト）に評価され target OS を表さない（正しくは `CARGO_CFG_TARGET_OS`）。`:31,:50` は `CARGO_MANIFEST_DIR` 由来の絶対パスを `-L native=` に埋め込む | 両方とも事実。ただし 0.10.0 の修正内容は未確認で、0.x のマイナー更新には破壊的変更があり得る。「更新すべき」と断定せず、アップグレード調査とクロスビルドの構造改善として扱う。現行 Linux ビルドの functional bug ではない | 中 |

## 検証詳細

### 用いたコマンドと一次根拠

- trash クレートの cfg と、`app/` 全体に macOS 向け cfg が存在しないことは `grep -n 'cfg\(.*macos' app/src` が 0 件であることでも裏取りした。
- ライセンス集計は `cargo metadata --offline --format-version 1` を `app/` で実行し、jq で `.packages[].license` を抽出。`MPL-2.0` の内訳は同コマンドに `select(.license=="MPL-2.0")` を付けて確認している。
- テスト件数の 174 は `#[test]` 属性の静的カウントであり、`cargo test -- --list` の実測ではない。整合性の指標としてのみ使う。
- `build.rs` のホスト評価は Rust のビルドスクリプトがホスト向けにコンパイルされるという仕様に基づく。GPUI 側の的动作確認ではなく静的推論である点を明記する。

### Cross-pass の重複ペア

- **`r3-state-03` = `r5-err-06`（主張 7）**: 同一の undo 事前チェック失敗経路。`dedup-index.md:91` も既に重複と判定済み。PASS 6 では `r3-state-03` を主担当とし、`r5-err-06` は吸収して 1 件に計上する。
- **`r3-input-01`（主張 2）**: `dedup-index.md:44` のクラスタ E（検索）と `:71` のクラスタ L（入力・操作モデル）の**両方**に同一 ID が掲載されている。主担当は L とし、E では言及のみに留めて二重計上を避ける。
- **`r4-life-01` と `r4-life-02`（主張 3）**: 別 ID だが「shutdown 処理を一箇所へ集約する」という同一テーマ。厳密な重複ではないが、PASS 6 では 1 つの統合項目にまとめるのが妥当。
- 重複ではない組: `r3-proof-01`（テスト件数の単一真実源不在）と `r3-proof-03`（audit スキップ）は証跡でも別問題。`r2-up-01`（依存 update）と `r2-up-09`（build script の host/target 混同）も別問題。

## 総評

10 件すべてについて前パスの判定を維持するか、scope を狭める修正にとどめ、**却下は 0 件**だった。ただし「高影響・functional bug」と受け取られやすい表現の多くは、実際には構造改善・ドキュメント整合・証跡の信頼性・レイアウトの軽微なドリフトであり、深刻度を下げて最終報告に載せるべきである。

- 文字どおり functional bug と言い切れるのは **`r5-trash-10`**（macOS ではコンパイルが通らない）と **`r3-state-03`**（undo 項目が黙って失われる）の 2 件のみ。
- **`r3-input-01` は verdict を `修正` に変更した**。Enter が届かないのは事実だが、ダブルクリックと右クリック「開く」で結果は開ける。最終報告に「検索結果を開けない」という記述が残っているなら必ず書き換えること。
- `r4-life-01/02`、`r4-lic-09`、`r2-sec-04`、`r3-proof-01/03`、`r2-up-01/09` は、いずれも現行の Linux 動作を壊すものではなく、設計・運用・将来の変更に対する脆弱性である。priority は「中〜低」で統一し、法的違反やセキュリティ侵入の断定を伴わせない。
- `r4-cfg-03` と `r5-render-01` は effort S の軽微な不整合で、最終報告では詳細セクションではなく付録的な扱いで十分である。
- 重複は上記 3 組（うち 1 組は明示的な同一 finding、1 組は複数クラスタへの二重登録、1 組は同一テーマの別 ID）。`r3-state-03` と `r3-input-01` の二重計上を特に防ぐこと。
