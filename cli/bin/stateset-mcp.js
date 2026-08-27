#!/usr/bin/env node
/**
 * StateSet Commerce MCP Server (stdio)
 *
 * The canonical entrypoint for MCP-native clients — Claude Desktop, Cursor,
 * Windsurf, Smithery, and anything else that launches stdio MCP servers.
 * Serves the full commerce tool surface over stdio and nothing else; for the
 * variant with an HTTP event-stream sidecar, see `stateset-mcp-events`.
 *
 * The docs and the Smithery listing have referenced this command for several
 * releases; until now the binary did not exist (only the events gateway did).
 *
 * Usage:
 *   stateset-mcp [--db ./store.db] [--apply] [--structured-tool-results]
 *
 * Writes are disabled unless --apply is passed, matching the CLI-wide
 * permission model: tools preview what they would do and return an
 * "requires --apply" hint instead of mutating.
 */
import { parseArgs } from 'node:util';
import { runMain } from '../src/graceful-shutdown.js';
import { CLI_VERSION } from '../src/config.js';
import { loadKernelConfig } from '../src/kernel-config.js';

const HELP = `
StateSet Commerce MCP Server (stdio)  v${CLI_VERSION}

USAGE:
  stateset-mcp [options]

OPTIONS:
  --db <path>                SQLite database path (default: ./store.db, env DB_PATH)
  --apply                    Enable write tools (default: preview-only)
  --structured-tool-results  Emit structured content blocks in tool results
  --profile <name>           Tool profile: core, operations, finance, agents, all (default: all)
  --domains <a,b,...>        Add specific tool domains to the selected profile
  --strict-protocol          Serve ONLY 2026-07-28; reject 2025-era clients
  --kernel-policy <path>     Trusted kernel policy JSON (env STATESET_KERNEL_POLICY)
  --kernel-principal <path>  Trusted principal JSON (env STATESET_KERNEL_PRINCIPAL)
  --kernel-store-id <id>     Logical store scope (env STATESET_KERNEL_STORE_ID)
  --kernel-allow-legacy-writes
                             Expose writes without typed kernel commands (unsafe migration only)
  -h, --help                 Show this help

EXAMPLES:
  # Claude Desktop / Cursor config:
  # { "mcpServers": { "stateset-commerce": {
  #     "command": "npx",
  #     "args": ["-y", "-p", "@stateset/cli", "stateset-mcp", "--db", "./store.db"] } } }
`;

async function main() {
  const { values } = parseArgs({
    options: {
      db: { type: 'string' },
      apply: { type: 'boolean', default: false },
      'structured-tool-results': { type: 'boolean', default: false },
      profile: { type: 'string', default: 'all' },
      domains: { type: 'string' },
      'strict-protocol': { type: 'boolean', default: false },
      'kernel-policy': { type: 'string' },
      'kernel-principal': { type: 'string' },
      'kernel-store-id': { type: 'string' },
      'kernel-allow-legacy-writes': { type: 'boolean', default: false },
      help: { type: 'boolean', short: 'h', default: false },
    },
  });

  if (values.help) {
    console.error(HELP);
    process.exit(0);
  }

  const [{ Commerce }, { serveStdio }, { createStatesetMcpServer }, { createStatesetV2McpServer }] =
    await Promise.all([
      import('@stateset/embedded'),
      import('@modelcontextprotocol/server/stdio'),
      import('../src/mcp-server.js'),
      import('../src/mcp/v2-server.js'),
    ]);

  const dbPath = values.db || process.env.DB_PATH || './store.db';
  const kernel = loadKernelConfig({
    policyPath: values['kernel-policy'],
    principalPath: values['kernel-principal'],
    storeId: values['kernel-store-id'],
    allowLegacyWrites: values['kernel-allow-legacy-writes'],
    requireForApply: values.apply,
  });

  let commerce;
  try {
    commerce = new Commerce(dbPath);
  } catch (error) {
    console.error(`[stateset-mcp] database init error: ${error.message}`);
    process.exit(1);
  }

  // `serveStdio` owns the era decision: the opening exchange selects it, one
  // instance from this factory is pinned for the connection, and everything
  // after passes straight through. 2025-era clients are served from the same
  // factory, so both revisions expose an identical tool surface.
  serveStdio(
    () =>
      createStatesetV2McpServer({
        createServer: createStatesetMcpServer,
        commerce,
        dbPath,
        allowApply: values.apply,
        structuredToolResults: values['structured-tool-results'],
        toolProfile: values.profile,
        toolDomains: values.domains
          ? values.domains
              .split(',')
              .map((value) => value.trim())
              .filter(Boolean)
          : [],
        kernel,
      }),
    {
      legacy: values['strict-protocol'] ? 'reject' : 'serve',
      onerror: (error) => console.error(`[stateset-mcp] ${error.message}`),
    },
  );

  // stderr only: stdout belongs to the MCP protocol.
  console.error(
    `[stateset-mcp] serving commerce tools over stdio (protocol 2026-07-28, db: ${dbPath}, writes: ${
      values.apply ? 'ENABLED' : 'preview-only, pass --apply to enable'
    }, profile: ${values.profile}, kernel: ${kernel ? (kernel.strict ? 'strict' : 'legacy-write escape hatch') : 'not configured'})`,
  );
}

runMain('stateset-mcp', main);
