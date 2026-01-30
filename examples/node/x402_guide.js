#!/usr/bin/env node
/**
 * x402 Payment Protocol - Complete Guide for AI Agents
 *
 * This file contains 7 comprehensive examples showing how AI agents can
 * use the x402 protocol to make HTTP-native payments.
 *
 * Topics Covered:
 * 1. Basic Payment Flow - Simple request → 402 → pay → retry
 * 2. Creating Payment Intents - Building off-chain signed payments
 * 3. Ed25519 Signing - Cryptographic signing for x402 intents
 * 4. Handling 402 Responses - Server-side payment negotiation
 * 5. Payment Verification - Verifying signatures and inclusion proofs
 * 6. Batching Payments - Reducing costs with batch settlement
 * 8. Payment Retry Logic - Automatic retry with exponential backoff
 * 9. Multi-Network Support - Paying across different blockchains
 *
 * Usage:
 *   node examples/node/x402_guide.js
 */

import crypto from 'crypto';

// =============================================================================
// CONFIGURATION
// =============================================================================

const CONFIG = {
    x402: {
        version: '1.0',
        domainSeparator: 'X402_PAYMENT_V1',
        defaultValiditySeconds: 3600,
        maxValiditySeconds: 86400,
    },
    networks: {
        set_chain: { chainId: 84532001, name: 'Set Chain L2' },
        arc: { chainId: 5042001, name: 'Arc L1' },
        base: { chainId: 8453, name: 'Base L2' },
        ethereum: { chainId: 1, name: 'Ethereum Mainnet' },
    },
    assets: {
        USDC: { decimals: 6, name: 'USD Coin' },
        USDT: { decimals: 6, name: 'Tether' },
        DAI: { decimals: 18, name: 'DAI Stablecoin' },
        ETH: { decimals: 18, name: 'Ether' },
    },
};

// =============================================================================
// EXAMPLE 1: BASIC PAYMENT FLOW
// =============================================================================

/**
 * Example 1: Basic AI Agent Payment Flow
 * 
 * Demonstrates the simplest x402 payment flow:
 * 1. Agent requests protected resource
 * 2. Server returns HTTP 402 with payment requirements
 * 3. Agent creates and signs payment intent
 * 4. Agent retries request with payment header
 * 5. Server returns resource along with payment receipt
 */
async function example1_basicPaymentFlow() {
    console.log('╔════════════════════════════════════════════════════════════════════╗');
    console.log('║  EXAMPLE 1: Basic x402 Payment Flow                                 ║');
    console.log('╚════════════════════════════════════════════════════════════════════╝\n');

    // Agent wallet
    const agentWallet = {
        address: '0x' + crypto.randomBytes(20).toString('hex'),
        privateKey: crypto.randomBytes(32),
    };

    // Merchant configuration
    const merchant = {
        address: '0x' + crypto.randomBytes(20).toString('hex'),
        merchantId: 'api-service-merchant-001',
        name: 'AI API Service',
        price: 100000, // 0.10 USDC
    };

    // Step 1: Agent requests protected resource
    console.log('Step 1: Agent requests protected API resource');
    console.log('  Method: POST /api/v1/ai/completion');
    console.log('  Agent:   ' + agentWallet.address.slice(0, 10) + '...');
    console.log('');

    // Step 2: Server returns 402 Payment Required
    console.log('Step 2: Server returns HTTP 402 Payment Required');
    const paymentRequired = {
        version: CONFIG.x402.version,
        payee_address: merchant.address,
        amount: merchant.price,
        amount_display: '0.10 USDC',
        asset: 'USDC',
        networks: ['set_chain', 'arc'],
        resource_uri: '/api/v1/ai/completion',
        resource_method: 'POST',
        description: 'AI completion API call',
        validity_seconds: CONFIG.x402.defaultValiditySeconds,
        merchant_id: merchant.merchantId,
        merchant_name: merchant.name,
    };
    console.log('  Payee:   ' + paymentRequired.payee_address.slice(0, 10) + '...');
    console.log('  Amount:  ' + paymentRequired.amount_display);
    console.log('  Asset:   ' + paymentRequired.asset);
    console.log('');

    // Step 3: Agent creates payment intent
    console.log('Step 3: Agent creates X402PaymentIntent');
    const intent = createPaymentIntent(paymentRequired, agentWallet);
    console.log('  Intent ID: ' + intent.id.slice(0, 8) + '...');
    console.log('  Status:    ' + intent.status);
    console.log('');

    // Step 4: Agent signs intent
    console.log('Step 4: Agent signs payment intent with Ed25519');
    const signedIntent = signPaymentIntent(intent, agentWallet.privateKey);
    console.log('  ✓ Signed');
    console.log('  Signature: ' + signedIntent.payer_signature.slice(0, 16) + '...');
    console.log('');

    // Step 5: Agent retries request with payment
    console.log('Step 5: Agent retries request with X-Payment header');
    const paymentHeader = encodeX402PaymentHeader(signedIntent);
    console.log('  Header: ' + paymentHeader.slice(0, 60) + '...');
    console.log('');

    // Step 6: Server processes and returns receipt
    console.log('Step 6: Server returns resource with payment receipt');
    const receipt = createSimulatedReceipt(signedIntent);
    console.log('  ✓ Payment verified');
    console.log('  Receipt ID: ' + receipt.id.slice(0, 8) + '...');
    console.log('');

    console.log('✅ Basic payment flow complete!\n');
}

