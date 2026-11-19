import { readFile } from 'node:fs/promises';
import { existsSync } from 'node:fs';
import { importExternalCatalog } from '../lib/catalog-import.js';
import { withCatalogStore } from './catalog-store.js';
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
    'whop-catalog': { type: 'string' },
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
  if (values['next-catalog'] || values['whop-catalog']) {
    const format = values['next-catalog'] ? 'next' : 'whop';
    const fileOption = format + '-catalog';
    const allowed = [
      fileOption,
      'apply',
      'reviewed',
      ...(format === 'next' ? ['source-store'] : []),
    ];
    if (Object.keys(values).some((k) => !allowed.includes(k)))
      throw new Error('Run one catalog import at a time.');
    if (values.apply && !/^[a-f0-9]{64}$/.test(values.reviewed || ''))
      throw new Error('Preview first, then supply --reviewed FINGERPRINT with --apply');
    const file = JSON.parse(await readFile(values[fileOption], 'utf8'));
    const db = env.STATESET_DB_PATH || './store.db';
    const settings = { sourceStore: values['source-store'], expectedFingerprint: values.reviewed };
    // Validate all input before opening the destination, including for an initial apply.
    await importExternalCatalog(null, format, file, {
      ...settings,
      expectedFingerprint: existsSync(db) ? undefined : values.reviewed,
    });
    const report = await withCatalogStore(db, values.apply, (target) =>
      importExternalCatalog(target, format, file, { ...settings, apply: values.apply }),
    );
    console.log(JSON.stringify(report, null, 2));
  } else {
    if (values['source-store'] || values.reviewed)
      throw new Error('Choose --next-catalog FILE or --whop-catalog FILE for this import');
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
