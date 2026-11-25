<!-- SPDX-License-Identifier: AGPL-3.0-or-later -->

# GPUI File Explorer 試作 — 独立検証レポート（task-7）

- 検証日時: 2026-11-11 16:10 JST
- 検証者: fe-verifier（実装者 fe-dev から独立。実装者の報告を鵜呑みにせず、成果物・実ファイル・コマンド出力で再確認）
- 対象: `work/gpui-file-explorer/app/`（Cargo.toml 14:57 / src 15:47–15:50 / README・NOTICE・PROGRESS 15:53 / Cargo.lock 15:14 時点の最終版）
- 設計基準: `work/gpui-file-explorer/DESIGN.md`（§4 アーキテクチャ、§6 安全、§9 ライセンス、§10.3 GUI 手順、§10.4 AC-1〜AC-13）
- **検証スコープ（Lead 指示により縮小）**: GUI の実行検証は Lead が担当（`notes/lead-verify/`）。本レポートは
  **①ライセンス/コンプライアンス監査 ②静的コードレビュー ③Build/Test ④実装者証拠（スクショ・D-Bus ログ・設定ファイル）の独立確認 ⑤AC 対応表** を対象とする。
  GUI 実行の独立再現結果は Lead の記録を正とする。

## 0. 結論

**試作（P0）として許容できる。blocker（クラッシュ経路・データ損失・ライセンス違反）は検出されなかった。**

- Build/Test: `cargo build` 成功・警告 0、`cargo test` **35 passed / 0 failed**（いずれも `CARGO_NET_OFFLINE=true` で実行し、ネットワーク不要を確認）。
- ライセンス: LICENSE は AGPLv3 全文（661 行、`security-review/Argus/LICENSE` と SHA-256 完全一致）。`Cargo.toml` の license フィールド、6 ソース全部の SPDX ヘッダ、NOTICE の直接依存 10 件（gpui-kit/gpui-pre Apache-2.0、Lucide ISC を含む）を確認。**ライセンス違反なし**。ただし配布前対応（repository/homepage、ソース提供 URL）が未整備（minor）。
- コード: ゴミ箱のみ（完全削除なし）、確認ダイアログ経由、世代/Task キャンセル、監視の 400ms/10s/1.5s 構成は設計どおり。**blocker 0 / major 3 / minor 13**（§4）。
- major 3 件はいずれも「設計が明示的に禁止した UI スレッド同期 I/O」「描画パスでの毎フレーム I/O」「行 index キーのメニュー操作が更新で別エントリを指しうる」で、**プロトタイプの受入は妨げないが P1 着手前に修正すべき**。
- AC 表: AC-1/2/5 PASS、AC-3/4/6/7/11/13 は実装者証拠を独立確認（PASS*。GUI 独立再実行は Lead）、AC-8/9/10/12 は**入力注入手段が無く NOT TESTABLE**。
- 実装者側の過大主張・文書不整合: AC-7 の before/after スクショは既存 crop のバイト同一コピー、PROGRESS の「31 passed」が古い、README の設定ファイル「削除済み」が事実と相違、NOTICE の crate 名タイポ、AC-9 のエラー通知未実装（§7）。

## 1. 実行コマンドと結果

実行場所: `work/gpui-file-explorer/app/`（cargo 1.99.1 / rustc 1.99.1、Kubuntu 2026.04 / Plasma 6 / Wayland）

### 1-1. Build（AC-1）

```
$ CARGO_NET_OFFLINE=true cargo build   # 実行時 cargo/rustc プロセスなしを確認して実施
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.38s
    real 0m0.421s / user 0m0.248s / sys 0m0.139s   exit=0
warning 行: 0 / error 行: 0
$ stat target/debug/gpui-file-explorer
2026-11-11 15:50:36  172,865,384 bytes（約 164.9 MiB）
```

- 差分ビルド（0.38s）だが、ソース最終更新は 15:50（以後は md のみ 15:53）でバイナリと一致。**オフラインで成功**＝追加ネットワーク不要。
- README の「debug バイナリ 約 165MiB」と一致。
- 注: 本 build は 15:56 に実施（Lead の「build しない」指示より前、かつ同時に cargo が走っていないことを確認済み）。以降は再実行していない。

### 1-2. Test（AC-2）

