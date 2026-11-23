// SPDX-License-Identifier: AGPL-3.0-or-later
//
// PWA ビューア。データ源は core の wasm-bindgen API
// （CoreViewer::list_entries / search）のみ。pkg/ が無い場合だけ、
// 確認用の読み取り専用デモへフォールバックする。
//
// core API のシグネチャは確定済み（core/src/wasm.rs / pkg/gfe_core.d.ts を参照）。
// 生成物は camelCase の JSON 文字列を返し、CoreViewer は静的 `new()` で生成する。

const WASM_MODULE = './pkg/gfe_core.js';
const SEARCH_DEBOUNCE_MS = 150;
const LONG_PRESS_MS = 500;
const SWIPE_MIN_DX = 80;
const SWIPE_EDGE_GUARD = 24;
const CLICK_SUPPRESS_MS = 700;

// Node's `node --test` imports this module to exercise the pure helpers.
// Only the browser has `document`; skip DOM wiring there.
const hasDom = typeof document !== 'undefined';
const els = hasDom ? {
  status: document.getElementById('status'),
  search: document.getElementById('search'),
  pickDir: document.getElementById('pick-dir'),
  modeNote: document.getElementById('mode-note'),
  list: document.getElementById('entry-list'),
  title: document.getElementById('list-title'),
  count: document.getElementById('list-count'),
} : {};

const state = {
  module: null,
  viewer: null,
  wasmReady: false,
  entries: [],
  debounce: 0,
  rootName: '.',
  // 現在描画中の一覧と選択状態（タップ / 長押し / スワイプ / キーボードで共有）。
  renderedEntries: [],
  selected: null,
  touch: null,
  suppressClickUntil: 0,
};

// --- 小さなユーティリティ -------------------------------------------------

// Pure clock comparison: no `performance.now()` call, so it can be verified
// with a fixed `now` from node --test.
function isClickSuppressed(now, until) {
  return now < until;
}

// 右方向スワイプの純粋判定。touchmove は 10px 超で moved を立てるため、
// ここへ moved は入れない（80px のスワイプが moved に潰されるのを防ぐ）。
function shouldSwipeBack(touchState, dx, dy) {
  if (!touchState || !touchState.swipeEnabled) return false;
  if (!Number.isFinite(dx) || !Number.isFinite(dy)) return false;
  return dx > SWIPE_MIN_DX && Math.abs(dx) > Math.abs(dy) * 1.2;
}

// 一覧の keydown をコマンドへ変換する。修飾キー付きはブラウザ / OS の
// ショートカットへ譲り、Ctrl/Cmd/Alt+I 等を横取りしない。
function listKeyCommand(event) {
  if (!event || event.ctrlKey || event.metaKey || event.altKey) return null;
  switch (event.key) {
    case 'ArrowDown': return 'down';
    case 'ArrowUp': return 'up';
    case 'Home': return 'home';
    case 'End': return 'end';
    case ' ':
    case 'Spacebar': return 'details';
    case 'Enter': return 'enter';
    default:
      return typeof event.key === 'string' && event.key.toLowerCase() === 'i'
        ? 'details'
        : null;
  }
}

// FSA のフォルダ選択は core API がロード済みのときだけ意味を持つ。
// デモ（## 一覧（読み取り専用デモ））ではパスを選んでも走査できないため
// ボタンを無効化する。pure helper にして node --test から検証できるようにする。
function canPickDirectory(wasmReady, fsaSupported) {
  return Boolean(wasmReady && fsaSupported);
}

// DOM 側の状態反映。wasm 初期化の成否が変わるどの経路でも呼ぶ。
// 引数を明示渡しできるようにして、node --test から DOM モックで disabled を検証する。
function updatePickDirAvailability({
  button = hasDom ? els.pickDir : null,
  wasmReady = state.wasmReady,
  fsaSupported = supportsFileSystemAccess(),
} = {}) {
  if (!button) return;
  button.disabled = !canPickDirectory(wasmReady, fsaSupported);
}

function setStatus(message, level = 'info') {
  els.status.textContent = message;
  els.status.dataset.level = level;
}

