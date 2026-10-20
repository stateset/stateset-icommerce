import { fork } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { OrderOperations } from './order-operations.js';

export class TerminalBackend {
  constructor(dbPath = null, operator = {}) {
    this.dbPath = dbPath ? path.resolve(dbPath) : null;
    this.directory = fs.mkdtemp(path.join(os.tmpdir(), 'stateset-tui-'));
    this.operations = new Map();
    this.closed = false;
    this.orderOperations = new OrderOperations({
      dbPath: this.dbPath,
      allowApply: operator.allowApply,
      loadKernel: operator.loadKernel,
      directory: operator.operationsDirectory,
      invoke: (operation, payload, kernel) => this.#request(operation, payload, () => {}, kernel),
    });
  }
  async request(operation, payload = {}, onProgress = () => {}) {
    if (this.closed) throw new Error('Terminal backend is closed');
    const action = new Map([
      ['preview-order', 'preview'],
      ['apply-order', 'apply'],
      ['reconcile-order', 'reconcile'],
    ]).get(operation);
    if (action) return this.orderOperations.run(action, payload);
    if (!['snapshot', 'simulate'].includes(operation))
      throw new Error('Unknown terminal operation');
    return this.#request(operation, payload, onProgress);
  }
  async #request(operation, payload, onProgress, kernel = null) {
    if (this.closed) throw new Error('Terminal backend is closed');
    if (this.operations.has(operation)) throw new Error('This engine operation is already running');
    let finished;
    const context = {
      child: null,
      canceled: false,
      done: new Promise((resolve) => {
        finished = resolve;
      }),
    };
    this.operations.set(operation, context);
    let directory, operationDirectory;
    try {
      directory = await this.directory;
      operationDirectory = ['simulate', 'order-preview'].includes(operation)
        ? await fs.mkdtemp(path.join(directory, 'run-'))
        : directory;
      if (context.canceled || this.closed) throw new Error('Simulation canceled');
      const child = fork(fileURLToPath(new URL('./worker.js', import.meta.url)), [], {
        // NORMAL is a private-store optimization only. A live apply uses
        // FULL so each live commit synchronizes its WAL.
        env: {
          PATH: process.env.PATH,
          ...(operation === 'order-apply'
            ? { NODE_ENV: 'production', STATESET_SQLITE_SYNCHRONOUS: 'FULL' }
            : { NODE_ENV: 'test', STATESET_SQLITE_SYNCHRONOUS: 'NORMAL' }),
        },
        execArgv: ['--max-old-space-size=128'],
        stdio: ['ignore', 'ignore', 'pipe', 'ipc'],
      });
      context.child = child;
      return await new Promise((resolve, reject) => {
        let response,
          failure,
          stderr = '';
        child.stderr.on('data', (chunk) => {
          stderr = (stderr + chunk).slice(-1500);
        });
        const timer = setTimeout(() => {
          failure = new Error('Engine request exceeded its time limit');
          child.kill('SIGKILL');
        }, 45000);
        child.on('message', (value) => {
          if (value?.type === 'progress') onProgress(String(value.message).slice(0, 200));
          else response = value;
        });
        child.once('error', (error) => {
          failure = error;
        });
        child.once('close', (code) => {
          clearTimeout(timer);
          if (context.canceled) reject(new Error('Simulation canceled; previous result retained'));
          else if (failure) reject(failure);
          else if (response?.success && code === 0) resolve(response.data);
          else reject(new Error(response?.error || stderr || 'Engine worker did not complete'));
        });
        child.send(
          { operation, payload, directory: operationDirectory, dbPath: this.dbPath, kernel },
          (error) => {
            if (error) {
              failure = error;
              child.kill('SIGKILL');
            }
          },
        );
      });
    } finally {
      try {
        if (operationDirectory && operationDirectory !== directory)
          await fs.rm(operationDirectory, { recursive: true, force: true });
      } finally {
        this.operations.delete(operation);
        finished();
      }
    }
  }
  cancel(operation) {
    if (operation === 'order-apply') return false;
    const context = this.operations.get(operation);
    if (!context || context.canceled) return false;
    context.canceled = true;
    context.child?.kill('SIGKILL');
    return true;
  }
  async close() {
    if (this.closed) return;
    this.closed = true;
    const active = [...this.operations.values()];
    for (const operation of this.operations.keys()) this.cancel(operation);
    await Promise.all(active.map((context) => context.done));
    await this.orderOperations.settle();
    await fs.rm(await this.directory, { recursive: true, force: true });
  }
}
