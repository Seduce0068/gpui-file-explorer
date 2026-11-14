# GPUI ファイルエクスプローラー 設計書（試作版 v0.1）

- 版: **0.1**（2026-11-11、初版。以後の変更は本ファイル末尾の変更履歴に追記）
- 対象コード: [`app/`](app/)（Rust 試作 / crate 名 `gpui-file-explorer`）
- 前提資料: [`RESEARCH.md`](RESEARCH.md) / [`notes/api-notes.md`](notes/api-notes.md) / [`notes/ecosystem-notes.md`](notes/ecosystem-notes.md) / [`notes/fs-cross-platform-notes.md`](notes/fs-cross-platform-notes.md) / [`notes/poc-report.md`](notes/poc-report.md) / [`poc/src/main.rs`](poc/src/main.rs)
- 想定環境: Kubuntu 2026.04 / KDE Plasma 6 / Wayland / NVIDIA GTX 1660 SUPER / rustc 1.99.1（POC 実測、[`notes/poc-report.md`](notes/poc-report.md)）
- ライセンス: **AGPL-3.0-or-later**（依頼者決定、[`RESEARCH.md`](RESEARCH.md) §11.1.1）
- 記法: **✅** = 一次情報（crate ソース・実機）で確認済み、**⚠** = 未確認。crate ソースはローカル `~/.cargo/registry/src/index.crates.io-*/` 配下を指す。

> 本設計書は「fe-dev が実装する P0 試作の契約」である。§4（モジュール構成）・§5（型）・§10.4（受け入れ条件）は実装者が従う。逸脱する場合は [`app/README.md`](app/README.md) の「設計からの差分」に理由付きで記録する。

---

## 1. 目的・非目的・対象・ユーザー・成功基準

### 1.1 目的

1. GPUI（gpui-kit 0.9.1 スタック）で **Linux/KDE Wayland 実機で実用に耐えるファイルエクスプローラー**の設計を確立し、動く試作（P0）で検証する。
2. 調査済みの知見（[`RESEARCH.md`](RESEARCH.md) の 12 結論、fs 落とし穴 22 項目）を**設計チェックリストとして消化**し、後戻りの大きい決定（スレッドモデル・ゴミ箱・監視・パス表現）を初期に固定する。
3. Windows/macOS への拡張余地（Phase2）を壊さない構造にする。UI はクロスプラットフォーム、OS 統合は差し替え可能な `fs/` 層に閉じ込める。

### 1.2 非目的（この試作でやらない）

- Windows Explorer / Dolphin の完全互換（権限編集・拡張メニュー・アーカイブ・クラウド・ネットワークブラウズ）。
- **完全削除（permanent delete）**。P0/MVP では UI に出さない（§6）。
- 検索インデックス、内容検索、サムネイル生成、PDF/動画プレビュー、タブ、Dock、プラグイン機構。
- macOS App Store 配布（AGPL と衝突、[`RESEARCH.md`](RESEARCH.md) §11.1.1）。
- Zed の GPL コード（`crates/ui` 等）の取り込み。参照はするがコピーしない（§9.5）。

### 1.3 対象 OS（優先順位）

| OS | 環境 | P0 試作 | MVP | Phase2 | 備考 |
|---|---|---|---|---|---|
| Linux | Kubuntu 2026.04 / KDE Plasma 6 / Wayland（NVIDIA） | ✅ 実機で受入 | ✅ | ✅ | **本命**。X11 は feature で同時ビルド（⚠ 実機未検証） |
| Windows | Windows 11 / MSVC | △ ビルド設計のみ | △ `cargo check` | ✅ 実機 | 長パス・大小文字・Recycle Bin |
| macOS | macOS 15+ / Metal | × | × | ✅ 実機 | NFD・サンドボックス方針は Phase2 で決定（⚠） |

- P0 の受入は Linux/Wayland のみ（[`RESEARCH.md`](RESEARCH.md) §7.1、[`notes/poc-report.md`](notes/poc-report.md) §0）。
- Windows/macOS は Phase2 まで「コンパイル可能性を壊さない」ことが目標（OS 固有処理は `#[cfg]` で分離、§7）。

### 1.4 想定ユーザー

- 第一: 依頼者本人（KDE 常用・キーボード操作・AGPL 許容・日本語 UI）。日常のフォルダ巡回とファイル整理が主用途。
- 第二（MVP 以降）: Linux デスクトップ中級者。**「軽快・日本語ファイル名が壊れない・ゴミ箱が確実」**を価値とする。
- 非ターゲット: 大量のクラウドストレージ運用、Windows ヘビーユーザー（当面）。

### 1.5 成功基準（success criteria）

| # | 基準 | 検証方法 |
|---|---|---|
| SC-1 | 起動して 3 秒以内（debug）/ 1.5 秒以内（release）に一覧が操作可能 | 目視 + スクショ |
| SC-2 | 1,000 件のディレクトリで入力→描画のフリーズが体感されない（1 操作 100 ms 超のブロックなし） | 操作ログ + 体感 |
| SC-3 | 日本語・非 UTF-8（表示は lossy）・シンボリックリンクを含むディレクトリで落ちない | 手動フィクスチャ |
| SC-4 | 外部で `touch`/`rm` したファイルが 1 秒以内に一覧へ反映される | 監視実証（§10.3） |
| SC-5 | ゴミ箱へ移動が確認ダイアログ経由で行え、キャンセルで何も起きない。移動後は一覧から即座に消える | §10.4 AC-10 |
| SC-6 | 再起動後に最後のパス・ソート・隠しファイル設定が復元される | §10.4 AC-11 |
| SC-7 | 受入条件 AC-1〜AC-13（§10.4）をすべて満たす | 受入チェックリスト |

---

## 2. 機能マトリクス

凡例: **P0** = 試作（今回実装）/ **P1** = MVP（2〜4 週）/ **P2** = Phase2（3〜6 週）/ **P3** = Phase3。◎=中核、○=あると良い、△=限定、×=やらない。
根拠は [`RESEARCH.md`](RESEARCH.md) / notes の該当節。

| 機能 | P0 | P1 | P2 | P3 | 根拠・判断 |
|---|---|---|---|---|---|
| 詳細リスト（仮想化） | ◎ | | | | `uniform_list` で十分（api-notes §2.1 / RESEARCH §4.1）。固定行高 28px |
| アイコンビュー（グリッド） | × | △ | ◎ | | 行= N セル方式（api-notes §2.3）。P0 は詳細のみ |
| 自然順ソート（dirs first） | ◎ | | | | `alphanumeric-sort` 1.5（fs-notes §1.3） |
| 列ヘッダソート（サイズ/更新日時） | × | ◎ | | | P0 は名前順固定。ヘッダクリックは P1 |
| 隠しファイル切替（Ctrl+H） | ◎ | | | | Linux は `.` 前置。Windows 属性は P2（fs-notes §5.4） |
| パンくず + 戻る/進む/上へ | ◎ | | | | 専用部品なし＝自作（RESEARCH §4.4）。履歴は自前 |
| サイドバー（場所） | ◎ | | | | `dirs` でホーム等を解決（fs-notes §5.4） |
| パス入力（Ctrl+L） | × | ◎ | | | `InputState` 必須（IME）。P0 はパンくずのみ |
| 非同期読込 + 世代管理 | ◎ | | | | RESEARCH §3.4 のとおり。**UI スレッドで I/O しない** |
| ファイル監視（自動更新） | ◎ | | | | notify 8.2 + debouncer-full 0.7、非再帰（fs-notes §2） |
| 10 秒リコンサイル | ◎ | | | | イベント溢れ・ネットワーク FS 対策（RESEARCH §3.5） |
| 設定永続化 | ◎ | | | | serde_json + `dirs`。P0 は 5 項目（§8） |
| ファイル種別アイコン（Lucide） | ◎ | | | | gpui-kit の静的アイコン + 拡張子マップ（RESEARCH §4.6） |
| コンテキストメニュー | ◎ | | | | gpui-component `ContextMenuExt`/`PopupMenu`（⚠ 実行時未検証、§3.2） |
| 開く（既定アプリ） | ◎ | | | | `cx.open_with_system`（RESEARCH §4.7, api-notes §4.5） |
| リビール（ファイラで表示） | ◎ | | | | `cx.reveal_path`（同上）。KDE は FileManager1 ⚠ |
| パスをコピー | ◎ | | | | `ClipboardItem::new_string`（api-notes §4.4） |
| ゴミ箱へ移動 | ◎ | | | | `trash` 5.3（fs-notes §5.2）。確認 + 楽観更新 |
| 新規フォルダ | ◎ | | | | `create_dir`。名前は固定名から連番（P0 はダイアログ無し） |
| 名前変更 | × | ◎ | | | P0 は実装しない（IME/Input のリスク、RESEARCH §4.5） |
| コピー/移動/貼り付け | × | ◎ | | | 進捗・キャンセル・衝突解決を最初から（fs-notes §5.1） |
| 複数選択（Ctrl/Shift） | × | ◎ | | | 型は P0 から複数対応（§5.3） |
| ゴミ箱ビュー（復元/完全削除） | × | × | ◎ | | `trash::list/restore_all/purge_all`（fs-notes §5.2） |
| OS ネイティブアイコン | × | × | ◎ | | Linux: freedesktop-icons（fs-notes §3） |
| 画像サムネイル | × | × | ◎ | | `img(PathBuf)` で可能（api-notes §7.2）。非同期+LRU |
| 再帰検索（名前） | × | × | ◎ | | `ignore` 並列 walker（fs-notes §8） |
| ボリューム/空き容量 | × | × | ◎ | | `sysinfo` Disks（fs-notes §4） |
| タブ / 複数ウィンドウ | × | × | △ | ◎ | まず 1 ウィンドウ 1 状態（ADR-002） |
| OS からの D&D（取り込み） | × | × | △ | ◎ | `on_drop::<ExternalPaths>`（api-notes §4.2） |
| アプリ外への D&D | × | × | × | △ | 実装コスト大（fs-notes §7.4） |
| ファイルリスト clipboard | × | × | ◎ | | arboard 3.7（外部アプリ連携） |
| アプリメニューバー | × | × | △ | | `set_menus`。ツールバーで代替（api-notes §8.2） |
| 完全削除 | × | × | △ | | **P0/MVP 禁止**。P2 で明示確認付きの限定解禁（§6.7） |
| テーマ切替（ライト/ダーク） | × | × | ◎ | | gpui-component Theme。P0 は既定ダーク固定 |
| i18n リソース化 | × | × | ◎ | | P0 は `strings.rs` 集約（§8.5） |
| ロケール照合（五十音順） | × | × | × | ◎ | ICU4X（fs-notes §1.4） |

**P0 で明確に切るもの**: 名前変更、コピー/移動、複数選択、検索、サムネイル、OS アイコン、タブ、パス入力、完全削除。理由は「P0 の受入（§10.4）を 1〜2 日で通すため」+「IME/入力と衝突解決は独立した設計課題で、中途半端に入れると危険」（RESEARCH §12 Phase1）。

---

## 3. UX

### 3.1 画面レイアウト（P0）

