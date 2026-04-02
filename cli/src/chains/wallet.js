/**
 * Agent wallet management for blockchain payments.
 *
 * Derives blockchain wallets from VES Ed25519 keys using deterministic
 * chain-specific paths and address encodings.
 *
 * Based on VES-CHAIN-1 specification from stateset-sequencer.
 */

import crypto from 'crypto';
import { getKeyManager } from '../sync/keys.js';
import { CHAINS, isEd25519Chain, isEvmChain, isZcashChain, isBitcoinChain } from './config.js';
import { encodeSegwitAddress } from './bitcoin-address.js';
import {
  privateKeyToEthAddress,
  secp256k1GetPublicKey,
  ripemd160,
  sha256Double,
} from './crypto-utils.js';

// =============================================================================
// BIP-44 DERIVATION PATHS (per VES-CHAIN-1)
// =============================================================================

const DERIVATION_PATHS = {
  // VES keys (signing and encryption)
  ves_signing: "m/44'/9999'", // VES signing keys
  ves_encryption: "m/44'/9998'", // VES encryption keys

  // Blockchain-specific paths
  solana: "m/44'/501'",
  near: "m/44'/397'",
  stellar: "m/44'/148'",
  cosmos: "m/44'/118'",
  aptos: "m/44'/637'",
  sui: "m/44'/784'",

  // EVM chains (shared path)
  evm: "m/44'/60'",

  // Zcash (coin type 133 mainnet, 1 testnet)
  zcash: "m/44'/133'",
  zcash_testnet: "m/44'/1'",

  // Bitcoin native SegWit receive paths (BIP-84)
  bitcoin: "m/84'/0'",
  bitcoin_testnet: "m/84'/1'",
};

// =============================================================================
// WALLET DERIVATION
// =============================================================================

/**
 * @typedef {Object} DerivedWallet
 * @property {string} address - Wallet address
 * @property {Buffer} publicKey - Public key bytes
 * @property {Buffer} privateKey - Private key bytes (keep secure!)
 * @property {string} chainId - Chain identifier
 * @property {string} derivationPath - Full derivation path used
 * @property {string} [legacyAddress] - Legacy P2PKH address for compatibility
 * @property {string} [segwitAddress] - Native SegWit P2WPKH address
 */

/**
 * Derive a blockchain wallet from the agent's VES signing key
 *
 * For Ed25519 chains (Solana, NEAR, etc.), the VES key can be used directly
 * or re-derived from a seed. For EVM chains, we derive a secp256k1 key.
 *
 * @param {string} agentId - Agent identifier
 * @param {string} chainId - Chain identifier (solana, base, set_chain, etc.)
 * @param {Object} [options]
 * @param {string} [options.configDir] - Config directory (default: .stateset)
 * @returns {Promise<DerivedWallet>}
 */
export async function deriveWallet(agentId, chainId, options = {}) {
  const { configDir = '.stateset' } = options;
  const keyManager = getKeyManager(configDir);

  // Get the agent's current signing key
  const signingKey = await keyManager.getCurrentSigningKey(agentId);
  if (!signingKey) {
    throw new Error(`No signing key found for agent ${agentId}. Run key generation first.`);
  }

  const chain = CHAINS[chainId];
  if (!chain) {
    throw new Error(`Unknown chain: ${chainId}`);
  }

  // Use the signing key's private key as seed for derivation
  const seed = signingKey.privateKey;

  if (isEd25519Chain(chainId)) {
    return deriveEd25519Wallet(seed, chainId, chain);
  } else if (isZcashChain(chainId)) {
    return deriveZcashWallet(seed, chainId, chain);
  } else if (isBitcoinChain(chainId)) {
    return deriveBitcoinWallet(seed, chainId, chain);
  } else if (isEvmChain(chainId)) {
    return deriveEvmWallet(seed, chainId, chain);
  } else {
    throw new Error(`Unsupported chain type: ${chainId}`);
  }
}

/**
 * Derive Ed25519 wallet for Solana, NEAR, etc.
 * @param {Buffer} seed - 32-byte seed
 * @param {string} chainId - Chain identifier
 * @param {Object} _chain - Chain config
 * @returns {DerivedWallet}
 */
