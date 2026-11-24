---
type: Reference
title: Round 4 — 配布 / パッケージング / CI レビュー
description: 読み取り専用エージェント r4-dist の所見（15 件）。
tags: [task-discovery, cross-review, distribution, gpui-file-explorer]
generated: 2026-11-18T00:12:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 所見

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-dist-01 | `.desktop` に `MimeType=` が無くファイル関連付けが成立しない | 残タスク | `packaging/linux/gpui-file-explorer.desktop:10-16` | Exec=%f で引数は受けるが MimeType 行が無く freedesktop の MIME ハンドラ登録がされない。「既定のアプリ」/Dolphin「開く」に出ない。 | ファイルマネージャ連携 | S | 新規 | 高 |
| r4-dist-02 | 残存 .deb / staged tree の desktop が source と乖離 | 残タスク | `gpui-file-explorer.desktop:10,13` vs staged 版 `:9,12`; `README.md:65` | stage は Exec に %f が無く Categories も FileTools 欠落。README が当該 .deb を案内しており再ビルドせず拾うと古い挙動が配布される。 | 陳腐化成果物の誤配布防止 | S | 新規 | 高 |
| r4-dist-03 | DEB 同梱 README が `132 tests` のまま | 残タスク | staged README:27 vs `app/README.md:27` | stage は 471 行・132 tests、生成元は 502 行・167 tests。build-deb.sh は毎回コピーするが残存 .deb は古い。 | 古い検証結果の配布防止 | S | round1/2/3と重複(r1-verify-03) | 高 |
| r4-dist-04 | 配布ビルドだけ `cargo build --release` に `--locked` が無い | 残タスク | `build-deb.sh:108`; `scripts/check.sh:66-67`; `ci.yml:41` | 検証側は --locked 付きなのに配布側だけ外れておりリリ時にロックが更新されうる。 | 再現性 | S | round1/2/3と重複(r3-upstream-01) | 高 |
| r4-dist-05 | check.sh が audit スキップ時も `All checks passed.` を出力 | 残タスク | `scripts/check.sh:90-99`; `ci.yml:35` | cargo-audit 未インストール時に advisories を飛ばしつつ成功文言。CI は audit: true で常に実行するためローカル成功は CI と等価でない。 | ゲートの信頼性 | S | round1/2/3と重複(r3-upstream-15) | 高 |
| r4-dist-06 | CI に DEB のビルド・検証ゲートが存在しない | 残タスク | `ci.yml:21-43`; `README.md:60-77` | dpkg-deb --contents / desktop-file-validate / lintian 等が無い。唯一の配布物が DEB なのに CI green は配布可能性を何も保証しない。 | 配布物の quality gate | M | 新規 | 高 |
| r4-dist-07 | 更新・アンインストール・バージョン追跡・CHANGELOG が存在しない | 機能追加案 | `README.md:77,87`; `Cargo.toml:4` | 署名・リポジトリ公開・タグ付け未実施で更新導線・削除手順・リリースノートが無い。 | 継続配信 | M | 新規 | 高 |
| r4-dist-08 | 設定スキーマに v1→v2 移行経路とダウングレード検出が無い | 残タスク | `settings.rs:209-224`; `app/README.md:363-364` | migrate() は v0/v1 で何もせず未知 version は read_only にするだけ。 | バージョン間移行 | M | 既知(ドキュメント記載あり) | 高 |
| r4-dist-09 | 保存タブが CLI 引数を黙って上書きし .desktop %f が死ぬ | 残タスク | `main.rs:5683-5688`; `README.md:27,69` | restore_session は保存タブがあれば cli_dir を見ず early return。root/app README 同士も矛盾。 | CLI 契約 | M | round1/2/3と重複(r3-config-02) | 高 |
| r4-dist-10 | DEB control の Maintainer に個人 Gmail が混入 | 調査項目 | `build-deb.sh:178,182`; staged control:6,10 | `Maintainer` に個人メールが固定埋め込みされていたが、汎用値へ置換済み。公開方針は要確認。 | プライバシー | S | 新規 | 中 |
| r4-dist-11 | `/home/testuser/...` とユーザー名が source/証跡に残存 | 残タスク | `settings.rs:9,14,15,335,348`; `model.rs:699`; `DESIGN.md:119`; `notes/poc-report.md:216` | 個人パスがドキュメント例・fixture・スクショ説明に多数残る。 | 個人情報露出の低減 | S | round1/2/3と重複(r1-sec-15) | 高 |
| r4-dist-12 | `ldd`+`dpkg -S` 生成の Depends がビルドホスト依存で脆い | 調査項目 | `build-deb.sh:140-165,168-169` | dlopen 分は libvulkan1 のみ手追加。Wayland/X11/ポータルの実効依存が漏れる/過剰に入り、Depends がビルドマシンで変わりうる。 | 他環境での install 成功率 | M | 新規 | 中 |
| r4-dist-13 | copyright が AGPL 全文のコピーで DEP-5 形式でない | 残タスク | `build-deb.sh:136`; staged copyright:1-661 | Format/Files/Copyright/License スタンザが無く Debian Policy 違反・lintian 警告対象。 | Debian 規約適合 | S | 新規 | 中 |
| r4-dist-14 | README が DEB のバージョン・アーキをハードコード | 構造改善 | `README.md:65`; `app/README.md:348`; `build-deb.sh:69-74` | スクリプトは動的計算するのに README は固定記載で次バージョンで陳腐化。 | ドキュメント自己整合 | S | 新規 | 高 |
| r4-dist-15 | 全ゲートが外部再利用ワークフローに委譲され成功条件が検証不能 | 調査項目 | `ci.yml:2-4,24`; `README.md:116-119` | 呼び出し側にゲート内容のコメントのみで v1 の実体は repo 内に無い。可変タグの付け替えでゲートが変わっても検出不可。 | CI 成功の透明性 | S | 新規 | 中 |

## レンズ要約

ビルドスクリプトのガードや CI の permissions/concurrency は方針どおりだが、「作った .deb が正しいか」を検証するゲートが皆無で、残存成果物の desktop/README が source と乖離している。`.desktop` の MimeType 欠落と保存タブによる CLI 引数上書きでファイル関連付けは実質機能せず、更新・アンインストール・設定移行の運用も空白。Depends・copyright・外部ワークフローは外部仕様依存として要確認に切り分けた。
