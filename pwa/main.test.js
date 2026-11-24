// SPDX-License-Identifier: AGPL-3.0-or-later
// Pure click-suppression boundary tests: no browser, no timers, no dependencies.
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {
  isClickSuppressed,
  secondClickSuppressed,
  doubleClickCommand,
  nextSearchGeneration,
  isCurrentSearchRequest,
  longPressTargetIndex,
  canPickDirectory,
  emptyStateText,
  shouldShowClearSearch,
  shouldSwipeBack,
  listKeyCommand,
  updatePickDirAvailability,
  toEntry,
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
} = require('./main.js');

test('isClickSuppressed uses a fixed clock and a strict lower bound', () => {
  const until = 1_000;
  assert.equal(isClickSuppressed(0, until), true);
  assert.equal(isClickSuppressed(999, until), true);
  assert.equal(isClickSuppressed(until, until), false);
  assert.equal(isClickSuppressed(1_001, until), false);
});

test('isClickSuppressed handles a zero deadline and the 700ms window', () => {
  assert.equal(isClickSuppressed(0, 0), false);
  assert.equal(isClickSuppressed(1, 0), false);
  assert.equal(isClickSuppressed(300, 300 + 699), true);
  assert.equal(isClickSuppressed(300 + 700, 300 + 700), false);
});

test('isClickSuppressed distinguishes mid-window from expired deadlines', () => {
  const until = 1_000;
  assert.equal(isClickSuppressed(1, until), true, 'now < until is active');
  assert.equal(isClickSuppressed(999.999, until), true, 'fractional now is still active');
  assert.equal(isClickSuppressed(1_001, until), false, 'now > until is expired');
  assert.equal(isClickSuppressed(1_000.001, until), false, 'fractional now past deadline');
});

test('isClickSuppressed is safe at integer-number boundaries', () => {
  const max = Number.MAX_SAFE_INTEGER;
  assert.equal(isClickSuppressed(max - 1, max), true);
  assert.equal(isClickSuppressed(max, max), false);
  assert.equal(isClickSuppressed(max + 1, max), false, 'past the largest safe deadline');
  assert.equal(isClickSuppressed(0, Number.MAX_VALUE), true);
});

// --- BE02: ダブルクリックの第2 click 抑制 ----------------------------------

test('secondClickSuppressed only suppresses the second and later click', () => {
  assert.equal(secondClickSuppressed(0), false);
  assert.equal(secondClickSuppressed(1), false, 'a single click still selects');
  assert.equal(secondClickSuppressed(2), true, 'the second click of a dblclick is ignored');
  assert.equal(secondClickSuppressed(3), true, 'rapid repeated clicks are ignored too');
});

test('secondClickSuppressed rejects malformed detail values', () => {
  assert.equal(secondClickSuppressed(undefined), false);
  assert.equal(secondClickSuppressed(null), false);
  assert.equal(secondClickSuppressed(NaN), false);
  assert.equal(secondClickSuppressed('2'), false);
  assert.equal(secondClickSuppressed(1.5), false);
  assert.equal(secondClickSuppressed(-1), false);
});

test('doubleClickCommand only activates on exactly two clicks', () => {
  assert.equal(doubleClickCommand(2), 'confirm');
  assert.equal(doubleClickCommand(0), null);
  assert.equal(doubleClickCommand(1), null);
  assert.equal(doubleClickCommand(3), null, 'a third click must not re-activate');
  assert.equal(doubleClickCommand(undefined), null);
  assert.equal(doubleClickCommand('2'), null);
});

test('ダブルクリックでトグルが相殺されない（BE02 回帰）', () => {
  let selectedPaths = [];
  const path = '/demo/notes.txt';
  // onListClick と同じ判定順を pure helper で再現する。
  function click(detail) {
    if (secondClickSuppressed(detail)) return;
    selectedPaths = selectionToggle(selectedPaths, path);
  }

  click(1);
  assert.deepEqual(selectedPaths, [path], '1回目で1行だけトグルされる');
  click(2);
  assert.deepEqual(selectedPaths, [path], '2回目はトグルを相殺しない');
  assert.equal(doubleClickCommand(2), 'confirm', 'dblclick 側で選択確定へ進む');
  click(3);
  assert.deepEqual(selectedPaths, [path], '3回目も選択を書き換えない');
  assert.equal(doubleClickCommand(3), null, '3回目は活性化しない');
});

// --- 非同期検索の世代ガード（BD05） ----------------------------------------