function supportsFileSystemAccess() {
  return typeof window !== 'undefined' && typeof window.showDirectoryPicker === 'function';
}

function pick(obj, ...keys) {
  for (const key of keys) {
    if (obj && obj[key] !== undefined && obj[key] !== null) return obj[key];
  }
  return undefined;
}

// wasm-bindgen / serde_wasm_bindgen の出力差を吸収する。
// JSON 文字列、SearchResult { entry, match_line } ラッパーにも対応。
function toEntry(raw) {
  let value = raw;
  if (typeof value === 'string') {
    try {
      value = JSON.parse(value);
    } catch {
      return null;
    }
  }
  if (!value || typeof value !== 'object') return null;

  const wrapped = pick(value, 'entry');
  if (wrapped && typeof wrapped === 'object') {
    const inner = toEntry(wrapped);
    if (inner) inner.matchLine = pick(value, 'match_line', 'matchLine');
    return inner;
  }

  const name = pick(value, 'name_text', 'nameText', 'name');
  if (name == null) return null;
  return {
    name: String(name),
    path: String(pick(value, 'path', 'full_path', 'fullPath') ?? name),
    isDir: Boolean(pick(value, 'is_dir', 'isDir')),
    isSymlink: Boolean(pick(value, 'is_symlink', 'isSymlink')),
    size: Number(pick(value, 'size', 'len') ?? 0),
    modified: pick(value, 'modified'),
    matchLine: pick(value, 'match_line', 'matchLine'),
  };
}

// core API は失敗時に JSON ではなくエラー文言の JsValue 文字列を返す。
// JSON.parse に投げて空配列にすると「0 件」と誤表示するため、ここで切り分ける。
function decodeCorePayload(value) {
  if (typeof value === 'string') {
    try {
      return { ok: true, data: JSON.parse(value) };
    } catch {
      return { ok: false, error: value };
    }
  }
  return { ok: true, data: value };
}

// wasm-bindgen は Rust の Err(JsValue) を文字列例外として throw する。
function errorMessage(error) {
  if (error instanceof Error && error.message) return error.message;
  if (typeof error === 'string') return error;
  return String(error ?? 'unknown error');
}

function normalizeEntries(value) {
  let list = value;
  if (typeof list === 'string') {
    try {
      list = JSON.parse(list);
    } catch {
      return [];
    }
  }
  if (!Array.isArray(list)) {
    const nested = pick(list, 'entries', 'results', 'items');
    list = Array.isArray(nested) ? nested : [];
  }
  return list.map(toEntry).filter(Boolean);
}

function humanSize(bytes) {
  if (!Number.isFinite(bytes) || bytes <= 0) return '—';
  const units = ['B', 'K', 'M', 'G', 'T'];
  let value = bytes;
  let index = 0;
  while (value >= 1024 && index + 1 < units.length) {
    value /= 1024;
    index += 1;
  }
  return index === 0 ? `${bytes} B` : `${value.toFixed(1)} ${units[index]}`;
}

// --- 描画（テキストはすべて textContent で設定し XSS を避ける） -------------

