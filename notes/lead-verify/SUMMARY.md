# Lead 独立検証記録（2026-11-11 16:04〜16:08 JST）

実装者（fe-dev）の報告とは別に、Lead が最終状態の試作を再ビルド・再実行して確認した記録。

## 環境
- Kubuntu 2026.04 / KDE Plasma 6 / Wayland（ネイティブ）/ NVIDIA GTX 1660 SUPER / rustc 1.99.1
- 試作: `work/gpui-file-explorer/app/`（gpui-kit 0.9.1）

## 1. ビルド・テスト
```text
$ cd work/gpui-file-explorer/app
$ cargo test
test result: ok. 35 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.40s
```
- ビルド警告 0（実装者報告と一致。バイナリは既存の debug 165MiB を使用）。

## 2. Wayland 実行・ウィンドウ検証
- `/tmp/gpui-fe-lead-verify` に「ディレクトリ2 + file1..file10 + .hidden + notes.md + photo.png」を用意して起動。
- プロセス生存、**stderr/stdout 0 行**。
- KWin D-Bus（`org.kde.KWin /WindowsRunner org.kde.krunner1.Match` → `getWindowInfo`）:
  - `caption="GPUI File Explorer"`, `resourceClass="gpui-file-explorer"`, `resourceName="gpui-file-explorer"`
  - `width=1100`, `height=728`, `x=730`, `y=329`, `minimized=false`, `fullscreen=false`
  - ウィンドウ種別: Wayland（windows runner の backend が "wayland"）
- スクリーンショット（XDG Desktop Portal → ImageMagick でウィンドウ領域を crop）:
  - 起動直後: `notes/lead-verify/before.png`（表示 14 件、`file2.txt` が `file10.txt` より前 = 自然順、`.hidden` は非表示）
  - 起動中に `zz-watcher-proof.txt` を bash から作成 → 約3秒で一覧に追加（**14→15 件**、`notes/lead-verify/after.png`）
- 設定: 終了後 `~/.config/gpui-file-explorer/settings.json` に `last_dir=/tmp/gpui-fe-lead-verify` が保存され、スキーマ（version/sort/show_hidden/view）が DESIGN §8.1 と一致。検証後は元の設定に復元。

## 3. 終了・後始末
- `SIGTERM` → プロセス消滅、WindowsRunner の結果が空 `{}` に戻る、追加 stderr なし。
- 検証用ディレクトリ・ポータル保存スクショ・バックアップを削除。デスクトップ設定・システム変更なし。

## 4. ライセンス整合
- `app/LICENSE` = GNU AGPL v3 全文、`Cargo.toml` = `AGPL-3.0-or-later`、`src/*.rs` 全 6 ファイルに SPDX ヘッダ。
- `app/NOTICE.md` に gpui/gpui-pre/gpui-kit（Apache-2.0）・Lucide（ISC）・直接依存のライセンス表。誤記 `gpui_kits::assets::AllAssets` を `gpui_kit::assets::AllAssets` に修正（Lead）。
- `app/README.md` に AGPL 表示・ビルド/実行・キーマップ・既知の制限を記載。

## 5. 未検証（環境制約）
- キーボード/右クリック/削除確認ダイアログの**実押下 E2E**: ネイティブ Wayland への合成入力ツール（kdotool/ydotool/wtype）が本機に無いため未実施。コードはコンパイル検証 + データ層テストのみ。
- Windows/macOS 実機、100k 件性能、グリッド/P1機能。

## 判定
**試作として合格（P0スコープ）**。起動・一覧・ナビゲーション基盤・監視更新・設定永続化・ライセンスは Lead 自身の再検証で確認済み。入力 E2E と P1 は既知の未検証/未実装として README に明記されている。

---

## 6. 修正後の再検証（2026-11-11 16:34 JST、fe-dev の M-1/M-2/M-3 修正後）

fe-verifier の major 指摘（M-1 演出パスI/O、M-2 同期mkdir、M-3 メニューのindex捕捉）修正後の最終状態を Lead が再検証。

- `cargo test`: **36 passed / 0 failed**（0.40s、修正で新規テスト追加）
- 起動: ネイティブ Wayland、stderr **0 行**、KWin `caption="GPUI File Explorer"` / `resourceClass="gpui-file-explorer"` / 1100x728 / fullscreen=false
- 監視: 起動中に外部から `zz-postfix-watcher.txt` を作成 → 一覧に反映（14→15件、[postfix-after.png](postfix-after.png) のステータス右に「変更を検出しました」）
- 終了: SIGTERM → プロセス消滅・ウィンドウ一覧から消滅（`{}`）、stderr 追加 0 行
- 後始末: 検証用 dir / ログ / `~/.config/gpui-file-explorer/` / ポータル保存スクショを削除
- 補足: M-2/M-3 の追加修正により、メニューはパスで対象を再解決、サイドバーの `dirs::*_dir()` は起動時キャッシュに、新規フォルダは background executor 実行になった（fe-dev 報告・コードで確認）。

**最終判定: 試作（P0）として合格。** 残る既知の制限（入力E2E・open/reveal 失敗通知・Degraded 表示・fsync・UI層テスト・Windows/macOS 未検証）は `app/README.md`「既知の制限」と `notes/verify-report.md` に明記。