test('search generations advance monotonically and only the latest request is current', () => {
  let generation = 0;
  const first = generation = nextSearchGeneration(generation);
  const second = generation = nextSearchGeneration(generation);
  assert.equal(first, 1);
  assert.equal(second, 2);
  assert.ok(second > first, 'a new request must invalidate the previous one');
  assert.equal(isCurrentSearchRequest(generation, first), false, 'older request is stale');
  assert.equal(isCurrentSearchRequest(generation, second), true, 'newest request is current');
});

test('isCurrentSearchRequest rejects malformed identifiers', () => {
  assert.equal(isCurrentSearchRequest(undefined, undefined), false);
  assert.equal(isCurrentSearchRequest(null, 1), false);
  assert.equal(isCurrentSearchRequest(1, null), false);
  assert.equal(isCurrentSearchRequest(1.5, 1.5), false, 'only safe integers count');
  assert.equal(isCurrentSearchRequest(3, 2), false, 'a later generation wins');
});

test('古い検索結果が描画されない（BD05 回帰）', () => {
  let generation = 0;
  const rendered = [];
  // main.js の runSearch / refreshList と同じ判定を pure helper へ委譲する。
  function complete(captured, label) {
    if (!isCurrentSearchRequest(generation, captured)) return;
    rendered.push(label);
  }

  // 1 回目の検索が await 中に、連続入力で 2 回目の検索が始まる。
  const first = generation = nextSearchGeneration(generation);
  const second = generation = nextSearchGeneration(generation);
  complete(first, '最初の検索');
  assert.deepEqual(rendered, [], '古い await 完了は一覧を書き換えない');
  complete(second, '2 回目の検索');
  assert.deepEqual(rendered, ['2 回目の検索'], '最新の結果だけが描画される');
});

test('clear 後に古い検索結果が復活しない（BD05 回帰）', () => {
  let generation = 0;
  const rendered = [];
  function complete(captured, label) {
    if (!isCurrentSearchRequest(generation, captured)) return;
    rendered.push(label);
  }

  // 検索 await 中 → clearSearch() → runSearch('') → refreshList() の順に世代が進む。
  const search = generation = nextSearchGeneration(generation);
  const cleared = generation = nextSearchGeneration(generation);
  const emptyQuery = generation = nextSearchGeneration(generation);
  const list = generation = nextSearchGeneration(generation);

  complete(search, '古い検索結果');
  complete(cleared, 'クリア操作');
  complete(emptyQuery, '空クエリの検索');
  assert.deepEqual(rendered, [], 'クリア後に古い検索結果が復活しない');
  complete(list, '一覧');
  assert.deepEqual(rendered, ['一覧'], 'クリア後の一覧だけが残る');
});

test('新しい入力はデバウンス待ちの間も実行中リクエストを無効化する（BD05）', () => {
  let generation = 0;
  const inFlight = generation = nextSearchGeneration(generation);
  // input ハンドラは clearTimeout と同時に世代を進める（150ms 待つ前に無効化）。
  generation = nextSearchGeneration(generation);
  assert.equal(isCurrentSearchRequest(generation, inFlight), false,
    'デバウンスが発火する前に古い結果を破棄する');
});

// BE04: 上の BD05 テストは pure helper を再実装しただけなので、本番の
// runSearch / refreshList / clearSearch からガードが消えても緑のままだった。
// node:fs で main.js のソースを読み、await 完了直後の配線そのものを固定する。
const mainSource = fs.readFileSync(__dirname + '/main.js', 'utf8');

function sourceRegion(source, start, end) {
  const from = source.indexOf(start);
  assert.notEqual(from, -1, `missing start marker: ${start}`);
  assert.equal(source.indexOf(start, from + 1), -1, `start marker must be unique: ${start}`);
  const afterStart = from + start.length;
  const to = source.indexOf(end, afterStart);
  assert.notEqual(to, -1, `missing end marker: ${end}`);
  return source.slice(afterStart, to);
}

test('本番 runSearch / refreshList の await 直後に世代ガードがある（BD05 配線）', () => {
  const refreshList = sourceRegion(
    mainSource, 'async function refreshList() {', 'async function runSearch(query) {');
  const runSearch = sourceRegion(
    mainSource, 'async function runSearch(query) {', '// --- File System Access API');

  for (const [name, body] of [['refreshList', refreshList], ['runSearch', runSearch]]) {
    const lines = body.split('\n');
    let checked = 0;
    lines.forEach((line, index) => {
      if (line.trimStart().startsWith('//')) return; // コメント内の「await」は対象外
      if (!line.includes('await ')) return;
      // 空クエリは refreshList へ委譲するだけで、ガードは委譲先が持つ。
      if (line.includes('await refreshList()')) return;
      checked += 1;
      const after = lines.slice(index + 1, index + 4).join('\n');
      assert.match(
        after,
        /isCurrentSearchRequest\(state\.searchGeneration, generation\)/,
        `${name} の await 完了直後に現役判定が必要: ${line.trim()}`,
      );
    });
    assert.ok(checked > 0, `${name} に検証対象の await が必要`);
  }
});

