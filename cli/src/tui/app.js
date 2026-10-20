import { explorerRequest } from './domains.js';
import readline from 'node:readline';
import { TerminalBackend } from './backend.js';
import { initialState, handleKey, records, PRESETS } from './model.js';
import { runRecord, scenarioSettings, useRunAssumptions } from './workspace.js';
import { productEditor } from './basket.js';
import { readScenarioExport } from './import.js';
import { render, detailLimit, panelLimit, importReviewLimit, contentLimit } from './render.js';

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
  const wasRaw = Boolean(input.isRaw);
  let finish;
  const done = new Promise((resolve) => {
    finish = resolve;
  });
  const draw = () => {
    if (closed || suspended) return;
    if ((state.help || state.view === 0) && !state.input)
      state = {
        ...state,
        contentOffset: Math.min(
          state.contentOffset,
          contentLimit(state, output.columns || 80, output.rows || 24),
        ),
      };
    if (state.input?.kind === 'import-review')
      state = {
        ...state,
        input: {
          ...state.input,
          offset: Math.min(
            state.input.offset,
            importReviewLimit(state.input.preview, output.columns || 80, output.rows || 24),
          ),
        },
      };
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
      if (workspace) await workspace.tail;
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
    explorer: state.view === 6 ? explorerRequest(state.explorer) : null,
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
      if (!closed) {
        const previous = records(state)[state.selected];
        const nextRows = records({ ...state, data });
        const identity = previous?.id || previous?.sku;
        const matched = identity
          ? nextRows.findIndex((item) => (item.id || item.sku) === identity)
          : -1;
        const selected =
          state.view === 5 || state.panel
            ? state.selected
            : matched >= 0
              ? matched
              : Math.min(state.selected, Math.max(0, nextRows.length - 1));
        state = {
          ...state,
          data,
          pages: { ...state.pages, ...data.pages },
          explorer: data.explorer
            ? { ...state.explorer, page: data.explorer.page }
            : state.explorer,
          focusRecord: data.focusRecord || null,
          error: null,
          selected,
        };
      }
    } catch (error) {
      if (!closed && JSON.stringify(request) !== JSON.stringify(snapshotInputs())) {
        refreshAgain = true;
        return;
      }
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
      : state.panel === 'results'
        ? PRESETS.find((p) => p.id === state.selectedPreset)
        : records({ ...state, panel: null })[state.selected];
    if (!preset) return;
    const seed = replay ? state.sim.seed : state.seed;
    const controls = { ...(replay ? state.sim.controls : state.planning) };
    const products = structuredClone(replay ? state.sim.products || [] : state.products);
    const previous = { ...state.sim };
    state = {
      ...state,
      notice: '',
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
    state = {
      ...state,
      notice: {
        save: 'Saving scenario…',
        export: 'Exporting run…',
        import: 'Reading scenario export…',
      }[kind],
    };
    try {
      if (!workspace) throw new Error('No workspace configured');
      if (kind === 'import') {
        const preview = await readScenarioExport(state.pendingText);
        if (!closed)
          state = {
            ...state,
            input: { kind: 'import-review', preview, offset: 0 },
            notice: 'Import ready for review; nothing ran.',
          };
      } else if (kind === 'save') {
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
  const renameScenario = async () => {
    const prompt = state.input;
    try {
      if (!workspace) throw new Error('No workspace configured');
      const library = await workspace.renameScenario(prompt.id, prompt.text);
      if (!closed) {
        state = {
          ...state,
          library,
          ...(state.input === prompt
            ? {
                input: null,
                panelQuery: '',
                selected: library.scenarios.findIndex((item) => item.id === prompt.id),
                notice: 'Scenario renamed. Filter cleared to keep it selected.',
              }
            : {}),
        };
      }
    } catch (error) {
      if (!closed && state.input === prompt)
        state = { ...state, input: { ...prompt, saving: false, error: error.message } };
    }
    draw();
  };
  const openEntry = () => {
    const item = records(state)[state.selected];
    if (!item) return;
    if (state.panel === 'products') {
      state = { ...state, input: productEditor(item, state.products.indexOf(item)) };
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
          'selected',
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
  const undoRemoval = async () => {
    const undo = state.undoRemoval;
    if (!undo) return;
    try {
      let library = state.library,
        products = state.products;
      if (undo.kind === 'products') {
        if (products.length >= 3 || products.some((p) => p.sku === undo.item.sku))
          throw new Error('SKU cannot be restored; undo kept available');
        products = [...products];
        products.splice(Math.min(undo.index, products.length), 0, undo.item);
      } else {
        if (!workspace) throw new Error('No workspace configured');
        library = await workspace.restoreEntry(undo.kind, undo.item);
      }
      if (!closed)
        state = {
          ...state,
          library,
          products,
          undoRemoval: state.undoRemoval === undo ? null : state.undoRemoval,
          selected: 0,
          panelQuery: '',
          notice: 'Entry restored',
        };
    } catch (error) {
      if (!closed) state = { ...state, notice: error.message };
    }
    draw();
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
    if (['save', 'export', 'import'].includes(result.effect)) void persist(result.effect);
    if (result.effect === 'rename-scenario') void renameScenario();
    if (result.effect === 'apply-import') {
      const settings = state.input.preview.settings;
      state = {
        ...state,
        view: 5,
        input: null,
        panel: null,
        panelQuery: '',
        planning: settings.controls,
        products: settings.products,
        seed: settings.seed,
        selectedPreset: settings.presetId,
        selected: PRESETS.findIndex((p) => p.id === settings.presetId),
        notice: 'Imported assumptions only. s runs privately; w saves.',
      };
    }
    if (result.effect === 'undo-removal') void undoRemoval();
    if (result.effect === 'open-entry') {
      try {
        openEntry();
      } catch (error) {
        state = { ...state, notice: error.message };
      }
    }
    if (result.effect === 'use-run-assumptions') {
      try {
        const run =
          state.panel === 'history'
            ? records(state)[state.selected]
            : state.sim.record || state.sim;
        state = useRunAssumptions(state, run);
      } catch (error) {
        state = { ...state, notice: error.message };
      }
    }
    if (result.effect === 'remove-entry') {
      const item = records(state)[state.selected];
      const kind = state.panel === 'history' ? 'runs' : 'scenarios';
      if (item && workspace)
        void workspace
          .removeEntry(kind, item.id)
          .then((library) => {
            if (!closed) {
              state = {
                ...state,
                library,
                selected: 0,
                undoRemoval: { kind, item },
                notice: 'Entry removed. Shift+U restores it.',
              };
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
    if (result.effect === 'open-lab')
      state = {
        ...state,
        view: 5,
        selected: PRESETS.findIndex((p) => p.id === state.selectedPreset),
        detail: false,
        query: '',
        input: null,
      };
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
  const ticker = setInterval(() => {
    if (state.sim.running) draw();
  }, 250);
  ticker.unref();
  enter();
  draw();
  void refresh();
  return done;
}
