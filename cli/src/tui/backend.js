import { fork } from 'node:child_process';
import fs from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

export class TerminalBackend {
  constructor(dbPath = null) {
    this.dbPath = dbPath ? path.resolve(dbPath) : null;
    this.directory = fs.mkdtemp(path.join(os.tmpdir(), 'stateset-tui-'));
    this.children = new Set();
    this.operations = new Set();
    this.closed = false;
  }

  async request(operation, payload = {}) {
    const directory = await this.directory;
    if (this.closed) throw new Error('Terminal backend is closed');
    if (this.operations.has(operation)) throw new Error('This engine operation is already running');
    const child = fork(fileURLToPath(new URL('./worker.js', import.meta.url)), [], {
      env: { PATH: process.env.PATH, NODE_ENV: 'test', STATESET_SQLITE_SYNCHRONOUS: 'NORMAL' },
      execArgv: ['--max-old-space-size=128'],
      stdio: ['ignore', 'ignore', 'pipe', 'ipc'],
    });
    this.children.add(child);
    this.operations.add(operation);
    return new Promise((resolve, reject) => {
      let response,
        stderr = '';
      child.stderr.on('data', (chunk) => {
        stderr = (stderr + chunk).slice(-1500);
      });
      const timer = setTimeout(() => child.kill('SIGKILL'), 45000);
      child.once('message', (value) => {
        response = value;
      });
      child.once('error', (error) => {
        clearTimeout(timer);
        this.children.delete(child);
        this.operations.delete(operation);
        reject(error);
      });
      child.once('close', (code) => {
        clearTimeout(timer);
        this.children.delete(child);
        this.operations.delete(operation);
        if (response?.success && code === 0) resolve(response.data);
        else
          reject(
            new Error(
              response?.error ||
                (code === null
                  ? 'Engine request exceeded its time limit'
                  : stderr || 'Engine worker did not complete'),
            ),
          );
      });
      child.send({ operation, payload, directory, dbPath: this.dbPath }, (error) => {
        if (error) child.kill('SIGKILL');
      });
    });
  }

  async close() {
    if (this.closed) return;
    this.closed = true;
    await Promise.all(
      [...this.children].map(
        (child) =>
          new Promise((resolve) => {
            child.once('close', resolve);
            child.kill('SIGKILL');
          }),
      ),
    );
    // Only remove the private directory allocated by this instance.
    await fs.rm(await this.directory, { recursive: true, force: true });
  }
}
