/**
 * Tests for cli/src/approvals/queue.js
 *
 * Covers: ApprovalStatus, ApprovalTier, ApprovalChain, ApprovalDecision,
 * ApprovalRequest, ApprovalQueue, ApprovalChainTemplates.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';

import {
  ApprovalStatus,
  ApprovalTier,
  ApprovalChain,
  ApprovalDecision,
  ApprovalRequest,
  ApprovalQueue,
  ApprovalChainTemplates,
} from '../../src/approvals/queue.js';

// ---------------------------------------------------------------------------
// ApprovalStatus
// ---------------------------------------------------------------------------

describe('ApprovalStatus', () => {
  it('has all expected statuses', () => {
    const expected = [
      'PENDING',
      'APPROVED',
      'REJECTED',
      'ESCALATED',
      'EXPIRED',
      'AUTO_APPROVED',
      'AUTO_REJECTED',
      'CANCELLED',
    ];
    for (const s of expected) {
      assert.ok(s in ApprovalStatus, `missing status: ${s}`);
    }
  });
});

// ---------------------------------------------------------------------------
// ApprovalTier
// ---------------------------------------------------------------------------

describe('ApprovalTier', () => {
  it('stores all properties', () => {
    const tier = new ApprovalTier({
      level: 1,
      name: 'Manager',
      approvers: ['mgr1'],
      requiredApprovals: 2,
      timeout: 3600000,
      timeoutAction: 'escalate',
      canApproveAmount: 5000,
    });
    assert.equal(tier.level, 1);
    assert.equal(tier.name, 'Manager');
    assert.deepEqual(tier.approvers, ['mgr1']);
    assert.equal(tier.requiredApprovals, 2);
    assert.equal(tier.timeout, 3600000);
    assert.equal(tier.timeoutAction, 'escalate');
    assert.equal(tier.canApproveAmount, 5000);
  });

  it('defaults optional fields', () => {
    const tier = new ApprovalTier({ level: 1, name: 'T1' });
    assert.deepEqual(tier.approvers, []);
    assert.equal(tier.requiredApprovals, 1);
    assert.equal(tier.timeout, null);
    assert.equal(tier.timeoutAction, 'escalate');
    assert.equal(tier.canApproveAmount, null);
  });

  it('toJSON includes all fields', () => {
    const tier = new ApprovalTier({ level: 2, name: 'Dir' });
    const json = tier.toJSON();
    assert.equal(json.level, 2);
    assert.equal(json.name, 'Dir');
    assert.ok('approvers' in json);
    assert.ok('timeout' in json);
  });
});

// ---------------------------------------------------------------------------
// ApprovalChain
// ---------------------------------------------------------------------------

describe('ApprovalChain', () => {
  const makeTiers = () => [
    { level: 1, name: 'T1', canApproveAmount: 100 },
    { level: 2, name: 'T2', canApproveAmount: 1000 },
    { level: 3, name: 'T3', canApproveAmount: null },
  ];

  it('auto-generates id', () => {
    const chain = new ApprovalChain({ name: 'C', domain: 'orders', tiers: makeTiers() });
    assert.ok(chain.id);
  });

  it('sorts tiers by level', () => {
    const chain = new ApprovalChain({
      name: 'C',
      domain: 'orders',
      tiers: [
        { level: 3, name: 'T3' },
        { level: 1, name: 'T1' },
        { level: 2, name: 'T2' },
      ],
    });
    assert.equal(chain.tiers[0].level, 1);
    assert.equal(chain.tiers[1].level, 2);
    assert.equal(chain.tiers[2].level, 3);
  });

  it('getTier returns correct tier', () => {
    const chain = new ApprovalChain({ name: 'C', domain: 'orders', tiers: makeTiers() });
    assert.equal(chain.getTier(2).name, 'T2');
    assert.equal(chain.getTier(99), undefined);
  });

  it('getNextTier returns next higher tier', () => {
    const chain = new ApprovalChain({ name: 'C', domain: 'orders', tiers: makeTiers() });
    assert.equal(chain.getNextTier(1).level, 2);
    assert.equal(chain.getNextTier(3), undefined);
  });

  it('getTierForAmount selects lowest sufficient tier', () => {
    const chain = new ApprovalChain({ name: 'C', domain: 'orders', tiers: makeTiers() });
    assert.equal(chain.getTierForAmount(50).level, 1);
    assert.equal(chain.getTierForAmount(500).level, 2);
    assert.equal(chain.getTierForAmount(5000).level, 3);
  });

  it('toJSON serializes correctly', () => {
    const chain = new ApprovalChain({ name: 'C', domain: 'orders', tiers: makeTiers() });
    const json = chain.toJSON();
    assert.equal(json.name, 'C');
    assert.equal(json.domain, 'orders');
    assert.equal(json.tiers.length, 3);
    assert.equal(json.enabled, true);
  });
});

// ---------------------------------------------------------------------------
// ApprovalDecision
// ---------------------------------------------------------------------------

describe('ApprovalDecision', () => {
  it('stores all fields', () => {
    const d = new ApprovalDecision({
      approverId: 'user-1',
      approverName: 'Alice',
      action: 'approve',
      reason: 'looks good',
      tier: 1,
    });
    assert.equal(d.approverId, 'user-1');
    assert.equal(d.approverName, 'Alice');
    assert.equal(d.action, 'approve');
    assert.equal(d.reason, 'looks good');
    assert.equal(d.tier, 1);
    assert.ok(d.timestamp);
  });

  it('toJSON works', () => {
    const d = new ApprovalDecision({ approverId: 'u', action: 'reject', tier: 2 });
    const json = d.toJSON();
    assert.equal(json.approverId, 'u');
    assert.equal(json.action, 'reject');
  });
});

// ---------------------------------------------------------------------------
// ApprovalRequest
// ---------------------------------------------------------------------------

describe('ApprovalRequest', () => {
  it('auto-generates id', () => {
    const r = new ApprovalRequest({
      chainId: 'c1',
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'T',
      requestedBy: 'agent',
    });
    assert.ok(r.id);
  });

  it('defaults to PENDING status', () => {
    const r = new ApprovalRequest({
      chainId: 'c1',
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'T',
      requestedBy: 'agent',
    });
    assert.equal(r.status, ApprovalStatus.PENDING);
  });

  it('isResolved returns false for pending', () => {
    const r = new ApprovalRequest({
      chainId: 'c1',
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'T',
      requestedBy: 'agent',
    });
    assert.equal(r.isResolved(), false);
  });

  it('isResolved returns true for terminal statuses', () => {
    for (const status of [
      'approved',
      'rejected',
      'expired',
      'auto_approved',
      'auto_rejected',
      'cancelled',
    ]) {
      const r = new ApprovalRequest({
        chainId: 'c1',
        domain: 'orders',
        entityType: 'order',
        entityId: 'o1',
        title: 'T',
        requestedBy: 'agent',
        status,
      });
      assert.equal(r.isResolved(), true, `${status} should be resolved`);
    }
  });

  it('getCurrentTierApprovals filters correctly', () => {
    const r = new ApprovalRequest({
      chainId: 'c1',
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'T',
      requestedBy: 'agent',
      currentTier: 2,
      decisions: [
        { approverId: 'u1', action: 'approve', tier: 1 },
        { approverId: 'u2', action: 'approve', tier: 2 },
        { approverId: 'u3', action: 'reject', tier: 2 },
      ],
    });
    const approvals = r.getCurrentTierApprovals();
    assert.equal(approvals.length, 1);
    assert.equal(approvals[0].approverId, 'u2');
  });

  it('toJSON serializes all fields', () => {
    const r = new ApprovalRequest({
      chainId: 'c1',
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Big Order',
      requestedBy: 'agent',
      amount: 5000,
    });
    const json = r.toJSON();
    assert.equal(json.title, 'Big Order');
    assert.equal(json.amount, 5000);
    assert.ok(json.createdAt);
  });
});

// ---------------------------------------------------------------------------
// ApprovalQueue — in-memory, no storage
// ---------------------------------------------------------------------------

describe('ApprovalQueue', () => {
  /** @type {ApprovalQueue} */
  let queue;

  function registerOrderChain(q) {
    return q.registerChain({
      name: 'Order Approval',
      domain: 'orders',
      tiers: [
        {
          level: 1,
          name: 'Tier 1',
          approvers: ['mgr'],
          requiredApprovals: 1,
          canApproveAmount: 1000,
        },
        {
          level: 2,
          name: 'Tier 2',
          approvers: ['dir'],
          requiredApprovals: 1,
          canApproveAmount: null,
        },
      ],
    });
  }

  beforeEach(() => {
    queue = new ApprovalQueue({});
  });

  afterEach(() => {
    queue.stop();
  });

  it('starts with empty state', () => {
    const status = queue.getStatus();
    assert.equal(status.pendingCount, 0);
    assert.equal(status.chainCount, 0);
  });

  it('registerChain adds a chain', () => {
    const chain = registerOrderChain(queue);
    assert.ok(chain instanceof ApprovalChain);
    assert.equal(queue.getStatus().chainCount, 1);
  });

  it('getChainForDomain returns matching chain', () => {
    registerOrderChain(queue);
    const chain = queue.getChainForDomain('orders');
    assert.ok(chain);
    assert.equal(chain.domain, 'orders');
  });

  it('getChainForDomain returns null for unknown domain', () => {
    assert.equal(queue.getChainForDomain('shipping'), null);
  });

  it('createRequest returns { required: false } when no chain matches', async () => {
    const result = await queue.createRequest({
      domain: 'unknown',
      entityType: 'x',
      entityId: '1',
      title: 'T',
      requestedBy: 'agent',
    });
    assert.equal(result.required, false);
  });

  it('createRequest creates a pending request', async () => {
    registerOrderChain(queue);
    const { required, request } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order #123',
      amount: 500,
      requestedBy: 'bot',
    });
    assert.equal(required, true);
    assert.equal(request.status, ApprovalStatus.PENDING);
    assert.equal(request.currentTier, 1);
    assert.equal(queue.getStatus().pendingCount, 1);
  });

  it('createRequest routes high amount to correct tier', async () => {
    registerOrderChain(queue);
    const { request } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o2',
      title: 'Big Order',
      amount: 5000,
      requestedBy: 'bot',
    });
    assert.equal(request.currentTier, 2);
  });

  it('approve resolves when enough approvals', async () => {
    registerOrderChain(queue);
    const { request } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order',
      amount: 500,
      requestedBy: 'bot',
    });
    const result = await queue.approve(request.id, 'mgr');
    assert.equal(result.status, ApprovalStatus.APPROVED);
    assert.equal(queue.getStatus().pendingCount, 0);
  });

  it('approve emits request:approved', async () => {
    registerOrderChain(queue);
    const { request } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order',
      amount: 500,
      requestedBy: 'bot',
    });
    const events = [];
    queue.on('request:approved', (e) => events.push(e));
    await queue.approve(request.id, 'mgr');
    assert.equal(events.length, 1);
  });

  it('reject marks request as rejected', async () => {
    registerOrderChain(queue);
    const { request } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order',
      amount: 500,
      requestedBy: 'bot',
    });
    const result = await queue.reject(request.id, 'mgr', { reason: 'suspicious' });
    assert.equal(result.status, ApprovalStatus.REJECTED);
    assert.equal(queue.getStatus().pendingCount, 0);
  });

  it('reject throws for unknown request', async () => {
    await assert.rejects(() => queue.reject('nonexistent', 'mgr'), /not found/);
  });

  it('reject throws for already resolved request', async () => {
    registerOrderChain(queue);
    const { request } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order',
      amount: 500,
      requestedBy: 'bot',
    });
    await queue.approve(request.id, 'mgr');
    // After approval, request is moved to history — reject throws "not found"
    await assert.rejects(() => queue.reject(request.id, 'mgr'), /not found/);
  });

  it('escalate moves to next tier', async () => {
    registerOrderChain(queue);
    const { request } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order',
      amount: 500,
      requestedBy: 'bot',
    });
    const result = await queue.escalate(request.id, 'mgr', 'needs director');
    assert.equal(result.currentTier, 2);
    assert.equal(result.status, ApprovalStatus.ESCALATED);
  });

  it('escalate throws when at highest tier', async () => {
    registerOrderChain(queue);
    const { request } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o2',
      title: 'Big Order',
      amount: 5000,
      requestedBy: 'bot',
    });
    // Request starts at tier 2 (highest)
    await assert.rejects(() => queue.escalate(request.id, 'dir', 'test'), /No higher tier/);
  });

  it('cancel marks request as cancelled', async () => {
    registerOrderChain(queue);
    const { request } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order',
      amount: 500,
      requestedBy: 'bot',
    });
    const result = await queue.cancel(request.id, 'changed mind');
    assert.equal(result.status, ApprovalStatus.CANCELLED);
  });

  it('autoApprove sets AUTO_APPROVED status', async () => {
    registerOrderChain(queue);
    const { request } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order',
      amount: 500,
      requestedBy: 'bot',
    });
    const result = await queue.autoApprove(request.id, 'timeout');
    assert.equal(result.status, ApprovalStatus.AUTO_APPROVED);
  });

  it('autoReject sets AUTO_REJECTED status', async () => {
    registerOrderChain(queue);
    const { request } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order',
      amount: 500,
      requestedBy: 'bot',
    });
    const result = await queue.autoReject(request.id, 'timeout');
    assert.equal(result.status, ApprovalStatus.AUTO_REJECTED);
  });

  it('getRequest finds pending requests', async () => {
    registerOrderChain(queue);
    const { request } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order',
      amount: 500,
      requestedBy: 'bot',
    });
    assert.ok(queue.getRequest(request.id));
  });

  it('getRequest finds resolved requests in history', async () => {
    registerOrderChain(queue);
    const { request } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order',
      amount: 500,
      requestedBy: 'bot',
    });
    await queue.approve(request.id, 'mgr');
    assert.ok(queue.getRequest(request.id));
  });

  it('listPending filters by domain', async () => {
    registerOrderChain(queue);
    await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order',
      amount: 500,
      requestedBy: 'bot',
    });
    assert.equal(queue.listPending({ domain: 'orders' }).length, 1);
    assert.equal(queue.listPending({ domain: 'returns' }).length, 0);
  });

  it('getHistory filters by status', async () => {
    registerOrderChain(queue);
    const { request: r1 } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order 1',
      amount: 500,
      requestedBy: 'bot',
    });
    const { request: r2 } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o2',
      title: 'Order 2',
      amount: 500,
      requestedBy: 'bot',
    });
    await queue.approve(r1.id, 'mgr');
    await queue.reject(r2.id, 'mgr');
    assert.equal(queue.getHistory({ status: 'approved' }).length, 1);
    assert.equal(queue.getHistory({ status: 'rejected' }).length, 1);
  });

  it('approve executes action via executor', async () => {
    const executed = [];
    const q = new ApprovalQueue({
      executor: async (action, ctx) => executed.push({ action, ctx }),
    });
    q.registerChain({
      name: 'C',
      domain: 'orders',
      tiers: [{ level: 1, name: 'T1', approvers: ['mgr'], requiredApprovals: 1 }],
    });
    const { request } = await q.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order',
      requestedBy: 'bot',
      action: { type: 'fulfill' },
    });
    await q.approve(request.id, 'mgr');
    q.stop();
    assert.equal(executed.length, 1);
    assert.deepEqual(executed[0].action, { type: 'fulfill' });
  });

  it('notifier is called when request is created', async () => {
    const notifications = [];
    const q = new ApprovalQueue({
      notifier: async (data) => notifications.push(data),
    });
    q.registerChain({
      name: 'C',
      domain: 'orders',
      tiers: [{ level: 1, name: 'T1', approvers: ['mgr'], requiredApprovals: 1 }],
    });
    await q.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order',
      requestedBy: 'bot',
    });
    q.stop();
    assert.equal(notifications.length, 1);
    assert.equal(notifications[0].type, 'approval_required');
  });

  it('start/stop manage background timer', () => {
    queue.start();
    assert.equal(queue.isRunning, true);
    assert.ok(queue.checkTimer);
    queue.stop();
    assert.equal(queue.isRunning, false);
    assert.equal(queue.checkTimer, null);
  });

  it('moveToHistory trims to 1000 entries', () => {
    // Fill history past limit
    for (let i = 0; i < 1005; i++) {
      queue.history.push(
        new ApprovalRequest({
          chainId: 'c',
          domain: 'orders',
          entityType: 'order',
          entityId: `o${i}`,
          title: 'T',
          requestedBy: 'bot',
          status: 'approved',
        }),
      );
    }
    // Add one more via moveToHistory
    const r = new ApprovalRequest({
      chainId: 'c',
      domain: 'orders',
      entityType: 'order',
      entityId: 'final',
      title: 'T',
      requestedBy: 'bot',
    });
    queue.requests.set(r.id, r);
    queue.moveToHistory(r);
    assert.ok(queue.history.length <= 1001);
  });
});

