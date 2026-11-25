# GPUI エコシステム / スタック / ビルド・配布 調査ノート（2026-11-11）

対象: Rust + GPUI でクロスプラットフォーム（Linux / Windows / macOS）のファイルエクスプローラを新規に作る場合の、
依存スタック選定・コンポーネント充足・ビルド要件・供給網リスク・配布方法。

- 調査日: **2026-11-11**（Asia/Tokyo）。数値・日付はすべてこの日に取得。
- 取得方法: crates.io API（`curl -A 'research-agent/1.0'`）、crates.io が公開する README / crate tarball、docs.rs、
  gpui-kit.com の `llms.txt` と各 `.md` ドキュメント、GitHub API / raw、web 検索。
- 表記: **【検証済】** = 一次情報を実際に取得して確認。**【推測】** = 取得情報からの推定。**【未確認】** = 未検証。
- 注: 調査中に自動レビュー基盤が断続的に 429 を返し、一部 API 取得が失敗した時間帯がある。失敗分は「未確認」に明記し、
  Lead が別途検証済みの事実（`gpui-pre-linux` の依存構成など）はその旨を付記した。

---

## 0. 結論（推奨スタック）

### 0.1 第一候補: `gpui-kit = "=0.9.1"`（既定 features、`gpui-fast` は使わない）

```toml
[dependencies]
gpui-kit = "=0.9.1"
# 後述のとおり初手は gpui-fast を有効化しない（必要になったら feature で後付け可能）
```

```rust
use gpui_kit::component::button::*;
use gpui_kit::*;

struct App;
impl Render for App {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().child(Button::new("ok").primary().label("Hello"))
    }
}

fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets) // Lucide 既定アイコン
        .run(|cx| {
            gpui_kit::init(cx); // コンポーネント初期化（base 初期化を含む）
            gpui_kit::open_window(WindowOptions::default(), cx, |_, cx| cx.new(|_| App))
                .expect("failed to open window");
        });
}
```

理由（詳細は後述）:

1. **単一依存で版が揃う**: `gpui-kit` が `gpui-pre = "=0.3.12"`（`package = "gpui-pre"`, lib 名 `gpui`）/
   `gpui-pre-platform = "=0.3.12"` / `gpui-pre-macros = "=0.3.12"` を厳密ピンで取り込み、`gpui-base` /
   `gpui-component` / `gpui-kit-assets` を再エクスポートする【検証済: gpui-kit 0.9.1 の正規化 Cargo.toml】。
   アプリ側で GPUI 本体を直接指定しないため、型不一致事故が起きにくい。
2. **ファイルエクスプローラに必要な部品がほぼ揃う**: 仮想スクロール付き DataTable（列リサイズ・ソート・固定列・
   セル/行選択）、VirtualList、List（D&D 並べ替え）、Tree（ファイルツリー例あり）、Tabs、PopupMenu（コンテキストメニュー）、
   Dialog、Input、Icon、Resizable、Dock、Tooltip、Notification、Theme、Sidebar、Toolbar、StatusBar、TitleBar。
3. **実運用の裏付け**: Longbridge Pro（商用デスクトップアプリ）で day-one から使用と README が明記。GitHub 20,270★、
   週次リリース（v0.9.1 = 2026-11-09）。CI が Linux x86_64 / macOS aarch64 / Windows x86_64 の 3 OS でテスト。
4. **ライセンスが緩い**: コードは Apache-2.0（gpui-kit 系すべて）。Zed アプリ本体の GPL-3.0 コードをコピーせずに済む。

### 0.2 フォールバック

| 優先 | スタック | 使いどころ |
|---|---|---|
| B1 | `gpui-pre` 0.3.12 + `gpui-pre-platform` 0.3.12 を直接（`gpui = { package = "gpui-pre", ... }`） | gpui-kit の見た目・抽象化が邪魔、UI を完全自前設計したい場合。機能は GPUI 素のまま。 |
| B2 | `zed-industries/zed` への **git 依存（rev 固定）** | 最新 upstream API に追従したい場合。実例あり（後述の hoja）。ただし Zed の GPL crate を引くとアプリも GPL 化。 |
| B3 | 公式 `gpui` 0.2.2（crates.io） | 学習・実験用のみ。2025-10-22 公開で約 1 年更新なし、`gpui_platform` 相当が無く単体アプリ化が面倒。**本番非推奨**。 |
| B4 | `gpui-fast` 0.1.14（`gpui-kit` の feature `gpui-fast`） | 性能問題が実測で出た後の最適化オプション。実験的でレイアウト/描画が upstream と異なる。初手では使わない。 |
| B5 | `gpui-ce`（GPUI Community Edition, 1,586★, Apache-2.0） | コミュニティフォーク。API 互換性は「mostly」で変化中。周辺実装（Atlas）は vendoring で利用。採用は将来判断。 |

### 0.3 ユーザー（依頼者）が受け入れる必要があること

- **pre-1.0 / 週次破壊的変更**: gpui-kit は 2026-09-03 の 0.1.0 から 0.9.1 まで約 2 か月で 15 リリース（週次）。
  `gpui-pre` も 13 リリース。API 破壊は日常的。対策: `=` 完全固定 + `Cargo.lock` コミット + 更新は隔週でまとめて。
- **第三者ミラーへの信頼**: `gpui-pre-*` は Zed 公式 crates.io ではなく huacnlee（Longbridge）個人アカウントが
  Zed の commit を再公開したもの。中身は Zed の Apache-2.0 コードのスナップショットだが、公開主体は Zed ではない。
- **コンポーネントの穴**: パンくず（Breadcrumb）専用部品なし、OS ネイティブのドラッグ&ドロップ（他アプリから
  ファイルを掴んで落とす）の専用部品なし、サムネイル生成/ファイルアイコンテーマ部品なし。自前実装が必要。
- **監査の限界**: 既存の cargo-audit 実行（.tmp-deps-audit）は「脆弱性 0 件」ではなく「未整備クレート 8 件の警告 +
  ネットワーク失敗 367 件」であり、再実行が必要（§5.6）。

---

## 1. 比較表: 単体アプリとしての GPUI 入手経路

crates.io API で 2026-11-11 に取得した値。ダウンロード数は累計。

