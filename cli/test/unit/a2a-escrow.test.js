/**
 * Tests for cli/src/a2a/escrow.js
 *
 * Covers: createEscrow, fundEscrow, releaseEscrow, refundEscrow, disputeEscrow,
 * checkConditions, confirmCondition, checkExpired, getEscrow, listEscrows.
 *
 * State machine: created -> funded -> active -> released/refunded/disputed/expired
 */

import { describe, it, beforeEach } from 'node:test';
import assert from 'node:assert/strict';

import { createEscrowService } from '../../src/a2a/escrow.js';

// ---------------------------------------------------------------------------
// Mock store factory
// ---------------------------------------------------------------------------

function createMockStore() {
  const escrows = new Map();
  const quotes = new Map();
  return {
    createEscrow: async (record) => {
      escrows.set(record.id, { ...record });
    },
    getEscrow: async (id) => escrows.get(id) || null,
    updateEscrow: async (id, updates) => {
      const existing = escrows.get(id);
      if (existing) escrows.set(id, { ...existing, ...updates });
    },
    listEscrows: async (filter) => {
      let results = [...escrows.values()];
      if (filter?.status) results = results.filter((e) => e.status === filter.status);
      if (filter?.buyer_address)
        results = results.filter((e) => e.buyer_address === filter.buyer_address);
      return results;
    },
    getQuote: async (id) => quotes.get(id) || null,
    _escrows: escrows,
    _quotes: quotes,
  };
}

// ---------------------------------------------------------------------------
// Helper: create a funded/active escrow in the store for transition tests
// ---------------------------------------------------------------------------

async function createActiveEscrow(service, overrides = {}) {
  const result = await service.createEscrow({
    buyerAddress: '0xBuyer',
    sellerAddress: '0xSeller',
    amount: 100,
    ...overrides,
  });
  const funded = await service.fundEscrow(result.escrow.id);
  return funded.escrow;
}

// ---------------------------------------------------------------------------
// 1. createEscrow
// ---------------------------------------------------------------------------

