import crypto from 'node:crypto';
import { getChain } from './config.js';
import { decodeSegwitAddress, encodeSegwitAddress } from './bitcoin-address.js';
import { base58Decode, base58Encode } from './wallet.js';
import { sha256Double } from './crypto-utils.js';

const SIGHASH_ALL = 0x01;
const DEFAULT_FEE_RATE_SAT_VBYTE = 5;
const DUST_THRESHOLD_SATS = 546n;
const SECP256K1_ORDER = BigInt(
  '0xFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFFEBAAEDCE6AF48A03BBFD25E8CD0364141',
);
const SECP256K1_HALF_ORDER = SECP256K1_ORDER / 2n;
const BITCOIN_P2PKH_VERSIONS = new Set([0x00, 0x6f]);
const BITCOIN_P2SH_VERSIONS = new Set([0x05, 0xc4]);
const BITCOIN_WITNESS_HRPS = new Set(['bc', 'tb']);

function getEsploraBaseUrl(chainId) {
  const chain = getChain(chainId);
  if (!chain?.rpcUrl) {
    throw new Error(`RPC URL is not configured for chain ${chainId}`);
  }
  return chain.rpcUrl.replace(/\/+$/, '');
}

async function fetchJson(url, options = {}) {
  const response = await fetch(url, options);
  if (!response.ok) {
    throw new Error(`Bitcoin API request failed (${response.status}) for ${url}`);
  }
  return response.json();
}

async function fetchText(url, options = {}) {
  const response = await fetch(url, options);
  const body = await response.text();
  if (!response.ok) {
    throw new Error(`Bitcoin API request failed (${response.status}): ${body || url}`);
  }
  return body;
}

function reverseBuffer(buffer) {
  return Buffer.from(buffer).reverse();
}

function encodeUInt32LE(value) {
  const buf = Buffer.alloc(4);
  buf.writeUInt32LE(value >>> 0, 0);
  return buf;
}

function encodeUInt64LE(value) {
  const big = typeof value === 'bigint' ? value : BigInt(value);
  const buf = Buffer.alloc(8);
  buf.writeUInt32LE(Number(big & 0xffffffffn), 0);
  buf.writeUInt32LE(Number((big >> 32n) & 0xffffffffn), 4);
  return buf;
}

function encodeVarInt(value) {
  const big = typeof value === 'bigint' ? value : BigInt(value);
  if (big < 0xfdn) {
    return Buffer.from([Number(big)]);
  }
  if (big <= 0xffffn) {
    const buf = Buffer.alloc(3);
    buf[0] = 0xfd;
    buf.writeUInt16LE(Number(big), 1);
    return buf;
  }
  if (big <= 0xffffffffn) {
    return Buffer.concat([Buffer.from([0xfe]), encodeUInt32LE(Number(big))]);
  }
  return Buffer.concat([Buffer.from([0xff]), encodeUInt64LE(big)]);
}

function pushData(buffer) {
  if (buffer.length < 0x4c) {
    return Buffer.concat([Buffer.from([buffer.length]), buffer]);
  }
  if (buffer.length <= 0xff) {
    return Buffer.concat([Buffer.from([0x4c, buffer.length]), buffer]);
  }
  if (buffer.length <= 0xffff) {
    const len = Buffer.alloc(2);
    len.writeUInt16LE(buffer.length, 0);
    return Buffer.concat([Buffer.from([0x4d]), len, buffer]);
  }
  const len = Buffer.alloc(4);
  len.writeUInt32LE(buffer.length, 0);
  return Buffer.concat([Buffer.from([0x4e]), len, buffer]);
}

function readBase58Check(address) {
  const decoded = base58Decode(address);
  if (decoded.length < 5) {
    throw new Error(`Invalid Base58Check address: ${address}`);
  }
  const payload = decoded.subarray(0, -4);
  const checksum = decoded.subarray(-4);
  const expected = sha256Double(payload).subarray(0, 4);
  if (!checksum.equals(expected)) {
    throw new Error(`Invalid Base58Check checksum for ${address}`);
  }
  return payload;
}

function encodeBase58Check(versionByte, payload) {
  const body = Buffer.concat([Buffer.from([versionByte]), payload]);
  const checksum = sha256Double(body).subarray(0, 4);
  return base58Encode(Buffer.concat([body, checksum]));
}

function p2pkhScriptFromHash(hash) {
  return Buffer.concat([Buffer.from([0x76, 0xa9, 0x14]), hash, Buffer.from([0x88, 0xac])]);
}

function p2wpkhScriptFromHash(hash) {
  return Buffer.concat([Buffer.from([0x00, 0x14]), hash]);
}

