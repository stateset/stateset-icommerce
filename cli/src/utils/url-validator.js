/**
 * URL validation for SSRF prevention.
 *
 * Blocks requests to private/internal IP ranges and non-HTTP protocols.
 */

import dns from 'node:dns/promises';
import net from 'node:net';

/**
 * @typedef {{ address?: string, family?: number }} DnsLookupRecord
 * @typedef {(hostname: string, options: { all: boolean, verbatim: boolean }) => Promise<Array<string | DnsLookupRecord> | string | DnsLookupRecord>} DnsLookup
 * @typedef {{ lookup?: DnsLookup, cache?: boolean, fetch?: typeof fetch, maxRedirects?: number }} ValidatedFetchOptions
 */

const DNS_CACHE_TTL_MS = 60_000;
const REDIRECT_STATUSES = new Set([301, 302, 303, 307, 308]);
const DEFAULT_MAX_REDIRECTS = 10;
/** @type {Map<string, { expiresAt: number, error: string }>} */
const dnsValidationCache = new Map();

/**
 * @param {string} hostname
 * @returns {string}
 */
function normalizeHostname(hostname) {
  let host = String(hostname || '')
    .trim()
    .toLowerCase();
  if (host.startsWith('[') && host.endsWith(']')) {
    host = host.slice(1, -1);
  }
  if (host.endsWith('.')) {
    host = host.slice(0, -1);
  }
  return host;
}

/**
 * @param {string} address
 * @returns {number | null}
 */
function ipv4ToInt(address) {
  const parts = address.split('.').map((part) => Number.parseInt(part, 10));
  if (
    parts.length !== 4 ||
    parts.some((part) => !Number.isInteger(part) || part < 0 || part > 255)
  ) {
    return null;
  }
  return (
    (((parts[0] << 24) >>> 0) +
      ((parts[1] << 16) >>> 0) +
      ((parts[2] << 8) >>> 0) +
      (parts[3] >>> 0)) >>>
    0
  );
}

/**
 * @param {number} value
 * @param {number} base
 * @param {number} prefixLength
 * @returns {boolean}
 */
function ipv4InRange(value, base, prefixLength) {
  const mask = prefixLength === 0 ? 0 : (0xffffffff << (32 - prefixLength)) >>> 0;
  return (value & mask) === (base & mask);
}

/**
 * @param {string} address
 * @returns {boolean}
 */
function isBlockedIpv4(address) {
  const value = ipv4ToInt(address);
  if (value === null) return true;

  /** @type {Array<[string, number]>} */
  const ranges = [
    ['0.0.0.0', 8],
    ['10.0.0.0', 8],
    ['100.64.0.0', 10],
    ['127.0.0.0', 8],
    ['169.254.0.0', 16],
    ['172.16.0.0', 12],
    ['192.0.0.0', 24],
    ['192.0.2.0', 24],
    ['192.88.99.0', 24],
    ['192.168.0.0', 16],
    ['198.18.0.0', 15],
    ['198.51.100.0', 24],
    ['203.0.113.0', 24],
    ['224.0.0.0', 4],
    ['240.0.0.0', 4],
  ];

  return ranges.some(([base, prefixLength]) => {
    const baseValue = ipv4ToInt(base);
    return baseValue !== null && ipv4InRange(value, baseValue, prefixLength);
  });
}

/**
 * @param {string} address
 * @returns {bigint | null}
 */
function ipv6ToBigInt(address) {
  let normalized = normalizeHostname(address).split('%')[0];
  const ipv4Match = normalized.match(/^(.*:)(\d{1,3}(?:\.\d{1,3}){3})$/);
  if (ipv4Match) {
    const ipv4Value = ipv4ToInt(ipv4Match[2]);
    if (ipv4Value === null) return null;
    const high = ((ipv4Value >>> 16) & 0xffff).toString(16);
    const low = (ipv4Value & 0xffff).toString(16);
    normalized = `${ipv4Match[1]}${high}:${low}`;
  }

  const doubleColonParts = normalized.split('::');
  if (doubleColonParts.length > 2) return null;

  const left = doubleColonParts[0] ? doubleColonParts[0].split(':') : [];
  const right =
    doubleColonParts.length === 2 && doubleColonParts[1] ? doubleColonParts[1].split(':') : [];
  const fillCount = doubleColonParts.length === 2 ? 8 - left.length - right.length : 0;
  if (fillCount < 0) return null;

  const parts =
    doubleColonParts.length === 2 ? [...left, ...Array(fillCount).fill('0'), ...right] : left;
  if (parts.length !== 8) return null;

  let value = 0n;
  for (const part of parts) {
    if (!/^[0-9a-f]{1,4}$/i.test(part)) return null;
    value = (value << 16n) + BigInt(Number.parseInt(part, 16));
  }
  return value;
}

