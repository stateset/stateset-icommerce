/**
 * Tests for cli/src/channels/plugin-manifest.js
 *
 * Covers: validateManifest, readManifest, validateConfig, applyConfigDefaults.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'fs';
import path from 'path';
import os from 'os';

import {
  validateManifest,
  readManifest,
  validateConfig,
  applyConfigDefaults,
} from '../../src/channels/plugin-manifest.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function validManifest(overrides = {}) {
  return {
    id: 'my-plugin',
    name: 'My Plugin',
    entry: 'index.js',
    ...overrides,
  };
}

function tmpPluginDir(manifest) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'pm-test-'));
  if (manifest) {
    fs.writeFileSync(path.join(dir, 'stateset.plugin.json'), JSON.stringify(manifest));
  }
  return dir;
}

// ---------------------------------------------------------------------------
// validateManifest
// ---------------------------------------------------------------------------

describe('validateManifest', () => {
  it('validates a minimal valid manifest', () => {
    const result = validateManifest(validManifest());
    assert.ok(result.valid);
    assert.equal(result.errors.length, 0);
    assert.equal(result.manifest.id, 'my-plugin');
  });

  it('normalizes defaults', () => {
    const result = validateManifest(validManifest());
    assert.equal(result.manifest.version, '0.0.0');
    assert.equal(result.manifest.kind, 'general');
    assert.equal(result.manifest.enabledByDefault, false);
    assert.deepStrictEqual(result.manifest.channels, []);
    assert.deepStrictEqual(result.manifest.provides, []);
  });

  it('rejects null input', () => {
    const result = validateManifest(null);
    assert.ok(!result.valid);
    assert.ok(result.errors[0].includes('JSON object'));
  });

  it('requires id field', () => {
    const result = validateManifest({ name: 'X', entry: 'x.js' });
    assert.ok(!result.valid);
    assert.ok(result.errors.some((e) => e.includes('id')));
  });

  it('requires name field', () => {
    const result = validateManifest({ id: 'x', entry: 'x.js' });
    assert.ok(!result.valid);
    assert.ok(result.errors.some((e) => e.includes('name')));
  });

  it('requires entry field', () => {
    const result = validateManifest({ id: 'x', name: 'X' });
    assert.ok(!result.valid);
    assert.ok(result.errors.some((e) => e.includes('entry')));
  });

  it('rejects invalid id format', () => {
    const result = validateManifest(validManifest({ id: 'BAD ID' }));
    assert.ok(!result.valid);
    assert.ok(result.errors.some((e) => e.includes('Invalid plugin ID')));
  });

  it('rejects invalid kind', () => {
    const result = validateManifest(validManifest({ kind: 'bogus' }));
    assert.ok(!result.valid);
    assert.ok(result.errors.some((e) => e.includes('Invalid kind')));
  });

  it('accepts valid kinds', () => {
    for (const kind of ['general', 'channel', 'memory', 'provider']) {
      const result = validateManifest(validManifest({ kind }));
      assert.ok(result.valid, `kind "${kind}" should be valid`);
    }
  });

  it('warns on non-semver version', () => {
    const result = validateManifest(validManifest({ version: 'beta' }));
    assert.ok(result.valid);
    assert.ok(result.warnings.some((w) => w.includes('SemVer')));
  });

  it('warns when entry file not found', () => {
    const result = validateManifest(validManifest({ entry: 'nonexistent.js' }), '/tmp');
    assert.ok(result.valid);
    assert.ok(result.warnings.some((w) => w.includes('not found')));
  });

  it('rejects non-object configSchema', () => {
    const result = validateManifest(validManifest({ configSchema: 'bad' }));
    assert.ok(!result.valid);
    assert.ok(result.errors.some((e) => e.includes('configSchema')));
  });

  it('rejects non-array channels', () => {
    const result = validateManifest(validManifest({ channels: 'telegram' }));
    assert.ok(!result.valid);
    assert.ok(result.errors.some((e) => e.includes('channels')));
  });

  it('rejects non-array provides', () => {
    const result = validateManifest(validManifest({ provides: 'commands' }));
    assert.ok(!result.valid);
    assert.ok(result.errors.some((e) => e.includes('provides')));
  });
});

// ---------------------------------------------------------------------------
// readManifest
// ---------------------------------------------------------------------------

describe('readManifest', () => {
  let dir;

  afterEach(() => {
    if (dir) {
      try {
        fs.rmSync(dir, { recursive: true });
      } catch {
        /* ok */
      }
    }
  });

  it('reads and validates a manifest file', () => {
    dir = tmpPluginDir(validManifest());
    // Also create the entry file so no warning
    fs.writeFileSync(path.join(dir, 'index.js'), '');

    const result = readManifest(dir);
    assert.ok(result.found);
    assert.ok(result.manifest);
    assert.equal(result.manifest.id, 'my-plugin');
  });

  it('returns found=false for missing manifest', () => {
    dir = tmpPluginDir(null);
    const result = readManifest(dir);
    assert.ok(!result.found);
  });

  it('returns errors for invalid manifest', () => {
    dir = tmpPluginDir({ invalid: true });
    const result = readManifest(dir);
    assert.ok(result.found);
    assert.ok(result.errors.length > 0);
  });

  it('handles malformed JSON', () => {
    dir = fs.mkdtempSync(path.join(os.tmpdir(), 'pm-json-'));
    fs.writeFileSync(path.join(dir, 'stateset.plugin.json'), '{not json}');

    const result = readManifest(dir);
    assert.ok(result.found);
    assert.ok(result.errors.some((e) => e.includes('parse')));
  });
});

