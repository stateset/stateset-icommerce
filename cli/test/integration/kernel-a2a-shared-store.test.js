import { afterEach, describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';

import { Commerce } from '@stateset/embedded';
import { createEmbeddedAgentToolkit } from '../../src/agent-toolkit.js';

let fixtureDir;
let toolkit;

afterEach(async () => {
  try {
    await toolkit?.server?.close?.();
  } catch {
    // The SDK server may never have been connected; storage cleanup still runs.
  }
  toolkit = undefined;
  if (fixtureDir) await rm(fixtureDir, { recursive: true, force: true });
  fixtureDir = undefined;
});

describe('governed A2A shared-store lifecycle', () => {
  it('launches an exact-price product and exact-quantity stock through the native kernel', async () => {
    fixtureDir = await mkdtemp(join(tmpdir(), 'stateset-kernel-launch-'));
    const commerce = new Commerce(join(fixtureDir, 'store.db'));
    const kernelCapabilities = ['products.create', 'inventory.item.create'];
    const commandPolicy = (capability) => ({
      required_capabilities: [capability],
      requires_approval: false,
      requires_tenant: true,
      requires_store: true,
      allowed_tenant_ids: ['tenant:test'],
      allowed_store_ids: ['store:test'],
      requires_agent_delegation: true,
      requires_signed_authority: false,
    });
    toolkit = createEmbeddedAgentToolkit({
      commerce,
      allowApply: true,
      capabilities: ['create_product', 'create_inventory_item', ...kernelCapabilities],
      kernel: {
        strict: true,
        storeId: 'store:test',
        principal: {
          id: 'agent:launch',
          kind: 'agent',
          tenantId: 'tenant:test',
          delegatedBy: 'user:test',
          capabilities: kernelCapabilities,
        },
        policy: {
          version: 'launch-v1',
          commands: Object.fromEntries(
            kernelCapabilities.map((capability) => [capability, commandPolicy(capability)]),
          ),
          trusted_authority_keys: {},
        },
      },
    });

    const product = await toolkit.executeTool(
      'create_product',
      {
        name: 'Native Agent Offer',
        variants: [{ sku: 'NATIVE-AGENT-001', price: '9007199254740993.25' }],
      },
      { idempotencyKey: 'native-product-create-1' },
    );
    const inventory = await toolkit.executeTool(
      'create_inventory_item',
      {
        sku: 'NATIVE-AGENT-001',
        name: 'Native Agent Offer stock',
        initialQuantity: '9007199254740993.125',
        reorderPoint: '0.125',
      },
      { idempotencyKey: 'native-inventory-create-1' },
    );

    assert.equal(product.result.kernel, true);
    assert.equal(product.result.receipt.status, 'succeeded');
    assert.ok(product.result.receipt.audit_hash);
    assert.equal(inventory.result.kernel, true);
    assert.equal(inventory.result.receipt.result.sku, 'NATIVE-AGENT-001');
    assert.ok(inventory.result.receipt.audit_hash);
    const stock = await commerce.inventory.getStock('NATIVE-AGENT-001');
    assert.equal(stock.totalOnHand, '9007199254740993.125');
    assert.equal(stock.totalAvailable, '9007199254740993.125');
  });

  it('releases the escrow created and funded by public tools through the native kernel', async () => {
    fixtureDir = await mkdtemp(join(tmpdir(), 'stateset-kernel-a2a-'));
    const dbPath = join(fixtureDir, 'store.db');
    const commerce = new Commerce(dbPath);
    const commandPolicy = (capability) => ({
      required_capabilities: [capability],
      requires_approval: false,
      requires_tenant: true,
      requires_store: true,
      allowed_tenant_ids: ['tenant:test'],
      allowed_store_ids: ['store:test'],
      requires_agent_delegation: true,
      requires_signed_authority: false,
    });
    const kernelCapabilities = [
      'a2a.escrow.create',
      'a2a.escrow.dispute',
      'a2a.escrow.fund',
      'a2a.escrow.refund',
      'a2a.escrow.release',
      'a2a.dispute.file',
      'a2a.dispute.evidence.submit',
      'a2a.dispute.resolve',
    ];
    toolkit = createEmbeddedAgentToolkit({
      commerce,
      dbPath,
      allowApply: true,
      capabilities: [
        'a2a_create_escrow',
        'a2a_dispute_escrow',
        'a2a_fund_escrow',
        'a2a_refund_escrow',
        'a2a_release_escrow',
        'a2a_file_dispute',
        'a2a_submit_evidence',
        'a2a_resolve_dispute',
        ...kernelCapabilities,
      ],
      agentConfig: { walletAddress: '0xagent' },
      kernel: {
        strict: true,
        storeId: 'store:test',
        principal: {
          id: '0xagent',
          kind: 'agent',
          tenantId: 'tenant:test',
          delegatedBy: 'user:test',
          capabilities: kernelCapabilities,
        },
        policy: {
          version: 'a2a-shared-store-v1',
          commands: Object.fromEntries(
            kernelCapabilities.map((capability) => [capability, commandPolicy(capability)]),
          ),
          trusted_authority_keys: {},
        },
      },
    });

    const created = await toolkit.executeTool('a2a_create_escrow', {
      buyerAddress: '0xbuyer',
      sellerAddress: '0xseller',
      amount: '1.000001',
    });
    const escrowId = created.result.receipt.result.id;
    const funded = await toolkit.executeTool('a2a_fund_escrow', { escrowId });
    const released = await toolkit.executeTool(
      'a2a_release_escrow',
      { escrowId },
      { idempotencyKey: 'a2a-release-shared-store-1' },
    );
    const refundable = await toolkit.executeTool(
      'a2a_create_escrow',
      {
        buyerAddress: '0xagent',
        sellerAddress: '0xseller',
        amount: '0.250001',
      },
      { idempotencyKey: 'a2a-create-refundable-shared-store-1' },
    );
    const refundableId = refundable.result.receipt.result.id;
    await toolkit.executeTool(
      'a2a_fund_escrow',
      { escrowId: refundableId },
      { idempotencyKey: 'a2a-fund-refundable-shared-store-1' },
    );
    const disputed = await toolkit.executeTool(
      'a2a_file_dispute',
      {
        escrowId: refundableId,
        reason: 'delivery evidence missing',
        category: 'non_delivery',
      },
      { idempotencyKey: 'a2a-dispute-shared-store-1' },
    );
    const disputeId = disputed.result.receipt.result.id;
    const evidence = await toolkit.executeTool(
      'a2a_submit_evidence',
      {
        disputeId,
        evidenceType: 'communication',
        title: 'Seller conversation',
        content: 'seller acknowledged non-delivery',
      },
      { idempotencyKey: 'a2a-evidence-shared-store-1' },
    );
    const resolved = await toolkit.executeTool(
      'a2a_resolve_dispute',
      {
        disputeId,
        resolutionType: 'split',
        buyerAmount: '0.100001',
        sellerAmount: '0.150000',
      },
      { idempotencyKey: 'a2a-resolve-shared-store-1' },
    );

    assert.equal(created.result.kernel, true);
    assert.equal(created.result.receipt.result.amount_decimal, '1.000001');
    assert.equal(funded.result.kernel, true);
    assert.equal(funded.result.receipt.result.status, 'active');
    assert.equal(released.result.kernel, true);
    assert.equal(released.result.receipt.status, 'succeeded');
    assert.equal(released.result.receipt.result.id, escrowId);
    assert.equal(released.result.receipt.result.status, 'released');
    assert.equal(released.result.receipt.event_ids.length, 1);
    assert.ok(released.result.receipt.audit_hash);
    assert.equal(disputed.result.kernel, true);
    assert.equal(disputed.result.receipt.result.status, 'filed');
    assert.ok(disputed.result.receipt.audit_hash);
    assert.equal(evidence.result.kernel, true);
    assert.ok(evidence.result.receipt.result.content_hash.startsWith('sha256:'));
    assert.equal(resolved.result.kernel, true);
    assert.equal(resolved.result.receipt.result.dispute.status, 'resolved');
    assert.equal(resolved.result.receipt.result.escrow.status, 'resolved');
    assert.equal(resolved.result.receipt.result.dispute.buyer_amount, '0.100001');
    assert.equal(resolved.result.receipt.result.dispute.seller_amount, '0.150000');
    assert.ok(resolved.result.receipt.audit_hash);
  });
});
