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
  // SW 更新通知の再読込導線（index.html の #status 直下に配置）。
  updateReload: document.getElementById('update-reload'),
  search: document.getElementById('search'),
  pickDir: document.getElementById('pick-dir'),
  modeNote: document.getElementById('mode-note'),
  list: document.getElementById('entry-list'),
  title: document.getElementById('list-title'),
  count: document.getElementById('list-count'),
  // 選択モード関連（index.html の専用トグル。修飾キーは横取りしない）。
  selectMode: document.getElementById('select-mode'),
  selectActions: document.getElementById('select-actions'),
  selectAll: document.getElementById('select-all'),
  selectRange: document.getElementById('select-range'),
  selectClear: document.getElementById('select-clear'),
  selectCount: document.getElementById('select-count'),
  // ソート関連（core wasm API の既定値に依存せず JS 側で並べ替える）。
  sortKey: document.getElementById('sort-key'),
  sortDir: document.getElementById('sort-dir'),
} : {};

const state = {
  module: null,
  viewer: null,
  wasmReady: false,
  entries: [],
  debounce: 0,
  // 検索リクエストの単調カウンタ。連続入力 / clearSearch / スワイプバックの
  // 後に古い await 完了が新しい結果を上書きしないための識別子（BD05）。
  searchGeneration: 0,
  // SW の更新を通知済みなら、後続の setStatus で通知を上書きしない（BH02）。
  updateNotified: false,
  rootName: '.',
  // 現在描画中の一覧と選択状態（タップ / 長押し / スワイプ / キーボードで共有）。
  // 選択は object 参照ではなく path で保持する。再描画 / 検索 / 再ソートで
  // エントリ object が差し替わっても選択が消えないようにするため。
  renderedEntries: [],
  lastRender: { entries: [], options: {} },
  selectedPaths: [],
  anchorPath: null,
  focusIndex: 0,
  selectMode: false,
  rangeArmed: false,
  touch: null,
  suppressClickUntil: 0,
};

// --- 小さなユーティリティ -------------------------------------------------

// Pure clock comparison: no `performance.now()` call, so it can be verified
// with a fixed `now` from node --test.
function isClickSuppressed(now, until) {
  return now < until;
}

// BE02: ブラウザは dblclick の前に click を2回発火する。選択モードで両方を
// toggleAt に渡すとトグルが相殺されるため、第2回以降（event.detail >= 2）は
// click 側で無視し、dblclick 側で詳細表示 / 選択確定を行う。
function secondClickSuppressed(eventDetail) {
  return Number.isInteger(eventDetail) && eventDetail >= 2;
}

