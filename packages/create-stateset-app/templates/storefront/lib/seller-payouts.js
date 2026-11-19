import { createHash } from 'node:crypto';
import { decimalToUnits, unitsToDecimal } from './money.js';
import { firstMembershipPayment } from './membership-checkout.js';
import { settledInvoiceIntent } from './stripe-settlement.js';
import { renewalSellerTerms } from './renewal-payments.js';
import {
  firstMerchantRecord,
  merchantRecords,
  readMerchantRecord,
  saveMerchantRecord,
} from './merchant-records.js';

const hash = (text) => createHash('sha256').update(text).digest('hex');
const sellerCode = /^[a-z0-9][a-z0-9_-]{0,47}$/;
const cents = (amount) => decimalToUnits(amount, 2);
const format = (amount) => unitsToDecimal(amount, 2);
const signedAmount = (units) => (units < 0n ? '-' + format(-units) : format(units));
function sourceId(key) {
  const digest = hash(key);
  return [
    digest.slice(0, 8),
    digest.slice(8, 12),
    '5' + digest.slice(13, 16),
    '8' + digest.slice(17, 20),
    digest.slice(20, 32),
  ].join('-');
}

export function onboardSeller(commerce, stripe, env, options) {
  return onboardPayoutAccount(commerce, stripe, env, 'seller', options.seller, options);
}

export function onboardAffiliate(commerce, stripe, env, options) {
  return onboardPayoutAccount(commerce, stripe, env, 'affiliate', options.affiliate, options);
}

async function onboardPayoutAccount(
  commerce,
  stripe,
  env,
  role,
  code,
  { country, email, ownerWallet, apply = false },
) {
  if (!sellerCode.test(code || '') || !/^[A-Z]{2}$/.test(country || '') || !email?.includes('@'))
    throw new Error('Supply a partner code, country and email');
  if (role === 'affiliate' && !(await readMerchantRecord(commerce, 'affiliate-' + code)))
    throw new Error('Register the affiliate before setting up payouts');
  if (!apply)
    return { [role]: code, country, action: 'create or resume Stripe Express onboarding' };
  const affiliate = role === 'affiliate';
  const key = (affiliate ? 'affiliate-connect-' : 'seller-') + code;
  let binding = await readMerchantRecord(commerce, key);
  if (!binding) {
    const intent = await firstMerchantRecord(
      commerce,
      (affiliate ? 'affiliate-onboard-' : 'onboard-') + code,
      {
        requestId: crypto.randomUUID(),
        country,
        email,
        ...(ownerWallet && { ownerWallet }),
      },
    );
    if (
      intent.country !== country ||
      intent.email !== email ||
      (ownerWallet && intent.ownerWallet !== ownerWallet)
    )
      throw new Error('Onboarding terms differ from the original request');
    let account;
    for await (const candidate of stripe.accounts.list({ limit: 100 })) {
      if (candidate.metadata?.stateset_onboarding_id === intent.requestId) {
        account = candidate;
        break;
      }
    }
    account ||= await stripe.accounts.create(
      {
        type: 'express',
        country,
        email,
        capabilities: { transfers: { requested: true } },
        metadata: { stateset_onboarding_id: intent.requestId, ['stateset_' + role]: code },
      },
      { idempotencyKey: intent.requestId },
    );
    binding = await saveMerchantRecord(
      commerce,
      key,
      {
        [role]: code,
        account: account.id,
        country,
        email,
        ...(intent.ownerWallet && { ownerWallet: intent.ownerWallet }),
      },
      affiliate ? 'affiliate-connect-accounts' : 'sellers',
    );
  }
  if (
    binding.country !== country ||
    binding.email !== email ||
    (ownerWallet && binding.ownerWallet !== ownerWallet)
  )
    throw new Error('Seller identity conflicts with existing connected account');
  const link = await stripe.accountLinks.create({
    account: binding.account,
    type: 'account_onboarding',
    return_url: new URL('/seller-onboarding?state=returned', env.NEXT_PUBLIC_BASE_URL).href,
    refresh_url: new URL('/seller-onboarding?state=expired', env.NEXT_PUBLIC_BASE_URL).href,
  });
  return {
    [role]: code,
    account: binding.account,
    onboardingUrl: link.url,
    expiresAt: link.expires_at,
  };
}

