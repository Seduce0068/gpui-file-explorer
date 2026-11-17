---
type: Reference
title: "DOC-vs-CODE ギャップ分析（ラウンド1・パス1）"
description: "設計・README・レビュー報告に記載されつつ未実装／未解決の項目を抽出したタスク発見メモ"
tags:
  - gap-analysis
  - task-discovery
  - gpui-file-explorer
  - round-1
generated:
  by: "r1-gap2"
  at: "2026-11-17T19:50:21+09:00"
status: complete
---

## 発見事項

<!-- markdownlint-disable MD013 -->

指定された 4 資料（DESIGN.md / app/README.md / review-report.md / README.md）の該当行のみを根拠に、
「文書にはあるが未実装・未解決」の項目を抽出した。行番号は実読で確認済み。

### r1-gap-01: OS ファイルクリップボード連携（text/uri-list）未対応

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-gap-01 | OS ファイルクリップボード連携（text/uri-list） | 機能追加案 | README.md:83; app/README.md:381-383; DESIGN.md:98 | コピー/移動はアプリ内クリップボードのみで、GPUI 0.3.12 Linux バックエンドがファイル一覧の読み書きを伴わないため OS 連携が未実装。P2「ファイルリスト clipboard」(arboard 3.7) に位置づけられ、GPUI 側対応待ちまたは独自実装で閉じる。 | 高 | 高 | 中 | 高 |

### r1-gap-02: 検索結果のソース表示強化 未実装

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-gap-02 | 検索結果のソース表示強化 | 残タスク | README.md:83; app/README.md:423-424 | README の未実装栏に明記。検索結果から削除・リネームすると一覧が自動更新されず再検索が必要で、結果と元データを紐付ける表示/再同期の仕組みで閉じる。再帰検索は P1/P2 実装済み。 | 中 | 中 | 中 | 高 |

### r1-gap-03: 開く / リビールの失敗検知不可

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-gap-03 | 開く / リビールの失敗検知 | 機能追加案 | app/README.md:353-357; DESIGN.md:1095 | `cx.open_with_system` / `cx.reveal_path` が戻り値を持たず、xdg-open 失敗時も成功表示のまま。未確認事項 #4（KDE Wayland 実挙動）と対応し、D-Bus / プロセス終了コード監視の導入で閉じる。 | 中 | 高 | 中 | 高 |

### r1-gap-04: E2E 実押下検証（キーボード / 右クリック / ダイアログ）未実施

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-gap-04 | E2E 実押下検証の未実施 | 調査項目 | README.md:81-82; app/README.md:368-373; DESIGN.md:1092-1093,1099 | Wayland 合成入力ツールが無く、キーボード・右クリック・確認ダイアログはコンパイル＋データ層テストのみ。未確認事項 #1/#2/#8 に相当し、kdotool/ydotool 等での実押下確認または手動検証で閉じる。 | 高 | 中 | 低 | 高 |

### r1-gap-05: 完全削除（P2 限定解禁）未実装

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-gap-05 | 完全削除 | 機能追加案 | DESIGN.md:100; app/README.md:406,419 | 現状は `trash` のみで完全削除はなく、設計上も P0/MVP 禁止・P2 で明示確認付きの限定解禁。ゴミ箱ビューにも完全削除が無く、確認ダイアログと楽観更新を備えた P2 実装で閉じる。 | 中 | 中 | 低 | 高 |

### r1-gap-06: 設定保存の完全性（dir fsync / ダウングレード検出）未実装

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-gap-06 | 設定保存の耐性とバージョン検出 | 残タスク | app/README.md:358-361 | tmp + rename + `sync_all()` だがディレクトリの fsync が無く、`version` は 1 固定でダウングレード検出も未実装。ディレクトリ fsync とマイグレーション分岐の追加で閉じる（AC-11 系の既知制限）。 | 中 | 低 | 低 | 高 |

### r1-gap-07: サムネイル / OS ネイティブアイコン抽出 未実装

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-gap-07 | 画像サムネイルと OS ネイティブアイコン | 機能追加案 | DESIGN.md:91-92; app/README.md:416 | Lucide 静的アイコンのみで、P2 の OS ネイティブアイコン（freedesktop-icons）と画像サムネイル（`img(PathBuf)` + 非同期 LRU）は未実装。ロードマップ P2「OS アイコン、サムネイル」として実装で閉じる。 | 中 | 高 | 中 | 高 |

