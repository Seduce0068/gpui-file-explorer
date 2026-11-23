---
type: ADR
title: "デュアルペイン（Double Commander 風）UI のアーキテクチャ選定"
description: "新要件「2 ディレクトリを左右に並べて同時操作」へのPaneState抽出と段階的導入の推奨方針"
tags:
  - gpui
  - dual-pane
  - architecture
  - refactor
  - licensing
generated:
  by: "adr-dualpane"
  at: "2026-11-22T23:12:01+09:00"
verified:
  by: "adr-dualpane"
  at: "2026-11-22T23:12:01+09:00"
status: draft
---

## コンテキスト

新たに「Double Commander のように左右 2 ペインでディレクトリを並べ、
コピー元とコピー先を同時に見て操作したい」という要件が追加された。
本 ADR は、ローカルに存在するコード・設計文書・発見タスクを実読して確認した
事実に基づき、実現選択肢を比較して推奨方針を記録する。
結論は**推奨（RECOMMENDATION）であり確定決定ではない**。
ユーザーの明示的な承認を得るまで `status: draft` を維持する。

## 検証済み事実

### 単一ペイン前提の状態設計（God Object）

- `app/src/main.rs` の `struct Explorer` は `cwd: PathBuf`、
  `all_entries` / `entries`、`selected` / `selection`、`history: History`、
  `scroll_handle: UniformListScrollHandle`、`watcher: DirWatcher`、
  `view` / `load` を 1 構造体に保持する。
  ペインごとの状態分離は存在しない。
  （行番号は main.rs の更新で頻繁に変わるため、以降はシンボル名で参照する。）
- `app/src/tabs.rs` の `struct Tab` は `cwd` と `History` のみ。
  `Pane` / `PaneState` 型は `app/src` 全体を grep しても存在しない。
- `app/src/main.rs` の `Explorer::render` 内の `body` は 1 つの `sidebar` と
  1 つの `flex_col(header + list_area)` だけを子に持つ。
  ルート要素の id は `explorer-root` の 1 つのみ。

### 設計上の先送りと既知のギャップ

- `DESIGN.md` の「決定（ADR-002）: P0 は単一 Entity」（§4.2）は
  「P0 は単一 Entity。タブ導入時に初めて分割すればよい」と明記し、
  ADR 表の ADR-002 行も同じ方針。
  `DESIGN.md` の「タブ / 複数ウィンドウ」行は P2 △ / P3 ◎ とし、
  分割ビューは明記されていない。
- `notes/task-discovery/round-2/competitor.md:37-41`（r2-cmp-01）は、
  「単一 Entity 前提の ADR-002 を再設計し、ペインごとの
  Listing/Selection/Watcher を管理する必要がある」と effort: L で記録済み。
- `notes/task-discovery/round-2/scenario.md:61`（r2-scn-01）も、
  「各ペインが独立した cwd・選択・スクロールを持つ分割レイアウト」を
  effort: L で提起し、ADR-002 の 1 状態からの拡張を指摘している。

### 入力・操作の配線

- 既存 `KeyAction` 列挙の `NextTab` / `PrevTab`、`FocusSidebar`、
  `ToggleSelection`、`CopyPath`。ディスパッチは
  `Explorer::on_key_down` の `match action`。
  アクティブペインを切替える列挙は存在しない。
- コンテキストメニューは `ContextMenuExt` を `Explorer::render` の row / cell
  要素へ適用（`.context_menu(...)`）。**as-built（2026-11-23）**: 旧本文の
  「マウス起点の UI であり、キー操作の代替ではない」という記述は古い。
  `Shift+F10` / `Menu` で同じメニューをキーボードから開ける実装済みであり、
  F1 ヘルプと設定「ショートカット」タブにも掲載されている
  （`app/src/settings_ui.rs` の `SHORTCUTS_DEFAULT_LINES`）。

### ライセンス

- アプリは `app/Cargo.toml:7` で `AGPL-3.0-or-later`。
- GPUI スタックは Apache-2.0（モバイル ADR の「結果とリスク」でも確認）。
  新規依存なしの方針を維持し、ライセンス互換の新たな論点は作らない。

