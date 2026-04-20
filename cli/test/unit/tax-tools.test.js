/**
 * Tax Tools — Provider Workflow Test Suite
 *
 * Covers provider-backed quote/commit/void APIs added to src/tools/tax.js.
 */

import { beforeEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { taxTools } from '../../src/tools/tax.js';
import { __resetTaxProviderState } from '../../src/tools/providers/tax.js';

function findTool(name) {
  const tool = taxTools.find((entry) => entry.name === name);
  if (!tool) throw new Error(`Tool '${name}' not found in taxTools`);
  return tool;
}

beforeEach(() => {
  __resetTaxProviderState();
});

describe('Tax provider tools — structure', () => {
  it('contains expected provider tools', () => {
    const names = taxTools.map((tool) => tool.name);
    for (const expected of [
      'calculate_item_tax',
      'get_tax_jurisdiction',
      'create_tax_jurisdiction',
      'get_tax_rate_record',
      'create_tax_rate',
      'get_tax_exemption',
      'check_customer_tax_exempt',
      'list_tax_providers',
      'update_tax_settings',
      'set_tax_enabled',
      'check_tax_enabled',
      'validate_tax_jurisdiction_compliance',
      'calculate_tax_quote',
      'calculate_tax_quote_with_failover',
      'get_tax_quote',
      'commit_tax_transaction',
      'get_tax_transaction',
      'list_tax_transactions',
      'void_tax_transaction',
      'ingest_tax_provider_webhook',
    ]) {
      assert.ok(names.includes(expected), `missing tool: ${expected}`);
    }
  });
});

function createCommerceTaxApi(overrides = {}) {
  return {
    tax: {
      calculateForItem: async () => 7.5,
      getJurisdiction: async (jurisdictionId) => ({ id: jurisdictionId, code: 'CA', name: 'California' }),
      getJurisdictionByCode: async (code) => ({ id: 'jur_001', code, name: 'California' }),
      createJurisdiction: async (params) => ({ id: 'jur_new', ...params }),
      getRate: async (rateId) => ({ id: rateId, rate: 0.0725, name: 'CA state rate' }),
      createRate: async (params) => ({ id: 'rate_new', ...params }),
      getExemption: async (exemptionId) => ({ id: exemptionId, customerId: 'cust_001' }),
      customerIsExempt: async (customerId) => customerId === 'cust_exempt',
      updateSettings: async (params) => ({ id: 'settings_001', ...params }),
      setEnabled: async (enabled) => ({ enabled }),
      isEnabled: async () => true,
      ...overrides,
    },
  };
}

describe('non-provider tax MCP tools', () => {
  it('calculate_item_tax delegates to commerce.tax.calculateForItem', async () => {
    const tool = findTool('calculate_item_tax');
    let calledWith = null;
    const commerce = createCommerceTaxApi({
      calculateForItem: async (...args) => {
        calledWith = args;
        return 8.25;
      },
    });

    const result = await tool.handler({
      commerce,
      params: {
        unitPrice: 55,
        quantity: 2,
        category: 'standard',
        shippingAddress: { country: 'US', state: 'CA', postalCode: '94105' },
      },
    });

    assert.equal(result.success, true);
    assert.equal(result.taxAmount, 8.25);
    assert.deepEqual(calledWith, [55, 2, 'standard', { country: 'US', state: 'CA', postalCode: '94105' }]);
  });

  it('get_tax_jurisdiction can resolve by code', async () => {
    const tool = findTool('get_tax_jurisdiction');
    const result = await tool.handler({
      commerce: createCommerceTaxApi(),
      params: { code: 'CA' },
    });
    assert.equal(result.success, true);
    assert.equal(result.jurisdiction.code, 'CA');
  });

  it('create_tax_jurisdiction requires --apply', async () => {
    const tool = findTool('create_tax_jurisdiction');
    const result = await tool.handler({
      commerce: createCommerceTaxApi(),
      params: {
        code: 'CA-SF',
        name: 'San Francisco',
        level: 'city',
        countryCode: 'US',
        stateCode: 'CA',
      },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('update_tax_settings persists settings with --apply', async () => {
    const tool = findTool('update_tax_settings');
    const result = await tool.handler({
      commerce: createCommerceTaxApi(),
      params: { enabled: true, taxProvider: 'avalara' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.settings.enabled, true);
    assert.equal(result.settings.taxProvider, 'avalara');
  });

  it('set_tax_enabled and check_tax_enabled reflect the commerce binding', async () => {
    const setTool = findTool('set_tax_enabled');
    const checkTool = findTool('check_tax_enabled');
    const commerce = createCommerceTaxApi({
      setEnabled: async (enabled) => ({ enabled }),
      isEnabled: async () => false,
    });

    const setResult = await setTool.handler({
      commerce,
      params: { enabled: false },
      allowApply: true,
    });
    const checkResult = await checkTool.handler({ commerce });

    assert.equal(setResult.success, true);
    assert.equal(setResult.settings.enabled, false);
    assert.equal(checkResult.success, true);
    assert.equal(checkResult.enabled, false);
  });
});

describe('list_tax_providers', () => {
  const tool = findTool('list_tax_providers');

  it('is read permission', () => {
    assert.equal(tool.permission, 'read');
  });

  it('returns providers', async () => {
    const result = await tool.handler({ params: {} });
    assert.equal(result.success, true);
    assert.ok(result.count >= 2);
    assert.ok(result.providers.some((provider) => provider.id === 'deterministic-mock'));
  });
});

describe('calculate_tax_quote', () => {
  const tool = findTool('calculate_tax_quote');
  const params = {
    providerId: 'deterministic-mock',
    items: [
      { id: 'line-1', unitPrice: 100, quantity: 1, taxCategory: 'standard' },
      { id: 'line-2', unitPrice: 25, quantity: 2, taxCategory: 'reduced' },
    ],
    shippingAddress: {
      country: 'US',
      state: 'CA',
      city: 'Los Angeles',
      postalCode: '90001',
    },
    shippingAmount: 9.99,
    currency: 'USD',
  };

  it('returns quote result', async () => {
    const result = await tool.handler({ params });
    assert.equal(result.success, true);
    assert.ok(result.quote.id);
    assert.equal(result.quote.providerId, 'deterministic-mock');
    assert.ok(Number.parseFloat(result.quote.totalTax) > 0);
  });

  it('is idempotent with idempotencyKey', async () => {
    const first = await tool.handler({
      params: {
        ...params,
        idempotencyKey: 'tax-quote-idem-1',
      },
    });
    const second = await tool.handler({
      params: {
        ...params,
        idempotencyKey: 'tax-quote-idem-1',
      },
    });
    assert.equal(first.quote.id, second.quote.id);
    assert.equal(second.idempotent, true);
  });
});

describe('tax compliance and failover tools', () => {
  const complianceTool = findTool('validate_tax_jurisdiction_compliance');
  const failoverTool = findTool('calculate_tax_quote_with_failover');

  it('validate_tax_jurisdiction_compliance reports missing required fields in strict mode', async () => {
    const result = await complianceTool.handler({
      params: {
        items: [{ id: 'line-1', unitPrice: 10, quantity: 1 }],
        shippingAddress: { country: 'US' },
        strictCompliance: true,
      },
    });
    assert.equal(result.success, true);
    assert.equal(result.compliance.valid, false);
    assert.ok(result.compliance.errors.some((entry) => entry.includes('shippingAddress.state')));
    assert.ok(result.compliance.errors.some((entry) => entry.includes('shippingAddress.postalCode')));
  });

  it('calculate_tax_quote_with_failover falls back when primary provider does not support country', async () => {
    const result = await failoverTool.handler({
      params: {
        providerId: 'taxjar',
        fallbackProviderIds: ['avalara', 'deterministic-mock'],
        items: [{ id: 'line-1', unitPrice: 55, quantity: 1, taxCategory: 'standard' }],
        shippingAddress: { country: 'DE', postalCode: '10115', city: 'Berlin' },
        currency: 'EUR',
      },
    });
    assert.equal(result.success, true);
    assert.equal(result.quote.providerId, 'avalara');
    assert.equal(result.failover.selectedProviderId, 'avalara');
    assert.equal(result.failover.attempted, true);
  });
});

describe('commit_tax_transaction', () => {
  const quoteTool = findTool('calculate_tax_quote');
  const getQuoteTool = findTool('get_tax_quote');
  const commitTool = findTool('commit_tax_transaction');
  const getTransactionTool = findTool('get_tax_transaction');
  const listTransactionsTool = findTool('list_tax_transactions');

  async function createQuoteId() {
    const quoteResult = await quoteTool.handler({
      params: {
        providerId: 'deterministic-mock',
        items: [{ id: 'line-1', unitPrice: 49.99, quantity: 1 }],
        shippingAddress: { country: 'US', state: 'TX', postalCode: '78701' },
      },
    });
    return quoteResult.quote.id;
  }

  it('requires --apply', async () => {
    const quoteId = await createQuoteId();
    const result = await commitTool.handler({
      params: { quoteId },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('commits transaction with allowApply true', async () => {
    const quoteId = await createQuoteId();
    const result = await commitTool.handler({
      params: {
        quoteId,
        transactionReference: 'order-abc-123',
      },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.ok(result.transaction.id);
    assert.equal(result.transaction.status, 'committed');

    const quoteLookup = await getQuoteTool.handler({ params: { quoteId } });
    assert.equal(quoteLookup.success, true);
    assert.equal(quoteLookup.quote.status, 'committed');

    const transactionLookup = await getTransactionTool.handler({
      params: { transactionId: result.transaction.id },
    });
    assert.equal(transactionLookup.success, true);
    assert.equal(transactionLookup.transaction.status, 'committed');

    const listResult = await listTransactionsTool.handler({ params: { quoteId } });
    assert.equal(listResult.success, true);
    assert.equal(listResult.count, 1);
  });
});

describe('void_tax_transaction', () => {
  const quoteTool = findTool('calculate_tax_quote');
  const commitTool = findTool('commit_tax_transaction');
  const voidTool = findTool('void_tax_transaction');
  const ingestWebhookTool = findTool('ingest_tax_provider_webhook');

  async function createCommittedTransaction() {
    const quoteResult = await quoteTool.handler({
      params: {
        providerId: 'deterministic-mock',
        items: [{ id: 'line-1', unitPrice: 80, quantity: 1 }],
        shippingAddress: { country: 'US', state: 'NY', postalCode: '10001' },
      },
    });
    const commitResult = await commitTool.handler({
      params: { quoteId: quoteResult.quote.id },
      allowApply: true,
    });
    return commitResult.transaction.id;
  }

  it('requires --apply', async () => {
    const transactionId = await createCommittedTransaction();
    const result = await voidTool.handler({
      params: { transactionId, reason: 'duplicate_entry' },
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
  });

  it('voids transaction with allowApply true', async () => {
    const transactionId = await createCommittedTransaction();
    const result = await voidTool.handler({
      params: { transactionId, reason: 'customer_canceled' },
      allowApply: true,
    });
    assert.equal(result.success, true);
    assert.equal(result.transaction.status, 'voided');
    assert.equal(result.quote.status, 'voided');
  });

  it('ingest_tax_provider_webhook commits quote when transaction is missing', async () => {
    const quoteResult = await quoteTool.handler({
      params: {
        providerId: 'deterministic-mock',
        items: [{ id: 'line-1', unitPrice: 30, quantity: 1 }],
        shippingAddress: { country: 'US', state: 'CA', postalCode: '94105' },
      },
    });

    const committed = await ingestWebhookTool.handler({
      params: {
        providerId: 'deterministic-mock',
        eventType: 'transaction.committed',
        eventId: 'tax_evt_1',
        payload: {
          quoteId: quoteResult.quote.id,
          reference: 'webhook-commit-ref',
        },
      },
      allowApply: true,
    });
    assert.equal(committed.success, true);
    assert.equal(committed.webhook.action, 'committed');
    assert.equal(committed.webhook.transaction.status, 'committed');
  });

  it('ingest_tax_provider_webhook is idempotent for duplicate event IDs', async () => {
    const transactionId = await createCommittedTransaction();
    const params = {
      providerId: 'deterministic-mock',
      eventType: 'transaction.voided',
      eventId: 'tax_evt_dup',
      payload: {
        transactionId,
        reason: 'duplicate',
      },
    };

    const first = await ingestWebhookTool.handler({ params, allowApply: true });
    const second = await ingestWebhookTool.handler({ params, allowApply: true });
    assert.equal(first.webhook.idempotent, false);
    assert.equal(second.webhook.idempotent, true);
  });
});
