---
type: Reference
title: PASS 4 LICENSE/COMPLIANCE レンズ発見事項（gpui-file-explorer）
description: SPDX ヘッダー網羅、NOTICE と Cargo.lock の照合、AGPL ソース提供案内、第三者アセット帰属、Zed GPL 禁止の検証など、ライセンス運用およびコンプライアンス手続きの発見事項を証拠付きで列挙する。
tags:
  - license
  - compliance
  - agpl
  - spdx
  - notice
  - pass4
  - discovery
generated: 2026-11-17T21:12:34+09:00
status: complete
---

## 発見事項

### r4-lic-01 SPDX ヘッダーは全対象ファイルに付与済みだが自動ゲート化は未実施

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-lic-01 | SPDX ヘッダーは全対象ファイルに付与済みだが自動ゲート化は未実施 | 残タスク | `grep -rL 'SPDX-License-Identifier' app/src/*.rs app/build.rs poc/src/*.rs poc/build.rs scripts/*` は出力空（EXIT:0）。個別確認で 19 ファイルすべて `HAS`（`app/src/` 14 点、`app/build.rs`、`poc/src/main.rs`、`poc/build.rs`、`scripts/check.sh`、`scripts/dev-setup.sh`）。`scripts/check.sh` のライセンス関連行は 2 行目の SPDX コメントのみ。`DESIGN.md:881`, `DESIGN.md:885` が必須要件を規定 | `DESIGN.md` が要求する先頭 1 行目の SPDX ヘッダーは現状すべて満たしている。ただし網羅性の担保が手動 grep に依存しており、`scripts/check.sh` に検証ステップが無いため、新規ファイル追加時に要件が静かに欠落しうる。CI 等へ再現可能なチェックを組み込むかどうかは運用判断として検討余地がある（法的結論ではない） | 中 | 小 | 中 | 高 |

### r4-lic-02 NOTICE 類に著作権者・著作権年が明記されていない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-lic-02 | NOTICE 類に著作権者・著作権年が明記されていない | 調査項目 | `grep -nE 'Copyright|©|authors[[:space:]]*=' NOTICE.md app/NOTICE.md app/Cargo.toml README.md` はすべて一致なし（EXIT:0）。`NOTICE.md:5-11` はライセンスとソース場所のみ。`app/Cargo.toml:2-11` に `authors` フィールド無し。`LICENSE:4` は FSF の著作権表示（ライセンス正文）。`DESIGN.md:879` は「本アプリの著作権表示 + 第三者ライセンス一覧」を必須ファイルとして規定 | 第三者ライセンス一覧は充実している一方、本アプリ自身の著作権者・年号の記載が確認できない。`DESIGN.md:879` の「本アプリの著作権表示」との対応関係をどう取るか（誰をholderとするか、どの範囲の文書に載せるか）は要確認事項であり、ここでは要否・適否を断定しない。表中の「確認できなかった」はローカルファイルの検索結果に基づく | 中 | 小 | 中 | 高 |

### r4-lic-03 NOTICE の直接依存バージョンは Cargo.lock と整合（抽象表記のみ残存）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-lic-03 | NOTICE の直接依存バージョンは Cargo.lock と整合（抽象表記のみ残存） | 調査項目 | `app/NOTICE.md:38-51` と `app/Cargo.lock` の抽出値（`awk` による照合）を比較: `alphanumeric-sort 1.5.8`、`notify 8.2.2`、`notify-debouncer-full 0.7.0`、`trash 5.3.0`、`gpui-kit/gpui-component/gpui-kit-assets 0.9.1`、`gpui-pre/gpui-pre-platform 0.3.12`、`async-channel 2.5.0`、`dirs 6.0.0` は完全一致。`serde 1.0.229`、`serde_json 1.0.154`、`chrono 0.4.45`、`tracing 0.1.44`、`tracing-subscriber 0.3.23` は NOTICE 側が `1.0.x` / `0.4.x` / `0.3.x` の抽象表記（`app/NOTICE.md:45-51`） | 具体的数値が書かれた行については NOTICES とロックファイルの不一致は検出されなかった。誤った不一致を主張しないため、ここでは「照合済みで整合」と記録する。残る論点は `1.0.x` 等の抽象表記を配布時の通知としてどう扱うか（記載方針の問題）で、ライセンス上の要求として何が十分かは結論付けない | 中 | 小 | 中 | 高 |

