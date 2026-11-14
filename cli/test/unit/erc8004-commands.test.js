/**
 * `stateset erc8004 list` renders the identity rows it fetched.
 *
 * The identity helpers return snake_case rows; the table columns used
 * camelCase keys, so every Registry/Agent/URI/Wallet cell printed blank.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

import { execute } from '../../src/commands/erc8004.js';
import { registerIdentity } from '../../src/erc8004/index.js';
import { createOutput } from '../../src/output.js';

let tmpDir;
let dbPath;

describe('erc8004 list command', () => {
  beforeEach(() => {
    tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), 'erc8004-commands-'));
    dbPath = path.join(tmpDir, 'identities.db');
  });
  afterEach(() => fs.rmSync(tmpDir, { recursive: true, force: true }));

  it('fills every table column from the identity rows', async () => {
    registerIdentity(dbPath, {
      agentRegistry: 'eip155:8453:0xregistry',
      agentId: 'agent-7',
      agentUri: 'https://agents.example/7.json',
      agentWallet: '0xwallet7',
    });

    const result = await execute('list', [], {
      output: createOutput({ color: false }),
      jsonOutput: false,
      dbPath,
    });

    const dataLine = result.formatted.split('\n').at(-1);
    const cells = dataLine.split('│').map((cell) => cell.trim());
    assert.deepEqual(cells, [
      'eip155:8453:0xregistry',
      'agent-7',
      'https://agents.example/7.json',
      '0xwallet7',
      'true',
    ]);
  });
});