test('本番 clearSearch は await より前に世代を進め古い結果を無効化する（BD05 配線）', () => {
  const clearSearch = sourceRegion(
    mainSource, 'async function clearSearch() {', 'async function onSwipeBack()');
  const bump = clearSearch.indexOf(
    'state.searchGeneration = nextSearchGeneration(state.searchGeneration);');
  const awaited = clearSearch.indexOf("await runSearch('')");
  assert.notEqual(bump, -1, 'clearSearch は世代を進める必要がある');
  assert.notEqual(awaited, -1, 'clearSearch は runSearch へ委譲する必要がある');
  assert.ok(bump < awaited, 'clear は await の前に世代を進め、実行中の検索を無効化する');
});

// --- Phase 1 受け入れ経路（in-memory API）の配線 ----------------------------
// 受け入れは「listEntriesFromJson / searchInJson を通した core API 経由の表示」。
// pure helper の再実装ではなく main.js の本番配線を sourceRegion で固定する。

test('本番 refreshList は in-memory API を主経路とし core API 経由を通知する', () => {
  const refreshList = sourceRegion(
    mainSource, 'async function refreshList() {', 'async function runSearch(query) {');
  const tokens = {
    guard: "typeof state.module?.listEntriesFromJson === 'function'",
    call: 'state.module.listEntriesFromJson(demoJson())',
    fallback: "callCore(state.viewer, 'list_entries'",
    status: 'setStatus(`core API 経由で ${state.entries.length} 件を表示`)',
  };
  for (const [label, token] of Object.entries(tokens)) {
    assert.ok(refreshList.includes(token), `refreshList に ${label} の配線が必要`);
  }
  assert.ok(refreshList.indexOf(tokens.call) < refreshList.indexOf(tokens.fallback),
    'in-memory API が std::fs フォールバックより先に呼ばれる');
});

test('本番 runSearch は searchInJson を主経路とし core API の検索結果を通知する', () => {
  const runSearch = sourceRegion(
    mainSource, 'async function runSearch(query) {', '// --- File System Access API');
  const tokens = {
    guard: "typeof state.module?.searchInJson === 'function'",
    call: 'state.module.searchInJson(q, demoJson())',
    fallback: "callCore(state.viewer, 'search'",
    status: 'setStatus(`core API の検索で ${results.length} 件`)',
  };
  for (const [label, token] of Object.entries(tokens)) {
    assert.ok(runSearch.includes(token), `runSearch に ${label} の配線が必要`);
  }
  assert.ok(runSearch.indexOf(tokens.call) < runSearch.indexOf(tokens.fallback),
    'searchInJson が std::fs フォールバックより先に呼ばれる');
});

test('demoJson はルート名を参照せず in-memory API 渡し用 JSON だけを作る', () => {
  const demo = sourceRegion(mainSource, 'function demoJson() {', 'function fillDemoIfEmpty() {');
  assert.ok(!demo.includes('rootName'), 'demoJson は state.rootName を使わない');
  assert.ok(demo.includes('JSON.stringify(DEMO_ENTRIES.map('),
    'DEMO_ENTRIES を JSON 化して core へ渡す');
});

// --- 長押し対象の path 再解決（BD12） --------------------------------------

test('longPressTargetIndex follows the touched path when the indices shift', () => {
  const before = [{ path: '/a' }, { path: '/b' }, { path: '/c' }];
  const oldIndex = 1;
  const path = before[oldIndex].path;
  assert.equal(longPressTargetIndex(before, path), oldIndex);

  // 500ms の間に検索が完了し、先頭に 1 行追加された想定。
  const after = [{ path: '/new' }, { path: '/a' }, { path: '/b' }, { path: '/c' }];
  assert.equal(after[oldIndex].path, '/a', '旧 index は別行を指す');
  const target = longPressTargetIndex(after, path);
  assert.equal(target, 2, 'path は新しい index へ再解決される');
  assert.equal(after[target].path, path, '旧 index ではなく path の行が選ばれる');
  assert.notEqual(after[oldIndex].path, path);
});