### r4-lic-04 dirs の版数記述に設計書・実装・通知の差分がある

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-lic-04 | dirs の版数記述に設計書・実装・通知の差分がある | 調査項目 | `DESIGN.md:923` は「dirs 7（試作は 6 可） MIT OR Apache-2.0（⚠ 未確認）」。`app/Cargo.toml:21` は `dirs = "6"`。`app/NOTICE.md:47` は `dirs 6.0.0`。`app/Cargo.lock` の抽出では `dirs 5.0.1` と `dirs 6.0.0` が併存。ライセンス expression の「未確認」は DESIGN 側のまま | 設計書の想定（7 系、試作は 6 可）と実装・通知（6.0.0）に記述差がある。加えてロックファイルには推移的な 5.0.1 も存在し、`app/NOTICE.md:53-58` の「推移的依存は一覧を cargo metadata で」という方針と合わせて、どの範囲を通知対象とみなすかの整理が必要。依存の更新是非ではなく、記述の整合性の問題として扱う | 中 | 小 | 中 | 高 |

### r4-lic-05 README のソース提供案内にタグ・ハッシュの明記がない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-lic-05 | README のソース提供案内にタグ・ハッシュの明記がない | 調査項目 | `README.md:108-112` は「本リポジトリがそのソースです」とだけ記載し、リポジトリ URL は `README.md:5` の CI バッジと `README.md:18` の `git clone` に分散。`NOTICE.md:20` に `https://github.com/Seduce0068/gpui-file-explorer` あり。`DESIGN.md:890-892` は同一リリースページでのソース tarball + タグ + SHA-256、README の「ソースコード」節への URL とタグ明記を要求。`README.md:77` はタグ付け未実施を記載 | 現状は未リリースであり `DESIGN.md` の配布時要件を満たせていないのは当然の状態だが、README に専用の「ソースコード」節が無く、URL が複数箇所に分散している。配布時にどの粒度の案内（URL のみか、タグ/SHA-256 までか）を README と NOTICE のどちらに集約するかは要確認。ここでは要件充足/不充足の法的評価は行わない | 中 | 中 | 中 | 高 |

### r4-lic-06 DEB への必須通知同梱は確認済み（copyright はライセンス全文のみ）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-lic-06 | DEB への必須通知同梱は確認済み（copyright はライセンス全文のみ） | 構造改善 | `packaging/linux/build-deb.sh:134-138` が `doc_dir=/usr/share/doc/${pkg}` に `app/README.md` → README.md、`LICENSE` → copyright、`NOTICE.md` → NOTICE.md、`app/NOTICE.md` → NOTICE-app.md を `install -Dm644`。冒頭コメント `build-deb.sh:10` と実際の install 行が一致。`LICENSE:1-8` は AGPL 正文でアプリ固有の著作権者行は無し（r4-lic-02 と連動） | `DESIGN.md:893` の「ライセンス全文と NOTICE をバイナリ配布物に同梱（`/usr/share/doc/<pkg>/copyright`）」は実装上充足している。改善余地は copyright が AGPL 全文の単純コピーで、アプリの著作権者・ライセンス要約（DEP-5 相当のヘッダ）を備えていない点。要約の追記要否は r4-lic-02 の確認結果に依存するため、ここでは結論を保留する | 中 | 小 | 中 | 高 |

### r4-lic-07 desktop ファイルの Comment にソース入手案内がない

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-lic-07 | desktop ファイルの Comment にソース入手案内がない | 残タスク | `packaging/linux/gpui-file-explorer.desktop:8-9` は `Comment=A fast cross-platform file explorer built with GPUI` / `Comment[ja]=...` のみ。`build-deb.sh:182` の control には `Homepage: https://github.com/Seduce0068/gpui-file-explorer` あり。`grep -nE 'Comment=|Homepage=|source|Source|AGPL|github' packaging/linux/build-deb.sh` にソース入手案内の記述なし。`DESIGN.md:891` は control・desktop の Comment にも短い案内を求める（P2 でも可） | control の Homepage からリポジトリには到達できるが、`DESIGN.md:891` が想定した「ソースはどこで入手できるか」の短い案内は desktop Comment に無い。P2 可とされているため優先度は低いが、配布物側の案内として追加するかは設計上の選択。追加文言の法的十分性はここでは判断しない | 低 | 小 | 低 | 高 |