describe('createEscrow', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEscrowService(store);
  });

  // --- Validation ---

  it('rejects missing buyerAddress', async () => {
    await assert.rejects(() => service.createEscrow({ sellerAddress: '0xSeller', amount: 100 }), {
      message: 'buyerAddress is required',
    });
  });

  it('rejects missing sellerAddress', async () => {
    await assert.rejects(() => service.createEscrow({ buyerAddress: '0xBuyer', amount: 100 }), {
      message: 'sellerAddress is required',
    });
  });

  it('rejects missing amount (undefined)', async () => {
    await assert.rejects(
      () => service.createEscrow({ buyerAddress: '0xBuyer', sellerAddress: '0xSeller' }),
      { message: 'amount is required' },
    );
  });

  it('rejects null amount', async () => {
    await assert.rejects(
      () =>
        service.createEscrow({
          buyerAddress: '0xBuyer',
          sellerAddress: '0xSeller',
          amount: null,
        }),
      { message: 'amount is required' },
    );
  });

  it('rejects zero amount', async () => {
    await assert.rejects(
      () =>
        service.createEscrow({
          buyerAddress: '0xBuyer',
          sellerAddress: '0xSeller',
          amount: 0,
        }),
      { message: 'amount must be positive' },
    );
  });

  it('rejects negative amount', async () => {
    await assert.rejects(
      () =>
        service.createEscrow({
          buyerAddress: '0xBuyer',
          sellerAddress: '0xSeller',
          amount: -50,
        }),
      { message: 'amount must be positive' },
    );
  });

  // --- Defaults ---

  it('creates escrow with default asset USDC', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    assert.equal(result.escrow.asset, 'USDC');
  });

  it('creates escrow with default network set_chain', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    assert.equal(result.escrow.network, 'set_chain');
  });

  it('derives BTC as the default asset for bitcoin escrows', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      network: 'bitcoin',
    });
    assert.equal(result.escrow.asset, 'BTC');
    assert.equal(result.escrow.network, 'bitcoin');
  });

  it('creates escrow with default 72h expiry', async () => {
    const before = new Date();
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    const expiresAt = new Date(result.escrow.expiresAt);
    // Should be roughly 72h from now (allow 10s tolerance)
    const expectedMs = before.getTime() + 72 * 60 * 60 * 1000;
    assert.ok(
      Math.abs(expiresAt.getTime() - expectedMs) < 10_000,
      `Expiry should be ~72h from now, got ${expiresAt.toISOString()}`,
    );
  });

  it('creates escrow with custom expiry hours', async () => {
    const before = new Date();
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      expiresInHours: 24,
    });
    const expiresAt = new Date(result.escrow.expiresAt);
    const expectedMs = before.getTime() + 24 * 60 * 60 * 1000;
    assert.ok(Math.abs(expiresAt.getTime() - expectedMs) < 10_000);
  });

  it('uppercases asset name', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      asset: 'dai',
    });
    assert.equal(result.escrow.asset, 'DAI');
  });

  it('uses amountDecimal when provided', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100000000,
      amountDecimal: 100.0,
    });
    assert.equal(result.escrow.amount, 100.0);
  });

  it('falls back to amount when amountDecimal is not provided', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 50,
    });
    assert.equal(result.escrow.amount, 50);
  });

  // --- Success / structure ---

  it('returns success true with escrow object', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 200,
    });
    assert.equal(result.success, true);
    assert.ok(result.escrow);
    assert.ok(result.escrow.id);
    assert.equal(result.escrow.status, 'created');
  });

  it('sets initial status to created', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    assert.equal(result.escrow.status, 'created');
  });

  it('stores quoteId when provided', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      quoteId: 'q-123',
    });
    assert.equal(result.escrow.quoteId, 'q-123');
  });

  it('quoteId defaults to null when omitted', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    assert.equal(result.escrow.quoteId, null);
  });

  it('stores buyerAddress and sellerAddress', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xAlice',
      sellerAddress: '0xBob',
      amount: 10,
    });
    assert.equal(result.escrow.buyerAddress, '0xAlice');
    assert.equal(result.escrow.sellerAddress, '0xBob');
  });

  it('sets createdAt and updatedAt timestamps', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    assert.ok(result.escrow.createdAt);
    assert.ok(result.escrow.updatedAt);
  });

  it('persists the escrow in the store', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    assert.ok(store._escrows.has(result.escrow.id));
  });

  // --- Condition building ---

  it('builds seller_fulfilled condition with quoteId from condition', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      conditions: [{ type: 'seller_fulfilled', quoteId: 'q-specific' }],
    });
    const conds = result.escrow.releaseConditions;
    assert.equal(conds.length, 1);
    assert.equal(conds[0].type, 'seller_fulfilled');
    assert.equal(conds[0].quoteId, 'q-specific');
  });

  it('builds seller_fulfilled condition falling back to top-level quoteId', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      quoteId: 'q-top-level',
      conditions: [{ type: 'seller_fulfilled' }],
    });
    assert.equal(result.escrow.releaseConditions[0].quoteId, 'q-top-level');
  });

  it('builds buyer_confirmed condition with completed=false', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      conditions: [{ type: 'buyer_confirmed' }],
    });
    const cond = result.escrow.releaseConditions[0];
    assert.equal(cond.type, 'buyer_confirmed');
    assert.equal(cond.completed, false);
  });

  it('builds time_lock condition with releaseAfter', async () => {
    const future = new Date(Date.now() + 86400000).toISOString();
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      conditions: [{ type: 'time_lock', releaseAfter: future }],
    });
    const cond = result.escrow.releaseConditions[0];
    assert.equal(cond.type, 'time_lock');
    assert.equal(cond.releaseAfter, future);
  });

  it('builds milestone condition with description and completed=false', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      conditions: [{ type: 'milestone', description: 'Ship product' }],
    });
    const cond = result.escrow.releaseConditions[0];
    assert.equal(cond.type, 'milestone');
    assert.equal(cond.description, 'Ship product');
    assert.equal(cond.completed, false);
  });

  it('handles unknown condition type with completed=false', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      conditions: [{ type: 'custom_check', customField: 'abc' }],
    });
    const cond = result.escrow.releaseConditions[0];
    assert.equal(cond.type, 'custom_check');
    assert.equal(cond.completed, false);
    assert.equal(cond.customField, 'abc');
  });

  it('builds multiple conditions', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      conditions: [{ type: 'buyer_confirmed' }, { type: 'milestone', description: 'QA pass' }],
    });
    assert.equal(result.escrow.releaseConditions.length, 2);
  });

  // --- autoReleaseAfterHours ---

  it('adds time_lock condition when autoReleaseAfterHours is set', async () => {
    const before = new Date();
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      autoReleaseAfterHours: 48,
    });
    const conds = result.escrow.releaseConditions;
    assert.equal(conds.length, 1);
    assert.equal(conds[0].type, 'time_lock');
    const releaseAfter = new Date(conds[0].releaseAfter);
    const expectedMs = before.getTime() + 48 * 60 * 60 * 1000;
    assert.ok(Math.abs(releaseAfter.getTime() - expectedMs) < 10_000);
  });

  it('does not duplicate time_lock if conditions already has one', async () => {
    const future = new Date(Date.now() + 86400000).toISOString();
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      conditions: [{ type: 'time_lock', releaseAfter: future }],
      autoReleaseAfterHours: 48,
    });
    const timeLocks = result.escrow.releaseConditions.filter((c) => c.type === 'time_lock');
    assert.equal(timeLocks.length, 1);
    assert.equal(timeLocks[0].releaseAfter, future);
  });

  // --- metadata ---

  it('stores metadata when provided', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      metadata: { orderId: 'ORD-1' },
    });
    const raw = store._escrows.get(result.escrow.id);
    assert.deepEqual(JSON.parse(raw.metadata), { orderId: 'ORD-1' });
  });

  it('metadata is null when not provided', async () => {
    const result = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    const raw = store._escrows.get(result.escrow.id);
    assert.equal(raw.metadata, null);
  });
});