// ---------------------------------------------------------------------------
// ApprovalChainTemplates
// ---------------------------------------------------------------------------

describe('ApprovalChainTemplates', () => {
  it('has orderApproval template', () => {
    assert.ok(ApprovalChainTemplates.orderApproval);
    assert.equal(ApprovalChainTemplates.orderApproval.domain, 'orders');
    assert.ok(ApprovalChainTemplates.orderApproval.tiers.length >= 2);
  });

  it('has returnApproval template', () => {
    assert.ok(ApprovalChainTemplates.returnApproval);
    assert.equal(ApprovalChainTemplates.returnApproval.domain, 'returns');
  });

  it('has purchaseOrderApproval template', () => {
    assert.ok(ApprovalChainTemplates.purchaseOrderApproval);
    assert.equal(ApprovalChainTemplates.purchaseOrderApproval.domain, 'purchase_orders');
  });

  it('has refundApproval template', () => {
    assert.ok(ApprovalChainTemplates.refundApproval);
    assert.equal(ApprovalChainTemplates.refundApproval.domain, 'refunds');
  });

  it('templates can be registered as chains', () => {
    const q = new ApprovalQueue({});
    const chain = q.registerChain(ApprovalChainTemplates.orderApproval);
    assert.ok(chain instanceof ApprovalChain);
    assert.equal(chain.tiers.length, 4);
    q.stop();
  });
});

