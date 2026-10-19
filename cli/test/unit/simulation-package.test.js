import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

test('npm package includes every README-linked simulation example and built-in input', () => {
  const cli = new URL('../../', import.meta.url);
  const readme = readFileSync(new URL('README.md', cli), 'utf8');
  const examples = new Set([
    'examples/commerce-scenario.json',
    'examples/inventory-plan.json',
    'examples/inventory-experiment.json',
    ...Array.from(readme.matchAll(/\]\((examples\/[^\s)#]+\.json)\)/g), (match) => match[1]),
  ]);
  const [packed] = JSON.parse(
    execFileSync('npm', ['pack', '--dry-run', '--json', '--ignore-scripts'], {
      cwd: fileURLToPath(cli),
      encoding: 'utf8',
      timeout: 60_000,
      maxBuffer: 16 * 1024 * 1024,
    }),
  );
  const files = new Set(packed.files.map((file) => file.path));
  for (const example of examples) {
    assert.ok(files.has(example), `Published CLI omits simulation example: ${example}`);
    assert.doesNotThrow(() => JSON.parse(readFileSync(new URL(example, cli), 'utf8')));
  }
});
