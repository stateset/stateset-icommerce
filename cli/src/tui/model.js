import { DOMAINS, explorerRows, explorerKey, openExplorer } from './domains.js';
import {
  DEFAULT_PLANNING,
  PLANNING_FIELDS,
  planningControls,
  planningEditor,
  constrainedCash,
} from './planning.js';
import { editText } from './editor.js';
import { graphemes } from './text.js';
import { PRODUCT_FIELDS, productEditor, productFromDraft, planningProducts } from './basket.js';

export const VIEWS = [
  'Overview',
  'Orders',
  'Inventory',
  'Customers',
  'Payments',
  'Simulation lab',
  'Engine explorer',
];
export const PAGE_SIZE = 50;
export const PRESETS = [
  {
    id: 'baseline',
    name: 'Steady demand',
    note: 'Run the current planning assumptions.',
  },
  {
    id: 'spike',
    name: 'Demand spike',
    note: 'Compare steady demand with a threefold demand spike.',
  },
  {
    id: 'delay',
    name: 'Supplier delay',
    note: 'Compare the baseline with three extra supplier days.',
  },
  {
    id: 'cash',
    name: 'Cash constraint',
    note: 'Compare the baseline with 5% of its opening cash.',
  },
];

export function simulationPlans(preset, seed, controls = DEFAULT_PLANNING, products = []) {
  if (!PRESETS.some((p) => p.id === preset)) throw new Error('Unknown simulation preset');
  if (!Number.isSafeInteger(seed) || seed < 1 || seed > 999999) throw new Error('Invalid seed');
  const assumptions = planningControls(controls);
  const base = {
    name: 'Baseline',
    seed,
    currency: 'USD',
    days: assumptions.days,
    startingCash: assumptions.startingCash,
    products: products.length
      ? planningProducts(products)
      : [
          {
            sku: 'COFFEE-250G',
            initialStock: assumptions.initialStock,
            unitPrice: '12.50',
            unitCost: '5.25',
            demand: { min: 2, max: 5 },
            reorderPoint: assumptions.reorderPoint,
            targetStock: assumptions.targetStock,
            leadTimeDays: assumptions.leadTimeDays,
          },
        ],
  };
  if (preset === 'baseline') return [base];
  const candidate = structuredClone(base);
  candidate.name = PRESETS.find((p) => p.id === preset).name;
  if (preset === 'spike')
    for (const item of candidate.products)
      item.demand = { min: item.demand.min * 3, max: item.demand.max * 3 };
  if (preset === 'delay') for (const item of candidate.products) item.leadTimeDays += 3;
  if (preset === 'cash') candidate.startingCash = constrainedCash(assumptions.startingCash);
  return [base, candidate];
}

export function initialState(source = 'Private demo') {
  return {
    explorer: {
      mode: 'domains',
      domain: null,
      table: null,
      query: '',
      page: 0,
      sort: 0,
      stack: [],
      relation: null,
    },
    view: 0,
    selected: 0,
    query: '',
    input: null,
    detail: false,
    detailOffset: 0,
    source,
    data: null,
    loading: false,
    error: null,
    notice: '',
    help: false,
    pages: { orders: 0, inventory: 0, customers: 0, payments: 0 },
    queries: {},
    sorts: {},
    relation: null,
    back: [],
    panel: null,
    panelOffset: 0,
    panelQuery: '',
    undoRemoval: null,
    products: [],
    library: { scenarios: [], runs: [] },
    contentOffset: 0,
    seed: 42,
    selectedPreset: 'baseline',
    planning: { ...DEFAULT_PLANNING },
    sim: { running: false, result: null, error: null, preset: null, seed: null },
  };
}

