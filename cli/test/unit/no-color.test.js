import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { RichOutput } from '../../src/output.js';

describe('NO_COLOR support', () => {
  let savedEnv;

  beforeEach(() => {
    savedEnv = process.env.NO_COLOR;
  });

  afterEach(() => {
    if (savedEnv === undefined) {
      delete process.env.NO_COLOR;
    } else {
      process.env.NO_COLOR = savedEnv;
    }
  });

  it('disables color when NO_COLOR env var is set', () => {
    process.env.NO_COLOR = '1';
    const output = new RichOutput({ color: true });
    // Even with color:true, NO_COLOR overrides (isTTY is also false in tests)
    assert.equal(output.color, false);
  });

  it('color property is always a boolean', () => {
    process.env.NO_COLOR = '';
    const output = new RichOutput({ color: true });
    assert.equal(typeof output.color, 'boolean');
  });

  it('color is boolean when NO_COLOR is not set', () => {
    delete process.env.NO_COLOR;
    const output = new RichOutput({ color: true });
    // In test environment, isTTY is typically undefined, so color will be false
    assert.equal(typeof output.color, 'boolean');
  });

  it('respects color:false option regardless of NO_COLOR', () => {
    delete process.env.NO_COLOR;
    const output = new RichOutput({ color: false });
    assert.equal(output.color, false);
  });

  it('bold() returns plain text when color is disabled', () => {
    process.env.NO_COLOR = '1';
    const output = new RichOutput({ color: true });
    const result = output.bold('test');
    assert.equal(result, 'test');
    assert.ok(!result.includes('\x1b['), 'should not contain ANSI escape codes');
  });

  it('green() returns plain text when color is disabled', () => {
    process.env.NO_COLOR = '1';
    const output = new RichOutput({ color: true });
    const result = output.green('test');
    assert.equal(result, 'test');
  });

  it('dim() returns plain text when color is disabled', () => {
    process.env.NO_COLOR = '1';
    const output = new RichOutput({ color: true });
    const result = output.dim('test');
    assert.equal(result, 'test');
  });

  it('status() returns plain text when color is disabled', () => {
    process.env.NO_COLOR = '1';
    const output = new RichOutput({ color: true });
    const result = output.status('error', 'something broke');
    assert.ok(result.includes('something broke'));
    assert.ok(!result.includes('\x1b['));
  });
});
