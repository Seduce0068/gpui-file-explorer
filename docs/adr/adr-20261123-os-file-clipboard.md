---
type: ADR
title: "OS ファイルクリップボード相互運用の現状と実装方針"
description: "CF_HDROP / text-uri-list / 共有 Intent の対応状況をローカル源码で確認し、段階的な実装方針を記録する"
tags:
  - gpui
  - clipboard
  - file-transfer
  - cross-platform
  - adr
generated:
  by: "research-clipboard"
  at: "2026-11-23T01:23:50+09:00"
verified:
  by: "research-clipboard"
  at: "2026-11-23T01:23:50+09:00"
status: draft
---

## コンテキスト

NW10（[todo.md](../../todo.md) の ID `NW10`。行番号は更新で変わるため ID で参照）は
OS ファイルクリップボード相互運用を扱う。
対象は Windows の CF_HDROP、POSIX の `text/uri-list`（X11 / Wayland）、
Android / iOS の共有 Intent である。

本 ADR は 2026-11-23 時点でローカルに取得済みの源码を実読した結果を記録し、
推奨方針を示す。確定決定ではない。ユーザーの明示的な承認まで `status: draft` を維持する。
上流 gpui-pre の issue / PR の進行状況は今回の調査範囲外で、未確認である。

根拠とした既存調査は [既存 notes](../../notes/os-file-clipboard-notes.md) である。
ただし本 ADR の確認で、その結論の一部を補正する必要が生じた（後述）。

## 検証済み事実

### クリップボードはアプリ内部専用

- `app/src/main.rs::ClipboardSlot` は `paths: Vec<PathBuf>` と `cut: bool` だけを持つ。
  直上のコメントは、OS クリップボードの cut / copy 意味がプラットフォーム依存である
  ことと、ユーザーがコピーした内容を上書きしないことを理由に、意図的に使わないと述べる。
- 保持フィールドは `app/src/main.rs::Explorer::clipboard`
  （`clipboard: Option<ClipboardSlot>`）である。
- `app/src/main.rs::Explorer::copy_selected` は `ClipboardSlot { paths, cut: false }` を、
  `app/src/main.rs::Explorer::cut_selected` は `ClipboardSlot { paths, cut: true }` を設定する。
  どちらも OS へは書き出さない。
- `app/src/main.rs::Explorer::paste` は内部スロットを複製して開始する。競合時は常に
  `ConflictPolicy::Rename` を使い、既存ファイルを上書きしない。存在しなくなった元パスは
  除外し、カットで同一ディレクトリの項目も除外する。
- 転送後のスロット破棄は `app/src/main.rs::Explorer::paste` 内の `cut && fully_succeeded`
  の場合だけである。キャンセルや部分失敗ではスロットを残し、再試行できるようにする。
- 調査依頼に記載された行番号（5785 など）は現行ファイルと約 200 行ずれている。
  本 ADR は [app/src/main.rs](../../app/src/main.rs) のシンボル名を優先して参照する。
  行番号は as-built スナップショットで並行変更によりずれ得るため、`#L` アンカーは使わず、
  シンボル名で参照する（R43/AP02）。

### OS クリップボードへの書き込みはパス文字列 1 本だけ

- `app/src/main.rs::Explorer::copy_path` は次を呼ぶ。複数パスや `ExternalPaths` は渡さない。

  ```rust
  cx.write_to_clipboard(ClipboardItem::new_string(display_path(&path)));
  ```

- `app/src` を grep した結果、`read_from_clipboard` の呼び出しは 0 件である。
  OS クリップボードを読む経路は存在しない。

### 外部パスはドロップ経路でのみ扱う

- `ExternalPaths` の `on_drop::<ExternalPaths>` ハンドラは 3 か所にある
  （行番号は並行変更でずれ得るため、`app/src/main.rs` 内の
  `on_drop::<ExternalPaths>` を検索して特定する）。