| # | 経路 | パッケージ | 最新版 / 公開日 | 版数 | DL | owner | license | 位置づけ・所見 |
|---|---|---|---|---|---|---|---|---|
| a | 公式 GPUI | `gpui` | **0.2.2 / 2025-10-22** | 7（3 は yanked テスト版） | 471,005 | maxbrunsfeld, mikayla-maki, MrSubidubi, github:zed-industries | Apache-2.0 | Zed 本体の GPUI。crate の公開が **約 1 年前で停止**。Zed main の `crates/gpui/Cargo.toml` も今なお `version = "0.2.2"` で、crates.io の 0.2.2 以降のコードは未公開【検証済】。`gpui_platform` 相当の別 crate が無い。 |
| b | 再公開スナップショット | `gpui-pre` + `gpui-pre-platform`（+ `gpui-pre-linux/windows/macos/web/ios/wgpu`） | **0.3.12 / 2026-11-09 00:30 UTC** | 各 13（2026-09-03〜） | gpui-pre 467,360 / platform 540,240 / linux 360,861 / wgpu 365,972 | huacnlee | Apache-2.0 | `zed@d2a4fb6` の snapshot。lib 名は `gpui` / `gpui_platform`。**公開マニフェストに `[package.metadata.gpui-pre] zed-rev = "d2a4fb6..."` が埋め込まれており来歴を自己申告**【検証済】。Zed の commit（2026-11-08 20:08 UTC）から**約 4.4 時間後**に公開。 |
| c | オールインワン | `gpui-kit` | **0.9.1 / 2026-11-09 09:50 UTC** | 15（2026-09-03〜） | 324,072 | huacnlee | Apache-2.0 | `gpui-pre` =0.3.12 + `gpui-pre-platform` =0.3.12 + `gpui-pre-macros` + `gpui-base` + `gpui-component` + `gpui-kit-assets`（+ opt `gpui-fast`）。`gpui_kit::*` で GPUI API を再エクスポート。docs: https://gpui-kit.com |
| d | git 依存 | `zed-industries/zed` | main（2026-11-10 push） | - | - | Zed Industries | 混在（GPUI crate は Apache-2.0、アプリ crate は GPL-3.0） | **実例あり: slastra/hoja が `rev = c7bea343838f...` で `gpui` と `gpui_platform` を同時に git 依存**（features `wayland`,`x11`）。ワークスペース内 path 依存は Cargo が同一 git ソースから解決するため成立する。ただし同じ git ソースから全 gpui crate を揃える必要あり（混在で型が二重化）。 |
| e | コンポーネントのみ | `gpui-component` | **0.9.1 / 2026-11-09 09:48 UTC** | 38（2025-02-06〜） | 524,678 | huacnlee | Apache-2.0 | スタイル付き 75+ コンポーネント。`gpui`（=`gpui-pre`）=0.3.12 を厳密ピンするが `gpui-base` / `gpui-kit-assets` は caret（^0.9.1）。単体利用はピン管理を自前で行う必要あり。 |
| f | コミュニティフォーク | `gpui-ce`（GPUI Community Edition） | GitHub: gpui-ce/gpui-ce（1,586★、2026-11-11 push、Apache-2.0、2025-12 作成） | - | - | gpui-ce org | Apache-2.0 | Zed GPUI のフォーク。「upstream と mostly API 互換だが変化中」。crates.io 公開は未確認。Atlas は vendoring して利用。 |

補助 crate（同日取得）:

| crate | 最新版 / 公開日 | DL | license | 役割 |
|---|---|---|---|---|
| `gpui-base` | 0.9.1 / 2026-11-09 | 377,819 | Apache-2.0 | 非スタイルの挙動・状態・基盤（focus, a11y, theme tokens, virtual list, history, Table セマンティクス等） |
| `gpui-kit-assets` | 0.9.1 / 2026-11-09 | 391,426 | Apache-2.0（Lucide は ISC、Feather 由来は MIT） | Lucide 1.43.0 全 1,818 + 独自 12 = 1,830 SVG。既定コンポーネントアイコン 101 個は約 44 KiB、`AllAssets` は約 731 KiB |
| `gpui-pre-macros` | 0.3.12 / 2026-11-09 | 555,345 | Apache-2.0 | derive/attribute マクロ |
| `gpui-fast` | 0.1.14 / 2026-11-09 | 133,395 | Apache-2.0 | GPUI の性能最適化フォーク（retained mode / scroll layers）。README 自ら「experimental」 |
| `gpui-fast-platform` | 0.1.14 / 2026-11-09 | 117,758 | Apache-2.0 | fast 版プラットフォーム選択メタ crate |
| `gpui-pre-reqwest` | 0.12.15 / 2026-09-03 | 271,401 | MIT OR Apache-2.0 | **huacnlee が手動公開した reqwest の再公開**（repo は seanmonstar/reqwest）。`gpui-kit-assets` が使用。名称が別なので依存グラフ上は独立クレートとして扱われる点に注意。 |

### 1.1 各経路の install スニペット（一次情報）

- **gpui-kit**（crate README / https://gpui-kit.com/docs/installation）【検証済】:
  ```toml
  [dependencies]
  gpui-kit = "0.9.1"
  # 性能最適化を試す場合のみ:
  # gpui-kit = { version = "0.9.1", features = ["gpui-fast"] }
  ```
  feature: `default = ["component", "assets"]`。`component` = gpui-component、`assets` = 既定アイコン。
  その他 `test-support`, `inspector`, `profiler`, `tree-sitter`, `syntect`, `speech` など。
- **gpui-pre + gpui-pre-platform**（gpui-pre crate README / 0.3.12 tarball）【検証済】:
  ```toml
  [dependencies]
  gpui = { package = "gpui-pre", version = "=0.3.12" }
  gpui_platform = { package = "gpui-pre-platform", version = "=0.3.12",
                    features = ["font-kit", "wayland", "x11"] }
  ```
  ```rust
  fn main() {
      gpui_platform::application().run(|cx: &mut App| { /* ... */ });
  }
  ```
  **注意**: `gpui-pre-platform` の既定 feature は `["multithreaded"]` のみで、`wayland`/`x11`/`font-kit` は含まれない
  【検証済: 0.3.12 の features】。Linux で feature を付け忘れると「コンパイルは通るが起動時に `current_platform()`
  が `unreachable!()` で落ちる」ことが実際のアプリ（hoja）のマニフェストコメントで報告されている。
- **gpui-component 単体**: `gpui-component = "0.9.1"` に加え `gpui = { package = "gpui-pre", version = "=0.3.12" }` 等を
  自前で揃える必要がある（README は gpui-kit 経由を推奨）【検証済: gpui-component README】。
- **公式 gpui 0.2.2**（crates.io README）:
  ```toml
  gpui = { version = "*" }   # README 原文。実運用では "=0.2.2"
  ```
  `Application::new()` から開始。README には「macOS or Linux」とあるが、Cargo.toml に Windows 向け依存と
  `windows-manifest` feature があり、コード上は Windows 対応済み。README の記述が古い【検証済】。
- **Zed git 依存（hoja の実例）**【検証済: マニフェスト】:
  ```toml
  [dependencies]
  gpui = { git = "https://github.com/zed-industries/zed", rev = "c7bea343838f42b4c14151393e41f0bbe033c95e" }
  gpui_platform = { git = "https://github.com/zed-industries/zed",
                    rev = "c7bea343838f42b4c14151393e41f0bbe033c95e",
                    features = ["wayland", "x11"] }
  ```
  hoja のコメント（要旨）: 全 gpui crate を同じソースから取ること（一部だけ patch すると `gpui` が 2 コピーになり型が壊れる）。
  `gpui_platform` は crates.io に出ていないため必須。wayland feature は省略不可。`font-kit` は Linux では no-op。

### 1.2 なぜ「再公開」が必要なのか（構造的理由）【検証済 + 推測】

- Zed は GPUI を crates.io に継続公開しておらず、最後の公開が 2025-10-22。Zed 本体内では **2026-02 に
  `gpui_platform` と platform 別 crate（linux/macos/windows/web/ios）へ分割**されたが、crates.io には出ていない
  （hoja のマニフェストコメントと、gpui-pre の公開 crate 群から確認）。