```
┌───────────────────────────────────────────────────────────────────────────────┐
│ GPUI File Explorer                                              [–] [□] [×]    │ ← KDE ネイティブ枠
├───────────────────────────────────────────────────────────────────────────────┤
│ [←] [→] [↑] [⟳]  │  隠しファイル: オフ  │  1,180 / 1,234 件      [設定?]      │ ← toolbar (36px)
├───────────────────────────────────────────────────────────────────────────────┤
│ /  home  koppe  document  deepseek-harness  default-workspace  work  app      │ ← breadcrumb (28px)
├───────────────┬───────────────────────────────────────────────────────────────┤
│ 場所          │ 名前                              │ サイズ  │ 更新日時           │ ← 列ヘッダ (24px)
│ ───────────── │ ───────────────────────────────────────────────────────────── │
│ [D] ホーム    │ [D] .cargo                        │   —     │ 2026-11-11 14:02   │
│ [D] デスクトップ│ [D] src                          │   —     │ 2026-11-11 13:40   │
│ [D] ドキュメント│ [F] Cargo.toml                   │ 1.2 KB  │ 2026-11-11 12:01   │
│ [D] ダウンロード│ [F] README.md                    │ 3.4 KB  │ 2026-11-10 21:15   │
│ [D] ピクチャ  │ [F] main.rs                       │ 9.1 KB  │ 2026-11-11 14:31   │
│ [D] ミュージック│                                     ▲ スクロールバー（右端）  │
│ [D] ビデオ    │                                     │                          │
│ [D] ルート    │                                     │                          │
├───────────────┴───────────────────────────────────────────────────────────────┤
│ 1,234 件 / 表示 1,180 件  │ 選択: main.rs  │ 自動更新: 有効  │ gpui-kit 0.9.1 │ ← status bar (24px)
└───────────────────────────────────────────────────────────────────────────────┘
   ↑ サイドバー 200px 固定（P0）           ↑ 行 28px・仮想化（可視範囲のみ構築）
```

- 幅 1100×高 700 を初期値（`WindowOptions::window_bounds`、POC の起動実績に準拠）。
- 下部ステータスは **左: 件数 / 中央: 選択・エラー・読込状態 / 右: 監視状態・バージョン**。
- 列ヘッダは P0 ではソート切替なし（名前昇順・dirs first 固定）。P1 でクリックソート。
- 長い名前は `.truncate()`（1 行省略）。ツールチップは P1（api-notes §7.3）。

### 3.2 コンテキストメニュー（右クリック）

```
 右クリック位置（行 or 空白）に anchored 表示。外側クリック / Esc で閉じる。
            ┌──────────────────────────────┐
            │ 開く                    ↵     │  ← 行のみ
            │ ファイルマネージャで表示       │  ← 行のみ（cx.reveal_path）
            │ パスをコピー                 │  ← 行のみ
            ├──────────────────────────────┤
            │ ゴミ箱へ移動              Del │  ← 行のみ・警告色
            ├──────────────────────────────┤
            │ 新規フォルダ                  │  ← 空白のみ
            │ 更新                       F5 │  ← 空白のみ
            └──────────────────────────────┘
```

- 実装: **第一候補** `gpui_kit::component::menu::ContextMenuExt::context_menu()`（行/空白要素にアタッチ、`PopupMenuItem::on_click` は `PathBuf` をキャプチャした closure でよい）。**フォールバック** `deferred` + `anchored().position()` + ルートの `on_mouse_down_out`（core API は api-notes §5 で ✅、gpui-component の ContextMenu/PopupMenu はソース存在のみ確認 ✅、Wayland 挙動は ⚠）。
- メニュー項目は `Action` を経由せず closure で直接呼ぶ（フォーカス移動に依存せず、対象パスを確実に渡せるため）。P2 で PopupMenu + Action 統合を検討。
- 空領域右クリックは「新規フォルダ / 更新」のみ。

### 3.3 主要フロー

**(A) 起動 → 一覧**

```
main()
 └ gpui_kit::application().run(cx)
    ├ gpui_kit::init(cx)                       // component/base 初期化。1 回だけ
    ├ Settings::load()  → last_dir が存在しディレクトリなら cwd、無ければ $HOME
    ├ ExplorerApp::new(cx)                     // load: Loading で開始
    ├ gpui_kit::open_window(WindowOptions{ app_id:"gpui-file-explorer", 1100x700 })
    │    └ Root(Base) + ExplorerApp
    └ navigate(cwd) → background read_dir → Ready → watch 開始 → pump 開始
 表示: 一覧領域「読み込み中…」→ 結果（0 件なら空状態）→ ステータス件数更新
```

**(B) 移動（サイドバー / パンくず / Enter / ダブルクリック / Alt+←→↑）**

```
操作 → open_location(target, mode)
  ├ target == cwd なら Refresh のみ
  ├ mode: Push（通常）/ Back / Forward / Up
  ├ history 更新（Push は進む履歴を捨てる）
  ├ cwd = target; load_gen += 1; load_task.take()（前ロードをキャンセル）
  ├ entries クリア・選択クリア・スクロール先頭へ
  ├ watch を付け替え（旧 debouncer を drop）
  ├ background_spawn(read_entries) → UI に戻る
  │    └ gen 一致時のみ: entries 反映 → sort/filter → selection.reconcile → Ready
  └ 失敗時: Error 状態（一覧領域にメッセージ + 再試行 F5）。パンくず・サイドバーは生きている
```

**(C) 選択 → 開く**

```
クリック（単一）/ ↑↓（フォーカス移動）
  └ selection.focused = path（P1 で Ctrl/Shift 複数選択）
ダブルクリック（click_count == 2）または Enter
  ├ EntryKind::Dir | SymlinkToDir → open_location(path, Push)
  ├ File / SymlinkToFile → cx.open_with_system(path)  ※失敗は通知トースト
  └ Other → 何もしない（通知「開けません」）
```

**(D) ゴミ箱へ移動（P0）**

```
右クリック → 「ゴミ箱へ移動」→ 対象 = 選択項目
  ├ suppress_until = now + 1.5s（自己操作の監視イベント抑制）
  ├ window.prompt(PromptLevel::Warning, "N 件をゴミ箱へ移動しますか？",
  │               detail=先頭名..., [PromptButton::ok("ゴミ箱へ移動"), cancel("キャンセル")])
  ├ oneshot を cx.spawn で await（UI はブロックしない）
  ├ Cancel → 何もしない（メニュー閉、選択維持）
  └ OK → background_spawn(trash::delete_all(paths))
       ├ 成功: 楽観的に一覧から除去 → selection.reconcile → notify
       │        通知トースト「ゴミ箱へ移動しました」
       └ 失敗: 一覧は変更しない。通知トースト（理由）+ 監視/F5 で再同期
```

### 3.4 キーボードマップ（P0）

`actions!` + `KeyBinding` + `.key_context("Explorer")`（api-notes §3.2）。矢印/Home/End は `on_key_down` で処理（Action にしない：リスト移動は可視範囲と密結合）。

| キー | 動作 | 実装 |
|---|---|---|
| `Alt+Left` | 戻る | `Action GoBack` |
| `Alt+Right` | 進む | `Action GoForward` |
| `Alt+Up` / `Backspace` | 親ディレクトリへ | `Action GoUp` |
| `Enter` | 選択項目を開く / ディレクトリへ移動 | `Action OpenSelected` |
| `Up` / `Down` | フォーカス移動（`scroll_to_item(Nearest)`） | `on_key_down` |
| `Home` / `End` | 先頭 / 末尾 | `on_key_down` |
| `PageUp` / `PageDown` | 可視行数分移動 | `on_key_down` |
| `F5` / `Ctrl+R` | 再読込 | `Action Refresh` |
| `Ctrl+H` | 隠しファイル表示切替 | `Action ToggleHidden` |
| `Delete` | ゴミ箱へ移動（確認あり） | `Action TrashSelected` |
| `Ctrl+Shift+N` | 新規フォルダ | `Action NewFolder` |
| `Ctrl+L` | パス入力にフォーカス（P1） | `Action FocusPathBar` |
| `F2` | 名前変更（P1） | `Action RenameSelected` |
| `Ctrl+C` / `Ctrl+X` / `Ctrl+V` | コピー/切り取り/貼り付け（P1） | `Action Copy/Cut/Paste` |
| `Ctrl+A` | 全選択（P1） | `Action SelectAll` |
| `Escape` | メニュー/ダイアログを閉じる → 選択維持 | 各 UI |
| `Shift+Up/Down` | 範囲選択（P1） | `on_key_down` |
| `Ctrl+Q` | 終了 | `Action Quit`（KDE 慣習。`cx.quit()`） |

- **フォーカス規約**: ルート要素が `track_focus`。一覧のクリック/空クリックでフォーカスを奪い、ショートカットを常に有効化（RESEARCH §3.3 の「フォーカス喪失でショートカット死」対策）。ダイアログ表示中はダイアログがフォーカスを持ち、閉じたらリストへ戻す。
- **IME**: P0 はテキスト入力 UI を持たない（新規フォルダ名は `新規フォルダ`・`新規フォルダ (2)` の自動採番）。P1 のリネーム/パス入力で gpui-component `Input` を使う（自前 IME 実装は禁止、RESEARCH §4.5）。

### 3.5 状態表示（loading / error / empty）

| 状態 | 一覧領域 | ステータスバー | 操作可否 |
|---|---|---|---|
| 読込中 | 「読み込み中…」+ 細い不定プログレス（テキストで可） | 左「読み込み中」 | ツールバー/サイドバー可（新規ナビゲーションで前ロードは破棄） |
| 空 | 中央に「このフォルダは空です」 | 「0 件」 | 可（親へ移動・更新） |
| 権限なし | 「アクセスできません: <path>」+「親フォルダへ」ボタン | エラー種別 | 可 |
| 存在しない | 「このフォルダは削除されました」+ 自動で親へ（P0 はメッセージ + 親移動ボタン） | エラー | 可 |
| その他 I/O エラー | メッセージ + 再試行（F5） | エラー | 可 |
| 監視無効 | 影響なし | 右「自動更新: 無効（F5 で更新）」 | 可 |
| 削除失敗 | 一覧は維持 | 右にトースト（`push_notification`） | 可 |

- `read_dir` 途中の個別エントリ失敗（`NotFound` 等）は**行を落とさず一覧全体を失敗させない**（fs-notes §10-2）。`metadata` 失敗はサイズ/日時を「—」表示。
- エラー文言は日本語（§8.5）。パスは `display()`（lossy）で表示。

### 3.6 マウス・その他

- 単クリック = 選択（P1 で `Ctrl`=トグル、`Shift`=範囲）、ダブルクリック（`click_count == 2`）= 開く。
- 中クリック = 何もしない（P2 で新タブ）。
- 右クリック = コンテキストメニュー。**行の右クリックは対象行を選択してからメニュー**（操作性のため）。
- サイドバー項目クリック = 移動。現在地はハイライト。
- ホバーで行背景を薄く変える（`.hover()`）。選択行はアクセント色。

---

## 4. アーキテクチャ

### 4.1 クレート構成（単一バイナリ crate）

