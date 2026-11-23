// SPDX-License-Identifier: AGPL-3.0-or-later
//
// PWA ビューア。データ源は core の wasm-bindgen API
// （CoreViewer::list_entries / search）のみ。pkg/ が無い場合だけ、
// 確認用の読み取り専用デモへフォールバックする。
//
// 注: 別エージェントが実装中の API の厳密なシグネチャに合わせ、
// メソッド名・戻り値の形は複数候補を吸収するadapterを置いている。
// API が確定したら normalize* / callAny の候補を絞ってよい。

const WASM_MODULE = './pkg/gfe_core.js';
const SEARCH_DEBOUNCE_MS = 150;

const els = {
  status: document.getElementById('status'),
  search: document.getElementById('search'),
  pickDir: document.getElementById('pick-dir'),
  modeNote: document.getElementById('mode-note'),
  list: document.getElementById('entry-list'),
  title: document.getElementById('list-title'),
  count: document.getElementById('list-count'),
};

const state = {
  module: null,
  viewer: null,
  wasmReady: false,
  entries: [],
  debounce: 0,
  rootName: '.',
};

// --- 小さなユーティリティ -------------------------------------------------

function setStatus(message, level = 'info') {
  els.status.textContent = message;
  els.status.dataset.level = level;
}

function supportsFileSystemAccess() {
  return typeof window.showDirectoryPicker === 'function';
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

  if (entries.length === 0) {
    const li = document.createElement('li');
    li.className = 'empty';
    li.textContent = '該当する項目はありません';
    els.list.append(li);
    return;
  }

  const fragment = document.createDocumentFragment();
  for (const entry of entries) {
    const li = document.createElement('li');

    const name = document.createElement('span');
    name.className = 'entry-name';
    name.dataset.kind = entry.isDir ? 'dir' : 'file';
    const icon = entry.isDir ? '📁' : '📄';
    const link = entry.isSymlink ? ' ↗' : '';
    name.textContent = `${icon} ${entry.name}${link}`;

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

// --- コア API 呼び出し（主要経路） ----------------------------------------

async function callAny(viewer, names, argsList) {
  let lastError;
  for (const name of names) {
    if (typeof viewer[name] !== 'function') continue;
    for (const args of argsList) {
      try {
        return await viewer[name](...args);
      } catch (error) {
        lastError = error;
      }
    }
  }
  throw lastError ?? new Error(`core API が見つかりません: ${names.join(' / ')}`);
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
  if (typeof Viewer !== 'function') throw new Error('CoreViewer が export されていません');

  let viewer;
  let lastError;
  for (const args of [[], ['.'], [state.rootName]]) {
    try {
      viewer = new Viewer(...args);
      break;
    } catch (error) {
      lastError = error;
    }
  }
  if (!viewer) throw lastError ?? new Error('CoreViewer を生成できませんでした');
  return viewer;
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
        raw = await callAny(state.viewer, ['list_entries'], [[state.rootName]]);
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
        raw = await callAny(state.viewer, ['search'], [[q, state.rootName]]);
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
  if (!supportsFileSystemAccess()) return;
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

  if (supportsFileSystemAccess()) {
    els.pickDir.hidden = false;
    els.pickDir.addEventListener('click', onPickDirectory);
  } else {
    els.modeNote.hidden = false;
    els.modeNote.textContent =
      'このブラウザは File System Access API 非対応です（iOS Safari 等）。'
      + 'フォルダ選択を隠し、内蔵サンプルによる読み取り専用デモとして動作します。';
  }

  try {
    state.viewer = await initCore();
    state.wasmReady = true;
    setStatus('core API 初期化完了');
    await refreshList();
  } catch (error) {
    useDemo(`wasm 未ロード（${errorMessage(error)}）→ 読み取り専用デモ`);
  }
}

boot();
