# GPUI API調査ノート（クロスプラットフォーム・ファイルエクスプローラー自作のためのAPI棚卸し）

- 調査対象（一次情報・ローカル全文）: `.tmp-deps-audit/crates/up-gpui-pre/` = crates.io `gpui-pre` 0.3.9（zed rev 93d7456 のスナップショット、Apache-2.0）
- 位置づけ: GPUIはpre-1.0。APIはバージョン間で破壊的変更がある。本ノートの行番号は上記スナップショット時点のもの。
- crates.io現行: `gpui-pre` 0.3.12（zed@d2a4fb6, 2026-11-09）。差異が助言に影響する箇所のみ本文で注記する。
- ステータス: ドラフト完成（2026-11-11）。§10に未確認事項あり。
- 関連ノート: `work/gpui-file-explorer/notes/fs-cross-platform-notes.md`（fs/プラットフォーム側の調査）。最終まとめは `work/gpui-file-explorer/RESEARCH.md`（Lead）。

## 0. 要点サマリ

- 起動: `gpui_platform::application().run(|cx: &mut App| ...)` が正規入口（README:15,21）。`Application::run` = src/app.rs:249、`App::open_window` = src/app.rs:1500、`Render` = src/element.rs:164。
- 一覧: `uniform_list(id, count, |Range, &mut Window, &mut App| -> Vec<_>)` = src/elements/uniform_list.rs:22。`scroll_to_item(ix, ScrollStrategy)` = :150。可変高さ `list(ListState, ..)` = src/elements/list.rs:24、`ListState::new(count, ListAlignment, overdraw)` = :314。
- 入力: `MouseDownEvent.click_count` = src/interactive.rs:159（ダブルクリック）。`on_key_down` = div.rs:496、`track_focus` = div.rs:788。
- D&D: `on_drag` = div.rs:615 / `on_drop` = div.rs:568 / `external_drag_payload` = div.rs:643。OSからのファイルドロップは `ExternalPaths`（interactive.rs:694）と `FileDropEvent`（:737）。`ClipboardEntry::ExternalPaths` = platform.rs:3016 でファイルコピー可。
- ダイアログ: `prompt_for_paths`（app.rs:1891）/`prompt_for_new_path`（:1904）/`reveal_path`（:1913）/`open_with_system`（:1918）/`open_url`（:1809）。
- メニュー: `App::set_menus` = app.rs:2788、`Menu` = platform/app_menu.rs:4。
- コンテキストメニューwidgetはcoreに無い（grep 0件）。**`WindowKind::AnchoredPopup` + `popup::PopupOptions { grab: true }`**（platform/popup.rs:16-52）がWayland対応の本命。マウスダウンから開くこと（clickハンドラだとgrab拒否）。または `deferred`+`anchored` で自作。
- 非同期: `Context::spawn` = app/context.rs:230、`background_spawn` = app.rs:3335。`cx.notify` = app/context.rs:221。`BackgroundExecutor::timer` = executor.rs:183。
- ダイアログ: `PathPromptOptions` は `files`/`directories`/`multiple`/`prompt` のみで**拡張子フィルタ無し**（platform.rs:2800-2811）。`reveal_path` は**ある**（app.rs:1913）。
- テキスト入力: **IMEの配管のみ**（`EntityInputHandler` — input.rs:13 → `ElementInputHandler` → `Window::handle_input`）。入力欄widgetはcoreに無く自作（リネーム機能の主要コスト）。
- アイコン: `AssetSource { load, list }`（assets.rs:13-26）+ `svg()`/`img()`。OS MIMEアイコン取得APIは無いのでSVGセット同梱。
- 詳細は §1-§9、未確認は §10、根拠は §11。

## 1. スタンドアロン起動（bootstrap）

### 1.1 crate名とlib名（最初の落とし穴）

- パッケージ名 `gpui-pre` だが **lib target name は `gpui`**（`Cargo.toml:83-85`: `[lib] name = "gpui"` / `path = "src/gpui.rs"`）。同梱examplesも `use gpui::...`。
- `[package.metadata.gpui-pre]`: `zed-crate = "gpui"`, `zed-version = "0.2.2"`, `zed-rev = "93d745668805029476e6d7fbb19652116fed51de"`（`Cargo.toml:45-48`）。
- デフォルトfeature: `default = ["font-kit", "wayland", "x11", "windows-manifest"]`（`Cargo.toml:58-63`）。
- 起動は別crate `gpui_platform`（crates.io名 `gpui-pre-platform`）。README:15「Everything in a standalone GPUI app starts with an `Application`...」。Cargo例はREADME:9-13。
- gpui-pre利用時は `gpui = { package = "gpui-pre", version = "0.3.12" }` のrenameが要る可能性（未検証・§10）。

### 1.2 最小起動コード（`examples/hello_world.rs:96-119`）

```rust
use gpui::{App, Bounds, Context, Window, WindowBounds, WindowOptions, div, prelude::*, px, rgb, size};
use gpui_platform::application;

struct Root { /* state */ }
impl Render for Root {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().flex_col().bg(rgb(0x505050)).size(px(500.0))
    }
}
fn main() {
    application().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(500.), px(500.0)), cx);
        cx.open_window(
            WindowOptions { window_bounds: Some(WindowBounds::Windowed(bounds)), ..Default::default() },
            |_, cx| cx.new(|_| Root {}),
        ).unwrap();
        cx.activate(true);
    });
}
```

### 1.3 Application / App 主要API

- `pub struct Application(Rc<AppCell>);` — `src/app.rs:148` / `impl Application` — `:177`
  - `with_platform(Rc<dyn Platform>)` — `:179` / `new_inaccessible(platform)` — `:194`（a11y無効で軽量化）
  - `with_assets(impl AssetSource)` — `:203` / `with_quit_mode(QuitMode)` — `:228`付近 / `with_windowing(WindowingRequest)` — `:240`付近
  - `run<F>(self, F) where F: 'static + FnOnce(&mut App)` — `:249` / `run_embedded(..) -> ApplicationHandle` — `:275`
  - `on_open_urls(..)` / `on_reopen(..)` — URL起動・再起動フック
- `App::open_window<V: Render>(&mut self, options: WindowOptions, build: impl FnOnce(&mut Window, &mut App) -> Entity<V>) -> anyhow::Result<WindowHandle<V>>` — `src/app.rs:1500`
- `activate(bool)` — `:1536` / `quit()` — `:1231` / `windows()` — `:1474`付近 / `bind_keys(..)` — `:2606` / `on_window_closed(..)` — `:2751` / `focus_handle()` — `:3047`
- `QuitMode { Default, LastWindowClosed, Explicit }` — `src/app.rs:456-466`（DefaultはmacOS=Explicit、他=LastWindowClosed）

### 1.4 WindowOptions / TitlebarOptions / WindowBounds / WindowKind

- `pub struct WindowOptions` — `src/platform.rs:2491`。フィールド: `window_bounds`, `titlebar`, `focus`, `show`, `kind: WindowKind`, `is_movable`, `app_owns_titlebar_drag`, `inactive_frame_interval`, `is_resizable`, `is_minimizable`, `display_id`, `window_background: WindowBackgroundAppearance`, `app_id`, `window_min_size`, `window_decorations: Option<WindowDecorations>`, `icon`（X11のみ）, `tabbing_identifier`（macOS）
- `Default for WindowOptions` — `src/platform.rs:2671`付近（`titlebar: Some(..)`, `focus: true`, `show: true`, `kind: Normal`, `is_resizable: true`）
- `pub struct TitlebarOptions { title: Option<SharedString>, appears_transparent: bool, traffic_light_position: Option<Point<Pixels>> }` — `src/platform.rs:2692`
  - **重要**: `appears_transparent` はmacOS/Windowsのみ。**LinuxのCSD/SSDは `WindowOptions::window_decorations`**（doc `src/platform.rs:2694-2699`）。KDE Waylandで自前タイトルバーにする場合はこちら。
