/**
 * Block Reply Pipeline for StateSet iCommerce
 *
 * Manages outbound message delivery with:
 * - Deduplication (prevent duplicate sends)
 * - Buffering (coalesce rapid messages)
 * - Timeout handling with abort signals
 * - Streaming response coalescing
 * - Rate limit awareness
 *
 * Inspired by moltbot's block-reply-pipeline.
 */

// ============================================================================
// Types
// ============================================================================

/**
 * @typedef {Object} ReplyPayload
 * @property {string} targetId - Chat/channel ID to send to
 * @property {string} text - Message text
 * @property {import('./rich-messages.js').RichMessage} [richMessage] - Rich message
 * @property {string} [replyTo] - Message ID to reply to
 * @property {boolean} [silent=false] - No notification
 * @property {string} [key] - Dedup key (auto-generated if not provided)
 */

/**
 * @typedef {Object} PipelineOptions
 * @property {(payload: ReplyPayload, opts?: object) => Promise<void>} onBlockReply - Send callback
 * @property {number} [bufferMs=0] - Buffer time in ms (0 = no buffering)
 * @property {number} [timeoutMs=30000] - Max time to wait for buffered messages
 * @property {number} [rateLimitMs=0] - Minimum ms between sends to same target
 * @property {boolean} [dedup=true] - Enable deduplication
 * @property {number} [dedupWindowMs=5000] - Dedup window
 * @property {StreamCoalescing} [coalescing] - Streaming coalescing config
 */

/**
 * @typedef {Object} StreamCoalescing
 * @property {boolean} enabled - Whether to coalesce streaming chunks
 * @property {number} [flushIntervalMs=500] - Flush interval for buffered chunks
 * @property {string} [separator=''] - Separator between coalesced chunks
 * @property {(chunks: string[]) => string} [coalescer] - Custom coalesce function
 */

// ============================================================================
// Key Generation
// ============================================================================

/**
 * Generate a dedup key from payload.
 *
 * @param {ReplyPayload} payload
 * @returns {string}
 */
function generateKey(payload) {
  if (payload.key) return payload.key;
  return `${payload.targetId}:${hashText(payload.text)}`;
}

/**
 * Simple hash for dedup purposes (not cryptographic).
 *
 * @param {string} text
 * @returns {string}
 */
function hashText(text) {
  let hash = 0;
  for (let i = 0; i < text.length; i++) {
    const ch = text.charCodeAt(i);
    hash = ((hash << 5) - hash) + ch;
    hash = hash & hash; // Convert to 32-bit integer
  }
  return hash.toString(36);
}

// ============================================================================
// ReplyPipeline
// ============================================================================

export class ReplyPipeline {
  /**
   * @param {PipelineOptions} opts
   */
  constructor(opts) {
    this._onBlockReply = opts.onBlockReply || opts.sendFn;
    this._sendFnStyle = !opts.onBlockReply && !!opts.sendFn;
    this._bufferMs = opts.bufferMs ?? (opts.buffer?.enabled ? (opts.buffer.ms ?? 200) : 0);
    this._timeoutMs = opts.timeoutMs ?? 30000;
    this._rateLimitMs = opts.rateLimitMs ?? (opts.rateLimit?.enabled ? (opts.rateLimit.ms ?? 100) : 0);
    this._coalescing = opts.coalescing || null;

    // Dedup config: accept boolean, object { enabled, windowMs }, or default
    if (typeof opts.dedup === 'object' && opts.dedup !== null) {
      this._dedup = opts.dedup.enabled !== false;
      this._dedupWindowMs = opts.dedup.windowMs ?? 5000;
    } else {
      this._dedup = opts.dedup !== false;
      this._dedupWindowMs = opts.dedupWindowMs ?? 5000;
    }

    /** @type {Map<string, number>} - Key -> timestamp of last send */
    this._seen = new Map();

    /** @type {Map<string, ReplyPayload[]>} - TargetId -> buffered payloads */
    this._buffer = new Map();

    /** @type {Map<string, NodeJS.Timeout>} - TargetId -> buffer flush timer */
    this._bufferTimers = new Map();

    /** @type {Map<string, number>} - TargetId -> last send timestamp */
    this._lastSend = new Map();

    /** @type {Map<string, StreamBuffer>} - TargetId -> active stream buffer */
    this._streams = new Map();

    /** @type {{ sent: number, deduped: number, errors: number }} - Counters */
    this._counters = { sent: 0, deduped: 0, errors: 0 };

    // Cleanup old dedup keys periodically
    this._cleanupInterval = setInterval(() => this._cleanupSeen(), this._dedupWindowMs * 2);
  }