function render(entries, { heading }) {
  els.title.textContent = heading;
  els.count.textContent = `${entries.length} 件`;
  els.list.replaceChildren();
  // 再描画時は選択をリセットし、以降の操作はこの配列を基準にする。
  state.renderedEntries = entries;
  state.selected = null;

  if (entries.length === 0) {
    const li = document.createElement('li');
    // 空状態の li は listbox の選択肢ではないため、支援技術へ選択肢として
    // 読み上げられないよう role=presentation を付与する（状態通知は #status の
    // aria-live に一本化済み）。
    li.setAttribute('role', 'presentation');
    li.className = 'empty';
    li.textContent = '該当する項目はありません';
    els.list.append(li);
    return;
  }

  const fragment = document.createDocumentFragment();
  for (const [index, entry] of entries.entries()) {
    const li = document.createElement('li');
    li.dataset.index = String(index);
    li.setAttribute('role', 'option');
    li.setAttribute('aria-selected', 'false');
    li.tabIndex = index === 0 ? 0 : -1;

    const name = document.createElement('span');
    name.className = 'entry-name';
    name.dataset.kind = entry.isDir ? 'dir' : 'file';

    // 装飾記号（📁 / 📄 / ↗）は aria-hidden の別 span へ分離する。
    // textContent に混ぜると option のアクセシブル名へ記号が読み上げられる。
    const icon = document.createElement('span');
    icon.className = 'entry-icon';
    icon.setAttribute('aria-hidden', 'true');
    icon.textContent = entry.isDir ? '📁' : '📄';
    const label = document.createElement('span');
    label.className = 'entry-label';
    label.textContent = entry.name;
    name.append(icon, label);
    if (entry.isSymlink) {
      const link = document.createElement('span');
      link.className = 'entry-link-icon';
      link.setAttribute('aria-hidden', 'true');
      link.textContent = '↗';
      name.append(link);
    }

    const meta = document.createElement('span');
    meta.className = 'entry-meta';
    const parts = [entry.isDir ? 'フォルダ' : humanSize(entry.size)];
    if (entry.matchLine != null) parts.push(`L${entry.matchLine}`);
    meta.textContent = parts.join(' · ');
    meta.title = entry.path;

    li.append(name, meta);
    fragment.append(li);
  }
  els.list.append(fragment);
}

// --- 選択 / タッチ / マウス / キーボード操作 -------------------------------
// 一覧は差し替わるため、リスナーは els.list への委譲で 1 回だけ登録する。

function entryAt(index) {
  const i = Number(index);
  return Number.isInteger(i) && i >= 0 && i < state.renderedEntries.length
    ? state.renderedEntries[i]
    : null;
}

function selectedIndex() {
  return state.selected ? state.renderedEntries.indexOf(state.selected) : -1;
}

function applySelection(index) {
  for (const li of els.list.querySelectorAll('li[data-index]')) {
    const isSelected = Number(li.dataset.index) === index;
    li.setAttribute('aria-selected', String(isSelected));
    li.tabIndex = isSelected ? 0 : -1;
  }
}

function selectIndex(index, { fromKeyboard = false } = {}) {
  const entry = entryAt(index);
  if (!entry) return;
  state.selected = entry;
  applySelection(index);
  setStatus(`選択: ${entry.name}`);
  const li = els.list.querySelector(`li[data-index="${index}"]`);
  if (!li) return;
  li.focus({ preventScroll: true });
  // preventScroll だけだとキーボード連続選択が画面外へ消えるため、
  // キーボード操作のときだけ nearest で viewport 内へ追従させる。
  if (fromKeyboard && typeof li.scrollIntoView === 'function') {
    li.scrollIntoView({ block: 'nearest' });
  }
}

function moveSelection(delta) {
  const length = state.renderedEntries.length;
  if (length === 0) return;
  const current = selectedIndex();
  const start = current < 0 ? (delta > 0 ? -1 : 0) : current;
  selectIndex(Math.min(length - 1, Math.max(0, start + delta)), { fromKeyboard: true });
}

function showEntryMeta(entry) {
  if (!entry) return;
  const kind = entry.isDir ? 'フォルダ' : 'ファイル';
  // textContent 経由（setStatus）なので path に HTML が入っていても実行されない。
  setStatus(
    `詳細: ${entry.path} · size: ${entry.size} B (${humanSize(entry.size)}) · isDir: ${entry.isDir} · ${kind}`,
  );
  // 長押し / 右クリック後の click が選択として誤処理されないようにする。
  state.suppressClickUntil = performance.now() + CLICK_SUPPRESS_MS;
}

function entryFromEvent(event) {
  const li = event.target instanceof Element
    ? event.target.closest('li[data-index]')
    : null;
  return { li, entry: li ? entryAt(li.dataset.index) : null };
}

function onListClick(event) {
  if (isClickSuppressed(performance.now(), state.suppressClickUntil)) return;
  const { li, entry } = entryFromEvent(event);
  if (li && entry) selectIndex(Number(li.dataset.index));
}

