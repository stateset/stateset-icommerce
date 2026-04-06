/**
 * Shared Redis / Cache Client
 *
 * CacheMap<V> class that wraps Upstash Redis REST API,
 * falling back to in-memory Map when Redis is unavailable.
 */

const UPSTASH_REDIS_REST_URL = process.env.UPSTASH_REDIS_REST_URL;
const UPSTASH_REDIS_REST_TOKEN = process.env.UPSTASH_REDIS_REST_TOKEN;

interface RedisResponse {
  result: unknown;
}

async function redisCommand(command: string[]): Promise<unknown> {
  if (!UPSTASH_REDIS_REST_URL || !UPSTASH_REDIS_REST_TOKEN) {
    return null;
  }

  const response = await fetch(`${UPSTASH_REDIS_REST_URL}`, {
    method: 'POST',
    headers: {
      Authorization: `Bearer ${UPSTASH_REDIS_REST_TOKEN}`,
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(command),
  });

  if (!response.ok) {
    throw new Error(`Redis command failed: ${response.status}`);
  }

  const data: RedisResponse = await response.json();
  return data.result;
}

interface CacheEntry<V> {
  value: V;
  expiresAt: number;
}

/**
 * CacheMap provides a Map-like interface backed by Redis (Upstash REST API).
 * Falls back to an in-memory Map when Redis is unavailable.
 *
 * @template V - Value type (must be JSON-serializable)
 */
export class CacheMap<V> {
  private prefix: string;
  private defaultTtlMs: number;
  private useRedis: boolean;
  private memoryFallback: Map<string, CacheEntry<V>> = new Map();

  constructor(prefix: string, defaultTtlMs: number) {
    this.prefix = prefix;
    this.defaultTtlMs = defaultTtlMs;
    this.useRedis = Boolean(UPSTASH_REDIS_REST_URL && UPSTASH_REDIS_REST_TOKEN);
  }

  private redisKey(key: string): string {
    return `${this.prefix}:${key}`;
  }

  async get(key: string): Promise<V | undefined> {
    if (this.useRedis) {
      try {
        const result = await redisCommand(['GET', this.redisKey(key)]);
        if (result === null || result === undefined) return undefined;
        return JSON.parse(result as string) as V;
      } catch {
        // Fall through to memory
      }
    }

    const entry = this.memoryFallback.get(key);
    if (!entry) return undefined;
    if (Date.now() > entry.expiresAt) {
      this.memoryFallback.delete(key);
      return undefined;
    }
    return entry.value;
  }

  async set(key: string, value: V, ttlMs?: number): Promise<void> {
    const ttl = ttlMs ?? this.defaultTtlMs;
    const ttlSeconds = Math.ceil(ttl / 1000);

    if (this.useRedis) {
      try {
        await redisCommand([
          'SET',
          this.redisKey(key),
          JSON.stringify(value),
          'EX',
          String(ttlSeconds),
        ]);
        return;
      } catch {
        // Fall through to memory
      }
    }

    this.memoryFallback.set(key, {
      value,
      expiresAt: Date.now() + ttl,
    });
  }

  async delete(key: string): Promise<boolean> {
    if (this.useRedis) {
      try {
        const result = await redisCommand(['DEL', this.redisKey(key)]);
        return (result as number) > 0;
      } catch {
        // Fall through to memory
      }
    }

    return this.memoryFallback.delete(key);
  }

  async has(key: string): Promise<boolean> {
    if (this.useRedis) {
      try {
        const result = await redisCommand(['EXISTS', this.redisKey(key)]);
        return (result as number) > 0;
      } catch {
        // Fall through to memory
      }
    }

    const entry = this.memoryFallback.get(key);
    if (!entry) return false;
    if (Date.now() > entry.expiresAt) {
      this.memoryFallback.delete(key);
      return false;
    }
    return true;
  }

  /**
   * Cleanup expired entries from in-memory fallback.
   * Called periodically to prevent memory leaks.
   */
  cleanup(): void {
    const now = Date.now();
    for (const [key, entry] of this.memoryFallback) {
      if (now > entry.expiresAt) {
        this.memoryFallback.delete(key);
      }
    }
  }
}

// ============================================================================
// Pre-configured cache instances
// ============================================================================

/** Pending confirmations for agent chat (5min TTL) */
export const pendingConfirmations = new CacheMap<{
  chatId: string;
  action: string;
  params: Record<string, unknown>;
}>('confirm', 5 * 60 * 1000);

/** Sandbox cache (10min TTL) */
export const sandboxCache = new CacheMap<{
  sandboxId: string;
  url: string;
  createdAt: number;
}>('sandbox', 10 * 60 * 1000);

/** Health check results (30s TTL) */
export const healthCheckCache = new CacheMap<{
  healthy: boolean;
  checkedAt: number;
}>('health', 30 * 1000);

/** Success cache for sandbox operations (60s TTL) */
export const successCache = new CacheMap<{
  success: boolean;
  result: unknown;
}>('success', 60 * 1000);

/** Active loop states (60min TTL) */
export const activeLoops = new CacheMap<{
  chatId: string;
  status: string;
  iteration: number;
  startedAt: number;
}>('loop-state', 60 * 60 * 1000);

/** Loop metadata (60min TTL) */
export const loopMetadata = new CacheMap<{
  chatId: string;
  config: Record<string, unknown>;
  createdAt: number;
}>('loop', 60 * 60 * 1000);

/** Guardrail evaluation cache (60s TTL) */
export const guardrailCache = new CacheMap<{
  passed: boolean;
  score: number;
}>('guardrail', 60 * 1000);

/** Sandbox API key cache (12hr TTL) */
export const sandboxApiKeyCache = new CacheMap<{
  apiKey: string;
  expiresAt: number;
}>('sbx-key', 12 * 60 * 60 * 1000);

/** Rate limit store (1min TTL) */
export const rateLimitStore = new CacheMap<{
  count: number;
  resetAt: number;
}>('ratelimit', 60 * 1000);

// Periodic cleanup of in-memory fallbacks (every 5 minutes)
if (typeof globalThis !== 'undefined') {
  const CLEANUP_INTERVAL = 5 * 60 * 1000;
  const allCaches = [
    pendingConfirmations,
    sandboxCache,
    healthCheckCache,
    successCache,
    activeLoops,
    loopMetadata,
    guardrailCache,
    sandboxApiKeyCache,
    rateLimitStore,
  ];

  setInterval(() => {
    allCaches.forEach((cache) => cache.cleanup());
  }, CLEANUP_INTERVAL).unref?.();
}