function deriveEd25519Wallet(seed, chainId, _chain) {
  // For Ed25519 chains, we can use the seed directly as the private key
  // or derive using HKDF for additional isolation

  // For Solana, the private key is the 32-byte seed, and the full keypair
  // is seed || publicKey (64 bytes total)
  let address;
  let privKeyBytes;

  if (chainId.startsWith('solana')) {
    // Solana uses base58-encoded public key as address
    // We'll use a simplified derivation here
    // In production, use @solana/web3.js Keypair

    // Derive deterministic keypair using HKDF
    const solanaPrivKey = Buffer.from(
      crypto.hkdfSync('sha256', seed, Buffer.alloc(0), Buffer.from('solana-keypair', 'utf8'), 32),
    );

    // Generate Ed25519 keypair
    const keyObj = crypto.createPrivateKey({
      key: Buffer.concat([Buffer.from('302e020100300506032b657004220420', 'hex'), solanaPrivKey]),
      format: 'der',
      type: 'pkcs8',
    });

    const pubKeyObj = crypto.createPublicKey(keyObj);
    const pubKeyDer = pubKeyObj.export({ type: 'spki', format: 'der' });
    const solanaPubKey = pubKeyDer.subarray(-32);

    // Base58 encode for Solana address
    address = base58Encode(solanaPubKey);
    privKeyBytes = solanaPrivKey;

    return {
      address,
      publicKey: solanaPubKey,
      privateKey: privKeyBytes,
      chainId,
      derivationPath: `${DERIVATION_PATHS.solana}/0'/0'`,
    };
  }

  // Generic Ed25519 (NEAR, etc.)
  const genericPrivKey = Buffer.from(
    crypto.hkdfSync('sha256', seed, Buffer.alloc(0), Buffer.from(`${chainId}-keypair`, 'utf8'), 32),
  );

  const keyObjGeneric = crypto.createPrivateKey({
    key: Buffer.concat([Buffer.from('302e020100300506032b657004220420', 'hex'), genericPrivKey]),
    format: 'der',
    type: 'pkcs8',
  });

  const pubKeyObjGeneric = crypto.createPublicKey(keyObjGeneric);
  const pubKeyDerGeneric = pubKeyObjGeneric.export({ type: 'spki', format: 'der' });
  const genericPubKey = pubKeyDerGeneric.subarray(-32);

  return {
    address: '0x' + genericPubKey.toString('hex'),
    publicKey: genericPubKey,
    privateKey: genericPrivKey,
    chainId,
    derivationPath: `${DERIVATION_PATHS[chainId] || DERIVATION_PATHS.ves_signing}/0'/0'`,
  };
}

/**
 * Derive EVM wallet (secp256k1) for Ethereum, Base, Arbitrum, SET Chain, etc.
 * @param {Buffer} seed - 32-byte seed
 * @param {string} chainId - Chain identifier
 * @param {Object} _chain - Chain config
 * @returns {DerivedWallet}
 */
function deriveEvmWallet(seed, chainId, _chain) {
  // Derive a deterministic private key from Ed25519 seed using HKDF
  // This creates a chain-specific secp256k1 private key
  const info = Buffer.from(`stateset:evm:${chainId}`, 'utf8');
  const privKeyBytes = Buffer.from(crypto.hkdfSync('sha256', seed, Buffer.alloc(0), info, 32));

  // Derive proper Ethereum address using secp256k1 and Keccak256
  // Address = 0x + last 20 bytes of Keccak256(uncompressed_pubkey[1:])
  const address = privateKeyToEthAddress(privKeyBytes);

  // Get the secp256k1 public key (65 bytes uncompressed)
  const publicKey = secp256k1GetPublicKey(privKeyBytes);

  return {
    address,
    publicKey,
    privateKey: privKeyBytes,
    chainId,
    derivationPath: `${DERIVATION_PATHS.evm}/0'/0/0`,
  };
}

/**
 * Derive an EVM wallet from raw 32-byte seed material.
 *
 * This is used by x402 exact-EVM flows where the caller already has access
 * to the agent signing seed and does not need key-manager lookup.
 *
 * @param {Buffer} seed - 32-byte Ed25519 seed
 * @param {string} chainId - EVM chain identifier
 * @returns {DerivedWallet}
 */
