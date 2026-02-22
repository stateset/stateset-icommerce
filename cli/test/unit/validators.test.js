import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import {
  validateFormat,
  validateBudget,
  validateProvider,
  validateModel,
  validateThinkLevel,
  VALID_FORMATS,
  VALID_THINK_LEVELS,
  VALID_PROVIDERS,
} from '../../src/utils/validators.js';

describe('validateFormat', () => {
  for (const fmt of VALID_FORMATS) {
    it(`accepts '${fmt}'`, () => {
      assert.deepEqual(validateFormat(fmt), { valid: true });
    });
  }

  it('rejects invalid format', () => {
    const result = validateFormat('xml');
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('xml'));
    assert.ok(result.error.includes('table'));
  });

  it('rejects empty string', () => {
    const result = validateFormat('');
    assert.equal(result.valid, false);
  });

  it('error message lists all valid formats', () => {
    const result = validateFormat('invalid');
    for (const fmt of VALID_FORMATS) {
      assert.ok(result.error.includes(fmt), `error should mention '${fmt}'`);
    }
  });
});

describe('validateBudget', () => {
  it('accepts positive number', () => {
    assert.deepEqual(validateBudget('1.00'), { valid: true });
  });

  it('accepts integer', () => {
    assert.deepEqual(validateBudget('5'), { valid: true });
  });

  it('accepts large budget', () => {
    assert.deepEqual(validateBudget('1000'), { valid: true });
  });

  it('strips dollar sign prefix', () => {
    assert.deepEqual(validateBudget('$50'), { valid: true });
  });

  it('rejects zero', () => {
    const result = validateBudget('0');
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('positive'));
  });

  it('rejects negative', () => {
    const result = validateBudget('-5');
    assert.equal(result.valid, false);
  });

  it('rejects non-numeric', () => {
    const result = validateBudget('abc');
    assert.equal(result.valid, false);
  });

  it('rejects Infinity', () => {
    const result = validateBudget('Infinity');
    assert.equal(result.valid, false);
  });
});

describe('validateProvider', () => {
  for (const p of VALID_PROVIDERS) {
    it(`accepts '${p}'`, () => {
      assert.deepEqual(validateProvider(p), { valid: true });
    });
  }

  it('rejects unknown provider', () => {
    const result = validateProvider('azure');
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('azure'));
    assert.ok(result.error.includes('claude'));
  });
});

describe('validateModel', () => {
  it('accepts claude model', () => {
    const result = validateModel('claude-sonnet-4-5-20250929');
    assert.equal(result.valid, true);
    assert.equal(result.warning, undefined);
  });

  it('accepts gpt model', () => {
    const result = validateModel('gpt-4o');
    assert.equal(result.valid, true);
  });

  it('accepts gemini model', () => {
    const result = validateModel('gemini-2.0-flash');
    assert.equal(result.valid, true);
  });

  it('warns on unknown model (does not reject)', () => {
    const result = validateModel('my-custom-model');
    assert.equal(result.valid, true);
    assert.ok(result.warning);
    assert.ok(result.warning.includes('my-custom-model'));
  });

  it('warns on ollama-style model names', () => {
    const result = validateModel('llama3:latest');
    assert.equal(result.valid, true);
    assert.ok(result.warning.includes('ollama'));
  });
});

describe('validateThinkLevel', () => {
  for (const level of VALID_THINK_LEVELS) {
    it(`accepts '${level}'`, () => {
      assert.deepEqual(validateThinkLevel(level), { valid: true });
    });
  }

  it('rejects invalid level', () => {
    const result = validateThinkLevel('max');
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('max'));
    assert.ok(result.error.includes('off'));
  });
});
