/**
 * Unit tests for a2a/webhook-verify.js — Webhook Signature Verification SDK
 *
 * Covers: verifyWebhookSignature, verifyWebhookTimestamp, createWebhookVerifier,
 * extractWebhookHeaders, isReplayAttack, constant-time comparison, edge cases.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { createHmac } from 'node:crypto';
import {
  verifyWebhookSignature,
  verifyWebhookTimestamp,
  createWebhookVerifier,
  extractWebhookHeaders,
  isReplayAttack,
} from '../../src/a2a/webhook-verify.js';

// ─── Helpers ─────────────────────────────────────────────────────────────────

const SECRET = 'whsec_test_secret_123';

/** Build a valid HMAC-SHA256 hex for a given body and secret */
function sign(body, secret = SECRET) {
  return createHmac('sha256', secret).update(body).digest('hex');
}

/** Build a full set of StateSet webhook headers */
function buildHeaders(body, overrides = {}) {
  const timestamp = overrides.timestamp ?? new Date().toISOString();
  const event = overrides.event ?? 'payment.completed';
  const idempotencyKey = overrides.idempotencyKey ?? 'idem-001';
  const deliveryId = overrides.deliveryId ?? 'del-001';
  const sig = overrides.signature ?? `sha256=${sign(body)}`;

  return {
    'x-stateset-signature': sig,
    'x-stateset-timestamp': timestamp,
    'x-stateset-event': event,
    'x-stateset-idempotency-key': idempotencyKey,
    'x-stateset-delivery-id': deliveryId,
  };
}

// ─── verifyWebhookSignature ──────────────────────────────────────────────────

describe('verifyWebhookSignature', () => {
  it('validates a correct HMAC-SHA256 signature', () => {
    const body = JSON.stringify({ event_type: 'payment.completed', payload: { id: 'p1' } });
    const header = `sha256=${sign(body)}`;

    const result = verifyWebhookSignature(body, header, SECRET);
    assert.equal(result.valid, true);
    assert.equal(result.error, undefined);
  });

  it('rejects a signature computed with the wrong secret', () => {
    const body = JSON.stringify({ event_type: 'payment.completed' });
    const header = `sha256=${sign(body, 'wrong-secret')}`;

    const result = verifyWebhookSignature(body, header, SECRET);
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('mismatch'));
  });

  it('rejects a malformed signature header (missing prefix)', () => {
    const body = '{}';
    const hex = sign(body);

    const result = verifyWebhookSignature(body, hex, SECRET);
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('sha256='));
  });

  it('rejects a malformed signature header (invalid hex)', () => {
    const body = '{}';

    const result = verifyWebhookSignature(body, 'sha256=ZZZZ', SECRET);
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('hex'));
  });

  it('returns error when secret is empty string', () => {
    const body = '{}';
    const result = verifyWebhookSignature(body, 'sha256=aabb', '');
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('secret'));
  });

  it('returns error when secret is undefined', () => {
    const body = '{}';
    const result = verifyWebhookSignature(body, 'sha256=aabb', undefined);
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('secret'));
  });

  it('returns error when body is empty string', () => {
    const result = verifyWebhookSignature('', 'sha256=aabb', SECRET);
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('body'));
  });

  it('returns error when body is undefined', () => {
    const result = verifyWebhookSignature(undefined, 'sha256=aabb', SECRET);
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('body'));
  });

  it('returns error when signature header is undefined', () => {
    const result = verifyWebhookSignature('{}', undefined, SECRET);
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('Signature header'));
  });

  it('uses constant-time comparison (timing-safe)', () => {
    // This test verifies the code path rather than measuring actual timing.
    // We confirm that a near-miss signature (first byte wrong) is rejected the
    // same way as a completely different signature — both go through
    // timingSafeEqual.  Actual timing-attack prevention is a property of
    // crypto.timingSafeEqual itself (verified by Node.js).
    const body = '{"test": true}';
    const correctHex = sign(body);

    // Flip one character in the middle of the hex
    const chars = correctHex.split('');
    chars[16] = chars[16] === 'a' ? 'b' : 'a';
    const nearMiss = chars.join('');

    const r1 = verifyWebhookSignature(body, `sha256=${nearMiss}`, SECRET);
    assert.equal(r1.valid, false);

    // Completely wrong signature (same length)
    const allZeros = '0'.repeat(correctHex.length);
    const r2 = verifyWebhookSignature(body, `sha256=${allZeros}`, SECRET);
    assert.equal(r2.valid, false);
  });
});

