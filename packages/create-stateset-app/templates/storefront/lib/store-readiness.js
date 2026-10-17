import { getCheckoutConfiguration } from './checkout-config.js';
import { getTaxProvider } from './tax.js';
import { getShippingProvider } from './shipping.js';

// Configuration checks only: no provider calls, settlements, or database writes.
export function assessStoreConfiguration(environment, { launch = false } = {}) {
  const checks = [];
  const add = (id, status, message, action = null) => checks.push({ id, status, message, action });
  try {
    const config = getCheckoutConfiguration(environment);
    add('payment', 'pass', 'USDC on Base is configured; provider connectivity is not checked.');
    if (launch && !config.rpcUrl.startsWith('https:')) {
      add(
        'payment-transport',
        'fail',
        'The payment RPC URL must use HTTPS for launch.',
        'Set STATESET_BASE_RPC_URL to your HTTPS Base RPC endpoint.',
      );
    }
  } catch {
    add(
      'payment',
      'fail',
      'Payment configuration is incomplete or invalid.',
      'Set a non-zero NEXT_PUBLIC_STORE_WALLET_ADDRESS, an HTTP(S) STATESET_BASE_RPC_URL, and positive integer STATESET_USDC_CONFIRMATIONS.',
    );
  }
  try {
    const tax = getTaxProvider(environment);
    if (tax.source === 'starter') {
      add(
        'tax',
        launch ? 'fail' : 'warn',
        'Tax uses the demo state-rate table.',
        'Set STATESET_TAX_RATES_JSON to your reviewed jurisdictions and exact decimal-string rates.',
      );
    } else if (Object.keys(JSON.parse(environment.STATESET_TAX_RATES_JSON)).length === 0) {
      add(
        'tax',
        'fail',
        'The tax rate table has no jurisdictions.',
        'Configure at least one supported US state and its exact rate.',
      );
    } else {
      add(
        'tax',
        'pass',
        'An operator tax rate table is configured; tax compliance is not verified.',
      );
    }
  } catch {
    add(
      'tax',
      'fail',
      'The tax rate table is invalid.',
      'Set STATESET_TAX_RATES_JSON to an object of jurisdiction codes and decimal-string rates between 0 and 1.',
    );
  }
  try {
    const shipping = getShippingProvider(environment);
    const methods = shipping.listMethods({
      firstName: 'Setup',
      lastName: 'Check',
      line1: '1 Main St',
      city: 'Portland',
      state: 'OR',
      postalCode: '97201',
      country: 'US',
    });
    if (!methods.length) {
      add(
        'shipping',
        'fail',
        'No shipping method serves the supported US checkout.',
        'Configure at least one shipping method with countries: ["US"].',
      );
    } else if (shipping.source === 'starter') {
      add(
        'shipping',
        launch ? 'fail' : 'warn',
        'Shipping uses the demo free-shipping method.',
        'Set STATESET_SHIPPING_METHODS_JSON to your fulfillment methods and exact decimal-string charges; a reviewed zero charge is allowed.',
      );
    } else {
      add(
        'shipping',
        'pass',
        'Operator shipping rates are configured; carrier booking and tracking are not connected.',
      );
    }
  } catch {
    add(
      'shipping',
      'fail',
      'Shipping configuration is invalid.',
      'Set STATESET_SHIPPING_METHODS_JSON to non-empty methods with unique ids, labels, decimal-string amounts and supported countries.',
    );
  }
  try {
    const site = new URL(environment.NEXT_PUBLIC_BASE_URL || 'http://localhost:3000');
    if (
      !['http:', 'https:'].includes(site.protocol) ||
      site.username ||
      site.password ||
      site.pathname !== '/' ||
      site.search ||
      site.hash
    )
      throw new Error();
    const local = ['localhost', '127.0.0.1', '[::1]'].includes(site.hostname);
    add(
      'store-url',
      launch && (site.protocol !== 'https:' || local) ? 'fail' : local ? 'warn' : 'pass',
      local ? 'The storefront URL is local.' : 'The storefront URL is configured.',
      launch && (site.protocol !== 'https:' || local)
        ? 'Set NEXT_PUBLIC_BASE_URL to your public HTTPS storefront origin before building.'
        : null,
    );
  } catch {
    add(
      'store-url',
      'fail',
      'The storefront URL is invalid.',
      'Set NEXT_PUBLIC_BASE_URL to an HTTP(S) origin without a path, query or credentials.',
    );
  }
  add(
    'chat',
    environment.ANTHROPIC_API_KEY?.trim() ? 'pass' : 'warn',
    environment.ANTHROPIC_API_KEY?.trim()
      ? 'AI chat has a configured key; connectivity is not checked.'
      : 'Optional AI chat is not configured.',
    environment.ANTHROPIC_API_KEY?.trim()
      ? null
      : 'Set ANTHROPIC_API_KEY if you want AI shopping assistance.',
  );
  return {
    schemaVersion: 1,
    scope: 'local-configuration',
    mode: launch ? 'launch' : 'development',
    configurationReady: checks.every((check) => check.status !== 'fail'),
    checks,
    limitations: [
      'No external providers were contacted.',
      'Configuration does not verify settlement, tax compliance, carrier fulfillment or production reliability.',
    ],
  };
}