/**
 * @param {bigint} value
 * @param {bigint} base
 * @param {number} prefixLength
 * @returns {boolean}
 */
function ipv6InRange(value, base, prefixLength) {
  const bits = 128n;
  const prefix = BigInt(prefixLength);
  const mask = prefixLength === 0 ? 0n : ((1n << prefix) - 1n) << (bits - prefix);
  return (value & mask) === (base & mask);
}

/**
 * @param {string} address
 * @returns {boolean}
 */
function isBlockedIpv6(address) {
  const value = ipv6ToBigInt(address);
  if (value === null) return true;

  const mappedIpv4Base = ipv6ToBigInt('::ffff:0:0');
  if (mappedIpv4Base !== null && ipv6InRange(value, mappedIpv4Base, 96)) {
    const ipv4Value = Number(value & 0xffffffffn);
    const ipv4 = [
      (ipv4Value >>> 24) & 0xff,
      (ipv4Value >>> 16) & 0xff,
      (ipv4Value >>> 8) & 0xff,
      ipv4Value & 0xff,
    ].join('.');
    return isBlockedIpv4(ipv4);
  }

  /** @type {Array<[string, number]>} */
  const ranges = [
    ['::', 128],
    ['::1', 128],
    ['100::', 64],
    ['2001::', 32],
    ['2001:db8::', 32],
    ['2002::', 16],
    ['fc00::', 7],
    ['fe80::', 10],
    ['ff00::', 8],
  ];

  return ranges.some(([base, prefixLength]) => {
    const baseValue = ipv6ToBigInt(base);
    return baseValue !== null && ipv6InRange(value, baseValue, prefixLength);
  });
}

/**
 * @param {string} address
 * @returns {boolean}
 */
export function isBlockedIpAddress(address) {
  const host = normalizeHostname(address);
  const family = net.isIP(host);
  if (family === 4) return isBlockedIpv4(host);
  if (family === 6) return isBlockedIpv6(host);
  return false;
}

/**
 * @param {string} host
 * @returns {boolean}
 */
function isDocumentationHostname(host) {
  return (
    host === 'example.com' ||
    host.endsWith('.example.com') ||
    host === 'example.net' ||
    host.endsWith('.example.net') ||
    host === 'example.org' ||
    host.endsWith('.example.org') ||
    host === 'example' ||
    host.endsWith('.example')
  );
}

/**
 * @param {string} hostname
 * @returns {boolean}
 */
export function isBlockedFetchHostname(hostname) {
  const host = normalizeHostname(hostname);
  if (!host) return true;
  if (
    host === 'localhost' ||
    host.endsWith('.localhost') ||
    host.endsWith('.internal') ||
    host.endsWith('.local')
  ) {
    return true;
  }
  return isBlockedIpAddress(host);
}

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
  } catch (err) {
    const error = err instanceof Error ? err : new Error(String(err));
    console.debug('[url-validator] URL parse failed:', error.message);
    return false;
  }
}

/**
 * Validate a URL before passing it to fetch().
 * Throws if the URL targets a private/internal network or uses a non-HTTP protocol.
 *
 * @param {string} url
 * @returns {void}
 * @throws {Error} If the URL is blocked
 */
export function validateFetchUrl(url) {
  const parsed = new URL(url);
  if (!['http:', 'https:'].includes(parsed.protocol)) {
    throw new Error(`Unsupported protocol: ${parsed.protocol}`);
  }
  if (isBlockedFetchHostname(parsed.hostname)) {
    throw new Error(`SSRF blocked: cannot fetch internal URL ${parsed.origin}`);
  }
}

/**
 * @param {Headers | Map<string, string> | Record<string, unknown> | { get(name: string): string | null } | null | undefined} headers
 * @param {string} name
 * @returns {string | null}
 */
function readHeader(headers, name) {
  if (!headers) return null;
  if (typeof Headers !== 'undefined' && headers instanceof Headers) {
    return headers.get(name) || headers.get(name.toLowerCase()) || null;
  }
  if (headers instanceof Map) {
    return headers.get(name) || headers.get(name.toLowerCase()) || null;
  }
  if (typeof headers.get === 'function') {
    return headers.get(name) || headers.get(name.toLowerCase()) || null;
  }
  const headerRecord = /** @type {Record<string, unknown>} */ (headers);
  const direct = headerRecord[name] ?? headerRecord[name.toLowerCase()];
  return direct === undefined || direct === null ? null : String(direct);
}

