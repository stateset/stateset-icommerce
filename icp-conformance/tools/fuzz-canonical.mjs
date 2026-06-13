#!/usr/bin/env node
// Differential canonicalization fuzzer for the ICP-1.0 IUTs.
//
// Generates a deterministic stream of pathological JSON values — control-char
// -dense strings, escaped-char object keys, astral-plane characters, doubles
// at the ECMAScript notation boundaries (1e-6 / 1e21), and integer literals
// beyond 2^53 — then feeds every available IUT adapter the *same* batch through
// the `02-canonical-json` test and asserts that all adapters emit
// byte-identical canonical strings for every value.
//
// This is the durable complement to the fixed conformance vectors: vectors pin
// known-divergence cases forever; the fuzzer hunts for *unknown* divergences
// across a far larger input space. A single byte of disagreement between any
// two implementations means a signature produced by one will not verify under
// another — the property ICP's entire trust model rests on.
//
// Usage:
//   node tools/fuzz-canonical.mjs [--seed <uint32>] [--count <N>] [--iut <name>]...
//                                 [--verbose]
//
// Flags:
//   --seed <uint32>  PRNG seed (default 0x1cebab1e, fixed for reproducibility).
//   --count <N>      Number of random values to generate (default 1000).
//   --iut <name>     Restrict to a specific IUT (repeatable). Default: all
//                    adapters in iut-adapters/registry.json that support
//                    icp-1.0-core and whose command resolves.
//   --verbose        Print the first divergence's full canonical strings.
//
// Exit codes:
//   0 — every value canonicalized byte-identically across all adapters.
//   1 — at least one divergence (details printed) OR fewer than two adapters
//       were available to compare.
//   2 — harness error (registry missing, adapter crash, etc.).

import { readFileSync, existsSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { dirname, join, resolve } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));
const ROOT = resolve(__dirname, '..');

// ===========================================================================
// Args
// ===========================================================================

// Parse a seed in decimal or 0x-prefixed hex form, so both
// `--seed 485206814` and `--seed 0x1cebab1e` work.
function parseSeed(text) {
  if (typeof text !== 'string') return Number.NaN;
  return /^0x/i.test(text) ? Number.parseInt(text.slice(2), 16) : Number.parseInt(text, 10);
}

function parseArgs(argv) {
  const out = { iut: [] };
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if (arg === '--verbose' || arg === '-v') out.verbose = true;
    else if (arg === '--seed') out.seed = parseSeed(argv[++i]);
    else if (arg === '--count') out.count = Number.parseInt(argv[++i], 10);
    else if (arg === '--iut') out.iut.push(argv[++i]);
    else if (arg.startsWith('--')) {
      process.stderr.write(`unknown flag: ${arg}\n`);
      process.exit(2);
    }
  }
  return out;
}

const args = parseArgs(process.argv.slice(2));
// Fixed default seed keeps CI runs reproducible — a divergence is always
// re-runnable bit-for-bit from the printed seed.
const seed = Number.isFinite(args.seed) ? args.seed >>> 0 : 0x1cebab1e;
const count = Number.isFinite(args.count) && args.count > 0 ? args.count : 1000;
const verbose = args.verbose === true;

// ===========================================================================
// Seeded PRNG — xorshift32. Deterministic, no OS entropy.
// ===========================================================================

function makeRng(seedValue) {
  // xorshift32 never visits 0; seed of 0 would lock the generator, so coerce.
  let state = (seedValue >>> 0) || 0x9e3779b9;
  return {
    // Returns a uint32.
    nextU32() {
      let x = state;
      x ^= x << 13;
      x ^= x >>> 17;
      x ^= x << 5;
      state = x >>> 0;
      return state;
    },
    // Returns a float in [0, 1).
    nextFloat() {
      return this.nextU32() / 0x1_0000_0000;
    },
    // Returns an integer in [0, n).
    nextInt(n) {
      return this.nextU32() % n;
    },
    // Picks a random element of arr.
    pick(arr) {
      return arr[this.nextInt(arr.length)];
    },
  };
}

// ===========================================================================
// Value generation
// ===========================================================================