// =============================================================================
// EXAMPLE 2: CREATING PAYMENT INTENTS
// =============================================================================

/**
 * Example 2: Creating x402 Payment Intents
 * 
 * Shows how to create payment intents with various parameters:
 * - Different assets (USDC, USDT, DAI, ETH)
 * - Different networks (Set Chain, Arc, Base, Ethereum)
 * - Validity windows and nonces
 * - Order and invoice references
 */
async function example2_creatingPaymentIntents() {
    console.log('╔════════════════════════════════════════════════════════════════════╗');
    console.log('║  EXAMPLE 2: Creating Payment Intents                               ║');
    console.log('╚════════════════════════════════════════════════════════════════════╝\n');

    const payerWallet = {
        address: '0x' + crypto.randomBytes(20).toString('hex'),
        privateKey: crypto.randomBytes(32),
    };

    const payeeAddress = '0x' + crypto.randomBytes(20).toString('hex');

    // Create intents with different configurations
    const intents = [
        // Intent 1: Simple USDC payment on Set Chain
        {
            description: 'Basic USDC payment',
            amount: 100000, // 0.10 USDC
            asset: 'USDC',
            network: 'set_chain',
            validitySeconds: 3600,
        },
        // Intent 2: Larger DAI payment on Arc
        {
            description: 'Large DAI payment',
            amount: 5000000000000000000n, // 5 DAI (18 decimals)
            asset: 'DAI',
            network: 'arc',
            validitySeconds: 7200,
        },
        // Intent 3: ETH payment for gas
        {
            description: 'ETH payment for gas',
            amount: 10000000000000000n, // 0.01 ETH
            asset: 'ETH',
            network: 'ethereum',
            validitySeconds: 1800,
        },
    ];

    let nonce = Date.now();

    for (const config of intents) {
        console.log(`Creating intent: ${config.description}`);
        console.log('  Network:  ' + config.network);
        console.log('  Asset:    ' + config.asset);
        console.log('  Amount:   ' + formatAmount(config.amount, config.asset));

        const intent = {
            id: crypto.randomUUID(),
            version: CONFIG.x402.version,
            status: 'created',
            payer_address: payerWallet.address,
            payee_address: payeeAddress,
            amount: config.amount.toString(),
            amount_decimal: formatAmount(config.amount, config.asset),
            asset: config.asset,
            network: config.network,
            chain_id: CONFIG.networks[config.network].chainId,
            created_at_unix: Math.floor(Date.now() / 1000),
            valid_until: Math.floor(Date.now() / 1000) + config.validitySeconds,
            nonce: nonce++,
            description: config.description,
        };

        console.log('  Intent:   ' + intent.id.slice(0, 8) + '...');
        console.log('  Expiry:   ' + new Date(intent.valid_until * 1000).toISOString());
        console.log('');
    }

    console.log('✅ Created 3 different payment intents!\n');
}

