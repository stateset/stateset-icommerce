import crypto from 'node:crypto';
import { Contract, JsonRpcProvider, Signature, Wallet, verifyTypedData } from 'ethers';

/**
 * @typedef {Record<string, unknown>} JsonRecord
 * @typedef {{
 *   assetTransferMethod?: unknown,
 *   name?: unknown,
 *   version?: unknown,
 * }} ExactEvmExtraLike
 * @typedef {{
 *   scheme?: unknown,
 *   network?: unknown,
 *   amount?: unknown,
 *   asset?: unknown,
 *   payTo?: unknown,
 *   maxTimeoutSeconds?: unknown,
 *   extra?: unknown,
 * }} ExactEvmRequirementLike
 * @typedef {{
 *   from: string,
 *   to: string,
 *   value: string,
 *   validAfter: string,
 *   validBefore: string,
 *   nonce: string,
 * }} ExactEvmAuthorization
 * @typedef {{
 *   signature?: unknown,
 *   authorization?: unknown,
 * }} ExactEvmSchemePayloadLike
 * @typedef {{
 *   x402Version?: unknown,
 *   accepted?: unknown,
 *   payload?: unknown,
 *   extensions?: unknown,
 *   resource?: unknown,
 *   description?: unknown,
 * }} ExactEvmPaymentPayloadLike
 * @typedef {{
 *   chainId: number,
 *   rpcUrl: string,
 *   usdcAddress?: string,
 * }} ExactEvmChainInfo
 * @typedef {{
 *   address: string,
 *   privateKey: Buffer,
 * }} DerivedExactEvmWallet
 */

const EIP3009_ABI = [
  'function transferWithAuthorization(address from, address to, uint256 value, uint256 validAfter, uint256 validBefore, bytes32 nonce, uint8 v, bytes32 r, bytes32 s)',
  'function authorizationState(address authorizer, bytes32 nonce) view returns (bool)',
  'function balanceOf(address owner) view returns (uint256)',
];

const EIP3009_TYPES = {
  TransferWithAuthorization: [
    { name: 'from', type: 'address' },
    { name: 'to', type: 'address' },
    { name: 'value', type: 'uint256' },
    { name: 'validAfter', type: 'uint256' },
    { name: 'validBefore', type: 'uint256' },
    { name: 'nonce', type: 'bytes32' },
  ],
};

/** @type {Record<string, ExactEvmChainInfo>} */
const EXACT_EVM_CHAINS = {
  set_chain: {
    chainId: 84532001,
    rpcUrl: process.env.SET_CHAIN_RPC_URL || 'https://rpc.setchain.io',
  },
  set_chain_testnet: {
    chainId: 84532002,
    rpcUrl: process.env.SET_CHAIN_TESTNET_RPC_URL || 'https://rpc.testnet.setchain.io',
  },
  base: {
    chainId: 8453,
    rpcUrl: process.env.BASE_RPC_URL || 'https://mainnet.base.org',
    usdcAddress: '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
  },
  base_sepolia: {
    chainId: 84532,
    rpcUrl: process.env.BASE_SEPOLIA_RPC_URL || 'https://sepolia.base.org',
    usdcAddress: '0x036CbD53842c5426634e7929541eC2318f3dCF7e',
  },
  ethereum: {
    chainId: 1,
    rpcUrl: process.env.ETH_RPC_URL || 'https://eth.llamarpc.com',
    usdcAddress: '0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48',
  },
  ethereum_sepolia: {
    chainId: 11155111,
    rpcUrl: process.env.ETH_SEPOLIA_RPC_URL || 'https://ethereum-sepolia-rpc.publicnode.com',
    usdcAddress: '0x1c7D4B196Cb0C7B01d743Fbc6116a902379C7238',
  },
  arbitrum: {
    chainId: 42161,
    rpcUrl: process.env.ARB_RPC_URL || 'https://arb1.arbitrum.io/rpc',
  },
  arc: {
    chainId: 5042001,
    rpcUrl: process.env.ARC_RPC_URL || 'https://rpc.arc.network',
  },
  arc_testnet: {
    chainId: 5042002,
    rpcUrl: process.env.ARC_TESTNET_RPC_URL || 'https://rpc.testnet.arc.network',
  },
};

