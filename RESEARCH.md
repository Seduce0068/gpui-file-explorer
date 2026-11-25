# GPUI でクロスプラットフォーム・ファイルエクスプローラーを自作する際の要点（調査レポート）

- 調査日: **2026-11-11**（Asia/Tokyo）
- 対象: Rust + GPUI で Linux / Windows / macOS 向けのデスクトップ・ファイルエクスプローラーを新規に作る場合
- 検証環境: Kubuntu 2026.04（KDE Plasma 6 / Wayland / NVIDIA GPU）、Rust 1.99.1、複数コア / 十分な RAM
- 先行コンテキスト: 同日未明に GPUI 系 crate の依存監査（`.tmp-deps-audit/`）、前日に Zed 1.27 の設定・環境調査（`work/zed/`）を実施済み。本レポートはその続きとして「自作アプリの設計判断」に必要な要点をまとめたもの。

**関連ノート（詳細な一次調査。本レポートは要点の統合版）**

| ファイル | 内容 | 調査担当 |
|---|---|---|
| [`notes/api-notes.md`](notes/api-notes.md) | GPUI 0.3.x の API 棚卸し（起動・仮想リスト・入力・D&D・クリップボード・非同期・アイコン等。ファイル:行の根拠付き） | gpui-api-scout |
| [`notes/ecosystem-notes.md`](notes/ecosystem-notes.md) | 依存スタック選定、gpui-kit コンポーネント表、ビルド要件、配布、供給網・ライセンス | gpui-ecosystem-scout |
| [`notes/fs-cross-platform-notes.md`](notes/fs-cross-platform-notes.md) | ファイルシステム/OS 統合（列挙・監視・アイコン・ゴミ箱・クリップボード・パス差分・crate 選定表） | fs-platform-scout |
| [`poc/`](poc/) / [`notes/poc-report.md`](notes/poc-report.md) | 実機 PoC（ビルド・起動検証。付録 A） | gpui-poc-builder |

> 表記: **✅** = 本調査で一次情報（ソース/実機）を確認、**⚠** = 未確認・要検証。GPUI は pre-1.0 のため、版・API・挙動は 2026-11-11 時点の情報である。

---

## 0. 結論サマリ（この 12 項目だけ読めば判断できる）

1. **実現可能性: 高い。ただし「GPUI 単体」ではなく「GPUI Kit スタック」で作るのが現実的。**
   - 公式 `gpui`（crates.io）は **0.2.2（2025-10-22 公開）で約 1 年更新が止まっており**、単体アプリ用のプラットフォーム分離（`gpui_platform` 相当）も無い（✅ crates.io API）。
   - 現在の事実上の最新入口は、Zed のスナップショットを再公開した **`gpui-pre` 0.3.12（zed@d2a4fb6, 2026-11-09, Apache-2.0）** と、そのプラットフォーム選択 crate **`gpui-pre-platform` 0.3.12**（✅ 取得してソース確認）。
   - これを 1 依存に束ねた **`gpui-kit` 0.9.1（2026-11-09, Apache-2.0）** が、UI 部品（仮想 Table/List/Tree/Tabs/PopupMenu/Dialog/Input/Icon/Resizable/Dock/Theme 等 75+）とアセットまで含み、**ファイルエクスプローラーに必要な部品の大半をカバー**する（✅ crate ソース/README、⚠ 実機での見た目・挙動は PoC で確認中）。
2. **推奨スタック（第一候補）**: `gpui-kit = "=0.9.1"`（`gpui-fast` feature は**有効にしない**）。理由は後述 §2。
   - フォールバック: `gpui-pre = "=0.3.12"` + `gpui-pre-platform = "=0.3.12"` を直接使う（UI を完全自前にする場合）。
3. **アーキテクチャの核**: GPUI は「単一 UI スレッド + マルチスレッド background executor」の 2 層。**ファイル I/O はすべて `cx.background_spawn` に逃がし、UI スレッドでは絶対に `read_dir`/`metadata`/画像デコードをしない**（✅ API ノート §6）。ディレクトリ読込は「世代番号 + `Task` の drop でキャンセル」で古い結果を捨てる。
4. **一覧の仮想化**: 固定行高なら `uniform_list`（✅ 0.3.9 ソース）、可変行高なら `list`/`ListState`、アイコングリッドは「1 行 = N セル」を `uniform_list` に流すのが最速。カスタム `Element` は最後の手段（`uniform_list` 実装が参考になる）。
5. **コンテキストメニューは GPUI core に存在しない**（✅ grep 0 件）。`gpui-kit` の `PopupMenu` を使うか、`deferred` + `anchored` + `on_mouse_down_out` で自作する。Zed の `crates/ui` は GPL-3.0-or-later なので**コピー不可**（✅ raw GitHub で確認）。
6. **OS 統合は思ったより GPUI コアが持っている**: ネイティブのファイル選択 `prompt_for_paths` / 保存先 `prompt_for_new_path`、`reveal_path`（ファイラで表示）、`open_with_system`、`open_url`、クリップボードの**ファイルパス表現**（`ClipboardEntry::ExternalPaths`）、OS からのファイルドロップ（`ExternalPaths` / `FileDropEvent`）、アプリ外へのファイルドラッグ（`external_drag_payload`、Wayland 実装確認済み ✅）が core API にある。
7. **Linux（本命環境）は Wayland/X11 とも実装が揃っている**: `gpui-pre-linux` が Wayland（wayland-client）と X11（x11rb）を実装し、レンダラは wgpu、ファイルダイアログは XDG ポータル（ashpd）、通知は notify-rust、アクセシビリティは accesskit_unix、IME（Wayland text-input / X11 XIM）も実装（✅ 取得ソース確認）。GPU は Vulkan 必須で、NVIDIA GPU + NVIDIA proprietary ドライバなら稼働見込み（⚠ PoC で最終確認）。
8. **クロスプラットフォームの本当の難所は UI ではなく FS 差**: 非 UTF-8 ファイル名（Linux）、NFD 正規化（macOS）、大小文字非区別と長パス（Windows）、ネットワーク FS の監視不能、ゴミ箱に入らない FS、大小文字だけの rename、シンボリックリンク判定など。詳細は [`notes/fs-cross-platform-notes.md`](notes/fs-cross-platform-notes.md) の落とし穴 22 項目がそのまま設計チェックリストになる。
9. **ファイル監視は「表示中ディレクトリ + 親、非再帰」が正解**: `notify` 8.2.2 + `notify-debouncer-full` 0.7.0、300〜500 ms デバウンス、自操作の抑制窓、5〜10 秒の定期リコンサイル（イベント溢れ対策）をセットで。再帰 watch は初期実装ではやらない（inotify watch 消費・ネットワーク FS で破綻）。
10. **アイコン戦略**: まず**同梱 SVG フォールバック**（`gpui-kit` の Lucide アイコン 1,818 種 + 拡張子マップ）で成立させる。OS ネイティブアイコン（Linux: freedesktop-icons、Windows: `SHGetFileInfoW`、macOS: `NSWorkspace`）と画像サムネイル（`image` crate）は Phase 1/2。重いので非同期 + キャッシュ + scale をキーに含める。
11. **供給網とライセンスの注意**: `gpui-pre-*` は Zed 公式ではなく **huacnlee（Longbridge）個人アカウントによる Zed スナップショットの再公開**（✅ crates.io owners）。ライセンスは Apache-2.0 で問題ないが、**`=0.3.12` の完全ピン + `Cargo.lock` コミット + 更新は隔週でまとめる**運用が必須（週次で破壊的変更が入る世界）。`gpui-fast` はレンダリング/レイアウトを変える実験的フォークなので MVP では使わない。
12. **MVP の現実的な形**: 「一覧 + ナビゲーション + 監視 + 開く/リビール + ゴミ箱 + コピー/移動」までを Phase 1 とし、検索・サムネイル・タブ/ドック・クラウド連携は Phase 2 以降。ファイル操作の進捗・キャンセル・衝突解決を最初から設計に入れる（後付けは難しい）。

---

## 1. 前提と調査方法

### 1.1 何を作るか（前提の置き方）

「クロスプラットフォーム・ファイルエクスプローラー」の範囲を、以下の 4 層に分けて考える。

| 層 | 内容 | 自作の難易度 |
|---|---|---|
| L1 表示 | 一覧（詳細/アイコン/コンパクト）、仮想スクロール、ソート、フィルタ、選択 | 低〜中（GPUI + gpui-kit でほぼ成立） |
| L2 ナビゲーション | パスバー/ブレッドクラム、戻る/進む、サイドバー（場所/お気に入り）、複数タブ | 中（GPUI に部品は無いが単純な合成で作れる） |
| L3 OS 統合 | 開く/リビール、ゴミ箱、コピー/移動/リネーム、監視、アイコン、ボリューム | 中〜高（crate と OS API の組み合わせ。ここが差別化点） |
| L4 発展 | 検索、サムネイル、プレビュー、アーカイブ、クラウド/ネットワーク、権限管理 | 高（MVP 外） |

本レポートは L1〜L3 を中心に、L4 の入り口まで扱う。

### 1.2 調査方法（根拠の種類）

- **一次ソースの実読**: crates.io から `gpui-pre 0.3.9/0.3.12`、`gpui-pre-platform 0.3.12`、`gpui-pre-linux 0.3.12`、`gpui-kit 0.9.1`、`gpui-component 0.9.1` の crate を取得し、`Cargo.toml`・README・`src/` を直接確認。ローカルには先行監査で展開済みの `gpui-pre 0.3.9` 全文（`.tmp-deps-audit/crates/up-gpui-pre/`）があり、API の行番号根拠はこれに基づく。
- **バージョン・ライセンス・所有者**: crates.io API（`curl -A 'research-agent/1.0' https://crates.io/api/v1/crates/<name>`）で 2026-11-11 に取得。
- **実機**: Kubuntu 2026.04 / KDE Plasma 6 / Wayland / NVIDIA。inotify 上限などを実測（`fs-cross-platform-notes.md`）。GUI 検証は KDE の KWin D-Bus（`qdbus6 org.kde.KWin /KWin getWindowInfo`）と XDG Desktop Portal スクリーンショットの手順を `work/zed/env-audit.md` から再利用。
- **先行監査**: `.tmp-deps-audit/audit.out`（cargo-audit、850 crate、2026-11-11 02:22 実行）と `changed.txt` / `changed-win.txt` / `gpui-pre.diff`。当初「再公開 crate と upstream の差分」と解釈していたが、実際は **crates.io `gpui-pre` と AGPL アプリ Argus の vendor パッチ版との差分**だった（✅ ecosystem ノート §5.2 で訂正）。再公開の忠実性は `[package.metadata.gpui-pre] zed-rev` と Zed commit の照合で検証するのが正しい。
- **制約**: 調査中に自動レビュー基盤の 429 が断続発生し、一部の Web/API 取得が不完全。未取得分は「⚠ 未確認」と明記した。

