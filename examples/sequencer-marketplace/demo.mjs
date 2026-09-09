#!/usr/bin/env node

import assert from 'node:assert/strict';
import { createHash, generateKeyPairSync, randomUUID } from 'node:crypto';

import {
  KernelMarketplaceBridge,
  MemoryBridgeStore,
  createAwardCommandPlanner,
  signMarketplaceMessage,
  verifyMarketplaceMessage,
} from '../../cli/src/marketplace/kernel-bridge.js';

const TENANT_ID = process.env.STATESET_TENANT_ID ?? '00000000-0000-0000-0000-000000000001';
const STORE_ID = process.env.STATESET_STORE_ID ?? '00000000-0000-0000-0000-000000000001';
const SEQUENCER_URL = (process.env.STATESET_SEQUENCER_URL ?? 'http://localhost:8080').replace(
  /\/$/,
  '',
);
const API_KEY = process.env.STATESET_SEQUENCER_API_KEY ?? 'dev_admin_key';
const AUCTION_ID = `auction:${randomUUID()}`;

const AGENTS = Object.freeze({
  buyer: { id: '10000000-0000-4000-8000-000000000001', name: 'buyer.acme.procurement' },
  alpha: { id: '20000000-0000-4000-8000-000000000001', name: 'merchant.alpha' },
  beta: { id: '30000000-0000-4000-8000-000000000001', name: 'merchant.beta' },
  gamma: { id: '40000000-0000-4000-8000-000000000001', name: 'merchant.gamma' },
  payment: { id: '50000000-0000-4000-8000-000000000001', name: 'payment.settler' },
});
const AGENT_KEYS = new Map(
  Object.values(AGENTS).map((agent) => [agent.id, generateKeyPairSync('ed25519')]),
);

function signedMessage(agent, message) {
  return signMarketplaceMessage(message, AGENT_KEYS.get(agent.id).privateKey, `${agent.id}:1`);
}

function assertSignedMessage(agent, message) {
  assert.equal(
    verifyMarketplaceMessage(message, AGENT_KEYS.get(agent.id).publicKey),
    true,
    `invalid signature from ${agent.name}`,
  );
}

function canonicalize(value) {
  if (Array.isArray(value)) return `[${value.map(canonicalize).join(',')}]`;
  if (value !== null && typeof value === 'object') {
    return `{${Object.keys(value)
      .sort()
      .map((key) => `${JSON.stringify(key)}:${canonicalize(value[key])}`)
      .join(',')}}`;
  }
  return JSON.stringify(value);
}

function hashPayload(payload) {
  return createHash('sha256').update(canonicalize(payload)).digest('hex');
}

function money(amount, currency = 'USD') {
  assert.match(amount, /^(0|[1-9]\d*)\.\d{2}$/, 'money must be an exact decimal string');
  return { amount, currency };
}

function minorUnits({ amount }) {
  const [whole, fraction] = amount.split('.');
  return BigInt(whole) * 100n + BigInt(fraction);
}

class SequencerBoard {
  constructor({ url = SEQUENCER_URL, apiKey = API_KEY } = {}) {
    this.url = url;
    this.apiKey = apiKey;
  }

  async request(path, options = {}) {
    const response = await fetch(`${this.url}${path}`, {
      ...options,
      headers: {
        Authorization: `ApiKey ${this.apiKey}`,
        'Content-Type': 'application/json',
        ...options.headers,
      },
    });
    const text = await response.text();
    const body = text ? JSON.parse(text) : null;
    if (!response.ok) {
      throw new Error(`${options.method ?? 'GET'} ${path} failed (${response.status}): ${text}`);
    }
    return body;
  }

  async health() {
    const response = await fetch(`${this.url}/health`);
    if (!response.ok) throw new Error(`sequencer health check failed (${response.status})`);
  }

  async publish(agent, eventType, payload, { inReplyTo = null, messageId = randomUUID() } = {}) {
    const message = signedMessage(agent, {
      protocol: 'stateset.marketplace.v1',
      message_id: messageId,
      conversation_id: AUCTION_ID,
      in_reply_to: inReplyTo,
      from: agent.name,
      sent_at: new Date().toISOString(),
      ...payload,
    });
    assertSignedMessage(agent, message);
    const event = {
      envelope_version: 1,
      event_id: messageId,
      command_id: messageId,
      tenant_id: TENANT_ID,
      store_id: STORE_ID,
      entity_type: 'marketplace.negotiation',
      entity_id: AUCTION_ID,
      event_type: eventType,
      payload: message,
      payload_hash: hashPayload(message),
      created_at: message.sent_at,
      source_agent: agent.id,
    };
    const receipt = await this.request('/api/v1/events/ingest', {
      method: 'POST',
      body: JSON.stringify({ agent_id: agent.id, events: [event] }),
    });
    if (receipt.events_accepted !== 1) {
      throw new Error(`sequencer rejected ${eventType}: ${JSON.stringify(receipt.rejections)}`);
    }
    return { message, sequence: receipt.assigned_sequence_start };
  }