```
$ CARGO_NET_OFFLINE=true cargo test
running 35 tests
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured
finished in 0.40s   exit=0   warning 0 / error 0
内訳: model 16 / settings 9 / history 5 / icons 3 / watcher 2
```

- DESIGN §10.2 の最低ライン「sort/filter/selection/history/settings/format で 8 件以上」を大きく上回る。
- ただし **UI 本体 `main.rs` にテストは 1 件もない**（ナビゲーション・世代管理・抑制窓は未テスト。minor m-11）。

### 1-3. ライセンス照合

```
$ sha256sum app/LICENSE security-review/Argus/LICENSE
0d96a4ff68ad6d4b6f1f30f713b18d5184912ba8dd389f86aa7710db079abcb0  app/LICENSE
0d96a4ff68ad6d4b6f1f30f713b18d5184912ba8dd389f86aa7710db079abcb0  security-review/Argus/LICENSE
$ wc -l app/LICENSE → 661 行、§13 Remote Network Interaction を含む完全な AGPLv3 本文
```

```
$ cargo metadata --format-version 1 --offline   # 808 packages
直接依存の license（cargo metadata 実測）:
gpui-kit 0.9.1 Apache-2.0 / alphanumeric-sort 1.5.8 MIT / notify 8.2.2 CC0-1.0 /
notify-debouncer-full 0.7.0 MIT OR Apache-2.0 / trash 5.3.0 MIT / serde 1.0.229 MIT OR Apache-2.0 /
serde_json 1.0.154 MIT OR Apache-2.0 / dirs 6.0.0 MIT OR Apache-2.0 /
async-channel 2.5.0 Apache-2.0 OR MIT / chrono 0.4.45 MIT OR Apache-2.0
→ NOTICE.md の記載と全件一致。AGPL/GPL-only/SSPL の推移依存は無し。
   （MPL-2.0: cbindgen/dwrote/option-ext、Unicode-3.0: ICU 系、NCSA/BSL-1.0/デュアル LGPL あり。いずれも AGPLv3 と互換）
```

### 1-4. スクショの実在・重複確認（sha256）

```
041cc9b8…  screenshot/window.png == window-crop.png == watcher-before.png   ← 同一ファイル
0cd6f279…  screenshot/watcher-after.png == watcher-refresh-crop.png        ← 同一ファイル
37307ad9…  screenshot/settings-restore-crop.png
a8b8c401…  screenshot/watcher-refresh.png
```

## 2. AC-1〜AC-13 対応表（DESIGN §10.4）

判定凡例: **PASS** = 私がコマンド/画像で直接確認。**PASS\*** = 実装者の記録・成果物を独立確認したが、GUI 実行の独立再現は Lead 管轄（最終判定は Lead の実行結果）。**NOT TESTABLE** = 環境に入力注入手段が無く実押下不可（理由記載）。

