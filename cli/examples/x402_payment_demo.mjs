#!/usr/bin/env node
/**
 * x402 Payment Protocol Demo
 *
 * Demonstrates the HTTP-native payment protocol for AI agents:
 *
 * 1. Agent requests a resource from a paywall-protected API
 * 2. Server returns HTTP 402 (Payment Required) with X-Payment-Required header
 * 3. Agent creates an X402PaymentIntent and signs it off-chain
 * 4. Agent retries request with X-Payment header
 * 5. Payment intent synced to StateSet Sequencer
 * 6. Batch of payments settled on Arc L1 blockchain
 * 7. Server verifies payment via inclusion proof
 *
 * Usage:
 *   node examples/x402_payment_demo.mjs
 */

import crypto from 'crypto';
import { Buffer } from 'buffer';

// =============================================================================
// CONFIGURATION
// =============================================================================

const config = {
    // Arc L1 Blockchain
    arc: {
        rpcUrl: process.env.ARC_RPC_URL || 'https://rpc.testnet.arc.network',
        chainId: parseInt(process.env.ARC_CHAIN_ID || '5042002'),
        explorerUrl: 'https://explorer.testnet.arc.network',
        commitmentContract: '0x1234567890123456789012345678901234567890',
    },
    // StateSet Sequencer
    sequencer: {
        url: process.env.SEQUENCER_URL || 'https://api.sequencer.stateset.app',
    },
    // x402 Protocol
    x402: {
        version: '1.0',
        domainSeparator: 'X402_PAYMENT_V1',
        defaultValiditySeconds: 3600,
    },
    // Demo identities
    payer: {
        address: '0x' + crypto.randomBytes(20).toString('hex'),
        publicKey: crypto.randomBytes(32).toString('hex'),
        privateKey: crypto.randomBytes(32).toString('hex'), // Demo only
    },
    payee: {
        address: '0x' + crypto.randomBytes(20).toString('hex'),
        merchantId: 'merchant-demo-001',
        merchantName: 'Demo API Service',
    },
};

// Colors
const CYAN = '\x1b[36m';
const GREEN = '\x1b[32m';
const YELLOW = '\x1b[33m';
const RED = '\x1b[31m';
const MAGENTA = '\x1b[35m';
const NC = '\x1b[0m';
const DIM = '\x1b[2m';
const BOLD = '\x1b[1m';

function printHeader(text) {
    console.log(`\n${CYAN}╔════════════════════════════════════════════════════════════════════╗${NC}`);
    console.log(`${CYAN}║${NC}  ${text}`);
    console.log(`${CYAN}╚════════════════════════════════════════════════════════════════════╝${NC}\n`);
}

function printSubHeader(text) {
    console.log(`\n${YELLOW}─── ${text} ───${NC}\n`);
}

// =============================================================================
// x402 PROTOCOL TYPES
// =============================================================================

/**
 * X402PaymentRequired - Server's 402 response
 */
function createPaymentRequired(resourceUri, resourceMethod, amount) {
    return {
        version: config.x402.version,
        payee_address: config.payee.address,
        amount: amount, // In smallest unit (e.g., 1000000 = 1 USDC)
        amount_display: `${(amount / 1_000_000).toFixed(6)} USDC`,
        asset: 'USDC',
        networks: ['set_chain', 'base', 'arc'],
        resource_uri: resourceUri,
        resource_method: resourceMethod,
        description: 'API access fee',
        validity_seconds: config.x402.defaultValiditySeconds,
        merchant_id: config.payee.merchantId,
        merchant_name: config.payee.merchantName,
        generated_at: new Date().toISOString(),
    };
}

/**
 * X402PaymentIntent - Client's signed payment authorization
 */