  /**
   * Queue a reply payload for delivery.
   *
   * @param {ReplyPayload} payload
   * @param {Object} [opts]
   * @param {AbortSignal} [opts.signal] - Abort signal
   * @returns {Promise<{ sent: boolean, reason?: string }>}
   */
  async send(payload, opts = {}) {
    // Check abort
    if (opts.signal?.aborted) {
      return { sent: false, reason: 'aborted' };
    }

    // Dedup check
    if (this._dedup) {
      const key = generateKey(payload);
      const lastSeen = this._seen.get(key);
      if (lastSeen && Date.now() - lastSeen < this._dedupWindowMs) {
        this._counters.deduped++;
        return { sent: false, reason: 'duplicate' };
      }
      this._seen.set(key, Date.now());
    }

    // If buffering enabled, add to buffer
    if (this._bufferMs > 0) {
      return this._addToBuffer(payload, opts);
    }

    // Direct send (with rate limiting)
    return this._sendWithRateLimit(payload, opts);
  }

  /**
   * Send multiple payloads.
   *
   * @param {ReplyPayload[]} payloads
   * @param {Object} [opts]
   * @returns {Promise<Array<{ sent: boolean, reason?: string }>>}
   */
  async sendAll(payloads, opts = {}) {
    const results = [];
    for (const payload of payloads) {
      results.push(await this.send(payload, opts));
    }
    return results;
  }

  // ============================================================================
  // Streaming
  // ============================================================================

  /**
   * Start a streaming session for coalescing chunks.
   *
   * @param {string} targetId
   * @returns {StreamSession}
   */
  startStream(targetId) {
    if (this._streams.has(targetId)) {
      // Flush existing stream
      this._flushStream(targetId);
    }

    const session = new StreamSession(targetId, this);
    this._streams.set(targetId, session);
    return session;
  }

  /**
   * Flush a streaming session.
   * @private
   */
  async _flushStream(targetId) {
    const session = this._streams.get(targetId);
    if (!session) return;

    const text = session.flush();
    if (text) {
      await this._sendDirect({ targetId, text });
    }

    this._streams.delete(targetId);
  }

  // ============================================================================
  // Buffering
  // ============================================================================

  /**
   * Add payload to buffer and schedule flush.
   * @private
   */
  async _addToBuffer(payload, opts) {
    const { targetId } = payload;

    if (!this._buffer.has(targetId)) {
      this._buffer.set(targetId, []);
    }

    this._buffer.get(targetId).push(payload);

    // Schedule flush if not already scheduled
    if (!this._bufferTimers.has(targetId)) {
      const timer = setTimeout(() => {
        this._flushBuffer(targetId, opts);
      }, this._bufferMs);

      this._bufferTimers.set(targetId, timer);

      // Also set a hard timeout
      if (this._timeoutMs > 0) {
        setTimeout(() => {
          this._flushBuffer(targetId, opts);
        }, this._timeoutMs);
      }
    }

    return { sent: true, reason: 'buffered' };
  }

  /**
   * Flush buffered payloads for a target.
   * @private
   */
  async _flushBuffer(targetId, opts = {}) {
    const timer = this._bufferTimers.get(targetId);
    if (timer) {
      clearTimeout(timer);
      this._bufferTimers.delete(targetId);
    }

    const payloads = this._buffer.get(targetId) || [];
    this._buffer.delete(targetId);

    if (payloads.length === 0) return;

    // Coalesce payloads if configured
    if (this._coalescing?.enabled && payloads.length > 1) {
      const separator = this._coalescing.separator || '\n';
      const texts = payloads.map((p) => p.text);
      const coalesced = this._coalescing.coalescer
        ? this._coalescing.coalescer(texts)
        : texts.join(separator);

      await this._sendWithRateLimit({
        targetId,
        text: coalesced,
        richMessage: payloads[payloads.length - 1].richMessage,
      }, opts);
    } else {
      // Send individually
      for (const payload of payloads) {
        await this._sendWithRateLimit(payload, opts);
      }
    }
  }

  // ============================================================================
  // Rate Limiting
  // ============================================================================

  /**
   * Send with rate limit awareness.
   * @private
   */
  async _sendWithRateLimit(payload, opts = {}) {
    if (opts.signal?.aborted) {
      return { sent: false, reason: 'aborted' };
    }

    if (this._rateLimitMs > 0) {
      const lastSend = this._lastSend.get(payload.targetId) || 0;
      const elapsed = Date.now() - lastSend;

      if (elapsed < this._rateLimitMs) {
        const wait = this._rateLimitMs - elapsed;
        await new Promise((resolve) => setTimeout(resolve, wait));
      }
    }

    try {
      await this._sendDirect(payload);
      this._lastSend.set(payload.targetId, Date.now());
      return { sent: true };
    } catch (err) {
      this._counters.errors++;
      return { sent: false, reason: err.message };
    }
  }