// ─── verifyWebhookTimestamp ──────────────────────────────────────────────────

describe('verifyWebhookTimestamp', () => {
  it('accepts a fresh timestamp (just now)', () => {
    const ts = new Date().toISOString();
    const result = verifyWebhookTimestamp(ts);
    assert.equal(result.valid, true);
    assert.ok(result.ageMs >= 0);
    assert.ok(result.ageMs < 5000); // Should be near-zero
  });

  it('accepts a timestamp within tolerance', () => {
    const ts = new Date(Date.now() - 60_000).toISOString(); // 1 minute ago
    const result = verifyWebhookTimestamp(ts, 300_000);
    assert.equal(result.valid, true);
    assert.ok(result.ageMs >= 59_000 && result.ageMs <= 62_000);
  });

  it('rejects a timestamp older than tolerance', () => {
    const ts = new Date(Date.now() - 600_000).toISOString(); // 10 minutes ago
    const result = verifyWebhookTimestamp(ts, 300_000);
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('old'));
    assert.ok(result.ageMs >= 599_000);
  });

  it('rejects a timestamp far in the future', () => {
    const ts = new Date(Date.now() + 600_000).toISOString(); // 10 minutes ahead
    const result = verifyWebhookTimestamp(ts, 300_000);
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('future'));
  });

  it('rejects an invalid date string', () => {
    const result = verifyWebhookTimestamp('not-a-date');
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('format'));
    assert.equal(result.ageMs, -1);
  });

  it('rejects when timestamp header is empty', () => {
    const result = verifyWebhookTimestamp('');
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('required'));
  });

  it('rejects when timestamp header is undefined', () => {
    const result = verifyWebhookTimestamp(undefined);
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('required'));
  });

  it('accepts a timestamp with custom short tolerance', () => {
    const ts = new Date(Date.now() - 500).toISOString(); // 500ms ago
    const result = verifyWebhookTimestamp(ts, 1000); // 1s tolerance
    assert.equal(result.valid, true);
  });

  it('rejects a slightly stale timestamp with tight tolerance', () => {
    const ts = new Date(Date.now() - 2000).toISOString(); // 2s ago
    const result = verifyWebhookTimestamp(ts, 1000); // 1s tolerance
    assert.equal(result.valid, false);
  });
});

// ─── extractWebhookHeaders ───────────────────────────────────────────────────

describe('extractWebhookHeaders', () => {
  it('parses all StateSet webhook headers from a flat object', () => {
    const body = '{}';
    const headers = buildHeaders(body, {
      event: 'escrow.released',
      idempotencyKey: 'idem-42',
      deliveryId: 'del-42',
    });

    const parsed = extractWebhookHeaders(headers);
    assert.ok(parsed.signature.startsWith('sha256='));
    assert.ok(parsed.timestamp);
    assert.equal(parsed.event, 'escrow.released');
    assert.equal(parsed.idempotencyKey, 'idem-42');
    assert.equal(parsed.deliveryId, 'del-42');
  });

  it('extracts from an IncomingMessage-style object with .headers property', () => {
    const body = '{}';
    const raw = buildHeaders(body);
    const req = { headers: raw };

    const parsed = extractWebhookHeaders(req);
    assert.ok(parsed.signature);
    assert.ok(parsed.timestamp);
    assert.ok(parsed.event);
  });

  it('returns undefined for missing headers', () => {
    const parsed = extractWebhookHeaders({});
    assert.equal(parsed.signature, undefined);
    assert.equal(parsed.timestamp, undefined);
    assert.equal(parsed.event, undefined);
    assert.equal(parsed.idempotencyKey, undefined);
    assert.equal(parsed.deliveryId, undefined);
  });

  it('handles null/undefined input gracefully', () => {
    const r1 = extractWebhookHeaders(null);
    assert.equal(r1.signature, undefined);

    const r2 = extractWebhookHeaders(undefined);
    assert.equal(r2.signature, undefined);
  });
});

// ─── isReplayAttack ──────────────────────────────────────────────────────────