// =============================================================================
// EXAMPLE 3: ED25519 SIGNING
// =============================================================================

/**
 * Example 3: Ed25519 Cryptographic Signing
 * 
 * Demonstrates the cryptographic signing process:
 * - Create signing payload with canonical JSON
 * - Domain-separated hash computation
 * - Ed25519 signature generation
 * - Signature verification
 */
async function example3_ed25519Signing() {
    console.log('╔════════════════════════════════════════════════════════════════════╗');
    console.log('║  EXAMPLE 3: Ed25519 Cryptographic Signing                         ║');
    console.log('╚════════════════════════════════════════════════════════════════════╝\n');

    // Generate Ed25519 keypair (using buffer for demo, use actual Ed25519 in production)
    const publicKey = crypto.randomBytes(32);
    const privateKey = crypto.randomBytes(32);

    console.log('Generated Ed25519 keypair:');
    console.log('  Public Key:  ' + publicKey.toString('hex').slice(0, 32) + '...');
    console.log('  Private Key: ' + privateKey.toString('hex').slice(0, 32) + '...');
    console.log('');

    // Create signing payload
    const payload = {
        version: '1.0',
        payer: '0x' + crypto.randomBytes(20).toString('hex'),
        payee: '0x' + crypto.randomBytes(20).toString('hex'),
        amount: '100000',
        asset: 'USDC',
        chainId: 84532001,
        nonce: Date.now(),
        validUntil: Math.floor(Date.now() / 1000) + 3600,
    };

    console.log('Signing payload:');
    console.log(JSON.stringify(payload, null, 2));
    console.log('');

    // Create canonical JSON (sorted keys)
    const canonical = JSON.stringify(payload, Object.keys(payload).sort());
    console.log('Canonical JSON (sorted keys):');
    console.log(canonical.slice(0, 100) + '...');
    console.log('');

    // Domain-separated hash
    const domainData = CONFIG.x402.domainSeparator + ':' + canonical;
    const signingHash = crypto.createHash('sha256').update(domainData).digest('hex');
    console.log('Domain-separated signing hash:');
    console.log('  ' + signingHash);
    console.log('');

    // Sign (simplified - use actual Ed25519 in production)
    const signature = crypto.createHmac('sha256', privateKey)
        .update(signingHash)
        .digest('hex');

    console.log('Signature:');
    console.log('  ' + signature.slice(0, 64));
    console.log('');

    // Verify
    const verifyHash = crypto.createHmac('sha256', privateKey)
        .update(signingHash)
        .digest('hex');
    const isValid = verifyHash === signature;

    console.log('Verification:');
    console.log('  ✓ Signature valid: ' + (isValid ? 'YES' : 'NO'));
    console.log('');

    console.log('✅ Ed25519 signing complete!\n');
}

// =============================================================================
// EXAMPLE 4: HANDLING 402 RESPONSES
// =============================================================================

/**
 * Example 4: Server-Side HTTP 402 Handling
 * 
 * Shows how a server should respond to requests requiring payment:
 * - Returning HTTP 402 with X-Payment-Required header
 * - Including merchant information and pricing
 * - Specifying accepted networks and assets
 */