// ---------------------------------------------------------------------------
// ApprovalQueue — condition matching
// ---------------------------------------------------------------------------

describe('ApprovalQueue — conditional chains', () => {
  let queue;

  afterEach(() => {
    queue?.stop();
  });

  it('matches chain with conditions', () => {
    queue = new ApprovalQueue({});
    queue.registerChain({
      name: 'VIP Orders',
      domain: 'orders',
      conditions: { customerType: 'vip' },
      tiers: [{ level: 1, name: 'T1', approvers: ['mgr'], requiredApprovals: 1 }],
    });
    const match = queue.getChainForDomain('orders', { customerType: 'vip' });
    assert.ok(match);
    assert.equal(match.name, 'VIP Orders');
  });

  it('skips chain when conditions do not match', () => {
    queue = new ApprovalQueue({});
    queue.registerChain({
      name: 'VIP Only',
      domain: 'orders',
      conditions: { customerType: 'vip' },
      tiers: [{ level: 1, name: 'T1', approvers: ['mgr'], requiredApprovals: 1 }],
    });
    const match = queue.getChainForDomain('orders', { customerType: 'regular' });
    assert.equal(match, null);
  });

  it('skips disabled chains', () => {
    queue = new ApprovalQueue({});
    queue.registerChain({
      name: 'Disabled',
      domain: 'orders',
      enabled: false,
      tiers: [{ level: 1, name: 'T1', approvers: ['mgr'], requiredApprovals: 1 }],
    });
    assert.equal(queue.getChainForDomain('orders'), null);
  });
});

// ---------------------------------------------------------------------------
// ApprovalQueue — multi-approval
// ---------------------------------------------------------------------------

describe('ApprovalQueue — multi-approval tiers', () => {
  let queue;

  afterEach(() => {
    queue?.stop();
  });

  it('requires multiple approvals when configured', async () => {
    queue = new ApprovalQueue({});
    queue.registerChain({
      name: 'C',
      domain: 'orders',
      tiers: [{ level: 1, name: 'T1', approvers: ['a', 'b'], requiredApprovals: 2 }],
    });
    const { request } = await queue.createRequest({
      domain: 'orders',
      entityType: 'order',
      entityId: 'o1',
      title: 'Order',
      requestedBy: 'bot',
    });
    // First approval: still pending
    const after1 = await queue.approve(request.id, 'a');
    assert.equal(after1.status, ApprovalStatus.PENDING);
    // Second approval: approved
    const after2 = await queue.approve(request.id, 'b');
    assert.equal(after2.status, ApprovalStatus.APPROVED);
  });
});
