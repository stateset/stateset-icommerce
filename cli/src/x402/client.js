/**
 * x402 Sequencer REST Client
 */

import { SyncConfig } from '../sync/config.js';

function buildBaseUrl(input) {
  const raw = typeof input === 'string' ? input : input?.sequencerUrl || input?.sequencer?.url;
  if (!raw) {
    throw new Error('Sequencer URL is required');
  }
  const url = new URL(raw);
  if (url.protocol === 'grpc:' || url.protocol === 'grpcs:') {
    const restProtocol = url.protocol === 'grpcs:' ? 'https:' : 'http:';
    return `${restProtocol}//${url.host}`;
  }
  return url.toString().replace(/\/$/, '');
}

function getCredentials(config) {
  if (!config) return { apiKey: null, jwt: null };
  if (typeof config.getCredentials === 'function') return config.getCredentials();
  return {
    apiKey: config?.auth?.apiKey ?? null,
    jwt: config?.auth?.jwt ?? null,
  };
}

export class X402SequencerClient {
  /**
   * @param {SyncConfig|{sequencerUrl?: string, sequencer?: {url?: string}, auth?: {apiKey?: string, jwt?: string}}|string} config
   */
  constructor(config) {
    this.config = config instanceof SyncConfig ? config : config;
    this.baseUrl = buildBaseUrl(config);
  }

  _getHeaders() {
    const headers = {
      'Content-Type': 'application/json',
    };
    const creds = getCredentials(this.config);
    if (creds.apiKey) headers.Authorization = `Bearer ${creds.apiKey}`;
    else if (creds.jwt) headers.Authorization = `Bearer ${creds.jwt}`;
    return headers;
  }

  async _request(method, path, body) {
    const url = `${this.baseUrl}${path}`;
    const options = {
      method,
      headers: this._getHeaders(),
    };
    if (body) {
      options.body = JSON.stringify(body);
    }
    const response = await fetch(url, options);
    if (!response.ok) {
      const text = await response.text();
      throw new Error(`Sequencer request failed: ${response.status} ${text}`);
    }
    return response.json();
  }

  async submitPaymentIntent(payload) {
    return this._request('POST', '/api/v1/x402/payments', payload);
  }

  async getPaymentStatus(intentId) {
    return this._request('GET', `/api/v1/x402/payments/${intentId}`);
  }

  async getPaymentReceipt(intentId) {
    return this._request('GET', `/api/v1/x402/payments/${intentId}/receipt`);
  }

  async createBatch(payload) {
    return this._request('POST', '/api/v1/x402/batches', payload);
  }

  async settleBatch(payload) {
    return this._request('POST', '/api/v1/x402/batches/settle', payload);
  }

  async waitForReceipt(intentId, { timeoutMs = 300_000, intervalMs = 2_000 } = {}) {
    const start = Date.now();
    // eslint-disable-next-line no-constant-condition
    while (true) {
      try {
        const response = await this.getPaymentReceipt(intentId);
        if (response?.receipt) return response.receipt;
      } catch (err) {
        // ignore until timeout
      }
      if (Date.now() - start > timeoutMs) {
        throw new Error(`Timed out waiting for receipt for intent ${intentId}`);
      }
      await new Promise(resolve => setTimeout(resolve, intervalMs));
    }
  }
}
