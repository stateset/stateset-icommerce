/**
 * x402 Sequencer REST Client
 *
 * Features:
 *   - Circuit breaker (closed → open → half-open)
 *   - Exponential backoff retries (max 3 per request)
 *   - Fallback sequencer URL
 *   - Offline payment queue (in-memory, flushed when circuit closes)
 *   - Health check endpoint polling
 */

const ALLOWED_SEQUENCER_PROTOCOLS = new Set(['grpc:', 'grpcs:', 'http:', 'https:']);

/**
 * @typedef {{ apiKey?: string | null, jwt?: string | null }} SequencerAuth
 * @typedef {{
 *   sequencerUrl?: string,
 *   sequencer?: { url?: string },
 *   auth?: SequencerAuth,
 *   getCredentials?: () => SequencerAuth,
 *   fallbackSequencerUrl?: string,
 *   retryOptions?: { maxRetries?: number, baseDelayMs?: number, maxDelayMs?: number },
 *   circuitBreaker?: { failureThreshold?: number, resetTimeoutMs?: number, halfOpenMax?: number },
 * }} SequencerConfigLike
 * @typedef {SequencerConfigLike | string | null | undefined} SequencerInput
 */

/**
 * @param {unknown} error
 * @returns {string}
 */
function messageFromError(error) {
  return error instanceof Error ? error.message : String(error);
}

/**
 * @param {unknown} url
 * @returns {URL}
 */
function parseSequencerUrl(url) {
  if (typeof url !== 'string' || !url.trim()) {
    throw new Error('Sequencer URL is required');
  }

  const parsed = new URL(url.trim());
  if (!ALLOWED_SEQUENCER_PROTOCOLS.has(parsed.protocol)) {
    throw new Error(`Unsupported sequencer protocol: ${parsed.protocol}`);
  }
  if (!parsed.hostname) {
    throw new Error('Sequencer URL must include a host');
  }
  return parsed;
}

/**
 * @param {SequencerInput} input
 * @returns {string}
 */
function buildBaseUrl(input) {
  const raw = typeof input === 'string' ? input : input?.sequencerUrl || input?.sequencer?.url;
  if (!raw) {
    throw new Error('Sequencer URL is required');
  }
  const url = parseSequencerUrl(raw);
  if (url.protocol === 'grpc:' || url.protocol === 'grpcs:') {
    const restProtocol = url.protocol === 'grpcs:' ? 'https:' : 'http:';
    return `${restProtocol}//${url.host}`;
  }
  return url.toString().replace(/\/$/, '');
}

/**
 * @param {SequencerInput} config
 * @returns {{ apiKey: string | null, jwt: string | null }}
 */
function getCredentials(config) {
  if (!config) return { apiKey: null, jwt: null };
  if (typeof config === 'string') return { apiKey: null, jwt: null };
  if (typeof config.getCredentials === 'function') {
    const credentials = config.getCredentials();
    return {
      apiKey: credentials.apiKey ?? null,
      jwt: credentials.jwt ?? null,
    };
  }
  return {
    apiKey: config?.auth?.apiKey ?? null,
    jwt: config?.auth?.jwt ?? null,
  };
}

// ── Circuit Breaker ─────────────────────────────────────────────────────

/** @enum {string} */
const CircuitState = { CLOSED: 'closed', OPEN: 'open', HALF_OPEN: 'half_open' };

class SequencerCircuitBreaker {
  /**
   * @param {Object} [opts]
   * @param {number} [opts.failureThreshold=5]
   * @param {number} [opts.resetTimeoutMs=30000]
   * @param {number} [opts.halfOpenMax=2]
   */
  constructor(opts = {}) {
    this.failureThreshold = opts.failureThreshold ?? 5;
    this.resetTimeoutMs = opts.resetTimeoutMs ?? 30_000;
    this.halfOpenMax = opts.halfOpenMax ?? 2;
    this.state = CircuitState.CLOSED;
    this.failures = 0;
    this.halfOpenSuccesses = 0;
    this.lastFailureTime = 0;
  }

