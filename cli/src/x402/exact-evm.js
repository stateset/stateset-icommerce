import crypto from 'node:crypto';
import { Contract, JsonRpcProvider, Signature, Wallet, verifyTypedData } from 'ethers';
import { CHAINS, getChain } from '../chains/config.js';
import { deriveEvmWalletFromSeed } from '../chains/wallet.js';

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

const FALLBACK_DOMAIN_BY_CHAIN_AND_ASSET = {
  'eip155:8453:0x833589fcd6edb6e08f4c7c32d4f71b54bda02913': { name: 'USD Coin', version: '2' },
  'eip155:1:0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48': { name: 'USD Coin', version: '2' },
  'eip155:84532:0x036cbd53842c5426634e7929541ec2318f3dcf7e': { name: 'USDC', version: '2' },
  'eip155:11155111:0x1c7d4b196cb0c7b01d743fbc6116a902379c7238': { name: 'USDC', version: '2' },
};

function asObject(value) {
  return value && typeof value === 'object' && !Array.isArray(value) ? value : null;
}

function cloneJson(value) {
  if (value === undefined) return undefined;
  return JSON.parse(JSON.stringify(value));
}

function isHexAddress(value) {
  return /^0x[a-fA-F0-9]{40}$/.test(String(value || ''));
}

function isBytes32(value) {
  return /^0x[a-fA-F0-9]{64}$/.test(String(value || ''));
}

function normalizeHex(value) {
  return String(value || '').toLowerCase();
}

function addressesEqual(left, right) {
  return normalizeHex(left) === normalizeHex(right);
}

export function caip2ToChainId(network) {
  const value = String(network || '').trim().toLowerCase();
  if (!value.startsWith('eip155:')) return null;
  const reference = Number(value.slice('eip155:'.length));
  if (!Number.isFinite(reference)) return null;

  for (const [chainId, chain] of Object.entries(CHAINS)) {
    if (chain?.chainId === reference) {
      return chainId;
    }
  }

  return null;
}

export function chainIdToCaip2(chainId) {
  const chain = getChain(chainId);
  if (!chain?.chainId) return null;
  return `eip155:${chain.chainId}`;
}

function resolveDomainInfo(requirement) {
  const extra = asObject(requirement?.extra) || {};
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

function buildDomain(requirement) {
  const chainKey = caip2ToChainId(requirement?.network);
  const chain = chainKey ? getChain(chainKey) : null;
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
    verifyingContract: requirement.asset,
  };
}

function buildAcceptedRequirement(requirement) {
  const extra = asObject(requirement?.extra) || {};
  const domain = resolveDomainInfo(requirement);
  return {
    scheme: 'exact',
    network: String(requirement.network),
    amount: String(requirement.amount),
    asset: String(requirement.asset),
    payTo: String(requirement.payTo),
    maxTimeoutSeconds: Number(requirement.maxTimeoutSeconds ?? 60),
    extra: {
      assetTransferMethod: String(extra.assetTransferMethod || 'eip3009'),
      name: String(extra.name || domain.name),
      version: String(extra.version || domain.version),
      ...cloneJson(extra),
    },
  };
}

function buildResourceInfo(paymentRequired, resourceUrl) {
  const resource = asObject(paymentRequired?.resource);
  if (resource?.url) {
    return {
      url: String(resource.url),
      ...(resource.description ? { description: String(resource.description) } : {}),
      ...(resource.mimeType ? { mimeType: String(resource.mimeType) } : {}),
    };
  }

  return {
    url: resourceUrl,
    ...(paymentRequired?.description ? { description: String(paymentRequired.description) } : {}),
  };
}

function normalizeUintString(value, name) {
  const normalized = String(value ?? '').trim();
  if (!/^\d+$/.test(normalized)) {
    throw new Error(`${name} must be an unsigned integer string`);
  }
  return normalized;
}

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

function resolveMaxTimeoutSeconds(requirement) {
  const maxTimeoutSeconds = Number(requirement?.maxTimeoutSeconds ?? 60);
  if (!Number.isFinite(maxTimeoutSeconds) || maxTimeoutSeconds <= 0) {
    throw new Error('Exact EVM maxTimeoutSeconds must be a positive number');
  }
  return Math.floor(maxTimeoutSeconds);
}

function getRpcProvider(requirement) {
  const chainKey = caip2ToChainId(requirement?.network);
  const chain = chainKey ? getChain(chainKey) : null;
  if (!chain?.rpcUrl || !chain?.chainId) {
    throw new Error(`RPC URL is not configured for network ${requirement?.network}`);
  }
  return new JsonRpcProvider(chain.rpcUrl, chain.chainId);
}

function splitSignature(signature) {
  const parsed = Signature.from(signature);
  return {
    v: parsed.v,
    r: parsed.r,
    s: parsed.s,
  };
}

export function isExactEvmRequirement(requirement) {
  const extra = asObject(requirement?.extra) || {};
  const assetTransferMethod =
    typeof extra.assetTransferMethod === 'string' ? extra.assetTransferMethod.toLowerCase() : null;

  return (
    String(requirement?.scheme || '').toLowerCase() === 'exact' &&
    String(requirement?.network || '').toLowerCase().startsWith('eip155:') &&
    isHexAddress(requirement?.asset) &&
    isHexAddress(requirement?.payTo) &&
    (assetTransferMethod === null || assetTransferMethod === 'eip3009')
  );
}

