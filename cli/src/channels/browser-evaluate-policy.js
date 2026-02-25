/**
 * Read-only policy for /browser/evaluate route.
 *
 * This policy intentionally allows only narrow browser queries.
 */

const MAX_BROWSER_EXPRESSION_LENGTH = 4_000;

const SAFE_BROWSER_LITERAL_PATTERNS = [
  /^document\.(title|readyState|URL|baseURI|referrer)$/,
  /^window\.(innerWidth|innerHeight|outerWidth|outerHeight|devicePixelRatio|scrollX|scrollY)$/,
  /^[\d+\-*/%().\s]+$/,
];

const SAFE_BROWSER_SELECTOR_PATTERNS = [
  /^document\.querySelector\(\s*(["'])(?:\\.|(?!\1)[^\\\r\n]){1,512}\1\s*\)\.(textContent|innerText|innerHTML|value|href|src|id|className)$/,
  /^document\.getElementById\(\s*(["'])(?:\\.|(?!\1)[^\\\r\n]){1,512}\1\s*\)\.(textContent|innerText|innerHTML|value|href|src|id|className)$/,
  /^document\.querySelectorAll\(\s*(["'])(?:\\.|(?!\1)[^\\\r\n]){1,512}\1\s*\)\.length$/,
];

/**
 * Validate expression for /browser/evaluate.
 *
 * @param {unknown} expression
 * @returns {string|null}
 */
export function validateBrowserExpression(expression) {
  if (typeof expression !== 'string') {
    return 'Missing required field: expression';
  }

  const trimmed = expression.trim();
  if (!trimmed) {
    return 'Missing required field: expression';
  }

  if (trimmed.length > MAX_BROWSER_EXPRESSION_LENGTH) {
    return `Expression exceeds maximum length of ${MAX_BROWSER_EXPRESSION_LENGTH} characters`;
  }

  for (const pattern of SAFE_BROWSER_LITERAL_PATTERNS) {
    if (pattern.test(trimmed)) {
      return null;
    }
  }

  for (const pattern of SAFE_BROWSER_SELECTOR_PATTERNS) {
    if (pattern.test(trimmed)) {
      return null;
    }
  }

  return 'Expression is restricted to read-only browser queries';
}
