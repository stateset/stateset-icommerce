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

const HELP = `
StateSet Commerce MCP Server (stdio)  v${CLI_VERSION}

USAGE:
  stateset-mcp [options]

OPTIONS:
  --db <path>                SQLite database path (default: ./store.db, env DB_PATH)
  --apply                    Enable write tools (default: preview-only)
  --structured-tool-results  Emit structured content blocks in tool results
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
      help: { type: 'boolean', short: 'h', default: false },
    },
  });

  if (values.help) {
    console.error(HELP);
    process.exit(0);
  }

  const [{ Commerce }, { StdioServerTransport }, { createStatesetMcpServer }] = await Promise.all([
    import('@stateset/embedded'),
    import('@modelcontextprotocol/sdk/server/stdio.js'),
    import('../src/mcp-server.js'),
  ]);

  const dbPath = values.db || process.env.DB_PATH || './store.db';

  let commerce;
  try {
    commerce = new Commerce(dbPath);
  } catch (error) {
    console.error(`[stateset-mcp] database init error: ${error.message}`);
    process.exit(1);
  }

  const mcpServer = createStatesetMcpServer({
    commerce,
    dbPath,
    allowApply: values.apply,
    structuredToolResults: values['structured-tool-results'],
  });
  const mcpInstance = mcpServer?.instance || mcpServer?.server || mcpServer;
  if (!mcpInstance || typeof mcpInstance.connect !== 'function') {
    throw new Error('Failed to initialize MCP server instance');
  }

  const transport = new StdioServerTransport();
  await mcpInstance.connect(transport);
  // stderr only: stdout belongs to the MCP protocol.
  console.error(
    `[stateset-mcp] serving commerce tools over stdio (db: ${dbPath}, writes: ${
      values.apply ? 'ENABLED' : 'preview-only, pass --apply to enable'
    })`,
  );
}

runMain('stateset-mcp', main);
