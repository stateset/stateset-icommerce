import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

// ui.js depends on @clack/prompts which requires TTY for interactive methods.
// These tests verify the module loads and exports the expected API surface.
// Interactive behavior is tested via the non-TTY fallback paths.

describe('ui', () => {
  let ui;

  it('module loads without error', async () => {
    ui = await import('../../src/ui.js');
    assert.ok(ui);
  });

  it('exports withSpinner', async () => {
    ui = await import('../../src/ui.js');
    assert.ok(typeof ui.withSpinner === 'function');
  });

  it('exports confirm', async () => {
    ui = await import('../../src/ui.js');
    assert.ok(typeof ui.confirm === 'function');
  });

  it('exports select', async () => {
    ui = await import('../../src/ui.js');
    assert.ok(typeof ui.select === 'function');
  });

  it('exports text', async () => {
    ui = await import('../../src/ui.js');
    assert.ok(typeof ui.text === 'function');
  });

  it('exports password', async () => {
    ui = await import('../../src/ui.js');
    assert.ok(typeof ui.password === 'function');
  });

  it('exports intro', async () => {
    ui = await import('../../src/ui.js');
    assert.ok(typeof ui.intro === 'function');
  });

  it('exports outro', async () => {
    ui = await import('../../src/ui.js');
    assert.ok(typeof ui.outro === 'function');
  });

  it('exports note', async () => {
    ui = await import('../../src/ui.js');
    assert.ok(typeof ui.note === 'function');
  });

  it('exports tasks', async () => {
    ui = await import('../../src/ui.js');
    assert.ok(typeof ui.tasks === 'function');
  });

  it('exports log helpers', async () => {
    ui = await import('../../src/ui.js');
    assert.ok(typeof ui.log === 'function');
    assert.ok(typeof ui.logSuccess === 'function');
    assert.ok(typeof ui.logError === 'function');
    assert.ok(typeof ui.logWarning === 'function');
    assert.ok(typeof ui.logInfo === 'function');
  });

  describe('confirm with assumeYes', () => {
    it('returns true when assumeYes is set', async () => {
      ui = await import('../../src/ui.js');
      const result = await ui.confirm('Proceed?', { assumeYes: true });
      assert.strictEqual(result, true);
    });
  });

  describe('non-TTY fallbacks', () => {
    it('confirm returns false in non-TTY', async () => {
      ui = await import('../../src/ui.js');
      const origIsTTY = process.stdin.isTTY;
      process.stdin.isTTY = false;
      try {
        const result = await ui.confirm('Proceed?');
        assert.strictEqual(result, false);
      } finally {
        process.stdin.isTTY = origIsTTY;
      }
    });

    it('select returns first option in non-TTY', async () => {
      ui = await import('../../src/ui.js');
      const origIsTTY = process.stdin.isTTY;
      process.stdin.isTTY = false;
      try {
        const result = await ui.select('Pick one', [
          { value: 'a', label: 'Option A' },
          { value: 'b', label: 'Option B' },
        ]);
        assert.strictEqual(result, 'a');
      } finally {
        process.stdin.isTTY = origIsTTY;
      }
    });

    it('text returns defaultValue in non-TTY', async () => {
      ui = await import('../../src/ui.js');
      const origIsTTY = process.stdin.isTTY;
      process.stdin.isTTY = false;
      try {
        const result = await ui.text('Name?', { defaultValue: 'default' });
        assert.strictEqual(result, 'default');
      } finally {
        process.stdin.isTTY = origIsTTY;
      }
    });

    it('text returns empty string in non-TTY with no default', async () => {
      ui = await import('../../src/ui.js');
      const origIsTTY = process.stdin.isTTY;
      process.stdin.isTTY = false;
      try {
        const result = await ui.text('Name?');
        assert.strictEqual(result, '');
      } finally {
        process.stdin.isTTY = origIsTTY;
      }
    });

    it('password returns empty string in non-TTY', async () => {
      ui = await import('../../src/ui.js');
      const origIsTTY = process.stdin.isTTY;
      process.stdin.isTTY = false;
      try {
        const result = await ui.password('Key?');
        assert.strictEqual(result, '');
      } finally {
        process.stdin.isTTY = origIsTTY;
      }
    });

    it('withSpinner works in non-TTY', async () => {
      ui = await import('../../src/ui.js');
      const origIsTTY = process.stderr.isTTY;
      process.stderr.isTTY = false;

      const logs = [];
      const origError = console.error;
      console.error = (...args) => logs.push(args.join(' '));

      try {
        const result = await ui.withSpinner('Loading', async () => 42);
        assert.strictEqual(result, 42);
        assert.ok(logs.some((l) => l.includes('Loading')));
      } finally {
        console.error = origError;
        process.stderr.isTTY = origIsTTY;
      }
    });

    it('withSpinner propagates errors in non-TTY', async () => {
      ui = await import('../../src/ui.js');
      const origIsTTY = process.stderr.isTTY;
      process.stderr.isTTY = false;

      const origError = console.error;
      console.error = () => {};

      try {
        await assert.rejects(
          () => ui.withSpinner('Loading', async () => { throw new Error('fail'); }),
          { message: 'fail' },
        );
      } finally {
        console.error = origError;
        process.stderr.isTTY = origIsTTY;
      }
    });
  });
});
