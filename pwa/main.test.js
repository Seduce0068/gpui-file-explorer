// SPDX-License-Identifier: AGPL-3.0-or-later
// Pure click-suppression boundary tests: no browser, no timers, no dependencies.
const test = require('node:test');
const assert = require('node:assert/strict');
const {
  isClickSuppressed,
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