// ---------------------------------------------------------------------------
// 2. fundEscrow
// ---------------------------------------------------------------------------

describe('fundEscrow', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEscrowService(store);
  });

  it('funds a created escrow and moves it to active', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    const result = await service.fundEscrow(created.escrow.id);
    assert.equal(result.success, true);
    assert.equal(result.escrow.status, 'active');
    assert.ok(result.escrow.fundedAt);
  });

  it('throws if escrow not found', async () => {
    await assert.rejects(() => service.fundEscrow('nonexistent'), {
      message: 'Escrow not found',
    });
  });

  it('throws for invalid transition from released', async () => {
    const escrow = await createActiveEscrow(service);
    // Release it (no conditions = all met)
    await service.releaseEscrow(escrow.id);
    await assert.rejects(() => service.fundEscrow(escrow.id), /Invalid escrow transition/);
  });

  it('throws for invalid transition from refunded', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    await service.refundEscrow(created.escrow.id);
    await assert.rejects(() => service.fundEscrow(created.escrow.id), /Invalid escrow transition/);
  });

  it('throws for invalid transition from active (already funded)', async () => {
    const escrow = await createActiveEscrow(service);
    await assert.rejects(() => service.fundEscrow(escrow.id), /Invalid escrow transition/);
  });
});

// ---------------------------------------------------------------------------
// 3. releaseEscrow
// ---------------------------------------------------------------------------

describe('releaseEscrow', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEscrowService(store);
  });

  it('releases an active escrow with no conditions (all met)', async () => {
    const escrow = await createActiveEscrow(service);
    const result = await service.releaseEscrow(escrow.id);
    assert.equal(result.success, true);
    assert.equal(result.escrow.status, 'released');
    assert.ok(result.escrow.releasedAt);
  });

  it('fails to release when conditions are not met', async () => {
    const escrow = await createActiveEscrow(service, {
      conditions: [{ type: 'buyer_confirmed' }],
    });
    const result = await service.releaseEscrow(escrow.id);
    assert.equal(result.success, false);
    assert.ok(result.unmetConditions);
    assert.ok(result.unmetConditions.length > 0);
    assert.match(result.error, /Not all release conditions are met/);
  });

  it('returns unmet condition descriptions in failure', async () => {
    const escrow = await createActiveEscrow(service, {
      conditions: [{ type: 'milestone', description: 'Deliver product' }],
    });
    const result = await service.releaseEscrow(escrow.id);
    assert.equal(result.success, false);
    assert.ok(result.unmetConditions.some((u) => u.includes('Deliver product')));
  });

  it('throws if escrow not found', async () => {
    await assert.rejects(() => service.releaseEscrow('nonexistent'), {
      message: 'Escrow not found',
    });
  });

  it('throws for created status (not yet funded)', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    await assert.rejects(
      () => service.releaseEscrow(created.escrow.id),
      /Cannot release escrow in status: created/,
    );
  });

  it('throws for already released escrow', async () => {
    const escrow = await createActiveEscrow(service);
    await service.releaseEscrow(escrow.id);
    await assert.rejects(
      () => service.releaseEscrow(escrow.id),
      /Cannot release escrow in status: released/,
    );
  });

  it('throws for refunded escrow', async () => {
    const escrow = await createActiveEscrow(service);
    await service.refundEscrow(escrow.id);
    await assert.rejects(
      () => service.releaseEscrow(escrow.id),
      /Cannot release escrow in status: refunded/,
    );
  });

  it('releases from funded status with no conditions', async () => {
    // Directly manipulate to funded (not active)
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    // Fund moves to active via the implementation, but the validation accepts "funded"
    // We directly set it to funded in the store
    store._escrows.get(created.escrow.id).status = 'funded';
    const result = await service.releaseEscrow(created.escrow.id);
    assert.equal(result.success, true);
    assert.equal(result.escrow.status, 'released');
  });
});

// ---------------------------------------------------------------------------
// 4. refundEscrow
// ---------------------------------------------------------------------------

describe('refundEscrow', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEscrowService(store);
  });

  it('refunds from created status', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    const result = await service.refundEscrow(created.escrow.id);
    assert.equal(result.success, true);
    assert.equal(result.escrow.status, 'refunded');
  });

  it('refunds from funded status', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    store._escrows.get(created.escrow.id).status = 'funded';
    const result = await service.refundEscrow(created.escrow.id);
    assert.equal(result.success, true);
    assert.equal(result.escrow.status, 'refunded');
  });

  it('refunds from active status', async () => {
    const escrow = await createActiveEscrow(service);
    const result = await service.refundEscrow(escrow.id);
    assert.equal(result.success, true);
    assert.equal(result.escrow.status, 'refunded');
  });

  it('throws if escrow not found', async () => {
    await assert.rejects(() => service.refundEscrow('nonexistent'), {
      message: 'Escrow not found',
    });
  });

  it('throws for released escrow', async () => {
    const escrow = await createActiveEscrow(service);
    await service.releaseEscrow(escrow.id);
    await assert.rejects(
      () => service.refundEscrow(escrow.id),
      /Cannot refund escrow in status: released/,
    );
  });

  it('throws for already refunded escrow', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    await service.refundEscrow(created.escrow.id);
    await assert.rejects(
      () => service.refundEscrow(created.escrow.id),
      /Cannot refund escrow in status: refunded/,
    );
  });

  it('throws for disputed escrow', async () => {
    const escrow = await createActiveEscrow(service);
    await service.disputeEscrow(escrow.id, { reason: 'fraud' });
    await assert.rejects(
      () => service.refundEscrow(escrow.id),
      /Cannot refund escrow in status: disputed/,
    );
  });
});