/**
 * @param {RequestInit} options
 * @param {number} status
 * @returns {RequestInit}
 */
function optionsForRedirect(options, status) {
  const method = String(options.method || 'GET').toUpperCase();
  if (
    status !== 303 &&
    !((status === 301 || status === 302) && method !== 'GET' && method !== 'HEAD')
  ) {
    return options;
  }

  const nextOptions = {
    ...options,
    method: 'GET',
  };
  delete nextOptions.body;
  delete (/** @type {RequestInit & { duplex?: unknown }} */ (nextOptions).duplex);
  return nextOptions;
}

/**
 * Fetch a URL while validating the initial URL and every redirect target.
 *
 * The platform fetch implementation follows redirects automatically by default,
 * so callers that care about SSRF need redirect validation at the fetch layer.
 *
 * @param {string} url
 * @param {RequestInit} [options]
 * @param {ValidatedFetchOptions} [validationOptions]
 * @returns {Promise<Response>}
 */
export async function fetchWithValidatedRedirects(url, options = {}, validationOptions = {}) {
  const fetchImpl = validationOptions.fetch || globalThis.fetch;
  if (typeof fetchImpl !== 'function') {
    throw new Error('fetch implementation is required');
  }

  const maxRedirects = Number.isSafeInteger(validationOptions.maxRedirects)
    ? /** @type {number} */ (validationOptions.maxRedirects)
    : DEFAULT_MAX_REDIRECTS;
  let currentUrl = String(url);
  let currentOptions = { ...options };

  for (let redirectCount = 0; ; redirectCount += 1) {
    await validateResolvedFetchUrl(currentUrl, {
      lookup: validationOptions.lookup,
      cache: validationOptions.cache,
    });

    const response = await fetchImpl(currentUrl, {
      ...currentOptions,
      redirect: 'manual',
    });

    if (!REDIRECT_STATUSES.has(response.status)) {
      return response;
    }

    const location = readHeader(response.headers, 'location');
    if (!location) {
      return response;
    }
    if (redirectCount >= maxRedirects) {
      throw new Error(`Too many redirects while fetching ${url}`);
    }

    currentUrl = new URL(location, currentUrl).toString();
    currentOptions = optionsForRedirect(currentOptions, response.status);
  }
}

/**
 * Validate URL syntax and resolve DNS before fetching.
 *
 * This closes the gap where a public-looking hostname resolves to loopback,
 * private, link-local, metadata, multicast, or reserved address space.
 *
 * @param {string} url
 * @param {{ lookup?: DnsLookup, cache?: boolean }} [options]
 * @returns {Promise<void>}
 */
export async function validateResolvedFetchUrl(url, options = {}) {
  validateFetchUrl(url);

  const parsed = new URL(url);
  const host = normalizeHostname(parsed.hostname);
  if (net.isIP(host) || isDocumentationHostname(host)) {
    return;
  }

  const useCache = options.cache !== false;
  const cached = dnsValidationCache.get(host);
  if (useCache && cached && cached.expiresAt > Date.now()) {
    throw new Error(cached.error);
  }

  const lookup = options.lookup || dns.lookup;
  let records;
  try {
    records = await lookup(host, { all: true, verbatim: true });
  } catch (err) {
    const message = `Unable to resolve URL host ${host}: ${err instanceof Error ? err.message : String(err)}`;
    if (useCache) {
      dnsValidationCache.set(host, {
        expiresAt: Date.now() + DNS_CACHE_TTL_MS,
        error: message,
      });
    }
    throw new Error(message);
  }

  /** @type {string[]} */
  const addresses = [];
  for (const record of Array.isArray(records) ? records : [records]) {
    const address = typeof record === 'string' ? record : record?.address;
    if (typeof address === 'string' && address) {
      addresses.push(address);
    }
  }

  if (addresses.length === 0) {
    throw new Error(`Unable to resolve URL host ${host}: no DNS addresses returned`);
  }

  const blocked = addresses.find((address) => isBlockedIpAddress(address));
  if (blocked) {
    const message = `SSRF blocked: ${host} resolves to internal address ${blocked}`;
    if (useCache) {
      dnsValidationCache.set(host, {
        expiresAt: Date.now() + DNS_CACHE_TTL_MS,
        error: message,
      });
    }
    throw new Error(message);
  }

  // Successful DNS validations are intentionally not cached. Hostnames can
  // legitimately change, and a stale allow decision weakens SSRF protection.
}