- ハンドラは `drop_external_into(cwd, paths)` または `drop_external(paths)` を呼ぶ。
  `app/src/main.rs::Explorer::drop_external_into` は常にコピーかつ
  `ConflictPolicy::Rename` で、特殊ファイルや同一パスはスキップする。
- 調査依頼に記載された行番号（9807 / 10099 / 11214）も現行ファイルではずれている。

### GPUI 0.3.12 のクリップボード実装

型と API は存在する。
`gpui-pre-0.3.12/src/platform.rs:3143-3150` の `ClipboardEntry` は
`String` / `Image` / `ExternalPaths` の 3 つの variant を持つ。
`App::read_from_clipboard` と `write_to_clipboard` は
`gpui-pre-0.3.12/src/app.rs:1834` と `:1868` にある。

プラットフォームごとの実装をローカル源码で確認した結果は次のとおりである。

| プラットフォーム | 読み取り | 書き込み |
| --- | --- | --- |
| Linux Wayland | テキストと画像のみ。URI list の分岐なし | テキスト MIME のみ |
| Linux X11 | 画像とテキストのみ | `set_text` のみ |
| Windows | `CF_HDROP` → `ExternalPaths` | `ExternalPaths` は無視 |
| macOS | `NSFilenamesPboardType` → `ExternalPaths` | 無視 |
| iOS | 文字列のみ | 文字列のみ |

根拠となるローカル源码は次のとおりである（いずれも
`~/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/` 配下）。

- Wayland 読み取り: `gpui-pre-linux-0.3.12` の
  `src/linux/wayland/clipboard.rs:19,193-209`
- Wayland 書き込み: 同 `src/linux/wayland/client.rs:1462-1486`
- X11: `gpui-pre-linux-0.3.12` の `src/linux/x11/clipboard.rs:81,1026-1046`、
  `src/linux/x11/client.rs:1841-1853`
- Windows: `gpui-pre-windows-0.3.12` の
  `src/clipboard.rs:70-90,92-128,267-275`
- macOS: `gpui-pre-macos-0.3.12` の `src/pasteboard.rs:57-91,160-173`
- iOS: `gpui-pre-ios-0.3.12` の `src/ios/platform.rs:425-437`

Wayland の読み取りは次のコードだけである。

```rust
// gpui-pre-linux-0.3.12: src/linux/wayland/clipboard.rs:203-205
let item = offer
    .read_text(&self.connection)
    .or_else(|| offer.read_image(&self.connection))?;
```

Wayland の書き込みもテキスト系 MIME だけを offer する。

```rust
// gpui-pre-linux-0.3.12: src/linux/wayland/client.rs:1480-1483
for mime_type in TEXT_MIME_TYPES {
    data_source.offer(mime_type.to_string());
}
data_source.offer(state.clipboard.self_mime());
```

`FILE_LIST_MIME_TYPE = "text/uri-list"` は `wayland/clipboard.rs:19` で定義されるが、
クリップボードではなくドラッグ側（`wayland/client.rs:693-707`）と
ドロップ側（同 `:2929,3043`）で使われる。経路が異なる。

X11 は `x11/clipboard.rs:81` で `// URI_LIST: b"text/uri-list",` がコメントアウトされ、
`get_any` は画像とテキストの原子だけを読む。
`x11/window.rs:51` の `TextUriList` は XDND 用で、クリップボード用ではない。

Windows は読み取りに対応している。

