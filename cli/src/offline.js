/**
 * Offline Support Module for StateSet CLI
 *
 * Provides graceful degradation when API is unavailable,
 * with automatic fallback to direct mode.
 */

import { createLogger } from './logger.js';

const logger = createLogger({ context: { module: 'offline' } });

/**
 * Check if the Anthropic API is available
 */
export async function checkApiAvailability(apiKey, options = {}) {
  if (!apiKey) {
    return {
      available: false,
      reason: 'no_api_key',
      message: 'ANTHROPIC_API_KEY not configured'
    };
  }

  const timeout = options.timeout || 5000;

  try {
    const controller = new AbortController();
    const timeoutId = setTimeout(() => controller.abort(), timeout);

    const response = await fetch('https://api.anthropic.com/v1/messages', {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'x-api-key': apiKey,
        'anthropic-version': '2023-06-01'
      },
      body: JSON.stringify({
        model: 'claude-haiku-3-5-20241022',
        max_tokens: 1,
        messages: [{ role: 'user', content: 'ping' }]
      }),
      signal: controller.signal
    });

    clearTimeout(timeoutId);

    // 401 means API is up but key is invalid
    if (response.status === 401) {
      return {
        available: false,
        reason: 'invalid_api_key',
        message: 'API key is invalid'
      };
    }

    // Any 2xx or expected error means API is reachable
    if (response.status < 500) {
      return {
        available: true,
        reason: 'ok',
        message: 'API is available'
      };
    }

    return {
      available: false,
      reason: 'server_error',
      message: `API returned status ${response.status}`
    };
  } catch (error) {
    if (error.name === 'AbortError') {
      return {
        available: false,
        reason: 'timeout',
        message: `API check timed out after ${timeout}ms`
      };
    }

    return {
      available: false,
      reason: 'network_error',
      message: error.message
    };
  }
}

/**
 * OfflineManager - Manages offline mode and fallback behavior
 */
export class OfflineManager {
  constructor(options = {}) {
    this.apiKey = options.apiKey || process.env.ANTHROPIC_API_KEY;
    this.forceOffline = options.forceOffline || false;
    this.checkInterval = options.checkInterval || 30000; // 30 seconds
    this.lastCheck = null;
    this.cachedStatus = null;
    this.onStatusChange = options.onStatusChange || null;
  }

  /**
   * Check if we should operate in offline mode
   */
  async isOffline(options = {}) {
    if (this.forceOffline) {
      return true;
    }

    // Use cached status if recent
    const now = Date.now();
    if (this.cachedStatus && this.lastCheck && (now - this.lastCheck) < this.checkInterval) {
      return !this.cachedStatus.available;
    }

    // Perform fresh check
    const status = await checkApiAvailability(this.apiKey, options);
    const wasOffline = this.cachedStatus ? !this.cachedStatus.available : null;
    const isNowOffline = !status.available;

    this.cachedStatus = status;
    this.lastCheck = now;

    // Notify on status change
    if (wasOffline !== null && wasOffline !== isNowOffline && this.onStatusChange) {
      this.onStatusChange({
        wasOffline,
        isOffline: isNowOffline,
        status
      });
    }

    return isNowOffline;
  }

  /**
   * Get current status
   */
  getStatus() {
    return this.cachedStatus;
  }

  /**
   * Force offline mode
   */
  setOffline(offline = true) {
    this.forceOffline = offline;
    logger.info('Offline mode changed', { offline });
  }

  /**
   * Create a wrapper that falls back to direct mode
   */
  createFallbackWrapper(aiHandler, directHandler) {
    return async (...args) => {
      const offline = await this.isOffline();

      if (offline) {
        logger.info('Using offline fallback', {
          reason: this.cachedStatus?.reason
        });
        return directHandler(...args);
      }

      try {
        return await aiHandler(...args);
      } catch (error) {
        // Check if error is API-related
        if (this.isApiError(error)) {
          logger.warn('AI request failed, falling back to direct mode', {
            error: error.message
          });

          // Update cached status
          this.cachedStatus = {
            available: false,
            reason: 'request_failed',
            message: error.message
          };

          return directHandler(...args);
        }

        throw error;
      }
    };
  }

  /**
   * Check if an error is API-related
   */
  isApiError(error) {
    const apiErrorPatterns = [
      'ECONNREFUSED',
      'ETIMEDOUT',
      'ENOTFOUND',
      'fetch failed',
      'network error',
      'API error',
      '503',
      '502',
      '500',
      'rate limit'
    ];

    const message = error.message?.toLowerCase() || '';
    return apiErrorPatterns.some(pattern =>
      message.includes(pattern.toLowerCase())
    );
  }
}

/**
 * Create an offline manager
 */
export function createOfflineManager(options = {}) {
  return new OfflineManager(options);
}

/**
 * Display offline mode warning
 */
export function showOfflineWarning(output, status) {
  const warnings = {
    no_api_key: 'Running in offline mode: ANTHROPIC_API_KEY not set',
    invalid_api_key: 'Running in offline mode: API key is invalid',
    timeout: 'Running in offline mode: API is not responding',
    network_error: 'Running in offline mode: Network unavailable',
    server_error: 'Running in offline mode: API server error',
    request_failed: 'Running in offline mode: API request failed'
  };

  const message = warnings[status.reason] || `Running in offline mode: ${status.message}`;

  console.log(`\n\x1b[33m⚠ ${message}\x1b[0m`);
  console.log('\x1b[90m  Using stateset-direct mode. Some AI features unavailable.\x1b[0m\n');
}

/**
 * OfflineCache - Cache for offline operation
 * Stores frequently accessed data for offline use
 */
export class OfflineCache {
  constructor(options = {}) {
    this.cache = new Map();
    this.maxAge = options.maxAge || 3600000; // 1 hour
    this.maxSize = options.maxSize || 1000;
  }

  /**
   * Get cached value
   */
  get(key) {
    const entry = this.cache.get(key);
    if (!entry) return null;

    if (Date.now() - entry.timestamp > this.maxAge) {
      this.cache.delete(key);
      return null;
    }

    return entry.value;
  }

  /**
   * Set cached value
   */
  set(key, value) {
    // Evict oldest entries if at capacity
    if (this.cache.size >= this.maxSize) {
      const oldest = [...this.cache.entries()]
        .sort((a, b) => a[1].timestamp - b[1].timestamp)[0];
      if (oldest) {
        this.cache.delete(oldest[0]);
      }
    }

    this.cache.set(key, {
      value,
      timestamp: Date.now()
    });
  }

  /**
   * Clear cache
   */
  clear() {
    this.cache.clear();
  }

  /**
   * Get cache statistics
   */
  getStats() {
    const now = Date.now();
    let valid = 0;
    let expired = 0;

    for (const entry of this.cache.values()) {
      if (now - entry.timestamp > this.maxAge) {
        expired++;
      } else {
        valid++;
      }
    }

    return {
      total: this.cache.size,
      valid,
      expired,
      maxSize: this.maxSize,
      maxAge: this.maxAge
    };
  }
}

export default {
  checkApiAvailability,
  OfflineManager,
  createOfflineManager,
  showOfflineWarning,
  OfflineCache
};