### 1.3 このレポートで扱わないこと

- Zed 本体の拡張機能としてのファイルエクスプローラー（`project_panel`）の改造（→ GPL-3.0 のため別問題。§11 参照）。
- Tauri / Dioxus / egui / Iced 等との比較選定（ユーザーの前提が「GPUI で作る」なので割愛）。
- 課金/同期/クラウド API の設計。

---

## 2. 依存スタックの選定（最初に決めるべき最重要事項）

### 2.1 事実関係（2026-11-11 時点、✅ crates.io API + crate ソース）

| crate / 経路 | 最新版 | 公開日 | 所有者 | ライセンス | 位置づけ |
|---|---|---|---|---|---|
| `gpui`（公式） | **0.2.2** | **2025-10-22** | Zed チーム（maxbrunsfeld 等） | Apache-2.0 | 約 1 年更新停止。`gpui_platform` 分離前の API。単体アプリ化が面倒。**本番非推奨** |
| `gpui-pre` | **0.3.12** | 2026-11-09 | **huacnlee**（Longbridge） | Apache-2.0 | zed@d2a4fb6 のスナップショット。zed の `gpui` crate を正規化して再公開 |
| `gpui-pre-platform` | 0.3.12 | 2026-11-09 | 同上 | Apache-2.0 | `gpui-pre-linux` / `-windows` / `-macos` / `-web` / `-ios` / `-wgpu` を OS で切り替えるメタ crate。`application()` / `headless()` を提供 |
| `gpui-pre-linux` | 0.3.12 | 2026-11-09 | 同上 | Apache-2.0 | Wayland/X11 バックエンド、wgpu レンダラ、ashpd、accesskit_unix、notify-rust |
| `gpui-kit` | **0.9.1** | 2026-11-09 | huacnlee（Longbridge） | Apache-2.0 | `gpui-pre` + `gpui-pre-platform` + `gpui-base` + `gpui-component` + アセットを 1 依存に束ねたもの |
| `gpui-component` | 0.9.1 | 2026-11-09 | 同上 | Apache-2.0 | 75+ のスタイル付きコンポーネント（DataTable/VirtualList/Tree/Tabs/PopupMenu/Dialog/Input/Icon/Resizable/Dock/Theme 等） |
| `gpui-base` | 0.9.1 | 2026-11-09 | 同上 | Apache-2.0 | スタイル無しの挙動・状態・基盤（ElementExt、履歴、フォーカス管理など） |
| `gpui-fast` | 0.1.14 | 2026-11-09 | 同上 | Apache-2.0 | GPUI の性能特化フォーク（retained mode / scroll layers / native composition）。実験的 |

- `gpui-kit` の依存は `gpui-pre = "=0.3.12"` / `gpui-pre-platform = "=0.3.12"` と**完全一致ピン**（✅ `gpui-kit 0.9.1` の Cargo.toml）。アプリ側で GPUI 本体を直接指定しなくてよい。
- **重要な落とし穴**: `gpui-pre-platform` の既定 feature は **`multithreaded` のみ**。Linux で直接使う場合は `wayland` / `x11` を明示的に有効化しないと**起動時に `unreachable!()` で落ちる**（✅ ecosystem ノート §1、実アプリ hoja のコメントでも確認）。`gpui-kit` は既定で正しく有効化済み。
- 公式 `gpui` が古い理由は「Zed のワークスペース crate はそのままでは crates.io で使いにくい（workspace 継承/path 依存）」ため。`gpui-pre` はその正規化版で、**Zed のリポジトリを git 依存するより実用的**（✅ ecosystem ノート §1）。git 依存は workspace 継承のためそのままでは成立しにくい（⚠ 未試行）。

### 2.2 推奨: `gpui-kit = "=0.9.1"`

```toml
[package]
name = "my-file-explorer"
version = "0.1.0"
edition = "2024"
rust-version = "1.99"

[dependencies]
gpui-kit = "=0.9.1"            # GPUI 本体 + platform + base + component + assets
serde = { version = "1", features = ["derive"] }
serde_json = "1"
notify = "8.2"                 # 監視（gpui-kit 自身はテーマ用にしか使っていない）
notify-debouncer-full = "0.7"
trash = "5.3"
opener = { version = "0.9", features = ["reveal"] }
alphanumeric-sort = "1.5"
# Phase 1 以降: arboard (file_list), sysinfo, freedesktop-icons, image, ...
```

```rust
// main.rs の最小骨格（gpui-kit 0.9.1 の README に準拠）
use gpui_kit::*;

struct App;

impl Render for App {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full().child("Hello, file explorer")
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets) // Lucide 既定アイコン
        .run(|cx| {
            gpui_kit::init(cx); // テーマ/フォーカス等の初期化。二重呼び出し注意
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| App))
                .expect("failed to open window");
        });
}
```

- 直接 `gpui-pre` + `gpui-pre-platform` を使う場合の起動は `gpui_platform::application().run(|cx: &mut App| { ... })`（✅ ローカル `examples/hello_world.rs`）。`headless()` も用意されている（テスト用）。
- **`gpui-fast` は有効にしない**。retained mode により「追跡外の状態変更は `cx.notify()` が必須」になる・レイアウト/描画が upstream と変わる・供給網が 1 段増える、というリスクがある（✅ ecosystem ノート §5.4）。性能問題が実測で出たときだけ feature で A/B する。

### 2.3 フォールバック

| 優先 | スタック | 使いどころ |
|---|---|---|
| B1 | `gpui-pre = "=0.3.12"` + `gpui-pre-platform = { version = "=0.3.12", features = ["wayland","x11"] }` | gpui-kit の見た目・抽象化を避け、UI を完全自前設計する場合。**feature 明示が必須** |
| B2 | 公式 `gpui = "=0.2.2"` | 学習・実験用のみ。`Application::new()` 時代の API で、単体アプリ化に追加作業が必要 |
| B3 | `zed-industries/zed` への git 依存 | 最新 upstream に追従したい場合の最終手段（workspace 継承のため難航が予想される） |
| B4 | `gpui-kit` + `features = ["gpui-fast"]` | 描画性能が問題になった後の最適化オプション |

### 2.4 受け入れる必要がある前提

1. **pre-1.0 / 週次破壊的変更**: `gpui-kit` は 2026-09-03 の 0.1.0 から 0.9.1 まで約 2 か月で 15 リリース。`=x.y.z` 完全ピン + `Cargo.lock` コミット + 更新は隔週でまとめる（✅ ecosystem ノート §5.3。caret 指定で当日ビルド不能になった実事故あり）。
2. **第三者ミラーへの信頼**: `gpui-pre-*` の公開主体は Zed ではない（huacnlee）。中身は Zed の Apache-2.0 コードのスナップショットだが、長期保守は Zed 公式保証ではない。→ 供給網チェック（§11）と、必要なら「使う版のソースを vendor して差分監査」を行う。
3. **コンポーネントの穴**: ブレッドクラム専用部品なし、OS ネイティブのファイルドロップ専用部品なし、OS ファイルアイコン/サムネイル部品なし。→ 本レポートの設計で自前実装箇所を明示する。
4. **監査の限界**: 先行 cargo-audit は「脆弱性 0 件」だが、未整備 8 件 + ネットワーク失敗 367 件で**不完全**。リリース前に再実行する。

### 2.5 既存の GPUI 製ファイルマネージャー（2026-11-11 時点の実査、13 件）

ecosystem scout が GitHub を調査した結果、GPUI 製のファイルマネージャー/エクスプローラーは既に複数存在し、**スタックは多様**:

| 例 | スタック | 学び |
|---|---|---|
| FileForce / files | `gpui-kit 0.8.1` | gpui-kit でファイルマネージャーが実際に作れる実例。部品選定の参考 |
| shuffle / rust-explorer | 公式 `gpui 0.2.2` | 古い公式 crate でも小規模なら成立するが、最新 API/部品は使えない |
| skwir | Zed の gpui を vendoring | upstream 追従を自前で行うパターン |
| hoja (slastra) | Zed git 依存（rev 固定、`gpui` + `gpui_platform` を features wayland/x11 で） | **git 依存は可能**。ただし Zed の GPL crate（theme 等）を引くとアプリ全体が GPL-3.0 化する実例 |
| Atlas | gpui-component + `gpui-ce` を vendor | 別系統のコミュニティ版 GPUI。AGPL なので参考のみ |
| huacnlee 自身の Omarchy 向け FM | GPUI Shell + JS | タブ/スプリット/仮想リスト/プレビュー/ゴミ箱/テーマ。今後の部品充実の方向性 |

- **Linux まで含めたクロスプラットフォーム対応の既存例はまだ少数**。したがって「gpui-kit + 自前の OS 統合層」で作る本レポートの方針は妥当。
- 既存実装は「UI の作り方」の参考になるが、**コードのライセンス（GPL/AGPL）を必ず確認**すること。特に hoja の git 依存パターンは、GPL crate を引いた時点でアプリのライセンスが変わる。
- 参考 URL は [`notes/ecosystem-notes.md`](notes/ecosystem-notes.md) §6。

---

## 3. アプリの骨格（起動・状態・非同期の設計）

### 3.1 実行モデル（ここを外すと全部崩れる）

GPUI の実行モデルは 2 層（✅ api-notes §6.1）:

- **foreground（UI スレッド、単一）**: `App` / `Window` / `Entity` / `Context`。UI 型は `Send` ではない。
- **background（マルチスレッド executor）**: `Send` な future のみ。`cx.background_spawn(async { ... })`。
- 橋渡し: `AsyncApp::update_entity(&weak, |this, cx| { ... })` で UI スレッドに戻る。`cx.notify()` で再描画。
- キャンセル: `Task` を drop すると future がキャンセルされる。`WeakEntity` を使った `cx.spawn` は entity drop で自動キャンセル。

**原則**: `std::fs` / 画像デコード / アイコン抽出 / 検索は**必ず background**。UI スレッドで 1 回でも 10 ms 超の処理をすると 60/120 fps が落ちる。

### 3.2 Entity 構成（推奨）