// Freeze SKU ownership and fees in the checkout quote before taking payment.
export async function sellerAllocations(commerce, env, lines) {
  const configured = JSON.parse(env.STATESET_SELLER_SKUS_JSON || '{}');
  const grouped = new Map();
  for (const item of lines) {
    if (!Object.hasOwn(configured, item.sku)) continue;
    const { seller, feeBps } = configured[item.sku];
    if (
      !sellerCode.test(seller || '') ||
      !Number.isSafeInteger(feeBps) ||
      feeBps < 0 ||
      feeBps > 10000
    )
      throw new Error('Invalid seller SKU allocation');
    const binding = await readMerchantRecord(commerce, 'seller-' + seller);
    if (!binding) throw new Error('Onboard the configured seller before accepting this order');
    const gross = cents(item.unitPrice) * BigInt(item.quantity) - cents(item.discount || '0');
    const row = grouped.get(seller) || { seller, account: binding.account, gross: 0n, fee: 0n };
    row.gross += gross;
    row.fee += (gross * BigInt(feeBps)) / 10000n;
    grouped.set(seller, row);
  }
  return [...grouped.values()]
    .sort((a, b) => a.seller.localeCompare(b.seller))
    .map((row) => ({
      ...row,
      gross: format(row.gross),
      fee: format(row.fee),
      payable: format(row.gross - row.fee),
    }));
}

export async function sellerStatement(commerce) {
  const sales = [],
    refunds = new Map();
  for await (const event of merchantRecords(commerce, 'merchant-events')) {
    if (event.type === 'sale') sales.push(event);
    if (event.type === 'refund') {
      if (!refunds.has(event.orderId)) refunds.set(event.orderId, []);
      refunds.get(event.orderId).push(event);
    }
  }
  for await (const receipt of merchantRecords(commerce, 'renewal-receipts')) {
    const terms = await renewalSellerTerms(commerce, receipt.subscriptionId);
    if (!terms.allocations.length) continue;
    const payment = await commerce.payments.get(receipt.paymentId);
    if (!payment || payment.idempotencyKey !== 'stripe:' + receipt.intentId)
      throw new Error('Renewal allocation is missing its settled payment');
    const returns = (await commerce.payments.getRefunds(payment.id)).filter(
      (r) => r.status === 'completed',
    );
    refunds.set(
      receipt.invoiceId,
      returns.map((r) => ({
        amount: r.amountExact,
        refundId: r.id,
        at: r.refundedAt || r.updatedAt,
      })),
    );
    sales.push({
      orderId: terms.orderId,
      invoiceId: receipt.invoiceId,
      paymentId: payment.id,
      terms,
      at: receipt.paidAt || payment.createdAt,
      total: payment.amountExact,
      currency: payment.currency,
    });
  }
  const rows = [];
  for (const sale of sales) {
    const terms =
      sale.terms || (await readMerchantRecord(commerce, 'seller-order-' + sale.orderId));
    if (!terms) continue;
    const charged = cents(sale.total);
    const source = sale.invoiceId || sale.orderId;
    const returns = refunds.get(source) || [];
    for (const split of terms.allocations) {
      const gross = cents(split.gross),
        payable = cents(split.payable),
        fee = cents(split.fee);
      let deducted = 0n,
        feesReturned = 0n,
        refunded = 0n;
      const adjustments = [];
      for (const event of returns) {
        const amount = cents(event.amount);
        const net = (amount * payable) / charged,
          commission = (amount * fee) / charged;
        deducted += net;
        feesReturned += commission;
        refunded += amount;
        adjustments.push({
          id: event.refundId,
          at: event.at,
          net: format(net),
          fee: format(commission),
        });
      }
      if (refunded > charged) throw new Error('Refund total exceeds original payment');
      if (refunded === charged && (deducted !== payable || feesReturned !== fee)) {
        adjustments.push({
          id: 'full',
          at: returns
            .map((e) => e.at)
            .sort()
            .at(-1),
          net: format(payable - deducted),
          fee: format(fee - feesReturned),
        });
        deducted = payable;
        feesReturned = fee;
      }
      const allocationId = hash(source + ':' + split.seller);
      let transferred = 0n;
      for await (const movement of merchantRecords(commerce, 'seller-movements-' + allocationId)) {
        transferred +=
          movement.type === 'reversal' ? -cents(movement.amount) : cents(movement.amount);
      }
      rows.push({
        seller: split.seller,
        account: split.account,
        orderId: sale.orderId,
        ...(sale.invoiceId && { invoiceId: sale.invoiceId, paymentId: sale.paymentId }),
        at: sale.at,
        currency: sale.currency,
        gross: format(gross),
        fee: format(fee),
        originalPayable: format(payable),
        refundDeductions: format(deducted),
        returnedFees: format(feesReturned),
        payable: format(payable - deducted),
        transferred: signedAmount(transferred),
        outstanding: signedAmount(payable - deducted - transferred),
        adjustments,
      });
    }
  }
  return rows;
}