test('longPressTargetIndex skips a vanished row and stays pure', () => {
  const entries = [{ path: '/a' }, { path: '/b' }];
  assert.equal(longPressTargetIndex(entries, '/gone'), -1, '消えた行はスキップ');
  assert.equal(longPressTargetIndex([], '/a'), -1);
  assert.equal(longPressTargetIndex(null, '/a'), -1);
  assert.equal(longPressTargetIndex(undefined, '/a'), -1);
  assert.equal(longPressTargetIndex('not-an-array', '/a'), -1);
  assert.equal(longPressTargetIndex(entries, null), -1);
  assert.equal(longPressTargetIndex(entries, ''), -1);
  const snapshot = JSON.parse(JSON.stringify(entries));
  longPressTargetIndex(entries, '/a');
  assert.deepEqual(entries, snapshot, '入力配列は変更しない');
});

test('canPickDirectory needs both core API and File System Access', () => {
  assert.equal(canPickDirectory(true, true), true);
  assert.equal(canPickDirectory(false, true), false, 'demo mode must disable FSA');
  assert.equal(canPickDirectory(true, false), false, 'unsupported browser');
  assert.equal(canPickDirectory(false, false), false);
});

test('emptyStateText distinguishes the three empty states', () => {
  // クエリあり → クエリをエコー
  assert.equal(
    emptyStateText('README', 'search'),
    '「README」に一致する項目はありません',
    'query is echoed back to the user',
  );
  // クエリなし & 検索モード → 検索0件
  assert.equal(
    emptyStateText('', 'search'),
    '検索に一致する項目はありません',
    'an empty query in search mode is still a search miss',
  );
  // 一覧ゼロ → 一覧そのものが空
  assert.equal(
    emptyStateText('', 'list'),
    '項目がありません',
    'an empty listing is not confused with a search miss',
  );
});

test('emptyStateText trims the query and keeps markup as plain text', () => {
  assert.equal(emptyStateText('  notes  ', 'search'), '「notes」に一致する項目はありません');
  assert.equal(emptyStateText('   ', 'search'), '検索に一致する項目はありません');
  assert.equal(emptyStateText(null, 'search'), '検索に一致する項目はありません');
  assert.equal(emptyStateText(undefined, 'list'), '項目がありません');
  // 描画側は textContent を使うため、'<img>' はタグではなく文字列として扱われる。
  assert.equal(
    emptyStateText('<img src=x onerror=1>', 'search'),
    '「<img src=x onerror=1>」に一致する項目はありません',
  );
});

test('shouldShowClearSearch shows the button only for a search empty state', () => {
  assert.equal(shouldShowClearSearch('search'), true);
  assert.equal(shouldShowClearSearch('list'), false, 'no clear button for an empty listing');
  assert.equal(shouldShowClearSearch('other'), false);
  assert.equal(shouldShowClearSearch(undefined), false);
});

test('shouldSwipeBack ignores moved and uses only edge guard plus dx/dy', () => {
  const enabled = { swipeEnabled: true, moved: true };
  assert.equal(shouldSwipeBack(enabled, 90, 10), true, 'moved must not block a real swipe');
  assert.equal(shouldSwipeBack({ swipeEnabled: true, moved: false }, 90, 10), true);
  assert.equal(shouldSwipeBack(enabled, 20, 0), false, '20px is below the threshold');
  assert.equal(shouldSwipeBack(enabled, 40, 0), false, '40px is below the threshold');
  assert.equal(shouldSwipeBack(enabled, 81, 0), true, 'just past 80px');
  assert.equal(shouldSwipeBack(enabled, 80, 0), false, 'exactly 80px is not > 80');
  assert.equal(shouldSwipeBack(enabled, -90, 10), false, 'leftward is not back');
  assert.equal(shouldSwipeBack(enabled, 90, 100), false, 'vertical gesture wins');
  assert.equal(shouldSwipeBack(enabled, 90, 70), true, '1.2 ratio still horizontal');
  assert.equal(shouldSwipeBack(enabled, 90, 80), false, 'ratio boundary rejects');
  assert.equal(shouldSwipeBack({ swipeEnabled: false, moved: false }, 200, 0), false,
    'left-edge gesture is yielded to the OS');
  assert.equal(shouldSwipeBack(null, 90, 10), false, 'no active touch');
  assert.equal(shouldSwipeBack(enabled, Number.NaN, 0), false);
  assert.equal(shouldSwipeBack(enabled, Number.POSITIVE_INFINITY, 0), false);
});

