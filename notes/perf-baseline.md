---
type: Reference
title: "パフォーマンス計測ベースライン（read_entries）"
description: >-
  DESIGN.md §10.5 の性能目標に対するデータ層の実測記録。
  クロスレビュー指摘 F6-42 の計測証跡不在を解消する。
tags:
  - performance
  - baseline
  - read-entries
  - fixture
generated:
  by: "perf-baseline"
  at: "2026-11-18T09:58:27+09:00"
verified:
  by: "perf-baseline"
  at: "2026-11-18T09:58:06+09:00"
status: "review"
---

## 概要

クロスレビュー指摘 F6-42 は、性能目標に対する計測証跡が存在しない
ことを指摘していた。本文書は、アプリのデータ層に限定した実測値と、
計測できなかった項目をその理由まで含めて記録する。

数値はすべて本セッションで実行したコマンドの出力に由来する。
推定値には明示的に「推定」と記し、実測値と混同しない。

対象は `app/src/model.rs` の `read_entries` と、それを呼ぶ
`app/src/main.rs` の一覧読み込み経路である。

## 環境

| 項目 | 実測値 |
| --- | --- |
| 計測日時 | `2026-11-18T09:53:37+09:00` 〜 `09:58:27+09:00` |
| カーネル | `Linux 7.0.0-43-generic x86_64`（`uname -srm`） |
| CPU | AMD Ryzen 5 3600 6-Core Processor |
| 論理 CPU 数 | 12（`nproc`） |
| メモリ合計 | 62 GiB（`free -h`） |
| 計測時利用可能 | 約 25 GiB（`free -h`） |
| Rust | `rustc 1.100.0 (0ef1bc386 2026-11-09)` |
| Cargo | `cargo 1.100.0 (3acc79c0c 2026-11-05)` |
| rust-script | 0.36.0 |
| Bash | GNU bash 5.3.9(1)-release |
| shellcheck | 0.11.0 |
| `/tmp` 種別 | tmpfs（`rw,noatime,inode64`） |
| `/tmp` 容量 | 32 GiB / 空き 11 GiB（67% 使用） |
| `/tmp` inode | 8,221,603 / 空き 8,016,755（3% 使用、生成前） |
| `/` | nvme0n1p7, 587 GiB / 空き 188 GiB |

カーネル文字列の全体は次である。

```text
Linux 7.0.0-43-generic #43-Ubuntu SMP PREEMPT_DYNAMIC
Fri Oct 23 13:25:25 UTC 2026 x86_64 GNU/Linux
```

## メモリを駆動するコードの調査

`read_entries`（`app/src/model.rs:87`）は `std::fs::read_dir` で
各項目を列挙し、`Entry` を `Vec` に格納する。
`Entry` は `OsString`、`name_text`、`sort_key`、`PathBuf` の
4 つのヒープ確保文字列とメタデータフィールドを持つ。

各項目につき `symlink_metadata` と `metadata` の
2 回の stat 系呼び出しが発生する（同ファイル 63〜71 行目）。

`apply_loaded`（`app/src/main.rs:1575`）は返された `Vec` を
`all_entries` に保持し、`rebuild_view` が
`visible_entries`（`app/src/model.rs:402`）で全件を clone して
`entries` にも保持する。すなわち一覧表示時は
同一内容の `Vec<Entry>` が 2 本同時に生存する。
`fingerprint`（同 648 行目）は全件を走査する。

検索経路（`app/src/main.rs:5461-5469`）では `all_entries` が
検索結果に入れ替わり、`search_lines` が別に保持される。

## 実行したコマンド

フィクスチャ生成スクリプトを作成し、構文と静的検査を行った。

```bash
bash -n scripts/gen-perf-fixture.sh
shellcheck scripts/gen-perf-fixture.sh
chmod 755 scripts/gen-perf-fixture.sh
```

両方とも終了コード 0 であった。
生成・計測はリポジトリ外の `/tmp` に限定した。

```bash
PERF_ROOT=$(mktemp -d /tmp/gpui-perf.XXXXXX)
# PERF_ROOT=/tmp/gpui-perf.eHhkZ5
/usr/bin/time -f 'real=%e user=%U sys=%S' \
  bash scripts/gen-perf-fixture.sh --tmp-root "$PERF_ROOT" 1000
/usr/bin/time -f 'real=%e user=%U sys=%S' \
  bash scripts/gen-perf-fixture.sh --tmp-root "$PERF_ROOT" 10000
/usr/bin/time -f 'real=%e user=%U sys=%S' \
  bash scripts/gen-perf-fixture.sh --tmp-root "$PERF_ROOT" 100000
```

データ層の計測は、アプリ本体の `model.rs` を
`#[path]` で取り込んだ一時 `rust-script` から
実関数 `read_entries` を呼んで行った。
一時ファイルは `/tmp` に置き、リポジトリのソースと
`Cargo.toml` は変更していない。
スニペットの依存はアプリ既存の
`alphanumeric-sort`、`serde`、`chrono` のみである。

```bash
export CARGO_NET_OFFLINE=true
/usr/bin/time -f 'wall=%e' rust-script \
  "$PERF_ROOT/measure_read_entries.rs" \
  "$PERF_ROOT/gpui-fe-perf-100000" 100k
/usr/bin/time -f 'wall=%e' rust-script \
  "$PERF_ROOT/measure_apply_loaded.rs" \
  "$PERF_ROOT/gpui-fe-perf-100000" 100000
```

後始末はスクリプトの `--cleanup` を用い、
3 つのフィクスチャをすべて削除した。