function describeInputAddress(address) {
  if (address.startsWith('bc1') || address.startsWith('tb1')) {
    const decoded = decodeSegwitAddress(address);
    if (
      !BITCOIN_WITNESS_HRPS.has(decoded.hrp) ||
      decoded.version !== 0 ||
      decoded.program.length !== 20
    ) {
      throw new Error(`Only P2WPKH sender addresses are supported for Bitcoin signing: ${address}`);
    }

    return {
      address,
      inputType: 'p2wpkh',
      pubKeyHash: decoded.program,
      prevScriptPubKey: p2wpkhScriptFromHash(decoded.program),
      scriptCode: p2pkhScriptFromHash(decoded.program),
      network: decoded.hrp === 'bc' ? 'mainnet' : 'testnet',
    };
  }

  const payload = readBase58Check(address);
  const version = payload[0];
  const hash = payload.subarray(1);
  if (!BITCOIN_P2PKH_VERSIONS.has(version) || hash.length !== 20) {
    throw new Error(
      `Only P2PKH and P2WPKH sender addresses are supported for Bitcoin signing: ${address}`,
    );
  }

  return {
    address,
    inputType: 'p2pkh',
    pubKeyHash: hash,
    prevScriptPubKey: p2pkhScriptFromHash(hash),
    scriptCode: p2pkhScriptFromHash(hash),
    network: version === 0x00 ? 'mainnet' : 'testnet',
  };
}

function deriveAddressVariants(address) {
  try {
    const details = describeInputAddress(address);
    const variants = [address];

    if (details.inputType === 'p2wpkh') {
      variants.push(
        encodeBase58Check(details.network === 'mainnet' ? 0x00 : 0x6f, details.pubKeyHash),
      );
    } else if (details.inputType === 'p2pkh') {
      variants.push(
        encodeSegwitAddress(details.network === 'mainnet' ? 'bc' : 'tb', 0, details.pubKeyHash),
      );
    }

    return [...new Set(variants)];
  } catch {
    return [address];
  }
}

function scriptPubKeyForAddress(address) {
  if (address.startsWith('bc1') || address.startsWith('tb1')) {
    const decoded = decodeSegwitAddress(address);
    if (!BITCOIN_WITNESS_HRPS.has(decoded.hrp)) {
      throw new Error(`Unsupported Bitcoin witness HRP for ${address}`);
    }
    const versionOpcode = decoded.version === 0 ? 0x00 : 0x50 + decoded.version;
    return Buffer.concat([Buffer.from([versionOpcode]), pushData(decoded.program)]);
  }

  const payload = readBase58Check(address);
  const version = payload[0];
  const hash = payload.subarray(1);
  if (hash.length !== 20) {
    throw new Error(`Unsupported Base58 address payload length for ${address}`);
  }

  if (BITCOIN_P2PKH_VERSIONS.has(version)) {
    return Buffer.concat([Buffer.from([0x76, 0xa9, 0x14]), hash, Buffer.from([0x88, 0xac])]);
  }

  if (BITCOIN_P2SH_VERSIONS.has(version)) {
    return Buffer.concat([Buffer.from([0xa9, 0x14]), hash, Buffer.from([0x87])]);
  }

  throw new Error(`Unsupported Bitcoin address version for ${address}`);
}

function serializeInput(input) {
  return Buffer.concat([
    reverseBuffer(Buffer.from(input.txid, 'hex')),
    encodeUInt32LE(input.vout),
    encodeVarInt(input.scriptSig.length),
    input.scriptSig,
    encodeUInt32LE(input.sequence ?? 0xffffffff),
  ]);
}

function serializeOutput(output) {
  return Buffer.concat([
    encodeUInt64LE(output.value),
    encodeVarInt(output.scriptPubKey.length),
    output.scriptPubKey,
  ]);
}

function serializeWitness(witness = []) {
  return Buffer.concat([
    encodeVarInt(witness.length),
    ...witness.map((item) => Buffer.concat([encodeVarInt(item.length), item])),
  ]);
}

function serializeTransaction(tx, options = {}) {
  const includeWitness =
    options.includeWitness === true &&
    tx.inputs.some((input) => Array.isArray(input.witness) && input.witness.length > 0);
  const parts = [encodeUInt32LE(tx.version ?? 2)];

  if (includeWitness) {
    parts.push(Buffer.from([0x00, 0x01]));
  }

  parts.push(
    encodeVarInt(tx.inputs.length),
    ...tx.inputs.map(serializeInput),
    encodeVarInt(tx.outputs.length),
    ...tx.outputs.map(serializeOutput),
  );

  if (includeWitness) {
    parts.push(...tx.inputs.map((input) => serializeWitness(input.witness || [])));
  }

  parts.push(encodeUInt32LE(tx.locktime ?? 0));
  return Buffer.concat(parts);
}

