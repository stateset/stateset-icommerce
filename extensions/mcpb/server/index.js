#!/usr/bin/env node
// Claude Desktop extension entry point: launches the StateSet MCP server
// from the published npm package so the extension stays small. The db path
// and tool profile come from the extension's user config via env vars.
'use strict';

const { spawn } = require('node:child_process');
const os = require('node:os');
const path = require('node:path');

const dbPath = process.env.STATESET_DB_PATH || path.join(os.homedir(), 'stateset-store.db');
const profile = process.env.STATESET_TOOL_PROFILE || 'core';

const child = spawn(
  process.platform === 'win32' ? 'npx.cmd' : 'npx',
  ['-y', '-p', '@stateset/cli@1.31.0', 'stateset-mcp', '--db', dbPath, '--profile', profile],
  { stdio: 'inherit' },
);

child.on('exit', (code, signal) => {
  process.exit(signal ? 1 : code ?? 0);
});
child.on('error', (error) => {
  console.error(`[stateset-icommerce extension] failed to launch: ${error.message}`);
  process.exit(1);
});
