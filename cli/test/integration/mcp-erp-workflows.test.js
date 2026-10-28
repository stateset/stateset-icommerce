/** Real ERP workflows through MCP and the native engine; no mocked repositories. */
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createStatesetMcpServer } from '../../src/mcp-server.js';
import { getCommerce, getGlobalManager } from '../../src/database.js';

async function fixture(t, profile, governedLedger = false) {
  const dir = mkdtempSync(join(tmpdir(), 'erp-mcp-workflow-'));
  const dbPath = join(dir, 'store.db');
  const maker = createStatesetMcpServer({
    dbPath,
    allowApply: true,
    toolProfile: profile,
    actorId: 'maker',
  });
  const kernel = governedLedger
    ? {
        strict: false,
        storeId: 'store:erp',
        principal: {
          id: 'checker',
          kind: 'agent',
          tenantId: 'tenant:erp',
          delegatedBy: 'operator',
          capabilities: ['ledger.post'],
        },
        policy: {
          version: 'erp-test',
          commands: {
            'ledger.post': {
              required_capabilities: ['ledger.post'],
              requires_approval: false,
              requires_tenant: true,
              requires_store: true,
              allowed_tenant_ids: ['tenant:erp'],
              allowed_store_ids: ['store:erp'],
              requires_agent_delegation: true,
              requires_signed_authority: false,
            },
          },
          trusted_authority_keys: {},
        },
      }
    : undefined;
  const checker = createStatesetMcpServer({
    dbPath,
    allowApply: true,
    toolProfile: profile,
    actorId: 'checker',
    kernel,
  });
  t.after(async () => {
    await maker.dispose();
    await checker.dispose();
    getGlobalManager().close(dbPath);
    rmSync(dir, { recursive: true, force: true });
  });
  return { maker, checker, commerce: getCommerce(dbPath) };
}

async function call(server, name, params = {}) {
  const response = await server.executeTool(name, params);
  assert.equal(
    response.ok,
    true,
    `${name}: ${JSON.stringify(response.failure ?? response.result)}`,
  );
  assert.equal(response.preview, false, `${name} unexpectedly previewed`);
  return response.result;
}

test('operations agent receives a PO and puts accepted goods into shelf stock', async (t) => {
  const { maker, commerce } = await fixture(t, 'operations');
  const warehouse = await commerce.warehouse.createWarehouse({
    code: 'MCP',
    name: 'MCP warehouse',
  });
  const dock = await commerce.warehouse.createLocation({
    warehouseId: warehouse.id,
    locationType: 'receiving',
  });
  const shelf = await commerce.warehouse.createLocation({
    warehouseId: warehouse.id,
    locationType: 'bulk',
  });
  const supplier = await commerce.purchaseOrders.createSupplier({ name: 'MCP supplier' });
  const po = await commerce.purchaseOrders.create({
    supplierId: supplier.id,
    items: [{ sku: 'MCP-STOCK', name: 'Stock', quantity: 3, unitCostExact: '2.15' }],
  });
  await commerce.purchaseOrders.submit(po.id);
  await commerce.purchaseOrders.approve(po.id, 'buyer');
  await commerce.purchaseOrders.send(po.id);
  const [poLine] = await commerce.purchaseOrders.getItems(po.id);
  const { receipt: manual } = await call(maker, 'create_receipt', {
    receiptType: 'purchase_order',
    warehouseId: warehouse.id,
    purchaseOrderId: po.id,
    supplierId: supplier.id,
    items: [{ sku: poLine.sku, poLineId: poLine.id, expectedQuantity: '3', unitCost: '2.15' }],
  });
  assert.ok(manual.id);
  await call(maker, 'cancel_receipt', { receiptId: manual.id });
  const { receipt } = await call(maker, 'create_receipt_from_purchase_order', {
    purchaseOrderId: po.id,
    warehouseId: warehouse.id,
  });
  const { items } = await call(maker, 'get_receipt', { receiptId: receipt.id });
  assert.equal(items.length, 1);
  await call(maker, 'start_receiving', { receiptId: receipt.id });
  await call(maker, 'receive_receipt_items', {
    receiptId: receipt.id,
    receivingLocationId: dock.id,
    items: [{ receiptItemId: items[0].id, quantityReceived: '3' }],
  });
  await call(maker, 'complete_receiving', { receiptId: receipt.id });
  const { putAway } = await call(maker, 'create_put_away', {
    receiptId: receipt.id,
    receiptItemId: items[0].id,
    sku: 'MCP-STOCK',
    toLocationId: shelf.id,
    quantity: '3',
  });
  await call(maker, 'assign_put_away', { putAwayId: putAway.id, assignedTo: 'worker' });
  await call(maker, 'start_put_away', { putAwayId: putAway.id });
  await call(maker, 'complete_put_away', { putAwayId: putAway.id });
  assert.match((await commerce.inventory.getStock('MCP-STOCK')).totalOnHand, /^3(?:\.0+)?$/);
  assert.equal(
    (await call(maker, 'get_put_away', { putAwayId: putAway.id })).putAway.status,
    'completed',
  );
  const retry = await maker.executeTool('complete_put_away', { putAwayId: putAway.id });
  assert.equal(retry.ok, false);
  assert.match((await commerce.inventory.getStock('MCP-STOCK')).totalOnHand, /^3(?:\.0+)?$/);
});