- gpui-pre はこの差分を「Zed の commit をそのまま、パッケージ名とマニフェストだけ crates.io 向けに調整して再公開」する
  ことで埋めている。gpui-kit の docs は「`gpui-pre` は別実装ではなく Zed GPUI のスナップショット。公開処理は
  パッケージ名と依存マニフェストのみ調整する」と明記（https://gpui-kit.com/docs/installation）。
- 各 gpui-pre crate には `[package.metadata.gpui-pre] zed-crate` / `zed-version` / `zed-rev` が埋め込まれており、
  来歴を機械的に検証できる（例: `gpui-pre` → `zed-crate="gpui"`, `zed-version="0.2.2"`, `zed-rev="d2a4fb6..."`）【検証済】。
- つまり実質的な選択肢は「1 年前の公式 crate」か「数時間前の Zed を再公開したミラー」か「Zed への git 依存」の三択。

---

## 2. コンポーネント表（ファイルエクスプローラ観点）

出典は gpui-kit.com の各 `.md`（2026-11-11 取得）。URL は `https://gpui-kit.com/<path>`。
「成熟度」は docs の記述（"production-ready" / "high-performance" / 基本機能のみ）と README の主張に基づく評価で、
実機検証ではない。

| 必要機能 | gpui-kit の部品 / パス | 成熟度・要点 | ファイルエクスプローラでの用途 |
|---|---|---|---|
| **詳細ビュー（仮想 Table）** | **DataTable** `/component/data-table`（`gpui_kit::component::table::{DataTable, TableState, TableDelegate, Column, ColumnSort, ColumnFixed}`） | ◎ 本命。仮想スクロール・列リサイズ/移動/固定・ソート・行/列/セル選択・キーボード操作・カスタムセル描画・行/セル右クリックメニュー・無限ロード・イベント。`TableDelegate` 実装 + `TableState` | 名前/サイズ/更新日時/種類の列、ソート、数千〜数十万行 |
| 簡易 Table | Table `/component/table`（base にセマンティック primitive も） | ○ 仮想化なしの基本表。小さい一覧・設定画面向け | 小規模ディレクトリやプレビュー用 |
| 仮想リスト | VirtualList `/component/virtual-list`（base: `v_virtual_list`/`h_virtual_list`） | ◎ 可変サイズ行対応、水平/垂直、スクロール位置制御、性能最適化の記述 | 一覧表示（アイコン/リスト/コンパクト） |
| リスト | List `/component/list` | ○ セクション・検索・選択・無限スクロール・ローディング・**D&D 並べ替え** | サイドバー（場所/クイックアクセス）、最近使った項目 |
| **ツリー** | **Tree** `/component/tree`（base primitives: Tree は仮想化階層リスト） | ◎ 階層表示、**ファイルツリー + アイコンの例**、動的ロード、選択、無効項目、`TreeState`/`TreeItem` | 左ペインのディレクトリツリー |
| **タブ** | Tabs `/component/tabs` | ◎ variant/サイズ/アイコン/prefix・suffix/無効/動的追加/メニュー/最大幅 | 複数フォルダのタブ |
| **コンテキストメニュー** | **PopupMenu** `/component/menu` | ◎ ContextMenu / DropdownMenu、アイコン、ショートカット表示、サブメニュー、チェック状態、区切り、リンク項目 | 右クリック（開く/コピー/移動/削除/プロパティ） |
| **ダイアログ** | Dialog `/component/dialog`（AlertDialog も） | ◎ モーダル、フォーカス管理、フォーム、スクロール、入場アニメーション | 名前変更/削除確認/競合解決 |
| **入力** | Input `/component/input`（Textarea, InputGroup, NumberInput, Combobox, Select） | ◎ 検証・マスク・クリア・prefix/suffix・サイズ・無効/読み取り専用 | パス入力、検索、リネーム |
| **アイコン** | Icon `/component/icon` + `gpui-kit-assets` | ◎ `IconName`（Lucide 1,818 + 独自 12）、サイズ/色/回転、カスタム SVG path/bytes。既定 101 個は 44 KiB | ファイル種別・操作アイコン |
| **リサイズ可能ペイン** | Resizable `/component/resizable` | ◎ パネルグループ、ドラッグハンドル、サイズ制約、固定、ネスト、表示切替 | ツリー/一覧/詳細の 3 ペイン |
| **ドック** | Dock `/component/dock`（base にも Dock） | ◎ タブ・スプリット・エッジドック・レイアウトのシリアライズ/永続化（"production-ready"） | プロ向けのドッキング UI（MVP では過剰） |
| **パンくず** | **該当部品なし**（sitemap 386 URL を走査、`breadcrumb` 0 件）【検証済】 | ▲ 自前実装。`gpui-base` の `History`（ブラウザ風の戻る/進む履歴）とボタン/Icon を組合せ | パス表示・祖先移動 |
| ツールチップ | Tooltip `/component/tooltip` | ◎ 遅延表示、キーボードショートカット表記、リッチ内容 | ツールバー説明 |
| 進捗 | Progress `/component/progress`（円形 ProgressCircle も） | ◎ | コピー/移動/削除の進捗 |
| 通知 | Notification `/component/notification`（OS 通知は `/docs/system-notification`） | ◎ ウィンドウ右上トースト、自動消去、種類、配置、アクション | 完了/エラーのトースト |
| テーマ | Theme `/component/theme` + Root `/component/root` | ◎ ライト/ダーク、色・タイポグラフィ・角丸。Root がテーマ/通知/ダイアログを束ねる | ライト/ダーク対応 |
| サイドバー | Sidebar `/component/sidebar` | ◎ 折りたたみ、ネストメニュー、バッジ、右配置、コンテキストメニュー | 左ペイン（場所/お気に入り） |
| ツールバー | Toolbar `/component/toolbar` | ◎ コマンド、グループ、キーボード | 上部操作バー |
| ステータスバー | StatusBar `/component/status-bar` | ○ 左右中央の 3 領域 | 下部の件数/選択状態 |
| タイトルバー | TitleBar `/component/title-bar` | ○ ウィンドウ操作 + カスタム内容 | 独自タイトルバー |
| スクロール | Scrollable `/component/scrollable` | ◎ カスタムスクロールバー、仮想化連携 | 一覧スクロール |
| ドラッグ&ドロップ | List の D&D 並べ替え / GridLayout のドラッグ・リサイズ `/component/grid-layout` | ▲ アプリ内 D&D はあり。**OS 外部からのファイルドロップは専用部品が見当たらない**【未確認】 | ファイル移動、外部からの取り込み |
| 空状態/ローディング | Empty `/component/empty`, Skeleton, Spinner, Shimmer | ○ | 空フォルダ、読込中 |
| 検索/コマンド | Command `/component/command`（コマンドパレット）, Combobox, Select | ○ | ファジー検索、ジャンプ |
| 添付/プレビュー | Attachment `/component/attachment` | ▲ アップロード用の添付 UI。ファイルプレビュー部品ではない | 参考程度 |