describe('isReplayAttack', () => {
  it('detects a duplicate idempotency key', () => {
    const seen = new Set(['idem-001', 'idem-002']);
    assert.equal(isReplayAttack('idem-001', seen), true);
  });

  it('returns false for a fresh idempotency key', () => {
    const seen = new Set(['idem-001']);
    assert.equal(isReplayAttack('idem-999', seen), false);
  });

  it('returns false when idempotencyKey is undefined', () => {
    const seen = new Set(['idem-001']);
    assert.equal(isReplayAttack(undefined, seen), false);
  });

  it('returns false when seenKeys is undefined', () => {
    assert.equal(isReplayAttack('idem-001', undefined), false);
  });

  it('returns false for empty Set', () => {
    assert.equal(isReplayAttack('idem-001', new Set()), false);
  });
});

// ─── createWebhookVerifier ───────────────────────────────────────────────────

describe('createWebhookVerifier', () => {
  it('verify() validates a full valid request', () => {
    const body = JSON.stringify({ event_type: 'payment.completed', payload: { id: 'p1' }, timestamp: new Date().toISOString() });
    const headers = buildHeaders(body);
    const verifier = createWebhookVerifier(SECRET);

    const result = verifier.verify({ headers, body });
    assert.equal(result.valid, true);
    assert.equal(result.event, 'payment.completed');
    assert.ok(result.timestamp);
    assert.equal(result.idempotencyKey, 'idem-001');
    assert.ok(result.payload);
    assert.equal(result.payload.event_type, 'payment.completed');
  });

  it('rejects when signature is wrong', () => {
    const body = JSON.stringify({ test: true });
    const headers = buildHeaders(body, { signature: 'sha256=0000000000000000000000000000000000000000000000000000000000000000' });
    const verifier = createWebhookVerifier(SECRET);

    const result = verifier.verify({ headers, body });
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('mismatch'));
  });

  it('rejects when timestamp is too old', () => {
    const body = JSON.stringify({ test: true });
    const staleTs = new Date(Date.now() - 600_000).toISOString();
    const headers = buildHeaders(body, { timestamp: staleTs });
    const verifier = createWebhookVerifier(SECRET, { timestampToleranceMs: 300_000 });

    const result = verifier.verify({ headers, body });
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('old'));
  });

  it('rejects when timestamp header is missing and requireTimestamp=true', () => {
    const body = JSON.stringify({ test: true });
    const sig = `sha256=${sign(body)}`;
    const headers = {
      'x-stateset-signature': sig,
      'x-stateset-event': 'test.event',
    };
    const verifier = createWebhookVerifier(SECRET, { requireTimestamp: true });

    const result = verifier.verify({ headers, body });
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('Timestamp'));
  });

  it('accepts when timestamp header is missing and requireTimestamp=false', () => {
    const body = JSON.stringify({ test: true });
    const sig = `sha256=${sign(body)}`;
    const headers = {
      'x-stateset-signature': sig,
      'x-stateset-event': 'test.event',
    };
    const verifier = createWebhookVerifier(SECRET, { requireTimestamp: false });

    const result = verifier.verify({ headers, body });
    assert.equal(result.valid, true);
  });

  it('rejects when req is null', () => {
    const verifier = createWebhookVerifier(SECRET);
    const result = verifier.verify(null);
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('Request'));
  });

  it('rejects when body is not a string', () => {
    const verifier = createWebhookVerifier(SECRET);
    const result = verifier.verify({ headers: {}, body: 123 });
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('string'));
  });

  it('rejects when body is not valid JSON', () => {
    const body = 'not-json{{{';
    const sig = `sha256=${sign(body)}`;
    const headers = {
      'x-stateset-signature': sig,
      'x-stateset-timestamp': new Date().toISOString(),
      'x-stateset-event': 'test.event',
    };
    const verifier = createWebhookVerifier(SECRET);

    const result = verifier.verify({ headers, body });
    assert.equal(result.valid, false);
    assert.ok(result.error.includes('JSON'));
  });

  it('returns deliveryId from headers', () => {
    const body = JSON.stringify({ ok: 1 });
    const headers = buildHeaders(body, { deliveryId: 'delivery-xyz' });
    const verifier = createWebhookVerifier(SECRET);

    const result = verifier.verify({ headers, body });
    assert.equal(result.valid, true);
    assert.equal(result.deliveryId, 'delivery-xyz');
  });
});