| AC | 条件（要約） | 判定 | 根拠・備考 |
|---|---|---|---|
| AC-1 | `cargo build` 成功・exit 0・警告記録 | **PASS** | §1-1。offline build exit=0、警告0、0.42s。 |
| AC-2 | `cargo test` 全 pass・最低 8 件 | **PASS** | §1-2。35 passed / 0 failed / 0 warnings。 |
| AC-3 | Wayland 起動・stderr に panic/EGL/Vulkan なし | **PASS\*** | 実装者記録「stderr 0 行・3秒生存」。Lead が独立再実行。 |
| AC-4 | KWin D-Bus でウィンドウ確認（caption / resourceClass / wayland） | **PASS\*** | 実装者記録: caption=`GPUI File Explorer`, resourceClass/Name=`gpui-file-explorer`, width=1100 height=728, type=0, fullscreen=false。参考: 15:42 に私自身も `qdbus6 … WindowsRunner Match` で `"0_{bf5311b8-…}", "GPUI File Explorer", "wayland"` を観測（実行主体は実装者側。Lead 再検証）。 |
| AC-5 | スクショで 5 領域（ツールバー/パンくず/サイドバー/一覧/ステータス） | **PASS** | `screenshot/window.png`（= window-crop.png と同一）を read_image で目視。ツールバー（←→↑⟳/パンくず/新規/目）、サイドバー8項目、列ヘッダ、一覧、ステータスバーを確認。 |
| AC-6 | 非同期ロードと読込中 UI 表示 | **PASS\***（実装確認） | overlay「読み込み中…」main.rs:712-718、ステータスバー main.rs:1046-1054。起動直後の一瞬表示のため静止画証拠は未取得 → Lead の実写確認を推奨。 |
| AC-7 | 監視更新: touch 1 秒以内反映 / rm で消える | **PASS\*** | `watcher-refresh-crop.png` を目視: `from-bash-while-running.txt` 行が追加され件数 12→13。ただし `watcher-before.png`/`watcher-after.png` は既存 crop のコピー（§1-4）で新規撮影ではない。Lead の独立監視テストを正とする。 |
| AC-8 | ナビゲーション全経路（サイドバー/パンくず/ダブルクリック/Enter/Backspace/Alt+←→） | **NOT TESTABLE** | 入力注入手段が無い（`kdotool`/`xdotool`/`ydotool`/`wtype` すべて未導入、`/dev/uinput` は root:root 0600 で sudo 不可）。コード経路は実装済み（main.rs:324-384, 925-930, 882-885, 761-767）。 |
| AC-9 | ファイルを開く（既定アプリ）またはエラー通知 | **NOT TESTABLE** | 同上で実押下不可。**加えて** `cx.open_with_system` の失敗を扱わず常に成功文言を表示（main.rs:394-395）＝「エラー通知」側は未実装（§7-7）。 |
| AC-10 | ゴミ箱: 確認→OK で Trash へ / Cancel で無変化 | **NOT TESTABLE** | 入力注入不可（右クリック・確認ダイアログ操作）。静的には trash-only・確認必須を確認（§5-4）。 |
| AC-11 | 設定永続化（ソート/隠し/最終パス）の再起動復元 | **PASS\*** | `settings-restore-crop.png` で再起動後のパンくず `/ › tmp › gpui-fe-persist-demo` を目視。現設定ファイルが DESIGN §8.1 のスキーマに一致。roundtrip/破損/部分JSON のテスト 9 件 pass。独立再起動は Lead。 |
| AC-12 | コンテキストメニュー（右クリック位置・外側クリック/Esc で閉じる） | **NOT TESTABLE** | 実押下不可。実装は `ContextMenuExt`。空き領域右クリックは未対応（README 記載の既知制限）。 |
| AC-13 | SIGTERM でクリーン終了・ウィンドウ/プロセス残存なし | **PASS\*** | 実装者記録「kill -TERM → 終了、KWin Match 空」。16:09 時点で私もアプリプロセス 0・KWin ウィンドウ 0 を確認（残留なし）。Lead 再検証。 |

**AC 判定の注記**: AC-8/9/10/12 を「入力コードがあるから PASS」とはしていない。ネイティブ Wayland への合成入力手段が本環境に存在しない以上、実押下の PASS は主張できない（DESIGN §10.4 の逸脱規定どおり「未達/未検証」と明記するのが正しい）。

## 3. ライセンス / コンプライアンス監査

| 項目 | 判定 | 根拠・指摘 |
|---|---|---|
| LICENSE = AGPLv3 全文 | ✅ | 661 行、見出し「GNU AFFERO GENERAL PUBLIC LICENSE Version 3」、§13 あり。`security-review/Argus/LICENSE` と SHA-256 完全一致（DESIGN §9.2 の「コピーして改変しない」どおり）。 |
| Cargo.toml `license` | ✅ | `license = "AGPL-3.0-or-later"`（Cargo.toml 5 行目）。先頭に SPDX コメント。 |
| SPDX ヘッダ全ソース | ✅ | `src/*.rs` 全 6 ファイルの 1 行目が `// SPDX-License-Identifier: AGPL-3.0-or-later`。README/NOTICE/PROGRESS にも `<!-- SPDX… -->`（DESIGN §9.2 の推奨どおり）。 |
| NOTICE の gpui/gpui-pre/gpui-kit（Apache-2.0） | ✅ | gpui-kit / gpui-component / gpui-kit-assets 0.9.1、gpui-pre / gpui-pre-platform 0.3.12 を Apache-2.0 として記載。Cargo.lock の実バージョンと一致。 |
| NOTICE の Lucide（ISC） | ✅ | ISC と明記、出典 URL あり。`AllAssets` で同梱される旨の説明あり。 |
| NOTICE の直接依存 10 件 | ✅ | Cargo.toml の 10 クレートすべて記載あり。バージョン・ライセンスは `cargo metadata` 実測と一致（§1-3）。 |
| NOTICE の推移依存の記述 | ⚠ minor | 「MIT / Apache-2.0 / BSD / ISC / CC0 / Zlib で permissive」と断定しているが、実測では MPL-2.0（cbindgen/dwrote/option-ext）、Unicode-3.0（ICU 系）、NCSA、BSL-1.0、LGPL-2.1-or-later デュアルも存在。**AGPLv3 と非互換な GPL-only は無い**が、記載は不正確。 |
| NOTICE の crate 名 | ⚠ minor | `gpui_kits::assets::AllAssets` はタイポ。正しくは `gpui_kit::assets::AllAssets`（main.rs:1189 と不一致）。 |
| Cargo.toml `repository` / `homepage` | ❌ minor | DESIGN §9.2 が要求するが未記載（`license` 以外のメタデータなし）。 |
| README の AGPL 告知 | ✅ | 冒頭・末尾の「ライセンス」節で AGPL-3.0-or-later、LICENSE/NOTICE へリンク。 |
| README のビルド/実行/キー/既知の制限 | ✅ | §ビルド、§実行、§キーバインド、§既知の制限、§設計からの差分 が揃う。 |
| README のソース提供（AGPL §4/§13） | ❌ minor | リポジトリ URL・タグ・ソース提供方法の記載なし（DESIGN §9.3）。**バイナリ未配布の現段階では違反ではない**が、配布前に必須。 |
| コピーレフト混入（Zed GPL / Argus 流用） | ✅ | gpl 系推移依存なし、Argus からのコピーファイル無し（派生表示の必要性なし）。`gpui-pre` は Apache-2.0 で互換。 |