  /** @returns {boolean} */
  canRequest() {
    if (this.state === CircuitState.CLOSED) return true;
    if (this.state === CircuitState.OPEN) {
      if (Date.now() - this.lastFailureTime >= this.resetTimeoutMs) {
        this.state = CircuitState.HALF_OPEN;
        this.halfOpenSuccesses = 0;
        return true;
      }
      return false;
    }
    // HALF_OPEN — allow limited requests
    return true;
  }

  recordSuccess() {
    if (this.state === CircuitState.HALF_OPEN) {
      this.halfOpenSuccesses++;
      if (this.halfOpenSuccesses >= this.halfOpenMax) {
        this.state = CircuitState.CLOSED;
        this.failures = 0;
      }
    } else {
      this.failures = 0;
    }
  }

  recordFailure() {
    this.failures++;
    this.lastFailureTime = Date.now();
    if (this.state === CircuitState.HALF_OPEN || this.failures >= this.failureThreshold) {
      this.state = CircuitState.OPEN;
    }
  }

  getState() {
    // Re-check open→half_open transition
    if (
      this.state === CircuitState.OPEN &&
      Date.now() - this.lastFailureTime >= this.resetTimeoutMs
    ) {
      this.state = CircuitState.HALF_OPEN;
      this.halfOpenSuccesses = 0;
    }
    return this.state;
  }
}

// ── Client ──────────────────────────────────────────────────────────────

export class X402SequencerClient {
  /**
   * @param {SequencerInput} config
   */
  constructor(config) {
    this.config = config;
    this.baseUrl = buildBaseUrl(config);

    // Fallback sequencer
    const fallbackUrl =
      typeof config !== 'string' && config?.fallbackSequencerUrl
        ? config.fallbackSequencerUrl
        : null;
    this.fallbackBaseUrl = fallbackUrl ? buildBaseUrl(fallbackUrl) : null;

    // Retry config
    const retryOpts = typeof config !== 'string' ? config?.retryOptions : undefined;
    this.maxRetries = retryOpts?.maxRetries ?? 3;
    this.baseDelayMs = retryOpts?.baseDelayMs ?? 500;
    this.maxDelayMs = retryOpts?.maxDelayMs ?? 10_000;

    // Circuit breaker
    const cbOpts = typeof config !== 'string' ? config?.circuitBreaker : undefined;
    this._circuitBreaker = new SequencerCircuitBreaker(cbOpts);

    // Offline queue for payments when circuit is open
    /** @type {Array<{payload: unknown, resolve: Function, reject: Function}>} */
    this._offlineQueue = [];
  }

  /**
   * @returns {Record<string, string>}
   */
  _getHeaders() {
    /** @type {Record<string, string>} */
    const headers = {
      'Content-Type': 'application/json',
    };
    const creds = getCredentials(this.config);
    if (creds.apiKey) headers.Authorization = `Bearer ${creds.apiKey}`;
    else if (creds.jwt) headers.Authorization = `Bearer ${creds.jwt}`;
    return headers;
  }

  /**
   * Execute a request with retries and circuit breaker.
   * @param {string} method
   * @param {string} path
   * @param {unknown} [body]
   * @param {{ maxRetries?: number }} [requestOptions]
   * @returns {Promise<any>}
   */
  async _request(method, path, body, requestOptions = {}) {
    // Check circuit breaker
    if (!this._circuitBreaker.canRequest()) {
      throw new Error(
        `Sequencer circuit breaker is OPEN — requests blocked. Will retry after ${this._circuitBreaker.resetTimeoutMs}ms`,
      );
    }

    const maxRetries = requestOptions.maxRetries ?? this.maxRetries;
    let lastError;
    for (let attempt = 0; attempt <= maxRetries; attempt++) {
      // Try primary, then fallback
      const urls =
        this.fallbackBaseUrl && attempt > 0
          ? [`${this.baseUrl}${path}`, `${this.fallbackBaseUrl}${path}`]
          : [`${this.baseUrl}${path}`];

      for (const url of urls) {
        try {
          /** @type {RequestInit} */
          const options = {
            method,
            headers: this._getHeaders(),
            signal: AbortSignal.timeout(15_000),
          };
          if (body !== undefined) {
            options.body = JSON.stringify(body);
          }

          const response = await fetch(url, options);
          if (!response.ok) {
            const text = await response.text();
            throw new Error(`Sequencer request failed: ${response.status} ${text}`);
          }

          const result = await response.json();
          this._circuitBreaker.recordSuccess();

          // Flush offline queue on success
          if (this._offlineQueue.length > 0) {
            this._flushOfflineQueue();
          }

          return result;
        } catch (err) {
          lastError = err;
        }
      }

      // Exponential backoff before retry
      if (attempt < maxRetries) {
        const delay = Math.min(this.baseDelayMs * Math.pow(2, attempt), this.maxDelayMs);
        await new Promise((resolve) => setTimeout(resolve, delay));
      }
    }

    // All retries exhausted
    this._circuitBreaker.recordFailure();
    throw lastError;
  }

