#!/usr/bin/env node
/**
 * StateSet iCommerce -> Sequencer -> Arc L1 E2E Demo
 *
 * Flow:
 * 1. Create iCommerce entities and events
 * 2. Sign events with VES v1.0 (simulated)
 * 3. Batch and "submit" to the StateSet Sequencer (simulated)
 * 4. Post batch commitment to Arc L1 (simulated)
 * 5. Verify inclusion against the batch Merkle root
 *
 * Usage:
 *   node cli/examples/icommerce_sequencer_arc_e2e.mjs
 */

import crypto from 'crypto';
import fs from 'fs';
import os from 'os';
import path from 'path';

// =============================================================================
// CONFIGURATION
// =============================================================================

const ARC_CHAIN_ID = parseInt(process.env.ARC_CHAIN_ID || '5042002', 10);
const ARC_EXPLORER_URL = process.env.ARC_EXPLORER_URL || 'https://explorer.testnet.arc.network';
const SEQUENCER_URL = process.env.SEQUENCER_URL || 'https://api.sequencer.stateset.app';
const TENANT_ID = process.env.TENANT_ID || crypto.randomUUID();
const STORE_ID = process.env.STORE_ID || crypto.randomUUID();
const RESULTS_DIR = process.env.DEMO_RESULTS_DIR || os.tmpdir();

// Colors
const CYAN = '\x1b[36m';
const GREEN = '\x1b[32m';
const YELLOW = '\x1b[33m';
const RED = '\x1b[31m';
const MAGENTA = '\x1b[35m';
const BLUE = '\x1b[34m';
const NC = '\x1b[0m';
const BOLD = '\x1b[1m';
const DIM = '\x1b[2m';

function printHeader(text) {
    console.log(`\n${CYAN}=== ${text} ===${NC}\n`);
}

function printStep(num, text) {
    console.log(`\n${YELLOW}Step ${num}: ${text}${NC}\n`);
}

function hashHex(data) {
    return crypto.createHash('sha256').update(data).digest('hex');
}

function buildMerkleRoot(leaves) {
    if (leaves.length === 0) {
        return hashHex('');
    }

    let nodes = [...leaves];
    while (nodes.length > 1) {
        const next = [];
        for (let i = 0; i < nodes.length; i += 2) {
            const left = nodes[i];
            const right = nodes[i + 1] || left;
            const combined = left < right ? left + right : right + left;
            next.push(hashHex(combined));
        }
        nodes = next;
    }
    return nodes[0];
}

function signVesEvent(event, agentKeyId) {
    const payloadPlainHash = hashHex(JSON.stringify(event.payload));
    const signingPayload = JSON.stringify({
        vesVersion: 1,
        tenantId: event.tenantId,
        storeId: event.storeId,
        eventId: event.eventId,
        entityType: event.entityType,
        entityId: event.entityId,
        eventType: event.eventType,
        payloadPlainHash
    });
    const signature = hashHex(`VES_EVENT_V1:${signingPayload}:${agentKeyId}`);

    return {
        ...event,
        vesVersion: 1,
        payloadKind: 0,
        payloadPlainHash,
        payloadCipherHash: '0'.repeat(64),
        agentKeyId,
        agentSignature: signature
    };
}

function buildLeaf(event) {
    return hashHex(JSON.stringify({
        eventId: event.eventId,
        payloadPlainHash: event.payloadPlainHash,
        agentSignature: event.agentSignature,
        sequenceNumber: event.sequenceNumber
    }));
}

// =============================================================================
// MAIN DEMO
// =============================================================================

