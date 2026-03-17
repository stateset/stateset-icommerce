/**
 * Shopify REST Admin API Client
 *
 * Uses Node.js built-in fetch() — no external npm dependencies.
 * Implements cursor-based pagination and rate limiting.
 */

/**
 * Validate a Shopify domain for SSRF prevention.
 * @param {string} domain
 * @returns {boolean}
 */
function isValidShopifyDomain(domain) {
  if (!domain || typeof domain !== 'string') return false;

  // Must match *.myshopify.com pattern
  if (!/^[a-z0-9-]+\.myshopify\.com$/i.test(domain)) return false;

  // Block private/internal patterns
  const blocked = ['localhost', '127.0.0.1', '0.0.0.0', '::1', '.internal', '.local'];
  const lower = domain.toLowerCase();
  for (const pattern of blocked) {
    if (lower.includes(pattern)) return false;
  }

  return true;
}

/**
 * Simple token-bucket rate limiter for Shopify's 2 req/sec REST limit.
 */
class RateLimiter {
  constructor(requestsPerSecond = 2) {
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

/**
 * Parse the Link header for cursor-based pagination.
 * @param {string|null} linkHeader
 * @returns {{ next: string|null, previous: string|null }}
 */
function parseLinkHeader(linkHeader) {
  const result = { next: null, previous: null };
  if (!linkHeader) return result;

  const parts = linkHeader.split(',');
  for (const part of parts) {
    const match = part.match(/<([^>]+)>;\s*rel="(\w+)"/);
    if (match) {
      const [, url, rel] = match;
      if (rel === 'next') result.next = url;
      if (rel === 'previous') result.previous = url;
    }
  }
  return result;
}

export class ShopifyClient {
  /**
   * @param {Object} config
   * @param {string} config.shopDomain - e.g., "my-store.myshopify.com"
   * @param {string} config.accessToken - Shopify Admin API access token
   * @param {string} [config.apiVersion='2024-01'] - API version
   * @param {number} [config.requestsPerSecond=2] - Rate limit
   */
  constructor({ shopDomain, accessToken, apiVersion = '2024-01', requestsPerSecond = 2 }) {
    if (!shopDomain) throw new Error('shopDomain is required');
    if (!accessToken) throw new Error('accessToken is required');

    if (!isValidShopifyDomain(shopDomain)) {
      throw new Error(
        `Invalid Shopify domain: "${shopDomain}". Must match *.myshopify.com pattern.`,
      );
    }

    this.baseUrl = `https://${shopDomain}/admin/api/${apiVersion}`;
    this.headers = {
      'X-Shopify-Access-Token': accessToken,
      'Content-Type': 'application/json',
    };
    this.rateLimiter = new RateLimiter(requestsPerSecond);
  }

  /**
   * Make an authenticated GET request to the Shopify API.
   * @param {string} url - Full URL or path relative to baseUrl
   * @param {Object} [params] - Query parameters
   * @returns {Promise<{data: Object, headers: Headers}>}
   */
  async get(url, params = {}) {
    await this.rateLimiter.wait();

    const fullUrl = url.startsWith('http') ? url : `${this.baseUrl}${url}`;
    const urlObj = new URL(fullUrl);
    for (const [key, value] of Object.entries(params)) {
      if (value !== null && value !== undefined) urlObj.searchParams.set(key, String(value));
    }

    const response = await fetch(urlObj.toString(), {
      method: 'GET',
      headers: this.headers,
    });

    if (!response.ok) {
      const body = await response.text().catch(() => '');
      throw new ShopifyApiError(response.status, response.statusText, body);
    }

    const data = await response.json();
    return { data, headers: response.headers };
  }

  /**
   * Fetch all pages of a paginated resource using cursor-based pagination.
   * @param {string} resource - e.g., '/customers.json'
   * @param {string} rootKey - e.g., 'customers'
   * @param {Object} [params] - Query parameters (limit, etc.)
   * @returns {AsyncGenerator<Object[]>} Yields arrays of records (one per page)
   */
  async *fetchPaginated(resource, rootKey, params = {}) {
    const queryParams = { limit: params.limit || 50, ...params };
    delete queryParams.limit; // Re-add explicitly
    queryParams.limit = params.limit || 50;

    let nextUrl = null;
    let isFirstPage = true;

    while (true) {
      const url = nextUrl || resource;
      const requestParams = isFirstPage ? queryParams : {};

      const { data, headers } = await this.get(url, requestParams);
      const records = data[rootKey] || [];

      if (records.length > 0) {
        yield records;
      }

      const links = parseLinkHeader(headers.get('link'));
      if (links.next) {
        nextUrl = links.next;
        isFirstPage = false;
      } else {
        break;
      }
    }
  }

  /**
   * Fetch customers.
   * @param {Object} [options]
   * @returns {AsyncGenerator<Object[]>}
   */
  async *getCustomers(options = {}) {
    yield* this.fetchPaginated('/customers.json', 'customers', options);
  }

  /**
   * Fetch products.
   * @param {Object} [options]
   * @returns {AsyncGenerator<Object[]>}
   */
  async *getProducts(options = {}) {
    yield* this.fetchPaginated('/products.json', 'products', options);
  }

  /**
   * Fetch orders.
   * @param {Object} [options]
   * @returns {AsyncGenerator<Object[]>}
   */
  async *getOrders(options = {}) {
    yield* this.fetchPaginated('/orders.json', 'orders', {
      status: 'any',
      ...options,
    });
  }

  /**
   * Fetch fulfillments.
   * @param {Object} [options]
   * @returns {AsyncGenerator<Object[]>}
   */
  async *getFulfillments(options = {}) {
    yield* this.fetchPaginated('/fulfillments.json', 'fulfillments', {
      status: 'any',
      ...options,
    });
  }

  /**
   * Fetch shop locations.
   * @returns {Promise<Object[]>}
   */
  async getLocations() {
    const { data } = await this.get('/locations.json');
    return data?.locations || [];
  }

  /**
   * Fetch inventory levels for a location.
   * @param {string} locationId
   * @param {Object} [options]
   * @returns {AsyncGenerator<Object[]>}
   */
  async *getInventoryLevels(locationId, options = {}) {
    yield* this.fetchPaginated('/inventory_levels.json', 'inventory_levels', {
      location_ids: locationId,
      ...options,
    });
  }

  /**
   * Make an authenticated POST request.
   * @param {string} url - Path relative to baseUrl
   * @param {Object} body - JSON body
   * @returns {Promise<{data: Object, headers: Headers}>}
   */
  async post(url, body) {
    await this.rateLimiter.wait();

    const fullUrl = `${this.baseUrl}${url}`;
    const response = await fetch(fullUrl, {
      method: 'POST',
      headers: this.headers,
      body: JSON.stringify(body),
    });

    if (!response.ok) {
      const text = await response.text().catch(() => '');
      throw new ShopifyApiError(response.status, response.statusText, text);
    }

    const data = await response.json();
    return { data, headers: response.headers };
  }

  /**
   * Make an authenticated PUT request.
   * @param {string} url - Path relative to baseUrl
   * @param {Object} body - JSON body
   * @returns {Promise<{data: Object, headers: Headers}>}
   */
  async put(url, body) {
    await this.rateLimiter.wait();

    const fullUrl = `${this.baseUrl}${url}`;
    const response = await fetch(fullUrl, {
      method: 'PUT',
      headers: this.headers,
      body: JSON.stringify(body),
    });

    if (!response.ok) {
      const text = await response.text().catch(() => '');
      throw new ShopifyApiError(response.status, response.statusText, text);
    }

    const data = await response.json();
    return { data, headers: response.headers };
  }

  /**
   * Create an order in Shopify.
   * @param {Object} orderData
   * @returns {Promise<Object>}
   */
  async createOrder(orderData) {
    const { data } = await this.post('/orders.json', { order: orderData });
    return data?.order;
  }

  /**
   * Update an order.
   * @param {string|number} orderId
   * @param {Object} updates
   * @returns {Promise<Object>}
   */
  async updateOrder(orderId, updates) {
    const { data } = await this.put(`/orders/${orderId}.json`, { order: updates });
    return data?.order;
  }

  /**
   * Create a fulfillment for an order.
   * @param {string|number} orderId
   * @param {Object} fulfillmentData - { tracking_number, tracking_company, tracking_urls, line_items }
   * @returns {Promise<Object>}
   */
  async createFulfillment(orderId, fulfillmentData) {
    const { data } = await this.post(`/orders/${orderId}/fulfillments.json`, {
      fulfillment: {
        notify_customer: true,
        ...fulfillmentData,
      },
    });
    return data?.fulfillment;
  }

  /**
   * Adjust inventory level at a location.
   * @param {string} inventoryItemId
   * @param {string} locationId
   * @param {number} adjustment - Positive or negative
   * @returns {Promise<Object>}
   */
  async adjustInventory(inventoryItemId, locationId, adjustment) {
    const { data } = await this.post('/inventory_levels/adjust.json', {
      inventory_item_id: inventoryItemId,
      location_id: locationId,
      available_adjustment: adjustment,
    });
    return data?.inventory_level;
  }

  /**
   * Create a refund for an order.
   * @param {string|number} orderId
   * @param {Object} refundData
   * @returns {Promise<Object>}
   */
  async createRefund(orderId, refundData) {
    const { data } = await this.post(`/orders/${orderId}/refunds.json`, {
      refund: refundData,
    });
    return data?.refund;
  }

  /**
   * Test connection by fetching shop info.
   * @returns {Promise<boolean>}
   */
  async testConnection() {
    try {
      await this.get('/shop.json');
      return true;
    } catch {
      return false;
    }
  }
}

/**
 * Shopify API error with status and body.
 */
export class ShopifyApiError extends Error {
  constructor(status, statusText, body = '') {
    super(`Shopify API error: ${status} ${statusText}`);
    this.name = 'ShopifyApiError';
    this.status = status;
    this.statusText = statusText;
    this.body = body;
  }
}

// Export helpers for testing
export { isValidShopifyDomain, parseLinkHeader, RateLimiter };
