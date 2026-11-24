---
type: Reference
title: "E2E 合成入力ツールの導入・プローブ実測記録"
description: >-
  setup-e2e-input.sh の実行結果と ldd / スモークテストの終了コード、
  ならびに KWin 環境での入力到達性に関する観測記録。
tags:
  - e2e
  - wayland
  - kwin
  - synthetic-input
  - runtime-proof
generated:
  by: "fix-e2e-docs-r20"
  at: "2026-11-19T07:08:40+09:00"
status: "draft"
---

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

## 概要

`scripts/setup-e2e-input.sh` を実機で実行し、インストール結果・動的リンク・
スモークテスト・入力到達性を確認した記録です。ドキュメント側の記述矛盾
（`app/README.md` の「ツールが無い」 vs 「導入済みだが届かない」）を解消する
ための Runtime Proof として扱います。

- 実行日時: 2026-11-19 07:05〜07:09 JST
- 環境: Kubuntu 2026.04 / KDE Plasma 6 / Wayland
  - `DISPLAY=:1`、`WAYLAND_DISPLAY=wayland-0`、`XDG_SESSION_TYPE=wayland`
- 変更範囲: スクリプトの出力先 `~/.local/e2e-tools/` のみ。
  リポジトリへの変更は許可された 3 ファイルと本ノートに限定。
- root / udev 操作は一切実施していません（`/dev/uinput` は `root:root 0600` のまま）。

## 静的チェック

| コマンド | 終了コード | 結果 |
| --- | --- | --- |
| `bash -n scripts/setup-e2e-input.sh` | 0 | 構文エラーなし |
| `shellcheck scripts/setup-e2e-input.sh` | 0 | 警告なし |
| `shfmt -d -i 2 scripts/setup-e2e-input.sh` | 0 | 差分なし（既存の 2 スペース風格） |

## セットアップスクリプトの実行

コマンド: `bash scripts/setup-e2e-input.sh` — **終了コード 0**。
主要出力は次のとおりです（3 ツールは前回実行時に導入済みのため再ダウンロードは発生しませんでした）。

```text
wtype: already installed in /home/testuser/.local/e2e-tools/bin
xdotool: already installed in /home/testuser/.local/e2e-tools/bin
ydotool: already installed in /home/testuser/.local/e2e-tools/bin

=== [A] Binary-install / link check (expected: PASS) ===
ldd OK   : wtype (all shared libraries resolved)
ldd OK   : xdotool (all shared libraries resolved)
ldd OK   : ydotool (all shared libraries resolved)

=== Smoke tests (binary invocations, NOT input delivery) ===
smoke OK   : xdotool --version (exit 0)
             xdotool version 3.20160805.1
smoke WARN : wtype --version (best effort) (exit 1)
             Missing argument to --version
smoke OK   : ydotool help (exit 0)
             Usage: ydotool <cmd> <args>

=== Shell setup ===
  export PATH="/home/testuser/.local/e2e-tools/bin:${PATH}"
```

## 個別スモーク / プローブの観測値

| コマンド | 終了コード | 観測出力 | 意味 |
| --- | --- | --- | --- |
| `xdotool --version` | 0 | `xdotool version 3.20160805.1` | バイナリ導入・実行 OK |
| `wtype --version` | 1 | `Missing argument to --version` | 導入失敗ではない |
| `ydotool help` | 0 | `Usage: ydotool <cmd> <args>` | バイナリ導入・実行 OK |
| `ldd .../xdotool \| grep 'not found'` | 1 | 出力なし | 依存解決済み（grep 非一致で 1） |
| `xdotool getdisplaygeometry` | 0 | `2560 1440` | Xwayland 側の読み取りクエリは成功 |
| `xdotool getactivewindow` | 0 | `2097152` (`0x200000`) | 同上。X11 側のクエリ成功を示すのみ |

`wtype -k a` と `ydotool key F1` は、フォーカス中の任意ウィンドウへ実際のキーが
入る可能性があるため**実行していません**。KWin は
`zwp_virtual_keyboard_v1` 非対応、`/dev/uinput` は `root:root 0600` であり、
Wayland ネイティブのアプリへ到達しないことは環境上既知です。

## 解釈と注意点

- 3 ツール（`wtype` / `xdotool` / `ydotool`）は
  `~/.local/e2e-tools/bin` に導入済みです。利用には
  `export PATH="$HOME/.local/e2e-tools/bin:$PATH"` が必要です。
- 今回のホストでは `libxdo3`（`1:3.20160805.1-5.1build1`）がシステム側に
  インストール済みで、`ldd` は全バイナリについて依存解決済みでした。
  したがってスクリプトの `libxdo3` 抽出フォールバックは発火していません
  （未検証の経路として残ります）。
- `xdotool getactivewindow` が成功しても、キーが Wayland ネイティブの
  `gpui-file-explorer` へ届くことは意味しません。XTEST は Xwayland 側の
  クライアントにしか効かず、「Control input devices」ポータル許可も未承認です。
- `wtype --version` の終了コード 1 はバージョンフラグ非対応によるもので、
  `ldd OK` がインストール成否の権威ある判定です。

## 参照

- [scripts/setup-e2e-input.sh](../scripts/setup-e2e-input.sh)
- [app/README.md の「既知の制限」](../app/README.md)
- [README.md の「既知の制限」](../README.md)