```
RootView (Render)
├── ExplorerModel (Entity)         … 現在パス、履歴、設定、監視ハンドル
│   ├── DirListing (Entity)        … 表示中ディレクトリのエントリ + ソート + フィルタ
│   │   └── Entry のモデル         … PathBuf, file_type, size, mtime, icon_key
│   ├── Selection (Entity)         … 単一/複数選択、アンカー、キーボードフォーカス
│   ├── VolumeList (Entity)        … マウント/ドライブ一覧
│   └── ClipboardState (Entity)    … cut/copy のアプリ内状態（OS クリップボードと別）
├── SidebarView (View)             … 場所/お気に入り（List/Sidebar 部品）
├── PathBarView (View)             … ブレッドクラム + パス入力
├── ListView (View)                … DataTable / VirtualList / uniform_list
└── StatusBarView (View)
```

- 小さく始めるなら RootView 1 個 + `DirListing` だけで良い。**「ディレクトリの状態」と「UI 部品」を Entity で分ける**と、監視イベントでの差分更新とテストが楽になる。
- クロス Entity の通知は `EventEmitter<T>` + `cx.subscribe`（✅ api-notes §6.3）。例: `DirListing` が `Changed` を emit → RootView が再描画。
- 設定の永続化は core に無い。`serde` + `directories`/`etcetera` クレート等で自前（⚠ 未選定）。

### 3.3 Action / KeyBinding（キーボード操作の土台）

```rust
actions!(explorer, [GoBack, GoForward, GoUp, OpenSelected, RenameSelected, DeleteSelected, NewFolder, Copy, Cut, Paste, ToggleHidden]);

// 起動時
cx.bind_keys([
    KeyBinding::new("alt-left", GoBack, Some("Explorer")),
    KeyBinding::new("enter", OpenSelected, Some("Explorer")),
    KeyBinding::new("f2", RenameSelected, Some("Explorer")),
    KeyBinding::new("delete", DeleteSelected, Some("Explorer")),
    KeyBinding::new("ctrl-h", ToggleHidden, Some("Explorer")),
]);

// ルート要素
div()
    .key_context("Explorer")        // KeyBinding の context と一致させる
    .track_focus(&self.focus_handle)
    .on_action(cx.listener(Self::on_go_back))
    .on_action(cx.listener(Self::on_open_selected))
    // ...
```

- キー配送は「フォーカスされている要素」から `key_context` に沿って行われる。**フォーカスを失うとショートカットが死ぬ**ので、リストのクリックで `focus_handle.focus(window, cx)` する（✅ api-notes §3.2 / `docs/key_dispatch.md`）。
- ダブルクリックは `MouseDownEvent.click_count == 2`（✅ `src/interactive.rs:159`）。`on_click` の `ClickEvent` にも `click_count` がある。
- 修飾キー（Ctrl/Shift）での複数選択は `Modifiers` を見て自前実装。GPUI に選択モデルは無い。

### 3.4 ディレクトリ読込の定石（世代 + キャンセル）

```rust
fn navigate_to(&mut self, path: PathBuf, cx: &mut Context<Self>) {
    self.generation += 1;
    let generation = self.generation;
    self.loading = true;

    // 前のタスクを drop = キャンセル
    self.load_task = Some(cx.spawn(|this: WeakEntity<Self>, mut cx: AsyncApp| async move {
        let dir = path.clone();
        let entries = cx.background_spawn(async move {
            // ここは Send。read_dir + file_type だけでまず一覧を作る（metadata は必要列のみ後続で）
            std::fs::read_dir(&dir)
                .map(|rd| {
                    rd.filter_map(|e| e.ok())
                        .map(|e| {
                            let p = e.path();
                            let ft = e.file_type().ok();
                            Entry { path: p, is_dir: ft.map(|t| t.is_dir()).unwrap_or(false), ..Default::default() }
                        })
                        .collect::<Vec<_>>()
                })
        }).await;

        let _ = cx.update_entity(&this, |this, cx| {
            if this.generation != generation { return; } // 古い結果は捨てる
            this.apply_entries(entries.unwrap_or_default());
            this.loading = false;
            cx.notify();
        });
    }));
}
```

- ポイント: (a) `background_spawn` の中で UI 型を触らない、(b) 世代番号で古い結果を破棄、(c) `Task` を `Option` で持ち、新規読込時に `take()` して drop（=キャンセル）、(d) `read_dir` のエラーは「空ディレクトリ」ではなくエラー状態として UI に出す（権限拒否・削除済み等）。
- 大量ディレクトリ（10 万件）では、まず **名前と種別だけ** 読んで即描画し、サイズ/更新時刻/アイコンは可視範囲 + 逐次で埋める（2 パス方式）。`metadata()` を全件で呼ぶと遅い（`DT_UNKNOWN` の FS では特に）。

### 3.5 監視（watch）→ 差分更新

- `notify 8.2.2` + `notify-debouncer-full 0.7.0` を **background スレッド**で起動し、イベントを `async_channel`/`std::sync::mpsc` で foreground に渡す。GPUI に FS 監視 API は無い（✅ fs-notes §2）。
- 監視対象は **表示中ディレクトリ（非再帰）+ 親ディレクトリ（非再帰）**。300〜500 ms デバウンス、自操作は 1.5 秒の抑制窓、5〜10 秒ごとにリコンサイル（`Rescan` / overflow 対策）。
- 差分適用は `ListState::splice`（可変リスト）または `reset` + `scroll_to_item`。`uniform_list` の場合は件数変更時に作り直しになるため、選択・スクロール位置を保存して復元する。
- 自操作（rename/copy/delete）後は「イベントを待たずに楽観更新 → 監視イベントは抑制窓で捨てる」方がちらつかない。

---

## 4. 一覧 UI の実装要点

### 4.1 仮想化の選択

| 表示 | 使う API | 根拠 |
|---|---|---|
| 詳細リスト（固定行高） | `uniform_list(id, count, |range, window, cx| -> Vec<_>)`。可視範囲だけ構築される | ✅ `src/elements/uniform_list.rs:22`。`.track_scroll(&UniformListScrollHandle)` + `scroll_to_item(ix, ScrollStrategy)` |
| 詳細リスト（行高が可変） | `list(ListState, |ix, window, cx| -> AnyElement)`。`ListState::new(count, ListAlignment::Top, overdraw)` | ✅ `src/elements/list.rs:24,314`。`splice`/`reset` で差分更新 |
| アイコングリッド | 「1 行 = N セル」の行を `uniform_list` で仮想化（`item_count = ceil(n/cols)`） | ✅ api-notes §2.3。列数・行高が変わるときだけ再構築 |
| 完全自由レイアウト | カスタム `Element`（`request_layout`/`prepaint`/`paint`） | ✅ `Element` トレイト。`uniform_list` 実装が参考 |
| 大量列テーブル | `gpui-kit` の `DataTable`（仮想スクロール・列リサイズ/固定・ソート・セル選択・右クリック） | ⚠ docs/README ベース。PoC で実機確認中 |

- **最初は `uniform_list` で十分**。ファイル一覧は行高固定が自然で、`DataTable` にすると列ヘッダ・ソート・列幅の扱いが楽になる（gpui-kit 採用時）。
- `uniform_list` をスクロール領域に置くには親に `overflow_y: hidden` + 固定/最大高さが必要（実装コメント）。`h_full()` 等で高さを確定させる。
- キーボードで選択を動かすときは `scroll_to_item`（`ScrollStrategy::Nearest`）で追従。

### 4.2 行（Entry）の描画

```rust
fn render_row(entry: &Entry, selected: bool, focused: bool, cx: &mut Context<App>) -> impl IntoElement {
    div()
        .id(entry.id())
        .h(px(28.))
        .px_2()
        .flex().items_center().gap_2()
        .when(selected, |d| d.bg(rgb(0x2d5f8a)))
        .when(focused, |d| d.border_1().border_color(rgb(0x5aa9e6)))
        .hover(|d| d.bg(rgb(0x2a2a2a)))
        .on_click(cx.listener(move |this, ev: &ClickEvent, window, cx| {
            this.select(entry.id(), ev.modifiers());
            if ev.click_count() == 2 { this.open(entry.path(), cx); }
        }))
        .child(icon_for(&entry.icon_key))          // svg()/Icon 部品。後述
        .child(div().flex_1().min_w_0().truncate().child(entry.display_name())) // 省略表示
        .child(div().w(px(90.)).text_right().child(entry.size_label()))
        .child(div().w(px(150.)).text_right().child(entry.mtime_label()))
}
```

- `display_name()` は **`OsStr` を保持したまま `to_string_lossy()` で表示**（Linux の非 UTF-8 名で壊さない）。ソート・検索は `OsStr`/正規化済みキーで行う。
- ファイル名は `.truncate()`（`overflow_hidden` + `whitespace_nowrap` 相当）で 1 行省略。`TextOverflow` / `Styled::truncate` が core にある（✅ api-notes §7.3）。
- 色・角丸・フォントは `gpui-kit` の `Theme`/`Root` に寄せるとライト/ダーク対応が楽。core にテーマ機構は無い。
- 選択行の背景・ホバー・フォーカス枠は自前。GPUI の `:hover` は `.hover(...)`、フォーカスは `FocusHandle`。

### 4.3 コンテキストメニュー（右クリック）

GPUI core には**コンテキストメニュー部品が無い**（✅ `grep -rn "ContextMenu|context_menu" src/` = 0 件）。選択肢は 3 つ:

1. **`gpui-kit` の `PopupMenu`**（`/component/menu`）— ContextMenu/DropdownMenu、アイコン、ショートカット表示、サブメニュー、チェック状態、区切り。**推奨**。
2. 自作: ルートに `deferred(...)` でメニューを重ね、`anchored()` でマウス位置に配置、ルート要素の `on_mouse_down_out` で閉じる。`WindowKind::AnchoredPopup(PopupOptions)` で OS ネイティブのポップアップウィンドウとして出す方法もある（✅ `src/platform/popup.rs:16`。`grab: true` で外側クリック/Esc dismiss、Wayland の xdg_popup 対応。**マウスダウンから開く必要あり**）。
3. Zed の `crates/ui` の `ContextMenu` を参考にする — **コードは GPL-3.0-or-later なのでコピー不可**（✅ raw GitHub 確認）。発想だけ参考にする。

メニュー項目の例: 開く / 既定のアプリで開く / 新しいウィンドウで開く / パスをコピー / 切り取り / コピー / 貼り付け / 名前変更 / ゴミ箱へ移動 / 完全に削除 / プロパティ / リビール（ファイルマネージャで表示）。

### 4.4 ブレッドクラム・サイドバー・タブ・ステータスバー