総評: **ライセンス違反は無い**。配布（バイナリ公開）前に (1) repository/homepage (2) source-offer URL/タグ (3) NOTICE の推移ライセンス文言修正 (4) cargo-deny/cargo-audit を追加すること。

## 4. コードレビュー所見（重大度つき・file:line は最終版）

### blocker

なし（クラッシュ・データ損失・完全削除・シェル実行・秘密漏えいは検出されず）。

### major

- **M-1【描画パスで毎フレーム同期ファイル I/O】main.rs:1162 / 833-841 / 862**
  `render()` から `sidebar()` を毎回呼び、`dirs::desktop_dir()`〜`dirs::video_dir()`（main.rs:836-841）を毎回評価している。`dirs` 6.0.0 は `dirs-sys` 0.5.0 経由で **呼び出しごとに `$XDG_CONFIG_HOME/user-dirs.dirs` を read** し（crate ソース確認済み: `xdg_user_dirs::single()` → `read_all()`）、さらに各サイドバー項目に `target.is_dir()`（main.rs:862）で stat する。**1 フレームあたり最大 6 ファイル読み + 8 stat**。DESIGN §4.4「UI スレッドで metadata / read 禁止」に明確に違反し、ネットワーク FS・低速ディスクで描画停止リスク。ローカル SSD では実害が小さいため AC は通るが、設計違反として major。
- **M-2【新規フォルダ作成が UI スレッド同期】main.rs:508-511（+ model.rs:341-352 `unique_child_name` の `exists()` ループ）**
  `std::fs::create_dir` と一意名探索の `Path::exists()` を UI コールバック内で同期実行。DESIGN §4.4/§6.6 は background 実行を要求（他 I/O は `cx.background_spawn` 済みなのにここだけ同期）。低速 FS で UI ブロック。加えて exists→create の TOCTOU で、同時作成時は連番リトライせずエラー表示のみ（minor 相当の副次問題）。
- **M-3【コンテキストメニューが行 index を捕捉 → 更新後に別エントリへ作用】main.rs:595-645（対象解決は 612, 619, 626, 635）/ 390 / 525-541 / 551**
  `entry_menu(index, …)` が位置 index を捕捉し、クリック時に `self.entries.get(index)` で対象解決する。メニュー表示中に 400ms debounce の watcher 更新で並び替わると、**右クリックした行と別のエントリ**が対象になる。ゴミ箱は確認ダイアログが「その時点の対象名」を表示するため誤削除に気づけるが、開く/パスをコピー/ファイルマネージャで表示は**無言で別エントリ**に作用しうる。DESIGN §5.3 はパスキー選択を要求（P0 は単一選択だがメニュー対象はパスで固定できる）。修正は「メニュー生成時に `PathBuf` を捕捉し、クリック時にそのパスの行を再解決」。

### minor