/** @type {Record<string, { name: string, version: string }>} */
const FALLBACK_DOMAIN_BY_CHAIN_AND_ASSET = {
  'eip155:8453:0x833589fcd6edb6e08f4c7c32d4f71b54bda02913': { name: 'USD Coin', version: '2' },
  'eip155:1:0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48': { name: 'USD Coin', version: '2' },
  'eip155:84532:0x036cbd53842c5426634e7929541ec2318f3dcf7e': { name: 'USDC', version: '2' },
  'eip155:11155111:0x1c7d4b196cb0c7b01d743fbc6116a902379c7238': { name: 'USDC', version: '2' },
};

/**
 * @param {unknown} value
 * @returns {JsonRecord | null}
 */
function asObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value)
    ? /** @type {JsonRecord} */ (value)
    : null;
}

/**
 * @param {unknown} value
 * @returns {unknown}
 */
function cloneJson(value) {
  if (value === undefined) return undefined;
  return JSON.parse(JSON.stringify(value));
}

/**
 * @param {unknown} value
 * @returns {boolean}
 */
function isHexAddress(value) {
  return /^0x[a-fA-F0-9]{40}$/.test(String(value || ''));
}

/**
 * @param {unknown} value
 * @returns {boolean}
 */
function isBytes32(value) {
  return /^0x[a-fA-F0-9]{64}$/.test(String(value || ''));
}

/**
 * @param {unknown} value
 * @returns {string}
 */
function normalizeHex(value) {
  return String(value || '').toLowerCase();
}

/**
 * @param {unknown} left
 * @param {unknown} right
 * @returns {boolean}
 */
function addressesEqual(left, right) {
  return normalizeHex(left) === normalizeHex(right);
}

/**
 * @param {string} chainKey
 * @returns {ExactEvmChainInfo | null}
 */
function getExactEvmChain(chainKey) {
  return EXACT_EVM_CHAINS[chainKey] || null;
}

/**
 * @param {Buffer} seed
 * @param {string} chainKey
 * @returns {DerivedExactEvmWallet}
 */
function deriveLocalEvmWalletFromSeed(seed, chainKey) {
  const info = Buffer.from(`stateset:evm:${chainKey}`, 'utf8');
  const privateKey = Buffer.from(crypto.hkdfSync('sha256', seed, Buffer.alloc(0), info, 32));
  const address = new Wallet(`0x${privateKey.toString('hex')}`).address;
  return { address, privateKey };
}

/**
 * @param {unknown} network
 * @returns {string | null}
 */
export function caip2ToChainId(network) {
  const value = String(network || '')
    .trim()
    .toLowerCase();
  if (!value.startsWith('eip155:')) return null;
  const reference = Number(value.slice('eip155:'.length));
  if (!Number.isFinite(reference)) return null;

  for (const [chainId, chain] of Object.entries(EXACT_EVM_CHAINS)) {
    if (chain?.chainId === reference) {
      return chainId;
    }
  }

  return null;
}

/**
 * @param {string} chainId
 * @returns {string | null}
 */
export function chainIdToCaip2(chainId) {
  const chain = getExactEvmChain(chainId);
  if (!chain?.chainId) return null;
  return `eip155:${chain.chainId}`;
}

/**
 * @param {ExactEvmRequirementLike | JsonRecord | null | undefined} requirement
 * @returns {{ name: string, version: string }}
 */
function resolveDomainInfo(requirement) {
  const extra = /** @type {ExactEvmExtraLike} */ (asObject(requirement?.extra) || {});
  const name = typeof extra.name === 'string' && extra.name.trim() ? extra.name.trim() : null;
  const version =
    typeof extra.version === 'string' && extra.version.trim() ? extra.version.trim() : null;

  if (name && version) {
    return { name, version };
  }

  const fallback =
    FALLBACK_DOMAIN_BY_CHAIN_AND_ASSET[
      `${String(requirement?.network || '').toLowerCase()}:${normalizeHex(requirement?.asset)}`
    ];
  if (fallback) {
    return fallback;
  }

  throw new Error('Exact EVM payment requirements missing extra.name/version');
}

/**
 * @param {ExactEvmRequirementLike | JsonRecord | null | undefined} requirement
 * @returns {{ name: string, version: string, chainId: number, verifyingContract: string }}
 */
function buildDomain(requirement) {
  const chainKey = caip2ToChainId(requirement?.network);
  const chain = chainKey ? getExactEvmChain(chainKey) : null;
  if (!chain?.chainId) {
    throw new Error(`Unsupported exact EVM network: ${requirement?.network}`);
  }
  if (!isHexAddress(requirement?.asset)) {
    throw new Error('Exact EVM requirement asset must be an ERC-20 contract address');
  }
  const { name, version } = resolveDomainInfo(requirement);
  return {
    name,
    version,
    chainId: chain.chainId,
    verifyingContract: String(requirement?.asset),
  };
}

