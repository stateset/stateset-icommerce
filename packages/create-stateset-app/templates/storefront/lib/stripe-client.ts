import Stripe from 'stripe';
import { getStripeConfiguration, getStripePlatformConfiguration } from './stripe-config.js';

export function storefrontStripe() {
  const config = getStripeConfiguration(process.env);
  return { ...config, stripe: new Stripe(config.key, { maxNetworkRetries: 2, timeout: 12000 }) };
}

export function payoutStripe() {
  const config = getStripePlatformConfiguration(process.env);
  return new Stripe(config.key, { maxNetworkRetries: 2, timeout: 12000 });
}
