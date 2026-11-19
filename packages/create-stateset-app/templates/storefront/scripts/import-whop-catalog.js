import { existsSync } from 'node:fs';
import { withCatalogStore } from './catalog-store.js';
import { readFile } from 'node:fs/promises';
import { parseArgs } from 'node:util';
import { loadStoreEnvironment } from '../lib/store-environment.js';
import { importWhopCatalog } from '../lib/whop-catalog.js';

try {
  const { values, positionals } = parseArgs({
    allowPositionals: true,
    options: {
      apply: { type: 'boolean' },
      fingerprint: { type: 'string' },
    },
  });
  if (positionals.length !== 1)
    throw new Error(
      'Usage: node scripts/import-whop-catalog.js BUNDLE.json [--apply] [--fingerprint SHA256]',
    );
  const payload = JSON.parse(await readFile(positionals[0], 'utf8'));
  await loadStoreEnvironment();
  const path = process.env.STATESET_DB_PATH || './store.db';
  await importWhopCatalog(null, payload, {
    expectedFingerprint: existsSync(path) ? undefined : values.fingerprint,
  });
  const report = await withCatalogStore(path, values.apply, (commerce) =>
    importWhopCatalog(commerce, payload, {
      apply: values.apply,
      expectedFingerprint: values.fingerprint,
    }),
  );
  console.log(JSON.stringify(report, null, 2));
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
}
