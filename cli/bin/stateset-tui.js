#!/usr/bin/env node
import { parseArgs } from 'node:util';
import path from 'node:path';
import { createRequire } from 'node:module';
import { clean } from '../src/tui/text.js';
import { loadKernelConfig } from '../src/kernel-config.js';

const HELP = `
StateSet iCommerce terminal workbench

Usage:

  stateset-tui                       Explore a private engine demo store
  stateset-tui --db ./store.db        Browse a consistent read-only snapshot
  stateset-tui --snapshot            Print one screen without a TTY
  stateset-tui --db ./store.db --json Print the engine's snapshot data

  --demo          Private demo store (default; cannot combine with --db)
  --db <path>     Existing SQLite store; browsing uses a private snapshot
  --workspace <path>  Local scenario/history directory (default: XDG state/stateset/tui)
  --snapshot      Plain terminal screen, suitable for logs
  --json          Snapshot data as JSON; no terminal escape sequences
  --no-color      Disable colors in the interactive workbench
  --apply         Enable reviewed order actions on --db (trusted kernel files required)
  --kernel-policy <path>     Operator-owned kernel policy JSON
  --kernel-principal <path>  Operator-owned principal JSON
  --kernel-store-id <id>     Trusted store scope for governed actions
  --help, -h      Show help
  --version, -v   Show version

Keys: 1–7 / Tab views · arrows / j k select · Enter details · / search
      n / p pages · r refresh · : command palette · ? help · q quit
Lab:  e assumptions · g set seed · p selected SKUs · f results · w save · l library · h history
      x export JSON · i import assumptions · Shift+U undo removal · c cancel run · Shift+R replay · + / - seed
Stock: a adds selected SKU to a private plan; cost/demand/supplier terms are explicit.
Browse: / search entire view on Enter · o sort · t related records · u customer · b back
Orders: W reviewed order status workflow · P reconcile last order action
Explorer: 7 all domains · T tool reference · t linked records · b back · d domains
Editing: Left/Right/Home/End · Backspace/Delete · Ctrl+U clear · Ctrl+K to end
All simulations use private native stores. History retains 20 runs per source.
Exports never overwrite files; workspace data can contain sensitive planning and operation receipts.

No AI credentials required. Browsing stays read-only. Order actions preview first;
--apply, trusted kernel configuration and typed confirmation enable a live change.
Live browsing needs optional better-sqlite3 for a WAL-consistent backup;
during browsing, the embedded engine reads and migrates only that temporary snapshot.
Explicit apply opens the actual store and may run native migrations.
`;

async function main() {
  const { values } = parseArgs({
    options: {
      db: { type: 'string' },
      workspace: { type: 'string' },
      demo: { type: 'boolean' },
      snapshot: { type: 'boolean' },
      json: { type: 'boolean' },
      'no-color': { type: 'boolean' },
      apply: { type: 'boolean' },
      'kernel-policy': { type: 'string' },
      'kernel-principal': { type: 'string' },
      'kernel-store-id': { type: 'string' },
      help: { type: 'boolean', short: 'h' },
      version: { type: 'boolean', short: 'v' },
    },
  });
  if (values.help) {
    console.log(HELP);
    return;
  }
  if (values.version) {
    const require = createRequire(import.meta.url);
    console.log(`stateset-tui v${require('../package.json').version}`);
    return;
  }
  if (values.db !== undefined && !values.db.trim())
    throw new Error('--db must name an existing SQLite file');
  if (values.db && values.demo) throw new Error('Choose --demo or --db, not both');
  if (values.db === ':memory:')
    throw new Error('Use --demo for a private store; --db must name an existing SQLite file');
  const dbPath = values.db ? path.resolve(values.db) : null;
  if (values.apply && (!dbPath || values.snapshot || values.json))
    throw new Error(
      '--apply requires an interactive --db session. Demo and snapshot modes remain read-only.',
    );
  const kernelOptions = {
    policyPath: values['kernel-policy'] || process.env.STATESET_KERNEL_POLICY,
    principalPath: values['kernel-principal'] || process.env.STATESET_KERNEL_PRINCIPAL,
    storeId: values['kernel-store-id'] || process.env.STATESET_KERNEL_STORE_ID,
    requireForApply: values.apply,
    env: {},
  };
  loadKernelConfig(kernelOptions);
  if (
    !values.snapshot &&
    !values.json &&
    (!process.stdin.isTTY || !process.stdout.isTTY || process.env.TERM === 'dumb')
  )
    throw new Error(
      'Interactive mode needs a terminal. Use --snapshot or --json for non-interactive output.',
    );
  if (values.snapshot || values.json) {
    const { TerminalBackend } = await import('../src/tui/backend.js');
    const backend = new TerminalBackend(dbPath);
    try {
      const data = await backend.request('snapshot');
      if (values.json) console.log(JSON.stringify(data, null, 2));
      else {
        const { initialState } = await import('../src/tui/model.js');
        const { render } = await import('../src/tui/render.js');
        console.log(
          render(
            {
              ...initialState(dbPath ? `Snapshot · ${dbPath}` : 'DEMO · private engine store'),
              data,
            },
            process.stdout.columns || 100,
            process.stdout.rows || 28,
            { color: false },
          ),
        );
      }
      if (data.errors.length) process.exitCode = 1;
    } finally {
      await backend.close();
    }
  } else {
    const { startTerminal } = await import('../src/tui/app.js');
    const { TerminalWorkspace } = await import('../src/tui/workspace.js');
    const { TerminalBackend } = await import('../src/tui/backend.js');
    const workspace = new TerminalWorkspace(values.workspace, dbPath || 'demo');
    await startTerminal({
      dbPath,
      workspace,
      backend: new TerminalBackend(dbPath, {
        allowApply: values.apply,
        loadKernel: () => loadKernelConfig(kernelOptions),
        operationsDirectory: workspace.directory,
      }),
      allowApply: values.apply === true,
      color: !values['no-color'] && !process.env.NO_COLOR,
    });
  }
}
main().catch((error) => {
  console.error(`iCommerce: ${clean(error.message)}`);
  process.exitCode = 1;
});
