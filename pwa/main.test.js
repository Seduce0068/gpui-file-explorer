// SPDX-License-Identifier: AGPL-3.0-or-later
// Pure click-suppression boundary tests: no browser, no timers, no dependencies.
const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const {
  isClickSuppressed,
  secondClickSuppressed,
  doubleClickCommand,
  nextSearchGeneration,
  isCurrentSearchRequest,
  shouldNotifySwUpdate,
  longPressTargetIndex,
  canPickDirectory,
  emptyStateText,
  shouldShowClearSearch,
  shouldSwipeBack,
  shouldClearSearchOnSwipeBack,
  canTrackSwipe,
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
  resolveFocusIndex,
  detailTargetEntry,
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

// --- BN10: スワイプバックは検索モードだけ検索をクリアする -------------------

test('BN10: shouldClearSearchOnSwipeBack は検索モードだけ clearSearch を選ぶ', () => {
  assert.equal(shouldClearSearchOnSwipeBack('search'), true);
  assert.equal(shouldClearSearchOnSwipeBack('list'), false, '通常一覧では選択を維持する');
  assert.equal(shouldClearSearchOnSwipeBack(undefined), false, '初期 lastRender でも誤消去しない');
  assert.equal(shouldClearSearchOnSwipeBack(null), false);
  assert.equal(shouldClearSearchOnSwipeBack('SEARCH'), false, '完全一致のみ');
});

test('BN10: vm で一覧のスワイプが選択を維持し検索だけクリアする', async () => {
  // 本番の shouldClearSearchOnSwipeBack / onSwipeBack を vm で動かし、
  // モード無関係に clearSearch() を呼ぶ回帰と誤通知を実行経路で固定する。
  const guardSource = 'function shouldClearSearchOnSwipeBack(' + sourceRegion(
    mainSource, 'function shouldClearSearchOnSwipeBack(', '// BM11: スワイプ追跡の可否');
  const swipeBackSource = 'async function onSwipeBack() {' + sourceRegion(
    mainSource, 'async function onSwipeBack() {', 'function onListKeyDown(');

  const statuses = [];
  let clearCalls = 0;
  const state = {
    lastRender: { entries: [], options: { mode: 'list' } },
    selectedPaths: ['/a.txt', '/c.txt'],
    suppressClickUntil: 0,
  };
  const sandbox = {
    console,
    state,
    CLICK_SUPPRESS_MS: 700,
    performance: { now: () => 1000 },
    setStatus(message) { statuses.push(message); },
    async clearSearch() {
      clearCalls += 1;
      state.selectedPaths = [];
      statuses.push('検索をクリアして一覧へ戻りました');
    },
  };
  vm.createContext(sandbox);
  vm.runInContext(
    `${guardSource}\n${swipeBackSource}\nglobalThis.__onSwipeBack = onSwipeBack;`,
    sandbox
  );

  // 通常一覧: 選択も通知も触らず、後続 click の抑止だけ行う。
  await sandbox.__onSwipeBack();
  assert.equal(clearCalls, 0, '一覧では clearSearch を呼ばない');
  assert.deepEqual(state.selectedPaths, ['/a.txt', '/c.txt'], '複数選択を維持する');
  assert.deepEqual(statuses, [], '「検索をクリア」と誤通知しない');
  assert.equal(state.suppressClickUntil, 1700, 'スワイプ後の click 抑止は維持');

  // 検索モード: 従来どおり検索をクリアして一覧へ戻る。
  state.lastRender.options.mode = 'search';
  await sandbox.__onSwipeBack();
  assert.equal(clearCalls, 1, '検索モードでは clearSearch を呼ぶ');
  assert.deepEqual(state.selectedPaths, [], '検索クリア時は選択も解除する');
  assert.deepEqual(statuses, ['検索をクリアして一覧へ戻りました']);

  // 未初期化の lastRender.options でも一覧扱い（誤消去しない）。
  state.lastRender.options = {};
  await sandbox.__onSwipeBack();
  assert.equal(clearCalls, 1, 'mode 不明では search 扱いにしない');
});

test('BM11: canTrackSwipe は検索フォーカス中でも左端以外なら追跡する', () => {
  // 非フォーカス時は従来どおり（左端除外は swipeEnabled 側で行う）。
  assert.equal(canTrackSwipe(10, false), true, '非フォーカス時は左端も追跡して swipeEnabled=false にする');
  assert.equal(canTrackSwipe(100, false), true);
  // フォーカス中でも左端でなければ有効（旧実装はここで false を返していた）。
  assert.equal(canTrackSwipe(100, true), true, '検索フォーカス中でも左端以外は追跡する');
  assert.equal(canTrackSwipe(25, true), true, '24px 境界の外側は有効');
  assert.equal(canTrackSwipe(24, true), false, '左端 24px は OS の戻る操作へ譲る');
  assert.equal(canTrackSwipe(10, true), false);
  assert.equal(canTrackSwipe(Number.NaN, true), false);
  assert.equal(canTrackSwipe(Number.POSITIVE_INFINITY, false), false);
  assert.equal(canTrackSwipe(undefined), false);
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

test('toEntry preserves the core kind field (BO14)', () => {
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
  assert.equal(entry.kind, 'oversized', 'core の oversized シグナルを保持する');
  assert.ok(Object.keys(entry).includes('kind'), 'kind がビューへ引き継がれる');
  // SearchResult ラッパー経由でも kind が失われないこと。
  const wrapped = toEntry(JSON.stringify({ entry: JSON.parse(raw), match_line: 12 }));
  assert.equal(wrapped.kind, 'oversized', 'ラッパー経由でも kind を保持する');
  assert.equal(toEntry(JSON.stringify({ name: 'plain.txt' })).kind, undefined,
    'kind の無い core データでは undefined（疑似値を作らない）');
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
  // 期待値は被テスト関数から生成せずリテラルで固定する（BO13）。
  const desc = ['Zebra', 'alpha', 'file10.txt', 'file2.txt', 'Beta'];
  const asc = ['alpha', 'Zebra', 'Beta', 'file2.txt', 'file10.txt'];
  assert.deepEqual(names(sortEntries(fixtureEntries(), 'name', 'desc')), desc);
  assert.deepEqual(names(sortEntries(fixtureEntries(), 'name', 'DESC')), desc,
    'String(dir).toLowerCase() === "desc"');
  assert.deepEqual(names(sortEntries(fixtureEntries(), 'name', 'Desc')), desc);
  assert.deepEqual(names(sortEntries(fixtureEntries(), 'name', 'ASC')), asc);
  assert.deepEqual(names(sortEntries(fixtureEntries(), 'name', 'asc')), asc);
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
  // BO03: scope the required-asset check to the PRECACHE array itself, so a
  // quoted path that only appears in a comment or another literal cannot pass.
  const precache = sourceRegion(sw, 'const PRECACHE = [', '];');
  for (const asset of [
    './',
    './index.html',
    './manifest.webmanifest',
    './main.js',
    './style.css',
    './pkg/gfe_core.js',
    './pkg/gfe_core_bg.wasm',
  ]) {
    assert.ok(
      precache.includes(`'${asset}'`) || precache.includes(`"${asset}"`),
      `PRECACHE must contain ${asset}`
    );
  }
  // BI12/BI18: pwa/pkg/ は gitignore 済みで clean checkout には無い。
  // 実配置の有無にかかわらず、PRECACHE に snippet が「ちょうど 1 件」だけ
  // 正しい形で書かれていることを常時検証する（以前は pkg が無いと丸ごとスキップされた）。
  // BO09: accept both quote styles while still requiring matching delimiters;
  // the mapped tuple keeps [0] = full match and [1] = captured asset path.
  const snippets = [
    ...precache.matchAll(/(['"])(\.\/pkg\/snippets\/[^'"\/\n]+\/inline0\.js)\1/g),
  ].map((match) => [match[0], match[2]]);
  assert.equal(snippets.length, 1, 'PRECACHE must contain exactly one snippet inline0.js');
  assert.match(
    snippets[0][1],
    /^\.\/pkg\/snippets\/[A-Za-z0-9_-]+-[0-9a-f]{16,}\/inline0\.js$/,
    'the snippet directory must carry the wasm-pack hash suffix'
  );
  // ビルド成果物がある環境では、ハッシュが実際の配置と一致することも確認する。
  // BN11: readdirSync()[0] だけ見ると、古いハッシュのディレクトリが残っていても
  // 気づけなかった。全エントリを走査し、inline0.js を実際に持つディレクトリが
  // PRECACHE の 1 件と一致すること（複数残存していないこと）を明示的に検証する。
  const snippetDirPath = path.join(__dirname, 'pkg', 'snippets');
  if (fs.existsSync(snippetDirPath)) {
    const actualSnippets = fs.readdirSync(snippetDirPath, { withFileTypes: true })
      .filter((entry) => entry.isDirectory())
      .map((entry) => entry.name)
      .filter((name) => fs.existsSync(path.join(snippetDirPath, name, 'inline0.js')));
    assert.equal(actualSnippets.length, 1,
      `pkg/snippets/ に inline0.js を持つディレクトリは 1 つだけである必要があります: ${actualSnippets.join(', ')}`);
    assert.equal(snippets[0][1], `./pkg/snippets/${actualSnippets[0]}/inline0.js`,
      'sw.js must precache the actual generated inline0.js');
  }
  for (const listener of ['install', 'activate', 'fetch']) {
    assert.ok(sw.includes(`addEventListener('${listener}'`), `missing ${listener} handler`);
  }
  // BI07: 全キャッシュ横断の caches.match(request) ではなく明示キャッシュ参照。
  assert.ok(sw.includes('precache.match(request)'), 'cache-first lookup must be cache-scoped');
  assert.ok(!/\bcaches\.match\(request\)/.test(sw), 'cross-cache match must not be used');
  assert.ok(sw.includes('self.clients.claim()'), 'activate must claim existing clients');
});

test('service worker keeps the old shell when precache fails (BH01/BH16)', () => {
  const sw = readPwaFile('sw.js');
  // BI04: ファイル全体の includes だと別箇所の throw でも誤って満たされる。
  // precacheAll 本体を sourceRegion で切り出し、その領域内で失敗を再 throw
  // していることを固定する（fetch 側の best-effort 書き込みとは別経路）。
  const precacheAll = sourceRegion(
    sw, 'function precacheAll() {', "self.addEventListener('install'");
  assert.ok(precacheAll.includes('throw error'),
    'precacheAll は precache 失敗を再 throw して install を失敗させる必要がある');
  // 旧 SW / 旧キャッシュを維持するため install は precacheAll を待つ。
  const install = sourceRegion(
    sw, "self.addEventListener('install'", "self.addEventListener('message'");
  assert.ok(install.includes('waitUntil(precacheAll())'),
    'install は precacheAll() を waitUntil で待つ必要がある');
  // 失敗を console.warn で飲み込まず、install を失敗させること。
  // BI01 で fetch 側に追加した best-effort warn は対象外なので precacheAll に限定。
  assert.ok(!/console\.warn\s*\(/.test(precacheAll), 'precaching failures must not be swallowed');
  assert.ok(
    !/\.then\(\(\) => self\.skipWaiting\(\)\)/.test(sw),
    'install must not skipWaiting before precache is confirmed'
  );
  assert.ok(
    sw.includes("type === 'SKIP_WAITING'"),
    'a waiting worker must be activated by a client message'
  );
});

test('service worker scopes cache writes and deletion (BH09/BH10)', () => {
  const sw = readPwaFile('sw.js');
  assert.ok(sw.includes('key.startsWith(CACHE_PREFIX)'), 'only own caches may be deleted');
  assert.ok(sw.includes('!keep.has(key)'), 'current caches must survive cleanup');
  assert.ok(sw.includes('new Set([CACHE_NAME, RUNTIME_CACHE])'), 'keep set must cover live caches');
  assert.ok(
    !/filter\(\(key\) => key !== CACHE_NAME\)/.test(sw),
    'other service workers caches must survive'
  );
  assert.ok(sw.includes('RUNTIME_CACHE'), 'runtime responses need a separate cache');
  assert.ok(sw.includes('RUNTIME_MAX_ENTRIES'), 'runtime cache needs an entry cap');
  assert.ok(sw.includes('RUNTIME_TTL_MS'), 'runtime cache needs a freshness bound');
  assert.ok(sw.includes('if (url.search) return false'), 'query URLs must not proliferate');
  assert.ok(sw.includes('{ ignoreSearch: true }'), 'navigation shell must ignore search');
  assert.ok(sw.includes("request.mode === 'navigate'"), 'navigation needs a distinct strategy');
});

test('main.js registers the service worker during boot (BG13)', () => {
  const main = readPwaFile('main.js');
  assert.ok(main.includes("navigator.serviceWorker.register('./sw.js')"));
  assert.ok(main.includes("'serviceWorker' in navigator"), 'guard unsupported browsers');
});

test('main.js wires the service worker update flow and reload affordance (BH02)', () => {
  const main = readPwaFile('main.js');
  for (const marker of [
    "addEventListener('updatefound'",
    "addEventListener('statechange'",
    "type: 'SKIP_WAITING'",
    "addEventListener('controllerchange'",
    'window.location.reload()',
  ]) {
    assert.ok(main.includes(marker), `main.js must contain ${marker}`);
  }
  const html = readPwaFile('index.html');
  assert.ok(html.includes('id="update-reload"'), 'a reload button must exist');
  assert.ok(html.includes('aria-live="polite"'), 'a live region must announce the update');
  // BI03: 更新通知は #status と別ノード。通常通知を永久抑制しないための分離。
  assert.match(
    html,
    /id="sw-update"[^>]*role="status"[^>]*aria-live="polite"/,
    'a dedicated aria-live node for SW update notices must exist'
  );
});

test('main.js pins the installing-worker wiring, not just marker strings (BI11)', () => {
  // 文字列存在だけでは watchInstallingWorker の呼び出しが消えても緑だった。
  // 本番の更新フロー本体を sourceRegion で切り出し、配線を固定する。
  const updateFlow = sourceRegion(mainSource, 'function announceSwUpdate(', 'function registerServiceWorker()');
  // BJ04: assert.match(..., /g) は1一致でも成功するため、一致数を明示検証する。
  const installingCallSites =
    (updateFlow.match(/watchInstallingWorker\(registration, registration\.installing\)/g) || []).length;
  assert.equal(installingCallSites, 2,
    'both the current installing worker and updatefound must be watched (2 call sites)');
  assert.match(updateFlow, /addEventListener\('updatefound'/, 'updatefound must be observed');
  assert.match(updateFlow, /shouldNotifySwUpdate\(\{/, 'notification must go through the guard');
  assert.match(updateFlow, /announceSwUpdate\(registration, worker\)/,
    'the guarded path must actually announce');

  const precacheAll = sourceRegion(readPwaFile('sw.js'), 'function precacheAll() {', "self.addEventListener('install'");
  assert.match(precacheAll, /caches\.open\(CACHE_NAME\)/, 'precache must use its own cache');
  assert.match(precacheAll, /cache\.add\(asset\)/, 'each asset must be added individually');
  assert.match(precacheAll, /throw error/, 'a failed asset must reject install');
  const install = sourceRegion(readPwaFile('sw.js'), "self.addEventListener('install'", "self.addEventListener('message'");
  assert.match(install, /event\.waitUntil\(precacheAll\(\)\)/, 'install must await precacheAll');
});

test('shouldNotifySwUpdate only announces real updates', () => {
  assert.equal(shouldNotifySwUpdate({
    hasController: true, workerState: 'installed',
  }), true, 'a controlled page with a newly installed worker is an update');
  assert.equal(shouldNotifySwUpdate({
    hasController: false, workerState: 'installed',
  }), false, 'first install must not be announced as an update');
  assert.equal(shouldNotifySwUpdate({
    hasController: true, workerState: 'installing',
  }), false, 'only the installed state is actionable');
  assert.equal(shouldNotifySwUpdate({
    hasController: true, workerState: 'installed', alreadyNotified: true,
  }), false, 'the notice must not repeat');
  assert.equal(shouldNotifySwUpdate(), false, 'missing context is not an update');
});

test('BM02: [hidden] が author display より優先され一括操作行が隠れる', () => {
  const css = readPwaFile('style.css').replace(/\/\*[\s\S]*?\*\//g, '');
  const hiddenRule = css.match(/\[hidden\]\s*\{([^}]*)\}/);
  assert.ok(hiddenRule, 'style.css must define an explicit [hidden] rule');
  assert.match(hiddenRule[1], /display:\s*none\s*!important/,
    '[hidden] must force display:none over author display:flex');
  const hiddenAt = css.indexOf('[hidden]');
  const controlsAt = css.indexOf('.controls {');
  assert.ok(hiddenAt !== -1 && controlsAt !== -1 && hiddenAt < controlsAt,
    'the hidden rule must be declared before .controls for readable cascade order');
  const html = readPwaFile('index.html');
  assert.match(html, /id="select-actions"[^>]*\bhidden\b/,
    '#select-actions must start hidden until select mode is entered');
});

test('BM03: vm DOM モックで「詳細」ボタンが選択行/フォーカス行の詳細を表示する', () => {
  // 本番の humanSize / entryAt / isPathSelected / selectedEntries /
  // detailTargetEntry / showEntryMeta / wireSelectDetailsButton を sourceRegion で
  // 切り出し、vm 上の DOM モックでボタン配線そのものを検証する
  // （文字列マーカーだけでは配線解除を検出できない）。
  const humanSizeSource = 'function humanSize(bytes) {' + sourceRegion(
    mainSource, 'function humanSize(bytes) {', '// --- 描画（テキストはすべて textContent');
  const detailTargetSource = 'function detailTargetEntry(' + sourceRegion(
    mainSource, 'function detailTargetEntry(', '// --- 自然順ソートの pure helper');
  const selectionHelpers = 'function entryAt(index) {' + sourceRegion(
    mainSource, 'function entryAt(index) {', '// 複数選択 listbox では aria-selected');
  const showEntryMetaSource = 'function showEntryMeta(entry) {' + sourceRegion(
    mainSource, 'function showEntryMeta(entry) {', '// BM03: 選択操作行の「詳細」ボタン');
  const wireSource = 'function wireSelectDetailsButton(button) {' + sourceRegion(
    mainSource, 'function wireSelectDetailsButton(button) {', 'function entryFromEvent(');

  const messages = [];
  const state = {
    renderedEntries: [
      { name: 'a.txt', path: '/a.txt', size: 10, isDir: false },
      { name: 'b.txt', path: '/b.txt', size: 2048, isDir: false },
    ],
    selectedPaths: [],
    focusIndex: 0,
    suppressClickUntil: 0,
  };
  const button = {
    handlers: {},
    addEventListener(type, fn) {
      (this.handlers[type] ||= []).push(fn);
    },
  };
  const sandbox = {
    console,
    state,
    CLICK_SUPPRESS_MS: 700,
    setStatus(message, level = 'info') { messages.push({ message, level }); },
    performance: { now: () => 1000 },
  };
  vm.createContext(sandbox);
  vm.runInContext(
    `${humanSizeSource}\n${selectionHelpers}\n${detailTargetSource}\n`
    + `${showEntryMetaSource}\n${wireSource}\n`
    + 'globalThis.__wire = wireSelectDetailsButton;',
    sandbox
  );

  sandbox.__wire(button);
  assert.equal(button.handlers.click.length, 1, 'click リスナは 1 回だけ配線される');

  // 選択が無ければフォーカス行（index 0）の詳細を表示する。
  button.handlers.click[0]();
  assert.equal(messages.length, 1);
  assert.match(messages[0].message, /^詳細: \/a\.txt · size: 10 B \(10 B\) · isDir: false · ファイル$/);
  assert.equal(state.suppressClickUntil, 1700, '長押し/右クリックと同様に後続 click を抑止する');

  // 選択行があればフォーカス行より優先される。
  state.selectedPaths = ['/b.txt'];
  button.handlers.click[0]();
  assert.equal(messages.length, 2);
  assert.match(messages[1].message, /^詳細: \/b\.txt · size: 2048 B \(2\.0 K\) · isDir: false · ファイル$/);

  // 対象が無いときは何も表示しない（空一覧で押しても例外に落ちない）。
  state.selectedPaths = [];
  state.focusIndex = 99;
  button.handlers.click[0]();
  assert.equal(messages.length, 2, '対象行が無ければ通知しない');
});

// --- BN05: 「詳細」ボタンが最後に触れた行を表示する ------------------------

test('BN05: detailTargetEntry は描画順の先頭選択行より最後に触れた行を優先する', () => {
  const entries = [
    { name: 'a.txt', path: '/a.txt' },
    { name: 'b.txt', path: '/b.txt' },
    { name: 'c.txt', path: '/c.txt' },
    { name: 'd.txt', path: '/d.txt' },
  ];
  // 複数選択: selectedEntries()[0] は /a.txt だが最後に触れたのは /c.txt。
  assert.equal(
    detailTargetEntry(entries, ['/a.txt', '/c.txt'], 2, '/c.txt').path,
    '/c.txt',
    '選択中フォーカス行が最優先'
  );
  // 範囲選択: 終端行が選択されていればそちらを表示する。
  assert.equal(
    detailTargetEntry(entries, ['/a.txt', '/b.txt', '/c.txt'], 2, '/a.txt').path,
    '/c.txt',
    '範囲選択の終端が選択されていれば優先'
  );
  // 範囲選択後に矢印で未選択行へフォーカスが移動しても、アンカーへ戻る。
  assert.equal(
    detailTargetEntry(entries, ['/a.txt', '/b.txt', '/c.txt'], 3, '/a.txt').path,
    '/a.txt',
    '未選択フォーカス行は anchorPath を解決'
  );
  // アンカーが再描画で一覧から消えた場合は先頭選択行へフォールバック。
  assert.equal(
    detailTargetEntry(entries, ['/b.txt', '/c.txt'], 3, '/gone.txt').path,
    '/b.txt',
    '消えたアンカーは先頭選択行へ安全にフォールバック'
  );
  // 選択が無ければフォーカス行、それも無ければ null。
  assert.equal(detailTargetEntry(entries, [], 1, null).path, '/b.txt');
  assert.equal(detailTargetEntry(entries, [], 99, null), null);
  assert.equal(detailTargetEntry(null, null, -1, null), null, '欠損入力でも例外に落ちない');
});

test('BN05: vm の詳細ボタンで複数/範囲選択の最後に触れた行が表示される', () => {
  // 本番の wireSelectDetailsButton が detailTargetEntry を実際に呼ぶことを
  // 実行経路で固定する（pure helper だけのテストでは配線解除を検出できない）。
  const humanSizeSource = 'function humanSize(bytes) {' + sourceRegion(
    mainSource, 'function humanSize(bytes) {', '// --- 描画（テキストはすべて textContent');
  const detailTargetSource = 'function detailTargetEntry(' + sourceRegion(
    mainSource, 'function detailTargetEntry(', '// --- 自然順ソートの pure helper');
  const showEntryMetaSource = 'function showEntryMeta(entry) {' + sourceRegion(
    mainSource, 'function showEntryMeta(entry) {', '// BM03: 選択操作行の「詳細」ボタン');
  const wireSource = 'function wireSelectDetailsButton(button) {' + sourceRegion(
    mainSource, 'function wireSelectDetailsButton(button) {', 'function entryFromEvent(');

  const messages = [];
  const state = {
    renderedEntries: [
      { name: 'a.txt', path: '/a.txt', size: 10, isDir: false },
      { name: 'b.txt', path: '/b.txt', size: 20, isDir: false },
      { name: 'c.txt', path: '/c.txt', size: 30, isDir: false },
      { name: 'd.txt', path: '/d.txt', size: 40, isDir: false },
    ],
    selectedPaths: [],
    anchorPath: null,
    focusIndex: 0,
    suppressClickUntil: 0,
  };
  const button = {
    handlers: {},
    addEventListener(type, fn) { (this.handlers[type] ||= []).push(fn); },
  };
  const sandbox = {
    console,
    state,
    CLICK_SUPPRESS_MS: 700,
    setStatus(message) { messages.push(message); },
    performance: { now: () => 1000 },
  };
  vm.createContext(sandbox);
  vm.runInContext(
    `${humanSizeSource}\n${detailTargetSource}\n${showEntryMetaSource}\n${wireSource}\n`
    + 'globalThis.__wire = wireSelectDetailsButton;',
    sandbox
  );
  sandbox.__wire(button);

  // 複数選択: 先頭選択行 /a.txt ではなく最後に触れた /c.txt。
  state.selectedPaths = ['/a.txt', '/c.txt'];
  state.anchorPath = '/c.txt';
  state.focusIndex = 2;
  button.handlers.click[0]();
  assert.match(messages.at(-1), /^詳細: \/c\.txt · size: 30 B/);

  // 範囲選択: 終端行へフォーカスがあるならその行。
  state.selectedPaths = ['/a.txt', '/b.txt', '/c.txt'];
  state.anchorPath = '/a.txt';
  state.focusIndex = 2;
  button.handlers.click[0]();
  assert.match(messages.at(-1), /^詳細: \/c\.txt · size: 30 B/);

  // 未選択行へフォーカスが動いたらアンカーの /a.txt を表示する。
  state.focusIndex = 3;
  button.handlers.click[0]();
  assert.match(messages.at(-1), /^詳細: \/a\.txt · size: 10 B/);
});

// --- BO06: ソート変更後もフォーカス行 / 詳細対象が同じ行を指す -------------

test('BO06: resolveFocusIndex がソート前後でフォーカス行を path で再解決する', () => {
  const entries = [
    { name: 'a.txt', path: '/a.txt' },
    { name: 'b.txt', path: '/b.txt' },
    { name: 'c.txt', path: '/c.txt' },
    { name: 'd.txt', path: '/d.txt' },
  ];
  const selectedPaths = ['/a.txt', '/b.txt', '/c.txt'];
  const anchorPath = '/a.txt';
  const focusIndex = 2; // 範囲選択の終端 /c.txt（選択済み）
  const before = detailTargetEntry(entries, selectedPaths, focusIndex, anchorPath);
  assert.equal(before.path, '/c.txt');

  const sorted = sortEntries(entries, 'name', 'desc');
  assert.deepEqual(names(sorted), ['d.txt', 'c.txt', 'b.txt', 'a.txt']);
  // 再解決しない旧挙動では index 2 が /b.txt（選択済み）を指し詳細が変わる。
  assert.equal(detailTargetEntry(sorted, selectedPaths, focusIndex, anchorPath).path, '/b.txt');

  const resolved = resolveFocusIndex(entries, sorted, focusIndex, anchorPath);
  assert.equal(sorted[resolved].path, '/c.txt', '同じ行へフォーカスを戻す');
  assert.equal(detailTargetEntry(sorted, selectedPaths, resolved, anchorPath).path, before.path,
    'ソート前後で詳細対象が変わらない');

  // 検索でフォーカス行が消えた場合は anchorPath へ逃がす。
  const searched = entries.slice(0, 1); // /a.txt だけ残る
  assert.equal(
    searched[resolveFocusIndex(entries, searched, focusIndex, anchorPath)].path,
    '/a.txt',
  );
  // アンカーも消えた場合は index を新長さへクランプし、空一覧なら 0。
  assert.equal(resolveFocusIndex(entries, searched, 9, '/gone.txt'), 0);
  assert.equal(resolveFocusIndex(entries, [], focusIndex, anchorPath), 0);
});

test('BO06: render() は再描画時に resolveFocusIndex を呼ぶ', () => {
  const render = sourceRegion(
    mainSource, 'function render(entries,', '// --- 選択 / タッチ / マウス / キーボード操作');
  assert.match(render, /const previousEntries = state\.renderedEntries;/,
    '描画前に旧一覧を保存する');
  assert.match(
    render,
    /state\.focusIndex = resolveFocusIndex\(\s*previousEntries, view, state\.focusIndex, state\.anchorPath,\s*\);/,
    '新しい一覧へフォーカスを再解決する',
  );
  assert.ok(
    render.indexOf('const previousEntries = state.renderedEntries;')
      < render.indexOf('state.renderedEntries = view;'),
    '旧一覧の上書き前に保存する',
  );
});

// --- BO04: 詳細表示の3経路を同じ vm テストで固定 ---------------------------

test('BN05/BO04: 詳細ボタン・ダブルクリック・details キーが同じ対象を表示する', () => {
  const source = (start, end) => start + sourceRegion(mainSource, start, end);
  const humanSizeSource = source('function humanSize(bytes) {', '// --- 描画（テキストはすべて textContent');
  const selectionHelpers = source('function entryAt(index) {', '// 複数選択 listbox では aria-selected');
  const showMetaSource = source('function showEntryMeta(entry) {', '// BM03: 選択操作行の「詳細」ボタン');
  const wireSource = source('function wireSelectDetailsButton(button) {', 'function entryFromEvent(');
  const eventSource = source('function entryFromEvent(event) {', 'function onListClick(');
  const dblClickSource = source('function onListDblClick(event) {', 'function onListContextMenu(');
  const keyDownSource = source('function onListKeyDown(event) {', '// --- コア API 呼び出し');

  const messages = [];
  const state = {
    renderedEntries: [
      { name: 'a.txt', path: '/a.txt', size: 10, isDir: false },
      { name: 'b.txt', path: '/b.txt', size: 20, isDir: false },
      { name: 'c.txt', path: '/c.txt', size: 30, isDir: false },
      { name: 'd.txt', path: '/d.txt', size: 40, isDir: false },
    ],
    selectedPaths: ['/a.txt', '/b.txt', '/c.txt'],
    anchorPath: '/a.txt',
    focusIndex: 2, // 範囲選択の終端 /c.txt（選択済み）
    selectMode: true,
    suppressClickUntil: 0,
  };
  let detailCalls = 0;
  const sandbox = {
    console,
    state,
    Element: class Element {},
    CLICK_SUPPRESS_MS: 700,
    isClickSuppressed,
    doubleClickCommand,
    listKeyCommand,
    detailTargetEntry(...args) {
      detailCalls += 1;
      return detailTargetEntry(...args);
    },
    setStatus(message, level = 'info') { messages.push({ message, level }); },
    performance: { now: () => 1000 },
  };
  vm.createContext(sandbox);
  vm.runInContext([
    humanSizeSource, selectionHelpers, showMetaSource, wireSource, eventSource,
    dblClickSource, keyDownSource,
    'globalThis.__wire = wireSelectDetailsButton;',
    'globalThis.__dbl = onListDblClick;',
    'globalThis.__key = onListKeyDown;',
  ].join('\n'), sandbox);

  const button = {
    handlers: {},
    addEventListener(type, fn) { (this.handlers[type] ||= []).push(fn); },
  };
  sandbox.__wire(button);

  function fakeEvent({ detail = 2, key = ' ', index = 3 } = {}) {
    const target = new sandbox.Element();
    target.closest = () => ({ dataset: { index: String(index) } });
    return { detail, key, target, preventDefault() {} };
  }

  // 1) 詳細ボタン: 選択先頭 /a.txt ではなく最後に触れた /c.txt。
  button.handlers.click[0]();
  assert.match(messages.at(-1).message, /^詳細: \/c\.txt · size: 30 B/);
  assert.equal(detailCalls, 1, '詳細ボタンが detailTargetEntry を呼ぶ');

  // 2) ダブルクリック: イベント行は未選択の /d.txt でも /c.txt を表示。
  state.suppressClickUntil = 0;
  sandbox.__dbl(fakeEvent());
  assert.match(messages.at(-2).message, /^詳細: \/c\.txt · size: 30 B/);
  assert.match(messages.at(-1).message, /^選択確定: c\.txt（3 件）$/);
  assert.equal(detailCalls, 2, 'dblclick が detailTargetEntry を呼ぶ');

  // 3) details キー（Space）: 未選択行 /d.txt へフォーカスしても /c.txt。
  state.suppressClickUntil = 0;
  sandbox.__key(fakeEvent({ key: ' ' }));
  assert.match(messages.at(-1).message, /^詳細: \/c\.txt · size: 30 B/);
  assert.equal(detailCalls, 3, 'details キーが detailTargetEntry を呼ぶ');

  assert.equal(
    messages.filter(({ message }) => message.startsWith('詳細: /c.txt ·')).length,
    3,
    '3 経路すべてが同じ対象を表示する',
  );
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

test('BI03: DOM モックで更新通知後も #status が通常通知で更新される', () => {
  // pure helper の再実装ではなく、本番の setStatus / announceSwUpdate 本体を
  // sourceRegion で切り出して vm 上の DOM モックで実行する。
  // sourceRegion は開始マーカー自体を除くため、関数シグネチャを先頭へ戻す。
  const setStatusSource = 'function setStatus(' + sourceRegion(
    mainSource, 'function setStatus(', '// SW 更新通知の判定を純粋関数にして');
  const announceSource = 'function announceSwUpdate(' + sourceRegion(
    mainSource, 'function announceSwUpdate(', 'function watchInstallingWorker(');
  const messageDecl = mainSource.match(/^const SW_UPDATE_MESSAGE = .*;$/m);
  assert.ok(messageDecl, 'SW_UPDATE_MESSAGE must be declared in main.js');

  function fakeNode() {
    return {
      textContent: '',
      dataset: {},
      hidden: true,
      handlers: {},
      addEventListener(type, fn) {
        (this.handlers[type] ||= []).push(fn);
      },
    };
  }
  const status = fakeNode();
  const swUpdate = fakeNode();
  const updateReload = fakeNode();
  const state = { updateNotified: false };
  const posted = [];
  const listenerTypes = [];
  const controllerListeners = [];
  const sandbox = {
    console,
    state,
    els: { status, swUpdate, updateReload },
    window: { location: { reload() { sandbox.reloaded = true; } } },
    navigator: {
      serviceWorker: {
        addEventListener(type, fn) {
          listenerTypes.push(type);
          controllerListeners.push({ type, fn });
        },
      },
    },
  };
  vm.createContext(sandbox);
  vm.runInContext(
    `${messageDecl[0]}\n${setStatusSource}\n${announceSource}\n`
    + 'globalThis.__setStatus = setStatus; globalThis.__announce = announceSwUpdate;',
    sandbox
  );
  const message = vm.runInContext('SW_UPDATE_MESSAGE', sandbox);

  sandbox.__setStatus('一覧を読み込みました');
  assert.equal(status.textContent, '一覧を読み込みました');
  assert.equal(status.dataset.level, 'info');

  const worker = { state: 'installed', postMessage(msg) { posted.push(msg); } };
  sandbox.__announce({ waiting: worker }, worker);
  assert.equal(state.updateNotified, true);
  assert.equal(swUpdate.hidden, false);
  assert.equal(swUpdate.textContent, message, '更新通知は専用ノードへ書かれる');
  assert.equal(status.textContent, '一覧を読み込みました', '更新通知は #status を上書きしない');
  assert.equal(updateReload.hidden, false);

  // 回帰: 以前は updateNotified=true の間 level!=='error' が永久抑制された。
  sandbox.__setStatus('検索結果 4 件');
  assert.equal(status.textContent, '検索結果 4 件', '通常通知は更新通知後も #status へ届く');
  assert.equal(swUpdate.textContent, message, '更新通知はそのまま残る');

  updateReload.handlers.click[0]();
  // vm の別 realm で作られた object は deepStrictEqual の prototype 比較に落ちる。
  assert.equal(posted.length, 1, '1回のクリックで1回 postMessage する');
  assert.equal(posted[0].type, 'SKIP_WAITING', '再読込ボタンは待機 SW を進める');
  assert.ok(listenerTypes.includes('controllerchange'), 'controllerchange も配線される');

  // BJ08: once を外したことで、activate 失敗時などに SKIP_WAITING を再送できる。
  updateReload.handlers.click[0]();
  assert.equal(posted.length, 2, '2回目クリックでも SKIP_WAITING を再送する');
  assert.equal(updateReload.handlers.click.length, 1, 'click リスナは積まれない');
  assert.equal(state.skipWaitingSent, true, '送信済み状態は state で管理される');

  // BK20/BL06: controllerchange 発火を実際に流し、skipWaitingSent が実ガードか
  // 固定する。送信先が activate していない別由来の変更では再読込しない。
  const onControllerChange = controllerListeners.find((h) => h.type === 'controllerchange').fn;
  assert.ok(onControllerChange, 'controllerchange ハンドラが登録される');
  state.skipWaitingSent = false;
  onControllerChange();
  assert.equal(sandbox.reloaded, undefined, 'SKIP_WAITING 前の controllerchange では再読込しない');
  state.skipWaitingSent = true;
  onControllerChange();
  assert.equal(sandbox.reloaded, undefined, '送信先が activate 前の別由来 controllerchange では再読込しない');
  assert.equal(state.skipWaitingSent, true, '別由来の変更では送信状態を保持する');
  worker.state = 'activated';
  onControllerChange();
  assert.equal(sandbox.reloaded, true, '送信先 worker が activate した controllerchange で再読込する');
  assert.equal(state.skipWaitingSent, false, '再読込後に送信状態をリセットする');

  // BK08: worker 状態分岐 — 待機中でなければ postMessage せず再読込へ流す。
  worker.state = 'activated';
  sandbox.reloaded = false;
  updateReload.handlers.click[0]();
  assert.equal(posted.length, 2, '非待機 worker には SKIP_WAITING を送らない');
  assert.equal(sandbox.reloaded, true, '非待機 worker は再読込へ流す');
});

test('BJ01: vm モックで2回目 updatefound を流しても更新通知される', () => {
  // 本番の shouldNotifySwUpdate / announceSwUpdate / watchInstallingWorker /
  // watchServiceWorker を vm 上で動かし、別 worker の installed で
  // updateNotified が戻ることを固定する（文字列マーカーではなく実行経路）。
  const guardSource = 'function shouldNotifySwUpdate(' + sourceRegion(
    mainSource, 'function shouldNotifySwUpdate(', 'function supportsFileSystemAccess(');
  const announceSource = 'function announceSwUpdate(' + sourceRegion(
    mainSource, 'function announceSwUpdate(', 'function watchInstallingWorker(');
  const installingSource = 'function watchInstallingWorker(' + sourceRegion(
    mainSource, 'function watchInstallingWorker(', 'function watchServiceWorker(');
  const serviceSource = 'function watchServiceWorker(' + sourceRegion(
    mainSource, 'function watchServiceWorker(', 'function registerServiceWorker(');
  const messageDecl = mainSource.match(/^const SW_UPDATE_MESSAGE = .*;$/m);
  assert.ok(messageDecl, 'SW_UPDATE_MESSAGE must be declared in main.js');

  const state = {
    updateNotified: false,
    notifiedWorker: null,
    updateReloadWired: false,
    controllerReloadWired: false,
    skipWaitingSent: false,
  };
  const posted = [];
  const controllerListeners = [];
  let noticeWrites = 0;
  function fakeNode() {
    return {
      textContent: '',
      dataset: {},
      hidden: true,
      handlers: {},
      addEventListener(type, fn) {
        (this.handlers[type] ||= []).push(fn);
      },
    };
  }
  const swUpdate = {
    hidden: true,
    dataset: {},
    _text: '',
    get textContent() { return this._text; },
    set textContent(value) { this._text = value; noticeWrites += 1; },
  };
  const updateReload = fakeNode();
  const registration = {
    waiting: null,
    installing: null,
    listeners: {},
    addEventListener(type, fn) {
      (this.listeners[type] ||= []).push(fn);
    },
  };
  const sandbox = {
    console,
    state,
    els: { status: fakeNode(), swUpdate, updateReload },
    window: { location: { reload() { sandbox.reloaded = true; } } },
    navigator: {
      serviceWorker: {
        controller: { state: 'activated' },
        addEventListener(type, fn) { controllerListeners.push({ type, fn }); },
      },
    },
  };
  vm.createContext(sandbox);
  vm.runInContext(
    `${messageDecl[0]}\n${guardSource}\n${announceSource}\n`
    + `${installingSource}\n${serviceSource}\n`
    + 'globalThis.__watchServiceWorker = watchServiceWorker;',
    sandbox
  );

  function fakeWorker() {
    return {
      state: 'installing',
      listeners: {},
      addEventListener(type, fn) {
        (this.listeners[type] ||= []).push(fn);
      },
      postMessage(msg) { posted.push(msg); },
    };
  }
  function fireUpdatefound(worker) {
    registration.installing = worker;
    assert.equal(registration.listeners.updatefound.length, 1, 'updatefound must be wired');
    registration.listeners.updatefound[0]();
    worker.state = 'installed';
    assert.equal(worker.listeners.statechange.length, 1, 'statechange must be wired');
    worker.listeners.statechange[0]();
  }

  sandbox.__watchServiceWorker(registration);
  const first = fakeWorker();
  fireUpdatefound(first);
  assert.equal(noticeWrites, 1, '最初の更新は通知される');
  assert.equal(state.notifiedWorker, first, '通知対象 worker が記録される');
  updateReload.handlers.click[0]();
  assert.equal(state.skipWaitingSent, true);

  // BJ01: 別の worker が updatefound → installed になったら再通知する。
  const second = fakeWorker();
  fireUpdatefound(second);
  assert.equal(noticeWrites, 2, '別 worker の2回目 updatefound でも通知される');
  assert.equal(state.notifiedWorker, second, '通知対象 worker が差し替わる');
  // BL05: 新しい通知で skipWaitingSent をリセットすると、前回送信に対する
  // controllerchange の再読込が失われるため保持されなければならない。
  assert.equal(state.skipWaitingSent, true, '新しい通知でも前回の送信状態を保持する');
  assert.equal(updateReload.handlers.click.length, 1, 'click リスナは積まれない');
  // BM12: message リスナも増えたため種別で数える（controllerchange は 1 つのまま）。
  assert.equal(controllerListeners.filter((h) => h.type === 'controllerchange').length, 1,
    'controllerchange リスナは積まれない');

  // BK07: 2回目の更新でも常設 controllerchange リスナが生きていて再読込する
  // （once だとここでリスナが消えて2回目の更新が再読込されなかった）。
  updateReload.handlers.click[0]();
  assert.equal(state.skipWaitingSent, true, '2回目のクリックでも送信状態が立つ');
  // BL06: 送信先が activate する前の別由来 controllerchange では再読込しない。
  controllerListeners[0].fn();
  assert.equal(sandbox.reloaded, undefined, 'activate 前の別由来 controllerchange では再読込しない');
  assert.equal(state.skipWaitingSent, true, '別由来の変更では送信状態を保持する');
  second.state = 'activated';
  controllerListeners[0].fn();
  assert.equal(sandbox.reloaded, true, '2回目の controllerchange でも再読込する');
  assert.equal(state.skipWaitingSent, false, '再読込後に送信状態がリセットされる');

  // 同じ worker の statechange が再発しても通知は繰り返さない。
  second.listeners.statechange[0]();
  assert.equal(noticeWrites, 2, '同一 worker の再 statechange は重複通知しない');
});

test('BK18: 再オープン時の registration.waiting 経路で通知と SKIP_WAITING を検証', () => {
  // BJ01 は waiting=null で watchServiceWorker を呼ぶため、ページ再オープン時に
  // 既に待機 worker が居る経路が未実行だった。本番関数を vm で動かして固定する。
  const guardSource = 'function shouldNotifySwUpdate(' + sourceRegion(
    mainSource, 'function shouldNotifySwUpdate(', 'function supportsFileSystemAccess(');
  const announceSource = 'function announceSwUpdate(' + sourceRegion(
    mainSource, 'function announceSwUpdate(', 'function watchInstallingWorker(');
  const installingSource = 'function watchInstallingWorker(' + sourceRegion(
    mainSource, 'function watchInstallingWorker(', 'function watchServiceWorker(');
  const serviceSource = 'function watchServiceWorker(' + sourceRegion(
    mainSource, 'function watchServiceWorker(', 'function registerServiceWorker(');
  const messageDecl = mainSource.match(/^const SW_UPDATE_MESSAGE = .*;$/m);

  const state = {
    updateNotified: false,
    notifiedWorker: null,
    updateReloadWired: false,
    controllerReloadWired: false,
    skipWaitingSent: false,
  };
  const posted = [];
  const controllerListeners = [];
  function fakeNode() {
    return {
      textContent: '',
      dataset: {},
      hidden: true,
      handlers: {},
      addEventListener(type, fn) {
        (this.handlers[type] ||= []).push(fn);
      },
    };
  }
  const swUpdate = fakeNode();
  const updateReload = fakeNode();
  const waitingWorker = {
    state: 'installed',
    listeners: {},
    addEventListener(type, fn) {
      (this.listeners[type] ||= []).push(fn);
    },
    postMessage(msg) { posted.push(msg); },
  };
  const registration = {
    waiting: waitingWorker,
    installing: null,
    listeners: {},
    addEventListener(type, fn) {
      (this.listeners[type] ||= []).push(fn);
    },
  };
  const sandbox = {
    console,
    state,
    els: { status: fakeNode(), swUpdate, updateReload },
    window: { location: { reload() { sandbox.reloaded = true; } } },
    navigator: {
      serviceWorker: {
        controller: { state: 'activated' },
        addEventListener(type, fn) { controllerListeners.push({ type, fn }); },
      },
    },
  };
  vm.createContext(sandbox);
  vm.runInContext(
    `${messageDecl[0]}\n${guardSource}\n${announceSource}\n`
    + `${installingSource}\n${serviceSource}\n`
    + 'globalThis.__watchServiceWorker = watchServiceWorker;',
    sandbox
  );

  sandbox.__watchServiceWorker(registration);
  assert.equal(state.updateNotified, true, 'waiting があるだけで更新通知される');
  assert.equal(swUpdate.hidden, false, '通知ノードが表示される');
  assert.equal(state.notifiedWorker, waitingWorker, '通知対象は registration.waiting');
  assert.equal(updateReload.hidden, false, '再読込ボタンが表示される');

  updateReload.handlers.click[0]();
  assert.equal(posted.length, 1, 'waiting 経路でも SKIP_WAITING が送られる');
  assert.equal(posted[0].type, 'SKIP_WAITING');
  assert.equal(state.skipWaitingSent, true);
});

test('BM12: vm で2タブ分の state を作り SW の RELOAD で両タブが再読込する', () => {
  // 本番の announceSwUpdate / wireSwReloadMessage を vm 上で2回（別タブとして）
  // 動かし、activate 後の全クライアント通知で自タブ以外も再読込することを
  // 実行経路で固定する。
  const setStatusSource = 'function setStatus(' + sourceRegion(
    mainSource, 'function setStatus(', '// SW 更新通知の判定を純粋関数にして');
  const announceSource = 'function announceSwUpdate(' + sourceRegion(
    mainSource, 'function announceSwUpdate(', 'function watchInstallingWorker(');
  const wireMessageSource = 'function wireSwReloadMessage() {' + sourceRegion(
    mainSource, 'function wireSwReloadMessage() {', 'function watchInstallingWorker(');
  const messageDecl = mainSource.match(/^const SW_UPDATE_MESSAGE = .*;$/m);
  assert.ok(messageDecl, 'SW_UPDATE_MESSAGE must be declared in main.js');

  function fakeNode() {
    return {
      textContent: '',
      dataset: {},
      hidden: true,
      handlers: {},
      addEventListener(type, fn) {
        (this.handlers[type] ||= []).push(fn);
      },
    };
  }
  function makeTab() {
    const state = {
      updateNotified: false,
      notifiedWorker: null,
      updateReloadWired: false,
      controllerReloadWired: false,
      reloadMessageWired: false,
      reloadRequested: false,
      skipWaitingSent: false,
      skipWaitingWorker: null,
      skipWaitingAt: 0,
    };
    const listeners = {};
    const sandbox = {
      console,
      state,
      els: { status: fakeNode(), swUpdate: fakeNode(), updateReload: fakeNode() },
      reloadCount: 0,
      window: { location: { reload() { sandbox.reloadCount += 1; } } },
      navigator: {
        serviceWorker: {
          addEventListener(type, fn) {
            (listeners[type] ||= []).push(fn);
          },
        },
      },
    };
    vm.createContext(sandbox);
    vm.runInContext(
      `${messageDecl[0]}\n${setStatusSource}\n${announceSource}\n${wireMessageSource}\n`
      + 'globalThis.__announce = announceSwUpdate;'
      + 'globalThis.__wireMessage = wireSwReloadMessage;',
      sandbox
    );
    // 実ブラウザと同じく更新通知の有無にかかわらず message リスナを張る。
    sandbox.__wireMessage();
    const worker = { state: 'installed', postMessage() {} };
    sandbox.__announce({ waiting: worker }, worker);
    return { sandbox, listeners, state };
  }

  const tabA = makeTab();
  const tabB = makeTab();
  assert.equal(tabA.listeners.message.length, 1, 'message リスナが1回だけ配線される');
  assert.equal(tabA.listeners.controllerchange.length, 1, 'controllerchange も残る');

  // 無関係なメッセージでは再読込しない。
  tabA.listeners.message[0]({ data: { type: 'OTHER' } });
  assert.equal(tabA.sandbox.reloadCount, 0);

  // SW の activate 後ブロードキャストを両タブへ届ける（自タブのみではない）。
  tabA.listeners.message[0]({ data: { type: 'RELOAD' } });
  tabB.listeners.message[0]({ data: { type: 'RELOAD' } });
  assert.equal(tabA.sandbox.reloadCount, 1, 'タブAが再読込する');
  assert.equal(tabB.sandbox.reloadCount, 1, 'タブBも再読込する');

  // 自タブの controllerchange と重複しても reloadRequested ガードで1回だけ。
  tabA.listeners.message[0]({ data: { type: 'RELOAD' } });
  assert.equal(tabA.sandbox.reloadCount, 1, '重複 RELOAD は無視する');
  assert.equal(tabA.state.reloadRequested, true);

  // sw.js 側が claim の後に全ウィンドウへ RELOAD を送ることを固定する。
  const sw = readPwaFile('sw.js');
  const activate = sourceRegion(sw, "self.addEventListener('activate'", 'function isCacheable');
  assert.match(activate, /await notifyClientsReload\(\)/, 'activate で全クライアント通知する');
  assert.match(sw, /clients\.matchAll\(\{[^}]*type:\s*'window'[^}]*includeUncontrolled:\s*true/);
  assert.match(sw, /postMessage\(\{ type: 'RELOAD' \}\)/);
});

test('BN03: vm で旧シェル相当タブにも client.navigate が届く（RELOAD リスナ無し）', async () => {
  // 本番の notifyClientsReload を vm で動かし、postMessage だけでは再読込しない
  // 旧シェルタブも同一 URL への navigate で確実に更新されることを固定する。
  const notifySource = 'async function notifyClientsReload() {' + sourceRegion(
    readPwaFile('sw.js'), 'async function notifyClientsReload() {', "self.addEventListener('activate'");
  const posted = [];
  const navigated = [];

  function fakeClient(url, { navigate = true, navigateFails = false } = {}) {
    const client = {
      url,
      postMessage(msg) { posted.push({ url, type: msg.type }); },
    };
    if (navigate) {
      client.navigate = (target) => {
        navigated.push({ url, target });
        return navigateFails
          ? Promise.reject(new Error('navigate failed'))
          : Promise.resolve(client);
      };
    }
    return client;
  }
  const newShell = fakeClient('https://example.test/new');
  const oldShell = fakeClient('https://example.test/old');
  const legacy = fakeClient('https://example.test/legacy', { navigate: false });
  const failing = fakeClient('https://example.test/failing', { navigateFails: true });
  let matchQuery = null;
  const sandbox = {
    console: { warn() {}, error() {} },
    self: {
      clients: {
        async matchAll(query) {
          matchQuery = query;
          return [newShell, oldShell, legacy, failing];
        },
      },
    },
  };
  vm.createContext(sandbox);
  vm.runInContext(
    `${notifySource}\nglobalThis.__notify = notifyClientsReload;`,
    sandbox
  );
  await sandbox.__notify();

  // vm 別 realm の object は prototype 比較に落ちるためプロパティで確認する。
  assert.equal(matchQuery.type, 'window');
  assert.equal(matchQuery.includeUncontrolled, true);
  // 全ウィンドウへ RELOAD の postMessage が届く（新シェルはリスナで再読込）。
  assert.deepEqual(posted.map((p) => p.type), ['RELOAD', 'RELOAD', 'RELOAD', 'RELOAD']);
  assert.deepEqual(posted.map((p) => p.url), [
    'https://example.test/new', 'https://example.test/old',
    'https://example.test/legacy', 'https://example.test/failing',
  ]);
  // 旧シェル相当タブも navigate される（ここが BN03 の回帰）。
  assert.deepEqual(navigated.map((n) => n.target), [
    'https://example.test/new',
    'https://example.test/old',
    'https://example.test/failing',
  ]);
  assert.ok(!navigated.some((n) => n.url.includes('legacy')),
    'navigate 非対応クライアントでも postMessage は届き例外に落ちない');
  // navigate が 1 タブで失敗しても await が完了し、waitUntil は解決する。

  // matchAll 自体が失敗しても best-effort で飲み込まれる。
  sandbox.self.clients.matchAll = async () => { throw new Error('matchAll failed'); };
  await sandbox.__notify();
});

test('BJ09/BJ10/BJ11: runtime cache の即時 return・期限切れ削除・put 後削除', () => {
  const sw = readPwaFile('sw.js');
  const navigation = sourceRegion(sw, "if (request.mode === 'navigate') {", '// 静的アセットは cache-first');
  assert.ok(!/await\s+putRuntime\s*\(/.test(navigation),
    'navigation must not await putRuntime');
  assert.match(
    navigation,
    /putRuntime\('\.\/index\.html', response\.clone\(\)\)\.catch\(/,
    'navigation cache write must be fire-and-forget with a catch'
  );

  // BK20: sourceRegion で waitUntil 登録を必須化する。respondWith 解決後の
  // waitUntil は InvalidStateError になるため、登録が respondWith より前で
  // 同期的に行われ、deferred が書き込み promise を受け取ることも固定する。
  const fetchHandler = sourceRegion(sw, "self.addEventListener('fetch', (event) => {", '// 静的アセットは cache-first');
  const waitAt = fetchHandler.indexOf('event.waitUntil(');
  const respondAt = fetchHandler.indexOf('event.respondWith(');
  assert.ok(waitAt !== -1 && respondAt !== -1 && waitAt < respondAt,
    'navigation cache write must be registered with waitUntil before respondWith');

  // BL12: waitUntil に渡る promise と settleCacheWrite の resolver が同一の
  // deferred であることを固定する。片方の存在だけでは緑になっていた。
  const deferred = fetchHandler.match(
    /const (\w+) = new Promise\(\(resolve\) => \{ (\w+) = resolve; \}\)/);
  assert.ok(deferred, 'navigation must build a deferred cache-write promise');
  const [doneVar, settleVar] = deferred.slice(1);
  assert.equal(settleVar, 'settleCacheWrite', 'the deferred resolver must be settleCacheWrite');
  const waitArg = fetchHandler.match(/event\.waitUntil\((\w+)\)/);
  assert.ok(waitArg, 'waitUntil must be called with a single deferred promise');
  assert.equal(waitArg[1], doneVar,
    'waitUntil must adopt the very promise that settleCacheWrite resolves');
  assert.match(fetchHandler, /settleCacheWrite\(putRuntime\('\.\/index\.html', response\.clone\(\)\)\.catch\(/,
    'the write promise must flow into that same deferred resolver');

  // BL13: 引数なし settleCacheWrite() は fetch 失敗と non-ok 応答の2経路。
  const settleCalls = (fetchHandler.match(/settleCacheWrite\(\)/g) || []).length;
  assert.equal(settleCalls, 2, 'both no-write paths must settle the deferred');
  const catchPath = sourceRegion(navigation, '} catch (error) {', '// BI06:');
  assert.match(catchPath, /settleCacheWrite\(\)/, 'fetch failure must settle the deferred');
  const notOkPath = sourceRegion(navigation, '} else {', 'return response;');
  assert.match(notOkPath, /settleCacheWrite\(\)/, 'non-ok response must settle the deferred');

  const shellResponse = sourceRegion(sw, 'async function shellResponse() {', 'async function cachedAsset(');
  const staleBranch = sourceRegion(shellResponse, 'if (dynamic) {', 'return null;');
  assert.match(staleBranch, /isFresh\(dynamic\)/, 'stale shell must be detected');
  assert.match(staleBranch, /runtime\.delete\('\.\/index\.html'\)/,
    'stale runtime shell must be deleted before returning null');

  const putRuntime = sourceRegion(sw, 'async function putRuntime(', "self.addEventListener('fetch'");
  const putAt = putRuntime.indexOf('await cache.put(');
  const deleteAt = putRuntime.indexOf('cache.delete(');
  assert.ok(putAt !== -1 && deleteAt !== -1 && putAt < deleteAt,
    'put must succeed before evicting old entries');
  assert.ok(
    !/if \(keys\.length >= RUNTIME_MAX_ENTRIES\) await cache\.delete\(keys\[0\]\)/.test(putRuntime),
    'pre-put eviction must be gone'
  );
});
