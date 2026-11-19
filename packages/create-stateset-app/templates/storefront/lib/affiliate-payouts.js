import { createHash, randomUUID } from 'node:crypto';
import { campaignAccounting } from './campaign-accounting.js';
import { decimalToUnits, unitsToDecimal } from './money.js';
import {
  firstMerchantRecord,
  merchantRecords,
  readMerchantRecord,
  saveMerchantRecord,
} from './merchant-records.js';

const slug = /^[a-z0-9][a-z0-9_-]{0,47}$/;
const cents = (n) => decimalToUnits(n, 2);
const format = (n) => (n < 0n ? '-' : '') + unitsToDecimal(n < 0n ? -n : n, 2);
const signed = (n) => (n.startsWith('-') ? -cents(n.slice(1)) : cents(n));
const receiptKey = (batch, affiliate) =>
  'affiliate-receipt-' +
  createHash('sha256')
    .update(JSON.stringify([batch, affiliate]))
    .digest('hex');

export async function affiliateBalances(commerce) {
  const { commissions } = await campaignAccounting(commerce);
  const accounts = new Map();
  function account(affiliate) {
    if (!accounts.has(affiliate))
      accounts.set(affiliate, { earned: 0n, exported: 0n, paid: 0n, pending: new Map() });
    return accounts.get(affiliate);
  }
  function adjust(acct, { orderId, transferId, batchId }, amount) {
    const key = transferId ? 'transfer/' + transferId : orderId ? 'order/' + orderId : 'carry';
    const entry = acct.pending.get(key) ?? {
      orderId,
      ...(transferId && { transferId, batchId }),
      units: 0n,
    };
    entry.units += amount;
    acct.pending.set(key, entry);
  }
  for (const commission of commissions) {
    const acct = account(commission.affiliate),
      amount = cents(commission.earned);
    acct.earned += amount;
    adjust(acct, commission, amount);
  }
  const batches = [],
    payouts = [],
    destinations = new Map();
  for await (const batch of merchantRecords(commerce, 'affiliate-batches')) {
    batches.push(batch);
    const legacy = await readMerchantRecord(commerce, 'affiliate-paid-' + batch.batchId);
    for (const row of batch.rows) {
      const acct = account(row.affiliate),
        amount = cents(row.amount);
      const receipt =
        (await readMerchantRecord(commerce, receiptKey(batch.batchId, row.affiliate))) ?? legacy;
      let reversed = 0n;
      const sources = [];
      if (receipt?.transferId) {
        if (receipt.destinationPayment) sources.push(receipt.destinationPayment);
        for await (const reversal of merchantRecords(
          commerce,
          'affiliate-bank-sources-' + receipt.transferId,
        ))
          sources.push(reversal.source);
        for await (const observation of merchantRecords(
          commerce,
          'affiliate-reversals-' + receipt.transferId,
        )) {
          const units = cents(observation.amount);
          if (units > reversed) reversed = units;
        }
      }
      if (reversed > amount) throw new Error('Reversed commission exceeds its transfer');
      acct.exported += amount - reversed;
      if (receipt) acct.paid += amount - reversed;
      if (reversed)
        adjust(
          acct,
          { orderId: null, transferId: receipt.transferId, batchId: batch.batchId },
          reversed,
        );
      // Old exports only recorded an aggregate. Keep that carry explicit in the first new statement.
      for (const line of row.lines ?? [{ orderId: null, amount: row.amount }])
        adjust(acct, line, -signed(line.amount));
      const payout = {
        batch: batch.batchId,
        affiliate: row.affiliate,
        currency: row.currency,
        amount: row.amount,
        rail: batch.rail || 'external',
        reservedAt: batch.createdAt ?? null,
        status: !receipt
          ? 'reserved'
          : reversed > 0n
            ? reversed === amount
              ? 'reversed'
              : 'partially reversed'
            : receipt.transferId
              ? 'transferred'
              : 'paid',
        settledAmount: receipt ? format(amount - reversed) : '0',
        reversedAmount: format(reversed),
        reference: receipt?.reference ?? null,
        paidAt: receipt?.paidAt ?? null,
        confirmedAt: receipt?.confirmedAt ?? null,
        lines: row.lines ?? null,
        bankPayouts: [],
      };
      payouts.push(payout);
      if (sources.length) {
        if (!destinations.has(row.affiliate))
          destinations.set(row.affiliate, { account: receipt.destination, payments: new Map() });
        for (const source of sources) destinations.get(row.affiliate).payments.set(source, payout);
      }
    }
  }
  for (const [affiliate, { account, payments }] of destinations) {
    const latest = new Map(),
      statuses = ['pending', 'in_transit', 'paid', 'failed', 'canceled'];
    for await (const bank of merchantRecords(commerce, 'affiliate-bank-' + affiliate)) {
      if (
        !latest.has(bank.id) ||
        statuses.indexOf(bank.status) > statuses.indexOf(latest.get(bank.id).status)
      )
        latest.set(bank.id, bank);
    }
    for (const bank of latest.values()) {
      const key = createHash('sha256')
        .update(account + ':' + bank.id)
        .digest('hex');
      const sheet = await readMerchantRecord(commerce, 'bank-lines-' + key);
      for (const line of sheet?.lines || []) {
        const payout = payments.get(line.source);
        if (payout)
          payout.bankPayouts.push({
            id: bank.id,
            status: bank.status,
            arrivalDate: new Date(bank.arrival * 1000).toISOString().slice(0, 10),
            currency: bank.currency.toUpperCase(),
            amount: format(BigInt(line.netMinor)),
            transactionId: line.id,
          });
      }
    }
  }
  const balances = [],
    availableRows = [];
  for (const [affiliate, a] of [...accounts].sort(([a], [b]) => a.localeCompare(b))) {
    const available = a.earned - a.exported;
    balances.push({
      affiliate,
      currency: 'USD',
      earned: format(a.earned),
      paid: format(a.paid),
      reserved: format(a.exported - a.paid),
      available: format(available),
    });
    if (available <= 0n) continue;
    const lines = [...a.pending]
      .filter(([, entry]) => entry.units !== 0n)
      .sort(([a], [b]) => a.localeCompare(b))
      .map(([, { units, ...source }]) => ({ ...source, amount: format(units) }));
    if (lines.reduce((sum, line) => sum + signed(line.amount), 0n) !== available)
      throw new Error('Commission detail differs from the available balance');
    availableRows.push({ affiliate, currency: 'USD', amount: format(available), lines });
  }
  return { balances, batches, payouts, availableRows, commissions };
}

