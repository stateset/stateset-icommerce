import { existsSync } from 'node:fs';
import { readFile } from 'node:fs/promises';
import { parseArgs } from 'node:util';
import { loadStoreEnvironment } from '../lib/store-environment.js';
import { mapWhopCatalog, importWhopCatalog } from '../lib/whop-catalog.js';

let commerce;
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
  if (existsSync(path) || values.apply) {
    // Check the complete source before opening a new native database.
    mapWhopCatalog(payload);
    const { Commerce } = await import('@stateset/embedded');
    commerce = new Commerce(path);
  }
  console.log(
    JSON.stringify(
      await importWhopCatalog(commerce, payload, {
        apply: values.apply,
        expectedFingerprint: values.fingerprint,
      }),
      null,
      2,
    ),
  );
} catch (error) {
  console.error(error.message);
  process.exitCode = 1;
} finally {
  if (commerce) await commerce.close();
}