その他: README は「75+ components」、sitemap は英語 + zh-CN で 386 URL。`Breadcrumb` 不在は公式 docs の
索引（llms.txt）にも記載がないことを確認済み。

---

## 3. ビルド要件

### 3.1 Linux / Wayland / X11（本命環境: Kubuntu 2026.04 KDE Plasma 6 Wayland / NVIDIA GPU）

- **feature flags**:
  - `gpui-pre`（コア crate）の既定 feature は `["font-kit", "wayland", "x11", "windows-manifest"]`【検証済】。
  - `gpui-pre-platform`（アプリの入口）の既定 feature は `["multithreaded"]` **のみ**。Linux では `wayland` / `x11` を
    明示的に有効化する必要がある【検証済】。両方推奨（feature がレンダラとテキストシステムもコンパイルする）。
  - macOS は `font-kit` 必須（無いとグリフが描画されない）。Windows は feature 不要（Win32 + DirectWrite）。
- **必要システムライブラリ**（gpui-kit 公式インストールページ、Ubuntu 24.04 で検証済みの apt 一覧）【検証済】:
  ```bash
  sudo apt update
  sudo apt install -y gcc g++ clang libfontconfig-dev libwayland-dev \
    libwebkit2gtk-4.1-dev libxkbcommon-x11-dev libx11-xcb-dev \
    libssl-dev libzstd-dev libasound2-dev vulkan-validationlayers libvulkan1
  # pkg-config が無ければ追加
  ```
  - `libasound2-dev` は `speech` feature のみ必要。
  - **Vulkan ドライバが必須**（`libvulkan1` だけでは不十分）。NVIDIA GPU は NVIDIA proprietary ドライバで Vulkan 可。
  - `libwebkit2gtk-4.1-dev` は WebView 機能用（使わなくてもリンク要件に入る場合あり）。
- **Rust**: gpui-kit の locked 依存（`oo7 0.6.0`）が Rust 1.92+ を要求。手元は 1.99.1 なので OK【検証済/Lead 環境】。
- **Zed 側の Linux 前提**（work/zed/docs/linux.md、2026-11-10 時点の Zed ドキュメント）:
  Vulkan 対応 GPU、glibc >= 2.31（x86_64）、ビルドは `script/linux` で依存導入、Flatpak は `script/flatpak/*`。
- **Wayland 対応状況**: GPUI/Zed は Wayland を first-class feature として実装（`wayland` feature、layer-shell 対応コードも
  gpui スナップショットに存在）。gpui-kit の CI も ubuntu-latest で Vulkan 描画テストを実施。**KDE Plasma 6 Wayland での
  実機検証は未実施（未確認）**。IME・グローバルショートカット・画面共有などの周辺機能は GPUI 側の成熟度に依存する。
- 開発時の体感性能対策として、公式 docs は `[profile.dev.package]` で `gpui-pre` / `gpui-component` / `gpui-kit` /
  `rustybuzz` / `taffy` / `ttf-parser` を `opt-level = 3` にする設定を推奨【検証済】。
- **実アプリの知見（hoja）**: Wayland の D&D で `wl_data_device.start_drag` の icon に gpui が `None` を渡すため、
  ウィンドウ外に出るとドラッグチップが消える。hoja は別 `wl_seat` バインドで自前ドラッグを実装して回避。
  `raw-window-handle` は **gpui がリンクする版（0.6）と一致**させる必要がある（不一致で `HasWindowHandle` が別 trait になる）。

### 3.2 Windows

- **Windows 10 以降**、Visual Studio 2022 Build Tools / Community の「Desktop development with C++」ワークロード
  （MSVC + Windows SDK）、CMake を PATH に。**GNU ツールチェーン不可、MSVC 必須**【検証済: gpui-kit installation】。
- 追加 feature 不要。GPUI は Win32 + DirectWrite、DirectX レンダラ。`windows-manifest` feature が
  `resources/windows/gpui.manifest.xml` を埋め込む（gpui スナップショットに manifest/rc あり）【検証済】。
- gpui-kit CI に `x86_64-windows-msvc` on windows-latest のテストあり。gpui-fast は WebView2 合成による
  native content 合成（Wayland/X11 含む）をサポートと README に記載。
- 実アプリの例（FileForce/files、Windows ビルド）: `winresource = "0.1"` を build-dependencies に入れ、
  アプリの `.ico` を exe のリソース（ウィンドウ/タスクバー/Explorer 表示）として埋め込み。
  `windows-sys` で `Win32_UI_Shell` / `Win32_System_Power` 等を利用。

### 3.3 macOS

- **macOS 15 以降**、Xcode または Command Line Tools（`xcode-select --install`）【検証済: gpui-kit installation】。
- レンダリングは Metal 固定。`font-kit` を有効にしないとテキストが「レイアウトはされるがグリフが出ない」。
  gpui-kit は既定で font-kit を有効化済み。gpui-pre を直接使う場合は `gpui-pre-platform` の `font-kit` feature が必須。
- User 環境（Linux）なので macOS 実機検証は不可（**未確認**）。CI（GitHub Actions macos-latest = aarch64）での
  コンパイル/テストは gpui-kit 側で実施されている。

### 3.4 実装上の注意（GPUI 固有）

- アプリのエントリは `gpui_platform::application()`（gpui-kit では `gpui_kit::application()`）。`Application::new()` は
  gpui 0.2.2 時代 / Zed 公式 README の API。
- gpui-kit では `gpui_kit::init(cx)` を最初に呼ぶ（base のテーマ/フォーカストラップ登録を含む）。二重初期化注意。
- アイコンは `application().with_assets(gpui_kit::assets::Assets)` で登録。自前アイコンは `AssetSource` を合成。
- 状態は `Entity` + `Context`。非同期は `Task`。テストは `#[gpui_kit::test]` + `TestAppContext` / UI 統合テスト。
- `gpui-fast` を有効化すると retained mode になり、追跡対象外の描画状態は `cx.notify()` が必須（移行時の注意）。
- アプリ識別子（通知・設定の紐付け）は `cx.set_app_identity("com.example.helloworld", "HelloWorld")` で起動時に設定
  【検証済: /docs/packaging】。

---

## 4. 配布・パッケージング・CI

### 4.1 GPUI Kit 公式が案内する配布（/docs/packaging、2026-11-11 取得）

**「`cargo build --release` は実行ファイルであってインストーラではない。パッケージ識別子・アイコン・外部ファイル・
署名・リリーステストはアプリの責任」**という方針。各プラットフォームでビルドする。

| Platform | ポータブル | インストール版 | 要点 |
|---|---|---|---|
| macOS | `.app` を ZIP/tar | `.dmg` | `.app` は `Contents/MacOS` + `Contents/Resources` + `Info.plist`。外部配布は Developer ID 署名 + hardened runtime + notarization（`codesign --options runtime` → `hdiutil create` → `notarytool submit --wait` → `stapler staple`）|
| Windows | `.exe` の ZIP | Inno Setup `.exe` | `packaging.iss` の実例（AppId/AppVersion/DefaultDirName/Icons/Run）。署名は **exe → インストーラの順**。Inno の uninstaller 署名は別対応 |
| Linux | `.tar.gz` | `.deb` | `dpkg-deb --root-owner-group --build`。`DEBIAN/control` の `Depends` は `dpkg-shlibdeps` で自アプリから導出。desktop entry / hicolor icon / `usr/lib/<app>`。`/usr/local` は Debian policy で禁止。X11 または XWayland の WebKitGTK 前提（WebView 使用時） |