// ---------------------------------------------------------------------------
// 5. disputeEscrow
// ---------------------------------------------------------------------------

describe('disputeEscrow', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEscrowService(store);
  });

  it('disputes from active status', async () => {
    const escrow = await createActiveEscrow(service);
    const result = await service.disputeEscrow(escrow.id, {
      reason: 'Item not received',
      category: 'delivery',
    });
    assert.equal(result.success, true);
    assert.equal(result.escrow.status, 'disputed');
    assert.equal(result.disputeNeeded, true);
    assert.ok(result.escrow.disputedAt);
  });

  it('disputes from funded status', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    store._escrows.get(created.escrow.id).status = 'funded';
    const result = await service.disputeEscrow(created.escrow.id, { reason: 'wrong item' });
    assert.equal(result.success, true);
    assert.equal(result.escrow.status, 'disputed');
  });

  it('stores dispute metadata (reason and category)', async () => {
    const escrow = await createActiveEscrow(service);
    await service.disputeEscrow(escrow.id, {
      reason: 'Damaged goods',
      category: 'quality',
    });
    const raw = store._escrows.get(escrow.id);
    const meta = JSON.parse(raw.metadata);
    assert.equal(meta.dispute.reason, 'Damaged goods');
    assert.equal(meta.dispute.category, 'quality');
    assert.ok(meta.dispute.disputedAt);
  });

  it('preserves existing metadata when adding dispute', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      metadata: { orderId: 'ORD-5' },
    });
    await service.fundEscrow(created.escrow.id);
    await service.disputeEscrow(created.escrow.id, { reason: 'fraud' });
    const raw = store._escrows.get(created.escrow.id);
    const meta = JSON.parse(raw.metadata);
    assert.equal(meta.orderId, 'ORD-5');
    assert.equal(meta.dispute.reason, 'fraud');
  });

  it('handles dispute with no reason/category', async () => {
    const escrow = await createActiveEscrow(service);
    const result = await service.disputeEscrow(escrow.id, {});
    assert.equal(result.success, true);
    const raw = store._escrows.get(escrow.id);
    const meta = JSON.parse(raw.metadata);
    assert.equal(meta.dispute.reason, null);
    assert.equal(meta.dispute.category, null);
  });

  it('throws if escrow not found', async () => {
    await assert.rejects(() => service.disputeEscrow('nonexistent', { reason: 'test' }), {
      message: 'Escrow not found',
    });
  });

  it('throws for created escrow (not yet funded)', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    await assert.rejects(
      () => service.disputeEscrow(created.escrow.id, { reason: 'test' }),
      /Cannot dispute escrow in status: created/,
    );
  });

  it('throws for released escrow', async () => {
    const escrow = await createActiveEscrow(service);
    await service.releaseEscrow(escrow.id);
    await assert.rejects(
      () => service.disputeEscrow(escrow.id, { reason: 'test' }),
      /Cannot dispute escrow in status: released/,
    );
  });

  it('throws for already disputed escrow', async () => {
    const escrow = await createActiveEscrow(service);
    await service.disputeEscrow(escrow.id, { reason: 'first' });
    await assert.rejects(
      () => service.disputeEscrow(escrow.id, { reason: 'second' }),
      /Cannot dispute escrow in status: disputed/,
    );
  });
});

// ---------------------------------------------------------------------------
// 6. checkConditions
// ---------------------------------------------------------------------------