test('finance agent creates, posts and reverses an exact journal, then closes and locks', async (t) => {
  const { checker } = await fixture(t, 'finance', true);
  const { accounts } = await call(checker, 'initialize_chart_of_accounts');
  const cash = accounts.find((a) => a.accountNumber === '1010');
  const sales = accounts.find((a) => a.accountNumber === '4010');
  const { period } = await call(checker, 'create_gl_period', {
    periodName: 'October 2026',
    fiscalYear: 2026,
    periodNumber: 10,
    startDate: '2026-10-01',
    endDate: '2026-10-31',
  });
  await call(checker, 'open_gl_period', { periodId: period.id });
  const { journalEntry } = await call(checker, 'create_journal_entry', {
    entryDate: '2026-10-15',
    description: 'Exact sale',
    lines: [
      { accountId: cash.id, debitAmount: '123.45' },
      { accountId: sales.id, creditAmount: '123.45' },
    ],
  });
  const posted = await call(checker, 'post_journal_entry', {
    journalEntryId: journalEntry.id,
    postedBy: 'spoofed',
  });
  assert.ok(posted.kernel);
  assert.equal(
    (await call(checker, 'get_journal_entry', { journalEntryId: journalEntry.id })).journalEntry
      .postedBy,
    'checker',
  );
  await call(checker, 'reverse_journal_entry', {
    journalEntryId: journalEntry.id,
    reversalDate: '2026-10-16',
  });
  assert.match(
    (await call(checker, 'get_gl_account_balance', { accountId: cash.id })).balance,
    /^0(?:\.0+)?$/,
  );
  await call(checker, 'close_gl_period', { periodId: period.id });
  await call(checker, 'reopen_gl_period', { periodId: period.id });
  await call(checker, 'close_gl_period', { periodId: period.id });
  await call(checker, 'lock_gl_period', { periodId: period.id });
  assert.match(
    (await call(checker, 'get_gl_period', { periodId: period.id })).period.status,
    /locked/i,
  );
});

test('bill and payment-run approval require another actor, and pay the exact bill currency', async (t) => {
  const { maker, checker, commerce } = await fixture(t, 'finance');
  const supplier = await commerce.purchaseOrders.createSupplier({ name: 'European supplier' });
  const { bill } = await call(maker, 'create_bill', {
    supplierId: supplier.id,
    currency: 'EUR',
    dueDate: '2026-10-31T00:00:00Z',
    items: [{ description: 'Goods', quantity: '1', unitPrice: '18.25' }],
  });
  assert.equal(bill.createdBy, 'maker');
  assert.equal((await maker.executeTool('approve_bill', { billId: bill.id })).ok, false);
  await call(checker, 'approve_bill', { billId: bill.id });
  const { result: run } = await call(maker, 'create_bill_payment_run', {
    paymentDate: '2026-10-27T00:00:00Z',
    paymentMethod: 'ach',
    billIds: [bill.id],
  });
  assert.equal(run.currency, 'EUR');
  assert.equal((await maker.executeTool('approve_bill_payment_run', { runId: run.id })).ok, false);
  await call(checker, 'approve_bill_payment_run', { runId: run.id });
  await call(checker, 'process_bill_payment_run', { runId: run.id });
  const paid = (await call(checker, 'get_bill', { billId: bill.id })).bill;
  assert.match(paid.status, /paid/i);
  assert.equal(paid.amountPaid, '18.25');
});

test('receivables agent applies cash, writes off part, reverses and releases cash', async (t) => {
  const { checker, commerce } = await fixture(t, 'finance');
  const customer = await commerce.customers.create({
    email: 'mcp-ar@example.invalid',
    firstName: 'AR',
    lastName: 'Test',
  });
  const { invoice } = await call(checker, 'create_invoice', {
    customerId: customer.id,
    items: JSON.stringify([{ description: 'Goods', quantity: '1', unitPrice: '100.00' }]),
  });
  await call(checker, 'send_invoice', { invoiceId: invoice.id });
  const payment = await commerce.payments.create({
    customerId: customer.id,
    amountExact: '20.00',
    currency: 'USD',
  });
  await commerce.payments.markCompleted(payment.id);
  const { result: applications } = await call(checker, 'apply_payment_to_invoices', {
    paymentId: payment.id,
    applications: [{ invoiceId: invoice.id, amount: '20.00' }],
  });
  const { result: writeOff } = await call(checker, 'create_write_off', {
    invoiceId: invoice.id,
    amount: '10.00',
    reason: 'small_balance',
  });
  assert.notEqual(
    (await call(checker, 'get_invoice', { invoiceId: invoice.id })).invoice.status.toLowerCase(),
    'written_off',
  );
  assert.equal(writeOff.approvedBy, 'checker');
  await call(checker, 'reverse_write_off', { writeOffId: writeOff.id });
  await call(checker, 'unapply_invoice_payment', { applicationId: applications[0].id });
  assert.match(
    (await call(checker, 'get_invoice', { invoiceId: invoice.id })).invoice.amountPaid,
    /^0(?:\.0+)?$/,
  );
});