export async function resolveRenewalSeller(
  commerce,
  subscriptionId,
  { seller, bps, platformOwned, apply },
) {
  const terms = await renewalSellerTerms(commerce, subscriptionId);
  if (terms.status !== 'needs review')
    return { subscriptionId, status: terms.status, allocations: terms.allocations };
  if (
    !terms.item ||
    (platformOwned
      ? seller || bps !== undefined
      : !terms.original.some((row) => row.seller === seller) || !/^\d{1,5}$/.test(bps || ''))
  )
    throw new Error(
      'Choose a seller from the original purchase and an explicit fee in basis points, or select platform ownership',
    );
  const allocations = platformOwned
    ? []
    : await sellerAllocations(
        commerce,
        {
          STATESET_SELLER_SKUS_JSON: JSON.stringify({
            [terms.item.sku]: { seller, feeBps: Number(bps) },
          }),
        },
        [{ ...terms.item, discount: '0' }],
      );
  if (
    allocations.length &&
    !terms.original.some((row) => row.seller === seller && row.account === allocations[0].account)
  )
    throw new Error('Payout account differs from the original purchase');
  if (apply)
    await saveMerchantRecord(commerce, 'renewal-sellers-' + terms.orderId, { allocations });
  return { subscriptionId, mode: apply ? 'applied' : 'preview', allocations };
}

export async function sellerBalances(commerce, sellerCodeFilter) {
  const statements = await sellerStatement(commerce);
  const balances = [];
  for await (const seller of merchantRecords(commerce, 'sellers')) {
    if (sellerCodeFilter && seller.seller !== sellerCodeFilter) continue;
    const orders = statements.filter((row) => row.seller === seller.seller);
    const total = (field) =>
      signedAmount(
        orders.reduce((sum, row) => {
          const value = row[field];
          return sum + (value.startsWith('-') ? -cents(value.slice(1)) : cents(value));
        }, 0n),
      );
    const payouts = new Map();
    const progression = ['pending', 'in_transit', 'paid', 'failed', 'canceled'];
    for await (const entry of merchantRecords(commerce, 'seller-bank-' + seller.seller)) {
      const previous = payouts.get(entry.id);
      if (!previous || progression.indexOf(entry.status) > progression.indexOf(previous.status)) {
        const { key, ...payout } = entry;
        payouts.set(entry.id, payout);
      }
    }
    for (const payout of payouts.values()) {
      const sheet = await readMerchantRecord(
        commerce,
        'bank-lines-' + hash(seller.account + ':' + payout.id),
      );
      if (!sheet) continue;
      const transactions = [];
      let net = 0n;
      for (const line of sheet.lines) {
        const source =
          line.source &&
          (await readMerchantRecord(
            commerce,
            'seller-source-' + hash(seller.account + ':' + line.source),
          ));
        transactions.push({
          ...line,
          orderId: source?.orderId ?? null,
          ...(source?.invoiceId && { invoiceId: source.invoiceId }),
        });
        net += BigInt(line.netMinor);
      }
      payout.reconciliation = {
        transactions,
        netMinor: String(net),
        differenceMinor: String(BigInt(payout.amountMinor) - net),
      };
    }
    balances.push({
      seller: seller.seller,
      currency: 'USD',
      gross: total('gross'),
      fees: total('fee'),
      returnedFees: total('returnedFees'),
      refundDeductions: total('refundDeductions'),
      payable: total('payable'),
      transferred: total('transferred'),
      outstanding: total('outstanding'),
      orders,
      bankPayouts: [...payouts.values()].sort((a, b) => b.arrival - a.arrival),
    });
  }
  return balances;
}

