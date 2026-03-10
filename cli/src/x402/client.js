/**
 * x402 Sequencer REST Client
 */

const ALLOWED_SEQUENCER_PROTOCOLS = new Set(['grpc:', 'grpcs:', 'http:', 'https:']);

/**
 * @typedef {{ apiKey?: string | null, jwt?: string | null }} SequencerAuth
 * @typedef {{
 *   sequencerUrl?: string,
 *   sequencer?: { url?: string },
 *   auth?: SequencerAuth,
 *   getCredentials?: () => SequencerAuth,
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

export class X402SequencerClient {
  /**
   * @param {SequencerInput} config
   */
  constructor(config) {
    this.config = config;
    this.baseUrl = buildBaseUrl(config);
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
   * @param {string} method
   * @param {string} path
   * @param {unknown} [body]
   * @returns {Promise<any>}
   */
  async _request(method, path, body) {
    const url = `${this.baseUrl}${path}`;
    /** @type {RequestInit} */
    const options = {
      method,
      headers: this._getHeaders(),
    };
    if (body !== undefined) {
      options.body = JSON.stringify(body);
    }
    const response = await fetch(url, options);
    if (!response.ok) {
      const text = await response.text();
      throw new Error(`Sequencer request failed: ${response.status} ${text}`);
    }
    return response.json();
  }

  /**
   * @param {unknown} payload
   * @returns {Promise<any>}
   */
  async submitPaymentIntent(payload) {
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
   * @returns {Promise<any>}
   */
  async getPaymentReceipt(intentId) {
    return this._request('GET', `/api/v1/x402/payments/${intentId}/receipt`);
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
        const response = await this.getPaymentReceipt(intentId);
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
}