- **m-1【watcher の実行時エラー/Degraded 未実装】** watcher.rs:33 `let _ = result;` でイベント内容と `DebounceEventResult` の Err を捨てる。DESIGN §4.6 の `WatchState::Degraded`・再試行が無く、状態表示は「監視: 無効」/件数のみ。`last_error()`（watcher.rs:54）は初期化失敗だけ、`Drop` 実装（watcher.rs:90-97）は何もせずコメント（「stop する」）と不一致。
- **m-2【watch の再試行なし】** watcher.rs:66-68 対象集合が同じなら再 watch しないため、一度失敗した（権限・一時消失）ディレクトリは reload を繰り返しても復旧しない。
- **m-3【抑制窓の範囲と reconcile の分解能】** 抑制は全イベント対象の時間窓（main.rs:51, 281-286, 576）で、窓内の外部変更を最大 1.5s 取りこぼす（README 記載の既知制限）。10s reconcile は mtime を秒精度でしかハッシュしない（model.rs:275, 280-285）ため、**同秒・同サイズの内容変更は検出不能**。
- **m-4【settings の細部】** tmp+rename で原子的（settings.rs:114-123）だが fsync なし。破損 JSON は `settings.json.broken-<epoch>` に退避（settings.rs:79-90）——README の「安全に無視」より副作用が強い（意図的だが README 未記載）。同一秒の複数破損で退避先が衝突。`version` を無条件に 1 へ丸めるため将来スキーマのダウングレードリスク（settings.rs:92-93）。
- **m-5【UI スレッドの単発 fs】** 起動時 `canonicalize`+`is_dir`（main.rs:115-116）、`navigate_to`/`go_back`/`go_forward` の `is_dir`（main.rs:341, 355, 366）、`Settings::validate` の `is_dir`（settings.rs:98）。単発なので影響小だが、厳密には §4.4 違反。
- **m-6【read_dir の per-entry エラー無言スキップ】** model.rs:65-70。件数が実際と異なりうる（設計意図の「全体失敗しない」には沿う）。
- **m-7【成功メッセージ消失】** reload 完了時に `status.clear()`（main.rs:183）するため、「ゴミ箱へ移動しました」「変更を検出しました」がほぼ見えない。
- **m-8【open/reveal の失敗を無視】** main.rs:394-395 / 538-539。`cx.open_with_system`/`cx.reveal_path` の失敗を扱わず常に成功文言。AC-9 の「またはエラー通知」は未実装。
- **m-9【prompt 再入防止なし】** main.rs:554。ダイアログ表示中に別 prompt を出さない保証が無い（DESIGN §6.2 の再入防止）。`receiver.await.unwrap_or(0)` は失敗時キャンセル扱いで安全側。
- **m-10【非 UTF-8 名】** model.rs:33 で lossy 表示。I/O は PathBuf のままなので破壊はない（README 記載どおり）。同名に化けると区別不能・ソートも lossy。
- **m-11【UI 層テストゼロ】** main.rs にテストなし。世代管理・抑制窓・メニュー対象解決など UI ロジックは未検証。
- **m-12【dirs_first が実質固定】** SortSpec.dirs_first を保存するが読み込み時は常に先頭固定（model.rs:205-238）。スキーマ先行の設計意図どおりだが、false を書いても無視される。
- **m-13【マシン固有パス】** `.cargo/config.toml` の `-L native=<絶対パス>` と `native-libs` の絶対 symlink（README 記載済み）。他マシン再現には書き換えが必要。

### 依頼された観点ごとの判定

