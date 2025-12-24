/**
 * Unit tests for config.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import {
  MODELS,
  DEFAULT_MODEL,
  CLI_DEFAULTS,
  FEATURES,
  getModelForAgent,
  getParseArgsOptions
} from '../../src/config.js';

describe('config', () => {
  describe('MODELS', () => {
    it('should have all required model definitions', () => {
      assert.ok(MODELS.SONNET, 'SONNET model should be defined');
      assert.ok(MODELS.OPUS, 'OPUS model should be defined');
      assert.ok(MODELS.HAIKU, 'HAIKU model should be defined');
      assert.ok(MODELS.DEFAULT, 'DEFAULT model should be defined');
    });

    it('should have valid model ID formats', () => {
      for (const [name, model] of Object.entries(MODELS)) {
        assert.ok(model.startsWith('claude-'), `${name} should start with claude-`);
      }
    });
  });

  describe('DEFAULT_MODEL', () => {
    it('should be a valid model', () => {
      assert.ok(DEFAULT_MODEL, 'DEFAULT_MODEL should be defined');
      assert.ok(Object.values(MODELS).includes(DEFAULT_MODEL), 'DEFAULT_MODEL should be one of MODELS');
    });
  });

  describe('CLI_DEFAULTS', () => {
    it('should have safe defaults', () => {
      assert.strictEqual(CLI_DEFAULTS.apply, false, 'apply should default to false');
      assert.strictEqual(CLI_DEFAULTS.json, false, 'json should default to false');
      assert.strictEqual(CLI_DEFAULTS.verbose, false, 'verbose should default to false');
    });

    it('should have required properties', () => {
      assert.ok(CLI_DEFAULTS.dbPath, 'dbPath should be defined');
      assert.ok(CLI_DEFAULTS.model, 'model should be defined');
    });
  });

  describe('FEATURES', () => {
    it('should have feature flags', () => {
      assert.strictEqual(typeof FEATURES.semanticRouting, 'boolean');
      assert.strictEqual(typeof FEATURES.retryOnFailure, 'boolean');
      assert.strictEqual(typeof FEATURES.telemetryEnabled, 'boolean');
    });

    it('should have valid retry configuration', () => {
      assert.ok(FEATURES.maxRetries > 0, 'maxRetries should be positive');
      assert.ok(FEATURES.maxRetries <= 10, 'maxRetries should be reasonable');
    });
  });

  describe('getModelForAgent', () => {
    it('should return DEFAULT_MODEL for unknown agents', () => {
      const result = getModelForAgent('unknown-agent');
      assert.strictEqual(result, DEFAULT_MODEL);
    });

    it('should return DEFAULT_MODEL for known agents without override', () => {
      const result = getModelForAgent('customer-service');
      assert.strictEqual(result, DEFAULT_MODEL);
    });
  });

  describe('getParseArgsOptions', () => {
    it('should return valid parseArgs options', () => {
      const options = getParseArgsOptions();

      assert.ok(options.db, 'db option should be defined');
      assert.ok(options.apply, 'apply option should be defined');
      assert.ok(options.model, 'model option should be defined');
      assert.ok(options.help, 'help option should be defined');
    });

    it('should allow overrides', () => {
      const options = getParseArgsOptions({
        custom: { type: 'boolean', default: true }
      });

      assert.ok(options.custom, 'custom option should be merged');
      assert.strictEqual(options.custom.default, true);
    });

    it('should have correct types', () => {
      const options = getParseArgsOptions();

      assert.strictEqual(options.db.type, 'string');
      assert.strictEqual(options.apply.type, 'boolean');
      assert.strictEqual(options.json.type, 'boolean');
    });
  });
});
