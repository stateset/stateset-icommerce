import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { z } from 'zod';
import { invoiceTools } from '../../src/tools/invoices.js';
import { creditTools } from '../../src/tools/credit.js';
import { costAccountingTools } from '../../src/tools/cost-accounting.js';
import { accountsReceivableTools } from '../../src/tools/accounts-receivable.js';
import { accountsPayableTools } from '../../src/tools/accounts-payable.js';

const tools = [
  ...invoiceTools,
  ...creditTools,
  ...costAccountingTools,
  ...accountsReceivableTools,
  ...accountsPayableTools,
];
const find = (name) => tools.find((tool) => tool.name === name);
describe('ERP decimal contracts', () => {
  for (const [toolName, field] of [
    ['record_invoice_payment', 'amount'],
    ['create_credit_account', 'creditLimit'],
    ['check_customer_credit', 'orderAmount'],
    ['adjust_credit_limit', 'newLimit'],
    ['set_item_cost', 'standardCost'],
    ['update_average_item_cost', 'unitCost'],
    ['create_credit_memo', 'amount'],
  ]) {
    it(`${toolName} accepts exact strings and rejects numeric money`, () => {
      const schema = find(toolName).inputSchema[field];
      assert.equal(schema.parse('9007199254740993.01'), '9007199254740993.01');
      assert.equal(schema.safeParse(0.1).success, false);
      assert.equal(schema.safeParse('NaN').success, false);
    });
  }
  it('validates invoice line strings before invoking the binding', async () => {
    const tool = find('create_invoice');
    let seen;
    const commerce = {
      invoices: {
        create: async (input) => {
          seen = input;
          return input;
        },
      },
    };
    const invoke = (items) =>
      tool.handler({
        commerce,
        allowApply: true,
        params: { customerId: 'customer', currency: 'USD', items: JSON.stringify(items) },
      });
    assert.equal(
      (await invoke([{ description: 'Line', quantity: 1, unitPrice: 0.1 }])).success,
      false,
    );
    assert.equal(seen, undefined);
    const response = await invoke([
      { description: 'Line', quantity: '1', unitPrice: '9007199254740993.01' },
    ]);
    assert.equal(response.success, true);
    assert.equal(seen.items[0].unitPrice, '9007199254740993.01');
  });
  it('preserves payment retry identity and preview gating', async () => {
    const tool = find('record_invoice_payment');
    const paymentId = '123e4567-e89b-42d3-a456-426614174000';
    const params = z
      .object(tool.inputSchema)
      .parse({ invoiceId: 'invoice', paymentId, amount: '0.10' });
    let seen;
    const commerce = {
      invoices: {
        recordPayment: async (_id, input) => {
          seen = input;
          return input;
        },
      },
    };
    await tool.handler({ commerce, params, allowApply: false });
    assert.equal(seen, undefined);
    await tool.handler({ commerce, params, allowApply: true });
    assert.equal(seen.paymentId, paymentId);
    assert.equal(seen.amount, '0.10');
  });
  it('records explicit bill-match overrides and never approves during preview', async () => {
    const tool = find('approve_bill');
    let seen;
    const commerce = {
      accountsPayable: {
        approveBill: async (...args) => {
          seen = args;
          return {};
        },
      },
    };
    const params = { billId: 'bill', overrideReason: 'Approved contractual exception' };
    await tool.handler({ commerce, params, allowApply: false });
    assert.equal(seen, undefined);
    await tool.handler({ commerce, params, allowApply: true, actorId: 'approver' });
    assert.deepEqual(seen, ['bill', params.overrideReason, 'approver']);
  });
});