/**
 * @param {ExactEvmRequirementLike | JsonRecord | null | undefined} requirement
 */
function buildAcceptedRequirement(requirement) {
  const source = asObject(requirement) || {};
  const extraRecord = asObject(requirement?.extra) || {};
  const extra = /** @type {ExactEvmExtraLike} */ (extraRecord);
  const domain = resolveDomainInfo(requirement);
  return {
    scheme: 'exact',
    network: String(source.network),
    amount: String(source.amount),
    asset: String(source.asset),
    payTo: String(source.payTo),
    maxTimeoutSeconds: Number(source.maxTimeoutSeconds ?? 60),
    extra: {
      assetTransferMethod: String(extra.assetTransferMethod || 'eip3009'),
      name: String(extra.name || domain.name),
      version: String(extra.version || domain.version),
      .../** @type {JsonRecord} */ (cloneJson(extraRecord) || {}),
    },
  };
}

/**
 * @param {ExactEvmPaymentPayloadLike | JsonRecord | null | undefined} paymentRequired
 * @param {string} resourceUrl
 * @param {string | undefined} method
 */
function buildResourceInfo(paymentRequired, resourceUrl, method) {
  const resource = asObject(paymentRequired?.resource);
  if (resource?.url) {
    return {
      url: String(resource.url),
      ...(resource.method ? { method: String(resource.method) } : {}),
      ...(resource.description ? { description: String(resource.description) } : {}),
      ...(resource.mimeType ? { mimeType: String(resource.mimeType) } : {}),
    };
  }

  return {
    url: resourceUrl,
    ...(method ? { method: String(method) } : {}),
    ...(paymentRequired?.description ? { description: String(paymentRequired.description) } : {}),
  };
}

/**
 * @param {unknown} value
 * @param {string} name
 * @returns {string}
 */
function normalizeUintString(value, name) {
  const normalized = String(value ?? '').trim();
  if (!/^\d+$/.test(normalized)) {
    throw new Error(`${name} must be an unsigned integer string`);
  }
  return normalized;
}

/**
 * @param {unknown} authorization
 * @returns {ExactEvmAuthorization}
 */
function normalizeAuthorization(authorization) {
  const payload = asObject(authorization);
  if (!payload) throw new Error('Exact EVM authorization payload is required');
  return {
    from: String(payload.from),
    to: String(payload.to),
    value: normalizeUintString(payload.value, 'authorization.value'),
    validAfter: normalizeUintString(payload.validAfter, 'authorization.validAfter'),
    validBefore: normalizeUintString(payload.validBefore, 'authorization.validBefore'),
    nonce: String(payload.nonce),
  };
}

/**
 * @param {ExactEvmRequirementLike | JsonRecord | null | undefined} requirement
 * @returns {number}
 */
function resolveMaxTimeoutSeconds(requirement) {
  const maxTimeoutSeconds = Number(requirement?.maxTimeoutSeconds ?? 60);
  if (!Number.isFinite(maxTimeoutSeconds) || maxTimeoutSeconds <= 0) {
    throw new Error('Exact EVM maxTimeoutSeconds must be a positive number');
  }
  return Math.floor(maxTimeoutSeconds);
}

/**
 * @param {ExactEvmRequirementLike | JsonRecord | null | undefined} requirement
 * @returns {JsonRpcProvider}
 */
function getRpcProvider(requirement) {
  const chainKey = caip2ToChainId(requirement?.network);
  const chain = chainKey ? getExactEvmChain(chainKey) : null;
  if (!chain?.rpcUrl || !chain?.chainId) {
    throw new Error(`RPC URL is not configured for network ${requirement?.network}`);
  }
  return new JsonRpcProvider(chain.rpcUrl, chain.chainId);
}

/**
 * @param {string} signature
 * @returns {{ v: number, r: string, s: string }}
 */
function splitSignature(signature) {
  const parsed = Signature.from(signature);
  return {
    v: parsed.v,
    r: parsed.r,
    s: parsed.s,
  };
}

/**
 * @param {ExactEvmRequirementLike | JsonRecord | null | undefined} requirement
 * @returns {boolean}
 */
