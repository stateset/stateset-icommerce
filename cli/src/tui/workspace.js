import fs from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import { createHash, randomUUID } from 'node:crypto';
import { createRequire } from 'node:module';
import { planningControls } from './planning.js';
import { planningProducts } from './basket.js';
import { simulationPlans, navigate, PRESETS } from './model.js';
import { SIMULATION_KERNEL_VERSION } from '../simulation/limits.js';

const require = createRequire(import.meta.url);
const VERSION = require('../../package.json').version;
const MAX_BYTES = 8 * 1024 * 1024;
const empty = () => ({ version: 1, scenarios: [], runs: [] });
export const digestInputs = (inputs) =>
  createHash('sha256').update(JSON.stringify(inputs)).digest('hex');

export function scenarioSettings(value) {
  if (
    !value ||
    !['baseline', 'spike', 'delay', 'cash'].includes(value.presetId) ||
    !Number.isSafeInteger(value.seed) ||
    value.seed < 1 ||
    value.seed > 999999
  )
    throw new Error('Invalid saved scenario');
  return {
    presetId: value.presetId,
    seed: value.seed,
    controls: planningControls(value.controls),
    products: planningProducts(value.products || []),
  };
}

export function runRecord(sim) {
  const settings = scenarioSettings({ ...sim, controls: sim.controls, products: sim.products });
  if (!sim.result?.success || !Array.isArray(sim.result.inputs))
    throw new Error('No successful run to save');
  return {
    id: randomUUID(),
    createdAt: new Date().toISOString(),
    cliVersion: VERSION,
    kernelVersion: SIMULATION_KERNEL_VERSION,
    nodeVersion: process.version,
    ...settings,
    preset: sim.preset,
    inputDigest: digestInputs(sim.result.inputs),
    result: sim.result,
  };
}

export function useRunAssumptions(state, run) {
  if (state.sim.running)
    throw new Error('Wait for the active simulation before loading assumptions');
  const settings = scenarioSettings(run);
  if (
    !run.result?.success ||
    JSON.stringify(
      simulationPlans(settings.presetId, settings.seed, settings.controls, settings.products),
    ) !== JSON.stringify(run.result.inputs)
  )
    throw new Error('Run assumptions differ from executed inputs');
  if (run.inputDigest !== undefined && run.inputDigest !== digestInputs(run.result.inputs))
    throw new Error('Run input digest mismatch');
  return {
    ...navigate(state, 5),
    planning: settings.controls,
    products: settings.products,
    seed: settings.seed,
    selectedPreset: settings.presetId,
    selected: PRESETS.findIndex((preset) => preset.id === settings.presetId),
    notice: 'Run assumptions loaded. e edits; s runs privately; w saves a new scenario.',
  };
}