test('listKeyCommand yields to modifier shortcuts and maps list keys', () => {
  assert.equal(listKeyCommand({ key: 'i', ctrlKey: true }), null, 'Ctrl+I is not hijacked');
  assert.equal(listKeyCommand({ key: 'i', metaKey: true }), null, 'Cmd+I is not hijacked');
  assert.equal(listKeyCommand({ key: 'i', altKey: true }), null, 'Alt+I is not hijacked');
  assert.equal(listKeyCommand({ key: 'ArrowDown', ctrlKey: true }), null);
  assert.equal(listKeyCommand({ key: 'ArrowUp', metaKey: true }), null);
  assert.equal(listKeyCommand({ key: 'Home', altKey: true }), null);
  assert.equal(listKeyCommand(null), null, 'missing event');
  assert.equal(listKeyCommand({}), null, 'missing key must not throw');

  assert.equal(listKeyCommand({ key: 'ArrowDown' }), 'down');
  assert.equal(listKeyCommand({ key: 'ArrowUp' }), 'up');
  assert.equal(listKeyCommand({ key: 'Home' }), 'home');
  assert.equal(listKeyCommand({ key: 'End' }), 'end');
  assert.equal(listKeyCommand({ key: ' ' }), 'details');
  assert.equal(listKeyCommand({ key: 'Spacebar' }), 'details');
  assert.equal(listKeyCommand({ key: 'i' }), 'details');
  assert.equal(listKeyCommand({ key: 'I', shiftKey: true }), 'details');
  assert.equal(listKeyCommand({ key: 'Enter' }), 'enter');
  assert.equal(listKeyCommand({ key: 'Escape' }), 'escape', 'BB04: keyboard escape route');
  assert.equal(listKeyCommand({ key: 'Escape', shiftKey: true }), 'escape');
  assert.equal(listKeyCommand({ key: 'Escape', ctrlKey: true }), null);
  assert.equal(listKeyCommand({ key: 'x' }), null, 'unrelated keys pass through');
});

test('listKeyCommand yields Shift+navigation keys but keeps Shift+I (BB14)', () => {
  const shifted = [
    { key: 'ArrowDown', expected: 'down' },
    { key: 'ArrowUp', expected: 'up' },
    { key: 'Home', expected: 'home' },
    { key: 'End', expected: 'end' },
  ];
  for (const { key, expected } of shifted) {
    assert.equal(listKeyCommand({ key }), expected, `${key} works without Shift`);
    assert.equal(listKeyCommand({ key, shiftKey: true }), null, `Shift+${key} is yielded`);
    assert.equal(listKeyCommand({ key, shiftKey: true, ctrlKey: true }), null);
  }
  // Shift+I は大文字入力なので詳細表示のまま（README の修飾キー方針と整合）。
  assert.equal(listKeyCommand({ key: 'I', shiftKey: true }), 'details');
  assert.equal(listKeyCommand({ key: 'i', shiftKey: true }), 'details');
  // ナビゲーション以外は Shift でも横取りする（最小修正）。
  assert.equal(listKeyCommand({ key: ' ', shiftKey: true }), 'details');
  assert.equal(listKeyCommand({ key: 'Enter', shiftKey: true }), 'enter');
});

test('updatePickDirAvailability mirrors canPickDirectory into a DOM button', () => {
  const button = { disabled: false };
  updatePickDirAvailability({ button, wasmReady: true, fsaSupported: true });
  assert.equal(button.disabled, false, 'core + FSA enables the picker');
  updatePickDirAvailability({ button, wasmReady: false, fsaSupported: true });
  assert.equal(button.disabled, true, 'demo mode disables the picker');
  updatePickDirAvailability({ button, wasmReady: true, fsaSupported: false });
  assert.equal(button.disabled, true, 'unsupported browser disables the picker');
  updatePickDirAvailability({ button, wasmReady: false, fsaSupported: false });
  assert.equal(button.disabled, true);
  assert.doesNotThrow(() => updatePickDirAvailability({
    button: null, wasmReady: true, fsaSupported: true,
  }), 'no button is a no-op (Node has no DOM)');
});

test('toEntry currently drops the core kind field (documented regression)', () => {
  const raw = JSON.stringify({
    name: 'huge.png',
    path: '/demo/huge.png',
    isDir: false,
    isSymlink: false,
    size: 8 * 1024 * 1024 + 1,
    kind: 'oversized',
    modifiedMs: null,
  });
  const entry = toEntry(raw);
  assert.equal(entry.name, 'huge.png');
  assert.equal(entry.size, 8 * 1024 * 1024 + 1, 'size survives the JS boundary');
  assert.equal(entry.kind, undefined,
    'PWA does not read kind yet; oversized sentinel is ignored here');
  assert.ok(!Object.keys(entry).includes('kind'), 'kind must not leak into the view model');
});

// --- 複数選択の pure helper（BA03 / BA07） --------------------------------

