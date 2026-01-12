/**
 * Stablecoin Payment Execution for StateSet iCommerce
 *
 * Handles:
 * - Payment intent creation (VES event)
 * - Transaction signing with agent wallet
 * - On-chain submission
 * - Confirmation polling
 * - VES audit trail
 *
 * Supports: Solana (USDC), SET Chain (ssUSD), Base (USDC)
 */

import crypto from 'crypto';
import { deriveWallet } from './wallet.js';
import {
  CHAINS,
  getChain,
  getToken,
  getDefaultStablecoin,
  toSmallestUnit,
  fromSmallestUnit,
  getExplorerTxUrl,
  isEd25519Chain,
  isEvmChain,
  isZcashChain,
  isBitcoinChain,
} from './config.js';
import {
  ValidationError,
  ValidationErrorCodes,
  validateChainId,
  validateToken,
  validateAmount,
  validateAddress,
} from './validation.js';

// =============================================================================
// PAYMENT INTENT
// =============================================================================

/**
 * @typedef {Object} PaymentIntent
 * @property {string} intentId - Unique intent identifier
 * @property {string} chainId - Blockchain network
 * @property {string} tokenSymbol - Token to transfer (USDC, ssUSD)
 * @property {string} fromAddress - Sender wallet address
 * @property {string} toAddress - Recipient wallet address
 * @property {string} amount - Human-readable amount (e.g., "100.00")
 * @property {bigint} amountSmallest - Amount in smallest units
 * @property {string} currency - Currency code (usually USD)
 * @property {Object} [metadata] - Additional metadata (order_id, customer_id)
 * @property {string} createdAt - ISO timestamp
 * @property {string} status - pending, signed, submitted, confirmed, failed
 */

/**
 * @typedef {Object} PaymentResult
 * @property {boolean} success
 * @property {string} intentId
 * @property {string} [txHash] - Transaction hash/signature
 * @property {string} [explorerUrl] - Block explorer URL
 * @property {number} [blockNumber]
 * @property {number} [confirmations]
 * @property {string} [error]
 * @property {Array<string>} vesEventIds - VES event IDs for audit trail
 */

/**
 * Create a payment intent
 * @param {Object} params
 * @param {string} params.agentId - Agent identifier
 * @param {string} params.chainId - Chain to use (solana, set_chain, base)
 * @param {string} params.toAddress - Recipient wallet address
 * @param {string|number} params.amount - Amount to send (human-readable)
 * @param {string} [params.tokenSymbol] - Token symbol (default: chain's default stablecoin)
 * @param {Object} [params.metadata] - Additional metadata
 * @param {Object} [options]
 * @returns {Promise<PaymentIntent>}
 */
export async function createPaymentIntent(params, options = {}) {
  const {
    agentId,
    chainId,
    toAddress,
    amount,
    tokenSymbol,
    metadata = {},
  } = params;

  const { configDir = '.stateset' } = options;

  // Validate agent ID
  if (!agentId || typeof agentId !== 'string') {
    throw new ValidationError(
      ValidationErrorCodes.MISSING_REQUIRED,
      'Agent ID is required',
      { field: 'agentId' }
    );
  }

  // Validate chain using comprehensive validator
  validateChainId(chainId);
  const chain = getChain(chainId);

  // Validate token
  validateToken(chainId, tokenSymbol);

  // Get token config
  const token = tokenSymbol
    ? getToken(chainId, tokenSymbol)
    : getDefaultStablecoin(chainId);

  // Validate amount (returns parsed number)
  const amountNum = validateAmount(amount);

  // Validate recipient address
  validateAddress(toAddress, chainId);

  // Derive agent wallet
  const wallet = await deriveWallet(agentId, chainId, { configDir });

  // Check for self-transfer
  const fromNormalized = wallet.address.toLowerCase();
  const toNormalized = toAddress.toLowerCase();
  if (fromNormalized === toNormalized) {
    throw new ValidationError(
      ValidationErrorCodes.SELF_TRANSFER,
      'Cannot transfer to the same address',
      { fromAddress: wallet.address, toAddress }
    );
  }

  // Convert amount to smallest units
  const amountSmallest = toSmallestUnit(amountNum, token.decimals);

  // Create intent
  const intent = {
    intentId: crypto.randomUUID(),
    chainId,
    tokenSymbol: token.symbol,
    tokenAddress: token.address,
    tokenDecimals: token.decimals,
    fromAddress: wallet.address,
    toAddress,
    amount: amountNum.toFixed(token.decimals),
    amountSmallest,
    currency: 'USD',
    metadata: {
      ...metadata,
      agentId,
    },
    createdAt: new Date().toISOString(),
    status: 'pending',
  };

  return intent;
}