| 観点 | 判定 | 根拠 |
|---|---|---|
| (a) render/UI での blocking fs | **FAIL（major M-1/M-2、minor m-5）** | 上記。 |
| (b) generation / Task drop キャンセル | **PASS** | reload: generation+1 → 旧 `load_task.take()`（drop）→ 結果に `if this.generation != generation { return; }`（main.rs:163-197）。reconcile: generation + loading + fingerprint ガード（main.rs:250-271）。妥当。 |
| (c) watcher（非再帰・400ms・10s・1.5s） | **PASS（実装確認）+ minor m-1/m-2** | 400ms: watcher.rs:17。非再帰・cwd+親: watcher.rs:77, main.rs:232-241。10s: main.rs:50, 298-309。1.5s: main.rs:51, 310-320, 513, 576。Degraded 未実装のみ。 |
| (d) trash-only + 削除前確認 | **PASS（静的）** | 生産コードに `remove_file`/`remove_dir_all` は無し（test モジュールのみ）。削除は `trash::delete` のみ（main.rs:571）。`window.prompt` 確認後にのみ実行（main.rs:554-568）。Delete キー・行メニューとも同じ経路（main.rs:476-479, 626-635）。E2E は NOT TESTABLE。 |
| (e) settings 書込の安全性・読込失敗 | **PASS（minor m-4）** | 一時ファイル + rename（settings.rs:114-123）、破損時は退避して既定値（settings.rs:79-90）、`save()` はエラー無視（settings.rs:105-111）。 |
| (f) unwrap/expect/panic 監査 | **PASS** | 生産コードの `unwrap/expect/panic` は main.rs:1221 の `expect("failed to open window")` のみ（起動不能時の致命処理として許容）。他は `Option`/`Result` で処理。test 内の unwrap は対象外。 |
| (g) パス扱い・シェル補間 | **PASS** | I/O は `PathBuf`（lossy は表示名のみ、README 明記）。`Command`/`std::process` 実行なし＝シェル補間リスクなし。 |
| (h) データ損失・symlink | **PASS（静的）** | 完全削除なし。symlink は `symlink_metadata` でリンク判定、`metadata` で dir 判定（model.rs:37-48、DESIGN §6.3）。再帰削除なし。TOCTOU は理論上あるがデータ損失経路ではない。 |
| (i) 秘密・パスログ | **PASS** | `println/eprintln/log` なし。パスは UI 表示とクリップボード（明示操作）、設定の last_dir のみ。ログへの漏えいなし。 |

## 5. 実機に関する証拠（実装者成果物の独立確認）

GUI の独立再実行は Lead 管轄。以下は実装者が残した成果物を**私が直接開いて確認した**もの。

1. **ウィンドウ/ネイティブ Wayland**
   - 実装者記録（README §実行検証記録）: `getWindowInfo` で caption=`GPUI File Explorer`, resourceClass/Name=`gpui-file-explorer`, width=1100 height=728, minimized=false, fullscreen=false, type=0。KRunner Match で `"wayland"`。
   - 参考（私の観測）: 15:42 に `qdbus6 --literal org.kde.KWin /WindowsRunner org.kde.krunner1.Match "GPUI File Explorer"` が `"0_{bf5311b8-ddc4-49d9-b0ae-b9fbadc138be}", "GPUI File Explorer", "wayland", …` を返すことを確認（その実行は実装者/Lead 側。私の起動ではない）。
2. **スクリーンショット（read_image で目視）**
   - `app/screenshot/window-crop.png`（1140x790）: タイトルバー `GPUI File Explorer`、ツールバー（←→↑⟳・パンくず `/ › tmp › gpui-fe-demo`・新規フォルダ・目アイコン）、サイドバー 8 項目（ホーム/デスクトップ/ドキュメント/ダウンロード/ピクチャ/ミュージック/ビデオ/ルート）、列ヘッダ（名前/サイズ/更新日時）、一覧（dirs first、`file2.txt` < `file10.txt` の自然順、日本語フォルダ名・日本語ファイル名も表示）、ステータスバー（パス・12 件・選択・隠し 1 件・監視 2 ディレクトリ）を確認。**5 領域すべて実在＝AC-5 の主張は本物**。
   - `app/screenshot/watcher-refresh-crop.png`: 起動中に作成した `from-bash-while-running.txt` が行として追加され、件数が **12→13** に増えている。監視更新の証拠として妥当（AC-7）。ただし AC 用の `watcher-before.png`/`watcher-after.png` は本画像とバイト同一のコピー（§1-4）。
   - `app/screenshot/settings-restore-crop.png`: 再起動後のパンくずが `/ › tmp › gpui-fe-persist-demo`。AC-11 の復元証拠として妥当。
   - フルスクリーンショット（portal 出力）はプライバシー上の理由から削除済み。crop 画像のみ保持。
3. **設定ファイル（DESIGN §8.1 スキーマ確認）**
   - `~/.config/gpui-file-explorer/settings.json` は
     `{"version":1,"last_dir":"/tmp/gpui-fe-schema-check","sort":{"key":"name","dir":"asc","dirs_first":true},"show_hidden":false,"view":"details"}`
     で DESIGN §5.7/§8.1 と一致（実装者がこのスキーマへ修正したことを確認）。
   - **注意（ユーザー設定の残骸）**: このファイルは残っており、`last_dir=/tmp/gpui-fe-schema-check` は既に存在しない。タスク指示 6（実行後に検証用設定を復元/削除）は未完了。Lead の実行検証でも同じ設定を触るため、**検証後は XDG_CONFIG_HOME 分離または復元**が必要。