export function isExactEvmRequirement(requirement) {
  const extra = /** @type {ExactEvmExtraLike} */ (asObject(requirement?.extra) || {});
  const assetTransferMethod =
    typeof extra.assetTransferMethod === 'string' ? extra.assetTransferMethod.toLowerCase() : null;

  return (
    String(requirement?.scheme || '').toLowerCase() === 'exact' &&
    String(requirement?.network || '')
      .toLowerCase()
      .startsWith('eip155:') &&
    isHexAddress(requirement?.asset) &&
    isHexAddress(requirement?.payTo) &&
    (assetTransferMethod === null || assetTransferMethod === 'eip3009')
  );
}

/**
 * @param {{
 *   signingKey: { privateKey?: Buffer | Uint8Array | string } | null | undefined,
 *   requirement: ExactEvmRequirementLike | JsonRecord | null | undefined,
 *   payerAddress?: string | null,
 * }} input
 * @returns {DerivedExactEvmWallet}
 */
export function deriveExactEvmWallet({ signingKey, requirement, payerAddress = null }) {
  const chainKey = caip2ToChainId(requirement?.network);
  if (!chainKey) {
    throw new Error(`Unsupported exact EVM network: ${requirement?.network}`);
  }
  if (!signingKey?.privateKey) {
    throw new Error('signingKey.privateKey is required for exact EVM payments');
  }

  const wallet = deriveLocalEvmWalletFromSeed(Buffer.from(signingKey.privateKey), chainKey);
  if (payerAddress && !addressesEqual(wallet.address, payerAddress)) {
    throw new Error(
      `Configured payerAddress ${payerAddress} does not match derived exact EVM wallet ${wallet.address}`,
    );
  }

  return wallet;
}

/**
 * @param {{
 *   requirement: ExactEvmRequirementLike | JsonRecord | null | undefined,
 *   paymentRequired?: ExactEvmPaymentPayloadLike | JsonRecord | null,
 *   signingKey: { privateKey?: Buffer | Uint8Array | string } | null | undefined,
 *   payerAddress?: string | null,
 *   resourceUrl: string,
 *   method?: string,
 * }} input
 */
export async function createExactEvmPaymentPayload({
  requirement,
  paymentRequired = null,
  signingKey,
  payerAddress = null,
  resourceUrl,
  method,
}) {
  if (!isExactEvmRequirement(requirement)) {
    throw new Error('Payment requirement is not supported by exact EVM handler');
  }

  const wallet = deriveExactEvmWallet({ signingKey, requirement, payerAddress });
  const accepted = buildAcceptedRequirement(requirement);
  const resource = buildResourceInfo(paymentRequired, resourceUrl, method);
  const now = Math.floor(Date.now() / 1000);
  const validAfter = String(now);
  const validBefore = String(now + accepted.maxTimeoutSeconds);
  const authorization = {
    from: wallet.address,
    to: accepted.payTo,
    value: accepted.amount,
    validAfter,
    validBefore,
    nonce: `0x${crypto.randomBytes(32).toString('hex')}`,
  };
  const signature = await new Wallet(`0x${wallet.privateKey.toString('hex')}`).signTypedData(
    buildDomain(accepted),
    EIP3009_TYPES,
    authorization,
  );

  return {
    x402Version: 2,
    resource,
    accepted,
    payload: {
      signature,
      authorization,
    },
    extensions: cloneJson(asObject(paymentRequired?.extensions) || {}),
  };
}

/**
 * @param {{
 *   paymentPayload: ExactEvmPaymentPayloadLike | JsonRecord | null | undefined,
 *   paymentRequirements?: ExactEvmRequirementLike | JsonRecord | null,
 *   checkOnchain?: boolean,
 * }} input
 */
