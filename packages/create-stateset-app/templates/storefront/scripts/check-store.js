import { parseArgs } from 'node:util';
import { assessStoreConfiguration } from '../lib/store-readiness.js';
import { inspectStoreDatabase } from '../lib/store-database-check.js';
import { loadStoreEnvironment } from '../lib/store-environment.js';

let json = false;
try {
  const { values } = parseArgs({
    options: {
      launch: { type: 'boolean' },
      json: { type: 'boolean' },
      help: { type: 'boolean' },
      'no-env-files': { type: 'boolean' },
    },
  });
  if (values.help) {
    console.log(
      'Usage: npm run doctor -- [--launch] [--json] [--no-env-files]\nRead-only configuration check. --launch rejects demo defaults. Loads Next.js environment files unless --no-env-files uses only the process environment. Never creates a store or contacts providers.',
    );
  } else {
    json = Boolean(values.json);
    if (!values['no-env-files']) await loadStoreEnvironment({ development: !values.launch });
    const options = { launch: Boolean(values.launch) };
    const report = assessStoreConfiguration(process.env, options);
    report.checks.push(inspectStoreDatabase(process.env, options));
    report.configurationReady = report.checks.every((check) => check.status !== 'fail');
    if (values.json) console.log(JSON.stringify(report, null, 2));
    else {
      console.log(
        `Store configuration: ${report.configurationReady ? 'checks passed' : 'needs attention'} (${report.mode})\n`,
      );
      for (const check of report.checks) {
        console.log(`${check.status.toUpperCase()} ${check.id}: ${check.message}`);
        if (check.action) console.log(`  ${check.action}`);
      }
      console.log('\n' + report.limitations.join('\n'));
    }
    process.exitCode = report.configurationReady ? 0 : 1;
  }
} catch {
  const message =
    'Store check could not run. Use supported options (--help), install dependencies with npm install, and review environment file permissions.';
  if (json)
    console.log(
      JSON.stringify({
        schemaVersion: 1,
        configurationReady: false,
        checks: [{ id: 'environment', status: 'fail', message }],
      }),
    );
  else console.error(message);
  process.exitCode = 1;
}
