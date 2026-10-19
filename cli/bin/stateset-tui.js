#!/usr/bin/env node
import { parseArgs } from 'node:util';
import path from 'node:path';
import { createRequire } from 'node:module';
import { clean } from '../src/tui/text.js';

const HELP = `
StateSet iCommerce terminal workbench

Usage:

  stateset-tui                       Explore a private engine demo store
  stateset-tui --db ./store.db        Browse a consistent read-only snapshot
  stateset-tui --snapshot            Print one screen without a TTY
  stateset-tui --db ./store.db --json Print the engine's snapshot data

  --demo          Private demo store (default; cannot combine with --db)
  --db <path>     Existing SQLite store; never initialized or migrated in place
  --workspace <path>  Local scenario/history directory (default: XDG state/stateset/tui)
  --snapshot      Plain terminal screen, suitable for logs
  --json          Snapshot data as JSON; no terminal escape sequences
  --no-color      Disable colors in the interactive workbench
  --help, -h      Show help
  --version, -v   Show version

Keys: 1–6 / Tab views · arrows / j k select · Enter details · / search
      n / p pages · r refresh · : command palette · ? help · q quit
Lab:  e assumptions · p selected SKUs · f results · w save · l library · h history
      x export JSON · i import assumptions · Shift+U undo removal · c cancel run · Shift+R replay · + / - seed
Stock: a adds selected SKU to a private plan; cost/demand/supplier terms are explicit.
Browse: / search entire view on Enter · o sort · t related records · u customer · b back
All simulations use private native stores. History retains 20 runs per source.
Exports never overwrite files; workspace JSON contains sensitive planning data.

No AI credentials required. Browsing has no business-write actions.
Live browsing needs optional better-sqlite3 for a WAL-consistent backup;
the embedded engine reads and migrates only that temporary snapshot.
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
    await startTerminal({
      dbPath,
      workspace: new TerminalWorkspace(values.workspace, dbPath || 'demo'),
      color: !values['no-color'] && !process.env.NO_COLOR,
    });
  }
}
main().catch((error) => {
  console.error(`iCommerce: ${clean(error.message)}`);
  process.exitCode = 1;
});