4. **終了・残留**
   - 16:09 時点で `pgrep -af gpui-file-explorer` は 0 件、KWin Match も空 `{}`。プロセス/ウィンドウ残留なし。

## 6. DESIGN §4.1 との差分（as-built）

README の「設計（DESIGN.md）からの差分」表と整合することを確認。主なもの:

- モジュール構成はフラット（main/model/history/settings/icons/watcher）。DESIGN の `app.rs`+`fs/*`+`ui/*` 分割は P1。
- キー入力は `actions!`+KeyBinding でなく `on_key_down`（track_focus）。DESIGN §3.4 のキー動作はカバー。
- 設定スキーマは DESIGN §5.7/§8.1 に一致（15:45-16:05 の修正で解消済み）。
- 選択は `selected: usize` の単一選択（P0 要件どおり。DESIGN §5.3 の PathBuf キー複数選択は P1）。
- ステータスバーは自作 div（gpui-component StatusBar でない）。テーマ色は直書き（§8.2 の「最小限」からは逸脱）。
- 文言は `strings.rs` になく main.rs 内リテラル。`view` フィールドは保存のみ。
- **DESIGN との非記載差分を 1 点発見**: DESIGN §3.1/§2 は「列ヘッダソートは P0 なし・P1」だが、実装はヘッダクリックソートを P0 で実装済み（README の差分表に未記載。機能追加なので害はない）。
- P0 の未実装（設計どおり P1 送り）: リネーム、コピー/移動、複数選択、タブ、検索、パス入力、グリッド、OS ネイティブアイコン、空き領域右クリック。実装者も README/PROGRESS に明記しており、**隠れた未実装は無い**（P0 omissions としての重大な見落としなし）。

## 7. 過大主張・文書の不整合（フェアに指摘）

1. **AC-7 の「touch 前後」スクショ**: `watcher-before.png`/`watcher-after.png` は既存 `window-crop.png`/`watcher-refresh-crop.png` と**バイト同一**（sha256 一致）。画像自体は本物の実行証拠だが「新たに touch 前後を撮った」ものではない。README 本文は旧ファイル名を参照しており、主張の実質的な過大化は小さい。
2. **PROGRESS.md の状態表が「31 passed」のまま**（実測・README は 35 passed）。また「最終更新: 15:40 JST」と内容（16:05 まで記載）が不整合。
3. **README「検証用の設定ファイル・一時ディレクトリ…は検証後に削除済み」**: 実際は `~/.config/gpui-file-explorer/settings.json` が残存（`last_dir` は存在しないディレクトリ）。削除漏れ。
4. **NOTICE の crate 名タイポ**: `gpui_kits::assets::AllAssets`（正: `gpui_kit::assets::AllAssets`）。また推移依存を「permissive のみ」と書くのは不正確（§3）。
5. **AC-9 の「またはエラー通知」**: open/reveal の失敗処理が無く常に成功文言（main.rs:394-395, 538-539）。AC 文言の代替条件を満たしていない。
6. **README の「壊れた JSON … 安全に無視」**: 実際は破損ファイルを `settings.json.broken-<epoch>` へ rename する副作用あり（安全側だが記述より強い）。
7. **自己申告タイムスタンプ**: 実装者のメッセージは「最終コード 16:05 確定」としていたが、ファイル mtime は src 15:47-15:50 / docs 15:53。以後の変更は無く、実質的な問題ではない（記録の不正確さのみ）。
8. 実装者申告「35 passed」「警告 0」「165MiB」「settings スキーマ一致」は**私の再実行・実測と一致**（過大主張ではない）。

## 8. 環境制約（未検証の理由）