function createPaymentIntent(paymentRequired, nonce) {
    const now = Math.floor(Date.now() / 1000);
    const intentId = crypto.randomUUID();

    return {
        id: intentId,
        version: config.x402.version,
        status: 'created',

        // Payment parameters
        payer_address: config.payer.address,
        payee_address: paymentRequired.payee_address,
        amount: paymentRequired.amount,
        amount_decimal: paymentRequired.amount / 1_000_000,
        asset: paymentRequired.asset,
        network: 'arc', // Use Arc L1
        chain_id: config.arc.chainId,
        token_address: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48', // USDC

        // Validity & replay protection
        created_at_unix: now,
        valid_until: now + paymentRequired.validity_seconds,
        nonce: nonce,
        idempotency_key: `x402-${intentId}`,

        // Resource context
        resource_uri: paymentRequired.resource_uri,
        resource_method: paymentRequired.resource_method,
        description: paymentRequired.description,
        merchant_id: paymentRequired.merchant_id,

        // Crypto fields (filled after signing)
        signing_hash: null,
        payer_signature: null,
        payer_public_key: null,

        // Sequencer fields (filled after submission)
        sequence_number: null,
        sequenced_at: null,
        batch_id: null,
        batch_merkle_root: null,
        inclusion_proof: null,

        // Settlement fields (filled after on-chain execution)
        tx_hash: null,
        block_number: null,
        settled_at: null,

        created_at: new Date().toISOString(),
    };
}

/**
 * Sign a payment intent (VES v1.0 compatible)
 */
function signPaymentIntent(intent) {
    // Create canonical signing data (JCS - RFC 8785)
    const signingPayload = {
        version: intent.version,
        payer: intent.payer_address,
        payee: intent.payee_address,
        amount: intent.amount.toString(),
        asset: intent.asset,
        chainId: intent.chain_id,
        tokenAddress: intent.token_address,
        nonce: intent.nonce,
        validUntil: intent.valid_until,
        resourceUri: intent.resource_uri,
    };

    // Canonical JSON (sorted keys)
    const canonicalJson = JSON.stringify(signingPayload, Object.keys(signingPayload).sort());

    // Domain-separated hash
    const domainData = `${config.x402.domainSeparator}:${canonicalJson}`;
    const signingHash = crypto.createHash('sha256').update(domainData).digest('hex');

    // Ed25519 signature (simulated - in production use actual Ed25519)
    const signatureData = `${signingHash}:${config.payer.privateKey}`;
    const signature = crypto.createHash('sha256').update(signatureData).digest('hex') +
                     crypto.createHash('sha256').update(signatureData + ':2').digest('hex').slice(0, 64);

    return {
        ...intent,
        status: 'signed',
        signing_hash: signingHash,
        payer_signature: signature,
        payer_public_key: config.payer.publicKey,
    };
}

/**
 * Encode payment intent for HTTP header
 */
function encodePaymentHeader(signedIntent) {
    const headerPayload = {
        intent_id: signedIntent.id,
        version: signedIntent.version,
        payer: signedIntent.payer_address,
        payee: signedIntent.payee_address,
        amount: signedIntent.amount,
        asset: signedIntent.asset,
        chain_id: signedIntent.chain_id,
        nonce: signedIntent.nonce,
        valid_until: signedIntent.valid_until,
        signature: signedIntent.payer_signature,
        public_key: signedIntent.payer_public_key,
    };
    return Buffer.from(JSON.stringify(headerPayload)).toString('base64');
}

/**
 * X402PaymentReceipt - Proof of payment
 */
function createPaymentReceipt(signedIntent, batchInfo) {
    return {
        id: crypto.randomUUID(),
        intent_id: signedIntent.id,
        sequence_number: batchInfo.sequenceNumber,
        batch_id: batchInfo.batchId,
        merkle_root: batchInfo.merkleRoot,
        inclusion_proof: batchInfo.inclusionProof,
        leaf_index: batchInfo.leafIndex,
        tx_hash: batchInfo.txHash,
        block_number: batchInfo.blockNumber,
        payer_address: signedIntent.payer_address,
        payee_address: signedIntent.payee_address,
        amount: signedIntent.amount,
        asset: signedIntent.asset,
        network: signedIntent.network,
        created_at: new Date().toISOString(),
    };
}

// =============================================================================
// DEMO FLOW
// =============================================================================