  async read(from = 1) {
    const query = new URLSearchParams({
      tenant_id: TENANT_ID,
      store_id: STORE_ID,
      from: String(from),
      limit: '1000',
    });
    const result = await this.request(`/api/v1/events?${query}`);
    return result.events.filter(
      (event) =>
        event.envelope.entity_type === 'marketplace.negotiation' &&
        event.envelope.entity_id === AUCTION_ID,
    );
  }

  async pull(from = 1, limit = 100) {
    const events = (await this.read(from)).slice(0, limit);
    return { events, headSequence: events.at(-1)?.envelope.sequence_number ?? from - 1 };
  }
}

class MemoryBoard {
  constructor() {
    this.events = [];
  }

  async health() {}

  async publish(agent, eventType, payload, { inReplyTo = null, messageId = randomUUID() } = {}) {
    const message = signedMessage(agent, {
      protocol: 'stateset.marketplace.v1',
      message_id: messageId,
      conversation_id: AUCTION_ID,
      in_reply_to: inReplyTo,
      from: agent.name,
      sent_at: new Date().toISOString(),
      ...payload,
    });
    assertSignedMessage(agent, message);
    const sequence = this.events.length + 1;
    this.events.push({
      envelope: {
        entity_type: 'marketplace.negotiation',
        entity_id: AUCTION_ID,
        event_type: eventType,
        payload: message,
        event_id: message.message_id,
        tenant_id: TENANT_ID,
        store_id: STORE_ID,
        source_agent: agent.id,
        created_at: message.sent_at,
        sequence_number: sequence,
      },
    });
    return { message, sequence };
  }

  async read(from = 1) {
    return this.events.slice(Math.max(0, from - 1));
  }

  async pull(from = 1, limit = 100) {
    const events = (await this.read(from)).slice(0, limit);
    return { events, headSequence: this.events.length };
  }
}

function kernelPolicy(version, command, capability, extra = {}) {
  return {
    version,
    commands: {
      [command]: {
        required_capabilities: [capability],
        ...extra,
      },
    },
    trusted_authority_keys: {},
  };
}

async function executeAwardThroughKernel(board, fromSequence) {
  const { Commerce } = await import('../../bindings/node/index.js');
  const buyerCommerce = new Commerce(':memory:');
  const merchantCommerce = new Commerce(':memory:');
  await merchantCommerce.inventory.createItem({
    sku: 'SKU-100',
    name: 'Marketplace inventory',
    initialQuantity: 100,
  });

  const registry = new Map(
    Object.values(AGENTS).map((agent) => [
      agent.id,
      { id: agent.id, name: agent.name, publicKey: AGENT_KEYS.get(agent.id).publicKey },
    ]),
  );
  const run = async ({ id, agent, commerce, policy, side }) => {
    const published = [];
    const bridge = new KernelMarketplaceBridge({
      id,
      sequencer: board,
      commerce,
      store: new MemoryBridgeStore(),
      identity: {
        id: agent.id,
        principalId: side === 'buyer' ? 'company:acme' : 'company:merchant-beta',
        tenantId: TENANT_ID,
        storeId: STORE_ID,
        capabilities: Object.keys(policy.commands),
      },
      policy,
      registry,
      planner: createAwardCommandPlanner({ side }),
      async publishReceipt({ publicationId, source, command, receipt }) {
        published.push(receipt);
        await board.publish(
          agent,
          'economic.kernel_receipt.issued',
          {
            kind: 'kernel_receipt',
            to: [source.sourceAgent],
            source_event_id: source.eventId,
            command_type: command.command_type,
            kernel_receipt: receipt,
          },
          { inReplyTo: source.eventId, messageId: publicationId },
        );
      },
    });
    bridge.store.advance(id, fromSequence);
    const result = await bridge.pollOnce();
    return { result, published };
  };

  const buyer = await run({
    id: 'demo-buyer-kernel-worker',
    agent: AGENTS.buyer,
    commerce: buyerCommerce,
    side: 'buyer',
    policy: kernelPolicy('procurement-authority-v4', 'a2a.escrow.create', 'a2a.escrow.create', {
      max_asset_amount: { amount: '5000.00', asset: 'USDC' },
    }),
  });
  const merchant = await run({
    id: 'demo-merchant-kernel-worker',
    agent: AGENTS.beta,
    commerce: merchantCommerce,
    side: 'merchant',
    policy: kernelPolicy('merchant-fulfillment-v1', 'inventory.reserve', 'inventory.reserve', {
      max_quantity: '50',
    }),
  });
  const stock = await merchantCommerce.inventory.getStock('SKU-100');
  assert.equal(buyer.published[0]?.status, 'succeeded');
  assert.equal(merchant.published[0]?.status, 'succeeded');
  assert.equal(stock.totalAllocated, '50');
  return {
    buyerReceiptId: buyer.published[0].receipt_id,
    merchantReceiptId: merchant.published[0].receipt_id,
    escrowId: buyer.published[0].aggregate_id,
    reservationId: merchant.published[0].aggregate_id,
    inventoryAllocated: stock.totalAllocated,
  };
}