describe('checkConditions', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEscrowService(store);
  });

  it('returns allMet=true when there are no conditions', async () => {
    const escrow = await createActiveEscrow(service);
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.allMet, true);
    assert.deepEqual(result.conditions, []);
  });

  it('throws if escrow not found', async () => {
    await assert.rejects(() => service.checkConditions('nonexistent'), {
      message: 'Escrow not found',
    });
  });

  // --- seller_fulfilled ---

  it('seller_fulfilled is met when quote status is fulfilled', async () => {
    store._quotes.set('q-1', { id: 'q-1', status: 'fulfilled' });
    const escrow = await createActiveEscrow(service, {
      quoteId: 'q-1',
      conditions: [{ type: 'seller_fulfilled' }],
    });
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.allMet, true);
    assert.equal(result.conditions[0].met, true);
  });

  it('seller_fulfilled is not met when quote status is not fulfilled', async () => {
    store._quotes.set('q-2', { id: 'q-2', status: 'pending' });
    const escrow = await createActiveEscrow(service, {
      quoteId: 'q-2',
      conditions: [{ type: 'seller_fulfilled' }],
    });
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.allMet, false);
    assert.equal(result.conditions[0].met, false);
  });

  it('seller_fulfilled is not met when quote does not exist', async () => {
    const escrow = await createActiveEscrow(service, {
      quoteId: 'q-missing',
      conditions: [{ type: 'seller_fulfilled' }],
    });
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.conditions[0].met, false);
  });

  it('seller_fulfilled uses condition-level quoteId over escrow-level', async () => {
    store._quotes.set('q-cond', { id: 'q-cond', status: 'fulfilled' });
    store._quotes.set('q-esc', { id: 'q-esc', status: 'pending' });
    const escrow = await createActiveEscrow(service, {
      quoteId: 'q-esc',
      conditions: [{ type: 'seller_fulfilled', quoteId: 'q-cond' }],
    });
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.conditions[0].met, true);
  });

  it('seller_fulfilled handles getQuote throwing an error', async () => {
    const failStore = createMockStore();
    failStore.getQuote = async () => {
      throw new Error('DB error');
    };
    const failService = createEscrowService(failStore);
    const escrow = await createActiveEscrow(failService, {
      quoteId: 'q-err',
      conditions: [{ type: 'seller_fulfilled' }],
    });
    const result = await failService.checkConditions(escrow.id);
    assert.equal(result.conditions[0].met, false);
  });

  // --- buyer_confirmed ---

  it('buyer_confirmed is not met initially', async () => {
    const escrow = await createActiveEscrow(service, {
      conditions: [{ type: 'buyer_confirmed' }],
    });
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.conditions[0].met, false);
  });

  it('buyer_confirmed is met when completed=true', async () => {
    const escrow = await createActiveEscrow(service, {
      conditions: [{ type: 'buyer_confirmed' }],
    });
    // Manually complete the condition
    const raw = store._escrows.get(escrow.id);
    const conds = JSON.parse(raw.release_conditions);
    conds[0].completed = true;
    raw.release_conditions = JSON.stringify(conds);
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.conditions[0].met, true);
  });

  // --- time_lock ---

  it('time_lock is met when releaseAfter is in the past', async () => {
    const past = new Date(Date.now() - 86400000).toISOString();
    const escrow = await createActiveEscrow(service, {
      conditions: [{ type: 'time_lock', releaseAfter: past }],
    });
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.conditions[0].met, true);
  });

  it('time_lock is not met when releaseAfter is in the future', async () => {
    const future = new Date(Date.now() + 86400000).toISOString();
    const escrow = await createActiveEscrow(service, {
      conditions: [{ type: 'time_lock', releaseAfter: future }],
    });
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.conditions[0].met, false);
  });

  it('time_lock with no releaseAfter is not met', async () => {
    const escrow = await createActiveEscrow(service, {
      conditions: [{ type: 'time_lock' }],
    });
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.conditions[0].met, false);
  });

  // --- milestone ---

  it('milestone is not met initially', async () => {
    const escrow = await createActiveEscrow(service, {
      conditions: [{ type: 'milestone', description: 'Ship it' }],
    });
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.conditions[0].met, false);
  });

  it('milestone is met when completed=true', async () => {
    const escrow = await createActiveEscrow(service, {
      conditions: [{ type: 'milestone', description: 'Ship it' }],
    });
    const raw = store._escrows.get(escrow.id);
    const conds = JSON.parse(raw.release_conditions);
    conds[0].completed = true;
    raw.release_conditions = JSON.stringify(conds);
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.conditions[0].met, true);
  });

  // --- unknown type ---

  it('unknown condition type defaults to not met', async () => {
    const escrow = await createActiveEscrow(service, {
      conditions: [{ type: 'custom_oracle' }],
    });
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.conditions[0].met, false);
  });

  it('unknown condition type is met when completed=true', async () => {
    const escrow = await createActiveEscrow(service, {
      conditions: [{ type: 'custom_oracle' }],
    });
    const raw = store._escrows.get(escrow.id);
    const conds = JSON.parse(raw.release_conditions);
    conds[0].completed = true;
    raw.release_conditions = JSON.stringify(conds);
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.conditions[0].met, true);
  });

  // --- mixed conditions ---

  it('allMet is false when at least one condition is not met', async () => {
    const past = new Date(Date.now() - 86400000).toISOString();
    const escrow = await createActiveEscrow(service, {
      conditions: [{ type: 'time_lock', releaseAfter: past }, { type: 'buyer_confirmed' }],
    });
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.allMet, false);
    assert.equal(result.conditions[0].met, true); // time_lock past
    assert.equal(result.conditions[1].met, false); // buyer not confirmed
  });

  it('allMet is true when all conditions are met', async () => {
    const past = new Date(Date.now() - 86400000).toISOString();
    store._quotes.set('q-ok', { id: 'q-ok', status: 'fulfilled' });
    const escrow = await createActiveEscrow(service, {
      quoteId: 'q-ok',
      conditions: [{ type: 'seller_fulfilled' }, { type: 'time_lock', releaseAfter: past }],
    });
    const result = await service.checkConditions(escrow.id);
    assert.equal(result.allMet, true);
  });
});