function onListContextMenu(event) {
  // ブラウザ標準のコンテキストメニューを抑制し、デスクトップ右クリックも
  // タッチの長押しと同じメタ表示として扱う。
  event.preventDefault();
  const { entry } = entryFromEvent(event);
  if (entry) showEntryMeta(entry);
}

function clearLongPress() {
  if (state.touch && state.touch.timer) {
    window.clearTimeout(state.touch.timer);
    state.touch.timer = 0;
  }
}

function onListTouchStart(event) {
  // 検索入力中は無効化（iOS Safari の標準スワイプバックとの競合を避ける）。
  // 2 本目の指が触れた場合もここを通る。state.touch を捨てる前にタイマーを
  // キャンセルしないと、長押しコールバックが残って詳細表示が誤って出る。
  if (document.activeElement === els.search || event.touches.length !== 1) {
    clearLongPress();
    state.touch = null;
    return;
  }
  const touch = event.touches[0];
  const { entry } = entryFromEvent(event);
  state.touch = {
    startX: touch.clientX,
    startY: touch.clientY,
    entry,
    moved: false,
    // 画面左端から始まるジェスチャは OS の戻る操作へ譲る。
    swipeEnabled: touch.clientX > SWIPE_EDGE_GUARD,
    timer: 0,
  };
  if (entry) {
    state.touch.timer = window.setTimeout(() => {
      state.touch = null;
      showEntryMeta(entry);
    }, LONG_PRESS_MS);
  }
}

function onListTouchMove(event) {
  const current = state.touch;
  if (!current || event.touches.length !== 1) return;
  const touch = event.touches[0];
  if (Math.hypot(touch.clientX - current.startX, touch.clientY - current.startY) > 10) {
    current.moved = true;
    clearLongPress();
  }
}

function onListTouchEnd(event) {
  const current = state.touch;
  if (!current) return;
  clearLongPress();
  state.touch = null;
  const touch = event.changedTouches[0];
  if (!touch) return;
  const dx = touch.clientX - current.startX;
  const dy = touch.clientY - current.startY;
  // moved は touchmove の 10px 超過で立つ。スワイプ判定を後に置くと 80px の
  // 右スワイプが毎回 moved に潰されるため、ここで先に判定して return する。
  if (document.activeElement !== els.search && shouldSwipeBack(current, dx, dy)) {
    onSwipeBack().catch((error) =>
      setStatus(`一覧へ戻る処理に失敗: ${errorMessage(error)}`, 'error'));
    return;
  }
  // 以降はロングプレス / クリック抑止。moved はスワイプ判定には使わない。
  if (current.moved || !current.swipeEnabled) return;
  if (isClickSuppressed(performance.now(), state.suppressClickUntil)) return;
}

function onListTouchCancel() {
  clearLongPress();
  state.touch = null;
}

async function onSwipeBack() {
  state.suppressClickUntil = performance.now() + CLICK_SUPPRESS_MS;
  window.clearTimeout(state.debounce);
  els.search.value = '';
  state.selected = null;
  setStatus('検索をクリアして一覧へ戻りました');
  await runSearch('');
}

function onListKeyDown(event) {
  const command = listKeyCommand(event);
  if (!command) return;
  switch (command) {
    case 'down':
      event.preventDefault();
      moveSelection(1);
      return;
    case 'up':
      event.preventDefault();
      moveSelection(-1);
      return;
    case 'home':
      event.preventDefault();
      selectIndex(0, { fromKeyboard: true });
      return;
    case 'end':
      event.preventDefault();
      selectIndex(state.renderedEntries.length - 1, { fromKeyboard: true });
      return;
    // ポインタの長押し / 右クリックと同じ詳細表示（path / size / isDir）を
    // キーボードからも呼べるようにする。Space はページスクロールを止める。
    case 'details': {
      event.preventDefault();
      const { entry } = entryFromEvent(event);
      showEntryMeta(state.selected ?? entry);
      return;
    }
    case 'enter': {
      const { li, entry } = entryFromEvent(event);
      if (li && entry) {
        selectIndex(Number(li.dataset.index), { fromKeyboard: true });
      } else if (!state.selected) {
        return;
      }
      event.preventDefault();
      setStatus(`選択確定: ${state.selected.name}`);
      return;
    }
    default:
      return;
  }
}