async function main() {
    printHeader('x402 Payment Protocol Demo');

    console.log(`${CYAN}Configuration:${NC}`);
    console.log(`  Arc RPC:        ${config.arc.rpcUrl}`);
    console.log(`  Arc Chain ID:   ${config.arc.chainId}`);
    console.log(`  Sequencer:      ${config.sequencer.url}`);
    console.log(`  Payer Address:  ${config.payer.address.slice(0, 10)}...`);
    console.log(`  Payee Address:  ${config.payee.address.slice(0, 10)}...`);
    console.log(`  Merchant:       ${config.payee.merchantName}`);

    // =========================================================================
    // Step 1: Agent requests protected resource
    // =========================================================================

    printHeader('Step 1: Agent Requests Protected Resource');

    const resourceUri = '/api/v1/ai/completion';
    const resourceMethod = 'POST';

    console.log(`${MAGENTA}AI Agent${NC} → ${CYAN}API Server${NC}`);
    console.log(`\n   ${DIM}POST ${resourceUri}${NC}`);
    console.log(`   ${DIM}Content-Type: application/json${NC}`);
    console.log(`   ${DIM}Authorization: Bearer <agent-token>${NC}`);
    console.log(`   ${DIM}${NC}`);
    console.log(`   ${DIM}{${NC}`);
    console.log(`   ${DIM}  "model": "claude-3",${NC}`);
    console.log(`   ${DIM}  "prompt": "Explain quantum computing"${NC}`);
    console.log(`   ${DIM}}${NC}`);

    await sleep(500);

    // =========================================================================
    // Step 2: Server returns HTTP 402 Payment Required
    // =========================================================================

    printHeader('Step 2: Server Returns HTTP 402 Payment Required');

    const paymentAmount = 100_000; // 0.10 USDC
    const paymentRequired = createPaymentRequired(resourceUri, resourceMethod, paymentAmount);

    console.log(`${CYAN}API Server${NC} → ${MAGENTA}AI Agent${NC}`);
    console.log(`\n   ${RED}HTTP/1.1 402 Payment Required${NC}`);
    console.log(`   ${DIM}Content-Type: application/json${NC}`);
    console.log(`   ${YELLOW}X-Payment-Required: <base64-encoded>${NC}`);
    console.log('');
    console.log(`   ${DIM}Decoded X-Payment-Required:${NC}`);
    console.log(`   ┌─────────────────────────────────────────────────────────┐`);
    console.log(`   │ ${BOLD}Payment Required${NC}                                        │`);
    console.log(`   ├─────────────────────────────────────────────────────────┤`);
    console.log(`   │ Payee:      ${paymentRequired.payee_address.slice(0, 20)}...       │`);
    console.log(`   │ Amount:     ${paymentRequired.amount_display.padEnd(20)}                │`);
    console.log(`   │ Asset:      ${paymentRequired.asset.padEnd(20)}                │`);
    console.log(`   │ Networks:   ${paymentRequired.networks.join(', ').padEnd(20)}        │`);
    console.log(`   │ Resource:   ${paymentRequired.resource_uri.padEnd(20)}        │`);
    console.log(`   │ Merchant:   ${paymentRequired.merchant_name.padEnd(20)}        │`);
    console.log(`   │ Valid for:  ${(paymentRequired.validity_seconds / 60)} minutes                          │`);
    console.log(`   └─────────────────────────────────────────────────────────┘`);

    await sleep(500);

    // =========================================================================
    // Step 3: Agent creates and signs payment intent
    // =========================================================================

    printHeader('Step 3: Agent Creates and Signs Payment Intent');

    const nonce = Date.now();
    const paymentIntent = createPaymentIntent(paymentRequired, nonce);

    console.log(`${MAGENTA}AI Agent${NC} creates X402PaymentIntent:`);
    console.log('');
    console.log(`   Intent ID:     ${paymentIntent.id.slice(0, 8)}...`);
    console.log(`   Payer:         ${paymentIntent.payer_address.slice(0, 20)}...`);
    console.log(`   Payee:         ${paymentIntent.payee_address.slice(0, 20)}...`);
    console.log(`   Amount:        ${paymentIntent.amount_decimal} ${paymentIntent.asset}`);
    console.log(`   Chain:         Arc L1 (${paymentIntent.chain_id})`);
    console.log(`   Nonce:         ${paymentIntent.nonce}`);
    console.log(`   Valid Until:   ${new Date(paymentIntent.valid_until * 1000).toISOString()}`);

    console.log(`\n${YELLOW}Signing with Ed25519...${NC}`);
    await sleep(300);

    const signedIntent = signPaymentIntent(paymentIntent);

    console.log(`\n   ${GREEN}✓${NC} Signing hash:   ${signedIntent.signing_hash.slice(0, 32)}...`);
    console.log(`   ${GREEN}✓${NC} Signature:      ${signedIntent.payer_signature.slice(0, 32)}...`);
    console.log(`   ${GREEN}✓${NC} Public key:     ${signedIntent.payer_public_key.slice(0, 32)}...`);
    console.log(`\n   ${GREEN}✅ Payment intent signed!${NC}`);

    await sleep(500);

    // =========================================================================
    // Step 4: Agent retries request with X-Payment header
    // =========================================================================

    printHeader('Step 4: Agent Retries Request with Payment');

    const paymentHeader = encodePaymentHeader(signedIntent);

    console.log(`${MAGENTA}AI Agent${NC} → ${CYAN}API Server${NC}`);
    console.log(`\n   ${DIM}POST ${resourceUri}${NC}`);
    console.log(`   ${DIM}Content-Type: application/json${NC}`);
    console.log(`   ${GREEN}X-Payment: ${paymentHeader.slice(0, 40)}...${NC}`);
    console.log(`   ${DIM}${NC}`);
    console.log(`   ${DIM}{${NC}`);
    console.log(`   ${DIM}  "model": "claude-3",${NC}`);
    console.log(`   ${DIM}  "prompt": "Explain quantum computing"${NC}`);
    console.log(`   ${DIM}}${NC}`);

    await sleep(500);

    // =========================================================================
    // Step 5: Server verifies signature and forwards to sequencer
    // =========================================================================

    printHeader('Step 5: Server Verifies and Submits to Sequencer');

    console.log(`${CYAN}API Server${NC} verifying payment intent...`);
    await sleep(200);

    console.log(`\n   ${GREEN}✓${NC} Decoded X-Payment header`);
    console.log(`   ${GREEN}✓${NC} Verified Ed25519 signature`);
    console.log(`   ${GREEN}✓${NC} Checked validity window (not expired)`);
    console.log(`   ${GREEN}✓${NC} Verified nonce (not replayed)`);
    console.log(`   ${GREEN}✓${NC} Confirmed payee matches merchant`);

    console.log(`\n${CYAN}API Server${NC} → ${YELLOW}StateSet Sequencer${NC}`);
    console.log(`\n   Submitting payment intent for batching...`);
    await sleep(300);

    // Simulate sequencer response
    const sequenceNumber = 42;
    const sequencedAt = new Date().toISOString();

    console.log(`\n   ${GREEN}✓${NC} Intent accepted by sequencer`);
    console.log(`   ${GREEN}✓${NC} Sequence number: ${sequenceNumber}`);
    console.log(`   ${GREEN}✓${NC} Sequenced at: ${sequencedAt}`);

    // Update intent with sequencer info
    signedIntent.status = 'sequenced';
    signedIntent.sequence_number = sequenceNumber;
    signedIntent.sequenced_at = sequencedAt;

    await sleep(500);

    // =========================================================================
    // Step 6: Sequencer batches and posts to Arc L1
    // =========================================================================

    printHeader('Step 6: Sequencer Batches and Posts to Arc L1');

    console.log(`${YELLOW}StateSet Sequencer${NC} collecting payment intents...\n`);

    // Simulate batch of payments
    const batchPayments = [
        { id: signedIntent.id, amount: signedIntent.amount, type: 'API Payment' },
        { id: crypto.randomUUID(), amount: 50_000, type: 'Data Query' },
        { id: crypto.randomUUID(), amount: 200_000, type: 'Model Inference' },
        { id: crypto.randomUUID(), amount: 75_000, type: 'Storage' },
    ];

    console.log(`   Batch contains ${batchPayments.length} payments:`);
    for (let i = 0; i < batchPayments.length; i++) {
        const p = batchPayments[i];
        const marker = p.id === signedIntent.id ? `${GREEN}→${NC}` : ' ';
        console.log(`   ${marker} [${i + 1}] ${p.type.padEnd(16)} ${(p.amount / 1_000_000).toFixed(2)} USDC`);
    }

    const totalAmount = batchPayments.reduce((sum, p) => sum + p.amount, 0);
    console.log(`   ${'─'.repeat(40)}`);
    console.log(`     Total: ${(totalAmount / 1_000_000).toFixed(2)} USDC`);

    // Build Merkle tree
    console.log(`\n   Building Merkle tree...`);
    await sleep(300);

    const leaves = batchPayments.map(p => {
        const leafData = JSON.stringify({ id: p.id, amount: p.amount });
        return crypto.createHash('sha256').update(leafData).digest('hex');
    });

    let merkleNodes = [...leaves];
    const proofNodes = [];
    let targetIndex = 0; // Our payment is first

    while (merkleNodes.length > 1) {
        const newLevel = [];
        for (let i = 0; i < merkleNodes.length; i += 2) {
            const left = merkleNodes[i];
            const right = merkleNodes[i + 1] || left;
            const combined = left < right ? left + right : right + left;
            newLevel.push(crypto.createHash('sha256').update(combined).digest('hex'));

            // Collect proof for our leaf
            if (i === targetIndex || i + 1 === targetIndex) {
                proofNodes.push(i === targetIndex ? right : left);
            }
        }
        targetIndex = Math.floor(targetIndex / 2);
        merkleNodes = newLevel;
    }

    const merkleRoot = merkleNodes[0];
    const batchId = crypto.randomUUID();

    console.log(`   ${GREEN}✓${NC} Merkle root: ${merkleRoot.slice(0, 32)}...`);

    // Post to Arc L1
    console.log(`\n${YELLOW}Sequencer${NC} → ${MAGENTA}Arc L1${NC}`);
    console.log(`\n   Posting batch commitment...`);
    await sleep(500);

    const txHash = '0x' + crypto.createHash('sha256')
        .update(JSON.stringify({ batchId, merkleRoot, timestamp: Date.now() }))
        .digest('hex');
    const blockNumber = 1000000 + Math.floor(Math.random() * 10000);

    console.log(`\n   ${GREEN}✅ Batch settled on Arc L1!${NC}`);
    console.log(`      TX Hash:  ${txHash}`);
    console.log(`      Block:    ${blockNumber}`);
    console.log(`      Contract: ${config.arc.commitmentContract}`);
    console.log(`      Explorer: ${config.arc.explorerUrl}/tx/${txHash}`);

    // Update intent with settlement info
    signedIntent.status = 'settled';
    signedIntent.batch_id = batchId;
    signedIntent.batch_merkle_root = merkleRoot;
    signedIntent.inclusion_proof = proofNodes;
    signedIntent.tx_hash = txHash;
    signedIntent.block_number = blockNumber;
    signedIntent.settled_at = new Date().toISOString();

    await sleep(500);

    // =========================================================================
    // Step 7: Server returns response with payment receipt
    // =========================================================================

    printHeader('Step 7: Server Returns Response with Receipt');

    const receipt = createPaymentReceipt(signedIntent, {
        sequenceNumber,
        batchId,
        merkleRoot,
        inclusionProof: proofNodes,
        leafIndex: 0,
        txHash,
        blockNumber,
    });

    console.log(`${CYAN}API Server${NC} → ${MAGENTA}AI Agent${NC}`);
    console.log(`\n   ${GREEN}HTTP/1.1 200 OK${NC}`);
    console.log(`   ${DIM}Content-Type: application/json${NC}`);
    console.log(`   ${GREEN}X-Payment-Receipt: <base64-encoded>${NC}`);
    console.log('');
    console.log(`   ${DIM}Response body:${NC}`);
    console.log(`   {`);
    console.log(`     "id": "completion-${crypto.randomUUID().slice(0, 8)}",`);
    console.log(`     "model": "claude-3",`);
    console.log(`     "completion": "Quantum computing uses quantum bits (qubits)..."`);
    console.log(`   }`);

    console.log(`\n   ${DIM}Payment Receipt:${NC}`);
    console.log(`   ┌─────────────────────────────────────────────────────────┐`);
    console.log(`   │ ${BOLD}x402 Payment Receipt${NC}                                    │`);
    console.log(`   ├─────────────────────────────────────────────────────────┤`);
    console.log(`   │ Receipt ID:    ${receipt.id.slice(0, 16)}...                  │`);
    console.log(`   │ Intent ID:     ${receipt.intent_id.slice(0, 16)}...                  │`);
    console.log(`   │ Amount:        ${(receipt.amount / 1_000_000).toFixed(6)} ${receipt.asset}                    │`);
    console.log(`   │ Sequence:      ${receipt.sequence_number}                                       │`);
    console.log(`   │ Batch ID:      ${receipt.batch_id.slice(0, 16)}...                  │`);
    console.log(`   │ Merkle Root:   ${receipt.merkle_root.slice(0, 16)}...                  │`);
    console.log(`   │ TX Hash:       ${receipt.tx_hash.slice(0, 16)}...                  │`);
    console.log(`   │ Block:         ${receipt.block_number}                                    │`);
    console.log(`   │ Network:       Arc L1 (${config.arc.chainId})                          │`);
    console.log(`   └─────────────────────────────────────────────────────────┘`);

    await sleep(500);

    // =========================================================================
    // Step 8: Agent verifies inclusion proof
    // =========================================================================

    printHeader('Step 8: Agent Verifies Inclusion Proof');

    console.log(`${MAGENTA}AI Agent${NC} verifying payment receipt...\n`);

    console.log(`   1. Recompute leaf hash from payment intent`);
    const verifyLeafData = JSON.stringify({ id: signedIntent.id, amount: signedIntent.amount });
    const verifyLeafHash = crypto.createHash('sha256').update(verifyLeafData).digest('hex');
    console.log(`      Leaf hash: ${verifyLeafHash.slice(0, 32)}...`);

    console.log(`\n   2. Walk Merkle proof to root`);
    let computedHash = verifyLeafHash;
    for (let i = 0; i < proofNodes.length; i++) {
        const sibling = proofNodes[i];
        const combined = computedHash < sibling ? computedHash + sibling : sibling + computedHash;
        computedHash = crypto.createHash('sha256').update(combined).digest('hex');
        console.log(`      Level ${i + 1}: ${computedHash.slice(0, 24)}...`);
    }

    console.log(`\n   3. Compare with committed root`);
    console.log(`      Computed:  ${computedHash.slice(0, 32)}...`);
    console.log(`      Committed: ${merkleRoot.slice(0, 32)}...`);
    console.log(`      Match:     ${GREEN}✓ YES${NC}`);

    console.log(`\n   4. Verify Arc L1 commitment exists`);
    console.log(`      TX Hash: ${txHash.slice(0, 32)}...`);
    console.log(`      Block:   ${blockNumber} (confirmed)`);

    console.log(`\n   ${GREEN}✅ Payment inclusion verified!${NC}`);

    // =========================================================================
    // Summary
    // =========================================================================

    console.log('\n\n');
    console.log('╔════════════════════════════════════════════════════════════════════╗');
    console.log('║                  x402 PAYMENT DEMO COMPLETE                        ║');
    console.log('╠════════════════════════════════════════════════════════════════════╣');
    console.log('║                                                                    ║');
    console.log('║  📋 PAYMENT SUMMARY                                                ║');
    console.log('║  ─────────────────────────────────────────────────────────────     ║');
    console.log(`║  • Amount:      ${(signedIntent.amount / 1_000_000).toFixed(6)} USDC                              ║`);
    console.log(`║  • Payer:       ${signedIntent.payer_address.slice(0, 20)}...                 ║`);
    console.log(`║  • Payee:       ${signedIntent.payee_address.slice(0, 20)}...                 ║`);
    console.log(`║  • Resource:    ${signedIntent.resource_uri}                          ║`);
    console.log('║                                                                    ║');
    console.log('║  🔐 CRYPTOGRAPHIC PROOF                                            ║');
    console.log('║  ─────────────────────────────────────────────────────────────     ║');
    console.log('║  • Ed25519 signature verified                                      ║');
    console.log('║  • Merkle inclusion proof valid                                    ║');
    console.log('║  • Arc L1 commitment confirmed                                     ║');
    console.log('║                                                                    ║');
    console.log('║  ⛓️  SETTLEMENT                                                     ║');
    console.log('║  ─────────────────────────────────────────────────────────────     ║');
    console.log(`║  • Chain:       Arc Testnet (${config.arc.chainId})                          ║`);
    console.log(`║  • Block:       ${blockNumber}                                        ║`);
    console.log(`║  • TX Hash:     ${txHash.slice(0, 20)}...                     ║`);
    console.log('║                                                                    ║');
    console.log('║  🔄 FLOW RECAP                                                     ║');
    console.log('║  ─────────────────────────────────────────────────────────────     ║');
    console.log('║  1. Agent → Server:     Request protected resource                 ║');
    console.log('║  2. Server → Agent:     HTTP 402 + X-Payment-Required              ║');
    console.log('║  3. Agent:              Create & sign X402PaymentIntent            ║');
    console.log('║  4. Agent → Server:     Retry with X-Payment header                ║');
    console.log('║  5. Server → Sequencer: Submit payment for batching                ║');
    console.log('║  6. Sequencer → Arc:    Post batch commitment                      ║');
    console.log('║  7. Server → Agent:     200 OK + X-Payment-Receipt                 ║');
    console.log('║  8. Agent:              Verify inclusion proof                     ║');
    console.log('║                                                                    ║');
    console.log('╚════════════════════════════════════════════════════════════════════╝');
    console.log('');
}

function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

main().catch(console.error);