| 部品 | 実装方針 |
|---|---|
| ブレッドクラム | **専用部品なし**（gpui-kit の docs 索引にも無い ✅）。`h_flex` + パス区切り + 各セグメントをボタン化。長いパスは中央省略。パス入力モード（Ctrl+L）は `Input` に切り替え |
| サイドバー | `gpui-kit` の `Sidebar`（折りたたみ・ネスト・バッジ）または `List`。場所（ホーム/デスクトップ/ダウンロード/ドキュメント/ゴミ箱/マウント） |
| タブ | `gpui-kit` の `Tabs`（variant/アイコン/動的追加/メニュー/最大幅） |
| ツールバー | `gpui-kit` の `Toolbar`（戻る/進む/上へ/更新/表示切替/検索） |
| ステータスバー | `gpui-kit` の `StatusBar`（件数/選択数/空き容量/転送中） |
| パネル分割 | `gpui-kit` の `Resizable`（ツリー/一覧/詳細の 3 ペイン）。`Dock` は MVP では過剰 |
| 空状態/読込中 | `Empty` / `Skeleton` / `Spinner`（gpui-kit） |

### 4.5 入力（検索・リネーム・パス入力）と IME（日本語）

- 日本語入力は「`on_key_down` + `key_char` を自前で組み立てる」と壊れる。**GPUI core にはテキスト入力ウィジェットが無く、IME 配管（`EntityInputHandler` / `ElementInputHandler` / `Window::handle_input`）だけがある**（✅ api-notes §8.4 / §10）。**gpui-kit の `Input` を使う**のが正解で、core だけで作る場合は `EntityInputHandler` の実装が必要になり、自作コストが最も高い部品の 1 つになる。
- Linux の IME は `gpui-pre-linux` が Wayland の `text-input` と X11 の XIM を実装済み（✅ 取得ソース: `linux/wayland/window.rs` の `enable_ime`/`handle_ime`、`x11/xim_handler.rs`）。fcitx5/mozc での実機確認は ⚠ 未実施。
- リネームは「別ウィンドウ + Input」または「行内に Input を重ねる」のどちらか。OS の `prompt_for_new_path` は保存ダイアログなのでリネームには使わない。
- IME 候補ウィンドウの位置は GPUI がテキストカーソル位置から計算する（API 詳細 ⚠ 未確認）。自前描画のエディタを作る場合は `EntityInputHandler` の実装が必要になる可能性が高い（⚠）。

### 4.6 アイコン

- **MVP**: `gpui-kit` の `Icon`（Lucide 1,818 種 + 独自 12 種、既定 101 個は 44 KiB）と `gpui-kit-assets`。拡張子 → `IconName` のマップを自前で持つ（フォルダ/画像/動画/音声/書庫/コード/PDF/テキスト/不明）。
- **Phase 1**: OS ネイティブアイコン。
  - Linux: `freedesktop-icons` 0.4.0（icon-theme-spec 準拠、SVG/scale/cache, MIT）。テーマ未設定時のフォールバックに自前 SVG。
  - Windows: `SHGetFileInfoW` / `SHGetImageList(SHIL_JUMBO)`（COM STA 注意）。
  - macOS: `NSWorkspace.iconForFile`（`objc2-app-kit` 0.3.2）。
- **キャッシュ**: キーは `(icon_key, pixel_size, scale_factor)`。HiDPI/モニタ移動で再取得。取得は background + プレースホルダ（先にフォールバック SVG を出す）。
- 自前 SVG は `AssetSource` に登録し `svg().path("icons/folder.svg")`。`application().with_assets(...)`（✅ api-notes §7.1）。

### 4.7 ドラッグ & ドロップ / クリップボード（GPUI core でどこまでできるか）

| 機能 | GPUI core の状況 | 実装 |
|---|---|---|
| アプリ内 D&D（行の移動） | `on_drag(value, preview_constructor)` + `on_drop::<T>` + `can_drop` | ✅ `src/elements/div.rs:615/568/580` |
| OS → アプリ（ファイルドロップ） | ウィンドウが `FileDropEvent` を内部 D&D に変換する（`Entered` で `ExternalPaths` を active_drag に、`Submit` を MouseUp に変換）。要素側は **`on_drop::<ExternalPaths>`** で受ける | ✅ `src/interactive.rs:737`, `src/window.rs:5656-5707`。**ドロップが MouseUp 変換のため `on_click` も発火しうる**（二重処理注意） |
| アプリ → OS（ファイルを掴んで外へ） | `external_drag_payload(\|value\| Some(ExternalDragPayload::Files(FileDragPaths::new(...))))` | ✅ `src/interactive.rs:705`。**Linux Wayland 実装確認済み**（`text/uri-list`）。X11/Windows/macOS の実装状況は ⚠ 要確認 |
| クリップボード（ファイルコピー） | `ClipboardItem { entries: vec![ClipboardEntry::ExternalPaths(paths)] }` + `write_to_clipboard` / `read_from_clipboard` | ✅ `src/platform.rs:2969,3010`。Linux Wayland は `text/uri-list` と独自 MIME を実装済み（✅ 取得ソース）。X11 は URI_LIST がコメントアウトされており ⚠（テキストのみの可能性） |
| クリップボード（テキスト） | `ClipboardItem::new_string` | ✅ |
| カット/コピー状態 | **OS クリップボードだけでは不十分**。アプリ内状態（cut/copy、対象パス）を保持し、行のグレー表示を自前管理 | 設計で対応 |
| ネイティブファイルダイアログ | `prompt_for_paths(PathPromptOptions{files,directories,multiple,prompt})` / `prompt_for_new_path` | ✅ `src/app.rs:1891,1904`。**拡張子フィルタは無い**（`PathPromptOptions` は files/directories/multiple/prompt のみ）。Linux は XDG ポータル（ashpd）で、ポータルが無い環境ではエラーになりうる |
| 開く/リビール | `open_with_system(path)` / `reveal_path(path)` / `open_url(url)` | ✅ `src/app.rs:1809,1913,1918`。Linux 実装は `gpui-pre-linux/src/linux/platform.rs` に存在（✅） |

**設計メモ**: OS からのファイルドロップは、**要素側は `on_drop::<ExternalPaths>` で受ける**のが正攻法（Window が `FileDropEvent` を内部 D&D に変換する）。`Submit` が MouseUp に変換されるため `on_click` も同時に発火しうる点に注意（ドロップ処理とクリック処理の二重実行を防ぐ）。複数ファイルは `ExternalPaths::paths()` を回す。

---

## 5. 非同期 I/O・状態管理の実践

### 5.1 データフロー（推奨パターン）

```
[ユーザー操作]                    [foreground (UI thread)]                 [background executor]
クリック/キー/監視イベント
      │                                   │                                      │
      ▼                                   ▼                                      │
 on_click/on_action ──────────▶ 状態を楽観更新（選択・スクロール保存）             │
                                      │  Task を take()（前処理キャンセル）          │
                                      │  cx.spawn(async {                     │
                                      │      let data = cx.background_spawn(  │
                                      │          read_dir / metadata /       │
                                      │          icon / decode / search       │
                                      │      ).await;                         │
                                      │      cx.update_entity(&weak, |this,..│
                                      │          if generation OK {           │
                                      │              splice/reset + notify     │
                                      │          }                            │
                                      │      })                               │
                                      ▼                                      ▼
                                  cx.notify() ──▶ 再描画（可視範囲だけ）
```

- **楽観更新 → 背景で確定 → 監視で補正**の 3 段構え。rename/delete を「イベント待ち」にすると、監視が効かない FS で UI が固まる。
- 監視イベントは **バッチ化**して 1 フレームに 1 回反映する（毎イベントで `notify` すると暴走する）。`async_channel` で受けて `cx.spawn` のループで `try_recv` を回すか、`std::sync::mpsc::Receiver` を 16 ms 間隔のタイマーで drain する。

### 5.2 ソートとフィルタ

- 自然順（`file2` < `file10`）は `alphanumeric-sort 1.5.8`（`compare_str`）。`sort_by_cached_key` でキーを事前計算（比較関数内で毎回 `to_lowercase` しない）。
- ディレクトリ優先 → 名前順、が既定。列ソート（サイズ/更新時刻/種類）はキーを変えるだけ。**昇順/降順のトグル状態は永続化**。
- ロケール照合（日本語の五十音順・濁点・全角半角）は Phase 2。`icu_collator 2.4.0`（ICU4X, Unicode-3.0）が候補だがデータサイズに注意。OS ネイティブ照合（Windows `CompareStringEx` / macOS `localizedStandardCompare`）は FFI が必要（⚠ 未検討）。
- 隠しファイル: Linux/macOS は `.` 始まり、Windows は `FILE_ATTRIBUTE_HIDDEN` / `SYSTEM`。**表示トグルは 3 OS で条件が違う**ので抽象化する。
- フィルタは `name.to_lowercase().contains(query)` 程度から。大文字小文字・全角半角の正規化をどこまでやるかは仕様判断。

### 5.3 選択モデル

- GPUI に選択モデルは無い。`Selection { anchor: Option<EntryId>, selected: BTreeSet<EntryId>, focus_ix: usize }` を自分で持つ。
- 単一クリック = 単一選択、Ctrl+クリック = トグル、Shift+クリック = 範囲、Ctrl+A = 全選択、矢印キー = 移動（Shift 併用で範囲拡張）。
- **更新後の選択復元**が重要: 監視で一覧が変わっても、パス（`PathBuf`、Windows は case-fold）をキーに選択を維持する。`EntryId` は絶対パス + ファイル ID（取れれば）で安定化。
- キーボードフォーカスと選択は別物。リストがフォーカスを持っているかでキー配送が変わる（§3.3）。

### 5.4 監視イベントの反映

- **イベント種別ごとの最短処理**: `Create` → エントリ追加、`Remove` → 削除、`Modify(Name)` → 同一 inode/file id の rename として差分更新、`Modify(Data/Metadata)` → 該当行だけ更新（サイズ/時刻）。`Rescan`/overflow → ディレクトリ全体を再読込（選択/スクロールは保存）。
- デバウンス後のイベントは **カレントディレクトリに直接影響するものだけ**適用し、それ以外は無視（親ディレクトリの watch は「現在地が消えた/rename された」検知用）。
- 自操作の抑制窓（1〜2 秒）を設けないと、rename 直後に「古い名前の削除 + 新名の追加」が二重適用されてちらつく。
- ネットワーク FS では watch が来ない/遅い。**10 秒リコンサイル + 「自動更新オフ」表示**にフォールバック。

