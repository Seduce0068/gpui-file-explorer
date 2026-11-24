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
// BI03: 更新通知は #status とは別の aria-live ノードへ書くための文言。
// 更新後も通常の setStatus が #status を更新し続けられるようにする。
const SW_UPDATE_MESSAGE = '新しい版が利用可能です。「新しい版を読み込む」を押すと更新されます。';

// Node's `node --test` imports this module to exercise the pure helpers.
// Only the browser has `document`; skip DOM wiring there.
const hasDom = typeof document !== 'undefined';
const els = hasDom ? {
  status: document.getElementById('status'),
  // BI03: 更新通知専用の aria-live ノード（#status 直下に配置）。
  swUpdate: document.getElementById('sw-update'),
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
  // BM03: タッチ専用の詳細表示ボタン（選択操作行に配置）。
  selectDetails: document.getElementById('select-details'),
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
  // BJ01: 通知した worker を記録する。別の worker が installed になったら
  // updateNotified を false へ戻し、次の更新も通知できるようにする。
  notifiedWorker: null,
  // BJ08: 更新ボタンの配線済みフラグと SKIP_WAITING の送信状態。
  // click は once にしない（activate 失敗時に再送できるようにするため）。
  updateReloadWired: false,
  controllerReloadWired: false,
  // BM12: SW が activate 後に全クライアントへ送る RELOAD の配線状態。
  // 自タブの controllerchange と二重になっても 1 回だけ再読込する。
  reloadMessageWired: false,
  reloadRequested: false,
  skipWaitingSent: false,
  // BL05/BL06: 送信先 worker と送信時刻。前回更新の保留リロードを新しい通知で
  // 失わないため、また別由来の controllerchange で誤リロードしないための追跡。
  skipWaitingWorker: null,
  skipWaitingAt: 0,
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

// BN10: スワイプバックの後始末を描画モードから決める pure helper。
// 検索結果から一覧へ戻るときだけ検索をクリアする。通常一覧では選択を維持し、
// 「検索をクリアして一覧へ戻りました」と誤通知しない。
function shouldClearSearchOnSwipeBack(mode) {
  return mode === 'search';
}

// BM11: スワイプ追跡の可否。検索欄フォーカス中でも左端以外なら追跡する。
// 旧実装はフォーカス中に touchstart 全体を返していたため右スワイプまで
// ブロックされていた。OS の戻るジェスチャと競合するのは左端だけなので、
// 無効化すべきは長押しタイマー（呼び出し側の !searchFocused）のみ。
function canTrackSwipe(startX, searchFocused = false) {
  if (!Number.isFinite(startX)) return false;
  // 非フォーカス時は従来どおり追跡し、swipeEnabled 側で左端を除外する。
  if (!searchFocused) return true;
  // フォーカス中は左端のジェスチャだけ OS へ譲る。
  return startX > SWIPE_EDGE_GUARD;
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

// BO06: ソートや検索で一覧が差し替わると、同じ focusIndex が別行を指す。
// フォーカス行を path で新一覧へ再解決し、見つからなければ anchorPath
// （最後に触れた行）へ逃がす。両方とも無い場合は index を新長さへクランプ
// する。入力を変更しない純粋関数（描画前に旧一覧を渡すこと）。
function resolveFocusIndex(previousEntries, nextEntries, focusIndex, anchorPath) {
  const next = Array.isArray(nextEntries) ? nextEntries : [];
  if (next.length === 0) return 0;
  const previous = Array.isArray(previousEntries) ? previousEntries : [];
  const index = Number.isInteger(focusIndex) ? focusIndex : -1;
  const focusedPath = index >= 0 && index < previous.length
    ? previous[index]?.path
    : null;
  for (const candidate of [focusedPath, anchorPath]) {
    if (typeof candidate !== 'string' || candidate.length === 0) continue;
    const resolved = next.findIndex((entry) => entry && entry.path === candidate);
    if (resolved >= 0) return resolved;
  }
  return Math.min(Math.max(index, 0), next.length - 1);
}

// BN05: 選択操作行の「詳細」ボタンが表示する行を決める pure helper。
// selectedEntries()[0] は描画順の先頭にすぎず、最後に触れた行とは限らない。
// フォーカス行が選択されていればそれを優先し、そうでなければ anchorPath
// （toggleAt / selectRangeTo / selectSingle が更新する最後に触れた行）を解決する。
function detailTargetEntry(entries, selectedPaths, focusIndex, anchorPath) {
  const list = Array.isArray(entries) ? entries : [];
  const selected = new Set(Array.isArray(selectedPaths) ? selectedPaths : []);
  const focus = Number.isInteger(focusIndex) && focusIndex >= 0 && focusIndex < list.length
    ? list[focusIndex]
    : null;
  if (focus && selected.has(focus.path)) return focus;
  if (typeof anchorPath === 'string' && anchorPath.length > 0) {
    const anchored = list.find((entry) => entry && entry.path === anchorPath);
    if (anchored && selected.has(anchored.path)) return anchored;
  }
  // アンカーが一覧外へ消えた場合の安全網（従来の selectedEntries()[0] 相当）。
  const firstSelected = list.find((entry) => entry && selected.has(entry.path));
  return firstSelected ?? focus ?? null;
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
  // BI03: 更新通知は #sw-update へ分離したため、updateNotified に関係なく
  // 通常通知は #status へ流す。以前はここで永久に抑制され、検索結果や
  // 選択結果が一切読み上げられなくなっていた。
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
    // BO14: core が oversized 等で使う kind を落とさずビューへ引き継ぐ。
    kind: pick(value, 'kind'),
    size: Number(pick(value, 'size', 'len') ?? 0),
    // BP08: core は camelCase の `modifiedMs` を返す（core/src/wasm.rs:95）。
    // 旧 `modified` も受け付け、どちらもビューア側の `modified` へ正規化する。
    modified: pick(value, 'modified', 'modifiedMs'),
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
  // BO06: 再描画前のフォーカス行 path を resolveFocusIndex へ渡すため保存する。
  const previousEntries = state.renderedEntries;

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
  // BO06: ソートで index が入れ替わっても同じ行（無ければ anchorPath）に
  // フォーカスを再解決し、BN05 の詳細対象が変わらないようにする。
  state.focusIndex = resolveFocusIndex(
    previousEntries, view, state.focusIndex, state.anchorPath,
  );

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

// BM03: 選択操作行の「詳細」ボタンを配線する。長押しは選択モード突入へ変更され、
// Space / i はキーボード専用なので、タッチだけで詳細へ到達できる導線が必要。
// BN05: 描画順の先頭ではなく、最後に触れた行（フォーカス行 or anchorPath）を表示する。
function wireSelectDetailsButton(button) {
  if (!button) return;
  button.addEventListener('click', () => {
    showEntryMeta(detailTargetEntry(
      state.renderedEntries, state.selectedPaths, state.focusIndex, state.anchorPath,
    ));
  });
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
  // BN05/BO04: 詳細ボタン・details キーと同じ detailTargetEntry 経路へ統一する。
  // 描画順の先頭選択行やイベント行ではなく、最後に触れた行を表示する。
  const chosen = detailTargetEntry(
    state.renderedEntries, state.selectedPaths, state.focusIndex, state.anchorPath,
  ) ?? entry;
  if (state.selectMode && state.selectedPaths.length) {
    showEntryMeta(chosen);
    setStatus(`選択確定: ${chosen.name}（${state.selectedPaths.length} 件）`);
    return;
  }
  showEntryMeta(chosen);
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
  // BM11: 2 本目の指は従来どおり無視。検索欄フォーカス中は長押しタイマーだけを
  // 止め、右スワイプの追跡は canTrackSwipe で維持する（旧実装はここで全体を
  // 返していたため、フォーカス中に右スワイプが効かなかった）。
  // state.touch を捨てる前にタイマーをキャンセルしないと、長押しコールバックが
  // 残って選択モードへ誤突入する。
  const searchFocused = document.activeElement === els.search;
  if (event.touches.length !== 1
    || !canTrackSwipe(event.touches[0].clientX, searchFocused)) {
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
  if (entry && !searchFocused) {
    // 長押しは詳細表示ではなく「選択モードへ突入 + 当該行をトグル」へ変更（BA19）。
    // 詳細表示はキーボードの Space / i、マウスの右クリック、
    // タッチは選択操作行の「詳細」ボタン（BM03）へ退避する。
    // BM11: 検索欄フォーカス中はこのタイマーだけを張らず、スワイプは追跡する。
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
  // BM11: フォーカス中でも canTrackSwipe で追跡を始めている以上、
  // ここで activeElement を見てスワイプを捨ててはいけない。左右判定は
  // shouldSwipeBack の swipeEnabled（左端ガード）に一本化する。
  if (shouldSwipeBack(current, dx, dy)) {
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
  // BP09: 旧実装はここで通知していたが、runSearch('') → refreshList() の
  // setStatus（core API 経由で N 件を表示）に即座に上書きされていた。
  // 一覧の取得が完了してから通知することで、クリアの確認が残る。
  await runSearch('');
  setStatus('検索をクリアして一覧へ戻りました');
}

async function onSwipeBack() {
  // スワイプ後の click が行選択として誤処理されないようにする（ボタン側は不要）。
  state.suppressClickUntil = performance.now() + CLICK_SUPPRESS_MS;
  // BN10: 検索モードのときだけ clearSearch() する。通常一覧では検索も選択も
  // 触らず、誤った「検索をクリア」通知も出さない。
  if (!shouldClearSearchOnSwipeBack(state.lastRender?.options?.mode)) return;
  await clearSearch();
}

function onListKeyDown(event) {
  // BP10: 空状態の「検索をクリア」ボタン等、行以外がフォーカスされているときは
  // Space を details として横取りしない。li が無ければここで返し、ボタンの
  // 既定 activation（Space / Enter）へ委譲する。
  const { li } = entryFromEvent(event);
  if (!li) return;
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
      // BN05/BO04: 詳細ボタン / ダブルクリックと同じ対象解決へ統一する。
      // 未選択行へフォーカスが動いても anchorPath（最後に触れた行）を優先する。
      showEntryMeta(detailTargetEntry(
        state.renderedEntries, state.selectedPaths, state.focusIndex, state.anchorPath,
      ));
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
  // BL06: 送信後の controllerchange が有効なのはこの窓だけ。activate しない
  // worker を待ち続けて無期限にフラグが立つと、別由来の制御権変更で誤再読込する。
  const SW_RELOAD_WINDOW_MS = 60_000;
  state.updateNotified = true;
  // BJ01: どの worker を通知したかを記録する。別 worker が installed に
  // なったときだけ updateNotified を戻し、次の更新も通知できるようにする。
  state.notifiedWorker = worker;
  // BL05: skipWaitingSent はここでリセットしない。前回の SKIP_WAITING がまだ
  // controllerchange を待っている場合、その再読込を失ってしまうため。
  // false に戻すのは controllerchange の消化時と再読込時だけ。
  // BI03: 更新通知は専用の aria-live ノードへ書く。#status は通常通知専用のまま。
  // 旧シェルが #sw-update を持たない場合だけ #status へフォールバックする。
  const notice = els.swUpdate || els.status;
  if (notice) {
    notice.hidden = false;
    notice.textContent = SW_UPDATE_MESSAGE;
    notice.dataset.level = 'info';
  }

  if (els.updateReload) {
    els.updateReload.hidden = false;
    // BJ08: once を外す。最初の postMessage が activate に結びつかない
    // （待機 worker がまだ居ない等）場合でもボタンから再送できるようにする。
    // リスナ自体は1回だけ配線し、送信状態は state で管理する。
    if (!state.updateReloadWired) {
      state.updateReloadWired = true;
      els.updateReload.addEventListener('click', () => {
        const waiting = state.notifiedWorker || registration.waiting || worker;
        // BK08: 待機中（installed）の worker にだけ SKIP_WAITING を送る。
        // 既に activate 済み等で待機していない場合はメッセージを送らず再読込へ流す。
        if (waiting && waiting.state === 'installed'
          && typeof waiting.postMessage === 'function') {
          // install では skipWaiting() しない代わりに、ここで待機 SW を進める。
          waiting.postMessage({ type: 'SKIP_WAITING' });
          state.skipWaitingSent = true;
          // BL06: 送信先と送信時刻を記録し、controllerchange が自 worker の
          // activate によるものかを照合できるようにする。
          state.skipWaitingWorker = waiting;
          state.skipWaitingAt = Date.now();
        } else {
          state.skipWaitingSent = false;
          state.skipWaitingWorker = null;
          window.location.reload();
        }
      });
    }
  }

  // 新 SW が control を取った時点で再読込し、新しいシェルを反映する。
  // BK07: once を外す。再通知で controllerReloadWired が true のままだと
  // リスナが張り直されず2回目の更新で再読込されないため、常設リスナにする。
  // BK08: skipWaitingSent を実ガードとして読み取り、SKIP_WAITING を送った
  // 後の controllerchange だけ再読込する（初回 clients.claim 等では再読込しない）。
  // BL06: さらに送信先 worker が activate していることと有効期限内であることを
  // 照合し、無関係な制御権変更での誤再読込を防ぐ。
  if (!state.controllerReloadWired) {
    state.controllerReloadWired = true;
    navigator.serviceWorker.addEventListener('controllerchange', () => {
      if (!state.skipWaitingSent) return;
      const target = state.skipWaitingWorker;
      if (Date.now() - state.skipWaitingAt > SW_RELOAD_WINDOW_MS) {
        // 期限切れの送信は破棄し、無期限 true による誤再読込を防ぐ。
        state.skipWaitingSent = false;
        state.skipWaitingWorker = null;
        return;
      }
      if (!target || target.state !== 'activated') {
        // 別由来の制御権変更。フラグは維持して自 worker の activate を待つ。
        return;
      }
      state.skipWaitingSent = false;
      state.skipWaitingWorker = null;
      // BM12: 自タブは controllerchange で再読込する。SW 側の全クライアント通知
      // （RELOAD）が同じタブへ届いても reloadRequested ガードで 1 回だけにする。
      if (state.reloadRequested) return;
      state.reloadRequested = true;
      window.location.reload();
    });
  }
}

// BM12: SW が activate 後に clients.matchAll() で全ウィンドウへ送る RELOAD を
// 受け取る常設リスナ。更新通知が無いタブでも早期から配線しておかないと、
// 他タブだけが再読込される（自タブのみ再読込の問題が直らない）。
// controllerchange の BL06 ガードとは独立した経路だが、送信元は登録済みの自 SW
// だけなので信頼できる。reloadRequested で controllerchange との二重を防ぐ。
function wireSwReloadMessage() {
  if (state.reloadMessageWired) return;
  if (typeof navigator === 'undefined' || !('serviceWorker' in navigator)) return;
  state.reloadMessageWired = true;
  navigator.serviceWorker.addEventListener('message', (event) => {
    if (!event || !event.data || event.data.type !== 'RELOAD') return;
    if (state.reloadRequested) return;
    state.reloadRequested = true;
    window.location.reload();
  });
}

function watchInstallingWorker(registration, worker) {
  worker.addEventListener('statechange', () => {
    // BJ01: 通知済みフラグは通知対象 worker に紐づく。前回とは別の worker が
    // installed になったら新たな更新として扱い、通知ガードを解除する。
    if (worker.state === 'installed' && state.notifiedWorker && state.notifiedWorker !== worker) {
      state.updateNotified = false;
      state.notifiedWorker = null;
    }
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
  // BM12: 更新通知の有無にかかわらず、全クライアント宛 RELOAD を受け取れるように
  // 最初からリスナを張っておく（activate 後の通知を取りこぼさないため）。
  wireSwReloadMessage();
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
  // BO14: core の oversized シグナルをデモでも保持するサンプル。
  { name: 'huge.png', path: '/demo/huge.png', is_dir: false, size: 8 * 1024 * 1024 + 1,
    kind: 'oversized' },
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
  // BM03: タッチ専用の詳細導線を選択操作行へ接続する。
  wireSelectDetailsButton(els.selectDetails);
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
    shouldClearSearchOnSwipeBack,
    // BM11: 検索フォーカス中でも左端以外ならスワイプ追跡を有効にする判定。
    canTrackSwipe,
    listKeyCommand,
    updatePickDirAvailability,
    toEntry,
    // BP17: core のエラー文言と JSON 配列を切り分けるデコーダを直接検証できるようにする。
    decodeCorePayload,
    normalizeEntries,
    // 複数選択 / 残存 / ソートの pure helper（node --test から直接検証する）。
    selectionToggle,
    selectionClear,
    selectionSelectSingle,
    selectionSelectAll,
    selectionRange,
    nextRangeArmed,
    retainSelectedPaths,
    resolveFocusIndex,
    detailTargetEntry,
    compareNatural,
    compareNames,
    sortEntries,
  };
}
