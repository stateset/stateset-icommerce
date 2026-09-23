#!/usr/bin/env node
/**
 * Everything that runs after `napi build` has rewritten the generated files.
 * Order matters: declarations first, then anything derived from them.
 */
import { existsSync } from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const steps = [
  'postbuild-types.mjs',
  'generate-tool-descriptors.mjs',
  'generate-api-reference.mjs',
];
for (const step of steps) {
  const script = path.join(here, step);
  if (!existsSync(script)) continue;
  const result = spawnSync(process.execPath, [script], { stdio: 'inherit' });
  if (result.status !== 0) {
    console.error(`postbuild: ${step} failed`);
    process.exit(result.status ?? 1);
  }
}
