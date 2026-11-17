---
type: Reference
title: "ユーザー向け機能発掘（パス1・UXレンズ）"
description: "実装済み機能とキーマップを基点に未実装のユーザー向け機能を発掘した一覧"
tags:
  - task-discovery
  - user-facing-features
  - ux
  - feature-ideation
  - gpui-file-explorer
generated:
  by: "r1-ux (AI subagent)"
  at: "2026-11-17T19:23:22+09:00"
status: complete
---

## レンズと基点

本レンズは USER-FACING FEATURE IDEATION を担当する。基点として
`README.md:79-88`、`app/README.md:80-116`（キーバインド）、
`app/README.md:118-247`（実装済み P0/P1）、`app/README.md:249-255`（今後の候補）、
`app/README.md:351-468`（既知の制限）、`DESIGN.md:61-105`（機能マトリクス）、
`DESIGN.md:109-270`（UX）、`app/src/strings.rs:7-77`（HELP_SECTIONS）を通読した。

既に実装済みの機能（コピー/移動/貼り付け、リネーム、複数選択、タブ、検索、
プレビュー、プロパティ、ブックマーク、最近使った場所、ボリューム一覧、
ゴミ箱ビューと復元、D&D、アンドゥ）は提案しない。加えて、
`app/README.md:249-255` の「今後の候補」および `DESIGN.md:100-103,1075` の
P2/P3 に既に載っている項目は、新角度を付けた場合のみ `novelty=既知` で記載した。

## 発見一覧

### r1-ux-01: 選択フォルダをターミナルで開く

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-01 | 選択フォルダをターミナルで開く | 機能追加案 | `app/README.md:114`, `app/src/main.rs:1358-1371`, `app/src/main.rs:4100-4203` | コンテキストメニューは「開く / リビール / パスをコピー」等のみでターミナル起動が無い。開発者は現在フォルダでシェルを開きたい。`std::process::Command` で端末エミュレータを cwd 付き起動する新コードが必要（GPUI API 外）。 | 高（開発者向け・定番） | M | 新規 | 高 |

### r1-ux-02: 空ファイルを作成

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-02 | 空ファイルを作成 | 機能追加案 | `app/README.md:108`, `app/README.md:137-142`, `app/src/main.rs:1664-1702` | 新規作成は「新規フォルダ」のみ。`create_new()`（O_EXCL）で空ファイルを作り、連番名 + 選択 + リネームへ促す。既存の `create_unique_folder` と衝突回避ロジックを流用できる。 | 高（Touch 相当の基本操作） | S | 新規 | 高 |

### r1-ux-03: ファイルの複製（Duplicate）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-03 | ファイルの複製（Duplicate） | 機能追加案 | `app/README.md:109-110`, `app/README.md:158-167`, `app/src/ops.rs:51-61`, `app/src/ops.rs:177` | コピー/貼り付けは 2 手で、同一フォルダへの複製も「コピー → 貼り付け」が必要。`ops` の `ConflictPolicy::Rename` と連番生成を使い、1 アクションで「コピー」副語尾の複製を作る。キーは Ctrl+D がブックマークで占有のため Ctrl+Shift+D 等を検討。 | 高 | S | 新規 | 高 |

### r1-ux-04: 相対パスをコピー

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-04 | 相対パスをコピー | 機能追加案 | `app/src/main.rs:1704-1708`, `app/README.md:114`, `app/src/strings.rs:7-77` | 「パスをコピー」は `display_path(&path)` の絶対パスのみ。選択項目から `cwd` 基準の相対パス（`strip_prefix`）をコピーする第二种を追加。設定やコード共有時の貼り付けが容易になる。GPUI のクリップボード API をそのまま利用可能。 | 中高 | S | 新規 | 高 |

### r1-ux-05: フォルダ内クイックフィルタ（インクリメンタル絞り込み）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-05 | フォルダ内クイックフィルタ | 機能追加案 | `app/README.md:189-203`, `app/README.md:196-197`, `app/README.md:426-427`, `app/src/strings.rs:55` | 既存の Ctrl+F は再帰検索＋後段フィルタで、表示中の一覧を文字入力で逐次絞り込む機能が無い。Dolphin の Ctrl+F / ranger の `/` 相当を現在フォルダに適用し、Esc で即解除する。 | 高（一覧が速く探せる） | M | 新規 | 高 |