- 付属の `install.sh` 例は「ローカル archive + 信頼できる経路から渡した SHA-256」を検証し、`~/.local/bin` と
  XDG data dir に非 root インストールする（Python 3 使用）。
- アプリ識別子: `cx.set_app_identity("com.example.helloworld", "HelloWorld")` を起動時に。
- 検証手順として「ダウンロードしたパッケージを別ディレクトリに展開して起動」「N → N+1 上書き」「クリーン VM で
  初回起動・描画・入力・通知」「SHA-256 の再取得比較」を明記。
- GPUI Kit 自体は **cargo-packager / cargo-bundle を使っていない**（手順は素のコマンド + Inno Setup）。

### 4.2 一般ツール（crates.io で 2026-11-11 確認）

| ツール | 最新版 / 公開日 | license | 所見 |
|---|---|---|---|
| `cargo-bundle` | 0.12.0 / **2026-09-20** | MIT OR Apache-2.0 | 活発。macOS `.app`（`.icns`/identifier/category を `[package.metadata.bundle]` に記述）、Linux `.deb` も。Atlas が実際に使用 |
| `cargo-packager` | 0.11.8 / **2025-11-27** | Apache-2.0 OR MIT | 約 1 年更新なし。`.deb`/AppImage/MSI/NSIS/DMG を単一設定で。採用するなら最新動作を要検証 |
| `cargo-dist` | 0.34.0 / **2026-11-02** | MIT OR Apache-2.0 | 活発。CLI バイナリ配布向け（インストーラ/アーカイブ/チェックサム）。GUI インストーラは対象外 |

- → **推奨**: Linux は `.deb`（+ AppImage は必要になったら）、Windows は Inno Setup、macOS は手順どおり `.app` + DMG 署名。
  `cargo-bundle` は macOS `.app` と Linux `.deb` の省力化に使える。`cargo-packager` は更新停滞のため「使うなら検証」。
- 配布チャネル: GitHub Releases に SHA-256 を併記。Kubuntu 向けは `.deb` を第一候補にする（ユーザー環境指示に合致）。

### 4.3 実プロジェクトの CI マトリクス（検証済の実例）

gpui-kit リポジトリ `.github/workflows/ci.yml`（2026-11-11 取得）:

```yaml
test:
  strategy:
    matrix:
      include:
        - target: aarch64-apple-darwin
          run_on: macos-latest
        - target: x86_64-linux-gnu
          run_on: ubuntu-latest
        - target: x86_64-windows-msvc
          run_on: windows-latest
```

- Linux/macOS は `script/bootstrap` で依存導入、Linux は `mesa-vulkan-drivers` を追加して offscreen 描画テスト。
- `gpui-fast` も ubuntu/macos/windows の 3 OS + WebAssembly（nightly-2026-10-31 固定）でテスト。
- リリースは `v*` タグで `script/publish-crates`（gpui-kit-assets → macros → base → fps → component → kit → webview → shell の順に
  `cargo publish`、既存版はスキップして再開可能）。**アプリ配布のワークフローは含まれない**（crate 公開専用）。

### 4.4 アセット/アイコン

- `gpui-kit-assets`: コード Apache-2.0、Lucide SVG は ISC（Feather 由来は MIT）。`IconName` カタログを使うだけで
  全 SVG がバイナリに入るわけではなく、既定 101 個 = 約 44 KiB、`AllAssets` 明示時のみ 1,830 個 = 約 731 KiB。
- 自前アイコンは `AssetSource` に別ソースを実装して合成（`icon_assets!` マクロで選択埋め込みも可）。
  ファイル種別アイコン（拡張子別）は自前で用意する必要がある（**未確認**: ファイルアイコンテーマ部品の有無）。
- アプリアイコン: macOS `.icns`（`CFBundleIconFile`）、Windows `.ico` + `winresource`、Linux 256×256 PNG + desktop entry。

---

## 5. 供給網・ライセンス・監査

### 5.1 公開主体（検証済）

| 項目 | 内容 |
|---|---|
| `gpui-pre*` / `gpui-kit*` / `gpui-base` / `gpui-component` / `gpui-fast*` の crates.io owner | **huacnlee ただ 1 名**（crates.io owner id 90689） |
| huacnlee とは | Jason Lee。GitHub user since 2008-04、followers 5,608、company **@longbridge**、blog huacnlee.github.io【GitHub API 検証済】 |
| 所属 | Longbridge（金融系。Longbridge Pro デスクトップアプリを提供） |
| リポジトリ | `longbridge/gpui-kit`（20,270★ / 1,237 fork / open issues 145 / created 2024-06-13（旧 gpui-component リポジトリ）/ Apache-2.0）。`longbridge/gpui-fast`（560★ / Apache-2.0 / created 2026-09-24） |
| 信頼性の評価 | 個人 1 アカウントが publish 権限を持つ **bus factor = 1**。ただしコードは Zed の Apache-2.0 スナップショット + Longbridge の OSS。GitHub 上の実体・商用実績・週次リリースは確認できる。**独立したセキュリティ監査は未確認** |

### 5.2 スナップショットの来歴（検証済）

- `gpui-pre` 0.3.12 の description: 「Zed's GPU-accelerated UI framework (gpui-pre snapshot of **zed@d2a4fb6**)」。
- **公開マニフェストに来歴メタデータ**:
  ```toml
  [package.metadata.gpui-pre]
  zed-crate = "gpui"
  zed-version = "0.2.2"
  zed-rev = "d2a4fb6ded7c470be25d1541281da3882495b65b"
  ```
  `gpui-pre-platform` は `zed-crate = "gpui_platform"`, `zed-version = "0.1.0"`, 同じ `zed-rev`。
