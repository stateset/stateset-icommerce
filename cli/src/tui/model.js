export const VIEWS = ['Overview', 'Orders', 'Inventory', 'Customers', 'Payments', 'Simulation lab'];
export const PAGE_SIZE = 50;
export const PRESETS = [
  {
    id: 'baseline',
    name: 'Steady demand',
    note: 'Seven days of demand with next-day replenishment.',
  },
  {
    id: 'spike',
    name: 'Demand spike',
    note: 'Compare steady demand with a threefold demand spike.',
  },
  {
    id: 'delay',
    name: 'Supplier delay',
    note: 'Compare next-day supply with a four-day lead time.',
  },
  { id: 'cash', name: 'Cash constraint', note: 'Compare opening cash of 200.00 with 10.00.' },
];

export function simulationPlans(preset, seed) {
  if (!PRESETS.some((p) => p.id === preset)) throw new Error('Unknown simulation preset');
  if (!Number.isSafeInteger(seed) || seed < 1 || seed > 999999) throw new Error('Invalid seed');
  const base = {
    name: 'Baseline',
    seed,
    currency: 'USD',
    days: 7,
    startingCash: '200.00',
    products: [
      {
        sku: 'COFFEE-250G',
        initialStock: 12,
        unitPrice: '12.50',
        unitCost: '5.25',
        demand: { min: 2, max: 5 },
        reorderPoint: 8,
        targetStock: 24,
        leadTimeDays: 1,
      },
    ],
  };
  if (preset === 'baseline') return [base];
  const candidate = structuredClone(base);
  candidate.name = PRESETS.find((p) => p.id === preset).name;
  if (preset === 'spike') candidate.products[0].demand = { min: 6, max: 15 };
  if (preset === 'delay') candidate.products[0].leadTimeDays = 4;
  if (preset === 'cash') candidate.startingCash = '10.00';
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
    pages: { orders: 0, customers: 0, payments: 0 },
    seed: 42,
    sim: { running: false, result: null, error: null, preset: null, seed: null },
  };
}

export function records(state) {
  const rows =
    [
      [],
      state.data?.orders,
      state.data?.inventory,
      state.data?.customers,
      state.data?.payments,
      PRESETS,
    ][state.view] || [];
  const query = state.query.toLocaleLowerCase();
  return rows.filter((row) => JSON.stringify(row).toLocaleLowerCase().includes(query));
}

export function pageKey(view) {
  return { 1: 'orders', 3: 'customers', 4: 'payments' }[view];
}

export function navigate(state, view) {
  return {
    ...state,
    view: (view + VIEWS.length) % VIEWS.length,
    selected: 0,
    query: '',
    detail: false,
    detailOffset: 0,
    help: false,
    input: null,
  };
}

// Reducer returns explicit effects; keystrokes never become shell commands or
// live-store mutations. Search/palette editing owns keys until dismissed.
export function handleKey(state, text, key = {}) {
  let next = { ...state };
  let effect;
  const name = key.name;
  if (key.ctrl && name === 'c') return { state, effect: 'quit' };
  if (state.input) {
    if (name === 'escape') next.input = null;
    else if (name === 'return') {
      if (state.input.kind === 'search') {
        next.query = state.input.text;
        next.selected = 0;
      } else {
        const command = paletteCommands(state.input.text)[0];
        if (command?.view !== undefined) next = navigate(next, command.view);
        else if (command) effect = command.effect;
      }
      next.input = null;
    } else if (name === 'backspace')
      next.input = { ...state.input, text: [...state.input.text].slice(0, -1).join('') };
    else if (text && !key.ctrl && !key.meta && !/\p{Cc}/u.test(text))
      next.input = { ...state.input, text: (state.input.text + text).slice(0, 120) };
    return { state: next, effect };
  }
  if (name === 'escape')
    return { state: { ...state, detail: false, help: false, query: '', detailOffset: 0 } };
  if (text === 'q') effect = 'quit';
  else if (text === '?') next.help = !state.help;
  else if ((key.ctrl && name === 'p') || text === ':') next.input = { kind: 'palette', text: '' };
  else if (text === '/') next.input = { kind: 'search', text: state.query };
  else if (name === 'tab') next = navigate(state, state.view + (key.shift ? -1 : 1));
  else if (/^[1-6]$/.test(text || '')) next = navigate(state, Number(text) - 1);
  else if (text === 'r') effect = 'refresh';
  else if (text === 'v' && state.view === 5 && state.sim.result) {
    next.detail = !state.detail;
    next.detailOffset = 0;
  } else if (
    (text === 's' || (name === 'return' && !state.detail)) &&
    state.view === 5 &&
    !state.help
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
    if (state.detail) next.detailOffset = Math.max(0, state.detailOffset + delta * amount);
    else
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
    const total = state.data?.counts?.[field];
    if (
      (text === 'p' && page > 0) ||
      (text === 'n' && typeof total === 'number' && (page + 1) * PAGE_SIZE < total)
    ) {
      next.pages = { ...state.pages, [field]: page + (text === 'n' ? 1 : -1) };
      next.selected = 0;
      next.detail = false;
      next.query = '';
      effect = 'refresh';
    }
  }
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