## 選択肢

### A: `PaneState` の完全抽出（推奨）

`PaneState { cwd, entries, selection, scroll, history, watcher, view, load }`
を新設し、`Explorer` は 2 つの `PaneState` と `active: ActivePane` を持つ。
フォーカス・キー・ドロップをアクティブペインへルーティングし、
タブは各ペインの内側に入れ子にする。

- 利点: Double Commander との再現度が最も高い。
  各ペインが独立した cwd / 選択 / スクロール / 監視を持ち、
  r2-scn-01 の同期スクロールや r2-cmp-01 のコピー元/先の同時表示を
  同じモデルで表現できる。純粋関数によるテストが可能。
- 欠点: 工数が最大。`app/src/main.rs`（約 1.7 万行）の God Object に触れ、
  `update_watcher` / load generation / 選択 reconciliation / `on_drop`
  など密結合な配線をほぼ作り直す（r2-cmp-01 の effort: L と一致）。
  ADR-002 の「単一 Entity」決定を明示的に改訂する必要がある。

### B: 軽量セカンダリペイン（比較用ビュー）

プライマリ `Explorer` 状態は維持し、もう 1 つのディレクトリを
読み取り専用の「比較ペイン」として描画する。

- 利点: 変更範囲が小さく短期で出せる。既存のロード/監視を再利用でき、
  r2-scn-01 の「目視比較」は部分的に満たせる。
- 欠点: 状態分離ではないため、選択・スクロール・履歴が独立しない。
  ペイン間コピー/移動（本要件の核心）を支えられない。
  r2-cmp-01 が指摘する「コピー元と先を同時に見られる」価値を取りこぼす。

### C: 2 ウィンドウを KWin でタイル表示（非解決策）

アプリを変更せず、KWin のタイル機能で 2 インスタンスを左右に並べる。

- 利点: アプリ側の工数ゼロ。今日から可能。
- 欠点: プロセス間で状態も選択も共有されず、ドラッグ移動や
  ペイン間コピーができない。要件（1 アプリ内のデュアルペイン）を
  満たさないため、本 ADR では**解決策として扱わない**。
  比較対象としてのみ記録する。

### D: デュアルペインを先送りし、単一ペインを磨く

ADR-002 を維持し、現在の単一ペイン UX の改善を先に行う。

- 利点: リスクと工数が最小。既存の確定決定と整合する。
- 欠点: 既に r2-cmp-01 / r2-scn-01 で「既知のギャップ」と記録された
  要件を未対応のまま残す。新要件を満たさない。

## 推奨決定（未確定）

**選択肢 A を推奨する**。ただし一括実装ではなく、以下の段階で進める。
これは**推奨（RECOMMENDATION）であり確定決定ではない**。
選択肢 A は ADR-002（単一 Entity）の改訂と God Object の大規模 refactoring
を伴うため、**ユーザーの明示的な承認を得た後に `status` を更新する**。
現状は `draft` のまま。

### Phase 0: `PaneState` 抽出（feature flag とテスト付き）

- `PaneState` を GPUI 非依存の純粋状態として定義し、
  `Explorer` の既存フィールドを 1 ペイン分として移行する。
- feature flag（例: `dual-pane`）で既存動作を維持したまま切替え可能にする。
- 既存の単一ペイン挙動の回帰テストを先に整備する（ADR-002 の資産を守る）。

### Phase 1: 2 ペインレイアウトとアクティブペイン routing

- `Explorer::render` の `body` を sidebar + 2 つの pane へ拡張。
- `ActivePane { Left, Right }` を導入し、フォーカス・キー・`on_drop` を
  アクティブペインへ振り分ける。タブ bar は各ペイン内へ配置する。

### Phase 2: ペイン間コピー/移動と同期スクロール

- アクティブペインから他ペインへのコピー/移動、および r2-scn-02 の
  同期スクロール/リンク選択を追加する。
- `ContextMenuExt` の row メニューに「他ペインへコピー/移動」を追加し、
  マウスとキーボードの両経路を揃える。

### Phase 3: ペインごとのタブ

