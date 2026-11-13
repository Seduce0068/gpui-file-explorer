# GPUI ファイルエクスプローラー: ファイルシステム / デスクトップ統合 クロスプラットフォーム調査ノート

- 調査日: 2026-11-11 (Asia/Tokyo)
- 対象: Linux (Kubuntu 2026.04 / KDE Plasma 6 / Wayland + X11) を第一、Windows 11、macOS 15+ をクロスプラットフォーム対象
- 調査方法: crates.io API (`curl -A 'research-agent/1.0' https://crates.io/api/v1/crates/<name>`) で現行版・ライセンスを確認し、主要 crate は `.crate` を取得してソース・API・実装を実読。ローカル環境の inotify 上限を実測。公式仕様 (freedesktop / Microsoft Learn / Apple Developer) は URL を一次資料として参照。
- 表記: ✅ = 本調査でソース/実測により確認済み、⚠ = 未確認・要 PoC 検証、📝 = 設計判断。
- 関連: GPUI 本体 API は別ノート `api-notes`（api-scout 担当）。本ノートは OS・ドメイン crate 側の証拠に限定。

---

## 0. 要点サマリ（10 項目）

1. **一覧取得は `std::fs::read_dir` + `DirEntry::file_type()` が最速**。Linux は getdents の `d_type`、Windows は `FindFirstFileW` が返す `WIN32_FIND_DATA.dwFileAttributes` を std がキャッシュするため、`file_type()` に追加 syscall は不要（`DT_UNKNOWN` の FS のみ lstat フォールバック）。サイズ/更新時刻列が必要なときだけ `metadata()` を並列取得。100k 件は仮想リスト + 分割ロード。
2. **`jwalk` は 0.9.0 の README に「Unmaintained。`dua-core` を使え」と明記**されており不採用。再帰検索は ripgrep 系の `ignore` 0.4.34（並列 walker）を使う。
3. **監視は `notify` 8.2.2 + `notify-debouncer-full` 0.7.0**（0.7.0 は `notify ^8.2.0` 依存を crates.io メタデータで確認）。Linux inotify の watch 上限到達は `ErrorKind::MaxFilesWatch`、キュー溢れは `EventKind::Other` + `Flag::Rescan` として通知される（ソース確認）。FSEvents も MustScanSubDirs 等で Rescan を発火。
4. **watch 範囲は「表示中ディレクトリ（非再帰）+ 親ディレクトリ（非再帰）」**。デバウンス 250〜500ms、自操作の抑制窓、5〜10 秒の定期リコンサイル、失敗時は `PollWatcher` フォールバック。ネットワーク FS は notify 公式 README に既知問題あり。
5. **アイコン**: Linux は `freedesktop-icons` 0.4.0（MIT、icon-theme-spec 準拠・SVG/scale 対応）。Windows は shell API（`SHGetFileInfoW` / `SHGetImageList(SHIL_JUMBO)` / サムネイルは `IShellItemImageFactory`、COM STA）。macOS は `NSWorkspace.iconForFile`（`objc2-app-kit` 0.3.2）。必ず同梱の拡張子→SVG フォールバックを持つ。
6. **MIME/サムネイル**: 拡張子判定は `mime_guess` 2.0.5、内容判定は `infer` 0.22.0、画像サムネイルは `image` 0.25.10（デコード上限 + EXIF orientation）。動画サムネイルは MVP 外（ffmpeg 依存が重い。後で OS サムネイルエンジン委譲）。
7. **ボリューム**: MVP は `sysinfo` 0.40.1 の `Disks` で列挙 + 空き容量 + 数秒ポーリング。ホットプラグは後回し（Linux: udisks2 over D-Bus `zbus` 6.2.1 / Windows: `WM_DEVICECHANGE` / macOS: `NSWorkspace` mount 通知）。
8. **ファイル操作**: 同一 FS は `std::fs::rename`、`EXDEV` は copy+delete。削除は `trash` 5.3.0（Windows Recycle Bin / macOS Trash / freedesktop Trash spec、`list`/`restore_all`/`purge_all` も提供）。大量コピーは自前再帰 + 進捗/キャンセル + `create_new` 衝突回避。
9. **OS 差の落とし穴**: Windows 長パス（`dunce` 1.0.5 + longPathAware + `\\?\`）、macOS NFD（`unicode-normalization` 0.1.26）、Linux 非 UTF-8 `OsStr`、大小文字のみ rename、予約名/禁止文字、read-only/immutable、シンボリックリンク/ジャンクション。
10. **デスクトップ統合/クリップボード**: オープン/リビールは `opener` 0.9.0（`reveal` feature、Linux は D-Bus `org.freedesktop.FileManager1.ShowItems`、Windows `explorer /select,`、macOS `open -R` をソース確認）。ファイルリストのコピー/貼り付けは `arboard` 3.7.0 が Linux(X11+Wayland)/Windows/macOS 対応（`file_list` get/set 実装確認）。GPUI 側は `ClipboardEntry::ExternalPaths` と `FileDropEvent` が公開 API に存在（gpui-pre 0.3.9 で確認）。ドラッグアウトは OS 実装コスト大のため Later。

---

## 1. ディレクトリ列挙・性能・ソート

### 1.1 推奨 API（MVP）

| 用途 | 推奨 | 版 | 備考 |
|---|---|---|---|
| 単一ディレクトリ列挙 | `std::fs::read_dir` + `DirEntry::file_type()` | std | 追加 stat 無し。名前は `OsString` のまま保持 |
| メタデータ列（サイズ/時刻） | `DirEntry::metadata()` をスレッドプールで並列 | `rayon` 等 | 100k 件では head-of-line を避け分割ロード |
| 再帰検索 | `ignore` の `WalkBuilder::build_parallel()` | 0.4.34 | 4 並列以上、`standard_filters(false)` で `.gitignore` を切る |
| 自然順ソート | `alphanumeric-sort` `compare_str` | 1.5.8 | `sort_by_cached_key` でキー事前計算 |
| ロケール照合 (Later) | `icu_collator` / OS API | 2.4.0 | 下記 1.4 |
| 名前フィルタ | 自前の小文字化 contains → 後で fuzzy | — | `nucleo-matcher` は Zed 製。版は未確認 ⚠ |

- Linux: glibc の `readdir`/`getdents64` が `d_type` を返す。`DT_UNKNOWN`（NFS/一部 FUSE）のときだけ std が `lstat` する ✅（std の `DirEntry::file_type` の仕様）。
- Windows: std の Windows 実装は `FindFirstFileW/FindNextFileW` の `WIN32_FIND_DATAW` を使い、`file_type()` は属性をキャッシュ参照する ✅（追加の `GetFileAttributesEx` は不要）。
- **巨大ディレクトリ (100k)**: `read_dir` のストリーミングを回しつつ、UI は最初の ~2,000 件で描画を開始し残りをバックグラウンドで追加。1 件あたり ~200B なら 100k で ~20MB。件数上限（例: 50,000）で打ち切って「もっと表示」を出す設計も可 📝。
- 並列化はディレクトリ単位ではなく **エントリ単位の metadata 取得**に効かせる（`read_dir` 自体は単一スレッドで十分速い）。GPU/UI をブロックしないため GPUI の background executor 上で実行する（API 名は api-notes）。

### 1.2 jwalk / ignore の状況

- `jwalk` 0.9.0 (MIT): README 冒頭に **「This crate is no longer maintained or supported. Use `dua-core` instead.」** ✅。採用しない。
- `ignore` 0.4.34 (Unlicense OR MIT): `WalkBuilder` に `build()`, `build_parallel()`, `threads(n)`, `max_depth()`, `standard_filters(bool)`, `hidden(bool)`, `same_file_system(bool)` を確認 ✅。**エクスプローラ用途では `.gitignore` 尊重が既定 ON なので `standard_filters(false)` + 自前の隠しファイル判定にする**。
- `walkdir` 2.5.0: 単純再帰が必要なときの代替（2024-03 更新で安定）。

### 1.3 ソート

- **MVP**: 名前（natural sort、大小無視）+ 種類/サイズ/更新時刻。`alphanumeric-sort` 1.5.8 の `compare_str`、`sort_slice_by_str_key` を確認 ✅。
- 落とし穴: natural sort は文字列を走査するため比較コストが高い。100k 件では **ソートキー（小文字化済み etc.）を事前計算**してから比較する。
- `natord` 1.0.9 は 2015 年から更新停止 ✅（crates.io 更新日）、`human-sort` 0.2.2 は 2019 年で停滞 ✅ → 新規採用は `alphanumeric-sort`。
- ディレクトリ/ファイル混在は「フォルダ先」設定、`..` は常に先頭。拡張子ソートは `Path::extension` を小文字化したもの＋自然順。

### 1.4 ロケール対応照合 (Later)

- Windows: `CompareStringEx(LOCALE_NAME_USER_DEFAULT, ..., SORT_DIGITSASNUMBERS)` を使うと OS の自然順＋ロケール照合が得られる（`SORT_DIGITSASNUMBERS` は数字を数値として比較）。
- macOS: `NSString.localizedStandardCompare:` が Finder 相当の照合。
- Linux: glibc の `strcoll` はプロセスロケール依存で GUI には不向き。`icu_collator` 2.4.0（ICU4X、ライセンス Unicode-3.0）か `rust_icu` 5.8.0（ICU-C への FFI、システム ICU 必須）。
- **MVP ではロケール照合しない**（natural sort のみ）。ファイル名の照合差はユーザー要望が出てから。

### 1.5 プラットフォーム・マトリクス（列挙）

| 項目 | Linux | Windows | macOS |
|---|---|---|---|
| 高速列挙 | getdents64 + d_type ✅ | FindFirstFileW キャッシュ ✅ | getdirentries + d_type 相当 ✅ |
| 100k 件 | 仮想リスト + 分割 | 同左（SMB は遅い） | 同左 |
| 隠しファイル判定 | 先頭 `.` | `FILE_ATTRIBUTE_HIDDEN`/`SYSTEM` | 先頭 `.` + `UF_HIDDEN` (chflags hidden) |
| 非 UTF-8 名 | あり得る（OsStr bytes） | 実質 UTF-16（WTF-8 経由） | NFD 正規化済み文字列 |
| 大文字小文字 | 基本区別（ext4） | 区別しない | 既定は区別しない (APFS) |

---

## 2. ファイル監視 (notify)

### 2.1 推奨構成

```toml
notify = "8.2.2"
notify-debouncer-full = "0.7.0"   # notify ^8.2.0 に依存（crates.io メタデータで確認）
```

```rust
use std::time::Duration;
use notify_debouncer_full::{new_debouncer, DebounceEventResult, notify::RecursiveMode};

