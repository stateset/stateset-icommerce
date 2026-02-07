/**
 * Unit tests for channels/middleware.js
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import {
  runMiddleware,
  rateLimiter,
  messageLogger,
  contentFilter,
  autoLanguageDetect,
} from '../../src/channels/middleware.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeCtx(overrides = {}) {
  return {
    channel: 'telegram',
    senderId: 'user-1',
    text: 'Hello world',
    blocked: false,
    blockReason: null,
    metadata: {},
    ...overrides,
  };
}

// ===========================================================================
// runMiddleware
// ===========================================================================

describe('runMiddleware', () => {
  it('runs middleware in order', async () => {
    const order = [];
    const stack = [
      async (ctx, next) => {
        order.push('a-before');
        await next();
        order.push('a-after');
      },
      async (ctx, next) => {
        order.push('b-before');
        await next();
        order.push('b-after');
      },
    ];

    await runMiddleware(stack, {});
    assert.deepStrictEqual(order, ['a-before', 'b-before', 'b-after', 'a-after']);
  });

  it('handles empty stack', async () => {
    await runMiddleware([], {});
    // Should not throw
  });

  it('short-circuits when next is not called', async () => {
    const order = [];
    const stack = [
      async (ctx, next) => {
        order.push('a');
        // Do NOT call next()
      },
      async (ctx, next) => {
        order.push('b');
        await next();
      },
    ];

    await runMiddleware(stack, {});
    assert.deepStrictEqual(order, ['a']);
  });

  it('throws on double next() call', async () => {
    const stack = [
      async (ctx, next) => {
        await next();
        await next(); // Should throw
      },
    ];

    await assert.rejects(() => runMiddleware(stack, {}), /next\(\) called multiple times/);
  });

  it('propagates errors from middleware', async () => {
    const stack = [
      async () => {
        throw new Error('middleware error');
      },
    ];

    await assert.rejects(() => runMiddleware(stack, {}), /middleware error/);
  });

  it('passes context to all middleware', async () => {
    const stack = [
      async (ctx, next) => {
        ctx.first = true;
        await next();
      },
      async (ctx, next) => {
        ctx.second = true;
        await next();
      },
    ];

    const ctx = {};
    await runMiddleware(stack, ctx);
    assert.strictEqual(ctx.first, true);
    assert.strictEqual(ctx.second, true);
  });
});

// ===========================================================================
// rateLimiter
// ===========================================================================

describe('rateLimiter', () => {
  it('allows messages within limit', async () => {
    const limiter = rateLimiter({ maxPerMinute: 5, maxPerHour: 100 });
    const ctx = makeCtx();
    await limiter(ctx, async () => {});
    assert.strictEqual(ctx.blocked, false);
  });

  it('blocks when per-minute limit exceeded', async () => {
    const limiter = rateLimiter({ maxPerMinute: 3, maxPerHour: 100 });

    for (let i = 0; i < 3; i++) {
      const ctx = makeCtx();
      await limiter(ctx, async () => {});
      assert.strictEqual(ctx.blocked, false);
    }

    const ctx = makeCtx();
    await limiter(ctx, async () => {});
    assert.strictEqual(ctx.blocked, true);
    assert.ok(ctx.blockReason.includes('per minute'));
  });

  it('blocks when per-hour limit exceeded', async () => {
    const limiter = rateLimiter({ maxPerMinute: 100, maxPerHour: 5 });

    for (let i = 0; i < 5; i++) {
      const ctx = makeCtx();
      await limiter(ctx, async () => {});
    }

    const ctx = makeCtx();
    await limiter(ctx, async () => {});
    assert.strictEqual(ctx.blocked, true);
    assert.ok(ctx.blockReason.includes('per hour'));
  });

  it('tracks different senders independently', async () => {
    const limiter = rateLimiter({ maxPerMinute: 2, maxPerHour: 100 });

    // Fill up user-1
    for (let i = 0; i < 2; i++) {
      const ctx = makeCtx({ senderId: 'user-1' });
      await limiter(ctx, async () => {});
    }

    // user-2 should still be allowed
    const ctx = makeCtx({ senderId: 'user-2' });
    await limiter(ctx, async () => {});
    assert.strictEqual(ctx.blocked, false);
  });

  it('does not call next when blocked', async () => {
    const limiter = rateLimiter({ maxPerMinute: 1, maxPerHour: 100 });
    let nextCalled = false;

    // Fill up
    const ctx1 = makeCtx();
    await limiter(ctx1, async () => {});

    // This one should be blocked
    const ctx2 = makeCtx();
    await limiter(ctx2, async () => {
      nextCalled = true;
    });
    assert.strictEqual(ctx2.blocked, true);
    assert.strictEqual(nextCalled, false);
  });
});

// ===========================================================================
// messageLogger
// ===========================================================================

describe('messageLogger', () => {
  it('logs incoming message', async () => {
    const logs = [];
    const mw = messageLogger({ logFn: (msg) => logs.push(msg) });
    const ctx = makeCtx({ text: 'Test message' });

    await mw(ctx, async () => {});

    assert.strictEqual(logs.length, 1);
    assert.ok(logs[0].includes('IN'));
    assert.ok(logs[0].includes('Test message'));
    assert.ok(logs[0].includes('telegram'));
  });

  it('logs blocked messages', async () => {
    const logs = [];
    const mw = messageLogger({ logFn: (msg) => logs.push(msg) });
    const ctx = makeCtx();

    await mw(ctx, async () => {
      ctx.blocked = true;
      ctx.blockReason = 'Rate limited';
    });

    assert.strictEqual(logs.length, 2);
    assert.ok(logs[1].includes('BLOCKED'));
    assert.ok(logs[1].includes('Rate limited'));
  });

  it('truncates long messages in log', async () => {
    const logs = [];
    const mw = messageLogger({ logFn: (msg) => logs.push(msg) });
    const longText = 'A'.repeat(200);
    const ctx = makeCtx({ text: longText });

    await mw(ctx, async () => {});

    // The log should contain at most 120 chars of the message
    assert.ok(logs[0].includes('A'.repeat(120)));
    assert.ok(!logs[0].includes('A'.repeat(200)));
  });
});

// ===========================================================================
// contentFilter
// ===========================================================================

describe('contentFilter', () => {
  it('blocks messages matching wordlist', async () => {
    const mw = contentFilter({ wordlist: ['spam', 'scam'] });
    const ctx = makeCtx({ text: 'This is a spam message' });

    await mw(ctx, async () => {});

    assert.strictEqual(ctx.blocked, true);
    assert.ok(ctx.blockReason.includes('content filter'));
  });

  it('allows messages not matching wordlist', async () => {
    const mw = contentFilter({ wordlist: ['spam', 'scam'] });
    const ctx = makeCtx({ text: 'Legitimate message' });
    let nextCalled = false;

    await mw(ctx, async () => {
      nextCalled = true;
    });

    assert.strictEqual(ctx.blocked, false);
    assert.strictEqual(nextCalled, true);
  });

  it('uses word boundaries', async () => {
    const mw = contentFilter({ wordlist: ['ham'] });
    const ctx = makeCtx({ text: 'This is a shamble' });
    let nextCalled = false;

    await mw(ctx, async () => {
      nextCalled = true;
    });

    // "ham" should not match inside "shamble" due to word boundary
    assert.strictEqual(ctx.blocked, false);
    assert.strictEqual(nextCalled, true);
  });

  it('is case-insensitive', async () => {
    const mw = contentFilter({ wordlist: ['spam'] });
    const ctx = makeCtx({ text: 'This is SPAM' });

    await mw(ctx, async () => {});
    assert.strictEqual(ctx.blocked, true);
  });

  it('warn action sets metadata but continues', async () => {
    const mw = contentFilter({ wordlist: ['spam'], action: 'warn' });
    const ctx = makeCtx({ text: 'This is spam' });
    let nextCalled = false;

    await mw(ctx, async () => {
      nextCalled = true;
    });

    assert.strictEqual(ctx.blocked, false);
    assert.strictEqual(ctx.metadata.contentWarning, true);
    assert.strictEqual(nextCalled, true);
  });

  it('calls onMatch callback', async () => {
    let matchInfo = null;
    const mw = contentFilter({
      wordlist: ['spam'],
      onMatch: (info) => {
        matchInfo = info;
      },
    });
    const ctx = makeCtx({ text: 'This is spam' });

    await mw(ctx, async () => {});

    assert.ok(matchInfo);
    assert.strictEqual(matchInfo.senderId, 'user-1');
    assert.ok(matchInfo.pattern.includes('spam'));
  });

  it('empty wordlist passes through', async () => {
    const mw = contentFilter({ wordlist: [] });
    const ctx = makeCtx();
    let nextCalled = false;

    await mw(ctx, async () => {
      nextCalled = true;
    });

    assert.strictEqual(nextCalled, true);
  });
});

// ===========================================================================
// autoLanguageDetect
// ===========================================================================

describe('autoLanguageDetect', () => {
  it('detects latin text', async () => {
    const mw = autoLanguageDetect();
    const ctx = makeCtx({ text: 'Hello world' });

    await mw(ctx, async () => {});
    assert.strictEqual(ctx.metadata.detectedLanguage, 'latin');
  });

  it('detects CJK text', async () => {
    const mw = autoLanguageDetect();
    const ctx = makeCtx({ text: '\u4F60\u597D\u4E16\u754C' }); // 你好世界

    await mw(ctx, async () => {});
    assert.strictEqual(ctx.metadata.detectedLanguage, 'cjk');
  });

  it('detects Cyrillic text', async () => {
    const mw = autoLanguageDetect();
    const ctx = makeCtx({ text: '\u041F\u0440\u0438\u0432\u0435\u0442' }); // Привет

    await mw(ctx, async () => {});
    assert.strictEqual(ctx.metadata.detectedLanguage, 'cyrillic');
  });

  it('detects Arabic text', async () => {
    const mw = autoLanguageDetect();
    const ctx = makeCtx({ text: '\u0645\u0631\u062D\u0628\u0627' }); // مرحبا

    await mw(ctx, async () => {});
    assert.strictEqual(ctx.metadata.detectedLanguage, 'arabic');
  });

  it('detects Thai text', async () => {
    const mw = autoLanguageDetect();
    const ctx = makeCtx({ text: '\u0E2A\u0E27\u0E31\u0E2A\u0E14\u0E35' }); // สวัสดี

    await mw(ctx, async () => {});
    assert.strictEqual(ctx.metadata.detectedLanguage, 'thai');
  });

  it('detects Devanagari text', async () => {
    const mw = autoLanguageDetect();
    const ctx = makeCtx({ text: '\u0928\u092E\u0938\u094D\u0924\u0947' }); // नमस्ते

    await mw(ctx, async () => {});
    assert.strictEqual(ctx.metadata.detectedLanguage, 'devanagari');
  });

  it('calls next()', async () => {
    const mw = autoLanguageDetect();
    const ctx = makeCtx();
    let nextCalled = false;

    await mw(ctx, async () => {
      nextCalled = true;
    });

    assert.strictEqual(nextCalled, true);
  });
});