export function records(state) {
  if (state.view === 6 && !state.panel) return explorerRows(state);
  const library = {
    scenarios: state.library.scenarios,
    history: state.library.runs,
    products: state.products,
  }[state.panel];
  const rows =
    library ||
    [
      [],
      state.data?.orders,
      state.data?.inventory,
      state.data?.customers,
      state.data?.payments,
      PRESETS,
    ][state.view] ||
    [];
  const query = (
    state.input?.kind === 'search'
      ? state.input.text
      : library
        ? state.panelQuery
        : state.data?.queries?.[pageKey(state.view)] === state.query
          ? ''
          : state.query
  ).toLocaleLowerCase();
  return rows.filter((row) =>
    JSON.stringify(
      library
        ? {
            name: row.name,
            preset: row.preset,
            sku: row.sku,
            seed: row.seed,
            createdAt: row.createdAt,
            controls: row.controls,
            products: row.products,
          }
        : row,
    )
      .toLocaleLowerCase()
      .includes(query),
  );
}

export function pageKey(view) {
  return { 1: 'orders', 2: 'inventory', 3: 'customers', 4: 'payments' }[view];
}

export function navigate(state, view) {
  if ((view + VIEWS.length) % VIEWS.length === 6) return openExplorer(state);
  return {
    ...state,
    view: (view + VIEWS.length) % VIEWS.length,
    selected:
      (view + VIEWS.length) % VIEWS.length === 5
        ? PRESETS.findIndex((p) => p.id === state.selectedPreset)
        : 0,
    query: state.queries[pageKey((view + VIEWS.length) % VIEWS.length)] || '',
    detail: false,
    focusRecord: null,
    focusTarget: null,
    detailOffset: 0,
    help: false,
    contentOffset: 0,
    input: null,
    panel: null,
    panelOffset: 0,
    panelQuery: '',
    relation: null,
  };
}

