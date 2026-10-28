import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { PermissionGate, TOOL_PERMISSIONS } from '../../src/permissions.js';
import { accountsPayableTools } from '../../src/tools/accounts-payable.js';
import { accountsReceivableTools } from '../../src/tools/accounts-receivable.js';
import { generalLedgerTools } from '../../src/tools/general-ledger.js';
import { invoiceTools } from '../../src/tools/invoices.js';

describe('finance permission separation', () => {
  const tools = [
    ...accountsPayableTools,
    ...accountsReceivableTools,
    ...generalLedgerTools,
    ...invoiceTools,
  ];
  for (const name of [
    'approve_bill',
    'create_credit_memo',
    'void_credit_memo',
    'record_invoice_payment',
    'post_journal_entry',
    'void_journal_entry',
    'revalue_gl',
    'close_month',
  ]) {
    it(`${name} cannot execute with write permission`, async () => {
      const gate = new PermissionGate({ level: 'write' });
      assert.equal(TOOL_PERMISSIONS[name], 'admin');
      assert.equal(tools.find((tool) => tool.name === name).permission, 'admin');
      assert.equal((await gate.checkPermission(name, {})).allowed, false);
    });
  }
  it('keeps bill drafting available to writers', () => {
    assert.equal(TOOL_PERMISSIONS.create_bill, 'write');
    assert.equal(
      accountsPayableTools.find((tool) => tool.name === 'create_bill').permission,
      'write',
    );
  });
});