let mut debouncer = new_debouncer(Duration::from_millis(300), None, |res: DebounceEventResult| {
    match res {
        Ok(events) => { /* channel へ送る */ }
        Err(errors) => { /* 監視エラー: MaxFilesWatch 等 → フォールバック */ }
    }
})?;
debouncer.watch(&current_dir, RecursiveMode::NonRecursive)?;
debouncer.watch(current_dir.parent().unwrap(), RecursiveMode::NonRecursive)?;
```

- `notify` 8.2.2 の API: `recommended_watcher(handler)`, `Watcher::watch(path, RecursiveMode)`, `Config::default().with_follow_symlinks(false)`, `Event { kind, paths, attrs }`, `EventKind`（`Access/Create/Modify/Remove/Any/Other`）, `ModifyKind::Name(RenameMode::{From,To,Both,Any,Other})` ✅（ソース確認）。シンボリックリンク追従は既定で無効化推奨（gpui-component も `with_follow_symlinks(false)` を使用 ✅）。
- `notify-debouncer-full` 0.7.0 の効果（README 記載 ✅）: リネーム From/To の結合、複数 Rename のマージ、未 emit イベントのパス更新、FS の file id 追跡（macOS/Windows）、ディレクトリ削除の 1 イベント化、重複 Create/Modify の抑制。`new_debouncer(timeout, tick_rate, handler)` と `debouncer.watch(...)` を確認 ✅。`FileIdMap`/`RecommendedCache` を re-export。

### 2.2 バックエンド差（ソース確認）

| | Linux (inotify) | macOS (FSEvents) | Windows (ReadDirectoryChangesW) |
|---|---|---|---|
| notify 実装 | `inotify` 0.11 + `mio` ✅ | `fsevent-sys`（既定 feature `macos_fsevent`） ✅ | `windows` crate + 自前スレッド ✅ |
| 再帰 | notify がサブディレクトリを walk して **ディレクトリごとに watch 追加** | FSEvents ストリーム自体が再帰 | `bWatchSubtree` で一括 |
| 上限/溢れ | `ENOSPC` → **`ErrorKind::MaxFilesWatch`**（ソース確認）。キュー溢れ → **`EventKind::Other` + `Flag::Rescan`** を emit（`inotify.rs` 実装確認） | `MUST_SCAN_SUBDIRS` / `USER_DROPPED` / `KERNEL_DROPPED` を検出し `Flag::Rescan` を emit（`fsevent.rs` 確認） | 要求バッファ `BUF_SIZE = 16384` 固定（`windows.rs` 確認）。**`ERROR_NOTIFY_ENUM_DIR`（バッファ溢れ）の明示処理はソースに見当たらず ⚠ → 取りこぼし前提のリコンサイル必須** |
| 既知の弱点 | watch 数上限、疑似 FS (/proc,/sys) はイベント無し | 所有者でないファイルのイベント漏れ、遅延/合体 | ネットワーク越しの遅延、バッファ溢れ |

- notify README の既知問題（✅ 原文確認）: **ネットワークマウント（NFS 等）はイベントが来ないことがある → `PollWatcher` 回避策**。WSL で Windows パスを watch する既知 issue #254。/proc,/sys は `PollWatcher` + `compare_contents`。親ディレクトリ削除のイベントは親を watch しないと取れない（issue #403）→ 本設計で親 watch する根拠。
- **ローカル実測値 (Kubuntu 2026.04, 2026-11-11)** ✅:
  - `/proc/sys/fs/inotify/max_user_watches` = **518185**
  - `max_user_instances` = **1024**
  - `max_queued_events` = **16384**
  - 注: inotify の watch は **ディレクトリ単位**で消費する（再帰 watch はサブディレクトリ数分）。notify の README は「ファイルもカウント」と書くが、少なくとも inotify バックエンドではファイル個別 watch はしない（📝 誤解注意）。`.cargo`/`target` 等の巨大ツリーを再帰 watch すると上限に達しやすい。

### 2.3 エクスプローラ向け監視設計（推奨）

1. 現在地を `NonRecursive` で watch（一覧の変化だけ欲しいので再帰不要）。加えて **親ディレクトリを `NonRecursive`** watch（カレントの rename/delete を検知）。
2. ナビゲーション時に旧 watch を drop → 新規作成。`debouncer` は 1 個を使い回し、`unwatch` する（0.7 は `Watcher::unwatch` 提供）。
3. イベントは `crossbeam-channel`/`std::sync::mpsc` で OS スレッドから受け、GPUI の背景エグゼキュータ経由で UI 更新（`cx.spawn`/`background_executor`）。
4. **自操作の抑制**: 操作（rename/copy/delete）開始時に「対象パス + 抑制期限（+1.5s）」を記録し、その間の該当イベントは無視。これが無いと一覧がチラつく/二重リロードになる。
5. **リコンサイル**: 10 秒タイマーで `read_dir` を実行し差分（名前/サイズ/mtime）だけ反映。notify の取りこぼし・Windows バッファ溢れの保険。ハッシュ/軽量比較で UI を更新。
6. **Rescan 受信時**: `EventKind::Other` + `Flag::Rescan` なら即 `read_dir` 全再読込。
7. **監視失敗時**（`MaxFilesWatch`, ネットワーク FS 判定）: `PollWatcher`（2 秒）へフォールバック + ステータスバーに「自動更新が制限されています」表示。
8. 表示ディレクトリが `read_dir` できない（権限なし）: watch を作らずエラー行を表示。

### 2.4 デバウンス戦略

- タイムアウト 250〜500ms、tick rate は `None`（0.7 の第 2 引数）。ファイルコピー中はイベントが連続するため、**「同一パスのイベントは 500ms 静止後に 1 回」**で十分。
- 大規模コピー中はデバウンスを 1〜2 秒に自動延長（イベント洪水で UI が固まるのを防ぐ）📝。

### 2.5 gpui-component の notify 利用状況（確認済み）

- `gpui-component` 0.9.1 の Cargo 依存に `notify ^8.2.0` がある ✅（crates.io メタデータ）。
- しかしソース上の利用は **`src/theme/registry.rs` のテーマ JSON ホットリロードのみ**（`notify::RecommendedWatcher::new` + `Config::default().with_follow_symlinks(false)` + `watcher.watch(&themes_dir, RecursiveMode::NonRecursive)`）✅。**ファイル一覧の監視機能は無い** → エクスプローラ側で自前実装が必要。
- なお `gpui-component` は `gpui-pre =0.3.12` に依存 ✅（0.9.1, 2026-11-09 更新）。GPUI 本体（`gpui` 0.2.2 / `gpui-pre` 0.3.x 系）のバージョン整合は api-scout のノートを参照。

---

## 3. アイコン・サムネイル

### 3.1 crate 選定

| 対象 | 推奨 | 版 | 検証/備考 |
|---|---|---|---|
| Linux アイコンテーマ検索 | `freedesktop-icons` | 0.4.0 | API: `lookup(name).with_size(48).with_scale(2).with_theme("breeze").with_cache().find() -> Option<PathBuf>` ✅。MIT。`with_cache()` は内部キャッシュ。`force_svg()` あり ✅ |
| （代替）テーマ探索を自前 | `xdg`/自前 `index.theme` パーサ | — | `icondir` という crate は **存在しない**（crates.io API 404 で確認） ✅ |
| 拡張子→MIME | `mime_guess` | 2.0.5 | MIT。最終更新 2024-06 だがマッピングは安定。`from_path()`/`first_or_octet_stream()` |
| 内容ベース MIME | `infer` | 0.22.0 | MIT。先頭バイトのマジック判定。拡張子なし/不正時のみ使用 |
| 画像デコード/サムネイル | `image` | 0.25.10 | MIT OR Apache-2.0。`ImageReader::with_guessed_format` + `Limits` |
| Windows shell | `windows` | 0.62.2 | MIT OR Apache-2.0。`SHGetFileInfoW`, `SHGetImageList`, `IShellItemImageFactory` |
| macOS AppKit | `objc2-app-kit` / `objc2-foundation` | 0.3.2 / 0.3.2 | MIT。`NSWorkspace`, `NSImage`, `NSFileManager` |
| SVG ラスタライズ | `resvg`/`usvg` | 0.45.1 系 | gpui-component が `resvg 0.45.1` に依存 ✅。GPUI 側に SVG 描画経路あり（api-notes 参照） |

### 3.2 OS 別アイコン取得

- **Linux**: `freedesktop-icons` でテーマ解決（`$XDG_DATA_HOME/icons`, `$XDG_DATA_DIRS/icons`, `~/.icons` を icon-theme-spec 順に探索）。KDE Plasma 6 は Breeze/Breeze-Dark、GTK アプリは Adwaita 等。**SVG が多い**ため、GPUI 側で SVG をラスタライズできるか確認（PoC で検証）。PNG の 2x (scale=2) は `with_scale(2)`。
- **Windows**: 
  - 小/中アイコン: `SHGetFileInfoW` + `SHGFI_ICON | SHGFI_LARGEICON`（32px）/ `SHGFI_SMALLICON`（16px）。`SHGFI_USEFILEATTRIBUTES` で実体なしでも拡張子ベース取得可。`DestroyIcon` 必須。
  - 大アイコン: `SHGFI_SYSICONINDEX` でシステムイメージリストの index を取り、`SHGetImageList(SHIL_JUMBO)`（256px）/`SHIL_EXTRALARGE`（48px）から取得。
  - サムネイル: `IShellItemImageFactory::GetImage`（`SIIGBF_THUMBNAILONLY`/`BIGGERSIZEOK`）。**COM は STA (`CoInitializeEx(COINIT_APARTMENTTHREADED)`) で初期化したワーカースレッドで実行**（UI スレッドはブロックしない）。⚠ スレッド/COM の細部は PoC で確認。
  - `windows` crate は gpui-component 内で 0.58.0 を使用 ✅。自前で 0.62.2 を追加すると二重バージョンになる（ビルド時間増のみ、実害は小）。0.58 に揃える選択肢も可。
- **macOS**: `NSWorkspace.sharedWorkspace.iconForFile(path)` → `NSImage`（複数解像度 reps を持つ。512/1024 が取れる）。`NSImage` を PNG 化（`TIFFRepresentation` → `NSBitmapImageRep`）して GPUI に渡す。⚠ メインスレッド外での AppKit 呼び出しは最小限に（PoC 検証）。`objc2-app-kit` 0.3.2。
- **フォールバック（全 OS 必須）**: 同梱 SVG セット + 拡張子カテゴリ表（folder/open-folder/file/text/image/video/audio/archive/pdf/code/symlink）。OS アイコン取得が失敗・遅延しても UI が破綻しない。MVP は **フォールバック SVG だけでも成立**（OS アイコンは Phase 1）。

### 3.3 MIME → アイコン名（Linux）

- freedesktop の慣習では `text-x-generic`, `image-x-generic`, `folder`, `application-pdf` 等のアイコン名を使う。shared-mime-info の `generic-icon` を読むのは重いので、**自前の「拡張子→アイコン名」表**を同梱（~100 拡張子で実用上十分）📝。
- 実行ファイルは `application-x-executable`、シンボリックリンクは `emblem-symbolic-link` オーバーレイ。

### 3.4 サムネイル

- `image` 0.25.10:
  - `ImageReader::open(path)?.with_guessed_format()?.decode()`。
  - **デコード爆弾対策**: `ImageReader::limits(Limits)`（既定でも上限あり）で最大幅/高さ/メモリを制限。⚠ 0.25.10 の正確な API 名は docs.rs で要確認。
  - **EXIF orientation**: `image` 0.25.x は `ImageDecoder::orientation()` / `DynamicImage::apply_orientation()` を持つ（⚠ 版により API 名・追加時期が異なるため PoC で確認）。EXIF が無い場合は 1。
  - 出力は表示サイズ×DPR に縮小した RGBA（例: 128px セルで 256px）。
- 動画/PDF サムネイル: **MVP 外**。将来は OS 委譲が現実的（Windows `IShellItemImageFactory`、macOS `QLThumbnailGenerator`、Linux は freedesktop thumbnailer 仕様 (`org.freedesktop.thumbnailer`) を D-Bus で利用、または KIO/GNOME thumbnailer に任せる）。ffmpeg 直リンクは配布が重い。
- ディスクキャッシュ: `~/.cache/<app>/thumbnails/` に PNG（freedesktop thumbnail spec は 256px 正規化）→ Later。
- メモリキャッシュ: `(path, mtime, size, scale, size_px)` をキーに LRU（`lru` crate 未検証 ⚠）。アイコンは `(icon_name, theme, size, scale)`。

### 3.5 HiDPI

- 物理ピクセルで取得（アイコン 2x、サムネイル 2x）し、GPUI の layout は論理 px で指定。
- モニタ間移動で scale が変わる → キャッシュキーに scale を含め、`on_window_moved` 等で再取得（api-notes 参照）。

### 3.6 マトリクス（アイコン）

| 項目 | Linux | Windows | macOS |
|---|---|---|---|
| ネイティブ取得 | freedesktop-icons 0.4.0 | SHGetFileInfoW / SHGetImageList / IShellItemImageFactory | NSWorkspace.iconForFile |
| 最大サイズ | テーマ依存 (256/512) | 256 (SHIL_JUMBO) / サムネイル任意 | 512/1024 |
| SVG | あり（要ラスタライズ） | 基本 ICO/PNG | 基本 PDF/PNG (NSImage) |
| スレッド制約 | なし | COM STA（ワーカースレッド） | AppKit メインスレッド推奨 ⚠ |
| ファイル MIME | mime_guess + 自前表 | SHGetFileInfo が拡張子処理 | UTI/extension |
| フォールバック | 同梱 SVG | 同梱 SVG | 同梱 SVG |

---

## 4. ボリューム・ドライブ・リムーバブルメディア

### 4.1 推奨

| 用途 | 推奨 | 版 | 備考 |
|---|---|---|---|
| 列挙 + 空き容量 (MVP) | `sysinfo` の `Disks` | 0.40.1 | MIT。Linux: /proc/mounts + statvfs / Windows: GetLogicalDrives 系 / macOS: getfsstat。`refresh(true)` で再取得。⚠ 0.40 の正確なメソッド名は docs.rs で確認 |
| 精密な空き容量 | `fs4` | 1.1.0 | MIT OR Apache-2.0。fs2 の後継。`statvfs` モジュール |
| Linux ホットプラグ (Later) | udisks2 over D-Bus (`zbus`) | zbus 6.2.1 | `org.freedesktop.UDisks2` の InterfacesAdded/Removed、`Filesystem.Mount`/`Unmount`。MIT |
| Linux ポータル全般 (Later) | `ashpd` | 0.14.0 | MIT。Trash portal 等。ドライブ列挙ポータルは無い（udisks2/GIO を使う） |
| GIO 経由 (Later) | `gio` | 0.22.10 | MIT。GVfs/VolumeMonitor。C ライブラリ依存が増えるため重い |
| ネイティブ API (Windows) | `windows` | 0.62.2 | GetLogicalDrives / GetDriveTypeW / GetVolumeInformationW / GetDiskFreeSpaceExW |
| ネイティブ API (macOS) | `objc2-foundation` | 0.3.2 | NSFileManager.mountedVolumeURLs + NSURLVolume* リソース |

### 4.2 OS 別

- **Linux**: `/proc/mounts` は疑似 FS（proc/sysfs/cgroup）も含むため、`nodev`/`fstype` でフィルタ。表示対象は `/`, `/home`, `/mnt/*`, `/media/<user>/*`, USB。`df` 相当は `statvfs`。udisks2 が動いていれば「取り出し（eject）」も実装可能（Later）。
- **Windows**: `GetLogicalDrives()`（ビットマスク）→ `GetDriveTypeW`（DRIVE_FIXED/REMOVABLE/REMOTE/CDROM/RAMDISK）→ `GetVolumeInformationW`（ラベル/FS/シリアル）→ `GetDiskFreeSpaceExW`（空き）。ホットプラグは `WM_DEVICECHANGE`（`DBT_DEVICEARRIVAL`/`DBT_DEVICEREMOVECOMPLETE`、`DEV_BROADCAST_VOLUME`）。GPUI のネイティブウィンドウメッセージフックが必要（api-scout と調整）。
- **macOS**: `NSFileManager.mountedVolumeURLs(includingResourceValuesForKeys:options:)` で `/Volumes` 配下 + ルート。リソースキー `NSURLVolumeNameKey`, `NSURLVolumeIsRemovableKey`, `NSURLVolumeIsEjectableKey`, `NSURLVolumeAvailableCapacityKey`。ホットプラグは `NSWorkspace.didMountNotification`/`didUnmountNotification`（NotificationCenter、メインスレッド）。
- **共通**: ネットワークドライブ（SMB/NFS）は空き容量取得に時間がかかる/失敗する → タイムアウト付き非同期取得。

### 4.3 設計

- **MVP**: sysinfo で 5 秒ポーリング（負荷小）。サイドバーに「ホーム / デスクトップ / ダウンロード / ドキュメント / ゴミ箱 / 各ドライブ」。空き容量は 30 秒ごと。
- マウント一覧の差分だけ UI 更新。イジェクト/マウントは MVP 外（root 権限や udisks ポリシーが絡む）。
- リムーバブル取り出し中の操作は EBUSY → エラー表示（データ損失防止のため「取り出し」は明示確認）。

### 4.4 マトリクス

| 項目 | Linux | Windows | macOS |
|---|---|---|---|
| 列挙 | /proc/mounts + sysinfo | GetLogicalDrives | NSFileManager / /Volumes |
| ラベル | mount source / udisks | GetVolumeInformationW | NSURLVolumeNameKey |
| 空き容量 | statvfs | GetDiskFreeSpaceExW | NSURLVolumeAvailableCapacityKey |
| ホットプラグ | udisks2 D-Bus (Later) | WM_DEVICECHANGE | NSWorkspace 通知 |
| イジェクト | udisks2 (Later) | 無し（OS UI に委譲） | NSWorkspace.unmountAndEjectDevice (Later) |

---

## 5. ファイル操作

### 5.1 copy / move / rename

- **同一 FS の move**: `std::fs::rename`。Unix は `rename(2)`、Windows は `MoveFileExW(..., MOVEFILE_REPLACE_EXISTING)` 相当（std 仕様）。**異なる FS は `EXDEV` で失敗**するので、その場合のみ再帰コピー + 削除にフォールバック。
- **コピー**: 自前再帰（`read_dir` + `create_new`）で、`io::copy` → 権限コピー（`fs::set_permissions`）→ 任意で `filetime` による mtime 維持（`filetime` crate 未検証 ⚠）。シンボリックリンクは **リンクとしてコピー**（`std::os::unix::fs::symlink`。Windows は開発者モード/権限が必要なため「リンク先をコピー」にフォールバックする選択肢も）。
- **進捗**: 事前に総バイト数/件数を walk で数える（大ツリーでは数え直しコスト大 → 件数ベース + 現在ファイル名表示が現実的）。`AtomicU64` で進捗、`AtomicBool` でキャンセル（チャンク境界で確認、コピー済みファイルは残す/消すを選択）。
- **衝突解決 UI**: 「置換 / スキップ / 両方保持（名前変更）/ すべてに適用」。既存名の検出は **大小文字を無視した比較**（Windows/macOS）。
- **置換の安全性**: 宛先を直接上書きせず、`dst.tmp` にコピー → `rename(tmp, dst)`（同一 FS 内で原子的）。Windows で宛先がディレクトリの場合は rename 置換不可 → 先にゴミ箱へ。
- **失敗時**: 途中までコピーしたファイルを削除（ロールバック）。ディレクトリは部分コピーを残さない。
- fsync: エクスプローラでは通常不要。重要な操作のみ `File::sync_all()`（クラッシュ時の耐久性、任意）。
- ⚠ スパースファイル: `std::fs::copy` (Windows は CopyFileExW) はスパース領域の保持を保証しない。大容量 VM イメージ等では膨張の可能性（要検証）。

### 5.2 削除（ゴミ箱）

- **`trash` 5.3.0 (MIT)** ✅:
  - API: `trash::delete(path)`, `trash::delete_all(paths)`, `TrashContext`（`delete`, `delete_all`）、`trash::list() -> Vec<TrashItem>`, `TrashItem::original_path()`, `TrashItem::size()`（`TrashItemSize`）, `trash::restore_all(items)`, `trash::purge_all(items)`, `trash::metadata(item)`, `trash::is_empty()`, `trash::trash_folders()` ✅（ソース確認）。
  - Linux: freedesktop Trash spec 1.0 実装（既定 feature `freedesktop`）。`~/.local/share/Trash/files` + `info`、別ボリュームは `<mount>/.Trash-$uid`。`portal` feature で xdg-desktop-portal の Trash ポータルも選択可（既定は freedesktop 優先）。⚠ crate docs に「Linux/FreeBSD で libc::getmntent 系のスレッド安全性問題への注意」記載あり（対策として Mutex 済みだが、他スレッドで直接呼ぶ場合は非推奨）。
  - Windows: Recycle Bin。macOS: Trash（`~/.Trash`/`.Trashes`）。ネットワーク FS では失敗する場合あり → エラー時に「完全削除しますか？」を確認。
  - ゴミ箱ビューは `list()` + `restore_all()`/`purge_all()` で実装可能（MVP 外でも API はある）。
- **完全削除**: `remove_file` / `remove_dir_all`。シンボリックリンクを辿らないこと（`symlink_metadata` で判定）。読み取り専用属性（Windows `FILE_ATTRIBUTE_READONLY`）、macOS `uchg` フラグ、Linux immutable 属性は失敗する → エラーを日本語で表示。

### 5.3 rename と OS 固有の検証

- バリデーション: 名前空/`.`/`..`/パス区切り、Windows 禁止文字 `< > : " / \ | ? *` と制御文字、予約名 `CON, PRN, AUX, NUL, COM1-9, LPT1-9`（拡張子付きも不可）、末尾のドット/スペース。長さ: Windows 260（長パス有効時 32,767）、Linux 255 bytes/コンポーネント、macOS 255 UTF-16 ユニット。
- **大小文字のみの rename** (Windows/macOS): 直接 `rename("foo","Foo")` は no-op または `ERROR_ALREADY_EXISTS` になる場合がある → **一時名 `foo.tmp-rename` 経由の 2 段階 rename**。⚠ macOS の実挙動は要検証。
- **Windows 長パス**: `dunce` 1.0.5 で `canonicalize` の `\\?\` を剥がす（表示用）。実際の I/O は `\\?\C:\...` 形式で行うと MAX_PATH 制限を回避（std は longPathAware manifest + `\\?\` で長パス対応）。⚠ manifest 設定と `\\?\` 併用は PoC で実機確認。
- **macOS NFD**: APFS/HFS+ はファイル名を正規化して保存するため、OS から返る名前は NFD のことがある（HFS+ 由来）。表示・検索前に `unicode-normalization` 0.1.26 で NFC 化し、**操作は元の `PathBuf` をそのまま使う**（NFC に変換したパスで open しない）。
- **Linux 非 UTF-8**: `OsStr`/`OsString` を保持。表示だけ `to_string_lossy()`（U+FFFD になる）。ソートは `std::os::unix::ffi::OsStrExt::as_bytes()` で比較。**文字列を経由してパスを再構築しない**。

### 5.4 隠しファイル・特殊ディレクトリ

- 隠し: Unix は先頭 `.`、Windows は `FILE_ATTRIBUTE_HIDDEN`（+ `SYSTEM`）、macOS は先頭 `.` + `UF_HIDDEN`（`~/Library` 等）。既定は非表示、ツールバーでトグル。📝 Windows では「hidden + system」を隠すのがエクスプローラ既定。
- 特殊ディレクトリ: `dirs` 7.0.0 ✅（`home_dir`, `desktop_dir`, `download_dir`, `document_dir`, `picture_dir`, `video_dir`, `audio_dir`, `public_dir`, `config_dir`, `data_dir`, `data_local_dir`, `cache_dir`, `runtime_dir`）。`dirs-next` 2.0.0 は 2020 年で停滞 → `dirs` を使う。
- ゴミ箱パス: Linux `~/.local/share/Trash`、Windows `C:\$Recycle.Bin`、macOS `~/.Trash`。UI では OS の「ゴミ箱」を仮想フォルダとして表示（`trash::list()` 経由が安全）。

### 5.5 マトリクス（操作）

| 項目 | Linux | Windows | macOS |
|---|---|---|---|
| move (同一FS) | rename(2) | MoveFileExW | rename(2) |
| move (異FS) | copy+delete | copy+delete | copy+delete |
| ゴミ箱 | freedesktop Trash spec | Recycle Bin | Trash |
| 長パス | 制限なし (PATH_MAX 4096) | MAX_PATH 260 → `\\?\` | 制限なし (実質 1024) |
| 非正規化名 | 任意バイト | WTF-8 (UTF-16) | NFD |
| 大小文字 | 区別 | 区別しない | 区別しない |
| 予約名 | 無し | あり | `:` のみ禁止（Finder 表示） |
| read-only 削除 | パーミッション | 属性解除要 | `uchg` 解除要 |

---

## 6. デスクトップ統合

### 6.1 既定アプリで開く

- **`opener` 0.9.0 (MIT OR Apache-2.0)** ✅: `opener::open(path)` / `open_browser(url)`。
  - Linux: `xdg-open` 系を起動（実装詳細は `linux_and_more.rs`。⚠ 内部コマンド名は PoC 確認）。
  - Windows: `ShellExecuteW`（`windows.rs`）。
  - macOS: `open` コマンド（`macos.rs`, `run_open(Command::new("open").arg(...))` ✅）。
- 代替: `open` 5.4.5（`open::that`）。`opener` は reveal 機能も持つため 1 つで済む。
- 「開く」失敗時（関連付けなし）: 「Open With」ダイアログを出す（Later）。

### 6.2 エクスプローラで表示（Reveal）

- **`opener::reveal(path)`（crate feature `reveal` 必須）** ✅（`opener-0.9.0/src/lib.rs:177`、docs に判定表）:
  - Linux: D-Bus `org.freedesktop.FileManager1` の `ShowItems` を第一候補（`freedesktop.rs` に zbus プロキシ実装確認 ✅）。未対応環境は `OpenURI` フォールバック。Nautilus/Dolphin/Thunar は ShowItems 対応。
  - Windows: `explorer.exe /select, <path>`（`linux_and_more.rs:134` 確認 ✅）。
  - macOS: `open -R <path>`（`macos.rs:11` 確認 ✅）。
- 注意: パスに `%` 等の特殊文字を含む場合のエスケープ（opener 側で処理）。WSL では `explorer.exe` 経由。

### 6.3 「Open With」/既定アプリ列挙（Later）

| | Linux | Windows | macOS |
|---|---|---|---|
| アプリ列挙 | `.desktop` 走査（`/usr/share/applications`, `~/.local/share/applications`, `$XDG_DATA_DIRS`）+ MIME 関連付け `mimeapps.list` | `IApplicationAssociationRegistration` / `SHOpenWithDialog` | `LSCopyApplicationURLsForURL` / LaunchServices |
| 実現コスト | 中（desktop-entry spec のパースが必要。crate 未検証 ⚠） | 中（`SHOpenWithDialog` で OS UI に委譲可） | 中〜高 |
| 推奨 | Phase 2+ | `SHOpenWithDialog` のみ Phase 2 | Phase 2+ |

### 6.4 その他

- 「ターミナルで開く」: Linux は `konsole --workdir <dir>` / `xdg-terminal-exec`、macOS は `open -a Terminal <dir>`、Windows は `wt -d <dir>`。MVP 外。
- 「パスをコピー」: テキストとしてクリップボードへ（`arboard` の text）。実装容易なので MVP に含めて良い。

---

## 7. クリップボードとドラッグ＆ドロップ

### 7.1 OS レベルのファイルリスト表現

- **Linux/X11**: `text/uri-list`（`file:///...` を CRLF 区切り）、`x-special/gnome-copied-files`（先頭行 `copy`/`cut` + URI）、KDE は `application/x-kde-cutselection` 付きの `text/uri-list`（Klipper/Dolphin 連携）。Wayland は `wl_data_device`/`wl_data_source` + `wl-clipboard-rs` の data-control プロトコルが必要（通常の `wl_data_offer` だけではクリップボードに常駐できない）。
- **Windows**: `CF_HDROP`（`DROPFILES` 構造 + ダブル NUL 終端のファイルリスト、`DragQueryFileW` で取り出し）+ `CFSTR_PREFERREDDROPEFFECT`（`DROPEFFECT_COPY`/`MOVE`、カットは MOVE 相当）。
- **macOS**: `NSPasteboard` に `NSPasteboardTypeFileURL`（`NSURL` 配列）を書く。ドラッグ中は `kPasteboardTypeFileURLPromise` でファイル約束も可。古い `NSFilenamesPboardType` は非推奨。

### 7.2 Rust crate

| crate | 検証版 | ファイルリスト対応 | 備考 |
|---|---|---|---|
| `arboard` | 3.7.0 | ✅ `Clipboard::get().file_list() -> Vec<PathBuf>` / `Clipboard::set().file_list(&[Path])` を確認 | Linux (X11 + Wayland WlDataControl), macOS (`osx.rs`), Windows (`windows.rs`) に実装あり ✅。MIT OR Apache-2.0 |
| `clipboard-rs` | 0.3.5 | ✅ `get_files()/set_files()`（`platform/{x11,wayland,win,macos}.rs` 確認） | Wayland は **feature `wayland`**（`wl-clipboard-rs`）が必要 ✅。MIT。最終更新 2026-06 |
| `copypasta` | 0.10.2 | ❌ テキストのみ | (MIT/Apache 表記は未確認 ⚠) |

- **推奨**: `arboard` 3.7.0 を第一候補（3 OS で同一 API、ファイルリスト対応、Wayland も WlDataControl で動作）。Wayland の制約（data-control プロトコル非対応コンポジタ）が問題になる環境のみ `clipboard-rs` を検討。
- 注意: Linux のクリップボードは**アプリが終了すると内容が消える**（X11/Wayland のクリップボードオーナー問題）。コピー操作後はアプリを生かしておく必要がある（エクスプローラは常駐なので問題なし）。

### 7.3 GPUI との関係（api-scout 領域の境界）

- `gpui-pre` 0.3.9（Zed `gpui` 0.2.2 相当）のプラットフォーム抽象に **`ClipboardItem` / `ClipboardEntry::{String, Image, ExternalPaths}` / `ExternalPaths` / `FileDropEvent` が公開 API として存在** ✅（`src/platform.rs`, `src/interactive.rs` を確認）。つまり GPUI 自体が「パスのリスト」をクリップボード/ドロップの第一級データとして扱う設計になっている。
- ただしこのスナップショットには Linux/Wayland 等の実プラットフォーム実装が含まれておらず（`visual_test` のみ）、**各 OS での `ExternalPaths` の読み書き・FileDrop 実装カバレッジは未確認 ⚠** → PoC で確認（api-scout と分担。私は OS 機構側の要件のみ提示）。
- 実装カバレッジ不足の場合の代替:
  1. アプリ層で `arboard` を直接使い、GPUI のテキストのみクリップボードと併用（ファイルリストは独自管理）。
  2. `ClipboardEntry::ExternalPaths` を GPUI がサポートしていればそれを使う（UTI/CF_HDROP/uri-list への変換は GPUI 側実装に依存）。
- 貼り付け時のフロー: クリップボードから `Vec<PathBuf>` を取得 → 「コピー」か「カット」かを判定（Linux は `x-special/gnome-copied-files` 先頭行、Windows は `Preferred DropEffect`、macOS は pasteboard のフレーバー）。arboard の `file_list()` はパスのみ返すため、**cut/copy の区別は OS 固有処理が必要**（MVP はコピーのみ対応でも良い 📝）。

### 7.4 ドラッグ＆ドロップ（外部 → アプリ / アプリ → 外部）

- **INTO（外部からアプリへ）**:
  - Wayland: フォーカス中の `wl_surface` に `wl_data_device::data_offer`（`text/uri-list`）が届く。DnD 中は `wl_data_offer` の receive を実装。
  - X11: XDND プロトコル（`XdndEnter/Position/Drop`、`text/uri-list`）。
  - Windows: `OleInitialize` + `RegisterDragDrop(hwnd, IDropTarget)`。`CF_HDROP` を受領。
  - macOS: `NSView.registerForDraggedTypes([.fileURL])` + `NSDraggingDestination`。
  - **GPUI は `FileDropEvent` を公開している** ✅ ので、framework が OS イベントを吸収している可能性が高い（実装状況は要確認 ⚠）。
- **OUT（アプリから外部へ）**: 実装コスト大。
  - Wayland: `wl_data_source` を保持し `start_drag`。ドラッグ中のアイコン描画（`wl_drag_icon`）も必要。コンポジタ依存の挙動差。
  - X11: XDND のソース側実装（XdndSelection の所有、`XdndStatus/Action` 処理）。
  - Windows: `IDataObject`（`CF_HDROP`）+ `IDropSource` + `DoDragDrop`。
  - macOS: `NSDraggingSource` + `beginDraggingSessionWithItems`（`NSDraggingItem` + `NSPasteboardItem`）。
  - **MVP 外 / Phase 3**。GPUI の `ExternalPaths` を使ったドラッグ（Zed のタブ DnD は内部 DnD）は可能でも、OS へのドラッグアウトは別実装。
- 代替 UX: ドラッグアウトの代わりに「コピー」「切り取り」メニュー + 他アプリで貼り付け（実装容易で確実）。

### 7.5 マトリクス（クリップボード/DnD）

| 項目 | Linux | Windows | macOS |
|---|---|---|---|
| ファイルコピー表現 | text/uri-list + gnome/KDE 拡張 | CF_HDROP | .fileURL (NSPasteboard) |
| crate 対応 | arboard 3.7.0 (X11+Wayland) ✅ | arboard ✅ | arboard ✅ |
| cut/copy 区別 | x-special/... 先頭行 | Preferred DropEffect | pasteboard flavor |
| drop INTO | Wayland data_device / XDND | IDropTarget | NSDraggingDestination |
| drag OUT | Wayland wl_data_source / XDND | IDataObject+DoDragDrop | NSDraggingSource |
| GPUI API | `ExternalPaths` / `FileDropEvent` 公開 ✅ / 実装未確認 ⚠ | 同左 | 同左 |

---

## 8. 検索

- **MVP**: 現在ディレクトリ内の名前フィルタ（インクリメンタル、大小無視）。`read_dir` 済みの Vec に対するメモリ内フィルタで十分（100k 件でも 1 桁 ms〜数十 ms）。
- **Phase 2 (再帰検索)**: `ignore` 0.4.34 の `WalkBuilder::build_parallel()` ✅。
  - 既定の `.gitignore`/`.ignore` 尊重はエクスプローラには不適切 → `standard_filters(false)`, `hidden(false)`（隠しファイルは自前制御）、`max_depth(Some(n))` で深さ制限、`same_file_system(true)` でマウント境界を越えない。
  - 結果はチャネルでストリーミング（100 件ごと、または 100ms ごとに UI 反映）。件数上限（例: 10,000）とキャンセル（`AtomicBool`）。
  - シンボリックリンクループ対策: `follow_links(false)`（既定）。
- **内容検索**: ripgrep 系 `grep-searcher` + `grep-regex`（Zed でも使用、版未検証 ⚠）。MVP 外。
- Windows の MFT/USN ジャーナル、macOS Spotlight (`mdfind`) は高速だが実装が重い → MVP 外。

---

## 9. 総合プラットフォーム・マトリクス

| 機能 | Linux (Kubuntu/Plasma 6) | Windows 11 | macOS | MVP |
|---|---|---|---|---|
| 列挙 | std read_dir + d_type | std read_dir (FindFirstFile) | std read_dir | ✅ |
| 監視 | inotify (notify 8.2.2) | RDCW (notify) | FSEvents (notify) | ✅ (表示中+親、非再帰) |
| watch 上限 | 実測 518185/1024 | バッファ 16KB/ハンドル | FSEvents ストリーム | — |
| アイコン | freedesktop-icons + SVG フォールバック | SHGetFileInfoW (+SHIL_JUMBO) | NSWorkspace.iconForFile | フォールバック SVG ✅ / OS アイコン Phase 1 |
| サムネイル | image crate | IShellItemImageFactory (後) | QLThumbnailGenerator (後) | 画像のみ ✅ |
| ボリューム | sysinfo (udisks2 後) | sysinfo (WM_DEVICECHANGE 後) | sysinfo (NSWorkspace 後) | 列挙+空き ✅ |
| コピー/移動 | rename + copy | 同左 (長パス注意) | 同左 (NFD 注意) | Phase 1 |
| ゴミ箱 | trash 5.3.0 (freedesktop) | trash (Recycle Bin) | trash (Trash) | ✅ |
| 開く/リビール | opener 0.9.0 + D-Bus ShowItems | ShellExecute + explorer /select, | open / open -R | ✅ |
| ファイルコピー (clipboard) | arboard (X11+Wayland) | arboard (CF_HDROP) | arboard (.fileURL) | Phase 1 |
| DnD IN | GPUI FileDropEvent (要確認) | 同左 | 同左 | 実装状況次第 |
| DnD OUT | 自前 (Wayland/X11) | IDataObject | NSDraggingSource | Phase 3 |
| 隠しファイル | dotfiles | 属性 | dotfiles/UF_HIDDEN | ✅ |
| 非 UTF-8 | OsStr 必須 | WTF-8 | NFD → NFC 表示 | ✅ |
| 再帰検索 | ignore | ignore | ignore | Phase 2 |

---

## 10. 設計上の落とし穴（チェックリスト）

1. **`DT_UNKNOWN`**: NFS/FUSE 等では `DirEntry::file_type()` が lstat にフォールバックし遅くなる。列挙中に大量の stat が走る場合はキャンセル可能にする。
2. **列挙中の削除/rename**: `read_dir` のイテレート中にエントリが消えると `metadata()` が `NotFound` を返す。個別エラーを許容し、一覧全体を失敗させない。
3. **inotify watch はディレクトリ単位**: 再帰 watch はサブディレクトリ数分の watch を消費。`node_modules`/`.git` を含むツリーの再帰 watch は避ける（本設計では非再帰）。
4. **notify イベントの溢れ**: inotify は `Flag::Rescan`、Windows は溢れの明示通知が無い可能性 ⚠ → **定期リコンサイル必須**（5〜10 秒）。
5. **自操作ループ**: 自分の rename/copy/delete がイベントを生み、一覧が二重更新される。抑制窓（1〜2 秒）で防ぐ。
6. **Windows の大小文字非区別**: 同一名判定は case-fold。`HashMap` のキーに生の名前を使うと重複表示/取りこぼしが起きる。
7. **macOS の NFD**: NFC で比較しないと検索・重複判定が壊れる。表示のみ正規化し、I/O は元パス。
8. **Linux の非 UTF-8 名**: `String` 経由でパスを作り直すと壊れる。`PathBuf` を保持し、表示のみ lossy。
9. **Windows MAX_PATH**: `\\?\` と manifest (`longPathAware`) の両方が必要。`dunce` は表示用の簡略化。
10. **大小文字のみの rename**: 2 段階 rename。**macOS は rename 成功でも大小変更が反映されない場合がある** ⚠（要検証）。
11. **read-only/immutable**: 削除・move が `EACCES`/`EPERM` で失敗。事前に属性確認し、解除はユーザー確認の上で。
12. **シンボリックリンク**: `remove_dir_all` はシンボリックリンクを辿らない仕様だが、自前再帰実装では `symlink_metadata` で必ず判定。リンク先を誤って消さない。
13. **ジャンクション/シンボリックリンク作成**は管理者権限/開発者モードが必要（Windows）。MVP 外。
14. **ネットワーク FS**: 空き容量取得と notify が遅い/効かない。タイムアウト + PollWatcher + 「自動更新なし」表示。
15. **大量コピー**: UI スレッドでやってはいけない。進捗・キャンセル・エラー集約（1 件失敗で全体を止めない）。
16. **置換の原子性**: 宛先を直接開いて書かない（途中で失敗するとファイル破損）。tmp → rename。
17. **ゴミ箱に入らない FS**: ネットワーク/一部リムーバブルでは失敗 → 完全削除の確認 UI。
18. **アイコン取得の遅延**: Windows shell/ macOS AppKit は重い。非同期 + キャッシュ + プレースホルダ（先にフォールバック SVG を表示）。
19. **HiDPI/モニタ移動**: キャッシュキーに scale を含める。スケール変更時に再取得。
20. **Wayland の DnD**: フォーカス/シートがないと data_offer が来ない。ドラッグ中のウィンドウ更新に注意。
21. **GPUI のファイル I/O は必ず background executor**: UI スレッドで `read_dir`/`metadata`/`image decode` をしない（フレーム落ち）。
22. **自然順ソートのコスト**: 比較関数が重い。`sort_by_cached_key` でキー事前計算、必要なら `sort_unstable_by`。

---

## 11. MVP 範囲（推奨ロードマップ）

### Phase 0: PoC（1〜2 日）
- Linux (Kubuntu / Wayland) のみ実機、Windows/macOS は `cargo check --target` でビルド確認。
- 一覧: `read_dir` + 自然順 + 名前フィルタ + 仮想リスト（10k 件でスクロール検証）。
- ナビゲーション: パンくず、戻る/進む、ダブルクリックで `opener::open`、コンテキストメニューで `opener::reveal`。
- 監視: notify 8.2.2 + debouncer-full 0.7.0、カレント + 親、300ms、Rescan → 再読込、リコンサイル 10s。
- アイコン: 同梱 SVG フォールバック + Linux は freedesktop-icons 0.4.0。
- 削除: `trash::delete`。
- クリップボード: arboard でパスのコピー（text）＋ file_list 読み書きの検証。
- GPUI 側の `ExternalPaths`/`FileDropEvent` 実機挙動確認（api-scout と共同）。

### Phase 1
- copy/move（進捗・キャンセル・衝突解決）、ゴミ箱ビュー（list/restore/purge）。
- ボリューム一覧 + 空き容量（sysinfo）、サイドバー（ホーム/デスクトップ/ダウンロード等）。
- ファイルリストのコピー/貼り付け（arboard `file_list`。cut/copy 区別は Linux のみ）。
- 隠しファイルトグル、ソート列（サイズ/更新時刻）、複数選択 + キーボード操作。
- Windows/macOS の実機ビルド・検証。

### Phase 2
- 再帰検索（ignore 並列 walker）、画像サムネイル、OS ネイティブアイコン、Open With（Windows SHOpenWithDialog から）、ホットプラグ（各 OS ネイティブ通知）、ロケール照合。
- ネットワーク FS の PollWatcher 自動フォールバック。

### Phase 3（将来）
- ドラッグアウト（OS 連携）、動画/PDF サムネイル、アーカイブ閲覧、タグ/検索履歴、クラウド統合、Windows MFT/USN、Spotlight 検索。

---

## 12. crate 選定表（2026-11-11 時点、crates.io API + ソースで検証）

| crate | 検証版 | 用途 | 成熟度/備考 | ライセンス |
|---|---|---|---|---|
| `notify` | **8.2.2** (2026-11-09) | ファイル監視 | 定番。要注意点は README 記載。Linux: inotify 0.11 / macOS: fsevent-sys / Win: windows | **CC0-1.0** ✅（Cargo.toml 確認） |
| `notify-types` | **2.3.0** (2026-11-09) | notify の型 | notify 8 系の EventKind/ErrorKind | MIT OR Apache-2.0 ✅ |
| `notify-debouncer-full` | **0.7.0** (2026-11-09) | デバウンス + rename 結合 | `notify ^8.2.0` 依存 ✅。file id 追跡 | MIT OR Apache-2.0 ✅ |
| `ignore` | **0.4.34** (2026-10-27) | 並列再帰 walker | ripgrep 製。`standard_filters(false)` 可 | Unlicense OR MIT ✅ |
| `walkdir` | **2.5.0** (2024-03-01) | 単純再帰 | 安定 | Unlicense OR MIT（未確認 ⚠） |
| `jwalk` | **0.9.0** (2026-08-05) | 並列 walker | **README に Unmaintained 明記** → 不採用 | MIT ✅ |
| `alphanumeric-sort` | **1.5.8** (2026-07-01) | 自然順ソート | `compare_str` 他 | MIT ✅ |
| `natord` | 1.0.9 (2015-12-16) | 自然順 | 更新停止 | MIT（未確認 ⚠） |
| `human-sort` | 0.2.2 (2019-04-14) | 自然順 | 更新停止 | 未確認 ⚠ |
| `icu_collator` | **2.4.0** (2026-10-22) | ロケール照合 | ICU4X。データ同梱サイズ注意 | Unicode-3.0 ✅ |
| `rust_icu` | 5.8.0 (2026-08-20) | ICU-C FFI | システム ICU 必須 | 未確認 ⚠ |
| `freedesktop-icons` | **0.4.0** (2025-04-16) | Linux アイコン探索 | icon-theme-spec 準拠、SVG/scale/cache | MIT ✅ |
| `mime_guess` | **2.0.5** (2024-06-29) | 拡張子→MIME | 安定・更新低調 | MIT（未確認 ⚠） |
| `infer` | **0.22.0** (2026-07-15) | 内容→MIME | マジックバイト判定 | MIT（未確認 ⚠） |
| `image` | **0.25.10** (2026-03-10) | デコード/サムネイル | 汎用。上限設定必須 | MIT OR Apache-2.0（未確認 ⚠） |
| `sysinfo` | **0.40.1** (2026-10-30) | ボリューム/空き容量 | クロスプラットフォーム | MIT（未確認 ⚠） |
| `fs4` | **1.1.0** (2026-04-28) | statvfs/空間 | fs2 後継 | MIT OR Apache-2.0 ✅ |
| `nix` | **0.31.3** (2026-05-11) | Unix syscall | statvfs 等 | MIT ✅ |
| `zbus` | **6.2.1** (2026-11-01) | D-Bus | udisks2/FileManager1 | MIT ✅ |
| `ashpd` | **0.14.0** (2026-11-05) | XDG ポータル | Trash ポータル等 | MIT ✅ |
| `gio` | 0.22.10 (2026-09-19) | GIO/VolumeMonitor | C 依存重い | MIT ✅ |
| `trash` | **5.3.0** (2026-11-05) | ゴミ箱 | 3 OS + portal。freedesktop 既定 | MIT ✅ |
| `opener` | **0.9.0** (2026-09-29) | open / reveal | `reveal` feature | MIT OR Apache-2.0 ✅ |
| `open` | 5.4.5 (2026-10-22) | open | opener の代替 | 未確認 ⚠ |
| `arboard` | **3.7.0** (2026-10-15) | クリップボード（ファイル対応） | Linux/Windows/macOS 実装確認 | MIT OR Apache-2.0 ✅ |
| `clipboard-rs` | **0.3.5** (2026-06-30) | クリップボード（files） | Wayland は feature | MIT ✅ |
| `copypasta` | 0.10.2 (2025-04-25) | テキストのみ | ファイル非対応 | 未確認 ⚠ |
| `dunce` | **1.0.5** (2024-08-04) | Windows パス簡略化 | `\\?\` 除去 | MIT OR Apache-2.0（未確認 ⚠） |
| `unicode-normalization` | **0.1.26** (2026-10-16) | NFC/NFD | macOS 対応 | MIT OR Apache-2.0（未確認 ⚠） |
| `dirs` | **7.0.0** (2026-09-05) | 特殊ディレクトリ | メンテ中 | MIT OR Apache-2.0（未確認 ⚠） |
| `windows` | **0.62.2** (2025-10-06) | Windows API | shell/COM/FS | MIT OR Apache-2.0（未確認 ⚠） |
| `objc2-app-kit` | **0.3.2** (2025-10-04) | macOS AppKit | NSWorkspace 等 | MIT ✅ |
| `objc2-foundation` | **0.3.2** (2025-10-04) | macOS Foundation | NSFileManager 等 | MIT ✅ |
| `dirs-next` | 2.0.0 (2020-10-22) | 特殊ディレクトリ | **更新停止 → 不採用** | 未確認 ⚠ |
| `fs2` | 0.4.3 (2018-01-06) | ファイルロック等 | **更新停止 → fs4 へ** | 未確認 ⚠ |

> ライセンスの「未確認」は crates.io メタデータを本調査で取得できなかったもの。PoC 着手時に `cargo tree`/crates.io で再確認すること（実装ブロッカーではない）。

---

## 13. 参考リンク（一次資料）

- ファイル監視: [notify crate](https://github.com/notify-rs/notify), [notify-debouncer-full](https://docs.rs/notify-debouncer-full/0.7.0/notify_debouncer_full/), [inotify(7)](https://man7.org/linux/man-pages/man7/inotify.7.html)
- freedesktop: [Trash spec 1.0](https://specifications.freedesktop.org/trash/1.0), [Icon Theme spec](https://specifications.freedesktop.org/icon-theme-spec/icon-theme-spec-latest.html), [Desktop Entry spec](https://specifications.freedesktop.org/desktop-entry-spec/desktop-entry-spec-latest.html), [MIME Apps spec](https://specifications.freedesktop.org/mime-apps-spec/mime-apps-spec-latest.html), [Clipboards wiki](https://www.freedesktop.org/wiki/Specifications/ClipboardsWiki/)
- xdg-desktop-portal: [Trash portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Trash.html), [FileTransfer portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.FileTransfer.html)
- Windows: [FindFirstFileW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-findfirstfilew), [ReadDirectoryChangesW](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw), [SHGetFileInfoW](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-shgetfileinfow), [IShellItemImageFactory](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ishellitemimagefactory), [GetLogicalDrives](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getlogicaldrives), [GetDriveTypeW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdrivetypew), [GetVolumeInformationW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumeinformationw), [GetDiskFreeSpaceExW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getdiskfreespaceexw), [CompareStringEx](https://learn.microsoft.com/en-us/windows/win32/api/stringapiset/nf-stringapiset-comparestringex), [Naming a file](https://learn.microsoft.com/en-us/windows/win32/fileio/naming-a-file), [Maximum path length](https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation), [DragQueryFileW (CF_HDROP)](https://learn.microsoft.com/en-us/windows/win32/api/shellapi/nf-shellapi-dragqueryfilew), [IFileOperation](https://learn.microsoft.com/en-us/windows/win32/api/shobjidl_core/nn-shobjidl_core-ifileoperation)
- Apple: [NSWorkspace](https://developer.apple.com/documentation/appkit/nsworkspace), [NSPasteboard](https://developer.apple.com/documentation/appkit/nspasteboard), [NSFileManager](https://developer.apple.com/documentation/foundation/nsfilemanager), [FSEvents Programming Guide](https://developer.apple.com/library/archive/documentation/Darwin/Conceptual/FSEvents_ProgGuide/Introduction/Introduction.html)
- Rust std: [`std::fs::rename`](https://doc.rust-lang.org/std/fs/fn.rename.html), [`std::fs::read_dir`](https://doc.rust-lang.org/std/fs/fn.read_dir.html), [`DirEntry`](https://doc.rust-lang.org/std/fs/struct.DirEntry.html)
- crates: [trash-rs](https://github.com/Byron/trash-rs), [arboard](https://github.com/1Password/arboard), [clipboard-rs](https://github.com/ChurchTao/clipboard-rs), [opener](https://github.com/Seeker14491/opener), [freedesktop-icons](https://github.com/oknozor/freedesktop-icons), [ignore](https://github.com/BurntSushi/ripgrep/tree/master/crates/ignore)

> URL 到達確認 (2026-11-11): Microsoft Learn の主要 API ページ / Apple NSWorkspace / xdg-desktop-portal Trash portal / icon-theme-spec / maximum-file-path は HTTP 200 を確認。**Trash spec は現行 URL が `https://specifications.freedesktop.org/trash/1.0`**（trash crate が参照する旧 `trash-spec/trashspec-1.0.html` は 404）。Clipboards wiki は CLI からは HTTP 418（bot 対策、ブラウザでは閲覧可）。

---

## 14. 未確認事項（PoC で潰す）

1. ⚠ notify 8.2.2 Windows バックエンドの `ERROR_NOTIFY_ENUM_DIR`（バッファ溢れ）時の挙動（Rescan を emit するか、単にイベントが欠けるか）。
2. ⚠ FSEvents の `NonRecursive` 指定時の挙動・イベント粒度（notify がどうフィルタするか）。
3. ⚠ `IShellItemImageFactory` / `SHGetFileInfoW` の正確な COM・スレッド要件と 256px アイコン取得手順（SHIL_JUMBO）。
4. ⚠ `NSWorkspace.iconForFile` のメインスレッド外呼び出しの可否。
5. ⚠ GPUI (gpui-pre 0.3.9 / gpui 0.2.2 系) の Linux/Wayland における `ClipboardEntry::ExternalPaths` と `FileDropEvent` の実装状況（api-scout と共同）。
6. ⚠ `image` 0.25.10 の EXIF orientation API 名（`metadata::Orientation` の有無）と `Limits` の正確な使い方。
7. ⚠ `sysinfo` 0.40.1 `Disks`/`Disk` の正確なメソッド名・フィールド（`is_removable` 等）。
8. ⚠ `trash` 5.3.0 のネットワーク FS / リムーバブルでの実挙動、`portal` feature の実機動作（KDE Plasma 6）。
9. ⚠ macOS の case-only rename の実挙動（2 段階 rename の必要性）。
10. ⚠ Windows `std::fs::copy` のスパースファイル保持。
11. ⚠ cut/copy の区別方法（arboard はパスのみ返す。Linux の gnome-copied-files 先頭行、Windows の Preferred DropEffect を直接読む必要がある）。
12. ⚠ ネットワーク FS の判定方法（`statfs` の f_type で NFS/CIFS/SMB を判定する実装。crate 未定）。
13. ⚠ udisks2 のホットプラグ購読実装（zbus 6.2.1 でのサンプルコード）と、KDE 環境での権限（polkit）。
14. ⚠ ライセンス未確認の crate（mime_guess, infer, image, sysinfo, dunce, unicode-normalization, dirs, windows, walkdir, natord, human-sort, open, copypasta, fs2, rust_icu）。
15. ⚠ `opener` Linux 実装の内部コマンド（xdg-open か gio open か）。
16. ⚠ GPUI で SVG アイコンをファイルから描画する経路（`ImageSource::Svg` 相当の有無）。
17. ⚠ 100k 件ディレクトリの実測ベンチ（この環境で生成して計測する。PoC で実施）。至急ではないが、性能目標（一覧 200ms 以内など）を決める根拠になる。