export async function reserveAffiliateBatch(
  commerce,
  batchId,
  apply = false,
  rail = 'external',
  { expected, affiliate } = {},
) {
  if (
    typeof batchId !== 'string' ||
    !slug.test(batchId) ||
    !['external', 'stripe'].includes(rail) ||
    (affiliate !== undefined && (typeof affiliate !== 'string' || !slug.test(affiliate)))
  )
    throw new Error('Provide a payout batch ID and an external or stripe payment method');
  function checkBatch(batch) {
    if ((batch.rail || 'external') !== rail)
      throw new Error('This payout batch uses a different payment method');
    if (affiliate !== undefined && batch.affiliate !== affiliate)
      throw new Error('This payout batch has a different recipient selection');
    if (expected !== undefined && batch.fingerprint !== expected)
      throw new Error('Commissions changed. Preview the batch again.');
    return batch;
  }
  for (;;) {
    const { batches, availableRows } = await affiliateBalances(commerce);
    const prior = batches.find((batch) => batch.batchId === batchId);
    if (prior) return { ...checkBatch(prior), alreadyReserved: true };
    const rows = [];
    for (const row of availableRows) {
      if (affiliate !== undefined && row.affiliate !== affiliate) continue;
      if (rail === 'external') {
        rows.push(row);
        continue;
      }
      const binding = await readMerchantRecord(commerce, 'affiliate-connect-' + row.affiliate);
      if (!binding) throw new Error('Complete Stripe onboarding for affiliate ' + row.affiliate);
      rows.push({
        ...row,
        destination: binding.account,
        ...(apply && { operation: randomUUID() }),
      });
    }
    const fingerprint = createHash('sha256')
      .update(
        JSON.stringify([
          batchId,
          rail,
          affiliate ?? null,
          rows.map(({ operation, ...terms }) => terms),
        ]),
      )
      .digest('hex');
    if (expected !== undefined && expected !== fingerprint)
      throw new Error('Commissions changed. Preview the batch again.');
    const batch = {
      batchId,
      rail,
      ...(affiliate && { affiliate }),
      rows,
      fingerprint,
      ...(apply && { createdAt: new Date().toISOString() }),
    };
    if (!apply) return batch;
    const winner = await firstMerchantRecord(
      commerce,
      'affiliate-export-' + batches.length,
      batch,
      'affiliate-batches',
    );
    if (winner.batchId !== batchId) continue;
    return checkBatch(winner);
  }
}