### 5.5 エラー処理

- `read_dir` のエラーは種類ごとに UI を変える: `PermissionDenied`（鍵アイコン + 「アクセスできません」）、`NotFound`（「削除されました」+ 親へ移動）、その他（メッセージ + 再試行）。
- 個別エントリの `metadata()` 失敗は**行を落とさない**（サイズ/時刻を「—」表示）。
- ファイル操作は「1 件失敗しても全体を止めない」。結果を `Vec<Result<...>>` で集約し、失敗一覧をダイアログで出す。
- ログは `tracing` 等でファイル + コンソール。パスをログに出すときは個人情報（ユーザー名・案件名）に注意（配布アプリでは既定オフ）。

---

## 6. ファイルエクスプローラー固有のクロスプラットフォーム要点

> 詳細・根拠 URL・crate のライセンスは [`notes/fs-cross-platform-notes.md`](notes/fs-cross-platform-notes.md)（548 行）を参照。ここでは設計判断に効く要点だけを抜く。

### 6.1 ディレクトリ列挙

- **`std::fs::read_dir` + `DirEntry::file_type()` が最速**。Linux は `getdents` の `d_type`、Windows は `FindFirstFileW` の属性を std がキャッシュするため、`file_type()` に追加 syscall は不要（`DT_UNKNOWN` の FS のみ lstat フォールバック）。✅
- サイズ/更新時刻が必要な列だけ `metadata()` を**並列**取得（`rayon` か、チャンク分割して background で）。全件 `metadata()` は遅い。
- 10 万件は「名前 + 種別だけ先に読む → 可視範囲から詳細を埋める」2 パス方式。`read_dir` 中の削除/rename は `NotFound` を許容。
- `jwalk 0.9.0` は README に **Unmaintained** 明記のため不採用（✅）。再帰検索は `ignore 0.4.34` の並列 walker（`standard_filters(false)` で `.gitignore` 解除）。
- ソートは `sort_by_cached_key`、フィルタは事前正規化したキーで。

### 6.2 ファイル監視（notify）

- **構成**: `notify 8.2.2` + `notify-debouncer-full 0.7.0`（0.7.0 は `notify ^8.2.0` 依存 ✅）。
- **watch 範囲**: 表示中ディレクトリ（非再帰）+ 親ディレクトリ（非再帰）。再帰 watch はサブディレクトリ数分の inotify watch を消費し、`node_modules`/`.git` で破綻する。
- **Linux inotify 実測値（この環境）**: `max_user_watches=518185`, `max_user_instances=1024`, `max_queued_events=16384`。溢れは `ErrorKind::MaxFilesWatch`（ENOSPC）と `EventKind::Other` + `Flag::Rescan` で通知される（✅ ソース確認）。**溢れたら再読込**。
- **Windows**: `ReadDirectoryChangesW`。notify 側のバッファは 16 KB 固定で、溢れの明示通知が無い可能性 → **定期リコンサイル必須**（⚠）。ネットワークドライブは監視不可のことがある。
- **macOS**: FSEvents。`MustScanSubDirs` 等で `Rescan` が来る（✅ ソース確認）。非再帰指定の粒度は ⚠。
- **Debounce**: 300〜500 ms。エディタの保存連打・コピー中の大量イベントをまとめる。自操作抑制 1.5 秒。

### 6.3 パス・ファイル名の地雷

| 問題 | OS | 対策 |
|---|---|---|
| 非 UTF-8 ファイル名 | Linux | **`PathBuf`/`OsString` を保持**。表示だけ `to_string_lossy()`。`String` 経由でパスを作り直さない |
| Unicode 正規化（NFD） | macOS | 表示・検索・重複判定は NFC に正規化して比較。I/O は元のパスを使う。`unicode-normalization 0.1.26` |
| 大小文字の非区別 | Windows, macOS（既定） | 同一名判定は case-fold。`HashMap` キーに生の名前を使わない。case-only rename は 2 段階 rename |
| 長いパス（>260） | Windows | `\\?\` プレフィックス + manifest の `longPathAware`。表示用に `dunce` で簡略化 |
| 予約名・禁止文字 | Windows | `CON`, `PRN`, `AUX`, `NUL`, `COM1`〜, 末尾の `.`/空白、`<>:"/\|?*` の検証 |
| シンボリックリンク/ジャンクション | 全 | 削除・コピーは `symlink_metadata` で判定。リンク先を誤って消さない。再帰削除は自前実装なら特に注意 |
| 権限・不変属性 | Linux/macOS | `EACCES`/`EPERM` を UI に出す。属性解除はユーザー確認の上で |
| ゴミ箱に入らない FS | 全 | ネットワーク/一部リムーバブルで失敗 → 完全削除の確認 UI にフォールバック |

### 6.4 ファイル操作

- **同一 FS 内の移動 = `std::fs::rename`**、`EXDEV`（cross-device）は「コピー → 元を削除」。移動先が既存なら衝突解決 UI。
- **削除は `trash 5.3.0`**（Windows Recycle Bin / macOS Trash / freedesktop Trash spec、`list`/`restore_all`/`purge_all` も提供）。MVP からゴミ箱を使う（誤削除の安全弁）。
- **コピー**: 自前再帰 + 進捗 + キャンセル + エラー集約。`create_new(true)` で衝突回避、置換は「tmp に書いて rename」で原子性を確保。シンボリックリンクはリンクとしてコピーするか中身をコピーするか仕様を決める。
- **進捗 UI**: 件数ベース + バイトベース。キャンセルは `AtomicBool` を background タスクが見る。完了/失敗は `Notification` で通知。
- **切り取り/コピーの区別**: OS クリップボードには「cut か copy か」が乗らない（Linux の `x-special/gnome-copied-files` 先頭行、Windows の `Preferred DropEffect` を読めば取れるが ⚠）。**アプリ内状態で管理**し、OS クリップボードにはパス一覧だけ書く。

### 6.5 ゴミ箱・ボリューム・デスクトップ統合

- **ゴミ箱**: `trash` crate で「削除/一覧/復元/完全削除」。Linux の Trash spec は `~/.local/share/Trash/{files,info}`、Windows は Recycle Bin、macOS は `~/.Trash`。ネットワーク FS は失敗しうる。
- **ボリューム**: MVP は `sysinfo 0.40.1` の `Disks` で列挙 + 空き容量 + 数秒ポーリング。ホットプラグ（udisks2 / `WM_DEVICECHANGE` / `NSWorkspace` 通知）は Phase 2。
- **開く/リビール**: `opener 0.9.0`（`reveal` feature）で 3 OS 対応。GPUI の `open_with_system` / `reveal_path` も使える（実装はプラットフォーム側）。Linux のリビールは `org.freedesktop.FileManager1.ShowItems`（Dolphin/Nautilus 等）。
- **Open With / 既定アプリ列挙**: Phase 2 以降（Windows `SHOpenWithDialog` 等）。
- **特殊ディレクトリ**: `dirs 7.0.0` 等でホーム/デスクトップ/ダウンロード/ドキュメント/ゴミ箱を解決。XDG 環境変数も尊重。

### 6.6 OS 別マトリクス（要約）

| 機能 | Linux (KDE/Wayland) | Windows 11 | macOS |
|---|---|---|---|
| 列挙 | `read_dir`（d_type） | `read_dir`（FindFirstFile 属性） | `read_dir` |
| 監視 | inotify（notify） | RDCW（notify） | FSEvents（notify） |
| アイコン | freedesktop-icons + SVG | `SHGetFileInfoW` + JUMBO | `NSWorkspace` |
| サムネイル | `image` crate | `IShellItemImageFactory`（後） | QuickLook（後） |
| ゴミ箱 | freedesktop Trash | Recycle Bin | `~/.Trash` |
| 開く/リビール | `xdg-open`/FileManager1 | ShellExecute/explorer | `open`/`open -R` |
| ファイル clipboard | `text/uri-list`（Wayland ✅ / X11 ⚠） | `CF_HDROP`（arboard ✅） | `.fileURL`（arboard ✅） |
| DnD IN | GPUI `FileDropEvent`（Wayland 実装確認 ✅） | GPUI（⚠ 実装状況要確認） | GPUI（⚠） |
| DnD OUT | GPUI `external_drag_payload`（Wayland ✅ / X11 ⚠） | OS 実装は ⚠ | OS 実装は ⚠ |

### 6.7 検索（Phase 2）

- 再帰のファイル名検索は `ignore` の `WalkBuilder::build_parallel()`。UI はチャネルで逐次結果を追加表示（全件完了を待たない）。キャンセル可能に。
- 内容検索（grep 相当）は `grep-searcher` 等。MVP 外。
- インデックス検索（Windows Search / Spotlight / Tracker）は OS API 依存で、Phase 3。

---

## 7. プラットフォーム別ノート

### 7.1 Linux（本命。Kubuntu 2026.04 / KDE Plasma 6 / Wayland）

- **GPUI バックエンドの実体（✅ 取得ソース確認）**: `gpui-pre-linux 0.3.12` に `linux/wayland/` と `linux/x11/` の両実装があり、feature は既定で `["wayland","x11"]`。レンダラは `gpui-pre-wgpu`（Vulkan/GL）で、`wayland`/`x11` feature がレンダラとテキストシステムをコンパイルする。
- **依存**: `ashpd`（XDG ポータル = ファイルダイアログ/Trash）、`accesskit_unix`（AT-SPI）、`notify-rust`（通知）、`wayland-client`/`wayland-protocols`、`x11rb`、`xkbcommon`、`calloop-wayland-source`。
- **実装確認済みの機能**: Wayland の **IME**（`text-input` + `handle_ime`）、X11 の **XIM**（`xim_handler.rs`）、Wayland の**アプリ外ファイルドラッグ**（`start_external_drag` + `text/uri-list`）、**ファイルドロップ受信**（`ExternalPaths`）、**ファイル clipboard**（`text/uri-list` + 独自 MIME）。✅
- **ファイルダイアログ**: `app.rs` の `prompt_for_paths` は Linux プラットフォーム実装経由（`linux/platform.rs` の実装あり ✅）。XDG ポータルが無い/拒否されるとエラーになる（doc 明記）。
- **描画**: Vulkan 必須。NVIDIA proprietary ドライバ（NVIDIA GPU）で動作見込み（⚠ PoC で最終確認）。ソフトウェアレンダリングのフォールバックは無い想定。
- **KDE 固有**: CSD/SSD は `WindowOptions::window_decorations` で切替（`TitlebarOptions::appears_transparent` は macOS/Windows のみ）。グローバルメニューバー（`set_menus`）はプラットフォーム実装があるが、KDE Wayland での挙動は ⚠ 未検証。分数スケーリング（125%/150%）の見え方は ⚠ 要確認（X11 は `GPUI_X11_SCALE_FACTOR`、Wayland は compositor 依存）。
- **inotify 上限**: 実測 518,185 watches / 1,024 instances / 16,384 queued events。監視は非再帰なので通常は余裕だが、他アプリと共有するので過信しない。