async function journal(commerce, env, key, date, signedLines) {
  const entries = signedLines.filter(([, units]) => units !== 0n);
  if (!entries.length) return;
  const accounts = JSON.parse(env.STATESET_SELLER_GL_JSON || '{}');
  if (!commerce.generalLedger.supportsExternalEventIds?.())
    throw new Error('Install a native engine supporting external journal event IDs');
  const eventId = sourceId(key);
  const find = async () =>
    (
      await commerce.generalLedger.listJournalEntries({
        sourceDocumentType: 'merchant_event',
        sourceDocumentId: eventId,
      })
    ).find((entry) => entry.status.toLowerCase() === 'posted');
  if (await find()) return;
  const lines = entries.map(([account, units]) => {
    if (!accounts[account]) throw new Error('Configure ledger account ' + account);
    return {
      accountId: accounts[account],
      ...(units > 0n ? { debitAmount: format(units) } : { creditAmount: format(-units) }),
    };
  });
  const base = await commerce.currency.getBaseCurrency();
  const exchangeRate = accounts.usdExchangeRate || (base === 'USD' ? '1' : null);
  if (
    typeof exchangeRate !== 'string' ||
    !/^\d+(?:\.\d+)?$/.test(exchangeRate) ||
    !/[1-9]/.test(exchangeRate)
  ) {
    throw new Error('Configure a positive USD exchange rate for ledger currency ' + base);
  }
  try {
    await commerce.generalLedger.createJournalEntry(
      {
        externalEventId: eventId,
        entryDate: date.slice(0, 10),
        description: key,
        lines,
        autoPost: true,
        currency: 'USD',
        exchangeRate,
      },
      'storefront:seller-settlement',
    );
  } catch (error) {
    if (!(await find())) throw error;
  }
}