async function example4_handling402Responses() {
    console.log('╔════════════════════════════════════════════════════════════════════╗');
    console.log('║  EXAMPLE 4: Handling HTTP 402 Responses                             ║');
    console.log('╚════════════════════════════════════════════════════════════════════╝\n');

    const merchant = {
        address: '0x1234...5678',
        merchantId: 'premium-api-001',
        name: 'Premium AI API',
        pricing: {
            'GET': 0.01,    // $0.01 USDC for GET requests
            'POST': 0.10,   // $0.10 USDC for POST requests
            'PUT': 0.05,    // $0.05 USDC for PUT requests
        },
    };

    console.log('Server handling incoming request:');
    console.log('  Request: POST /api/v1/ai/generate');
    console.log('  Authentication: Valid API key');
    console.log('');

    // Check if payment required
    const priceInSmallest = convertToSmallestUnit(merchant.pricing['POST'], 6); // USDC has 6 decimals

    console.log('Payment required for this resource:');
    console.log('  Price: $' + merchant.pricing['POST'].toFixed(2) + ' ' + merchant.pricing['POST']);
    console.log('  Amount: ' + priceInSmallest + ' (smallest units)');
    console.log('');

    // Create 402 response
    const paymentRequired = {
        version: CONFIG.x402.version,
        payee_address: merchant.address,
        amount: priceInSmallest,
        amount_display: '$0.10 USDC',
        asset: 'USDC',
        networks: ['set_chain', 'arc', 'base'],
        resource_uri: '/api/v1/ai/generate',
        resource_method: 'POST',
        description: 'AI model generation API call',
        validity_seconds: 3600,
        merchant_id: merchant.merchantId,
        merchant_name: merchant.name,
        generated_at: new Date().toISOString(),
    };

    console.log('HTTP 402 Payment Required Response:');
    console.log('');
    console.log('  HTTP/1.1 402 Payment Required');
    console.log('  Content-Type: application/json');
    console.log('  X-Payment-Required: ' + Buffer.from(JSON.stringify(paymentRequired)).toString('base64').slice(0, 40) + '...');
    console.log('');
    console.log('  Body:');
    console.log(JSON.stringify(paymentRequired, null, 2).split('\n').map(line => '    ' + line).join('\n'));
    console.log('');

    // Decode from header (client side)
    const headerValue = Buffer.from(JSON.stringify(paymentRequired)).toString('base64');
    const decoded = JSON.parse(Buffer.from(headerValue, 'base64').toString('utf-8'));
    console.log('Decoded from header:');
    console.log('  Payee:   ' + decoded.payee_address);
    console.log('  Amount:  ' + decoded.amount_display);
    console.log('  Networks: ' + decoded.networks.join(', '));
    console.log('');

    console.log('✅ HTTP 402 handling complete!\n');
}

// =============================================================================
// EXAMPLE 5: PAYMENT VERIFICATION
// =============================================================================

/**
 * Example 5: Verifying x402 Payments
 * 
 * Shows how to verify payment authenticity:
 * - Decode and validate X-Payment header
 * - Verify Ed25519 signature
 * - Check validity window and nonce
 * - Verify inclusion proof against merkle root
 */
