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
  at: "2026-11-18T11:09:33+09:00"
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
| カーネル | `Linux <kernel-version> x86_64`（`uname -srm`） |
| CPU | 汎用デスクトップCPU |
| 論理 CPU 数 | 複数（`nproc`） |
| メモリ合計 | 数十 GiB（`free -h`） |
| 計測時利用可能 | 十分な空き容量（`free -h`） |
| Rust | `rustc 1.100.0 (0ef1bc386 2026-11-09)` |
| Cargo | `cargo 1.100.0 (3acc79c0c 2026-11-05)` |
| rust-script | 0.36.0 |
| Bash | GNU bash 5.3.9(1)-release |
| shellcheck | 0.11.0 |
| `/tmp` 種別 | tmpfs（`rw,noatime,inode64`） |
| `/tmp` 容量 | 数十 GiB / 空き 約 10 GiB（使用率は中程度） |
| `/tmp` inode | 約 8 百万 / 空き 約 8 百万（低使用率、生成前） |
| `/` | `<disk-device>`, 約 600 GiB / 空き 約 200 GiB |

カーネル文字列の全体は次である。

```text
Linux <kernel-version> #<build> SMP PREEMPT_DYNAMIC
<build-date> UTC 2026 x86_64 GNU/Linux
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
| 1,001 | 約 4 | 約 2,300 | 約 2,800 | 約 400 |
| 10,001 | 約 37 | 約 2,300 | 約 5,600 | 約 3,200 |
| 100,001 | 約 365 | 約 2,300 | 約 33,700 | 約 31,000 |

100k の増分は約 31,000 KiB（約 30 MiB）である。

### `apply_loaded` 近似

`read_entries` の後に `visible_entries`
（`show_hidden = false`、既定ソート）と `fingerprint` を呼び、
2 本の `Vec` が生存した状態を測った。
`all_entries` と `entries` の両方を保持する実経路の近似である。

| エントリ数 | 表示数 | read ms | view ms | VmRSS 前 | VmRSS 中 | VmRSS 後 |
| --- | --- | --- | --- | --- | --- | --- |
| 1,001 | 1,000 | 約 4 | 約 0.5 | 約 2,400 | 約 2,800 | 約 3,000 |
| 10,001 | 10,000 | 約 39 | 約 6 | 約 2,300 | 約 5,600 | 約 8,400 |
| 100,001 | 100,000 | 約 380 | 約 60 | 約 2,400 | 約 33,700 | 約 62,000 |

100k の最終 VmRSS は約 62,000 KiB（約 60 MiB）、
呼び出し前からの増分は約 59,000 KiB（約 58 MiB）である。

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

## 実 GUI プロセスの VmRSS 実測（100,000 件）

前節で未計測だった実 GUI の VmRSS を、CLI 引数で
フィクスチャを指定する方法で計測した。
採用した値は `/proc/<pid>/status` の `VmRSS` のみである。

### 実行内容

- 計測時刻: `2026-11-18T10:37:51+09:00` 〜 `10:38:21+09:00`
- 実行バイナリ: `app/target/release/gpui-file-explorer`
- 再ビルド: `cd app && cargo build --release --locked`
  （終了コード 0、10.66 秒。旧バイナリより `settings.rs` と
  `main.rs` が新しかったため実行した）
- 表示環境: `DISPLAY=:1`、`WAYLAND_DISPLAY=wayland-0`、
  `XDG_SESSION_TYPE=wayland`
- 生成前の確認: `/tmp` 空き 19 GiB、空き inode 8,046,518。
  空ファイル 100,000 個でも枯渇リスクは低いと判断した。

実際に実行したコマンド（フィクスチャは `/tmp` 配下のみ、
計測後に削除済み）。

```bash
PERF_ROOT=$(mktemp -d /tmp/gpui-perf-vmss.XXXXXX)
# PERF_ROOT=/tmp/gpui-perf-vmss.7tY8zu
bash scripts/gen-perf-fixture.sh --tmp-root "$PERF_ROOT" 100000
setsid env DISPLAY=:1 WAYLAND_DISPLAY=wayland-0 \
  XDG_SESSION_TYPE=wayland nohup \
  ./app/target/release/gpui-file-explorer \
  /tmp/gpui-perf-vmss.7tY8zu/gpui-fe-perf-100000 \
  > /tmp/gpui-vmss-app.log 2>&1 < /dev/null &
LAUNCH_PID=$!  # 1486301
pgrep -a -f \
  'gpui-file-explorer /tmp/gpui-perf-vmss.7tY8zu/gpui-fe-perf-100000'
sleep 10
for i in 1 2 3 4 5; do
  TS=$(date '+%Y-%m-%dT%H:%M:%S%:z')
  RSS=$(awk '/^VmRSS:/{print $2}' /proc/1486301/status)
  SIZE=$(awk '/^VmSize:/{print $2}' /proc/1486301/status)
  DATA=$(awk '/^VmData:/{print $2}' /proc/1486301/status)
  THR=$(awk '/^Threads:/{print $2}' /proc/1486301/status)
  echo "$TS,1486301,$RSS,$SIZE,$DATA,$THR"
  if [ "$i" -lt 5 ]; then sleep 2; fi
