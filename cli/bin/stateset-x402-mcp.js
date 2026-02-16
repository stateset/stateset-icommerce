#!/usr/bin/env node

/**
 * StateSet x402 MCP Server
 *
 * Runs an MCP server over stdio for paid API calls using x402.
 */

import { parseArgs } from 'node:util';
import process from 'node:process';
import { StdioServerTransport } from '@modelcontextprotocol/sdk/server/stdio.js';
import { createX402McpServer } from '../src/x402-mcp-server.js';
import { CLI_VERSION } from '../src/config.js';

const HELP = `
StateSet x402 MCP Server

USAGE:
  stateset-x402-mcp [options]

  OPTIONS:
  --config-dir <path>   Config directory for keys (default: .stateset)
  --policy-dir <path>   Policy store directory (default: STATESET_POLICY_DIR/.stateset)
  --help, -h            Show this help
  --version, -v         Show version

REQUIRED ENV:
  X402_SEQUENCER_URL
  X402_TENANT_ID
  X402_STORE_ID
  X402_AGENT_ID
  X402_PAYER_ADDRESS

OPTIONAL ENV:
  X402_CONFIG_FILE
  X402_SIGNING_KEY_PATH
  X402_SIGNING_KEY
  X402_AGENT_KEY_ID
  X402_PREFERRED_NETWORKS
  X402_REQUIRE_RECEIPT
  X402_RECEIPT_TIMEOUT_MS
  X402_RECEIPT_POLL_MS
  X402_MAX_AMOUNT
  X402_BUDGET_PER_CALL
  X402_BUDGET_DAILY
  X402_STARTING_BALANCE
  X402_BUDGET_STATE_FILE
  X402_API_KEY
  X402_JWT
`;

async function main() {
  const { values } = parseArgs({
    options: {
      'config-dir': { type: 'string' },
      'policy-dir': { type: 'string' },
      help: { type: 'boolean', short: 'h', default: false },
      version: { type: 'boolean', short: 'v', default: false },
    },
    allowPositionals: true,
  });

  if (values.help) {
    console.log(HELP.trim());
    return;
  }

  if (values.version) {
    console.log(`stateset-x402-mcp v${CLI_VERSION}`);
    return;
  }

  const configDir = values['config-dir'] || process.env.STATESET_CONFIG_DIR || '.stateset';
  const policyDir = values['policy-dir'] || process.env.STATESET_POLICY_DIR || configDir;
  const server = createX402McpServer({
    env: process.env,
    configDir,
    policyStorePath: policyDir,
  });
  const transport = new StdioServerTransport();
  const instance = server?.instance || server?.server || server;

  const shutdown = async () => {
    try {
      if (instance?.close) {
        await instance.close();
      }
    } catch (error) {
      console.error(
        `[x402-mcp] shutdown error: ${error instanceof Error ? error.message : String(error)}`,
      );
    } finally {
      process.exit(0);
    }
  };

  process.on('SIGINT', shutdown);
  process.on('SIGTERM', shutdown);

  try {
    if (!instance?.connect) {
      throw new Error('MCP server instance is missing connect()');
    }
    await instance.connect(transport);
  } catch (error) {
    console.error(
      `[x402-mcp] failed to start: ${error instanceof Error ? error.message : String(error)}`,
    );
    process.exit(1);
  }
}

import { runMain } from '../src/graceful-shutdown.js';
runMain('stateset-x402-mcp', main);