### 7.2 Windows

- **ビルド**: MSVC 必須（GNU ツールチェーン不可）。Visual Studio 2022 Build Tools の「Desktop development with C++」+ Windows SDK + CMake。`rustup override set stable-x86_64-pc-windows-msvc`（このリポジトリの Argus と同様）。
- **描画**: Win32 + DirectWrite、DirectX レンダラ。`windows-manifest` feature が manifest を埋め込む（✅ `gpui-pre` の Cargo.toml・`resources/windows/`）。
- **プラットフォーム実装**: `gpui-pre-windows 0.3.9` のソース一覧に `clipboard.rs`・`dialog.rs`・`destination_list.rs`（IDropTarget）・`direct_manipulation.rs`・`events.rs`・`window.rs` がある（✅ 先行監査のファイル一覧）。ファイルダイアログ・クリップボード・ドロップ受信は実装済み。**アプリ外へのドラッグ（drag source）** は ⚠（`destination_list` は drop 側）。
- **FS 注意**: 長パス（`\\?\` + manifest）、大小文字非区別、予約名、Recycle Bin、`ReadDirectoryChangesW` のバッファ溢れ。
- **UI**: タイトルバーの透過/独自タイトルバーは Windows で `TitlebarOptions::appears_transparent` が効く。ただし KDE と見た目を揃えるなら自前タイトルバー + SSD の方が無難（⚠ 判断はデザイン次第）。

### 7.3 macOS

- **ビルド**: macOS 15+、Xcode / Command Line Tools、Metal 固定。`font-kit` 無効だとグリフが出ない（gpui-kit は既定有効）。✅（ドキュメント）
- **FS 注意**: NFD 正規化、大小文字非区別（既定）、`~/.Trash`、FSEvents、`NSWorkspace.iconForFile`、`localizedStandardCompare`。
- **サンドボックス**: **App Sandbox を有効にするとフルディスクアクセスが制限**され、`~/Library` や他アプリのコンテナに触れない。ファイルエクスプローラーとしては「ユーザー選択ファイルの security-scoped bookmarks」を持つか、**App Store 外配布（notarization のみ）でサンドボックスを無効**にするかの選択が必要（⚠ 設計判断。配布方法と直結）。
- **実機検証**: 手元に macOS が無いため未確認。CI の macOS ランナーでコンパイル確認するのが現実的。

---

## 8. パフォーマンス設計と計測

### 8.1 目標値（たたき台）

| 項目 | 目標 | 根拠/備考 |
|---|---|---|
| ディレクトリ表示（10,000 件） | 初回 100 ms 以内に「名前だけ」表示、詳細列は 1 秒以内に逐次 | 2 パス方式 |
| スクロール | 120 fps を維持（60 fps は最低ライン） | gpui-kit は 120 fps を主張 |
| キーボード移動の応答 | 1 フレーム以内（8 ms） | 仮想化 + `scroll_to_item` |
| メモリ（100,000 件） | 200 MB 以下 | エントリの構造体を小さく（`PathBuf` + u64） |
| 監視イベントの反映 | デバウンス後 1 フレーム | バッチ + 差分 splice |

### 8.2 ボトルネックと対策

1. **`metadata()` の全件呼び出し** → 必要列のみ、並列、可視範囲優先。
2. **日本語テキストのシェイピング**（`rustybuzz`/`ttf-parser`）→ 行のテキストは `SharedString` でキャッシュ。每フレーム作り直さない。フォントは Noto Sans CJK JP 等の実在名を指定（⚠ この環境のフォントは `work/zed/env-audit.md` に一覧あり）。
3. **画像デコード** → background + サイズ上限 + メモリキャッシュ（LRU）。`img()` は非同期ロード + キャッシュを持つが、数百枚のグリッドでは `uniform_list` の可視範囲だけ生成する。
4. **アイコン取得（OS API）** → 初回だけ取得し `(key, size, scale)` でキャッシュ。プレースホルダ SVG を先に描く。
5. **ソート** → `sort_by_cached_key`、必要なら `sort_unstable_by`。比較関数内でアロケーションしない。
6. **監視イベントの嵐** → debounce + 抑制窓 + バッチ反映 + リコンサイル。
7. **過剰な `cx.notify()`** → 状態が変わったときだけ。アニメーション以外で毎フレーム `notify` しない。

### 8.3 計測

- **GPUI 内蔵**: `gpui`/`gpui-kit` の `inspector` feature（要素ツリー・レイアウト・描画の確認）、`profiler` feature（フレームイベント）。✅（ecosystem ノート §2/§3）
- **開発プロファイル**: `[profile.dev.package]` で `gpui-pre` / `gpui-component` / `gpui-kit` / `rustybuzz` / `taffy` / `ttf-parser` を `opt-level = 3` にすると体感が大きく改善する（gpui-kit docs 推奨 ✅）。
- **実測計画**: 100k ファイルのディレクトリを生成し、初回表示・スクロール・監視反映・ソートの時間を計測。PoC の計測結果を §付録 A に追記する（⚠ 今回は未計測）。

---

## 9. テストと GUI 検証

### 9.1 自動テスト

- GPUI のテスト基盤: `#[gpui::test]` + `TestAppContext` / `VisualTestContext`。ヘッドレスウィンドウでレイアウト・入力合成・状態を検証できる（✅ api-notes §9）。
- `gpui-kit` は `test-support` feature で `#[gpui_kit::test]`、`TestAppContext`、`VisualTestContext`、`ElementSnapshot`、`TestWindowExt`/`TestAppContextExt` を提供。**実コンポーネントをヘッドレスでレンダリングし、クリック/キーを合成して状態・フォーカス・レイアウト・アクセシビリティを検証**できる（✅ README/TESTING.md）。
- モデル層（`DirListing` のソート/差分更新、パス正規化、自然順）は **UI 非依存の純関数**として切り出し、通常の `cargo test` で厚くテストする。GPUI テストは「1 画面 1 本」で十分。
- ヘッドレスの制約: `headless()` のウィンドウは「レイアウトと入力は処理するが描画しない」（✅ `gpui-pre-linux` の `HeadlessWindow` doc）。GPU 描画の確認は実ウィンドウで行う。

### 9.2 実機 GUI 検証（KDE Wayland）

先行の Zed 調査（`work/zed/env-audit.md`）で確立した手順を流用する:

1. アプリを起動（小さめのウィンドウ、フォーカスを奪いすぎない）。
2. ウィンドウの存在確認: KRunner の windows runner で UUID を取得 → `qdbus6 org.kde.KWin /KWin getWindowInfo "<uuid>"` で geometry/caption を確認。
3. スクリーンショット: XDG Desktop Portal の Screenshot（保存先は `~/picture/` 固定）で取得。**デスクトップ全体を撮る場合は個人情報に注意**（Argus の AGENTS.md は「全体キャプチャ禁止」を明記している。同様の配慮をする）。
4. 終了後、プロセスが残っていないこと・ログに panic が無いことを確認。

- 自動 E2E は「起動 → 少し待つ → ウィンドウ存在確認 → スクショ → kill」のシェルスクリプトで回すのが現実的。GUI テストフレームワーク（AccessKit 経由の操作）は Phase 2 以降。
- アクセシビリティ: GPUI は AccessKit 統合済み。`div` の a11y フック（`Element::write_a11y_info`）と gpui-kit のテストで role/name を検証できる（⚠ 実機 AT-SPI での読み上げ確認は未実施）。

### 9.3 i18n・設定

- i18n は core に無い。`rust-i18n`/`fluent` 等を任意で。**日本語 UI の文言は最初からリソース化**しておくと後が楽。
- 設定は JSON/TOML + `serde`。`directories` クレートで OS ごとの設定ディレクトリに保存。テーマ・列幅・ソート・隠しファイル表示・最後のパスを永続化。

---

## 10. ビルド・配布

### 10.1 Linux（Kubuntu 2026.04）

```bash
# gpui-kit 公式インストール手順（Ubuntu 24.04 検証済み）より
sudo apt update
sudo apt install -y gcc g++ clang libfontconfig-dev libwayland-dev \
  libwebkit2gtk-4.1-dev libxkbcommon-x11-dev libx11-xcb-dev \
  libssl-dev libzstd-dev libasound2-dev vulkan-validationlayers libvulkan1
# 必要に応じて pkg-config
```

- **Vulkan ドライバが必須**（`libvulkan1` パッケージだけでは不足）。NVIDIA は proprietary ドライバで可。
- **この一覧の `libxkbcommon-x11-dev` は必須**。本機（PoC 実行時）はこれが無く、`gpui-kit` 経由のビルドがリンクで失敗した（`unable to find library -lxkbcommon-x11`）。入っていない場合は apt で導入するか、`poc/` と同じローカル symlink 回避（付録 A.4）を使う。
- Rust は gpui-kit の依存（`oo7`）が 1.92+ を要求。手元は 1.99.1 で OK ✅。
- feature: Wayland/X11 は既定で両方。`font-kit` は既定。**配布ビルドは `--release`（LTO 推奨）**。
- ビルド時間（実測、付録 A）: 依存を含む初回 debug が **1 分 31 秒**（525 crate）、release が **3 分 05 秒**。debug 実行ファイルは 745 MiB、release は 51 MiB（strip 後 37 MiB）。`sccache` と `[profile.dev] debug = 0` の活用を推奨。

### 10.2 Windows / macOS

| 項目 | Windows | macOS |
|---|---|---|
| ツールチェーン | MSVC（Build Tools 2022 + Windows SDK + CMake） | Xcode / CLT、Metal |
| レンダラ | DirectX | Metal（`font-kit` 必須） |
| 配布形式 | MSI/NSIS + Authenticode 署名 | `.app` + codesign + notarize + DMG |
| 注意 | 長パス manifest、アイコン `.rc`、署名 | NFD、サンドボックス、公証 |

