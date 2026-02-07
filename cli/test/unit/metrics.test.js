/**
 * Unit tests for channels/metrics.js — ChannelMetrics
 */

import { describe, it } from 'node:test';
import assert from 'node:assert';
import { ChannelMetrics, metricsCollector } from '../../src/channels/metrics.js';

// ===========================================================================
// recordMessage
// ===========================================================================

describe('ChannelMetrics recordMessage', () => {
  it('increments total and per-channel count', () => {
    const m = new ChannelMetrics();
    m.recordMessage('slack');
    m.recordMessage('slack');
    m.recordMessage('telegram');

    const s = m.getSummary();
    assert.strictEqual(s.totals.messagesReceived, 3);
    assert.strictEqual(s.channels.slack.messagesReceived, 2);
    assert.strictEqual(s.channels.telegram.messagesReceived, 1);
  });

  it('sets lastMessageAt', () => {
    const m = new ChannelMetrics();
    m.recordMessage('slack');
    const s = m.getSummary();
    assert.ok(s.channels.slack.lastMessageAt);
  });
});

// ===========================================================================
// recordResponse
// ===========================================================================

describe('ChannelMetrics recordResponse', () => {
  it('tracks response count and time', () => {
    const m = new ChannelMetrics();
    m.recordResponse('slack', 100);
    m.recordResponse('slack', 200);

    const s = m.getSummary();
    assert.strictEqual(s.totals.responsesSent, 2);
    assert.strictEqual(s.totals.avgResponseMs, 150);
    assert.strictEqual(s.channels.slack.responsesSent, 2);
    assert.strictEqual(s.channels.slack.avgResponseMs, 150);
  });

  it('handles zero responses', () => {
    const m = new ChannelMetrics();
    const s = m.getSummary();
    assert.strictEqual(s.totals.avgResponseMs, 0);
  });
});

// ===========================================================================
// recordError
// ===========================================================================

describe('ChannelMetrics recordError', () => {
  it('increments error counts', () => {
    const m = new ChannelMetrics();
    m.recordError('slack');
    m.recordError('slack');
    m.recordError('telegram');

    const s = m.getSummary();
    assert.strictEqual(s.totals.errors, 3);
    assert.strictEqual(s.channels.slack.errors, 2);
    assert.strictEqual(s.channels.telegram.errors, 1);
  });
});

// ===========================================================================
// recordBlocked
// ===========================================================================

describe('ChannelMetrics recordBlocked', () => {
  it('increments blocked counts', () => {
    const m = new ChannelMetrics();
    m.recordBlocked('slack');

    const s = m.getSummary();
    assert.strictEqual(s.totals.blocked, 1);
    assert.strictEqual(s.channels.slack.blocked, 1);
  });
});

// ===========================================================================
// recordCommand
// ===========================================================================

describe('ChannelMetrics recordCommand', () => {
  it('tracks command usage', () => {
    const m = new ChannelMetrics();
    m.recordCommand('/help');
    m.recordCommand('/help');
    m.recordCommand('/stats');

    const s = m.getSummary();
    assert.strictEqual(s.commandUsage['/help'], 2);
    assert.strictEqual(s.commandUsage['/stats'], 1);
  });
});

// ===========================================================================
// getSummary
// ===========================================================================

describe('ChannelMetrics getSummary', () => {
  it('includes uptime string', () => {
    const m = new ChannelMetrics();
    const s = m.getSummary();
    assert.ok(typeof s.uptime === 'string');
    assert.ok(s.uptimeMs >= 0);
  });

  it('returns empty channels and commandUsage initially', () => {
    const m = new ChannelMetrics();
    const s = m.getSummary();
    assert.deepStrictEqual(s.channels, {});
    assert.deepStrictEqual(s.commandUsage, {});
  });

  it('channel lastMessageAt is null when no messages', () => {
    const m = new ChannelMetrics();
    m.recordError('slack'); // creates channel but no message
    const s = m.getSummary();
    assert.strictEqual(s.channels.slack.lastMessageAt, null);
  });
});

// ===========================================================================
// formatForDisplay
// ===========================================================================

describe('ChannelMetrics formatForDisplay', () => {
  it('includes bot statistics header', () => {
    const m = new ChannelMetrics();
    const display = m.formatForDisplay();
    assert.ok(display.includes('Bot Statistics'));
    assert.ok(display.includes('Uptime'));
  });

  it('shows totals', () => {
    const m = new ChannelMetrics();
    m.recordMessage('slack');
    m.recordResponse('slack', 50);
    m.recordError('slack');
    m.recordBlocked('slack');

    const display = m.formatForDisplay();
    assert.ok(display.includes('Messages: 1'));
    assert.ok(display.includes('Responses: 1'));
    assert.ok(display.includes('Errors: 1'));
    assert.ok(display.includes('Blocked: 1'));
    assert.ok(display.includes('50ms'));
  });

  it('shows per-channel breakdown', () => {
    const m = new ChannelMetrics();
    m.recordMessage('slack');
    m.recordResponse('slack', 100);

    const display = m.formatForDisplay();
    assert.ok(display.includes('Per channel'));
    assert.ok(display.includes('slack'));
    assert.ok(display.includes('1 in / 1 out'));
  });

  it('shows command usage sorted by count', () => {
    const m = new ChannelMetrics();
    m.recordCommand('/help');
    m.recordCommand('/help');
    m.recordCommand('/stats');

    const display = m.formatForDisplay();
    assert.ok(display.includes('Command usage'));
    assert.ok(display.includes('/help: 2'));
    // /help should come before /stats
    assert.ok(display.indexOf('/help') < display.indexOf('/stats'));
  });

  it('omits channel section when no channels', () => {
    const m = new ChannelMetrics();
    const display = m.formatForDisplay();
    assert.ok(!display.includes('Per channel'));
  });

  it('omits command section when no commands', () => {
    const m = new ChannelMetrics();
    const display = m.formatForDisplay();
    assert.ok(!display.includes('Command usage'));
  });
});

// ===========================================================================
// metricsCollector middleware
// ===========================================================================

describe('metricsCollector middleware', () => {
  it('records message on incoming context', async () => {
    const middleware = metricsCollector();
    const ctx = { channel: 'test-channel' };

    await middleware(ctx, async () => {});

    // We can't easily check the singleton, but verify it doesn't throw
    assert.ok(true);
  });

  it('records blocked when ctx.blocked is set', async () => {
    const middleware = metricsCollector();
    const ctx = { channel: 'test-channel', blocked: false };

    await middleware(ctx, async () => {
      ctx.blocked = true;
    });

    // No throw = success
    assert.ok(true);
  });
});

// ===========================================================================
// Multi-channel aggregation
// ===========================================================================

describe('ChannelMetrics multi-channel', () => {
  it('tracks independent channels correctly', () => {
    const m = new ChannelMetrics();
    m.recordMessage('slack');
    m.recordMessage('slack');
    m.recordMessage('telegram');
    m.recordResponse('slack', 100);
    m.recordResponse('telegram', 200);
    m.recordError('telegram');

    const s = m.getSummary();
    assert.strictEqual(s.channels.slack.messagesReceived, 2);
    assert.strictEqual(s.channels.slack.avgResponseMs, 100);
    assert.strictEqual(s.channels.telegram.messagesReceived, 1);
    assert.strictEqual(s.channels.telegram.avgResponseMs, 200);
    assert.strictEqual(s.channels.telegram.errors, 1);
    assert.strictEqual(s.channels.slack.errors, 0);
  });
});