function estimateTransactionVBytes(inputs, outputs) {
  const hasWitness = inputs.some((input) => input.inputType === 'p2wpkh');
  let baseSize = 4 + encodeVarInt(inputs.length).length + encodeVarInt(outputs.length).length + 4;
  let witnessSize = hasWitness ? 2 : 0;

  for (const input of inputs) {
    const scriptSigLength = input.inputType === 'p2pkh' ? 109 : 0;
    baseSize += 36 + encodeVarInt(scriptSigLength).length + scriptSigLength + 4;
    if (input.inputType === 'p2wpkh') {
      witnessSize += 109;
    }
  }

  for (const output of outputs) {
    baseSize += 8 + encodeVarInt(output.scriptPubKey.length).length + output.scriptPubKey.length;
  }

  return Math.ceil((baseSize * 4 + witnessSize) / 4);
}

function clampFeeRate(value) {
  if (!Number.isFinite(value) || value <= 0) {
    return DEFAULT_FEE_RATE_SAT_VBYTE;
  }
  return Math.max(1, Math.min(500, Math.ceil(value)));
}

async function getFeeRate(chainId) {
  try {
    const chain = getChain(chainId);
    const estimates = await fetchJson(`${getEsploraBaseUrl(chainId)}/fee-estimates`);
    return clampFeeRate(
      estimates?.[String(chain?.executionConfirmations || 1)] ??
        estimates?.['2'] ??
        estimates?.['6'] ??
        estimates?.['1'],
    );
  } catch {
    return DEFAULT_FEE_RATE_SAT_VBYTE;
  }
}

async function getSpendableUtxos(address, chainId) {
  const addresses = deriveAddressVariants(address);
  const seen = new Set();
  const allUtxos = [];

  for (const sourceAddress of addresses) {
    const addressInfo = describeInputAddress(sourceAddress);
    const utxos = await fetchJson(`${getEsploraBaseUrl(chainId)}/address/${sourceAddress}/utxo`);
    for (const utxo of utxos) {
      if (!utxo?.status?.confirmed) {
        continue;
      }

      const key = `${utxo.txid}:${utxo.vout}`;
      if (seen.has(key)) {
        continue;
      }
      seen.add(key);
      allUtxos.push({
        txid: utxo.txid,
        vout: Number(utxo.vout),
        value: BigInt(utxo.value),
        status: utxo.status || {},
        sourceAddress,
        inputType: addressInfo.inputType,
        prevScriptPubKey: addressInfo.prevScriptPubKey,
        scriptCode: addressInfo.scriptCode,
        pubKeyHash: addressInfo.pubKeyHash,
      });
    }
  }

  return allUtxos.sort((a, b) => Number(b.value - a.value));
}

function selectUtxos(utxos, amount, recipientScript, changeScript, feeRate) {
  const selected = [];
  let total = 0n;
  const noChangeOutputs = [{ value: amount, scriptPubKey: recipientScript }];

  for (const utxo of utxos) {
    selected.push(utxo);
    total += utxo.value;

    const minFee = BigInt(estimateTransactionVBytes(selected, noChangeOutputs) * feeRate);
    if (total < amount + minFee) {
      continue;
    }

    const outputsWithChange = [
      { value: amount, scriptPubKey: recipientScript },
      { value: 0n, scriptPubKey: changeScript },
    ];
    const estimatedFee = BigInt(estimateTransactionVBytes(selected, outputsWithChange) * feeRate);
    const changeValue = total - amount - estimatedFee;

    if (changeValue >= DUST_THRESHOLD_SATS) {
      return {
        selectedUtxos: selected,
        totalInputValue: total,
        feeValue: estimatedFee,
        changeValue,
        outputs: [
          { value: amount, scriptPubKey: recipientScript },
          { value: changeValue, scriptPubKey: changeScript },
        ],
      };
    }

    const feeWithoutChange = total - amount;
    if (feeWithoutChange >= minFee) {
      return {
        selectedUtxos: selected,
        totalInputValue: total,
        feeValue: feeWithoutChange,
        changeValue: 0n,
        outputs: [{ value: amount, scriptPubKey: recipientScript }],
      };
    }
  }

  throw new Error('Insufficient confirmed BTC balance to cover amount and network fee');
}

