/**
 * Middleware Pipeline for StateSet Channel Gateways
 *
 * Provides composable middleware functions that run on every incoming
 * message before bot commands and agent processing.
 *
 * Middleware signature: async (context, next) => void
 * Uses Koa-style onion model — call next() to pass to the next middleware.
 *
 * Set context.blocked = true to halt processing.
 */

// ============================================================================
// Pipeline runner
// ============================================================================

/**
 * Run a middleware stack in Koa-style onion order.
 *
 * @param {Function[]} stack - Array of middleware functions
 * @param {Object} context - Mutable context object
 * @returns {Promise<void>}
 */
export async function runMiddleware(stack, context) {
  let index = -1;

  async function dispatch(i) {
    if (i <= index) throw new Error('next() called multiple times');
    index = i;
    if (i >= stack.length) return;
    const fn = stack[i];
    await fn(context, () => dispatch(i + 1));
  }

  await dispatch(0);
}

// ============================================================================
// Built-in middleware factories
// ============================================================================

/**
 * Rate limiter middleware using a token bucket per sender.
 *
 * @param {Object} [opts]
 * @param {number} [opts.maxPerMinute=20]
 * @param {number} [opts.maxPerHour=200]
 * @returns {Function}
 */
export function rateLimiter({ maxPerMinute = 20, maxPerHour = 200 } = {}) {
  // Track per-sender message timestamps
  const senderBuckets = new Map();

  // Periodic cleanup of stale entries
  const CLEANUP_INTERVAL = 10 * 60 * 1000;
  const cleanupTimer = setInterval(() => {
    const cutoff = Date.now() - 60 * 60 * 1000;
    for (const [id, bucket] of senderBuckets) {
      bucket.timestamps = bucket.timestamps.filter((t) => t > cutoff);
      if (bucket.timestamps.length === 0) senderBuckets.delete(id);
    }
  }, CLEANUP_INTERVAL);
  if (cleanupTimer.unref) cleanupTimer.unref();

  return async function rateLimiterMiddleware(ctx, next) {
    const now = Date.now();
    const oneMinuteAgo = now - 60 * 1000;
    const oneHourAgo = now - 60 * 60 * 1000;

    let bucket = senderBuckets.get(ctx.senderId);
    if (!bucket) {
      bucket = { timestamps: [] };
      senderBuckets.set(ctx.senderId, bucket);
    }

    // Prune old timestamps
    bucket.timestamps = bucket.timestamps.filter((t) => t > oneHourAgo);

    const minuteCount = bucket.timestamps.filter((t) => t > oneMinuteAgo).length;
    const hourCount = bucket.timestamps.length;

    if (minuteCount >= maxPerMinute) {
      ctx.blocked = true;
      ctx.blockReason = `Rate limit exceeded: ${maxPerMinute} messages per minute`;
      return;
    }

    if (hourCount >= maxPerHour) {
      ctx.blocked = true;
      ctx.blockReason = `Rate limit exceeded: ${maxPerHour} messages per hour`;
      return;
    }

    bucket.timestamps.push(now);
    await next();
  };
}

/**
 * Message logger middleware.
 *
 * @param {Object} [opts]
 * @param {Function} [opts.logFn=console.log] - Logging function
 * @returns {Function}
 */
export function messageLogger({ logFn = console.log } = {}) {
  return async function messageLoggerMiddleware(ctx, next) {
    const ts = new Date().toISOString();
    logFn(`[${ts}] [${ctx.channel}] IN  ${ctx.senderId}: ${ctx.text.slice(0, 120)}`);

    await next();

    if (ctx.blocked) {
      logFn(`[${ts}] [${ctx.channel}] BLOCKED ${ctx.senderId}: ${ctx.blockReason}`);
    }
  };
}

/**
 * Content filter middleware.
 * Checks message text against a wordlist using regex.
 *
 * @param {Object} opts
 * @param {string[]} opts.wordlist - Words/patterns to filter
 * @param {'block'|'warn'} [opts.action='block'] - Action on match
 * @param {Function} [opts.onMatch] - Callback when match found
 * @returns {Function}
 */
export function contentFilter({ wordlist = [], action = 'block', onMatch } = {}) {
  if (wordlist.length === 0) {
    return async (_ctx, next) => next();
  }

  const patterns = wordlist.map((w) => new RegExp(`\\b${w}\\b`, 'i'));

  return async function contentFilterMiddleware(ctx, next) {
    for (const pattern of patterns) {
      if (pattern.test(ctx.text)) {
        if (onMatch) onMatch({ senderId: ctx.senderId, pattern: pattern.source, text: ctx.text });

        if (action === 'block') {
          ctx.blocked = true;
          ctx.blockReason = 'Message blocked by content filter';
          return;
        }
        // 'warn' — add metadata but continue
        ctx.metadata.contentWarning = true;
        break;
      }
    }

    await next();
  };
}

/**
 * Auto language detection middleware.
 * Heuristic-based detection for CJK, Cyrillic, Arabic, and Latin scripts.
 * Sets ctx.metadata.detectedLanguage.
 *
 * @returns {Function}
 */
export function autoLanguageDetect() {
  const SCRIPT_RANGES = [
    { name: 'cjk', pattern: /[\u4E00-\u9FFF\u3040-\u309F\u30A0-\u30FF\uAC00-\uD7AF]/ },
    { name: 'cyrillic', pattern: /[\u0400-\u04FF]/ },
    { name: 'arabic', pattern: /[\u0600-\u06FF\u0750-\u077F]/ },
    { name: 'devanagari', pattern: /[\u0900-\u097F]/ },
    { name: 'thai', pattern: /[\u0E00-\u0E7F]/ },
    { name: 'korean', pattern: /[\uAC00-\uD7AF]/ },
  ];

  return async function autoLanguageDetectMiddleware(ctx, next) {
    const sample = ctx.text.slice(0, 200);
    let detected = 'latin';

    for (const { name, pattern } of SCRIPT_RANGES) {
      if (pattern.test(sample)) {
        detected = name;
        break;
      }
    }

    ctx.metadata.detectedLanguage = detected;
    await next();
  };
}
