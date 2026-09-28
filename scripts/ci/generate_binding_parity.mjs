#!/usr/bin/env node
// Binding parity report + gate.
//
// For every engine domain accessor on `stateset_embedded::Commerce`
// (`commerce.orders()`, `commerce.promotions()`, ...) this lists, per language
// binding, which engine methods the binding actually reaches, which it does
// not, and which are exempt. It builds on generate_binding_api_inventory.mjs
// (shared helpers and the .NET/Swift surface parsers) rather than re-parsing
// package manifests.
//
// How "exposed" is decided — by tracing calls, not by trusting names:
//   * Node (napi) and Python (pyo3): each exported binding method's body, plus
//     any helper it calls, is scanned for engine calls such as
//     `commerce.promotions().get_by_code(`. A class counts for its PRIMARY
//     domain (the accessor its methods call most); calls into other domains
//     (e.g. a Returns method that looks up an order) are recorded as
//     cross-domain reach but are not counted as exposing that domain.
//   * Go: exported Go methods -> the `C.stateset_*` functions they call ->
//     the Rust FFI export -> engine calls.
//   * Other Rust-FFI bindings (.NET, Swift, Java, Kotlin, PHP, Ruby): the
//     union of engine calls anywhere in their native layer ("native reach";
//     the host-language wiring is not traced, and several hosts are known
//     in-memory fakes). WASM does not link the engine at all.
//   * .NET, Swift and WASM additionally get a NAME-matched surface score:
//     host method names are mapped to engine names across conventions
//     (`getByCode` / `GetByCode` / `get_by_code`) plus the documented alias
//     map in bindings/parity-baseline.json. A name match there is a claim,
//     not proof of engine backing.
//
// Gate (--check), against bindings/parity-baseline.json:
//   (a) a gated binding loses an engine method it exposed    -> fail
//   (b) Node exposes a method a gated binding lacks and the
//       baseline does not list it as a known gap             -> fail
//   (c) a baseline gap has been closed (stale entry)         -> fail (shrink-only)
//   (d) a binding exposes a method the baseline does not
//       record yet                                           -> fail (ratchet)
// plus the usual "generated artifact is stale" check.
//
// Usage:
//   node ./scripts/ci/generate_binding_parity.mjs                    # regenerate report
//   node ./scripts/ci/generate_binding_parity.mjs --check            # CI: report fresh + gate
//   node ./scripts/ci/generate_binding_parity.mjs --update-baseline  # accept new gaps/gains
//   ... --update-baseline --allow-loss                               # also accept losses

