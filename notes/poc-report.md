# GPUI ファイルエクスプローラ PoC — ビルド/実行検証レポート

- 日付: 2026-11-11
- 担当: gpui-poc-builder（task-4）
- 対象: Kubuntu 2026.04 / KDE Plasma 6 / Wayland (`XDG_SESSION_TYPE=wayland`, `WAYLAND_DISPLAY=wayland-0`, `DISPLAY=:1`) / NVIDIA GTX 1660 SUPER / rustc・cargo 1.99.1 / 12 cores
- 成果物: `work/gpui-file-explorer/poc/`（実行可能な PoC プロジェクト）、`poc/screenshot/window-crop.png`（実行証拠）

## 0. 結論

**スタックA `gpui-kit = "=0.9.1"`（内部で gpui-pre 0.3.12 + gpui-pre-platform 0.3.12、
Zed gpui snapshot zed@d2a4fb6）で、このマシン上のビルドと Wayland 実行に成功した。**

- 依存含む初回ビルド: **1m31s / 警告 0**（12 コア、debug。release は 3m05s / 51 MiB）
- リンク時のシステムライブラリ不足（`libxkbcommon-x11`）が唯一のビルドブロッカーで、
  **sudo なし・システム変更なしのローカル symlink + `.cargo/config.toml` で回避**した。
- 実行時は **ネイティブ Wayland ウィンドウ**が表示され、**stderr 出力 0 行**
  （EGL/Vulkan/Wayland/GPU エラーなし）。KWin D-Bus でウィンドウ実在・geometry を確認。
- 非同期ディレクトリ読み込み（`cx.background_spawn`）→ `uniform_list` 描画 →
  ステータス行、まで実際に動作（スクリーンショットで 167 entries 表示を確認）。
- キーボード/マウス操作の**入力注入による E2E 検証は未実施**（Wayland ネイティブ向けの
  `kdotool` 等が未導入、`xdotool` は Wayland ネイティブ窓に無効）。これは環境制約。

## 1. 使ったスタックと正確なバージョン

### スタックA（採用）

```toml
[dependencies]
gpui-kit = "=0.9.1"
```

`gpui-kit` は「GPUI 一式への単一依存」facade。`Cargo.toml` は `gpui-pre*` を直接書かずに済む。
`gpui-kit 0.9.1` が固定する依存（crates.io の正規化 Cargo.toml より）:

| gpui-kit 側の依存名 | package | version | 備考 |
|---|---|---|---|
| `gpui` | **gpui-pre** | **=0.3.12** | lib 名は `gpui`（`[lib] name = "gpui"`） |
| `gpui_platform` | **gpui-pre-platform** | **=0.3.12** | features `font-kit, x11, wayland, runtime_shaders` を gpui-kit が**固定で有効化** |
| `gpui-base` | gpui-base | 0.9.1 | |
| `gpui-component` | gpui-component | 0.9.1 | default feature |
| `gpui-kit-assets` | gpui-kit-assets | 0.9.1 | default feature（Lucide icons 埋め込み） |
| `gpui_macros` | gpui-pre-macros | =0.3.12 | |

`gpui-pre` の `[package.metadata.gpui-pre]` より: `zed-rev = "d2a4fb6ded7c470be25d1541281da3882495b65b"`。
つまり「Zed の gpui snapshot」を `gpui-pre` 名義で公開したもの。

### 実際に解決された Cargo.lock の gpui* エントリ