```
app/
├── Cargo.toml                  # gpui-kit = =0.9.1, notify 8.2, notify-debouncer-full 0.7, trash 5.3,
│                               # serde/serde_json, dirs, alphanumeric-sort 1.5（追加は最小限）
├── LICENSE                     # AGPL-3.0 全文（§9）
├── NOTICE.md                   # 第三者ライセンス表示（§9）
├── README.md                   # ビルド/実行/キー/既知の制限/AGPL/設計からの差分
└── src/
    ├── main.rs                 # bootstrap のみ（application/init/open_window/quit 配線）
    ├── app.rs                  # ExplorerApp（唯一の root Entity）、Actions/KeyBinding、ナビゲーション、
    │                           # コンテキストメニュー状態、ダイアログ起動、監視 pump の所有
    ├── model/
    │   ├── mod.rs
    │   ├── entry.rs            # Entry / EntryKind / display_name / is_hidden 判定入力
    │   ├── sort.rs             # SortKey / SortDir / SortSpec / sort_entries
    │   ├── listing.rs          # Listing（entries + visible インデックス）と rebuild/upsert
    │   ├── selection.rs        # Selection（path キー）と移動/範囲/整合
    │   ├── history.rs          # History（back/forward）
    │   ├── load.rs             # LoadState / FsError / FsErrorKind
    │   └── format.rs           # human_size / human_age（純関数・テスト対象）
    ├── fs/
    │   ├── mod.rs
    │   ├── listing.rs          # read_entries(&Path) -> Result<Vec<Entry>, FsError>（background 専用）
    │   ├── watch.rs            # FsWatch（Debouncer 所有）/ FsEvent / WatchState / drain
    │   ├── ops.rs              # trash_paths / create_folder（P1: rename/copy/move）
    │   ├── open.rs             # open_path / reveal_path（GPUI core API 呼び出し）
    │   └── paths.rs            # is_hidden / sort_key / same_path（case-fold は P2）
    ├── settings.rs             # Settings / load / save（原子的書込）/ 既定値
    ├── icons.rs                # 拡張子 -> IconName。UI 非依存（model に依存）
    ├── strings.rs              # 日本語 UI 文言の集約（i18n 準備、§8.5）
    └── ui/
        ├── mod.rs
        ├── shell.rs            # 全体レイアウト（toolbar/breadcrumb/sidebar/list/status の合成）
        ├── toolbar.rs          # 戻る/進む/上へ/更新/隠しトグル
        ├── breadcrumb.rs       # パスセグメント（末尾省略・クリック移動）
        ├── sidebar.rs          # 場所一覧
        ├── list.rs             # 列ヘッダ + uniform_list + 行描画 + キー移動
        ├── status_bar.rs       # gpui-component StatusBar
        ├── context_menu.rs     # ContextMenuExt アタッチ（フォールバック実装もここ）
        └── dialogs.rs          # 削除確認ラッパ・エラーダイアログ・名前変更（P1）
```

- **POC の単一 `main.rs`（477 行）をそのまま拡張しない**。ロジックのテストと並行安全レビューのため、最初から上記分割で書く（`poc/src/main.rs` は API の実例として参照）。
- `ui/` は **model/fs に依存するが、その逆は禁止**。`icons.rs` は `model::Entry` を受け取り `IconName` を返す（model 側はアイコン型を知らない）。
- 設定・アイコン・文字列のモジュールは UI から独立して単体テスト可能にする。

#### P0 実装の現状（as-built）との差分（2026-11-11 15:20 時点）

fe-dev が並行実装中の `app/src/` は、時間優先で次の**フラット構成**を採っている。設計の目標構成（上図）との差分を記録する。**受入条件（§10.4）には影響しない**。P1 冒頭で目標構成へ寄せる（機能追加と同時に分割すると差分レビューが困難になるため、P0 は現状維持）。

| 実装（現状） | 設計（§4.1/§5） | 評価・扱い |
|---|---|---|
| `main.rs` に `Explorer`(root Entity) + UI 描画を集約 | `app.rs` + `ui/*` に分割 | P0 は許容。1200 行超のため P1 で `ui/{toolbar,breadcrumb,sidebar,list,status_bar,context_menu}` へ抽出 |
| `model.rs` に Entry/ソート/列挙/整形を集約（GPUI 非依存） | `model/{entry,sort,listing,format}.rs` + `fs/listing.rs` | GPUI 非依存は設計どおり（◎）。分割のみ P1 |
| `Entry { name: String（lossy）, path: PathBuf, is_dir, is_symlink, size, modified }` | `Entry { name: OsString, sort_key, kind, size: Option<u64>, is_hidden }` | **I/O は `path` 経由なので非 UTF-8 名でも壊れない**。表示・ソートは lossy。P1 で `OsString` + 事前計算 `sort_key` に移行（§5.1） |
| 選択は `selected: usize`（単一） | `Selection`（PathBuf キー、複数） | P0 要件は単一選択なので可。P1 で §5.3 の型へ置換 |
| `loading: bool` + `error: Option<String>` | `LoadState` / `FsError` | P0 は簡略表現で可。エラー文言の日本語化は実装済み。P1 で型化 |
| `history.rs`（`visit/go_back/go_forward`） | `model/history.rs`（`apply(target, mode)`） | 意味は同等。統合は P1 |
| `watcher.rs`（`DirWatcher`）が `async-channel` 経由で前景ループへ通知 | `fs/watch.rs` + mpsc/timer pump（ADR-006） | **async-channel 採用は許容**（GPUI の依存に既にあり、`Cargo.toml` に明示追加済み）。前景ループ + 10s tick で設計意図（UI スレッドで I/O しない/バッチ化）は満たす |
| `chrono` を日時表示に使用 | §8.4 は std で可とした | P0 は許容（表示専用）。依存を増やしたくない場合は P1 で `std::time` に戻す |
| `icons.rs`（FileKind + IconName） | `icons.rs`（同じ役割） | 一致 |
| `settings.rs`（atomic save） | `settings.rs` | 一致 |
| （P1 先行）`ops.rs` に copy/move/rename エンジンを追加（ユニットテスト 11 件）＋ `Ctrl+C/X/V`・右クリックメニュー・F2 リネームダイアログへ接続済み（貼り付けは常に Rename、Esc でキャンセル、リネーム失敗時は原名維持） | `fs/ops.rs`（§6 の安全則） | 上書き禁止・symlink 非追跡・`.partial` 経由の原子的コピー・EXDEV フォールバック・キャンセル・64 KiB 単位のバイト進捗通知を実装。複数選択（`model::Selection`、パスキー）とバッチ操作（コピー/切り取り/ゴミ箱、件数付き確認）、ゴミ箱ビュー（`trash_view.rs`、一覧・復元のみで完全削除なし）、視覚的進捗バー（`ops::total_bytes` の事前計算 + ステータスバー表示）、アイコングリッド表示（`uniform_list` の行チャンク方式、`settings.view` に永続化）、再帰検索（`search.rs`、symlink非追跡・上限付き・キャンセル対応）にも対応。最終的に `fs/ops.rs` へ移動 |

- **差分の記録義務**: 上表に無い逸脱が生じた場合は [`app/README.md`](app/README.md) の「設計からの差分」に追記し、本設計書の改版（v0.2）で反映する。

### 4.2 Entity グラフ

```
gpui_kit::open_window が Root(Base) を自動生成（✅ gpui-kit 0.9.1 src/lib.rs）
Window
└── Root (gpui_base::Root + component の WindowState プラグイン: ダイアログ/通知/ツールチップ)
    └── ExplorerApp (Entity<ExplorerApp>: Render)  ← 唯一のアプリ状態
        ├── model::Listing / Selection / History / LoadState / Settings   （通常の struct）
        ├── fs::FsWatch（Debouncer を所有。UI 型を跨がない）              （通常の struct）
        ├── Task<()> ×3: load_task / watch_pump /（P1 copy/move）
        ├── FocusHandle / UniformListScrollHandle
        ├── Option<ContextMenuState>（右クリック位置と対象）
        └── Option<Entity<InputState>>（P1 リネーム/パス入力のみ Entity）
```

- **決定（ADR-002）: P0 は単一 Entity**。`DirListing`/`Selection` を別 Entity にしない。理由: (a) 監視・ロード・選択が同一状態を共有し、Entity 間購読の順序バグを避けられる、(b) テストは純関数で行える、(c) タブ導入時に初めて分割すればよい。**将来の分割点**は `Listing` を `Entity<DirListing>` にするだけ（UI は `app.listing` 参照を差し替え）。
- **決定: 純粋 View は Entity にしない**。`ui/*` は `&mut ExplorerApp` + `Options` を受ける関数/`RenderOnce` コンポーネントとし、`cx.listener` で `ExplorerApp` にイベントを返す。状態を持つのは `ExplorerApp` と `InputState`（P1）のみ。
- 通知は `cx.notify()` のみ（cross-entity の `EventEmitter` は P0 では使わない）。テスト容易性のため `EventEmitter` は P2 で再検討。

### 4.3 データフロー（3 レーン）

```
【入力】クリック / キー / メニュー / 監視ポンプ
   │
   ▼  (UI スレッド・単一)
ExplorerApp のメソッド（open_location / select / open_selected / trash_selected / apply_watch_events）
   │
   ├─(1) 同期: 状態の楽観更新（history/cwd/selection/context_menu）→ cx.notify()
   │
   ├─(2) 重い I/O: cx.background_spawn(async move { fs::listing::read_entries(&dir) })
   │        │  ※ Send のみ。App/Entity/Window は触れない
   │        ▼  (background executor スレッド)
   │      Result<Vec<Entry>, FsError>
   │        │
   │        ▼  cx.spawn(async move |this, cx| { result = task.await; this.update(...) })
   │      generation 一致時のみ: Listing 再構築 → Selection::reconcile → LoadState 更新 → cx.notify()
   │
   └─(3) 監視イベント: pump が 120ms ごとに mpsc を drain → FsEvent 正規化
             └ 抑制窓内なら破棄 / それ以外は差分適用（P0 は対象ディレクトリのイベントで再読込）
                 → sort/filter 再構築 → notify
   ▼
render(): ui::shell(app, cx) → uniform_list は可視範囲のみ構築（フレームごとの重い処理なし）
```

- **楽観更新 → 背景で確定 → 監視で補正**の 3 段構え（RESEARCH §5.1）。削除/新規フォルダは監視イベントを待たずに反映し、抑制窓で二重適用を防ぐ。
- 監視イベントの反映は **1 フレーム 1 回**にバッチ化（毎イベント `notify` しない）。
- 状態が変わらない場合は `notify()` しない（過剰再描画の禁止、RESEARCH §8.2）。

### 4.4 スレッドモデル（厳守）

| スレッド | 担当 | 禁止 |
|---|---|---|
| **UI (foreground, 単一)** | Entity 状態、レイアウト、メニュー/ダイアログ、10k 件程度のソート（数 ms）、`cx.notify()`、小さな設定 JSON（≤1KB）の同期保存 | `read_dir` / `metadata` / `trash` / `open` の待機 / 画像デコード / `sleep` / `recv` ブロック |
| **background executor（マルチ）** | `read_dir`+`file_type`、`trash`、walk、将来のコピー/検索/アイコン取得 | `App`/`Entity`/`Window` へのアクセス（`Send` 制約でコンパイル不可） |
| **notify debouncer スレッド** | inotify 監視、デバウンス、チャネル送信（mpsc / async-channel） | UI 型の保持 |
| **専用 I/O スレッド** | P0 では作らない（`background_spawn` で足りる） | — |

- 橋渡しは `cx.background_spawn(...).await` → `this.update(cx, |this, cx| ...)`（api-notes §6.1）。`WeakEntity` 経由なので Entity drop で future はキャンセルされる。
- ソートは UI スレッドで行う（`sort_by_cached_key`、10k 件で数 ms）。100k 件規模は P2 で background 化（§10.5）。

### 4.5 ロードの世代とキャンセル

