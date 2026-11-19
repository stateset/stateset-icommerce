import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Commerce } from '@stateset/embedded';
import { registerAffiliate } from '../lib/campaigns.js';
import {
  reserveAffiliateBatch,
  affiliateBalances,
  confirmPayoutExport,
  settleAffiliateTransfer,
} from '../lib/affiliate-payouts.js';
import { onboardAffiliate, affiliateBankPayouts } from '../lib/seller-payouts.js';
import { saveMerchantRecord, readMerchantRecord } from '../lib/merchant-records.js';
import { portalReport } from '../lib/merchant-portal.js';
const dir = mkdtempSync(join(tmpdir(), 'stateset-commissions-')),
  db = join(dir, 'test.db');
let commerce = new Commerce(db);
const env = { NEXT_PUBLIC_BASE_URL: 'https://store.example.test' };
const now = Math.floor(Date.now() / 1000),
  paidAt = new Date((now - 60) * 1000).toISOString();
const transfers = new Map(),
  accounts = [],
  attempts = [];
let beforeAcceptance = true,
  interrupt = true,
  lowBalance = true;
const stripe = {
  accounts: {
    list: async function* () {
      yield* accounts;
    },
    create: async (params) => {
      const account = {
        ...params,
        id: 'acct_fixture',
        capabilities: { transfers: 'active' },
        payouts_enabled: true,
      };
      accounts.push(account);
      return account;
    },
    retrieve: async (id) => accounts.find((a) => a.id === id),
  },
  accountLinks: {
    create: async () => ({ url: 'https://connect.example.test', expires_at: now + 300 }),
  },
  transfers: {
    list: async function* ({ transfer_group }) {
      yield* [...transfers.values()].filter((t) => t.transfer_group === transfer_group);
    },
    retrieve: async (id) => transfers.get(id),
    create: async (params, { idempotencyKey }) => {
      attempts.push(idempotencyKey);
      if (lowBalance) {
        lowBalance = false;
        throw Object.assign(new Error('Insufficient balance'), { code: 'balance_insufficient' });
      }
      if (beforeAcceptance) {
        beforeAcceptance = false;
        throw new Error('No response before processing');
      }
      assert.equal(params.destination, accounts[0].id);
      assert.equal(transfers.size, 0);
      const transfer = {
        ...params,
        id: 'tr_commission',
        created: now,
        amount_reversed: 0,
        destination_payment: 'py_affiliate',
      };
      transfers.set(transfer.id, transfer);
      if (interrupt) {
        interrupt = false;
        throw new Error('Connection lost after provider accepted');
      }
      return transfer;
    },
    listReversals: async function* (transfer) {
      yield { transfer, destination_payment_refund: 'pyr_affiliate' };
    },
  },
  payouts: {
    list: async function* (_, request) {
      assert.equal(request.stripeAccount, accounts[0].id);
      yield {
        id: 'po_affiliate',
        amount: 300,
        currency: 'usd',
        status: 'paid',
        automatic: true,
        reconciliation_status: 'completed',
        arrival_date: now + 86400,
      };
    },
  },
  balanceTransactions: {
    list: async function* ({ payout }, request) {
      assert.equal(request.stripeAccount, accounts[0].id);
      assert.equal(payout, 'po_affiliate');
      yield {
        id: 'txn_affiliate',
        source: 'py_affiliate',
        type: 'payment',
        currency: 'usd',
        amount: 310,
        fee: 0,
        net: 310,
      };
      yield {
        id: 'txn_reversed',
        source: 'pyr_affiliate',
        type: 'payment_refund',
        currency: 'usd',
        amount: -10,
        fee: 0,
        net: -10,
      };
    },
  },
};
async function sale(orderId, affiliate, amount) {
  await saveMerchantRecord(commerce, 'campaign-order-' + orderId, {
    campaignId: 'summer',
    variant: 'a',
    visitor: orderId,
    affiliate,
    commissionBps: 1000,
  });
  await saveMerchantRecord(
    commerce,
    'sale-' + orderId,
    {
      type: 'sale',
      orderId,
      total: amount,
      currency: 'USD',
      items: [{ unitPrice: amount, quantity: 1 }],
    },
    'merchant-events',
  );
}
async function statement(affiliate) {
  return portalReport(commerce, env, { role: 'affiliate', subject: affiliate });
}
try {
  await registerAffiliate(commerce, 'ada', 1000, true);
  await registerAffiliate(commerce, 'bea', 1000, true);
  function previewCommand(args) {
    return JSON.parse(
      execFileSync(
        process.execPath,
        [fileURLToPath(new URL('./merchant-operations.js', import.meta.url)), ...args],
        {
          cwd: dir,
          encoding: 'utf8',
          env: {
            ...process.env,
            STATESET_DB_PATH: db,
            STATESET_CHECKOUT_PROVIDER: 'base',
            STRIPE_SECRET_KEY: '',
            STRIPE_WEBHOOK_SECRET: '',
          },
        },
      ),
    );
  }
  assert.equal(
    previewCommand(['onboard-affiliate', 'ada', '--country', 'US', '--email', 'ada@example.test'])
      .action,
    'create or resume Stripe Express onboarding',
  );
  await sale('first', 'ada', '10.00');
  await sale('second', 'bea', '12.50');
  const preview = await reserveAffiliateBatch(commerce, 'first-batch', false);
  assert.equal((await affiliateBalances(commerce)).batches.length, 0);
  await sale('third', 'ada', '6.00');
  await assert.rejects(
    reserveAffiliateBatch(commerce, 'first-batch', true, 'external', {
      expected: preview.fingerprint,
    }),
    /changed/,
  );
  const raced = await Promise.all(
    ['batch-a', 'batch-b'].map((id) => reserveAffiliateBatch(commerce, id, true)),
  );
  const nonempty = raced.filter((batch) => batch.rows.length);
  assert.equal(nonempty.length, 1);
  const first = nonempty[0].batchId;
  assert.deepEqual(nonempty[0].rows.find((row) => row.affiliate === 'ada').lines, [
    { orderId: 'first', amount: '1' },
    { orderId: 'third', amount: '0.6' },
  ]);
  await confirmPayoutExport(commerce, first, 'BANK-BEA', true, { affiliate: 'bea', paidAt });
  await assert.rejects(confirmPayoutExport(commerce, first, 'BULK-CONFLICT', true), /different/);
  assert.equal((await statement('ada')).payouts[0].status, 'reserved');
  await confirmPayoutExport(commerce, first, 'BANK-ADA', true, { affiliate: 'ada', paidAt });
  const paid = await statement('ada');
  assert.equal(paid.payouts.length, 1);
  assert.equal(paid.payouts[0].reference, 'BANK-ADA');
  assert.equal(paid.payouts[0].paidAt, paidAt);
  assert.ok(!JSON.stringify(paid).includes('BANK-BEA'));
  const replay = await confirmPayoutExport(commerce, first, 'BANK-ADA', true, {
    affiliate: 'ada',
    paidAt,
  });
  assert.equal(replay.receipts[0].confirmedAt, paid.payouts[0].confirmedAt);
  await assert.rejects(
    confirmPayoutExport(commerce, first, 'CHANGED', true, { affiliate: 'ada' }),
    /different/,
  );
  await saveMerchantRecord(
    commerce,
    'refund-part',
    { type: 'refund', orderId: 'first', amount: '4' },
    'merchant-events',
  );
  await sale('later', 'ada', '35');
  await assert.rejects(reserveAffiliateBatch(commerce, 'connect', true, 'stripe'), /onboarding/);
  await onboardAffiliate(commerce, stripe, env, {
    affiliate: 'ada',
    email: 'ada@example.test',
    country: 'US',
    apply: true,
  });
  assert.equal(accounts[0].metadata.stateset_affiliate, 'ada');
  assert.equal(await readMerchantRecord(commerce, 'seller-ada'), null);
  await sale('separate', 'bea', '10');
  await assert.rejects(reserveAffiliateBatch(commerce, 'mixed', true, 'stripe'), /onboarding/);
  const batch = await reserveAffiliateBatch(commerce, 'connect', true, 'stripe', {
    affiliate: 'ada',
  });
  const separate = await reserveAffiliateBatch(commerce, 'separate-payment', true, 'external', {
    affiliate: 'bea',
  });
  assert.equal(separate.rows[0].amount, '1');
  await assert.rejects(
    reserveAffiliateBatch(commerce, 'connect', true, 'stripe', { affiliate: 'bea' }),
    /recipient selection/,
  );
  assert.equal(batch.rows[0].amount, '3.1');
  assert.equal(previewCommand(['settle-affiliate', 'ada', '--batch', 'connect']).mode, 'preview');
  assert.deepEqual(batch.rows[0].lines, [
    { orderId: 'first', amount: '-0.4' },
    { orderId: 'later', amount: '3.5' },
  ]);
  await assert.rejects(
    confirmPayoutExport(commerce, 'connect', 'wrong', true),
    /different payment method/,
  );
  assert.equal((await settleAffiliateTransfer(commerce, stripe, 'connect', 'ada')).mode, 'preview');
  assert.equal(transfers.size, 0);
  await assert.rejects(
    settleAffiliateTransfer(commerce, stripe, 'connect', 'ada', true),
    /Insufficient balance/,
  );
  await assert.rejects(
    settleAffiliateTransfer(commerce, stripe, 'connect', 'ada', true),
    /No response/,
  );
  const realNow = Date.now;
  try {
    Date.now = () => realNow() + 25 * 3600000;
    await assert.rejects(
      settleAffiliateTransfer(commerce, stripe, 'connect', 'ada', true),
      /--retry-missing-attempt 1/,
    );
    await assert.rejects(
      settleAffiliateTransfer(commerce, stripe, 'connect', 'ada', true, 0),
      /--retry-missing-attempt 1/,
    );
    await assert.rejects(
      settleAffiliateTransfer(commerce, stripe, 'connect', 'ada', true, 1),
      /Connection lost/,
    );
  } finally {
    Date.now = realNow;
  }
  assert.equal(new Set(attempts).size, 3);
  await commerce.close();
  commerce = new Commerce(db);
  const remote = transfers.get('tr_commission');
  remote.destination = 'acct_wrong';
  await assert.rejects(
    settleAffiliateTransfer(commerce, stripe, 'connect', 'ada', true),
    /differs/,
  );
  remote.destination = accounts[0].id;
  await settleAffiliateTransfer(commerce, stripe, 'connect', 'ada', true);
  assert.equal((await statement('ada')).balance.paid, '4.7');
  remote.amount_reversed = 10;
  await settleAffiliateTransfer(commerce, stripe, 'connect', 'ada', true);
  await affiliateBankPayouts(commerce, stripe, 'ada', true);
  await affiliateBankPayouts(commerce, stripe, 'ada', true);
  const report = await statement('ada'),
    connected = report.payouts.find((p) => p.batch === 'connect');
  assert.equal(transfers.size, 1);
  assert.equal(connected.status, 'partially reversed');
  assert.equal(connected.settledAmount, '3');
  assert.equal(report.balance.reserved, '0');
  assert.equal(report.balance.available, '0.1');
  assert.deepEqual(connected.bankPayouts.map((p) => [p.id, p.status, p.amount]).sort(), [
    ['po_affiliate', 'paid', '-0.1'],
    ['po_affiliate', 'paid', '3.1'],
  ]);
  assert.ok(!(await statement('bea')).payouts.some((p) => p.batch === 'connect'));
  await sale('legacy', 'bea', '25');
  const count = (await affiliateBalances(commerce)).batches.length;
  await saveMerchantRecord(
    commerce,
    'affiliate-export-' + count,
    { batchId: 'historical', rows: [{ affiliate: 'bea', currency: 'USD', amount: '2' }] },
    'affiliate-batches',
  );
  await saveMerchantRecord(commerce, 'affiliate-paid-historical', {
    batchId: 'historical',
    reference: 'BANK-OLD',
  });
  const legacy = (await statement('bea')).payouts.find((p) => p.batch === 'historical');
  assert.equal(legacy.lines, null);
  assert.equal(legacy.paidAt, null);
  assert.equal(legacy.status, 'paid');
  const remaining = await reserveAffiliateBatch(commerce, 'after-history', true);
  assert.deepEqual(remaining.rows.find((row) => row.affiliate === 'ada').lines, [
    { orderId: null, transferId: 'tr_commission', batchId: 'connect', amount: '0.1' },
  ]);
  const row = remaining.rows.find((r) => r.affiliate === 'bea');
  assert.equal(row.amount, '0.5');
  assert.deepEqual(row.lines, [
    { orderId: null, amount: '-2' },
    { orderId: 'legacy', amount: '2.5' },
  ]);
  assert.equal((await reserveAffiliateBatch(commerce, 'last', false)).rows.length, 0);
  console.log(
    JSON.stringify({
      result: 'passed',
      externalProvidersVerified: false,
      checks: [
        'order and refund allocation across raced batches',
        'individual bank receipts and affiliate isolation',
        'insufficient-funds retry and accepted transfer recovered after restart',
        'destination validation and reversal reconciliation',
        'bank payout traced to original commission orders',
        'historical aggregate carry',
      ],
    }),
  );
} finally {
  await commerce.close();
  rmSync(dir, { recursive: true, force: true });
}
