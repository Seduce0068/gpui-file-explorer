<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# GPUI File Explorer — デスクトップ試作

Rust + [GPUI](https://github.com/zed-industries/zed/tree/main/crates/gpui) 系スタック
（`gpui-kit` 0.9.1 = GPUI snapshot + GPUI Component + Lucide assets）で作った
ファイルエクスプローラの**試作**です。まず Linux / KDE Plasma / Wayland で動くことを
目標にしつつ、コードはクロスプラットフォーム設計（`std::fs` + `dirs` + `trash`）です。

ライセンス: **AGPL-3.0-or-later**（[LICENSE](LICENSE)、依存クレジットは [NOTICE.md](NOTICE.md)）。

![ウィンドウ（Wayland 実行、/tmp/gpui-fe-demo）](screenshot/window-crop.png)

## 検証環境

- Kubuntu 2026.04 / KDE Plasma 6 / Wayland（ネイティブ Wayland ウィンドウ）
- NVIDIA GTX 1660 SUPER / rustc 1.99.1 / cargo 1.99.1
- `gpui-kit 0.9.1` → `gpui-pre 0.3.12` + `gpui-pre-platform 0.3.12`（lib 名 `gpui`）

## ビルド

```bash
cd gpui-file-explorer   # リポジトリ直下
bash scripts/dev-setup.sh   # 初回のみ（下記「前提」を参照）
cd app
cargo build                 # 初回（依存込み）約 1m30s / 2回目以降 約2s / debug バイナリ 約 165MiB
cargo test                  # 36 tests（データ層 + 監視）
```

リポジトリ直下から全体をビルドする場合:

```bash
bash scripts/dev-setup.sh
cargo build --manifest-path app/Cargo.toml
```

### 前提: `libxkbcommon-x11` のリンク回避（Linux で `-dev` が無いホスト）

`gpui-kit` は `gpui-pre-platform` の `x11` / `wayland` feature を固定で有効にします。
`gpui-pre-linux` は `-lxkbcommon-x11` を要求しますが、Ubuntu/Kubuntu では実行時
ライブラリ（`libxkbcommon-x11-0`）と開発用ファイル（`libxkbcommon-x11-dev`:
バージョン無し `libxkbcommon-x11.so` と pkg-config）が別パッケージです。
`-dev` が無いと最終リンクが失敗します。

推奨（システム全体、sudo 必要）:

```bash
sudo apt install libxkbcommon-x11-dev
```

sudo を使わない場合は同梱スクリプトで回避できます（システムは変更しません）:

```bash
bash scripts/dev-setup.sh   # app/native-libs/libxkbcommon-x11.so を作成
```

- スクリプトは `libxkbcommon-x11.so.0` を探して `app/native-libs/` に symlink します。
  pkg-config で開発用ファイルが見つかるホストでは、逆に symlink を削除します。
- `app/build.rs` が `native-libs/` を **絶対パスで** リンカ検索パスに追加します
  （CWD に依存しないため、どのディレクトリから `cargo build` しても動作します）。
- symlink が無いホストでは `build.rs` は何もしません（通常のリンクで通ります）。


## 実行

```bash
cd app
./target/debug/gpui-file-explorer              # 前回のディレクトリ or $HOME
./target/debug/gpui-file-explorer /usr/share   # 引数で開始ディレクトリを指定
```

ウィンドウは 1100x700（コンテンツ）、Wayland `app_id` / KWin `resourceClass` は
`gpui-file-explorer` です。閉じると終了します。

## キーバインド

| キー | 動作 |
|---|---|
| `Up` / `Down` | 選択を上下に移動（選択が見えるようスクロール） |
| `Shift+Up/Down/Home/End` | 範囲選択（アンカーから拡張） |
| `Ctrl+クリック` / `Shift+クリック` | 個別トグル / 範囲選択 |
| `Ctrl+A` | 表示中の全項目を選択 |
| `Ctrl+1` / `Ctrl+2` | 詳細リスト / アイコングリッド表示（ツールバーのボタンでも切替） |
| `Ctrl+F` | 検索バーを開く（ディレクトリ表示のみ）。`Enter` で再帰検索、`Esc` で閉じる |
| `Ctrl+T` / `Ctrl+W` | タブを開く（現在のディレクトリ）/ アクティブタブを閉じる（最後の1枚は不可） |
| `Ctrl+Tab` / `Ctrl+Shift+Tab` | 次のタブ / 前のタブ（循環） |
| `Home` / `End` | 先頭 / 末尾を選択 |
| `Enter` | 選択項目を開く（ディレクトリ→移動、ファイル→既定アプリ） |
| `Backspace` / `Alt+Up` / ↑ボタン | 親ディレクトリへ |
| `Alt+Left` / `Alt+Right` | 戻る / 進む（履歴） |
| `Ctrl+H` | 隠しファイル（`.` 始まり）の表示切替 |
| `PageUp` / `PageDown` | 可視行数分の選択移動 |
| `Delete` | 選択項目をゴミ箱へ移動（確認ダイアログあり） |
| `Ctrl+R` / `F5` / ⟳ボタン | 再読み込み |
| `F2` | 選択項目の名前を変更（ダイアログ） |
| `Ctrl+Shift+N` | 新規フォルダ作成 |
| `Ctrl+C` / `Ctrl+X` | 選択項目をコピー / 切り取り（アプリ内クリップボード） |
| `Ctrl+V` | コピー/切り取り項目を現在のフォルダへ貼り付け（上書きしません） |
| `Esc` | 実行中のコピー/移動をキャンセル |
| `Ctrl+Q` | 終了 |
| ダブルクリック | ディレクトリ移動 / ファイルを既定アプリで開く |
| 右クリック | コンテキストメニュー（開く / ファイルマネージャで表示 / パスをコピー / ゴミ箱へ移動 / 新規フォルダ） |
| ヘッダクリック | 名前 / サイズ / 更新日時でソート（同一列で昇順⇔降順） |
| サイドバー / パンくずクリック | その場所へ移動 |

## 実装済み機能（P0）

- [x] ツールバー: 戻る・進む・親へ・更新、クリック可能なパンくず、サイドバー
      （ホーム / デスクトップ / ドキュメント / ダウンロード / ピクチャ /
      ミュージック / ビデオ / ルート / ゴミ箱）、「最近」と「ドライブ」一覧。場所の解決と存在確認は**起動時 + 再読込時のみ**
      行い、描画パスでは `user-dirs.dirs` の再読や stat をしない（毎フレーム I/O 禁止）
- [x] 仮想化詳細リスト（`uniform_list`）: アイコン / 名前（省略表示）/
      サイズ / 更新日時。ディレクトリ先頭、自然順ソート
      （`file2` < `file10`、大文字小文字を無視）
- [x] ステータスバー: 現在パス / 件数 / 選択項目 / 読み込み・エラー /
      非表示の隠しファイル数 / 監視状態
- [x] 非同期ディレクトリ読み込み（`cx.background_spawn`）。UI スレッドで
      `read_dir` をしない。ジェネレーション番号 + `Task` drop で古い結果を破棄。
      loading / error / empty（空フォルダ・隠しファイルのみ）表示
- [x] ナビゲーション: サイドバー、ダブルクリック、Enter、Backspace、↑、
      Alt+Left/Right、パンくず、browser 風の戻る/進む履歴
- [x] 選択: クリック / キーボード、`scroll_to_item(Nearest)` で追従、
      ステータスに選択内容表示
- [x] ファイルを開く: ダブルクリック / Enter → `cx.open_with_system`
- [x] コンテキストメニュー（右クリック位置に表示、外側クリック/Esc で閉じる）:
      既定アプリで開く / ファイルマネージャで表示（`cx.reveal_path`）/
      パスをコピー（`ClipboardItem`）/ ゴミ箱へ移動（ネイティブ確認ダイアログ後、
      `trash` クレート）/ 新規フォルダ。メニューは**行の `PathBuf` を捕捉**し、
      クリック時にパスで対象を再解決する（表示中に監視更新で並び替わっても
      別エントリへ誤操作しない）
- [x] ファイル監視: `notify` 8.2 + `notify-debouncer-full` 0.7、
      カレント + 親を非再帰 watch、400ms デバウンス、変更時に自動リフレッシュ。
      10 秒ごとの照合（reconcile）で取りこぼしを回復。自分の trash/mkdir 後は
      1.5 秒間イベントを抑制して自己ループを防止
- [x] 新規フォルダ作成は background executor で実行（`新しいフォルダ` の
      連番プローブ + `create_dir` を UI スレッドで行わない）
- [x] 設定の永続化: `~/.config/gpui-file-explorer/settings.json`
      （最終ディレクトリ・ソートキー・昇順/降順・隠しファイル表示）。
      起動時ロード、変更時保存。壊れた JSON は `settings.json.broken-<epoch秒>` に
      退避して既定値で起動（起動失敗にしない）。存在しない `last_dir` は無視
- [x] AGPL-3.0-or-later（LICENSE）、NOTICE、各ソースに SPDX ヘッダ

## 進行中（P1）

- [x] **安全なファイル操作エンジン**（`src/ops.rs`、UI 接続済み・ユニットテストあり）
  - 上書き禁止が既定（`ConflictPolicy::{Fail, Skip, Rename}`。`Overwrite` は意図的に無し）
  - コピーは `.partial-*` へ書き、完了後に hard link / rename で配置（部分ファイルを残さない）
  - シンボリックリンクはリンクとしてコピーし、削除時は**絶対に辿らない**（`symlink_metadata`）
  - 移動は同一 FS なら `rename`、`EXDEV` は「コピー成功後にのみ元を削除」
  - キャンセル（`CancelFlag`）で部分ファイルを掃除。ディレクトリを自身の配下へコピー/移動する操作を拒否
  - リネームは上書き拒否 + 大小文字のみ変更を 2 段階 rename で対応
  - 11 件のユニットテスト（上書き拒否 / Skip / Rename / 再帰コピー / symlink / キャンセル / EXDEV 相当 / 安全削除）
- [x] **UI 接続（コピー/切り取り/貼り付け）**: `Ctrl+C` / `Ctrl+X` / `Ctrl+V` と右クリックメニュー。
  貼り付けは常に `ConflictPolicy::Rename`（既存ファイルを上書きしない）、実行は background executor、
  `Esc` で協調キャンセル。完了後に件数/サイズをステータス表示し、貼り付け先を選択して再読込
- [x] **リネームダイアログ（F2 / 右クリック「名前変更」）**: 入力欄に現在名をプリフィル、
  Enter で確定 / Esc・キャンセルで中止。名前検証と既存先チェックを行い、
  実際の rename は background executor。失敗時はダイアログにエラーを表示し、
  元のファイルは変更しません。成功時は watcher を抑制して再読込し、新しい名前を選択
- [x] **コピー/移動の進捗表示**: エンジンが 64 KiB チャンクごとに累積バイトを通知し、
  `async_channel` 経由で前景へ転送。ステータスバーに「コピー中… 12.3 MB（Esc でキャンセル）」を
  約 80 ms スロットルで表示（操作完了後は最終結果で上書き）
- [x] **複数選択**: Ctrl+クリック（トグル）/ Shift+クリック・Shift+矢印（範囲）/ Ctrl+A。
  ステータスに「N 件選択」を表示。選択はパスをキーに保持し、再読込・ソート後も維持
- [x] **バッチ操作**: コピー/切り取り/貼り付け・ゴミ箱移動を選択全体に適用。
  ゴミ箱は件数付き確認ダイアログ、貼り付けは 1 件ずつ `Rename` ポリシーで処理し、
  一部失敗時は成功分と失敗件数を表示
- [x] **ゴミ箱ビュー（一覧・復元）**: サイドバーの「ゴミ箱」でシステムのゴミ箱を仮想一覧表示。
  削除日時の新しい順、選択/複数選択に対応。`Enter` / ダブルクリック / 右クリック
  「元の場所へ復元」で復元（衝突時はエラー表示、項目はゴミ箱に残る）。
  **完全削除は提供しません**（`Delete` は「未対応」表示）
- [x] **視覚的な進捗バー**: コピー/移動の前に総バイト数を background で事前計算し、
  ステータスバーにバー + パーセントを表示（例: `コピー中… 35.0 MB / 100.0 MB`）。
  進捗は 64 KiB チャンクごとに `async_channel` で前景へ送り、約 80 ms でスロットル
- [x] **アイコングリッド表示**: ツールバーの切替 / `Ctrl+2`。`uniform_list` の「1 行 = N セル」方式で
  仮想化し、ウィンドウ幅から列数を自動計算（1 セル 108px 基準）。設定に永続化（`"view": "icons"`）
- [x] **再帰検索（ファイル名）**: `Ctrl+F` → 検索バー → `Enter`。`std::fs` ベースの自作 walker で
  カレントディレクトリ配下を検索（新しい依存なし）。symlink ディレクトリは**辿らない**、
  隠し項目は既定で除外（Ctrl+H の表示設定に追従）、最大 2000 件 / 深さ 24 で打ち切り、
  `Esc` でキャンセル・終了。結果はルートからの相対パスで表示
  - [x] **タブ**: 複数ディレクトリをタブで保持（`Ctrl+T` / `Ctrl+W` / `Ctrl+Tab`、
  タブバーのクリック・「+」・「×」）。タブごとに**カレントディレクトリと戻る/進む履歴**を保持し、
  切替時に保存/復元。最後の1枚は閉じられません
- [x] **ドラッグ&ドロップ**:
  - OS（Dolphin 等）からウィンドウへファイル/フォルダをドロップすると**コピー**。
    フォルダ行/セルへドロップした場合は**そのフォルダへ**、それ以外はカレントディレクトリへ。
    常にコピー（移動しない）・`ConflictPolicy::Rename` で**上書きしない**・`Esc` でキャンセル。
    symlink フォルダはドロップ先にできません（安全側）
  - 行/セルを OS へドラッグすると外部ファイルドラッグとして渡す（選択全体を対象、ディレクトリ情報付き）
  - ゴミ箱・検索ビューでのドロップ、操作実行中のドロップは拒否
- [x] **最近使った場所**: 移動したディレクトリを最大 10 件まで設定に保存し、サイドバー「最近」に表示。
  クリックで移動。存在しないディレクトリは再読込時に除去（描画パスでは stat しない）
- [x] **ボリューム（ドライブ）一覧**: サイドバーに「ドライブ」セクション。Linux は
  `/proc/mounts` を解析（pseudo FS・システムマウントを除外、重複排除、octal escape 対応）。
  再読込時に再取得。クリックで移動

## 未実装 / 今後の候補（P1 以降）

- 検索結果の絞り込み（拡張子・種類）、ドラッグ&ドロップ
- ドラッグ&ドロップ、OS のファイルクリップボード連携
- サムネイル / プレビュー、OS ネイティブアイコン抽出
- 右クリックメニューの「空き領域」版（現状は行の右クリックと
  ツールバーの「新規フォルダ」ボタン。理由は既知の制限を参照）

## アーキテクチャ

| ファイル | 役割 |
|---|---|
| `src/main.rs` | ウィンドウ / UI レイアウト（ツールバー・サイドバー・リスト・ステータスバー）/ キー・マウス操作 / コンテキストメニュー / 非同期ロード / 設定・履歴の結線 |
| `src/model.rs` | `Entry` と `read_dir`、フィルタ、自然順ソート、人間向けサイズ/日時、選択計算、パンくず、fingerprint（GPUI 非依存 = テスト容易） |
| `src/history.rs` | 戻る/進む履歴スタック |
| `src/settings.rs` | serde_json 永続化（アトミック書き込み） |
| `src/icons.rs` | 拡張子 → Lucide アイコン分類（GPUI 非依存の分類 + アイコン写像） |
| `src/watcher.rs` | debounce 付き OS ファイル監視（カレント + 親、非再帰） |

設計方針:

- **データ層（model/history/settings/icons の分類）は GPUI 非依存**にして
  ヘッドレステスト。UI 層だけが GPUI に依存。
- 一覧は「全件（`all_entries`）」と「表示用（`entries`）」を分離し、
  隠しファイル切替・ソート切替は再読み込みなしで再構築。
- 監視スレッドは UI に触れず、`async-channel` でフォアグラウンドの
  GPUI タスクに通知（GPUI の Entity はスレッド非依存のため）。

## テスト

```bash
cargo test
# running 36 tests ... test result: ok. 36 passed; 0 failed
```

カバレッジ: ディレクトリ読み込み（ディレクトリ先頭・メタデータ・エラー）、
自然順/大文字小文字無視ソート、サイズ/更新日時ソートと方向、
隠しファイルフィルタ、選択クランプ/移動、履歴の戻る/進む/分岐、
設定の roundtrip・壊れた JSON・部分 JSON・存在しないディレクトリ、
fingerprint の変化検出、パンくず生成、一意な新規フォルダ名、
アイコン分類（拡張子・大文字小文字・ドットファイル）、
実 inotify でのファイル作成イベント検知。

## 実行検証記録（2026-11-11, このマシン）

- `cargo build`: 警告 0 / `cargo test`: 36 passed
- 起動: stderr 0 行（EGL/Vulkan/Wayland エラーなし）
- KWin D-Bus（Wayland ネイティブであることの確認）:

```text
$ qdbus6 --literal org.kde.KWin /WindowsRunner org.kde.krunner1.Match "GPUI File Explorer"
"0_{...}", "GPUI File Explorer", "wayland", ...
$ qdbus6 --literal org.kde.KWin /KWin getWindowInfo "{...}"
caption="GPUI File Explorer", resourceClass="gpui-file-explorer",
width=1100, height=728 (コンテンツ 1100x700 + タイトルバー),
minimized=false, fullscreen=false, type=0
```

- スクリーンショット: `screenshot/window-crop.png`（XDG Desktop Portal で取得し
  ウィンドウ領域を crop）
- **監視の実証**: 起動したまま bash から `from-bash-while-running.txt` を作成 →
  約2秒後に一覧へ自動反映（件数 12→13、行が追加）。
  `screenshot/watcher-refresh-crop.png` が「反映後」。stderr 0 行のまま。
- **設定の永続化の実証**: 引数ありで起動して終了 →
  `~/.config/gpui-file-explorer/settings.json` に下記（DESIGN.md §8.1 のスキーマ）が
  保存され、引数なしの再起動でそのディレクトリが復元されることを確認
  （`screenshot/settings-restore-crop.png` のパンくずが
  `/ › tmp › gpui-fe-persist-demo`）。

```json
{
  "version": 1,
  "last_dir": "/tmp/gpui-fe-schema-check",
  "sort": { "key": "name", "dir": "asc", "dirs_first": true },
  "show_hidden": false,
  "view": "details"
}
```
- 終了: `kill -TERM` 後、KWin のウィンドウ一覧から消えること（`{}`）を確認。
  デスクトップ設定・システムには一切変更なし。検証用の一時ディレクトリ・
  ポータル保存スクショは削除済みで、`~/.config/gpui-file-explorer/settings.json`
  も最終確認後に削除しています（Lead の独立検証でも残存なしを確認済み）。

## 配布（Linux DEB）

リポジトリ直下のスクリプトで DEB を作成できます（root 不要、ホストへはインストールしません）:

```bash
bash packaging/linux/build-deb.sh
# 出力: packaging/linux/build/gpui-file-explorer_<version>-1_<arch>.deb
```

`.desktop`（`StartupWMClass=gpui-file-explorer`）、hicolor アイコン、ドキュメントを同梱します。
依存パッケージはビルド時に `ldd` + `dpkg -S` から生成します。

## 既知の制限

- **ファイルを開く / リビールの失敗は検知できません**: 使用 API
  （`cx.open_with_system` / `cx.reveal_path`）は戻り値を持たないため、
  xdg-open 等が失敗しても「開きました」「表示しました」という成功文言のままです
  （AC-9）。失敗通知は P1 で D-Bus / プロセス終了コード監視などを導入して対応します。
- 設定の保存は tmp + rename のみで **fsync は行いません**（電源断耐性は保証外）。
  `version` は現状 1 固定で、将来スキーマのダウングレードは検出しません。
- 起動時 `canonicalize` / 各所の単発 `is_dir()`（起動・ナビゲーション時のみ）は
  同期実行です（影響はマイクロ秒級。DESIGN §4.4 の厳密適用は P1）。
- `Window::prompt` のダイアログ表示中に別の prompt を開かない保証はありません
  （GPS の再入制約に依存。失敗時はキャンセル扱いで安全側）。
- `SortSpec.dirs_first` は将来互換のため保存していますが、P0 は常に
  ディレクトリ先頭固定です（`false` は無視）。
- **マウス/キーボード操作の E2E 検証は未実施**です。ネイティブ Wayland ウィンドウに
  合成入力を送るツール（kdotool / ydotool / wtype 等）が本機に無く、
  `xdotool` は XWayland 専用のため使えません。キーボード・右クリック・
  確認ダイアログのコードはコンパイル検証 + データ層のユニットテストのみで、
  実押下は未確認です（PoC と同じ環境制約）。手動確認する場合は
  `Ctrl+H` / `Alt+←` / 行の右クリック / ゴミ箱移動などをお試しください。
- 右クリックメニューは**行の上**でのみ開きます。仮想リストが空白領域まで
  占有するため、空き領域の右クリックメニューは未対応です
  （新規フォルダはツールバーのボタンでも作成できます）。
- 複数選択はクリック/キーボードのみ（ラバーバンド選択は未実装）。リネームは
  フォーカス行（最後にクリックした行）にのみ適用されます。
- バッチ貼り付けは 1 件ずつ順次処理します（部分失敗時は成功分を残し、失敗件数を表示）。
  進行中のバッチは Esc でキャンセルできますが、処理済みの項目は元に戻りません。
- コピー/移動は**アプリ内クリップボード**のみ（OS のファイルクリップボードには書きません）。
  貼り付けは常に「別名で保存」（`ConflictPolicy::Rename`）で、上書き確認は出しません。
  進捗はステータスバーにバー + 累積/総バイトで表示します（約 80 ms 間隔）。
  総バイト数はコピー前の事前走査による**ベストエフォート**で、symlink は追跡せず、
  読めない項目は除外します（巨大ツリーでは事前走査に時間がかかります）。
  `Esc` で協調キャンセルできます。
- リネームの**実押下 E2E は未検証**です（Wayland への合成入力ツールが無いため）。
  ダイアログ描画とエンジン（`ops::rename_entry`）は検証済みですが、入力→Enter→反映の
  一連の操作は手動確認をお願いします。UI 検証用に `GPUI_FE_DEBUG_RENAME=1` で起動すると
  最初の項目のリネームダイアログを開いた状態で表示できます（スクリーンショット用）。
  同様に `GPUI_FE_DEBUG_SELECT_ALL=1`（全選択）、`GPUI_FE_DEBUG_TRASH=1`（ゴミ箱ビュー）、
  `GPUI_FE_DEBUG_TRASH_RESTORE=1`（最初の項目を復元）、`GPUI_FE_DEBUG_GRID=1`（グリッド）、
  `GPUI_FE_DEBUG_SEARCH=1` + `GPUI_FE_DEBUG_SEARCH_QUERY`（検索）、`GPUI_FE_DEBUG_TABS=1`（タブ）、
  `GPUI_FE_DEBUG_RECENT=1`（最近）、`GPUI_FE_DEBUG_DROP_PATH=<file>` + `GPUI_FE_DEBUG_DROP_TARGET=<folder>`（外部ドロップ相当を1回実行）
  の開発用フックがあります。
- 現状の削除は従来どおり `trash` のみで、完全削除はありません。
- ソートはヘッダクリックのみ（ツールバーにソート UI は無し）。
- ファイル名の非 UTF-8 バイト列は lossy 変換されます。
- 監視はカレントディレクトリと親の**非再帰**のみ。深い子孫は監視しません。
- 自己操作の抑制は時間ベース（1.5 秒）。まれに外部の同期的変更を
  1 回見逃す可能性があるため、10 秒 reconcile で回復します。
- 起動時の設定読み込みで最終ディレクトリが消えていれば `$HOME` にフォールバック。
- サムネイル・プレビュー・アイコン抽出なし（Lucide アイコンのみ）。
- ゴミ箱ビューは**システムのゴミ箱全体**を表示します（他アプリが捨てた項目も含む）。
  行はファイルとして表示され、サイズは「-」固定です（trash crate のメタデータ拡張は未対応）。
  復元は元の場所へ戻すのみで、衝突時は失敗し項目はゴミ箱に残ります。完全削除はありません。
- 詳細リストとグリッドの切替時、スクロール位置はリセットされます（選択は維持）。
- 検索は**ファイル名のみ**（内容検索なし）。隠し項目は表示設定に従い、symlink ディレクトリは
  辿りません。上限（2000 件 / 深さ 24）に達すると打ち切ります。検索結果から削除・リネームを
  行った場合、結果一覧は操作後に自動更新されないため、再検索が必要です。
- ボリューム一覧は **Linux（/proc/mounts）のみ**。空き容量・リムーバブルメディアの
  ホットプラグ検知・Windows/macOS のドライブ列挙は未対応です（macOS/Windows では
  「ドライブ」セクションが出ません）。
- タブバーは**ディレクトリ表示のみ**（ゴミ箱・検索中は非表示）。タブごとに履歴は保持しますが、
  選択行・スクロール位置は切替時にリセットされます。タブ構成は再起動で復元されません。
- 「最近」は全タブ共通の最大 10 件で、存在しないディレクトリは再読込時に消えます。
  タブごとの履歴はタブ側で保持します。
- ドラッグ&ドロップの**実操作 E2E は未検証**です（Wayland への合成入力ツールが無いため）。
  ドロップ受信パイプラインは `GPUI_FE_DEBUG_DROP_PATH` / `GPUI_FE_DEBUG_DROP_TARGET` で検証済み
  （カレント/フォルダ指定コピー・選択・再読込）。
- クロスプラットフォーム設計ですが、Windows/macOS でのビルド・実行は未検証です。
- **ブラウザ（wasm）では動作しません**。GPUI の Web バックエンド自体は存在しますが、
  本アプリは `std::fs` / `notify` / `trash` / OS ダイアログに依存しています。
  調査と移植案は [../notes/wasm-browser-notes.md](../notes/wasm-browser-notes.md) を参照。

## 設計（DESIGN.md）からの差分

DESIGN.md §4.1 の指示に従い、P0 実装で設計からずれた点を記録します
（受入条件 AC-1〜AC-13 には影響しない範囲）。

| 項目 | DESIGN.md | 本実装（P0） | 扱い |
|---|---|---|---|
| モジュール構成 | `app.rs` + `model/*` + `fs/*` + `ui/*` に分割 | `main.rs`（UI+操作）+ `model.rs` / `history.rs` / `settings.rs` / `icons.rs` / `watcher.rs` のフラット構成（GPUI 非依存のデータ層は分離済み） | P1 で `ui/*` へ抽出 |
| キー入力 | 大半を `actions!` + `KeyBinding` + `.key_context("Explorer")` で実装 | ルート要素の `track_focus` + `on_key_down` で実装（矢印キー等と同じ経路に統一） | キー動作は §3.4 の表をカバー。Action 化は P1 |
| 設定スキーマ | `version` / `sort`（`key`/`dir`/`dirs_first`）/ `show_hidden` / `view` | **DESIGN.md §5.7・§8.1 のスキーマに一致**（`settings.json` サンプル参照） | 一致 |
| 選択 | `Selection`（`PathBuf` キー、複数選択の余地） | `selected: usize`（単一選択） | P0 要件どおり。P1 で型置換 |
| ロード状態 | `LoadState` / `FsError` の型 | `loading: bool` + `error: Option<String>` | 表示同等。P1 で型化 |
| ステータスバー | gpui-component `StatusBar` | 自作 `div`（5 領域の 1 つとして実装） | 見た目は同等。P1 で置換可 |
| テーマ | gpui-component 既定ダークテーマの意味色 | Catppuccin 系の色を直書き（`Theme::change(Dark)` はメニュー/ダイアログ用に実施） | P0 は許容（DESIGN §8.2 の「色直書きは最小限」からは逸脱） |
| 日本語文言 | `strings.rs` に集約 | `main.rs` 内にリテラルで記述 | P1 で `strings.rs` へ抽出 |
| アイコン | 拡張子→`IconName` の静的対応表 | `icons.rs` の `FileKind` 経由（同一役割） | 一致 |
| `dirs` crate | 7.0.0 推奨（6 系も許容と明記） | 6.0.0 | 許容（`config_dir()` API 同一） |
| chrono | std で可（P0 は許容） | chrono 0.4（ローカル時刻表示） | 許容 |
| `view` フィールド | 詳細/アイコン | `details` 固定で保存のみ（アイコンビューは P2） | スキーマ先行 |
| 空き領域の右クリックメニュー | 明記なし | 未対応（仮想リストが領域を占有。新規フォルダはツールバー/行メニューから） | 既知の制限 |
| 入力 E2E | §10.3 の手動操作 | 合成入力ツールが無く **キーボード/右クリック/確認ダイアログの実押下は未検証** | 既知の制限（§実行検証記録） |

## ライセンス

本プロジェクトは **GNU Affero General Public License v3.0 or later**
（AGPL-3.0-or-later）で配布されます。全文は [LICENSE](LICENSE)。

利用している第三者コンポーネント（GPUI / GPUI Component / Lucide 等）の
ライセンスとクレジットは [NOTICE.md](NOTICE.md) を参照してください。
