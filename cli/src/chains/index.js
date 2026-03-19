/**
 * StateSet iCommerce Blockchain Integration
 *
 * Provides native blockchain payment capabilities for AI agents.
 *
 * Supports:
 * - Solana (USDC) - Fast, cheap, proven liquidity
 * - SET Chain (ssUSD) - StateSet native, yield-bearing stablecoin
 * - Base (USDC) - Coinbase L2, low fees
 * - Ethereum (USDC) - Maximum security and liquidity
 * - Bitcoin (BTC) - Native UTXO payments
 * - Zcash (ZEC) - Shielded payments via wallet RPC
 */

// Chain configuration
export {
  CHAINS,
  getChain,
  getToken,
  getDefaultStablecoin,
  getDefaultPaymentToken,
  getExplorerTxUrl,
  getExplorerAddressUrl,
  toSmallestUnit,
  fromSmallestUnit,
  formatAmount,
  isEd25519Chain,
  isEvmChain,
  isZcashChain,
  isBitcoinChain,
  listChains,
  getRecommendedChain,
} from './config.js';

// Wallet derivation
export {
  deriveWallet,
  getOrCreateWallet,
  getWalletAddress,
  listWalletAddresses,
  DERIVATION_PATHS,
  base58Encode,
  base58Decode,
  compressPublicKey,
} from './wallet.js';

// Chain payments
export {
  createPaymentIntent,
  executePayment,
  getTransactionStatus,
  getBalance,
  hasSufficientBalance,
} from './stablecoin.js';

// Input validation
export {
  ValidationError,
  ValidationErrorCodes,
  validateChainId,
  validateToken,
  validateAmount,
  validateAddress,
  validateEvmAddress,
  validateSolanaAddress,
  validateZcashAddress,
  validateBitcoinAddress,
  validatePaymentParams,
} from './validation.js';

// Crypto utilities
export {
  keccak256,
  secp256k1GetPublicKey,
  privateKeyToEthAddress,
  toChecksumAddress,
  isValidEthAddress,
  ripemd160,
  sha256Double,
} from './crypto-utils.js';

// Default export with all functionality
import chainConfig from './config.js';
import walletModule from './wallet.js';
import stablecoinModule from './stablecoin.js';
import validationModule from './validation.js';
import cryptoUtilsModule from './crypto-utils.js';

export default {
  ...chainConfig,
  ...walletModule,
  ...stablecoinModule,
  ...validationModule,
  ...cryptoUtilsModule,
};