```
gpui-base 0.9.1
gpui-component 0.9.1
gpui-component-macros 0.9.1
gpui-file-explorer-poc 0.1.0
gpui-kit 0.9.1
gpui-kit-assets 0.9.1
gpui-pre 0.3.12
gpui-pre-apple 0.3.12
gpui-pre-collections 0.3.12
gpui-pre-derive-refineable 0.3.12
gpui-pre-http-client 0.3.12
gpui-pre-ios 0.3.12
gpui-pre-linux 0.3.12      ← Linux 実行ファイルに実際にリンク
gpui-pre-macos 0.3.12
gpui-pre-macros 0.3.12
gpui-pre-platform 0.3.12
gpui-pre-refineable 0.3.12
gpui-pre-reqwest 0.12.15
gpui-pre-scheduler 0.3.12
gpui-pre-shared-string 0.3.12
gpui-pre-sum-tree 0.3.12
gpui-pre-util 0.3.12
gpui-pre-web 0.3.12
gpui-pre-wgpu 0.3.12
gpui-pre-windows 0.3.12
gpui-pre-ztracing 0.3.12
gpui-pre-ztracing-macro 0.3.12
```

（`gpui-pre-macos/apple/ios/windows/web` は lock には載るが、Linux ではコンパイル対象外）

### ツールチェーン

```
rustc 1.99.1 (58cf8f9c3 2026-10-13)
cargo 1.99.1 (5f94df478 2026-08-27)
edition 2021 / 依存含む総クレート数 約525（debug）
```

### スタックB（フォールバック）を使わなかった理由

`gpui-pre =0.3.12` + `gpui-pre-platform =0.3.12` を直接使う構成は未使用。
スタックAでコンパイルまで到達し、gpui-kit が x11/wayland feature を固定で有効化しているため
実質同一のビルドになる。ただし後述の `libxkbcommon-x11` 問題を避けたい場合、
**B で `gpui-pre-platform` の features を `["wayland"]` のみにすれば X11 リンク自体が不要**になる
（未検証。次に試すこと参照）。

## 2. ビルド結果（時間・サイズ）

実行環境: `work/gpui-file-explorer/poc/`、`CARGO_TARGET_DIR` 未設定（`poc/target/`）。

### 2-1. 依存解決（`cargo fetch`）

```text
$ cargo fetch
real 0m18.036s
```

### 2-2. 初回ビルド（依存 525 クレート + プレースホルダ main）

```text
$ /usr/bin/time -v cargo build
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 31s
  Elapsed (wall clock) time: 1:31.54
  Maximum resident set size: 1,824,564 KB
  Exit status: 0
```
- 警告: **0**（`warning:` 行なし）
- `Compiling` 行数: 525

### 2-3. rustflags 変更後のフルリビルド

```text
$ time cargo build
Finished `dev` profile [unoptimized + debuginfo] target(s) in 1m 34s
real 1m35.720s
```

### 2-4. アプリ実装後の差分ビルド

```text
$ time cargo build
Finished `dev` profile [unoptimized + debuginfo] target(s) in 2.94s
```

### 2-5. サイズ

```text
$ ls -la target/debug/gpui-file-explorer-poc
-rwxrwxr-x 2 testuser testuser 781161720 Nov 11 12:09 target/debug/gpui-file-explorer-poc
$ du -sh target/
12G   target/     # debug 8.3G → release ビルド追加後
```
- **debug バイナリ 781,161,720 bytes（約 745 MiB）**。GPUI + wgpu + 全 debuginfo で巨大。
- 実用時は `[profile.dev] debug = 1`（または 0）や release ビルドを推奨。
- release ビルド: **3m05s**（`Finished release profile in 3m 05s` / Elapsed 3:05.94）→
  バイナリ **53,528,672 bytes（約 51 MiB）**、`strip` 後 **38,866,192 bytes（約 37 MiB）**。
  debug 比で約 1/15。release バイナリでも Wayland 実行・ウィンドウ表示を確認済み（§6）。

### 2-6. ユニットテスト（データ層）

```text
$ cargo test
running 4 tests
test tests::human_age_handles_missing_time ... ok
test tests::human_size_units ... ok
test tests::read_entries_reports_errors ... ok
test tests::read_entries_lists_dirs_first_with_metadata ... ok
test result: ok. 4 passed; 0 failed; 0 ignored
```
※ GPUI の test-support（headless なリスト更新テスト）は採用せず（§4・§5 参照）。

## 3. 起動・ウィンドウ検証の証拠

### 3-1. 実行

