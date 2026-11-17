---
type: Reference
title: 敵対的セキュリティ再レビュー（ラウンド2・セキュリティレンズ）
description: >-
  app/src/ops.rs / settings.rs / search.rs / preview.rs / main.rs / model.rs を
  敵対的視点で再精読し、TOCTOU・パス操作・リソース枯渇・情報漏えい・
  デバッグフックなどの新規攻撃面を file:line 根拠付きで列挙した調査項目一覧。
tags:
  - task-discovery
  - gpui-file-explorer
  - security
  - threat-model
  - round-2
generated:
  by: "r2-security"
  at: "2026-11-17T20:18:55+09:00"
status: complete
---

## 発見一覧

PASS 1 の対象（case-only rename O_EXCL / Windows symlink copy / 未保守依存 /
open・reveal 失敗検知 / watcher 回復 / 設定耐久性・スキーマ / 100k 性能 /
AGPL §13 / EXDEV・中途キャンセル試験）は意図的に除外した。
以下はすべて実読した `file:line` に基づく。「深刻度」は description 内に明記した。
エクスプロイトを証明しきれていない項目は「調査項目」として硬化タスクに留め、
過大評価していない。

| ID | title | category | evidence | description | value | effort | novelty | confidence |
|---|---|---|---|---|---|---|---|---|
| r2-sec-01 | デバッグフックがリリースビルドでも環境変数で作動する | 構造改善 | main.rs:507-539, 589-630, 5301-5314, 5326-5368（`cfg(debug_assertions)` は app/src 全体に皆無） | 深刻度: Medium。`Explorer::new` が `GPUI_FE_DEBUG_*` を無条件に読み、`GPUI_FE_DEBUG_DROP_PATH`(628) は 5301-5314 で `PathBuf::from` して `drop_external` → 現在フォルダーへコピーする。攻撃者が .desktop / systemd unit / シェルプロファイル経由で環境変数を設定できれば、起動時に任意パスのファイルを作業ディレクトリへ複製させられる。TRASH / TRASH_RESTORE / RENAME も同様に起動時発火。緩和策: フック全体を `#[cfg(debug_assertions)]` で囲むか、リリースビルドでは `GPUI_FE_DEBUG_*` を無視する明示的ゲートを追加し、統合テストでリリース設定時にフックが死んでいることを検証する。 | リリースバイナリの攻撃面を実質的にゼロ化。環境変数経由の意図しないファイル操作を防止 | S | 新規 | 高 |
| r2-sec-02 | プレビューの symlink 検査と File::open の間に TOCTOU | 調査項目 | preview.rs:101-108, 116, 127-134, 149-158 | 深刻度: Medium（マルチユーザー/サービス権限コンテキストでは High）。`symlink_metadata` で symlink と非 regular file を拒否した後、`File::open(path)`(116, 158) はパスを再解決して symlink を辿る。書き込み可能な共有ディレクトリで、通常ファイル `note.txt` を選択直後に `/home/other/.ssh/id_*` へ向けた symlink へ差し替えると、最大 256 KiB(MAX_PREVIEW_BYTES:20) / 8 MiB(MAX_IMAGE_BYTES:22) を読み取り表示する可能性がある。現状はユーザー自身の権限範囲に留まるため単一ユーザー desktop では越境しない点を明示。緩和策: `O_NOFOLLOW` 相当の open（cap-std / rustix 等、unsafe  prohibition に留意）または open 後の `fstat` で inode を照合し、検査済み object と同一であることを確認する。 | 共有ディレクトリ閲覧時の symlink リードthrough を遮断。監査証跡としても重要 | M | 新規 | 中 |
| r2-sec-03 | ゴミ箱移動の存在確認と削除が別フェーズ（TOCTOU） | 調査項目 | main.rs:3474-3479, 3505-3518, 3519-3529 | 深刻度: Low-Medium。UI スレッドで `path.exists() \|\| path.is_symlink()` を確認し、確認ダイアログを挟んでから background で `trash::delete(&path)`(3524) を呼ぶ。確認後に攻撃者が対象を symlink や別ディレクトリへ差し替えると、ユーザーが承認していない object がゴミ箱へ送られる可能性がある。`trash` crate が最終コンポーネントを rename する実装なら symlink 自体が移動するだけだが、crate の実装詳細を本レビューでは未検証。緩和策: 確認時点で canonicalize した ID を保持し、削除直前に同一 inode かを再確認、失敗時はスキップしてユーザーに通知する。 | 「確認したものだけを削除」という安全保証を並行環境でも維持 | M | 新規 | 中 |
| r2-sec-04 | 設定 JSON の配列が読み込み時に上限適用されない | 残タスク | settings.rs:34-39, 169-201, 226-233, 248-258 / main.rs:636 | 深刻度: Medium。`MAX_RECENT_DIRS=10` / `MAX_BOOKMARKS=50` / `MAX_RECENT_SEARCHES=10` は `record_recent`(107) / `toggle_bookmark`(123) / `record_search`(139) のミューテータ側だけで、`load_from_with_backup` は `read_to_string`(176) でファイル全体を読み込み `serde_json::from_str`(179) するだけ。`validate()`(226-233) は `last_dir` の存在チェックしかしない。悪意ある settings.json に数百万件の `tabs` / `bookmarks` / `recent_dirs` を入れるとメモリを消費し、起動時の `this.settings.save()`(main.rs:636) でそのまま書き戻されてディスクも消費する（serde の再帰深度制限は配列要素数を縛らない）。配置先がユーザー設定ディレクトリなので特権昇格ではない点を明記。緩和策: デシリアライズ後に各 Vec を `truncate(MAX_*)`、`tabs` にも上限を新設、`active_tab` も clamp、読み込み前にファイルサイズ上限を設ける。 | 細工された設定による起動時 OOM / ディスク枯渇を防止。境界値テストも追加可能 | S | 新規 | 高 |
| r2-sec-05 | 再帰的なコピー/削除/サイズ計算に深度上限が無くスタック枯渇 | 構造改善 | ops.rs:395-441 (`copy_recursive`), 482-499 (`remove_entry_safe`), 557-577 (`total_bytes_of`) | 深刻度: Medium (DoS)。3 関数はいずれも自分自身を再帰呼び出しし、深さ制限が無い。数万段の入れ子ディレクトリを用意して paste / 削除 / サイズ表示を引き起こすとスタックオーバーフローでプロセスが落ちる。対照的に `dir_stats`(523-555) と `search_by_name`(search.rs:104-152) は明示的スタックで反復実装しており、モジュール内で方針が不一致。PASS 1 の「100k 性能」は描画・件数性能の話であり、本件は再帰呼び出しによるクラッシュという別問題。実際のクラッシュ深度はスタックサイズ依存で未測定。緩和策: 3 関数を `dir_stats` 同様の反復スタックへ変換するか、共通の `MAX_WALK_DEPTH` を導入して超過時に `OpError::Unsupported` を返す。 | 深いツリーに対する堅牢性を全ファイル操作で統一。クラッシュによる作業中データ損失を防止 | M | 新規 | 高 |
| r2-sec-06 | コピー元/先の自己包含チェック後にディレクトリを差し替えられる | 調査項目 | ops.rs:240-252 (`ensure_not_into_itself`), 446-478 (`copy_entry`), 581-610 (`move_entry`) | 深刻度: Low-Medium。`ensure_not_into_itself` は `canonicalize(src)` と `canonicalize(dest_dir)` を比較して「自分自身/配下へのコピー」を拒否するが、その後の `copy_recursive` / `fs::rename` までは時間が空く。攻撃者がdest_dir を別場所への symlink に差し替えると、検証は通ったまま意図外のディレクトリへ書き込まれる。個々のファイルは `copy_file_atomic` の O_EXCL 予約(331-360)で守られるが、親ディレクトリの ID は再検証されない。緩和策: canonicalize 結果を Open 時点まで保持し、ディレクトリハンドル（cap-std 等）で操作するか、書き込み直前に親の `symlink_metadata` と inode を再照合する。 | 検証と使用を同じ object に結びつけ、安全機構の前提を強化 | M | 新規 | 中 |
| r2-sec-07 | Windows 予約名チェックが末尾ドット/空白を正規化していない | 残タスク | ops.rs:152-169（特に 158 行目の `name.split('.').next()`） | 深刻度: Low。`validate_file_name` は Windows で `CON` / `NUL` / `COM1` 等を拒否するが、`stem = name.split('.').next()` のため `CON .txt` の stem は `"CON "` となり RESERVED に一致せず通過する。Windows は末尾の空白/ドットを除去するため、デバイス名への意図しない解決や操作失敗を招きうる。`a\0b` や `..` の検査(137-151)は有效。Windows 実機が無いため本レビューでは静的推論どまりで未検証。緩和策: 比較前に `trim_end_matches([' ', '.'])` を適用し、Win32 の予約名规则（末尾ドット/空白、COLON 等）を網羅したテストを Windows CI で追加。 | Windows でのファイル名検証の精度向上。cross-platform 一体性 | S | 新規 | 中 |
| r2-sec-08 | パーミッション保持が rename 後かつ best-effort で行われる | 残タスク | ops.rs:344 (rename), 362-368 (`set_permissions` を `let _ =` で無視) | 深刻度: Low-Medium。`copy_file_atomic` は `.partial-*` を O_EXCL で作り `fs::rename(&partial, dest)` した後で `fs::set_permissions(dest, mode)` を呼ぶ。そのため (1) rename 直後から set_permissions まで dest は umask 依存の既定パーミッション（典型 0644）で存在し、(2) set_permissions が失敗しても `let _ =` で握りつぶされ、0600 の秘密ファイルが 0644 のコピーとして残る。マルチユーザー環境では所有権は変わらないが、同一ユーザーの他プロセスやグループからの読み取りが一時的に/永続的に許される。緩和策: 一時ファイル生成時に `OpenOptionsExt::mode(0o600)` で源の mode を先に設定するか、set_permissions の失敗を `OpError` として報告する。 | 0600 → 0644 の「静かな権限緩和」を防止。データ機密性の保証を向上 | S | 新規 | 高 |
| r2-sec-09 | 絶対パス・設定内容がステータス/エラー/クリップボードへ露出 | 構造改善 | ops.rs:76-89 (`OpError::Display`), 109-129 (`io_error`), main.rs:3558-3568, 1704-1707 | 深刻度: Low (情報漏えい)。`OpError` は `NotFound(path)` / `AlreadyExists(path)` 等で `path.display()` をそのまま表示し、`Unsupported` / `CrossesDevices` メッセージにもフルパスを埋め込む(116-123)。ゴミ箱失敗通知(main.rs:3563-3567)は crate の `reason` をそのまま連結する。`copy_path`(1704-1707) はフルパスを OS クリップボードへ書き込み、その内容がステータスバーにも残る。画面共有/スクリーンショット/ログ転送時にホームディレクトリ構造やユーザー名、マウント先が漏れる。緩和策: ユーザー向け表示はファイル名と親ディレクトリ名に短縮し、完全パスは `tracing::debug!` のみに出す。クリップボードへは必要に応じて相対パスを採用。 | 画面・ログ経由のパス情報漏えいを低減。エラー文言の一貫性も改善 | S | 新規 | 高 |
| r2-sec-10 | SVG プレビューが外部参照を含む可能性（内容 sniffing のみ） | 調査項目 | preview.rs:81-88 (`text.contains("<svg")`), 127-134 (`ImageBytes` を renderer へ渡す) | 深刻度: Low-Medium（未証明）。`sniff_image_format` は先頭 1 KiB に `<svg` があれば `ImageFormat::Svg` と判定し、そのバイト列を gpui_kit の画像レンダラーへ渡す。SVG は `<image xlink:href="file:///...">` や `<foreignObject>` で外部リソースを参照しうるため、レンダラーが参照を解決する実装ならローカルファイル読み取りやネットワークアクセスが発生しうる。本レビューでは gpui_kit 側の SVG 実装を確認しておらず、エクスプロイトは未証明。緩和策: 使用中の gpui_kit / GPUI の SVG ラスタライザが外部参照を解決するか一次情報で確認し、解決するなら SVG をプレビュー対象外にするか、`<image>` / `<foreignObject>` / `href` を含む SVG を拒否するサニタイズを追加する。 | プレビュー機能を情報漏えい/SSRF の入口にしない。依存の挙動を明文化 | M | 新規 | 低 |
| r2-sec-11 | ブックマーク/タブ汚染で検索が機微ディレクトリへ誘導される | 調査項目 | search.rs:196-233 (`search_roots`), 319-403 (`search_contents`) / settings.rs:62-65 | 深刻度: Low-Medium。`search_roots` は `bookmarks` / `tabs` を `is_dir()` だけで採用し、ユーザーの意図した範囲かを検証しない。settings.json の `bookmarks` に `/etc` や `~/.ssh` を仕込んで「ブックマークを検索」を選ぶと、content search がその配下を再帰してファイル内容を読み、マッチ行を UI に出す。`include_hidden` が有効なら `.env` 等も対象。ローカル設定書き込み権限とユーザーの検索操作が必要なため単独では権限昇格にならない点を明記。緩和策: 検索ルートをホーム以下または現在ワークスペースに制限する設定を追加するか、機微ディレクトリ（`/etc`, `~/.ssh`, `~/.gnupg` 等）を検索対象外にする拒否リストを導入する。 | 検索機能が「意図しない機微データ閲覧」の近道にならないようにする | S | 新規 | 中 |
| r2-sec-12 | 設定ファイルが umask 依存のパーミッションで保存される | 残タスク | settings.rs:248-279（特に 263-274 の tmp 生成と 278 の rename） | 深刻度: Low。`save_to` は `OpenOptions::new().write(true).create_new(true).open(&tmp)` で一時ファイルを作り `fs::rename` するが、Unix で mode を指定していないため既定は `0666 & ~umask`（一般的な umask 022 なら 0644）。設定には `bookmarks` / `recent_dirs` / `recent_searches` が含まれ、ディレクトリ構造や検索語（パスワード碎片の可能性）が他のローカルユーザーから読める。`~/.config` 自体が 0700 の環境では実害は限られる点を明記。緩和策: `#[cfg(unix)] OpenOptionsExt::mode(0o600)` を指定し、既存 settings.json も必要なら 0600 に修正、親ディレクトリも 0700 を保証する。 | ローカル他ユーザーからの設定情報漏えいを防止。r2-sec-09 と相補的な硬化 | S | 新規 | 高 |

## レンズ総評

本レンズは「既に丁寧に書かれた安全策の**継ぎ目**」を狙った。特に価値が高いのは
r2-sec-01（リリースビルドで環境変数フックが死なない点）と r2-sec-04（設定配列の
上限が読み込み時に効かない点）で、どちらも数行〜十数行の修正で攻撃面を
実質的に潰せる。r2-sec-02 / 03 / 06 は典型的な TOCTOU だが、いずれも
「書き込み権限を持つ攻撃者が親ディレクトリを操作でき、かつ競合に成功する」
という条件が必要で、単一ユーザーの desktop ユースケースでは越境しない。
過大評価せず「調査項目」として残した。r2-sec-05 は PASS 1 の「100k 性能」と
混同されやすいが、再帰呼び出しによるスタック枯渇という別のクラッシュ経路であり、
反復化する価値が高い。r2-sec-10 は gpui_kit の SVG 実装を未確認のため
最も確度が低く、一次情報の確認が先である。全体として既存の「承認」判定を
覆す Critical / High の確定脆弱性は見つからなかった。
