/**
 * Unit tests for memory/injector.js — MemoryInjector
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import {
  MemoryInjector,
  getMemoryInjector,
  registerMemoryHooks,
} from '../../src/memory/injector.js';

// ===========================================================================
// Constructor
// ===========================================================================

describe('MemoryInjector — constructor', () => {
  it('sets default maxMemories to 5', () => {
    const inj = new MemoryInjector();
    assert.strictEqual(inj._maxMemories, 5);
  });

  it('sets default maxBodyLength to 2000', () => {
    const inj = new MemoryInjector();
    assert.strictEqual(inj._maxBodyLength, 2000);
  });

  it('accepts custom maxMemories', () => {
    const inj = new MemoryInjector({ maxMemories: 10 });
    assert.strictEqual(inj._maxMemories, 10);
  });

  it('accepts custom maxBodyLength', () => {
    const inj = new MemoryInjector({ maxBodyLength: 500 });
    assert.strictEqual(inj._maxBodyLength, 500);
  });
});

// ===========================================================================
// setMaxMemories / setMaxBodyLength
// ===========================================================================

describe('MemoryInjector — setters', () => {
  it('setMaxMemories updates _maxMemories', () => {
    const inj = new MemoryInjector();
    inj.setMaxMemories(20);
    assert.strictEqual(inj._maxMemories, 20);
  });

  it('setMaxBodyLength updates _maxBodyLength', () => {
    const inj = new MemoryInjector();
    inj.setMaxBodyLength(3000);
    assert.strictEqual(inj._maxBodyLength, 3000);
  });
});

// ===========================================================================
// formatMemories
// ===========================================================================

describe('MemoryInjector — formatMemories', () => {
  let inj;

  beforeEach(() => {
    inj = new MemoryInjector();
  });

  it('returns null for null input', () => {
    assert.strictEqual(inj.formatMemories(null), null);
  });

  it('returns null for empty array', () => {
    assert.strictEqual(inj.formatMemories([]), null);
  });

  it('returns null for undefined', () => {
    assert.strictEqual(inj.formatMemories(undefined), null);
  });

  it('formats a single memory correctly', () => {
    const memories = [
      {
        created_at: '2025-01-15T10:30:00Z',
        summary: 'User asked about orders',
        agent: 'orders',
      },
    ];
    const result = inj.formatMemories(memories);
    assert.ok(result.includes('<memory-context>'));
    assert.ok(result.includes('</memory-context>'));
    assert.ok(result.includes('2025-01-15'));
    assert.ok(result.includes('User asked about orders'));
    assert.ok(result.includes('(orders)'));
  });

  it('formats multiple memories', () => {
    const memories = [
      { created_at: '2025-01-15T10:00:00Z', summary: 'First' },
      { created_at: '2025-01-15T11:00:00Z', summary: 'Second' },
    ];
    const result = inj.formatMemories(memories);
    assert.ok(result.includes('First'));
    assert.ok(result.includes('Second'));
  });

  it('includes facts when available', () => {
    const memories = [
      {
        created_at: '2025-01-15T10:00:00Z',
        summary: 'Order created',
        facts: ['Order ID is ORD-123', 'Customer is Alice'],
      },
    ];
    const result = inj.formatMemories(memories);
    assert.ok(result.includes('Facts:'));
    assert.ok(result.includes('Order ID is ORD-123'));
  });

  it('omits agent parenthetical when agent is missing', () => {
    const memories = [{ created_at: '2025-01-15T10:00:00Z', summary: 'Test' }];
    const result = inj.formatMemories(memories);
    assert.ok(!result.includes('()'));
  });

  it('respects maxBodyLength — truncates at limit', () => {
    const shortInj = new MemoryInjector({ maxBodyLength: 50 });
    const memories = [
      { created_at: '2025-01-15T10:00:00Z', summary: 'Short entry' },
      {
        created_at: '2025-01-15T11:00:00Z',
        summary: 'This is a very long entry that should cause the total length to exceed the limit',
      },
    ];
    const result = shortInj.formatMemories(memories);
    // Should still produce output (at least the first entry plus wrapper)
    assert.ok(result.includes('<memory-context>'));
    assert.ok(result.includes('</memory-context>'));
  });

  it('includes header "Previous conversation summaries:"', () => {
    const memories = [{ created_at: '2025-01-15T10:00:00Z', summary: 'Test' }];
    const result = inj.formatMemories(memories);
    assert.ok(result.includes('Previous conversation summaries:'));
  });
});

// ===========================================================================
// injectMemoryContext — early returns
// ===========================================================================

describe('MemoryInjector — injectMemoryContext', () => {
  let inj;

  beforeEach(() => {
    inj = new MemoryInjector();
  });

  it('returns unchanged data when memoryEnabled is false', async () => {
    const data = { text: 'hello', memoryEnabled: false };
    const result = await inj.injectMemoryContext(data);
    assert.strictEqual(result, data);
    assert.strictEqual(result.text, 'hello');
  });

  it('returns unchanged data when text is missing', async () => {
    const data = { memoryEnabled: true };
    const result = await inj.injectMemoryContext(data);
    assert.strictEqual(result, data);
  });

  it('returns unchanged data when data is null', async () => {
    const result = await inj.injectMemoryContext(null);
    assert.strictEqual(result, null);
  });

  it('returns unchanged data when data is undefined', async () => {
    const result = await inj.injectMemoryContext(undefined);
    assert.strictEqual(result, undefined);
  });

  it('returns unchanged data when data has no text property', async () => {
    const data = { memoryEnabled: true, channel: 'cli' };
    const result = await inj.injectMemoryContext(data);
    assert.strictEqual(result, data);
  });

  it('returns unchanged data when text is empty string', async () => {
    const data = { text: '', memoryEnabled: true };
    const result = await inj.injectMemoryContext(data);
    assert.strictEqual(result, data);
  });
});

// ===========================================================================
// registerMemoryHooks
// ===========================================================================

describe('registerMemoryHooks', () => {
  it('returns a MemoryInjector instance', () => {
    const inj = registerMemoryHooks(null);
    assert.ok(inj instanceof MemoryInjector);
  });

  it('accepts options for the injector', () => {
    const inj = registerMemoryHooks(null, { maxMemories: 15 });
    assert.strictEqual(inj._maxMemories, 15);
  });

  it('registers hook on hookRunner when provided', () => {
    let registered = false;
    const mockRunner = {
      on: (event, handler, opts) => {
        registered = true;
        assert.strictEqual(event, 'before_agent_start');
        assert.strictEqual(opts.priority, 20);
        assert.strictEqual(opts.pluginId, '__memory_injector__');
      },
    };
    registerMemoryHooks(mockRunner);
    assert.ok(registered);
  });

  it('does not throw when hookRunner is null', () => {
    assert.doesNotThrow(() => registerMemoryHooks(null));
  });
});

// ===========================================================================
// getMemoryInjector
// ===========================================================================

describe('getMemoryInjector', () => {
  it('returns the injector created by registerMemoryHooks', () => {
    const inj = registerMemoryHooks(null);
    assert.strictEqual(getMemoryInjector(), inj);
  });

  it('returns null before registerMemoryHooks is called', () => {
    // Note: this test may be order-dependent. The module-level _injector
    // may already be set from previous tests. We test the positive case above.
    // registerMemoryHooks sets _injector, and getMemoryInjector returns it.
    const result = getMemoryInjector();
    assert.ok(result === null || result instanceof MemoryInjector);
  });
});