- `pub enum WindowBounds { Windowed(Bounds<Pixels>), Maximized(..), Fullscreen(..) }` — `src/platform.rs:2628`付近; `WindowBounds::centered(size, cx)` — `:2656`付近
- `pub enum WindowKind { Normal, PopUp, AnchoredPopup(popup::PopupOptions), Floating, LayerShell(..), Dialog }` — `src/platform.rs:2714`付近
  - `AnchoredPopup` doc: 「親ウィンドウ基準のネイティブポップアップ。メニュー/コンボボックス/**コンテキストメニュー**/ツールチップ用。サイズは `WindowOptions::window_bounds` から取得しoriginは無視。`popup::PopupOptions` で配置指定。未対応PFは `PopupNotSupportedError`」→ §5の鍵。

### 1.5 終了・ライフサイクル（`examples/on_window_close_quit.rs:49-113` が雛形）

```rust
actions!(example, [CloseWindow]);
impl Render for ExampleWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().on_action(|_: &CloseWindow, window, _| { window.remove_window(); })
           .track_focus(&self.focus_handle)
    }
}
fn run_example() {
    application().run(|cx: &mut App| {
        cx.bind_keys([KeyBinding::new("cmd-w", CloseWindow, None)]);
        cx.on_window_closed(|cx, _id| { if cx.windows().is_empty() { cx.quit(); } }).detach();
        cx.open_window(WindowOptions::default(), |window, cx| {
            cx.activate(false);
            cx.new(|cx| { let f = cx.focus_handle(); f.focus(window, cx); ExampleWindow { focus_handle: f } })
        }).unwrap();
    });
}
```
`window.remove_window()` — 同:21。`FocusHandle` — `src/window.rs:538`（`tab_index`/`tab_stop`/`focus`/`is_focused` — `:583`-`:640`付近）。

### 1.6 actions! / Action / KeyBinding

```rust
actions!(file_explorer, [OpenSelected, GoUp, NewFolder, Delete]);
#[derive(Clone, PartialEq, Debug, gpui::Action)]
#[action(namespace = file_explorer)]
pub struct Rename { pub name: String }
```
- `actions!` — `src/action.rs:25-57`（namespace付き/なし）。`Action` derive が付く。データ付きActionは `Clone + PartialEq` +（`no_json` でなければ）`serde::Deserialize` + `schemars::JsonSchema` 必須（`src/action.rs:62-80`）。
- `KeyBinding::new(keystrokes: &str, action: A, context: Option<&str>)` — `src/keymap/binding.rs:32`（パース失敗でpanic）。安全版 `KeyBinding::load(..)` — `:49`。複数ストロークは空白区切り（`"ctrl-k ctrl-o"`）。
- `cx.bind_keys([...])` — `src/app.rs:2606`。context述語 — `src/keymap/binding.rs:34`。
- キー配送の詳細は `docs/key_dispatch.md`、`Context` の使い方は `docs/contexts.md`。

### 1.7 ビューとEntity

- `pub trait Render` — `src/element.rs:164` / `Element` — `:52` / `IntoElement` — `:146` / `RenderOnce` — `:180`（`#[derive(IntoElement)]`）
- `Entity<T>` — `src/app/entity_map.rs:435`
- prelude — `src/prelude.rs:5-10`

## 2. 仮想化リスト / カスタムElement

### 2.1 uniform_list（固定高さ。大量ファイル一覧の第一候補）

```rust
pub fn uniform_list<R>(
    id: impl Into<ElementId>,
    item_count: usize,
    f: impl 'static + Fn(Range<usize>, &mut Window, &mut App) -> Vec<R>,
) -> UniformList where R: IntoElement
```
— `src/elements/uniform_list.rs:22-32`。可視範囲のみ `f(range)` が呼ばれる。

```rust
uniform_list("entries", 50, cx.processor(|_this, range, _window, _cx| {
    let mut items = Vec::new();
    for ix in range {
        items.push(div().id(ix).px_2().cursor_pointer()
            .on_click(move |_event, _w, _cx| println!("clicked {ix}"))
            .child(format!("Item {}", ix + 1)));
    }
    items
})).h_full()
```
— `examples/uniform_list.rs:20-46`。`cx.processor(..)` は `Context<Self>` から `Fn` を作るヘルパ。

- `.track_scroll(&UniformListScrollHandle)` — `src/elements/uniform_list.rs:686`
- `UniformListScrollHandle::new()` — `:132`; `scroll_to_item(ix, ScrollStrategy)` — `:150`; `scroll_to_item_strict` — `:163`; `scroll_to_bottom` — `:252`
- `ScrollStrategy { Top, Center, Bottom, Nearest }` — `:87-106`
- 1px単位の `ScrollHandle` — `src/elements/div.rs:4249`（`offset()`, `max_offset()`, `top_item()`, `bottom_item()`, `scroll_to_item(ix)`, `scroll_to_bottom()`, `set_offset(..)`）
- `div().overflow_y_scroll()` — `src/elements/div.rs:1535` / `.overflow_scroll()` — `:1522` / `.track_scroll(&ScrollHandle)` — `:1549`

### 2.2 list（可変高さ・行ごとにサイズが違う場合）

```rust
pub fn list(state: ListState, render_item: impl FnMut(usize, &mut Window, &mut App) -> AnyElement + 'static) -> List
```
— `src/elements/list.rs:24-30`。`ListState::new(item_count: usize, alignment: ListAlignment, overdraw: Pixels) -> Self` — `:314`。`ListAlignment { Top, Bottom }` — `:164`。

主な `ListState` メソッド（すべて `src/elements/list.rs`）:
- `.measure_all()` — `:336` / `.with_uniform_item_height(height)` — `:347`
- `.reset(element_count)` — `:355` / `.reset_with_uniform_height(..)` — `:372` / `.remeasure()` — `:400` / `.remeasure_items(range)` — `:412`
- `.item_count()` — `:478` / `.is_scrolled_to_end()` — `:484`
- `.splice(old_range, count)` — `:503` / `.splice_focusable(..)` — `:511`（**差分更新。ファイル追加/削除/リネーム時にこれを使う**）
- `.set_scroll_handler(handler)` — `:552`（スクロールイベント取得）/ `.logical_scroll_top()` — `:560`
- `.scroll_by(distance)` — `:565` / `.scroll_to_end()` — `:603` / `.scroll_to(ListOffset)` — `:660` / `.scroll_to_reveal_item(ix)` — `:677`
- `.bounds_for_item(ix)` — `:711` / `.viewport_bounds()` — `:799`
- スクロールバー自前描画用: `.max_offset_for_scrollbar()` `:772`, `.scroll_px_offset_for_scrollbar()` `:781`, `.scrollbar_drag_started/ended` `:743/:751`, `.set_offset_from_scrollbar(point)` `:766`
- 使用例: `examples/list_example.rs:20-30,88-120`（自前スクロールバー付き）

### 2.3 グリッド/サムネイル表示（アイコンビュー）

- coreに「グリッド仮想化」専用要素は無い（`grid_layout` の例はCSS Gridの配置例 `examples/grid_layout.rs`）。
- 現実的な実装: **列数を決めて「行」を1アイテムとして `uniform_list` に流す**（`item_count = ceil(n / cols)`）。行の高さが固定ならこれが最速で、`scroll_to_item` も行単位で効く。
- 拡大縮小（サムネイルサイズ変更）は行高さ・列数が変わるので、`ListState`（可変高さ）か、`uniform_list` の再構築で対応。
- 完全自由なレイアウト（可変セル幅・折返し）は §2.4 のカスタム `Element`。

### 2.4 カスタム Element の最小実装

`pub trait Element: 'static + IntoElement` — `src/element.rs:52-150`。3段階:

```rust
impl Element for MyGrid {
    type RequestLayoutState = ();      // request_layout が返し prepaint/paint に &mut で渡る
    type PrepaintState = ();           // prepaint が返し paint に &mut で渡る
    fn id(&self) -> Option<ElementId> { None }
    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> { None }
    fn request_layout(&mut self, id: Option<&GlobalElementId>, inspector: Option<&InspectorElementId>,
                      window: &mut Window, cx: &mut App) -> (LayoutId, Self::RequestLayoutState) {
        // window.request_layout(style, children, cx) でtaffyにレイアウト要求
        todo!()
    }
    fn prepaint(&mut self, id: Option<&GlobalElementId>, inspector: Option<&InspectorElementId>,
                bounds: Bounds<Pixels>, request_layout: &mut Self::RequestLayoutState,
                window: &mut Window, cx: &mut App) -> Self::PrepaintState {
        // ヒットボックス登録・計測。window.insert_hitbox(bounds, HitboxBehavior::Normal)
        todo!()
    }
    fn paint(&mut self, id: Option<&GlobalElementId>, inspector: Option<&InspectorElementId>,
             bounds: Bounds<Pixels>, request_layout: &mut Self::RequestLayoutState,
             prepaint: &mut Self::PrepaintState, window: &mut Window, cx: &mut App) {
        // window.paint_quad(quad(...)) や child.paint(window, cx) で描画
        todo!()
    }
}
```
- a11yフック: `fn a11y_role(&self) -> Option<accesskit::Role>` / `fn write_a11y_info(&self, &mut accesskit::Node)` / `fn a11y_synthetic_children(..)` — `src/element.rs:115-140` 付近（§8）。
- 実装に使う `Window` API（確認済み）: `window.request_layout(style, child_layout_ids, cx) -> LayoutId`（実例: `src/elements/anchored.rs:117`）、`window.insert_hitbox(bounds, HitboxBehavior::Normal) -> Hitbox` — `src/window.rs:5295`、`window.paint_quad(PaintQuad)` — `src/window.rs:4684`、子の描画は `child.paint(window, cx)`。
- 既存の実装例: `src/elements/uniform_list.rs` の `impl Element for UniformList`（`:266`付近〜）が「仮想化＋スクロール＋スクロールバー」の完全な参考実装。`src/elements/list.rs` も同様。
- `deferred` / `anchored`（§5）は通常の `div` に組み込めるので、まず `uniform_list` + `deferred` でグリッドを作り、足りない場合だけ Element 実装に踏み込むのが現実的。

## 3. 入力・選択（マウス/キー/フォーカス/ホバー）

### 3.1 マウス（クリック・ダブルクリック・右クリック）

- `InteractiveElement::on_mouse_down(&mut self, button: MouseButton, listener: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static)` — `src/elements/div.rs:126-130`（bubbleフェーズのみ、hover判定付き）
- バリエーション: `on_any_mouse_down` — `:162`、`capture_any_mouse_down` — `:146`（captureフェーズ）、`on_mouse_down_out` — `:263`
- `pub struct MouseDownEvent { button: MouseButton, position: Point<Pixels>, modifiers: Modifiers, click_count: usize }` — `src/interactive.rs:148-160`
  - **ダブルクリック判定**: `click_count == 2`（`:159`）。GPUIは自前でダブルクリックを判定してくれる。
  - `MouseUpEvent` も同様（`src/interactive.rs` 内）。
- `on_click(listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static)` — `src/elements/div.rs:591`
  - `ClickEvent` は struct ではなく **enum**: `Mouse(MouseClickEvent)` / `Keyboard(KeyboardClickEvent)` / `Touch(TouchClickEvent)` — `src/interactive.rs:290-296`
  - 共通アクセサ: `.click_count() -> usize`（`Mouse` は `event.up.click_count`、`Touch` は `tap_count`）— `src/interactive.rs:426-428`、`.modifiers()` — `:310`付近、`.position()` — `:330`付近、`.is_keyboard()` / `.first_mouse()` など — `:343`-`:436`
  - ダブルクリックは `on_mouse_down` の `MouseDownEvent.click_count == 2` か、`on_click(|e, ..| if e.click_count() == 2 { ... })` のどちらでも取れる。
- `on_aux_click`（中/右クリック） — `src/elements/div.rs:607`付近
- 例: `examples/uniform_list.rs:35-38` の `.on_click(...)`、`examples/drag_drop.rs` にドラッグ開始例。

### 3.2 キー入力とAction

- `on_key_down(listener: impl Fn(&KeyDownEvent, &mut Window, &mut App) + 'static)` — `src/elements/div.rs:496`
- `pub struct KeyDownEvent { keystroke: Keystroke, is_held: bool, prefer_character_input: bool }` — `src/interactive.rs:25-35`
  - 修飾キーは `event.keystroke.modifiers`（`Modifiers`）、キーは `event.keystroke.key` / `key_char`。
- メニュー的操作は **Action方式が推奨**: `actions!` で宣言 → `cx.bind_keys([KeyBinding::new("ctrl-c", Copy, Some("FileExplorer"))])` → 要素に `.on_action(|action: &Copy, window, cx| ...)`（`src/elements/div.rs` の `on_action`）。要素は `.track_focus(&focus_handle)` されている必要がある（キー配送はフォーカス要素から）。
- `.key_context("FileExplorer")` でコンテキストを付けると、`KeyBinding::new(.., Some("FileExplorer"))` の述語とマッチする（`KeyBindingContextPredicate` — `src/keymap/binding.rs:34`。詳細は `docs/key_dispatch.md`）。
- フォーカス: `track_focus(&FocusHandle)` — `src/elements/div.rs:788`、`tab_stop(bool)` — `:800`、`FocusHandle::tab_index`/`tab_stop` — `src/window.rs:583/595`。

### 3.3 ホバー・カーソル

- `on_hover(listener: impl Fn(&bool, &mut Window, &mut App) + 'static)` — `src/elements/div.rs:675`（true=enter, false=leave。レイアウト変化でも発火）
- `hover_listener_mode(HoverListenerMode)` — `src/elements/div.rs:694`付近（既定ではキー入力中はホバー抑止。`InputModalityIndependent` で常時）
- カーソル: `.cursor_pointer()` などのスタイルメソッド（`examples/uniform_list.rs:36` で使用）。低レベルは `CursorStyle` enum（`src/platform.rs:2950`付近、`DragCopy`/`ContextualMenu` 等）と `Window` のカーソル設定API。
- ツールチップ: `.tooltip(|window, cx| AnyView)` / `.hoverable_tooltip(...)` — `src/elements/div.rs:706`付近/`:722`付近（core機能。§5も参照）

## 4. ドラッグ＆ドロップ・クリップボード・ファイル操作ダイアログ

### 4.1 GPUI内D&D（アプリ内の移動/コピー）

※タスク指示にあった `DragAndDrop` という型は**このスナップショットには存在しない**（`grep -rn "DragAndDrop" src/` = 0件）。実APIは以下:

- `on_drag<T, W>(&mut self, value: T, constructor: impl Fn(&T, Point<Pixels>, &mut Window, &mut App) -> Entity<W> + 'static)` — `src/elements/div.rs:615-637`
  - `value` がドラッグされるデータ。`constructor` は**ドラッグ中に追従表示するプレビューView**（`Entity<W: Render>`）を作る。同一要素に2回呼ぶとpanic。
- `external_drag_payload<T>(&mut self, resolver: impl Fn(&T, &mut Window, &mut App) -> Option<ExternalDragPayload> + 'static)` — `src/elements/div.rs:643-668`
  - **ウィンドウ外へドラッグが出た時にOSへ渡すペイロード**を返す。`on_drag` の後に、同じ型 `T` で呼ぶ。
- `on_drop<T: 'static>(&mut self, listener: impl Fn(&T, &mut Window, &mut App) + 'static)` — `src/elements/div.rs:568-575`（ドロップ受付。型 `T` でダウンキャスト）
- `can_drop(predicate: impl Fn(&dyn Any, &mut Window, &mut App) -> bool + 'static)` — `src/elements/div.rs:580`付近
- `on_drag_move`（`DragMoveEvent<T>` — `src/elements/div.rs:67`）でドラッグ中の座標追従
- 実例: `examples/drag_drop.rs:1-130`

### 4.2 OSからのファイルドロップ（エクスプローラー必須）