```bash
cd work/gpui-file-explorer/poc
./target/debug/gpui-file-explorer-poc "$HOME" > /tmp/poc-run1.log 2>&1 &
```

### 3-2. プロセス生存 + stderr

```text
app pid=3528188 (wrapper) / 実際のバイナリ pid=3528191
kill -0 → ALIVE
$ wc -l /tmp/poc-run1.log
0 /tmp/poc-run1.log
```
**stderr 0 行**: EGL / Vulkan / Wayland / GPU 関連のエラー・警告なし。

### 3-3. ウィンドウ実在確認（KWin D-Bus。`work/zed/env-audit.md` のレシピ）

Wayland ネイティブ窓は `xwininfo -root -tree` には出ないため、KRunner の windows runner を使用。

```bash
$ qdbus6 --literal org.kde.KWin /WindowsRunner org.kde.krunner1.Match "GPUI"
[Argument: a(sssida{sv}) {[Argument: (sssida{sv})
  "0_{f58fa8bd-f4ab-4a1d-acf4-3ba747f688c7}",
  "GPUI File Explorer PoC", "wayland", 100, 0.8, ...]}]
```
→ タイトル `GPUI File Explorer PoC`、**"wayland"**（ネイティブ Wayland）で検出。

```bash
$ qdbus6 --literal org.kde.KWin /KWin getWindowInfo "{f58fa8bd-f4ab-4a1d-acf4-3ba747f688c7}"
[Argument: a{sv} {
  "caption" = "GPUI File Explorer PoC",
  "desktopFile" = "gpui-file-explorer-poc",
  "resourceClass" = "gpui-file-explorer-poc",
  "resourceName" = "gpui-file-explorer-poc",
  "width" = 760.0, "height" = 548.0, "x" = 900.0, "y" = 419.0,
  "minimized" = false, "fullscreen" = false, "layer" = 2, "type" = 0,
  "skipTaskbar" = false, "uuid" = "{f58fa8bd-f4ab-4a1d-acf4-3ba747f688c7}" }]
```
- ウィンドウサイズ 760x548（コンテンツ 760x520 + タイトルバー）、**フルスクリーンでない**、
  通常ウィンドウ（type=0, layer=2）。
- `WindowOptions.app_id = "gpui-file-explorer-poc"` が Wayland の app_id →
  KWin の resourceClass/desktopFile に反映されることを確認（API メモとして有用）。

### 3-4. スクリーンショット（XDG Desktop Portal）

```bash
$ gdbus call --session --dest org.freedesktop.portal.Desktop \
    --object-path /org/freedesktop/portal/desktop \
    --method org.freedesktop.portal.Screenshot.Screenshot "" "{}"
(objectpath '/org/freedesktop/portal/desktop/request/1_1889/t',)
# 保存: /home/testuser/picture/Screenshot_20261111_120500.png (2560x1440)
$ cp ~/picture/Screenshot_20261111_120500.png poc/screenshot/wayland-window.png
$ convert ... -crop 820x620+870+390 +repage poc/screenshot/window-crop.png
```

スクリーンショット（`poc/screenshot/window-crop.png`）で確認できた内容:

- タイトルバー: `GPUI File Explorer PoC`
- ヘッダ: `GPUI File Explorer PoC — /home/testuser` / 右に `gpui-kit 0.9.1 / gpui-pre 0.3.12`
- リスト: `DIR .agents / .aitk / .amazon-q.dotfiles.bak / .android / .antigravity-ide ...`
  （ディレクトリ優先ソート、サイズ列はディレクトリ `-`、更新時刻は `2d ago` 等の相対表示）
- 先頭行 `.agents` が選択ハイライト表示
- ステータス行: `167 entries | .agents <dir> 2d ago`（**非同期ロード完了 + 件数表示**）
- フォント: システムフォントで ASCII は問題なく描画（明示的フォント登録なし）

→ 非同期ディレクトリ読込 → 仮想化リスト描画 → ステータス行、が実際に動くことを確認。