### r4-lic-08 Lucide の ISC ライセンス本文が NOTICE に無い（帰属のみ）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-lic-08 | Lucide の ISC ライセンス本文が NOTICE に無い（帰属のみ） | 調査項目 | `app/NOTICE.md:27-32` は Lucide を ISC として帰属表示し URL を記載、`gpui-kit-assets` を Apache-2.0 として記載。`app/NOTICE.md:34` は OS ネイティブアイコンを同梱しない旨。`DESIGN.md:917` は Lucide を ISC・帰属表示、`DESIGN.md:916` は gpui-kit-assets を Apache-2.0・表示 + ライセンス文、`DESIGN.md:918` は notify を CC0-1.0・任意と分類。`NOTICE.md:12-15` も同樣の要約 | 第三者アセットの帰属（Lucide ISC / gpui 系 Apache-2.0 / notify CC0-1.0）は NOTICE 群に記載済みで、種別の誤記は見当たらない。`DESIGN.md` 上は ISC に「帰属表示」のみ求めているが、配布時にライセンス本文の同梱まで必要かどうかはここで断定せず調査項目とする。DEB の同梱先は r4-lic-06 の通り | 中 | 小 | 中 | 高 |

### r4-lic-09 推移的依存のライセンス摘要に DESIGN の想定外の expression が含まれる

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-lic-09 | 推移的依存のライセンス摘要に DESIGN の想定外の expression が含まれる | 調査項目 | `app/NOTICE.md:55-58` は推移的依存を「MIT / Apache-2.0 / BSD / ISC / CC0 / Zlib」と要約。`cargo metadata --format-version 1 --offline --manifest-path app/Cargo.toml` + jq の集計では `MPL-2.0`（`cbindgen 0.28.0`、`dwrote 0.11.5`、`option-ext 0.2.0`）、`Unicode-3.0`（22 件）、`0BSD`、`Apache-2.0 WITH LLVM-exception`、`Apache-2.0 OR BSL-1.0`、`(MIT OR Apache-2.0) AND NCSA`、`MIT OR Apache-2.0 OR LGPL-2.1-or-later`（`r-efi`）、`Apache-2.0 OR GPL-2.0-only`（`self_cell 1.3.0`）等が出現。`DESIGN.md:925` の allow 一覧は AGPL-3.0-or-later / Apache-2.0 / MIT / ISC / CC0-1.0 / Unlicense / BSD-3-Clause / Unicode-3.0 で MPL-2.0 は含まれない。`deny.toml` / `audit.toml` は `find` の結果存在せず（`DESIGN.md:925` は自動化を P1 と記載） | NOTICE の要約は「主なもの」としては読めるが、実際の expression 集合とは一致しない（特に MPL-2.0 は DESIGN の allow 一覧外）。`cbindgen` はビルド時依存、`dwrote` は Windows 向けなどターゲット限定の可能性があり、配布バイナリへの影響は未確認。ここでは「拒否すべき」等の法的判断を行わず、allow/deny 方針と NOTICE 要約の整合を検討する調査項目として記録する。`cargo metadata --offline` はローカルのロックファイルに基づく実行結果である | 高 | 中 | 高 | 高 |

