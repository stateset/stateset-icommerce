import assert from 'node:assert/strict';
import { test } from 'node:test';
import { editText, editorWindow } from '../../src/tui/editor.js';
import { width, wrap } from '../../src/tui/text.js';
import { initialState, navigate, handleKey } from '../../src/tui/model.js';
import { render, contentLimit } from '../../src/tui/render.js';

test('text editing inserts and deletes at a bounded cursor instead of appending only', () => {
  let value = editText('12.50', undefined, '', { name: 'home' });
  value = editText(value.text, value.cursor, '', { name: 'right' });
  value = editText(value.text, value.cursor, '', { name: 'delete' });
  value = editText(value.text, value.cursor, '9', {});
  assert.equal(value.text, '19.50');
  assert.equal(value.cursor, 2);
  value = editText(value.text, value.cursor, '', { name: 'k', ctrl: true });
  assert.equal(value.text, '19');
  value = editText(value.text, value.cursor, '', { name: 'a', ctrl: true });
  assert.equal(value.cursor, 0);
  value = editText(value.text, value.cursor, '', { name: 'u', ctrl: true });
  assert.deepEqual(value, { text: '', cursor: 0, changed: true });
});

test('Unicode deletion and insertion keep combining accents, flags and emoji families intact', () => {
  const family = '👨‍👩‍👧‍👦';
  let value = editText(`e\u0301${family}🇨🇦`, undefined, '', { name: 'backspace' });
  assert.equal(value.text, `e\u0301${family}`);
  value = editText(value.text, value.cursor, '', { name: 'backspace' });
  assert.equal(value.text, 'e\u0301');
  assert.equal(editText('A', 1, family, {}, { limit: 2 }).text, 'A' + family);
  assert.equal(editText('A', 1, 'BC', {}, { limit: 2 }).text, 'AB');
  assert.equal(width('🇨🇦'), 2);
  assert.equal(width('1️⃣'), 2);
});

test('cursor windows fit narrow panes and reveal either end of long Unicode paths', () => {
  const value = '/very/long/path/珈琲/👨‍👩‍👧‍👦/planning.json';
  for (const cells of [1, 2, 5, 12, 25])
    for (const cursor of [0, 4, 999]) {
      const output = editorWindow(value, cursor, cells);
      assert.ok(width(output) <= cells, JSON.stringify({ output, cells }));
      assert.match(output, /▏/);
    }
  assert.match(editorWindow(value, undefined, 18), /planning.json▏/);
  assert.match(editorWindow(value, 0, 18), /^▏\/very/);
});

test('numeric planning edits preserve exact decimals and validate before committing', () => {
  let state = handleKey(navigate(initialState(), 5), 'e', {}).state;
  state = handleKey(state, '', { name: 'tab' }).state;
  state = handleKey(state, '', { name: 'home' }).state;
  state = handleKey(state, '', { name: 'delete' }).state;
  state = handleKey(state, '1', {}).state;
  assert.equal(state.input.draft.startingCash, '100.00');
  state = handleKey(state, '', { name: 's', ctrl: true }).state;
  assert.equal(state.planning.startingCash, '100.00');
});

test('seed entry validates atomically, cancels cleanly and never launches a run', () => {
  let state = navigate(initialState(), 5);
  state = handleKey(state, 'g', {}).state;
  state = handleKey(state, '', { name: 'u', ctrl: true }).state;
  state = handleKey(state, '0', {}).state;
  state = handleKey(state, '', { name: 'return' }).state;
  assert.equal(state.seed, 42);
  assert.match(state.input.error, /1 to 999999/);
  state = handleKey(state, '', { name: 'u', ctrl: true }).state;
  state = handleKey(state, '999999', {}).state;
  const accepted = handleKey(state, '', { name: 'return' });
  assert.equal(accepted.state.seed, 999999);
  assert.equal(accepted.effect, undefined);
  state = handleKey(accepted.state, 'g', {}).state;
  state = handleKey(state, '', { name: 'u', ctrl: true }).state;
  state = handleKey(state, '1', {}).state;
  state = handleKey(state, '', { name: 'escape' }).state;
  assert.equal(state.seed, 999999);
});

test('path editing owns q/digits and retains the edited path literally', () => {
  let state = {
    ...navigate(initialState(), 5),
    input: { kind: 'import', text: '/tmp/wrong.json' },
  };
  state = handleKey(state, '', { name: 'home' }).state;
  state = handleKey(state, 'q6', {}).state;
  const accepted = handleKey(state, '', { name: 'return' });
  assert.equal(accepted.effect, 'import');
  assert.equal(accepted.state.pendingText, 'q6/tmp/wrong.json');
  assert.equal(accepted.state.view, 5);
});

test('wrapping favors words, preserves indentation and keeps Unicode clusters whole', () => {
  assert.deepEqual(wrap('hello world again', 7), ['hello', 'world', 'again']);
  assert.equal(wrap('  nested: true', 40)[0], '  nested: true');
  assert.deepEqual(wrap('abcdefghijkl', 5), ['abcde', 'fghij', 'kl']);
  for (const line of wrap('珈琲 👨‍👩‍👧‍👦 🇨🇦 hello world', 6)) assert.ok(width(line) <= 6);
});

test('Home and Help expose finite scroll limits, including in short panes', () => {
  const state = initialState();
  state.data = { counts: {}, errors: [], inventory: [] };
  assert.ok(contentLimit(state, 55, 14) > 0);
  state.help = true;
  const limit = contentLimit(state, 55, 20);
  assert.ok(limit > 0 && limit < 100);
  const end = render({ ...state, contentOffset: limit }, 55, 20, { color: false });
  const previous = render({ ...state, contentOffset: limit - 1 }, 55, 20, { color: false });
  assert.notEqual(end, previous);
});

test('closing Help preserves the underlying report and comparison selection', () => {
  const state = {
    ...navigate(initialState(), 5),
    help: true,
    panel: 'results',
    detail: true,
    detailOffset: 12,
  };
  const dismissed = handleKey(state, '', { name: 'escape' }).state;
  assert.equal(dismissed.help, false);
  assert.equal(dismissed.detail, true);
  assert.equal(dismissed.detailOffset, 12);
  assert.equal(dismissed.panel, 'results');
});

test('inserting a combining accent within text leaves the cursor beside that grapheme', () => {
  const accented = editText('ab', 1, '\u0301', {});
  assert.equal(accented.text, 'a\u0301b');
  assert.equal(accented.cursor, 1);
  const continued = editText(accented.text, accented.cursor, 'X', {});
  assert.equal(continued.text, 'a\u0301Xb');
});
