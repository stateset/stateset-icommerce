/**
 * Child-process entry for the oracle. See `runOracleIsolated` in oracle.mjs
 * for why the oracle never reads the database from the engine's process.
 */
import { runOracle } from './oracle.mjs';

process.once('message', async (input) => {
  try {
    const result = await runOracle(input);
    process.send({ ok: true, result }, () => process.exit(0));
  } catch (error) {
    process.send({ ok: false, error: String(error?.stack ?? error) }, () => process.exit(2));
  }
});
