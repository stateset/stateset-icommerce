import { parseArgs } from 'node:util';
import { Commerce } from '@stateset/embedded';
import { issuePortal, portalUrl } from '../lib/merchant-portal.js';
import { loadStoreEnvironment } from '../lib/store-environment.js';

let commerce;
try {
  const { values } = parseArgs({ options: { apply: { type: 'boolean' } } });
  await loadStoreEnvironment({ development: process.env.NODE_ENV !== 'production' });
  const database = process.env.STATESET_DB_PATH || './store.db';
  if (database === ':memory:' || /^(?:file:|[a-z]+:\/\/)/i.test(database))
    throw new Error('Set STATESET_DB_PATH to a persistent SQLite file path for merchant setup.');
  const origin = process.env.NEXT_PUBLIC_BASE_URL || 'http://localhost:3000';
  // Validate the link origin before initializing a new store.
  portalUrl(origin);
  if (values.apply) {
    commerce = new Commerce(database);
    const grant = await issuePortal(commerce, origin, 'merchant', 'shop', 1, true);
    console.log('Start the storefront, then open your private merchant setup link.');
    console.log(grant.url);
    console.log(
      'This link grants merchant access until ' + new Date(grant.expires).toISOString() + '.',
    );
    console.log('Revoke it early with npm run merchant -- revoke-portal ' + grant.id + ' --apply');
  } else {
    console.log('Setup will open or create ' + database + ' and issue a one-day merchant link.');
    console.log('Run npm run setup -- --apply to continue.');
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : 'Merchant setup failed.');
  process.exitCode = 1;
} finally {
  if (commerce) await commerce.close();
}