export function deriveExactEvmWallet({ signingKey, requirement, payerAddress = null }) {
  const chainKey = caip2ToChainId(requirement?.network);
  if (!chainKey) {
    throw new Error(`Unsupported exact EVM network: ${requirement?.network}`);
  }
  if (!signingKey?.privateKey) {
    throw new Error('signingKey.privateKey is required for exact EVM payments');
  }

  const wallet = deriveEvmWalletFromSeed(Buffer.from(signingKey.privateKey), chainKey);
  if (payerAddress && !addressesEqual(wallet.address, payerAddress)) {
    throw new Error(
      `Configured payerAddress ${payerAddress} does not match derived exact EVM wallet ${wallet.address}`,
    );
  }

  return wallet;
}

export async function createExactEvmPaymentPayload({
  requirement,
  paymentRequired = null,
  signingKey,
  payerAddress = null,
  resourceUrl,
}) {
  if (!isExactEvmRequirement(requirement)) {
    throw new Error('Payment requirement is not supported by exact EVM handler');
  }

  const wallet = deriveExactEvmWallet({ signingKey, requirement, payerAddress });
  const accepted = buildAcceptedRequirement(requirement);
  const resource = buildResourceInfo(paymentRequired, resourceUrl);
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

export async function verifyExactEvmPaymentPayload({
  paymentPayload,
  paymentRequirements,
  checkOnchain = true,
}) {
  try {
    const payload = asObject(paymentPayload);
    const accepted = asObject(payload?.accepted);
    const schemePayload = asObject(payload?.payload);
    const signature = typeof schemePayload?.signature === 'string' ? schemePayload.signature : null;
    const authorization = normalizeAuthorization(schemePayload?.authorization);
    const requirements = paymentRequirements || accepted;

    if (!payload || payload.x402Version !== 2 || !accepted || !schemePayload || !signature) {
      return { isValid: false, invalidReason: 'invalid_payload' };
    }

    if (!isExactEvmRequirement(requirements)) {
      return { isValid: false, invalidReason: 'unsupported_scheme' };
    }

    if (String(accepted.scheme).toLowerCase() !== 'exact') {
      return { isValid: false, invalidReason: 'invalid_scheme' };
    }
    if (String(accepted.network) !== String(requirements.network)) {
      return { isValid: false, invalidReason: 'invalid_network' };
    }
    if (!addressesEqual(accepted.asset, requirements.asset)) {
      return { isValid: false, invalidReason: 'invalid_asset' };
    }
    if (!addressesEqual(accepted.payTo, requirements.payTo)) {
      return { isValid: false, invalidReason: 'invalid_exact_evm_payload_recipient_mismatch' };
    }
    if (String(accepted.amount) !== String(requirements.amount)) {
      return { isValid: false, invalidReason: 'invalid_exact_evm_payload_authorization_value_mismatch' };
    }
    const requiredTimeout = resolveMaxTimeoutSeconds(requirements);
    if (Number(accepted.maxTimeoutSeconds) !== requiredTimeout) {
      return {
        isValid: false,
        invalidReason: 'invalid_payment_requirements',
      };
    }
    if (!addressesEqual(authorization.to, requirements.payTo)) {
      return { isValid: false, invalidReason: 'invalid_exact_evm_payload_recipient_mismatch' };
    }
    if (String(authorization.value) !== String(requirements.amount)) {
      return { isValid: false, invalidReason: 'invalid_exact_evm_payload_authorization_value_mismatch' };
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

    const provider = getRpcProvider(requirements);
    const token = new Contract(requirements.asset, EIP3009_ABI, provider);
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
    } catch (_error) {
      return {
        isValid: false,
        invalidReason: 'invalid_transaction_state',
        payer: authorization.from,
      };
    }

    return { isValid: true, payer: authorization.from };
  } catch (_error) {
    return { isValid: false, invalidReason: 'unexpected_verify_error' };
  }
}

export async function settleExactEvmPaymentPayload({
  paymentPayload,
  paymentRequirements,
  facilitatorPrivateKey,
}) {
  const requirements = paymentRequirements || paymentPayload?.accepted;
  const authorization = normalizeAuthorization(paymentPayload?.payload?.authorization);
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

    const provider = getRpcProvider(requirements);
    const normalizedKey = String(facilitatorPrivateKey || '').trim();
    if (!/^0x[a-fA-F0-9]{64}$/.test(normalizedKey)) {
      throw new Error('facilitatorPrivateKey must be a 32-byte hex private key');
    }

    const facilitator = new Wallet(normalizedKey, provider);
    const token = new Contract(requirements.asset, EIP3009_ABI, facilitator);
    const { v, r, s } = splitSignature(paymentPayload.payload.signature);
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
      network: String(requirements.network),
      amount: String(requirements.amount),
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
  const supportedChains = ['base_sepolia', 'base', 'ethereum_sepolia', 'ethereum']
    .map((chainId) => {
      const chain = getChain(chainId);
      const usdc = chain?.tokens?.USDC;
      if (!chain?.chainId || !usdc?.address || usdc.address === 'native') return null;
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
    .filter(Boolean);

  return /** @type {Array<{ x402Version: number, scheme: string, network: string, extra: Record<string, string> }>} */ (
    supportedChains
  );
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