export async function verifyExactEvmPaymentPayload({
  paymentPayload,
  paymentRequirements,
  checkOnchain = true,
}) {
  try {
    const payload = asObject(paymentPayload);
    const accepted = asObject(payload?.accepted);
    const schemePayload = /** @type {ExactEvmSchemePayloadLike | null} */ (
      asObject(payload?.payload)
    );
    const signature = typeof schemePayload?.signature === 'string' ? schemePayload.signature : null;
    const authorization = normalizeAuthorization(schemePayload?.authorization);
    const requirements = paymentRequirements || accepted;

    if (!payload || payload.x402Version !== 2 || !accepted || !schemePayload || !signature) {
      return { isValid: false, invalidReason: 'invalid_payload' };
    }

    if (!isExactEvmRequirement(requirements)) {
      return { isValid: false, invalidReason: 'unsupported_scheme' };
    }

    const acceptedRequirement = /** @type {ExactEvmRequirementLike & JsonRecord} */ (accepted);
    const requirement = /** @type {ExactEvmRequirementLike & JsonRecord} */ (requirements);

    if (String(acceptedRequirement.scheme).toLowerCase() !== 'exact') {
      return { isValid: false, invalidReason: 'invalid_scheme' };
    }
    if (String(acceptedRequirement.network) !== String(requirement.network)) {
      return { isValid: false, invalidReason: 'invalid_network' };
    }
    if (!addressesEqual(acceptedRequirement.asset, requirement.asset)) {
      return { isValid: false, invalidReason: 'invalid_asset' };
    }
    if (!addressesEqual(acceptedRequirement.payTo, requirement.payTo)) {
      return { isValid: false, invalidReason: 'invalid_exact_evm_payload_recipient_mismatch' };
    }
    if (String(acceptedRequirement.amount) !== String(requirement.amount)) {
      return {
        isValid: false,
        invalidReason: 'invalid_exact_evm_payload_authorization_value_mismatch',
      };
    }
    const requiredTimeout = resolveMaxTimeoutSeconds(requirement);
    if (Number(acceptedRequirement.maxTimeoutSeconds) !== requiredTimeout) {
      return {
        isValid: false,
        invalidReason: 'invalid_payment_requirements',
      };
    }
    if (!addressesEqual(authorization.to, requirement.payTo)) {
      return { isValid: false, invalidReason: 'invalid_exact_evm_payload_recipient_mismatch' };
    }
    if (String(authorization.value) !== String(requirement.amount)) {
      return {
        isValid: false,
        invalidReason: 'invalid_exact_evm_payload_authorization_value_mismatch',
      };
    }
    if (!isHexAddress(authorization.from)) {
      return { isValid: false, invalidReason: 'invalid_payload' };
    }
    if (!isBytes32(authorization.nonce)) {
      return { isValid: false, invalidReason: 'invalid_payload' };
    }

    const validAfter = BigInt(authorization.validAfter);
    const validBefore = BigInt(authorization.validBefore);
    if (validBefore <= validAfter) {
      return {
        isValid: false,
        invalidReason: 'invalid_exact_evm_payload_authorization_valid_before',
        payer: authorization.from,
      };
    }
    if (validBefore - validAfter > BigInt(requiredTimeout)) {
      return {
        isValid: false,
        invalidReason: 'invalid_payment_requirements',
        payer: authorization.from,
      };
    }

    const now = BigInt(Math.floor(Date.now() / 1000));
    if (validAfter > now) {
      return {
        isValid: false,
        invalidReason: 'invalid_exact_evm_payload_authorization_valid_after',
        payer: authorization.from,
      };
    }
    if (validBefore < now) {
      return {
        isValid: false,
        invalidReason: 'invalid_exact_evm_payload_authorization_valid_before',
        payer: authorization.from,
      };
    }

    const recovered = verifyTypedData(
      buildDomain(accepted),
      EIP3009_TYPES,
      authorization,
      signature,
    );
    if (!addressesEqual(recovered, authorization.from)) {
      return {
        isValid: false,
        invalidReason: 'invalid_exact_evm_payload_signature',
        payer: authorization.from,
      };
    }

    if (!checkOnchain) {
      return { isValid: true, payer: authorization.from };
    }

    const provider = getRpcProvider(requirement);
    const token = new Contract(String(requirement.asset), EIP3009_ABI, provider);
    const [balance, authorizationUsed] = await Promise.all([
      token.balanceOf(authorization.from),
      token.authorizationState(authorization.from, authorization.nonce).catch(() => false),
    ]);

    if (BigInt(balance.toString()) < BigInt(authorization.value)) {
      return {
        isValid: false,
        invalidReason: 'insufficient_funds',
        payer: authorization.from,
      };
    }

    if (authorizationUsed) {
      return {
        isValid: false,
        invalidReason: 'invalid_exact_evm_payload_duplicate_nonce',
        payer: authorization.from,
      };
    }

    try {
      const { v, r, s } = splitSignature(signature);
      await token.transferWithAuthorization.staticCall(
        authorization.from,
        authorization.to,
        authorization.value,
        authorization.validAfter,
        authorization.validBefore,
        authorization.nonce,
        v,
        r,
        s,
      );
    } catch {
      return {
        isValid: false,
        invalidReason: 'invalid_transaction_state',
        payer: authorization.from,
      };
    }

    return { isValid: true, payer: authorization.from };
  } catch {
    return { isValid: false, invalidReason: 'unexpected_verify_error' };
  }
}