// Reducer returns explicit effects; keystrokes never become shell commands or
// live-store mutations. Search/palette editing owns keys until dismissed.
export function handleKey(state, text, key = {}) {
  let next = { ...state };
  let effect;
  const name = key.name;
  if (key.ctrl && name === 'c') return { state, effect: 'quit' };
  if (state.input?.kind === 'import-review') {
    if (name === 'escape') {
      next.input = null;
      next.notice = 'Import canceled; assumptions unchanged';
    } else if (name === 'return') effect = 'apply-import';
    else if (['up', 'down', 'pageup', 'pagedown', 'home', 'end'].includes(name))
      next.input = {
        ...state.input,
        offset:
          name === 'home'
            ? 0
            : name === 'end'
              ? Number.MAX_SAFE_INTEGER
              : Math.max(
                  0,
                  state.input.offset +
                    (['up', 'pageup'].includes(name) ? -1 : 1) *
                      (['pageup', 'pagedown'].includes(name) ? 8 : 1),
                ),
      };
    return { state: next, effect };
  }
  if (state.input?.kind === 'planning' || state.input?.kind === 'product') {
    const editor = state.input,
      fields =
        editor.kind === 'product'
          ? PRODUCT_FIELDS
          : state.products.length
            ? PLANNING_FIELDS.slice(0, 2)
            : PLANNING_FIELDS,
      field = fields[editor.field];
    const save = () => {
      try {
        if (editor.kind === 'product') {
          const product = productFromDraft(editor.sku, editor.draft);
          next.products = [...state.products];
          if (editor.index >= 0) next.products[editor.index] = product;
          else {
            if (next.products.length >= 3) throw new Error('Choose at most 3 SKUs');
            if (next.products.some((p) => p.sku === product.sku))
              throw new Error('SKU already selected');
            next.products.push(product);
          }
        } else next.planning = planningControls(editor.draft);
        next.input = null;
      } catch (error) {
        next.input = { ...editor, error: error.message };
      }
    };
    if (name === 'escape') next.input = null;
    else if (key.ctrl && name === 's') save();
    else if (name === 'return' && editor.field === fields.length - 1) save();
    else if (name === 'return' || name === 'tab' || name === 'up' || name === 'down') {
      const fieldIndex =
        (editor.field + (name === 'up' || key.shift ? -1 : 1) + fields.length) % fields.length;
      next.input = {
        ...editor,
        field: fieldIndex,
        cursor: graphemes(editor.draft[fields[fieldIndex].key]).length,
      };
    } else {
      const edited = editText(editor.draft[field.key], editor.cursor, text, key, {
        limit: 12,
        accept: (s) => /^[0-9.]+$/.test(s),
      });
      if (edited)
        next.input = {
          ...editor,
          cursor: edited.cursor,
          draft: { ...editor.draft, [field.key]: edited.text },
          error: edited.changed ? null : editor.error,
        };
    }
    return { state: next };
  }
  if (state.input) {
    if (state.input.saving) return { state };
    if (name === 'escape') next.input = null;
    else if (name === 'return') {
      if (state.input.kind === 'rename') {
        if (!state.input.text.trim() || state.input.text.length > 80)
          return {
            state: {
              ...state,
              input: { ...state.input, error: 'Choose a scenario name (1–80 characters)' },
            },
          };
        return {
          state: { ...state, input: { ...state.input, saving: true, error: null } },
          effect: 'rename-scenario',
        };
      } else if (state.input.kind === 'seed') {
        if (!/^[1-9]\d{0,5}$/.test(state.input.text))
          return {
            state: {
              ...state,
              input: { ...state.input, error: 'Seed must be an integer from 1 to 999999' },
            },
          };
        next.seed = Number(state.input.text);
      } else if (state.input.kind === 'search' && state.panel) {
        next.panelQuery = state.input.text;
        next.selected = 0;
      } else if (state.input.kind === 'search' && state.view === 6) {
        next.explorer = { ...state.explorer, query: state.input.text, page: 0 };
        next.selected = 0;
        if (next.explorer.mode === 'records') effect = 'refresh';
      } else if (state.input.kind === 'search') {
        next.query = state.input.text;
        next.selected = 0;
        const field = pageKey(state.view);
        if (field) {
          next.queries = { ...state.queries, [field]: state.input.text };
          next.pages = { ...state.pages, [field]: 0 };
          effect = 'refresh';
        }
      } else if (['save', 'export', 'import'].includes(state.input.kind)) {
        next.pendingText = state.input.text;
        effect = state.input.kind;
      } else {
        const command = paletteCommands(state.input.text, state)[state.input.selected || 0];
        if (command?.domain) next = openExplorer(next, command.domain);
        else if (command?.view !== undefined) next = navigate(next, command.view);
        else if (command?.key) return handleKey({ ...state, input: null }, command.key, {});
        else if (command) effect = command.effect;
      }
      next.input = null;
    } else if (state.input.kind === 'palette' && ['up', 'down'].includes(name))
      next.input = {
        ...state.input,
        selected: Math.max(
          0,
          Math.min(
            paletteCommands(state.input.text, state).length - 1,
            (state.input.selected || 0) + (name === 'up' ? -1 : 1),
          ),
        ),
      };
    else {
      const edited = editText(state.input.text, state.input.cursor, text, key, {
        limit: ['export', 'import'].includes(state.input.kind)
          ? 1024
          : state.input.kind === 'seed'
            ? 6
            : state.input.kind === 'rename'
              ? 80
              : 120,
        accept: state.input.kind === 'seed' ? (s) => /^\d+$/.test(s) : undefined,
      });
      if (edited)
        next.input = {
          ...state.input,
          text: edited.text,
          cursor: edited.cursor,
          selected: edited.changed ? 0 : state.input.selected,
          error: edited.changed ? null : state.input.error,
        };
    }
    if (state.input.kind === 'search') {
      next.selected = 0;
      if (next.view === 5 && !next.panel && !next.input)
        next.selectedPreset = records(next)[0]?.id || next.selectedPreset;
    }
    if (
      (state.relation || state.focusTarget) &&
      (next.relation !== state.relation || next.focusTarget !== state.focusTarget)
    )
      effect = 'refresh';
    return { state: next, effect };
  }
  if (state.help) {
    if (name === 'escape' || text === '?')
      return { state: { ...state, help: false, contentOffset: 0 } };
    if (text === 'q') return { state, effect: 'quit' };
    if (['up', 'down', 'pageup', 'pagedown', 'home', 'end'].includes(name))
      next.contentOffset =
        name === 'home'
          ? 0
          : name === 'end'
            ? Number.MAX_SAFE_INTEGER
            : Math.max(
                0,
                state.contentOffset +
                  (['up', 'pageup'].includes(name) ? -1 : 1) *
                    (['pageup', 'pagedown'].includes(name) ? 10 : 1),
              );
    return { state: next };
  }
  if (state.view === 6) {
    const result = explorerKey(state, text, key);
    if (result) return result;
  }
  if (name === 'escape' && state.detail)
    return {
      state: { ...state, detail: false, detailOffset: 0, focusRecord: null, focusTarget: null },
    };
  if (name === 'escape' && state.panelQuery && state.panel)
    return { state: { ...state, panelQuery: '', selected: 0 } };
  if (name === 'escape' && state.panel)
    return {
      state: {
        ...state,
        panel: null,
        panelOffset: 0,
        selected: PRESETS.findIndex((p) => p.id === state.selectedPreset),
      },
    };
  if (name === 'escape' && state.query && pageKey(state.view))
    return {
      state: {
        ...state,
        query: '',
        queries: { ...state.queries, [pageKey(state.view)]: '' },
        pages: { ...state.pages, [pageKey(state.view)]: 0 },
      },
      effect: 'refresh',
    };
  if (name === 'escape')
    return { state: { ...state, detail: false, help: false, query: '', detailOffset: 0 } };
  if (text === 'q') effect = 'quit';
  else if (text === '?') {
    next.help = !state.help;
    next.contentOffset = 0;
  } else if ((key.ctrl && name === 'p') || text === ':') next.input = { kind: 'palette', text: '' };
  else if (
    text === '/' &&
    (!state.panel || ['scenarios', 'history', 'products'].includes(state.panel))
  ) {
    next.input = {
      kind: 'search',
      text: state.view === 6 ? state.explorer.query : state.panel ? state.panelQuery : state.query,
    };
    next.detail = false;
    next.focusRecord = null;
    next.focusTarget = null;
  } else if (name === 'tab') next = navigate(state, state.view + (key.shift ? -1 : 1));
  else if (/^[1-7]$/.test(text || '')) next = navigate(state, Number(text) - 1);
  else if (text === 'r') effect = 'refresh';
  else if (text === 'c' && state.sim.running) effect = 'cancel';
  else if (text === 'g' && state.view === 5 && !state.sim.running)
    next.input = { kind: 'seed', text: String(state.seed) };
  else if (text === 'i' && state.view === 5 && !state.sim.running)
    next.input = { kind: 'import', text: '' };
  else if (text === 'U' && state.undoRemoval && !state.sim.running) effect = 'undo-removal';
  else if (text === 'w' && state.view === 5 && !state.sim.running)
    next.input = { kind: 'save', text: '' };
  else if (
    text === 'N' &&
    state.view === 5 &&
    state.panel === 'scenarios' &&
    !state.sim.running &&
    records(state)[state.selected]
  ) {
    const item = records(state)[state.selected];
    next.input = { kind: 'rename', id: item.id, text: item.name };
  } else if (text === 'x' && state.view === 5 && state.sim.result)
    next.input = { kind: 'export', text: '' };
  else if (['h', 'l', 'p'].includes(text) && state.view === 5 && !state.sim.running) {
    next.panel = { h: 'history', l: 'scenarios', p: 'products' }[text];
    next.selected = 0;
    next.panelQuery = '';
  } else if (
    text === 'a' &&
    state.view === 2 &&
    !state.sim.running &&
    records(state)[state.selected]
  )
    next.input = productEditor(records(state)[state.selected]);
  else if (text === 'D' && ['history', 'scenarios'].includes(state.panel)) effect = 'remove-entry';
  else if (text === 'd' && state.panel === 'products' && !state.sim.running) {
    const item = records(state)[state.selected];
    if (item) {
      next.undoRemoval = { kind: 'products', item, index: state.products.indexOf(item) };
      next.products = state.products.filter((p) => p.sku !== item.sku);
    }
    next.selected = Math.max(0, state.selected - 1);
  } else if (
    ['[', ']'].includes(text) &&
    state.view === 5 &&
    state.panel === 'products' &&
    !state.sim.running
  ) {
    const item = records(state)[state.selected];
    if (!item) return { state };
    const from = state.products.indexOf(item);
    const to = from + (text === '[' ? -1 : 1);
    if (from < 0 || to < 0 || to >= state.products.length) return { state };
    next.products = [...state.products];
    next.products.splice(from, 1);
    next.products.splice(to, 0, item);
    next.selected = records(next).findIndex((product) => product.sku === item.sku);
    next.notice = `${item.sku}: purchase priority ${to + 1} of ${next.products.length}. Applies to the next run.`;
  } else if (
    text === 'o' &&
    !state.detail &&
    !state.panel &&
    pageKey(state.view) &&
    !state.loading
  ) {
    const f = pageKey(state.view);
    next.sorts = { ...state.sorts, [f]: state.sorts[f] === 1 ? 0 : 1 };
    next.pages = { ...state.pages, [f]: 0 };
    effect = 'refresh';
  } else if (['t', 'u'].includes(text) && state.detail && state.view !== 5 && !state.loading)
    effect = text === 'u' ? 'customer' : 'related';
  else if (text === 'b' && state.back.length) effect = 'back';
  else if (name === 'return' && ['scenarios', 'history', 'products'].includes(state.panel))
    effect = 'open-entry';
  else if (
    text === 'a' &&
    state.view === 5 &&
    !state.sim.running &&
    ((state.panel === 'history' && records(state)[state.selected]) ||
      (state.panel === 'results' && state.sim.result))
  )
    effect = 'use-run-assumptions';
  else if (text === 'f' && state.view === 5 && state.sim.result) {
    next.panel = 'results';
    next.panelOffset = 0;
  } else if (text === 'R' && state.view === 5 && state.sim.result && !state.sim.running)
    effect = 'replay';
  else if (text === 'e' && state.view === 5 && !state.sim.running)
    next.input =
      state.panel === 'products' && records(state)[state.selected]
        ? productEditor(
            records(state)[state.selected],
            state.products.indexOf(records(state)[state.selected]),
          )
        : planningEditor(state.planning);
  else if (text === 'v' && state.view === 5 && state.sim.result) {
    next.detail = !state.detail;
    next.detailOffset = 0;
  } else if (
    (text === 's' || (name === 'return' && !state.detail)) &&
    state.view === 5 &&
    !state.help &&
    (!state.panel || state.panel === 'results')
  )
    effect = 'simulate';
  else if ((text === '+' || text === '=') && state.view === 5)
    next.seed = Math.min(999999, state.seed + 1);
  else if (text === '-' && state.view === 5) next.seed = Math.max(1, state.seed - 1);
  else if (name === 'return' && records(state).length) {
    next.detail = !state.detail;
    next.detailOffset = 0;
  } else if (
    ['up', 'down', 'pageup', 'pagedown', 'home', 'end'].includes(name) ||
    ['j', 'k'].includes(text)
  ) {
    const delta = ['up', 'pageup'].includes(name) || text === 'k' ? -1 : 1;
    const amount = ['pageup', 'pagedown'].includes(name) ? 10 : 1;
    if (state.detail) {
      if (name === 'home') next.detailOffset = 0;
      else if (name === 'end') effect = 'detail-end';
      else next.detailOffset = Math.max(0, state.detailOffset + delta * amount);
    } else if (state.view === 0) {
      next.contentOffset =
        name === 'home'
          ? 0
          : name === 'end'
            ? Number.MAX_SAFE_INTEGER
            : Math.max(0, state.contentOffset + delta * amount);
    } else if (state.panel === 'results') {
      next.panelOffset =
        name === 'home'
          ? 0
          : name === 'end'
            ? Number.MAX_SAFE_INTEGER
            : Math.max(0, state.panelOffset + delta * amount);
    } else
      next.selected = Math.max(
        0,
        Math.min(
          records(state).length - 1,
          name === 'home'
            ? 0
            : name === 'end'
              ? records(state).length - 1
              : state.selected + delta * amount,
        ),
      );
  } else if (['n', 'p'].includes(text) && pageKey(state.view) && !state.loading) {
    const field = pageKey(state.view),
      page = state.pages[field];
    const total = state.data?.matched?.[field] ?? state.data?.counts?.[field];
    if (
      (text === 'p' && page > 0) ||
      (text === 'n' && typeof total === 'number' && (page + 1) * PAGE_SIZE < total)
    ) {
      next.pages = { ...state.pages, [field]: page + (text === 'n' ? 1 : -1) };
      next.selected = 0;
      next.detail = false;
      next.query = state.queries[field] || '';
      effect = 'refresh';
    }
  }
  if (next.view === 5 && !next.panel && !next.input)
    next.selectedPreset = records(next)[next.selected]?.id || state.selectedPreset;
  if (
    (state.relation || state.focusTarget) &&
    (next.relation !== state.relation || next.focusTarget !== state.focusTarget)
  )
    effect = 'refresh';
  return { state: next, effect };
}