import { mkdir, readdir, readFile, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import {
  buildDotnetBindingInventory,
  buildSwiftBindingInventory,
  compareStrings,
  renderMarkdownTable,
} from './generate_binding_api_inventory.mjs';
import {
  lineAt,
  maskGo,
  maskRust,
  normalizeName,
  parseFreeFunctions,
  parseImplBlocks,
  skipBalanced,
  snakeToCamel,
  splitMethods,
} from './lib/rust_source.mjs';

const __filename = fileURLToPath(import.meta.url);
const defaultRoot = path.resolve(path.dirname(__filename), '../..');

export const PATHS = {
  engineSrc: 'crates/stateset-embedded/src',
  baseline: 'bindings/parity-baseline.json',
  json: 'artifacts/compatibility/binding-parity.json',
  markdown: 'docs/src/appendix/binding-parity.md',
};

const GENERATOR = 'scripts/ci/generate_binding_parity.mjs';
const ROOT_DOMAIN = 'commerce';
// Chain links that sit between an accessor and the engine method without
// changing the receiver: `.vector(key).map_err(..)?.search(..)`.
const TRANSPARENT_CALLS = new Set([
  'map_err',
  'expect',
  'unwrap',
  'clone',
  'as_ref',
  'ok_or',
  'ok_or_else',
  'context',
  'with_context',
]);

// ---------------------------------------------------------------------------
// Engine model
// ---------------------------------------------------------------------------

function featuresOf(attributes) {
  return [...attributes.join(' ').matchAll(/feature\s*=\s*"([^"]+)"/g)]
    .map((m) => m[1])
    .filter((value, index, all) => all.indexOf(value) === index)
    .sort();
}

function hasSelfReceiver(signature) {
  return /\(\s*&?\s*(?:mut\s+)?self\b/.test(signature);
}

/**
 * Engine domains: every `Commerce` method that returns a type with its own
 * inherent impl is a domain accessor; the remaining `&self` methods form the
 * root `commerce` domain.
 *
 * @param {{ path: string, text: string }[]} files engine source files
 */
export function buildEngineModel(files) {
  const typeImpls = new Map();
  const commerceMethods = [];
  for (const file of files) {
    const masked = maskRust(file.text);
    for (const block of parseImplBlocks(file.text, masked)) {
      const methods = splitMethods(block).filter(
        (method) => method.visibility === 'pub' && hasSelfReceiver(method.signature),
      );
      if (block.type === 'Commerce') {
        for (const method of methods) commerceMethods.push({ ...method, file: file.path });
        continue;
      }
      if (!typeImpls.has(block.type)) typeImpls.set(block.type, []);
      for (const method of methods) {
        if (method.name === 'new') continue;
        typeImpls.get(block.type).push({ ...method, file: file.path });
      }
    }
  }

  const domains = new Map();
  const accessorAliases = [];
  const rootMethods = [];
  for (const method of commerceMethods) {
    const returned = /->\s*(?:Result<\s*)?(?:crate::)?(?:\w+::)*([A-Z]\w*)/.exec(
      method.signature,
    )?.[1];
    const impl = returned ? typeImpls.get(returned) : undefined;
    if (impl && impl.length > 0) {
      const existing = [...domains.values()].find((domain) => domain.type === returned);
      if (existing) {
        accessorAliases.push({ accessor: method.name, aliasOf: existing.name });
        continue;
      }
      const seen = new Map();
      for (const engineMethod of impl) {
        if (!seen.has(engineMethod.name)) seen.set(engineMethod.name, engineMethod);
      }
      domains.set(method.name, {
        name: method.name,
        type: returned,
        accessorFile: method.file,
        accessorLine: method.line,
        features: featuresOf(method.attributes),
        methods: [...seen.values()]
          .map((m) => ({
            name: m.name,
            file: m.file,
            line: m.line,
            features: featuresOf(m.attributes),
          }))
          .sort((a, b) => a.name.localeCompare(b.name)),
      });
    } else {
      rootMethods.push({
        name: method.name,
        file: method.file,
        line: method.line,
        features: featuresOf(method.attributes),
      });
    }
  }
  const root = new Map();
  for (const method of rootMethods) if (!root.has(method.name)) root.set(method.name, method);
  domains.set(ROOT_DOMAIN, {
    name: ROOT_DOMAIN,
    type: 'Commerce',
    accessorFile: null,
    accessorLine: null,
    features: [],
    methods: [...root.values()].sort((a, b) => a.name.localeCompare(b.name)),
  });

  const sorted = new Map(
    [...domains.entries()].sort((a, b) => {
      if (a[0] === ROOT_DOMAIN) return -1;
      if (b[0] === ROOT_DOMAIN) return 1;
      return a[0].localeCompare(b[0]);
    }),
  );
  return {
    domains: sorted,
    accessorAliases: accessorAliases.sort((a, b) => a.accessor.localeCompare(b.accessor)),
    methodSets: new Map(
      [...sorted.entries()].map(([name, d]) => [name, new Set(d.methods.map((m) => m.name))]),
    ),
  };
}

// ---------------------------------------------------------------------------
// Engine call extraction
// ---------------------------------------------------------------------------

/**
 * Engine calls made directly in one (masked) body. Recognises
 *   commerce.orders().create(          chained
 *   .vector(key).map_err(..)?.search(  chained through transparent links
 *   let v = commerce.vector(k)...; v.search(   bound to a local
 *   commerce.calculate_cart_tax(       root methods
 *
 * @param {string} body masked source
 * @param {ReturnType<typeof buildEngineModel>} engine
 * @returns {{ domain: string, method: string }[]}
 */
export function extractEngineCalls(body, engine) {
  const calls = [];
  const rootSet = engine.methodSets.get(ROOT_DOMAIN);
  const accessorNames = new Set(
    [...engine.methodSets.keys()].filter((name) => name !== ROOT_DOMAIN),
  );
  for (const alias of engine.accessorAliases) accessorNames.add(alias.accessor);
  const resolveAccessor = (name) =>
    engine.accessorAliases.find((alias) => alias.accessor === name)?.aliasOf ?? name;

  // Returns the method called on the accessor's result, or null.
  const followChain = (openParen) => {
    let cursor = skipBalanced(body, openParen);
    for (;;) {
      const rest = /^\s*\??\s*\.\s*([A-Za-z_]\w*)\s*(?:::<[^>]*>\s*)?\(/.exec(
        body.slice(cursor, cursor + 200),
      );
      if (!rest) return { method: null, end: cursor };
      if (TRANSPARENT_CALLS.has(rest[1])) {
        cursor = skipBalanced(body, cursor + rest[0].length - 1);
        continue;
      }
      return { method: rest[1], end: cursor };
    }
  };

  const callRe = /\.\s*([a-z_]\w*)\s*\(/g;
  let match;
  while ((match = callRe.exec(body)) !== null) {
    const name = match[1];
    if (accessorNames.has(name)) {
      const domain = resolveAccessor(name);
      const { method } = followChain(match.index + match[0].length - 1);
      if (method && engine.methodSets.get(domain).has(method)) calls.push({ domain, method });
    } else if (rootSet.has(name)) {
      calls.push({ domain: ROOT_DOMAIN, method: name });
    }
  }

  // Locals bound to a domain handle: `let vector = { ...; commerce.vector(k)? };`
  const letRe = /\blet\s+(?:mut\s+)?([a-z_]\w*)\s*(?::[^=;]+)?=/g;
  while ((match = letRe.exec(body)) !== null) {
    const variable = match[1];
    let end = match.index + match[0].length;
    let depth = 0;
    for (; end < body.length; end += 1) {
      const ch = body[end];
      if (ch === '(' || ch === '[' || ch === '{') depth += 1;
      else if (ch === ')' || ch === ']' || ch === '}') depth -= 1;
      else if (ch === ';' && depth === 0) break;
      if (depth < 0) break;
    }
    const statement = body.slice(match.index + match[0].length, end);
    const accessorRe = /\.\s*([a-z_]\w*)\s*\(/g;
    let bound = null;
    let accessorMatch;
    while ((accessorMatch = accessorRe.exec(statement)) !== null) {
      if (!accessorNames.has(accessorMatch[1])) continue;
      const open = accessorMatch.index + accessorMatch[0].length - 1;
      const local = statement;
      let cursor = skipBalanced(local, open);
      let terminal = true;
      for (;;) {
        const rest = /^\s*\??\s*\.\s*([A-Za-z_]\w*)\s*\(/.exec(local.slice(cursor, cursor + 200));
        if (!rest) break;
        if (TRANSPARENT_CALLS.has(rest[1])) {
          cursor = skipBalanced(local, cursor + rest[0].length - 1);
          continue;
        }
        terminal = false;
        break;
      }
      // The handle is the statement's value only if nothing but `?`, `}` or
      // whitespace follows the accessor chain.
      if (terminal && /^[\s?};]*$/.test(local.slice(cursor))) {
        bound = resolveAccessor(accessorMatch[1]);
      }
    }
    if (!bound) continue;
    const usageRe = new RegExp(`\\b${variable}\\s*\\??\\s*\\.\\s*([a-z_]\\w*)\\s*\\(`, 'g');
    const after = body.slice(end);
    let usage;
    while ((usage = usageRe.exec(after)) !== null) {
      if (engine.methodSets.get(bound).has(usage[1]))
        calls.push({ domain: bound, method: usage[1] });
    }
  }
  return calls;
}

function callKey(call) {
  return `${call.domain}.${call.method}`;
}

/**
 * Follow `self.helper(`, `Self::helper(` and free-function helpers so a
 * binding method that delegates still gets credit for the engine calls it
 * makes. Memoised; cycles are cut.
 */
function createCallResolver(engine, freeFunctions, classMethods) {
  const freeByName = new Map();
  for (const fn of freeFunctions) {
    if (!freeByName.has(fn.name)) freeByName.set(fn.name, []);
    freeByName.get(fn.name).push(fn);
  }
  const memo = new Map();
  const resolve = (key, body, className) => {
    if (memo.has(key)) return memo.get(key);
    memo.set(key, new Map());
    const result = new Map();
    for (const call of extractEngineCalls(body, engine)) result.set(callKey(call), call);
    const add = (other) => {
      for (const [k, v] of other) result.set(k, v);
    };
    if (className) {
      for (const helper of body.matchAll(/\b(?:self|Self)\s*(?:\.|::)\s*([a-z_]\w*)\s*\(/g)) {
        for (const method of classMethods.get(className)?.get(helper[1]) ?? []) {
          add(resolve(`m:${className}.${helper[1]}:${method.line}`, method.body, className));
        }
      }
    }
    for (const helper of body.matchAll(
      /(?<![.\w])(?:(?:crate|super)::(?:\w+::)*)?([a-z_]\w*)\s*(?:::<[^>]*>)?\s*\(/g,
    )) {
      for (const fn of freeByName.get(helper[1]) ?? []) {
        add(resolve(`f:${fn.name}:${fn.file}:${fn.line}`, fn.body, null));
      }
    }
    memo.set(key, result);
    return result;
  };
  return resolve;
}

// ---------------------------------------------------------------------------
// Binding analysers
// ---------------------------------------------------------------------------

async function listRustFiles(rootDir, relativeDir) {
  const entries = await readdir(path.join(rootDir, relativeDir), {
    withFileTypes: true,
    recursive: true,
  });
  return entries
    .filter((entry) => entry.isFile() && entry.name.endsWith('.rs'))
    .map((entry) => path.relative(rootDir, path.join(entry.parentPath ?? entry.path, entry.name)))
    .sort();
}

async function loadSources(rootDir, relativePaths) {
  return Promise.all(
    relativePaths.map(async (relativePath) => {
      const text = await readFile(path.join(rootDir, relativePath), 'utf8');
      return { path: relativePath, text, masked: maskRust(text) };
    }),
  );
}

function tallyPrimaryDomain(methods, className) {
  if (className === 'Commerce') return ROOT_DOMAIN;
  const counts = new Map();
  for (const method of methods) {
    for (const call of method.calls.values()) {
      if (call.domain === ROOT_DOMAIN) continue;
      counts.set(call.domain, (counts.get(call.domain) ?? 0) + 1);
    }
  }
  let best = null;
  for (const [domain, count] of [...counts.entries()].sort((a, b) => a[0].localeCompare(b[0]))) {
    if (!best || count > best.count) best = { domain, count };
  }
  return best?.domain ?? null;
}

/**
 * Collapse traced host methods into per-domain exposure.
 *
 * @param {{ className: string, hostName: string, calls: Map<string, {domain: string, method: string}> }[]} hostMethods
 */
export function summariseExposure(hostMethods) {
  const byClass = new Map();
  for (const method of hostMethods) {
    if (!byClass.has(method.className)) byClass.set(method.className, []);
    byClass.get(method.className).push(method);
  }
  const exposed = new Map(); // "domain.method" -> [Class.host]
  const crossDomain = new Map();
  const classes = [];
  for (const [className, methods] of [...byClass.entries()].sort((a, b) =>
    a[0].localeCompare(b[0]),
  )) {
    const primary = tallyPrimaryDomain(methods, className);
    classes.push({ className, domain: primary, methodCount: methods.length });
    for (const method of methods) {
      for (const [key, call] of method.calls) {
        const label = `${className}.${method.hostName}`;
        const target =
          call.domain === ROOT_DOMAIN || call.domain === primary ? exposed : crossDomain;
        if (!target.has(key)) target.set(key, []);
        if (!target.get(key).includes(label)) target.get(key).push(label);
      }
    }
  }
  for (const key of [...crossDomain.keys()]) if (exposed.has(key)) crossDomain.delete(key);
  return { exposed, crossDomain, classes };
}

function napiHostName(method) {
  const jsName = /js_name\s*=\s*"([^"]+)"/.exec(method.attributes.join(' '))?.[1];
  return jsName ?? snakeToCamel(method.name);
}

function pyHostName(method) {
  return (
    /#\[pyo3\([^)]*\bname\s*=\s*"([^"]+)"/.exec(method.attributes.join(' '))?.[1] ?? method.name
  );
}

/**
 * Node (napi) and Python (pyo3): exported methods live in attributed impls.
 */
export function analyseRustClassBinding(sources, engine, { implAttribute, isExported, hostName }) {
  const freeFunctions = [];
  const classMethods = new Map();
  const exportedImpls = [];
  for (const source of sources) {
    for (const fn of parseFreeFunctions(source.text, source.masked))
      freeFunctions.push({ ...fn, file: source.path });
    for (const block of parseImplBlocks(source.text, source.masked)) {
      const methods = splitMethods(block);
      if (!classMethods.has(block.type)) classMethods.set(block.type, new Map());
      for (const method of methods) {
        const bucket = classMethods.get(block.type);
        if (!bucket.has(method.name)) bucket.set(method.name, []);
        bucket.get(method.name).push(method);
      }
      if (block.attributes.some((attribute) => attribute.startsWith(implAttribute))) {
        exportedImpls.push({ block, methods, file: source.path });
      }
    }
  }
  const resolve = createCallResolver(engine, freeFunctions, classMethods);
  const hostMethods = [];
  for (const { block, methods, file } of exportedImpls) {
    for (const method of methods) {
      if (!isExported(method)) continue;
      hostMethods.push({
        className: block.type,
        hostName: hostName(method),
        file,
        line: method.line,
        calls: resolve(`m:${block.type}.${method.name}:${method.line}`, method.body, block.type),
      });
    }
  }
  return { hostMethods, ...summariseExposure(hostMethods) };
}

/**
 * Go: exported Go methods -> C.stateset_* -> Rust FFI export -> engine.
 */
export function analyseGoBinding(goSources, ffiSources, engine) {
  const freeFunctions = [];
  for (const source of ffiSources) {
    for (const fn of parseFreeFunctions(source.text, source.masked))
      freeFunctions.push({ ...fn, file: source.path });
  }
  const ffiByName = new Map(freeFunctions.map((fn) => [fn.name, fn]));
  const resolve = createCallResolver(engine, freeFunctions, new Map());
  const hostMethods = [];
  for (const source of goSources) {
    const masked = maskGo(source.text);
    const methodRe = /^func\s+\(\s*\w+\s+\*?([A-Z]\w*)\s*\)\s+([A-Z]\w*)\s*\(/gm;
    const starts = [...masked.matchAll(methodRe)];
    starts.forEach((start, index) => {
      const stopCandidates = [
        masked.indexOf('\nfunc ', start.index + 1),
        starts[index + 1]?.index ?? -1,
      ].filter((value) => value > 0);
      const stop = stopCandidates.length ? Math.min(...stopCandidates) : masked.length;
      const body = masked.slice(start.index, stop);
      const calls = new Map();
      for (const cCall of body.matchAll(/\bC\.(stateset_\w+)\s*\(/g)) {
        const fn = ffiByName.get(cCall[1]);
        if (!fn) continue;
        for (const [key, call] of resolve(`f:${fn.name}:${fn.file}:${fn.line}`, fn.body, null))
          calls.set(key, call);
      }
      hostMethods.push({
        className: start[1],
        hostName: start[2],
        file: source.path,
        line: lineAt(masked, start.index),
        calls,
      });
    });
  }
  return { hostMethods, ...summariseExposure(hostMethods) };
}

/** Union of engine calls anywhere in a binding's native (Rust) layer. */
export function analyseNativeReach(sources, engine) {
  const reach = new Map();
  for (const source of sources) {
    for (const call of extractEngineCalls(source.masked, engine)) {
      const key = callKey(call);
      if (!reach.has(key)) reach.set(key, [source.path]);
      else if (!reach.get(key).includes(source.path)) reach.get(key).push(source.path);
    }
  }
  return reach;
}

/**
 * Alias keys are written `domain.snake_case_host_name` and match any naming
 * convention: `inventory.get_level` covers `getLevel`, `GetLevel`, `get_level`.
 */
export function lookupAlias(aliases, domain, hostName) {
  const wanted = normalizeName(`${domain}.${hostName}`);
  for (const [key, target] of Object.entries(aliases ?? {})) {
    if (normalizeName(key) === wanted) return target;
  }
  return null;
}

/**
 * Name-matched surface for hosts whose engine wiring is not traced. Maps
 * `{ type, method }` host methods onto engine methods via the facade
 * property/getter that returns each type.
 *
 * @param {{ type: string, method: string }[]} apiMethods
 * @param {Map<string, string>} typeToDomain host type -> engine domain
 * @param {Record<string, string>} aliases "domain.host_name" -> engine method
 */
export function matchSurfaceByName(apiMethods, typeToDomain, engine, aliases = {}) {
  const matched = new Map();
  const unmatched = [];
  for (const { type, method } of apiMethods) {
    const domain = typeToDomain.get(type);
    if (!domain) continue;
    const engineNames = engine.methodSets.get(domain);
    const aliased = lookupAlias(aliases, domain, method);
    const target =
      (aliased && engineNames.has(aliased) ? aliased : null) ??
      [...engineNames].find((name) => normalizeName(name) === normalizeName(method));
    if (target) {
      const key = `${domain}.${target}`;
      if (!matched.has(key)) matched.set(key, []);
      matched.get(key).push(`${type}.${method}`);
    } else {
      unmatched.push(`${type}.${method}`);
    }
  }
  return { exposed: matched, unmatched: unmatched.sort() };
}

function facadeTypeMap(facadeProperties, engine) {
  const accessorByKey = new Map(
    [...engine.domains.keys()].map((name) => [normalizeName(name), name]),
  );
  const map = new Map();
  for (const { name, type } of facadeProperties) {
    const domain = accessorByKey.get(normalizeName(name));
    if (domain && !map.has(type)) map.set(type, domain);
  }
  return map;
}

function parseWasmSurface(source) {
  const blocks = parseImplBlocks(source.text, source.masked).filter((block) =>
    block.attributes.some((attribute) => attribute.startsWith('#[wasm_bindgen')),
  );
  const facade = [];
  const apiMethods = [];
  for (const block of blocks) {
    for (const method of splitMethods(block)) {
      if (method.visibility !== 'pub') continue;
      const attributes = method.attributes.join(' ');
      const jsName = /js_name\s*=\s*"?([A-Za-z_]\w*)"?/.exec(attributes)?.[1];
      const hostName = jsName ?? snakeToCamel(method.name);
      if (block.type === 'Commerce' && /getter/.test(attributes)) {
        const returned = /->\s*([A-Z]\w*)/.exec(method.signature)?.[1];
        if (returned) facade.push({ name: method.name, type: returned });
      } else if (block.type !== 'Commerce') {
        apiMethods.push({ type: block.type, method: hostName });
      }
    }
  }
  return { facade, apiMethods };
}

// ---------------------------------------------------------------------------
// Report assembly
// ---------------------------------------------------------------------------

export function exemptSet(baseline, bindingId) {
  const exempt = new Map(Object.entries(baseline.exempt?.all ?? {}));
  for (const [key, reason] of Object.entries(baseline.exempt?.[bindingId] ?? {}))
    exempt.set(key, reason);
  return exempt;
}

function percent(numerator, denominator) {
  if (denominator === 0) return 0;
  return Math.round((numerator / denominator) * 1000) / 10;
}

function groupByDomain(keys) {
  const grouped = {};
  for (const key of [...keys].sort()) {
    const dot = key.indexOf('.');
    const domain = key.slice(0, dot);
    (grouped[domain] ??= []).push(key.slice(dot + 1));
  }
  return grouped;
}

function flatten(grouped) {
  const keys = new Set();
  for (const [domain, methods] of Object.entries(grouped ?? {})) {
    for (const method of methods) keys.add(`${domain}.${method}`);
  }
  return keys;
}

/**
 * Build the per-binding coverage records.
 */
export function buildBindingRecords(engine, analyses, baseline) {
  const records = [];
  for (const analysis of analyses) {
    const exempt = exemptSet(baseline, analysis.id);
    const domains = {};
    let exposedTotal = 0;
    let applicableTotal = 0;
    for (const [domainName, domain] of engine.domains) {
      const exposedMethods = [];
      const missing = [];
      const exempted = [];
      for (const method of domain.methods) {
        const key = `${domainName}.${method.name}`;
        if (exempt.has(key)) exempted.push(method.name);
        else if (analysis.exposed.has(key)) exposedMethods.push(method.name);
        else missing.push(method.name);
      }
      exposedTotal += exposedMethods.length;
      applicableTotal += exposedMethods.length + missing.length;
      domains[domainName] = {
        exposed: exposedMethods,
        missing,
        exempt: exempted,
        coverage: percent(exposedMethods.length, exposedMethods.length + missing.length),
      };
    }
    records.push({
      id: analysis.id,
      language: analysis.language,
      evidence: analysis.evidence,
      engineBacked: analysis.engineBacked,
      gated: (baseline.gated ?? []).includes(analysis.id),
      exposedCount: exposedTotal,
      applicableCount: applicableTotal,
      coverage: percent(exposedTotal, applicableTotal),
      domains,
      notes: analysis.notes ?? null,
    });
  }
  return records;
}

/** Gaps of `bindingId` relative to the reference binding (Node). */
export function computeGaps(records, bindingId, referenceId) {
  const reference = records.find((record) => record.id === referenceId);
  const binding = records.find((record) => record.id === bindingId);
  const gaps = new Set();
  for (const [domain, entry] of Object.entries(reference.domains)) {
    const have = new Set(binding.domains[domain].exposed);
    const exempt = new Set(binding.domains[domain].exempt);
    for (const method of entry.exposed) {
      if (!have.has(method) && !exempt.has(method)) gaps.add(`${domain}.${method}`);
    }
  }
  return gaps;
}

function exposedKeys(record) {
  const keys = new Set();
  for (const [domain, entry] of Object.entries(record.domains)) {
    for (const method of entry.exposed) keys.add(`${domain}.${method}`);
  }
  return keys;
}

/**
 * Compare current state against the checked-in baseline.
 *
 * @returns {{ errors: string[], current: { exposed: object, gaps: object } }}
 */
export function evaluateGate(records, baseline) {
  const referenceId = baseline.reference ?? 'node';
  const errors = [];
  const current = { exposed: {}, gaps: {} };
  for (const bindingId of baseline.gated ?? []) {
    const record = records.find((r) => r.id === bindingId);
    if (!record) {
      errors.push(`gated binding '${bindingId}' was not analysed`);
      continue;
    }
    const nowExposed = exposedKeys(record);
    current.exposed[bindingId] = groupByDomain(nowExposed);
    const recordedExposed = flatten(baseline.exposed?.[bindingId]);
    for (const key of [...recordedExposed].sort()) {
      if (!nowExposed.has(key)) {
        errors.push(
          `${bindingId} lost engine method ${key} (it was exposed in the baseline). Restore it; if the loss is intended, exempt it in ${PATHS.baseline} when it is not portable and accept it with '--update-baseline --allow-loss'.`,
        );
      }
    }
    for (const key of [...nowExposed].sort()) {
      if (!recordedExposed.has(key)) {
        errors.push(
          `${bindingId} now exposes ${key}, which ${PATHS.baseline} does not record. Run 'node ./${GENERATOR} --update-baseline' to ratchet it in.`,
        );
      }
    }
    if (bindingId === referenceId) continue;
    const nowGaps = computeGaps(records, bindingId, referenceId);
    current.gaps[bindingId] = groupByDomain(nowGaps);
    const recordedGaps = flatten(baseline.gaps?.[bindingId]);
    for (const key of [...nowGaps].sort()) {
      if (!recordedGaps.has(key)) {
        errors.push(
          `${referenceId} exposes ${key} but ${bindingId} does not, and ${PATHS.baseline} does not list the gap. Add the ${bindingId} counterpart, or record the gap explicitly with 'node ./${GENERATOR} --update-baseline'.`,
        );
      }
    }
    for (const key of [...recordedGaps].sort()) {
      if (!nowGaps.has(key)) {
        errors.push(
          `Stale baseline gap: ${bindingId} ${key} is no longer a gap. Remove it from ${PATHS.baseline} (the gap list only shrinks) — 'node ./${GENERATOR} --update-baseline' does this.`,
        );
      }
    }
  }
  return { errors, current };
}

/** Exemptions and aliases must name real engine methods (catches typos and renames). */
export function validateBaselineConfig(baseline, engine) {
  const errors = [];
  const known = (domain, method) => engine.methodSets.get(domain)?.has(method) ?? false;
  for (const [scope, entries] of Object.entries(baseline.exempt ?? {})) {
    for (const key of Object.keys(entries)) {
      const [domain, method] = key.split('.');
      if (!known(domain, method)) {
        errors.push(`Exemption ${scope}:${key} does not name an engine method; fix or remove it.`);
      }
    }
  }
  for (const [key, target] of Object.entries(baseline.aliases ?? {})) {
    const [domain] = key.split('.');
    if (!known(domain, target)) {
      errors.push(`Alias ${key} -> ${target} does not name an engine method; fix or remove it.`);
    }
  }
  return errors;
}

/**
 * Hollow-binding signals: struct literals in binding code that pass an
 * always-empty collection where the engine takes one (e.g. `items: vec![]`).
 */
const ENGINE_INPUT_NAME =
  /^(?:Create|Update|Add|Apply|Record|Receive|Adjust|Cancel|Ship|New|Set|Import|Ingest|Capture|Register|Issue|Post)[A-Z]/;

export function findHollowSignals(sources) {
  const signals = [];
  for (const source of sources) {
    const lines = source.masked.split('\n');
    lines.forEach((line, index) => {
      const match =
        /^(\s*)([a-z_]\w*)\s*:\s*(vec!\[\s*\]|Vec::new\(\)|Default::default\(\))\s*,?\s*$/.exec(
          line,
        );
      if (!match) return;
      const [, indent, field, value] = match;
      if (value === 'Default::default()' && !/(items|lines|components|allocations)$/.test(field))
        return;
      // Find the enclosing struct literal: the nearest line above with less
      // indentation that opens a `{`.
      let owner = null;
      for (let k = index - 1; k >= 0 && index - k < 200; k -= 1) {
        const candidate = lines[k];
        const candidateIndent = /^(\s*)/.exec(candidate)[1].length;
        if (candidate.trim() === '' || candidateIndent >= indent.length) continue;
        const literal = /((?:[A-Za-z_]\w*::)*([A-Z]\w*))\s*\{\s*$/.exec(candidate);
        owner = literal ? literal[1] : null;
        break;
      }
      if (!owner) return;
      const typeName = owner.split('::').pop();
      // Only engine inputs: a stateset_core/stateset_embedded path, or an
      // imported input type (`CreateOrder {`). Output/DTO structs the binding
      // builds for its host (and `Self { .. }`) are not engine inputs.
      const enginePath = /^stateset_(?:core|embedded)::/.test(owner);
      const inputName = ENGINE_INPUT_NAME.test(typeName);
      if (!enginePath && !inputName) return;
      if (/(Output|Result|Response|Info|Summary|Row|View|Error)$/.test(typeName)) return;
      signals.push({
        file: source.path,
        line: index + 1,
        field,
        value: value.replace(/\s+/g, ''),
        structLiteral: owner,
      });
    });
  }
  return signals.sort((a, b) => a.file.localeCompare(b.file) || a.line - b.line);
}

/** Host methods whose name does not match the engine method(s) they call. */
export function findRenames(hostMethodsById, classesById, aliases) {
  const renames = [];
  for (const [bindingId, hostMethods] of Object.entries(hostMethodsById)) {
    const primaryByClass = new Map(classesById[bindingId].map((c) => [c.className, c.domain]));
    for (const method of hostMethods) {
      const primary = primaryByClass.get(method.className);
      const inDomain = [...method.calls.values()].filter(
        (call) => call.domain === primary || call.domain === ROOT_DOMAIN,
      );
      if (inDomain.length === 0) continue;
      const hostKey = normalizeName(method.hostName);
      if (inDomain.some((call) => normalizeName(call.method) === hostKey)) continue;
      const domain = primary ?? ROOT_DOMAIN;
      const alias = lookupAlias(aliases, domain, method.hostName);
      renames.push({
        binding: bindingId,
        hostMethod: `${method.className}.${method.hostName}`,
        domain,
        engineMethods: [...new Set(inDomain.map(callKey))].sort(),
        documented: alias !== null && inDomain.some((call) => call.method === alias),
      });
    }
  }
  return renames.sort(
    (a, b) => a.binding.localeCompare(b.binding) || a.hostMethod.localeCompare(b.hostMethod),
  );
}

// ---------------------------------------------------------------------------
// Orchestration
// ---------------------------------------------------------------------------

export async function buildParity(rootDir = defaultRoot) {
  const baseline = JSON.parse(await readFile(path.join(rootDir, PATHS.baseline), 'utf8'));

  const engineFiles = [
    ...(await readdir(path.join(rootDir, PATHS.engineSrc), { withFileTypes: true }))
      .filter((entry) => entry.isFile() && entry.name.endsWith('.rs'))
      .map((entry) => `${PATHS.engineSrc}/${entry.name}`),
    ...(await readdir(path.join(rootDir, PATHS.engineSrc, 'commerce')))
      .filter((name) => name.endsWith('.rs') && name !== 'tests.rs')
      .map((name) => `${PATHS.engineSrc}/commerce/${name}`),
  ].sort();
  const engineSources = await loadSources(rootDir, engineFiles);
  const engine = buildEngineModel(engineSources);

  const analyses = [];
  const hostMethodsById = {};
  const classesById = {};
  const crossDomainById = {};

  const nodeSources = await loadSources(rootDir, await listRustFiles(rootDir, 'bindings/node/src'));
  const node = analyseRustClassBinding(nodeSources, engine, {
    implAttribute: '#[napi',
    isExported: (method) =>
      method.visibility === 'pub' && method.attributes.some((a) => a.startsWith('#[napi')),
    hostName: napiHostName,
  });
  analyses.push({
    id: 'node',
    language: 'Node.js',
    evidence: 'traced: #[napi] methods -> engine calls',
    engineBacked: true,
    exposed: node.exposed,
  });

  const pythonSources = await loadSources(
    rootDir,
    await listRustFiles(rootDir, 'bindings/python/src'),
  );
  const python = analyseRustClassBinding(pythonSources, engine, {
    implAttribute: '#[pymethods]',
    isExported: () => true,
    hostName: pyHostName,
  });
  analyses.push({
    id: 'python',
    language: 'Python',
    evidence: 'traced: #[pymethods] methods -> engine calls',
    engineBacked: true,
    exposed: python.exposed,
  });

  const goHostFiles = (await readdir(path.join(rootDir, 'bindings/go/stateset')))
    .filter((name) => name.endsWith('.go') && !name.endsWith('_test.go'))
    .map((name) => `bindings/go/stateset/${name}`)
    .sort();
  const goHostSources = await Promise.all(
    goHostFiles.map(async (p) => ({
      path: p,
      text: await readFile(path.join(rootDir, p), 'utf8'),
    })),
  );
  const goFfiSources = await loadSources(rootDir, await listRustFiles(rootDir, 'bindings/go/src'));
  const go = analyseGoBinding(goHostSources, goFfiSources, engine);
  analyses.push({
    id: 'go',
    language: 'Go',
    evidence: 'traced: Go methods -> C.stateset_* -> Rust FFI -> engine calls',
    engineBacked: true,
    exposed: go.exposed,
  });

  for (const [id, result] of [
    ['node', node],
    ['python', python],
    ['go', go],
  ]) {
    hostMethodsById[id] = result.hostMethods;
    classesById[id] = result.classes;
    crossDomainById[id] = result.crossDomain;
  }

  // Native-layer reach for the remaining FFI bindings.
  const nativeBindings = [
    [
      'dotnet',
      '.NET',
      'bindings/dotnet/src',
      'host (C#) layer is not traced; known in-memory fake at the host level',
    ],
    ['java', 'Java', 'bindings/java/src', 'host (Java/JNI) layer is not traced'],
    ['kotlin', 'Kotlin', 'bindings/kotlin/src', 'host (Kotlin/JNI) layer is not traced'],
    ['php', 'PHP', 'bindings/php/src', 'engine linkage is behind an optional cargo feature'],
    ['ruby', 'Ruby', 'bindings/ruby/src', 'does not depend on stateset-embedded'],
    [
      'swift',
      'Swift',
      'bindings/swift/src',
      'host (Swift) layer is not traced; known in-memory fake at the host level',
    ],
  ];
  for (const [id, language, dir, notes] of nativeBindings) {
    const sources = await loadSources(rootDir, await listRustFiles(rootDir, dir));
    analyses.push({
      id,
      language,
      evidence: 'native reach: engine calls anywhere in the Rust layer',
      engineBacked: sources.some(
        (s) => /stateset_embedded::/.test(s.masked) || /\bRustCommerce\b/.test(s.masked),
      ),
      exposed: analyseNativeReach(sources, engine),
      notes,
    });
  }
  const wasmSources = await loadSources(rootDir, ['bindings/wasm/src/lib.rs']);
  analyses.push({
    id: 'wasm',
    language: 'WASM',
    evidence: 'native reach: engine calls anywhere in the Rust layer',
    engineBacked: false,
    exposed: analyseNativeReach(wasmSources, engine),
    notes: 'in-memory reimplementation; does not link stateset-embedded',
  });

  // Name-matched surfaces (claims, not proof of engine backing).
  const surfaceAliases = baseline.aliases ?? {};
  const [dotnetInventory, swiftInventory] = await Promise.all([
    buildDotnetBindingInventory(),
    buildSwiftBindingInventory(),
  ]);
  const wasmSurface = parseWasmSurface(wasmSources[0]);
  const surfaces = [
    [
      'dotnet',
      '.NET',
      dotnetInventory.apiMethods,
      facadeTypeMap(dotnetInventory.facadeProperties, engine),
    ],
    [
      'swift',
      'Swift',
      swiftInventory.apiMethods,
      facadeTypeMap(swiftInventory.facadeProperties, engine),
    ],
    ['wasm', 'WASM', wasmSurface.apiMethods, facadeTypeMap(wasmSurface.facade, engine)],
  ].map(([id, language, apiMethods, typeToDomain]) => {
    const result = matchSurfaceByName(apiMethods, typeToDomain, engine, surfaceAliases);
    return {
      id,
      language,
      hostMethodCount: apiMethods.length,
      mappedTypes: Object.fromEntries([...typeToDomain.entries()].sort()),
      exposed: result.exposed,
      unmatched: result.unmatched,
    };
  });

  const records = buildBindingRecords(engine, analyses, baseline);
  const surfaceRecords = buildBindingRecords(
    engine,
    surfaces.map((surface) => ({
      ...surface,
      evidence: 'name-matched host surface',
      engineBacked: null,
    })),
    { ...baseline, gated: [] },
  ).map((record, index) => ({
    ...record,
    hostMethodCount: surfaces[index].hostMethodCount,
    mappedTypes: surfaces[index].mappedTypes,
    unmatchedHostMethods: surfaces[index].unmatched,
  }));

  const hollowDirs = ['node', 'python', 'go', 'dotnet', 'java', 'kotlin', 'php', 'swift'].map(
    (id) => `bindings/${id}/src`,
  );
  const engineSourceSet = await loadSources(
    rootDir,
    (await Promise.all(hollowDirs.map((dir) => listRustFiles(rootDir, dir)))).flat(),
  );
  const hollowSignals = findHollowSignals(engineSourceSet);
  const renames = findRenames(hostMethodsById, classesById, baseline.aliases);
  const gate = evaluateGate(records, baseline);
  gate.errors.push(...validateBaselineConfig(baseline, engine));

  const referenceId = baseline.reference ?? 'node';
  const parityVsReference = {};
  const reference = records.find((r) => r.id === referenceId);
  const referenceExposed = exposedKeys(reference);
  for (const record of records) {
    if (record.id === referenceId) continue;
    const have = exposedKeys(record);
    const shared = [...referenceExposed].filter((key) => have.has(key)).length;
    parityVsReference[record.id] = {
      shared,
      referenceExposed: referenceExposed.size,
      parity: percent(shared, referenceExposed.size),
      beyondReference: [...have].filter((key) => !referenceExposed.has(key)).sort(),
    };
  }

  const report = {
    source: {
      generator: GENERATOR,
      builds_on: 'scripts/ci/generate_binding_api_inventory.mjs',
      baseline: PATHS.baseline,
      engine: PATHS.engineSrc,
      reference: referenceId,
    },
    engine: {
      domainCount: engine.domains.size,
      methodCount: [...engine.domains.values()].reduce((sum, d) => sum + d.methods.length, 0),
      accessorAliases: engine.accessorAliases,
      domains: [...engine.domains.values()].map((domain) => ({
        name: domain.name,
        type: domain.type,
        features: domain.features,
        methods: domain.methods.map((m) =>
          m.features.length ? { name: m.name, features: m.features } : m.name,
        ),
      })),
    },
    bindings: records,
    parityVsReference,
    nameMatchedSurfaces: surfaceRecords,
    classes: classesById,
    crossDomainReach: Object.fromEntries(
      Object.entries(crossDomainById).map(([id, map]) => [id, groupByDomain(map.keys())]),
    ),
    renames,
    hollowSignals,
  };
  return { report, gate, baseline, engine };
}

// ---------------------------------------------------------------------------
// Markdown
// ---------------------------------------------------------------------------

function cell(entry) {
  const applicable = entry.exposed.length + entry.missing.length;
  if (applicable === 0) return entry.exempt.length ? 'exempt' : '—';
  return `${entry.exposed.length}/${applicable}`;
}

export function renderMarkdown(report, baseline) {
  const tracedIds = ['node', 'python', 'go'];
  const traced = tracedIds.map((id) => report.bindings.find((b) => b.id === id));
  const native = report.bindings.filter((b) => !tracedIds.includes(b.id));
  const surfaces = report.nameMatchedSurfaces;

  const summaryRows = [
    ...traced.map((b) => [
      b.language,
      b.evidence,
      b.gated ? 'yes' : 'no',
      `${b.exposedCount}/${b.applicableCount}`,
      `${b.coverage}%`,
      b.id === report.source.reference ? 'reference' : `${report.parityVsReference[b.id].parity}%`,
    ]),
    ...native.map((b) => [
      b.language,
      b.evidence,
      'no',
      `${b.exposedCount}/${b.applicableCount}`,
      `${b.coverage}%`,
      `${report.parityVsReference[b.id].parity}%`,
    ]),
  ];

  const surfaceRows = surfaces.map((s) => [
    s.language,
    String(s.hostMethodCount),
    `${s.exposedCount}/${s.applicableCount}`,
    `${s.coverage}%`,
    String(s.unmatchedHostMethods.length),
  ]);

  const domainNames = Object.keys(traced[0].domains);
  const matrixHeaders = [
    'Domain',
    'Engine methods',
    'Exempt',
    ...traced.map((b) => b.language),
    ...native.map((b) => b.language),
  ];
  const matrixRows = domainNames.map((domain) => {
    const engineDomain = report.engine.domains.find((d) => d.name === domain);
    return [
      `\`${domain}\``,
      String(engineDomain.methods.length),
      String(traced[0].domains[domain].exempt.length),
      ...traced.map((b) => cell(b.domains[domain])),
      ...native.map((b) => cell(b.domains[domain])),
    ];
  });

  const exemptRows = Object.entries(baseline.exempt ?? {}).flatMap(([scope, entries]) =>
    Object.entries(entries).map(([key, reason]) => [scope, `\`${key}\``, reason]),
  );

  const gapRows = ['python', 'go'].map((id) => {
    const gaps = flatten(baseline.gaps?.[id]);
    return [id, String(gaps.size)];
  });

  const detailSections = domainNames
    .map((domain) => {
      const engineDomain = report.engine.domains.find((d) => d.name === domain);
      const rows = engineDomain.methods.map((m) => {
        const name = typeof m === 'string' ? m : m.name;
        const status = (b) => {
          const entry = b.domains[domain];
          if (entry.exempt.includes(name)) return 'exempt';
          return entry.exposed.includes(name) ? 'yes' : '—';
        };
        return [`\`${name}\``, ...traced.map(status)];
      });
      const title =
        domain === 'commerce' ? '`commerce` (root methods)' : `\`commerce.${domain}()\``;
      return `### ${title}\n\n${renderMarkdownTable(['Engine method', ...traced.map((b) => b.language)], rows)}`;
    })
    .join('\n\n');

  const renameRows = report.renames.map((r) => [
    r.binding,
    `\`${r.hostMethod}\``,
    r.engineMethods.map((m) => `\`${m}\``).join(', '),
    r.documented ? 'yes' : 'no',
  ]);

  const aliasRows = Object.entries(baseline.aliases ?? {})
    .filter(([key]) => !key.startsWith('$'))
    .map(([host, engineMethod]) => {
      const [domain] = host.split('.');
      return [`\`${host}\``, `\`${domain}.${engineMethod}\``];
    });

  const hollowRows = report.hollowSignals.map((s) => [
    `\`${s.file}:${s.line}\``,
    `\`${s.structLiteral}\``,
    `\`${s.field}: ${s.value}\``,
  ]);

  const unmatchedRows = surfaces.flatMap((s) =>
    s.unmatchedHostMethods.map((m) => [s.language, `\`${m}\``]),
  );

  return `# Binding Parity

This page is generated from the engine's public API (\`crates/stateset-embedded/src\`) and the
binding sources under \`bindings/\`. Do not edit it by hand. Regenerate it with:

\`\`\`bash
node ./scripts/ci/generate_binding_parity.mjs
\`\`\`

Machine-readable output lives at \`artifacts/compatibility/binding-parity.json\`. The package-level
inventory this builds on is [Binding API Inventory](binding-api-inventory.md).

## How exposure is measured

- **Domains** are the \`Commerce\` accessors that return a handle (\`commerce.orders()\`,
  \`commerce.promotions()\`, ...) plus the root \`commerce\` methods. Each domain's engine methods are
  the \`pub fn\` methods with a \`self\` receiver on the returned type. The engine exposes
  ${report.engine.domainCount} domains and ${report.engine.methodCount} methods.
- **Traced bindings** (Node, Python, Go): an engine method is *exposed* only when an exported
  binding method (or a helper it calls) actually calls it — \`commerce.promotions().get_by_code(\`.
  Names do not matter, so renames (\`GetLevel\` → \`get_stock\`) are credited correctly. A class
  counts for its primary domain; calls into other domains are recorded as cross-domain reach in the
  JSON but not counted.
- **Native reach** (the other FFI bindings): any engine call in the binding's Rust layer. The host
  language layer is not traced, so this is an upper bound on what users of that binding can reach.
- **Name-matched surfaces** (.NET, Swift, WASM host APIs): host method names mapped to engine
  names across conventions (\`getByCode\` / \`GetByCode\` / \`get_by_code\`) plus the alias map
  below. A match is a claim about the surface, not evidence of engine backing.
- **Exempt** methods are engine APIs that are not meant to cross a language boundary; they are
  listed with reasons in \`${PATHS.baseline}\`.

## Coverage summary

${renderMarkdownTable(['Binding', 'Evidence', 'Gated', 'Exposed', 'Coverage', 'Parity vs Node'], summaryRows)}

Coverage is exposed / (engine methods − exempt). Parity vs Node is the share of Node-exposed engine
methods the binding also exposes.

## Name-matched host surfaces

${renderMarkdownTable(['Binding', 'Host methods', 'Name-matched', 'Coverage', 'Unmatched host methods'], surfaceRows)}

## Per-domain matrix

Cells are exposed / applicable engine methods.

${renderMarkdownTable(matrixHeaders, matrixRows)}

## Gate

\`scripts/ci/check_release_hygiene.sh\` runs \`node ./scripts/ci/generate_binding_parity.mjs --check\`,
which fails when this page or the JSON artifact is stale, or when the current state disagrees with
\`${PATHS.baseline}\`:

1. a gated binding **loses** an engine method it exposed;
2. Node exposes a method that a gated binding lacks and the baseline does **not list the gap** —
   new gaps must be explicit;
3. a listed gap has been **closed** (stale entry — the gap list only shrinks);
4. a binding **gains** a method the baseline has not recorded (so a later loss is caught).

\`--update-baseline\` rewrites the baseline from the current state and refuses to record losses
unless \`--allow-loss\` is also passed. Gated bindings: ${(baseline.gated ?? []).map((id) => `\`${id}\``).join(', ')}
(reference: \`${baseline.reference}\`).

${renderMarkdownTable(['Binding', 'Known gaps vs Node (baseline)'], gapRows)}

## Exemptions

${renderMarkdownTable(['Scope', 'Engine method', 'Reason'], exemptRows)}

## Name mapping

Host methods whose name differs from the engine method they call. These do not affect coverage
(exposure is traced), but undocumented renames are where binding docs drift from the engine docs.
The documented alias map lives under \`aliases\` in \`${PATHS.baseline}\` and is also used for the
name-matched surfaces.

${renderMarkdownTable(['Alias (domain.host_name)', 'Engine method'], aliasRows)}

${renderMarkdownTable(['Binding', 'Host method', 'Engine method(s) called', 'Documented'], renameRows)}

## Hollow signals (report only)

Struct literals in engine-backed bindings that hand the engine an always-empty collection
(\`items: vec![]\` and similar). Each is a place where a binding may accept a call that can never
carry the data the engine supports — the defect class behind Python's line-item-less invoices,
purchase orders and promotions. They are not a CI failure; review each one.

${hollowRows.length ? renderMarkdownTable(['Location', 'Struct literal', 'Field'], hollowRows) : 'None found.'}

## Unmatched host methods (name-matched surfaces)

${unmatchedRows.length ? renderMarkdownTable(['Binding', 'Host method'], unmatchedRows) : 'None.'}

## Per-domain detail (traced bindings)

${detailSections}
`;
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

/**
 * The JSON artifact: per-domain `exposed` everywhere, `missing` only for the
 * traced bindings (it is derivable: engine methods - exposed - exempt), and
 * the exemptions once rather than per binding.
 */
export function compactReport(report, baseline) {
  const compactDomains = (record, withMissing) =>
    Object.fromEntries(
      Object.entries(record.domains)
        .filter(([, entry]) => withMissing || entry.exposed.length > 0)
        .map(([domain, entry]) => [
          domain,
          withMissing
            ? { coverage: entry.coverage, exposed: entry.exposed, missing: entry.missing }
            : { coverage: entry.coverage, exposed: entry.exposed },
        ]),
    );
  const strip = (record, withMissing) => {
    const { domains, ...rest } = record;
    return { ...rest, domains: compactDomains(record, withMissing) };
  };
  return {
    ...report,
    exempt: baseline.exempt ?? {},
    bindings: report.bindings.map((record) => strip(record, record.evidence.startsWith('traced'))),
    nameMatchedSurfaces: report.nameMatchedSurfaces.map((record) => strip(record, false)),
  };
}

function renderBaseline(baseline, current) {
  const next = {
    $comment: baseline.$comment,
    reference: baseline.reference,
    gated: baseline.gated,
    exempt: baseline.exempt,
    aliases: baseline.aliases,
    exposed: current.exposed,
    gaps: current.gaps,
  };
  return `${JSON.stringify(next, null, 2)}\n`;
}

async function verifyOutput(rootDir, relativePath, expected) {
  try {
    const actual = await readFile(path.join(rootDir, relativePath), 'utf8');
    if (compareStrings(actual, expected)) return true;
    console.error(
      `::error file=${relativePath}::Generated binding parity report is out of date. Run 'node ./${GENERATOR}'.`,
    );
  } catch (error) {
    const message = error instanceof Error ? error.message : 'unknown error';
    console.error(
      `::error file=${relativePath}::Unable to read generated binding parity report (${message}). Run 'node ./${GENERATOR}'.`,
    );
  }
  return false;
}

export async function main(argv = process.argv.slice(2), rootDir = defaultRoot) {
  const checkMode = argv.includes('--check');
  const updateBaseline = argv.includes('--update-baseline');
  const allowLoss = argv.includes('--allow-loss');

  let { report, gate, baseline } = await buildParity(rootDir);

  if (updateBaseline) {
    const losses = gate.errors.filter((error) => / lost engine method /.test(error));
    if (losses.length && !allowLoss) {
      for (const error of losses) console.error(`::error file=${PATHS.baseline}::${error}`);
      console.error('Refusing to record losses in the baseline without --allow-loss.');
      return 1;
    }
    const configErrors = gate.errors.filter((error) => /^(Exemption|Alias) /.test(error));
    if (configErrors.length) {
      for (const error of configErrors) console.error(`::error file=${PATHS.baseline}::${error}`);
      return 1;
    }
    await writeFile(
      path.join(rootDir, PATHS.baseline),
      renderBaseline(baseline, gate.current),
      'utf8',
    );
    console.log(`Updated ${PATHS.baseline} (${gate.errors.length} change(s) accepted).`);
    ({ report, gate, baseline } = await buildParity(rootDir));
  }

  const jsonContent = `${JSON.stringify(compactReport(report, baseline), null, 2)}\n`;
  const markdownContent = renderMarkdown(report, baseline);
  const summary = report.bindings
    .filter((b) => b.gated)
    .map((b) => `${b.id} ${b.coverage}%`)
    .join(', ');

  if (checkMode) {
    const fresh = await Promise.all([
      verifyOutput(rootDir, PATHS.json, jsonContent),
      verifyOutput(rootDir, PATHS.markdown, markdownContent),
    ]);
    for (const error of gate.errors) console.error(`::error file=${PATHS.baseline}::${error}`);
    if (!fresh.every(Boolean) || gate.errors.length) return 1;
    console.log(`Binding parity is up to date and within baseline (${summary}).`);
    return 0;
  }

  await mkdir(path.dirname(path.join(rootDir, PATHS.json)), { recursive: true });
  await writeFile(path.join(rootDir, PATHS.json), jsonContent, 'utf8');
  await writeFile(path.join(rootDir, PATHS.markdown), markdownContent, 'utf8');
  console.log(`Generated binding parity report (${summary}).`);
  if (gate.errors.length) {
    console.warn(
      `Baseline disagrees with the current state (${gate.errors.length} issue(s)); run with --check for details.`,
    );
  }
  return 0;
}

if (process.argv[1] && path.resolve(process.argv[1]) === __filename) {
  process.exitCode = await main();
}