export function deriveEvmWalletFromSeed(seed, chainId) {
  if (!Buffer.isBuffer(seed) || seed.length !== 32) {
    throw new Error('EVM wallet derivation requires a 32-byte seed');
  }
  const chain = CHAINS[chainId];
  if (!chain || !isEvmChain(chainId)) {
    throw new Error(`Unsupported EVM chain for wallet derivation: ${chainId}`);
  }
  return deriveEvmWallet(seed, chainId, chain);
}

// =============================================================================
// ZCASH T-ADDRESS VERSION BYTES
// =============================================================================

const ZCASH_ADDRESS_VERSIONS = {
  mainnet: {
    p2pkh: Buffer.from([0x1c, 0xb8]), // t1 addresses
    p2sh: Buffer.from([0x1c, 0xbd]), // t3 addresses
  },
  testnet: {
    p2pkh: Buffer.from([0x1d, 0x25]), // tm addresses
    p2sh: Buffer.from([0x1c, 0xba]), // t2 addresses
  },
};

/**
 * Compress a 65-byte uncompressed secp256k1 public key to 33 bytes
 * @param {Buffer} uncompressedKey - 65-byte key (04 || x || y)
 * @returns {Buffer} - 33-byte compressed key (02/03 || x)
 */
function compressPublicKey(uncompressedKey) {
  if (uncompressedKey.length !== 65 || uncompressedKey[0] !== 0x04) {
    throw new Error('Invalid uncompressed public key format');
  }

  const x = uncompressedKey.subarray(1, 33);
  const y = uncompressedKey.subarray(33, 65);

  // If Y is even, prefix is 0x02; if odd, prefix is 0x03
  const prefix = (y[31] & 1) === 0 ? 0x02 : 0x03;

  return Buffer.concat([Buffer.from([prefix]), x]);
}

/**
 * Derive Zcash t-address wallet (secp256k1-based transparent address)
 *
 * Derivation flow:
 * 1. HKDF-SHA256 from VES Ed25519 seed → secp256k1 private key
 * 2. secp256k1 public key (compressed 33 bytes)
 * 3. SHA256 → RIPEMD160 = 20-byte pubkey hash
 * 4. Version bytes (2) + pubkey hash (20) + checksum (4) = 26 bytes
 * 5. Base58 encode = t-address
 *
 * @param {Buffer} seed - 32-byte seed from VES signing key
 * @param {string} chainId - Chain identifier (zcash or zcash_testnet)
 * @param {Object} _chain - Chain config
 * @returns {DerivedWallet}
 */
function deriveZcashWallet(seed, chainId, _chain) {
  // Step 1: Derive secp256k1 private key using HKDF
  const info = Buffer.from(`stateset:zcash:${chainId}`, 'utf8');
  const privKeyBytes = Buffer.from(crypto.hkdfSync('sha256', seed, Buffer.alloc(0), info, 32));

  // Step 2: Get secp256k1 public key (compressed, 33 bytes)
  const publicKeyUncompressed = secp256k1GetPublicKey(privKeyBytes);
  const publicKeyCompressed = compressPublicKey(publicKeyUncompressed);

  // Step 3: Hash160 (SHA256 then RIPEMD160)
  const sha256Hash = crypto.createHash('sha256').update(publicKeyCompressed).digest();
  const pubkeyHash = ripemd160(sha256Hash);

  // Step 4: Determine version bytes based on network
  const isTestnet = chainId === 'zcash_testnet';
  const versionBytes = isTestnet
    ? ZCASH_ADDRESS_VERSIONS.testnet.p2pkh
    : ZCASH_ADDRESS_VERSIONS.mainnet.p2pkh;

  // Step 5: Build address with checksum
  const payload = Buffer.concat([versionBytes, pubkeyHash]);
  const checksum = sha256Double(payload).subarray(0, 4);
  const addressBytes = Buffer.concat([payload, checksum]);

  // Step 6: Base58 encode
  const address = base58Encode(addressBytes);

  // Determine derivation path
  const derivationPath = isTestnet
    ? `${DERIVATION_PATHS.zcash_testnet}/0'/0/0`
    : `${DERIVATION_PATHS.zcash}/0'/0/0`;

  return {
    address,
    publicKey: publicKeyCompressed,
    privateKey: privKeyBytes,
    chainId,
    derivationPath,
  };
}

// =============================================================================
// BITCOIN ADDRESS VERSION BYTES
// =============================================================================