export async function payoutExport(
  commerce,
  batchId,
  apply,
  rail = 'external',
  affiliate = undefined,
) {
  const batch = await reserveAffiliateBatch(commerce, batchId, apply, rail, { affiliate });
  return [
    'batch,affiliate,currency,amount',
    ...batch.rows.map((r) => [batchId, r.affiliate, r.currency, r.amount].join(',')),
  ].join('\r\n');
}

async function existingBatch(commerce, batchId, rail) {
  if (typeof batchId !== 'string' || !slug.test(batchId)) throw new Error('Invalid payout batch');
  for await (const batch of merchantRecords(commerce, 'affiliate-batches')) {
    if (batch.batchId !== batchId) continue;
    if ((batch.rail || 'external') !== rail)
      throw new Error('This payout batch uses a different payment method');
    return batch;
  }
  throw new Error('Unknown exported payout batch');
}

export async function confirmPayoutExport(
  commerce,
  batchId,
  reference,
  apply,
  { affiliate, paidAt } = {},
) {
  if (
    typeof reference !== 'string' ||
    !reference.trim() ||
    reference.length > 160 ||
    /[\x00-\x1f\x7f]/u.test(reference)
  )
    throw new Error('Provide the bank or payment reference');
  reference = reference.trim();
  if (
    paidAt !== undefined &&
    (typeof paidAt !== 'string' ||
      !/^\d{4}-\d{2}-\d{2}T.*(?:Z|[+-]\d\d:\d\d)$/.test(paidAt) ||
      !Number.isFinite(Date.parse(paidAt)) ||
      Date.parse(paidAt) > Date.now())
  )
    throw new Error('Supply a past payment timestamp with a timezone');
  const batch = await existingBatch(commerce, batchId, 'external');
  const rows =
    affiliate === undefined ? batch.rows : batch.rows.filter((r) => r.affiliate === affiliate);
  if (affiliate !== undefined && !rows.length)
    throw new Error('This affiliate is not in the batch');
  const legacy = await readMerchantRecord(commerce, 'affiliate-paid-' + batchId);
  const records = [];
  const confirmedAt = new Date().toISOString();
  function checkReceipt(saved, data) {
    if (saved && (saved.reference !== reference || (paidAt && saved.paidAt !== data.paidAt)))
      throw new Error('A different payment confirmation already exists for ' + data.affiliate);
  }
  for (const row of rows) {
    const key = receiptKey(batchId, row.affiliate);
    const data = {
      batchId,
      affiliate: row.affiliate,
      amount: row.amount,
      currency: row.currency,
      reference,
      ...(paidAt && { paidAt: new Date(paidAt).toISOString() }),
      confirmedAt,
    };
    const prior = (await readMerchantRecord(commerce, key)) ?? legacy;
    checkReceipt(prior, data);
    records.push({ key, data, prior });
  }
  const receipts = [];
  for (const { key, data, prior } of records) {
    const saved = prior ?? (apply ? await firstMerchantRecord(commerce, key, data) : data);
    checkReceipt(saved, data);
    receipts.push(saved);
  }
  return { batchId, reference, receipts, mode: apply ? 'confirmed' : 'preview' };
}

