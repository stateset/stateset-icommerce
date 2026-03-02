/**
 * WooCommerce REST API v3 Client
 *
 * Uses Node.js built-in fetch() — no external npm dependencies.
 * Implements page-based pagination and SSRF protection.
 * Auth: Basic Auth over HTTPS (base64 of consumerKey:consumerSecret).
 */

/**
 * Validate a URL for SSRF prevention — block private/internal IPs and hostnames.
 * @param {string} urlString
 * @returns {{ valid: boolean, error?: string }}
 */
export function validateUrl(urlString) {
  if (!urlString || typeof urlString !== 'string') {
    return { valid: false, error: 'URL is required' };
  }

  let parsed;
  try {
    parsed = new URL(urlString);
  } catch {
    return { valid: false, error: `Invalid URL: ${urlString}` };
  }

  const hostname = parsed.hostname.toLowerCase();

  // Block localhost variants
  if (
    hostname === 'localhost' ||
    hostname === '127.0.0.1' ||
    hostname === '0.0.0.0' ||
    hostname === '::1' ||
    hostname === '[::1]'
  ) {
    return { valid: false, error: `Blocked private hostname: ${hostname}` };
  }

  // Block private IP ranges
  const ipMatch = hostname.match(/^(\d+)\.(\d+)\.(\d+)\.(\d+)$/);
  if (ipMatch) {
    const [, a, b] = ipMatch.map(Number);
    // 10.x.x.x
    if (a === 10) return { valid: false, error: `Blocked private IP: ${hostname}` };
    // 172.16.0.0 – 172.31.255.255
    if (a === 172 && b >= 16 && b <= 31)
      return { valid: false, error: `Blocked private IP: ${hostname}` };
    // 192.168.x.x
    if (a === 192 && b === 168) return { valid: false, error: `Blocked private IP: ${hostname}` };
  }

  // Block .local and .internal TLDs
  if (hostname.endsWith('.local') || hostname.endsWith('.internal')) {
    return { valid: false, error: `Blocked internal hostname: ${hostname}` };
  }

  return { valid: true };
}

/**
 * Build the Basic Auth header value.
 * @param {string} consumerKey
 * @param {string} consumerSecret
 * @returns {string}
 */
export function buildBasicAuth(consumerKey, consumerSecret) {
  const credentials = `${consumerKey}:${consumerSecret}`;
  const encoded = Buffer.from(credentials, 'utf-8').toString('base64');
  return `Basic ${encoded}`;
}

/**
 * Simple rate limiter.
 */
export class RateLimiter {
  /**
   * @param {number} requestsPerSecond
   */
  constructor(requestsPerSecond = 5) {
    this.minInterval = 1000 / requestsPerSecond;
    this.lastRequest = 0;
  }

  async wait() {
    const now = Date.now();
    const elapsed = now - this.lastRequest;
    if (elapsed < this.minInterval) {
      await new Promise((resolve) => setTimeout(resolve, this.minInterval - elapsed));
    }
    this.lastRequest = Date.now();
  }
}

export class WooCommerceClient {
  /**
   * @param {Object} config
   * @param {string} config.siteUrl - e.g., "https://mystore.example.com"
   * @param {string} config.consumerKey - WooCommerce REST API consumer key
   * @param {string} config.consumerSecret - WooCommerce REST API consumer secret
   * @param {string} [config.apiVersion='wc/v3'] - API version prefix
   * @param {number} [config.requestsPerSecond=5] - Rate limit
   */
  constructor({
    siteUrl,
    consumerKey,
    consumerSecret,
    apiVersion = 'wc/v3',
    requestsPerSecond = 5,
  }) {
    if (!siteUrl) throw new Error('siteUrl is required');
    if (!consumerKey) throw new Error('consumerKey is required');
    if (!consumerSecret) throw new Error('consumerSecret is required');

    // Normalize: strip trailing slash
    const normalizedUrl = siteUrl.replace(/\/+$/, '');

    // SSRF validation
    const urlCheck = validateUrl(normalizedUrl);
    if (!urlCheck.valid) {
      throw new Error(`Invalid site URL: ${urlCheck.error}`);
    }

    this.baseUrl = `${normalizedUrl}/wp-json/${apiVersion}`;
    this.authHeader = buildBasicAuth(consumerKey, consumerSecret);
    this.headers = {
      Authorization: this.authHeader,
      'Content-Type': 'application/json',
    };
    this.rateLimiter = new RateLimiter(requestsPerSecond);
  }

  /**
   * Make an authenticated GET request.
   * @param {string} endpoint - e.g., '/products'
   * @param {Object} [params] - Query parameters
   * @returns {Promise<{data: Object, headers: Headers}>}
   */
  async get(endpoint, params = {}) {
    await this.rateLimiter.wait();

    const url = endpoint.startsWith('http') ? endpoint : `${this.baseUrl}${endpoint}`;
    const urlObj = new URL(url);
    for (const [key, value] of Object.entries(params)) {
      if (value !== null && value !== undefined) urlObj.searchParams.set(key, String(value));
    }

    const response = await fetch(urlObj.toString(), {
      method: 'GET',
      headers: this.headers,
    });

    if (!response.ok) {
      const body = await response.text().catch(() => '');
      throw new WooCommerceApiError(response.status, response.statusText, body);
    }

    const data = await response.json();
    return { data, headers: response.headers };
  }

  /**
   * Fetch all pages of a paginated resource using page-based pagination.
   * WooCommerce uses `page` and `per_page` query params.
   * Total pages are returned in the `X-WP-TotalPages` response header.
   *
   * @param {string} endpoint - e.g., '/products'
   * @param {Object} [params] - Query parameters
   * @returns {AsyncGenerator<Object[]>} Yields arrays of records (one per page)
   */
  async *fetchPaginated(endpoint, params = {}) {
    const perPage = params.per_page || 50;
    let page = 1;

    while (true) {
      const queryParams = { ...params, page, per_page: perPage };
      const { data, headers } = await this.get(endpoint, queryParams);

      const records = Array.isArray(data) ? data : [];
      if (records.length > 0) {
        yield records;
      }

      const totalPages = parseInt(headers.get('x-wp-totalpages') || '0', 10);
      if (page >= totalPages || records.length < perPage) {
        break;
      }
      page++;
    }
  }

  /**
   * Fetch customers.
   * @param {Object} [options]
   * @returns {AsyncGenerator<Object[]>}
   */
  async *getCustomers(options = {}) {
    yield* this.fetchPaginated('/customers', options);
  }

  /**
   * Fetch products.
   * @param {Object} [options]
   * @returns {AsyncGenerator<Object[]>}
   */
  async *getProducts(options = {}) {
    yield* this.fetchPaginated('/products', options);
  }

  /**
   * Fetch orders.
   * @param {Object} [options]
   * @returns {AsyncGenerator<Object[]>}
   */
  async *getOrders(options = {}) {
    yield* this.fetchPaginated('/orders', options);
  }

  /**
   * Test connection by fetching system status.
   * @returns {Promise<boolean>}
   */
  async testConnection() {
    try {
      await this.get('/system_status');
      return true;
    } catch {
      return false;
    }
  }
}

/**
 * WooCommerce API error with status and body.
 */
export class WooCommerceApiError extends Error {
  /**
   * @param {number} status
   * @param {string} statusText
   * @param {string} body
   */
  constructor(status, statusText, body = '') {
    super(`WooCommerce API error: ${status} ${statusText}`);
    this.name = 'WooCommerceApiError';
    this.status = status;
    this.statusText = statusText;
    this.body = body;
  }
}
