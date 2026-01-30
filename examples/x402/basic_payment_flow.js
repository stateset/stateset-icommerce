/**
 * Basic x402 Payment Flow Example
 *
 * Simple demonstration of x402 protocol:
 * 1. Create a payment intent
 * 2. Sign it
 * 3. Include it in HTTP requests
 *
 * Usage:
 *   node examples/x402/basic_payment_flow.js
 */

const crypto = require('crypto');

// =============================================================================
// CONFIGURATION
// =============================================================================

const X402_VERSION = '1.0';
const X402_DOMAIN_SEPARATOR = 'X402_PAYMENT_V1';

// =============================================================================
// x402 PAYMENT INTENT CREATION
// =============================================================================

function createPaymentIntent(payerAddress, payeeAddress, amountUSDC, resourceUri) {
  const now = Math.floor(Date.now() / 1000);
  const amountSmallest = Math.floor(amountUSDC * 1_000_000); // USDC has 6 decimals

  return {
    id: crypto.randomUUID(),
    version: X402_VERSION,
    status: 'created',

    // Payment parameters
    payer_address: payerAddress,
    payee_address: payeeAddress,
    amount: amountSmallest,
    amount_display: `${amountUSDC.toFixed(2)} USDC`,
    asset: 'USDC',
    network: 'set_chain',
    chain_id: 84532001, // Set Chain mainnet
    token_address: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',

    // Validity & replay protection
    created_at_unix: now,
    valid_until: now + 3600, // 1 hour
    nonce: Date.now(),

    // Resource context
    resource_uri: resourceUri,
    resource_method: 'POST',

    // Cryptographic fields (filled after signing)
    signing_hash: null,
    payer_signature: null,
    payer_public_key: null,
  };
}

// =============================================================================
// x402 PAYMENT SIGNING (Ed25519)
// =============================================================================

function signPaymentIntent(intent, privateKey) {
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
  const domainData = `${X402_DOMAIN_SEPARATOR}:${canonicalJson}`;
  const signingHash = crypto.createHash('sha256').update(domainData).digest('hex');

  // Simulate Ed25519 signature (use @noble/ed25519 in production)
  const signatureData = `${signingHash}:${privateKey}`;
  const signature = crypto.createHash('sha256')
    .update(signatureData)
    .digest('hex') +
    crypto.createHash('sha256')
      .update(signatureData + ':2')
      .digest('hex')
      .slice(0, 64);

  return {
    ...intent,
    status: 'signed',
    signing_hash: signingHash,
    payer_signature: signature,
    payer_public_key: crypto.createHash('sha256')
      .update(privateKey + 'pub')
      .digest('hex'),
  };
}

// =============================================================================
// ENCODE FOR HTTP HEADER
// =============================================================================

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

  const json = JSON.stringify(headerPayload);
  return Buffer.from(json).toString('base64');
}

// =============================================================================
// MAKE API REQUEST WITH PAYMENT
// =============================================================================

async function makePaidRequest(url, signedIntent) {
  const paymentHeader = encodePaymentHeader(signedIntent);

  const response = await fetch(url, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'X-Payment': paymentHeader,
    },
    body: JSON.stringify({
      model: 'gpt-4',
      messages: [{ role: 'user', content: 'Hello!' }],
    }),
  });

  return response;
}

// =============================================================================
// EXAMPLE: PAYING FOR AN API CALL
// =============================================================================

async function example() {
  console.log('=== Basic x402 Payment Flow Example ===\n');

  // Step 1: Create payment intent
  console.log('Step 1: Creating payment intent...');
  const intent = createPaymentIntent(
    '0x' + crypto.randomBytes(20).toString('hex'), // Payer address
    '0xabcdef1234567890abcdef1234567890abcdef12', // Payee address (merchant)
    0.10, // 0.10 USDC
    '/api/v1/chat/completions' // Resource URI
  );

  console.log(`  Intent ID: ${intent.id}`);
  console.log(`  Amount: ${intent.amount_display}`);
  console.log(`  Resource: ${intent.resource_uri}\n`);

  // Step 2: Sign the intent
  console.log('Step 2: Signing payment intent...');
  const privateKey = crypto.randomBytes(32).toString('hex');
  const signedIntent = signPaymentIntent(intent, privateKey);

  console.log(`  Signing hash: ${signedIntent.signing_hash}`);
  console.log(`  Signature: ${signedIntent.payer_signature.substring(0, 32)}...\n`);

  // Step 3: Encode for HTTP header
  console.log('Step 3: Encoding payment header...');
  const paymentHeader = encodePaymentHeader(signedIntent);

  console.log(`  X-Payment header length: ${paymentHeader.length} characters`);
  console.log(`  Preview: ${paymentHeader.substring(0, 60)}...\n`);

  // Step 4: Example HTTP request
  console.log('Step 4: Example HTTP request...');
  console.log('```');
  console.log(`POST ${intent.resource_uri} HTTP/1.1`);
  console.log('Host: api.example.com');
  console.log('Content-Type: application/json');
  console.log(`X-Payment: ${paymentHeader.substring(0, 40)}...`);
  console.log('');
  console.log('{');
  console.log('  "model": "gpt-4",');
  console.log('  "messages": [{"role": "user", "content": "Hello!"}]');
  console.log('}');
  console.log('```\n');

  // Step 5: What happens next
  console.log('Step 5: What happens next...');
  console.log('  1. Server decodes X-Payment header');
  console.log('  2. Verifies Ed25519 signature');
  console.log('  3. Checks validity window and nonce');
  console.log('  4. Submits to sequencer for batching');
  console.log('  5. Processes request and returns result');

  // Success summary
  console.log('\n✅ Payment flow complete!\n');
  console.log('Summary:');
  console.log(`  - Payment amount: ${signedIntent.amount_display}`);
  console.log(`  - Payment intent: ${signedIntent.id}`);
  console.log(`  - Valid until: ${new Date(signedIntent.valid_until * 1000).toISOString()}`);
  console.log(`  - Chain: Set Chain (ID: ${signedIntent.chain_id})`);
}

// =============================================================================
// RUN EXAMPLE
// =============================================================================

if (require.main === module) {
  example().catch(console.error);
}

module.exports = {
  createPaymentIntent,
  signPaymentIntent,
  encodePaymentHeader,
  makePaidRequest,
};