/**
 * x402 Sequencer REST Client
 *
 * Features:
 *   - Circuit breaker (closed → open → half-open)
 *   - Exponential backoff retries (max 3 per request)
 *   - Fallback sequencer URL
 *   - Offline payment queue (in-memory; drained by the client itself once the
 *     breaker admits requests again — trial-sized while half-open, flushed
 *     when the circuit closes)
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
   * @param {number} [opts.halfOpenMax=2] - Trial requests admitted at a time
   *   while half-open, and the successes needed to close again
   */
  constructor(opts = {}) {
    this.failureThreshold = opts.failureThreshold ?? 5;
    this.resetTimeoutMs = opts.resetTimeoutMs ?? 30_000;
    this.halfOpenMax = opts.halfOpenMax ?? 2;
    this.state = CircuitState.CLOSED;
    this.failures = 0;
    this.halfOpenSuccesses = 0;
    /** Trial requests admitted in this half-open window and not yet reported. */
    this.halfOpenInFlight = 0;
    this.halfOpenSince = 0;
    this.lastFailureTime = 0;
  }

  /**
   * Admit a request. While half-open this claims one of `halfOpenMax` trial
   * slots, which the caller gives back by calling recordSuccess() or
   * recordFailure(); further requests are refused until one does.
   * @returns {boolean}
   */
  canRequest() {
    if (this.state === CircuitState.CLOSED) return true;
    if (this.state === CircuitState.OPEN) {
      if (Date.now() - this.lastFailureTime < this.resetTimeoutMs) return false;
      this._enterHalfOpen();
    }
    // HALF_OPEN — admit a limited number of trial requests
    if (this.halfOpenInFlight >= this.halfOpenMax) {
      if (Date.now() - this.halfOpenSince < this.resetTimeoutMs) return false;
      // Trials that never reported back: open a fresh probe window instead of
      // staying shut for good.
      this._enterHalfOpen();
    }
    this.halfOpenInFlight++;
    return true;
  }

  /** @private */
  _enterHalfOpen() {
    this.state = CircuitState.HALF_OPEN;
    this.halfOpenSuccesses = 0;
    this.halfOpenInFlight = 0;
    this.halfOpenSince = Date.now();
  }

  recordSuccess() {
    if (this.state === CircuitState.HALF_OPEN) {
      this.halfOpenInFlight = Math.max(0, this.halfOpenInFlight - 1);
      this.halfOpenSuccesses++;
      if (this.halfOpenSuccesses >= this.halfOpenMax) {
        this.state = CircuitState.CLOSED;
        this.failures = 0;
        this.halfOpenInFlight = 0;
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
      this.halfOpenInFlight = 0;
    }
  }

  getState() {
    // Re-check open→half_open transition
    if (
      this.state === CircuitState.OPEN &&
      Date.now() - this.lastFailureTime >= this.resetTimeoutMs
    ) {
      this._enterHalfOpen();
    }
    return this.state;
  }

  /**
   * Milliseconds until canRequest() could next admit a request: 0 when it can
   * now, the rest of the reset timeout while open, and — while every half-open
   * trial slot is taken — the time until those trials count as lost (a slot
   * that is given back sooner makes it 0 again). Does not change state.
   * @returns {number}
   */
  admissionDelayMs() {
    const now = Date.now();
    if (this.state === CircuitState.CLOSED) return 0;
    if (this.state === CircuitState.OPEN) {
      return Math.max(0, this.lastFailureTime + this.resetTimeoutMs - now);
    }
    if (this.halfOpenInFlight < this.halfOpenMax) return 0;
    return Math.max(0, this.halfOpenSince + this.resetTimeoutMs - now);
  }
}