```rust
// app.rs（概念コード。型は §5 と一致させる）
fn open_location(&mut self, target: PathBuf, mode: NavMode, cx: &mut Context<Self>) {
    if target == self.cwd && mode == NavMode::Push { self.reload(cx); return; }
    let Some(target) = self.history.apply(target, mode) else { return; }; // 戻る/進む不可なら no-op
    self.load_gen = self.load_gen.wrapping_add(1); // ① 世代を進める
    let gen = self.load_gen;
    self.load_task.take();                         // ② 前の Task を drop = キャンセル
    self.load = LoadState::Loading { generation: gen };
    self.listing = Listing::default();             // 表示をクリア（旧一覧の誤表示を防ぐ）
    self.selection.clear();
    self.cwd = target.clone();
    self.rearm_watch(cx);                          // ③ watch を新しい cwd へ付け替え
    let task = cx.background_spawn(async move { fs::listing::read_entries(&target) });
    self.load_task = Some(cx.spawn(async move |this, cx| {
        let result = task.await;
        let _ = this.update(cx, |this, cx| {
            if this.load_gen != gen { return; }    // ④ 古い結果は捨てる（上書きしない）
            match result {
                Ok(entries) => this.apply_listing(entries, gen, cx),
                Err(err) => { this.load = LoadState::Error { generation: gen, error: err }; cx.notify(); }
            }
        });
    }));
    cx.notify();
}
```

- ①〜④は **必ずセット**（fs-notes 落とし穴 #8: ディレクトリ切替の競合）。`rearm_watch` も世代と同様に「新しい watch が成功するまで旧 watch を残さない」。
- `reload`（F5/監視リコンサイル/起動）は「cwd を変えない open_location」として同じ経路を通す（履歴に積まない）。
- 読込中の再ナビゲーションは前 Task を drop して即座に破棄（キャンセル）。`read_dir` 自体は途中まで走ってもよい（結果を捨てる）。

### 4.6 監視: watch → debounce → 差分更新 → reconcile の状態機械

```
                 rearm(cwd ok)                        DebounceEventResult
   [Idle] ──────────────────────▶ [Armed] ────────────────────────▶ [Dirty]
      ▲                              │  ▲                               │
      │ navigate/quit                │  │ 適用完了                       │ pump が 120ms 以内に drain
      │ (handle drop)                │  └──────────── [Applying] ◀────────┘
      │                              │                   │  ※ 抑制窓内のイベントは破棄
      │                              │ 10s tick          │
      │                              ▼                   ▼
      │                          [Reconcile] ──────▶ 再読込（gen ガード）
      │                              │
      │  watch エラー/対象消滅        ▼
      └────────────────────── [Degraded] ── 再試行成功 ──▶ [Armed]
                                 （ステータスに「自動更新: 無効」表示）
```

- **watch 範囲**: 表示中ディレクトリ（非再帰）**＋親ディレクトリ（非再帰）**。親は「現在地が rename/削除された」検知用（イベントは cwd に直接影響するものだけ適用、親由来は再読込トリガ）。
- **debounce**: 300〜500 ms（`new_debouncer(Duration::from_millis(400), None, tx)`）。POC の API 実績に合わせ、`std::sync::mpsc::Sender<DebounceEventResult>` を handler に使う（`notify-debouncer-full 0.7.0` が `Sender` 実装を提供 ✅）。
- **イベント正規化**（`fs/watch.rs`）:

| notify の種別 | `FsEvent` | P0 の処理 |
|---|---|---|
| `Create` | `Created(paths)` | 対象が cwd 直下なら差分（P0 は再読込でも可） |
| `Remove` | `Removed(paths)` | 対象が cwd 直下なら除去 + 選択 reconcile |
| `Modify(Name)` | `Renamed { from, to }` | from/to を確定できなければ再読込 |
| `Modify(Data/Metadata)` | `Modified(paths)` | サイズ/日時のみ更新（P0 は無視してよい） |
| `Any` / 溢れ / watch エラー | `Rescan` | 再読込（gen ガード） |
| `Access` | 無視 | — |

- **自操作抑制**: ゴミ箱・新規フォルダ実行時に `suppress_until = now + 1.5s`（+ 対象パス集合）。抑制窓内のイベントは破棄するが、**10 秒リコンサイルは常に有効**（抑制で取りこぼしても回復）。
- **10 秒リコンサイル**: pump の tick カウントで `reload` を発火（ネットワーク FS/イベント溢れ/Windows の RDCW 溢れ対策、RESEARCH §3.5）。抑制窓と独立。
- **watch 失敗時**: `WatchState::Degraded { reason }`。通知トースト + ステータス右「自動更新: 無効」+ F5 を案内。ネットワーク FS では watch を張らず `Degraded` で開始する判定を P2 で追加（`statfs` の f_type、⚠ 実装未定）。
- pump は `cx.spawn` の前景ループで実装する。監視スレッドからの通知は **チャネル**（`std::sync::mpsc` か `async-channel`。現行試作は `async-channel`、§4.1 as-built）で受け、前景ループが 120 ms タイマーごとに drain してバッチ適用する（ADR-006）。UI スレッドで `recv` ブロックしない。10 秒リコンサイルは同じループの tick カウントで発火する。

---

## 5. データモデルと型（実装契約）

以下は `model/` と `fs/` に置く型の**提案シグネチャ**。fe-dev はこれに従う（細部の追加は可、意味の変更は README に記録）。

### 5.1 Entry / Listing

```rust
// model/entry.rs
use std::ffi::OsString;
use std::path::PathBuf;
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EntryKind { Dir, File, SymlinkToDir, SymlinkToFile, Other }

impl EntryKind {
    pub fn is_dir_like(self) -> bool { matches!(self, Self::Dir | Self::SymlinkToDir) }
    pub fn is_symlink(self) -> bool { matches!(self, Self::SymlinkToDir | Self::SymlinkToFile) }
}

#[derive(Clone, Debug)]
pub struct Entry {
    /// 唯一の真実。OsStr を捨てない（Linux 非 UTF-8 名対応、fs-notes §5.3）
    pub path: PathBuf,
    pub name: OsString,
    /// 表示・ソート用にキャッシュ。`name.to_string_lossy()` を小文字化したもの
    pub sort_key: String,
    pub kind: EntryKind,
    pub size: Option<u64>,              // Dir / 取得失敗は None
    pub modified: Option<SystemTime>,
    pub is_hidden: bool,                // Linux: 名前が '.' 始まり（P0）
}

impl Entry {
    pub fn is_dir(&self) -> bool;                 // kind.is_dir_like()
    pub fn display_name(&self) -> std::borrow::Cow<'_, str>; // to_string_lossy
    pub fn extension_lower(&self) -> Option<String>;
}

// model/listing.rs
#[derive(Default, Clone, Debug)]
pub struct Listing {
    /// cwd 直下の全エントリ（隠し含む・未ソートでもよい）
    pub entries: Vec<Entry>,
    /// entries へのインデックス。フィルタ + ソート適用済みの表示順
    pub visible: Vec<usize>,
}

impl Listing {
    pub fn new(entries: Vec<Entry>, spec: SortSpec, show_hidden: bool) -> Self;
    pub fn rebuild(&mut self, spec: SortSpec, show_hidden: bool);   // ソート・フィルタのみ（I/O なし）
    pub fn len_visible(&self) -> usize;
    pub fn visible_entry(&self, ix: usize) -> Option<&Entry>;
    pub fn upsert(&mut self, entry: Entry);          // P1 の差分用
    pub fn remove_path(&mut self, path: &std::path::Path) -> bool;
    pub fn index_of_visible(&self, path: &std::path::Path) -> Option<usize>;
}
```

- ソート順: **dirs first → 名前自然順（大小文字無視）** → 同値は `path` で安定化。比較は `sort_key` の事前計算 + `alphanumeric_sort::compare_str`（fs-notes §1.3）。`SortSpec.key` が Name 以外のときは `size`/`modified` を主キー、同値は名前で安定化。
- `visible` 方式の理由: 隠しファイル切替・ソート変更で **再読込せずに** 表示だけ作り直せる（SC-2）。P2 の 100k 件ではソートを background 化。

### 5.2 SortSpec

```rust
// model/sort.rs
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortKey { #[default] Name, Size, Modified, Kind }

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SortDir { #[default] Asc, Desc }

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(default)]
pub struct SortSpec { pub key: SortKey, pub dir: SortDir, pub dirs_first: bool }

impl Default for SortSpec {
    fn default() -> Self { Self { key: SortKey::Name, dir: SortDir::Asc, dirs_first: true } }
}

pub fn sort_entries(entries: &mut [Entry], spec: SortSpec);
```

### 5.3 Selection（パスをキーにする）

```rust
// model/selection.rs
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SelectMode { Replace, Toggle, Range }

/// index ではなく PathBuf をキーにする。監視による再読込後もそのまま復元できる。
#[derive(Default, Clone, Debug)]
pub struct Selection {
    pub focused: Option<PathBuf>,
    pub anchor: Option<PathBuf>,
    pub selected: BTreeSet<PathBuf>,
}

impl Selection {
    pub fn clear(&mut self);
    pub fn select_only(&mut self, path: &Path);                       // Replace
    /// entries ではなく Listing を渡す（visible インデックスと entries の対応をここで解決）
    pub fn click(&mut self, listing: &Listing, visible_ix: usize, mode: SelectMode, shift: bool);
    pub fn select_all(&mut self, listing: &Listing);
    pub fn move_focus(&mut self, listing: &Listing, delta: isize, extend: bool)
        -> Option<usize>;                                             // 新しい可視 index
    pub fn focus_first(&mut self, listing: &Listing) -> Option<usize>;
    pub fn focused_entry<'a>(&self, listing: &'a Listing) -> Option<&'a Entry>;
    pub fn selected_paths(&self, listing: &Listing) -> Vec<PathBuf>;
    pub fn paths_for_action(&self, listing: &Listing) -> Vec<PathBuf>; // selected が空なら focused
    pub fn reconcile(&mut self, listing: &Listing, prev_visible: Option<usize>);
}
```

- `reconcile`: 消えたパスを `selected` から除去し、`focused` が消えたら「旧可視 index に近い前後のエントリ」へ移す（削除後のフォーカス維持、RESEARCH §5.3）。P0 は単一選択でもこの型を使う（複数選択の P1 拡張で UI 変更を局所化）。

### 5.4 History

```rust
// model/history.rs
#[derive(Clone, Debug)]
pub struct History { back: Vec<PathBuf>, forward: Vec<PathBuf>, current: PathBuf }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NavMode { Push, Back, Forward, Up, Replace }

impl History {
    pub fn new(current: PathBuf) -> Self;
    pub fn current(&self) -> &Path;
    pub fn can_back(&self) -> bool;
    pub fn can_forward(&self) -> bool;
    /// mode に応じて戻る/進むスタックを更新し、次の current を返す（移動できなければ None）
    pub fn apply(&mut self, target: PathBuf, mode: NavMode) -> Option<PathBuf>;
}
```

### 5.5 LoadState / FsError

```rust
// model/load.rs
use std::path::PathBuf;
use std::time::Instant;

#[derive(Clone, Debug)]
pub enum LoadState {
    Loading { generation: u64 },
    Ready { loaded_at: Instant },
    Error { generation: u64, error: FsError },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FsErrorKind { NotFound, PermissionDenied, NotADirectory, Io, Unknown }

#[derive(Clone, Debug)]
pub struct FsError {
    pub kind: FsErrorKind,
    pub path: PathBuf,
    pub os_message: Option<String>,   // io::Error の表示（ユーザーには日本語メッセージ + 必要なら詳細）
}

impl FsError {
    pub fn from_io(path: PathBuf, err: &std::io::Error) -> Self;  // ErrorKind → FsErrorKind
    pub fn message_ja(&self) -> &'static str;
}
```

### 5.6 FsEvent / WatchState

