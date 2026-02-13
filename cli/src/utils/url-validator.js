/**
 * URL validation for SSRF prevention.
 *
 * Blocks requests to private/internal IP ranges and non-HTTP protocols.
 */

/**
 * Validate a URL before passing it to fetch().
 * Throws if the URL targets a private/internal network or uses a non-HTTP protocol.
 *
 * @param {string} url - The URL to validate
 * @throws {Error} If the URL is blocked
 */
/**
 * Check if a URL is safe to display in chat embeds/buttons.
 * Only allows http: and https: protocols — blocks javascript:, data:, file:, etc.
 *
 * @param {string} url - The URL to validate
 * @returns {boolean} true if safe to display
 */
export function isSafeDisplayUrl(url) {
  if (!url || typeof url !== 'string') return false;
  try {
    const parsed = new URL(url);
    return parsed.protocol === 'http:' || parsed.protocol === 'https:';
  } catch {
    return false;
  }
}

export function validateFetchUrl(url) {
  const parsed = new URL(url);
  if (!['http:', 'https:'].includes(parsed.protocol)) {
    throw new Error(`Unsupported protocol: ${parsed.protocol}`);
  }
  const host = parsed.hostname;
  if (
    host === 'localhost' ||
    host === '127.0.0.1' ||
    host === '::1' ||
    host === '0.0.0.0' ||
    host.startsWith('10.') ||
    host.startsWith('192.168.') ||
    /^172\.(1[6-9]|2\d|3[01])\./.test(host) ||
    host.endsWith('.internal') ||
    host.endsWith('.local')
  ) {
    throw new Error(`SSRF blocked: cannot fetch internal URL ${parsed.origin}`);
  }
}