// Characters chosen to exercise every escaping branch and the UTF-16 key
// ordering boundary: the named two-char escapes, generic control chars
// (\u00xx), the raw-but-suspicious set (DEL, U+2028/U+2029, <, >, &), a BMP
// ligature, and an astral-plane code point (surrogate pair in UTF-16).
const SPICY_CHARS = [
  '\b', '\f', '\n', '\r', '\t', '"', '\\', '/',
  '\u0000', '\u0001', '\u001f', '\u0007', '\u000b', '\u001b',
  '\u007f', '\u0080', '\u2028', '\u2029', '\u00a0', '\ufeff',
  '<', '>', '&', "'", '=',
  'a', 'Z', '0', '9', ' ', '!',
  '\u20ac', '\u00f6', '\ufb01', // Euro, o-diaeresis, fi ligature
  '\u{10000}', '\u{1f4a9}', // astral: U+10000, pile of poo
];

// Doubles that hit the ECMAScript Number::toString notation boundaries
// (RFC 8785 §3.2.2.3 / Appendix B), where naive formatters diverge.
const BOUNDARY_DOUBLES = [
  0, -0, 1, -1, 0.5, 1.5, 3.14, 100,
  1e-6, 9.999999e-7, 1.000001e-6, // 1e-6 is the plain↔exponent threshold
  1e21, 1e20, 9.999999e20, // 1e21 is the upper plain↔exponent threshold
  1e-7, 5e-324, 1.7976931348623157e308,
  333333333.3333333, 9.999999999999997e22, 1e30,
  0.000001, 123456.789, -0.0001,
];

// Integer literals straddling 2^53, where exact-int vs IEEE-754-double
// semantics diverge. Emitted as RAW JSON literals (not JS numbers) so the
// IUTs parse them, not us — JS would already round them at JSON.parse time.
const BIGINT_LITERALS = [
  '9007199254740991', // 2^53 - 1 (largest exact)
  '9007199254740993', // 2^53 + 1 (first non-representable odd)
  '12345678901234567890',
  '18446744073709551615', // u64::MAX
  '-12345678901234567890',
  '1000000000000000000000', // 1e21 magnitude → exponent form
  '99999999999999999999999',
];

function randomString(rng) {
  const len = rng.nextInt(12);
  let s = '';
  for (let i = 0; i < len; i++) s += rng.pick(SPICY_CHARS);
  return s;
}

function randomKey(rng) {
  // Bias keys toward escaped/control/astral chars: those are exactly the
  // inputs that distinguish raw-UTF-16 ordering from escaped-byte ordering.
  const len = 1 + rng.nextInt(4);
  let s = '';
  for (let i = 0; i < len; i++) s += rng.pick(SPICY_CHARS);
  return s;
}

// `value` slots may be raw JSON text (for big-int literals that must NOT pass
// through a JS number) or a JS value. We track which via a tagged wrapper.
function rawLiteral(text) {
  return { __raw: text };
}

function randomScalar(rng) {
  switch (rng.nextInt(6)) {
    case 0:
      return randomString(rng);
    case 1:
      return rng.pick(BOUNDARY_DOUBLES);
    case 2:
      return rawLiteral(rng.pick(BIGINT_LITERALS));
    case 3:
      return rng.nextInt(2) === 0;
    case 4:
      return null;
    default:
      // Small safe integer.
      return rng.nextInt(2_000_000) - 1_000_000;
  }
}

function randomValue(rng, depth) {
  if (depth <= 0) return randomScalar(rng);
  switch (rng.nextInt(5)) {
    case 0: {
      const n = rng.nextInt(4);
      const arr = [];
      for (let i = 0; i < n; i++) arr.push(randomValue(rng, depth - 1));
      return arr;
    }
    case 1: {
      const n = rng.nextInt(4);
      const obj = {};
      for (let i = 0; i < n; i++) obj[randomKey(rng)] = randomValue(rng, depth - 1);
      return obj;
    }
    default:
      return randomScalar(rng);
  }
}

// Serialize a generated value to raw JSON text.
//
// All string and key escaping is delegated to `JSON.stringify`, the canonical
// JSON producer: its output is RFC 8259 text that every conformant parser
// (V8, serde_json, encoding/json, Python json) reads back to the identical
// value. Hand-rolling the escaper instead caused subtle raw-text artifacts
// where different parsers disagreed on a key's bytes — a *parser* divergence,
// not a *canonicalization* one, and therefore noise this fuzzer must not emit.
//
// The one thing JSON.stringify cannot express is an integer literal beyond
// 2^53 (it would round it to a JS double first), and it normalizes -0 to 0.
// Those slots are carried as `{ __raw: "<literal>" }` / `{ __negzero: true }`
// sentinels and spliced in via unique, JSON-safe placeholder tokens that are
// substituted back after stringification.
const RAW_PLACEHOLDERS = new Map();
let placeholderSeq = 0;