```rust
// fs/watch.rs
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use notify::RecommendedWatcher;
use notify_debouncer_full::{Debouncer, DebounceEventResult, RecommendedCache};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FsEvent {
    Created(Vec<PathBuf>),
    Removed(Vec<PathBuf>),
    Modified(Vec<PathBuf>),
    Renamed { from: Option<PathBuf>, to: Option<PathBuf> },
    Rescan,
}

#[derive(Clone, Debug)]
pub enum WatchState {
    Off,
    Active { dir: PathBuf },
    Degraded { dir: PathBuf, reason: String },
}

/// Debouncer は drop で監視を停止する（= 付替えは「旧 handle を drop して新規作成」）。
pub struct FsWatch {
    dir: PathBuf,
    debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
    rx: Receiver<DebounceEventResult>,
}

impl FsWatch {
    /// cwd と親（存在する場合）を非再帰で監視。400ms デバウンス。
    pub fn start(dir: &Path) -> Result<Self, notify::Error>;
    pub fn dir(&self) -> &Path;
    /// ノンブロッキング drain。エラーは Rescan として返す。
    pub fn drain(&self) -> Vec<FsEvent>;
}
```

- `drain` は `tx.try_recv()` で `DebounceEventResult` を集め、`notify` の型 → `FsEvent` に正規化する（`notify` の型を UI 層に漏らさない）。
- 型の確認（✅ 0.7.0 ソース）: `DebounceEventResult = Result<Vec<DebouncedEvent>, Vec<notify::Error>>`（`src/lib.rs:157`）。`DebouncedEvent` は `event: notify::Event`（`kind` / `paths`）と `time` を持ち、`Deref` で `Event` として読める（✅ notify-types 2.1）。`RecommendedWatcher` は `notify` crate 側の型なので `use notify::RecommendedWatcher;` で import する。
- `notify` と `notify-debouncer-full` のバージョンは密結合（0.7.0 は `notify ^8.2.0` 依存、✅ crate メタデータ）。Cargo.toml で両方を明示し、`Cargo.lock` をコミットする。
- `Debouncer::stop(self)` は最新 tick までブロックしうるため、drop に任せる（`stop` を UI スレッドで呼ばない）。

### 5.7 Settings

```rust
// settings.rs
use std::path::PathBuf;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "snake_case")]
pub struct Settings {
    pub version: u32,              // 1
    pub last_dir: Option<PathBuf>,
    pub sort: crate::model::SortSpec,
    pub show_hidden: bool,
    pub view: ViewMode,            // Details（P0）/Icons（P2）
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ViewMode { #[default] Details, Icons }

impl Settings {
    pub fn load() -> Self;         // 失敗・破損時は既定値 + 破損ファイルを退避
    pub fn save(&self) -> std::io::Result<()>;   // tmp に書いて rename（原子的）
    pub fn config_path() -> PathBuf;             // dirs::config_dir()/gpui-file-explorer/settings.json
}

impl Default for Settings { /* version: 1, last_dir: None, sort: default, show_hidden: false, view: Details */ }
```

### 5.8 Action 定義（`app.rs`）

```rust
gpui_kit::actions!(explorer, [
    GoBack, GoForward, GoUp, OpenSelected, Refresh, ToggleHidden,
    TrashSelected, NewFolder, Quit,
    // P1: RenameSelected, Copy, Cut, Paste, SelectAll, FocusPathBar
]);
// 起動時: cx.bind_keys([...])、ルート要素に .key_context("Explorer") + .on_action(...)
```

- `gpui_kit::actions!` は gpui-kit 独自マクロ（`#[action(namespace = ...)]` を内部付与、✅ gpui-kit 0.9.1 `src/lib.rs`）。`use gpui_kit::*` で利用可能。
- POC は `on_key_down` 直処理だったが、P0 は **Action + KeyBinding** に移行（メニュー/ツールチップとの整合、ユーザーによる再割当の余地）。矢印キーだけは `on_key_down` のまま。

---

## 6. ファイル操作の安全性

### 6.1 原則（trash-first）

1. **削除 = ゴミ箱へ移動**。P0/MVP に完全削除の機能は実装しない（UI にも出さない）。`trash 5.3.0`（MIT）を使用（fs-notes §5.2）。
2. 削除は**必ず確認ダイアログを経由**する（1 件でも）。「元に戻せる」ことをメッセージに明記する。
3. 失敗は握りつぶさない。**1 件失敗しても全体を止めず、結果を集約**して通知する（P1 の複数件で重要）。
4. パスは `PathBuf` のまま扱い、文字列再構築しない（Linux 非 UTF-8 名）。

### 6.2 確認フロー（P0）

```rust
// fs/ops.rs
pub fn confirm_and_trash(
    paths: Vec<PathBuf>, window: &mut Window, cx: &mut AsyncApp,
) -> Task<Result<Vec<PathBuf>, FsError>>;   // 実装イメージ
```

- 確認 UI は `Window::prompt(PromptLevel::Warning, message, detail, &[PromptButton::ok("ゴミ箱へ移動"), PromptButton::cancel("キャンセル")], cx)` を第一候補とする（api-notes §4.5 の PromptLevel/PromptButton。✅ gpui-pre 0.3.12 `src/window.rs:6959`）。
- **重要な実測知見（✅ ソース確認）**: `gpui-pre-linux 0.3.12` の Wayland/X11 実装は `PlatformWindow::prompt` で **`None` を返す**（`src/linux/wayland/window.rs:1857-1865`、`src/linux/x11/window.rs:1520-1528`）。そのため Linux では GPUI 既定の `PromptBuilder::Default` による **GPUI 描画のフォールバックダイアログ**になる（`gpui-pre 0.3.12 src/window.rs:6977-7011`）。「ネイティブ確認」は Windows/macOS でのみ真のネイティブになる想定（⚠ 未検証）。
- 代替: gpui-component の `window.open_alert_dialog(cx, |alert, ..| alert.warning().title(..).description(..).on_ok(..).on_cancel(..))`（✅ component 0.9.1 `src/window_ext.rs`・`src/dialog/alert_dialog.rs` に存在）。見た目・フォーカス管理はこちらが良いが、Root のプラグイン経由で動くため、**P0 は `Window::prompt` を第一候補、動かなければ AlertDialog** に切り替える（ADR-008）。
- `prompt` は `oneshot::Receiver<usize>` を返す。UI スレッドをブロックせず `cx.spawn` で await し、ボタン index（0=OK, 1=Cancel）で分岐する。**再入防止**: `cx.prompt_builder` は take/戻しで再入不可のため、ダイアログ表示中は別の prompt を出さない。

### 6.3 シンボリックリンク

- 列挙時: `DirEntry::file_type()` ではリンクを辿らない。**ディレクトリ判定は `fs::metadata`（辿る）で行い、リンク自体は `symlink_metadata` で判定**する。POC はディレクトリ判定のみ `fs::metadata` を使用（[`poc/src/main.rs`](poc/src/main.rs) 74-77 行）。
- 削除/移動: 対象は**リンク自身**（`trash::delete` はパスをそのままゴミ箱へ移す）。**リンク先を消さない**。再帰削除を将来実装する場合も `symlink_metadata` で必ず分岐（fs-notes §10-12）。
- `SymlinkToDir` への Enter はディレクトリ移動として扱う（リンク先 `path` へ）。リンク切れは `Other` 扱い（開けない通知）。

### 6.4 権限・read-only・immutable

- `read_dir` の `PermissionDenied` → 「アクセスできません」+ 親へ移動ボタン。`NotFound` → 「削除されました」+ 親へ移動。
- 削除/移動の `EACCES`/`EPERM` → 「権限がないため操作できません」を表示して**属性解除を自動でやらない**。解除はユーザーが手動で行う（MVP 以降で「属性を確認」の情報表示のみ検討）。
- immutable/read-only 属性（Linux `chattr +i`、Windows READONLY、macOS `uchg`）はエラーをそのまま日本語化して表示（fs-notes §5.2）。

### 6.5 ネットワーク FS・リムーバブル

- `trash` が失敗する FS（NFS/SMB、一部リムーバブル）がある。**P0/MVP では完全削除にフォールバックしない**。「この場所ではゴミ箱へ移動できません」と通知し、ユーザーに手動削除を委ねる（安全側）。
- `notify` が効かない/遅い環境: `WatchState::Degraded` とし、10 秒リコンサイルを継続 + ステータスに明示（RESEARCH §5.4）。
- ネットワーク FS 判定（`statfs` の f_type）は P2（⚠ 実装未定、fs-notes §14-12）。

### 6.6 削除・作成後の UI 整合

1. 成功した削除対象を `Listing::remove_path` で即時除去 → `rebuild` → `Selection::reconcile` → `scroll` 位置を clamp → `notify`。
2. 失敗時は一覧を変更しない（楽観更新をしない）。通知で理由を表示し、監視イベント/F5 で再同期。
3. 新規フォルダ: `create_dir` を background 実行 → 成功したら再読込（または upsert）→ 新フォルダを選択状態に。名前は `新規フォルダ`、衝突時 `新規フォルダ (2)`…（`exists` チェックは `create_dir` の `AlreadyExists` で検出）。
4. 監視の自己操作抑制（§4.6）とセットで、ちらつきを防ぐ。

### 6.7 完全削除（将来の扱い）

- P2 で「ゴミ箱が使えない FS 向けの最終手段」としてのみ導入する場合も、**二段確認 + 既定無効 + 赤警告 + 対象のフルパス表示**を必須とする。P0/MVP のコードベースには関数を作らない（誤用防止）。
- 参考: 調査のコンテキストメニュー例には「完全に削除」が含まれる（[`RESEARCH.md`](RESEARCH.md) §4.3）が、**本プロジェクトでは P0/MVP で意図的に除外**する。

---

## 7. OS 統合マトリクスとフェーズ分割