function buildSigningTransaction(tx, inputIndex, prevScriptPubKey) {
  const inputs = tx.inputs.map((input, index) => ({
    ...input,
    scriptSig: index === inputIndex ? prevScriptPubKey : Buffer.alloc(0),
    witness: [],
  }));
  return {
    version: tx.version,
    inputs,
    outputs: tx.outputs,
    locktime: tx.locktime,
  };
}

function createSecp256k1PrivateKey(privateKey) {
  const ecdh = crypto.createECDH('secp256k1');
  ecdh.setPrivateKey(privateKey);
  const publicKey = ecdh.getPublicKey();
  const der = Buffer.concat([
    Buffer.from('30740201010420', 'hex'),
    privateKey,
    Buffer.from('a00706052b8104000aa14403420004', 'hex'),
    publicKey.subarray(1),
  ]);
  return crypto.createPrivateKey({
    key: der,
    format: 'der',
    type: 'sec1',
  });
}

function encodeDerInteger(value) {
  let hex = value.toString(16);
  if (hex.length % 2 !== 0) {
    hex = `0${hex}`;
  }
  let buffer = Buffer.from(hex, 'hex');
  while (buffer.length > 1 && buffer[0] === 0x00 && (buffer[1] & 0x80) === 0) {
    buffer = buffer.subarray(1);
  }
  if (buffer[0] & 0x80) {
    buffer = Buffer.concat([Buffer.from([0x00]), buffer]);
  }
  return buffer;
}

function encodeDerSignature(rawSignature) {
  const r = BigInt(`0x${rawSignature.subarray(0, 32).toString('hex')}`);
  const rawS = BigInt(`0x${rawSignature.subarray(32, 64).toString('hex')}`);
  const s = rawS > SECP256K1_HALF_ORDER ? SECP256K1_ORDER - rawS : rawS;
  const rEncoded = encodeDerInteger(r);
  const sEncoded = encodeDerInteger(s);
  const sequenceLength = 2 + rEncoded.length + 2 + sEncoded.length;
  return Buffer.concat([
    Buffer.from([0x30, sequenceLength, 0x02, rEncoded.length]),
    rEncoded,
    Buffer.from([0x02, sEncoded.length]),
    sEncoded,
  ]);
}

function signDigest(privateKey, digest) {
  const keyObject = createSecp256k1PrivateKey(privateKey);
  const rawSignature = crypto.sign(null, digest, {
    key: keyObject,
    dsaEncoding: 'ieee-p1363',
  });
  if (rawSignature.length !== 64) {
    throw new Error(`Unexpected secp256k1 signature length: ${rawSignature.length}`);
  }
  return encodeDerSignature(rawSignature);
}

function hashPrevouts(tx) {
  return sha256Double(
    Buffer.concat(
      tx.inputs.map((input) =>
        Buffer.concat([reverseBuffer(Buffer.from(input.txid, 'hex')), encodeUInt32LE(input.vout)]),
      ),
    ),
  );
}

function hashSequence(tx) {
  return sha256Double(
    Buffer.concat(tx.inputs.map((input) => encodeUInt32LE(input.sequence ?? 0xffffffff))),
  );
}

function hashOutputs(tx) {
  return sha256Double(Buffer.concat(tx.outputs.map(serializeOutput)));
}

function createSegwitDigest(tx, inputIndex, input) {
  const outpoint = Buffer.concat([
    reverseBuffer(Buffer.from(input.txid, 'hex')),
    encodeUInt32LE(input.vout),
  ]);

  return sha256Double(
    Buffer.concat([
      encodeUInt32LE(tx.version ?? 2),
      hashPrevouts(tx),
      hashSequence(tx),
      outpoint,
      encodeVarInt(input.scriptCode.length),
      input.scriptCode,
      encodeUInt64LE(input.value),
      encodeUInt32LE(input.sequence ?? 0xffffffff),
      hashOutputs(tx),
      encodeUInt32LE(tx.locktime ?? 0),
      encodeUInt32LE(SIGHASH_ALL),
    ]),
  );
}

function transactionIdFromTransaction(tx) {
  return reverseBuffer(sha256Double(serializeTransaction(tx))).toString('hex');
}