test('selectionToggle adds, removes and never mutates its input', () => {
  const original = ['/a', '/b'];
  assert.deepEqual(selectionToggle(original, '/c'), ['/a', '/b', '/c'], 'add at the end');
  assert.deepEqual(selectionToggle(original, '/a'), ['/b'], 'remove an existing path');
  assert.deepEqual(original, ['/a', '/b'], 'input is untouched');
  assert.deepEqual(selectionToggle(['/a', '/b', '/a'], '/a'), ['/b'],
    'all duplicate occurrences are removed');
  assert.deepEqual(selectionToggle(null, null), [], 'non-string paths are ignored');
  assert.deepEqual(selectionToggle(undefined, '/a'), ['/a']);
});

test('selectionClear / selectSingle / selectAll return fresh arrays', () => {
  const first = selectionClear();
  first.push('/leak');
  assert.deepEqual(selectionClear(), [], 'clear does not share the previous array');
  assert.deepEqual(selectionSelectSingle('/a'), ['/a']);
  assert.deepEqual(selectionSelectSingle(''), []);
  assert.deepEqual(selectionSelectAll([
    { path: '/a' }, { path: '/b' }, { path: '/a' },
  ]), ['/a', '/b', '/a'], 'selectAll keeps the view order');
  assert.deepEqual(selectionSelectAll(null), []);
});

test('selectionRange selects an inclusive span in view order', () => {
  const entries = ['/a', '/b', '/c', '/d'].map((path) => ({ path }));
  assert.deepEqual(selectionRange(entries, '/a', '/c'), ['/a', '/b', '/c']);
  assert.deepEqual(selectionRange(entries, '/c', '/a'), ['/a', '/b', '/c'],
    'a backwards range is normalised');
  assert.deepEqual(selectionRange(entries, '/b', '/b'), ['/b'], 'single-row range');
  assert.deepEqual(selectionRange(entries, null, '/b'), ['/b'], 'no anchor falls back to single');
  assert.deepEqual(selectionRange(entries, '/missing', '/b'), ['/b'],
    'an anchor outside the view falls back to single');
  assert.deepEqual(selectionRange(entries, '/a', '/missing'), [],
    'a target outside the view selects nothing');
  assert.deepEqual(selectionRange([], '/a', '/b'), []);
});

test('selectionRange handles non-array input, duplicate paths and stays pure (BB23)', () => {
  assert.deepEqual(selectionRange(null, '/a', '/b'), [], 'null entries → []');
  assert.deepEqual(selectionRange(undefined, '/a', '/b'), [], 'undefined entries → []');
  assert.deepEqual(selectionRange('not-an-array', '/a', '/b'), [], 'string entries → []');
  assert.deepEqual(selectionRange({ entries: [] }, '/a', '/b'), [], 'object entries → []');

  // 同じ path が 2 回現れても indexOf の位置を両端に使うため重複は保持される。
  const duplicated = [
    { path: '/a' }, { path: '/b' }, { path: '/a' }, { path: '/c' },
  ];
  assert.deepEqual(selectionRange(duplicated, '/a', '/c'), ['/a', '/b', '/a', '/c'],
    'duplicate paths inside the span are kept in view order');
  assert.deepEqual(selectionRange(duplicated, '/b', '/a'), ['/a', '/b'],
    'the first occurrence of a duplicated target is used and the span stays in view order');

  // pure 性: 入力配列とその要素は一切変更されない。
  const input = [{ path: '/a' }, { path: '/b' }, { path: '/c' }];
  const snapshot = JSON.parse(JSON.stringify(input));
  const result = selectionRange(input, '/a', '/c');
  assert.deepEqual(input, snapshot, 'the entries array is not mutated');
  assert.notEqual(result, input, 'a new array is returned');
  result.push('/leak');
  assert.deepEqual(input, snapshot, 'mutating the result cannot leak into the input');
});

test('nextRangeArmed toggles from the button and always consumes after selection (BB15)', () => {
  assert.equal(nextRangeArmed(false), true, 'button arms the range mode');
  assert.equal(nextRangeArmed(true), false, 'button disarms the range mode');
  assert.equal(nextRangeArmed(true, { completed: true }), false,
    'one range selection consumes the arm state');
  assert.equal(nextRangeArmed(false, { completed: true }), false,
    'completed is idempotent even when nothing was armed');
});

