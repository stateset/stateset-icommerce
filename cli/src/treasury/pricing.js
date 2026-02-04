/**
 * Treasury pricing helpers.
 */

export const STABLECOIN_SYMBOLS = new Set([
  'USDC',
  'USDT',
  'DAI',
  'SSUSD',
  'WSSUSD'
]);

export function normalizeSymbol(symbol) {
  if (!symbol) return '';
  return symbol.trim().toUpperCase();
}

export function isStablecoinSymbol(symbol) {
  return STABLECOIN_SYMBOLS.has(normalizeSymbol(symbol));
}

export function resolveTokenPriceUsd(token, overrides = {}) {
  if (!token) return null;
  if (overrides && overrides.priceUsd != null) {
    const override = Number(overrides.priceUsd);
    return Number.isFinite(override) ? override : null;
  }

  if (isStablecoinSymbol(token.symbol)) {
    return 1;
  }

  const price = token.priceUsd ?? token.price_usd ?? null;
  if (price == null) return null;
  const numeric = Number(price);
  return Number.isFinite(numeric) ? numeric : null;
}

export default {
  STABLECOIN_SYMBOLS,
  normalizeSymbol,
  isStablecoinSymbol,
  resolveTokenPriceUsd
};