function placeholderFor(literal) {
  // A token that survives JSON.stringify as a plain string and cannot collide
  // with generated content (generated strings never contain this prefix).
  const token = `@@RAWLIT_${placeholderSeq++}@@`;
  RAW_PLACEHOLDERS.set(token, literal);
  return token;
}

// Recursively replace sentinel objects with placeholder strings so the whole
// value becomes JSON.stringify-able, then stringify, then swap placeholders
// (each wrapped in quotes by JSON.stringify) back to their raw literal.
function toRawJson(value) {
  RAW_PLACEHOLDERS.clear();
  placeholderSeq = 0;
  const prepared = prepareForStringify(value);
  const text = JSON.stringify(prepared);
  if (RAW_PLACEHOLDERS.size === 0) return text;
  return text.replace(/"(@@RAWLIT_\d+@@)"/g, (_m, token) => {
    const literal = RAW_PLACEHOLDERS.get(token);
    if (literal === undefined) throw new Error(`dangling placeholder ${token}`);
    return literal;
  });
}

function prepareForStringify(value) {
  if (value === null || typeof value !== 'object') {
    if (Object.is(value, -0)) return placeholderFor('-0');
    return value;
  }
  if (typeof value.__raw === 'string') return placeholderFor(value.__raw);
  if (Array.isArray(value)) return value.map(prepareForStringify);
  const out = {};
  for (const [k, v] of Object.entries(value)) out[k] = prepareForStringify(v);
  return out;
}

// ===========================================================================
// Adapter discovery + invocation
// ===========================================================================

function loadRegistry() {
  const path = join(ROOT, 'iut-adapters', 'registry.json');
  try {
    return JSON.parse(readFileSync(path, 'utf8'));
  } catch (err) {
    process.stderr.write(`fuzz: cannot read registry: ${err.message}\n`);
    process.exit(2);
  }
}

// An adapter command's first token is either a path-y artifact (the Rust/Go
// binary) or an interpreter (node/python3). Path-y commands that don't exist
// are skipped with a warning; interpreters are assumed present.
function adapterAvailable(command) {
  const exe = command[0];
  const looksLikePath = exe.includes('/') || exe.includes('\\');
  if (!looksLikePath) return true; // node, python3, etc.
  const resolved = resolve(ROOT, exe);
  return existsSync(resolved);
}

function selectAdapters(registry, restrict) {
  const names = restrict.length > 0 ? restrict : Object.keys(registry);
  const adapters = [];
  for (const name of names) {
    const entry = registry[name];
    if (!entry) {
      process.stderr.write(`fuzz: unknown IUT '${name}', skipping\n`);
      continue;
    }
    if (!entry.supports?.includes('icp-1.0-core')) continue;
    if (!adapterAvailable(entry.command)) {
      process.stderr.write(
        `fuzz: adapter '${name}' artifact not built (${entry.command[0]}), skipping\n`,
      );
      continue;
    }
    adapters.push({ name, command: entry.command });
  }
  return adapters;
}

// Runs one adapter on a `02-canonical-json` batch, returns its
// canonical_strings array. Throws on protocol violation.
function runAdapter(adapter, rawInput) {
  const [cmd, ...argv] = adapter.command;
  const child = spawnSync(cmd, [...argv, '02-canonical-json'], {
    cwd: ROOT,
    input: rawInput,
    encoding: 'utf8',
    timeout: 60000,
    maxBuffer: 64 * 1024 * 1024,
  });
  if (child.status === 2) {
    throw new Error(`adapter ${adapter.name} SKIP: ${child.stderr.trim()}`);
  }
  if (child.status !== 0) {
    throw new Error(
      `adapter ${adapter.name} exited ${child.status}: ${child.stderr.trim()}`,
    );
  }
  let parsed;
  try {
    parsed = JSON.parse(child.stdout);
  } catch (err) {
    throw new Error(`adapter ${adapter.name} stdout not JSON: ${err.message}`);
  }
  if (!Array.isArray(parsed.canonical_strings)) {
    throw new Error(`adapter ${adapter.name} missing canonical_strings`);
  }
  return parsed.canonical_strings;
}

// ===========================================================================
// Main
// ===========================================================================

