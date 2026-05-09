// Unit tests for the replay-log sanitization helpers extracted from
// cli/src/mcp-server.js. Covers redaction, depth/breadth caps, cycle detection,
// stable serialization, and the SHA-256 hash helper.

import { strict as assert } from 'node:assert';
import { describe, it } from 'node:test';

import {
  MAX_REPLAY_ARRAY_ITEMS,
  MAX_REPLAY_OBJECT_KEYS,
  MAX_REPLAY_STRING_CHARS,
  REDACT_REPLAY_KEYS,
  compactReplayValue,
  sanitizeReplayValue,
  sha256,
  stableStringify,
} from '../../src/mcp/replay-sanitizer.js';

describe('replay-sanitizer · stableStringify', () => {
  it('produces identical output for objects with reordered keys', () => {
    const a = { x: 1, y: 2, z: 3 };
    const b = { z: 3, y: 2, x: 1 };
    assert.equal(stableStringify(a), stableStringify(b));
  });

  it('sorts keys at every nesting level', () => {
    const v = { b: { z: 1, a: 2 }, a: 1 };
    assert.equal(stableStringify(v), '{"a":1,"b":{"a":2,"z":1}}');
  });

  it('preserves array order', () => {
    assert.equal(stableStringify([3, 1, 2]), '[3,1,2]');
  });

  it('handles null and undefined', () => {
    assert.equal(stableStringify(null), 'null');
    assert.equal(stableStringify(undefined), undefined);
  });
});

describe('replay-sanitizer · sha256', () => {
  it('returns a 64-char lowercase hex digest', () => {
    const digest = sha256('hello');
    assert.match(digest, /^[a-f0-9]{64}$/);
  });

  it('is deterministic', () => {
    assert.equal(sha256('payload'), sha256('payload'));
  });

  it('produces different digests for different inputs', () => {
    assert.notEqual(sha256('a'), sha256('b'));
  });
});

