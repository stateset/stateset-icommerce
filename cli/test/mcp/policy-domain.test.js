// Unit tests for cli/src/mcp/policy-domain.js
//
// Covers `inferStaticPolicyDomain`'s priority order:
//   1. Exact match in `byName` (per-tool override)
//   2. Multi-part prefix matches (a2a_*, agent_card_*, custom_object_*)
//   3. First underscore-token hit in STATIC_POLICY_DOMAIN_BY_TOKEN
//   4. Default → "commerce"

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  STATIC_POLICY_DOMAIN_BY_TOKEN,
  inferPolicyDomain,
  inferStaticPolicyDomain,
} from '../../src/mcp/policy-domain.js';

describe('STATIC_POLICY_DOMAIN_BY_TOKEN', () => {
  it('maps every value to a non-empty string', () => {
    for (const [token, domain] of Object.entries(STATIC_POLICY_DOMAIN_BY_TOKEN)) {
      assert.ok(token.length > 0);
      assert.equal(typeof domain, 'string');
      assert.ok(domain.length > 0);
    }
  });

  it('routes singular and plural tokens to the same domain', () => {
    // Sanity: pluralization shouldn't move the policy domain.
    assert.equal(STATIC_POLICY_DOMAIN_BY_TOKEN.order, 'orders');
    assert.equal(STATIC_POLICY_DOMAIN_BY_TOKEN.orders, 'orders');
    assert.equal(STATIC_POLICY_DOMAIN_BY_TOKEN.customer, 'customers');
    assert.equal(STATIC_POLICY_DOMAIN_BY_TOKEN.customers, 'customers');
    assert.equal(STATIC_POLICY_DOMAIN_BY_TOKEN.cart, 'carts');
    assert.equal(STATIC_POLICY_DOMAIN_BY_TOKEN.carts, 'carts');
  });

  it('routes generic CRUD verbs to the umbrella "commerce" domain', () => {
    for (const verb of ['create', 'get', 'list', 'update', 'delete', 'set']) {
      assert.equal(STATIC_POLICY_DOMAIN_BY_TOKEN[verb], 'commerce');
    }
  });
});

