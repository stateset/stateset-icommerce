import readline from 'node:readline';
import { TerminalBackend } from './backend.js';
import { initialState, handleKey, records, PRESETS } from './model.js';
import { runRecord, scenarioSettings } from './workspace.js';
import { productEditor } from './basket.js';
import { render, detailLimit, panelLimit } from './render.js';

export async function startTerminal({
  dbPath = null,
  input = process.stdin,
  output = process.stdout,
  backend = new TerminalBackend(dbPath),
  signals = process,
  workspace = null,
  color = !process.env.NO_COLOR,
} = {}) {
  let state = initialState(dbPath ? `Snapshot · ${dbPath}` : 'DEMO · private engine store');
  let closed = false,
    suspended = false;
  if (workspace) {
    try {
      state.library = await workspace.load();
    } catch (error) {
      state.notice = error.message;
    }
  }
  let ticker;
  const wasRaw = Boolean(input.isRaw);
  let finish;
  const done = new Promise((resolve) => {
    finish = resolve;
  });
  const draw = () => {
    if (closed || suspended) return;
    if (state.panel === 'results')
      state = {
        ...state,
        panelOffset: Math.min(
          state.panelOffset,
          panelLimit(state, output.columns || 80, output.rows || 24),
        ),
      };
    output.write(
      '\x1b[H' +
        render(state, output.columns || 80, output.rows || 24, { color }).replace(
          /\n/g,
          '\x1b[K\r\n',
        ) +
        '\x1b[J',
    );
  };
  const enter = () => {
    input.setRawMode(true);
    output.write('\x1b[?1049h\x1b[?25l');
    input.resume();
  };
  const restore = () => {
    try {
      input.setRawMode(wasRaw);
    } catch {
      /* The TTY may already be gone. */
    }
    if (!output.destroyed) output.write('\x1b[0m\x1b[?25h\x1b[?1049l');
  };
  const close = async () => {
    if (closed) return;
    closed = true;
    clearInterval(ticker);
    input.off('keypress', onKey);
    input.off('end', close);
    input.off('close', close);
    input.off('error', onOutputError);
    output.off('resize', draw);
    output.off('error', onOutputError);
    for (const name of ['SIGINT', 'SIGTERM', 'SIGHUP']) signals.off(name, close);
    signals.off('SIGTSTP', suspend);
    signals.off('SIGCONT', resume);
    signals.off('exit', onExit);
    restore();
    input.pause();
    try {
      await backend.close();
    } finally {
      finish();
    }
  };
  const onOutputError = () => {
    void close();
  };
  const onExit = () => {
    if (!closed) restore();
  };
  const suspend = () => {
    if (!closed) {
      suspended = true;
      restore();
      signals.kill(signals.pid, 'SIGSTOP');
    }
  };
  const resume = () => {
    if (!closed) {
      suspended = false;
      enter();
      draw();
    }
  };
  let refreshAgain = false;
  const snapshotInputs = () => ({
    pages: { ...state.pages },
    queries: { ...state.queries },
    sorts: { ...state.sorts },
    relation: state.relation,
    focus: state.focusTarget,
  });
  const refresh = async () => {
    if (closed) return;
    if (state.loading) {
      refreshAgain = true;
      return;
    }
    const request = snapshotInputs();
    state = { ...state, loading: true, error: null };
    draw();
    try {
      const data = await backend.request('snapshot', request);
      if (!closed && JSON.stringify(request) !== JSON.stringify(snapshotInputs())) {
        refreshAgain = true;
        return;
      }
      if (!closed)
        state = {
          ...state,
          data,
          pages: { ...state.pages, ...data.pages },
          focusRecord: data.focusRecord || null,
          error: null,
          selected: 0,
        };
    } catch (error) {
      if (!closed)
        state = { ...state, error: error.message, pages: state.data?.pages || state.pages };
    } finally {
      if (!closed) {
        state = { ...state, loading: false };
        draw();
        if (refreshAgain) {
          refreshAgain = false;
          void refresh();
        }
      }
    }
  };
  const simulate = async (replay = false) => {
    if (closed || state.view !== 5 || state.sim.running) return;
    const preset = replay
      ? { id: state.sim.presetId, name: state.sim.preset }
      : records({ ...state, panel: null })[state.selected];
    if (!preset) return;
    const seed = replay ? state.sim.seed : state.seed;
    const controls = { ...(replay ? state.sim.controls : state.planning) };
    const products = structuredClone(replay ? state.sim.products || [] : state.products);
    const previous = { ...state.sim };
    state = {
      ...state,
      sim: {
        ...previous,
        running: true,
        error: null,
        pending: { preset: preset.name, seed, controls },
        startedAt: Date.now(),
        progress: 'Preparing private engine',
      },
    };
    draw();
    try {
      const result = await backend.request(
        'simulate',
        { preset: preset.id, seed, controls, products },
        (progress) => {
          if (!closed) {
            state = { ...state, sim: { ...state.sim, progress } };
            draw();
          }
        },
      );
      if (!closed)
        state = {
          ...state,
          sim: {
            ...state.sim,
            result,
            preset: preset.name,
            presetId: preset.id,
            seed,
            controls,
            products,
            record: null,
          },
          panel: state.view === 5 ? 'results' : state.panel,
          panelOffset: 0,
        };
      if (!closed && workspace) {
        const record = runRecord(state.sim);
        state = { ...state, sim: { ...state.sim, record } };
        try {
          const library = await workspace.appendRun(record);
          if (!closed) state = { ...state, library };
        } catch (error) {
          if (!closed)
            state = { ...state, notice: `Run complete; history not saved: ${error.message}` };
        }
      }
    } catch (error) {
      if (!closed) state = { ...state, sim: { ...state.sim, error: error.message } };
    } finally {
      if (!closed) {
        state = { ...state, sim: { ...state.sim, running: false, pending: null } };
        draw();
      }
    }
  };
  const persist = async (kind) => {
    try {
      if (!workspace) throw new Error('No workspace configured');
      if (kind === 'save') {
        const preset = PRESETS.find((p) => p.id === state.selectedPreset);
        if (!preset) throw new Error('Choose a scenario first');
        const library = await workspace.saveScenario(state.pendingText, {
          presetId: preset.id,
          seed: state.seed,
          controls: state.planning,
          products: state.products,
        });
        if (!closed) state = { ...state, library, notice: 'Scenario saved. l opens the library.' };
      } else {
        if (!state.pendingText?.trim()) throw new Error('Enter a destination JSON filename');
        const target = await workspace.exportRun(
          state.pendingText.trim(),
          state.sim.record || runRecord(state.sim),
        );
        if (!closed) state = { ...state, notice: `Exported ${target}` };
      }
    } catch (error) {
      if (!closed) state = { ...state, notice: error.message };
    }
    draw();
  };
  const openEntry = () => {
    const item = records(state)[state.selected];
    if (!item) return;
    if (state.panel === 'products') {
      state = { ...state, input: productEditor(item, state.selected) };
      return;
    }
    const settings = scenarioSettings(item);
    if (state.panel === 'history') {
      state = {
        ...state,
        sim: {
          ...settings,
          preset: item.preset,
          result: item.result,
          record: item,
          running: false,
          error: null,
        },
        panel: 'results',
        panelOffset: 0,
      };
    } else
      state = {
        ...state,
        planning: settings.controls,
        products: settings.products,
        seed: settings.seed,
        selected: ['baseline', 'spike', 'delay', 'cash'].indexOf(settings.presetId),
        selectedPreset: settings.presetId,
        panel: null,
        notice: `Loaded ${item.name}`,
      };
  };
  const related = async (effect) => {
    if (effect === 'back') {
      const prior = state.back[state.back.length - 1];
      state = { ...state, ...prior, back: state.back.slice(0, -1) };
    } else {
      const record = state.focusRecord || records(state)[state.selected];
      if (!record) return;
      const prior = Object.fromEntries(
        [
          'view',
          'pages',
          'queries',
          'relation',
          'query',
          'detail',
          'focusRecord',
          'focusTarget',
        ].map((k) => [k, state[k]]),
      );
      const back = [...state.back, prior].slice(-10);
      if (effect === 'customer') {
        if (!record.customerId) {
          state = { ...state, notice: 'No customer linked to this record' };
          return;
        }
        state = {
          ...state,
          view: 3,
          detail: true,
          focusTarget: { domain: 'customers', id: record.customerId },
          focusRecord: null,
          back,
          query: '',
        };
      } else if (state.view === 4) {
        if (!record.orderId) {
          state = { ...state, notice: 'No order linked to this payment' };
          return;
        }
        state = {
          ...state,
          view: 1,
          detail: true,
          focusTarget: { domain: 'orders', id: record.orderId },
          focusRecord: null,
          back,
          query: '',
        };
      } else {
        const field = state.view === 3 ? 'orders' : state.view === 1 ? 'payments' : null;
        if (!field) return;
        state = {
          ...state,
          view: field === 'orders' ? 1 : 4,
          detail: false,
          focusTarget: null,
          focusRecord: null,
          relation: {
            field,
            column: field === 'orders' ? 'customer_id' : 'order_id',
            id: record.id,
          },
          pages: { ...state.pages, [field]: 0 },
          queries: { ...state.queries, [field]: '' },
          query: '',
          selected: 0,
          back,
        };
      }
    }
    await refresh();
  };
  function onKey(text, key) {
    if (key?.ctrl && key.name === 'z') {
      suspend();
      return;
    }
    const result = handleKey(state, text, key);
    state = result.state;
    if (state.detail) {
      const limit = detailLimit(state, output.columns || 80, output.rows || 24);
      state = {
        ...state,
        detailOffset: result.effect === 'detail-end' ? limit : Math.min(state.detailOffset, limit),
      };
    }
    if (result.effect === 'quit') {
      void close();
      return;
    }
    if (result.effect === 'cancel') backend.cancel('simulate');
    if (result.effect === 'save' || result.effect === 'export') void persist(result.effect);
    if (result.effect === 'open-entry') {
      try {
        openEntry();
      } catch (error) {
        state = { ...state, notice: error.message };
      }
    }
    if (result.effect === 'remove-entry') {
      const item = records(state)[state.selected];
      if (item && workspace)
        void workspace
          .removeEntry(state.panel === 'history' ? 'runs' : 'scenarios', item.id)
          .then((library) => {
            if (!closed) {
              state = { ...state, library, selected: 0, notice: 'Entry removed' };
              draw();
            }
          })
          .catch((error) => {
            if (!closed) {
              state = { ...state, notice: error.message };
              draw();
            }
          });
    }
    if (['related', 'customer', 'back'].includes(result.effect)) void related(result.effect);
    if (result.effect === 'refresh') void refresh();
    if (result.effect === 'simulate') void simulate();
    if (result.effect === 'replay') void simulate(true);
    if (result.effect === 'help') state = { ...state, help: true };
    draw();
  }
  readline.emitKeypressEvents(input);
  input.on('keypress', onKey);
  input.on('end', close);
  input.on('close', close);
  input.on('error', onOutputError);
  output.on('resize', draw);
  output.on('error', onOutputError);
  for (const name of ['SIGINT', 'SIGTERM', 'SIGHUP']) signals.on(name, close);
  signals.on('SIGTSTP', suspend);
  signals.on('SIGCONT', resume);
  signals.on('exit', onExit);
  ticker = setInterval(() => {
    if (state.sim.running) draw();
  }, 250);
  ticker.unref();
  enter();
  draw();
  void refresh();
  return done;
}
