import {
  DEFAULT_PLANNING,
  PLANNING_FIELDS,
  planningControls,
  planningEditor,
  constrainedCash,
} from './planning.js';
import { PRODUCT_FIELDS, productEditor, productFromDraft, planningProducts } from './basket.js';

export const VIEWS = ['Overview', 'Orders', 'Inventory', 'Customers', 'Payments', 'Simulation lab'];
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
  if (state.panel === 'scenarios') return state.library.scenarios;
  if (state.panel === 'history') return state.library.runs;
  if (state.panel === 'products') return state.products;
  const rows =
    [
      [],
      state.data?.orders,
      state.data?.inventory,
      state.data?.customers,
      state.data?.payments,
      PRESETS,
    ][state.view] || [];
  const query = (
    state.input?.kind === 'search'
      ? state.input.text
      : state.data?.queries?.[pageKey(state.view)] === state.query
        ? ''
        : state.query
  ).toLocaleLowerCase();
  return rows.filter((row) => JSON.stringify(row).toLocaleLowerCase().includes(query));
}

export function pageKey(view) {
  return { 1: 'orders', 2: 'inventory', 3: 'customers', 4: 'payments' }[view];
}

export function navigate(state, view) {
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
    else if (name === 'return' || name === 'tab' || name === 'up' || name === 'down')
      next.input = {
        ...editor,
        field:
          (editor.field + (name === 'up' || key.shift ? -1 : 1) + fields.length) % fields.length,
      };
    else if (key.ctrl && name === 'u')
      next.input = { ...editor, draft: { ...editor.draft, [field.key]: '' }, error: null };
    else if (name === 'backspace')
      next.input = {
        ...editor,
        draft: { ...editor.draft, [field.key]: editor.draft[field.key].slice(0, -1) },
        error: null,
      };
    else if (text && !key.ctrl && !key.meta && /^[0-9.]+$/.test(text))
      next.input = {
        ...editor,
        draft: { ...editor.draft, [field.key]: (editor.draft[field.key] + text).slice(0, 12) },
        error: null,
      };
    return { state: next };
  }
  if (state.input) {
    if (name === 'escape') next.input = null;
    else if (name === 'return') {
      if (state.input.kind === 'search') {
        next.query = state.input.text;
        next.selected = 0;
        const field = pageKey(state.view);
        if (field && state.loading)
          return {
            state: {
              ...state,
              input: null,
              notice: 'Wait for the current refresh, then search again',
            },
          };
        if (field) {
          next.queries = { ...state.queries, [field]: state.input.text };
          next.pages = { ...state.pages, [field]: 0 };
          effect = 'refresh';
        }
      } else if (state.input.kind === 'save' || state.input.kind === 'export') {
        next.pendingText = state.input.text;
        effect = state.input.kind;
      } else {
        const command = paletteCommands(state.input.text)[state.input.selected || 0];
        if (command?.view !== undefined) next = navigate(next, command.view);
        else if (command) effect = command.effect;
      }
      next.input = null;
    } else if (state.input.kind === 'palette' && ['up', 'down'].includes(name))
      next.input = {
        ...state.input,
        selected: Math.max(
          0,
          Math.min(
            paletteCommands(state.input.text).length - 1,
            (state.input.selected || 0) + (name === 'up' ? -1 : 1),
          ),
        ),
      };
    else if (name === 'backspace')
      next.input = {
        ...state.input,
        selected: 0,
        text: [...state.input.text].slice(0, -1).join(''),
      };
    else if (text && !key.ctrl && !key.meta && !/\p{Cc}/u.test(text))
      next.input = { ...state.input, selected: 0, text: (state.input.text + text).slice(0, 120) };
    if (state.input.kind === 'search') next.selected = 0;
    if (
      (state.relation || state.focusTarget) &&
      (next.relation !== state.relation || next.focusTarget !== state.focusTarget)
    )
      effect = 'refresh';
    return { state: next, effect };
  }
  if (name === 'escape' && state.detail)
    return {
      state: { ...state, detail: false, detailOffset: 0, focusRecord: null, focusTarget: null },
    };
  if (state.help && !['escape'].includes(name) && !['q', '?'].includes(text)) {
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
  else if (text === '/') {
    next.input = { kind: 'search', text: state.query };
    next.detail = false;
    next.focusRecord = null;
    next.focusTarget = null;
  } else if (name === 'tab') next = navigate(state, state.view + (key.shift ? -1 : 1));
  else if (/^[1-6]$/.test(text || '')) next = navigate(state, Number(text) - 1);
  else if (text === 'r') effect = 'refresh';
  else if (text === 'c' && state.sim.running) effect = 'cancel';
  else if (text === 'w' && state.view === 5 && !state.sim.running)
    next.input = { kind: 'save', text: '' };
  else if (text === 'x' && state.view === 5 && state.sim.result)
    next.input = { kind: 'export', text: '' };
  else if (['h', 'l', 'p'].includes(text) && state.view === 5 && !state.sim.running) {
    next.panel = { h: 'history', l: 'scenarios', p: 'products' }[text];
    next.selected = 0;
  } else if (
    text === 'a' &&
    state.view === 2 &&
    !state.sim.running &&
    records(state)[state.selected]
  )
    next.input = productEditor(records(state)[state.selected]);
  else if (text === 'D' && ['history', 'scenarios'].includes(state.panel)) effect = 'remove-entry';
  else if (text === 'd' && state.panel === 'products' && !state.sim.running) {
    next.products = state.products.filter((_, i) => i !== state.selected);
    next.selected = Math.max(0, state.selected - 1);
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
  else if (text === 'f' && state.view === 5 && state.sim.result) {
    next.panel = 'results';
    next.panelOffset = 0;
  } else if (text === 'R' && state.view === 5 && state.sim.result && !state.sim.running)
    effect = 'replay';
  else if (text === 'e' && state.view === 5 && !state.sim.running)
    next.input =
      state.panel === 'products' && records(state)[state.selected]
        ? productEditor(records(state)[state.selected], state.selected)
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
  if (state.view === 5 && !state.panel && !state.input)
    next.selectedPreset = PRESETS[next.selected]?.id || state.selectedPreset;
  if (
    (state.relation || state.focusTarget) &&
    (next.relation !== state.relation || next.focusTarget !== state.focusTarget)
  )
    effect = 'refresh';
  return { state: next, effect };
}

export function paletteCommands(query = '') {
  return [
    ...VIEWS.map((label, view) => ({ label, view })),
    { label: 'Refresh snapshot', effect: 'refresh' },
    { label: 'Keyboard help', effect: 'help' },
    { label: 'Quit', effect: 'quit' },
  ].filter((c) => c.label.toLowerCase().includes(query.toLowerCase()));
}
