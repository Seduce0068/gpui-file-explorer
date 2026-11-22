# OS ファイルクリップボード連携の調査（2026-11-14）

- 目的: OS（Dolphin 等）でコピーしたファイルを本アプリの `Ctrl+V` で貼り付け、
  逆に本アプリの選択を OS のファイルクリップボードへ書き出すことは可能か
- 検証環境: Kubuntu 2026.04 / Wayland ネイティブ / gpui-pre 0.3.12
- 結論: **GPUI 0.3.12 の Linux バックエンドはクリップボードのファイル一覧
  （`text/uri-list`）を読み書きしないため、現状では実装不可。**
  GPUI 側の拡張（または別 crate でのプラットフォーム実装）が必要。

## 根拠（取得 crate のソース確認）

### 1. 型は存在する

`gpui-pre` の `platform.rs` にはファイルパス用の型がある:

```rust
pub enum ClipboardEntry {
    String(ClipboardString),
    Image(Image),
    ExternalPaths(crate::ExternalPaths),
}
```

### 2. Wayland の読み取りは text/image のみ

`gpui-pre-linux-0.3.12/src/linux/wayland/clipboard.rs`:

```rust
let item = offer
    .read_text(&self.connection)
    .or_else(|| offer.read_image(&self.connection))?;
```

`text/uri-list` を読む分岐は無い。`FILE_LIST_MIME_TYPE` 定数は定義されているが、
Wayland のクリップボード実装内で**未使用**（grep で定義1箇所のみ）。

### 3. Wayland の書き込みはテキスト MIME のみを offer

`gpui-pre-linux-0.3.12/src/linux/wayland/client.rs` の `write_to_clipboard`:

```rust
for mime_type in TEXT_MIME_TYPES {
    data_source.offer(mime_type.to_string());
}
data_source.offer(state.clipboard.self_mime());
```

`text/uri-list` を offer しないため、Dolphin 等へ「ファイルのコピー」として渡せない。

### 4. X11 も未対応

`gpui-pre-linux-0.3.12/src/linux/x11/clipboard.rs` では URI_LIST がコメントアウトされている:

```rust
// URI_LIST: b"text/uri-list",
```

### 5. ドラッグ&ドロップは別経路で対応済み

`wl_data_device` のドロップ（`client.rs` の `file_list` 読み取り）は実装されており、
本アプリの D&D 機能はこれを利用している。クリップボードとは経路が異なる。

## 代替・将来対応

1. **現状の代替**: 右クリック「パスをコピー」でテキストとしてパスを OS クリップボードへ
   書き出すことは可能（実装済み）。テキストとして貼り付ける用途には使える。
2. **本格対応する場合**:
   - gpui-pre-linux に `text/uri-list` の read/write を追加するパッチを upstream へ提案
   - あるいは `wl-clipboard-rs` 等を直接使う別経路を実装（依存追加・ユーザー承認が必要）
3. 本アプリ側は `ClipboardEntry::ExternalPaths` を受けた時点で
   `drop_external_into(cwd, paths)` と同じ安全パイプライン（常にコピー・Rename）へ
   流す設計にすれば、GPUI 側が対応した際の実装量は小さい。

## 参考

- `gpui-pre-linux-0.3.12/src/linux/wayland/clipboard.rs`（read_text/read_image のみ）
- `gpui-pre-linux-0.3.12/src/linux/wayland/client.rs`（write_to_clipboard の MIME offer）
- `gpui-pre-linux-0.3.12/src/linux/x11/clipboard.rs`（URI_LIST コメントアウト）
- `gpui-pre-0.3.12/src/platform.rs`（ClipboardEntry::ExternalPaths の型定義）

## 2026-11-23 時点のステータス

詳細は [ADR](../docs/adr/adr-20261123-os-file-clipboard.md)（status: draft）を参照。

- ローカル取得源码を再読した結果、Linux の結論は変わらない。
  Wayland / X11 ともクリップボードの `text/uri-list` を読み書きしない。
  `FILE_LIST_MIME_TYPE` は D&D の送受信で使われ、クリップボードとは経路が別。
- ただし Windows は `CF_HDROP`、macOS は `NSFilenamesPboardType` の
  読み取りが gpui-pre 0.3.12 で実装済み（`ClipboardEntry::ExternalPaths` を生成）。
  書き込みはいずれのプラットフォームも未対応。
- iOS は `UIPasteboard` の文字列のみ。Android 用 gpui-pre バックエンドは
  ローカルに存在しない。共有 Intent は別 ADR の対象。
- Windows の `Preferred DropEffect` はローカルの gpui-pre-windows に未実装で、
  cut / copy の区別は GPUI API からは得られない。
- 推奨方針: 選択肢 A（上流 API の読み取りを活用）を先行。
  選択肢 B（直接実装）は NF02 と同形式の承認後のみ。