function chooseBid(bids, constraints) {
  const eligible = bids.filter(
    (bid) =>
      BigInt(bid.unit_count) >= BigInt(constraints.quantity) &&
      minorUnits(bid.total) <= minorUnits(constraints.max_total) &&
      bid.delivery_date <= constraints.deliver_by,
  );
  return eligible.sort((left, right) => {
    const leftPrice = minorUnits(left.total);
    const rightPrice = minorUnits(right.total);
    return leftPrice < rightPrice
      ? -1
      : leftPrice > rightPrice
        ? 1
        : left.delivery_date.localeCompare(right.delivery_date);
  })[0];
}

async function runMarketplace(board, { print = true, executeKernel = false } = {}) {
  const transcript = [];
  const emit = async (agent, type, payload, options) => {
    const entry = await board.publish(agent, type, payload, options);
    transcript.push({ sequence: entry.sequence, type, from: agent.name, ...entry.message });
    if (print) {
      console.log(`${String(entry.sequence).padStart(3, '0')}  ${agent.name.padEnd(24)} ${type}`);
    }
    return entry;
  };

  await board.health();
  const constraints = {
    sku: 'SKU-100',
    quantity: 50,
    max_total: money('5000.00'),
    deliver_by: '2026-09-14',
  };
  const rfq = await emit(AGENTS.buyer, 'marketplace.rfq.opened', {
    kind: 'rfq',
    to: [AGENTS.alpha.name, AGENTS.beta.name, AGENTS.gamma.name],
    constraints,
    authority: {
      principal: 'company:acme',
      budget_id: 'budget:procurement:2026-09',
      autonomous_up_to: money('5000.00'),
    },
  });

  const offered = [
    [AGENTS.alpha, '4900.00', '2026-09-10'],
    [AGENTS.beta, '4650.00', '2026-09-12'],
    [AGENTS.gamma, '4500.00', '2026-09-18'],
  ];
  const bids = [];
  for (const [agent, amount, deliveryDate] of offered) {
    const bid = {
      kind: 'bid',
      bid_id: `bid:${randomUUID()}`,
      to: [AGENTS.buyer.name],
      sku: constraints.sku,
      unit_count: '50',
      total: money(amount),
      delivery_date: deliveryDate,
      terms: { incoterm: 'DAP', payment: 'USDC-on-acceptance' },
      expires_at: '2026-09-05T18:00:00Z',
    };
    const entry = await emit(agent, 'marketplace.bid.submitted', bid, {
      inReplyTo: rfq.message.message_id,
    });
    bids.push({ ...bid, message_id: entry.message.message_id });
  }

  const selected = chooseBid(bids, constraints);
  assert.equal(selected.total.amount, '4650.00', 'deadline should disqualify the cheapest bid');
  const counter = await emit(
    AGENTS.buyer,
    'marketplace.counteroffer.created',
    {
      kind: 'counteroffer',
      to: [AGENTS.beta.name],
      bid_id: selected.bid_id,
      requested_total: money('4550.00'),
      unchanged: ['sku', 'unit_count', 'delivery_date', 'terms'],
    },
    { inReplyTo: selected.message_id },
  );
  const accepted = await emit(
    AGENTS.beta,
    'marketplace.counteroffer.accepted',
    {
      kind: 'acceptance',
      to: [AGENTS.buyer.name],
      bid_id: selected.bid_id,
      accepted_total: money('4550.00'),
      delivery_date: selected.delivery_date,
    },
    { inReplyTo: counter.message.message_id },
  );
  const award = await emit(
    AGENTS.buyer,
    'marketplace.award.created',
    {
      kind: 'award',
      to: [AGENTS.beta.id, AGENTS.payment.id],
      bid_id: selected.bid_id,
      winner: AGENTS.beta.id,
      winner_name: AGENTS.beta.name,
      expires_at: new Date(Date.now() + 86_400_000).toISOString(),
      commitment: {
        amount: money('4550.00'),
        counterparty_id: AGENTS.beta.id,
        quantity: '50',
        asset: constraints.sku,
      },
      settlement: {
        asset_amount: { amount: '4550.00', asset: 'USDC' },
        network: 'set_chain',
        buyer_address: 'wallet:acme-procurement',
        seller_address: 'wallet:merchant-beta',
      },
      policy_decision: 'approved',
      policy_id: 'procurement-authority-v4',
    },
    { inReplyTo: accepted.message.message_id },
  );
  const reservation = await emit(
    AGENTS.beta,
    'commerce.inventory.reserved',
    {
      kind: 'inventory_reservation',
      to: [AGENTS.buyer.name],
      reservation_id: `reservation:${randomUUID()}`,
      sku: constraints.sku,
      quantity: '50',
      status: 'reserved',
    },
    { inReplyTo: award.message.message_id },
  );
  const settlement = await emit(
    AGENTS.payment,
    'commerce.settlement.authorized',
    {
      kind: 'settlement_authorization',
      to: [AGENTS.buyer.name, AGENTS.beta.name],
      rail: 'x402',
      asset_amount: { amount: '4550.00', asset: 'USDC' },
      authorization_id: `x402:${randomUUID()}`,
      status: 'authorized',
    },
    { inReplyTo: award.message.message_id },
  );
  const order = await emit(
    AGENTS.beta,
    'commerce.order.created',
    {
      kind: 'order',
      to: [AGENTS.buyer.name],
      order_id: `order:${randomUUID()}`,
      reservation_id: reservation.message.reservation_id,
      settlement_id: settlement.message.authorization_id,
      sku: constraints.sku,
      quantity: '50',
      total: money('4550.00'),
      status: 'confirmed',
    },
    { inReplyTo: award.message.message_id },
  );
  const receipt = await emit(
    AGENTS.payment,
    'economic.receipt.issued',
    {
      kind: 'economic_receipt',
      to: [AGENTS.buyer.name, AGENTS.beta.name],
      agent: AGENTS.buyer.name,
      principal: 'company:acme',
      counterparty: AGENTS.beta.name,
      intent: `buy 50 ${constraints.sku}`,
      amount: money('4550.00'),
      authority: 'procurement-authority-v4',
      decision: 'approved',
      settlement: settlement.message.authorization_id,
      transaction: order.message.order_id,
      proof: {
        negotiation_entity: AUCTION_ID,
        sequence_start: rfq.sequence,
      },
    },
    { inReplyTo: order.message.message_id },
  );
  const sequenced = await board.read(rfq.sequence);
  assert.equal(sequenced.length, 11);
  const sequenceNumbers = sequenced.map((event) => event.envelope.sequence_number);
  assert.ok(
    sequenceNumbers.every((value, index) => index === 0 || value > sequenceNumbers[index - 1]),
    'the negotiation must have one strict total order',
  );
  assert.equal(order.message.total.amount, '4550.00');
  assert.equal(receipt.message.transaction, order.message.order_id);
  const kernel = executeKernel ? await executeAwardThroughKernel(board, rfq.sequence) : null;

  return {
    auctionId: AUCTION_ID,
    agents: Object.values(AGENTS).map(({ name }) => name),
    eventCount: sequenced.length,
    sequenceRange: [rfq.sequence, receipt.sequence],
    winningMerchant: AGENTS.beta.name,
    winningTotal: order.message.total,
    orderId: order.message.order_id,
    settlementId: settlement.message.authorization_id,
    kernel,
    transcript,
  };
}

