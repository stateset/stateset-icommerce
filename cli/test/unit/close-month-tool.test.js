/**
 * close_month General Ledger Tool Test Suite
 *
 * The close_month tool orchestrates the month-end close (depreciation,
 * revenue recognition, FX revaluation, period close). It is allowApply-guarded
 * for real closes; dry runs are allowed without apply.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { generalLedgerTools } from '../../src/tools/general-ledger.js';
import { TOOL_POLICY_DOMAIN_BY_NAME } from '../../src/tools/domain-registry.js';

function findTool(tools, name) {
  const tool = tools.find((t) => t.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found`);
  return tool;
}

const tool = findTool(generalLedgerTools, 'close_month');

const sampleReport = {
  periodId: 'per_001',
  periodName: '2026-01',
  dryRun: false,
  depreciation: { status: 'executed', entryCount: 2, totalAmount: '200.00', warnings: [] },
  revenueRecognition: { status: 'executed', entryCount: 1, totalAmount: '100.00', warnings: [] },
  fxRevaluation: { status: 'skipped', entryCount: 0, totalAmount: '0', warnings: [] },
  periodClose: { status: 'executed', entryCount: 1, totalAmount: '100.00', warnings: [] },
  periodStatus: 'closed',
};

describe('close_month registration', () => {
  it('is a write tool in the general_ledger policy domain', () => {
    assert.equal(tool.permission, 'write');
    assert.equal(tool.policyDomain, 'general_ledger');
    assert.equal(TOOL_POLICY_DOMAIN_BY_NAME.close_month, 'general_ledger');
  });
});

describe('close_month handler', () => {
  it('requires allowApply for a real close', async () => {
    let called = false;
    const commerce = {
      generalLedger: {
        closeMonth: async () => {
          called = true;
          return sampleReport;
        },
      },
    };
    const result = await tool.handler({
      commerce,
      params: { periodId: 'per_001' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.equal(called, false);
  });

  it('allows a dry run without allowApply and passes dryRun through', async () => {
    let received;
    const commerce = {
      generalLedger: {
        closeMonth: async (periodId, options) => {
          received = { periodId, options };
          return { ...sampleReport, dryRun: true };
        },
      },
    };
    const result = await tool.handler({
      commerce,
      params: { periodId: 'per_001', dryRun: true },
      allowApply: false,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Close month dry run computed');
    assert.equal(received.periodId, 'per_001');
    assert.equal(received.options.dryRun, true);
    assert.equal(result.report.dryRun, true);
  });

  it('closes the month with skip flags and closedBy when applied', async () => {
    let received;
    const commerce = {
      generalLedger: {
        closeMonth: async (periodId, options) => {
          received = { periodId, options };
          return sampleReport;
        },
      },
    };
    const result = await tool.handler({
      commerce,
      params: {
        periodId: 'per_001',
        skipFxRevaluation: true,
        skipDepreciation: true,
        closedBy: 'controller',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.message, 'Month closed');
    assert.deepEqual(received, {
      periodId: 'per_001',
      options: {
        dryRun: false,
        skipDepreciation: true,
        skipRevenueRecognition: undefined,
        skipFxRevaluation: true,
        skipPeriodClose: undefined,
        closedBy: 'controller',
      },
    });
    assert.equal(result.report.periodStatus, 'closed');
    assert.equal(result.report.depreciation.entryCount, 2);
  });
});
