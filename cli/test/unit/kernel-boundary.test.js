import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import {
  classifyKernelToolBoundary,
  isMutationPermission,
  KERNEL_GOVERNED_COMPOSITE_TOOLS,
  selectStrictKernelToolDefinitions,
} from '../../src/kernel-boundary.js';
import { KERNEL_CAPABILITY_BY_TOOL } from '../../src/kernel-tool-execution.js';
import { AGENTIC_RUNTIME_TOOLS } from '../../src/mcp/agentic-runtime-tools.js';
import { ALL_DOMAIN_TOOLS } from '../../src/tools/domain-registry.js';

const ALL_TOOLS = [...ALL_DOMAIN_TOOLS, ...AGENTIC_RUNTIME_TOOLS];

describe('kernel mutation boundary', () => {
  it('classifies every registry tool and fails closed for non-read permissions', () => {
    const report = classifyKernelToolBoundary(ALL_TOOLS);
    assert.equal(report.counts.total, ALL_TOOLS.length);
    assert.equal(report.counts.readOnly + report.counts.mutations, report.counts.total);
    assert.equal(
      report.counts.governed + report.counts.governedComposite + report.counts.blocked,
      report.counts.mutations,
    );
    assert.match(report.digest, /^sha256:[0-9a-f]{64}$/);

    for (const entry of report.entries) {
      assert.equal(entry.mutation, entry.permission !== 'read');
      assert.equal(
        entry.disposition,
        entry.permission === 'read'
          ? 'read_only'
          : KERNEL_CAPABILITY_BY_TOOL[entry.name]
            ? 'governed'
            : KERNEL_GOVERNED_COMPOSITE_TOOLS.includes(entry.name)
              ? 'governed_composite'
              : 'blocked',
      );
    }
  });

  it('strict exposure contains only reads, typed commands, and governed composites', () => {
    const exposed = selectStrictKernelToolDefinitions(ALL_TOOLS);
    const exposedNames = new Set(exposed.map((tool) => tool.name));

    for (const tool of ALL_TOOLS) {
      assert.equal(
        exposedNames.has(tool.name),
        tool.permission === 'read' ||
          Boolean(KERNEL_CAPABILITY_BY_TOOL[tool.name]) ||
          KERNEL_GOVERNED_COMPOSITE_TOOLS.includes(tool.name),
        tool.name,
      );
    }
    assert.equal(exposedNames.has('delete_customer'), false);
    assert.equal(exposedNames.has('backup_database'), false);
    assert.equal(exposedNames.has('set_exchange_rate'), false);
    assert.equal(exposedNames.has('create_payment'), true);
    assert.equal(exposedNames.has('agentic_execute_plan'), true);
    const composite = classifyKernelToolBoundary(ALL_TOOLS).entries.find(
      (entry) => entry.name === 'agentic_execute_plan',
    );
    assert.equal(composite.permission, 'write');
    assert.equal(composite.disposition, 'governed_composite');
  });

  it('treats missing and future permission classes as mutations', () => {
    assert.equal(isMutationPermission('read'), false);
    assert.equal(isMutationPermission('write'), true);
    assert.equal(isMutationPermission('delete'), true);
    assert.equal(isMutationPermission('admin'), true);
    assert.equal(isMutationPermission('future_permission'), true);
    assert.equal(isMutationPermission(undefined), true);
  });

  it('rejects unnamed or duplicate registry entries', () => {
    assert.throws(() => classifyKernelToolBoundary([{ permission: 'read' }]), /unnamed tool/);
    assert.throws(
      () =>
        classifyKernelToolBoundary([
          { name: 'same', permission: 'read' },
          { name: 'same', permission: 'write' },
        ]),
      /duplicate tool/,
    );
  });
});
