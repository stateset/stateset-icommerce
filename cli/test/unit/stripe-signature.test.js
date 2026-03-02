import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'crypto';

import {
  parseStripeSignatureHeader,
  computeSignature,
  verifyStripeSignature,
} from '../../src/adapters/stripe/signature.js';

const TEST_SECRET = 'whsec_test_secret_1234567890';
const TEST_BODY = '{"id":"evt_test","type":"payment_intent.succeeded"}';

function makeSignatureHeader(body, secret, timestamp = Math.floor(Date.now() / 1000)) {
  const sig = crypto
    .createHmac('sha256', secret)
    .update(`${timestamp}.${body}`, 'utf-8')
    .digest('hex');
  return { header: `t=${timestamp},v1=${sig}`, timestamp, sig };
}

// ---------------------------------------------------------------------------
// parseStripeSignatureHeader
// ---------------------------------------------------------------------------

describe('stripe signature — parseStripeSignatureHeader', () => {
  it('parses a valid header', () => {
    const { header } = makeSignatureHeader(TEST_BODY, TEST_SECRET);
    const parsed = parseStripeSignatureHeader(header);
    assert.ok(parsed.timestamp > 0);
    assert.equal(parsed.signatures.length, 1);
  });

  it('parses multiple v1 signatures', () => {
    const parsed = parseStripeSignatureHeader('t=1234567890,v1=sig1,v1=sig2');
    assert.equal(parsed.signatures.length, 2);
    assert.deepEqual(parsed.signatures, ['sig1', 'sig2']);
  });

  it('throws on null input', () => {
    assert.throws(() => parseStripeSignatureHeader(null), /Missing/);
  });

  it('throws on empty string', () => {
    assert.throws(() => parseStripeSignatureHeader(''), /Missing/);
  });

  it('throws on missing timestamp', () => {
    assert.throws(() => parseStripeSignatureHeader('v1=abc'), /No timestamp/);
  });

  it('throws on invalid timestamp', () => {
    assert.throws(() => parseStripeSignatureHeader('t=abc,v1=sig'), /Invalid timestamp/);
  });

  it('throws on missing v1 signatures', () => {
    assert.throws(() => parseStripeSignatureHeader('t=1234567890'), /No v1 signatures/);
  });
});

// ---------------------------------------------------------------------------
// computeSignature
// ---------------------------------------------------------------------------

describe('stripe signature — computeSignature', () => {
  it('produces consistent hex output', () => {
    const sig1 = computeSignature(TEST_BODY, 1709251200, TEST_SECRET);
    const sig2 = computeSignature(TEST_BODY, 1709251200, TEST_SECRET);
    assert.equal(sig1, sig2);
  });

  it('changes with different body', () => {
    const sig1 = computeSignature('body1', 1000, TEST_SECRET);
    const sig2 = computeSignature('body2', 1000, TEST_SECRET);
    assert.notEqual(sig1, sig2);
  });

  it('changes with different timestamp', () => {
    const sig1 = computeSignature(TEST_BODY, 1000, TEST_SECRET);
    const sig2 = computeSignature(TEST_BODY, 2000, TEST_SECRET);
    assert.notEqual(sig1, sig2);
  });

  it('changes with different secret', () => {
    const sig1 = computeSignature(TEST_BODY, 1000, 'secret1');
    const sig2 = computeSignature(TEST_BODY, 1000, 'secret2');
    assert.notEqual(sig1, sig2);
  });
});

// ---------------------------------------------------------------------------
// verifyStripeSignature
// ---------------------------------------------------------------------------

describe('stripe signature — verifyStripeSignature', () => {
  it('accepts a valid signature', () => {
    const { header } = makeSignatureHeader(TEST_BODY, TEST_SECRET);
    const result = verifyStripeSignature(TEST_BODY, header, TEST_SECRET);
    assert.equal(result.valid, true);
  });

  it('rejects wrong body', () => {
    const { header } = makeSignatureHeader(TEST_BODY, TEST_SECRET);
    const result = verifyStripeSignature('tampered', header, TEST_SECRET);
    assert.equal(result.valid, false);
    assert.match(result.error, /mismatch/i);
  });

  it('rejects wrong secret', () => {
    const { header } = makeSignatureHeader(TEST_BODY, TEST_SECRET);
    const result = verifyStripeSignature(TEST_BODY, header, 'wrong_secret');
    assert.equal(result.valid, false);
  });

  it('rejects expired timestamp', () => {
    const oldTimestamp = Math.floor(Date.now() / 1000) - 600; // 10 minutes ago
    const { header } = makeSignatureHeader(TEST_BODY, TEST_SECRET, oldTimestamp);
    const result = verifyStripeSignature(TEST_BODY, header, TEST_SECRET, 300);
    assert.equal(result.valid, false);
    assert.match(result.error, /tolerance/i);
  });

  it('accepts timestamp within tolerance', () => {
    const recentTimestamp = Math.floor(Date.now() / 1000) - 60; // 1 minute ago
    const { header } = makeSignatureHeader(TEST_BODY, TEST_SECRET, recentTimestamp);
    const result = verifyStripeSignature(TEST_BODY, header, TEST_SECRET, 300);
    assert.equal(result.valid, true);
  });

  it('rejects missing body', () => {
    const result = verifyStripeSignature(null, 't=1,v1=a', TEST_SECRET);
    assert.equal(result.valid, false);
    assert.match(result.error, /body/i);
  });

  it('rejects missing secret', () => {
    const result = verifyStripeSignature(TEST_BODY, 't=1,v1=a', null);
    assert.equal(result.valid, false);
    assert.match(result.error, /secret/i);
  });

  it('rejects malformed header', () => {
    const result = verifyStripeSignature(TEST_BODY, 'garbage', TEST_SECRET);
    assert.equal(result.valid, false);
  });
});
