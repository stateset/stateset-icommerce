import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  GOVERNED_AUTONOMOUS_CAPABILITIES,
  PRODUCTION_LAUNCH_REQUIREMENTS,
  createBusinessBootstrap,
  evaluateBusinessReadiness,
} from '../../src/autonomous/business-bootstrap.js';

describe('autonomous business bootstrap', () => {
  it('creates one scoped deny-by-default control plane without credentials', () => {
    const bundle = createBusinessBootstrap({
      name: 'Ada Goods',
      ownerId: 'user:ada',
      jurisdiction: 'CA',
      baseCurrency: 'cad',
      createdAt: '2026-08-26T00:00:00.000Z',
    });

    assert.equal(bundle.manifest.id, 'business:ada-goods');
    assert.equal(bundle.manifest.operating_mode, 'preview');
    assert.equal(bundle.manifest.jurisdiction, 'CA');
    assert.equal(bundle.manifest.base_currency, 'CAD');
    assert.equal(bundle.manifest.metadata.credentials_embedded, false);
    assert.equal(bundle.principal.delegated_by, 'user:ada');
    assert.deepEqual(bundle.principal.capabilities, GOVERNED_AUTONOMOUS_CAPABILITIES);
    assert.deepEqual(Object.keys(bundle.policy.commands), GOVERNED_AUTONOMOUS_CAPABILITIES);

    for (const rule of Object.values(bundle.policy.commands)) {
      assert.deepEqual(rule.allowed_tenant_ids, ['tenant:ada-goods']);
      assert.deepEqual(rule.allowed_store_ids, ['store:ada-goods']);
      assert.equal(rule.requires_agent_delegation, true);
    }
    assert.equal(bundle.readiness.ready_for_preview, true);
    assert.equal(bundle.readiness.ready_for_apply, false);
    assert.deepEqual(bundle.readiness.blockers, PRODUCTION_LAUNCH_REQUIREMENTS);
  });

  it('refuses to declare production readiness until every launch rail is evidenced', () => {
    const bundle = createBusinessBootstrap({ name: 'World Agent', ownerId: 'user:owner' });
    bundle.manifest.operating_mode = 'apply';
    for (const requirement of PRODUCTION_LAUNCH_REQUIREMENTS) {
      bundle.manifest.launch_requirements[requirement] = `connector:${requirement}:verified`;
    }

    const readiness = evaluateBusinessReadiness(bundle);
    assert.equal(readiness.ready_for_preview, true);
    assert.equal(readiness.ready_for_apply, true);
    assert.deepEqual(readiness.blockers, []);
  });

  it('validates globally portable jurisdiction and currency identifiers', () => {
    assert.throws(
      () => createBusinessBootstrap({ name: 'Agent', ownerId: 'user:1', jurisdiction: 'Canada' }),
      /two-letter country code/,
    );
    assert.throws(
      () => createBusinessBootstrap({ name: 'Agent', ownerId: 'user:1', baseCurrency: 'dollars' }),
      /three-letter ISO 4217/,
    );
  });
});