### r1-ux-06: ファイルへのスター／カラータグ

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-06 | ファイルへのスター／カラータグ | 機能追加案 | `app/src/main.rs:4761`, `app/README.md:241-244`, `app/src/strings.rs:66` | Star アイコンはサイドバーのブックマーク（ディレクトリ専用）にのみ使用。ファイル単位の星/色ラベルと設定への永続化（`settings.rs` のパスキー保持を拡張）で「重要ファイル」を目立たせる。パス変更時の同期課題あり。 | 中 | M〜L | 新規 | 中 |

### r1-ux-07: 表示列のカスタマイズ

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-07 | 表示列のカスタマイズ | 機能追加案 | `app/README.md:124-125`, `app/README.md:409-410`, `app/src/main.rs:4237-4249` | 列は「名前 / 種類 / サイズ / 更新日時」固定で、ソートもヘッダクリックのみ。列の表示/非表示と顺序、ツールバーへのソートチップ配置を追加し、設定に保存する。`uniform_list` の行レイアウト改修が必要。 | 中 | M | 新規 | 中 |

### r1-ux-08: 選択項目の合計サイズをステータスバーに表示

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-08 | 選択項目の合計サイズ | 機能追加案 | `app/README.md:127-128`, `app/README.md:175-176`, `app/src/main.rs:3191-3195` | ステータスバーは件数/選択名/監視状態を表示するが選択サイズの合計が無い。各 Entry の `size` を合計し「3 件（12.4 MB）」と表示。ゴミ箱や未取得サイズは「-」扱い。空き容量自体は `app/README.md:431-433` で既知。 | 中高 | S | 新規（空き容量とは別角度） | 高 |

### r1-ux-09: 最近使ったファイル

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-09 | 最近使ったファイル | 機能追加案 | `app/README.md:243-244`, `app/src/settings.rs:106`, `app/src/main.rs:1358-1371` | 「最近」はディレクトリ最大 10 件のみ。`open_path` で開いたファイルも別セクション「最近使ったファイル」に記録し、プレビュー/既定アプリ起動へワンクリックで戻る。設定スキーマ拡張が必要。 | 中 | M | 新規 | 高 |

### r1-ux-10: バッチリネーム（連番・パターン）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-10 | バッチリネーム | 機能追加案 | `app/README.md:377-378`, `app/README.md:168-171`, `app/src/ops.rs:658-689` | 複数選択はできるがリネームはフォーカス行のみ。選択一式に「ベース名 (1)」「 prefix + 連番」等を適用する一括リネームを追加。既存の `rename_entry` を各件に適用し、衝突は件数表示で集約。 | 中 | M | 新規 | 中 |

### r1-ux-11: ブックマークのドラッグ並べ替え

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-11 | ブックマークの並べ替え | 機能追加案 | `app/README.md:241-244`, `app/README.md:443-444`, `app/README.md:206` | タブは `Ctrl+Shift+PageUp/Down` で並べ替え可能なのに、ブックマークは登録順固定で削除ボタンも無い。ドラッグまたは上下キーで順序変更し、設定の配列順を保存する。 | 中 | S | 新規 | 高 |

### r1-ux-12: プロパティに作成日時・最終アクセス日時を追加

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-12 | プロパティの日時拡張 | 機能追加案 | `app/src/main.rs:3008-3010`, `app/src/main.rs:3199`, `app/README.md:440-442` | プロパティは「更新日時」のみ表示。`std::fs::Metadata::created()` / `accessed()` を読み取り、サポートされない FS は「-」で表示するだけなので低リスク。 | 中 | S | 新規 | 高 |

### r1-ux-13: シンボリックリンクの作成

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-13 | シンボリックリンクの作成 | 機能追加案 | `app/README.md:160`, `app/src/ops.rs:380-381`, `app/src/main.rs:3203-3205` | リンクはコピー時・プロパティ表示で「辿らない」扱いが徹底されているが、リンクを作成する UI が無い。`std::os::unix::fs::symlink` を background で実行し、Windows は権限/開発者モード検知で無効化。 | 中 | M | 新規 | 中 |

