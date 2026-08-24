// Unit tests for cli/src/mcp/permission-gating.js
//
// Covers `buildReadOnlyToolSet` and `createCheckPermission`:
//  - a configured PermissionGate wins and its result is passed through
//  - without a gate, --apply or a read-only tool allows the call
//  - otherwise the call is downgraded to a preview with a `wouldDo` block
//  - every branch emits a `permission_decision` telemetry event with the
//    exact pre-extraction shape

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { buildReadOnlyToolSet, createCheckPermission } from '../../src/mcp/permission-gating.js';

function makeTelemetry() {
  const events = [];
  return {
    events,
    logCustomEvent: (name, payload) => events.push({ name, payload }),
  };
}

describe('buildReadOnlyToolSet', () => {
  it('collects only tools whose permission is "read"', () => {
    const set = buildReadOnlyToolSet([
      { name: 'list_orders', permission: 'read' },
      { name: 'create_order', permission: 'write' },
      { name: 'get_order', permission: 'read' },
      { name: 'mystery' },
    ]);
    assert.deepEqual([...set].sort(), ['get_order', 'list_orders']);
  });
});

describe('createCheckPermission', () => {
  it('delegates to the permission gate and logs its decision', async () => {
    const telemetry = makeTelemetry();
    const gateCalls = [];
    const permissionGate = {
      checkPermission: async (tool, params) => {
        gateCalls.push({ tool, params });
        return { allowed: false, preview: false, reason: 'denied by gate' };
      },
    };
    const checkPermission = createCheckPermission({
      permissionGate,
      telemetry,
      allowApply: true,
      isReadOnly: () => true,
    });

    const result = await checkPermission('create_order', { a: 1 });

    assert.deepEqual(result, { allowed: false, preview: false, reason: 'denied by gate' });
    assert.deepEqual(gateCalls, [{ tool: 'create_order', params: { a: 1 } }]);
    assert.deepEqual(telemetry.events, [
      {
        name: 'permission_decision',
        payload: { tool: 'create_order', allowed: false, preview: false, reason: 'denied by gate' },
      },
    ]);
  });

  it('normalizes missing preview/reason from the gate result in telemetry', async () => {
    const telemetry = makeTelemetry();
    const checkPermission = createCheckPermission({
      permissionGate: { checkPermission: async () => ({ allowed: true }) },
      telemetry,
      allowApply: false,
      isReadOnly: () => false,
    });
    await checkPermission('x', {});
    assert.deepEqual(telemetry.events[0].payload, {
      tool: 'x',
      allowed: true,
      preview: false,
      reason: null,
    });
  });

  it('allows when --apply is set (no gate)', async () => {
    const telemetry = makeTelemetry();
    const checkPermission = createCheckPermission({
      permissionGate: null,
      telemetry,
      allowApply: true,
      isReadOnly: () => false,
    });
    const result = await checkPermission('create_order', {});
    assert.deepEqual(result, { allowed: true });
    assert.deepEqual(telemetry.events, [
      {
        name: 'permission_decision',
        payload: { tool: 'create_order', allowed: true, preview: false },
      },
    ]);
  });

  it('allows read-only tools without --apply', async () => {
    const checkPermission = createCheckPermission({
      permissionGate: null,
      telemetry: null,
      allowApply: false,
      isReadOnly: (name) => name === 'list_orders',
    });
    assert.deepEqual(await checkPermission('list_orders', {}), { allowed: true });
  });

  it('downgrades write tools to preview without --apply', async () => {
    const telemetry = makeTelemetry();
    const checkPermission = createCheckPermission({
      permissionGate: null,
      telemetry,
      allowApply: false,
      isReadOnly: () => false,
    });
    const params = { customerId: 'c1' };
    const result = await checkPermission('create_order', params);
    assert.deepEqual(result, {
      allowed: false,
      preview: true,
      reason: "Preview mode: would execute 'create_order' if --apply flag is set",
      wouldDo: { tool: 'create_order', params },
    });
    assert.deepEqual(telemetry.events, [
      {
        name: 'permission_decision',
        payload: {
          tool: 'create_order',
          allowed: false,
          preview: true,
          reason: result.reason,
        },
      },
    ]);
  });

  it('works without telemetry', async () => {
    const checkPermission = createCheckPermission({
      permissionGate: null,
      telemetry: null,
      allowApply: false,
      isReadOnly: () => false,
    });
    const result = await checkPermission('create_order', {});
    assert.equal(result.preview, true);
  });
});