- `zed-industries/zed` の commit `d2a4fb6ded7c470be25d1541281da3882495b65b` を GitHub API で確認:
  committer date **2026-11-08T20:08:14Z**、メッセージ「tabular_data_preview: Range selection, edge jumps, and page navigation (#65161)」。
  ← 公開 2026-11-09 00:30 UTC なので、**約 4.4 時間前の Zed 先端**をスナップショットしている。
- パッケージの lib 名は `gpui` / `gpui_platform`（依存側からは通常の GPUI として import 可能）。
  README・LICENSE-APACHE は Zed のものを保持。gpui-kit docs は「公開処理はパッケージ名とマニフェストのみ調整」と明記。
- ただし **crates.io の `repository` メタデータは `zed-industries/zed` を指す**（`gpui-pre` / `gpui-pre-platform`）。
  「Zed 公式」と誤認しやすいので注意（owner は huacnlee 個人）。
- 追加の要注意 crate: `gpui-pre-reqwest`（reqwest の再公開。repo は seanmonstar/reqwest だが公開者は huacnlee。
  0.12.15 / 2026-09-03）。`gpui-kit-assets` が依存。名称が独立しているため依存グラフ上の監視対象になる。
- **`.tmp-deps-audit/gpui-pre.diff` の実体について（重要な訂正）**: このファイルは「Zed upstream と再公開版の差分」では
  なく、**crates.io の `gpui-pre` ソースと AGPL アプリ Argus が `vendor/` に置いたパッチ版との差分**である
  （diff ヘッダが `.tmp-deps-audit/crates/up-gpui-pre` → `security-review/Argus/vendor/gpui-pre`）。
  すなわち「アプリ側が GPUI Kit / gpui-pre にどれだけ手を入れたか」の実例であり、再公開の忠実性の証拠ではない。
  再公開の忠実性は `[package.metadata.gpui-pre]` の `zed-rev` と、Zed commit `d2a4fb6` の存在・日時で確認するのが正しい。
  Argus のパッチ規模は Linux 16 ファイル / Windows 12 ファイル、+5,453/-776 行（アニメーション用 view 無効化など）。

### 5.3 バージョン固定戦略（検証済）

- gpui-kit は `gpui-pre = "=0.3.12"` のように **完全一致（`=`）でピン**。CI に `script/check-gpui-pin.ts` があり、
  全 `gpui-pre-*` が同一の `=x.y.z` であることを検証（`bun script/check-gpui-pin.ts`）。
- そのスクリプトのコメントに実事故の記録: 「caret 指定だと週次の gpui-pre リリースで、gpui-component がビルド検証して
  いないスナップショットにアプリが移ってしまい、**gpui-kit 0.6.4 が gpui-pre 0.3.6 公開当日にビルドできなくなった
  (#3156)**」→ 現在は完全ピン + `Cargo.lock` が前提。
- `gpui-fast` は `0.1.x` の互換更新を許す（docs に「Fast dependencies allow compatible 0.1.x updates」）。
- アプリ側の推奨: `gpui-kit = "=0.9.1"` に固定し、`Cargo.lock` をコミット。更新はリリースノート確認のうえ隔週程度でまとめる。
  1 週間で 1 minor 上がる世界なので「最新追従」は無理をしない。

### 5.4 `gpui-fast` のリスク（検証済）

- 出自: Zed GPUI のフォーク。README は「A performance-focused fork of Zed's GPUI ... experimental」と明記。
  v0.1.0 は Zed の commit `a1b71072e5` を取り込み、以降は `fast/` モジュールに独自実装を置いて upstream 変更に追従する方針。
- 変更内容: retained mode（変更のない view/layout/text 測定の再利用）、scroll layers（macOS/Linux/Windows で
  GPU タイル・行の再利用）、native window composition（WebView 等のネイティブコンテンツの上に GPUI の
  menu/popover/dialog を重ねる）。自己計測ではスクロール時 CPU -73〜-88% 等（**ベンダー自己申告値**、独立検証なし）。
- リスク: (1) upstream とのレイアウト/描画挙動の差、(2) retained mode では tracked 外の状態変更に `cx.notify()` 必須、
  (3) 独自の `gpui-fast-platform` / `gpui-fast-linux` 等を増やしており、供給網の面がさらに 1 段増える。
- 判断: **MVP では無効。** 性能が問題になったとき `features = ["gpui-fast"]` で A/B できるのが gpui-kit の利点。

### 5.5 ライセンス整理

| 対象 | ライセンス | アプリへの影響 |
|---|---|---|
| GPUI 本体（`gpui` / `gpui-pre` / プラットフォーム） | **Apache-2.0** | 自由に利用可（特許許諾・帰属表示）。 |
| `gpui-base` / `gpui-component` / `gpui-kit` / `gpui-kit-assets` / `gpui-fast` | **Apache-2.0**（GitHub の license 検出は NOASSERTION: DOCS の CC BY 4.0 等が混在するため） | 自由に利用可。ドキュメント散文は CC BY 4.0（引用時はクレジット）。 |
| Lucide アイコン | ISC（Feather 由来 MIT） | 帰属表示を LICENSE-LUCIDE に含める。 |
| Zed アプリ本体のコード（`crates/project_panel` 等のファイルエクスプローラ実装、`theme` / `file_icons` 等） | **GPL-3.0**（Zed リポジトリは GPL と Apache の混在） | **コピー不可**（非 GPL アプリに持ち込むと GPL 感染）。実例: hoja は Zed の `theme` / `file_icons` / `theme_settings` を git 依存したため **アプリ全体を GPL-3.0-or-later** にしている。GPUI フレームワーク部分（Apache-2.0）は OK。 |
| `gpui-ce`（Community Edition） | Apache-2.0 | フォーク自体は緩い。ただし upstream 互換性は保証されない。 |
| `security-review/Argus`（第三者の GPUI Kit アプリ） | **AGPL-3.0-or-later** | コピー不可。ただし `[patch.crates-io]` で gpui-pre を vendor する実例として参照のみ。 |

### 5.6 既存の cargo-audit 結果（.tmp-deps-audit/audit.out、2026-11-11 02:22 実行分）

- 結果: **脆弱性（vulnerability）の検出は 0 件。ただし「未整備（unmaintained）」警告 8 件 + ネットワーク失敗 367 件**。
- 警告 8 件:
  `instant 0.1.13` / `paste 1.0.15` / `rustls-pemfile 2.2.0` / `rustybuzz 0.20.1` / `ttf-parser 0.25.1` /
  `zbus_names 4.3.5` / `zvariant 5.15.1` / `zvariant_derive 5.15.1`（すべて RUSTSEC unmaintained、`warning: 8 allowed warnings found`）。
- 367 件の `error: couldn't check if the package is yanked: registry: request could not be completed` は
  当時のネットワーク/レート制限による取得失敗で、**監査結果は不完全**。再実行が必要（§8）。
- 補足: `paste` は gpui-component の依存（アトリビュート用途）で、代替（`pastey` 等）への移行が upstream で進むか要観察。
  `rustybuzz`/`ttf-parser` はテキスト整形系で GPUI 由来。いずれも脆弱性ではない。

### 5.7 推奨する検証ツール

| ツール | 用途 | 備考 |
|---|---|---|
| `cargo-audit` | RUSTSEC 脆弱性/未整備警告 | ネットワーク失敗に注意して再実行。CI では `--deny warnings` は未整備まで落ちるので `--ignore` 管理。 |
| `cargo-deny` | ライセンス・重複版・脆弱性・依存元の一括ポリシー | `licenses = ["Apache-2.0","MIT","ISC","Unicode-3.0",...]` を明示。GPL 混入検出に有効。 |
| `cargo-vet` | 依存の監査記録（cargo vet） | `gpui-pre*` は監査対象外なので exemption で運用開始し、必要ならスナップショットの diff 監査。 |
| `cargo-about` / `cargo-licenses` | 帰属表示の生成 | Lucide/ISC などの NOTICE を自動化。 |
| `cargo machete` | 未使用依存の検出 | gpui-kit の CI でも使用。 |

---

## 6. 既存の GPUI ファイルマネージャ / 参考プロジェクト（GitHub 検索 + Cargo.toml 実査、2026-11-11）

検索: `gpui file manager` / `gpui file explorer` / `topic:gpui`（stars 降順）。★ は取得時点。
「スタック」列はリポジトリ直下の Cargo.toml を取得して確認したもの（取れなかったものは未確認）。

| リポジトリ | ★ | 更新 | スタック（実査） | 概要・教訓 |
|---|---:|---|---|---|
| `neondeex/atlas-filemanager` | 189 | 2026-11-06 | gpui-component の**フォークを vendor** + `gpui-ce` を `[patch."https://github.com/gpui-ce/gpui-ce"]` で vendor。AGPL-3.0-or-later。`[package.metadata.bundle]` で cargo-bundle 設定 | Windows 向け GPU アクセラレート・ローカルファースト。**GPUI 側の不足をフォークで埋める前提**の実例。 |
| `WizenPainter/shuffle` | 174 | 2026-10-24 | **公式 `gpui = "0.2.2"`** | macOS Finder 代替。公式 crate でも単一プラットフォームなら成立。 |
| `Mingcodeingmiao/rust-explorer` | 34 | 2026-11-11 | **公式 `gpui = "=0.2.2"`**（default-features=false, optional） | Windows ネイティブ。 |
| `Carlton2001/Oxyde_FM` | 25 | 2026-11-11 | Rust コア + GPUI UI（スタック未取得） | 高速ファイルマネージャ。 |
| `pikachuu184/Piku` | 24 | 2026-10-21 | （未取得） | モノクロームの GPU アクセラレート型。 |
| `XPOL555/GPUIFileManager` | 11 | 2026-10-27 | （未取得） | Windows 向け軽量・省メモリ。 |
| `skwir-org/skwir` | 10 | 2026-11-10 | **Zed の `crates/gpui` を vendor して path 依存**（`vendor/zed/crates/gpui`, features wayland/x11/font-kit）。Apache-2.0, rust-version 1.97 | **クロスプラットフォーム**。独自パッチを当てた vendoring 方式。プラグイン + ネイティブ worker（PDF/メディア）構成。 |
| `slastra/hoja` | 4 | 2026-11-10 | **Zed へ git 依存（rev 固定）**: `gpui`, `gpui_platform`, `theme`, `file_icons`, `theme_settings`, `fuzzy_nucleo`。GPL-3.0-or-later | **Linux** 向け。Wayland D&D の落とし穴、gpui_platform 必須、GPL 感染の実例。 |
| `weiaoli2/Schnauz` | 4 | 2026-11-08 | （未取得） | macOS Finder 代替。ワークスペース/タブ/ターミナル統合。 |
| `sonatina-dream/ferrous-commander` | 3 | 2026-10-20 | （未取得） | macOS 向け 2 ペイン型。 |
| `FileForce/files` | 2 | 2026-11-10 | **`gpui-kit = "=0.8.1"`** + `gpui-pre = "=0.3.10"`（test-support）。FSL-1.1-MIT | macOS/Linux。Git/SSH/AI エージェント/拡張機能。`winresource`・`trash`・`notify-rust`・`souvlaki` 等の実用例。 |
| `nanderto/file-explorer` | 1 | 2026-11-11 | （未取得） | macOS。Windows Explorer 風の挙動。 |
| `noh-rs/nohrs`（日本語記事） | - | 2025-12 開始 | 公式 `gpui` + gpui-component（仮想化リスト） | macOS 先行・将来クロスプラットフォーム予定。Zenn 記事（2025-12-14）で設計思想を解説。 |

その他の関連情報:

- **huacnlee 本人が Omarchy 向けファイルマネージャを GPUI Shell + JavaScript で開発中**（2026-10-14 の X 投稿、
  omarchyarchive.com にアーカイブ）。機能: **タブ・スプリットペイン・仮想化リスト・Space プレビュー・ファイル操作・
  ゴミ箱/復元・Omarchy テーマ自動切替**。実装は `gpui-shell-benchs` の `apps/finder`。Rust コードなしで
  GPUI のファイルマネージャ UI が成立することの実証であり、部品の方向性が一致している根拠になる。
- **Spacedrive**（クロスプラットフォームのファイルマネージャ、Rust）が `docs/plans/` に GPUI 移行計画
  （"Two GPUI Interfaces", "Native App on GPUI"）を公開。既存 Tauri アプリからの移行検討例。
- **gpui-kit 自身の examples**: `system_monitor`（リアルタイムチャート）, `dock`, `table_in_scrollable`, `sidebar`,
  `tig`（git UI 風）, `editor`, `markdown`, `webview` など。story ギャラリーで全コンポーネントを確認可能。
- **実例の教訓（横断）**:
  - 単一 OS（Windows または macOS）向けの実装が大多数で、**Linux まで含むクロスプラットフォームはまだ少数**（skwir,
    FileForce, hoja など）。Linux は Wayland/X11 の feature と D&D の癖が追加コスト。
  - OS ネイティブ機能（Windows Shell / macOS NSWorkspace / trash / アイコン取得 / ドライブ列挙）は自前抽象化が必要。
    既存アプリは `trash`・`notify-rust`・`windows-sys`・`objc2` 等を直接使っている。
  - 既存アプリは GPUI を「素のまま」で使うか「フォーク/vendor してパッチ」するかの二択に寄っている
    （Atlas, skwir, hoja, Argus）。**upstream の不足をアプリ側で埋める前提**が現実的。
  - ライセンスはプロジェクトごとに大きく異なる（Apache-2.0 / AGPL / GPL / FSL）。Zed の GPL crate を引くと伝染する。

---

## 7. 既存 GPUI アプリの実績（参考）

- `longbridge/gpui-kit` 20,270★、`zed-industries/zed` 94,691★、`topic:gpui` には
  `AprilNEA/OpenLogi`（25,521★）、`zeronsh/zeron`（3,487★）、`sonorahq/sonora`（2,755★）、`vicanso/zedis`（2,197★）、
  `66HEX/frame`（2,268★）など大規模アプリが多数。GPUI は「実験的」段階を超えて実用アプリで使われている。
- `gpui-ce`（Community Edition、1,586★、2026-11-11 も更新）というフォークも活発。
- ただしその大半は Zed 本体の GPUI を git/サブモジュールで追従しているか、gpui-kit 経由。**crates.io 公式 `gpui` を
  そのまま使っている例は少ない**（WizenPainter/shuffle 等の単一 OS アプリに限られる）。

---

## 8. 未確認事項（次段で確認が必要）

1. **gpui-kit `/docs/packaging` の対象外部分**: 自動更新（`/docs/auto-update`）の具体実装、Flatpak/AppImage の手順は未取得
   （公式は「必要な形式だけ選べ」という方針で、Flatpak/AppImage の詳細は示していない）。
2. **OS 外部からのドラッグ&ドロップ**（デスクトップからウィンドウへのファイルドロップ）の GPUI API と gpui-kit 部品の有無。
   hoja の記述から GPUI の D&D 実装は Wayland で制約があることまでは確認。
3. **ネイティブファイルダイアログ**（開く/保存）の gpui-pre での公開 API（`Prompts` trait 相当）。
   gpui スナップショットに `src/window/prompts.rs` は存在するが、gpui-pre-platform からの利用可否は未確認。
4. **KDE Plasma 6 Wayland 実機での動作**（IME、クリップボード、ウィンドウ装飾、HiDPI、Vulkan/NVIDIA GPU）。
5. **`gpui-ce` の crates.io 公開状況と API 互換性**（GitHub のみ確認。Atlas は vendor 利用）。
6. **`gpui-pre-linux` の依存詳細**（wgpu、ashpd、accesskit_unix、notify-rust 等）は Lead が検証済み。
   本ノートでは `gpui-pre-platform` が `gpui-pre-linux/windows/macos/web/ios/wgpu` を =0.3.12 で選択することまで確認。
7. **cargo-audit の完全版**（ネットワーク失敗 367 件を除いた再実行）。`cargo-deny` でのライセンス/重複版チェックも未実施。
8. **huacnlee 個人アカウントのセキュリティ体制**（2FA、crates.io トークン管理、リリース署名の有無）。crates.io の
   署名（Sigstore）状況は未確認。
9. **gpui-kit の Windows/macOS 実機品質**（CI は通っているが、実機の描画・IME・メニュー統合は未検証）。
10. **Zed の GPL-3.0 範囲の正確な境界**（どの crate が GPL でどれが Apache か）。`crates/gpui/Cargo.toml` が
    Apache-2.0 であること、hoja が GPL 化したことは確認済みだが、`crates/project_panel` 等の個別確認は未実施。
11. **`gpui-pre-reqwest` の差分**（reqwest 0.12.15 の再公開だが、upstream とのコード差分は未比較）。

---

## 9. 参照一次情報（すべて 2026-11-11 取得）

- crates.io API:
  - https://crates.io/api/v1/crates/gpui
  - https://crates.io/api/v1/crates/gpui-pre （`/gpui-pre-platform` / `/gpui-pre-macros` / `/gpui-pre-linux` /
    `/gpui-pre-windows` / `/gpui-pre-macos` / `/gpui-pre-web` / `/gpui-pre-ios` / `/gpui-pre-wgpu` / `/gpui-pre-reqwest` も）
  - https://crates.io/api/v1/crates/gpui-kit （`/gpui-base` / `/gpui-component` / `/gpui-kit-assets` も）
  - https://crates.io/api/v1/crates/gpui-fast （`/gpui-fast-platform` も）
  - https://crates.io/api/v1/crates/cargo-packager, /cargo-bundle, /cargo-dist
  - owners: https://crates.io/api/v1/crates/gpui-pre/owners ほか
  - dependencies: https://crates.io/api/v1/crates/gpui-kit/0.9.1/dependencies ほか
- crate ソース（tarball, https://static.crates.io/crates/<name>/<name>-<ver>.crate）:
  - `gpui-pre-0.3.12/README.md`, `Cargo.toml.orig`（`[package.metadata.gpui-pre]` の来歴メタデータ）
  - `gpui-pre-platform-0.3.12/Cargo.toml.orig`（platform 別依存と features）
  - `gpui-kit-0.9.1/README.md`, `Cargo.toml.orig`, 正規化 `Cargo.toml`（`=0.3.12` ピン）
  - `gpui-component-0.9.1/README.md`, 正規化 `Cargo.toml`（`package = "gpui-pre"` エイリアス）
  - `gpui-fast-0.1.14/README.md`
  - `gpui-0.2.2/README.md`, features
- GPUI Kit ドキュメント:
  - https://gpui-kit.com/llms.txt （全ドキュメント索引）
  - https://gpui-kit.com/docs/installation
  - https://gpui-kit.com/docs/getting-started
  - https://gpui-kit.com/docs/packaging
  - https://gpui-kit.com/component/data-table, /component/table, /component/virtual-list, /component/list,
    /component/tree, /component/tabs, /component/menu, /component/dialog, /component/input, /component/icon,
    /component/resizable, /component/dock, /component/tooltip, /component/progress, /component/notification,
    /component/theme, /component/root, /component/sidebar, /component/toolbar, /component/status-bar,
    /component/title-bar, /component/scrollable, /component/empty, /component/command, /component/grid-layout
  - https://gpui-kit.com/docs/assets, /docs/auto-update, /docs/system-notification, /docs/test, /docs/comparison
- GitHub:
  - https://github.com/longbridge/gpui-kit （20,270★）
  - https://github.com/longbridge/gpui-fast （560★）
  - https://github.com/longbridge/gpui-kit/blob/main/.github/workflows/ci.yml
  - https://github.com/longbridge/gpui-kit/blob/main/script/check-gpui-pin.ts
  - https://github.com/longbridge/gpui-kit/blob/main/script/publish-crates
  - https://github.com/zed-industries/zed （94,691★）
  - https://github.com/zed-industries/zed/commit/d2a4fb6ded7c470be25d1541281da3882495b65b
  - https://github.com/zed-industries/zed/blob/main/crates/gpui/Cargo.toml （`version = "0.2.2"`, `license = "Apache-2.0"`）
  - https://github.com/zed-industries/zed/blob/main/docs/src/development/linux.md
  - https://github.com/gpui-ce/gpui-ce （1,586★, Apache-2.0）
- 既存ファイルマネージャ（GitHub search `gpui file manager` / `gpui file explorer` / `topic:gpui` + Cargo.toml 取得）:
  neondeex/atlas-filemanager, WizenPainter/shuffle, Mingcodeingmiao/rust-explorer, Carlton2001/Oxyde_FM,
  pikachuu184/Piku, XPOL555/GPUIFileManager, skwir-org/skwir, slastra/hoja, weiaoli2/Schnauz,
  sonatina-dream/ferrous-commander, FileForce/files, nanderto/file-explorer, noh-rs/nohrs
- 参考記事:
  - https://omarchyarchive.com/posts/huacnlee-gpui-file-manager/ （2026-10-14 の X 投稿アーカイブ）
  - https://zenn.dev/syuya2036/articles/7f02faaf8c8ec4 （Rust + gpui ファイルエクスプローラ、2025-12-14）
  - https://github.com/spacedriveapp/spacedrive/blob/11523b224d1422d98def9a1c0f99e89fafada00d/docs/plans/2026-08-18-native-app-gpui.md
  - https://github.com/spacedriveapp/spacedrive/blob/11523b224d1422d98def9a1c0f99e89fafada00d/docs/plans/2026-08-25-gpui-interfaces.md
- ローカル既存成果物:
  - `work/zed/docs/linux.md`（Zed の Linux 前提）
  - `.tmp-deps-audit/audit.out`（cargo-audit: 未整備 8 件、ネットワーク失敗 367 件）
  - `.tmp-deps-audit/gpui-pre.diff`（AGPL Argus の gpui-pre vendor パッチ差分: Linux 16 ファイル / Windows 12 ファイル、
    +5,453/-776 行）
  - `.tmp-deps-audit/owners-*.json`（全対象 crate の owner は huacnlee 1 名）
  - `security-review/Argus/Cargo.toml`（`[patch.crates-io]` で gpui-pre / gpui-pre-windows を vendor する実例）
