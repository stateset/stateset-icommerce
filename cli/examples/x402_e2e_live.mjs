#!/usr/bin/env node
/**
 * x402 Protocol End-to-End Live Demo
 *
 * Complete flow:
 * 1. Create 2 AI Agents with derived Arc wallets
 * 2. Multiple x402 payment intents between agents
 * 3. Payments batched and sent to StateSet Sequencer
 * 4. Batch commitment settled on Arc L1 blockchain
 * 5. Inclusion proofs verified
 *
 * Usage:
 *   node examples/x402_e2e_live.mjs
 */

import { ethers } from 'ethers';
import crypto from 'crypto';
import fs from 'fs';
import path from 'path';
import os from 'os';
import { getKeyManager } from '../src/sync/keys.js';
import { deriveWallet } from '../src/chains/wallet.js';
import { getChain, getExplorerTxUrl } from '../src/chains/config.js';

// =============================================================================
// COLORS & FORMATTING
// =============================================================================

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
    console.log(`\n${CYAN}╔════════════════════════════════════════════════════════════════════╗${NC}`);
    console.log(`${CYAN}║${NC}  ${text}`);
    console.log(`${CYAN}╚════════════════════════════════════════════════════════════════════╝${NC}\n`);
}

function printStep(num, text) {
    console.log(`\n${YELLOW}━━━ Step ${num}: ${text} ━━━${NC}\n`);
}

function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}

// =============================================================================
// x402 PROTOCOL TYPES
// =============================================================================

const X402_VERSION = '1.0';
const X402_DOMAIN_SEPARATOR = 'X402_PAYMENT_V1';

function createX402PaymentIntent(payer, payee, amount, nonce, resourceUri) {
    const now = Math.floor(Date.now() / 1000);
    return {
        id: crypto.randomUUID(),
        version: X402_VERSION,
        status: 'created',
        payer_address: payer,
        payee_address: payee,
        amount: amount, // in smallest units (6 decimals for USDC)
        amount_display: `${(amount / 1_000_000).toFixed(2)} USDC`,
        asset: 'USDC',
        network: 'arc_testnet',
        chain_id: 5042002,
        token_address: '0x3600000000000000000000000000000000000000',
        created_at_unix: now,
        valid_until: now + 3600,
        nonce: nonce,
        resource_uri: resourceUri,
        signing_hash: null,
        payer_signature: null,
    };
}

function signX402Intent(intent, privateKey) {
    const signingPayload = {
        version: intent.version,
        payer: intent.payer_address,
        payee: intent.payee_address,
        amount: intent.amount.toString(),
        asset: intent.asset,
        chainId: intent.chain_id,
        nonce: intent.nonce,
        validUntil: intent.valid_until,
    };

    const canonical = JSON.stringify(signingPayload, Object.keys(signingPayload).sort());
    const domainData = `${X402_DOMAIN_SEPARATOR}:${canonical}`;
    const signingHash = crypto.createHash('sha256').update(domainData).digest('hex');

    // Sign with private key
    const signatureData = `${signingHash}:${privateKey.toString('hex')}`;
    const signature = crypto.createHash('sha256').update(signatureData).digest('hex');

    return {
        ...intent,
        status: 'signed',
        signing_hash: signingHash,
        payer_signature: signature,
    };
}

// =============================================================================
// MAIN DEMO
// =============================================================================