export async function settleAffiliateTransfer(
  commerce,
  stripe,
  batchId,
  affiliate,
  apply = false,
  retryMissingAttempt = undefined,
) {
  const batch = await existingBatch(commerce, batchId, 'stripe');
  const row = batch.rows.find((r) => r.affiliate === affiliate);
  if (!row?.operation || !row.destination)
    throw new Error('This affiliate is not in the Stripe batch');
  const units = cents(row.amount);
  if (units <= 0n || units > BigInt(Number.MAX_SAFE_INTEGER))
    throw new Error('Invalid transfer amount');
  const key = receiptKey(batchId, affiliate),
    group = 'stateset_aff_' + row.operation;
  const receipt = await readMerchantRecord(commerce, key);
  const action = {
    batchId,
    affiliate,
    amount: row.amount,
    currency: row.currency,
    destination: row.destination,
  };
  if (!apply) return { ...action, mode: 'preview', reference: receipt?.reference ?? null };
  let transfer;
  if (receipt) transfer = await stripe.transfers.retrieve(receipt.transferId);
  else {
    for await (const item of stripe.transfers.list({ transfer_group: group, limit: 100 })) {
      if (transfer)
        throw new Error('Multiple transfers match the commission. Review the provider records.');
      transfer = item;
    }
    if (!transfer) {
      const account = await stripe.accounts.retrieve(row.destination);
      if (
        account.id !== row.destination ||
        account.capabilities?.transfers !== 'active' ||
        !account.payouts_enabled
      )
        throw new Error('Affiliate payout account is awaiting verification');
      let index = 0;
      while (await readMerchantRecord(commerce, key + '-declined-' + index)) index++;
      const attempt = await firstMerchantRecord(commerce, key + '-attempt-' + index, {
        at: Date.now(),
      });
      if (Date.now() - attempt.at >= 86400000) {
        if (retryMissingAttempt !== index)
          throw new Error(
            'Stripe returned no transfer for group ' +
              group +
              '. After checking this group, retry with --retry-missing-attempt ' +
              index +
              '.',
          );
        await firstMerchantRecord(commerce, key + '-declined-' + index, {
          code: 'operator_confirmed_missing',
          at: new Date().toISOString(),
        });
        return settleAffiliateTransfer(commerce, stripe, batchId, affiliate, true);
      }
      try {
        transfer = await stripe.transfers.create(
          {
            amount: Number(units),
            currency: 'usd',
            destination: row.destination,
            transfer_group: group,
            metadata: {
              stateset_affiliate: affiliate,
              stateset_batch: batchId,
              stateset_operation: row.operation,
            },
          },
          { idempotencyKey: group + '-' + index },
        );
      } catch (error) {
        // Insufficient available funds definitively refused this attempt. A timeout is ambiguous.
        if (error?.code === 'balance_insufficient')
          await saveMerchantRecord(commerce, key + '-declined-' + index, { code: error.code });
        throw error;
      }
    }
  }
  if (
    !transfer?.id ||
    (receipt && transfer.id !== receipt.transferId) ||
    transfer.transfer_group !== group ||
    transfer.metadata?.stateset_operation !== row.operation ||
    transfer.metadata?.stateset_affiliate !== affiliate ||
    transfer.metadata?.stateset_batch !== batchId ||
    transfer.destination !== row.destination ||
    transfer.currency !== 'usd' ||
    transfer.amount !== Number(units) ||
    !Number.isSafeInteger(transfer.created) ||
    transfer.created <= 0 ||
    !Number.isSafeInteger(transfer.amount_reversed) ||
    transfer.amount_reversed < 0 ||
    transfer.amount_reversed > transfer.amount
  )
    throw new Error('Provider transfer differs from the reserved commission');
  const evidence = {
    batchId,
    affiliate,
    amount: row.amount,
    currency: row.currency,
    reference: transfer.id,
    transferId: transfer.id,
    paidAt: new Date(transfer.created * 1000).toISOString(),
    destination: row.destination,
    destinationPayment:
      typeof transfer.destination_payment === 'string'
        ? transfer.destination_payment
        : (transfer.destination_payment?.id ?? null),
  };
  if (!receipt) await saveMerchantRecord(commerce, key, evidence);
  if (evidence.destinationPayment)
    await saveMerchantRecord(
      commerce,
      'affiliate-source-' + evidence.destinationPayment,
      { source: evidence.destinationPayment },
      'affiliate-bank-sources-' + transfer.id,
    );
  if (transfer.amount_reversed)
    await saveMerchantRecord(
      commerce,
      key + '-rev-' + transfer.amount_reversed,
      { amount: format(BigInt(transfer.amount_reversed)) },
      'affiliate-reversals-' + transfer.id,
    );
  if (transfer.amount_reversed) {
    for await (const reversal of stripe.transfers.listReversals(transfer.id, { limit: 100 })) {
      const source =
        typeof reversal.destination_payment_refund === 'string'
          ? reversal.destination_payment_refund
          : reversal.destination_payment_refund?.id;
      if (source && reversal.transfer === transfer.id)
        await saveMerchantRecord(
          commerce,
          'affiliate-source-' + source,
          { source },
          'affiliate-bank-sources-' + transfer.id,
        );
    }
  }
  return {
    ...action,
    mode: 'reconciled',
    reference: transfer.id,
    paidAt: evidence.paidAt,
    settledAmount: format(units - BigInt(transfer.amount_reversed)),
  };
}
