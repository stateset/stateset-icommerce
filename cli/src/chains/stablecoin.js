/**
 * Stablecoin Payment Execution for StateSet iCommerce
 *
 * Live settlement is supported on EVM chains, Solana, Bitcoin, and shielded
 * Zcash (via wallet-enabled JSON-RPC backend).
 */

import { randomUUID } from 'node:crypto';
import { Contract, JsonRpcProvider, Wallet } from 'ethers';
import { deriveWallet, getWalletAddress } from './wallet.js';
import {
  getChain,
  getToken,
  getDefaultPaymentToken,
  toSmallestUnit,
  fromSmallestUnit,
  getExplorerTxUrl,
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
import {
  buildBitcoinTransaction,
  signBitcoinTransaction,
  submitBitcoinTransaction,
  getBitcoinTransactionStatus,
  getBitcoinBalance,
} from './bitcoin.js';
import {
  executeZcashShieldedPayment,
  getZcashBalance,
  getZcashTransactionStatus,
  isZcashWalletRpcConfigured,
} from './zcash.js';

const ERC20_ABI = [
  'function transfer(address to, uint256 amount) returns (bool)',
  'function balanceOf(address owner) view returns (uint256)',
];

const DEFAULT_CONFIRMATION_ATTEMPTS = 60;
const DEFAULT_POLL_INTERVAL_MS = 2_000;
const MAX_SAFE_U64_AS_NUMBER = BigInt(Number.MAX_SAFE_INTEGER);
const SOLANA_COMMITMENT = 'confirmed';

let solanaSdkPromise = null;

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function requireEvmExecution(chainId) {
  if (!isEvmChain(chainId)) {
    throw new Error(
      `EVM execution expected for chain ${chainId}. Supported live settlement targets are EVM chains, Solana, Bitcoin, and shielded Zcash.`,
    );
  }
}

function isSolanaChain(chainId) {
  return chainId === 'solana' || chainId === 'solana_devnet';
}

function requireSolanaExecution(chainId) {
  if (!isSolanaChain(chainId)) {
    throw new Error(
      `Live Solana execution is only available on solana/solana_devnet. ${chainId} is not supported.`,
    );
  }
}

function createEvmProvider(chainId) {
  const chain = getChain(chainId);
  if (!chain?.rpcUrl) {
    throw new Error(`RPC URL is not configured for chain ${chainId}`);
  }
  return new JsonRpcProvider(chain.rpcUrl, chain.chainId || undefined);
}

async function loadSolanaSdk() {
  if (!solanaSdkPromise) {
    solanaSdkPromise = Promise.all([import('@solana/web3.js'), import('@solana/spl-token')])
      .then(([web3, splToken]) => ({ web3, splToken }))
      .catch((error) => {
        solanaSdkPromise = null;
        throw new Error(
          `Solana support requires @solana/web3.js and @solana/spl-token. Install them in cli/: ${error.message}`,
        );
      });
  }
  return solanaSdkPromise;
}

async function createSolanaConnection(chainId) {
  requireSolanaExecution(chainId);
  const chain = getChain(chainId);
  if (!chain?.rpcUrl) {
    throw new Error(`RPC URL is not configured for chain ${chainId}`);
  }
  const { web3 } = await loadSolanaSdk();
  return new web3.Connection(chain.rpcUrl, SOLANA_COMMITMENT);
}

async function createSolanaSigner(wallet, chainId) {
  requireSolanaExecution(chainId);
  const { web3 } = await loadSolanaSdk();
  if (!wallet?.privateKey || wallet.privateKey.length !== 32) {
    throw new Error('Invalid Solana private key material');
  }

  const signer = web3.Keypair.fromSeed(Uint8Array.from(wallet.privateKey));
  const expectedAddress = signer.publicKey.toBase58();
  if (expectedAddress !== wallet.address) {
    throw new Error('Derived wallet address does not match Solana signer public key');
  }

  return signer;
}

function createEvmSigner(wallet, provider) {
  const privateKeyHex = `0x${wallet.privateKey.toString('hex')}`;
  const signer = new Wallet(privateKeyHex, provider);

  if (signer.address.toLowerCase() !== wallet.address.toLowerCase()) {
    throw new Error('Derived wallet address does not match EVM signer address');
  }

  return signer;
}

function applyFeeData(request, feeData) {
  if (
    feeData?.maxFeePerGas !== null &&
    feeData?.maxFeePerGas !== undefined &&
    feeData?.maxPriorityFeePerGas !== null &&
    feeData?.maxPriorityFeePerGas !== undefined
  ) {
    request.maxFeePerGas = feeData.maxFeePerGas;
    request.maxPriorityFeePerGas = feeData.maxPriorityFeePerGas;
    return;
  }

  if (feeData?.gasPrice !== null && feeData?.gasPrice !== undefined) {
    request.gasPrice = feeData.gasPrice;
  }
}

function addGasMargin(estimatedGas) {
  const estimate =
    typeof estimatedGas === 'bigint' ? estimatedGas : BigInt(estimatedGas.toString());
  const extra = estimate / 5n; // 20% buffer
  return estimate + (extra > 0n ? extra : 1n);
}

function buildUnsupportedPreviewTransaction(intent, chainId) {
  return {
    type: 'unsupported_live_chain',
    chainId,
    fromAddress: intent.fromAddress,
    toAddress: intent.toAddress,
    amountSmallest: intent.amountSmallest.toString(),
    reason:
      'Simulation only: live execution is currently implemented for EVM, Solana, Bitcoin, and shielded Zcash chains.',
  };
}

/**
 * @typedef {Object} PaymentIntent
 * @property {string} intentId
 * @property {string} chainId
 * @property {string} tokenSymbol
 * @property {string} tokenAddress
 * @property {number} tokenDecimals
 * @property {string} fromAddress
 * @property {string} toAddress
 * @property {string} amount
 * @property {bigint} amountSmallest
 * @property {string} currency
 * @property {Object} [metadata]
 * @property {string} createdAt
 * @property {string} status
 */

/**
 * @typedef {Object} PaymentResult
 * @property {boolean} success
 * @property {string | undefined} intentId
 * @property {string} [txHash]
 * @property {string} [txSignature]
 * @property {string} [explorerUrl]
 * @property {number} [blockNumber]
 * @property {number} [confirmations]
 * @property {string} [error]
 * @property {boolean} [simulated]
 * @property {PaymentIntent} [intent]
 * @property {unknown} [txPreview]
 * @property {Array<string>} vesEventIds
 */

/**
 * Create a payment intent
 * @param {Object} params
 * @param {string} params.agentId
 * @param {string} params.chainId
 * @param {string} params.toAddress
 * @param {string|number} params.amount
 * @param {string} [params.tokenSymbol]
 * @param {Object} [params.metadata]
 * @param {Object} [options]
 * @param {string} [options.configDir]
 * @returns {Promise<PaymentIntent>}
 */
export async function createPaymentIntent(params, options = {}) {
  const { agentId, chainId, toAddress, amount, tokenSymbol, metadata = {} } = params;
  const { configDir = '.stateset' } = options;

  if (!agentId || typeof agentId !== 'string') {
    throw new ValidationError(ValidationErrorCodes.MISSING_REQUIRED, 'Agent ID is required', {
      field: 'agentId',
    });
  }

  validateChainId(chainId);
  getChain(chainId);

  validateToken(chainId, tokenSymbol);
  const token = tokenSymbol ? getToken(chainId, tokenSymbol) : getDefaultPaymentToken(chainId);

  validateAmount(amount);
  validateAddress(toAddress, chainId);

  const fromAddress = await getWalletAddress(agentId, chainId, { configDir });

  const caseInsensitiveAddressCompare = isEvmChain(chainId);
  const fromNormalized = caseInsensitiveAddressCompare ? fromAddress.toLowerCase() : fromAddress;
  const toNormalized = caseInsensitiveAddressCompare ? toAddress.toLowerCase() : toAddress;
  if (fromNormalized === toNormalized) {
    throw new ValidationError(
      ValidationErrorCodes.SELF_TRANSFER,
      'Cannot transfer to the same address',
      { fromAddress, toAddress },
    );
  }

  const amountSmallest = toSmallestUnit(amount, token.decimals);
  const normalizedAmount = fromSmallestUnit(amountSmallest, token.decimals);

  return {
    intentId: randomUUID(),
    chainId,
    tokenSymbol: token.symbol,
    tokenAddress: token.address,
    tokenDecimals: token.decimals,
    fromAddress,
    toAddress,
    amount: normalizedAmount,
    amountSmallest,
    currency: 'USD',
    metadata: {
      ...metadata,
      agentId,
    },
    createdAt: new Date().toISOString(),
    status: 'pending',
  };
}

/**
 * Execute a blockchain payment
 * @param {Object} params
 * @param {string} params.agentId
 * @param {string} params.chainId
 * @param {string} params.toAddress
 * @param {string|number} params.amount
 * @param {string} [params.tokenSymbol]
 * @param {Object} [params.metadata]
 * @param {Object} [options]
 * @param {string} [options.configDir]
 * @param {boolean} [options.simulate]
 * @param {Function} [options.onProgress]
 * @returns {Promise<PaymentResult>}
 */
export async function executePayment(params, options = {}) {
  const { agentId, chainId, toAddress, amount, tokenSymbol, metadata = {} } = params;
  const { configDir = '.stateset', simulate = false, onProgress = () => {} } = options;

  validateChainId(chainId);
  validateToken(chainId, tokenSymbol);
  validateAmount(amount);
  validateAddress(toAddress, chainId);

  if (isZcashChain(chainId)) {
    onProgress({
      step: 'creating_intent',
      message: 'Creating shielded Zcash payment...',
    });
    return executeZcashShieldedPayment(
      {
        agentId,
        chainId,
        toAddress,
        amount,
        tokenSymbol,
        metadata,
      },
      {
        configDir,
        simulate,
        onProgress,
      },
    );
  }

  const vesEventIds = [];
  let intent;

  try {
    onProgress({ step: 'creating_intent', message: 'Creating payment intent...' });
    intent = await createPaymentIntent(
      {
        agentId,
        chainId,
        toAddress,
        amount,
        tokenSymbol,
        metadata,
      },
      { configDir },
    );

    onProgress({
      step: 'intent_created',
      message: `Payment intent created: ${intent.intentId}`,
      intent,
    });

    onProgress({ step: 'deriving_wallet', message: 'Deriving agent wallet...' });
    const wallet = await deriveWallet(agentId, chainId, { configDir });

    onProgress({
      step: 'wallet_derived',
      message: `Wallet: ${wallet.address}`,
    });

    onProgress({ step: 'building_tx', message: 'Building transaction...' });
    const txData = await buildTransaction(intent, wallet, chainId, { simulate });

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
        txPreview: txData,
        vesEventIds,
      };
    }

    onProgress({ step: 'signing', message: 'Signing transaction...' });
    const signedTx = await signTransaction(txData, wallet, chainId);
    intent.status = 'signed';

    onProgress({ step: 'submitting', message: 'Submitting to network...' });
    const submitResult = await submitTransaction(signedTx, chainId);
    intent.status = 'submitted';

    onProgress({
      step: 'submitted',
      message: `Transaction submitted: ${submitResult.txHash}`,
      txHash: submitResult.txHash,
    });

    onProgress({ step: 'confirming', message: 'Waiting for confirmation...' });
    const confirmation = await waitForConfirmation(submitResult.txHash, chainId, {
      onProgress,
      ...submitResult,
    });
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

async function buildTransaction(intent, wallet, chainId, options = {}) {
  const { simulate = false } = options;

  if (isEvmChain(chainId)) {
    return buildEvmTransaction(intent, wallet, chainId);
  }

  if (isSolanaChain(chainId)) {
    return buildSolanaTransaction(intent, wallet, chainId);
  }

  if (isBitcoinChain(chainId)) {
    return buildBitcoinTransaction(intent, wallet, chainId, { simulate });
  }

  if (simulate) {
    return buildUnsupportedPreviewTransaction(intent, chainId);
  }

  throw new Error(
    `Live payment execution is not implemented for chain ${chainId}. Supported live chains: EVM, Solana, Bitcoin, and shielded Zcash.`,
  );
}

function ensureSafeNumberAmount(amount, fieldLabel) {
  if (amount > MAX_SAFE_U64_AS_NUMBER) {
    throw new Error(
      `${fieldLabel} exceeds JavaScript safe integer range and cannot be submitted safely`,
    );
  }
  return Number(amount);
}

async function buildSolanaTransaction(intent, wallet, chainId) {
  requireSolanaExecution(chainId);
  const connection = await createSolanaConnection(chainId);
  const signer = await createSolanaSigner(wallet, chainId);
  const { web3, splToken } = await loadSolanaSdk();

  const fromPubkey = signer.publicKey;
  const toPubkey = new web3.PublicKey(intent.toAddress);
  const transaction = new web3.Transaction();

  if (!intent.tokenAddress || intent.tokenAddress === 'native') {
    const lamports = ensureSafeNumberAmount(intent.amountSmallest, 'SOL transfer amount');
    transaction.add(
      web3.SystemProgram.transfer({
        fromPubkey,
        toPubkey,
        lamports,
      }),
    );

    const latestBlockhash = await connection.getLatestBlockhash(SOLANA_COMMITMENT);
    transaction.recentBlockhash = latestBlockhash.blockhash;
    transaction.feePayer = fromPubkey;

    return {
      type: 'solana_native_transfer',
      connection,
      transaction,
      signer,
      latestBlockhash,
    };
  }

  const mint = new web3.PublicKey(intent.tokenAddress);
  const sourceAta = splToken.getAssociatedTokenAddressSync(
    mint,
    fromPubkey,
    false,
    splToken.TOKEN_PROGRAM_ID,
    splToken.ASSOCIATED_TOKEN_PROGRAM_ID,
  );
  const destinationAta = splToken.getAssociatedTokenAddressSync(
    mint,
    toPubkey,
    false,
    splToken.TOKEN_PROGRAM_ID,
    splToken.ASSOCIATED_TOKEN_PROGRAM_ID,
  );

  const sourceAccount = await connection.getAccountInfo(sourceAta);
  if (!sourceAccount) {
    throw new Error(
      `Source token account does not exist for ${intent.tokenSymbol}. Fund ${sourceAta.toBase58()} first.`,
    );
  }

  const destinationAccount = await connection.getAccountInfo(destinationAta);
  if (!destinationAccount) {
    transaction.add(
      splToken.createAssociatedTokenAccountInstruction(
        fromPubkey,
        destinationAta,
        toPubkey,
        mint,
        splToken.TOKEN_PROGRAM_ID,
        splToken.ASSOCIATED_TOKEN_PROGRAM_ID,
      ),
    );
  }

  transaction.add(
    splToken.createTransferInstruction(
      sourceAta,
      destinationAta,
      fromPubkey,
      intent.amountSmallest,
      [],
      splToken.TOKEN_PROGRAM_ID,
    ),
  );

  const latestBlockhash = await connection.getLatestBlockhash(SOLANA_COMMITMENT);
  transaction.recentBlockhash = latestBlockhash.blockhash;
  transaction.feePayer = fromPubkey;

  return {
    type: 'solana_spl_transfer',
    connection,
    transaction,
    signer,
    latestBlockhash,
  };
}

async function buildEvmTransaction(intent, wallet, chainId) {
  requireEvmExecution(chainId);
  const chain = getChain(chainId);
  const provider = createEvmProvider(chainId);
  const signer = createEvmSigner(wallet, provider);
  const feeData = await provider.getFeeData();
  const nonce = await provider.getTransactionCount(intent.fromAddress, 'pending');

  const request = {
    chainId: chain.chainId,
    nonce,
    from: intent.fromAddress,
  };

  if (!intent.tokenAddress || intent.tokenAddress === 'native') {
    request.to = intent.toAddress;
    request.value = intent.amountSmallest;
    const gasEstimate = await provider.estimateGas(request);
    request.gasLimit = addGasMargin(gasEstimate);
    applyFeeData(request, feeData);

    return {
      type: 'evm_native_transfer',
      request,
      signerAddress: signer.address,
    };
  }

  const contract = new Contract(intent.tokenAddress, ERC20_ABI, provider);
  const populated = await contract.transfer.populateTransaction(
    intent.toAddress,
    intent.amountSmallest,
  );

  request.to = intent.tokenAddress;
  request.data = populated.data || '0x';
  request.value = 0n;
  const gasEstimate = await provider.estimateGas(request);
  request.gasLimit = addGasMargin(gasEstimate);
  applyFeeData(request, feeData);

  return {
    type: 'evm_erc20_transfer',
    request,
    signerAddress: signer.address,
  };
}

async function signTransaction(txData, wallet, chainId) {
  if (isSolanaChain(chainId)) {
    const signer = await createSolanaSigner(wallet, chainId);
    txData.transaction.sign(signer);
    const serializedTransaction = txData.transaction.serialize();

    return {
      ...txData,
      serializedTransaction,
      signedAt: new Date().toISOString(),
    };
  }

  if (isBitcoinChain(chainId)) {
    return signBitcoinTransaction(txData, wallet, chainId);
  }

  requireEvmExecution(chainId);

  const signer = createEvmSigner(wallet);
  const signedRawTransaction = await signer.signTransaction(txData.request);

  return {
    ...txData,
    signedRawTransaction,
    signedAt: new Date().toISOString(),
  };
}

async function submitTransaction(signedTx, chainId) {
  if (isSolanaChain(chainId)) {
    const connection = signedTx.connection || (await createSolanaConnection(chainId));
    const txHash = await connection.sendRawTransaction(signedTx.serializedTransaction, {
      skipPreflight: false,
      preflightCommitment: SOLANA_COMMITMENT,
      maxRetries: 3,
    });

    return {
      txHash,
      submittedAt: new Date().toISOString(),
      connection,
      latestBlockhash: signedTx.latestBlockhash || null,
    };
  }

  if (isBitcoinChain(chainId)) {
    return submitBitcoinTransaction(signedTx, chainId);
  }

  requireEvmExecution(chainId);

  const provider = createEvmProvider(chainId);
  const response = await provider.broadcastTransaction(signedTx.signedRawTransaction);

  return {
    txHash: response.hash,
    submittedAt: new Date().toISOString(),
  };
}

async function waitForConfirmation(txHash, chainId, options = {}) {
  const chain = getChain(chainId);
  const onProgress = options.onProgress || (() => {});
  const pollInterval =
    chain?.confirmationPollIntervalMs || options.pollInterval || DEFAULT_POLL_INTERVAL_MS;
  const maxAttempts =
    chain?.maxConfirmationAttempts || options.maxAttempts || DEFAULT_CONFIRMATION_ATTEMPTS;

  if (!isSolanaChain(chainId) && !isBitcoinChain(chainId)) {
    requireEvmExecution(chainId);
  }

  const requiredConfirmations = chain?.executionConfirmations || chain?.confirmations || 1;

  for (let attempt = 0; attempt < maxAttempts; attempt++) {
    const status = await getTransactionStatus(txHash, chainId, options);

    if (status.confirmed && status.confirmations >= requiredConfirmations) {
      return status;
    }

    onProgress({
      step: 'confirmation_update',
      confirmations: status.confirmations || 0,
      required: requiredConfirmations,
    });

    await sleep(pollInterval);
  }

  throw new Error(`Transaction not confirmed after ${maxAttempts} attempts`);
}

export async function getTransactionStatus(txHash, chainId, options = {}) {
  if (isSolanaChain(chainId)) {
    const connection = options.connection || (await createSolanaConnection(chainId));
    const response = await connection.getSignatureStatuses([txHash], {
      searchTransactionHistory: true,
    });
    const status = response?.value?.[0] || null;

    if (!status) {
      return {
        confirmed: false,
        confirmations: 0,
      };
    }

    if (status.err) {
      throw new Error(`Transaction ${txHash} failed on-chain: ${JSON.stringify(status.err)}`);
    }

    const chain = getChain(chainId);
    const requiredConfirmations = chain?.confirmations || 1;
    const confirmationStatus = status.confirmationStatus || null;
    const rawConfirmations = status.confirmations;
    const finalized = confirmationStatus === 'finalized';
    const confirmations =
      rawConfirmations === null
        ? finalized
          ? requiredConfirmations
          : 0
        : Number(rawConfirmations || 0);
    const confirmed = finalized || confirmations >= requiredConfirmations;

    return {
      confirmed,
      blockNumber: status.slot,
      confirmations,
      confirmedAt: new Date().toISOString(),
    };
  }

  if (isBitcoinChain(chainId)) {
    return getBitcoinTransactionStatus(txHash, chainId, options);
  }

  if (isZcashChain(chainId)) {
    if (!isZcashWalletRpcConfigured(chainId)) {
      throw new Error(
        `Zcash transaction status lookup requires a wallet-enabled JSON-RPC endpoint for ${chainId}.`,
      );
    }
    return getZcashTransactionStatus(txHash, chainId, options);
  }

  requireEvmExecution(chainId);

  const provider = createEvmProvider(chainId);
  const receipt = await provider.getTransactionReceipt(txHash);

  if (!receipt) {
    return {
      confirmed: false,
      confirmations: 0,
    };
  }

  if (receipt.status === 0) {
    throw new Error(`Transaction ${txHash} reverted on-chain`);
  }

  const latestBlock = await provider.getBlockNumber();
  const confirmations = Math.max(0, latestBlock - receipt.blockNumber + 1);

  return {
    confirmed: true,
    blockNumber: receipt.blockNumber,
    confirmations,
    confirmedAt: new Date().toISOString(),
  };
}

/**
 * Get token balance for an address
 * @param {string} address
 * @param {string} chainId
 * @param {string} [tokenSymbol]
 * @param {Object} [options]
 * @param {string} [options.configDir]
 * @returns {Promise<{balance: string, balanceSmallest: bigint, symbol: string}>}
 */
export async function getBalance(address, chainId, tokenSymbol, options = {}) {
  validateChainId(chainId);
  validateToken(chainId, tokenSymbol);
  validateAddress(address, chainId);

  const token = tokenSymbol ? getToken(chainId, tokenSymbol) : getDefaultPaymentToken(chainId);

  let balanceSmallest;
  if (isSolanaChain(chainId)) {
    const connection = await createSolanaConnection(chainId);
    const { web3, splToken } = await loadSolanaSdk();
    const owner = new web3.PublicKey(address);

    if (!token.address || token.address === 'native') {
      const rawBalance = await connection.getBalance(owner, SOLANA_COMMITMENT);
      balanceSmallest = BigInt(rawBalance.toString());
    } else {
      const mint = new web3.PublicKey(token.address);
      const tokenAccount = splToken.getAssociatedTokenAddressSync(
        mint,
        owner,
        false,
        splToken.TOKEN_PROGRAM_ID,
        splToken.ASSOCIATED_TOKEN_PROGRAM_ID,
      );
      const tokenAccountInfo = await connection.getAccountInfo(tokenAccount);
      if (!tokenAccountInfo) {
        balanceSmallest = 0n;
      } else {
        const raw = await connection.getTokenAccountBalance(tokenAccount, SOLANA_COMMITMENT);
        balanceSmallest = BigInt(raw.value.amount);
      }
    }
  } else if (isBitcoinChain(chainId)) {
    balanceSmallest = await getBitcoinBalance(address, chainId);
  } else if (isZcashChain(chainId)) {
    balanceSmallest = await getZcashBalance(address, chainId, options);
  } else {
    requireEvmExecution(chainId);
    const provider = createEvmProvider(chainId);

    let rawBalance;
    if (!token.address || token.address === 'native') {
      rawBalance = await provider.getBalance(address);
    } else {
      const contract = new Contract(token.address, ERC20_ABI, provider);
      rawBalance = await contract.balanceOf(address);
    }

    balanceSmallest = typeof rawBalance === 'bigint' ? rawBalance : BigInt(rawBalance.toString());
  }
  return {
    balance: fromSmallestUnit(balanceSmallest, token.decimals),
    balanceSmallest,
    symbol: token.symbol,
  };
}

/**
 * Check if wallet has sufficient balance for payment
 */
export async function hasSufficientBalance(address, chainId, amount, tokenSymbol, options = {}) {
  const { balanceSmallest, symbol } = await getBalance(address, chainId, tokenSymbol, options);
  const token = tokenSymbol ? getToken(chainId, tokenSymbol) : getDefaultPaymentToken(chainId);
  const requiredAmount = toSmallestUnit(amount, token.decimals);

  return {
    sufficient: balanceSmallest >= requiredAmount,
    balance: fromSmallestUnit(balanceSmallest, token.decimals),
    required: amount.toString(),
    symbol,
  };
}

export default {
  createPaymentIntent,
  executePayment,
  getTransactionStatus,
  getBalance,
  hasSufficientBalance,
};