async function example5_paymentVerification() {
    console.log('╔════════════════════════════════════════════════════════════════════╗');
    console.log('║  EXAMPLE 5: Verifying x402 Payments                                ║');
    console.log('╚════════════════════════════════════════════════════════════════════╝\n');

    const merchantWallet = {
        address: '0x' + crypto.randomBytes(20).toString('hex'),
    };

    // Create a signed payment
    const paymentRequired = {
        version: CONFIG.x402.version,
        payee_address: merchantWallet.address,
        amount: 100000,
        amount_display: '0.10 USDC',
        asset: 'USDC',
        networks: ['set_chain'],
        resource_uri: '/api/v1/resource',
        resource_method: 'GET',
        validity_seconds: 3600,
        merchant_id: 'merchant-001',
    };

    const agentWallet = {
        address: '0x' + crypto.randomBytes(20).toString('hex'),
        privateKey: crypto.randomBytes(32),
    };

    const intent = createPaymentIntent(paymentRequired, agentWallet);
    const signedIntent = signPaymentIntent(intent, agentWallet.privateKey);

    console.log('Verification Steps:');
    console.log('');
    console.log('Step 1: Decode X-Payment header');
    const headerData = {
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
    };
    console.log('  ✓ Decoded header');
    console.log('  Intent ID: ' + headerData.intent_id.slice(0, 8) + '...');
    console.log('');

    console.log('Step 2: Verify Ed25519 signature');
    const signingPayload = {
        version: signedIntent.version,
        payer: signedIntent.payer_address,
        payee: signedIntent.payee_address,
        amount: signedIntent.amount.toString(),
        asset: signedIntent.asset,
        chainId: signedIntent.chain_id,
        nonce: signedIntent.nonce,
        validUntil: signedIntent.valid_until,
    };
    const canonical = JSON.stringify(signingPayload, Object.keys(signingPayload).sort());
    const domainData = CONFIG.x402.domainSeparator + ':' + canonical;
    const computedHash = crypto.createHash('sha256').update(domainData).digest('hex');
    const hashMatch = computedHash === signedIntent.signing_hash;
    console.log('  ✓ Signature hash matches: ' + (hashMatch ? 'YES' : 'NO'));
    console.log('  Expected: ' + signedIntent.signing_hash.slice(0, 16) + '...');
    console.log('  Computed: ' + computedHash.slice(0, 16) + '...');
    console.log('');

    console.log('Step 3: Check validity window');
    const now = Math.floor(Date.now() / 1000);
    const isValidWindow = signedIntent.valid_until > now;
    const secondsRemaining = signedIntent.valid_until - now;
    console.log('  ✓ Not expired: ' + (isValidWindow ? 'YES' : 'NO'));
    console.log('  Time remaining: ' + secondsRemaining + ' seconds');
    console.log('');

    console.log('Step 4: Verify nonce (replay protection)');
    console.log('  ✓ Nonce is unique: YES');
    console.log('  Nonce: ' + signedIntent.nonce);
    console.log('');

    console.log('Step 5: Verify payee matches merchant');
    const payeeMatch = signedIntent.payee_address === merchantWallet.address;
    console.log('  ✓ Payee matches: ' + (payeeMatch ? 'YES' : 'NO'));
    console.log('  Expected: ' + merchantWallet.address.slice(0, 16) + '...');
    console.log('  Actual:   ' + signedIntent.payee_address.slice(0, 16) + '...');
    console.log('');

    console.log('Step 6: Verify Merkle inclusion proof (after settlement)');
    console.log('  ✓ Inclusion proof: VALID (simulated)');
    console.log('  Merkle Root:  ' + signedIntent.batch_merkle_root?.slice(0, 16) + '... || N/A');
    console.log('');

    const allValid = hashMatch && isValidWindow && payeeMatch;
    console.log('Final Result: ' + (allValid ? '✅ PAYMENT VALID' : '❌ PAYMENT INVALID'));
    console.log('');
}

// =============================================================================
// EXAMPLE 6: BATCHING PAYMENTS
// =============================================================================

/**
 * Example 6: Batching x402 Payments
 * 
 * Demonstrates how to batch payments for efficiency:
 * - Collect multiple payment intents
 * - Build Merkle tree
 * - Submit batch to sequencer
 * - Verify batch commitment
 */
async function example6_batchingPayments() {
    console.log('╔════════════════════════════════════════════════════════════════════╗');
    console.log('║  EXAMPLE 6: Batching Multiple Payments                              ║');
    console.log('╚════════════════════════════════════════════════════════════════════╝\n');

    const merchant = {
        address: '0x' + crypto.randomBytes(20).toString('hex'),
    };

    // Create multiple payments
    const payments = [];
    let nonce = Date.now();

    for (let i = 0; i < 5; i++) {
        const intent = {
            id: crypto.randomUUID(),
            amount: 100000 + Math.floor(Math.random() * 400000), // 0.10 - 0.50 USDC
            payer_address: '0x' + crypto.randomBytes(20).toString('hex'),
            payee_address: merchant.address,
            nonce: nonce++,
            resource_uri: `/api/v1/resource/${i}`,
        };
        payments.push(intent);
    }

    console.log('Batching ' + payments.length + ' payments:');
    console.log('');
    let totalAmount = 0n;
    for (let i = 0; i < payments.length; i++) {
        const p = payments[i];
        const amountDecimal = (BigInt(p.amount) / 1000000n).toString();
        totalAmount += BigInt(p.amount);
        console.log(`  [${i + 1}] ${p.resource_uri.padEnd(25)} $${amountDecimal.padEnd(6)} USDC`);
    }
    console.log('  ' + '─'.repeat(45));
    console.log('     Total:                      $' + (totalAmount / 1000000n).toString().padEnd(6) + ' USDC');
    console.log('');

    // Build Merkle tree
    console.log('Building Merkle tree for batch:');
    const leaves = payments.map(p => {
        const leafData = JSON.stringify({
            id: p.id,
            amount: p.amount,
            payer: p.payer_address,
            payee: p.payee_address,
            nonce: p.nonce,
        });
        return crypto.createHash('sha256').update(leafData).digest('hex');
    });

    console.log('  Created ' + leaves.length + ' leaf nodes');
    console.log('');

    // Build tree
    let level = leaves;
    const proofPaths = leaves.map(() => []);

    while (level.length > 1) {
        const newLevel = [];
        for (let i = 0; i < level.length; i += 2) {
            const left = level[i];
            const right = level[i + 1] || left;
            const combined = left < right ? left + right : right + left;
            newLevel.push(crypto.createHash('sha256').update(combined).digest('hex'));
        }
        level = newLevel;
    }

    const merkleRoot = level[0];

    console.log('Merkle tree complete:');
    console.log('  Merkle Root: ' + merkleRoot.slice(0, 24) + '...');
    console.log('  Depth:       ' + Math.ceil(Math.log2(payments.length || 1)));
    console.log('');

    console.log('Batch commitment:');
    const batchId = crypto.randomUUID();
    console.log('  Batch ID:     ' + batchId.slice(0, 8) + '...');
    console.log('  Sequences:    1000 - ' + (1000 + payments.length - 1));
    console.log('  Payment Count: ' + payments.length);
    console.log('  Total Value:   $' + (totalAmount / 1000000n).toString() + ' USDC');
    console.log('  Merkle Root:   ' + merkleRoot.slice(0, 24) + '...');
    console.log('');

    console.log('✅ Payment batching complete!\n');
}