```rust
// gpui-pre-windows-0.3.12: src/clipboard.rs:115-119
} else if !have_files && format == CF_HDROP.0 as u32 {
    if let Some(entry) = read_files() {
        entries.push(entry);
        have_files = true;
    }
}

```rust
fn read_files() -> Option<ClipboardEntry> {
    let locked = get_clipboard_data(CF_HDROP.0 as u32)?;
    // ... DragQueryFileW でパスを列挙 ...
    Some(ClipboardEntry::ExternalPaths(ExternalPaths(filenames.into())))
}
```

一方、書き込みは `src/clipboard.rs:81` の
`ClipboardEntry::ExternalPaths(_) => {}` で何もしない。
macOS も `src/pasteboard.rs:64,81` でファイルパスの読み取りを行い、
`:172` の `[ClipboardEntry::ExternalPaths(_)] => {}` で書き込みを無視する。

`gpui-pre-windows-0.3.12` を grep しても `Preferred DropEffect` に関係する処理は 0 件で、
OS 側の cut / copy 区別を GPUI API から取得できない。
iOS は `open_with_system` が `ios/platform.rs:346-348` でコメントのみのスタブで、
共有 Intent の実装は無い。ローカルの crate 一覧に Android 用 gpui-pre バックエンドも存在しない。

### 既存調査との食い違い

[RESEARCH.md](../../RESEARCH.md) §4.7（ドラッグ&ドロップ／クリップボード）は
「Wayland は `text/uri-list` と独自 MIME を実装済み」と記載するが、確認した限りその MIME は
ドラッグ＆ドロップの送受信で使われ、クリップボードの読み書きには使われていない。
[RESEARCH.md](../../RESEARCH.md) §4.7（ドラッグ&ドロップ／クリップボード）の D&D の記述と混同した可能性がある。
本 ADR では実読したソースコードを根拠にする。

## 選択肢

### 選択肢 A: 上流の `ClipboardEntry::ExternalPaths` を活用する

`read_from_clipboard` が `ExternalPaths` を返す Windows / macOS で、
そのパスを `drop_external_into(cwd, paths)` と同じコピー専用パイプラインへ流す。
Linux は上流が Wayland / X11 のクリップボードに `text/uri-list` の読み書きを追加するまで待つ。
書き込みは全プラットフォーム未対応のため、当面は「パスをコピー」のテキスト出力を維持する。
OS 側でカットされた項目もコピーとして扱い、元ファイルは削除しない。

新規依存と `unsafe` を追加せず、既存の安全パイプラインを再利用できる。
実装量が最も小さく、上流の API 変更にも追従しやすい。

### 選択肢 B: 直接クリップボード実装を行う

X11 では `text/uri-list` と `x-special/gnome-copied-files`（先頭行が `copy` または `cut`）を
扱う selection 実装、Wayland では data source / data offer の MIME 拡張、
Windows では `CF_HDROP` の生成と `Preferred DropEffect` による Copy / Move の区別が必要になる。

いずれも FFI と `unsafe` を伴う。AGENTS.md §15 の禁止事項に加え、
[todo.md](../../todo.md) の ID `NF02`（Windows 実ドライブ列挙の FFI 実装）と
同種のレビュー合意が必須である。
合意がない限り実装しない。

### 選択肢 C: 内部専用を維持する

現状のまま、制約をドキュメントに明記する。相互運用の要件は満たさないが、
安全性と実装量の面では最小である。選択肢 A が止まった場合の既定状態として残す。

### 選択肢 D: ドロップのみで代替する

既存の `on_drop::<ExternalPaths>` 経路だけを使う。ファイルマネージャからドラッグすれば
ファイルを取り込めるが、クリップボードの Ctrl+C / Ctrl+V を求める要件は満たさない。
単独の解決策ではなく、補助的な代替として扱う。

## 推奨決定

選択肢 A を今すぐ進める。具体的には、Windows / macOS で既に利用できる
`ClipboardEntry::ExternalPaths` の読み取りをアプリ側で受け取り、
既存の `drop_external_into` パイプラインへ接続する。
選択肢 B は、NF02 と同形式の `unsafe` レビュー合意が得られた場合に限り、
Linux のギャップを埋める手段として検討する。
選択肢 C は保留時の既定状態、選択肢 D は補助的な代替とする。

Android / iOS の共有 Intent は本 ADR の範囲外とし、別の ADR で設計する。
ローカルに Android 用バックエンドが無く、iOS も共有機能のスタブしか無いことが根拠である。

## 結果とリスク

- 新規依存と `unsafe` を追加せずに、Windows / macOS で OS コピーしたファイルの
  貼り付けを実現できる。
- 内部スロットと OS クリップボードの優先順位を決める必要がある。
  内部スロットが有効な間は内部を優先し、空のときだけ OS を読む方針を推奨する。
  これにより `STATUS_NO_CLIPBOARD` の既存挙動を変えずに済む。
- OS 側でカットした項目もコピーとして扱うため、移動は行われない。
  cut / copy の区別は選択肢 B の承認後に検討する。
- 上流 API は変更され得る。gpui-pre 0.3.12 の源码を実読した記録を残し、
  バージョン更新時に再確認する。

## 段階的な受け入れ基準

### フェーズ 0: 現状の固定

- [ ] 本 ADR と既存 notes の記述がローカル源码と一致すること。
- [ ] 内部のコピー / カット / 貼り付けと D&D の既存動作が変わらないこと。

### フェーズ 1: Windows / macOS の読み取り

- [ ] `read_from_clipboard` の `ExternalPaths` を検出し、
      `drop_external_into(cwd, paths)` と同じ経路へ流すこと。
- [ ] 同名ファイルは上書きせず、`Rename` で退避すること。
- [ ] OS 側でカットされた項目もコピーとして扱い、元ファイルを残すこと。

### フェーズ 2: Linux

- [ ] 上流が Wayland / X11 クリップボードの URI list 読み取りを
      入れたバージョンで、ファイルマネージャからコピーしたファイルを
      Ctrl+V で取り込めること。
- [ ] 未対応のバージョンでは内部スロットのみが動き、異常終了しないこと。

### フェーズ 3: 書き出しと cut / copy 区別（承認後）

- [ ] 選択したファイルを OS クリップボードへファイル一覧として書き出せること。
- [ ] `x-special/gnome-copied-files` の先頭行と Windows の
      `Preferred DropEffect` を解釈し、カットを移動として扱えること。
- [ ] 選択肢 B を実装する場合、NF02 形式の `unsafe` レビュー合意記録が存在すること。

### フェーズ 4: モバイル共有（別 ADR）

- [ ] Android の ACTION_SEND / `content://` URI と iOS の
      `UIActivityViewController` が別 ADR で調査・合意されていること。
      本 ADR では実装しない。