- `pub struct ExternalPaths(pub SmallVec<[PathBuf; 2]>)` — `src/interactive.rs:694`。`.paths() -> &[PathBuf]` — `:698`。
- `pub enum FileDropEvent { Entered { position, paths: ExternalPaths }, Pending { position }, Submit { position }, Exited, Ended }` — `src/interactive.rs:737-759`
  - **重要**: ドロップされたパスは `Entered` でのみ渡り、`Submit` には位置しか無い。→ `Entered` でパスを保持し、`Submit`/`Exited` で確定/破棄する実装が必要。
  - 配送は `Window` が `PlatformInput::FileDrop` として行う — `src/window.rs:5656-5707`。
  - **OSファイルドロップは「内部D&D」に変換される**（重要）: `FileDropEvent::Entered` で `cx.active_drag = Some(AnyDrag { value: Arc::new(paths.clone()), view: cx.new(|_| paths), .. })` を作る（`src/window.rs:5659-5670`）→ **`on_drop::<ExternalPaths>` で受け取れる**。`Pending` は `MouseMove`、`Submit` は `MouseUp`（`click_count: 1`）に変換される（`:5685-5694`）ので、ドロップ時に `on_click` も発火しうる点に注意（ファイル行のクリック処理と競合しないよう `on_drop` 側でフラグを立てる等）。
  - `on_file_drop_exit(listener)` — `src/elements/div.rs:338`（ウィンドウ外へ出た時の後始末）。
- ドラッグ中のOS側プレビュー: `impl Render for ExternalPaths` は `Empty` を返す（「platform will render icons for the dragged files」— `src/interactive.rs:728-733`）。

### 4.3 アプリ外へのファイルドラッグ（explorer → デスクトップ等）

- `pub enum ExternalDragPayload { Files(FileDragPaths) }` — `src/interactive.rs:705-709`
- `pub struct FileDragPaths(SmallVec<[(PathBuf, bool); 2]>)`（bool=ディレクトリか） — `:713-714`
  - `FileDragPaths::new(entries: impl IntoIterator<Item = (PathBuf, bool)>)` — `:718` / `.entries()` — `:723`
- 使用例（推定コード）:
  ```rust
  .on_drag(FileDragItem { paths }, |item, offset, window, cx| {
      cx.new(|_| DragPreview { .. })   // 追従表示
  })
  .external_drag_payload(|item: &FileDragItem, _window, _cx| {
      Some(ExternalDragPayload::Files(FileDragPaths::new(
          item.paths.iter().map(|p| (p.clone(), p.is_dir())))))
  })
  ```

### 4.4 クリップボード（ファイルのコピー/貼り付け）

- `pub struct ClipboardItem { pub entries: Vec<ClipboardEntry> }` — `src/platform.rs:2969-2972`
- `pub enum ClipboardEntry { String(ClipboardString), Image(Image), ExternalPaths(crate::ExternalPaths) }` — `src/platform.rs:3010-3017`
  - **`ExternalPaths` 変数がある＝ファイルパスのコピーはcoreで表現可能**（`text/uri-list` 相当をプラットフォーム実装が担う。Linux実装の詳細は未確認・§10）。
  - `ClipboardItem::new_string(text)` — `:3021` / `new_string_with_metadata(..)` — `:3030` / `new_string_with_json_metadata(..)` — `:3040` / `new_image(..)` — `:3050`付近
  - `From<String>`/`From<Image>`/`From<ClipboardEntry>` はあるが **`From<ExternalPaths>` は無い**（`src/platform.rs:3104-3135`）→ `ClipboardItem { entries: vec![ClipboardEntry::ExternalPaths(paths)] }` と手組みする。
- 読み書き: `App::read_from_clipboard() -> Option<ClipboardItem>` — `src/app.rs:1722` / `read_from_clipboard_async()` — `:1733` / `write_to_clipboard(item)` — `:1750`
- 実装層: `Platform::read_from_clipboard` — `src/platform.rs:573` / `write_to_clipboard` — `:574` / `read_from_clipboard_async` — `:583`
- **注意**: 切り取り/コピーの「アプリ内状態」は自前で保持する必要がある（OSクリップボードだけでは切り取りのグレー表示を管理できない）。

### 4.5 ネイティブダイアログ

- `App::prompt_for_paths(&self, options: PathPromptOptions) -> oneshot::Receiver<Result<Option<Vec<PathBuf>>>>` — `src/app.rs:1891-1897`
  - doc: 「プラットフォームモーダルでパス選択。選択はoneshotで非同期通知、キャンセルは `None`。**Linuxではファイルピッカーを開けない場合エラーを返しうる**」
- `App::prompt_for_new_path(&self, directory: &Path, suggested_name: Option<&str>) -> oneshot::Receiver<Result<Option<PathBuf>>>` — `src/app.rs:1904-1911`
- `App::reveal_path(&self, path: &Path)` — `src/app.rs:1913`（**ファイルマネージャで表示**。`Platform::reveal_path` — `src/platform.rs:454`）
- `App::open_with_system(&self, path: &Path)` — `src/app.rs:1918`（既定アプリで開く）
- `App::open_url(&self, url: &str)` — `src/app.rs:1809`（`Platform::open_url` — `src/platform.rs:440`）
- `PathPromptOptions` — `src/platform.rs:2800-2811`: フィールドは `files: bool`, `directories: bool`, `multiple: bool`, `prompt: Option<SharedString>` の4つのみ。**拡張子フィルタ（file filter）指定は無い**（ネイティブダイアログで拡張子を絞るAPIはこのスナップショットには存在しない）。
  - 例: フォルダ選択 → `PathPromptOptions { files: false, directories: true, multiple: false, prompt: Some("フォルダを選択".into()) }`
- `PromptLevel` / `PromptButton`（`src/platform.rs:2815`, `:2828`）は通知・確認ダイアログ用の別系統。
- ダイアログ結果は `oneshot::Receiver` → `cx.spawn` で await する（§6）。GUIスレッドをブロックしない。

## 5. オーバーレイ（anchored / deferred / ポップオーバー / コンテキストメニュー）

### 5.1 coreのオーバーレイ要素

- `pub fn deferred(child: impl IntoElement) -> Deferred` — `src/elements/deferred.rs:7`（レイアウトを遅延し、通常フローから外して描画。スクロール領域に切られないオーバーレイ用）
- `pub fn anchored() -> Anchored` — `src/elements/anchored.rs:27`（`Anchored` — `:16`、`AnchoredState` — `:10`）。`Anchored` は `ParentElement` なので `.child(...)` で中身を入れる。ビルダ:
  - `.anchor(Anchor)` — `:40`（`Anchor::TopLeft` 等、`src/geometry.rs:2168`）
  - `.position(Point<Pixels>)` — `:47`（ウィンドウ座標。デフォルトは描画位置）
  - `.offset(Point<Pixels>)` — `:54`（「PopoverMenu のように要素にアンカーしてオフセットする」とdocに明記）
  - `.position_mode(AnchoredPositionMode)` — `:62`（`Window`=ウィンドウ座標 / `Local`=親要素相対）
  - `.snap_to_window()` — `:68` / `.snap_to_window_with_margin(edges)` — `:74`（画面端で見切れないようスナップ）
  - 実装は `position: Absolute` の要素として `window.request_layout(...)` — `:111-117`（§2.4のカスタムElement実装の参考になる）