// ---------------------------------------------------------------------------
// 7. confirmCondition
// ---------------------------------------------------------------------------

describe('confirmCondition', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEscrowService(store);
  });

  it('marks a buyer_confirmed condition as completed', async () => {
    const escrow = await createActiveEscrow(service, {
      conditions: [{ type: 'buyer_confirmed' }],
    });
    const result = await service.confirmCondition(escrow.id, 0);
    assert.equal(result.success, true);
    // It should auto-release since all conditions are now met
    assert.equal(result.allConditionsMet, true);
    assert.equal(result.escrow.status, 'released');
  });

  it('marks a milestone condition as completed', async () => {
    const escrow = await createActiveEscrow(service, {
      conditions: [
        { type: 'milestone', description: 'Step 1' },
        { type: 'milestone', description: 'Step 2' },
      ],
    });
    const result = await service.confirmCondition(escrow.id, 0);
    assert.equal(result.success, true);
    assert.equal(result.allConditionsMet, false);
    assert.notEqual(result.escrow.status, 'released');
  });

  it('auto-releases when last condition is confirmed on active escrow', async () => {
    const escrow = await createActiveEscrow(service, {
      conditions: [
        { type: 'milestone', description: 'Step 1' },
        { type: 'milestone', description: 'Step 2' },
      ],
    });
    await service.confirmCondition(escrow.id, 0);
    const result = await service.confirmCondition(escrow.id, 1);
    assert.equal(result.allConditionsMet, true);
    assert.equal(result.escrow.status, 'released');
  });

  it('auto-releases when last condition confirmed on funded escrow', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      conditions: [{ type: 'buyer_confirmed' }],
    });
    store._escrows.get(created.escrow.id).status = 'funded';
    const result = await service.confirmCondition(created.escrow.id, 0);
    assert.equal(result.allConditionsMet, true);
    assert.equal(result.escrow.status, 'released');
  });

  it('throws for invalid condition index (negative)', async () => {
    const escrow = await createActiveEscrow(service, {
      conditions: [{ type: 'buyer_confirmed' }],
    });
    await assert.rejects(() => service.confirmCondition(escrow.id, -1), /Invalid condition index/);
  });

  it('throws for invalid condition index (out of bounds)', async () => {
    const escrow = await createActiveEscrow(service, {
      conditions: [{ type: 'buyer_confirmed' }],
    });
    await assert.rejects(
      () => service.confirmCondition(escrow.id, 5),
      /Invalid condition index: 5. Escrow has 1 conditions/,
    );
  });

  it('throws for invalid condition index on escrow with no conditions', async () => {
    const escrow = await createActiveEscrow(service);
    await assert.rejects(
      () => service.confirmCondition(escrow.id, 0),
      /Invalid condition index: 0. Escrow has 0 conditions/,
    );
  });

  it('throws if escrow not found', async () => {
    await assert.rejects(() => service.confirmCondition('nonexistent', 0), {
      message: 'Escrow not found',
    });
  });

  it('does not auto-release from created status', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      conditions: [{ type: 'buyer_confirmed' }],
    });
    const result = await service.confirmCondition(created.escrow.id, 0);
    assert.equal(result.success, true);
    assert.equal(result.allConditionsMet, true);
    // Should not auto-release because status is "created" (not active/funded)
    assert.notEqual(result.escrow.status, 'released');
  });

  it('confirms condition at specific index in multi-condition set', async () => {
    const escrow = await createActiveEscrow(service, {
      conditions: [
        { type: 'milestone', description: 'A' },
        { type: 'milestone', description: 'B' },
        { type: 'milestone', description: 'C' },
      ],
    });
    await service.confirmCondition(escrow.id, 1); // confirm B only
    const check = await service.checkConditions(escrow.id);
    assert.equal(check.conditions[0].met, false); // A
    assert.equal(check.conditions[1].met, true); // B
    assert.equal(check.conditions[2].met, false); // C
  });
});

// ---------------------------------------------------------------------------
// 8. checkExpired
// ---------------------------------------------------------------------------