  /**
   * Submit a payment intent. If circuit is open, queues for later submission.
   * @param {unknown} payload
   * @returns {Promise<any>}
   */
  async submitPaymentIntent(payload) {
    if (!this._circuitBreaker.canRequest()) {
      // Queue for later
      return new Promise((resolve, reject) => {
        this._offlineQueue.push({ payload, resolve, reject });
        console.warn(
          `[x402] Sequencer circuit OPEN — payment queued (queue depth: ${this._offlineQueue.length})`,
        );
      });
    }
    return this._request('POST', '/api/v1/x402/payments', payload);
  }

  /**
   * @param {string} intentId
   * @returns {Promise<any>}
   */
  async getPaymentStatus(intentId) {
    return this._request('GET', `/api/v1/x402/payments/${intentId}`);
  }

  /**
   * @param {string} intentId
   * @param {{ maxRetries?: number }} [requestOptions]
   * @returns {Promise<any>}
   */
  async getPaymentReceipt(intentId, requestOptions = {}) {
    return this._request(
      'GET',
      `/api/v1/x402/payments/${intentId}/receipt`,
      undefined,
      requestOptions,
    );
  }

  /**
   * @param {unknown} payload
   * @returns {Promise<any>}
   */
  async createBatch(payload) {
    return this._request('POST', '/api/v1/x402/batches', payload);
  }

  /**
   * @param {unknown} payload
   * @returns {Promise<any>}
   */
  async settleBatch(payload) {
    return this._request('POST', '/api/v1/x402/batches/settle', payload);
  }

  /**
   * @param {string} intentId
   * @param {{ timeoutMs?: number, intervalMs?: number }} [options]
   * @returns {Promise<any>}
   */
  async waitForReceipt(intentId, { timeoutMs = 300_000, intervalMs = 2_000 } = {}) {
    const start = Date.now();
    while (true) {
      try {
        // Receipt polling already retries at the poll-loop level. Avoid stacking
        // request-level exponential retries on top of each poll.
        const response = await this.getPaymentReceipt(intentId, { maxRetries: 0 });
        if (response?.receipt) return response.receipt;
      } catch (err) {
        console.warn('[x402] waitForReceipt poll error:', messageFromError(err));
      }
      if (Date.now() - start > timeoutMs) {
        throw new Error(`Timed out waiting for receipt for intent ${intentId}`);
      }
      await new Promise((resolve) => setTimeout(resolve, intervalMs));
    }
  }

  /**
   * Get circuit breaker status.
   * @returns {{ state: string, failures: number, queueDepth: number }}
   */
  getCircuitStatus() {
    return {
      state: this._circuitBreaker.getState(),
      failures: this._circuitBreaker.failures,
      queueDepth: this._offlineQueue.length,
    };
  }

  /**
   * Flush the offline payment queue (called when circuit closes).
   */
  async _flushOfflineQueue() {
    const queued = [...this._offlineQueue];
    this._offlineQueue.length = 0;

    for (const item of queued) {
      try {
        const result = await this._request('POST', '/api/v1/x402/payments', item.payload);
        item.resolve(result);
      } catch (err) {
        item.reject(err);
        // If circuit re-opened, stop flushing
        if (!this._circuitBreaker.canRequest()) {
          // Re-queue remaining items
          const remaining = queued.slice(queued.indexOf(item) + 1);
          this._offlineQueue.push(...remaining);
          break;
        }
      }
    }
  }
}

export { SequencerCircuitBreaker, CircuitState, buildBaseUrl, getCredentials };