export class TerminalWorkspace {
  constructor(directory, source = 'demo') {
    const key = createHash('sha256')
      .update(source === 'demo' ? source : path.resolve(source))
      .digest('hex')
      .slice(0, 20);
    this.directory = path.join(
      directory ||
        path.join(
          process.env.XDG_STATE_HOME || path.join(os.homedir(), '.local/state'),
          'stateset/tui',
        ),
      key,
    );
    this.file = path.join(this.directory, 'workspace.json');
    this.data = empty();
    this.tail = Promise.resolve();
    this.unreadable = false;
  }
  async load() {
    try {
      const stat = await fs.lstat(this.file);
      if (!stat.isFile() || stat.isSymbolicLink() || stat.size > MAX_BYTES)
        throw new Error('Invalid or oversized workspace file');
      const data = JSON.parse(await fs.readFile(this.file, 'utf8'));
      if (
        data.version !== 1 ||
        !Array.isArray(data.scenarios) ||
        !Array.isArray(data.runs) ||
        data.scenarios.length > 20 ||
        data.runs.length > 20
      )
        throw new Error('Unsupported workspace format');
      for (const item of data.scenarios) {
        scenarioSettings(item);
        if (typeof item.name !== 'string' || item.name.length > 80)
          throw new Error('Invalid scenario name');
      }
      for (const item of data.runs) {
        const settings = scenarioSettings(item);
        if (
          JSON.stringify(
            simulationPlans(settings.presetId, settings.seed, settings.controls, settings.products),
          ) !== JSON.stringify(item.result?.inputs)
        )
          throw new Error('Saved run assumptions differ from executed inputs');
        if (
          !Array.isArray(item.result?.results) ||
          !item.result.results.length ||
          item.result.results.some(
            (r) =>
              typeof r.name !== 'string' ||
              !r.summary ||
              !Array.isArray(r.timeline) ||
              !Array.isArray(r.inventory),
          )
        )
          throw new Error('Invalid saved report');
        if (!item.result?.success || item.inputDigest !== digestInputs(item.result.inputs))
          throw new Error('Saved run input digest mismatch');
      }
      this.data = data;
    } catch (error) {
      if (error.code !== 'ENOENT') {
        this.unreadable = true;
        throw new Error(`Workspace could not load: ${error.message}. Original file retained.`);
      }
    }
    return this.data;
  }
  update(change) {
    const operation = this.tail.then(async () => {
      if (this.unreadable)
        throw new Error('Workspace is unreadable; original file will not be overwritten');
      await fs.mkdir(this.directory, { recursive: true, mode: 0o700 });
      if ((await fs.lstat(this.directory)).isSymbolicLink())
        throw new Error('Workspace directory must not be a symlink');
      const lockPath = path.join(this.directory, 'workspace.lock');
      let lock;
      try {
        lock = await fs.open(lockPath, 'wx', 0o600);
      } catch (error) {
        if (error.code === 'EEXIST')
          throw new Error(
            'Workspace is locked by another writer or an interrupted save; original data preserved',
          );
        throw error;
      }
      try {
        await this.load();
        const next = change(structuredClone(this.data));
        const content = JSON.stringify(next, null, 2) + '\n';
        if (Buffer.byteLength(content) > MAX_BYTES)
          throw new Error('Workspace is full; export the run separately');
        await fs.mkdir(this.directory, { recursive: true, mode: 0o700 });
        const temporary = path.join(this.directory, `.workspace-${randomUUID()}.tmp`);
        try {
          const handle = await fs.open(temporary, 'wx', 0o600);
          try {
            await handle.writeFile(content);
            await handle.sync();
          } finally {
            await handle.close();
          }
          await fs.rename(temporary, this.file);
        } finally {
          await fs.rm(temporary, { force: true });
        }
        this.data = next;
        return next;
      } finally {
        await lock.close();
        await fs.rm(lockPath, { force: true });
      }
    });
    this.tail = operation.catch(() => {});
    return operation;
  }
  saveScenario(name, settings) {
    if (typeof name !== 'string' || !name.trim() || name.length > 80)
      throw new Error('Choose a scenario name (1–80 characters)');
    const item = { id: randomUUID(), name: name.trim(), ...scenarioSettings(settings) };
    return this.update((data) => {
      if (data.scenarios.length >= 20)
        throw new Error('Scenario library is full (20); existing entries are preserved');
      data.scenarios.unshift(item);
      return data;
    });
  }
  appendRun(record) {
    return this.update((data) => {
      data.runs = [record, ...data.runs].slice(0, 20);
      return data;
    });
  }
  renameScenario(id, name) {
    if (typeof id !== 'string' || !id) throw new Error('Choose a saved scenario');
    if (typeof name !== 'string' || !name.trim() || name.length > 80)
      throw new Error('Choose a scenario name (1–80 characters)');
    return this.update((data) => {
      const item = data.scenarios.find((scenario) => scenario.id === id);
      if (!item) throw new Error('Scenario no longer exists; nothing renamed');
      item.name = name.trim();
      return data;
    });
  }
  updateScenario(expected, settings) {
    if (typeof expected?.id !== 'string' || !expected.id)
      throw new Error('Choose a saved scenario');
    const id = expected.id,
      revision = JSON.stringify(expected),
      replacement = scenarioSettings(settings);
    return this.update((data) => {
      const item = data.scenarios.find((scenario) => scenario.id === id);
      if (!item) throw new Error('Scenario no longer exists; nothing updated');
      if (JSON.stringify(item) !== revision)
        throw new Error('Saved scenario changed. Esc, then Shift+S to review it again.');
      Object.assign(item, replacement);
      return data;
    });
  }
  removeEntry(kind, id) {
    if (!['scenarios', 'runs'].includes(kind)) throw new Error('Invalid library kind');
    return this.update((data) => {
      data[kind] = data[kind].filter((item) => item.id !== id);
      return data;
    });
  }
  restoreEntry(kind, item) {
    if (!['scenarios', 'runs'].includes(kind)) throw new Error('Invalid library kind');
    return this.update((data) => {
      if (data[kind].some((entry) => entry.id === item.id))
        throw new Error('Entry already restored');
      if (data[kind].length >= 20) throw new Error('Library is full; undo kept available');
      data[kind].unshift(item);
      return data;
    });
  }
  async exportRun(destination, record) {
    const target = path.resolve(destination);
    const content =
      JSON.stringify({ format: 'stateset-tui-run', version: 1, ...record }, null, 2) + '\n';
    const handle = await fs.open(target, 'wx', 0o600);
    try {
      await handle.writeFile(content);
      await handle.sync();
    } finally {
      await handle.close();
    }
    return target;
  }
}