### r4-lic-10 Zed GPL コード禁止はロックファイル・メタデータ上で確認済み

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-lic-10 | Zed GPL コード禁止はロックファイル・メタデータ上で確認済み | 構造改善 | `grep -nE 'zed|git =|crates/ui|project_panel|GPL' app/Cargo.toml app/Cargo.lock` のうち `source = "git` は `app/Cargo.lock` に 0 件。`zed-font-kit 0.14.1-zed`、`zed-scap 0.0.8-zed`、`zed-xim 0.4.0-zed` はいずれも `source = "registry+https://github.com/rust-lang/crates.io-index"`（`app/Cargo.lock:8367-8417`）。metadata での license は `MIT OR Apache-2.0` / `MIT` / `MIT`。GPL 系 expression は `self_cell 1.3.0` の `Apache-2.0 OR GPL-2.0-only` と `r-efi` の LGPL 選択肢のみで、GPL-only の単独ライセンス-package は無し。`DESIGN.md:905-909`, `app/NOTICE.md:17-23` が禁止方針と gpui-pre の Apache-2.0 扱いを規定 | `DESIGN.md:871`, `DESIGN.md:905-909` が禁じた Zed の GPL コード（`crates/ui` / `crates/project_panel` / `crates/theme`）の取り込みは、git 依存ゼロ・zed-* が crates.io の MIT/Apache 系であることから、現状のロックファイル上は確認されなかった。改善点は、この確認が都度の手動 grep に依存していること（r4-lic-01 と同様）。継続的な検知手順をどう持つかは運用設計の問題で、違法性の評価ではない | 中 | 中 | 中 | 高 |

### r4-lic-11 Argus 流用時の NOTICE 連動が未手順化（現状はコピー無し）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r4-lic-11 | Argus 流用時の NOTICE 連動が未手順化（現状はコピー無し） | 調査項目 | `DESIGN.md:896-903` は Argus（AGPL）流用時に LICENSE / THIRD-PARTY-NOTICES の表示保持、派生ファイル先頭コメント、変更ファイル一覧の NOTICE 記載を要求し、`DESIGN.md:903` は P0 ではコピーしないと明記。`grep -n -i 'argus' NOTICE.md app/NOTICE.md README.md` の結果は `README.md:48` の「`security-review/Argus`（リポジトリ外）への参照が残っている」という注記のみで、`NOTICE.md` / `app/NOTICE.md` には Argus 記載なし。`app/src/*.rs` 全 14 ファイルに SPDX 付与済み（r4-lic-01） | 現状は `DESIGN.md:903` の方針どおり Argus コードをコピーしていないため、NOTICE に Argus が無いこと自体は矛盾ではない。将来 P2 で流用する際に、`DESIGN.md:899-902` の表示保持・派生明示・変更一覧記載を漏れなく反映するための確認手順が無い。手順化の要否と時期は設計判断として調査項目とする | 低 | 中 | 中 | 中 |

## レンズ総評

- 証拠強度: 本稿の全 finding はローカルの `DESIGN.md` / `NOTICE.md` / `app/NOTICE.md` / `README.md` / `app/Cargo.toml` / `app/Cargo.lock` / `packaging/linux/build-deb.sh` / desktop ファイルと、実行済みの `grep` / `awk` / `cargo metadata --offline` / `jq` の出力に基づく。外部の法的判断・第三者のライセンス解釈は採用していない。
- 確認できた強み: SPDX ヘッダーは対象 19 ファイルすべてに付与（r4-lic-01）、NOTICE の直接依存バージョンはロックファイルと整合（r4-lic-03）、DEB は LICENSE / NOTICE 群 / README を `/usr/share/doc` に同梱（r4-lic-06）、Zed の GPL コードは git 依存ゼロで混入していない（r4-lic-10）。
- 主な残課題: 著作権者表記の不在（r4-lic-02）、dirs の記述差（r4-lic-04）、README のソース案内粒度（r4-lic-05）、desktop Comment の案内（r4-lic-07）は比較的小さな記載・手順の問題。より優先度が高いのは推移的ライセンス expression の摘要齟齬（r4-lic-09）で、`DESIGN.md:925` の allow 一覧と `app/NOTICE.md:55-58` の要約の両方を実データと突き合わせる作業が必要。
- 法的 uncertainties の扱い: 「ISC 本文同梱要否」（r4-lic-08）、「MPL-2.0 等の allow 判定」（r4-lic-09）、「著作権表示の主体と形式」（r4-lic-02）、「配布時案内の十分性」（r4-lic-05 / r4-lic-07）はすべて調査項目として記録し、結論を述べていない。PASS 4 の他レンズが扱うリリース署名・Flatpak・リポジトリ公開・依存の保守停止/version pin の話題は意図的に含めていない。
- 変更範囲: 本レンズはファイルを 1 つ（`notes/task-discovery/round-4/license.md`）作成したのみで、ソースコード・設定・既存文書は一切変更していない。