### 3-5. 終了

```text
$ kill -TERM <pid> → exited cleanly (SIGTERM)、stderr 追加出力なし
$ qdbus6 .../WindowsRunner Match "GPUI File" → [Argument: a(sssida{sv}) {}]  (空)
```
ウィンドウが消え、クリーン終了を確認。

（デスクトップへの影響: 小さい 760x520 ウィンドウを約 2 分表示したのみ。設定変更なし。）

## 4. API の摩擦と回避策

### 4-1. 【最大の摩擦】リンク時に `libxkbcommon-x11` が見つからない

```text
error: linking with `cc` failed: exit status: 1
  = note: rust-lld: error: unable to find library -lxkbcommon-x11
```

- 原因の連鎖:
  1. `gpui-kit` は `gpui-pre-platform` の features に **`x11` と `wayland` を固定で両方**指定。
  2. `gpui-pre-platform` の `x11` feature → `gpui-pre-linux` の `x11` feature → `xkbcommon` crate の `x11` feature。
  3. `xkbcommon 0.8.0` は `#[link(name = "xkbcommon-x11")]` を持つ。
  4. 本機は `libxkbcommon-x11-0`（`/usr/lib/x86_64-linux-gnu/libxkbcommon-x11.so.0`）は
     インストール済みだが **`libxkbcommon-x11-dev` が無い**ため、バージョン無し
     `libxkbcommon-x11.so` と pkg-config が存在しない（`pkg-config --libs xkbcommon-x11` も失敗）。
- 回避策（sudo なし・システム無変更）:
  - `poc/native-libs/libxkbcommon-x11.so` → システムの `.so.0` への symlink
  - `poc/.cargo/config.toml` に
    `rustflags = ["-L", "native=<abs>/poc/native-libs"]`
  - 実行時はシステムの `libxkbcommon-x11.so.0` が使われるため実害なし。
- 代替案: gpui-kit を使わず `gpui-pre-platform` を `features=["wayland"]` のみで使う
  （X11 バックエンド不要ならこちらがクリーン。未検証）。

### 4-2. `#[test]` が GPUI の `test` マクロに shadow される

`use gpui_kit::*;` は `pub use ::gpui::*;` 経由で **GPUI の `test` proc-macro（`#[gpui::test]`）** を
再エクスポートするため、テストモジュールで `use super::*;` すると組み込み `#[test]` が隠される。

```text
error: recursion limit reached while expanding `#[test]`
   --> src/main.rs:429:5
    = help: consider increasing the recursion limit by adding a `#![recursion_limit = "256"]`
    = note: this error originates in the attribute macro `test`
```
- 回避策: テストモジュールでは glob を使わず `use super::{read_entries, human_size, ...};` と明示 import。
- 正攻法は `test-support` feature を有効化して **`#[gpui_kit::test]`** を使うこと（gpui-kit README 記載）。

### 4-3. バージョン差異 0.3.9（ローカル監査スナップショット）vs 0.3.12（実際にフェッチした crate）

- ローカル `.tmp-deps-audit/crates/up-gpui-pre`（0.3.9）のサンプルは**API の形としては 0.3.12 でも有効**:
  `application().run(|cx| { cx.open_window(WindowOptions{...}, |window, cx| cx.new(...)) })`、
  `uniform_list("id", count, cx.processor(|this, range, window, cx| ...))`。