- **配布ツール（✅ ecosystem ノート §4、2026-11-11 時点）**: gpui-kit 公式の packaging ドキュメントは**特定ツールに依存せず手順ベース**（macOS: `.app` + 署名 + `notarytool`、Windows: Inno Setup、Linux: tar.gz + `dpkg-deb --root-owner-group` で DEB）。一般ツールでは `cargo-bundle 0.12.0`（2026-09-20、活発）が候補、`cargo-packager 0.11.8`（2025-11-27）は約 1 年更新停止で注意。Linux の AppImage（linuxdeploy）や Flatpak は Zed の Flatpak 定義（`script/flatpak/*`）が参考になる。
- **CI**: gpui-kit 自身が ubuntu / macOS / windows の 3 OS でテストを回している（✅ ecosystem ノート §4.3）。同じ構成で「`cargo check --all-targets` + 単体テスト + Linux ヘッドレス UI テスト」を PR ごとに、実機 GPU スモークは nightly で。
- **アセット**: `gpui-kit-assets` の Lucide アイコン（ISC）の帰属表示を LICENSE/NOTICE に含める。

---

## 11. ライセンスと供給網

### 11.1 ライセンス整理（✅ 一次情報で確認）

| 対象 | ライセンス | アプリへの影響 |
|---|---|---|
| GPUI 本体 / `gpui-pre` / `gpui-pre-*` | **Apache-2.0** | 自由に利用可（特許許諾・帰属表示） |
| `gpui-kit` / `gpui-base` / `gpui-component` / `gpui-kit-assets` / `gpui-fast` | **Apache-2.0**（crates.io メタデータ） | 自由に利用可。ドキュメント散文は CC BY 4.0 |
| Lucide アイコン | ISC | 帰属表示を NOTICE に含める |
| **Zed アプリ本体の `crates/ui` / `crates/project_panel`** | **GPL-3.0-or-later**（✅ raw GitHub の Cargo.toml で確認） | MIT/Apache のアプリにはコピー不可。**AGPL アプリなら GPLv3 §13 の互換規定で結合可能**だが、GPL 部分の扱いが複雑になるため非推奨（§11.1.1） |
| `security-review/Argus`（第三者の GPUI Kit アプリ） | **AGPL-3.0-or-later**（✅ `LICENSE` / `Cargo.toml` で確認） | **自作アプリを AGPL-3.0-or-later にする場合のみ流用可能**。著作権表示・変更明示・ソース提供が条件（§11.1.1） |

> 重要: 参考実装として最も近い Zed の `project_panel` は **GPL-3.0-or-later**。MIT/Apache で配布したいなら写経不可。本プロジェクトは **AGPL-3.0-or-later** とする方針（2026-11-11 依頼者判断）なので Argus の流用は可能だが、Zed の GPL コードを混ぜる場合は §11.1.1 の注意に従うこと。

### 11.1.1 方針: 自アプリを AGPL-3.0-or-later にする場合（2026-11-11 依頼者判断）

依頼者の判断により、自作アプリのライセンスは **AGPL-3.0-or-later** とする。これにより `security-review/Argus`（AGPL-3.0-or-later）のコードを流用できる。

**AGPLv3 の義務（要点）**

1. **配布時のソース提供**: バイナリを配布したら、対応する完全なソース（自分のコード + 改変 + ビルド/インストール用スクリプト）を AGPL で提供する。ダウンロード可能な場所の案内でよい。ソースに価格を付けること自体は可能。
2. **ネットワーク越しの利用（§13）**: 改変版をネットワーク経由で他者に使わせた場合、その利用者に対応ソースを提供する義務が生じる。**ローカルデスクトップアプリを自分だけで使う分には「配布」ではないので公開義務はない**。
3. **コピーレフト**: 派生物は全体として AGPL。AGPL コードを混ぜた部分は、後からクローズドソース化できない。
4. **表示の保持**: 元の著作権表示・ライセンス文・変更点の明示。Argus を流用するなら `security-review/Argus/LICENSE` と `THIRD-PARTY-NOTICES.txt`（422 KB）を保持/再生成する。
5. **「フォーク先も必ず公開」ではない**: 内部フォーク・私的改変は、配布しない限り公開不要。

**Argus / Zed コードを混ぜるときの注意**

- Argus のコードをコピーしたファイルは AGPL のまま。MIT/Apache への再許諾はできない。
- Argus が vendor している `gpui-pre` パッチは Apache-2.0 の gpui への改変。Apache-2.0 部分の表示は保持しつつ、Argus 改変部分は AGPL として扱う。`security-review/Argus/vendor/gpui-pre/ARGUS-PATCHES.md` と `vendor/gpui-pre-windows/ARGUS-PATCHES.md` を参照。
- **商標・ロゴはライセンス外**。Argus の名称・ロゴを自分のアプリに使わない。
- **Zed の `crates/ui` / `project_panel`（GPL-3.0-or-later）を混ぜる場合**: GPLv3 §13 により AGPLv3 との結合は可能だが、GPL 部分の表示・ソース提供の扱いが増える。素直に「gpui/gpui-kit（Apache-2.0）+ Argus（AGPL）+ 自作」に留めるのが安全。
- **配布チャネルとの相性**: Linux（Flathub/DEB）・Windows は問題なし。**macOS App Store は DRM/規約と AGPL が衝突**するため、macOS は notarize + 自前配布（DMG/Homebrew）にする。
- **依存の互換性**: gpui/gpui-kit（Apache-2.0）は AGPLv3 と両立（GPLv3 互換）。Lucide は ISC、ドキュメントは CC BY 4.0（帰属表示）。GPL 非互換ライセンスの crate を混ぜないこと（`cargo-deny` で検出）。
- **将来の選択肢**: 一度 AGPL コードを混ぜるとクローズドソース化・買収時のデューデリジェンスで障壁になりうる。個人利用/公開前提なら問題は小さい。

### 11.2 供給網（サプライチェーン）の注意

- `gpui-pre` / `gpui-kit` は **huacnlee（Longbridge）個人アカウント**が公開。Zed 公式の crates.io 公開ではない（✅ owners API）。コードは Zed の Apache-2.0 スナップショットだが、**公開主体・ビルド環境はサードパーティ**。加えて **対象 crate の crates.io owner は huacnlee 1 名のみ（bus factor = 1）** であり、長期の可用性・乗っ取りリスクをゼロとは言えない（✅ ecosystem ノート §5.1）。
- 来歴の検証は可能: `gpui-pre` の公開マニフェストに `[package.metadata.gpui-pre] zed-rev = "..."` が埋め込まれており、Zed の該当 commit（0.3.12 は `d2a4fb6`、公開の約 4.4 時間前の commit）と照合できる（✅ ecosystem ノート §5.2）。
- 対策:
  1. `gpui-kit = "=0.9.1"` のように**完全一致ピン** + `Cargo.lock` をコミット。
  2. 更新はリリースノートと差分を確認して**隔週でまとめて**。週次で追わない。
  3. `cargo-deny`（ライセンス/重複/脆弱性/依存元）、`cargo-vet`（監査記録）、`cargo-audit`（RUSTSEC）を CI に。
  4. 重要なら `cargo vendor` で固定し、`gpui-pre` のスナップショット元 rev（`zed@d2a4fb6` 等）との差分を監査。先行監査の資産（`.tmp-deps-audit/gpui-pre.diff` / `changed.txt`）がその雛形になる。
- **先行監査の結果（✅ `.tmp-deps-audit/audit.out`, 2026-11-11 02:22）**: 850 crate をスキャンし、**脆弱性 0 件**。ただし「未整備（unmaintained）」警告 8 件（`instant`/`paste`/`rustls-pemfile`/`rustybuzz`/`ttf-parser`/`zbus_names`/`zvariant`/`zvariant_derive`）と、**ネットワーク失敗 367 件**で完全ではない。リリース前に再実行すること。
- `gpui-fast` は upstream のフォークで、レイアウト/描画が変わる・独自 platform crate を増やす・retained mode の注意が必要。**MVP では無効**（✅ ecosystem ノート §5.4）。

---

## 12. MVP ロードマップ

前提: 1 人開発、Linux を第一に、Windows/macOS は CI ビルド + 後日実機。期間は目安。

### Phase 0: PoC（1〜3 日）— **本レポートで実施済み（付録 A）**
- ✅ `gpui-kit` でウィンドウ + 仮想リスト + ディレクトリ一覧 + キーボード移動 + 非同期読込（ビルド 1m31s、Wayland 起動・描画成功）。
- ⬜ 100k 件ディレクトリの表示/スクロール計測。
- ✅ Wayland 実機で起動確認（KWin D-Bus + XDG portal スクショ付き）。
- **完了条件**: ビルドが通り、一覧がスクロールでき、フレーム落ちが体感で無い（キー入力 E2E と 100k 計測が残課題）。

### Phase 1: MVP（2〜4 週間）
- 一覧（詳細 + アイコン）、自然順ソート、名前フィルタ、隠しファイルトグル。
- ナビゲーション: ブレッドクラム、戻る/進む/上へ、パス入力、ダブルクリックで開く。
- サイドバー（場所/お気に入り）、ステータスバー。
- 監視（notify）による自動更新 + 手動更新。
- ファイル操作: 削除（ゴミ箱）、リネーム、新規フォルダ、コピー/移動（同一 FS の rename 優先）、パスのコピー。
- コンテキストメニュー（gpui-kit PopupMenu）、開く/リビール。
- 設定保存（最後のパス、ビュー、表示設定）。
- テスト: モデル層の単体テスト + ヘッドレス UI テスト数本 + 実機スモーク。
- **完了条件**: 日常的にフォルダを巡回してファイルを開く/整理できる。落ちない。

### Phase 2: 実用化（3〜6 週間）
- OS ネイティブアイコン + 画像サムネイル、ボリューム一覧、ゴミ箱ビュー（復元/完全削除）。
- 再帰検索（ignore、逐次表示、キャンセル）。
- 複数選択 + キーボード操作の完成、列ソート/列幅、タブ。
- ファイル clipboard（arboard file_list）と D&D（OS 連携）の実装・検証。
- Windows/macOS の実機ビルド・検証、インストーラ/署名。
- **完了条件**: 他 OS でも主要操作が動く。監視の取りこぼしがリコンサイルで回復する。

### Phase 3: 差別化（将来）
- 検索インデックス、プレビュー（PDF/動画/テキスト）、アーカイブ閲覧、タグ/検索履歴、クラウド/ネットワーク FS 対応、Windows MFT/USN、Spotlight。
- ドラッグアウトの X11/Windows/macOS 対応の完成。

---

## 13. 落とし穴チェックリスト（設計レビュー用・Top 20）

