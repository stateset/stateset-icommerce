import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { safeJsonParse, safeJsonParseFile, strictJsonParse } from '../../src/utils/safe-json.js';
import { ValidationError } from '../../src/errors.js';

describe('safe-json', () => {
  describe('safeJsonParse', () => {
    it('parses valid JSON object', () => {
      assert.deepStrictEqual(safeJsonParse('{"a":1}'), { a: 1 });
    });

    it('parses valid JSON array', () => {
      assert.deepStrictEqual(safeJsonParse('[1,2,3]'), [1, 2, 3]);
    });

    it('parses valid JSON primitives', () => {
      assert.strictEqual(safeJsonParse('"hello"'), 'hello');
      assert.strictEqual(safeJsonParse('42'), 42);
      assert.strictEqual(safeJsonParse('true'), true);
      assert.strictEqual(safeJsonParse('null'), null);
    });

    it('returns null fallback by default on invalid JSON', () => {
      assert.strictEqual(safeJsonParse('{invalid}'), null);
    });

    it('returns custom fallback on invalid JSON', () => {
      assert.deepStrictEqual(safeJsonParse('{bad}', { fallback: {} }), {});
      assert.deepStrictEqual(safeJsonParse('{bad}', { fallback: [] }), []);
      assert.strictEqual(safeJsonParse('{bad}', { fallback: 'default' }), 'default');
    });

    it('returns fallback for empty string', () => {
      assert.deepStrictEqual(safeJsonParse('', { fallback: {} }), {});
    });

    it('returns fallback for non-string input', () => {
      assert.strictEqual(safeJsonParse(123), null);
      assert.strictEqual(safeJsonParse(undefined), null);
      assert.strictEqual(safeJsonParse(null), null);
      assert.deepStrictEqual(safeJsonParse({}, { fallback: [] }), []);
    });

    it('throws ValidationError when throwOnError is true', () => {
      assert.throws(() => safeJsonParse('{invalid}', { throwOnError: true }), ValidationError);
    });

    it('throws ValidationError for non-string when throwOnError is true', () => {
      assert.throws(() => safeJsonParse(123, { throwOnError: true }), ValidationError);
    });

    it('includes context in error message', () => {
      try {
        safeJsonParse('{bad}', { throwOnError: true, context: 'session file' });
        assert.fail('should have thrown');
      } catch (err) {
        assert.ok(err.message.includes('session file'));
      }
    });

    it('does not throw for valid JSON when throwOnError is true', () => {
      assert.deepStrictEqual(safeJsonParse('{"ok":true}', { throwOnError: true }), { ok: true });
    });
  });

  describe('safeJsonParseFile', () => {
    it('parses valid file content', () => {
      assert.deepStrictEqual(safeJsonParseFile('{"a":1}', '/path/config.json'), { a: 1 });
    });

    it('includes file path in error context', () => {
      try {
        safeJsonParseFile('{bad}', '/home/user/broken.json', { throwOnError: true });
        assert.fail('should have thrown');
      } catch (err) {
        assert.ok(err.message.includes('/home/user/broken.json'));
      }
    });

    it('returns fallback for invalid file content', () => {
      assert.deepStrictEqual(safeJsonParseFile('{bad}', '/a.json', { fallback: {} }), {});
    });
  });

  describe('strictJsonParse', () => {
    it('parses valid JSON', () => {
      assert.deepStrictEqual(strictJsonParse('{"a":1}'), { a: 1 });
    });

    it('always throws on invalid JSON', () => {
      assert.throws(() => strictJsonParse('{bad}'), ValidationError);
    });

    it('always throws on non-string', () => {
      assert.throws(() => strictJsonParse(123), ValidationError);
    });

    it('includes context in error', () => {
      try {
        strictJsonParse('{bad}', 'outbox row');
        assert.fail('should have thrown');
      } catch (err) {
        assert.ok(err.message.includes('outbox row'));
      }
    });
  });
});