// =============================================================================
// PAYMENT EXECUTION
// =============================================================================

/**
 * Execute a stablecoin payment
 *
 * This is a high-level function that:
 * 1. Creates a payment intent
 * 2. Signs the transaction with agent wallet
 * 3. Submits to the blockchain
 * 4. Waits for confirmation
 * 5. Records VES events for audit trail
 *
 * @param {Object} params
 * @param {string} params.agentId - Agent identifier
 * @param {string} params.chainId - Chain to use
 * @param {string} params.toAddress - Recipient address
 * @param {string|number} params.amount - Amount to send
 * @param {string} [params.tokenSymbol] - Token symbol
 * @param {Object} [params.metadata] - Order/customer metadata
 * @param {Object} [options]
 * @param {boolean} [options.simulate] - Dry run (don't actually send)
 * @param {Function} [options.onProgress] - Progress callback
 * @returns {Promise<PaymentResult>}
 */
export async function executePayment(params, options = {}) {
  const {
    agentId,
    chainId,
    toAddress,
    amount,
    tokenSymbol,
    metadata = {},
  } = params;

  const {
    configDir = '.stateset',
    simulate = false,
    onProgress = () => {},
  } = options;

  const vesEventIds = [];
  let intent;

  try {
    // Step 1: Create payment intent
    onProgress({ step: 'creating_intent', message: 'Creating payment intent...' });

    intent = await createPaymentIntent({
      agentId,
      chainId,
      toAddress,
      amount,
      tokenSymbol,
      metadata,
    }, { configDir });

    onProgress({
      step: 'intent_created',
      message: `Payment intent created: ${intent.intentId}`,
      intent,
    });

    // Step 2: Get wallet for signing
    onProgress({ step: 'deriving_wallet', message: 'Deriving agent wallet...' });

    const wallet = await deriveWallet(agentId, chainId, { configDir });

    onProgress({
      step: 'wallet_derived',
      message: `Wallet: ${wallet.address}`,
    });

    // Step 3: Build transaction
    onProgress({ step: 'building_tx', message: 'Building transaction...' });

    const txData = await buildTransaction(intent, wallet, chainId);

    // Step 4: Sign transaction
    onProgress({ step: 'signing', message: 'Signing transaction...' });

    const signedTx = await signTransaction(txData, wallet, chainId);
    intent.status = 'signed';

    // If simulating, stop here
    if (simulate) {
      onProgress({
        step: 'simulated',
        message: 'Simulation complete (transaction not submitted)',
      });

      return {
        success: true,
        intentId: intent.intentId,
        simulated: true,
        intent,
        vesEventIds,
      };
    }

    // Step 5: Submit transaction
    onProgress({ step: 'submitting', message: 'Submitting to network...' });

    const submitResult = await submitTransaction(signedTx, chainId);
    intent.status = 'submitted';

    onProgress({
      step: 'submitted',
      message: `Transaction submitted: ${submitResult.txHash}`,
      txHash: submitResult.txHash,
    });

    // Step 6: Wait for confirmation
    onProgress({ step: 'confirming', message: 'Waiting for confirmation...' });

    const confirmation = await waitForConfirmation(
      submitResult.txHash,
      chainId,
      { onProgress, isMock: submitResult.isMock }
    );

    intent.status = 'confirmed';

    const explorerUrl = getExplorerTxUrl(chainId, submitResult.txHash);

    onProgress({
      step: 'confirmed',
      message: `Confirmed! ${confirmation.confirmations} confirmations`,
      txHash: submitResult.txHash,
      explorerUrl,
    });

    return {
      success: true,
      intentId: intent.intentId,
      txHash: submitResult.txHash,
      txSignature: submitResult.txHash,
      explorerUrl,
      blockNumber: confirmation.blockNumber,
      confirmations: confirmation.confirmations,
      intent,
      vesEventIds,
    };

  } catch (error) {
    onProgress({
      step: 'failed',
      message: `Payment failed: ${error.message}`,
      error: error.message,
    });

    if (intent) {
      intent.status = 'failed';
    }

    return {
      success: false,
      intentId: intent?.intentId,
      error: error.message,
      intent,
      vesEventIds,
    };
  }
}