  /**
   * Direct send through the callback.
   * @private
   */
  async _sendDirect(payload) {
    if (this._sendFnStyle) {
      // sendFn(targetId, text, opts) convenience signature
      await this._onBlockReply(payload.targetId, payload.text, {
        replyTo: payload.replyTo,
        silent: payload.silent,
        richMessage: payload.richMessage,
      });
    } else {
      await this._onBlockReply(payload, {
        replyTo: payload.replyTo,
        silent: payload.silent,
      });
    }
    this._counters.sent++;
  }

  // ============================================================================
  // Cleanup
  // ============================================================================

  /**
   * Cleanup old dedup keys.
   * @private
   */
  _cleanupSeen() {
    const cutoff = Date.now() - this._dedupWindowMs;
    for (const [key, timestamp] of this._seen) {
      if (timestamp < cutoff) {
        this._seen.delete(key);
      }
    }
  }

  /**
   * Flush all buffers and streams.
   */
  async flush() {
    // Flush all buffers
    for (const targetId of [...this._buffer.keys()]) {
      await this._flushBuffer(targetId);
    }

    // Flush all streams
    for (const targetId of [...this._streams.keys()]) {
      await this._flushStream(targetId);
    }
  }

  /**
   * Stop the pipeline and clean up resources.
   */
  async shutdown() {
    await this.flush();

    clearInterval(this._cleanupInterval);

    for (const timer of this._bufferTimers.values()) {
      clearTimeout(timer);
    }
    this._bufferTimers.clear();
    this._buffer.clear();
    this._seen.clear();
    this._lastSend.clear();
    this._streams.clear();
  }

  /**
   * Get pipeline statistics.
   *
   * @returns {{ dedupKeys: number, buffered: number, activeStreams: number }}
   */
  getStats() {
    let buffered = 0;
    for (const payloads of this._buffer.values()) {
      buffered += payloads.length;
    }

    return {
      totalSent: this._counters.sent,
      totalDeduped: this._counters.deduped,
      totalErrors: this._counters.errors,
      dedupKeys: this._seen.size,
      buffered,
      activeStreams: this._streams.size,
    };
  }
}

// ============================================================================
// StreamSession
// ============================================================================

/**
 * A streaming session that buffers chunks and flushes periodically.
 */
class StreamSession {
  /**
   * @param {string} targetId
   * @param {ReplyPipeline} pipeline
   */
  constructor(targetId, pipeline) {
    this._targetId = targetId;
    this._pipeline = pipeline;
    this._chunks = [];
    this._flushed = false;
    this._flushTimer = null;

    const interval = pipeline._coalescing?.flushIntervalMs || 500;
    this._flushTimer = setInterval(() => this._periodicFlush(), interval);
  }

  /**
   * Write a chunk to the stream.
   *
   * @param {string} chunk
   */
  async write(chunk) {
    if (this._flushed) {
      throw new Error('Stream already ended');
    }
    this._chunks.push(chunk);
  }

  /**
   * End the stream and flush remaining content.
   *
   * @returns {string} - Full accumulated text
   */
  async end() {
    if (this._flushed) return '';

    this._flushed = true;

    if (this._flushTimer) {
      clearInterval(this._flushTimer);
      this._flushTimer = null;
    }

    const text = this.flush();
    if (text) {
      await this._pipeline._sendDirect({ targetId: this._targetId, text });
    }

    this._pipeline._streams.delete(this._targetId);
    return text;
  }

  /**
   * Abort the stream without sending.
   */
  abort() {
    this._flushed = true;
    this._chunks = [];

    if (this._flushTimer) {
      clearInterval(this._flushTimer);
      this._flushTimer = null;
    }

    this._pipeline._streams.delete(this._targetId);
  }

  /**
   * Flush accumulated chunks and return combined text.
   *
   * @returns {string}
   */
  flush() {
    if (this._chunks.length === 0) return '';

    const separator = this._pipeline._coalescing?.separator || '';
    const coalescer = this._pipeline._coalescing?.coalescer;

    const text = coalescer
      ? coalescer(this._chunks)
      : this._chunks.join(separator);

    this._chunks = [];
    return text;
  }

  /**
   * Periodic flush for streaming.
   * @private
   */
  async _periodicFlush() {
    const text = this.flush();
    if (text) {
      try {
        await this._pipeline._sendDirect({ targetId: this._targetId, text });
      } catch (err) {
        console.error(`[StreamSession] Flush error for ${this._targetId}:`, err.message);
      }
    }
  }
}

// ============================================================================
// Factory
// ============================================================================

/**
 * Create a reply pipeline.
 *
 * @param {PipelineOptions} opts
 * @returns {ReplyPipeline}
 */
export function createReplyPipeline(opts) {
  return new ReplyPipeline(opts);
}