describe('replay-sanitizer · sanitizeReplayValue', () => {
  it('redacts keys in REDACT_REPLAY_KEYS', () => {
    const out = sanitizeReplayValue({
      api_key: 'sk-...',
      apiKey: 'sk-...',
      password: 'secret-pw',
      token: 'jwt',
      private_key: 'pk',
      regular: 'visible',
    });
    assert.equal(out.api_key, '[REDACTED]');
    assert.equal(out.apiKey, '[REDACTED]');
    assert.equal(out.password, '[REDACTED]');
    assert.equal(out.token, '[REDACTED]');
    assert.equal(out.private_key, '[REDACTED]');
    assert.equal(out.regular, 'visible');
  });

  it('redacts any key containing the substring "secret"', () => {
    const out = sanitizeReplayValue({
      myCustomSecret: 'leaked',
      SECRETSAUCE: 'leaked',
    });
    assert.equal(out.myCustomSecret, '[REDACTED]');
    assert.equal(out.SECRETSAUCE, '[REDACTED]');
  });

  it('truncates long strings with "..." marker', () => {
    const long = 'x'.repeat(MAX_REPLAY_STRING_CHARS + 10);
    const out = sanitizeReplayValue(long);
    assert.equal(typeof out, 'string');
    assert.equal(out.length, MAX_REPLAY_STRING_CHARS + 3); // + "..."
    assert.ok(out.endsWith('...'));
  });

  it('passes short strings through unchanged', () => {
    assert.equal(sanitizeReplayValue('short'), 'short');
  });

  it('passes numbers and booleans through', () => {
    assert.equal(sanitizeReplayValue(42), 42);
    assert.equal(sanitizeReplayValue(true), true);
    assert.equal(sanitizeReplayValue(false), false);
  });

  it('summarizes Buffer with length', () => {
    const buf = Buffer.from('hello world');
    assert.equal(sanitizeReplayValue(buf), `<Buffer ${buf.length}>`);
  });

  it('serializes Date as ISO string', () => {
    const d = new Date('2026-01-01T00:00:00.000Z');
    assert.equal(sanitizeReplayValue(d), '2026-01-01T00:00:00.000Z');
  });

  it('summarizes Map with size + entries', () => {
    const m = new Map([
      ['a', 1],
      ['b', 2],
    ]);
    const out = sanitizeReplayValue(m);
    assert.equal(out._type, 'Map');
    assert.equal(out.size, 2);
    assert.equal(out.entries.length, 2);
  });

  it('summarizes Set with size + values', () => {
    const s = new Set(['a', 'b', 'c']);
    const out = sanitizeReplayValue(s);
    assert.equal(out._type, 'Set');
    assert.equal(out.size, 3);
    assert.equal(out.values.length, 3);
  });

  it('summarizes BigInt as a "n"-suffixed string', () => {
    assert.equal(sanitizeReplayValue(123n), '123n');
  });

  it('caps object keys at MAX_REPLAY_OBJECT_KEYS', () => {
    const big = {};
    for (let i = 0; i < MAX_REPLAY_OBJECT_KEYS + 5; i++) {
      big[`k${i}`] = i;
    }
    const out = sanitizeReplayValue(big);
    // __truncatedKeys is the synthetic marker
    assert.equal(out.__truncatedKeys, 5);
  });

  it('detects cycles and replaces with "[truncated]"', () => {
    const obj = { name: 'self' };
    obj.self = obj;
    const out = sanitizeReplayValue(obj);
    assert.equal(out.name, 'self');
    assert.equal(out.self, '[truncated]');
  });

  it('truncates beyond max depth', () => {
    let nested = { v: 'leaf' };
    for (let i = 0; i < 20; i++) nested = { v: nested };
    const out = sanitizeReplayValue(nested);
    // At max depth (4), further nesting becomes "[truncated]"
    let cur = out;
    let levels = 0;
    while (cur && typeof cur === 'object' && cur.v !== undefined && cur.v !== '[truncated]') {
      cur = cur.v;
      levels += 1;
      if (levels > 10) break;
    }
    assert.ok(levels < 10, 'depth recursion must terminate');
  });

  it('handles null and undefined identity', () => {
    assert.equal(sanitizeReplayValue(null), null);
    assert.equal(sanitizeReplayValue(undefined), undefined);
  });
});

describe('replay-sanitizer · compactReplayValue', () => {
  it('truncates long arrays at MAX_REPLAY_ARRAY_ITEMS with overflow marker', () => {
    const arr = Array.from({ length: MAX_REPLAY_ARRAY_ITEMS + 10 }, (_, i) => i);
    const out = compactReplayValue(arr);
    // First MAX_REPLAY_ARRAY_ITEMS values + 1 overflow marker
    assert.equal(out.length, MAX_REPLAY_ARRAY_ITEMS + 1);
    assert.equal(out[MAX_REPLAY_ARRAY_ITEMS], '[+10 more items]');
  });

  it('passes short arrays through unchanged', () => {
    assert.deepEqual(compactReplayValue([1, 2, 3]), [1, 2, 3]);
  });

  it('detects cycles in arrays', () => {
    const arr = [1, 2];
    arr.push(arr);
    const out = compactReplayValue(arr);
    assert.equal(out[0], 1);
    assert.equal(out[1], 2);
    assert.equal(out[2], '[truncated]');
  });

  it('delegates non-arrays to sanitizeReplayValue', () => {
    assert.equal(compactReplayValue('hi'), 'hi');
    assert.equal(compactReplayValue(7), 7);
  });
});

describe('replay-sanitizer · REDACT_REPLAY_KEYS surface', () => {
  it('contains canonical sensitive keys', () => {
    for (const k of [
      'api_key',
      'apiKey',
      'password',
      'secret',
      'token',
      'authorization',
      'private_key',
      'wallet_private_key',
      'signature',
    ]) {
      assert.ok(REDACT_REPLAY_KEYS.has(k), `expected ${k} to be redacted`);
    }
  });
});
