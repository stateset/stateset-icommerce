import { NextResponse } from 'next/server';
import { assessStoreConfiguration } from '@/lib/store-readiness.js';
import { inspectStoreDatabase } from '@/lib/store-database-check.js';
import { getCheckoutConfiguration } from '@/lib/checkout-config.js';

export const runtime = 'nodejs';
export const dynamic = 'force-dynamic';

export async function GET() {
  const options = { launch: process.env.NODE_ENV === 'production' };
  const configuration = assessStoreConfiguration(process.env, options);
  const database = inspectStoreDatabase(process.env, options);
  const checkoutAvailable = configuration.configurationReady && database.status !== 'fail';
  return NextResponse.json(
    {
      checkoutAvailable,
      ...(checkoutAvailable && {
        merchantAddress: getCheckoutConfiguration(process.env).merchantAddress,
        chainId: 8453,
      }),
    },
    { status: checkoutAvailable ? 200 : 503, headers: { 'Cache-Control': 'no-store' } },
  );
}