done
kill -TERM "$LAUNCH_PID"
bash scripts/gen-perf-fixture.sh --tmp-root "$PERF_ROOT" \
  --cleanup 100000
rmdir "$PERF_ROOT"
```

### VmRSS サンプルと目標照合

`VmSize` は全サンプルで 2,604,392 KiB、`VmData` と
`Threads` は下記のとおりだった。

| # | 時刻 (+09:00) | VmRSS KiB | VmData KiB | Threads |
| --- | --- | --- | --- | --- |
| 1 | 10:38:13 | 329,712 | 292,756 | 29 |
| 2 | 10:38:15 | 329,712 | 292,756 | 29 |
| 3 | 10:38:17 | 329,712 | 292,756 | 29 |
| 4 | 10:38:19 | 329,712 | 292,756 | 29 |
| 5 | 10:38:21 | 329,712 | 292,756 | 29 |

最小・最大・中央値はいずれも 329,712 KiB で、変動はなかった。
これは約 322.0 MiB、10 進では約 337.8 MB に相当する。

| DESIGN §10.5 の項目 | 目標 | 実測 | 判定 |
| --- | --- | --- | --- |
| 100,000 件のメモリ | 200 MB 以下 | 329,712 KiB | 未達 |

200 MiB 基準では 124,912 KiB（約 122.0 MiB）超過、
200 MB（10 進）基準でも約 137.8 MB 超過である。
比率はそれぞれ約 1.61 倍、約 1.69 倍。
よって本計測では目標を満たしていない。

### 一覧が表示されたことの証拠

- `spectacle -b -n -o /tmp/gpui-vmss-100k.png` で全画面を保存した。
- 前面ウィンドウのパンくずは
  `/ > tmp > gpui-perf-vmss.7tY8zu > gpui-fe-perf-100000` だった。
- 同ステータスバーは `100000 件`、
  `選択: entry_000001 (0 B / ただいま)`、
  `隠し 1件`、`監視:2 イベント` を示した。
- 可視領域には `entry_000001` から `entry_000020` まで
  連続して表示され、サイズはすべて `0 B` だった。
- 拡大確認用の画像は `/tmp/gpui-vmss-statusbar.png` と
  `/tmp/gpui-vmss-listcrop.png` に保存した。
- `xwininfo -root -tree` には該当ウィンドウが現れなかった
  （Wayland ネイティブと推定）ため、spectacle の画像を
  証拠として採用した。

### 残る注意点

- VmRSS はプロセス全体の常駐ページである。GPUI ランタイム、
  wgpu / GPU ドライバのマッピング、テキストレイアウト、
  監視スレッド等を含み、前節のデータ層約 60 MiB と
  直接比較できない。
- 1 回の起動・1 フィクスチャのみである。ロード後 8 秒間の
  5 サンプルでは変動がなかったが、起動間のばらつきは
  未確認である。
- フィクスチャは tmpfs 上の空ファイルである。実ディスクの
  stat コストやページキャッシュの挙動は反映されない。
- 計測中に別の debug ビルド GUI（`/tmp` を表示、PID 1141147）が
  同時に生存していた。共有ライブラリや GPU ドライバの
  ページが両プロセスの RSS に数えられている可能性がある。
- ウィンドウは前面に描画された状態だった。最小化時等は
  別の値になり得る。
- VmRSS は共有ページを含む。PSS は未計測である。
- `/usr/bin/time` の `maxrss` は使用していない。
- 本タスクでは Rust ソース、Cargo.toml、Git 状態を変更していない。
  再ビルドは `app/target/` の生成物のみである。

## 空ディレクトリのベースライン VmRSS 実測

空ディレクトリを CLI 引数で開き、固定オーバーヘッドを実測した。
前節の 100,000 件計測（329,712 KiB）との差を、
エントリに起因する実コストとして扱う。

### 空ディレクトリの計測手順

- 計測時刻: `2026-11-18T11:07:52+09:00` 〜 `11:08:23+09:00`
- 実行バイナリ: `app/target/release/gpui-file-explorer`
  （再ビルドなし。前節と同じ 10:37 時点のビルド）
- 対象: `/tmp/gpui-perf-empty.TdqM08`（`mktemp -d`、中身 0 件）
- 表示環境: `DISPLAY=:1`、`WAYLAND_DISPLAY=wayland-0`、
  `XDG_SESSION_TYPE=wayland`
- PID: 1596386（`pgrep -f` で取得し、`/proc/1596386/exe` が
  上記リリースバイナリを指すことを確認）

`ls -la` は `.` と `..` のみ、`find -mindepth 1 | wc -l` は 0 だった。
実行したコマンドは次のとおり。

```bash
EMPTY_ROOT=$(mktemp -d /tmp/gpui-perf-empty.XXXXXX)
# EMPTY_ROOT=/tmp/gpui-perf-empty.TdqM08
ls -la "$EMPTY_ROOT"
find "$EMPTY_ROOT" -mindepth 1 | wc -l   # 0
setsid env DISPLAY=:1 WAYLAND_DISPLAY=wayland-0 \
  XDG_SESSION_TYPE=wayland nohup \
  ./app/target/release/gpui-file-explorer "$EMPTY_ROOT" \
  > /tmp/gpui-empty-app.log 2>&1 < /dev/null &
