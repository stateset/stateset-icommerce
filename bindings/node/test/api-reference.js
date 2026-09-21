/**
 * Staleness guard for the generated Node API reference.
 *
 * `docs/src/api/node-reference.md` is derived from `index.d.ts` by
 * `scripts/generate-api-reference.mjs`. This test regenerates the page in
 * memory and compares it with the committed file, so a change to the
 * declarations (a `napi build`, a new hand-written fragment) cannot ship
 * without the reference being regenerated too.
 */

'use strict';

const assert = require('assert');
const fs = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');

const GENERATOR = path.join(__dirname, '..', 'scripts', 'generate-api-reference.mjs');

async function loadGenerator() {
  return import(GENERATOR);
}

test('the committed node-reference.md matches what the generator produces', async () => {
  const { generateApiReference, DECLARATIONS_PATH, OUTPUT_PATH } = await loadGenerator();
  const source = fs.readFileSync(DECLARATIONS_PATH, 'utf8');
  const { markdown, warnings } = generateApiReference(source);
  assert.deepStrictEqual(warnings, [], `index.d.ts contains constructs the reference generator does not handle:\n${warnings.join('\n')}`);

  let committed = null;
  try {
    committed = fs.readFileSync(OUTPUT_PATH, 'utf8');
  } catch {
    committed = null;
  }
  const relative = path.relative(path.join(__dirname, '..'), OUTPUT_PATH);
  assert.ok(committed !== null, `${relative} is missing; run \`node scripts/generate-api-reference.mjs\` in bindings/node and commit the result`);
  assert.strictEqual(
    committed,
    markdown,
    `${relative} is stale relative to index.d.ts; run \`node scripts/generate-api-reference.mjs\` in bindings/node and commit the result`,
  );
});

test('the generator is deterministic and covers the whole Commerce surface', async () => {
  const { generateApiReference, DECLARATIONS_PATH } = await loadGenerator();
  const source = fs.readFileSync(DECLARATIONS_PATH, 'utf8');
  const first = generateApiReference(source);
  const second = generateApiReference(source);
  assert.strictEqual(first.markdown, second.markdown);

  // Every `get x(): Class` accessor on Commerce must have its own section.
  const commerce = /export declare class Commerce \{([\s\S]*?)\n\}/.exec(source);
  assert.ok(commerce, 'index.d.ts declares class Commerce');
  const getters = [...commerce[1].matchAll(/^\s*get (\w+)\(\): (\w+)$/gm)];
  assert.ok(getters.length > 0, 'Commerce declares getters');
  const classNames = new Set([...source.matchAll(/^export declare class (\w+)/gm)].map((m) => m[1]));
  for (const [, name, type] of getters) {
    if (!classNames.has(type)) continue;
    assert.ok(
      first.markdown.includes(`### commerce.${name}\n`) || first.markdown.includes(`Also available as \`commerce.${name}\``),
      `commerce.${name} (${type}) has a section in the generated reference`,
    );
  }
  assert.strictEqual(first.stats.otherClasses, 0, 'every class is reachable from Commerce');
});

test('the generator handles the declaration shapes a regenerated index.d.ts may contain', async () => {
  const { generateApiReference } = await loadGenerator();
  const source = [
    "/** Order status. */",
    "export type OrderStatus = 'pending' | 'paid'",
    'export type Wide =',
    "  | 'a'",
    "  | 'b'",
    'export type Maybe<T> = T | null',
    'export interface Base { id: string }',
    '/** Derived */',
    'export interface Derived<T = string> extends Base, Record<string, unknown> {',
    '  /** Optional thing */',
    '  thing?: Array<T>',
    '  status: OrderStatus',
    '  amountExact: string',
    '  /** @deprecated use amountExact */',
    '  amount: number',
    '  [key: string]: unknown',
    '  cb: (e: Derived | null) => void',
    '}',
    "export enum Color { Red = 'red', Blue = 'blue' }",
    'export declare function f<T extends Base>(x: T, opts?: { a: number; b?: string }): Promise<T | null>',
    'export declare class Commerce {',
    '  constructor(p: string)',
    '  /** Foo API */',
    '  get foo(): Foo',
    '}',
    'export declare class Foo {',
    '  get(id: string): Promise<Maybe<Derived>>',
    '  make(): Bar',
    '}',
    'export declare class Bar { x(): void }',
    '',
  ].join('\n');
  const { markdown, warnings, stats } = generateApiReference(source);
  assert.deepStrictEqual(warnings, []);
  assert.deepStrictEqual(
    { classes: stats.classes, reachableClasses: stats.reachableClasses, interfaces: stats.interfaces, typeAliases: stats.typeAliases, enums: stats.enums, functions: stats.functions },
    { classes: 3, reachableClasses: 2, interfaces: 2, typeAliases: 3, enums: 1, functions: 1 },
  );
  for (const expected of [
    '### commerce.foo\n',
    '### commerce.foo.make()\n',
    "One of: `'pending'`, `'paid'`.",
    "type Wide = 'a' | 'b'",
    'Extends [`Base`](#base), `Record`.',
    '| `thing?` | `Array<T>` | Optional thing |',
    '**Deprecated.** use amountExact',
    '_Exact money',
    '| `[key: string]` | `unknown` |',
    '| `cb` | `(e: Derived \\| null) => void` |',
    "| `Red` | `'red'` |",
    'function f<T extends Base>(x: T, opts?: { a: number; b?: string }): Promise<T | null>',
  ]) {
    assert.ok(markdown.includes(expected), `generated reference contains ${JSON.stringify(expected)}`);
  }
});
