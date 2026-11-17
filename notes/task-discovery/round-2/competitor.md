---
type: Reference
title: "他ファイルマネージャーとの機能パリティ調査（ラウンド2・パス2）"
description: >-
  Dolphin / Nautilus / Thunar / Finder / ranger / Double Commander 等の一般機能と
   gpui-file-explorer の実装済み機能を突き合わせ、pass1 未カバーの未達・追加案を発掘したメモ。
tags:
  - competitor-analysis
  - feature-parity
  - task-discovery
  - gpui-file-explorer
  - round-2
generated:
  by: "r2-competitor"
  at: "2026-11-17T20:14:20+09:00"
status: complete
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 調査の前提と範囲

起点として `app/README.md:80-247`（キーバインド・実装済み P0/P1）と
`app/src/strings.rs:7-90`（`HELP_SECTIONS`）を実読し、実在機能を確定した上で、
他ファイルマネージャーの一般的機能（自らの知識に基づく。バージョン番号・出典は記載しない）と比較した。
`DESIGN.md:61-108` の機能マトリクスも実読し、P2/P3 に既に載っている項目は `novelty` を `既知` とした。
pass1 の対象テーマ（OS クリップボード、検索ソース、open/reveal 失敗検知、E2E、完全削除、
設定 durability、サムネイル、100k 性能、配布 CI、watcher 復旧、ui/* 分割、端末、
空ファイル、複製、相対パスコピー、クイックフィルタ、カラータグ、列カスタマイズ、
選択サイズ、最近使ったファイル、一括リネーム、ブックマーク並べ替え、作成/アクセス時刻、
シンボリックリンク作成、ゴミ箱 purge、アーカイブ、タブ復元、パンくず、debug フック、
sync_all、v2 移行、Windows 長パス、macOS NFD、ドライブ列挙、PollWatcher、
アプリ識別、AGPL §13）は意図的に除外した。

<!-- markdownlint-disable MD013 -->

### r2-cmp-01: 分割（デュアルペイン）ビュー

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-cmp-01 | 分割（デュアルペイン）ビュー | 構造改善 | app/README.md:88,204-208; DESIGN.md:95 | 現在は詳細/グリッド切替（Ctrl+1/2）とタブのみで、画面を左右に分割して 2 ディレクトリを同時表示する機能は無い。DESIGN.md:95 は「タブ / 複数ウィンドウ」を P2 △ / P3 ◎ とし、分割ビューは明示されていない。Double Commander / Dolphin の代表機能で、コピー元と先を同時に見られる点がタブと決定的に異なる。 | 高（ドラッグ移動・コピーの作業効率が段違い。Double Commander の核） | L（単一 Entity 前提の ADR-002 を再設計し、ペインごとの Listing/Selection/Watcher を管理する必要があり、既存の監視・ナビゲーション配線をほぼ作り直す） | 新規（DESIGN §2 に分割ペインの記載なし。タブ行の延長ではない独立機能） | 高（コード・設計共に実読。実装難易度は設計知見からの推測） |

### r2-cmp-02: 「Open With…」アプリ選択（既定アプリ以外）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-cmp-02 | 「Open With…」アプリ選択 | 調査項目 | app/README.md:100,113,136; DESIGN.md:82 | `Enter` / ダブルクリック / 右クリック「開く」は `cx.open_with_system` で既定アプリを起動するのみ。Dolphin / Nautilus / Thunar の「他のアプリで開く」に相当する、候補アプリ一覧からの選択 UI が無い。GPUI 側にアプリ指定 API が無い可能性が高く、実現性の調査が先。 | 高（「既定の見ographicallyViewer が違う」「このファイルだけ別エディタで開きたい」は日常的な要求） | M（実現性次第。デスクトップエントリの列挙と MIME 判定は自前 or `gio`/`xdg-mime` 委譲が必要で新規依存または外部プロセス連携） | 新規（pass1 の「open/reveal 失敗検知」は既存 API の結果通知の話で、アプリ選択 UI とは別） | 中（現状は実読で確定。GPUI API の可否は未検証のため中） |

### r2-cmp-03: 保存済み検索（名前を付けた検索条件）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-cmp-03 | 保存済み検索（named search） | 機能追加案 | app/README.md:189-203,198-199; app/src/strings.rs:55 | 再帰検索・内容検索・種別/拡張子絞り込み・検索範囲・検索履歴（最大 10 件）はあるが、条件（語句 + 種別 + 拡張子 + 範囲 + モード）をまとめて名前付きで保存し、サイドバーから再実行する機能は無い。Finder のスマートフォルダ、ranger の保存検索に相当。 | 中高（「今日のビルド失敗ログ」「社内 PNG だけ」等、繰り返す検索をワンクリック化できる。履歴チップより再利用性が高い） | M（既存 `settings.rs` の JSON 永続化を拡張するだけなら新規概念なし。検索条件の構造体 + UI リスト + 実行系の結線が必要） | 新規（pass1 の「クイックフィルタ」「検索ソース表示」は別。DESIGN §2 の再帰検索行にも保存検索の記載なし） | 高（README と strings.rs を実読。未実装は明らか） |

### r2-cmp-04: コマンドパレット（Ctrl+Shift+P 的な操作起動）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-cmp-04 | コマンドパレット | 構造改善 | app/src/strings.rs:7-77; app/README.md:80-116,478 | `HELP_SECTIONS` に約 30 のキー操作が列挙されているが、それらを横断検索して実行するパレットは無い。README:478 はキー入力の Action 化を「設計判断のため保留」としており、コマンドパレットは Action 定義（`DESIGN.md:720` の §5.8）と相性が良い。 | 中（キー記憶が不要になり、F1 ヘルプが「実行できるヘルプ」に昇格する。操作の発見性が上がる） | M（S ではない。全操作の Action 化またはディスパッチ表の整理が前提で、既存の `on_key_down` 経路との二重管理を避ける設計が必要。新規依存なし） | 新規（DESIGN §2 にパレットの記載なし。F1 ヘルプの延長ではあるが実行系が無い） | 高（F1 の一覧と Action 保留の記述を実読） |

### r2-cmp-05: チェックサム / ハッシュ計算

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-cmp-05 | チェックサム / ハッシュ計算 | 機能追加案 | app/README.md:229-232,441-442 | プロパティは名前・パス・種類・サイズ・更新日時・権限・リンク先を**読み取り専用**で表示するのみで、SHA-256 / MD5 等のダイジェスト表示は無い。Dolphin の「Integrity」や Double Commander のハッシュ計算に相当。 | 中（ダウンロード済みファイルの整合性確認、コピー後の同一性検証に使える。読み取り専用で安全） | M（既存 `ops.rs` の 64 KiB チャンク進捗基盤を再利用できる。`sha2` クレート追加で軽量、外部 `sha256sum` 委譲なら依存なしだが OS 依存） | 新規（pass1 のプロパティ関連は「選択サイズ」「作成/アクセス時刻」で、ダイジェストは未扱い） | 高（README 実読。未実装は明らか） |

### r2-cmp-06: ファイル比較 / diff（2 ファイル選択から）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-cmp-06 | ファイル比較 / diff | 機能追加案 | app/README.md:175-177,233-238 | 複数選択とテキストプレビューはあるが、2 ファイルを選んで差分を並べて表示する機能は無い。Double Commander の「比較」、ranger の外部 diff 連携に相当。PyCharm やFinder の「Compare」も近い。 | 中（設定ファイルやコードの微差確認がエクスプローラ内で完結する） | M（外部 `diff` / `meld` / `kompare` を起動する想定。インストール状態依存。自前の行 diff UI にすると L で新規依存 or 自作アルゴリズム） | 新規（既存の「名前/内容検索」は一致行番号の表示で、2 ファイル間の差分ではない） | 高（現状は実読。実装コストは見積り） |

### r2-cmp-07: ディスク使用量のツリーマップ可視化

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-cmp-07 | ディスク使用量ツリーマップ | 機能追加案 | app/README.md:231-232; DESIGN.md:94; app/src/mounts.rs:13-22 | ディレクトリの再帰サイズはプロパティの「サイズを計算」で得られるが、`MountPoint` は `source/path/fs_type` のみで空き容量を持たない。DESIGN.md:94 は「ボリューム/空き容量」を P2 ◎（`sysinfo` Disks）とするが、容量をツリーマップで可視化する構想は無い。Nautilus の Disk Usage Analyzer / Baobab に相当。 | 中（「どのフォルダが容量を食っているか」を一発で把握でき、既存の再帰サイズ計算を活かせる） | L（`sysinfo` 追加 + 再帰集計のキャンセル/進捗 + ツリーマップ描画と、UI・データ両面の新規作業） | 新規（pass1 の「ドライブ列挙」は一覧表示の話で、使用量分析・可視化は別。DESIGN §2 の容量行も数値表示の想定） | 高（README/DESIGN/mounts.rs を実読） |

### r2-cmp-08: 権限 / 所有者の編集（プロパティから）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-cmp-08 | 権限 / 所有者の編集 | 機能追加案 | app/README.md:229-232,441-442 | プロパティは権限を Unix のみ表示し、所有者/グループ名は表示しない。**読み取り専用**のため chmod/chown も不可。Nautilus / Thunar の「プロパティ → 権限」タブ、Finder の情報パネルに相当。 | 中高（USB メモリの実行権付与、共有フォルダの権限修正がエクスプローラ内で完結する） | M（`chmod`/`chown` 外部プロセス委譲を想定。numeric/mode の双方向 UI + 再帰適用まで含めると L。chown は特権が必要で設計判断を要する） | 新規（pass1 の「カラータグ」「読み取り専用プロパティ」は別。DESIGN §2 に権限編集の記載なし） | 高（README 実読。権限変更は破壊的操作につき確認 UI が必須） |

### r2-cmp-09: undo スタックの再起動後永続化

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-cmp-09 | undo スタックの再起動後永続化 | 構造改善 | app/README.md:218-224,407-408; app/src/undo.rs:12-19 | アンドゥは「名前変更とゴミ箱」のみ・最大 20 件で、**再起動すると消える**。操作履歴（Ctrl+Shift+U）もこのメモリ上スタックを表示するだけ。ゴミ箱 ID による復元は持続するので、ログを永続化すれば再起動後の「削除を取り消す」が可能になる。 | 中（誤削除に気づいたのが再起動後でも復元できる。trash-first 方針と親和性が高い） | M（既存 `settings.rs` とは別ファイルの op-log JSON を追加。ゴミ箱 ID の失効・元パスの再利用・対象が消えた場合の拒否ルールを設計する必要があり、単なる直列化ではない） | 新規（pass1 の「設定 durability / schema」は設定ファイルの話で、操作履歴の永続化とは別枠） | 高（README と undo.rs を実読） |

### r2-cmp-10: テキストプレビューのシンタックスハイライト

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-cmp-10 | テキストプレビューのシンタックスハイライト | 残タスク | app/README.md:233-238,434; app/src/preview.rs:19-22 | プレビューは等幅・読み取り専用のプレーンテキストで、README:434 は「シンタックスハイライトは非対応」と明記。拡張子に応じた色付けは、`preview.rs` が既に拡張子判定（画像形式）を持つため流入点は近い。 | 中（コード・設定ファイルを開かずに中身を把握できる。検索の行番号表示と合わせて価値が出る） | M（`syntect` 等の新規依存を許容すれば M。依存を避け自前のトークナイザにすると L かつ品質が落ちる。GPUI 側のテキスト描画 API 調査が必要） | 新規（pass1 の「サムネイル/OS アイコン」はアイコンの話で、テキスト着色は別） | 高（README と preview.rs を実読） |

### r2-cmp-11: PDF プレビュー

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-cmp-11 | PDF プレビュー | 残タスク | app/README.md:233-238,434; app/src/preview.rs:19-22 | README:434 が「プレビューはテキストと画像のみ（PDF・動画…は非対応）」と明記。Dolphin / Finder の Quick Look は PDF を表示する。画像判定は magic number ベースで `.pdf` を既に拒否している。 | 中（マニュアル・請求書・仕様書を外部アプリなしで確認できる） | L（PDF レンダラの新規依存が重い。外部ビューワ起動に逃がすなら M だが、それは「開く」と同義になりプレビュー価値が薄れる） | 既知（README で非対応と明示された既知の制限。ただし pass1 の対象リストには入っていないため、本ラウンドで初めて拾う） | 高（README と preview.rs を実読） |

### r2-cmp-12: SSH/SFTP リモートブラウズとネットワーク場所へのクイックアクセス

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-cmp-12 | SSH/SFTP リモートブラウズとネットワーク場所 | 調査項目 | app/src/mounts.rs:1-7; app/README.md:245-247,431-433 | サイドバー「ドライブ」は `/proc/mounts` 解析のみで他 OS は空リスト。カレント + 親の非再帰 watch 前提のため、リモート FS を通常の `Entry` として扱う経路が無い。Nautilus の「サーバーに接続」、Double Commander の VFS に相当。 | 中（リモート作業がエクスプローラ内で完結。ただし本プロジェクトの「ローカル・非破壊・依存最小」方針との相性は要検討） | L（ssh/sftp クレート or `gio`/`sshfs` 委譲、認証情報の取り扱い、watch 不可時の Degraded 表示など横断的な設計が必要。秘密情報を settings に書かない設計が必須） | 新規（pass1 の「ドライブ列挙」「PollWatcher」はローカルマウント/監視方式の話で、リモート VFS は別） | 中（現状は実読で確定。実現性・適否は未調査） |

### r2-cmp-13: EXIF / メタデータの編集

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-cmp-13 | EXIF / メタデータの編集 | 機能追加案 | app/src/preview.rs:24-38; app/README.md:229-232,441-442 | 画像は形式判定と描画のみで、撮影日時・タグ・GPS 等のメタデータの読み出しすら無い。プロパティも読み取り専用。Finder の情報パネルや Dolphin のメタデータタブに相当。 | 低中（写真整理のニッチは強いが、汎用ファイルマネージャの必須機能ではない） | L（exif 系クレート追加 + 書き込み時の破壊リスク + プレビュー全体を壊さない設計。読み取り専用表示だけなら M に下がる） | 新規（pass1 の「カラータグ」は独自ラベルで、EXIF の標準メタデータとは別） | 中（未実装は実読で確定。価値の大小はユーザー依存で中） |

### r2-cmp-14: パスを `file://` URI としてコピー

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-cmp-14 | パスを `file://` URI としてコピー | 機能追加案 | app/README.md:114,136-139 | 右クリック「パスをコピー」は `ClipboardItem` に生のパス文字列を入れるだけ。ブラウザの URL バー・Markdown リンク・一部アプリの入力欄に貼れる `file://` URI 形式のコピーは無い。 | 低中（バグ報告やドキュメントにファイルをリンクとして貼る用途。実装が軽いので費用対効果は良い） | S（既存 `copy_path` にバリアントを追加するだけ。new 依存・heavy API  work なし。パーセントエンコードだけ注意） | 新規（pass1 の「相対パスコピー」「パンくずコピー」は文字列の種類が異なり、`file://` URI は未扱い） | 高（README 実読。既存 copy_path の存在は明記されている） |

### r2-cmp-15: タブグループ / タブの名前付け・色分け

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-cmp-15 | タブグループ / タブのラベル | 機能追加案 | app/README.md:204-208; app/src/strings.rs:60-67; DESIGN.md:95 | タブはディレクトリ単位で開き、並べ替えと構成復元はあるが、グループ化・名前付け・色分けは無い（表示名はディレクトリ名固定）。DESIGN.md:95 が「タブ / 複数ウィンドウ」を P2 △ / P3 ◎ と位置づける延長線上にある。 | 低（多数タブを「作業別」にまとめたい場合に限定的に効く。単一ウィンドウ・少タブではほぼ不要） | M（`Tabs` 構造体と設定スキーマの拡張。並べ替え・復元処理との整合が必要だが新規依存なし） | 既知（DESIGN §2 の「タブ / 複数ウィンドウ」行の延長。独立した新規テーマではない） | 高（README/strings.rs/DESIGN を実読） |

## レンズ総評

実装済みの P0/P1 は「安全なファイル操作・検索・プレビュー・タブ」までかなり厚く、
heaviest な欠落は **r2-cmp-01（分割ペイン）** と **r2-cmp-07（使用量分析）** の 2 つである。
どちらも既存アーキテクチャ（単一 Entity / 非再帰 watch / read_dir ベース）に根本から触れるため、
「既存の延長」ではなく ADR を伴う設計判断として扱うべきだ。

次に費用対効果が高いのは **r2-cmp-14（file:// URI コピー, S）**、
**r2-cmp-05（ハッシュ計算, M）**、**r2-cmp-03（保存済み検索, M）**、**r2-cmp-09（undo 永続化, M）** で、
いずれも新規依存なしまたは軽量で、既存の `ops.rs` / `settings.rs` / `undo.rs` の延長で閉じる。
**r2-cmp-02（Open With…）** と **r2-cmp-12（SFTP）** は価値が高いが、
GPUI / OS API の可否と秘密情報の扱いが未確認のため、実装ではなく先に調査項目として残すのが誠実である。

検討して除外した候補と理由:

- フォルダテンプレート（`README.md:108,147-148` の新規フォルダの延長）は価値が低く、
  「連番プローブ + create_dir」の既存実装を壊す理由が弱い。
- ソート/フィルタのプリセット（`README.md:115,409-410`）は pass1 の「列カスタマイズ」と
  重複気味で、本レンズの独自性が出せない。
- ファイルロック、バージョニング、クラウド連携は外部サービス/常駐プロセスを要し、
  現状の「ローカル・非破壊・依存最小」方針と矛盾するため提案不成立とした。
- ゴミ箱保持方針 UI は pass1 の「trash purge / 完全削除」と近接し、重複を避けた。
- paste-as-link は pass1 の「シンボリックリンク作成」と意味が重なるため除外した。

なお外部ファイルマネージャーの機能比較は自らの知識に基づき、特定のバージョン番号や
公式ドキュメントへの引用は行っていない（精度より誤認回避を優先した）。
本ファイルで引用した `file:line` はすべて実読で確認済みである。
