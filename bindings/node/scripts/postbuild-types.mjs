#!/usr/bin/env node
/**
 * Append the hand-written declarations in `index-augment.d.ts` to the
 * napi-generated `index.d.ts`. Idempotent: a previous appended block is
 * replaced, never duplicated. `test/lifecycle.js` fails if the block is
 * missing, so a bare `napi build` (which rewrites index.d.ts) cannot ship
 * declarations that lack the JavaScript-side surface.
 *
 * Also drops the `__test*` probe declarations. They are compiled only with the
 * `test-panic` cargo feature, which `npm run build:debug` turns on, so a debug
 * build would otherwise leave four declarations in `index.d.ts` that no
 * published binary provides — and anything generated from the declarations
 * (the tool catalog, the API reference) would carry them too. Stripping here
 * means a debug build and a release build produce the same file, so running
 * the tests never dirties the tree. The runtime exports in `native-binding.js`
 * are left alone: `test/panic-containment.js` needs them after a debug build.
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
const TEST_PROBE = /(?:\/\*\*(?:[^*]|\*(?!\/))*\*\/\n)?export declare function __test\w+\([^\n]*\n/g;
const probes = generated.match(TEST_PROBE)?.length ?? 0;
if (probes > 0) {
  generated = generated.replace(TEST_PROBE, '');
  console.log(`postbuild-types: dropped ${probes} __test* probe declaration(s)`);
}
const start = generated.indexOf(BEGIN);
if (start !== -1) {
  const end = generated.indexOf(END, start);
  if (end === -1) throw new Error(`${target}: augmentation block has no end marker`);
  generated = generated.slice(0, start) + generated.slice(end + END.length);
}
// Normalise the tail so repeated runs cannot accumulate blank lines.
writeFileSync(target, `${generated.replace(/\s+$/, '')}\n\n${augment}`);
console.log(`postbuild-types: appended ${path.basename(target)} augmentation`);
