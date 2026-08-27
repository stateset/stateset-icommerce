import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { loadKernelConfig } from '../../src/kernel-config.js';

let fixtureDir;

afterEach(() => {
  if (fixtureDir) rmSync(fixtureDir, { recursive: true, force: true });
  fixtureDir = undefined;
});

function fixtures() {
  fixtureDir = mkdtempSync(join(tmpdir(), 'stateset-kernel-config-'));
  const policyPath = join(fixtureDir, 'policy.json');
  const principalPath = join(fixtureDir, 'principal.json');
  writeFileSync(
    policyPath,
    JSON.stringify({
      version: 'production-v1',
      commands: { 'payments.create': { required_capabilities: ['payments.create'] } },
      trusted_authority_keys: {},
    }),
  );
  writeFileSync(
    principalPath,
    JSON.stringify({
      id: 'agent:checkout',
      kind: 'agent',
      tenant_id: 'tenant:acme',
      delegated_by: 'user:operator',
      capabilities: ['payments.create'],
    }),
  );
  return { policyPath, principalPath };
}

describe('trusted kernel CLI configuration', () => {
  it('returns null when the operator did not configure a kernel profile', () => {
    assert.equal(loadKernelConfig({ env: {} }), null);
  });

  it('fails closed when durable apply is requested without trusted configuration', () => {
    assert.throws(
      () => loadKernelConfig({ requireForApply: true, env: {} }),
      /Apply mode requires trusted kernel configuration/,
    );
    assert.equal(
      loadKernelConfig({ requireForApply: true, allowLegacyWrites: true, env: {} }),
      null,
    );
  });

  it('loads a strict policy, principal, and store scope from trusted files', () => {
    const { policyPath, principalPath } = fixtures();
    const kernel = loadKernelConfig({
      policyPath,
      principalPath,
      storeId: 'store:west',
      env: {},
    });
    assert.equal(kernel.strict, true);
    assert.equal(kernel.policy.version, 'production-v1');
    assert.equal(kernel.principal.id, 'agent:checkout');
    assert.equal(kernel.storeId, 'store:west');
  });

  it('requires all three trusted inputs when any one is supplied', () => {
    const { policyPath } = fixtures();
    assert.throws(
      () => loadKernelConfig({ policyPath, env: {} }),
      /missing --kernel-principal, --kernel-store-id/,
    );
  });

  it('supports environment configuration and an explicit legacy escape hatch', () => {
    const { policyPath, principalPath } = fixtures();
    const kernel = loadKernelConfig({
      allowLegacyWrites: true,
      env: {
        STATESET_KERNEL_POLICY: policyPath,
        STATESET_KERNEL_PRINCIPAL: principalPath,
        STATESET_KERNEL_STORE_ID: 'store:legacy',
      },
    });
    assert.equal(kernel.strict, false);
    assert.equal(kernel.storeId, 'store:legacy');
  });

  it('rejects malformed policy and principal contracts before server startup', () => {
    const { policyPath, principalPath } = fixtures();
    writeFileSync(policyPath, JSON.stringify({ version: '', commands: {} }));
    assert.throws(
      () => loadKernelConfig({ policyPath, principalPath, storeId: 'store:test', env: {} }),
      /non-empty version/,
    );
    writeFileSync(policyPath, JSON.stringify({ version: 'v1', commands: {} }));
    writeFileSync(principalPath, JSON.stringify({ id: 'agent:test' }));
    assert.throws(
      () => loadKernelConfig({ policyPath, principalPath, storeId: 'store:test', env: {} }),
      /capabilities array/,
    );
  });
});
