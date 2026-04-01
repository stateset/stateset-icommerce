import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdirSync, rmSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import {
  getCredentialStore,
  resetCredentialStore,
  resolveProviderApiKey,
} from '../src/credentials.js';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const TMP_DIR = join(__dirname, '.tmp-credentials-test');
const OPENAI_ENV_KEY = 'OPENAI_API_KEY';

function createTempDbPath(name) {
  mkdirSync(TMP_DIR, { recursive: true });
  return join(TMP_DIR, name);
}

afterEach(() => {
  delete process.env[OPENAI_ENV_KEY];
  resetCredentialStore();
  rmSync(TMP_DIR, { recursive: true, force: true });
});

describe('credentials fallback', () => {
  it('uses the credential store when available and falls back to env without warning noise', () => {
    process.env[OPENAI_ENV_KEY] = 'env-openai-key';
    const dbPath = createTempDbPath('credentials.db');
    const warnings = [];
    const originalWarn = console.warn;

    console.warn = (...args) => {
      warnings.push(args.map(String).join(' '));
    };

    try {
      const store = getCredentialStore({ dbPath });
      assert.ok(store);

      const first = resolveProviderApiKey('openai');
      const second = resolveProviderApiKey('openai');

      assert.equal(first, 'env-openai-key');
      assert.equal(second, 'env-openai-key');
      assert.equal(warnings.length, 0);
    } finally {
      console.warn = originalWarn;
    }
  });
});