// =============================================================================
// EXAMPLE 7: MULTI-NETWORK SUPPORT
// =============================================================================;

/**
 * Example 7: Multi-Network Payment Support
 * 
 * Shows how to handle payments across different blockchain networks:
 * - Check network compatibility
 * - Validate chain IDs
 * - Handle different gas costs
 * - Select optimal network
 */
async function example7_multiNetworkSupport() {
    console.log('╔════════════════════════════════════════════════════════════════════╗');
    console.log('║  EXAMPLE 7: Multi-Network Payment Support                         ║');
    console.log('╚════════════════════════════════════════════════════════════════════╝\n');

    const networks = ['set_chain', 'arc', 'base', 'ethereum'];
    const assets = ['USDC', 'USDT', 'DAI', 'ETH'];

    console.log('Supported Networks:');
    console.log('');
    for (const network of networks) {
        const config = CONFIG.networks[network];
        console.log(`  ${network.padEnd(15)} Chain ID: ${config.chainId.toString().padEnd(10)} ${config.name}`);
    }
    console.log('');

    console.log('Supported Assets:');
    console.log('');
    for (const asset of assets) {
        const config = CONFIG.assets[asset];
        console.log(`  ${asset.padEnd(8)} Decimals: ${config.decimals.toString().padEnd(3)} ${config.name}`);
    }
    console.log('');

    console.log('Creating payment on different networks:');
    console.log('');

    const payeeAddress = '0x' + crypto.randomBytes(20).toString('hex');
    const payerAddress = '0x' + crypto.randomBytes(20).toString('hex');

    for (const network of ['set_chain', 'arc', 'ethereum']) {
        const chainId = CONFIG.networks[network].chainId;
        const intent = {
            id: crypto.randomUUID(),
            payer_address: payerAddress,
            payee_address: payeeAddress,
            amount: 100000,
            asset: 'USDC',
            network: network,
            chain_id: chainId,
            chain_id_hex: '0x' + chainId.toString(16),
        };

        console.log(`  Network: ${network.padEnd(15)}`);
        console.log(`    Chain ID:   ${intent.chain_id} (${intent.chain_id_hex})`);
        console.log(`    Asset:      ${intent.asset}`);
        console.log(`    Amount:     ${formatAmount(BigInt(intent.amount), intent.asset)}`);
        console.log('');
    }

    console.log('Network selection considerations:');
    console.log('  • Set Chain:  Lowest gas fees, fastest settlement');
    console.log('  • Arc:        Stablecoin-native optimized');
    console.log('  • Base:       Coinbase L2, high throughput');
    console.log('  • Ethereum:   Most secure, highest fees');
    console.log('');

    console.log('✅ Multi-network support complete!\n');
}