- ただし 0.3.12 で**実際のソースを確認して合わせた**点:
  - `uniform_list` 第3引数の実シグネチャは
    `impl Fn(Range<usize>, &mut Window, &mut App) -> Vec<R>`。
    `cx.processor` は `Fn(&mut T, E, &mut Window, &mut Context<T>)` に適合させるアダプタ。
  - キーボード選択を可視範囲に追従させるには、0.3.9 のサンプルには無い
    `UniformListScrollHandle::new()` → `.track_scroll(&handle)` → `handle.scroll_to_item(ix, ScrollStrategy::Nearest)` が必要。
  - `WindowOptions` に `app_id: Option<String>`（Wayland app_id）が存在（0.3.9 では未確認）。
  - `Context<T>` は `Deref<Target = App>` なので、listener 内から `cx.background_spawn(...)`・`cx.quit()` が直接使える。
  - 非同期: `let task = cx.background_spawn(async move { fs::read_dir... })` を作り、
    `cx.spawn(async move |this, cx| { let r = task.await; this.update(cx, |this, cx| {...}).ok(); }).detach()` で UI 反映。
  - キー: root div に `.track_focus(&focus_handle)` + `.on_key_down(cx.listener(...))`、
    `event.keystroke.key` の文字列（`"up"/"down"/"enter"/"backspace"/"escape"`）で分岐。
  - クリック: `.on_click(cx.listener(|this, event: &ClickEvent, window, cx| ...))` で
    `event.click_count()` によるダブルクリック判定。
  - フォーカス: `FocusHandle::focus(&self, window: &mut Window, cx: &mut App)` をウィンドウ生成直後に呼ぶ。

### 4-4. crate 名の対応関係（混乱しやすい）

- crates.io の公式 `gpui` は **0.2.2（2025-10-22、古い）**。今回のスタックでは `gpui` を直接依存に書かない。
  package `gpui-pre` の **lib 名が `gpui`** なので、`use gpui::...` はできるが、Cargo.toml に
  `gpui = "0.2.2"` を書くと別物を引くので注意。
- gpui-kit facade の対応:
  - `gpui_kit::*` → gpui（`use gpui_kit::*;` で GPUI API が入る）
  - `gpui_kit::platform` → gpui_platform（`application()`, `headless()` など）
  - `gpui_kit::base` / `gpui_kit::component` / `gpui_kit::assets`
  - 推奨エントリ: `gpui_kit::application().run(|cx| { gpui_kit::init(cx); gpui_kit::open_window(...) })`
    （`open_window` は view を `gpui-base::Root` でラップする）
- 注意: `gpui-kit` default features (`component`, `assets`) は gpui-component / markdown / html5ever 等を
  ビルド対象に含める。今回の PoC は素の div のみで components 未使用。
  ビルド時間短縮には `default-features = false` が使える可能性（未検証）。

### 4-5. アセット・フォント

- 素の div / uniform_list のみなら **AssetSource の登録は不要**（gpui-kit-assets は default feature で
  リンクされるが未使用）。`assets` feature を切っても動く可能性あり（未検証）。
- Linux ネイティブではサンプルの `load_fonts` は no-op。**システムフォント（fontconfig）で描画**され、
  明示的なフォント埋め込みなしで ASCII は正常描画。CJK 表示は未検証。
- `[patch.crates-io]` や vendoring は**不要**だった（Argus の Windows 向け patch 構成は今回不要）。

### 4-6. ビルドサイズ

- debug バイナリ 745 MiB / `target/` 12GB（debug+release 合計）。
  GPUI+wgt の debuginfo が支配的。PoC 用途では許容だが、配布時は release + strip を推奨。

## 5. 未検証事項

1. **キーボード/マウス操作の E2E 検証**: Wayland ネイティブ窓へ合成入力できるツールが無い
   （`kdotool`/`xdotool`/`wmctrl` すべて未導入。`xdotool` は XWayland アプリのみ有効で今回は無効）。
   選択ハイライトの描画はスクショで確認済みだが、Up/Down/Enter/Backspace の実押下は未確認。
   → コード上のハンドラはコンパイル済み・データ層はユニットテスト済み。
2. **GPUI headless テスト（test-support）**: `gpui-kit` に `test-support` feature と
   `#[gpui_kit::test]` / `TestAppContext` があることは確認したが、有効化すると GPUI 一式を
   test-support 付きで再ビルドする必要があり「安価」ではないため今回スキップ。