// --- コア API 呼び出し（主要経路） ----------------------------------------

// 実在する core API（list_entries(dir) / search(query, dir)）だけを呼ぶ。
// wasm-bindgen の Err(JsValue) は文字列例外として throw されるため、呼び出し側へ委譲する。
async function callCore(viewer, name, args) {
  if (typeof viewer[name] !== 'function') {
    throw new Error(`core API が見つかりません: ${name}`);
  }
  return await viewer[name](...args);
}

async function initCore() {
  // 動的 import なので pkg/ が無くてもページ自体は読み込める。
  const module = await import(WASM_MODULE);
  const init = module.default;
  if (typeof init !== 'function') throw new Error('wasm の init() が見つかりません');
  await init();
  state.module = module;
  // panic の内容を console.error に流す（core/src/wasm.rs の set_panic_hook）。
  if (typeof module.set_panic_hook === 'function') module.set_panic_hook();

  const Viewer = module.CoreViewer;
  if (typeof Viewer !== 'function' || typeof Viewer.new !== 'function') {
    throw new Error('CoreViewer.new() が export されていません');
  }
  // wasm-bindgen の private constructor は JS から new すると
  // __wbg_ptr の無い壊れた Object ができ、最初の試行で break してしまう。
  // 公開 API は静的 `new()` のみ（pkg/gfe_core.d.ts で確認済み）。
  return Viewer.new();
}

// wasm32 の std::fs はブラウザで実ファイルを読めないため、
// JS 側のサンプルを core の in-memory API へ渡して実際の並び順/
// 検索ロジックを通す（Phase 1 の受け入れ経路）。
function demoJson() {
  return JSON.stringify(DEMO_ENTRIES.map((item) => ({
    name: item.name,
    path: item.path,
    isDir: Boolean(item.is_dir),
    isSymlink: Boolean(item.is_symlink),
    size: Number(item.size || 0),
  })));
}

function fillDemoIfEmpty() {
  if (state.entries.length === 0) {
    state.entries = DEMO_ENTRIES.map(toEntry).filter(Boolean);
  }
}

async function refreshList() {
  if (state.wasmReady && state.viewer) {
    try {
      let raw;
      if (typeof state.module?.listEntriesFromJson === 'function') {
        raw = state.module.listEntriesFromJson(demoJson());
      } else {
        raw = await callCore(state.viewer, 'list_entries', [state.rootName]);
      }
      const decoded = decodeCorePayload(raw);
      if (!decoded.ok) throw new Error(decoded.error);
      state.entries = normalizeEntries(decoded.data);
      render(state.entries, { heading: '一覧（core API）' });
      setStatus(`core API 経由で ${state.entries.length} 件を表示`);
      return;
    } catch (error) {
      // wasm 上の std::fs は実ファイルを読めない。エラーを隠さず表示したうえで
      // scaffold の確認用に読み取り専用デモへ退避する（受け入れ基準を満たさない）。
      setStatus(`core API の一覧取得に失敗: ${errorMessage(error)}`, 'error');
      fillDemoIfEmpty();
    }
  }
  render(state.entries, { heading: '一覧（読み取り専用デモ）' });
}

async function runSearch(query) {
  const q = query.trim();
  if (!q) {
    await refreshList();
    return;
  }

  if (state.wasmReady && state.viewer) {
    try {
      let raw;
      if (typeof state.module?.searchInJson === 'function') {
        raw = state.module.searchInJson(q, demoJson());
      } else {
        raw = await callCore(state.viewer, 'search', [q, state.rootName]);
      }
      const decoded = decodeCorePayload(raw);
      if (!decoded.ok) throw new Error(decoded.error);
      const results = normalizeEntries(decoded.data);
      render(results, { heading: `検索: ${q}（core API）` });
      setStatus(`core API の検索で ${results.length} 件`);
      return;
    } catch (error) {
      setStatus(`core API の検索に失敗: ${errorMessage(error)}`, 'error');
      fillDemoIfEmpty();
    }
  }

  const filtered = state.entries.filter((entry) =>
    entry.name.toLowerCase().includes(q.toLowerCase()),
  );
  render(filtered, { heading: `検索: ${q}（読み取り専用デモ）` });
}

