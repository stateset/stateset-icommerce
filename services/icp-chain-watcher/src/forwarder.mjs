// Maps decoded ICPEscrow events to settler-stateset /admin/escrow/event
// payloads, then POSTs them.
//
// The Settler daemon (mock or chain mode) treats /admin/escrow/event as the
// authoritative source of lifecycle transitions. In mock mode the daemon
// accepts arbitrary injection; in chain mode (this watcher), every event
// reflects an on-chain log observed at the configured chain finality depth.

/**
 * Translate a decoded ICPEscrow event into a Settler admin event POST body.
 * Returns null if the event has no Settler-side semantics (shouldn't happen
 * for the 5 known events).
 *
 * @param {object} decoded  Output of abi-decoder.decodeLog()
 * @returns {object|null}   Settler-admin payload
 */
export function decodedToSettlerEvent(decoded) {
  if (!decoded) return null;
  const rail = decoded.rail_event;

  switch (decoded.eventName) {
    case 'EscrowFunded':
      return {
        escrow_id: decoded.escrow_id,
        kind: 'fund',
        init: {
          intent_id: null, // chain doesn't carry intent_id; Settler may need to resolve via quote_hash
          amount: { amount: weiToDecimal(decoded.amount, 6), currency: 'USDC' }, // USDC = 6 decimals
          buyer: decoded.buyer,
          merchant: decoded.merchant,
          quote_hash: decoded.quote_hash,
          fulfillment_deadline: decoded.fulfillment_deadline,
          dispute_window: decoded.dispute_window,
        },
        rail_event: rail,
      };
    case 'EscrowDisputed':
      return {
        escrow_id: decoded.escrow_id,
        kind: 'dispute',
        reason: decoded.reason,
        rail_event: rail,
      };
    case 'EscrowReleased':
      return {
        escrow_id: decoded.escrow_id,
        kind: 'release',
        payout_amount: weiToDecimal(decoded.amount, 6),
        payout_currency: 'USDC',
        rail_event: rail,
        fulfillment_receipt_hash: decoded.fulfillment_receipt_hash,
      };
    case 'EscrowRefunded':
      return {
        escrow_id: decoded.escrow_id,
        kind: 'refund',
        reason: decoded.reason,
        payout_amount: weiToDecimal(decoded.amount, 6),
        payout_currency: 'USDC',
        rail_event: rail,
      };
    case 'EscrowResolved':
      // EscrowResolved → release or refund based on beneficiary direction.
      // In chain mode the Settler treats it as `release` (funds going to merchant)
      // and inspects beneficiary; for refund (funds back to buyer) callers can
      // detect by comparing beneficiary == buyer from the prior EscrowFunded.
      return {
        escrow_id: decoded.escrow_id,
        kind: 'release',
        payout_amount: weiToDecimal(decoded.amount, 6),
        payout_currency: 'USDC',
        rail_event: rail,
        arbitration_decision_hash: decoded.arbitration_decision_hash,
        beneficiary: decoded.beneficiary,
      };
    default:
      return null;
  }
}

/**
 * POST a Settler admin event to the running settler-stateset daemon.
 * Returns the parsed response on success; throws on non-2xx.
 */
export async function postToSettler(settlerUrl, payload) {
  const url = `${settlerUrl.replace(/\/+$/, '')}/admin/escrow/event`;
  const r = await fetch(url, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload),
  });
  if (!r.ok) {
    const body = await r.text().catch(() => '');
    throw new Error(`settler returned ${r.status}: ${body}`);
  }
  return r.json();
}

/**
 * Convert a uint amount in base units (BigInt or string) to a decimal-formatted
 * string with `decimals` fractional digits. USDC uses 6.
 *
 * Examples (decimals=6):
 *   "100000000"  → "100.000000"  (100 USDC)
 *   "1500000"    → "1.500000"
 */
function weiToDecimal(value, decimals) {
  const big = typeof value === 'bigint' ? value : BigInt(value);
  const divisor = 10n ** BigInt(decimals);
  const whole = (big / divisor).toString();
  const frac = (big % divisor).toString().padStart(decimals, '0');
  return `${whole}.${frac}`;
}