- `Tabs` を各 `PaneState` の内側へ入れ子にし、
  `NextTab` / `PrevTab` をアクティブペインに対して作用させる。
- `FocusSidebar` / `ToggleSelection` / `CopyPath` も対象ペインを明確化する。

## 受け入れ基準

- feature flag 無効時に、既存の単一ペイン挙動が回帰なしで動く。
- 2 ペイン表示時に、左右が独立した cwd / 選択 / スクロール / 履歴を
  保持し、片方の操作でもう片方の cwd が変わらない。
- `NextTab` / `PrevTab` / `ToggleSelection` / `CopyPath` が
  アクティブペインに対してのみ作用する。
- `Explorer::render` の `on_drop::<ExternalPaths>` がアクティブペインの
  cwd を宛先にする。
- ペイン間コピー/移動が Phase 2 で動作し、衝突時は既存の安全側
  （エラー表示）に従う。
- 新規依存を追加しない。ライセンスは AGPL-3.0-or-later を維持する。

## テスト戦略

- アクティブペイン routing は GPUI 非依存の純粋ヘルパー
  （例: `active_pane_for(action, state) -> PaneId`）として切出し、
  `app/src/tabs.rs` と同じく状態のみのテストにする。
- `PaneState` の生成・切替・履歴・選択の不変条件を
  `#[cfg(test)]` のユニットテストで検証する。
- feature flag の on/off 双方で既存 keymap テスト
  （`app/src/main.rs` の `keymap_*` ユニットテスト）を通す。
- GUI の目視確認は Wayland 実機で実施し、
  `xwininfo -root -tree` / `xprop` でウィンドウ構成を記録する。
- Rust コードに `unsafe` は導入しない（AGENTS.md §15 準拠）。
  FFI 境界の新設も行わないため、例外手続きは不要。

## 順序と他 ADR との関係

- モバイル対応 ADR（[本文][mobile]）の Phase 0
  （Windows パリティ）を先に実施する。**Windows / モバイル ADR の
  Phase 0 を優先し、本 ADR の Phase 0 はその後に着手する**。
- 本 ADR の Phase 0 は ADR-002 の明示的な改訂を伴う。
  `DESIGN.md` の ADR 表（ADR-002 行）は承認後に更新対象とする
  （本タスクでは設計文書を変更しない）。

## 結果とリスク

- 良い結果: コピー元/先を同時表示でき、r2-cmp-01 / r2-scn-01 の
  価値が解消される。タブもペイン単位で独立する。
- 悪い結果: 状態が 2 倍になり、監視・ロードの購読順序バグや
  メモリ/CPU 増が顕在化する。
- リスク: God Object への集中変更による回帰
  （Phase 0 の feature flag と回帰テストで緩和）、ユーザー未承認のまま
  実装を進めるリスク（`status: draft` と承認チェックリストで防止）。

## 確認が必要な事項（ユーザー判断）

- [ ] 選択肢 A（PaneState 抽出・ADR-002 改訂）を承認するか。
- [ ] モバイル ADR の Phase 0（Windows）を本 ADR より先に実施するか。
- [ ] Phase ごとの承認ゲート（Phase 1 以降は都度承認）を設けるか。

## 参照

- `app/src/main.rs`: `struct Explorer`, `Explorer::render`（`body` /
  `explorer-root` / `.context_menu(...)` / `on_drop::<ExternalPaths>`）、
  `enum KeyAction`、`Explorer::on_key_down` の `match action`
- `app/src/tabs.rs`: `struct Tab`
- `DESIGN.md`: 「タブ / 複数ウィンドウ」行、「決定（ADR-002）: P0 は単一
  Entity」（§4.2）、ADR 表の ADR-002 行
- `notes/task-discovery/round-2/competitor.md:37-41`（r2-cmp-01）
- `notes/task-discovery/round-2/scenario.md:61-62`（r2-scn-01, r2-scn-02）
- `app/Cargo.toml:7`, `app/src/main.rs:1`
- [モバイル対応 ADR][mobile]

[mobile]: adr-20261122-mobile-android-ios-support.md