- ツールチップ: `.tooltip(|window, cx| AnyView)` / `.hoverable_tooltip(...)` — `src/elements/div.rs:706`付近/`:722`付近。要素に紐づく形でcoreが提供（自前 `deferred` 不要）。
- `WindowKind::AnchoredPopup(popup::PopupOptions)` — `src/platform.rs` の `WindowKind`（`:2714`付近）; `PopupOptions` — `src/platform/popup.rs:16-53`
  - doc `:6-14`: 「親ウィンドウ基準のポップアップ（メニュー・ドロップダウン・**コンテキストメニュー**・ツールチップ用）。アンカー矩形に対して配置し、最終位置はプラットフォームが解決する。**コンポジタがウィンドウ配置を持つWaylandでも、絶対座標系のプラットフォームでも動く**」。サイズは `WindowOptions::window_bounds` から取り origin は無視。
  - フィールド: `parent: AnyWindowHandle` / `anchor_rect: Bounds<Pixels>`（親ウィンドウ座標。例: 開いたボタンのbounds）/ `anchor: PopupAnchor`（Center, Top, Bottom, TopLeft, BottomLeft, TopRight, BottomRight 等 — `:57-77`）/ `gravity: PopupGravity`（`:84-104`）/ `constraint_adjustment: PopupConstraintAdjustment`（bitflags: SLIDE_X/Y, FLIP_X/Y, RESIZE_X/Y, NONE。画面外補正。`:106-120`）/ `offset: Point<Pixels>` / `grab: bool`
  - **`grab: true` がメニュー挙動の鍵**（doc `:39-52`）: キーボードフォーカスを取り、外側クリック・Escで自動dismiss。**マウスダウン ハンドラから開くこと**（clickハンドラからだとgrabが拒否される）。ネストしたgrabは逆順で閉じる。自アプリ内の別ウィンドウへのクリックは自動dismissされないので自前で閉じる。

### 5.2 コンテキストメニューwidgetは core に「無い」

- `grep -rln "context_menu\|ContextMenu" src/` → **0件**（`src/` 全体）。`App::show_context_menu` のようなAPIも無い。
- したがって、右クリックメニューは次のいずれかで自作する:
  1. **`deferred` + `anchored`** で通常要素ツリー内にメニューを描く（最も簡単。クリック外閉じは `on_mouse_down_out` — `src/elements/div.rs:263` で実装）。Zedの `PopoverMenu` もこの系統（`anchored` のdoc `src/elements/anchored.rs:53` に「such as in PopoverMenu」とある）。
  2. **`WindowKind::AnchoredPopup` + `PopupOptions { grab: true }`** で別ウィンドウとして出す（Waylandのxdg_popupを正しく使える。影・画面外補正・自動dismissが効く。**Linux Waylandではこちらが本命**）。マウスダウンから開くこと。
  3. **gpui-component のメニュー部品**を使う（§9）。GPUIの上に載るサードパーティ実装。
- OSネイティブの**アプリケーションメニューバー**は別API: `App::set_menus(menus: impl IntoIterator<Item = Menu>)` — `src/app.rs:2788`、`Platform::set_menus(menus: Vec<Menu>, keymap: &Keymap)` — `src/platform.rs:488`、`Menu` — `src/platform/app_menu.rs:4`。
  - `Menu` / `MenuItem` の詳細は §8.2（`src/platform/app_menu.rs:4-189` で確認済み）。使用例: `examples/set_menus.rs`。
  - Linux（KDE Wayland）での挙動（グローバルメニューバー vs ウィンドウ内メニュー）は未検証。
- ポップオーバー/ドロップダウンの部品（select, combobox, popover arrow等）はcoreには無い → 自作 or gpui-component。

### 5.3 「クリック外で閉じる」実装の材料

- `on_mouse_down_out(listener: impl Fn(&MouseDownEvent, &mut Window, &mut App) + 'static)` — `src/elements/div.rs:263`
- `Window` のフォーカス喪失・`on_blur`（要素の `on_blur` 系）と組み合わせる。
- `deferred` で描いたメニューはレイヤ順が最後になるため、背面クリックの検知は `on_mouse_down_out` をルート要素に置くのが定石。

## 6. 非同期IO（spawn / Task / AsyncApp / WeakEntity）

### 6.1 実行モデル（最重要）

GPUIは**単一スレッドのforeground（UI）**と、**マルチスレッドのbackground executor**の2層。

- foreground: `cx.spawn(|this: WeakEntity<Self>, cx: &mut AsyncApp| async move { ... })` — `src/app/context.rs:230`（`App::spawn` — `src/app.rs:2297`）。`WeakEntity` を渡され、**entityがdropされるとfutureはキャンセル**される（`this.update(...)` は `Option`/`Result` を返す）。
- background: `cx.background_spawn(async move { ... })` — `src/app/context.rs:872` / `src/app.rs:3335`。**`Future<Output=R> + Send + 'static` のみ**（UI型を跨がせない）。ファイルIO・ディレクトリ走査はこちら。
- `Context::spawn` のfutureは`Send`不要（`AsyncApp` を跨ぐのでUIスレッド専用）。ここから `cx.background_spawn` を呼んで重い処理を逃がす。
- `Task<R>`: drop するとキャンセル。`task.detach()` で投げっぱなし、`.await` で結果取得。`TaskExt` は prelude に含まれる。
- `AsyncApp` — `src/app/async_context.rs`。`AsyncApp::update_entity` / `update_window` でUI状態を更新。`cx.notify()` で再描画要求（`src/app/context.rs:221`、`src/app.rs:3052`）。
- タイマー: `BackgroundExecutor::timer(&self, duration: Duration) -> Task<()>` — `src/executor.rs:183`（`cx.background_executor().timer(..)`）。UI側の次フレームは `cx.on_next_frame(|window, cx| ...)` — `src/app/context.rs:288` / `Window::on_next_frame` — `src/window.rs:2710` / `AsyncApp::on_next_frame` — `src/app/async_context.rs:330`。

### 6.2 ディレクトリ読込の典型パターン（推奨）

```rust
fn load_dir(&mut self, dir: PathBuf, cx: &mut Context<Self>) {
    let weak = cx.entity().downgrade();
    self.task = Some(cx.spawn(move |_, cx: &mut AsyncApp| async move {
        let entries = cx.background_spawn(async move {
            std::fs::read_dir(&dir)?.filter_map(|e| e.ok())
                .map(|e| (e.path(), e.file_type().map(|t| t.is_dir()).unwrap_or(false)))
                .collect::<Vec<_>>()
        }).await;

        // UIスレッドに戻って状態更新
        cx.update_entity(&weak, |this, cx| {
            this.entries = entries;   // 件数分の ListState::splice / reset もここで
            cx.notify();              // 再描画
        }).ok();
    }));
}
```
- 世代番号（generation）で古い結果を捨てる: `self.generation += 1; let gen = self.generation;` → 結果適用時に `if gen != self.generation { return; }`。**`Task` を `Option` で保持し、新しい読込時に `self.task.take()` でdropすれば前のキャンセルになる**（drop=キャンセル）。
- `background_spawn` 内で `App`/`Entity`/`Window` を触らない（`Send` 制約でコンパイルエラーになる）。`std::fs` はblockingなので background 側でOK。大量ファイルは `ignore` クレート等で並列walkする。
- 注意: `cx.background_spawn` の戻りは `Task<R>`。`async move` 内で `.await` してUIへ戻るのが定石。
- 上記スニペットは**概念コード**（エラー処理・`Task` の型・`update_entity` の正確なシグネチャは実装時にコンパイラで確認）。`AsyncApp` のAPIは `src/app/async_context.rs` 参照。

### 6.3 イベント購読

- `cx.subscribe(&entity, |this, entity, event, cx| ...) -> Subscription` — `src/app/context.rs:94`。`Subscription` はdropで解除、保持しないなら `.detach()`（`examples/on_window_close_quit.rs:73-78`）。
- `EventEmitter<T>` を `impl` したEntityに対して `emit`。
- `App::observe`（`Entity` の変更監視）／`cx.observe_self` 系もあり（未確認・§10）。

## 7. アイコン・画像（AssetSource / svg / img / テキスト省略）

### 7.1 AssetSource（自前アイコンの埋め込み）

```rust
pub trait AssetSource: 'static + Send + Sync {
    fn load(&self, path: &str) -> anyhow::Result<Option<Cow<'static, [u8]>>>;
    fn list(&self, path: &str) -> anyhow::Result<Vec<SharedString>>;   // ディレクトリ列挙
}
```
— `src/assets.rs:13-26`（`list` は assets.rs:18。`load` のみ必須で `list` にはデフォルト実装あり `:22-26`）。

登録: `application().with_assets(MyAssets).run(|cx| ...)` — `Application::with_assets` `src/app.rs:203`。`asset_cache.rs:107/129` にキャッシュ実装。
実例: `examples/animation.rs:311`（`application().with_assets(Assets {}).run(...)`）、`examples/svg/`、`examples/image/` ディレクトリ。

