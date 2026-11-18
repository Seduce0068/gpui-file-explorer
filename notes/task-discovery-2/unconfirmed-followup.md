---
type: Reference
title: F6-39〜48 未確認項目のフォローアップ調査
description: >-
  第2次クロスレビュー最終統合レポートの「未確認」節 10 項目について、
  ローカルのコード・設定・registry ソース・既存ノートのみを根拠に
  「確認できた事実／未確認／必要な環境・手順」を整理した記録。
tags:
  - task-discovery
  - cross-review
  - unconfirmed
  - gpui-file-explorer
generated: 2026-11-18T05:23:00+09:00
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 調査の範囲と前提

- 出典: [consolidated.md](round-6/consolidated.md) の
  「未確認（外部仕様・実機・計測に依存）」節（F6-39〜F6-48）。
- 基準時刻: 2026-11-18T05:23:00+09:00（`date` コマンドで確認）。
- 文中の行番号は読み取り時点（2026-11-18 05:12〜05:22 JST）の
  スナップショット。調査中にも並行作業でソースが更新されたため、
  参照時は関数名・定数名で再検索してください。
- 現行 HEAD は `ac00ff7`。統合レポートの生成時点から修正コミット
  （例: `ef278ff` = F6-03、`70682a2` = F6-02、`c7849e2` =
  F6-14/F6-29）が入り、前提が変わった項目がある。
  本報告は現行ファイルの記述を優先し、行番号は上記スナップショットの値。
- 参照元: `app/src/*`、`app/Cargo.toml`、`deny.toml`、`README.md`、
  `DESIGN.md`、`NOTICE.md`、`app/NOTICE.md`、`packaging/`、
  `~/.cargo/registry/src/index.crates.io-*/` の依存ソース、および
  `notes/` 配下の既存調査ノート。
- 外部送信・実機操作・ビルド実行・git commit は行っていない。
  読み取りと、この新規ファイルへの書き込みのみ。
- 推測は「推測」と明記する。法的判断・性能達成の結論は行わない。

## 判定一覧

| ID | テーマ | 現状の整理 |
| --- | --- | --- |
| F6-39 | 攻撃連鎖 | 一部は修正済み。TOCTOU / FIFO / `/proc` は未再現 |
| F6-40 | 画像 OOM | バイト上限あり。次元検証なし・未再現 |
| F6-41 | case-only rename | 二段階 rename を実装。実機未検証 |
| F6-42 | 100k 実測 | `notes/perf-baseline.md` に実測あり。200 MB 目標は未達（P2） |
| F6-43 | シャットダウン | `quit()` のみ。終了時保存・join・掃除なし |
| F6-44 | 設定スコープ | 単一 Window / 単一保存先。flock なし |
| F6-45 | notify | 非再帰＋10 秒再同期。PollWatcher 化なし |
| F6-46 | WASM / AGPL | デスクトップ専用。アプリ内 §13 案内なし |
| F6-47 | Windows/macOS | 依存ゲートのみ。実機意味論は未検証 |
| F6-48 | ライセンス | 一覧と deny 設定あり。copyright は DEP-5 未対応 |

## F6-39: symlink / FIFO / `/proc` の攻撃連鎖

### F6-39 で確認できた事実

- `ops.rs:254-342`: 一時ファイル名に nanos + PID + カウンタを使い、
  `create_new(true)`（O_EXCL）で予約してから `rename` する。
  `sync_all()` は best-effort。エラー時は `.partial-*` を削除。
- `ops.rs:594-650`: move も `create_new(true)` で宛先を予約してから
  `fs::rename`（F6-02 の修正、`70682a2`）。
- `ops.rs:387-422`, `462-479`: コピー・削除とも `symlink_metadata`
  で特殊ファイルを拒否し、シンボリックリンクはリンクとして扱う。
- `preview.rs:101-126`: シンボリックリンクと非 regular file を拒否し、
  `metadata.len()` と実際の読み取りの二重上限を設ける。
- `search.rs:382-400`: 内容検索も `symlink_metadata` で判定し、
  リンクをスキップ。FIFO は `is_file()` でないため通常は除外。
