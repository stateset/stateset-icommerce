/**
 * Drift-prevention gate: embedded accessor surface vs. Node binding getters.
 *
 * Parses the `pub use module::Type;` accessor re-exports from
 * crates/stateset-embedded/src/lib.rs and the `get name(): Type` getters on the
 * Commerce class in bindings/node/index.d.ts, then reports embedded accessor
 * domains that have no corresponding binding getter.
 *
 * Dependency-light by design (fs + regex), mirroring mcp-api-coverage.js.
 */
import { readFileSync } from 'node:fs';

export const EMBEDDED_LIB_RS = new URL(
  '../../../crates/stateset-embedded/src/lib.rs',
  import.meta.url,
);

export const BINDING_INDEX_DTS = new URL('../../../bindings/node/index.d.ts', import.meta.url);

/**
 * Embedded modules that are not per-domain accessors (core plumbing,
 * feature-gated subsystems, or types re-exported for advanced users) and are
 * therefore exempt from the binding-getter requirement.
 */
export const NON_ACCESSOR_MODULES = Object.freeze([
  'commerce', // Commerce itself — the binding's root class, not a getter
  'notifications', // notification service types, not a Commerce accessor
  'events', // event system; the binding exposes `events` separately anyway
  'vector', // feature-gated sqlite-only vector search
  'async_commerce', // postgres async API; bindings wrap the sync surface
]);

/**
 * Embedded accessor domains knowingly absent from the Node binding.
 *
 * Every entry documents an intentional gap. The gate fails closed: an entry
 * whose getter appears in index.d.ts, or whose module vanishes from
 * lib.rs, must be removed. Adding a NEW embedded accessor without either a
 * binding getter or an entry here fails the gate.
 */
export const KNOWN_UNBOUND_ACCESSORS = Object.freeze([]);

/** Convert a snake_case module name to the camelCase binding getter name. */
export function moduleToGetter(moduleName) {
  return moduleName.replace(/_([a-z0-9])/g, (_, ch) => ch.toUpperCase());
}

/**
 * Accessor domain modules from stateset-embedded's lib.rs: the module part of
 * every single-type `pub use <module>::<Type>;` re-export.
 */
export function parseEmbeddedAccessorModules(source = readFileSync(EMBEDDED_LIB_RS, 'utf8')) {
  const modules = new Set();
  const pattern = /^pub use ([a-z0-9_]+)::([A-Za-z0-9]+);$/gm;
  for (const [, moduleName] of source.matchAll(pattern)) {
    // Only local accessor modules count: they are declared with `mod x;` in the
    // same file. This filters re-exports from other crates (e.g. stateset_db).
    const isLocalModule =
      source.includes(`\nmod ${moduleName};`) || source.includes(`\npub mod ${moduleName};`);
    if (isLocalModule && !NON_ACCESSOR_MODULES.includes(moduleName)) modules.add(moduleName);
  }
  return [...modules].sort();
}

/** Getter names declared on the Commerce class in bindings/node/index.d.ts. */
export function parseBindingGetters(source = readFileSync(BINDING_INDEX_DTS, 'utf8')) {
  const getters = new Set();
  for (const [, name] of source.matchAll(/^\s*get ([A-Za-z0-9]+)\(\)/gm)) {
    getters.add(name);
  }
  return [...getters].sort();
}

/**
 * Run the parity check.
 *
 * @returns {{ missing: string[], staleExceptions: string[], problems: string[] }}
 *   `problems` is empty when the gate passes; otherwise it holds actionable
 *   failure messages.
 */
export function checkBindingAccessorParity({
  embeddedModules = parseEmbeddedAccessorModules(),
  bindingGetters = parseBindingGetters(),
  knownUnbound = KNOWN_UNBOUND_ACCESSORS,
} = {}) {
  const getters = new Set(bindingGetters);
  const moduleSet = new Set(embeddedModules);
  const problems = [];

  const missing = embeddedModules.filter(
    (mod) => !knownUnbound.includes(mod) && !getters.has(moduleToGetter(mod)),
  );
  if (missing.length > 0) {
    problems.push(
      `Embedded accessor domain(s) without a Node binding getter: ${missing.join(', ')}. ` +
        `Add a \`get ${moduleToGetter(missing[0])}()\` accessor to bindings/node (index.d.ts + ` +
        `src/lib.rs + index.js), or — only for intentionally internal accessors — add the ` +
        `module to KNOWN_UNBOUND_ACCESSORS in cli/src/coverage/binding-accessor-parity.js ` +
        `with a justification.`,
    );
  }

  const staleExceptions = knownUnbound.filter(
    (mod) => !moduleSet.has(mod) || getters.has(moduleToGetter(mod)),
  );
  for (const mod of staleExceptions) {
    problems.push(
      getters.has(moduleToGetter(mod))
        ? `KNOWN_UNBOUND_ACCESSORS entry "${mod}" now has a binding getter ` +
            `(${moduleToGetter(mod)}) — remove it from the exception list so parity is enforced.`
        : `KNOWN_UNBOUND_ACCESSORS entry "${mod}" is no longer an embedded accessor module — ` +
            `remove the stale exception.`,
    );
  }

  return { missing, staleExceptions, problems };
}