// ---------------------------------------------------------------------------
// validateConfig
// ---------------------------------------------------------------------------

describe('validateConfig', () => {
  it('passes with no schema', () => {
    const result = validateConfig({ anything: true }, null);
    assert.ok(result.valid);
  });

  it('validates required fields', () => {
    const schema = { required: ['apiKey'] };
    const result = validateConfig({}, schema);
    assert.ok(!result.valid);
    assert.ok(result.errors[0].includes('apiKey'));
  });

  it('validates property types', () => {
    const schema = {
      properties: {
        count: { type: 'number' },
      },
    };
    const result = validateConfig({ count: 'not a number' }, schema);
    assert.ok(!result.valid);
    assert.ok(result.errors[0].includes('count'));
  });

  it('validates enum values', () => {
    const schema = {
      properties: {
        mode: { enum: ['fast', 'slow'] },
      },
    };
    const badResult = validateConfig({ mode: 'invalid' }, schema);
    assert.ok(!badResult.valid);

    const goodResult = validateConfig({ mode: 'fast' }, schema);
    assert.ok(goodResult.valid);
  });

  it('validates minLength', () => {
    const schema = {
      properties: {
        name: { minLength: 3 },
      },
    };
    const result = validateConfig({ name: 'ab' }, schema);
    assert.ok(!result.valid);
  });

  it('validates minimum / maximum', () => {
    const schema = {
      properties: {
        port: { minimum: 1, maximum: 65535 },
      },
    };
    assert.ok(!validateConfig({ port: 0 }, schema).valid);
    assert.ok(!validateConfig({ port: 70000 }, schema).valid);
    assert.ok(validateConfig({ port: 8080 }, schema).valid);
  });

  it('skips undefined properties', () => {
    const schema = {
      properties: {
        optional: { type: 'string' },
      },
    };
    const result = validateConfig({}, schema);
    assert.ok(result.valid);
  });
});

// ---------------------------------------------------------------------------
// applyConfigDefaults
// ---------------------------------------------------------------------------

describe('applyConfigDefaults', () => {
  it('applies defaults for missing keys', () => {
    const result = applyConfigDefaults({ a: 1 }, { a: 0, b: 2 });
    assert.equal(result.a, 1);
    assert.equal(result.b, 2);
  });

  it('user values override defaults', () => {
    const result = applyConfigDefaults({ x: 'user' }, { x: 'default' });
    assert.equal(result.x, 'user');
  });

  it('handles null defaults', () => {
    const result = applyConfigDefaults({ a: 1 }, null);
    assert.deepStrictEqual(result, { a: 1 });
  });

  it('ignores null/undefined user values', () => {
    const result = applyConfigDefaults({ a: null, b: undefined }, { a: 'default', b: 'default' });
    assert.equal(result.a, 'default');
    assert.equal(result.b, 'default');
  });
});