**ファイルタイプアイコン**: coreにOSのMIMEアイコンを取得するAPIは無い（§9）。内蔵SVGセットを `AssetSource` で同梱し、拡張子→SVGパスを自前マップする。`gpui-component` は `gpui-component-assets` 的なアイコンセットを持つ（ecosystem scoutの所見参照）。

### 7.2 SVG / 画像要素

- `pub fn svg() -> Svg` — `src/elements/svg.rs:27`。Svgのビルダ:
  - `.path(impl Into<SharedString>)` — `src/elements/svg.rs:40`（AssetSource登録パス）
  - `.external_path(impl Into<SharedString>)` — `:46`
  - `.data(&[u8])` — `:53`（**生SVGバイト列を直接渡す**。`include_bytes!` でアイコンを埋め込める。ハッシュでキャッシュ）
  - `.with_transformation(Transformation)` — `:66`
- `pub fn img(source: impl Into<ImageSource>) -> Img` — `src/elements/img.rs:201`
  - `pub enum ImageSource { Resource(Resource), Render(Arc<RenderImage>), Image(Arc<Image>), Custom(Arc<dyn Fn(&mut Window,&mut App) -> Option<Result<Arc<RenderImage>, ImageCacheError>>>) }` — `src/elements/img.rs:42-51`
  - **`From<&Path>`, `From<PathBuf>`, `From<Arc<Path>>` がある**（`img.rs:89-105`）→ **ローカル画像ファイルのサムネイル表示は `img(path)` で可能**。
  - `From<&str>`/`From<String>` はURIかAssetSourceパスかを自動判別（`:63-81`）。
  - `ImageCache` — `src/elements/image_cache.rs`（`load(..)` — `:46`）。大量サムネイルのデコード・LRUは自前管理が必要（`ImageCache` のサイズ制御は未確認）。
- 注意: サムネイルは非同期デコード。`img()` はロード完了までプレースホルダを描くので、巨大ディレクトリでは生成数を `uniform_list` で絞る（§2.1）。

### 7.3 テキスト省略（ファイル名のellipsis）

- `pub enum TextOverflow` — `src/style.rs:407`（`Truncate` / `Ellipsis` 等）
- `Styled::truncate()` — `src/styled.rs:139`（`div().truncate()` で1行省略。`overflow_hidden` + `whitespace_nowrap` 相当をまとめて設定）
- ツールチップと併用: `.tooltip(|window, cx| ...)`（§5.1）でフルネーム表示。
- ファイル名のミドル省略（`very_long_...name.txt`）は core に専用APIが無い → 文字列側で自前カットするか、`text_ellipsis` 系のスタイル（`TextOverflow`）で末尾省略にする。

## 8. プラットフォーム統合（open_url / reveal / メニュー / a11y / IME / ウィンドウドラッグ）

### 8.1 URL・ファイル

- `App::open_url(&self, url: &str)` — `src/app.rs:1809`（`Platform::open_url` — `src/platform.rs:440`）
- `App::reveal_path(&self, path: &Path)` — `src/app.rs:1913`（ファイルマネージャで表示）
- `App::open_with_system(&self, path: &Path)` — `src/app.rs:1918`（既定アプリで開く）
- `App::prompt_for_paths` / `prompt_for_new_path` — §4.5
- 起動URL受け取り: `Application::on_open_urls(|urls: Vec<String>| ...)`（`src/app.rs:304`付近）

### 8.2 アプリメニュー（ネイティブメニューバー）

- `App::set_menus(menus: impl IntoIterator<Item = Menu>)` — `src/app.rs:2788`
- `Platform::set_menus(menus: Vec<Menu>, keymap: &Keymap)` — `src/platform.rs:488`
- `Menu` — `src/platform/app_menu.rs:4-13`: `{ name: SharedString, items: Vec<MenuItem>, disabled: bool }`。`Menu::new(name).items([...]).disabled(false)` — `:15-35`
- `MenuItem` — `:76-104`: `Separator` / `Submenu(Menu)` / `SystemMenu(OsMenu)` / `Action { name, action: Box<dyn Action>, os_action, checked, disabled }`
  - コンストラクタ: `MenuItem::separator()` `:108` / `submenu(Menu)` `:113` / `os_submenu(name, SystemMenuType)` `:118` / `action(name, impl Action)` `:126` / `os_action(name, action, OsAction)` `:137`
  - `.checked(bool)` — `:176`（チェックマーク付きメニュー）
- 最小例:
  ```rust
  cx.set_menus([
      Menu::new("ファイル").items([
          MenuItem::action("新しいフォルダ", NewFolder),
          MenuItem::action("名前を変更", Rename),
          MenuItem::separator(),
          MenuItem::action("削除", Delete),
          MenuItem::separator(),
          MenuItem::action("終了", Quit),
      ]),
      Menu::new("表示").items([
          MenuItem::action("アイコン", ViewIcons).checked(true),
          MenuItem::action("リスト", ViewList),
      ]),
  ]);
  ```
  メニュー項目を選ぶと `Action` が配送される（`on_action` 側で受ける）。
- 例: `examples/set_menus.rs`。GPUIのキーバインド（`Keymap`）がメニューのショートカット表示に使われる。
- **Linux（KDE Wayland）での挙動は要注意**（グローバルメニュー対応/非対応、`app_id` によるタスクバーグループ化）。`WindowOptions::app_id`（`src/platform.rs`）を設定しておく。

### 8.3 アクセシビリティ（AccessKit）

- GPUIはAccessKit統合を内蔵（`src/_accessibility.rs` にガイド、`Element` traitにフック）。
- `Element::a11y_role(&self) -> Option<accesskit::Role>` — `src/element.rs:115`付近
- `Element::write_a11y_info(&self, node: &mut accesskit::Node)` — `:121`付近
- `Element::a11y_synthetic_children(&mut self, prepaint, builder: &mut A11ySubtreeBuilder)` — `:130`付近（独自要素で子ノードを合成する用）
- `div().role(...)` / `.aria_*` 系が `StatefulInteractiveElement` にある（`src/elements/div.rs`）。`Application::new_inaccessible(platform)` で完全無効化も可能（`src/app.rs:194`）。
- ファイルリストは `role(List/ListItem)` + `aria-selected` を付けると読み上げが効く（具体的なメソッド名は `.role(accesskit::Role::List)` 等。未検証・§10）。

### 8.4 IME（日本語入力。リネーム/検索で必須）

- GPUI coreは**IMEの配管を持っている**が、**入力欄ウィジェットは持たない**（§9）。
  - `EntityInputHandler` trait — `src/input.rs:13-124`。実装メソッド: `text_for_range` / `selected_text_range` / `marked_text_range`（**変換中テキストの範囲**）/ `unmark_text` / `replace_text_in_range` / `replace_and_mark_text_in_range`（**変換中テキストの差し替え**）/ `bounds_for_range`（候補ウィンドウ位置）/ `character_index_for_point` / `text_input_configuration` など。
  - 使い方: View（Entity）に `EntityInputHandler` を実装し、要素の `paint` 中に `ElementInputHandler<V>`（`src/input.rs:128`）を作って **`Window::handle_input(...)`**（`src/window.rs:5391`）で登録する（doc `src/input.rs:9-11`）。
  - 変換候補ウィンドウの位置: `InputHandler::ime_candidate_bounds()` — `src/platform.rs:2137`、`compute_ime_candidate_bounds` — `:2094`。`PlatformWindow::update_ime_position(bounds)` — `src/platform.rs:1350`。
  - IME変換中判定: `Modifiers::is_ime_in_progress()` — `src/platform/keystroke.rs:229`、`prefers_ime_for_printable_keys()` — `src/platform.rs:2196`。
- 実装の参考: Zed本体の `crates/ui/src/input/`（TextInput相当）。GPUI coreだけでは「テキスト入力欄」を一から作る必要がある点は、ファイルエクスプローラーのリネーム機能で大きなコスト。
- **日本語入力の実機検証（KDE Wayland + fcitx5/mozc）は未実施**（§10）。