LAUNCH_PID=$!   # 1596386
pgrep -f "gpui-file-explorer $EMPTY_ROOT"   # 1596386
readlink -f /proc/1596386/exe
sleep 12
for i in 1 2 3 4 5; do
  TS=$(date '+%Y-%m-%dT%H:%M:%S%:z')
  RSS=$(awk '/^VmRSS:/{print $2}' /proc/1596386/status)
  SIZE=$(awk '/^VmSize:/{print $2}' /proc/1596386/status)
  DATA=$(awk '/^VmData:/{print $2}' /proc/1596386/status)
  THR=$(awk '/^Threads:/{print $2}' /proc/1596386/status)
  echo "$TS,1596386,$RSS,$SIZE,$DATA,$THR"
  if [ "$i" -lt 5 ]; then sleep 2; fi
done
spectacle -b -n -o /tmp/gpui-perf-empty.png
kill -TERM "$LAUNCH_PID"
rm -f /tmp/gpui-empty-app.log
rmdir "$EMPTY_ROOT"
```

### VmRSS サンプル

全サンプルで `VmSize` は 2,473,936 KiB、`VmData` は 125,372 KiB、
`Threads` は 29 だった。

| # | 時刻 (+09:00) | VmRSS KiB | VmData KiB | Threads |
| --- | --- | --- | --- | --- |
| 1 | 11:08:15 | 204,784 | 125,372 | 29 |
| 2 | 11:08:17 | 204,792 | 125,372 | 29 |
| 3 | 11:08:19 | 204,792 | 125,372 | 29 |
| 4 | 11:08:21 | 204,792 | 125,372 | 29 |
| 5 | 11:08:23 | 204,796 | 125,372 | 29 |

最小 204,784 / 最大 204,796 / 中央値 204,792 KiB
（約 200.0 MiB、10 進で約 209.7 MB）。範囲は 12 KiB で
ほぼ安定していた。

### 空状態の証拠

- `spectacle -b -n -o /tmp/gpui-perf-empty.png` で全画面を保存した。
  画像は証拠として `/tmp` に残してある。
- パンくずは `/ > tmp > gpui-perf-empty.TdqM08` だった。
- 中央に「このフォルダは空です」と表示された。
- ステータスバーは
  `/tmp/gpui-perf-empty.TdqM08 | 0 件 | 選択: なし` だった。
- 計測後に `kill -TERM` で終了し、空ディレクトリを削除した。

### 100,000 件との差分

| 項目 | VmRSS KiB | 換算 |
| --- | --- | --- |
| 空ディレクトリ（本節・実測中央値） | 204,792 | 約 200.0 MiB |
| 100,000 件（前節・実測） | 329,712 | 約 322.0 MiB |
| 差分（100k 件の実コスト） | 124,920 | 約 122.0 MiB |

差 124,920 KiB はデータ層単体の 61,800 KiB（約 60.4 MiB）の
約 2.02 倍である。100k 件の増分のうち約 60 MiB がデータ層、
残り約 61.6 MiB が一覧描画・選択状態等の GUI 側構造に
相当すると推定される。

旧来の推測（329,712 - 61,800 = 267,912 KiB、約 261.6 MiB が
ランタイム）は、実測ベースライン 204,792 KiB を
63,120 KiB（約 61.6 MiB）過大評価していた。

### 証明されたことと依然として推定であること

証明されたこと（Runtime Proof）:

- 空ディレクトリを開いただけの同バイナリが、
  中央値 204,792 KiB の常駐メモリを使うこと。
- 100,000 件の GUI プロセスとの差が 124,920 KiB であること。
- 旧来の「ランタイム約 262 MiB」という推測が
  実測と一致しないこと。

依然として推定であること:

- 差 124,920 KiB の内訳（データ層 61,800 KiB と
  GUI 側 63,120 KiB への分割）は、2 種類の計測の
  単純減算であり、直接の内訳計測ではない。
- 100k 側と空側で、同時に生存する別プロセス
  （前節の debug GUI、PID 1141147 等）の有無が
  管理されていない。共有ライブラリや GPU ドライバの
  ページが両者の RSS に異なる形で含まれ得る。
- 双方とも 1 回の起動のみで、起動間ばらつきは未確認。
- VmRSS は共有ページを含む。PSS / USS は未計測。
- VmSize の差（2,604,392 - 2,473,936 = 130,456 KiB）は
  仮想アドレス空間の差であり、常駐メモリではない。
- 本節でも Rust ソース、Cargo.toml、Git 状態は変更していない。