- `main.rs:651-656`: `GPUI_FE_DEBUG_*`（29 種）は
  `cfg(debug_assertions)` の debug ビルドでのみ読み取られる
  （F6-03 の修正、`ef278ff`）。

### F6-39 で未確認の事項

- `O_NOFOLLOW` / `fstat` による inode 照合はコード内に存在しない
  （grep で一致 0 件）。`symlink_metadata` → `File::open` の
  check→use 窓は論理的に残存する。
- FIFO を check 後に差し替えた場合の `open` ブロッキングは未実測。
  `O_NONBLOCK` 相当の指定も見当たらない。
- `/proc/*` は `metadata.len()` が 0 を返すことが多く、サイズ検査を
  通過し得る。表示・検索への露出は未再現（推測を含む）。
- 統合レポートが攻撃連鎖の主要原因とした「リリース版デバッグフック」
  は現行コードでは無効化済み。したがって旧 `r5-adv-03/04` の
  連鎖はそのままでは成立しない（推測）。リリースバイナリで未確認。

### F6-39 に必要な環境・手順

- Linux（Kubuntu）の隔離一時ディレクトリと、書き込み権限を持つ
  別ユーザー／ディレクトリを用意する。
- 通常ファイルを対象に、check 直後に symlink へ付け替える競合
 ハーネスを繰り返し実行し、読み取り先を観測する。
- `mkfifo` で writer のいない FIFO を用意し、`timeout` 付きで
  プレビュー／内容検索が停止するかを計測する。
- `/proc/self/environ` 等への symlink 差し替えで、上限内に
  収まる内容が表示されるかを確認する。
- 緩和策の検証には `O_NOFOLLOW`（rustix / cap-std 等）または
  open 後の `fstat` による inode 照合の導入が必要（設計判断）。

## F6-40: 画像プレビューのピクセル次元 / OOM

### F6-40 で確認できた事実

- `preview.rs:20-22`: `MAX_PREVIEW_BYTES = 256 KiB`、
  `MAX_IMAGE_BYTES = 8 MiB`。
- `preview.rs:96-135`: ファイル種別・サイズを検査し、
  `File::take(max_bytes + 1)` で実読み取りも制限。
  マジックバイトで形式を判定するが、PNG の IHDR 等の
  幅・高さは解析していない。
- ローカル registry の `gpui-pre-0.3.12/src/platform.rs:3363-3370`:
  描画側は `image::ImageReader::with_format(...).into_decoder()` で
  デコードし、明示的な `Limits` は設定していない。
- 同 `image-0.25.10/src/io/limits.rs:49-56`: 既定の `Limits` は
  `max_image_width/height = None`、`max_alloc = 512 MiB`
  （non-strict、デコーダにより無視され得る）。

### F6-40 で未確認の事項

- Declared な巨大次元（例: 100000x100000）を持つ 8 MiB 未満の
  PNG で、実際に OOM / ハングするかは未再現。
- 各デコーダが既定 `max_alloc` をどこまで守るか、
  デコードエラーが UI にどう伝わるかは未確認。
- アプリ側で次元上限を追加するかどうかは設計判断であり、
  本調査では結論を出さない。

### F6-40 に必要な環境・手順

- 8 MiB 未満だが巨大な幅・高さを宣言する PNG / ICO を生成する。
- debug と release の両方でプレビューを開き、VmRSS
  （`/proc/<pid>/status`）と応答時間を記録する。
- デコーダ側のエラー文言と、UI が「読み込み中」で固まらない
  ことを確認する。

## F6-41: case-only rename の中間ファイル消失

### F6-41 で確認できた事実

- `ops.rs:656-695`: `rename_entry` は `validate_file_name`、
  `src.exists()`、`target.exists()` を検査する。
- 大小文字のみの変更は、`canonicalize` が同一を指す場合に
  `.rename-<pid>-<nanos>` という予測困難な一時名を経由して
  二段階で rename する（推測可能な一時名を使う旧
  `r5-adv-10` の指摘は現行コードで解消済み）。