const PAYMENTS_PATH = '/api/v1/x402/payments';

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
    /** @type {Array<{payload: unknown, resolve: (value: any) => void, reject: (reason?: unknown) => void}>} */
    this._offlineQueue = [];
    /**
     * Timer for the next attempt to drain the offline queue, armed only while
     * the queue holds payments the breaker is not yet admitting.
     * @type {ReturnType<typeof setTimeout> | null}
     */
    this._drainTimer = null;
    this._drainDueAt = 0;
    this._closed = false;
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
   * @param {{ maxRetries?: number, _admitted?: boolean }} [requestOptions] -
   *   `_admitted` is internal: the caller already holds the breaker's admission
   *   for this request, so it must not claim a second half-open slot.
   * @returns {Promise<any>}
   */
  async _request(method, path, body, requestOptions = {}) {
    // Check circuit breaker
    if (!requestOptions._admitted && !this._circuitBreaker.canRequest()) {
      if (this._circuitBreaker.state === CircuitState.HALF_OPEN) {
        throw new Error(
          `Sequencer circuit breaker is HALF_OPEN — ${this._circuitBreaker.halfOpenInFlight} trial request(s) already in flight; requests blocked until they report back`,
        );
      }
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

          // A success may have closed the circuit or given back a half-open
          // trial slot: let the offline queue move. It never pushes more than
          // the free trial slots' worth of the backlog while half-open.
          this._scheduleDrain();

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
    // The circuit may have (re-)opened: queued payments wait for the next
    // probe window instead of for outside traffic.
    this._scheduleDrain();
    throw lastError;
  }

  /**
   * Submit a payment intent. If the circuit is not admitting requests, the
   * payment is queued and submitted by the client itself once the breaker
   * admits it again (see `_drainOfflineQueue`); the returned promise settles
   * then, without needing any other request to come along.
   * @param {unknown} payload
   * @returns {Promise<any>}
   */
  async submitPaymentIntent(payload) {
    if (!this._circuitBreaker.canRequest()) {
      if (this._closed) {
        throw new Error(
          'X402 sequencer client is closed — the circuit is not admitting requests and the payment was not queued',
        );
      }
      // Queue for later
      return new Promise((resolve, reject) => {
        this._offlineQueue.push({ payload, resolve, reject });
        console.warn(
          `[x402] Sequencer circuit OPEN — payment queued (queue depth: ${this._offlineQueue.length})`,
        );
        this._scheduleDrain();
      });
    }
    // canRequest() above admitted this request; don't claim a second slot.
    return this._request('POST', PAYMENTS_PATH, payload, { _admitted: true });
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
   * Stop draining the offline queue and reject the payments still in it.
   * Afterwards a payment the breaker does not admit right away is refused
   * instead of queued. Call this when discarding a client: while payments are
   * queued, the drain timer keeps the process alive until each has settled.
   */
  close() {
    this._closed = true;
    this._clearDrainTimer();
    const queued = this._offlineQueue.splice(0);
    for (const item of queued) {
      item.reject(new Error('X402 sequencer client closed — queued payment was not submitted'));
    }
  }

  /** @private */
  _clearDrainTimer() {
    if (this._drainTimer) {
      clearTimeout(this._drainTimer);
      this._drainTimer = null;
    }
  }

  /**
   * Arrange for the offline queue to be drained as soon as the breaker can
   * admit a request — after the open circuit's reset timeout, or when a
   * half-open trial slot frees up — so queued payments settle without any
   * outside request traffic. An earlier deadline replaces a later one.
   * @private
   */
  _scheduleDrain() {
    if (this._closed || this._offlineQueue.length === 0) return;
    const delay = this._circuitBreaker.admissionDelayMs();
    const dueAt = Date.now() + delay;
    if (this._drainTimer) {
      if (this._drainDueAt <= dueAt) return;
      clearTimeout(this._drainTimer);
    }
    this._drainDueAt = dueAt;
    this._drainTimer = setTimeout(() => {
      this._drainTimer = null;
      this._drainOfflineQueue();
    }, delay);
  }

  /**
   * Move the offline queue forward. Once the circuit is closed, flush it.
   * While half-open, send queued payments as the trial requests themselves —
   * one per free trial slot, so at most `halfOpenMax` of the backlog is in
   * flight while the sequencer is on probation; each settles its own promise
   * and, by reporting back, schedules the next step. A payment whose trial
   * fails is rejected (as in a flush) and the circuit re-opens, so the rest
   * wait for the next window. When nothing can be admitted now, re-arm the
   * timer for when something can.
   * @private
   */
  _drainOfflineQueue() {
    while (!this._closed && this._offlineQueue.length > 0) {
      if (this._circuitBreaker.getState() === CircuitState.CLOSED) {
        this._flushOfflineQueue();
        return;
      }
      // Claims a half-open trial slot when one is free.
      if (!this._circuitBreaker.canRequest()) break;
      const item = this._offlineQueue.shift();
      if (!item) break;
      this._request('POST', PAYMENTS_PATH, item.payload, { _admitted: true }).then(
        item.resolve,
        item.reject,
      );
    }
    this._scheduleDrain();
  }

  /**
   * Flush the offline payment queue (called when circuit closes).
   */
  async _flushOfflineQueue() {
    const queued = [...this._offlineQueue];
    this._offlineQueue.length = 0;

    for (let index = 0; index < queued.length; index++) {
      // If the circuit is no longer closed (a failure re-opened it), stop
      // flushing and re-queue what is left for the next probe window. This
      // reads the state rather than calling canRequest(), which would claim a
      // half-open trial slot.
      if (this._circuitBreaker.getState() !== CircuitState.CLOSED) {
        this._offlineQueue.push(...queued.slice(index));
        this._scheduleDrain();
        break;
      }
      const item = queued[index];
      try {
        const result = await this._request('POST', PAYMENTS_PATH, item.payload);
        item.resolve(result);
      } catch (err) {
        item.reject(err);
      }
    }
  }
}

export { SequencerCircuitBreaker, CircuitState, buildBaseUrl, getCredentials };