| 機能 | Linux（Kubuntu/KDE Wayland） | Windows 11 | macOS | P0 | P1 | P2 | 根拠/API |
|---|---|---|---|---|---|---|---|
| 一覧読み込み | `std::fs::read_dir` + `file_type` ✅ | 同左（FindFirstFile 属性） | 同左 | ✅ | | | fs-notes §1.1 |
| ファイル監視 | inotify / notify 8.2 ✅ | RDCW / notify | FSEvents / notify | ✅ | | | fs-notes §2 |
| ゴミ箱 | freedesktop Trash / trash 5.3 ✅ | Recycle Bin（trash） | Trash（trash） | ✅ | | | fs-notes §5.2 |
| 開く | `cx.open_with_system`（xdg-open 系）✅ API / ⚠ 実行時 | ShellExecute | `open` | ✅ | | | api-notes §4.5 |
| リビール | `cx.reveal_path` → `org.freedesktop.FileManager1.ShowItems`（Dolphin 対応）⚠ KDE 実機 | `explorer /select,` | `open -R` | ✅ | | | api-notes §8.1 / fs-notes §6.2 |
| ファイルマネージャ起動 | 上記のフォールバックとして `OpenURI` | — | — |  | | | opener 0.9（reveal feature）。P2 で導入検討 |
| ファイル種別アイコン | Lucide + 拡張子マップ（P0）/ freedesktop-icons（P2） | SHGetFileInfoW（P2） | NSWorkspace（P2） | ✅ | | | fs-notes §3 |
| 画像サムネイル | `image` + `img(PathBuf)`（P2） | IShellItemImageFactory（P2） | QuickLook（P3） |  | | ✅ | api-notes §7.2 |
| パス文字列コピー | `ClipboardItem::new_string` ✅ | 同左 | 同左 | ✅ | | | api-notes §4.4 |
| ファイルリスト clipboard | GPUI `ClipboardEntry::ExternalPaths`（Wayland ✅ / X11 ⚠） / arboard 3.7（P2） | CF_HDROP | .fileURL |  |  | ✅ | api-notes §4.4 / fs-notes §7 |
| ネイティブフォルダ選択 | `prompt_for_paths`（XDG ポータル、⚠ ポータル必須） | IFileDialog | NSOpenPanel |  | ✅ | | api-notes §4.5 |
| アプリメニュー | `set_menus`（⚠ KDE 未検証） | 同左 | 同左 |  |  | △ | api-notes §8.2 |
| 通知 | gpui-component `push_notification` ✅ source / ⚠ Wayland 実機 | 同左 | 同左 | ✅ | | | component `src/window_ext.rs:118` |
| パス正規化 | `PathBuf`/`OsStr` 保持、表示のみ lossy ✅ | 長パス `\\?\` + `dunce`（P2） | NFD→NFC 表示（P2） | Linux のみ | | ✅ | fs-notes §5.3 |
| 隠しファイル | `.` 始まり ✅ | FILE_ATTRIBUTE_HIDDEN（P2） | `.` + UF_HIDDEN（P2） | ✅ | | | fs-notes §5.4 |
| ボリューム/空き容量 | sysinfo（P2） | sysinfo（P2） | sysinfo（P2） |  |  | ✅ | fs-notes §4 |
| D&D IN | `on_drop::<ExternalPaths>`（Wayland ✅/他 ⚠） | 同左 | 同左 |  |  | △ | api-notes §4.2 |
| D&D OUT | `external_drag_payload`（Wayland ✅） | ⚠ | ⚠ |  |  |  | api-notes §4.3 |

- **P0 の OS 統合は「GPUI core の API のみ」**で構成する（`open_with_system` / `reveal_path` / クリップボード text）。`opener` / `arboard` / `freedesktop-icons` の追加依存は P2 まで入れない（依存と検証面積を最小化、ADR-007）。
- KDE 固有: `WindowOptions::app_id = "gpui-file-explorer"` を必ず設定（Wayland の app_id → KWin の resourceClass/desktopFile に反映されることを POC で確認、[`notes/poc-report.md`](notes/poc-report.md) §3-3）。タイトルバーはネイティブ（CSD を自前実装しない）。

---

## 8. 設定スキーマ・テーマ・アイコン・i18n

### 8.1 設定ファイル

- パス: **`~/.config/gpui-file-explorer/settings.json`**（`dirs::config_dir()` 基準。XDG 準拠）。
- 形式: JSON（人が読める・差分が小さい・serde で十分）。`version` で将来のマイグレーション。

```json
{
  "version": 1,
  "last_dir": "/home/koppe/document",
  "sort": { "key": "name", "dir": "asc", "dirs_first": true },
  "show_hidden": false,
  "view": "details"
}
```

- ルール:
  - 未知フィールドは無視（`#[serde(default)]` + 将来追加に寛容）。欠損フィールドは既定値。
  - 破損 JSON: `settings.json.broken-<unix秒>` に退避して既定値で起動（起動失敗にしない）。⚠ 退避の実装は P0 で簡易ログでも可。
  - `last_dir` が存在しない/ディレクトリでない場合は `$HOME`（`dirs::home_dir()`）へフォールバック。
  - 保存は**原子的**（`settings.json.tmp` へ書いて `fs::rename`）。保存タイミング: ナビゲーション・ソート/隠しトグル・ウィンドウ終了時。頻度が低く 1KB 未満のため同期保存を許容（§4.4 の明示的例外）。
  - `dirs` は 7.0.0 推奨（fs-notes §12）。試作が 6 系でも `config_dir()/home_dir()` は同一 API のため許容（⚠ 版差の詳細未確認）。

### 8.2 テーマ方針

- P0: gpui-component の既定 Theme（ダーク）をそのまま使う。色は `ActiveTheme`（`cx.theme()`）の意味色を参照し、`rgb()` の直書きは `ui/` 内の最小限（アイコン色など）に留める。
- 意味色の割当: ディレクトリ=青系、シンボリックリンク=水色、隠し=減光、選択=アクセント背景、危険操作=赤。
- P2: ライト/ダーク切替 + システム追従（`ThemeMode`）。設定 `theme` を追加（`"system" | "light" | "dark"`）。
- コントラスト: 選択行とフォーカス行を区別（選択=背景、フォーカス=枠線）。色だけで状態を伝えない（アイコン/文字も併用）。

### 8.3 アイコン方針

- P0: **gpui-kit 同梱アイコン（Lucide 1,818 種 + 独自、ISC）** のみ。`icons.rs` に拡張子 → `IconName` の静的表:
  - ディレクトリ、画像（png/jpg/webp/svg…）、動画、音声、書庫（zip/tar/gz/7z…）、コード（rs/ts/js/py…）、PDF、テキスト（txt/md）、不明。
- 取得は同期（埋め込みアセットなので I/O なし）。OS ネイティブアイコンは P2（非同期 + `(icon_key, size, scale)` キャッシュ、RESEARCH §4.6）。
- アイコンの欠落時は `IconName::File` 相当の汎用にフォールバック（⚠ 正確な variant 名は実装時に component の `IconName` を確認）。

### 8.4 i18n 方針

- P0: **日本語 UI**。ユーザー向け文言はすべて `strings.rs` の `pub const` / 関数に集約（例: `pub const EMPTY_DIR: &str = "このフォルダは空です";`）。リテラルの散在禁止。
- 文言規約: 「フォルダ/ファイル」を使う（「ディレクトリ」はエラー詳細のみ）。敬体は「です/ます」、ボタンは体言止め（「ゴミ箱へ移動」「キャンセル」）。
- P2: `rust-i18n` 等でキー化（`strings::EMPTY_DIR` → `t!("empty_dir")`）。**キー名は P0 から i18n 前提の snake_case**（例: `error_permission_denied`）。
- 日時表示は相対（`3 分前`）を基本、詳細列は `YYYY-MM-DD HH:MM`（ローカルタイム）。現行試作は `chrono` 0.4 を追加してローカル時刻を整形している（P0 は許容。依存を減らすなら `std::time` + 自前変換でも可）。⚠ 相対表示の日本語化は純関数（`format.rs` 相当）で単体テスト。

---

## 9. ライセンス / AGPL-3.0-or-later 運用

### 9.1 方針

- アプリ全体を **AGPL-3.0-or-later** とする（依頼者決定、[`RESEARCH.md`](RESEARCH.md) §11.1.1）。gpui/gpui-kit（Apache-2.0）と AGPLv3 は両立（GPLv3 互換）。Lucide は ISC（帰属表示）。
- **Zed の GPL コード（`crates/ui` / `crates/project_panel` 等）は取り込まない**。読んで設計の参考にするのは可、コピー・git 依存は禁止（§9.5）。
- Argus（AGPL-3.0-or-later）のコードは**流用可能**だが、表示保持・変更明示・ソース提供の義務を負う（§9.4）。

### 9.2 リポジトリ構成（必須ファイル）

```
app/
├── LICENSE              # AGPL-3.0 全文（既存の AGPL プロジェクトからコピー、改変しない）
├── NOTICE.md            # 本アプリの著作権表示 + 第三者ライセンス一覧（下表）
├── README.md            # 機能・ビルド/実行・キー・既知の制限・AGPL・ソース入手先・設計からの差分
└── src/**.rs            # 各ファイル先頭に // SPDX-License-Identifier: AGPL-3.0-or-later
```

- `Cargo.toml` に `license = "AGPL-3.0-or-later"`、`repository` / `homepage` を記載。
- 各 `.rs` の先頭 1 行目: `// SPDX-License-Identifier: AGPL-3.0-or-later`（POC/試作も含む）。
- Markdown（README/NOTICE）にも `<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->` を推奨。DESIGN.md 等の文書は CC BY 4.0 のドキュメントとは分離（本プロジェクトの文書は AGPL に含める）。

### 9.3 配布時のソース提供（AGPLv3 §4・§13）

- バイナリ配布時は **対応する完全なソース**（アプリ本体 + 改変 + ビルド/インストール用スクリプト + 本設計書等のビルド補助物）を同一リリースページで提供（ソース tarball + タグ + SHA-256）。
- README の「ソースコード」節にリポジトリ URL とタグを明記。`.deb` のコントロール・desktop ファイル `Comment` にも短い案内（P2 でも可）。
- ネットワーク越しに改変版を使わせる場合（§13）: 「このアプリのソースは <URL> で入手できます」を About/README に表示（P2）。
- ライセンス全文と NOTICE をバイナリ配布物に同梱（`.deb` は `/usr/share/doc/<pkg>/copyright`）。
- macOS App Store 配布は行わない（DRM/規約と AGPL の衝突、[`RESEARCH.md`](RESEARCH.md) §11.1.1）。

### 9.4 Argus コード流用の境界（外部リポジトリ）

- 流用可能（同一 AGPL-3.0-or-later）。ただし:
  1. Argus の `LICENSE` と `THIRD-PARTY-NOTICES.txt` の表示を保持/再生成する（NOTICE.md に統合）。※Argus は元の開発ワークスペースの `security-review/Argus` にあり、本リポジトリには含まれない。
  2. コピーしたファイルには「元ファイル・元著作権者・変更内容」を先頭コメントに明記（例: `// Derived from security-review/Argus/src/... (AGPL-3.0-or-later, Copyright ...); modified 2026-11.`）。変更ファイル一覧を NOTICE.md に記載。
  3. Argus の名称・ロゴ・ブランドは使わない（商標はライセンス外）。
  4. Argus が vendor している gpui-pre パッチを流用する場合、Apache-2.0 部分の表示を保持し、Argus 改変部分は AGPL として扱う（`vendor/gpui-pre/ARGUS-PATCHES.md` 参照）。P0 では vendor 不要（gpui-kit で完結、[`notes/poc-report.md`](notes/poc-report.md) §4-4）。
- **P0 の実装は Argus コードをコピーしない**（UI 部品は gpui-kit で足りる）。流用は P2 以降で、コピー時に上記を実施。

### 9.5 Zed GPL コードの禁止

- Zed の `crates/ui` / `crates/project_panel` / `crates/theme` 等は **GPL-3.0-or-later**（[`RESEARCH.md`](RESEARCH.md) §11.1）。コピー・翻案・git 依存は禁止。**設計の参考（概念）に留める**。
- Zed の `gpui`（Apache-2.0）を使う gpui-pre は可（§2）が、`crates/ui` を引く依存は追加しない。
- 参考実装を探す場合もライセンス確認を必須とする（GPL 製 GPUI ファイルマネージャーのコードを混ぜない）。

### 9.6 依存ライセンス一覧（NOTICE.md に転記）

| 依存 | ライセンス | 表示要否 | 出典 |
|---|---|---|---|
| gpui-pre / gpui-pre-* 0.3.12 | Apache-2.0 | 表示 + ライセンス文 | [`notes/ecosystem-notes.md`](notes/ecosystem-notes.md) §5.5 |
| gpui-kit / gpui-base / gpui-component / gpui-kit-assets 0.9.1 | Apache-2.0 | 表示 + ライセンス文 | 同上 |
| Lucide アイコン（gpui-kit-assets 経由） | ISC | 帰属表示 | 同上 |
| notify 8.2 | CC0-1.0 | 任意（パブリックドメイン） | fs-notes §12 |
| notify-debouncer-full 0.7 | MIT OR Apache-2.0 | 表示 | fs-notes §12 |
| trash 5.3 | MIT | 表示 | fs-notes §12 |
| alphanumeric-sort 1.5 | MIT | 表示 | fs-notes §12 |
| serde / serde_json | MIT OR Apache-2.0 | 表示 | crates.io メタデータ |
| dirs 7（試作は 6 可） | MIT OR Apache-2.0（⚠ 未確認） | 表示 | fs-notes §12 |

