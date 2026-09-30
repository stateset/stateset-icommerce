/**
 * Every binding method an MCP tool or `executeCommand` module calls must exist
 * on the real binding.
 *
 * Tool unit tests hand handlers a mock `commerce`, and a mock can invent any
 * method it likes. `apply_cart_promotions` called `commerce.applyCartPromotions`
 * and the whole subscriptions domain called `commerce.listSubscriptionPlans`,
 * `commerce.createSubscription`, ... -- none of which the Node binding has ever
 * had -- so every real call threw a TypeError while the unit tests stayed green.
 *
 * This reads each tool module's source, extracts every
 * `commerce.<api>.<method>(`, `commerce.<api>().<method>(` and
 * `commerce.<method>(` call, and resolves it against a real `Commerce` adapted
 * exactly as the MCP server adapts it. The `executeCommand` modules in
 * `src/commands` get the same check -- they had the same dead calls -- and,
 * since their callers pass a raw `Commerce`, may not call an API as a function.
 * Not checked here:
 *   - `a2a` / `x402`, which the server attaches in JavaScript;
 *   - `_`-prefixed services (`commerce._costAnalytics`), attached by the A2A
 *     service layer; the tools that use them check for them first.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { readdirSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module';

import { adaptCommerceForTools, extendCommerceWithApis } from '../../src/mcp/commerce-adapter.js';
import { adaptCommerceApis } from '../../src/commerce.js';

const require = createRequire(import.meta.url);
const here = path.dirname(fileURLToPath(import.meta.url));
const srcDir = path.resolve(here, '../../src');
// Both library surfaces that hand a `commerce` to binding calls: the MCP tool
// modules and the `executeCommand` command modules.
const SCANNED_DIRS = ['tools', 'commands'];

const JS_ATTACHED_APIS = new Set(['a2a', 'x402']);
const CALL = /\bcommerce\.(\w+)(\(\))?(?:\.(\w+))?\s*\(/g;

/** Every binding call site in the scanned modules, with its file and line. */
function bindingCallSites(dir) {
  const sites = [];
  const absDir = path.join(srcDir, dir);
  for (const file of readdirSync(absDir)
    .filter((f) => f.endsWith('.js'))
    .sort()) {
    const source = readFileSync(path.join(absDir, file), 'utf8');
    for (const match of source.matchAll(CALL)) {
      const [, api, invoked, method] = match;
      if (JS_ATTACHED_APIS.has(api) || api.startsWith('_')) continue;
      const line = source.slice(0, match.index).split('\n').length;
      sites.push({
        where: `src/${dir}/${file}:${line}`,
        call: method ? `commerce.${api}${invoked ?? ''}.${method}` : `commerce.${api}`,
        api,
        invoked: Boolean(invoked),
        method,
      });
    }
  }
  return sites;
}

let raw;
let adapted;
try {
  const { Commerce } = require('@stateset/embedded');
  raw = new Commerce(':memory:');
  // The exact adapter stack `createStatesetMcpServer` builds.
  adapted = adaptCommerceApis(
    extendCommerceWithApis(adaptCommerceForTools(new Commerce(':memory:')), { a2a: () => ({}) }),
  );
} catch (error) {
  raw = null;
  console.warn(`[tool-binding-surface] native binding unavailable: ${error.message}`);
}

const MIN_SITES = { tools: 300, commands: 100 };
// MCP tools always receive the commerce the server adapts, whose callable
// Proxy also answers `commerce.promotions()`. `executeCommand` callers pass
// their own `new Commerce(...)`, on which `commerce.promotions()` throws, so
// the command modules must use the property form.
const RECEIVES_RAW_BINDING = { tools: false, commands: true };

for (const dir of SCANNED_DIRS) {
  describe(`src/${dir} calls only methods the binding has`, { skip: !raw }, () => {
    const sites = bindingCallSites(dir);

    it('finds the call sites it is meant to check', () => {
      // A regex that silently matched nothing would pass everything.
      assert.ok(
        sites.length > MIN_SITES[dir],
        `only ${sites.length} call sites found in src/${dir}`,
      );
    });

    it('resolves every call site on the raw binding', () => {
      // Checked on the binding itself, not through the adapter: the adapter's
      // callable Proxy is a function, and resolving through it once answered
      // `apply` / `call` / `bind` with Function.prototype's, so a missing method
      // with one of those names looked present.
      const missing = sites.filter(({ api, invoked, method }) => {
        const target = raw[api];
        if (method === undefined) return typeof target !== 'function';
        if (invoked && RECEIVES_RAW_BINDING[dir] && typeof target !== 'function') return true;
        return typeof target?.[method] !== 'function';
      });
      assert.deepEqual(
        missing.map(({ where, call }) => `${where}  ${call}`),
        [],
        `these src/${dir} modules call binding methods that do not exist, so every real call throws`,
      );
    });
  });
}

describe(
  'the MCP adapter reaches binding methods named like Function members',
  { skip: !raw },
  () => {
    it('routes storeCredits.apply to the binding, not Function.prototype.apply', async () => {
      // Before the fix this threw "CreateListFromArrayLike called on non-object".
      assert.notEqual(adapted.storeCredits.apply, Function.prototype.apply);
      await assert.rejects(
        adapted.storeCredits.apply('not-a-uuid', '1.00'),
        (error) => !(error instanceof TypeError) && /uuid/i.test(error.message),
      );
    });

    it('still answers promotions.apply with the binding method', async () => {
      const result = await adapted.promotions.apply({ lineItems: [], subtotal: 10 });
      assert.equal(result.totalDiscountExact, '0');
    });
  },
);
