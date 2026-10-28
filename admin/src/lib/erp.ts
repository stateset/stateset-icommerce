import 'server-only';
import type { Commerce } from '@stateset/embedded';
import { getCommerceEngine } from './embedded';

/** The native ERP surface; demo engines cannot simulate financial mutations. */
export async function getErpEngine(): Promise<Commerce> {
  const engine = (await getCommerceEngine()) as unknown as Commerce;
  if (typeof engine.accountsPayable?.createPayment !== 'function') {
    throw new Error(
      'ERP actions require the current native commerce engine. Demo mode supports viewing only.',
    );
  }
  return engine;
}
