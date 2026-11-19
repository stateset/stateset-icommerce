import { NextResponse } from 'next/server';
import { assessStoreConfiguration } from '@/lib/store-readiness.js';
import { inspectStoreDatabase } from '@/lib/store-database-check.js';
import { getCheckoutConfiguration } from '@/lib/checkout-config.js';
import { getCommerce } from '@/lib/commerce';
import { pricingEnvironment } from '@/lib/store-pricing.js';

export const runtime = 'nodejs';
export const dynamic = 'force-dynamic';

export async function GET() {
  const options = { launch: process.env.NODE_ENV === 'production' };
  const database = inspectStoreDatabase(process.env, options);
  const env = database.status === 'pass' ? await pricingEnvironment(getCommerce()) : process.env;
  const configuration = assessStoreConfiguration(env, options);
  const checkoutAvailable = configuration.configurationReady && database.status !== 'fail';
  return NextResponse.json(
    {
      checkoutAvailable,
      provider: process.env.STATESET_CHECKOUT_PROVIDER || 'base',
      ...(checkoutAvailable && (!process.env.STATESET_CHECKOUT_PROVIDER || process.env.STATESET_CHECKOUT_PROVIDER === 'base') && {
        merchantAddress: getCheckoutConfiguration(process.env).merchantAddress,
        chainId: 8453,
      }),
    },
    { status: checkoutAvailable ? 200 : 503, headers: { 'Cache-Control': 'no-store' } },
  );
}