### 8.5 ウィンドウドラッグ（自前タイトルバー）

- `WindowOptions::app_owns_titlebar_drag: bool`（macOSのみ有効）と `Window::start_window_move(...)` の組み合わせ（docコメント `src/platform.rs:2500`付近）。
- Linux: `WindowOptions::window_decorations: Option<WindowDecorations>`（CSD/SSD切替）。`TitlebarOptions::appears_transparent` はLinuxでは効かない（§1.4）。
- 移動可能: `is_movable`、リサイズ: `is_resizable`、最小化: `is_minimizable`。
- 完全な自前タイトルバー＋影は `examples/window_shadow.rs` / `examples/window_movable.rs` が参考。

## 9. GPUI coreが意図的に持たないもの（自作 or gpui-component）

GPUIは「UIフレームワークのプリミティブ（要素・レイアウト・イベント・テキスト入力）」であり、**ウィジェット集ではない**。Zed本体は `crates/ui` という自前ウィジェット層を持っている。以下はcoreに無い：

| 部品 | core | 代替 |
|---|---|---|
| コンテキストメニュー | **無い**（§5.2, grep 0件） | `deferred`+`anchored` 自作 / `WindowKind::AnchoredPopup` / gpui-component |
| メニューバー（アプリ内） | ネイティブ `set_menus` のみ | Linuxではネイティブ依存。自前メニューバーは自作 |
| タブ / ドック | 無い | 自作（`div` + `uniform_list` + 状態管理） |
| テーブル / データグリッド | 無い（`examples/data_table.rs` は自前実装例） | 自作 or gpui-component `Table` |
| ツリー（階層表示） | 無い（`examples/tree.rs` は自前実装の参考） | 自作（`ListState` を平坦化して使う） |
| ブレッドクラム | 無い | 自作（`h_flex` + ボタン） |
| サイドバー / ペイン分割 | 無い（`examples/pattern.rs` にレイアウト例） | 自作（`resizable` 相当は無い） |
| ファイルタイプアイコン | 無い（OS MIMEアイコン取得APIなし） | 内蔵SVGセット + 拡張子マップ（§7.1） |
| ツールチップ | **ある**（`.tooltip()`/`.hoverable_tooltip()` — §5.1） | — |
| テキスト入力欄（IME対応） | **配管のみ**（`EntityInputHandler` — `src/input.rs:13`。ウィジェットは無い） | 自作（Zed `crates/ui/src/input/` を参考）or gpui-component |
| スクロールバー | `ScrollHandle` + 自前描画（`examples/list_example.rs` が実例） | 自作 or 標準部品 |
| ダイアログ（ネイティブ） | **ある**（`prompt_for_paths` / `prompt_for_new_path` — §4.5） | — |
| 通知 | `system_notifications.rs`（`examples/system_notifications.rs`） | — |
| i18n / 翻訳 | 無い | 自前（`rust-i18n` 等を任意で） |
| 設定永続化 | 無い | `serde` + `directories`/`etcetera` クレート等を自前 |
| OS統合（ゴミ箱・マウント・ファイル監視） | 無い | `trash` / `notify` クレート等を自前 |

### gpui-component（huacnlee）を使う場合

- crates.io `gpui-component` 0.9.1 / `gpui-base` 0.9.1 / `gpui-kit` 0.9.1 / `gpui-fast` 0.1.14（2026-11時点）。
- 提供部品は `gpui-component` のドキュメント/リポジトリ参照（ecosystem scoutの担当領域）。**GPUI本体とは別バージョン管理**で、gpui-preのバージョンと密結合（0.9.1は `gpui-pre` 0.3.x 系に対応）。
- 採用時のリスク: サードパーティのAPI変更、gpui-pre本体の破壊的変更への追随、ライセンス（Apache-2.0）はクリア。
- **方針提案**: ファイルエクスプローラーの中核（仮想化リスト・選択・D&D・クリップボード・ダイアログ）はGPUI coreで完結する。見た目の部品（ボタン、メニュー、ツールチップ、アイコン、テーブル）だけ gpui-component に寄せる、または全部自作（Zedの `crates/ui` を参考に）が選択肢。

### 9.5 ファイルエクスプローラー実装への示唆（API側からの結論）

1. **一覧の骨格**: `uniform_list`（リスト表示/詳細表示の行）＋「行＝グリッド1段」方式（アイコンビュー）。`ListState` は可変高さ・差分更新（`splice`）が必要な場合のみ。数千ファイルでも実用速度。
2. **状態管理**: `Entity<FileExplorer>` 1個に「カレントディレクトリ・選択集合・ソート・フィルタ・タスク」を持たせ、`cx.notify()` で再描画。重いIOは `cx.background_spawn`、結果適用は `update_entity`。**Taskを保持して新規読込時にdrop＝キャンセル**、generationで古い結果を破棄。
3. **選択**: `on_mouse_down`（`click_count`）で単一/ダブルクリック、`Modifiers`（ctrl/shift）で複数選択、空白クリックで解除。フォーカスは `track_focus` + `on_action` でキーボード操作（矢印/Enter/Delete/F2）。
4. **D&D**: 内部は `on_drag`/`on_drop`（`value` にファイルパス集合）。OSからのドロップは `on_drop::<ExternalPaths>`（`window.rs` が内部D&Dに変換）。OSへのドラッグアウトは `external_drag_payload` + `FileDragPaths`。**ドロップは MouseUp に変換されるので `on_click` との二重発火に注意**。
5. **コンテキストメニュー**: `WindowKind::AnchoredPopup` + `PopupOptions { grab: true }`（Wayland対応）か `deferred`+`anchored` で自作。メニュー項目は `Action` として実装しキーバインドと共用。
6. **ネイティブ統合**: `prompt_for_paths`（folder picker。拡張子フィルタ無し）、`reveal_path`、`open_with_system`、`open_url`、`set_menus`。**Linuxではxdg-desktop-portal依存の挙動差を実機確認**。
7. **リネーム/検索**: 入力欄は自作（`EntityInputHandler` 実装）。IME実機確認が必須。**ここが最大の実装リスク**。
8. **アイコン**: 内蔵SVG（`svg().data(include_bytes!(..))` が手軽）+ 拡張子マップ。OS MIMEアイコンは無い。サムネイルは `img(PathBuf)`。

## 10. 未確認事項

以下は本調査で**確認できなかった**（または部分的にしか確認していない）点。実装前にソース確認を推奨。