```bash
bash scripts/gen-perf-fixture.sh --tmp-root "$PERF_ROOT" \
  --cleanup 1000
bash scripts/gen-perf-fixture.sh --tmp-root "$PERF_ROOT" \
  --cleanup 10000
bash scripts/gen-perf-fixture.sh --tmp-root "$PERF_ROOT" \
  --cleanup 100000
```

## 測定結果

### フィクスチャ生成時間

`/usr/bin/time` の出力（秒）。

| ファイル数 | real | user | sys |
| --- | --- | --- | --- |
| 1,000 | 0.04 | 0.01 | 0.03 |
| 10,000 | 0.19 | 0.03 | 0.16 |
| 100,000 | 1.77 | 0.32 | 1.50 |

生成後に `find` で数えた空ファイル数は
それぞれ 1,000 / 10,000 / 100,000 で一致した。

### `read_entries` 単体

`rust-script` は既定で最適化ビルド（release）である。
VmRSS はプロセス自身が `/proc/self/status` を読んで取得した。
時刻は呼び出し前後、単位は KiB。

| エントリ数 | 経過 ms | VmRSS 前 | VmRSS 後 | 増分 |
| --- | --- | --- | --- | --- |
| 1,001 | 3.8 | 2,340 | 2,752 | 412 |
| 10,001 | 36.8 | 2,340 | 5,564 | 3,224 |
| 100,001 | 364.9 | 2,336 | 33,688 | 31,352 |

100k の増分 31,352 KiB は約 30.6 MiB である。

### `apply_loaded` 近似

`read_entries` の後に `visible_entries`
（`show_hidden = false`、既定ソート）と `fingerprint` を呼び、
2 本の `Vec` が生存した状態を測った。
`all_entries` と `entries` の両方を保持する実経路の近似である。

| エントリ数 | 表示数 | read ms | view ms | VmRSS 前 | VmRSS 中 | VmRSS 後 |
| --- | --- | --- | --- | --- | --- | --- |
| 1,001 | 1,000 | 3.9 | 0.5 | 2,352 | 2,764 | 3,016 |
| 10,001 | 10,000 | 38.8 | 6.1 | 2,320 | 5,576 | 8,356 |
| 100,001 | 100,000 | 383.5 | 59.5 | 2,356 | 33,708 | 61,800 |

100k の最終 VmRSS 61,800 KiB は約 60.4 MiB、
呼び出し前からの増分は 59,444 KiB（約 58.0 MiB）である。

注記:

- マーカーファイル `.perf-fixture-marker` が隠しファイルとして
  1 つ存在するため、`read_entries` の件数は指定数 +1 になる。
  `visible_entries` はこれを除外するため表示数は指定数である。
- 100k の `read_entries` 時間は別実行で 364.9 ms と 383.5 ms
  が得られ、実行間変動がある。
- `/usr/bin/time` の `maxrss` は `rust-script` が起動した
  cargo / rustc を含む最大値であり、計測対象プロセスの
  VmRSS ではない。よって本文では
  `/proc/self/status` の値を採用した。

## 100k 生成の可否判断

生成前に `df -h /tmp` は空き 11 GiB、
`df -i /tmp` は空き 8,016,755 inode（使用率 3%）であった。
100,000 個の空ファイルは空き inode の約 1.2% に相当し、
データ量は実質 0 バイトである。
したがって inode / 容量の枯渇リスクは低いと判断し生成した。

生成後は空き inode 7,916,756（使用率 4%）、
空き容量 11 GiB で、枯渇は発生しなかった。

## 目標との照合

| DESIGN §10.5 の項目 | 目標 | 本セッションの実測 |
| --- | --- | --- |
| メモリ（100,000 件） | 200 MB 以下 | データ層のみ約 60 MiB |
| 1,000 件の一覧表示 | 200 ms（release） | データ層 read 3.8 ms |
| 10,000 件の初回表示 | 500 ms（名前+種別） | read 36.8 + view 6.1 ms |

上記はあくまでデータ層の値であり、
DESIGN が指定する `/proc/<pid>/status` の VmRSS そのものではない。
したがって「目標を満たした」とは結論づけない。

## 計測できなかった項目

- GUI プロセスの VmRSS: アプリは GPUI の GUI であり、
  本セッションではディスプレイ操作や
  `xdotool` / `kdotool` による画面駆動を行っていない。
  対象ディレクトリへの移動と一覧確定後の
  `/proc/<pid>/status` 取得ができず、
  DESIGN が定める実アプリの VmRSS は未計測である。
- 一覧表示の end-to-end レイテンシ:
  描画・テキストレイアウトを含む 1,000 件 / 10,000 件の
  表示完了時刻は未計測。データ層のみの時間を記載した。
- スクロール 60 fps、キー応答 16 ms、監視イベント反映、
  アイドル CPU: いずれも本セッションの対象外で未計測。
- 実ディスク上の挙動: フィクスチャは tmpfs に置いたため、
  SSD / HDD での stat コストやページキャッシュの挙動は
  反映されていない。

## 次のステップ（完全な 100k 実行）

1. 一時ディレクトリを通常ディスク上（例: `/home` 配下の
   `mktemp -d`）に作り、同じスクリプトで 100,000 件を生成する。
2. release ビルドをディスプレイ付きで起動し、
   ロケーション入力で当該ディレクトリを開く。
3. 一覧が安定した時点で
   `/proc/<pid>/status` の `VmRSS` を採取する。
4. 200 MB 閾値と比較し、3 回以上繰り返して中央値を記録する。
5. 1,000 件 / 10,000 件についても描画完了までの時間を
   ログまたはスクショ時刻差で取得し、§10.5 の表を埋める。

## 参照

- [DESIGN.md §10.5 性能目標](../DESIGN.md)
- [review-report.md（F6-42）](../review-report.md)
- [計測用スクリプト](../scripts/gen-perf-fixture.sh)
