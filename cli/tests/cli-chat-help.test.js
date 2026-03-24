import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { runNodeScript } from './helpers/run-node-script.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const CHAT_BIN = join(__dirname, '..', 'bin', 'stateset-chat.js');

describe('stateset-chat help', () => {
  it('documents stats and prompt diagnostics', () => {
    const result = runNodeScript(CHAT_BIN, ['--help']);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /--stats\s+Show prompt budget and execution stats/);
    assert.match(result.stdout, /\/stats\s+Toggle live prompt budget and execution stats/);
    assert.match(result.stdout, /\/prompt\s+Show the latest prompt budget report/);
    assert.match(result.stdout, /\/refreshes\s+Show session refresh history/);
  });
});