export async function buildBitcoinTransaction(intent, wallet, chainId, options = {}) {
  const { simulate = false } = options;
  const recipientScriptPubKey = scriptPubKeyForAddress(intent.toAddress);
  const changeScriptPubKey = scriptPubKeyForAddress(intent.fromAddress);
  const feeRateSatVbyte = await getFeeRate(chainId);
  const spendableUtxos = await getSpendableUtxos(intent.fromAddress, chainId);

  const selection = selectUtxos(
    spendableUtxos,
    intent.amountSmallest,
    recipientScriptPubKey,
    changeScriptPubKey,
    feeRateSatVbyte,
  );

  const tx = {
    version: 2,
    inputs: selection.selectedUtxos.map((utxo) => ({
      txid: utxo.txid,
      vout: utxo.vout,
      inputType: utxo.inputType,
      scriptSig: Buffer.alloc(0),
      witness: [],
      sequence: 0xffffffff,
      prevScriptPubKey: utxo.prevScriptPubKey,
      scriptCode: utxo.scriptCode,
      value: utxo.value,
    })),
    outputs: selection.outputs,
    locktime: 0,
  };

  return {
    type: 'bitcoin_native_transfer',
    tx,
    fromAddress: intent.fromAddress,
    toAddress: intent.toAddress,
    changeAddress: wallet.address,
    feeRateSatVbyte,
    feeValue: selection.feeValue,
    totalInputValue: selection.totalInputValue,
    changeValue: selection.changeValue,
    selectedUtxos: selection.selectedUtxos,
    preview: simulate
      ? {
          inputs: selection.selectedUtxos.map((utxo) => ({
            txid: utxo.txid,
            vout: utxo.vout,
            value: utxo.value.toString(),
          })),
          outputs: selection.outputs.map((output) => ({
            value: output.value.toString(),
            scriptPubKey: output.scriptPubKey.toString('hex'),
          })),
          estimatedFeeSat: selection.feeValue.toString(),
          changeSat: selection.changeValue.toString(),
          feeRateSatVbyte,
        }
      : null,
  };
}

export async function signBitcoinTransaction(txData, wallet) {
  const signedInputs = txData.tx.inputs.map((input, index) => {
    if (input.inputType === 'p2wpkh') {
      const digest = createSegwitDigest(txData.tx, index, input);
      const witnessSignature = Buffer.concat([
        signDigest(wallet.privateKey, digest),
        Buffer.from([SIGHASH_ALL]),
      ]);
      return {
        ...input,
        scriptSig: Buffer.alloc(0),
        witness: [witnessSignature, wallet.publicKey],
      };
    }

    const signingTx = buildSigningTransaction(txData.tx, index, input.prevScriptPubKey);
    const digest = sha256Double(
      Buffer.concat([serializeTransaction(signingTx), encodeUInt32LE(SIGHASH_ALL)]),
    );
    const legacySignature = Buffer.concat([
      signDigest(wallet.privateKey, digest),
      Buffer.from([SIGHASH_ALL]),
    ]);
    return {
      ...input,
      scriptSig: Buffer.concat([pushData(legacySignature), pushData(wallet.publicKey)]),
      witness: [],
    };
  });

  const signedTx = {
    version: txData.tx.version,
    inputs: signedInputs,
    outputs: txData.tx.outputs,
    locktime: txData.tx.locktime,
  };
  const rawHex = serializeTransaction(signedTx, { includeWitness: true }).toString('hex');
  const txHash = transactionIdFromTransaction(signedTx);

  return {
    ...txData,
    tx: signedTx,
    rawHex,
    txHash,
    signedAt: new Date().toISOString(),
  };
}

export async function submitBitcoinTransaction(signedTx, chainId) {
  const txHash = await fetchText(`${getEsploraBaseUrl(chainId)}/tx`, {
    method: 'POST',
    headers: {
      'content-type': 'text/plain',
    },
    body: signedTx.rawHex,
  });

  return {
    txHash: txHash.trim(),
    submittedAt: new Date().toISOString(),
  };
}

export async function getBitcoinTransactionStatus(txHash, chainId) {
  const status = await fetchJson(`${getEsploraBaseUrl(chainId)}/tx/${txHash}/status`);
  if (!status?.confirmed) {
    return {
      confirmed: false,
      confirmations: 0,
    };
  }

  const tipHeight = Number(
    await fetchText(`${getEsploraBaseUrl(chainId)}/blocks/tip/height`, {
      headers: { accept: 'text/plain' },
    }),
  );
  const blockNumber = Number(status.block_height || 0);

  return {
    confirmed: true,
    blockNumber,
    confirmations: Math.max(1, tipHeight - blockNumber + 1),
    confirmedAt: status.block_time
      ? new Date(Number(status.block_time) * 1000).toISOString()
      : new Date().toISOString(),
  };
}

export async function getBitcoinBalance(address, chainId) {
  const spendableUtxos = await getSpendableUtxos(address, chainId);
  return spendableUtxos.reduce((sum, utxo) => sum + utxo.value, 0n);
}