/**
 * @param {{
 *   paymentPayload: ExactEvmPaymentPayloadLike | JsonRecord,
 *   paymentRequirements?: ExactEvmRequirementLike | JsonRecord | null,
 *   facilitatorPrivateKey?: string | null,
 * }} input
 */
export async function settleExactEvmPaymentPayload({
  paymentPayload,
  paymentRequirements,
  facilitatorPrivateKey,
}) {
  const payload = /** @type {ExactEvmPaymentPayloadLike & JsonRecord} */ (paymentPayload);
  const payloadBody = /** @type {ExactEvmSchemePayloadLike | null} */ (asObject(payload.payload));
  const requirements = /** @type {ExactEvmRequirementLike | JsonRecord | null} */ (
    paymentRequirements || asObject(payload.accepted)
  );
  const authorization = normalizeAuthorization(payloadBody?.authorization);
  const payer = authorization.from;

  try {
    const verification = await verifyExactEvmPaymentPayload({
      paymentPayload,
      paymentRequirements: requirements,
      checkOnchain: true,
    });
    if (!verification.isValid) {
      return {
        success: false,
        errorReason: verification.invalidReason,
        payer,
        transaction: '',
        network: String(requirements?.network || ''),
      };
    }

    if (!isExactEvmRequirement(requirements)) {
      return {
        success: false,
        errorReason: 'unsupported_scheme',
        payer,
        transaction: '',
        network: String(asObject(requirements)?.network || ''),
      };
    }
    if (!payloadBody || typeof payloadBody.signature !== 'string') {
      return {
        success: false,
        errorReason: 'invalid_payload',
        payer,
        transaction: '',
        network: String(asObject(requirements)?.network || ''),
      };
    }

    const requirement = /** @type {ExactEvmRequirementLike & JsonRecord} */ (requirements);
    const provider = getRpcProvider(requirement);
    const normalizedKey = String(facilitatorPrivateKey || '').trim();
    if (!/^0x[a-fA-F0-9]{64}$/.test(normalizedKey)) {
      throw new Error('facilitatorPrivateKey must be a 32-byte hex private key');
    }

    const facilitator = new Wallet(normalizedKey, provider);
    const token = new Contract(String(requirement.asset), EIP3009_ABI, facilitator);
    const { v, r, s } = splitSignature(payloadBody.signature);
    const tx = await token.transferWithAuthorization(
      authorization.from,
      authorization.to,
      authorization.value,
      authorization.validAfter,
      authorization.validBefore,
      authorization.nonce,
      v,
      r,
      s,
    );
    const receipt = await tx.wait();

    return {
      success: true,
      payer,
      transaction: tx.hash,
      network: String(requirement.network),
      amount: String(requirement.amount),
      extensions: {
        receipt: {
          blockNumber: receipt?.blockNumber ?? null,
          gasUsed: receipt?.gasUsed?.toString?.() ?? null,
          facilitator: facilitator.address,
        },
      },
    };
  } catch (error) {
    return {
      success: false,
      errorReason: error instanceof Error ? error.message : String(error),
      payer,
      transaction: '',
      network: String(requirements?.network || ''),
    };
  }
}

export function getExactEvmSupportedKinds() {
  const supportedChains =
    /** @type {Array<{ x402Version: number, scheme: string, network: string, extra: Record<string, string> }>} */ (
      ['base_sepolia', 'base', 'ethereum_sepolia', 'ethereum']
        .map((chainId) => {
          const chain = getExactEvmChain(chainId);
          if (!chain?.chainId || !chain.usdcAddress) return null;
          return {
            x402Version: 2,
            scheme: 'exact',
            network: `eip155:${chain.chainId}`,
            extra: {
              assetTransferMethod: 'eip3009',
              name: 'USD Coin',
              version: '2',
            },
          };
        })
        .filter((chain) => chain !== null)
    );

  return supportedChains;
}

export default {
  caip2ToChainId,
  chainIdToCaip2,
  isExactEvmRequirement,
  deriveExactEvmWallet,
  createExactEvmPaymentPayload,
  verifyExactEvmPaymentPayload,
  settleExactEvmPaymentPayload,
  getExactEvmSupportedKinds,
};