- ネイティブ Wayland への入力注入手段が無い: `kdotool` / `xdotool` / `ydotool` / `wtype` すべて未導入。`/dev/uinput` は `crw------- root root` で sudo 不可のため uinput 自作も不可。`xdotool` は XWayland 専用で native Wayland 窓には無効。XDG Portal の `RemoteDesktop` IF は存在するが、利用には画面共有の許可ダイアログ操作が必要で「デスクトップを乱さない」制約に反するため未使用。
- そのためキーボード・右クリック・確認ダイアログ・削除 E2E（AC-8/9/10/12）は NOT TESTABLE。**コードレビューとデータ層テストで代替**した。
- sudo 不可（ユーザー指示）。システムへのパッケージ導入なし。
- GUI 実行検証は Lead へ移管（本レポートは静的監査＋実装者証拠の確認まで）。

## 9. 推奨修正（優先度順）

**P0（受入後すぐ・P1 着手前）**
1. M-1: サイドバーの場所解決と `is_dir()` を描画から排除（起動時/ディレクトリ変更時に計算してキャッシュ。`dirs::*_dir()` は呼び出しごとに user-dirs.dirs を読む点に注意）。
2. M-2: 新規フォルダの `unique_child_name`+`create_dir` を `cx.background_spawn` へ移し、完了後に再読込して `pending_select`。`AlreadyExists` は連番リトライ。
3. M-3: コンテキストメニューは生成時に `PathBuf` を捕捉し、クリック時にパスで再解決（index 依存を廃止）。対象が消えていたら何もしない。
4. m-8: `open_with_system`/`reveal_path` の失敗を通知（または成功文言を「要求しました」に弱める）。
5. ユーザー設定の後始末（`settings.json` の検証用 last_dir）を Lead の実行後に実施。以後の検証は `XDG_CONFIG_HOME` を一時ディレクトリに分離。

**P1**
6. Watcher の `DebounceEventResult` を保持して `Degraded` 状態を実装、失敗時の再試行（同一 dirs でも再 watch 可能に）。`Drop`/`last_error` の整理。
7. settings: fsync（または rename 後 dir fsync）、破損退避の重複回避、`version` のマイグレーション方針明記。
8. main.rs の UI ロジックにテスト追加（世代ガード、抑制窓、メニュー対象解決、settings 復元）。
9. m-7（status クリア）・m-3（reconcile の分解能: mtime nanos + inode など）改善。
10. 文書修正: PROGRESS の 31→35 と日時、README の削除済み記述、NOTICE タイポと推移ライセンス文言、Cargo.toml の repository/homepage、README にソース提供 URL/タグ節、AC-7 スクショの来歴明記。
11. 設計との差分表に「ヘッダソートを P0 で先行実装」を追記。

## 10. 付録: 主な監査コマンド

```bash
# Build / Test（cargo 未実行を確認してから）
pgrep -af 'cargo|rustc' || true
cd work/gpui-file-explorer/app
CARGO_NET_OFFLINE=true cargo build          # exit 0, warnings 0
CARGO_NET_OFFLINE=true cargo test           # 35 passed, 0 failed

# ライセンス
sha256sum LICENSE ../../../security-review/Argus/LICENSE   # 一致
cargo metadata --format-version 1 --offline > /tmp/meta.json   # 808 packages
# 直接依存 10 件の license を突合（NOTICE と一致）

# SPDX
for f in src/*.rs Cargo.toml README.md NOTICE.md PROGRESS.md; do head -1 "$f"; done

# スクショ重複・実在
sha256sum screenshot/*.png

# コード監査（抜粋）
grep -nE 'std::fs::|is_dir\(\)|exists\(\)|canonicalize' src/main.rs
grep -nE 'create_dir|trash::delete|prompt\(|remove_file|remove_dir' src/*.rs
grep -nE 'unwrap\(|expect\(|panic!' src/*.rs          # 生産コードは main.rs:1221 のみ
grep -nE 'Command|println!|eprintln!' src/*.rs        # なし

# ユーザー設定・残留確認
cat ~/.config/gpui-file-explorer/settings.json
pgrep -af gpui-file-explorer || true
qdbus6 --literal org.kde.KWin /WindowsRunner org.kde.krunner1.Match "GPUI File Explorer"
```

---

**最終判定**: 試作（P0）として **受入可（acceptable as 試作）**。blocker なし。major 3 件（UI スレッド I/O 2・メニュー index 競合 1）と文書の不整合は P1 着手前に修正することを推奨。AC-8/9/10/12 は環境制約により未検証であることを明示した上で、AC-1/2/5 は私が直接確認、AC-3/4/6/7/11/13 は実装者証拠を確認済み（GUI 独立再実行は Lead の結果を正とする）。