- 一時名は `new_name` を含まないため NAME_MAX 超過も避ける設計。

### F6-41 で未確認の事項

- Windows NTFS / macOS APFS の実機で、二段階 rename が
  期待どおり大小文字を反映するかは未検証。
- 一段目成功・二段目失敗時に、元ファイルが一時名のまま
  一覧から消える挙動は未確認（ロールバック処理は見当たらない）。
- Windows の予約名・末尾ドット/空白の判定が `validate_file_name`
  でどこまでカバーされるかは未確認。

### F6-41 に必要な環境・手順

- Windows 11（NTFS）と macOS 15（APFS）の実機を用意する。
- `foo` → `Foo`、`FOO.txt` → `foo.txt`、予約名、 NAME_MAX 近傍
  のケースを実行し、一覧と実 FS の両方で名前を確認する。
- 二段目の rename を失敗させる（競合ファイルを先に作る等）
  手法を用意し、元名への復帰可否を記録する。

## F6-42: 100k 件の実測と O(n) ホットスポット

### F6-42 で確認できた事実

- `DESIGN.md:1021-1032`: 100,000 件で VmRSS 200 MB 以下、
  スクロール 60 fps、キー応答 16 ms 以内等の目標が定義済み。
- 実測成果物は `notes/perf-baseline.md:249-475` に存在（2026-11-18 追記）。
  実 GUI で 100,000 件の VmRSS は 329,712 KiB、空ディレクトリは
  204,792 KiB、差分 124,920 KiB。200 MB 目標は未達（P2 先送り）。
  初回調査時点では存在しなかったため、当時の記述は陳腐化している。
- `scripts/check.sh` にベンチマーク / プロファイリングの手順は
  含まれていない。
- 検索の上限は `main.rs:4073-4082`, `4131-4165` で
  `global_cap = 2000` をルート数で均等分割する方式に修正済み
  （旧「per-root 500 固定」は F6-07 の修正で解消）。

### F6-42 で未確認の事項

- 1k / 10k / 100k のロード時間と VmRSS は `notes/perf-baseline.md` に
  実測済み（100k 実 GUI で 329,712 KiB、200 MB 目標は未達）。
  フレーム時間（60 fps / 16 ms）と GUI エンドツーエンドレイテンシは未計測。
- `r5-scale-01/02/03/09` 等の O(n²) ホットスポットは静的解析に
  基づく「理論的」候補であり、実測で裏付けられていない。

### F6-42 に必要な環境・手順

- 1k / 10k / 100k のファイルを生成するスクリプトを用意する。
- 同一マシン・同一ビルド設定で起動し、
  `/proc/<pid>/status` の VmRSS とフレーム時間を記録する。
- 実行コマンド、日時、環境（CPU / メモリ / FS）を
  `notes/perf-baseline.md` に残す（本タスクでは作成しない）。
- 測定後に O(n²) 候補を優先度付けし、修正の有無を判断する。

## F6-43: グレースフルシャットダウン

### F6-43 で確認できた事実

- `main.rs:1835`: Ctrl+Q は `cx.quit()` を呼ぶのみ。
- `main.rs:6140-6145`: 最後のウィンドウが閉じたときも `cx.quit()`。
  `main.rs:6160` が `open_window` の唯一の呼び出し。
- 操作キャンセルは協調型。`main.rs:462-476` のトークンと
  `ops.rs` の `CancelFlag` で、エントリ間・64 KiB チャンクごと
  に確認する。
- `ops.rs:300-310`: コピーエラー／キャンセル時は `.partial-*`
  と宛先予約を削除する（通常終了経路では掃除される）。
- `watcher.rs:172-178`: `DirWatcher::drop` は debouncer の
  Drop による停止フラグ設定に依存し、明示的な join はしない。
- `settings.save()` の呼び出しは操作・ナビゲーション系に分散し、
  `cx.quit()` の経路には見当たらない（`DESIGN.md:840` は
  「ウィンドウ終了時」の保存を想定）。
- 起動時の `.partial-*` 走査 / 掃除は実装されていない
  （参照は `ops.rs:944` のテストのみ）。