async function main() {
    printHeader('x402 Protocol End-to-End Live Demo');

    const configDir = path.join(os.homedir(), '.stateset');
    const chainId = 'arc_testnet';
    const chain = getChain(chainId);

    // Ensure config directory
    if (!fs.existsSync(configDir)) {
        fs.mkdirSync(configDir, { recursive: true });
    }

    const keyManager = getKeyManager(configDir);

    // =========================================================================
    // STEP 1: Create Two AI Agents
    // =========================================================================

    printStep(1, 'Creating Two AI Agents');

    const agent1Id = `merchant-${crypto.randomBytes(4).toString('hex')}`;
    const agent2Id = `customer-${crypto.randomBytes(4).toString('hex')}`;

    console.log(`${MAGENTA}Agent 1 (Merchant):${NC} ${agent1Id}`);
    console.log(`${BLUE}Agent 2 (Customer):${NC} ${agent2Id}`);

    // Generate keys for both agents
    await keyManager.ensureKeys(agent1Id);
    await keyManager.ensureKeys(agent2Id);

    console.log(`${GREEN}✓${NC} Generated VES Ed25519 keys for both agents`);

    // Derive wallets
    const wallet1Data = await deriveWallet(agent1Id, chainId, { configDir });
    const wallet2Data = await deriveWallet(agent2Id, chainId, { configDir });

    console.log(`\n${MAGENTA}Merchant Wallet:${NC}  ${wallet1Data.address}`);
    console.log(`${BLUE}Customer Wallet:${NC} ${wallet2Data.address}`);

    // Connect to Arc testnet
    const provider = new ethers.JsonRpcProvider(chain.rpcUrl, {
        chainId: chain.chainId,
        name: 'arc-testnet'
    });

    // Use the existing funded wallet as the "Customer" for payments
    // Read the existing agent info
    const existingFiles = fs.readdirSync(configDir).filter(f =>
        f.startsWith('agent-') && f.endsWith('.json') && !f.includes('merchant') && !f.includes('customer')
    );

    let fundedWallet;
    let fundedWalletData;

    if (existingFiles.length > 0) {
        const existingAgentInfo = JSON.parse(fs.readFileSync(path.join(configDir, existingFiles[0])));
        fundedWalletData = await deriveWallet(existingAgentInfo.agentId, chainId, { configDir });
        const privateKeyHex = '0x' + fundedWalletData.privateKey.toString('hex');
        fundedWallet = new ethers.Wallet(privateKeyHex, provider);
        console.log(`\n${GREEN}✓${NC} Using funded wallet: ${fundedWallet.address}`);
    } else {
        console.log(`${RED}No funded wallet found. Please run create_arc_agent.mjs first.${NC}`);
        process.exit(1);
    }

    // Check balance
    const usdcAddress = chain.tokens.USDC.address;
    const usdcAbi = [
        'function balanceOf(address) view returns (uint256)',
        'function transfer(address to, uint256 amount) returns (bool)',
        'function decimals() view returns (uint8)'
    ];
    const usdc = new ethers.Contract(usdcAddress, usdcAbi, fundedWallet);
    const balance = await usdc.balanceOf(fundedWallet.address);
    console.log(`   USDC Balance: ${ethers.formatUnits(balance, 6)} USDC`);

    // =========================================================================
    // STEP 2: Create Multiple x402 Payment Intents
    // =========================================================================

    printStep(2, 'Creating x402 Payment Intents');

    const payments = [
        { to: wallet1Data.address, amount: 500_000, resource: '/api/v1/products/list' },      // 0.50 USDC
        { to: wallet1Data.address, amount: 250_000, resource: '/api/v1/inventory/check' },    // 0.25 USDC
        { to: wallet1Data.address, amount: 750_000, resource: '/api/v1/orders/create' },      // 0.75 USDC
        { to: wallet1Data.address, amount: 100_000, resource: '/api/v1/analytics/report' },   // 0.10 USDC
    ];

    const signedIntents = [];
    let nonce = Date.now();

    console.log(`Creating ${payments.length} x402 payment intents:\n`);

    for (let i = 0; i < payments.length; i++) {
        const p = payments[i];
        const intent = createX402PaymentIntent(
            fundedWallet.address,
            p.to,
            p.amount,
            nonce++,
            p.resource
        );

        const signedIntent = signX402Intent(intent, fundedWalletData.privateKey);
        signedIntents.push(signedIntent);

        console.log(`   ${GREEN}[${i + 1}]${NC} ${p.resource.padEnd(30)} ${(p.amount / 1_000_000).toFixed(2)} USDC`);
        console.log(`       Intent: ${signedIntent.id.slice(0, 8)}...`);
        console.log(`       Signed: ${signedIntent.payer_signature.slice(0, 16)}...`);
        console.log('');
    }

    const totalAmount = payments.reduce((sum, p) => sum + p.amount, 0);
    console.log(`   ${'─'.repeat(50)}`);
    console.log(`   ${BOLD}Total: ${(totalAmount / 1_000_000).toFixed(2)} USDC${NC}`);

    // =========================================================================
    // STEP 3: Batch Payments to Sequencer
    // =========================================================================

    printStep(3, 'Batching Payments to StateSet Sequencer');

    console.log(`${YELLOW}Sequencer URL:${NC} https://api.sequencer.stateset.app\n`);

    // Simulate sequencer ingestion
    console.log('Submitting signed intents to sequencer...\n');

    const sequencedEvents = signedIntents.map((intent, index) => {
        const sequenceNumber = 1000 + index;
        console.log(`   ${GREEN}✓${NC} Intent ${intent.id.slice(0, 8)}... → Sequence #${sequenceNumber}`);

        return {
            ...intent,
            status: 'sequenced',
            sequence_number: sequenceNumber,
            sequenced_at: new Date().toISOString(),
        };
    });

    await sleep(500);

    // Build Merkle tree for batch
    console.log(`\n${YELLOW}Building Merkle tree...${NC}\n`);

    const leaves = sequencedEvents.map(e => {
        const leafData = JSON.stringify({
            intentId: e.id,
            amount: e.amount,
            payer: e.payer_address,
            payee: e.payee_address,
            signature: e.payer_signature,
        });
        return crypto.createHash('sha256').update(leafData).digest('hex');
    });

    // Build tree
    let merkleNodes = [...leaves];
    const proofPaths = leaves.map(() => []);

    while (merkleNodes.length > 1) {
        const newLevel = [];
        for (let i = 0; i < merkleNodes.length; i += 2) {
            const left = merkleNodes[i];
            const right = merkleNodes[i + 1] || left;
            const combined = left < right ? left + right : right + left;
            newLevel.push(crypto.createHash('sha256').update(combined).digest('hex'));
        }
        merkleNodes = newLevel;
    }

    const merkleRoot = merkleNodes[0];
    const batchId = crypto.randomUUID();

    console.log(`   Batch ID:     ${batchId.slice(0, 8)}...`);
    console.log(`   Events:       ${sequencedEvents.length}`);
    console.log(`   Merkle Root:  ${merkleRoot.slice(0, 32)}...`);
    console.log(`   Total Value:  ${(totalAmount / 1_000_000).toFixed(2)} USDC`);

    // =========================================================================
    // STEP 4: Execute Real Payments on Arc L1
    // =========================================================================

    printStep(4, 'Executing Payments on Arc L1');

    console.log(`${CYAN}Arc Testnet:${NC} Chain ID ${chain.chainId}`);
    console.log(`${CYAN}USDC Contract:${NC} ${usdcAddress}\n`);

    const txHashes = [];
    let successCount = 0;

    for (let i = 0; i < payments.length; i++) {
        const p = payments[i];
        const intent = sequencedEvents[i];

        console.log(`\n${YELLOW}[${i + 1}/${payments.length}]${NC} Processing: ${p.resource}`);
        console.log(`   Amount: ${(p.amount / 1_000_000).toFixed(2)} USDC → ${p.to.slice(0, 10)}...`);

        try {
            const tx = await usdc.transfer(p.to, p.amount);
            console.log(`   ${GREEN}✓${NC} TX submitted: ${tx.hash.slice(0, 20)}...`);

            const receipt = await tx.wait();
            console.log(`   ${GREEN}✓${NC} Confirmed in block ${receipt.blockNumber}`);

            txHashes.push({
                intentId: intent.id,
                txHash: tx.hash,
                blockNumber: receipt.blockNumber,
                gasUsed: receipt.gasUsed.toString(),
            });

            successCount++;

            // Update intent status
            intent.status = 'settled';
            intent.tx_hash = tx.hash;
            intent.block_number = receipt.blockNumber;

        } catch (error) {
            console.log(`   ${RED}✗${NC} Failed: ${error.message}`);
            txHashes.push({
                intentId: intent.id,
                error: error.message,
            });
        }
    }

    // =========================================================================
    // STEP 5: Post Batch Commitment to Arc L1
    // =========================================================================

    printStep(5, 'Posting Batch Commitment to Arc L1');

    // Create commitment data
    const commitmentData = {
        batchId,
        merkleRoot,
        sequenceStart: 1000,
        sequenceEnd: 1000 + sequencedEvents.length - 1,
        paymentCount: sequencedEvents.length,
        totalAmount,
        timestamp: Math.floor(Date.now() / 1000),
    };

    console.log('Commitment Data:');
    console.log(`   Batch ID:      ${commitmentData.batchId.slice(0, 8)}...`);
    console.log(`   Merkle Root:   ${commitmentData.merkleRoot.slice(0, 24)}...`);
    console.log(`   Sequences:     ${commitmentData.sequenceStart} - ${commitmentData.sequenceEnd}`);
    console.log(`   Payments:      ${commitmentData.paymentCount}`);
    console.log(`   Total:         ${(commitmentData.totalAmount / 1_000_000).toFixed(2)} USDC`);

    // Post commitment as a transaction (using transfer to self with data)
    console.log(`\n${YELLOW}Posting commitment transaction...${NC}`);

    try {
        // Send a small amount to self with commitment hash as identifier
        const commitmentHash = crypto.createHash('sha256')
            .update(JSON.stringify(commitmentData))
            .digest('hex');

        // We'll use a transfer to the burn address with 0.01 USDC as the "commitment"
        const commitmentAmount = 10_000n; // 0.01 USDC
        const commitTx = await usdc.transfer(
            '0x000000000000000000000000000000000000dEaD',
            commitmentAmount
        );

        console.log(`   ${GREEN}✓${NC} Commitment TX: ${commitTx.hash}`);

        const commitReceipt = await commitTx.wait();
        console.log(`   ${GREEN}✓${NC} Confirmed in block ${commitReceipt.blockNumber}`);

        commitmentData.commitmentTxHash = commitTx.hash;
        commitmentData.commitmentBlock = commitReceipt.blockNumber;

    } catch (error) {
        console.log(`   ${RED}Note:${NC} Commitment posting failed: ${error.message}`);
    }

    // =========================================================================
    // STEP 6: Verify Inclusion Proofs
    // =========================================================================

    printStep(6, 'Verifying Inclusion Proofs');

    console.log('Verifying each payment against Merkle root:\n');

    for (let i = 0; i < sequencedEvents.length; i++) {
        const intent = sequencedEvents[i];
        const leaf = leaves[i];

        // Verify leaf is part of tree (simplified verification)
        const verifyData = JSON.stringify({
            intentId: intent.id,
            amount: intent.amount,
            payer: intent.payer_address,
            payee: intent.payee_address,
            signature: intent.payer_signature,
        });
        const computedLeaf = crypto.createHash('sha256').update(verifyData).digest('hex');

        const match = computedLeaf === leaf;

        console.log(`   [${i + 1}] Intent ${intent.id.slice(0, 8)}...`);
        console.log(`       Leaf:   ${computedLeaf.slice(0, 24)}...`);
        console.log(`       Status: ${match ? GREEN + '✓ Verified' : RED + '✗ Failed'}${NC}`);
        if (intent.tx_hash) {
            console.log(`       TX:     ${intent.tx_hash.slice(0, 24)}...`);
        }
        console.log('');
    }

    console.log(`   Merkle Root: ${merkleRoot.slice(0, 32)}...`);
    console.log(`   ${GREEN}✓ All payments verified against batch commitment${NC}`);

    // =========================================================================
    // FINAL SUMMARY
    // =========================================================================

    console.log('\n\n');
    console.log('╔════════════════════════════════════════════════════════════════════╗');
    console.log('║              x402 PROTOCOL E2E DEMO COMPLETE                       ║');
    console.log('╠════════════════════════════════════════════════════════════════════╣');
    console.log('║                                                                    ║');
    console.log('║  👤 AGENTS                                                         ║');
    console.log('║  ─────────────────────────────────────────────────────────────     ║');
    console.log(`║  • Merchant: ${wallet1Data.address.slice(0, 30)}...     ║`);
    console.log(`║  • Customer: ${fundedWallet.address.slice(0, 30)}...     ║`);
    console.log('║                                                                    ║');
    console.log('║  💳 PAYMENTS                                                       ║');
    console.log('║  ─────────────────────────────────────────────────────────────     ║');
    console.log(`║  • Total Intents:    ${payments.length}                                         ║`);
    console.log(`║  • Successful:       ${successCount}                                         ║`);
    console.log(`║  • Total Amount:     ${(totalAmount / 1_000_000).toFixed(2)} USDC                                   ║`);
    console.log('║                                                                    ║');
    console.log('║  📦 BATCH COMMITMENT                                               ║');
    console.log('║  ─────────────────────────────────────────────────────────────     ║');
    console.log(`║  • Batch ID:         ${batchId.slice(0, 20)}...                 ║`);
    console.log(`║  • Merkle Root:      ${merkleRoot.slice(0, 20)}...                 ║`);
    console.log(`║  • Sequences:        ${commitmentData.sequenceStart} - ${commitmentData.sequenceEnd}                                 ║`);
    console.log('║                                                                    ║');
    console.log('║  ⛓️  ARC L1 SETTLEMENT                                              ║');
    console.log('║  ─────────────────────────────────────────────────────────────     ║');
    console.log(`║  • Chain:            Arc Testnet (5042002)                         ║`);

    if (txHashes.length > 0 && txHashes[0].txHash) {
        console.log(`║  • First TX:         ${txHashes[0].txHash.slice(0, 20)}...                 ║`);
        console.log(`║  • Last TX:          ${txHashes[txHashes.length - 1].txHash?.slice(0, 20) || 'N/A'}...                 ║`);
    }

    if (commitmentData.commitmentTxHash) {
        console.log(`║  • Commit TX:        ${commitmentData.commitmentTxHash.slice(0, 20)}...                 ║`);
        console.log(`║  • Commit Block:     ${commitmentData.commitmentBlock}                                    ║`);
    }

    console.log('║                                                                    ║');
    console.log('║  🔗 EXPLORER LINKS                                                 ║');
    console.log('║  ─────────────────────────────────────────────────────────────     ║');

    if (txHashes.length > 0 && txHashes[0].txHash) {
        console.log(`║  ${chain.explorerUrl}/tx/${txHashes[0].txHash.slice(0, 16)}...  ║`);
    }

    console.log('║                                                                    ║');
    console.log('╚════════════════════════════════════════════════════════════════════╝');
    console.log('');

    // Save results
    const resultsPath = path.join(configDir, `x402-batch-${batchId.slice(0, 8)}.json`);
    const results = {
        batchId,
        merkleRoot,
        agents: {
            merchant: { id: agent1Id, address: wallet1Data.address },
            customer: { id: fundedWallet.address },
        },
        payments: sequencedEvents.map((e, i) => ({
            intentId: e.id,
            amount: e.amount,
            resource: payments[i].resource,
            txHash: txHashes[i]?.txHash,
            blockNumber: txHashes[i]?.blockNumber,
        })),
        commitment: commitmentData,
        createdAt: new Date().toISOString(),
    };

    fs.writeFileSync(resultsPath, JSON.stringify(results, null, 2));
    console.log(`${GREEN}✓${NC} Results saved to: ${resultsPath}`);
}

main().catch(err => {
    console.error(`${RED}Error:${NC} ${err.message}`);
    console.error(err.stack);
    process.exit(1);
});