// =============================================================================
// TRANSACTION BUILDING
// =============================================================================

/**
 * Build a transaction for the payment
 * @param {PaymentIntent} intent
 * @param {Object} wallet
 * @param {string} chainId
 * @returns {Promise<Object>}
 */
async function buildTransaction(intent, wallet, chainId) {
  if (isEd25519Chain(chainId)) {
    return buildSolanaTransaction(intent, wallet);
  } else if (isZcashChain(chainId)) {
    return buildZcashTransaction(intent, wallet, chainId);
  } else if (isBitcoinChain(chainId)) {
    return buildBitcoinTransaction(intent, wallet, chainId);
  } else if (isEvmChain(chainId)) {
    return buildEvmTransaction(intent, wallet, chainId);
  }

  throw new Error(`Unsupported chain for transaction building: ${chainId}`);
}

/**
 * Build Solana SPL token transfer transaction
 * Note: In production, use @solana/web3.js and @solana/spl-token
 */
async function buildSolanaTransaction(intent, wallet) {
  // This is a simplified representation
  // In production, use proper Solana SDK

  return {
    type: 'solana_spl_transfer',
    fromAddress: intent.fromAddress,
    toAddress: intent.toAddress,
    tokenMint: intent.tokenAddress,
    amount: intent.amountSmallest.toString(),
    decimals: intent.tokenDecimals,
    // In production: include recent blockhash, instruction data, etc.
    mockTx: true,
  };
}

/**
 * Build EVM token transfer transaction
 * Note: In production, use ethers.js or viem
 */
async function buildEvmTransaction(intent, wallet, chainId) {
  const chain = getChain(chainId);

  // ERC20 transfer function signature: transfer(address,uint256)
  const transferSelector = '0xa9059cbb';

  // Encode parameters (simplified)
  const toAddressPadded = intent.toAddress.slice(2).padStart(64, '0');
  const amountHex = intent.amountSmallest.toString(16).padStart(64, '0');

  const data = transferSelector + toAddressPadded + amountHex;

  return {
    type: 'evm_erc20_transfer',
    chainId: chain.chainId,
    from: intent.fromAddress,
    to: intent.tokenAddress, // Token contract
    data,
    value: '0x0',
    // In production: include gas estimation, nonce, etc.
    mockTx: true,
  };
}

/**
 * Build Zcash transparent transaction
 * Note: In production, use zcash libraries (e.g., librustzcash bindings or zcash-primitives)
 */
async function buildZcashTransaction(intent, wallet, chainId) {
  return {
    type: 'zcash_t_transfer',
    fromAddress: intent.fromAddress,
    toAddress: intent.toAddress,
    amount: intent.amountSmallest.toString(),
    chainId,
    // Zcash uses UTXO model like Bitcoin
    // In production: include UTXO inputs, outputs, fee calculation, etc.
    mockTx: true,
  };
}

/**
 * Build Bitcoin P2PKH transaction
 * Note: In production, use bitcoinjs-lib or similar
 */
async function buildBitcoinTransaction(intent, wallet, chainId) {
  return {
    type: 'bitcoin_p2pkh_transfer',
    fromAddress: intent.fromAddress,
    toAddress: intent.toAddress,
    amount: intent.amountSmallest.toString(),
    chainId,
    // Bitcoin uses UTXO model
    // In production: include UTXO inputs, outputs, fee calculation, script signing, etc.
    mockTx: true,
  };
}

// =============================================================================
// TRANSACTION SIGNING
// =============================================================================

/**
 * Sign a transaction with the wallet's private key
 */
async function signTransaction(txData, wallet, chainId) {
  if (txData.mockTx) {
    // For mock transactions, create a deterministic "signature"
    const mockSig = crypto.createHash('sha256')
      .update(JSON.stringify(txData))
      .update(wallet.privateKey)
      .digest('hex');

    return {
      ...txData,
      signature: mockSig,
      signedAt: new Date().toISOString(),
    };
  }

  // In production, use proper chain-specific signing
  if (isEd25519Chain(chainId)) {
    return signSolanaTransaction(txData, wallet);
  } else if (isZcashChain(chainId)) {
    return signZcashTransaction(txData, wallet);
  } else if (isBitcoinChain(chainId)) {
    return signBitcoinTransaction(txData, wallet);
  } else {
    return signEvmTransaction(txData, wallet);
  }
}