### F6-43 で未確認の事項

- Ctrl+Q 時に進行中 Task がキャンセルされるか、
  監視スレッドが完全に終了するかは未実測（GPUI / notify の
  Drop 実装に依存する推測を含む）。
- 書き込み途中の SIGTERM / SIGKILL で `.partial-*` が残るかは
  未実測。プロセス / スレッドの残存も未確認。

### F6-43 に必要な環境・手順

- GUI セッションで大きなファイルのコピーを開始し、途中で
  Ctrl+Q、別ケースで SIGTERM / SIGKILL を送る。
- `ps` / `/proc/<pid>/task` でプロセス・スレッド残存を確認し、
  対象ディレクトリに `.partial-*` が残っていないかを検査する。
- 設定が終了時に保存される要件（DESIGN §8.1）の要否を決め、
  必要なら終了フックを設計する（本タスクでは実装しない）。

## F6-44: 複数ウィンドウ / インスタンスの設定スコープ

### F6-44 で確認できた事実

- `open_window` は `main.rs:6160` の 1 箇所のみ。複数タブは
  アプリ内状態であり、GPUI の複数 Window は使っていない。
- `settings.rs:390-444`: 保存は nanos + PID の一時ファイルへ
  書き、`rename` で原子的に置き換える。Unix では 0600。
- `flock` / `try_lock` 相当の排他はコード内に存在しない
  （grep で一致 0 件）。
- `settings.rs:408-421`: 一時名は `create_new(true)` で
  symlink の先読みを拒否する。

### F6-44 で未確認の事項

- GPUI が複数 Window をサポートする場合の設定スコープ
  （アプリ共通 / Window 固有）は未調査。
- 同一 `XDG_CONFIG_HOME` で 2 インスタンスを起動した場合の
  上書き順序は未実測（last-writer-wins と推測される）。
- flock の方針は ADR 未作成。

### F6-44 に必要な環境・手順

- 同一設定ディレクトリを指す 2 インスタンスを起動し、
  同時に設定変更を行って保存結果を比較する。
- GPUI 側の Window API と終了時挙動をローカル registry で確認し、
  設定をアプリスコープに固定するかを ADR で決定する。

## F6-45: notify のネットワーク FS / Windows バッファ溢れ

### F6-45 で確認できた事実

- `watcher.rs:16`, `101-104`: `notify-debouncer-full` の
  `RecommendedWatcher` を使用。`DEBOUNCE = 400 ms`。
- `watcher.rs:140-171`: 監視は `RecursiveMode::NonRecursive`、
  失敗は `last_error` と `Rescan` に変換される。
- `PollWatcher` へのフォールバックはコード内に存在しない。
- `main.rs:74`, `1105-1141`, `1268-1277`: 10 秒ごとの
  再スキャン（fingerprint 比較）でイベント取りこぼしを補う。
- ローカル registry `notify-8.2.2/src/windows.rs:41`:
  `BUF_SIZE = 16384`。`ERROR_NOTIFY_ENUM_DIR` の明示処理は
  grep で見当たらない（ソース上の記述のみ、実機未確認）。
- `notes/fs-cross-platform-notes.md:108-116`: NFS 等の
  ネットワークマウントでイベントが来ない既知問題と、
  PollWatcher 回避策が notify README に基づき記録されている。
  同ノートの inotify 実測値（max_user_watches 518185 等）は
  2026-11-11 のローカル値。

### F6-45 で未確認の事項

- ネットワーク FS 上で実際にイベントが欠落するか、
  10 秒再同期で復旧するかは未実測。
- Windows の `ReadDirectoryChangesW` バッファ溢れ時の挙動は
  実機未確認。
- PollWatcher を導入するかどうかは設計判断で未決定。

### F6-45 に必要な環境・手順

- NFS / SMB マウントを用意し、ファイル生成・削除のイベントが
  UI に反映されるかを確認する。
- Windows 11 で短時間に大量の変更を発生させ、欠落と
  `Rescan` / 再同期による復旧を観測する。
