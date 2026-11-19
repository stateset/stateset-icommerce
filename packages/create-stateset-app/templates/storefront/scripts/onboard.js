import { readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { importNextCatalog } from '../lib/next-catalog.js';
import { parseArgs } from 'node:util';
import { Commerce } from '@stateset/embedded';
import Stripe from 'stripe';
import { loadStoreEnvironment } from '../lib/store-environment.js';
import {
  importCatalog,
  localLaunchChecks,
  validateCatalog,
  verifyStripeConnection,
  runLaunchDrill,
} from '../lib/merchant-onboarding.js';
import { pricingEnvironment } from '../lib/store-pricing.js';
import { getStripeConfiguration } from '../lib/stripe-config.js';

const { values } = parseArgs({
  options: {
    catalog: { type: 'string' },
    'next-catalog': { type: 'string' },
    'source-store': { type: 'string' },
    reviewed: { type: 'string' },
    apply: { type: 'boolean' },
    provider: { type: 'boolean' },
    purchase: { type: 'string' },
    finish: { type: 'string' },
    key: { type: 'string' },
    email: { type: 'string' },
    address: { type: 'string' },
    shipping: { type: 'string' },
    refund: { type: 'string' },
    wallet: { type: 'string' },
  },
});
let commerce;
try {
  await loadStoreEnvironment();
  const env = process.env;
  if (values['next-catalog']) {
    if (
      Object.keys(values).some(
        (k) => !['next-catalog', 'source-store', 'apply', 'reviewed'].includes(k),
      )
    )
      throw new Error(
        'Run a Next Commerce catalog import separately from other onboarding operations',
      );
    const file = JSON.parse(await readFile(values['next-catalog'], 'utf8'));
    const settings = { sourceStore: values['source-store'], expectedFingerprint: values.reviewed };
    // Validate before creating a new SQLite file, even when apply was requested.
    await importNextCatalog(null, file, settings);
    if (values.apply && !values.reviewed)
      throw new Error('Preview first, then supply --reviewed FINGERPRINT with --apply');
    const db = env.STATESET_DB_PATH || './store.db';
    if (db === ':memory:') throw new Error('Set STATESET_DB_PATH to a persistent store file');
    if (existsSync(db) || values.apply) commerce = new Commerce(db);
    console.log(
      JSON.stringify(
        await importNextCatalog(commerce, file, { ...settings, apply: values.apply }),
        null,
        2,
      ),
    );
  } else {
    if (values['source-store'] || values.reviewed)
      throw new Error('Supply --next-catalog FILE for this import');
    const report = { mode: values.apply ? 'apply' : 'preview', checks: localLaunchChecks(env) };
    if (values.catalog) {
      const rows = validateCatalog(JSON.parse(await readFile(values.catalog, 'utf8')));
      if (values.apply) {
        commerce = new Commerce(env.STATESET_DB_PATH || './store.db');
        report.catalog = await importCatalog(commerce, rows);
        report.checks = localLaunchChecks(env);
      } else report.catalog = { rows: rows.length, action: 'import' };
    }
    if (values.provider || values.purchase || values.finish) {
      const config = getStripeConfiguration(env);
      const stripe = new Stripe(config.key, { maxNetworkRetries: 2, timeout: 12000 });
      report.processor = await verifyStripeConnection(
        stripe,
        config.origin,
        env.STATESET_RENEWAL_RECOVERY === 'true',
      );
      if (values.purchase || values.finish) {
        if (report.checks.find((row) => row.id === 'database').status !== 'pass')
          throw new Error('Import a catalog into an existing test store first');
        commerce ||= new Commerce(env.STATESET_DB_PATH || './store.db');
        const input = {
          ...values,
          address: values.address ? JSON.parse(await readFile(values.address, 'utf8')) : undefined,
        };
        const drill = await runLaunchDrill(commerce, stripe, env, input);
        Object.assign(report, drill);
        report.processor.settlementVerified = Boolean(drill.settlementVerified);
      }
    }
    if (commerce) report.checks = localLaunchChecks(await pricingEnvironment(commerce, env));
    console.log(JSON.stringify(report, null, 2));
    if (
      report.checks.some((check) => check.status === 'fail') ||
      (report.processor &&
        (!report.processor.chargesEnabled || !report.processor.checkoutWebhookRegistered))
    )
      process.exitCode = 1;
  }
} catch (error) {
  console.error(error instanceof Error ? error.message : 'Onboarding failed');
  process.exitCode = 1;
} finally {
  if (commerce) await commerce.close();
}