describe('checkExpired', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEscrowService(store);
  });

  it('expires an active escrow past its expiry', async () => {
    const escrow = await createActiveEscrow(service);
    // Set expiry in the past
    const raw = store._escrows.get(escrow.id);
    raw.expires_at = new Date(Date.now() - 1000).toISOString();
    const result = await service.checkExpired(escrow.id);
    assert.equal(result.expired, true);
    assert.equal(result.escrow.status, 'expired');
  });

  it('does not expire an active escrow before its expiry', async () => {
    const escrow = await createActiveEscrow(service);
    // Expiry is in the future (default 72h)
    const result = await service.checkExpired(escrow.id);
    assert.equal(result.expired, false);
    assert.equal(result.escrow.status, 'active');
  });

  it('expires a funded escrow past its expiry', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    store._escrows.get(created.escrow.id).status = 'funded';
    store._escrows.get(created.escrow.id).expires_at = new Date(Date.now() - 1000).toISOString();
    const result = await service.checkExpired(created.escrow.id);
    assert.equal(result.expired, true);
    assert.equal(result.escrow.status, 'expired');
  });

  it('ignores released escrow (returns expired=false)', async () => {
    const escrow = await createActiveEscrow(service);
    await service.releaseEscrow(escrow.id);
    const result = await service.checkExpired(escrow.id);
    assert.equal(result.expired, false);
    assert.equal(result.escrow.status, 'released');
  });

  it('ignores refunded escrow (returns expired=false)', async () => {
    const escrow = await createActiveEscrow(service);
    await service.refundEscrow(escrow.id);
    const result = await service.checkExpired(escrow.id);
    assert.equal(result.expired, false);
    assert.equal(result.escrow.status, 'refunded');
  });

  it('ignores disputed escrow', async () => {
    const escrow = await createActiveEscrow(service);
    await service.disputeEscrow(escrow.id, { reason: 'test' });
    const result = await service.checkExpired(escrow.id);
    assert.equal(result.expired, false);
    assert.equal(result.escrow.status, 'disputed');
  });

  it('ignores created escrow (not yet funded)', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    const raw = store._escrows.get(created.escrow.id);
    raw.expires_at = new Date(Date.now() - 1000).toISOString();
    const result = await service.checkExpired(created.escrow.id);
    assert.equal(result.expired, false);
  });

  it('throws if escrow not found', async () => {
    await assert.rejects(() => service.checkExpired('nonexistent'), {
      message: 'Escrow not found',
    });
  });

  it('handles escrow with no expires_at (does not expire)', async () => {
    const escrow = await createActiveEscrow(service);
    const raw = store._escrows.get(escrow.id);
    raw.expires_at = null;
    const result = await service.checkExpired(escrow.id);
    assert.equal(result.expired, false);
  });
});

// ---------------------------------------------------------------------------
// 9. getEscrow
// ---------------------------------------------------------------------------

describe('getEscrow', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEscrowService(store);
  });

  it('returns formatted escrow when found', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    const escrow = await service.getEscrow(created.escrow.id);
    assert.ok(escrow);
    assert.equal(escrow.id, created.escrow.id);
    assert.equal(escrow.buyerAddress, '0xBuyer');
    assert.equal(escrow.sellerAddress, '0xSeller');
    assert.equal(escrow.status, 'created');
  });

  it('returns null when not found', async () => {
    const result = await service.getEscrow('nonexistent');
    assert.equal(result, null);
  });

  it('returns camelCase keys', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      quoteId: 'q-1',
    });
    const escrow = await service.getEscrow(created.escrow.id);
    assert.ok('buyerAddress' in escrow);
    assert.ok('sellerAddress' in escrow);
    assert.ok('quoteId' in escrow);
    assert.ok('releaseConditions' in escrow);
    assert.ok('createdAt' in escrow);
    assert.ok('updatedAt' in escrow);
    assert.ok('expiresAt' in escrow);
  });

  it('parses release conditions from JSON string', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      conditions: [{ type: 'buyer_confirmed' }],
    });
    const escrow = await service.getEscrow(created.escrow.id);
    assert.ok(Array.isArray(escrow.releaseConditions));
    assert.equal(escrow.releaseConditions.length, 1);
    assert.equal(escrow.releaseConditions[0].type, 'buyer_confirmed');
  });
});

// ---------------------------------------------------------------------------
// 10. listEscrows
// ---------------------------------------------------------------------------

describe('listEscrows', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEscrowService(store);
  });

  it('returns empty array when no escrows exist', async () => {
    const result = await service.listEscrows();
    assert.deepEqual(result, []);
  });

  it('returns all escrows when no filter', async () => {
    await service.createEscrow({
      buyerAddress: '0xA',
      sellerAddress: '0xB',
      amount: 10,
    });
    await service.createEscrow({
      buyerAddress: '0xC',
      sellerAddress: '0xD',
      amount: 20,
    });
    const result = await service.listEscrows();
    assert.equal(result.length, 2);
  });

  it('filters by status', async () => {
    const e1 = await service.createEscrow({
      buyerAddress: '0xA',
      sellerAddress: '0xB',
      amount: 10,
    });
    await service.createEscrow({
      buyerAddress: '0xC',
      sellerAddress: '0xD',
      amount: 20,
    });
    await service.fundEscrow(e1.escrow.id);
    const active = await service.listEscrows({ status: 'active' });
    assert.equal(active.length, 1);
    assert.equal(active[0].buyerAddress, '0xA');
  });

  it('filters by buyer_address', async () => {
    await service.createEscrow({
      buyerAddress: '0xAlice',
      sellerAddress: '0xB',
      amount: 10,
    });
    await service.createEscrow({
      buyerAddress: '0xBob',
      sellerAddress: '0xD',
      amount: 20,
    });
    const result = await service.listEscrows({ buyer_address: '0xAlice' });
    assert.equal(result.length, 1);
    assert.equal(result[0].buyerAddress, '0xAlice');
  });

  it('returns formatted escrow objects with camelCase keys', async () => {
    await service.createEscrow({
      buyerAddress: '0xA',
      sellerAddress: '0xB',
      amount: 10,
    });
    const result = await service.listEscrows();
    assert.equal(result.length, 1);
    assert.ok('buyerAddress' in result[0]);
    assert.ok('sellerAddress' in result[0]);
    assert.ok('createdAt' in result[0]);
  });

  it('returns empty when filter matches nothing', async () => {
    await service.createEscrow({
      buyerAddress: '0xA',
      sellerAddress: '0xB',
      amount: 10,
    });
    const result = await service.listEscrows({ status: 'released' });
    assert.deepEqual(result, []);
  });
});