- 必要なら PollWatcher フォールバックの条件と間隔を設計する。

## F6-46: WASM 化と AGPL §13

### F6-46 で確認できた事実

- `README.md:96-104`: 現試作はデスクトップ専用。
  `gpui-pre` + `gpui-pre-platform` を `default-features = false`
  にすれば stable で wasm32 にコンパイル可能と記録
  （2026-11-14 の検証ログは `notes/wasm-browser-notes.md`）。
- 同ノート §1-1: `gpui-kit = "=0.9.1"` の既定 feature は
  nightly 必須の `wasm_thread` を有効にするため stable で失敗。
- `app/Cargo.toml:14,33`: 現行も `gpui-kit = "=0.9.1"` を使用。
  `trash` は Windows と Freedesktop Unix のみを依存対象とする
  target ゲートに変更済み（`c7849e2`）。
- `README.md:106-112`, `NOTICE.md:17-20`: リポジトリ URL と
  ソース提供の旨を記載。`DESIGN.md:888-892` は
  §13 の案内を README / About に表示する要件を P2 とする。
- `app/src/*` に About 画面やソース URL の文字列は見当たらない
  （grep で一致なし）。`README.md:77` はタグ付け未実施を記載。

### F6-46 で未確認の事項

- `cargo check --target wasm32-unknown-unknown --all-targets --locked`
  を 2026-11-18T11:38+09:00 に実行（終了コード 101）。
  `trash` の target ゲートは効いたが、今度は推移依存の
  `wasm_thread 0.3.3` が `#![feature(stdarch_wasm_atomic_wait)]` を
  要求し、stable コンパイラで E0554 で失敗。
  同ノート §1-1 の「nightly 必須」という結論は実測で再確認された。
- Web デモで改変版をネットワーク越しに使わせる場合の
  AGPL §13 の適用・十分な案内形式は法的に未確認。
- 本調査は法的判断を行わない。

### F6-46 に必要な環境・手順

- `wasm32-unknown-unknown` ターゲットで現行 HEAD の check を
  実行し、結果をログとして残す（本タスクの書き込み範囲外）。
- Web デモを想定するなら、UI / README / 配布ページに
  ソース入手 URL を明示し、タグと SHA-256 を用意する。
- §13 の適用範囲は法務レビューで確認する。

## F6-47: Windows / macOS のパス意味論

### F6-47 で確認できた事実

- `app/Cargo.toml:28-34`, `app/src/trash_view.rs:12-90`:
  `trash` を対応 OS のみに限定し、非対応 OS では
  空のゴミ箱ビューを返す no-op フォールバックを提供
  （F6-14 の修正、`c7849e2`）。
- `README.md:84-86`: Windows は `cargo check` レベルで成功、
  macOS は上流 `gpui-pre-apple` のクロスビルド制約で check 不可。
  実機ビルド・実行は未検証。