3. **X11 バックエンドでの実行**（リンクはされるが、Wayland セッションのため未選択）。
5. **CJK ファイル名の描画**（システムフォールバック依存。実機の日本語ファイル名で未確認）。
5. **他プラットフォーム（Windows/macOS）でのビルド・実行**（スタック自体はクロスプラットフォーム設計だが未検証）。
6. 巨大ディレクトリ（数万件）での仮想化性能・スクロール性能。

## 6. release ビルド・実行結果

```text
$ /usr/bin/time -v cargo build --release
Finished `release` profile [optimized] target(s) in 3m 05s
Elapsed (wall clock) time (h:mm:ss or m:ss): 3:05.94
Exit status: 0
```

| ビルド | 時間 | バイナリサイズ |
|---|---|---|
| debug | 初回 1m31s / 差分 2.94s | 781,161,720 B（約 745 MiB） |
| release | 3m05.94s | 53,528,672 B（約 51 MiB） |
| release + `strip` | - | 38,866,192 B（約 37 MiB） |

release バイナリも Wayland で起動確認:

```text
$ ./target/release/gpui-file-explorer-poc "$HOME" > /tmp/poc-run-release.log 2>&1 &
stderr lines: 0
$ qdbus6 .../WindowsRunner Match "GPUI File Explorer"
"0_{175502b1-4073-461d-adcb-6a201141ad72}", "GPUI File Explorer PoC", "wayland", ...
$ qdbus6 .../KWin getWindowInfo "{175502b1-...}"
caption="GPUI File Explorer PoC", resourceClass="gpui-file-explorer-poc",
width=760, height=548, x=900, y=419, minimized=false, fullscreen=false
# kill -TERM 後、windows runner の結果は空 ({}) に戻りクリーン終了
```

- `target/` 合計は debug+release で **約 12GB**。debuginfo を削る設定（`[profile.dev] debug=0`）や
  `CARGO_TARGET_DIR` の共有を検討する価値あり。

## 7. 次に試すこと

1. `kdotool`（または KDE の D-Bus 経由入力）を導入し、Up/Down/Enter/Backspace の E2E を自動検証。
   併せて `#[gpui_kit::test]` + `test-support` で headless なリスト更新テストを書く。
2. **スタックB（`gpui-pre-platform` を `features=["wayland"]` のみ）**でビルドし、
   `libxkbcommon-x11` リンク要件を消せるか確認（開発環境のセットアップ簡素化）。
3. `cargo build --release` + `strip` で配布サイズを計測。`[profile.dev] debug = 0/1` も比較。
4. `/usr/lib` など巨大ディレクトリで 10 万件規模の virtualization / スクロール性能を計測。
5. gpui-kit の `component`（List/Table）との実装コスト・見た目比較。PoC は素の `uniform_list` が最小。
6. Windows（Argus の vendor patch 構成）との差分、`std::fs` 以外のクロスプラットフォーム FS API
   （ファイル属性・シンボリックリンク・ごみ箱）の扱いを整理。
7. `gpui-kit` default-features（component/assets）を切った場合のビルド時間・サイズ削減効果を確認。

## 付録: 再現コマンド

```bash
cd work/gpui-file-explorer/poc
cargo fetch                 # 18s
cargo build                 # 初回 1m31s（依存込み）
cargo test                  # 4 tests
./target/debug/gpui-file-explorer-poc "$HOME" &   # Wayland 実行
# ウィンドウ確認
qdbus6 --literal org.kde.KWin /WindowsRunner org.kde.krunner1.Match "GPUI"
qdbus6 --literal org.kde.KWin /KWin getWindowInfo "{<UUID>}"
# 終了
kill -TERM <pid>
```

ファイル:
- `poc/src/main.rs` … アプリ本体（約400行、うちテスト約60行）
- `poc/Cargo.toml` … `gpui-kit = "=0.9.1"` のみ
- `poc/.cargo/config.toml` … `libxkbcommon-x11` ローカル symlink 用 `-L native=...`
- `poc/native-libs/libxkbcommon-x11.so` … システム `.so.0` への symlink
- `poc/screenshot/wayland-window.png` / `window-crop.png` … 実行証拠