- CI/リリース前に `cargo deny check licenses`（allow: AGPL-3.0-or-later, Apache-2.0, MIT, ISC, CC0-1.0, Unlicense, BSD-3-Clause, Unicode-3.0 / deny: GPL-2.0, GPL-3.0-only, SSPL）と `cargo audit` を実行（[`RESEARCH.md`](RESEARCH.md) §11.2）。P0 では `cargo tree` で直接依存のライセンス確認を README に記録（⚠ 自動化は P1）。

---

## 10. テスト・検証戦略

### 10.1 テスト層

| 層 | 対象 | 方法 | P0 |
|---|---|---|---|
| 単体（純関数） | sort/filter/selection/history/settings/format/error 変換 | `cargo test`（GPUI 不要） | ✅ 必須 |
| 結合（データ層） | `read_entries` の実 FS 挙動（一時ディレクトリ） | 一時 dir を作って `cargo test`（POC の 4 テストが雛形、[`poc/src/main.rs`](poc/src/main.rs) 420-477 行） | ✅ 必須 |
| UI ヘッドレス | クリック/キー合成、状態遷移 | `gpui-kit` `test-support` + `#[gpui_kit::test]` / `TestAppContext` | ×（P2。feature 追加が必要） |
| 実機 GUI スモーク | 起動・描画・操作・監視・ゴミ箱 | KWin D-Bus + XDG Portal スクショ（§10.3） | ✅ 必須 |
| 性能 | 1k/10k 件の初回表示・スクロール・監視反映 | 手動 + `date`/スクショ + `/proc/<pid>/status` | △（1k のみ） |

- **注意（✅ POC 実測）**: `use gpui_kit::*` は GPUI の `test` マクロを glob import し、組み込み `#[test]` を shadow する（"recursion limit reached"）。**テストモジュールでは明示 import を使い、`use super::*;` を避ける**（[`notes/poc-report.md`](notes/poc-report.md) §4-2 / [`poc/src/main.rs`](poc/src/main.rs) 420-425 行）。

### 10.2 単体テスト一覧（最低ライン）

| モジュール | テスト内容 |
|---|---|
| `model/sort.rs` | dirs first / 自然順（file2 < file10）/ 大小文字無視 / 降順 / サイズ・更新日時ソート / 同値時パス安定 |
| `model/listing.rs` | 隠しファイルフィルタ、`visible` 再構築、`upsert`、`remove_path` |
| `model/selection.rs` | Replace/Toggle/Range、`move_focus` の端で clamp、`reconcile`（削除後の近傍フォーカス）、`paths_for_action` |
| `model/history.rs` | Push で forward 消去、Back/Forward の往復、Up（親）、ルートで no-op |
| `model/format.rs` | `human_size`（0/1023/1024/1.5K/G）、`human_age`（None/秒/分/時/日）、0 件表示 |
| `model/load.rs` | `io::ErrorKind` → FsErrorKind 変換、日本語メッセージ |
| `settings.rs` | serde ラウンドトリップ、未知フィールド無視、破損 JSON で既定値、`last_dir` フォールバック、原子的保存（tmp 残存なし） |
| `fs/listing.rs` | 一時 dir の列挙（dirs first）、権限なし dir のエラー、read_dir 中の削除（一時ファイル削除）で全体失敗しない |
| `fs/watch.rs` | イベント正規化（notify 型 → FsEvent）の純関数テスト（可能なら） |
| `fs/paths.rs` | 隠し判定、拡張子抽出、`same_path`（Linux） |

### 10.3 GUI 受入手順（Kubuntu/Wayland）

```bash
# 0) ビルドとテスト
cd app
cargo build 2>&1 | tail -3          # 警告数を記録
cargo test 2>&1 | tail -5

# 1) 起動（cwd: まず $HOME を表示。小さめウィンドウでデスクトップを乱さない）
./target/debug/gpui-file-explorer > /tmp/gfe-run.log 2>&1 &
PID=$!
sleep 3
kill -0 "$PID" && echo ALIVE; wc -l /tmp/gfe-run.log; grep -iE 'panic|error|vulkan|egl' /tmp/gfe-run.log || true

# 2) ウィンドウ実在 + ネイティブ Wayland 確認（poc-report §3-3 のレシピ）
qdbus6 --literal org.kde.KWin /WindowsRunner org.kde.krunner1.Match "GPUI File Explorer"
#   → UUID を取得
qdbus6 --literal org.kde.KWin /KWin getWindowInfo "{<uuid>}"
#   → caption / resourceClass=gpui-file-explorer / width,height / minimized=false

# 3) スクリーンショット（XDG Desktop Portal）
gdbus call --session --dest org.freedesktop.portal.Desktop \
  --object-path /org/freedesktop/portal/desktop \
  --method org.freedesktop.portal.Screenshot.Screenshot "" "{}"
#   → ~/picture/Screenshot_*.png を app/screenshot/window.png にコピー（全体公開に注意: 自分用のみ）

# 4) 監視の実証（アプリが $HOME を表示中に実行）
touch "$HOME/__gfe_watch_test__"; sleep 1   # 一覧に現れることをスクショで確認
rm "$HOME/__gfe_watch_test__";     sleep 1   # 消えることを確認

# 5) ゴミ箱の実証（$HOME にテストファイルを作る）
echo test > "$HOME/__gfe_trash_test__.txt"; sleep 1
#  UI: 右クリック → ゴミ箱へ移動 → 確認ダイアログ → 実行
ls -la "$HOME/.local/share/Trash/files" | grep __gfe_trash_test__   # 入っていること
#  クリーンアップ（復元 or 手動削除）

# 6) 終了
kill -TERM "$PID"; sleep 1; kill -0 "$PID" 2>/dev/null || echo EXITED
qdbus6 --literal org.kde.KWin /WindowsRunner org.kde.krunner1.Match "GPUI File Explorer"  # 空
```

- **注意**: スクショはデスクトップ全体を撮るため、他ウィンドウ/個人情報が写りうる。確認用は**自分だけで見る**（Argus の AGENTS.md の全体キャプチャ禁止方針に準拠、[`RESEARCH.md`](RESEARCH.md) §9.2）。

### 10.4 P0 受け入れ条件（Prototype acceptance criteria）

| ID | 条件 | 合格判定 |
|---|---|---|
| AC-1 | `cargo build` が成功 | exit 0。エラー 0。警告は記録 |
| AC-2 | `cargo test` が成功 | 全 pass。§10.2 のうち sort/filter/selection/history/settings/format を最低 8 件 |
| AC-3 | Wayland で起動し、stderr に panic/EGL/Vulkan エラーが出ない | 起動後 3 秒でプロセス生存、ログに該当なし |
| AC-4 | ウィンドウが KWin D-Bus で確認できる | WindowsRunner で caption 検出、getWindowInfo で `resourceClass=gpui-file-explorer`、`wayland` |
| AC-5 | スクショで 5 領域（ツールバー/パンくず/サイドバー/一覧/ステータス）が確認できる | `app/screenshot/window.png` |
| AC-6 | 一覧が非同期にロードされ、読込中 UI が表示される | 「読み込み中…」表示を確認（スクショ or ログ）・起動後すぐ操作可能 |
| AC-7 | 監視更新: 外部 `touch` → 1 秒以内に一覧へ反映、`rm` → 消える | §10.3 手順 4 のスクショ 2 枚 |
| AC-8 | ナビゲーション: サイドバー/パンくず/ダブルクリック/Enter/Backspace/Alt+←→ が機能 | 手動で全経路確認。パス表示が一致 |
| AC-9 | 開く: ファイルのダブルクリック/Enter で既定アプリが起動（またはエラー通知） | 別ウィンドウ検出 or 通知で確認。アプリは落ちない |
| AC-10 | ゴミ箱: 確認ダイアログ→OK で一覧から消え Trash に入る。Cancel では何も起きない | §10.3 手順 5。2 経路とも確認 |
| AC-11 | 設定永続化: ソート/隠し/最後のパスが再起動後も復元 | 設定ファイル内容 + 再起動スクショ |
| AC-12 | コンテキストメニュー: 右クリック位置に表示、外側クリック/Esc で閉じる | 手動 + スクショ |
| AC-13 | SIGTERM でクリーン終了、ウィンドウ/プロセスが残らない | §10.3 手順 6。パニックログなし |

- **逸脱の扱い**: 環境制約で AC が満たせない場合（例: 既定アプリ起動がデスクトップを乱す、KDE の FileManager1 未対応）は、**試行の証拠（コマンド/ログ/スクショ）を README に記録した上で「未達」と明記**する。黙って除外しない。

### 10.5 性能目標（たたき台）

| 項目 | 目標 | 計測方法 | P0 での扱い |
|---|---|---|---|
| 起動 → 一覧操作可能 | debug 3 s / release 1.5 s 以内 | スクショ時刻差 | 参考記録 |
| 1,000 件の一覧表示 | 200 ms 以内（release） | ログ計測 | △ |
| 10,000 件の初回表示 | 名前+種別で 500 ms 以内、詳細列は 1 s 以内に逐次 | スクリプト生成 dir | P2（2 パス方式） |
| スクロール | 60 fps 維持（120 fps 目標） | 体感 + profiler ⚠ | フレーム落ちなし |
| キー移動の応答 | 1 フレーム（16 ms）以内 | 体感/ログ | 必須 |
| 監視イベント反映 | debounce 後 1 フレーム | スクショ | 必須 |
| メモリ（100,000 件） | 200 MB 以下 | `/proc/<pid>/status` VmRSS | P2 |
| アイドル CPU | ほぼ 0%（10 s リコンサイルのみ） | `top` | 必須 |

### 10.6 リスク

| ID | リスク | 影響 | 対策 |
|---|---|---|---|
| R1 | gpui-kit/gpui-pre の週次破壊的変更 | ビルド不能 | `=0.9.1` 完全ピン + `Cargo.lock` コミット + 隔週更新（RESEARCH §2.4） |
| R2 | Wayland/NVIDIA での描画問題 | 起動不能 | POC で実機確認済み ✅。release ビルドでもスモーク。EGL/Vulkan フォールバック無しを前提 |
| R3 | `-lxkbcommon-x11` リンク失敗（実測済み） | ビルド不能 | `scripts/dev-setup.sh` が `app/native-libs` に symlink を作成し、`app/build.rs` が絶対パスでリンカ検索パスへ追加（CWD 非依存。[`notes/poc-report.md`](notes/poc-report.md) §4-1 の当初案を改良）。恒久策は `libxkbcommon-x11-dev` 導入 |
| R4 | コンテキストメニュー部品の Wayland 挙動 | 右クリック不能 | `ContextMenuExt` 第一候補 → `deferred`+`anchored` フォールバック（§3.2） |
| R5 | 削除確認 prompt の Linux フォールバック | 見た目/フォーカス | `Window::prompt`（GPUI fallback）→ gpui-component AlertDialog に切替可能（ADR-008） |
| R6 | notify の取りこぼし/溢れ | 一覧が古い | 10 s リコンサイル + F5 + Degraded 表示 |
| R7 | trash 失敗（ネットワーク FS 等） | 削除不能 | 完全削除にフォールバックせず日本語エラー。安全側 |
| R8 | 監視の再帰 watch 過多 | inotify 枯渇 | 非再帰（cwd+親）を設計で固定 |
| R9 | AGPL 配布の実務漏れ | ライセンス違反 | §9 のチェックリスト + リリース時レビュー |
| R10 | 供給網（gpui-pre は個人公開・bus factor 1） | 将来の更新不能 | 完全ピン + `cargo-audit`/`cargo-deny`/`cargo-vet`、必要なら vendor（RESEARCH §11.2） |
| R11 | 日本語入力（IME）/リネーム | P1 の遅延 | P0 で入力 UI を持たない。P1 で gpui-component Input + fcitx5 実機検証を先に実施（⚠ 最大の実装リスク、RESEARCH §4.5） |
| R12 | 大量ディレクトリ（10 万件） | UI フリーズ | P0 は 1 パス + 10k 前提。P2 で 2 パス/バックグラウンドソート |