test('retainSelectedPaths keeps only surviving paths and preserves prev order', () => {
  const next = ['/b', '/c', '/d'];
  assert.deepEqual(retainSelectedPaths(['/a', '/b', '/c'], next), ['/b', '/c']);
  assert.deepEqual(retainSelectedPaths(['/c', '/a', '/b'], next), ['/c', '/b'],
    'the original selection order is preserved');
  assert.deepEqual(retainSelectedPaths([], next), []);
  assert.deepEqual(retainSelectedPaths(['/a'], []), []);
  assert.deepEqual(retainSelectedPaths(null, next), []);
  assert.deepEqual(retainSelectedPaths(['/a'], null), []);

  // BA07 の回帰: 検索で 1 文字入力しても、残る行の選択は消えない。
  const view = [{ path: '/README.md' }, { path: '/src' }];
  const selected = ['/README.md', '/src'];
  assert.deepEqual(retainSelectedPaths(selected, view.map((entry) => entry.path)), selected,
    'a one-character search that keeps both rows keeps both selections');
});

// --- 自然順ソート（BA18） --------------------------------------------------

test('compareNatural treats digit runs as numbers', () => {
  assert.equal(compareNatural('file2', 'file10'), -1);
  assert.equal(compareNatural('file10', 'file2'), 1);
  assert.equal(compareNatural('a1b2', 'a1b10'), -1);
  assert.equal(compareNatural('item', 'item'), 0);
  assert.equal(compareNatural('a2', 'a02'), -1, 'fewer leading zeros sorts first');
});

test('compareNames is case-insensitive with a case-sensitive tie-break', () => {
  assert.equal(compareNames('alpha', 'Zebra'), -1);
  assert.equal(compareNames('Alpha', 'alpha'), -1, 'case decides only when lowercase is equal');
  assert.equal(compareNames('Zebra', 'alpha'), 1);
  assert.equal(compareNames('file2', 'file10'), -1, 'natural ordering survives the helper');
});

function fixtureEntries() {
  return [
    { name: 'file10.txt', path: '/file10.txt', isDir: false, size: 10, modified: '2024-01-01' },
    { name: 'file2.txt', path: '/file2.txt', isDir: false, size: 300, modified: '2024-03-01' },
    { name: 'Zebra', path: '/Zebra', isDir: true, size: 0, modified: null },
    { name: 'alpha', path: '/alpha', isDir: true, size: 0, modified: '2024-02-01' },
    { name: 'Beta', path: '/Beta', isDir: false, size: 100, modified: null },
  ];
}

function names(entries) {
  return entries.map((entry) => entry.name);
}

test('sortEntries defaults to directories first, then natural case-insensitive names', () => {
  const input = fixtureEntries();
  const sorted = sortEntries(input);
  assert.deepEqual(names(sorted), ['alpha', 'Zebra', 'Beta', 'file2.txt', 'file10.txt']);
  assert.deepEqual(names(input), names(fixtureEntries()), 'input is not mutated');
  assert.notEqual(sorted, input, 'a new array is returned');
});

test('sortEntries reverses within each group for desc but keeps directories first', () => {
  assert.deepEqual(names(sortEntries(fixtureEntries(), 'name', 'desc')),
    ['Zebra', 'alpha', 'file10.txt', 'file2.txt', 'Beta']);
});

test('sortEntries honours size and modified keys', () => {
  assert.deepEqual(names(sortEntries(fixtureEntries(), 'size', 'desc')),
    ['Zebra', 'alpha', 'file2.txt', 'Beta', 'file10.txt'],
    'size descending, directories still first');
  assert.deepEqual(names(sortEntries(fixtureEntries(), 'modified', 'asc')),
    ['Zebra', 'alpha', 'Beta', 'file10.txt', 'file2.txt'],
    'a missing timestamp sorts before any real timestamp');
});

test('sortEntries uses the lower-cased extension for the kind key', () => {
  const input = fixtureEntries();
  input.push({ name: 'PICTURE.PNG', path: '/PICTURE.PNG', isDir: false, size: 1, modified: null });
  assert.deepEqual(names(sortEntries(input, 'kind', 'asc')),
    ['alpha', 'Zebra', 'Beta', 'PICTURE.PNG', 'file2.txt', 'file10.txt'],
    'no-extension files precede png, which precedes txt');
});

test('sortEntries falls back safely on unknown keys, bad dirs and empty input', () => {
  const input = fixtureEntries();
  assert.deepEqual(names(sortEntries(input, 'unknown', 'sideways')), names(sortEntries(input)));
  assert.deepEqual(sortEntries(null, 'name', 'asc'), []);
  assert.deepEqual(sortEntries(undefined), []);
  assert.deepEqual(sortEntries([]), []);
  // 大小文字だけが違う名前は、大小文字区別のタイブレークで安定して並ぶ。
  const ties = [{ name: 'alpha', path: '/a' }, { name: 'Alpha', path: '/A' }];
  assert.deepEqual(names(sortEntries(ties, 'name', 'asc')), ['Alpha', 'alpha']);
  assert.deepEqual(names(sortEntries(ties, 'name', 'desc')), ['alpha', 'Alpha']);
});

