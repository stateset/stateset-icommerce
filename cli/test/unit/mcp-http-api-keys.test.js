/**
 * Unit tests for src/mcp/http-api-keys.js — key collection, fingerprints and
 * the /mcp auth guard used by stateset-mcp-http.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import {
  API_KEYS_ENV,
  collectApiKeys,
  createApiKeyGuard,
  extractPresentedKey,
  keyFingerprint,
} from '../../src/mcp/http-api-keys.js';

const KEY_A = 'alpha-placeholder-key-not-a-secret'; // gitleaks:allow;
const KEY_B = 'bravo-placeholder-key-not-a-secret'; // gitleaks:allow;

function mockRes() {
  const res = { status: null, headers: null, body: '' };
  res.writeHead = (status, headers) => {
    res.status = status;
    res.headers = headers;
  };
  res.end = (body) => {
    res.body = body ?? '';
  };
  return res;
}

describe('keyFingerprint', () => {
  it('is the first 6 hex chars of sha256 and never contains the key', () => {
    const fp = keyFingerprint(KEY_A);
    assert.equal(fp, createHash('sha256').update(KEY_A).digest('hex').slice(0, 6));
    assert.equal(fp.length, 6);
    assert.ok(!KEY_A.includes(fp));
  });
});

describe('collectApiKeys', () => {
  it('names the env var', () => {
    assert.equal(API_KEYS_ENV, 'STATESET_MCP_API_KEYS');
  });

  it('returns nothing when no source is configured', () => {
    assert.deepEqual(collectApiKeys({}), []);
    assert.deepEqual(collectApiKeys({ flags: [], env: '' }), []);
  });

  it('merges flags, comma-separated env and a key file, de-duplicated in order', () => {
    const keys = collectApiKeys({
      flags: [KEY_A],
      env: ` ${KEY_B} , ${KEY_A} `,
      file: '/keys',
      readFile: () => `# ops keys\n\ncharlie-placeholder-key-not-a-secret\n${KEY_B}\n`,
    });
    assert.deepEqual(keys, [KEY_A, KEY_B, 'charlie-placeholder-key-not-a-secret']);
  });

  it('rejects keys shorter than 16 chars without echoing them', () => {
    assert.throws(
      () => collectApiKeys({ flags: ['short'] }),
      (error) => /too short/.test(error.message) && !error.message.includes('short)'),
    );
  });

  it('wraps an unreadable key file in a flag-named error', () => {
    assert.throws(
      () =>
        collectApiKeys({
          file: '/nope',
          readFile: () => {
            throw new Error('ENOENT');
          },
        }),
      /--api-key-file \/nope: ENOENT/,
    );
  });
});

describe('extractPresentedKey', () => {
  it('reads Bearer (case-insensitive scheme) then X-API-Key', () => {
    assert.equal(extractPresentedKey({ headers: { authorization: `Bearer ${KEY_A}` } }), KEY_A);
    assert.equal(extractPresentedKey({ headers: { authorization: `bearer ${KEY_A}` } }), KEY_A);
    assert.equal(extractPresentedKey({ headers: { 'x-api-key': KEY_B } }), KEY_B);
    assert.equal(
      extractPresentedKey({ headers: { authorization: `Bearer ${KEY_A}`, 'x-api-key': KEY_B } }),
      KEY_A,
    );
  });

  it('returns null for absent, empty or non-Bearer credentials', () => {
    assert.equal(extractPresentedKey({ headers: {} }), null);
    assert.equal(extractPresentedKey({ headers: { authorization: 'Basic abc' } }), null);
    assert.equal(extractPresentedKey({ headers: { authorization: 'Bearer ' } }), null);
    assert.equal(extractPresentedKey({ headers: { 'x-api-key': '  ' } }), null);
  });
});

describe('createApiKeyGuard', () => {
  it('is null when no keys are configured (auth off)', () => {
    assert.equal(createApiKeyGuard([]), null);
    assert.equal(createApiKeyGuard(undefined), null);
  });

  it('passes a valid Bearer or X-API-Key without touching the response', () => {
    const guard = createApiKeyGuard([KEY_A, KEY_B]);
    for (const headers of [
      { authorization: `Bearer ${KEY_A}` },
      { authorization: `Bearer ${KEY_B}` },
      { 'x-api-key': KEY_B },
    ]) {
      const res = mockRes();
      assert.equal(guard({ headers }, res), true);
      assert.equal(res.status, null);
    }
  });

  it('answers 401 with a JSON-RPC error and a Bearer challenge on a missing key', () => {
    const guard = createApiKeyGuard([KEY_A]);
    const res = mockRes();
    assert.equal(guard({ headers: {} }, res), false);
    assert.equal(res.status, 401);
    assert.match(res.headers['WWW-Authenticate'], /^Bearer /);
    const body = JSON.parse(res.body);
    assert.equal(body.jsonrpc, '2.0');
    assert.equal(body.id, null);
    assert.equal(body.error.code, -32001);
    assert.match(body.error.message, /missing API key/);
  });

  it('answers 401 on a wrong key, a prefix of a key, and a key with a suffix', () => {
    const guard = createApiKeyGuard([KEY_A]);
    for (const presented of ['x'.repeat(KEY_A.length), KEY_A.slice(0, -1), `${KEY_A}x`]) {
      const res = mockRes();
      assert.equal(guard({ headers: { authorization: `Bearer ${presented}` } }, res), false);
      assert.equal(res.status, 401);
      assert.match(JSON.parse(res.body).error.message, /invalid API key/);
    }
  });
});