### 10.7 ADR（設計判断）

| ID | 決定 | 代替案 | 理由・影響 |
|---|---|---|---|
| ADR-001 | **`gpui-kit = "=0.9.1"`** を採用（gpui-pre 0.3.12 + platform + component + assets） | 公式 gpui 0.2.2 / gpui-pre 直接 / Zed git 依存 | 単一依存で必要部品が揃い、POC で実機動作 ✅。完全ピン運用が前提（RESEARCH §2） |
| ADR-002 | **P0 は単一 `ExplorerApp` Entity**。Listing/Selection は通常 struct | 機能別 Entity + Subscription | 状態共有が密で購読順序バグを避ける。タブ導入時に分割（§4.2） |
| ADR-003 | 一覧は **`uniform_list`（固定行高）** | gpui-component DataTable / `list`(可変高) / カスタム Element | ファイル一覧は固定行高が自然。DataTable は列機能が豊富だが API 面積大・⚠ 未検証。P1 で列ソート必要時に再評価（RESEARCH §4.1） |
| ADR-004 | **trash-first / P0-MVP は完全削除なし** | 完全削除をメニューに入れる | 誤削除の安全弁。fs notes の推奨に一致。完全削除は P2 で二段確認付き（§6） |
| ADR-005 | 監視は **非再帰（cwd + 親）** + debounce 400 ms + 10 s リコンサイル | 再帰 watch / 監視なし | inotify watch 枯渇とネットワーク FS 破綻を回避（RESEARCH §3.5） |
| ADR-006 | 監視イベントは **チャネル + 前景 pump ループ**（120 ms tick で drain、10 s リコンサイル）。チャネルは `std::sync::mpsc` か `async-channel` のどちらでも可 | ブロッキング recv / notify コールバックから直接 UI 更新 / イベントごとの `notify()` | UI をブロックせずバッチ化できる。`Debouncer` は `std::sync::mpsc::Sender` 実装を提供 ✅。現行試作は `async-channel`（§4.1 as-built） |
| ADR-007 | P0 の OS 統合は **GPUI core API のみ**（open_with_system/reveal_path/クリップボード text） | opener/arboard を最初から導入 | 依存と検証面積を最小化。失敗時は通知で明示。P2 で opener/arboard を追加 |
| ADR-008 | 削除確認は **`Window::prompt`** を第一候補 | gpui-component `open_alert_dialog` / 独自モーダル | GPUI 標準 API で Windows/macOS ではネイティブになる。Linux は GPUI fallback（✅ ソース確認）。UI が見劣りする場合のみ AlertDialog へ |
| ADR-009 | アイコンは **Lucide + 拡張子マップ**、OS ネイティブは P2 | OS ネイティブのみ / アイコンなし | P0 で見た目を成立させ、OS API の非同期/キャッシュ設計を後回しにできる |
| ADR-010 | 設定は **JSON + `dirs` + 原子的書込**、UI スレッドで同期保存（≤1KB） | TOML / 非同期保存 / DB | 可読性・依存最小・実装容易。書込は 1KB 未満で例外扱い（§4.4） |
| ADR-011 | **モジュール分割を最初から行う**（main/app/model/fs/ui） | POC の単一 main.rs を拡張 | テスト容易性・並行安全レビュー・将来のタブ/複数ウィンドウ対応 |
| ADR-012 | ライセンスは **AGPL-3.0-or-later**、Zed GPL は不取込 | MIT/Apache で配布 | 依頼者決定。Argus 流用可能・コピーレフト。配布時ソース提供の運用を §9 に固定 |

### 10.8 ロードマップ

| フェーズ | 期間目安 | 内容 | 完了条件 |
|---|---|---|---|
| P0 試作（本設計） | 1〜2 日 | 一覧/ナビ/選択/開く/リビール/コンテキストメニュー/ゴミ箱/監視/設定/アイコン/ライセンスファイル | §10.4 AC-1〜AC-13 |
| P1 MVP | 2〜4 週 | 列ソート、名前変更、コピー/移動/貼り付け、複数選択、パス入力、IME 検証 | 日常のフォルダ巡回・整理ができる。落ちない |
| P2 実用化 | 3〜6 週 | ゴミ箱ビュー、OS アイコン、サムネイル、再帰検索、ボリューム、タブ、テーマ/ i18n、D&D IN | 他 OS でも主要操作。監視の取りこぼしがリコンサイルで回復 |
| P3 差別化 | 将来 | 検索インデックス、プレビュー、アーカイブ、クラウド、D&D OUT、ロケール照合 | — |

---

### 10.9 Linux DEB パッケージ（2026-11-14 追加）

- 実装: `packaging/linux/build-deb.sh`（root 不要、`dpkg-deb --root-owner-group`）。
  `app/target/release/gpui-file-explorer`、`.desktop`、hicolor アイコン（16〜512px + scalable）、
  `README` / `copyright` / `NOTICE` を FHS 準拠の配置で収録する。
- `Depends` は `ldd` + `dpkg -S` から自動生成（ローカライズされた diversion 行を除外）。
  Vulkan ローダは dlopen のため `libvulkan1` を明示追加する。
- 検証: `dpkg-deb -x` 展開後のバイナリで `--version` と Wayland GUI 起動、`apt-get -s install` の
  依存解決を確認済み。署名・リポジトリ公開・タグ付けはスコープ外（リリースはユーザー指示時のみ）。
- 制約: Linux amd64 のみ。AppImage/Flatpak/Windows/macOS のパッケージングは未実装。

## 11. 未確認事項（⚠ 実装前に確認する）

1. **gpui-component の ContextMenuExt / PopupMenu の Wayland 実行時挙動**（ソース存在は ✅ 確認。`src/menu/context_menu.rs`、`src/menu/popup_menu.rs`）。P0 実機で確認し、不調なら `deferred`+`anchored` に切替（ADR-008 のフォールバック）。
2. **`Window::prompt` の Linux 表示**（GPUI fallback の見た目・フォーカス・Esc 挙動）。⚠ 実機未検証。
3. **gpui-component `Breadcrumb`**（`src/breadcrumb.rs` に存在 ✅。ただし公式 docs 索引に無く、ecosystem-notes は「該当部品なし」としていた）。試作では**自前 h_flex 実装を第一**とし、Breadcrumb は採用しない（API 安定性不明のため）。
4. **`cx.open_with_system` / `cx.reveal_path` の KDE Wayland 実挙動**（xdg-open / FileManager1 の応答、Dolphin のウィンドウが出るか）。⚠ 実機で確認。KDE で reveal が効かない場合は「パスをコピー」で代替し README に記載。
5. **notify の rename ペアリングと inotify の挙動**（debouncer-full の file-id cache が Linux で効くか、`Rescan` が来る条件）。⚠ 実機でログ確認。
6. **監視イベントの溢れ検知**（inotify `max_queued_events=16,384` 実測 ✅。溢れ時の notify の振る舞い ⚠）。10 s リコンサイルで保険。
7. **trash 5.3 の KDE/Wayland 実挙動とネットワーク FS での失敗型**（エラー型・パス）。⚠ 実機で一時ファイル削除を確認。
8. **IME（fcitx5/mozc）でのリネーム/パス入力**（P1。Linux Wayland 実装は ✅ ソース確認だが実機未検証）。
9. **Windows/macOS 全般**（ビルド・動作・長パス・大小文字・NFD・Recycle Bin・NSWorkspace）。⚠ CI ビルド以外未検証。
10. **10 万件ディレクトリの実測性能**（未計測）。P2 の 2 パス方式・background ソートの要否をここで判断。
11. **gpui-kit `open_window` が返す Root のプラグイン（ダイアログ/通知）が component 機能付きで有効か**（✅ ソース: `gpui_kit::open_window` は `base::Root::new` で包み、`gpui_kit::init` が `gpui_component::init` を呼ぶ。⚠ 実行時は未検証）。
12. **`ClipboardItem::new_string` でコピーしたパスが KDE の他アプリ（Dolphin 等）に期待通り貼り付くか**（テキストとしては動く想定 ⚠）。
13. **日本語ファイル名（CJK）の描画・フォント**（POC は ASCII のみ確認。Noto Sans CJK JP 等の明示指定要否 ⚠）。
14. **`dirs` クレートのバージョン差**（設計は 7.0.0 推奨、試作は 6 系。API 差は要確認 ⚠）。
15. **AGPL 配布チャネルの実務**（.deb / tar.gz / Flathub の NOTICE 同梱とソース提供 URL）。リリース時に確認。
16. **gpui-fast の採用可否**（P0 では無効。性能問題が出た場合のみ A/B、RESEARCH §2.4）。
17. **`notify-debouncer-full` の `DebounceEventResult` の型**（`Result<Vec<DebouncedEvent>, Vec<Error>>` を ✅ ソース確認。`DebouncedEvent` のフィールドは実装時に確認）。

---

## 12. 参照（ローカル成果物）

- 統合調査: [`RESEARCH.md`](RESEARCH.md)（§2 スタック / §3 アーキテクチャ / §4 UI / §6 クロスプラットフォーム / §8 性能 / §10 ビルド / §11 ライセンス / §12 ロードマップ / 付録 A POC 実測）
- API 棚卸し: [`notes/api-notes.md`](notes/api-notes.md)（§2 仮想リスト / §5 オーバーレイ / §6 非同期 / §9 core に無いもの / §11 ファイル:行 根拠）
- エコシステム: [`notes/ecosystem-notes.md`](notes/ecosystem-notes.md)（§1 入手経路 / §2 コンポーネント表 / §5 供給網・ライセンス）
- FS/OS 統合: [`notes/fs-cross-platform-notes.md`](notes/fs-cross-platform-notes.md)（§2 監視 / §5 操作 / §7 クリップボード / §9 マトリクス / §10 落とし穴 22 / §12 crate 表）
- POC: [`notes/poc-report.md`](notes/poc-report.md) / [`poc/src/main.rs`](poc/src/main.rs) / [`poc/README.md`](poc/README.md) / [`poc/screenshot/`](poc/screenshot/)
- 試作実装: [`app/`](app/) / [`app/README.md`](app/README.md)（実装・既知の制限・設計からの差分）
- ライセンス原本: 本リポジトリの [LICENSE](LICENSE) / [app/NOTICE.md](app/NOTICE.md)（AGPL 全文・第三者表示）
- crate ソース（ローカル・✅ 確認に使用）:
  - `~/.cargo/registry/src/index.crates.io-*/gpui-kit-0.9.1/src/lib.rs`（facade / open_window / init）
  - `~/.cargo/registry/src/index.crates.io-*/gpui-component-0.9.1/src/`（`menu/context_menu.rs`, `menu/popup_menu.rs`, `window_ext.rs`, `breadcrumb.rs`, `status_bar.rs`, `input/`）
  - `~/.cargo/registry/src/index.crates.io-*/gpui-pre-0.3.12/src/`（`window.rs` prompt / `platform.rs` PromptLevel / `interactive.rs`）
  - `~/.cargo/registry/src/index.crates.io-*/notify-debouncer-full-0.7.0/src/lib.rs`

### 変更履歴

| 版 | 日付 | 変更 |
|---|---|---|
| 0.1 | 2026-11-11 | 初版（P0 契約、§4/§5/§10.4 が実装対象） |

<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->