### r1-ux-14: ゴミ箱の一括消去（二段確認付き）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-14 | ゴミ箱の一括消去 | 機能追加案 | `app/src/strings.rs:80-83`, `app/src/strings.rs:110`, `app/src/main.rs:4207-4232`, `DESIGN.md:784-787`, `DESIGN.md:100` | ゴミ箱メニューは「元の場所へ復元」のみで完全削除は未対応。`trash` クレートの API で purge を追加する際は、DESIGN §6.7 の二段確認・既定無効・赤警告・フルパス表示を必須化する。 | 高（容量回収） | M | 既知（DESIGN P2 §6.7 記載） | 高 |

### r1-ux-15: アーカイブの作成／抽出

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-15 | アーカイブの作成／抽出 | 機能追加案 | `app/src/icons.rs:52-55`, `app/Cargo.toml:13-27`, `DESIGN.md:1075`, `app/src/main.rs:4100-4203` | zip/tar 等は `FileKind::Archive` として認識されるが、作成も展開も出来ず「開く」は OS 任せ。右クリックに「ここで展開 / 圧縮」を追加するには `zip` / `tar` / `flate2` 等の新規依存が必要（DESIGN §7 の「P2 まで新規依存を入れない」方針と要調整）。 | 中 | L | 既知（DESIGN P3 ロードマップに「アーカイブ」記載） | 中 |

### r1-ux-16: タブの完全セッション復元

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-16 | タブの完全セッション復元 | 機能追加案 | `app/README.md:204-208`, `app/README.md:445-447`, `app/src/settings.rs:106` | 再起動時はタブのディレクトリとアクティブ index のみ復元され、各タブの戻る/進む履歴・選択・スクロール位置は失われる。`history.rs` / `Selection` を settings にシリアライズして復元する。 | 中 | M | 既知（既知の制限に記載・未対応） | 高 |

### r1-ux-17: パンくずセグメントのコピー／直接編集

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-ux-17 | パンくずのコピー／編集 | 機能追加案 | `app/README.md:116`, `app/README.md:120`, `app/README.md:239-240`, `app/src/strings.rs:13` | パンくずはクリック移動のみで、Ctrl+L のパス入力バーはあるが「セグメント名をコピー」「セグメントをクリックして入力バーに反映」が無い。各セグメントの右クリックメニューを追加し、移動とコピーを分離する。 | 中 | S | 新規（Ctrl+L あるいは新角度） | 中 |

## レンズ総評

実装済み P0/P1 は「ファイル操作の安全性」と「情報表示」に厚く、OC/Nautilus 系の
定型操作のうち **生成系（空ファイル・複製・シンボリックリンク・アーカイブ）** と
**検索/絞り込みの即時性（クイックフィルタ）** が明確に薄い。
特に `r1-ux-01`（ターミナルで開く）、`r1-ux-02`（空ファイル）、
`r1-ux-03`（複製）、`r1-ux-04`（相対パスコピー）、`r1-ux-05`（クイックフィルタ）は
既存の `ops.rs` の安全性基盤と `ClipboardItem` / `settings` の仕組みを再利用でき、
新規依存なしで S〜M 工数で実現できるため、次パスの優先候補として推奨する。

一方、`r1-ux-06`（カラータグ）、`r1-ux-07`（列カスタマイズ）、
`r1-ux-15`（アーカイブ）は UI/設定スキーマの拡張または新規クレートが必要で、
DESIGN §7 の「新規依存は P2 まで入れない」方針との調整が必要である。
`r1-ux-14` / `r1-ux-16` は既知項目だが、§6.7 の安全則と設定永続化の枠組みが
既に整っているため、実装難易度はドキュメント上の位置づけより低いと判断した。

全件は現行コード/ドキュメントの READ に基づく仮説であり、実機 E2E 未検証
（`app/README.md:368-373`）の制約は変わらない。6 パス目の重複排除では、
`r1-ux-08`（選択サイズ）と「空き容量」（DESIGN P2）を誤って統合しないよう注意されたい。