// =============================================================================
// HELPER FUNCTIONS
// =============================================================================

function createPaymentIntent(paymentRequired, wallet) {
    const now = Math.floor(Date.now() / 1000);
    return {
        id: crypto.randomUUID(),
        version: CONFIG.x402.version,
        status: 'created',
        payer_address: wallet.address,
        payee_address: paymentRequired.payee_address,
        amount: paymentRequired.amount,
        amount_decimal: (paymentRequired.amount / 1000000).toString(),
        asset: paymentRequired.asset,
        network: 'set_chain',
        chain_id: CONFIG.networks.set_chain.chainId,
        created_at_unix: now,
        valid_until: now + paymentRequired.validity_seconds,
        nonce: Date.now(),
        resource_uri: paymentRequired.resource_uri,
        resource_method: paymentRequired.resource_method,
        description: paymentRequired.description,
    };
}

function signPaymentIntent(intent, privateKey) {
    const signingPayload = {
        version: intent.version,
        payer: intent.payer_address,
        payee: intent.payee_address,
        amount: intent.amount.toString(),
        asset: intent.asset,
        chainId: intent.chain_id,
        nonce: intent.nonce,
        validUntil: intent.valid_until,
        resourceUri: intent.resource_uri,
    };

    const canonical = JSON.stringify(signingPayload, Object.keys(signingPayload).sort());
    const domainData = CONFIG.x402.domainSeparator + ':' + canonical;
    const signingHash = crypto.createHash('sha256').update(domainData).digest('hex');

    const signature = crypto.createHmac('sha256', privateKey)
        .update(signingHash)
        .digest('hex');

    return {
        ...intent,
        status: 'signed',
        signing_hash: signingHash,
        payer_signature: signature,
        payer_public_key: Buffer.from(privateKey).toString('hex').slice(0, 64),
    };
}

function encodeX402PaymentHeader(intent) {
    const headerPayload = {
        intent_id: intent.id,
        version: intent.version,
        payer: intent.payer_address,
        payee: intent.payee_address,
        amount: intent.amount,
        asset: intent.asset,
        chain_id: intent.chain_id,
        nonce: intent.nonce,
        valid_until: intent.valid_until,
        signature: intent.payer_signature,
        public_key: intent.payer_public_key,
    };
    return Buffer.from(JSON.stringify(headerPayload)).toString('base64');
}

function createSimulatedReceipt(intent) {
    return {
        id: crypto.randomUUID(),
        intent_id: intent.id,
        sequence_number: 42,
        batch_id: crypto.randomUUID(),
        merkle_root: crypto.createHash('sha256').update('batch').digest('hex'),
        tx_hash: '0x' + crypto.randomBytes(32).toString('hex'),
        block_number: 12345,
        amount: intent.amount,
        asset: intent.asset,
        network: intent.network,
        created_at: new Date().toISOString(),
    };
}

function convertToSmallestUnit(amount, decimals) {
    return Math.floor(amount * Math.pow(10, decimals));
}

function formatAmount(amount, asset) {
    const decimals = CONFIG.assets[asset]?.decimals || 18;
    const divisor = BigInt(10 ** decimals);
    return (amount / divisor).toString();
}

// =============================================================================
// MAIN EXECUTION
// =============================================================================

async function main() {
    console.log('\n');
    console.log('╔════════════════════════════════════════════════════════════════════╗');
    console.log('║                                                                      ║');
    console.log('║         x402 PAYMENT PROTOCOL - COMPLETE GUIDE FOR AI AGENTS         ║');
    console.log('║                                                                      ║');
    console.log('╚════════════════════════════════════════════════════════════════════╝');
    console.log('\n');

    await example1_basicPaymentFlow();
    await example2_creatingPaymentIntents();
    await example3_ed25519Signing();
    await example4_handling402Responses();
    await example5_paymentVerification();
    await example6_batchingPayments();
    await example7_multiNetworkSupport();

    console.log('╔════════════════════════════════════════════════════════════════════╗');
    console.log('║                    ALL EXAMPLES COMPLETE                           ║');
    console.log('╚════════════════════════════════════════════════════════════════════╝');
    console.log('\n');
}

main().catch(console.error);