async function settleSale(commerce, stripe, env, row) {
  const id = hash((row.invoiceId || row.orderId) + ':' + row.seller);
  const sourceRecord = { orderId: row.orderId, ...(row.invoiceId && { invoiceId: row.invoiceId }) };
  await journal(commerce, env, 'seller-sale-' + id, row.at, [
    ['salesClearing', cents(row.gross)],
    ['sellerPayable', -cents(row.originalPayable)],
    ['platformFees', -cents(row.fee)],
  ]);
  for (const adjustment of row.adjustments) {
    const net = cents(adjustment.net),
      fee = cents(adjustment.fee);
    await journal(commerce, env, 'seller-refund-' + id + '-' + adjustment.id, adjustment.at, [
      ['sellerPayable', net],
      ['platformFees', fee],
      ['salesClearing', -net - fee],
    ]);
  }
  const desired = cents(row.payable);
  const existing = await readMerchantRecord(commerce, 'seller-transfer-' + id);
  if (!existing && desired === 0n && !(await readMerchantRecord(commerce, 'seller-intent-' + id)))
    return { state: 'no-payable' };
  let plan = await readMerchantRecord(commerce, 'seller-intent-' + id);
  let transfer;
  if (existing) transfer = await stripe.transfers.retrieve(existing.transfer);
  else {
    const group = 'stateset_' + id;
    for await (const candidate of stripe.transfers.list({ transfer_group: group, limit: 100 })) {
      if (candidate.metadata?.stateset_transfer === id) {
        transfer = candidate;
        break;
      }
    }
    if (!transfer) {
      const account = await stripe.accounts.retrieve(row.account);
      if (account.capabilities?.transfers !== 'active' || !account.payouts_enabled)
        return { state: 'onboarding-incomplete' };
      plan ||= await firstMerchantRecord(commerce, 'seller-intent-' + id, { amount: row.payable });
      const binding = await readMerchantRecord(commerce, 'stripe-session-' + row.orderId);
      let intent;
      if (row.invoiceId) {
        const receipt = await readMerchantRecord(commerce, 'renewal-receipt-' + row.invoiceId);
        const invoice = await stripe.invoices.retrieve(row.invoiceId);
        const payment = await commerce.payments.get(row.paymentId);
        intent = await settledInvoiceIntent(stripe, invoice);
        if (
          !receipt ||
          !payment ||
          !intent ||
          receipt.paymentId !== payment.id ||
          invoice.id !== receipt.invoiceId ||
          invoice.billing_reason !== 'subscription_cycle' ||
          invoice.status !== 'paid' ||
          invoice.amount_remaining !== 0 ||
          receipt.intentId !== intent.id ||
          payment.idempotencyKey !== 'stripe:' + intent.id ||
          cents(payment.amountExact) !== BigInt(intent.amount_received)
        )
          throw new Error('Renewal transfer differs from the settled invoice');
      } else if (binding) {
        const session = await stripe.checkout.sessions.retrieve(binding.sessionId, {
          expand: ['payment_intent'],
        });
        if (session.payment_status !== 'paid' || session.client_reference_id !== row.orderId)
          throw new Error('Checkout session differs from seller order');
        if (session.mode === 'subscription') {
          const order = await commerce.orders.get(row.orderId);
          const terms = await readMerchantRecord(commerce, 'stripe-membership-' + row.orderId);
          if (!order || !terms || BigInt(session.amount_total) !== cents(order.totalAmountExact))
            throw new Error('Membership invoice differs from seller order');
          ({ intent } = await firstMembershipPayment(stripe, session, terms, order));
        } else {
          intent = session.payment_intent;
          if (intent?.metadata?.order_id !== row.orderId)
            throw new Error('Checkout session differs from seller order');
        }
      } else {
        const authorization = await readMerchantRecord(
          commerce,
          'routed-authorization-' + row.orderId,
        );
        if (authorization?.provider !== 'stripe')
          throw new Error('Cannot transfer without platform charge evidence');
        intent = await stripe.paymentIntents.retrieve(authorization.reference);
        const paid = (await commerce.payments.list({ orderId: row.orderId })).find(
          (p) => p.idempotencyKey === 'routed:' + row.orderId,
        );
        if (
          paid?.orderId !== row.orderId ||
          cents(paid.amountExact) !== BigInt(intent.amount_received)
        )
          throw new Error('Seller charge differs from the routed payment');
      }
      if (intent?.status !== 'succeeded' || !intent.latest_charge || intent.currency !== 'usd')
        throw new Error('Seller transfer is missing a settled charge');
      transfer = await stripe.transfers.create(
        {
          amount: Number(cents(plan.amount)),
          currency: 'usd',
          destination: row.account,
          source_transaction: intent.latest_charge,
          transfer_group: group,
          metadata: {
            stateset_transfer: id,
            order_id: row.orderId,
            seller: row.seller,
            ...(row.invoiceId && { invoice_id: row.invoiceId }),
          },
        },
        { idempotencyKey: group },
      );
    }
  }
  if (
    !plan ||
    transfer.destination !== row.account ||
    transfer.currency !== 'usd' ||
    BigInt(transfer.amount) !== cents(plan.amount)
  ) {
    throw new Error('Provider transfer differs from seller allocation');
  }
  await saveMerchantRecord(commerce, 'seller-transfer-' + id, { transfer: transfer.id });
  const destinationPayment =
    typeof transfer.destination_payment === 'string'
      ? transfer.destination_payment
      : transfer.destination_payment?.id;
  if (destinationPayment)
    await saveMerchantRecord(
      commerce,
      'seller-source-' + hash(row.account + ':' + destinationPayment),
      sourceRecord,
    );
  const transferDate = new Date(transfer.created * 1000).toISOString();
  await journal(commerce, env, 'seller-transfer-' + id, transferDate, [
    ['sellerPayable', cents(plan.amount)],
    ['processorClearing', -cents(plan.amount)],
  ]);
  await saveMerchantRecord(
    commerce,
    'seller-movement-' + transfer.id,
    {
      type: 'transfer',
      amount: plan.amount,
      transfer: transfer.id,
    },
    'seller-movements-' + id,
  );
  const needed = cents(plan.amount) > desired ? cents(plan.amount) - desired : 0n;
  // All workers that observe the same provider balance must use the same reversal amount and key.
  if (BigInt(transfer.amount_reversed) < needed) {
    const from = transfer.amount_reversed;
    const key = 'seller-reverse-' + id + '-' + from;
    const step = await firstMerchantRecord(commerce, key, { target: String(needed) });
    await stripe.transfers.createReversal(
      transfer.id,
      {
        amount: Number(BigInt(step.target) - BigInt(from)),
        metadata: { stateset_step: sourceId(key) },
      },
      { idempotencyKey: key },
    );
    transfer = await stripe.transfers.retrieve(transfer.id);
  }
  if (transfer.amount_reversed) {
    for await (const reversal of stripe.transfers.listReversals(transfer.id, { limit: 100 })) {
      const source =
        typeof reversal.destination_payment_refund === 'string'
          ? reversal.destination_payment_refund
          : reversal.destination_payment_refund?.id;
      if (source)
        await saveMerchantRecord(
          commerce,
          'seller-source-' + hash(row.account + ':' + source),
          sourceRecord,
        );
      const units = BigInt(reversal.amount);
      await journal(
        commerce,
        env,
        'seller-reversal-' + reversal.id,
        new Date(reversal.created * 1000).toISOString(),
        [
          ['processorClearing', units],
          ['sellerPayable', -units],
        ],
      );
      await saveMerchantRecord(
        commerce,
        'seller-movement-' + reversal.id,
        {
          type: 'reversal',
          amount: format(units),
          transfer: transfer.id,
          reversal: reversal.id,
        },
        'seller-movements-' + id,
      );
    }
  }
  return {
    state: BigInt(transfer.amount_reversed) < needed ? 'reversal-pending' : 'reconciled',
    transfer: transfer.id,
    transferred: format(BigInt(transfer.amount) - BigInt(transfer.amount_reversed)),
  };
}

