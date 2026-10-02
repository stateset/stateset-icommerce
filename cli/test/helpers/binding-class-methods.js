/**
 * Method names a class declares in the Node binding's generated
 * `bindings/node/index.d.ts`, so tool tests can prove their mocks (and the
 * tools themselves) only use methods the binding really has.
 */

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const DTS = path.resolve(here, '../../../bindings/node/index.d.ts');

/** @param {string} className @returns {Set<string>} */
export function bindingClassMethods(className) {
  const source = readFileSync(DTS, 'utf8');
  const start = source.indexOf(`export declare class ${className} {`);
  if (start === -1) throw new Error(`class ${className} not found in ${DTS}`);
  const end = source.indexOf('\n}', start);
  const body = source.slice(start, end);
  return new Set([...body.matchAll(/^\s+(\w+)\(/gm)].map((m) => m[1]));
}

/** Every `commerce.<api>.<method>(` call in a tool module's source. */
export function toolModuleCalls(toolFile, api) {
  const source = readFileSync(path.resolve(here, '../../src/tools', toolFile), 'utf8');
  const re = new RegExp(`\\bcommerce\\.${api}\\.(\\w+)\\s*\\(`, 'g');
  return new Set([...source.matchAll(re)].map((m) => m[1]));
}
