import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';
import {
  DOMAINS,
  TABLES,
  explorerRequest,
  explorerRequestKey,
  openExplorer,
} from '../../src/tui/domains.js';
import {
  initialState,
  handleKey,
  records,
  paletteCommands,
  navigate,
} from '../../src/tui/model.js';
import { render, detailLimit } from '../../src/tui/render.js';
import { width } from '../../src/tui/text.js';
import { buildTuiCatalog, CATALOG_PATH } from '../../scripts/generate-tui-catalog.mjs';

const key = (s, text, name = text) => handleKey(s, text, { name });
function resource(domain, table) {
  const state = openExplorer(initialState(), domain);
  return { ...state, explorer: { ...state.explorer, mode: 'records', table } };
}
function data(state, rows, extra = {}) {
  return {
    ...state,
    data: {
      counts: {},
      errors: [],
      explorer: {
        requestKey: explorerRequestKey(state.explorer),
        matched: rows.length,
        page: 0,
        rows,
        links: [],
        warnings: [],
        ...extra,
      },
    },
  };
}
test('generated metadata includes every domain and runtime tool without executable handlers', () => {
  assert.equal(readFileSync(CATALOG_PATH, 'utf8'), buildTuiCatalog());
  assert.equal(DOMAINS.length, 90);
  assert.equal(
    DOMAINS.reduce((n, d) => n + d.tools.length, 0),
    966,
  );
  assert.equal(new Set(DOMAINS.map((d) => d.id)).size, DOMAINS.length);
  assert.ok(TABLES.length >= 200);
  assert.ok(DOMAINS.every((d) => d.group && d.tools.every((t) => !('handler' in t))));
  assert.ok(DOMAINS.find((d) => d.id === 'vector').reason.includes('OPENAI_API_KEY'));
});
test('explorer filters all domains, opens resources, and keeps tool references separate from execution', () => {
  let state = key(initialState(), '7').state;
  assert.equal(state.view, 6);
  state = key(state, '/').state;
  state = key(state, 'general ledger').state;
  assert.equal(records(state).length, 1);
  state = key(state, '', 'return').state;
  state = key(state, '', 'return').state;
  assert.equal(state.explorer.domain, 'general-ledger');
  const opened = key(state, '', 'return');
  assert.equal(opened.effect, 'refresh');
  assert.equal(opened.state.explorer.table, 'gl_accounts');
  state = key(state, 'T').state;
  assert.equal(state.explorer.mode, 'tools');
  state = key(state, '/').state;
  state = key(state, 'create_gl_account').state;
  state = key(state, '', 'return').state;
  assert.ok(records(state).every((r) => r.permission === 'write'));
  const inspected = key(state, '', 'return');
  assert.equal(inspected.effect, undefined);
  assert.equal(inspected.state.detail, true);
  assert.match(render(inspected.state, 77, 24, { color: false }), /TOOL REFERENCE/);
  state = key(state, 'd').state;
  assert.equal(state.detail, false);
});
test('domain palette jumps are fixed and do not dispatch tools or shell commands', () => {
  let state = key(initialState(), ':').state;
  state = key(state, 'Explore accounts payable').state;
  const result = key(state, '', 'return');
  assert.equal(result.state.view, 6);
  assert.equal(result.state.explorer.domain, 'accounts-payable');
  assert.equal(result.effect, undefined);
  assert.equal(paletteCommands('$(touch /tmp/unsafe)').length, 0);
  assert.equal(navigate(result.state, 5).view, 5);
});
test('domain navigation starts with commerce and group shortcuts wrap through visible results', () => {
  let state = openExplorer(initialState());
  const groups = ['Commerce', 'Growth', 'Operations', 'Finance', 'Agents & trust', 'Integrations'];
  assert.deepEqual([...new Set(records(state).map((row) => row.group))], groups);
  const selectedGroup = () => records(state)[state.selected].group;
  assert.equal(selectedGroup(), 'Commerce');
  for (const expected of [...groups.slice(1), groups[0]]) {
    const result = key(state, ']');
    assert.equal(result.effect, undefined, 'group navigation does not query or mutate a store');
    state = result.state;
    assert.equal(selectedGroup(), expected);
    assert.match(render(state, 55, 20, { color: false }), new RegExp(expected));
  }
  state = key(state, '[').state;
  assert.equal(selectedGroup(), 'Integrations');
  state = key(state, '/').state;
  state = key(state, ' Finance ').state;
  assert.ok(records(state).length > 0);
  assert.ok(records(state).every((row) => row.group === 'Finance'));
  state = key(state, '', 'return').state;
  const filtered = state;
  assert.equal(key(state, ']').state, filtered, 'one visible group stays selected');
  assert.equal(key(state, '[').state, filtered);
  state = key(state, '', 'escape').state;
  assert.equal(records(state).length, DOMAINS.length);
});
test('domain filtering matches labels and tiers rather than hidden tool descriptions or JSON keys', () => {
  const filtered = (query) => {
    const state = openExplorer(initialState());
    return { ...state, explorer: { ...state.explorer, query } };
  };
  assert.deepEqual(
    records(filtered('CUSTOMERS')).map((row) => row.id),
    ['customers'],
  );
  assert.deepEqual(
    records(filtered('general-ledger')).map((row) => row.id),
    ['general-ledger'],
  );
  assert.ok(records(filtered('core')).every((row) => row.tier === 'core'));
  assert.ok(records(filtered('reference')).every((row) => !row.tables.length));
  for (const query of ['create_customer', 'permission']) {
    const state = filtered(query);
    assert.deepEqual(records(state), []);
    assert.match(render(state, 100, 24, { color: false }), /No matching domains/);
    assert.equal(key(state, '', 'return').state, state);
  }
  const resources = openExplorer(initialState(), 'customers');
  assert.deepEqual(
    records({ ...resources, explorer: { ...resources.explorer, query: 'name' } }),
    [],
  );
});
test('whole-resource search, pagination and related-record back navigation reject stale rows', () => {
  let state = data(resource('products', 'products'), [{ id: 'p1', name: 'House blend' }], {
    matched: 51,
    links: [
      [
        {
          name: 'Variants',
          domain: 'products',
          table: 'product_variants',
          column: 'product_id',
          value: 'p1',
        },
      ],
    ],
  });
  state = key(state, '/').state;
  state = key(state, 'House').state;
  const searched = key(state, '', 'return');
  assert.equal(searched.effect, 'refresh');
  assert.equal(searched.state.explorer.query, 'House');
  assert.deepEqual(
    records(searched.state),
    [],
    'old table/filter rows cannot appear under a new query',
  );
  const paged = key((state = { ...state, input: null }), 'n');
  assert.equal(paged.effect, 'refresh');
  assert.equal(paged.state.explorer.page, 1);
  const links = key(state, 't');
  assert.equal(links.state.explorer.mode, 'links');
  const related = key(links.state, '', 'return');
  assert.equal(related.effect, 'refresh');
  assert.deepEqual(related.state.explorer.relation, { column: 'product_id', value: 'p1' });
  const back = key(related.state, 'b');
  assert.equal(back.state.explorer.mode, 'links');
  const original = key(back.state, 'b');
  assert.equal(original.state.explorer.table, 'products');
  assert.equal(original.effect, 'refresh');
  assert.equal(key(original.state, 'o').state.explorer.sort, 1);
});
test('resource failures, optional availability and narrow/wide rendering stay honest and bounded', () => {
  let state = data(resource('products', 'products'), [
    { id: '1', name: '珈琲\x1b]52;c;evil\x07', total: '9007199254740993.01' },
  ]);
  for (const [cols, rows] of [
    [40, 14],
    [55, 20],
    [77, 24],
    [140, 44],
  ]) {
    const screen = render(state, cols, rows, { color: false });
    assert.ok(screen.split('\n').every((line) => width(line) <= cols - 1));
    assert.ok(screen.split('\n').length <= rows - 1);
    assert.doesNotMatch(screen, /\x1b|\x07/);
    assert.match(screen, /7 Explore/);
  }
  state = key(state, '', 'return').state;
  assert.ok(detailLimit(state, 55, 14) > 0);
  assert.match(
    render({ ...state, detailOffset: 0 }, 120, 44, { color: false }),
    /9007199254740993.01/,
  );
  state = data(resource('products', 'products'), [], {
    unavailable: 'Optional module not installed',
    matched: null,
  });
  assert.match(render(state, 100, 24, { color: false }), /Optional module not installed/);
  assert.doesNotMatch(render(state, 100, 24, { color: false }), /0 matches/);
  state = openExplorer(initialState(), 'vector');
  assert.match(render(state, 100, 30, { color: false }), /OPENAI_API_KEY/);
  assert.equal(explorerRequest(state.explorer), null);
});

test('catalog stays usable without native engine data and domain navigation clears unrelated focus', () => {
  const initial = {
    ...initialState(),
    error: 'Native module is not installed',
    focusTarget: { domain: 'orders', id: 'old-order' },
    focusRecord: { id: 'old-order' },
    relation: { field: 'payments', column: 'order_id', id: 'old-order' },
  };
  let state = openExplorer(initial);
  assert.equal(state.focusTarget, null);
  assert.equal(state.focusRecord, null);
  assert.equal(state.relation, null);
  assert.match(render(state, 100, 24, { color: false }), /89 domains/);
  state = openExplorer(state, 'maintenance');
  assert.match(render(state, 100, 24, { color: false }), /No local table browser/);
  assert.equal(key(state, 'T').effect, undefined);
  const detail = data(resource('products', 'products'), [{ id: 'p' }]);
  assert.equal(key({ ...detail, detail: true }, 'b').state.detail, false);
  assert.equal(key({ ...detail, detail: true }, 'd').state.explorer.mode, 'domains');
});