export async function settleSellers(commerce, stripe, env, apply) {
  const rows = await sellerStatement(commerce);
  const results = [];
  for (const row of rows) {
    try {
      results.push({
        ...row,
        ...(apply ? await settleSale(commerce, stripe, env, row) : { state: 'preview' }),
      });
    } catch (error) {
      results.push({ ...row, state: 'failed', error: error.message });
    }
  }
  return results;
}

export async function sellerBankPayouts(commerce, stripe, seller, apply) {
  return syncBankPayouts(commerce, stripe, 'seller', seller, apply);
}

export async function affiliateBankPayouts(commerce, stripe, affiliate, apply) {
  return syncBankPayouts(commerce, stripe, 'affiliate', affiliate, apply);
}

async function syncBankPayouts(commerce, stripe, role, code, apply) {
  const binding = await readMerchantRecord(
    commerce,
    (role === 'seller' ? 'seller-' : 'affiliate-connect-') + code,
  );
  if (!binding) throw new Error('Unknown payout account');
  const rows = [];
  for await (const p of stripe.payouts.list({ limit: 100 }, { stripeAccount: binding.account })) {
    const row = {
      [role]: code,
      id: p.id,
      currency: p.currency,
      amountMinor: String(p.amount),
      status: p.status,
      arrival: p.arrival_date,
    };
    if (apply)
      await firstMerchantRecord(
        commerce,
        (role === 'seller' ? 'bank-' : 'affiliate-bank-status-') +
          hash([binding.account, p.id, p.status].join(':')),
        row,
        role + '-bank-' + code,
      );
    const key = 'bank-lines-' + hash(binding.account + ':' + p.id);
    if (
      apply &&
      p.automatic &&
      p.reconciliation_status === 'completed' &&
      !(await readMerchantRecord(commerce, key))
    ) {
      const lines = [];
      for await (const t of stripe.balanceTransactions.list(
        { payout: p.id, limit: 100 },
        { stripeAccount: binding.account },
      )) {
        const source = typeof t.source === 'string' ? t.source : (t.source?.id ?? null);
        if (source === p.id) continue;
        if (
          t.currency !== p.currency ||
          ![t.amount, t.fee, t.net].every(Number.isSafeInteger) ||
          t.net !== t.amount - t.fee
        ) {
          throw new Error('Invalid bank payout transaction amounts');
        }
        lines.push({
          id: t.id,
          source,
          type: t.type,
          amountMinor: String(t.amount),
          feeMinor: String(t.fee),
          netMinor: String(t.net),
        });
      }
      await firstMerchantRecord(commerce, key, { lines });
    }
    rows.push(row);
  }
  return rows;
}