// --- File System Access API（対応ブラウザのみ） ----------------------------

async function onPickDirectory() {
  if (!supportsFileSystemAccess() || !state.wasmReady) return;
  try {
    const handle = await window.showDirectoryPicker({ mode: 'read' });
    state.rootName = handle.name;
    setStatus(`選択: ${handle.name}`);
    // TODO(P1-04): FileSystemDirectoryHandle を core の FileSystem ポートへ橋渡しする。
    // 現状はルート名だけを CoreViewer に渡す。実ツリーの走査には OPFS 側の移植が必要
    // （notes/wasm-browser-notes.md §2「ファイルシステム抽象化」を参照）。
    if (state.wasmReady) await refreshList();
  } catch (error) {
    if (error && error.name === 'AbortError') return;
    setStatus(`フォルダ選択に失敗: ${errorMessage(error)}`, 'error');
  }
}

// --- 起動 -----------------------------------------------------------------

const DEMO_ENTRIES = [
  { name: 'README.md', path: '/demo/README.md', is_dir: false, size: 2048 },
  { name: 'src', path: '/demo/src', is_dir: true, size: 0 },
  { name: 'notes.txt', path: '/demo/notes.txt', is_dir: false, size: 512 },
  { name: 'Cargo.toml', path: '/demo/Cargo.toml', is_dir: false, size: 330 },
];

function useDemo(message) {
  state.wasmReady = false;
  updatePickDirAvailability();
  state.entries = DEMO_ENTRIES.map(toEntry).filter(Boolean);
  setStatus(message, 'error');
  els.modeNote.hidden = false;
  els.modeNote.textContent =
    'pkg/ が見つからないためコア API を利用できません。README.md の手順で '
    + 'wasm-pack build を実行すると、一覧・検索が core API 経由になります。';
  render(state.entries, { heading: '一覧（読み取り専用デモ）' });
}

async function boot() {
  els.search.addEventListener('input', () => {
    window.clearTimeout(state.debounce);
    state.debounce = window.setTimeout(() => runSearch(els.search.value), SEARCH_DEBOUNCE_MS);
  });

  // ここでは検索 input の Enter には触れず、一覧内の操作だけを扱う。
  els.list.setAttribute('role', 'listbox');
  els.list.addEventListener('click', onListClick);
  els.list.addEventListener('contextmenu', onListContextMenu);
  els.list.addEventListener('keydown', onListKeyDown);
  els.list.addEventListener('touchstart', onListTouchStart, { passive: true });
  els.list.addEventListener('touchmove', onListTouchMove, { passive: true });
  els.list.addEventListener('touchend', onListTouchEnd, { passive: true });
  els.list.addEventListener('touchcancel', onListTouchCancel, { passive: true });

  if (supportsFileSystemAccess()) {
    els.pickDir.hidden = false;
    els.pickDir.addEventListener('click', onPickDirectory);
  } else {
    els.modeNote.hidden = false;
    els.modeNote.textContent =
      'このブラウザは File System Access API 非対応です（iOS Safari 等）。'
      + 'フォルダ選択を隠し、内蔵サンプルによる読み取り専用デモとして動作します。';
  }

  // wasm のロード完了前はボタンを押せないようにする（デモへ落ちた場合は
  // useDemo() でも同じ状態を維持する）。
  updatePickDirAvailability();

  try {
    state.viewer = await initCore();
    state.wasmReady = true;
    updatePickDirAvailability();
    setStatus('core API 初期化完了');
    await refreshList();
  } catch (error) {
    useDemo(`wasm 未ロード（${errorMessage(error)}）→ 読み取り専用デモ`);
  }
}

if (hasDom) boot();

if (typeof module !== 'undefined' && module.exports) {
  module.exports = {
    isClickSuppressed,
    canPickDirectory,
    shouldSwipeBack,
    listKeyCommand,
    updatePickDirAvailability,
    toEntry,
  };
}
