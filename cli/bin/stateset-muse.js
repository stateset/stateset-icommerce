#!/usr/bin/env node
import { createServer } from 'node:http';
import { parseArgs } from 'node:util';
import { createEmbeddedAgentToolkit } from '../src/agent-toolkit.js';
import { loadKernelConfig } from '../src/kernel-config.js';
import { collectApiKeys } from '../src/mcp/http-api-keys.js';
import { createMuseConnector, MUSE_DEFAULT_TOOLS } from '../src/connectors/muse.js';
import { loadMuseAccounts } from '../src/connectors/muse-accounts.js';
import { runMain } from '../src/graceful-shutdown.js';

let server;
let toolkit;
let connector;
runMain(
  'stateset-muse',
  async () => {
    const { values } = parseArgs({
      options: {
        host: { type: 'string', default: '127.0.0.1' },
        port: { type: 'string', default: '8092' },
        db: { type: 'string', default: './store.db' },
        'public-url': { type: 'string' },
        'allowed-host': { type: 'string', multiple: true },
        'api-key-file': { type: 'string' },
        'accounts-file': { type: 'string' },
        tools: { type: 'string' },
        'requests-per-minute': { type: 'string', default: '60' },
        'public-requests-per-minute': { type: 'string', default: '120' },
        'max-in-flight': { type: 'string', default: '16' },
        'max-account-in-flight': { type: 'string', default: '4' },
        'max-body-bytes': { type: 'string', default: '65536' },
        'quiet-audit': { type: 'boolean', default: false },
        apply: { type: 'boolean', default: false },
        'kernel-policy': { type: 'string' },
        'kernel-principal': { type: 'string' },
        'kernel-store-id': { type: 'string' },
        help: { type: 'boolean', short: 'h', default: false },
      },
    });
    if (values.help) {
      console.log(`StateSet connector service for the Muse personal assistant

stateset-muse --db ./store.db --api-key-file ./muse-keys.txt

  --host <host>           Bind address (default 127.0.0.1)
  --port <port>           Port (default 8092)
  --public-url <origin>   Public HTTPS origin; local testing defaults to loopback
  --allowed-host <host>  Additional reverse-proxy Host hostname (repeatable)
  --api-key-file <path>  Keys, one per line; or STATESET_MUSE_API_KEYS (comma-separated)
  --accounts-file <path> Named accounts with key files, tool scopes and write permissions
  --tools <a,b>          Exact allowed tool names; default curated commerce tools
  --requests-per-minute <n> Per-account request budget (default 60)
  --public-requests-per-minute <n> Shared public/invalid-key budget (default 120)
  --max-in-flight <n>    Concurrent tool requests per service (default 16)
  --max-account-in-flight <n> Concurrent tool requests per account (default 4)
  --max-body-bytes <n>   Maximum tool request body (default 65536)
  --quiet-audit          Disable structured request telemetry on stderr
  --apply                Enable writes with operator-owned kernel files
  --kernel-policy <path> Trusted policy JSON
  --kernel-principal <path> Trusted principal JSON
  --kernel-store-id <id> Store scope

GET /openapi.json describes POST /v1/tools/<name>; Bearer authentication required.
Writes preview by default. Public Muse directory availability requires Meta review.`);
      return;
    }
    const port = Number(values.port);
    if (!Number.isSafeInteger(port) || port < 1 || port > 65535) throw new Error('Invalid port.');
    const requestControls = {};
    for (const [flag, name] of Object.entries({
      'requests-per-minute': 'requestsPerMinute',
      'public-requests-per-minute': 'publicRequestsPerMinute',
      'max-in-flight': 'maxInFlight',
      'max-account-in-flight': 'maxAccountInFlight',
      'max-body-bytes': 'maxBodyBytes',
    })) {
      const value = Number(values[flag]);
      if (!Number.isSafeInteger(value) || value < 1)
        throw new Error(`--${flag} must be a positive integer.`);
      requestControls[name] = value;
    }
    const loopback = ['127.0.0.1', 'localhost', '::1'].includes(values.host);
    if (!loopback && !values['public-url'])
      throw new Error('Non-loopback binds require --public-url.');
    if (values['accounts-file'] && (values['api-key-file'] || process.env.STATESET_MUSE_API_KEYS)) {
      throw new Error('--accounts-file cannot be combined with shared API key sources.');
    }
    const accounts = values['accounts-file']
      ? loadMuseAccounts(values['accounts-file'])
      : undefined;
    const apiKeys = accounts
      ? undefined
      : collectApiKeys({
          file: values['api-key-file'],
          env: process.env.STATESET_MUSE_API_KEYS || '',
        });
    if (!accounts && !apiKeys.length)
      throw new Error('Configure --accounts-file, --api-key-file or STATESET_MUSE_API_KEYS.');
    const tools = values.tools
      ? values.tools.split(',').map((name) => name.trim())
      : [...MUSE_DEFAULT_TOOLS];
    const kernel = loadKernelConfig({
      policyPath: values['kernel-policy'],
      principalPath: values['kernel-principal'],
      storeId: values['kernel-store-id'],
      requireForApply: values.apply,
    });
    toolkit = createEmbeddedAgentToolkit({
      dbPath: values.db,
      capabilities: tools,
      allowApply: values.apply,
      kernel,
    });
    connector = createMuseConnector({
      toolkit,
      apiKeys,
      accounts,
      tools,
      allowApply: values.apply,
      publicUrl: values['public-url'] || `http://127.0.0.1:${port}`,
      allowedHosts: values['allowed-host'] || [],
      ...requestControls,
      onAudit: values['quiet-audit']
        ? undefined
        : (event) =>
            process.stderr.write(`${JSON.stringify({ type: 'muse_request', ...event })}\n`),
    });
    server = createServer(connector.handler);
    server.requestTimeout = 30000;
    server.headersTimeout = 10000;
    await new Promise((resolve, reject) => {
      server.once('error', reject);
      server.listen(port, values.host, resolve);
    });
    console.log(
      `StateSet Muse connector listening on ${values.host}:${port}; ${tools.length} tools; writes ${values.apply ? 'governed' : 'preview-only'}.`,
    );
  },
  {
    cleanup: async () => {
      if (server?.listening) await new Promise((resolve) => server.close(resolve));
      connector?.close();
      toolkit?.close();
    },
  },
);