const BITCOIN_ADDRESS_VERSIONS = {
  mainnet: {
    p2pkh: Buffer.from([0x00]), // 1... addresses
    p2sh: Buffer.from([0x05]), // 3... addresses
  },
  testnet: {
    p2pkh: Buffer.from([0x6f]), // m... or n... addresses
    p2sh: Buffer.from([0xc4]), // 2... addresses
  },
};

/**
 * Derive Bitcoin native SegWit wallet (P2WPKH) with a matching legacy address.
 *
 * Derivation flow:
 * 1. HKDF-SHA256 from VES Ed25519 seed → secp256k1 private key
 * 2. secp256k1 public key (compressed 33 bytes)
 * 3. SHA256 → RIPEMD160 = 20-byte pubkey hash
 * 4. Encode the pubkey hash as both P2WPKH (preferred) and legacy P2PKH
 *
 * @param {Buffer} seed - 32-byte seed from VES signing key
 * @param {string} chainId - Chain identifier (bitcoin or bitcoin_testnet)
 * @param {Object} _chain - Chain config
 * @returns {DerivedWallet}
 */
function deriveBitcoinWallet(seed, chainId, _chain) {
  // Step 1: Derive secp256k1 private key using HKDF
  const info = Buffer.from(`stateset:bitcoin:${chainId}`, 'utf8');
  const privKeyBytes = Buffer.from(crypto.hkdfSync('sha256', seed, Buffer.alloc(0), info, 32));

  // Step 2: Get secp256k1 public key (compressed, 33 bytes)
  const publicKeyUncompressed = secp256k1GetPublicKey(privKeyBytes);
  const publicKeyCompressed = compressPublicKey(publicKeyUncompressed);

  // Step 3: Hash160 (SHA256 then RIPEMD160)
  const sha256Hash = crypto.createHash('sha256').update(publicKeyCompressed).digest();
  const pubkeyHash = ripemd160(sha256Hash);

  // Step 4: Determine network-specific encodings
  const isTestnet = chainId === 'bitcoin_testnet';
  const versionByte = isTestnet
    ? BITCOIN_ADDRESS_VERSIONS.testnet.p2pkh
    : BITCOIN_ADDRESS_VERSIONS.mainnet.p2pkh;
  const segwitHrp = isTestnet ? 'tb' : 'bc';

  // Step 5: Build legacy compatible address with checksum
  const payload = Buffer.concat([versionByte, pubkeyHash]);
  const checksum = sha256Double(payload).subarray(0, 4);
  const addressBytes = Buffer.concat([payload, checksum]);
  const legacyAddress = base58Encode(addressBytes);

  // Step 6: Build preferred native SegWit receive/change address
  const segwitAddress = encodeSegwitAddress(segwitHrp, 0, pubkeyHash);

  // Determine derivation path
  const derivationPath = isTestnet
    ? `${DERIVATION_PATHS.bitcoin_testnet}/0'/0/0`
    : `${DERIVATION_PATHS.bitcoin}/0'/0/0`;

  return {
    address: segwitAddress,
    legacyAddress,
    segwitAddress,
    publicKey: publicKeyCompressed,
    privateKey: privKeyBytes,
    chainId,
    derivationPath,
  };
}

// =============================================================================
// WALLET MANAGEMENT
// =============================================================================

/**
 * Get or create a wallet for an agent on a specific chain
 * @param {string} agentId - Agent identifier
 * @param {string} chainId - Chain identifier
 * @param {Object} [options]
 * @returns {Promise<DerivedWallet>}
 */
export async function getOrCreateWallet(agentId, chainId, options = {}) {
  const { configDir = '.stateset' } = options;
  const keyManager = getKeyManager(configDir);

  // Ensure agent has keys
  await keyManager.ensureKeys(agentId);

  // Derive wallet
  return deriveWallet(agentId, chainId, options);
}

/**
 * Get wallet address for an agent on a chain (without exposing private key)
 * @param {string} agentId - Agent identifier
 * @param {string} chainId - Chain identifier
 * @param {Object} [options]
 * @returns {Promise<string>}
 */