1. **UI スレッドでファイル I/O / 画像デコードをしていないか**（1 回で 60 fps が死ぬ）。
2. **`PathBuf`/`OsString` を捨てて `String` でパスを保持していないか**（Linux 非 UTF-8 で壊れる）。
3. **macOS の NFD を考慮しているか**（検索・重複判定）。
4. **Windows の大小文字・長パス・予約名**を考慮しているか。
5. **監視の再帰 watch をしていないか**（inotify watch 枯渇・ネットワーク FS で破綻）。
6. **監視イベントの溢れ（Rescan/overflow）で再読込するか**、定期リコンサイルがあるか。
7. **自操作の抑制窓**（rename/copy/delete のイベントループ）があるか。
8. **ディレクトリ切替の競合**（古い read_dir 結果が後着で上書きしないか = 世代番号）。
9. **`read_dir` 中の `NotFound`** で一覧全体をエラーにしていないか。
10. **選択・スクロール位置の復元**（更新・ソート・削除後）。
11. **ダブルクリックが `click_count` で判定されているか**（単クリックとの競合）。
12. **フォーカスを失ったときショートカットが死ぬ**問題（`track_focus` / `key_context`）。
13. **IME を自前実装していないか**（日本語入力が壊れる。TextInput/Input を使う）。
14. **コンテキストメニューの外側クリックで閉じる**処理（`on_mouse_down_out`）。
15. **D&D の `Submit` にパスが無い**問題（`Entered` で保持）。
16. **OS クリップボードの cut/copy 区別**が取れない問題（アプリ内状態で管理）。
17. **ゴミ箱に入らない FS** でのフォールバック（完全削除の確認）。
18. **シンボリックリンクを辿って削除**していないか（`symlink_metadata`）。
19. **大量コピーで UI が固まらない**か（進捗/キャンセル/エラー集約）。
20. **バージョン固定**（`=x.y.z` + `Cargo.lock`）と**週次破壊的変更**への運用。

---

## 14. 参考資料

### ローカル成果物（このワークスペース）
- 本レポートの詳細根拠: [`notes/api-notes.md`](notes/api-notes.md) / [`notes/ecosystem-notes.md`](notes/ecosystem-notes.md) / [`notes/fs-cross-platform-notes.md`](notes/fs-cross-platform-notes.md)
- PoC: [`poc/`](poc/) / [`notes/poc-report.md`](notes/poc-report.md)
- 先行監査: `.tmp-deps-audit/`（`audit.out`, `gpui-pre.diff`, `changed.txt`, `changed-win.txt`, index/metadata スナップショット）
- 先行の Zed 調査: `work/zed/research.md`, `work/zed/env-audit.md`, `work/zed/docs/linux.md`
- 参考アプリ（ライセンス注意・読むだけ）: `security-review/Argus/`（AGPL-3.0-or-later の GPUI Kit アプリ）

### 一次情報（URL は 2026-11-11 取得）
- GPUI (Zed): <https://github.com/zed-industries/zed>, <https://gpui.rs>
- gpui-pre 0.3.12: <https://crates.io/crates/gpui-pre>, <https://docs.rs/gpui-pre/0.3.12>
- gpui-pre-platform 0.3.12: <https://crates.io/crates/gpui-pre-platform>
- gpui-kit 0.9.1: <https://crates.io/crates/gpui-kit>, <https://docs.rs/gpui-kit/0.9.1>
- gpui-component 0.9.1: <https://crates.io/crates/gpui-component>
- GPUI Kit docs: <https://gpui-kit.com>（`llms.txt` 起点）
- Zed Linux docs: <https://zed.dev/docs/linux>
- ICON/THEME/TRASH 仕様: <https://specifications.freedesktop.org/icon-theme-spec/icon-theme-spec-latest.html>, <https://specifications.freedesktop.org/trash/1.0>
- Windows ファイル API: <https://learn.microsoft.com/en-us/windows/win32/fileio/maximum-file-path-limitation>, <https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-readdirectorychangesw>
- notify crate: <https://github.com/notify-rs/notify>
- arboard: <https://github.com/1Password/arboard>
- trash: <https://github.com/Byron/trash-rs>
- opener: <https://github.com/Seeker14491/opener>

> 注意: 本レポートの「✅」は 2026-11-11 時点の一次情報に基づく。GPUI 系は週次で変わるため、実装開始時に `Cargo.lock` と crate ソースで再確認すること。

---

## 付録 A: 実機 PoC（Kubuntu 2026.04 / KDE Plasma 6 / Wayland / NVIDIA）の検証結果

**結論: `gpui-kit = "=0.9.1"` は本機でビルド・Wayland 起動・描画まで成功した。** PoC は「ディレクトリの非同期ロード + `uniform_list` による仮想リスト + キーボード移動 + ステータス行」を実装（`poc/src/main.rs` 約 477 行、テスト 4 件）。詳細: [`notes/poc-report.md`](notes/poc-report.md)。

### A.1 実測バージョン

| 項目 | 値 |
|---|---|
| toolchain | rustc 1.99.1 / cargo 1.99.1（edition 2021） |
| `gpui-kit` | 0.9.1 |
| `gpui-pre`（lib 名 `gpui`） | 0.3.12（zed@d2a4fb6） |
| `gpui-pre-platform` | 0.3.12（gpui-kit が `font-kit`/`x11`/`wayland`/`runtime_shaders` を固定有効） |
| `gpui-base` / `gpui-component` / `gpui-kit-assets` | 0.9.1 |
| 追加依存 | なし（PoC は `gpui-kit` のみ） |

### A.2 ビルド（実測）

| ビルド | 時間 | サイズ | 備考 |
|---|---|---|---|
| debug 初回（依存 525 crate） | **1 分 31 秒** | 実行ファイル 745 MiB | 警告 0。`target/` は debug+release で約 12 GB |
| debug 差分 | 2.94 秒 | — | |
| release | **3 分 05 秒** | 51 MiB（`strip` 後 37 MiB） | |
| `cargo test` | 4 件 pass | — | `read_entries` 等データ層 |

- `cargo fetch` は 18 秒。初回ビルドは 525 crate と重いが、2 回目以降は差分ビルドで実用的（**リリースビルド 3 分**は嬉しい誤算。GPUI のビルドは遅いという先入観を更新してよい）。
- debug バイナリ 745 MiB は debuginfo 由来。`[profile.dev] debug = 0` 等で削減可能（⚠ 未検証）。`target/` 12 GB は要注意。

### A.3 Wayland 実行（実測）

- **stderr 0 行**（EGL/Vulkan/Wayland のエラーなし）。ネイティブ Wayland ウィンドウとして起動。
- KWin D-Bus 検証: `caption="GPUI File Explorer PoC"`, `resourceClass/resourceName="gpui-file-explorer-poc"`, サイズ 760x548, `minimized=false`, `fullscreen=false`。
- XDG Desktop Portal のスクリーンショットで、`$HOME` の **<entry-count> entries** が非同期ロード後に描画され、選択ハイライトとステータス行が表示されることを確認。証拠画像は実パス・識別子を含むため削除済み。
- `SIGTERM` でクリーン終了し、ウィンドウが消滅することを確認。release バイナリでも同様に起動確認。

### A.4 実測で踏んだ罠と回避策

1. **リンクエラー `unable to find library -lxkbcommon-x11`**: gpui-kit が X11 feature を固定有効にするため `xkbcommon` crate が `-lxkbcommon-x11` を要求。本機は runtime の `libxkbcommon-x11.so.0` のみで **`-dev` パッケージが無い**。回避: `poc/native-libs/libxkbcommon-x11.so` → システム `.so.0` の symlink + リンカ検索パス追加（sudo 不要）。※当初は `.cargo/config.toml` の絶対パス指定だったが、CWD 依存で壊れるため現在は各 crate の `build.rs` + `scripts/dev-setup.sh` 方式に移行済み。恒久対策は `sudo apt install libxkbcommon-x11-dev`、または Wayland のみの feature 構成（⚠ 未検証）。
2. **`use gpui_kit::*` が組み込み `#[test]` マクロを shadow**（"recursion limit reached"）→ テストでは明示 import、または `test-support` + `#[gpui_kit::test]` を使う。
3. **0.3.9 スナップショットと 0.3.12 の API 差異**: `uniform_list` の実シグネチャ、`UniformListScrollHandle::{new, track_scroll, scroll_to_item}` の必要性、`WindowOptions.app_id` が Wayland の app_id に反映される点、`cx.spawn(async move |this, cx| ...)` + `this.update` / `cx.listener` 等は、フェッチ済み 0.3.12 の実ソースで確認して合わせた。
4. `[patch]` / `vendor` は不要だった（gpui-kit の facade で完結）。

### A.5 未検証（次にやるべきこと）

- **キー入力の E2E**: Wayland ネイティブ向け入力注入ツール（kdotool 等）が未導入のため未実施。ハンドラはコンパイル済み・データ層は unit test 済み。
- X11 バックエンドでの実行、CJK ファイル名の描画、10 万件規模の性能計測、headless `test-support` の有効化、Windows/macOS 実機。
- 詳細と再現手順: [`notes/poc-report.md`](notes/poc-report.md) / [`poc/README.md`](poc/README.md)。

---

## 付録 B: 検証に使った主なコマンド

```bash
# 環境
rustc --version && cargo --version
pkg-config --modversion wayland-client xkbcommon fontconfig freetype2
echo "$XDG_SESSION_TYPE / $WAYLAND_DISPLAY / $XDG_CURRENT_DESKTOP"

# crate の版・ライセンス・所有者（crates.io API は UA 必須）
curl -sS -A 'research-agent/1.0' https://crates.io/api/v1/crates/gpui-kit | \
  python3 -c 'import json,sys; d=json.load(sys.stdin); print(d["crate"]["max_version"], d["crate"]["updated_at"])'
curl -sS -A 'research-agent/1.0' https://crates.io/api/v1/crates/gpui-pre/owners

# crate ソースの取得（API と同じ）
curl -sSL -A 'research-agent/1.0' https://crates.io/api/v1/crates/gpui-pre-platform/0.3.12/download -o gpp.tar.gz

# デスクトップ/GPU の確認
glxinfo -B 2>/dev/null | head -20   # Xwayland
nvidia-smi 2>/dev/null | head -15

# inotify 上限
cat /proc/sys/fs/inotify/max_user_watches /proc/sys/fs/inotify/max_user_instances /proc/sys/fs/inotify/max_queued_events

# GUI 検証（先行の Zed 調査から）
qdbus6 org.kde.KWin /KWin getWindowInfo "<window-uuid>"   # UUID は KRunner windows runner 等で取得
```