// ---------------------------------------------------------------------------
// 11. formatEscrow edge cases
// ---------------------------------------------------------------------------

describe('formatEscrow (via getEscrow)', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEscrowService(store);
  });

  it('handles release_conditions already as an array (not JSON string)', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      conditions: [{ type: 'buyer_confirmed' }],
    });
    // Replace the string with an actual array in the raw store
    const raw = store._escrows.get(created.escrow.id);
    raw.release_conditions = [{ type: 'buyer_confirmed', completed: false }];
    const escrow = await service.getEscrow(created.escrow.id);
    assert.ok(Array.isArray(escrow.releaseConditions));
    assert.equal(escrow.releaseConditions[0].type, 'buyer_confirmed');
  });

  it('handles malformed JSON in release_conditions gracefully', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    const raw = store._escrows.get(created.escrow.id);
    raw.release_conditions = '{bad json';
    const escrow = await service.getEscrow(created.escrow.id);
    assert.deepEqual(escrow.releaseConditions, []);
  });

  it('returns null for missing optional date fields', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    const escrow = await service.getEscrow(created.escrow.id);
    assert.equal(escrow.fundedAt, null);
    assert.equal(escrow.releasedAt, null);
    assert.equal(escrow.disputedAt, null);
  });

  it('returns amount from amount_decimal when present', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100000000,
      amountDecimal: 100.5,
    });
    const escrow = await service.getEscrow(created.escrow.id);
    assert.equal(escrow.amount, 100.5);
  });
});

// ---------------------------------------------------------------------------
// 12. Full lifecycle integration tests
// ---------------------------------------------------------------------------

describe('full lifecycle', () => {
  let store;
  let service;

  beforeEach(() => {
    store = createMockStore();
    service = createEscrowService(store);
  });

  it('create -> fund -> release (no conditions)', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    assert.equal(created.escrow.status, 'created');

    const funded = await service.fundEscrow(created.escrow.id);
    assert.equal(funded.escrow.status, 'active');

    const released = await service.releaseEscrow(created.escrow.id);
    assert.equal(released.escrow.status, 'released');
  });

  it('create -> fund -> confirm conditions -> auto-release', async () => {
    store._quotes.set('q-1', { id: 'q-1', status: 'fulfilled' });
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 250,
      quoteId: 'q-1',
      conditions: [{ type: 'seller_fulfilled' }, { type: 'buyer_confirmed' }],
    });

    await service.fundEscrow(created.escrow.id);

    // Confirm buyer_confirmed (index 1) -> triggers auto-release because seller_fulfilled is also met
    const result = await service.confirmCondition(created.escrow.id, 1);
    assert.equal(result.allConditionsMet, true);
    assert.equal(result.escrow.status, 'released');
  });

  it('create -> fund -> dispute', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    await service.fundEscrow(created.escrow.id);
    const disputed = await service.disputeEscrow(created.escrow.id, {
      reason: 'Never shipped',
      category: 'fulfillment',
    });
    assert.equal(disputed.escrow.status, 'disputed');
    assert.equal(disputed.disputeNeeded, true);
  });

  it('create -> fund -> expire (past expiry)', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
      expiresInHours: 0.0001, // very short
    });
    await service.fundEscrow(created.escrow.id);
    // Force expiry to the past
    const raw = store._escrows.get(created.escrow.id);
    raw.expires_at = new Date(Date.now() - 1000).toISOString();
    const result = await service.checkExpired(created.escrow.id);
    assert.equal(result.expired, true);
    assert.equal(result.escrow.status, 'expired');
  });

  it('create -> refund (before funding)', async () => {
    const created = await service.createEscrow({
      buyerAddress: '0xBuyer',
      sellerAddress: '0xSeller',
      amount: 100,
    });
    const result = await service.refundEscrow(created.escrow.id);
    assert.equal(result.escrow.status, 'refunded');
  });
});
