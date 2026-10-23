import readline from 'node:readline';
import { TerminalBackend } from './backend.js';
import { initialState, handleKey, records } from './model.js';
import { render } from './render.js';

export async function startTerminal({
  dbPath = null,
  input = process.stdin,
  output = process.stdout,
  backend = new TerminalBackend(dbPath),
  signals = process,
  color = !process.env.NO_COLOR,
} = {}) {
  let state = initialState(dbPath ? `Snapshot · ${dbPath}` : 'DEMO · private engine store');
  let closed = false,
    suspended = false;
  const wasRaw = Boolean(input.isRaw);
  let finish;
  const done = new Promise((resolve) => {
    finish = resolve;
  });
  const draw = () => {
    if (closed || suspended) return;
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
  const refresh = async () => {
    if (closed || state.loading) return;
    state = { ...state, loading: true, error: null };
    draw();
    try {
      const data = await backend.request('snapshot', { pages: { ...state.pages } });
      if (!closed) state = { ...state, data, error: null, selected: 0 };
    } catch (error) {
      if (!closed)
        state = { ...state, error: error.message, pages: state.data?.pages || state.pages };
    } finally {
      if (!closed) {
        state = { ...state, loading: false };
        draw();
      }
    }
  };
  const simulate = async () => {
    if (closed || state.view !== 5 || state.sim.running) return;
    const preset = records(state)[state.selected];
    if (!preset) return;
    const seed = state.seed;
    state = {
      ...state,
      sim: { running: true, result: null, error: null, preset: preset.name, seed },
    };
    draw();
    try {
      const result = await backend.request('simulate', { preset: preset.id, seed });
      if (!closed) state = { ...state, sim: { ...state.sim, result } };
    } catch (error) {
      if (!closed) state = { ...state, sim: { ...state.sim, error: error.message } };
    } finally {
      if (!closed) {
        state = { ...state, sim: { ...state.sim, running: false } };
        draw();
      }
    }
  };
  function onKey(text, key) {
    if (key?.ctrl && key.name === 'z') {
      suspend();
      return;
    }
    const result = handleKey(state, text, key);
    state = result.state;
    if (result.effect === 'quit') {
      void close();
      return;
    }
    if (result.effect === 'refresh') void refresh();
    if (result.effect === 'simulate') void simulate();
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
  enter();
  draw();
  void refresh();
  return done;
}