describe('inferStaticPolicyDomain', () => {
  describe('falsy / non-string input', () => {
    it('falls back to "commerce" for null/undefined/empty/non-string', () => {
      assert.equal(inferStaticPolicyDomain(undefined, {}), 'commerce');
      assert.equal(inferStaticPolicyDomain(null, {}), 'commerce');
      assert.equal(inferStaticPolicyDomain('', {}), 'commerce');
      assert.equal(inferStaticPolicyDomain(42, {}), 'commerce');
      assert.equal(inferStaticPolicyDomain({}, {}), 'commerce');
    });
  });

  describe('per-tool override (priority 1)', () => {
    it('uses the per-tool map when an exact match exists', () => {
      const byName = { my_special_tool: 'special-domain' };
      assert.equal(
        inferStaticPolicyDomain('my_special_tool', byName),
        'special-domain',
      );
    });

    it('per-tool override beats token-based inference', () => {
      // Without override, "create_order" would resolve to "orders" via
      // the token "order". The override should win.
      const byName = { create_order: 'override-domain' };
      assert.equal(
        inferStaticPolicyDomain('create_order', byName),
        'override-domain',
      );
    });
  });

  describe('multi-part prefix matches (priority 2)', () => {
    it('routes a2a_* to "a2a"', () => {
      assert.equal(inferStaticPolicyDomain('a2a_request_quote', {}), 'a2a');
      assert.equal(inferStaticPolicyDomain('a2a_accept', {}), 'a2a');
    });

    it('routes agent_card_* to "agent_cards"', () => {
      assert.equal(
        inferStaticPolicyDomain('agent_card_register', {}),
        'agent_cards',
      );
      assert.equal(
        inferStaticPolicyDomain('agent_card_verify', {}),
        'agent_cards',
      );
    });

    it('routes custom_object_* to "custom_objects"', () => {
      assert.equal(
        inferStaticPolicyDomain('custom_object_create', {}),
        'custom_objects',
      );
      assert.equal(
        inferStaticPolicyDomain('custom_object_list', {}),
        'custom_objects',
      );
    });
  });

  describe('first-token hit (priority 3)', () => {
    it('CRUD verbs short-circuit to "commerce" when present in the map', () => {
      // The CRUD verbs (create, get, list, update, delete, set) are in the
      // token map mapping to "commerce". Since the inference walks tokens
      // left-to-right and returns on the *first* hit, these tools resolve
      // to "commerce" via the leading verb — the resource token is reached
      // only via per-tool overrides (TOOL_POLICY_DOMAIN_BY_NAME).
      assert.equal(inferStaticPolicyDomain('create_order', {}), 'commerce');
      assert.equal(inferStaticPolicyDomain('list_customers', {}), 'commerce');
      assert.equal(inferStaticPolicyDomain('get_subscription_plan', {}), 'commerce');
    });

    it('walks past tokens that are not in the map, lands on the next hit', () => {
      // "calculate" isn't in the map → walks past it. "tax" is → returns "tax".
      assert.equal(inferStaticPolicyDomain('calculate_tax', {}), 'tax');
      // "apply" isn't in the map → walks past it. "cart" is → returns "carts".
      assert.equal(
        inferStaticPolicyDomain('apply_cart_promotions', {}),
        'carts',
      );
    });

    it('returns the first matching token, regardless of later matches', () => {
      // "ship" is in the map → "orders". The function returns at "ship";
      // the later "order" token never gets evaluated. (Both happen to
      // resolve to "orders", which makes this a sanity check.)
      assert.equal(inferStaticPolicyDomain('ship_order', {}), 'orders');

      // First non-verb token wins — "cart" is first → "carts", even
      // though "payment" later in the name would map to "payments".
      assert.equal(
        inferStaticPolicyDomain('cart_payment_set', {}),
        'carts',
      );
    });
  });

  describe('default fallback (priority 4)', () => {
    it('falls back to "commerce" when no token matches', () => {
      assert.equal(
        inferStaticPolicyDomain('unrelated_tool_name', {}),
        'commerce',
      );
      assert.equal(
        inferStaticPolicyDomain('foo_bar_baz', {}),
        'commerce',
      );
    });
  });

  describe('default byName (no second arg)', () => {
    it('uses TOOL_POLICY_DOMAIN_BY_NAME from domain-registry by default', () => {
      // We don't assert specific contents (the registry can change) but
      // verify the function doesn't crash and returns a string.
      const result = inferStaticPolicyDomain('list_orders');
      assert.equal(typeof result, 'string');
      assert.ok(result.length > 0);
    });
  });
});

describe('inferPolicyDomain (with per-tool defs)', () => {
  it('uses the per-tool definition map when an entry is present', () => {
    const defs = new Map([
      ['my_special_tool', { policyDomain: 'special-domain' }],
    ]);
    assert.equal(inferPolicyDomain('my_special_tool', defs), 'special-domain');
  });

  it('falls back to static inference when the def has no policyDomain', () => {
    // Synthetic tool name that's guaranteed not in the real registry.
    // The def exists but carries no policyDomain → fall through to
    // static-token inference. Token "tax" maps to "tax".
    const defs = new Map([['xyzfoo_tax', { permission: 'write' }]]);
    assert.equal(inferPolicyDomain('xyzfoo_tax', defs), 'tax');
  });

  it('falls back to static inference when the def map is empty', () => {
    // Synthetic name; tokens are "xyzfoo" (no match) + "vector" (match).
    assert.equal(inferPolicyDomain('xyzfoo_vector', new Map()), 'vector');
  });

  it('handles a missing/undefined def map gracefully', () => {
    // Synthetic name to bypass any real registry entry.
    assert.equal(inferPolicyDomain('xyzfoo_tax', undefined), 'tax');
    assert.equal(inferPolicyDomain('xyzfoo_tax', null), 'tax');
  });

  it('handles a non-Map "byName" object gracefully (returns static)', () => {
    // Some callers might pass `{}` instead of a Map. The function uses
    // optional-chained `.get?.()`, so a plain object falls through
    // cleanly without throwing. Use a synthetic name so the fallback's
    // registry lookup doesn't muddy the assertion.
    assert.equal(inferPolicyDomain('xyzfoo_tax', {}), 'tax');
  });

  it('per-tool definition wins over the static-inference token match', () => {
    // Synthetic name + explicit override → override wins.
    const defs = new Map([
      ['xyzfoo_tax', { policyDomain: 'overridden-domain' }],
    ]);
    assert.equal(inferPolicyDomain('xyzfoo_tax', defs), 'overridden-domain');
  });
});