export async function getWalletAddress(agentId, chainId, options = {}) {
  if (isZcashChain(chainId)) {
    try {
      const { getPreferredZcashAddress } = await import('./zcash.js');
      const shieldedAddress = await getPreferredZcashAddress(agentId, chainId, {
        configDir: options.configDir || '.stateset',
        createIfMissing: options.createIfMissing !== false,
      });
      if (shieldedAddress) {
        return shieldedAddress;
      }
      if (options.requireShielded) {
        throw new Error(`Shielded Zcash address unavailable for ${agentId} on ${chainId}`);
      }
    } catch (error) {
      if (options.requireShielded) {
        throw error;
      }
    }
  }

  const wallet = await deriveWallet(agentId, chainId, options);
  return wallet.address;
}

/**
 * List all wallet addresses for an agent across supported chains
 * @param {string} agentId - Agent identifier
 * @param {Object} [options]
 * @returns {Promise<Object.<string, string>>}
 */
export async function listWalletAddresses(agentId, options = {}) {
  /** @type {Record<string, string>} */
  const addresses = {};

  for (const chainId of Object.keys(CHAINS)) {
    try {
      addresses[chainId] = await getWalletAddress(agentId, chainId, options);
    } catch (e) {
      // Skip chains that fail
      console.warn(`Could not derive wallet for ${chainId}: ${e.message}`);
    }
  }

  return addresses;
}

// =============================================================================
// BASE58 ENCODING (for Solana addresses)
// =============================================================================

const BASE58_ALPHABET = '123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz';

/**
 * Base58 encode a buffer
 * @param {Buffer} buffer
 * @returns {string}
 */
function base58Encode(buffer) {
  if (buffer.length === 0) return '';

  // Count leading zeros
  let zeros = 0;
  for (let i = 0; i < buffer.length && buffer[i] === 0; i++) {
    zeros++;
  }

  // Convert to base58
  const size = Math.ceil((buffer.length * 138) / 100) + 1;
  const b58 = new Uint8Array(size);

  let length = 0;
  for (let i = zeros; i < buffer.length; i++) {
    let carry = buffer[i];
    let j = 0;
    for (let k = size - 1; k >= 0 && (carry !== 0 || j < length); k--, j++) {
      carry += 256 * b58[k];
      b58[k] = carry % 58;
      carry = Math.floor(carry / 58);
    }
    length = j;
  }

  // Skip leading zeros in base58 result
  let i = size - length;
  while (i < size && b58[i] === 0) {
    i++;
  }

  // Build string
  let str = '';
  for (let z = 0; z < zeros; z++) {
    str += '1';
  }
  for (; i < size; i++) {
    str += BASE58_ALPHABET[b58[i]];
  }

  return str;
}

/**
 * Base58 decode a string
 * @param {string} str
 * @returns {Buffer}
 */
function base58Decode(str) {
  if (str.length === 0) return Buffer.alloc(0);

  // Count leading '1's (zeros)
  let zeros = 0;
  for (let i = 0; i < str.length && str[i] === '1'; i++) {
    zeros++;
  }

  // Decode
  const size = Math.ceil((str.length * 733) / 1000) + 1;
  const bytes = new Uint8Array(size);

  let length = 0;
  for (let i = zeros; i < str.length; i++) {
    const char = str[i];
    const value = BASE58_ALPHABET.indexOf(char);
    if (value === -1) {
      throw new Error(`Invalid base58 character: ${char}`);
    }

    let carry = value;
    let j = 0;
    for (let k = size - 1; k >= 0 && (carry !== 0 || j < length); k--, j++) {
      carry += 58 * bytes[k];
      bytes[k] = carry % 256;
      carry = Math.floor(carry / 256);
    }
    length = j;
  }

  // Skip leading zeros in result
  let i = size - length;
  while (i < size && bytes[i] === 0) {
    i++;
  }

  // Build buffer
  const result = Buffer.alloc(zeros + (size - i));
  for (let z = 0; z < zeros; z++) {
    result[z] = 0;
  }
  let j = zeros;
  while (i < size) {
    result[j++] = bytes[i++];
  }

  return result;
}

// =============================================================================
// EXPORTS
// =============================================================================

export { DERIVATION_PATHS, base58Encode, base58Decode, compressPublicKey };

export default {
  deriveWallet,
  getOrCreateWallet,
  getWalletAddress,
  listWalletAddresses,
  DERIVATION_PATHS,
  base58Encode,
  base58Decode,
};