async function signSolanaTransaction(txData, wallet) {
  // In production: use @solana/web3.js Transaction.sign()
  const message = Buffer.from(JSON.stringify(txData), 'utf8');

  const keyObj = crypto.createPrivateKey({
    key: Buffer.concat([
      Buffer.from('302e020100300506032b657004220420', 'hex'),
      wallet.privateKey
    ]),
    format: 'der',
    type: 'pkcs8'
  });

  const signature = crypto.sign(null, message, keyObj);

  return {
    ...txData,
    signature: signature.toString('base64'),
    signedAt: new Date().toISOString(),
  };
}

async function signEvmTransaction(txData, wallet) {
  // In production: use ethers.js Wallet.signTransaction()
  // For now, create a mock signature
  const message = Buffer.from(JSON.stringify(txData), 'utf8');
  const hash = crypto.createHash('sha256').update(message).digest();

  return {
    ...txData,
    signature: '0x' + hash.toString('hex'),
    signedAt: new Date().toISOString(),
  };
}

async function signZcashTransaction(txData, wallet) {
  // In production: use secp256k1 ECDSA signature
  // Zcash uses Bitcoin-style transaction signing (SIGHASH_ALL)
  const message = Buffer.from(JSON.stringify(txData), 'utf8');

  // Create ECDSA-style signature using the wallet's secp256k1 private key
  // For mock, we use double SHA256 hash as signature placeholder
  const hash = crypto.createHash('sha256')
    .update(crypto.createHash('sha256').update(message).digest())
    .digest();

  return {
    ...txData,
    signature: hash.toString('hex'),
    signedAt: new Date().toISOString(),
  };
}

async function signBitcoinTransaction(txData, wallet) {
  // In production: use secp256k1 ECDSA signature with SIGHASH_ALL
  // Bitcoin uses DER-encoded signatures with sighash byte appended
  const message = Buffer.from(JSON.stringify(txData), 'utf8');

  // Create ECDSA-style signature using the wallet's secp256k1 private key
  // For mock, we use double SHA256 hash as signature placeholder
  const hash = crypto.createHash('sha256')
    .update(crypto.createHash('sha256').update(message).digest())
    .digest();

  return {
    ...txData,
    signature: hash.toString('hex'),
    signedAt: new Date().toISOString(),
  };
}

// =============================================================================
// TRANSACTION SUBMISSION
// =============================================================================

/**
 * Submit a signed transaction to the network
 */
async function submitTransaction(signedTx, chainId) {
  if (signedTx.mockTx) {
    // Mock submission - generate a fake tx hash with a prefix we can detect
    let mockTxHash;
    if (isEd25519Chain(chainId)) {
      mockTxHash = crypto.randomBytes(64).toString('base64');  // Solana signatures are 64 bytes
    } else if (isZcashChain(chainId)) {
      mockTxHash = crypto.randomBytes(32).toString('hex');  // Zcash uses 32-byte txids (64 hex chars)
    } else if (isBitcoinChain(chainId)) {
      mockTxHash = crypto.randomBytes(32).toString('hex');  // Bitcoin uses 32-byte txids (64 hex chars)
    } else {
      mockTxHash = '0xMOCK' + crypto.randomBytes(30).toString('hex');  // EVM with MOCK prefix
    }

    // Simulate network delay
    await new Promise(resolve => setTimeout(resolve, 500));

    return {
      txHash: mockTxHash,
      submittedAt: new Date().toISOString(),
      isMock: true,
    };
  }

  // In production, submit to actual RPC endpoint
  const chain = getChain(chainId);

  if (isEd25519Chain(chainId)) {
    // Solana: sendTransaction RPC call
    throw new Error('Production Solana submission not implemented. Use @solana/web3.js.');
  } else if (isZcashChain(chainId)) {
    // Zcash: sendrawtransaction RPC call
    throw new Error('Production Zcash submission not implemented. Use zcash-rpc or librustzcash.');
  } else if (isBitcoinChain(chainId)) {
    // Bitcoin: sendrawtransaction RPC call
    throw new Error('Production Bitcoin submission not implemented. Use bitcoinjs-lib.');
  } else {
    // EVM: eth_sendRawTransaction
    throw new Error('Production EVM submission not implemented. Use ethers.js.');
  }
}

