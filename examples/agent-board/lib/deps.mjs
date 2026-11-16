/**
 * Native dependencies, resolved exactly as the CLI resolves them.
 *
 * The harness is a CLI-adjacent example: it imports the CLI's sync modules by
 * relative path and loads `better-sqlite3` and `@stateset/embedded` from the
 * CLI package, so it runs from a checkout after `npm install` in `cli/` with
 * nothing else to install, and so a stray global copy of either package can
 * never be picked up instead.
 */

import { createRequire } from 'node:module';

const cliRequire = createRequire(new URL('../../../cli/package.json', import.meta.url));

function load(name) {
  try {
    return cliRequire(name);
  } catch (error) {
    throw new Error(
      `${name} is not installed for the CLI (${error.code ?? error.message}); run \`npm install\` in cli/ first`,
    );
  }
}

/** @returns {typeof import('better-sqlite3')} */
export function loadDatabase() {
  return load('better-sqlite3');
}

/** @returns {typeof import('@stateset/embedded').Commerce} */
export function loadCommerce() {
  const { Commerce } = load('@stateset/embedded');
  if (typeof Commerce !== 'function') {
    throw new Error('@stateset/embedded does not export Commerce');
  }
  return Commerce;
}
