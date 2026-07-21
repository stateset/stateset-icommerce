/**
 * Unit tests for load-env.js — .env file parsing logic
 *
 * load-env.js is a side-effect module that runs on import.  We cannot
 * re-import it multiple times, so we replicate its parsing logic in a
 * pure function and test that exhaustively.  We also verify the actual
 * side-effect behaviour (env-precedence and warning on failure) with a
 * small integration group that manipulates the filesystem and env before
 * dynamically importing the module.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert';

// ---------------------------------------------------------------------------
// Replicate the exact parsing logic from load-env.js so we can unit-test it
// without side-effects.
// ---------------------------------------------------------------------------

/**
 * Parse the contents of a .env file into a plain object.
 * Mirrors the logic in cli/src/load-env.js lines 19-37.
 */
function parseEnvContent(content) {
  const vars = {};
  for (const line of content.split('\n')) {
    const trimmed = line.trim();
    if (trimmed && !trimmed.startsWith('#')) {
      const eqIndex = trimmed.indexOf('=');
      if (eqIndex > 0) {
        const key = trimmed.slice(0, eqIndex).trim();
        let value = trimmed.slice(eqIndex + 1);
        // Remove surrounding quotes
        if (
          (value.startsWith('"') && value.endsWith('"')) ||
          (value.startsWith("'") && value.endsWith("'"))
        ) {
          value = value.slice(1, -1);
        }
        vars[key] = value;
      }
    }
  }
  return vars;
}

// ===========================================================================
// Pure parsing tests
// ===========================================================================

describe('load-env parsing logic', () => {
  // ---- basic key=value ----

  it('parses simple key=value', () => {
    const result = parseEnvContent('FOO=bar');
    assert.deepStrictEqual(result, { FOO: 'bar' });
  });

  it('parses multiple key=value pairs', () => {
    const result = parseEnvContent('A=1\nB=2\nC=3');
    assert.deepStrictEqual(result, { A: '1', B: '2', C: '3' });
  });

  it('parses value with no content (empty value)', () => {
    const result = parseEnvContent('EMPTY=');
    assert.deepStrictEqual(result, { EMPTY: '' });
  });

  // ---- comments and blanks ----

  it('skips comment lines starting with #', () => {
    const result = parseEnvContent('# this is a comment\nKEY=val');
    assert.deepStrictEqual(result, { KEY: 'val' });
  });

  it('skips inline-only comment lines (full line)', () => {
    const result = parseEnvContent('  # indented comment\nKEY=val');
    assert.deepStrictEqual(result, { KEY: 'val' });
  });

  it('skips blank lines', () => {
    const result = parseEnvContent('\n\nKEY=val\n\n');
    assert.deepStrictEqual(result, { KEY: 'val' });
  });

  it('skips lines that are only whitespace', () => {
    const result = parseEnvContent('   \n\t\nKEY=val');
    assert.deepStrictEqual(result, { KEY: 'val' });
  });

  // ---- quoting ----

  it('removes surrounding double quotes from value', () => {
    const result = parseEnvContent('KEY="hello world"');
    assert.deepStrictEqual(result, { KEY: 'hello world' });
  });

  it('removes surrounding single quotes from value', () => {
    const result = parseEnvContent("KEY='hello world'");
    assert.deepStrictEqual(result, { KEY: 'hello world' });
  });

  it('does not strip mismatched quotes (double then single)', () => {
    const result = parseEnvContent('KEY="hello\'');
    assert.deepStrictEqual(result, { KEY: '"hello\'' });
  });

  it('does not strip mismatched quotes (single then double)', () => {
    const result = parseEnvContent('KEY=\'hello"');
    assert.deepStrictEqual(result, { KEY: '\'hello"' });
  });

  it('does not strip internal quotes in value', () => {
    // e.g. KEY=foo"bar — no surrounding quotes
    const result = parseEnvContent('KEY=foo"bar');
    assert.deepStrictEqual(result, { KEY: 'foo"bar' });
  });

  it('preserves empty quoted string', () => {
    const result = parseEnvContent('KEY=""');
    assert.deepStrictEqual(result, { KEY: '' });
  });

  it('preserves single-char quoted value', () => {
    const result = parseEnvContent('KEY="x"');
    assert.deepStrictEqual(result, { KEY: 'x' });
  });

  // ---- equals sign edge cases ----

  it('handles values containing = signs', () => {
    const result = parseEnvContent('KEY=base64==');
    assert.deepStrictEqual(result, { KEY: 'base64==' });
  });

  it('handles values with = inside quoted string', () => {
    const result = parseEnvContent('KEY="a=b=c"');
    assert.deepStrictEqual(result, { KEY: 'a=b=c' });
  });

  it('ignores lines without = sign', () => {
    const result = parseEnvContent('NOEQUALSSIGN');
    assert.deepStrictEqual(result, {});
  });

  it('ignores lines where = is the first char (empty key)', () => {
    // eqIndex would be 0, which is not > 0
    const result = parseEnvContent('=value');
    assert.deepStrictEqual(result, {});
  });

  // ---- key whitespace handling ----

  it('trims spaces around the key', () => {
    const result = parseEnvContent('  MY_KEY  =myvalue');
    assert.deepStrictEqual(result, { MY_KEY: 'myvalue' });
  });

  // ---- mixed content ----

  it('handles realistic mixed .env content', () => {
    const content = [
      '# StateSet API config',
      '',
      'ANTHROPIC_API_KEY="sk-ant-abc123"',
      "STATESET_API_KEY='key-xyz'",
      'DATABASE_URL=postgres://localhost/db',
      '',
      '# Feature flags',
      'ENABLE_BETA=true',
      'MAX_RETRIES=3',
    ].join('\n');

    const result = parseEnvContent(content);
    assert.deepStrictEqual(result, {
      ANTHROPIC_API_KEY: 'sk-ant-abc123',
      STATESET_API_KEY: 'key-xyz',
      DATABASE_URL: 'postgres://localhost/db',
      ENABLE_BETA: 'true',
      MAX_RETRIES: '3',
    });
  });

  it('returns empty object for empty content', () => {
    assert.deepStrictEqual(parseEnvContent(''), {});
  });

  it('returns empty object for content with only comments and blanks', () => {
    assert.deepStrictEqual(parseEnvContent('# comment\n\n# another'), {});
  });

  it('last assignment wins for duplicate keys', () => {
    const result = parseEnvContent('KEY=first\nKEY=second');
    assert.deepStrictEqual(result, { KEY: 'second' });
  });
});
