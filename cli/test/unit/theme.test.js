import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import { createTheme, PALETTE, theme } from '../../src/theme.js';

describe('theme', () => {
  describe('PALETTE', () => {
    it('exports accent color', () => {
      assert.ok(PALETTE.accent);
      assert.ok(PALETTE.accent.includes('\x1b['));
    });

    it('exports all semantic colors', () => {
      for (const key of ['accent', 'accentBright', 'accentDim', 'success', 'warn', 'error', 'info', 'muted']) {
        assert.ok(PALETTE[key], `missing palette entry: ${key}`);
      }
    });

    it('exports formatting codes', () => {
      assert.ok(PALETTE.bold);
      assert.ok(PALETTE.dim);
      assert.ok(PALETTE.reset);
    });
  });

  describe('createTheme({ color: true })', () => {
    let t;
    beforeEach(() => {
      t = createTheme({ color: true });
    });

    it('wraps text in ANSI codes', () => {
      const result = t.accent('hello');
      assert.ok(result.includes('hello'));
      assert.ok(result.includes('\x1b['));
      assert.ok(result.endsWith(PALETTE.reset));
    });

    it('applies success color', () => {
      const result = t.success('ok');
      assert.ok(result.includes(PALETTE.success));
      assert.ok(result.includes('ok'));
    });

    it('applies warn color', () => {
      const result = t.warn('caution');
      assert.ok(result.includes(PALETTE.warn));
    });

    it('applies error color', () => {
      const result = t.error('fail');
      assert.ok(result.includes(PALETTE.error));
    });

    it('applies info color', () => {
      const result = t.info('note');
      assert.ok(result.includes(PALETTE.info));
    });

    it('applies muted color', () => {
      const result = t.muted('dim text');
      assert.ok(result.includes(PALETTE.muted));
    });

    it('applies bold', () => {
      const result = t.bold('strong');
      assert.ok(result.includes(PALETTE.bold));
    });

    it('applies dim', () => {
      const result = t.dim('faded');
      assert.ok(result.includes(PALETTE.dim));
    });

    it('applies italic', () => {
      const result = t.italic('slanted');
      assert.ok(result.includes(PALETTE.italic));
    });

    it('applies underline', () => {
      const result = t.underline('lined');
      assert.ok(result.includes(PALETTE.underline));
    });

    it('heading combines bold + accent', () => {
      const result = t.heading('Title');
      assert.ok(result.includes(PALETTE.bold));
      assert.ok(result.includes(PALETTE.accent));
      assert.ok(result.includes('Title'));
    });

    it('command wraps in accentBright', () => {
      const result = t.command('stateset');
      assert.ok(result.includes(PALETTE.accentBright));
    });

    it('option wraps in warn', () => {
      const result = t.option('--apply');
      assert.ok(result.includes(PALETTE.warn));
    });

    it('label combines bold + white', () => {
      const result = t.label('Name');
      assert.ok(result.includes(PALETTE.bold));
    });

    it('isRich returns true', () => {
      assert.strictEqual(t.isRich(), true);
    });
  });

  describe('createTheme({ color: false })', () => {
    let t;
    beforeEach(() => {
      t = createTheme({ color: false });
    });

    it('returns plain text without ANSI codes', () => {
      assert.strictEqual(t.accent('hello'), 'hello');
      assert.strictEqual(t.success('ok'), 'ok');
      assert.strictEqual(t.error('fail'), 'fail');
      assert.strictEqual(t.bold('strong'), 'strong');
      assert.strictEqual(t.dim('faded'), 'faded');
    });

    it('heading returns plain text', () => {
      assert.strictEqual(t.heading('Title'), 'Title');
    });

    it('isRich returns false', () => {
      assert.strictEqual(t.isRich(), false);
    });

    it('converts non-string input to string', () => {
      assert.strictEqual(t.accent(42), '42');
      assert.strictEqual(t.success(true), 'true');
    });
  });

  describe('NO_COLOR environment variable', () => {
    let origNoColor;
    let origForceColor;

    beforeEach(() => {
      origNoColor = process.env.NO_COLOR;
      origForceColor = process.env.FORCE_COLOR;
    });

    afterEach(() => {
      if (origNoColor === undefined) delete process.env.NO_COLOR;
      else process.env.NO_COLOR = origNoColor;
      if (origForceColor === undefined) delete process.env.FORCE_COLOR;
      else process.env.FORCE_COLOR = origForceColor;
    });

    it('disables color when NO_COLOR is set', () => {
      process.env.NO_COLOR = '1';
      delete process.env.FORCE_COLOR;
      const t = createTheme();
      assert.strictEqual(t.isRich(), false);
      assert.strictEqual(t.accent('hello'), 'hello');
    });

    it('explicit color: true overrides NO_COLOR', () => {
      process.env.NO_COLOR = '1';
      const t = createTheme({ color: true });
      assert.strictEqual(t.isRich(), true);
      assert.ok(t.accent('hello').includes('\x1b['));
    });
  });

  describe('default theme singleton', () => {
    it('is a valid theme object', () => {
      assert.ok(typeof theme.accent === 'function');
      assert.ok(typeof theme.success === 'function');
      assert.ok(typeof theme.error === 'function');
      assert.ok(typeof theme.isRich === 'function');
    });
  });
});
