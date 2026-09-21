#!/usr/bin/env node
/**
 * Append the hand-written declarations in `index-augment.d.ts` to the
 * napi-generated `index.d.ts`. Idempotent: a previous appended block is
 * replaced, never duplicated. `test/lifecycle.js` fails if the block is
 * missing, so a bare `napi build` (which rewrites index.d.ts) cannot ship
 * declarations that lack the JavaScript-side surface.
 */
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const target = path.join(here, '..', 'index.d.ts');
const BEGIN = '// ---- BEGIN hand-written additions';
const END = '// ---- END hand-written additions ----\n';
const fragmentsDir = path.join(here, 'types');
const fragments = readdirSync(fragmentsDir)
  .filter((name) => name.endsWith('.d.ts'))
  .sort()
  .map((name) => `// ---- fragment: scripts/types/${name} ----\n` + readFileSync(path.join(fragmentsDir, name), 'utf8'));
const augment = readFileSync(path.join(here, 'index-augment.d.ts'), 'utf8').replace(
  END,
  fragments.join('\n') + (fragments.length ? '\n' : '') + END,
);

let generated = readFileSync(target, 'utf8');
const start = generated.indexOf(BEGIN);
if (start !== -1) {
  const end = generated.indexOf(END, start);
  if (end === -1) throw new Error(`${target}: augmentation block has no end marker`);
  generated = generated.slice(0, start) + generated.slice(end + END.length);
}
const separator = generated.endsWith('\n') ? '\n' : '\n\n';
writeFileSync(target, generated + separator + augment);
console.log(`postbuild-types: appended ${path.basename(target)} augmentation`);