export function paletteCommands(query = '', state) {
  return [
    ...VIEWS.map((label, view) => ({ label, view })),
    { label: 'Refresh snapshot', effect: 'refresh' },
    { label: 'Keyboard help', effect: 'help' },
    { label: 'Quit', effect: 'quit' },
    ...(state?.view === 5
      ? [
          ...(!state.sim.running
            ? [
                { label: 'Edit planning assumptions', key: 'e' },
                { label: 'Set simulation seed', key: 'g' },
                { label: 'Selected SKU terms', key: 'p' },
                ...(state.panel === 'products' && state.products.length > 1
                  ? [
                      { label: 'Move selected SKU earlier in purchase priority', key: '[' },
                      { label: 'Move selected SKU later in purchase priority', key: ']' },
                    ]
                  : []),
                { label: 'Save scenario', key: 'w' },
                { label: 'Saved scenarios', key: 'l' },
                ...(state.panel === 'scenarios' && records(state)[state.selected]
                  ? [{ label: 'Rename selected saved scenario', key: 'N' }]
                  : []),
                { label: 'Run history', key: 'h' },
                ...((state.panel === 'history' && records(state)[state.selected]) ||
                (state.panel === 'results' && state.sim.result)
                  ? [{ label: 'Use run assumptions as an editable draft', key: 'a' }]
                  : []),
                { label: 'Import scenario from JSON', key: 'i' },
              ]
            : [{ label: 'Cancel active simulation', key: 'c' }]),
          ...(state.sim.result
            ? [
                { label: 'Compare last result', key: 'f' },
                { label: 'Export successful run', key: 'x' },
                ...(!state.sim.running ? [{ label: 'Replay successful run', key: 'R' }] : []),
              ]
            : []),
        ]
      : []),
    ...DOMAINS.map((d) => ({ label: `Explore ${d.name} · ${d.group}`, domain: d.id })),
  ].filter((c) => c.label.toLowerCase().includes(query.toLowerCase()));
}