// ダブルクリック（正確に2回）だけを活性化コマンドへ変換する pure helper。
// 3回以上の連打は誤操作なのでコマンドを出さない（Rust 側の count==2 と同規則）。
function doubleClickCommand(eventDetail) {
  return eventDetail === 2 ? 'confirm' : null;
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
// Shift はナビゲーションキー（矢印 / Home / End）だけ譲る。Shift+I は
// 大文字入力なので詳細表示として受理する（BB14）。
function listKeyCommand(event) {
  if (!event || event.ctrlKey || event.metaKey || event.altKey) return null;
  const shifted = Boolean(event.shiftKey);
  switch (event.key) {
    case 'ArrowDown': return shifted ? null : 'down';
    case 'ArrowUp': return shifted ? null : 'up';
    case 'Home': return shifted ? null : 'home';
    case 'End': return shifted ? null : 'end';
    // 選択モード / 範囲 arm / 選択状態からの脱出経路（BB04）。
    case 'Escape': return 'escape';
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

// 空状態の文言をクエリとモードから決める pure helper。
// 返り値は呼び出し側が textContent へ設定するため、クエリに HTML が入っていても
// タグとして解釈されず XSS にならない。
//   query あり            → 「${q}」に一致する項目はありません（クエリをエコー）
//   query なし & 検索モード → 検索に一致する項目はありません
//   一覧ゼロ              → 項目がありません
function emptyStateText(query, mode) {
  const q = typeof query === 'string' ? query.trim() : '';
  if (q) return `「${q}」に一致する項目はありません`;
  return mode === 'search' ? '検索に一致する項目はありません' : '項目がありません';
}

// 「検索をクリア」ボタンは検索0件のときだけ出し、一覧そのものが空のときは出さない。
function shouldShowClearSearch(mode) {
  return mode === 'search';
}

// --- 非同期検索の世代ガード（BD05） ----------------------------------------
// デバウンス解除だけでは await 中のリクエストを止められないため、リクエスト
// 開始ごとに世代を進め、完了時に自分がまだ現役かを照合する。双方 pure なので
// node --test から「古い結果が描画されない」手順を固定できる。

function nextSearchGeneration(current) {
  return (Number.isSafeInteger(current) && current >= 0 ? current : 0) + 1;
}

function isCurrentSearchRequest(current, captured) {
  return Number.isSafeInteger(current)
    && Number.isSafeInteger(captured)
    && current === captured;
}

// --- 複数選択の pure helper ------------------------------------------------
// いずれも入力を変更せず新しい配列を返す。path をキーにするのは core の
// Selection（core/src/model.rs）と同じ不変条件（選択はパスで残す）に合わせるため。

// Ctrl+クリック相当のトグル。既存なら外し、無ければ末尾へ追加する。
function selectionToggle(paths, path) {
  if (typeof path !== 'string' || path.length === 0) return (paths || []).slice();
  const source = Array.isArray(paths) ? paths : [];
  const index = source.indexOf(path);
  if (index < 0) return [...source, path];
  return source.filter((item) => item !== path);
}

// 選択の全解除。毎回新しい配列を返す（呼び出し側の配列を共有しない）。
function selectionClear() {
  return [];
}

function selectionSelectSingle(path) {
  return typeof path === 'string' && path.length ? [path] : [];
}

function selectionSelectAll(entries) {
  return (Array.isArray(entries) ? entries : []).map((entry) => entry.path);
}

// Shift+クリック相当。anchor から target までを entries の並び順で両端込みで返す。
// anchor が未設定 / 一覧外なら target 単独へフォールバックする（core と同挙動）。
function selectionRange(entries, anchor, target) {
  const list = Array.isArray(entries) ? entries : [];
  const paths = list.map((entry) => entry.path);
  const targetIndex = paths.indexOf(target);
  if (targetIndex < 0) return [];
  const anchorIndex = anchor == null ? -1 : paths.indexOf(anchor);
  if (anchorIndex < 0) return [target];
  const start = Math.min(anchorIndex, targetIndex);
  const end = Math.max(anchorIndex, targetIndex);
  return paths.slice(start, end + 1);
}

// 範囲選択ボタンの arm 状態を決める pure helper（BB15）。
// ボタンではトグルし、1 回の範囲選択が完了したら completed に関わらず解除する。
function nextRangeArmed(currentArmed, { completed = false } = {}) {
  if (completed) return false;
  return !currentArmed;
}

// 再描画時に旧選択パスのうち新一覧にも残っているものだけを残す（BA07）。
// prev の順序を保つ。next に無いパス（検索で消えた項目）は選択から外れる。
function retainSelectedPaths(prev, next) {
  const previous = Array.isArray(prev) ? prev : [];
  const nextSet = new Set(Array.isArray(next) ? next : []);
  return previous.filter((path) => nextSet.has(path));
}

// --- 自然順ソートの pure helper --------------------------------------------
// core の alphanumeric_sort::compare_str と同じ「数字列を数値として比較」を
// 実装し、大小文字無視の比較が同値なら大小文字区別の比較でタイブレークする。

function compareNatural(a, b) {
  const left = String(a ?? '');
  const right = String(b ?? '');
  let i = 0;
  let j = 0;
  while (i < left.length && j < right.length) {
    const ca = left[i];
    const cb = right[j];
    const digitA = ca >= '0' && ca <= '9';
    const digitB = cb >= '0' && cb <= '9';
    if (digitA && digitB) {
      let iEnd = i;
      let jEnd = j;
      while (iEnd < left.length && left[iEnd] >= '0' && left[iEnd] <= '9') iEnd += 1;
      while (jEnd < right.length && right[jEnd] >= '0' && right[jEnd] <= '9') jEnd += 1;
      const rawA = left.slice(i, iEnd);
      const rawB = right.slice(j, jEnd);
      const numA = rawA.replace(/^0+/, '') || '0';
      const numB = rawB.replace(/^0+/, '') || '0';
      if (numA.length !== numB.length) return numA.length < numB.length ? -1 : 1;
      if (numA !== numB) return numA < numB ? -1 : 1;
      // 数値が等しいときは先頭ゼロの少ない方を前にする（桁あわせの安定化）。
      const zerosA = rawA.length - numA.length;
      const zerosB = rawB.length - numB.length;
      if (zerosA !== zerosB) return zerosA < zerosB ? -1 : 1;
      i = iEnd;
      j = jEnd;
      continue;
    }
    if (ca !== cb) return ca < cb ? -1 : 1;
    i += 1;
    j += 1;
  }
  if (i < left.length) return 1;
  if (j < right.length) return -1;
  return 0;
}

function compareNames(a, b) {
  const first = compareNatural(String(a ?? '').toLowerCase(), String(b ?? '').toLowerCase());
  return first !== 0 ? first : compareNatural(a, b);
}

function extensionOf(name) {
  const value = String(name ?? '');
  const dot = value.lastIndexOf('.');
  if (dot <= 0 || dot === value.length - 1) return '';
  return value.slice(dot + 1).toLowerCase();
}

function kindSortKey(entry) {
  if (entry && entry.isDir) return '';
  return extensionOf(entry ? entry.name : '');
}

function compareModified(a, b) {
  const aMissing = a === null || a === undefined || a === '';
  const bMissing = b === null || b === undefined || b === '';
  if (aMissing || bMissing) {
    if (aMissing && bMissing) return 0;
    return aMissing ? -1 : 1;
  }
  if (typeof a === 'number' && typeof b === 'number') return a < b ? -1 : a > b ? 1 : 0;
  const left = String(a);
  const right = String(b);
  return left < right ? -1 : left > right ? 1 : 0;
}

// core の SortOrder と同じ方針: ディレクトリ常に先頭、指定キー、名前でタイブレーク。
// key: 'name' | 'kind' | 'size' | 'modified'、dir: 'asc' | 'desc'。
// 入力配列は変更しない（純粋関数）。
function sortEntries(entries, key = 'name', dir = 'asc') {
  const source = Array.isArray(entries) ? entries : [];
  const descending = String(dir).toLowerCase() === 'desc';
  const comparators = {
    name: (a, b) => compareNames(a.name, b.name),
    kind: (a, b) => compareNatural(kindSortKey(a), kindSortKey(b)),
    size: (a, b) => {
      const sizeA = Number(a.size) || 0;
      const sizeB = Number(b.size) || 0;
      return sizeA < sizeB ? -1 : sizeA > sizeB ? 1 : 0;
    },
    modified: (a, b) => compareModified(a.modified, b.modified),
  };
  const primary = comparators[key] || comparators.name;
  return source.slice().sort((a, b) => {
    const dirsFirst = Number(Boolean(b.isDir)) - Number(Boolean(a.isDir));
    if (dirsFirst !== 0) return dirsFirst;
    let result = primary(a, b);
    if (result === 0) result = compareNames(a.name, b.name);
    return descending ? -result : result;
  });
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
  // wasm 初期化完了などの通常通知で更新通知を消さない。エラーは重要なので通す。
  if (state.updateNotified && level !== 'error') return;
  els.status.textContent = message;
  els.status.dataset.level = level;
}

// SW 更新通知の判定を純粋関数にして node --test から検証できるようにする。
// 初回インストール（制御中の SW が無い状態）では通知しない。
function shouldNotifySwUpdate({
  hasController = false,
  workerState = '',
  alreadyNotified = false,
} = {}) {
  return Boolean(hasController) && workerState === 'installed' && !alreadyNotified;
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

// 現在のセレクト値を pure sortEntries へ渡すための薄いラッパー。
function currentSort() {
  return {
    key: hasDom && els.sortKey ? els.sortKey.value : 'name',
    dir: hasDom && els.sortDir ? els.sortDir.value : 'asc',
  };
}

// ソート条件を変更したとき、最後の入力（一覧 or 検索結果）を同じ見出しで描き直す。
// render() 自身が sortEntries を通すため、選択は path で保持されたまま残る。
function rerender() {
  const { entries, options } = state.lastRender;
  render(entries, options);
}

function render(entries, { heading, query = '', mode = 'list' } = {}) {
  const options = { heading, query, mode };
  state.lastRender = { entries, options };
  const view = sortEntries(entries, currentSort().key, currentSort().dir);

  // BB05: 一覧内のフォーカスは replaceChildren() で失われるため、先に保存する。
  // 一覧外（検索欄 / ソート select / ボタン）にフォーカスがある場合は何もしない。
  const focusWasInList = captureListFocus();

  els.title.textContent = heading;
  els.count.textContent = `${view.length} 件`;
  els.list.replaceChildren();
  state.renderedEntries = view;

  // 選択は object 参照ではなく path で保持する。再描画 / 検索 / 再ソートで
  // エントリが差し替わっても新一覧に残っているパスだけを再選択する（BA07）。
  state.selectedPaths = retainSelectedPaths(
    state.selectedPaths,
    view.map((entry) => entry.path),
  );
  if (state.anchorPath && !state.selectedPaths.includes(state.anchorPath)) {
    state.anchorPath = state.selectedPaths.length ? state.selectedPaths[0] : null;
  }
  if (state.focusIndex >= view.length) state.focusIndex = Math.max(0, view.length - 1);

  if (view.length === 0) {
    const li = document.createElement('li');
    // 空状態の li は listbox の選択肢ではないため、支援技術へ選択肢として
    // 読み上げられないよう role=presentation を付与する（状態通知は #status の
    // aria-live に一本化済み）。
    li.setAttribute('role', 'presentation');
    li.className = 'empty';

    const text = document.createElement('span');
    text.className = 'empty-text';
    // クエリをエコーする文言も textContent 経由なので HTML として実行されない。
    text.textContent = emptyStateText(query, mode);
    li.append(text);

    // 検索0件のときだけ復帰導線を出す。一覧が空のときはクリアする検索が無いので出さない。
    if (shouldShowClearSearch(mode)) {
      const button = document.createElement('button');
      button.type = 'button';
      button.className = 'empty-clear';
      button.textContent = '検索をクリア';
      button.addEventListener('click', () => {
        clearSearch().catch((error) =>
          setStatus(`検索のクリアに失敗: ${errorMessage(error)}`, 'error'));
      });
      li.append(button);
    }
    els.list.append(li);
    applySelectionAria();
    // 空状態には復元する行が無い。検索0件なら検索欄へ戻し、body 落下を防ぐ。
    restoreListFocus(focusWasInList, { fallbackToSearch: true });
    return;
  }

  const fragment = document.createDocumentFragment();
  for (const [index, entry] of view.entries()) {
    const li = document.createElement('li');
    li.dataset.index = String(index);
    li.setAttribute('role', 'option');
    li.setAttribute('aria-selected', 'false');
    li.tabIndex = index === state.focusIndex ? 0 : -1;
    // BE18: 省略名の行でもパスを確認できるよう行全体にツールチップを付ける。
    li.title = entry.path;

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
  applySelectionAria();
  restoreListFocus(focusWasInList);
}

// --- 選択 / タッチ / マウス / キーボード操作 -------------------------------
// 一覧は差し替わるため、リスナーは els.list への委譲で 1 回だけ登録する。

function entryAt(index) {
  const i = Number(index);
  return Number.isInteger(i) && i >= 0 && i < state.renderedEntries.length
    ? state.renderedEntries[i]
    : null;
}

function isPathSelected(path) {
  return typeof path === 'string' && state.selectedPaths.includes(path);
}

function selectedEntries() {
  return state.renderedEntries.filter((entry) => isPathSelected(entry.path));
}

// 複数選択 listbox では aria-selected を各行へ個別に反映する。
// tabIndex は「フォーカス行」だけ 0 にする（選択行とは独立）。
function applySelectionAria() {
  if (!hasDom) return;
  for (const li of els.list.querySelectorAll('li[data-index]')) {
    const index = Number(li.dataset.index);
    const entry = entryAt(index);
    li.setAttribute('aria-selected', String(Boolean(entry && isPathSelected(entry.path))));
    li.tabIndex = index === state.focusIndex ? 0 : -1;
  }
  updateSelectModeUi();
}

// BB05: 一覧は render() で丸ごと差し替わるため、フォーカス中の行も
// replaceChildren() で消えて activeElement が body へ落ちる。描画前に
// 「一覧内にフォーカスがあったか」を保存し、描画後に同じ行へ戻す。
function captureListFocus() {
  return Boolean(hasDom && els.list && els.list.contains(document.activeElement));
}

function restoreListFocus(focusWasInList, { fallbackToSearch = false } = {}) {
  if (!focusWasInList || !hasDom) return;
  // 呼び出し時点で state.renderedEntries は新しい view へ更新済み。
  if (entryAt(state.focusIndex)) {
    focusRow(state.focusIndex, { fromKeyboard: false });
    return;
  }
  // 空状態には li[data-index] が無いため、検索欄（復帰導線）へ逃がす。
  if (fallbackToSearch && els.search) els.search.focus({ preventScroll: true });
}

function focusRow(index, { fromKeyboard = false } = {}) {
  if (!entryAt(index)) return;
  state.focusIndex = index;
  applySelectionAria();
  const li = els.list.querySelector(`li[data-index="${index}"]`);
  if (!li) return;
  li.focus({ preventScroll: true });
  // preventScroll だけだとキーボード連続選択が画面外へ消えるため、
  // キーボード操作のときだけ nearest で viewport 内へ追従させる。
  if (fromKeyboard && typeof li.scrollIntoView === 'function') {
    li.scrollIntoView({ block: 'nearest' });
  }
}

// 通常タップ / 矢印キー: 単一選択へ置き換える（選択モード外の既定動作）。
function selectSingle(index, { fromKeyboard = false } = {}) {
  const entry = entryAt(index);
  if (!entry) return;
  state.selectedPaths = selectionSelectSingle(entry.path);
  state.anchorPath = entry.path;
  setStatus(`選択: ${entry.name}`);
  focusRow(index, { fromKeyboard });
}

// 選択モードのタップ: 1 行をトグルする。アンカーは最後に触れた行へ追従する
// （core の Selection::toggle と同じ規則）。
function toggleAt(index) {
  const entry = entryAt(index);
  if (!entry) return;
  state.selectedPaths = selectionToggle(state.selectedPaths, entry.path);
  state.anchorPath = state.selectedPaths.length
    ? state.selectedPaths[state.selectedPaths.length - 1]
    : null;
  state.focusIndex = index;
  // 件数は #select-count の aria-live が知らせる（#status とは二重に読まない）。
  setStatus(`${isPathSelected(entry.path) ? '選択' : '解除'}: ${entry.name}`);
  focusRow(index);
}

// 範囲選択ボタンで arm した後のタップ: アンカーから当該行までを選択する。
// 1 回の範囲選択で arm を消費し、ボタン表示と終了通知を出す（BB15）。
function selectRangeTo(index) {
  const entry = entryAt(index);
  if (!entry) return;
  const anchor = state.anchorPath ?? entry.path;
  state.selectedPaths = selectionRange(state.renderedEntries, anchor, entry.path);
  state.focusIndex = index;
  state.rangeArmed = nextRangeArmed(state.rangeArmed, { completed: true });
  updateSelectModeUi();
  // 選択件数は #select-count（aria-live）が知らせるため二重に読ませない（BB24）。
  setStatus('範囲選択が完了しました（範囲選択モードを終了）');
  focusRow(index);
}

function selectAllEntries() {
  state.selectedPaths = selectionSelectAll(state.renderedEntries);
  if (state.selectedPaths.length) state.anchorPath = state.selectedPaths[0];
  setStatus('すべて選択しました');
  applySelectionAria();
}

function clearSelection() {
  state.selectedPaths = selectionClear();
  state.anchorPath = null;
  setStatus('選択を解除しました');
  applySelectionAria();
}

function enterSelectMode() {
  if (state.selectMode) return;
  state.selectMode = true;
  state.rangeArmed = false;
  updateSelectModeUi();
  setStatus('選択モード: 行をタップして選択 / すべて選択 / 範囲選択 / 解除', 'info');
}

function exitSelectMode() {
  if (!state.selectMode) return;
  state.selectMode = false;
  state.rangeArmed = false;
  state.selectedPaths = selectionClear();
  state.anchorPath = null;
  updateSelectModeUi();
  applySelectionAria();
  setStatus('選択モードを終了しました');
}

// 選択モードのトグルボタン / 一括操作の表示を同期する。
function updateSelectModeUi() {
  if (!hasDom) return;
  if (els.selectMode) {
    els.selectMode.setAttribute('aria-pressed', String(state.selectMode));
    els.selectMode.textContent = state.selectMode ? '選択モード: ON' : '選択モード';
  }
  if (els.selectActions) els.selectActions.hidden = !state.selectMode;
  if (els.selectRange) els.selectRange.setAttribute('aria-pressed', String(state.rangeArmed));
  if (els.selectCount) {
    els.selectCount.textContent = state.selectMode ? `${state.selectedPaths.length} 件選択中` : '';
  }
}

// キーボードの移動。選択モードではフォーカスのみ動かし、Enter でトグルする
// （複数選択 listbox の標準的な挙動）。通常モードは単一選択を追従させる。
function moveFocusTo(index, { fromKeyboard = true } = {}) {
  if (!entryAt(index)) return;
  if (state.selectMode) focusRow(index, { fromKeyboard });
  else selectSingle(index, { fromKeyboard });
}

function moveSelection(delta) {
  const length = state.renderedEntries.length;
  if (length === 0) return;
  const start = state.focusIndex < 0 ? (delta > 0 ? -1 : 0) : state.focusIndex;
  moveFocusTo(Math.min(length - 1, Math.max(0, start + delta)), { fromKeyboard: true });
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
  // BE02: dblclick の第2クリックはトグルを相殺するためここで捨てる。
  if (secondClickSuppressed(event.detail)) return;
  const { li, entry } = entryFromEvent(event);
  if (!li || !entry) return;
  const index = Number(li.dataset.index);
  if (state.selectMode) {
    if (state.rangeArmed) selectRangeTo(index);
    else toggleAt(index);
  } else {
    selectSingle(index);
  }
}

// BE02: ダブルクリックは click の第2回を抑制したうえで、既存の詳細表示 /
// 選択確定経路へ接続する。選択モードでは Enter と同じ「選択確定」を出す。
function onListDblClick(event) {
  if (isClickSuppressed(performance.now(), state.suppressClickUntil)) return;
  if (!doubleClickCommand(event.detail)) return;
  const { entry } = entryFromEvent(event);
  if (!entry) return;
  event.preventDefault();
  if (state.selectMode && state.selectedPaths.length) {
    const chosen = selectedEntries()[0] ?? entry;
    showEntryMeta(chosen);
    setStatus(`選択確定: ${chosen.name}（${state.selectedPaths.length} 件）`);
    return;
  }
  showEntryMeta(entry);
}

function onListContextMenu(event) {
  // ブラウザ標準のコンテキストメニューを抑制し、マウスの右クリックは
  // 詳細表示として扱う（タッチの長押しは選択モード突入へ変更済み）。
  event.preventDefault();
  const { entry } = entryFromEvent(event);
  if (entry) showEntryMeta(entry);
}

// 長押しは touchstart 時点の行 index を捕捉していたが、500ms の間に検索完了で
// 再描画されると別行をトグルしてしまう（BD12）。path で現在の一覧を再解決し、
// 見つからなければ（-1）発火自体をスキップする。入力は変更しない pure helper。
function longPressTargetIndex(entries, path) {
  if (typeof path !== 'string' || path.length === 0) return -1;
  const list = Array.isArray(entries) ? entries : [];
  return list.findIndex((entry) => Boolean(entry) && entry.path === path);
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
  // キャンセルしないと、長押しコールバックが残って選択モードへ誤突入する。
  if (document.activeElement === els.search || event.touches.length !== 1) {
    clearLongPress();
    state.touch = null;
    return;
  }
  const touch = event.touches[0];
  const { li, entry } = entryFromEvent(event);
  const index = li ? Number(li.dataset.index) : -1;
  // 長押しの対象は数値 index ではなく path で捕捉する（BD12）。500ms の間に
  // 検索完了で一覧が描き直されても、発火時に同じ行を探し直せるようにする。
  const path = entry ? entry.path : null;
  state.touch = {
    startX: touch.clientX,
    startY: touch.clientY,
    index,
    entry,
    path,
    moved: false,
    // 画面左端から始まるジェスチャは OS の戻る操作へ譲る。
    swipeEnabled: touch.clientX > SWIPE_EDGE_GUARD,
    timer: 0,
  };
  if (entry) {
    // 長押しは詳細表示ではなく「選択モードへ突入 + 当該行をトグル」へ変更（BA19）。
    // 詳細表示はキーボードの Space / i とマウスの右クリックに退避する。
    state.touch.timer = window.setTimeout(() => {
      state.touch = null;
      // 現在の一覧から path の位置を再解決する。旧 index のままトグルすると
      // 再描画後に別行を選択してしまうため、見つからなければ何もしない。
      const targetIndex = longPressTargetIndex(state.renderedEntries, path);
      if (targetIndex < 0) return;
      state.rangeArmed = false;
      enterSelectMode();
      toggleAt(targetIndex);
      // 長押し直後に発火する click で二重トグルされないようにする。
      state.suppressClickUntil = performance.now() + CLICK_SUPPRESS_MS;
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

// スワイプバックと空状態の「検索をクリア」ボタンから共通で呼ぶ。
async function clearSearch() {
  window.clearTimeout(state.debounce);
  // デバウンス解除に加え、既に await 中の検索も無効化する（BD05）。
  // runSearch('') → refreshList() でも世代が進むが、単調なので重複は無害。
  state.searchGeneration = nextSearchGeneration(state.searchGeneration);
  els.search.value = '';
  state.selectedPaths = selectionClear();
  state.anchorPath = null;
  setStatus('検索をクリアして一覧へ戻りました');
  await runSearch('');
}

async function onSwipeBack() {
  // スワイプ後の click が行選択として誤処理されないようにする（ボタン側は不要）。
  state.suppressClickUntil = performance.now() + CLICK_SUPPRESS_MS;
  await clearSearch();
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
      moveFocusTo(0);
      return;
    case 'end':
      event.preventDefault();
      moveFocusTo(state.renderedEntries.length - 1);
      return;
    // BB04: キーボードの脱出経路。arm → 選択モード → 選択の順で一段ずつ戻す。
    case 'escape':
      event.preventDefault();
      if (state.rangeArmed) {
        state.rangeArmed = false;
        updateSelectModeUi();
        setStatus('範囲選択を解除しました');
      } else if (state.selectMode) {
        exitSelectMode();
      } else {
        clearSelection();
      }
      return;
    // 詳細表示（path / size / isDir）は長押しから Space / i へ退避（BA19）。
    // Space はページスクロールを止める。修飾キー付きは listKeyCommand が
    // 既に null を返すため、ブラウザ / OS のショートカットを横取りしない。
    case 'details': {
      event.preventDefault();
      const { entry } = entryFromEvent(event);
      const firstSelected = selectedEntries()[0];
      showEntryMeta(entry ?? firstSelected ?? entryAt(state.focusIndex));
      return;
    }
    case 'enter': {
      const { li, entry } = entryFromEvent(event);
      const index = li ? Number(li.dataset.index) : state.focusIndex;
      if (state.selectMode && entryAt(index)) {
        // 選択モードの Enter は当該行のトグル（範囲 arm 中は範囲選択）。
        event.preventDefault();
        if (state.rangeArmed) selectRangeTo(index);
        else toggleAt(index);
        return;
      }
      const chosen = entry ?? entryAt(state.focusIndex);
      if (!chosen) return;
      event.preventDefault();
      if (index >= 0) focusRow(index, { fromKeyboard: true });
      setStatus(`選択確定: ${chosen.name}`);
      return;
    }
    default:
      return;
  }
}

// --- コア API 呼び出し（主要経路） ----------------------------------------

// 受け入れ経路は in-memory API（listEntriesFromJson / searchInJson）。
// std::fs を使う list_entries(dir) / search(query, dir) はフォールバック専用。
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
  // 開始ごとに世代を進めて捕捉する。await 完了時に現役でなければ、結果も
  // エラーも一切反映しない（BD05）。
  const generation = state.searchGeneration = nextSearchGeneration(state.searchGeneration);
  if (state.wasmReady && state.viewer) {
    try {
      let raw;
      if (typeof state.module?.listEntriesFromJson === 'function') {
        raw = state.module.listEntriesFromJson(demoJson());
      } else {
        raw = await callCore(state.viewer, 'list_entries', [state.rootName]);
      }
      if (!isCurrentSearchRequest(state.searchGeneration, generation)) return;
      const decoded = decodeCorePayload(raw);
      if (!decoded.ok) throw new Error(decoded.error);
      state.entries = normalizeEntries(decoded.data);
      render(state.entries, { heading: '一覧（core API）', mode: 'list' });
      setStatus(`core API 経由で ${state.entries.length} 件を表示`);
      return;
    } catch (error) {
      if (!isCurrentSearchRequest(state.searchGeneration, generation)) return;
      // wasm 上の std::fs は実ファイルを読めない。エラーを隠さず表示したうえで
      // scaffold の確認用に読み取り専用デモへ退避する（受け入れ基準を満たさない）。
      setStatus(`core API の一覧取得に失敗: ${errorMessage(error)}`, 'error');
      fillDemoIfEmpty();
    }
  }
  if (!isCurrentSearchRequest(state.searchGeneration, generation)) return;
  render(state.entries, { heading: '一覧（読み取り専用デモ）', mode: 'list' });
}

async function runSearch(query) {
  const generation = state.searchGeneration = nextSearchGeneration(state.searchGeneration);
  const q = query.trim();
  if (!q) {
    // 空クエリは一覧取得へ委譲する（refreshList 側でも世代を進める）。
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
      if (!isCurrentSearchRequest(state.searchGeneration, generation)) return;
      const decoded = decodeCorePayload(raw);
      if (!decoded.ok) throw new Error(decoded.error);
      const results = normalizeEntries(decoded.data);
      render(results, { heading: `検索: ${q}（core API）`, query: q, mode: 'search' });
      setStatus(`core API の検索で ${results.length} 件`);
      return;
    } catch (error) {
      if (!isCurrentSearchRequest(state.searchGeneration, generation)) return;
      setStatus(`core API の検索に失敗: ${errorMessage(error)}`, 'error');
      fillDemoIfEmpty();
    }
  }

  if (!isCurrentSearchRequest(state.searchGeneration, generation)) return;
  const filtered = state.entries.filter((entry) =>
    entry.name.toLowerCase().includes(q.toLowerCase()),
  );
  render(filtered, { heading: `検索: ${q}（読み取り専用デモ）`, query: q, mode: 'search' });
}

// --- File System Access API（対応ブラウザのみ） ----------------------------

async function onPickDirectory() {
  if (!supportsFileSystemAccess() || !state.wasmReady) return;
  try {
    const handle = await window.showDirectoryPicker({ mode: 'read' });
    state.rootName = handle.name;
    setStatus(`選択: ${handle.name}`);
    // TODO(P1-04): FileSystemDirectoryHandle を core の FileSystem ポートへ橋渡しする。
    // 現状はルート名を状態に保持するだけ。一覧・検索は demoJson() の in-memory API
    // 経路で表示し、ルート名は core へ渡さない。実ツリーの走査には OPFS 側の移植が必要
    // （notes/wasm-browser-notes.md §2「ファイルシステム抽象化」を参照）。
    if (state.wasmReady) await refreshList();
  } catch (error) {
    if (error && error.name === 'AbortError') return;
    setStatus(`フォルダ選択に失敗: ${errorMessage(error)}`, 'error');
  }
}

// --- Service Worker の更新ライフサイクル（BH02 / BH16） ---------------------

function announceSwUpdate(registration, worker) {
  state.updateNotified = true;
  // #status は role=status / aria-live=polite。直接書いて更新を読み上げる。
  els.status.textContent = '新しい版が利用可能です。「新しい版を読み込む」を押すと更新されます。';
  els.status.dataset.level = 'info';

  if (els.updateReload) {
    els.updateReload.hidden = false;
    els.updateReload.addEventListener('click', () => {
      const waiting = registration.waiting || worker;
      if (waiting && typeof waiting.postMessage === 'function') {
        // install では skipWaiting() しない代わりに、ここで待機 SW を進める。
        waiting.postMessage({ type: 'SKIP_WAITING' });
      } else {
        window.location.reload();
      }
    }, { once: true });
  }

  // 新 SW が control を取った時点で再読込し、新しいシェルを反映する。
  navigator.serviceWorker.addEventListener('controllerchange', () => {
    window.location.reload();
  }, { once: true });
}

function watchInstallingWorker(registration, worker) {
  worker.addEventListener('statechange', () => {
    if (shouldNotifySwUpdate({
      hasController: Boolean(navigator.serviceWorker.controller),
      workerState: worker.state,
      alreadyNotified: state.updateNotified,
    })) {
      announceSwUpdate(registration, worker);
    }
  });
}

function watchServiceWorker(registration) {
  // ページを開いた時点で待機 SW が居る場合（前回の通知を無視した等）も通知する。
  const waiting = registration.waiting;
  if (shouldNotifySwUpdate({
    hasController: Boolean(navigator.serviceWorker.controller),
    workerState: waiting && waiting.state,
    alreadyNotified: state.updateNotified,
  })) {
    announceSwUpdate(registration, waiting);
  }
  if (registration.installing) watchInstallingWorker(registration, registration.installing);
  registration.addEventListener('updatefound', () => {
    if (registration.installing) watchInstallingWorker(registration, registration.installing);
  });
}

function registerServiceWorker() {
  if (typeof navigator === 'undefined' || !('serviceWorker' in navigator)) return;
  navigator.serviceWorker.register('./sw.js')
    .then((registration) => watchServiceWorker(registration))
    .catch((error) => {
      console.warn('service worker の登録に失敗しました:', error);
    });
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
  render(state.entries, { heading: '一覧（読み取り専用デモ）', mode: 'list' });
}

async function boot() {
  // BG13: service worker を登録してオフライン起動可能にする。登録失敗は
  // オンライン動作へ影響しないため警告どまり（file:// / 非対応ブラウザも継続）。
  // 更新ライフサイクル（updatefound → 通知 → SKIP_WAITING → reload）もここで配線する。
  registerServiceWorker();

  els.search.addEventListener('input', () => {
    window.clearTimeout(state.debounce);
    // 新しい入力の時点で実行中リクエストを無効化する。デバウンス解除だけだと
    // 150ms の待ちの間に古い結果が描画されてしまう（BD05）。
    state.searchGeneration = nextSearchGeneration(state.searchGeneration);
    state.debounce = window.setTimeout(() => runSearch(els.search.value), SEARCH_DEBOUNCE_MS);
  });

  // ここでは検索 input の Enter には触れず、一覧内の操作だけを扱う。
  els.list.setAttribute('role', 'listbox');
  // 範囲選択できることを支援技術へ伝える（index.html にも静的に付与済み）。
  els.list.setAttribute('aria-multiselectable', 'true');
  els.list.addEventListener('click', onListClick);
  // BE02: click の第2回を抑制した分を dblclick で詳細表示 / 選択確定へ接続。
  els.list.addEventListener('dblclick', onListDblClick);
  els.list.addEventListener('contextmenu', onListContextMenu);
  els.list.addEventListener('keydown', onListKeyDown);
  els.list.addEventListener('touchstart', onListTouchStart, { passive: true });
  els.list.addEventListener('touchmove', onListTouchMove, { passive: true });
  els.list.addEventListener('touchend', onListTouchEnd, { passive: true });
  els.list.addEventListener('touchcancel', onListTouchCancel, { passive: true });

  // 修飾キー（Ctrl / Shift / Cmd / Alt）は横取りせず、専用トグルで複数選択を
  // 提供する。デスクトップ版の Ctrl+クリック / Shift+範囲 / Ctrl+A に相当する
  // 導線を、タッチしやすいボタンとして index.html に配置している。
  if (els.selectMode) {
    els.selectMode.addEventListener('click', () => {
      if (state.selectMode) exitSelectMode();
      else enterSelectMode();
    });
  }
  if (els.selectAll) els.selectAll.addEventListener('click', selectAllEntries);
  if (els.selectClear) els.selectClear.addEventListener('click', clearSelection);
  if (els.selectRange) {
    els.selectRange.addEventListener('click', () => {
      state.rangeArmed = nextRangeArmed(state.rangeArmed);
      updateSelectModeUi();
      setStatus(state.rangeArmed
        ? '範囲選択: 開始行をタップ、終端行をタップ'
        : '範囲選択を解除しました');
    });
  }
  if (els.sortKey) els.sortKey.addEventListener('change', rerender);
  if (els.sortDir) els.sortDir.addEventListener('change', rerender);
  updateSelectModeUi();

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
    // BE02: ダブルクリックの第2 click 抑制と活性化コマンド。
    secondClickSuppressed,
    doubleClickCommand,
    // 非同期検索の世代ガード / 長押し対象の再解決（BD05 / BD12）。
    nextSearchGeneration,
    isCurrentSearchRequest,
    // SW 更新通知の判定（BH02）。
    shouldNotifySwUpdate,
    longPressTargetIndex,
    canPickDirectory,
    emptyStateText,
    shouldShowClearSearch,
    shouldSwipeBack,
    listKeyCommand,
    updatePickDirAvailability,
    toEntry,
    // 複数選択 / 残存 / ソートの pure helper（node --test から直接検証する）。
    selectionToggle,
    selectionClear,
    selectionSelectSingle,
    selectionSelectAll,
    selectionRange,
    nextRangeArmed,
    retainSelectedPaths,
    compareNatural,
    compareNames,
    sortEntries,
  };
}