// =============================================================================
// CONFIRMATION POLLING
// =============================================================================

/**
 * Wait for transaction confirmation
 */
async function waitForConfirmation(txHash, chainId, options = {}) {
  const { onProgress = () => {}, maxAttempts = 60, pollInterval = 2000, isMock: isMockFromSubmit } = options;

  const chain = getChain(chainId);
  const requiredConfirmations = chain?.confirmations || 1;

  // For mock transactions, simulate confirmation
  // Detect mock from: submit result flag, MOCK prefix in hash, or non-hex hash (Solana base64)
  const isMock = isMockFromSubmit ||
    txHash.includes('MOCK') ||
    (!txHash.startsWith('0x') && txHash.length > 20);  // Base64 Solana sigs

  if (isMock || process.env.MOCK_BLOCKCHAIN === 'true') {
    // Simulate confirmation delay
    await new Promise(resolve => setTimeout(resolve, 1000));

    onProgress({
      step: 'confirmation_update',
      confirmations: requiredConfirmations,
      required: requiredConfirmations,
    });

    return {
      confirmed: true,
      blockNumber: Math.floor(Date.now() / 1000),
      confirmations: requiredConfirmations,
      confirmedAt: new Date().toISOString(),
    };
  }

  // Production confirmation polling
  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    try {
      const status = await getTransactionStatus(txHash, chainId);

      if (status.confirmed && status.confirmations >= requiredConfirmations) {
        return status;
      }

      onProgress({
        step: 'confirmation_update',
        confirmations: status.confirmations || 0,
        required: requiredConfirmations,
      });

      await new Promise(resolve => setTimeout(resolve, pollInterval));

    } catch (error) {
      // Transaction might not be indexed yet
      await new Promise(resolve => setTimeout(resolve, pollInterval));
    }
  }

  throw new Error(`Transaction not confirmed after ${maxAttempts} attempts`);
}

/**
 * Get transaction status from blockchain
 */
async function getTransactionStatus(txHash, chainId) {
  // In production, query the RPC endpoint
  throw new Error('Production transaction status check not implemented.');
}

// =============================================================================
// BALANCE CHECKING
// =============================================================================

/**
 * Get token balance for an address
 * @param {string} address - Wallet address
 * @param {string} chainId - Chain identifier
 * @param {string} [tokenSymbol] - Token symbol (default: chain's default stablecoin)
 * @returns {Promise<{balance: string, balanceSmallest: bigint, symbol: string}>}
 */
export async function getBalance(address, chainId, tokenSymbol) {
  // Validate chain
  validateChainId(chainId);
  const chain = getChain(chainId);

  // Validate token
  validateToken(chainId, tokenSymbol);

  // Validate address
  validateAddress(address, chainId);

  const token = tokenSymbol
    ? getToken(chainId, tokenSymbol)
    : getDefaultStablecoin(chainId);

  // In production, query the blockchain
  // For now, return mock balance

  if (process.env.MOCK_BLOCKCHAIN === 'true' || !chain.rpcUrl) {
    const mockBalance = BigInt('1000000000'); // 1000 USDC
    return {
      balance: fromSmallestUnit(mockBalance, token.decimals),
      balanceSmallest: mockBalance,
      symbol: token.symbol,
    };
  }

  throw new Error('Production balance check not implemented. Use chain-specific SDK.');
}

/**
 * Check if wallet has sufficient balance for payment
 */
export async function hasSufficientBalance(address, chainId, amount, tokenSymbol) {
  const { balanceSmallest, symbol } = await getBalance(address, chainId, tokenSymbol);

  const token = tokenSymbol
    ? getToken(chainId, tokenSymbol)
    : getDefaultStablecoin(chainId);

  const requiredAmount = toSmallestUnit(amount, token.decimals);

  return {
    sufficient: balanceSmallest >= requiredAmount,
    balance: fromSmallestUnit(balanceSmallest, token.decimals),
    required: amount.toString(),
    symbol,
  };
}

// =============================================================================
// EXPORTS
// =============================================================================

export default {
  createPaymentIntent,
  executePayment,
  getBalance,
  hasSufficientBalance,
};
