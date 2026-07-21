/**
 * Display-only money formatting for finance surfaces.
 *
 * The engine reports some amounts as exact decimal strings. Those must NEVER
 * be run through parseFloat/Number for arithmetic — this module only groups
 * digits and attaches a currency symbol via string manipulation, so exact
 * values survive verbatim.
 */

const DECIMAL_STRING = /^-?\d+(\.\d+)?$/;

/**
 * Format an exact decimal string (or an engine-reported number) for display.
 *
 * - Strings are formatted purely via string manipulation (no float parsing).
 * - Numbers (legacy engine outputs) are formatted with toFixed(2) for
 *   display only.
 * - Anything that is not a plain decimal is returned verbatim so bad input
 *   is visible instead of silently coerced.
 */
export function formatMoney(value: string | number, currency: string = 'USD'): string {
  const raw = typeof value === 'number' ? value.toFixed(2) : value.trim();
  if (!DECIMAL_STRING.test(raw)) {
    return raw;
  }

  const negative = raw.startsWith('-');
  const unsigned = negative ? raw.slice(1) : raw;
  const [intPart, fracPart = ''] = unsigned.split('.');
  const grouped = intPart.replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  const fraction = fracPart.length >= 2 ? fracPart : `${fracPart}00`.slice(0, 2);
  const symbol = currency === 'USD' ? '$' : `${currency} `;

  return `${negative ? '-' : ''}${symbol}${grouped}.${fraction}`;
}