const registry = loadRegistry();
const adapters = selectAdapters(registry, args.iut);

console.log(
  `ICP canonicalization differential fuzz — seed=0x${seed
    .toString(16)
    .padStart(8, '0')} count=${count} adapters=${adapters.map((a) => a.name).join(',')}`,
);

if (adapters.length < 2) {
  process.stderr.write(
    `fuzz: need at least 2 available adapters to differential-test, found ${adapters.length}.\n` +
      `Build the Rust IUT (cargo build -p stateset-icp-iut --release) and Go IUT (go build) first.\n`,
  );
  process.exit(1);
}

// Generate all N values up front (deterministic from the seed).
const rng = makeRng(seed);
const cases = [];
for (let i = 0; i < count; i++) {
  const value = randomValue(rng, 3);
  cases.push({ name: `fuzz-${i}`, raw: toRawJson(value) });
}

// Invoke adapters in small chunks rather than one giant batch. A differential
// fuzzer's load-bearing property is that every reported divergence is
// independently reproducible; feeding thousands of values in a single
// multi-hundred-KB JSON document risks tripping a parser's large-input edge
// cases (observed: V8's JSON.parse misreads one escaped key in a 150 KB
// document where serde_json, encoding/json, and Python's json all agree),
// producing an irreproducible false positive. Chunking keeps each value in a
// small, isolatable context — and ~20 spawns/adapter for 1000 values stays
// fast.
const CHUNK = 50;

// Assemble a `02-canonical-json` raw input for a slice of cases. We hand-build
// the JSON text so the big-int / -0 raw literals survive verbatim
// (JSON.stringify would round them). Contract: { test, cases: [ {name,value} ] }.
function buildBatch(slice) {
  return (
    '{"test":"02-canonical-json","cases":[' +
    slice
      .map((c) => `{"name":${JSON.stringify(c.name)},"value":${c.raw}}`)
      .join(',') +
    ']}'
  );
}

// Per-adapter accumulated canonical_strings, index-aligned with `cases`.
const accumulated = adapters.map((a) => ({ name: a.name, out: [] }));
try {
  for (let start = 0; start < count; start += CHUNK) {
    const slice = cases.slice(start, start + CHUNK);
    const rawInput = buildBatch(slice);
    for (let a = 0; a < adapters.length; a++) {
      const out = runAdapter(adapters[a], rawInput);
      if (out.length !== slice.length) {
        throw new Error(
          `adapter ${adapters[a].name} returned ${out.length} strings for ${slice.length} cases`,
        );
      }
      for (const s of out) accumulated[a].out.push(s);
    }
  }
} catch (err) {
  process.stderr.write(`fuzz: ${err.message}\n`);
  process.exit(2);
}
const results = accumulated;

// Cross-check: for every case index, every adapter must agree byte-for-byte
// with the first adapter (the reference baseline).
const baseline = results[0];
let divergences = 0;
let firstDivergence = null;

for (let i = 0; i < count; i++) {
  const want = baseline.out[i];
  for (let a = 1; a < results.length; a++) {
    const got = results[a].out[i];
    if (got !== want) {
      divergences++;
      if (!firstDivergence) {
        firstDivergence = {
          index: i,
          input: cases[i].raw,
          baseline: { name: baseline.name, str: want },
          diverged: { name: results[a].name, str: got },
        };
      }
    }
  }
}

console.log('');
if (divergences === 0) {
  console.log(
    `Result: ${count} values × ${adapters.length} adapters — 0 divergences. ` +
      'All canonical strings byte-identical.',
  );
  process.exit(0);
}

console.log(`Result: ${divergences} divergence(s) across ${count} values.`);
if (firstDivergence) {
  const d = firstDivergence;
  console.log(`First divergence at case ${d.index}:`);
  console.log(`  input:    ${d.input}`);
  console.log(`  ${d.baseline.name}: ${JSON.stringify(d.baseline.str)}`);
  console.log(`  ${d.diverged.name}: ${JSON.stringify(d.diverged.str)}`);
  if (verbose) {
    console.log('');
    console.log('Full canonical strings per adapter:');
    for (const r of results) {
      console.log(`  ${r.name}: ${JSON.stringify(r.out[d.index])}`);
    }
  }
}
console.log('');
console.log(
  `Reproduce: node tools/fuzz-canonical.mjs --seed ${seed} --count ${count}`,
);
process.exit(1);