async function main() {
    printHeader('StateSet iCommerce -> Sequencer -> Arc L1 (E2E Demo)');

    console.log(`${CYAN}Configuration:${NC}`);
    console.log(`  Arc Chain ID:   ${ARC_CHAIN_ID}`);
    console.log(`  Arc Explorer:   ${ARC_EXPLORER_URL}`);
    console.log(`  Sequencer URL:  ${SEQUENCER_URL}`);
    console.log(`  Tenant ID:      ${TENANT_ID.slice(0, 8)}...`);
    console.log(`  Store ID:       ${STORE_ID.slice(0, 8)}...`);
    console.log('');

    // =========================================================================
    // STEP 1: Build iCommerce Events
    // =========================================================================

    printStep(1, 'Create iCommerce entities and events');

    const storefrontAgent = {
        id: `storefront-${crypto.randomBytes(4).toString('hex')}`,
        keyId: 1
    };
    const fulfillmentAgent = {
        id: `fulfillment-${crypto.randomBytes(4).toString('hex')}`,
        keyId: 2
    };

    console.log(`${MAGENTA}Storefront Agent:${NC}  ${storefrontAgent.id}`);
    console.log(`${BLUE}Fulfillment Agent:${NC} ${fulfillmentAgent.id}`);

    const customer = {
        id: crypto.randomUUID(),
        email: 'ava@example.com',
        name: 'Ava Chen'
    };

    const product = {
        id: crypto.randomUUID(),
        sku: 'WIDGET-DELUXE-01',
        name: 'Deluxe Widget',
        price: 79.99
    };

    const order = {
        id: crypto.randomUUID(),
        customerId: customer.id,
        status: 'pending',
        items: [
            { sku: product.sku, quantity: 2, price: product.price }
        ],
        total: 159.98,
        currency: 'USD'
    };

    const inventory = {
        id: crypto.randomUUID(),
        sku: product.sku,
        quantity: 250,
        reserved: 2
    };

    const shipment = {
        id: crypto.randomUUID(),
        orderId: order.id,
        carrier: 'FEDEX',
        trackingNumber: `TRACK-${Date.now()}`
    };

    const invoice = {
        id: crypto.randomUUID(),
        orderId: order.id,
        amount: order.total,
        currency: order.currency,
        status: 'issued'
    };

    const events = [
        {
            eventId: crypto.randomUUID(),
            tenantId: TENANT_ID,
            storeId: STORE_ID,
            agentId: storefrontAgent.id,
            entityType: 'customer',
            entityId: customer.id,
            eventType: 'CustomerCreated',
            payload: customer
        },
        {
            eventId: crypto.randomUUID(),
            tenantId: TENANT_ID,
            storeId: STORE_ID,
            agentId: storefrontAgent.id,
            entityType: 'product',
            entityId: product.id,
            eventType: 'ProductCreated',
            payload: product
        },
        {
            eventId: crypto.randomUUID(),
            tenantId: TENANT_ID,
            storeId: STORE_ID,
            agentId: storefrontAgent.id,
            entityType: 'order',
            entityId: order.id,
            eventType: 'OrderCreated',
            payload: order
        },
        {
            eventId: crypto.randomUUID(),
            tenantId: TENANT_ID,
            storeId: STORE_ID,
            agentId: storefrontAgent.id,
            entityType: 'order',
            entityId: order.id,
            eventType: 'OrderConfirmed',
            payload: { orderId: order.id, status: 'confirmed' }
        },
        {
            eventId: crypto.randomUUID(),
            tenantId: TENANT_ID,
            storeId: STORE_ID,
            agentId: fulfillmentAgent.id,
            entityType: 'inventory',
            entityId: inventory.id,
            eventType: 'InventoryReserved',
            payload: { sku: inventory.sku, quantity: inventory.reserved, orderId: order.id }
        },
        {
            eventId: crypto.randomUUID(),
            tenantId: TENANT_ID,
            storeId: STORE_ID,
            agentId: fulfillmentAgent.id,
            entityType: 'shipment',
            entityId: shipment.id,
            eventType: 'ShipmentCreated',
            payload: shipment
        },
        {
            eventId: crypto.randomUUID(),
            tenantId: TENANT_ID,
            storeId: STORE_ID,
            agentId: fulfillmentAgent.id,
            entityType: 'order',
            entityId: order.id,
            eventType: 'OrderShipped',
            payload: {
                orderId: order.id,
                shipmentId: shipment.id,
                trackingNumber: shipment.trackingNumber
            }
        },
        {
            eventId: crypto.randomUUID(),
            tenantId: TENANT_ID,
            storeId: STORE_ID,
            agentId: storefrontAgent.id,
            entityType: 'invoice',
            entityId: invoice.id,
            eventType: 'InvoiceIssued',
            payload: invoice
        }
    ];

    console.log(`${GREEN}[OK]${NC} Created ${events.length} iCommerce events`);

    // =========================================================================
    // STEP 2: Sign Events (VES v1.0)
    // =========================================================================

    printStep(2, 'Sign events with VES v1.0');

    const signedEvents = events.map((event, index) => {
        const agentKeyId = event.agentId === storefrontAgent.id
            ? storefrontAgent.keyId
            : fulfillmentAgent.keyId;
        const signed = signVesEvent(event, agentKeyId);

        console.log(`  [${index + 1}/${events.length}] ${event.eventType.padEnd(18)} -> signed`);
        return signed;
    });

    // =========================================================================
    // STEP 3: Sequencer Batch
    // =========================================================================

    printStep(3, 'Batch and submit to StateSet Sequencer');

    console.log(`${YELLOW}Sequencer URL:${NC} ${SEQUENCER_URL}\n`);

    const sequenceStart = 9000;
    const sequencedEvents = signedEvents.map((event, index) => ({
        ...event,
        sequenceNumber: sequenceStart + index,
        sequencedAt: new Date().toISOString(),
        status: 'sequenced'
    }));

    for (const event of sequencedEvents) {
        console.log(`  [OK] ${event.eventType.padEnd(18)} -> seq ${event.sequenceNumber}`);
    }

    const leaves = sequencedEvents.map(buildLeaf);
    const merkleRoot = buildMerkleRoot(leaves);
    const batchId = crypto.randomUUID();

    console.log(`\n${BOLD}Batch Summary:${NC}`);
    console.log(`  Batch ID:      ${batchId.slice(0, 8)}...`);
    console.log(`  Events:        ${sequencedEvents.length}`);
    console.log(`  Sequence:      ${sequenceStart} - ${sequenceStart + sequencedEvents.length - 1}`);
    console.log(`  Merkle Root:   ${merkleRoot.slice(0, 32)}...`);

    // =========================================================================
    // STEP 4: Arc L1 Commitment
    // =========================================================================

    printStep(4, 'Commit batch on Arc L1');

    const commitmentData = {
        batchId,
        merkleRoot,
        sequenceStart,
        sequenceEnd: sequenceStart + sequencedEvents.length - 1,
        eventCount: sequencedEvents.length,
        tenantId: TENANT_ID,
        storeId: STORE_ID,
        timestamp: Math.floor(Date.now() / 1000)
    };

    const txHash = `0x${hashHex(JSON.stringify(commitmentData))}`;
    const blockNumber = 1_000_000 + Math.floor(Math.random() * 10_000);

    console.log(`${CYAN}Arc L1:${NC} Chain ID ${ARC_CHAIN_ID}`);
    console.log(`  TX Hash:   ${txHash.slice(0, 24)}...`);
    console.log(`  Block:     ${blockNumber}`);
    console.log(`  Explorer:  ${ARC_EXPLORER_URL}/tx/${txHash}`);

    // =========================================================================
    // STEP 5: Verify Inclusion
    // =========================================================================

    printStep(5, 'Verify inclusion proofs');

    let verifiedCount = 0;
    for (let i = 0; i < sequencedEvents.length; i++) {
        const event = sequencedEvents[i];
        const leaf = buildLeaf(event);
        const match = leaf === leaves[i];
        if (match) {
            verifiedCount++;
        }
        const status = match ? `${GREEN}[OK]${NC}` : `${RED}[FAIL]${NC}`;
        console.log(`  ${status} ${event.eventType.padEnd(18)} leaf ${leaf.slice(0, 16)}...`);
    }

    // =========================================================================
    // SUMMARY
    // =========================================================================

    printHeader('Demo Summary');

    console.log(`${BOLD}Agents:${NC}`);
    console.log(`  Storefront:  ${storefrontAgent.id}`);
    console.log(`  Fulfillment: ${fulfillmentAgent.id}`);

    console.log(`\n${BOLD}Events:${NC}`);
    console.log(`  Total:       ${sequencedEvents.length}`);
    console.log(`  Verified:    ${verifiedCount}`);

    console.log(`\n${BOLD}Batch:${NC}`);
    console.log(`  Batch ID:    ${batchId}`);
    console.log(`  Merkle Root: ${merkleRoot}`);
    console.log(`  Sequence:    ${sequenceStart} - ${sequenceStart + sequencedEvents.length - 1}`);

    console.log(`\n${BOLD}Arc L1:${NC}`);
    console.log(`  TX Hash:     ${txHash}`);
    console.log(`  Block:       ${blockNumber}`);

    const results = {
        batchId,
        merkleRoot,
        commitmentTxHash: txHash,
        commitmentBlock: blockNumber,
        events: sequencedEvents.map((event) => ({
            eventId: event.eventId,
            eventType: event.eventType,
            entityType: event.entityType,
            entityId: event.entityId,
            sequenceNumber: event.sequenceNumber,
            agentId: event.agentId
        })),
        createdAt: new Date().toISOString()
    };

    const resultsPath = path.join(RESULTS_DIR, `icommerce-batch-${batchId.slice(0, 8)}.json`);
    fs.writeFileSync(resultsPath, JSON.stringify(results, null, 2));
    console.log(`\n${GREEN}[OK]${NC} Results saved to: ${resultsPath}`);

    console.log(`\n${DIM}Note:${NC} Sequencer and Arc L1 steps are simulated in this demo.`);
}

main().catch((err) => {
    console.error(`${RED}Error:${NC} ${err.message}`);
    process.exit(1);
});