const memory =
  process.argv.includes('--self-test') || process.env.STATESET_TOOLKIT_OUTPUT === 'json';
const jsonOutput =
  process.argv.includes('--json') || process.env.STATESET_TOOLKIT_OUTPUT === 'json';
const summary = await runMarketplace(memory ? new MemoryBoard() : new SequencerBoard(), {
  print: !jsonOutput,
  executeKernel: process.argv.includes('--kernel'),
});
if (jsonOutput) {
  console.log(JSON.stringify(summary, null, 2));
} else {
  console.log('\nMarketplace complete');
  console.log(`  Auction:    ${summary.auctionId}`);
  console.log(`  Winner:     ${summary.winningMerchant}`);
  console.log(`  Total:      ${summary.winningTotal.amount} ${summary.winningTotal.currency}`);
  console.log(`  Order:      ${summary.orderId}`);
  console.log(`  Settlement: ${summary.settlementId}`);
  console.log(`  Proof:      sequencer events ${summary.sequenceRange.join('..')}`);
  if (summary.kernel) {
    console.log(`  Escrow:     ${summary.kernel.escrowId}`);
    console.log(`  Reservation:${summary.kernel.reservationId}`);
    console.log(`  Allocated:  ${summary.kernel.inventoryAllocated} units`);
  }
}