1. **`gpui_platform` crate（gpui-pre-platform）の実体**: ローカル未展開。`application()` の戻り型、feature名（`font-kit`/`wayland`/`x11`）、`web_init()`、`ApplicationHandle` の詳細はREADMEとexamplesからの推定。
2. **パッケージrenameの要否**: `gpui = { package = "gpui-pre", ... }` が必須か（lib名が `gpui` なので rename 不要の可能性もある）。実際に `cargo check` して確認するのが確実。`gpui_platform` 側も同様。
3. **gpui-pre 0.3.12（zed@d2a4fb6）との差分**: 本ノートは0.3.9基準。0.3.12で `uniform_list`/`list`/`WindowOptions`/`ExternalPaths` 等のAPIが変わっていないかは未確認（0.3.10〜0.3.12の変更点は未調査）。
4. ~~**`PathPromptOptions` のフィールド**~~ → 確認済み（`src/platform.rs:2800-2811`: `files`/`directories`/`multiple`/`prompt`。**拡張子フィルタは無い**）。
5. ~~**`Menu` / `MenuItem` の構造**~~ → 確認済み（`src/platform/app_menu.rs:4-189`。§8.2に記載）。
6. ~~**`anchored()` のビルダメソッド**~~ → 確認済み（`src/elements/anchored.rs:40-77`。§5.1に記載）。
7. ~~**`popup::PopupOptions` のフィールド**~~ → 確認済み（`src/platform/popup.rs:16-134`。§5.1に記載）。ただし `WindowKind::AnchoredPopup` を指定した `open_window` の実例（examplesに有無）は未確認。
8. ~~**`svg()` / `ImageSource`**~~ → 確認済み（`src/elements/svg.rs:27-70`、`src/elements/img.rs:42-105`。**`img(PathBuf)` でローカル画像を表示可能**＝サムネイル対応。§7.2に記載）。残る未確認: `ImageCache` のサイズ制御・`RenderImage` のデコード元。
9. ~~**`ClickEvent` のフィールド**~~ → 確認済み（`src/interactive.rs:290-296` のenum、`.click_count()` は `:426-428`。§3.1に記載）。
10. **IME API（部分確認）**: 配管は `EntityInputHandler`（`src/input.rs:13`）→ `ElementInputHandler` → `Window::handle_input` と判明（§8.4）。ただし `Window::handle_input` の正確なシグネチャ、`TextInputConfiguration`（`src/platform.rs:2434`）のフィールド、候補ウィンドウ位置の実挙動は未読。**KDE Wayland + fcitx5/mozc での実機検証は未実施**。
11. **a11y**: `div().role(..)` / `aria_*` メソッドの正確な名前（`StatefulInteractiveElement` 実装）。AccessKitのLinux（AT-SPI）での動作実機検証は未実施。
12. ~~**`BackgroundExecutor::timer`**~~ → 確認済み（`src/executor.rs:183`。§6.1に記載）。
13. **Linuxでの `set_menus`・`reveal_path`・`prompt_for_paths` の実装**（xdg-desktop-portal 経由か、GTKか）。KDEでの動作は未実機検証。
14. **`examples/move_entity_between_windows.rs` / `window_shadow.rs` / `input.rs` の内容**: 未読（自前タイトルバー、複数ウィンドウ、入力欄の実装参考）。
15. **バージョン固定の推奨**: `gpui-pre` はpre-1.0で破壊的変更が頻繁。`Cargo.toml` で `=0.3.12` のように固定するか、`Cargo.lock` をコミットする運用が必要（バージョン方針はecosystem scoutの所見と合わせて決定）。

## 11. ファイル:行 根拠一覧

（すべて `.tmp-deps-audit/crates/up-gpui-pre/` 配下 = gpui-pre 0.3.9、zed rev 93d7456）

| 項目 | 根拠 |
|---|---|
| パッケージ/lib名/metadata | `Cargo.toml:45-48`, `Cargo.toml:58-63`, `Cargo.toml:83-85` |
| gpui_platform 起動 | `README.md:9-13`, `:15`, `:21`; `examples/hello_world.rs:96-119` |
| Application::run / builders | `src/app.rs:148`, `:177`, `:179`, `:194`, `:203`, `:228`, `:240`, `:249`, `:275` |
| App::open_window | `src/app.rs:1500-1527` |
| App::activate / quit / windows | `src/app.rs:1536`, `:1231`, `:1474` |
| App::bind_keys / on_window_closed | `src/app.rs:2606`, `:2751` |
| App::open_url / prompt_* / reveal_path / open_with_system | `src/app.rs:1809`, `:1891`, `:1904`, `:1913`, `:1918` |
| App::set_menus | `src/app.rs:2788`; `src/platform.rs:488`; `src/platform/app_menu.rs:4` |
| App::read/write clipboard | `src/app.rs:1722`, `:1733`, `:1750` |
| QuitMode | `src/app.rs:456-466` |
| WindowOptions / TitlebarOptions | `src/platform.rs:2491`, `:2671`付近, `:2692` |
| WindowBounds / WindowKind | `src/platform.rs:2628`付近, `:2714`付近 |
| Anchor | `src/geometry.rs:2168` |
| FocusHandle | `src/window.rs:538`, `:583`, `:595` |
| remove_window / start_window_move | `src/window.rs`（`examples/on_window_close_quit.rs:21`） |
| FileDrop変換 | `src/window.rs:5656-5707` |
| Render / Element / IntoElement / RenderOnce | `src/element.rs:164`, `:52`, `:146`, `:180` |
| Element a11yフック | `src/element.rs:115-140`付近 |
| Entity | `src/app/entity_map.rs:435` |
| prelude | `src/prelude.rs:5-10` |
| actions! | `src/action.rs:25-57`, `:62-80` |
| KeyBinding::new / load / context | `src/keymap/binding.rs:32`, `:49`, `:34` |
| on_mouse_down / any / capture / out | `src/elements/div.rs:126`, `:162`, `:146`, `:263` |
| MouseDownEvent | `src/interactive.rs:148-160` |
| on_click / on_aux_click | `src/elements/div.rs:591`, `:607`付近 |
| on_key_down | `src/elements/div.rs:496`; `KeyDownEvent` `src/interactive.rs:25-35` |
| on_hover / hover_listener_mode | `src/elements/div.rs:675`, `:694`付近 |
| track_focus / tab_stop | `src/elements/div.rs:788`, `:800` |
| tooltip / hoverable_tooltip | `src/elements/div.rs:706`付近, `:722`付近 |
| on_drag / external_drag_payload / on_drop / can_drop | `src/elements/div.rs:615`, `:643`, `:568`, `:580`付近 |
| DragMoveEvent | `src/elements/div.rs:67` |
| ExternalPaths / FileDropEvent | `src/interactive.rs:694`, `:737-759` |
| ExternalDragPayload / FileDragPaths | `src/interactive.rs:705-726` |
| ClipboardItem / ClipboardEntry / ExternalPaths variant | `src/platform.rs:2969`, `:3010-3017` |
| ClipboardItem constructors / From impl | `src/platform.rs:3021-3050`, `:3104-3135` |
| Platform::open_url/prompt_*/reveal_path | `src/platform.rs:440`, `:444`, `:448`, `:454` |
| Clipboard platform trait | `src/platform.rs:573`, `:574`, `:583` |
| CursorStyle | `src/platform.rs:2950`付近 |
| uniform_list | `src/elements/uniform_list.rs:22-32`, `:87-106`, `:132`, `:150`, `:163`, `:252`, `:686` |
| list / ListState / ListAlignment | `src/elements/list.rs:24`, `:164`, `:314`-`:829` |
| ScrollHandle / div scroll | `src/elements/div.rs:4249`, `:1522`, `:1535`, `:1549` |
| deferred / anchored / PopupOptions | `src/elements/deferred.rs:7`, `src/elements/anchored.rs:27`, `src/platform/popup.rs:16` |
| spawn / background_spawn / notify / on_next_frame | `src/app/context.rs:230`, `:872`, `:221`, `:288`; `src/app.rs:2297`, `:3335`, `:3052`; `src/window.rs:2710`; `src/app/async_context.rs:330` |
| subscribe | `src/app/context.rs:94` |
| AssetSource | `src/assets.rs:13-26`; `src/asset_cache.rs:107/129`; `src/app.rs:203` |
| svg() / img() / ImageCache | `src/elements/svg.rs:27`（`.path` `:40`, `.data` `:53`）; `src/elements/img.rs:42-51`, `:201`; `src/elements/image_cache.rs:46` |
| Menu / MenuItem | `src/platform/app_menu.rs:4-13`, `:76-104`, `:108-149` |
| IME / 入力 | `src/input.rs:13-124`（EntityInputHandler）, `:128`（ElementInputHandler）; `src/window.rs:5391`（handle_input）; `src/platform.rs:1350`, `:2137`, `:2434` |
| PathPromptOptions | `src/platform.rs:2800-2811` |
| TextOverflow / truncate | `src/style.rs:407`; `src/styled.rs:139` |

### 参考example一覧（ローカル）

`examples/hello_world.rs`, `uniform_list.rs`, `list_example.rs`, `drag_drop.rs`, `tree.rs`, `data_table.rs`, `set_menus.rs`, `popover.rs`, `anchor.rs`, `input.rs`, `scrollable.rs`, `on_window_close_quit.rs`, `window_shadow.rs`, `window_movable.rs`, `a11y.rs`, `animation.rs`（Assets登録例）, `grid_layout.rs`, `pattern.rs`, `system_notifications.rs`, `text_wrapper.rs`。
