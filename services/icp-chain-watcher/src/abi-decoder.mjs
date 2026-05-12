// Hand-rolled ABI decoder for the 5 ICPEscrow.sol events.
//
// Zero dependencies. Implements only the Solidity ABI subset needed for
// these specific event shapes:
//   - bytes32 (indexed + non-indexed)
//   - address (indexed)
//   - uint128, uint64 (non-indexed)
//   - string (non-indexed, variable-length)
//
// Topic hashes were computed via `cast keccak '<signature>'` and verified
// against the foundry/forge tooling installed in this repo. They are
// stable for the lifetime of the ICPEscrow.sol contract.

/** keccak256 of each event signature. Match topics[0] to identify the event. */
export const EVENT_TOPICS = {
  EscrowFunded:   '0x5c5e9cbd002f416577cd999eb1297865013aecaf0f8c6f593e56c9c334d4644f',
  EscrowDisputed: '0x85df63e82b1c4b692591e851fd05ac7c87d4dd28557d780c47c462a11f64e0c8',
  EscrowReleased: '0x95d522762e04d28e21709344963474d18d6d8c19cea99865cf53029a3c25ec54',
  EscrowRefunded: '0xa3a9c68367292ca26571c2c1b730c525eb110a42666b162ac6ceeb25ffa461f0',
  EscrowResolved: '0x86e741358ba245b5ec9be2af9edd5f3c7be4399b7701dc5e4b009ea0aeac0302',
};

/** Inverse map for fast event name lookup. */
export const EVENT_NAMES = Object.fromEntries(
  Object.entries(EVENT_TOPICS).map(([name, topic]) => [topic.toLowerCase(), name]),
);

/**
 * Decode an eth_getLogs entry into a normalized event object.
 *
 * @param {{address: string, topics: string[], data: string, blockNumber: string, transactionHash: string, logIndex: string}} log
 * @returns {object|null} Decoded event { eventName, ...fields, rail_event } or null if topic0 unrecognized.
 */
export function decodeLog(log) {
  const t0 = (log.topics?.[0] || '').toLowerCase();
  const eventName = EVENT_NAMES[t0];
  if (!eventName) return null;

  const railEvent = {
    rail: 'evm',
    block_number: parseInt(log.blockNumber, 16),
    tx_hash: log.transactionHash,
    log_index: parseInt(log.logIndex, 16),
    contract: log.address,
  };

  const data = stripHex(log.data);

  switch (eventName) {
    case 'EscrowFunded': {
      // indexed: escrowId (bytes32), buyer (address), merchant (address)
      // data:    amount (uint128), fulfillmentDeadline (uint64), disputeWindow (uint64), quoteHash (bytes32)
      return {
        eventName,
        escrow_id: log.topics[1],
        buyer: decodeAddress(log.topics[2]),
        merchant: decodeAddress(log.topics[3]),
        amount: decodeUint(data, 0, 128).toString(),
        fulfillment_deadline: Number(decodeUint(data, 1, 64)),
        dispute_window: Number(decodeUint(data, 2, 64)),
        quote_hash: '0x' + data.slice(3 * 64, 4 * 64),
        rail_event: railEvent,
      };
    }
    case 'EscrowDisputed': {
      // indexed: escrowId, by
      // data:    reason (string)
      return {
        eventName,
        escrow_id: log.topics[1],
        by: decodeAddress(log.topics[2]),
        reason: decodeString(data, 0),
        rail_event: railEvent,
      };
    }
    case 'EscrowReleased': {
      // indexed: escrowId, merchant
      // data:    amount (uint128), fulfillmentReceiptHash (bytes32)
      return {
        eventName,
        escrow_id: log.topics[1],
        merchant: decodeAddress(log.topics[2]),
        amount: decodeUint(data, 0, 128).toString(),
        fulfillment_receipt_hash: '0x' + data.slice(1 * 64, 2 * 64),
        rail_event: railEvent,
      };
    }
    case 'EscrowRefunded': {
      // indexed: escrowId, buyer
      // data:    amount (uint128), reason (string)
      return {
        eventName,
        escrow_id: log.topics[1],
        buyer: decodeAddress(log.topics[2]),
        amount: decodeUint(data, 0, 128).toString(),
        reason: decodeString(data, 1),
        rail_event: railEvent,
      };
    }
    case 'EscrowResolved': {
      // indexed: escrowId, beneficiary
      // data:    amount (uint128), arbitrationDecisionHash (bytes32)
      return {
        eventName,
        escrow_id: log.topics[1],
        beneficiary: decodeAddress(log.topics[2]),
        amount: decodeUint(data, 0, 128).toString(),
        arbitration_decision_hash: '0x' + data.slice(1 * 64, 2 * 64),
        rail_event: railEvent,
      };
    }
    default:
      return null;
  }
}

// ===========================================================================
// Primitives — Solidity ABI subset
// ===========================================================================

/**
 * Decode a Solidity address from a 32-byte (64-char hex) padded value.
 * Address is the last 20 bytes (40 hex chars).
 */
export function decodeAddress(topic) {
  const h = stripHex(topic);
  return '0x' + h.slice(h.length - 40).toLowerCase();
}

/**
 * Decode a uint at word offset `wordIndex`, where the type is `bits` wide.
 * Returns a BigInt (callers should toString() for serialization).
 *
 * @param {string} dataHex  hex string (no 0x prefix) of the event's data field
 * @param {number} wordIndex zero-based 32-byte word index
 * @param {number} bits     bit width of the uint (e.g. 128, 64, 32)
 */
export function decodeUint(dataHex, wordIndex, bits) {
  const word = dataHex.slice(wordIndex * 64, (wordIndex + 1) * 64);
  if (word.length !== 64) throw new Error(`abi: short word at index ${wordIndex}: ${word.length} chars`);
  // For uintN where N < 256, the value is right-aligned (big-endian) in the word.
  // Take the last 2*ceil(N/8) hex chars.
  const bytes = bits / 8;
  const valueHex = word.slice(64 - 2 * bytes);
  return BigInt('0x' + valueHex);
}

/**
 * Decode a Solidity string from the event data.
 *
 * Strings (and other dynamic types) are encoded as:
 *   <head>  = 32-byte pointer (offset from start of data) to the tail
 *   <tail>  = 32-byte length, then UTF-8 bytes padded up to a 32-byte boundary
 *
 * @param {string} dataHex   hex (no 0x prefix) of the event's data field
 * @param {number} wordIndex zero-based word index of the head pointer
 * @returns {string}
 */
export function decodeString(dataHex, wordIndex) {
  // The head at wordIndex holds a 32-byte offset (in BYTES from start of data).
  const headWord = dataHex.slice(wordIndex * 64, (wordIndex + 1) * 64);
  const offsetBytes = Number(BigInt('0x' + headWord));
  const offsetHex = offsetBytes * 2;
  // Tail: 32-byte length, then content.
  const lengthHex = dataHex.slice(offsetHex, offsetHex + 64);
  const length = Number(BigInt('0x' + lengthHex));
  const contentHex = dataHex.slice(offsetHex + 64, offsetHex + 64 + length * 2);
  // UTF-8 decode
  const bytes = Buffer.from(contentHex, 'hex');
  return bytes.toString('utf8');
}

function stripHex(s) {
  if (!s) return '';
  return s.toLowerCase().startsWith('0x') ? s.slice(2) : s;
}
