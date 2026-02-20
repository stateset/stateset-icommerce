/**
 * Privacy & Redaction Utilities
 */

const DEFAULT_PATTERNS = [
  { name: 'email', re: /\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b/gi, replace: '[email]' },
  {
    name: 'phone',
    re: /\b(?:\+?\d{1,3}[-.\s]?)?(?:\(?\d{3}\)?[-.\s]?)\d{3}[-.\s]?\d{4}\b/g,
    replace: '[phone]',
  },
  { name: 'card', re: /\b(?:\d[ -]*?){13,19}\b/g, replace: '[card]' },
  { name: 'api_key', re: /\bsk-[A-Za-z0-9]{16,}\b/g, replace: '[api_key]' },
  { name: 'slack_token', re: /\bxox[baprs]-[A-Za-z0-9-]{10,}\b/g, replace: '[token]' },
  { name: 'github_token', re: /\bgh[pousr]_[A-Za-z0-9]{20,}\b/g, replace: '[token]' },
];

export function redactSensitive(text, options = {}) {
  if (!text || typeof text !== 'string') return text;
  if (options.enabled === false) return text;

  let redacted = text;
  const patterns = options.patterns || DEFAULT_PATTERNS;
  for (const pattern of patterns) {
    redacted = redacted.replace(pattern.re, pattern.replace);
  }
  return redacted;
}

export function redactObject(value, options = {}) {
  if (!value) return value;
  if (typeof value === 'string') return redactSensitive(value, options);
  try {
    const json = JSON.stringify(value);
    return JSON.parse(redactSensitive(json, options));
  } catch (err) {
    console.debug('[privacy] Deep redaction failed:', err.message || err);
    return value;
  }
}