- `notes/fs-cross-platform-notes.md:263-275`: Windows 長パス
  （`\\?\` + longPathAware）、macOS NFD、大小文字非区別、
  隠し属性の扱いが設計知識として記録されている（一部 ⚠）。
- 現行 `app/Cargo.toml` に `dunce` / `unicode-normalization`
  は含まれていない。
- `app/src/mounts.rs:140` は `/proc/mounts` を直接読み、
  Linux 固有の実装になっている。
- `ops.rs:656-695`: case-only rename の二段階化は実装済み。

### F6-47 で未確認の事項

- 長パス（MAX_PATH 超過）、NFD 正規化、大小文字のみの
  rename、隠し属性の判定について実機再現は皆無。
- Windows / macOS の実機で trash が期待どおり動くかも未検証。
- macOS のクロスビルド制約の詳細は README の記述に依存。

### F6-47 に必要な環境・手順

- Windows 11（longPathAware 有効・開発者モード）と
  macOS 15（APFS）の実機を用意する。
- 長パス、NFD ファイル名、大小文字衝突、隠し属性の
  一覧・検索・コピー・削除・rename をケース表で実行する。
- 実機結果を `notes/` に記録し、必要なら依存追加
  （dunce / unicode-normalization 等）を検討する。

## F6-48: 第三者ライセンス本文と DEP-5 copyright

### F6-48 で確認できた事実

- `deny.toml` が存在し、`licenses.allow` に
  AGPL-3.0-or-later / Apache-2.0 / MIT / MPL-2.0 /
  Unicode-3.0 等を列挙。`advisories` は
  RUSTSEC-2026-0543 を ignore、`unmaintained = "workspace"`。
- `scripts/check.sh:109-149` は cargo-deny / cargo-audit /
  cargo-vet を導入時に実行する（ローカル確認:
  cargo-deny 0.21.0、cargo-audit 0.23.0 が導入済み）。
- Runtime Proof: `cargo metadata --format-version 1 --offline
  --manifest-path app/Cargo.toml` の結果は 815 パッケージ。
  うち MPL-2.0 は 3 件（`cbindgen 0.28.0`、`dwrote 0.11.5`、
  `option-ext 0.2.0`）、`self_cell 1.3.0` は
  `Apache-2.0 OR GPL-2.0-only`、`r-efi` は
  `MIT OR Apache-2.0 OR LGPL-2.1-or-later`、
  `libfuzzer-sys` は `(MIT OR Apache-2.0) AND NCSA`。
- `app/NOTICE.md` は主な直接依存と Lucide（ISC）の帰属を
  記載し、完全な一覧は `cargo metadata` コマンドで得る方針。
  第三者ライセンス本文は同梱していない。
- `packaging/linux/build-deb.sh:134-138`: DEB の
  `/usr/share/doc/gpui-file-explorer/` に `LICENSE` を
  `copyright` として、NOTICE 群と README を配置。
- ステージング済み `copyright` の先頭は AGPL 正文であり、
  Format / Files / Copyright / License の DEP-5 スタンザはない。
- `app/Cargo.toml` / `NOTICE.md` / `app/NOTICE.md` に
  アプリ自身の `authors` / `Copyright` 行は見当たらない。

### F6-48 で未確認の事項

- この DEB はローカルビルド品であり、Debian アーカイブ
  登録を前提とするなら Debian Policy §5.6.7 / DEP-5 の
  適用範囲を確認する必要がある（法的・手続き的判断は未実施）。
- バイナリ配布で各推移依存のライセンス本文をどこまで
  同梱すべきか、ISC 本文が必要かは未確認。
- `deny.toml` の allow 一覧が実際の配布物に十分かは
  `cargo deny check` の実行結果に依存し、本タスクでは未実行。

### F6-48 に必要な環境・手順

- Debian Policy §5.6.7 と DEP-5 の一次資料を読み、配布形態
  （私的配布 / リポジトリ公開 / Debian 登録）ごとの要件を整理する。
- `lintian` でビルド済み DEB を検査し、copyright に関する
  警告を確認する。
- 必要であれば Files / Copyright / License スタンザ付きの
  DEP-5 `copyright` を生成し、著作権者・年号を決定する。
- 法的判断は法務レビューに委ね、本リポジトリの成果物には
  事実と手順のみを記録する。

## 証拠の索引

- [app/src/ops.rs](../../app/src/ops.rs)
- [app/src/preview.rs](../../app/src/preview.rs)
- [app/src/search.rs](../../app/src/search.rs)
- [app/src/watcher.rs](../../app/src/watcher.rs)
- [app/src/settings.rs](../../app/src/settings.rs)
- [app/src/trash_view.rs](../../app/src/trash_view.rs)
- [app/src/main.rs](../../app/src/main.rs)
- [app/Cargo.toml](../../app/Cargo.toml)
- [deny.toml](../../deny.toml)
- [scripts/check.sh](../../scripts/check.sh)
- [README.md](../../README.md)
- [DESIGN.md](../../DESIGN.md)
- [NOTICE.md](../../NOTICE.md)
- [app/NOTICE.md](../../app/NOTICE.md)
- [fs-cross-platform-notes.md](../fs-cross-platform-notes.md)
- [wasm-browser-notes.md](../wasm-browser-notes.md)
