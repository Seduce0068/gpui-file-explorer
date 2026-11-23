// SPDX-License-Identifier: AGPL-3.0-or-later
// Pure click-suppression boundary tests: no browser, no timers, no dependencies.
const test = require('node:test');
const assert = require('node:assert/strict');
const {
  isClickSuppressed,
  canPickDirectory,
  shouldSwipeBack,
  listKeyCommand,
  updatePickDirAvailability,
  toEntry,
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
  assert.equal(listKeyCommand({ key: 'x' }), null, 'unrelated keys pass through');
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