### r1-gap-08: 10 万件ディレクトリの実測性能 未計測

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-gap-08 | 大規模ディレクトリの実測性能 | 調査項目 | DESIGN.md:1101; DESIGN.md:1074 | 未確認事項 #10 で「未計測」とされ、P2 の 2 パス方式・background ソートの要否はここで判断すると明記。10 万件での計測と結果に基づく実装判断で閉じる。 | 中 | 中 | 中 | 高 |

### r1-gap-09: Windows / macOS 実機未検証とパッケージング未実装

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-gap-09 | クロスプラットフォーム検証と配布 | 調査項目 | README.md:84-87; app/README.md:462; DESIGN.md:1088,1100 | 未確認事項 #9（Windows/macOS 全般）が CI ビルド以外未検証で、配布は Linux amd64 の DEB のみ（署名・AppImage/Flatpak・Win/mac 未対応）。実機ビルド/実行検証と追加ターゲットのパッケージングで閉じる。 | 高 | 高 | 中 | 高 |

### r1-gap-10: 監視イベントの溢れ検知 / 自己操作抑制の取りこぼし

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-gap-10 | ファイル監視の取りこぼし対策 | 調査項目 | DESIGN.md:1097; app/README.md:412-414 | 未確認事項 #6 で溢れ時の notify 挙動が ⚠ で、自己操作抑制は 1.5 秒の時間ベース（外部同期変更を 1 回見逃す可能性）。10 秒 reconcile が保険だが、溢れ検知の実機ログ確認と抑制方式の改善で閉じる。 | 中 | 中 | 中 | 高 |

### r1-gap-11: unmaintained 依存警告 8 件（High 5 追跡継続）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-gap-11 | unmaintained 依存の追跡 | 残タスク | app/README.md:456-461; review-report.md:52-53,67 | `gpui-kit` が引き込む 8 クレートが unmaintained 警告で、レビュー判定でも High 5 として上流依存のため追跡継続。CVE は修正済みだが、`gpui-kit`/`gpui-pre` 更新後の `cargo update`＋`cargo audit` 再実行で閉じる。 | 中 | 中 | 低 | 高 |

### r1-gap-12: ui/* 完全分割の保留（設計判断）

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r1-gap-12 | ui/* モジュール分割 | 構造改善 | review-report.md:61-62 | High 1 は状態集約とフック抽出で実質軽減したが、完全な `ui/*` 分割は設計判断のため保留と明記。分割方針の合意と段階的なモジュール移動で閉じる（機能バグではなく構造負債）。 | 中 | 高 | 低 | 高 |

## レンズ総評

DOC-vs-CODE レンズとして、設計マトリクス・未確認事項・既知の制限・レビュー判定の 4 系統から
12 件の未実装／未解決項目を抽出した。全件が指定行の実読で裏付けられ、推測による追加は行っていない。

- **即効性が高い残タスク**: r1-gap-01（OS クリップボード）、r1-gap-02（検索結果ソース表示）、
  r1-gap-06（設定保存耐性）、r1-gap-11（unmaintained 依存）は文書上「未実装／追跡継続」と
  明記されており、優先度付けしやすい。
- **実機検証がボトルネック**: r1-gap-04 / r1-gap-08 / r1-gap-09 / r1-gap-10 は
  未確認事項 #1/#2/#8/#10/#9/#6 と重なり、コードではなく検証環境（Wayland 合成入力・
  実機・大規模データ）の整備が完了条件。
- **P2 相当の機能追加**: r1-gap-05（完全削除）、r1-gap-07（サムネイル/OS アイコン）は
  ロードマップ上の明示的な未実装で、価値は中程度・工数は高め。
- **構造負債**: r1-gap-12 はレビューで「承認」に昇格した後も設計判断で保留された項目で、
  機能ではなく保守性の課題として別トラックで扱うのが妥当。

注: 本パスは抽出専用であり、他レンズ（ux/platform/code/quality）のファイルは读んでいない。
重複排除・妥当性検証は後続パスに委ねる。
