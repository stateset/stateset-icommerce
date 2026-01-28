/**
 * Conversation Metrics for StateSet Channel Gateways
 *
 * Lightweight in-process counters for monitoring bot performance
 * across channels. Collects message counts, response times, errors,
 * and command usage.
 *
 * Designed to run as middleware so every message is automatically tracked.
 */

// ============================================================================
// ChannelMetrics
// ============================================================================

export class ChannelMetrics {
  constructor() {
    this._startedAt = Date.now();

    /** @type {Map<string, ChannelStats>} */
    this._channels = new Map();

    /** @type {Map<string, number>} */
    this._commandUsage = new Map();

    this._totals = {
      messagesReceived: 0,
      responsesSent: 0,
      errors: 0,
      blocked: 0,
      totalResponseTimeMs: 0,
    };
  }

  /**
   * Get or create stats for a channel.
   * @param {string} channel
   * @returns {ChannelStats}
   */
  _channel(channel) {
    let stats = this._channels.get(channel);
    if (!stats) {
      stats = {
        messagesReceived: 0,
        responsesSent: 0,
        errors: 0,
        blocked: 0,
        totalResponseTimeMs: 0,
        lastMessageAt: null,
      };
      this._channels.set(channel, stats);
    }
    return stats;
  }

  /**
   * Record an incoming message.
   */
  recordMessage(channel) {
    this._totals.messagesReceived++;
    const ch = this._channel(channel);
    ch.messagesReceived++;
    ch.lastMessageAt = Date.now();
  }

  /**
   * Record a response sent.
   */
  recordResponse(channel, responseTimeMs) {
    this._totals.responsesSent++;
    this._totals.totalResponseTimeMs += responseTimeMs;
    const ch = this._channel(channel);
    ch.responsesSent++;
    ch.totalResponseTimeMs += responseTimeMs;
  }

  /**
   * Record an error.
   */
  recordError(channel) {
    this._totals.errors++;
    this._channel(channel).errors++;
  }

  /**
   * Record a blocked message.
   */
  recordBlocked(channel) {
    this._totals.blocked++;
    this._channel(channel).blocked++;
  }

  /**
   * Record a command usage.
   */
  recordCommand(command) {
    const current = this._commandUsage.get(command) || 0;
    this._commandUsage.set(command, current + 1);
  }

  /**
   * Get a summary of all metrics.
   */
  getSummary() {
    const uptimeMs = Date.now() - this._startedAt;
    const avgResponseMs = this._totals.responsesSent > 0
      ? Math.round(this._totals.totalResponseTimeMs / this._totals.responsesSent)
      : 0;

    const channels = {};
    for (const [name, stats] of this._channels) {
      const chAvg = stats.responsesSent > 0
        ? Math.round(stats.totalResponseTimeMs / stats.responsesSent)
        : 0;
      channels[name] = {
        messagesReceived: stats.messagesReceived,
        responsesSent: stats.responsesSent,
        errors: stats.errors,
        blocked: stats.blocked,
        avgResponseMs: chAvg,
        lastMessageAt: stats.lastMessageAt ? new Date(stats.lastMessageAt).toISOString() : null,
      };
    }

    const commandUsage = {};
    for (const [cmd, count] of this._commandUsage) {
      commandUsage[cmd] = count;
    }

    return {
      uptime: formatUptime(uptimeMs),
      uptimeMs,
      totals: {
        messagesReceived: this._totals.messagesReceived,
        responsesSent: this._totals.responsesSent,
        errors: this._totals.errors,
        blocked: this._totals.blocked,
        avgResponseMs,
      },
      channels,
      commandUsage,
    };
  }

  /**
   * Format metrics as a human-readable string for the /stats command.
   */
  formatForDisplay() {
    const s = this.getSummary();
    const lines = [];

    lines.push('Bot Statistics');
    lines.push(`Uptime: ${s.uptime}`);
    lines.push('');

    lines.push('Totals:');
    lines.push(`  Messages: ${s.totals.messagesReceived}`);
    lines.push(`  Responses: ${s.totals.responsesSent}`);
    lines.push(`  Errors: ${s.totals.errors}`);
    lines.push(`  Blocked: ${s.totals.blocked}`);
    lines.push(`  Avg response: ${s.totals.avgResponseMs}ms`);

    if (Object.keys(s.channels).length > 0) {
      lines.push('');
      lines.push('Per channel:');
      for (const [name, ch] of Object.entries(s.channels)) {
        lines.push(`  ${name}: ${ch.messagesReceived} in / ${ch.responsesSent} out (avg ${ch.avgResponseMs}ms)`);
      }
    }

    if (Object.keys(s.commandUsage).length > 0) {
      lines.push('');
      lines.push('Command usage:');
      const sorted = Object.entries(s.commandUsage).sort((a, b) => b[1] - a[1]);
      for (const [cmd, count] of sorted.slice(0, 10)) {
        lines.push(`  ${cmd}: ${count}`);
      }
    }

    return lines.join('\n');
  }
}

// ============================================================================
// Singleton
// ============================================================================

let _instance = null;

/**
 * Get the global ChannelMetrics instance.
 * @returns {ChannelMetrics}
 */
export function getMetrics() {
  if (!_instance) {
    _instance = new ChannelMetrics();
  }
  return _instance;
}

// ============================================================================
// Middleware
// ============================================================================

/**
 * Create a metrics collector middleware.
 * Records message counts per channel. Response time tracking requires
 * wrapping at the processSingle level (done in base.js).
 *
 * @returns {Function}
 */
export function metricsCollector() {
  return async function metricsCollectorMiddleware(ctx, next) {
    const metrics = getMetrics();
    metrics.recordMessage(ctx.channel);

    await next();

    if (ctx.blocked) {
      metrics.recordBlocked(ctx.channel);
    }
  };
}

// ============================================================================
// Helpers
// ============================================================================

function formatUptime(ms) {
  const seconds = Math.floor(ms / 1000);
  const minutes = Math.floor(seconds / 60);
  const hours = Math.floor(minutes / 60);
  const days = Math.floor(hours / 24);

  if (days > 0) return `${days}d ${hours % 24}h ${minutes % 60}m`;
  if (hours > 0) return `${hours}h ${minutes % 60}m`;
  if (minutes > 0) return `${minutes}m ${seconds % 60}s`;
  return `${seconds}s`;
}
