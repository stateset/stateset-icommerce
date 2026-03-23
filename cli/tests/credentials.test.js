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
  it('falls back to env and warns at most once when the native store is unavailable', () => {
    process.env[OPENAI_ENV_KEY] = 'env-openai-key';
    const dbPath = createTempDbPath('credentials.db');
    const warnings = [];
    const originalWarn = console.warn;
    let storeAvailable = true;

    console.warn = (...args) => {
      warnings.push(args.map(String).join(' '));
    };

    try {
      try {
        getCredentialStore({ dbPath });
      } catch {
        storeAvailable = false;
      }

      const first = resolveProviderApiKey('openai');
      const second = resolveProviderApiKey('openai');

      assert.equal(first, 'env-openai-key');
      assert.equal(second, 'env-openai-key');

      if (storeAvailable) {
        assert.equal(warnings.length, 0);
      } else {
        assert.equal(warnings.length, 1);
        assert.match(
          warnings[0],
          /Credential store unavailable, falling back to environment variables:/,
        );
      }
    } finally {
      console.warn = originalWarn;
    }
  });
});