## 読み手から見える挙動

- 現状: Ctrl+C / Ctrl+X / Ctrl+V はアプリ内だけで完結し、OS クリップボードを上書きしない。
  「パスをコピー」は 1 つのパスを文字列として OS クリップボードへ置く。
  ファイルマネージャからドロップしたファイルはコピーとして取り込む。
- フェーズ 1 完了後（Windows / macOS）: 内部スロットが空のとき、
  OS でコピーしたファイルが Ctrl+V でカレントディレクトリへコピーされる。
  同名ファイルは Rename で退避し、ステータスバーに件数を表示する。
  OS 側のカット属性は無視してコピーする。
- Linux: 上流が対応するまで挙動は変わらない。D&D は従来どおりコピーとして動く。
- どのフェーズでも、元ファイルの削除と上書きは行わない。

## 参照

- [notes/os-file-clipboard-notes.md](../../notes/os-file-clipboard-notes.md)
- [todo.md](../../todo.md) の ID `NW10`（OS クリップボード調査と ADR）、
  ID `NF02`（Windows 実ドライブ列挙の FFI 実装）
- [RESEARCH.md](../../RESEARCH.md) §4.7（ドラッグ&ドロップ／クリップボード）（D&D とクリップボードの混同に注意）
- ローカル取得源码: `gpui-pre-0.3.12`、`gpui-pre-linux-0.3.12`、
  `gpui-pre-windows-0.3.12`、`gpui-pre-macos-0.3.12`、`gpui-pre-ios-0.3.12`
  （`~/.cargo/registry/src/index.crates.io-*/`）