test('sortEntries accepts an upper-case dir string (BB22)', () => {
  const expected = names(sortEntries(fixtureEntries(), 'name', 'desc'));
  assert.deepEqual(names(sortEntries(fixtureEntries(), 'name', 'DESC')), expected,
    'String(dir).toLowerCase() === "desc"');
  assert.deepEqual(names(sortEntries(fixtureEntries(), 'name', 'Desc')), expected);
  assert.deepEqual(names(sortEntries(fixtureEntries(), 'name', 'ASC')),
    names(sortEntries(fixtureEntries(), 'name', 'asc')));
});

test('sortEntries treats modified:"" as missing and never throws on mixed types (BB22)', () => {
  const empty = fixtureEntries();
  empty[1].modified = ''; // file2.txt を欠損値にする
  // null / '' はどちらも欠損として名前でタイブレーク → Beta, file2.txt の順。
  assert.deepEqual(names(sortEntries(empty, 'modified', 'asc')),
    ['Zebra', 'alpha', 'Beta', 'file2.txt', 'file10.txt']);
  // desc では各グループ内で欠損値が最後へ回る（ディレクトリは常に先頭のまま）。
  assert.deepEqual(names(sortEntries(empty, 'modified', 'desc')),
    ['alpha', 'Zebra', 'file10.txt', 'file2.txt', 'Beta']);

  const mixed = [
    { name: 'epoch.txt', path: '/epoch', isDir: false, modified: 1704067200000 },
    { name: 'iso.txt', path: '/iso', isDir: false, modified: '2024-01-01' },
    { name: 'missing.txt', path: '/missing', isDir: false, modified: null },
  ];
  // number は String() されて ISO 文字列と辞書順比較される（決定的な順序）。
  // "1704067200000" < "2024-01-01" なので epoch → iso、欠損は asc で先頭。
  assert.deepEqual(names(sortEntries(mixed, 'modified', 'asc')),
    ['missing.txt', 'epoch.txt', 'iso.txt']);
  assert.deepEqual(names(sortEntries(mixed, 'modified', 'desc')),
    ['iso.txt', 'epoch.txt', 'missing.txt']);
});

// --- BG13/19/20: オフライン起動・マニフェスト・アイコン ---------------------

function readPwaFile(name) {
  return fs.readFileSync(path.join(__dirname, name), 'utf8');
}

test('service worker precaches the shell and every core wasm asset (BG13)', () => {
  const sw = readPwaFile('sw.js');
  for (const asset of [
    './',
    './index.html',
    './manifest.webmanifest',
    './main.js',
    './style.css',
    './pkg/gfe_core.js',
    './pkg/gfe_core_bg.wasm',
  ]) {
    assert.ok(sw.includes(`'${asset}'`), `sw.js must precache ${asset}`);
  }
  // snippet のディレクトリは wasm-pack のハッシュ依存なので実配置から導く。
  const snippetDir = fs.readdirSync(path.join(__dirname, 'pkg', 'snippets'))[0];
  assert.ok(snippetDir, 'pkg/snippets/<hash>/inline0.js must exist');
  assert.ok(
    sw.includes(`'./pkg/snippets/${snippetDir}/inline0.js'`),
    'sw.js must precache the actual generated inline0.js'
  );
  for (const listener of ['install', 'activate', 'fetch']) {
    assert.ok(sw.includes(`addEventListener('${listener}'`), `missing ${listener} handler`);
  }
  assert.ok(sw.includes('caches.match(request)'), 'cache-first lookup is required');
  assert.ok(sw.includes('self.clients.claim()'), 'activate must claim existing clients');
});

test('main.js registers the service worker during boot (BG13)', () => {
  const main = readPwaFile('main.js');
  assert.ok(main.includes("navigator.serviceWorker.register('./sw.js')"));
  assert.ok(main.includes("'serviceWorker' in navigator"), 'guard unsupported browsers');
});

test('manifest name and inline favicon agree (BG19/20)', () => {
  const manifest = JSON.parse(readPwaFile('manifest.webmanifest'));
  assert.equal(manifest.name, 'GFE Viewer');
  assert.equal(manifest.short_name, 'GFE Viewer');
  const html = readPwaFile('index.html');
  assert.ok(html.includes('rel="icon"'), 'index.html must declare a favicon');
  assert.ok(
    html.includes(manifest.icons[0].src),
    'favicon must reuse the manifest SVG data URI'
  );
  assert.ok(!/\.ico/.test(html), 'no missing .ico request');
});
