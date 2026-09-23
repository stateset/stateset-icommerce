/**
 * `sync doctor` — inspect quarantine, inspect pins, promote events that now verify.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import Database from 'better-sqlite3';

import { createOutbox } from '../../src/sync/outbox.js';
import { syncDoctor } from '../../src/commands/sync.js';

const AGENT = '44444444-4444-4444-4444-444444444444';

function sampleEvent(index = 1) {
  return {
    sequenceNumber: index,
    eventId: `11111111-1111-1111-1111-${String(index).padStart(12, '0')}`,
    tenantId: '22222222-2222-2222-2222-222222222222',
    storeId: '33333333-3333-3333-3333-333333333333',
    entityType: 'order',
    entityId: 'ORD-1',
    eventType: 'order.created',
    vesVersion: 1,
    payload: { total: 1 },
    payloadKind: 0,
    payloadPlainHash: '0x00',
    payloadCipherHash: '0x00',
    agentKeyId: 1,
    agentSignature: '0xdead',
    baseVersion: 0,
    createdAt: '2026-09-12T00:00:00.000Z',
    sequencedAt: '2026-09-12T00:00:01.000Z',
    sourceAgent: AGENT,
  };
}

describe('syncDoctor', () => {
  let db;
  let outbox;

  beforeEach(() => {
    db = new Database(':memory:');
    outbox = createOutbox(db, {});
  });

  afterEach(() => db.close());

  it('summarizes quarantined events by reason', async () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'key_unresolved');

    const report = await syncDoctor({ outbox, client: {}, keyDirectory: {} });
    assert.deepEqual(report.quarantined, [{ reason: 'key_unresolved', count: 1 }]);
    assert.equal(report.promoted, 0);
  });

  it('promotes events that verify once the key arrives', async () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'key_unresolved');

    const report = await syncDoctor({
      outbox,
      client: { verifyEventSignature: () => true },
      keyDirectory: {
        async resolve() {
          return { publicKey: '0xaa', publicKeyBundle: null };
        },
      },
      promote: true,
    });

    assert.equal(report.promoted, 1);
    assert.equal(outbox.getPulledEvents().length, 1);
    assert.equal(outbox.getQuarantinedEvents().length, 0);
  });

  it('leaves events that still do not verify in quarantine', async () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'signature_invalid');

    const report = await syncDoctor({
      outbox,
      client: { verifyEventSignature: () => false },
      keyDirectory: {
        async resolve() {
          return { publicKey: '0xaa', publicKeyBundle: null };
        },
      },
      promote: true,
    });

    assert.equal(report.promoted, 0);
    assert.equal(outbox.getQuarantinedEvents().length, 1);
  });

  it('lists current pins', async () => {
    outbox.pinPeerKey(AGENT, 1, '0xaa');
    const report = await syncDoctor({ outbox, client: {}, keyDirectory: {} });
    assert.equal(report.pins.length, 1);
    assert.equal(report.pins[0].agentId, AGENT);
    assert.equal(report.pins[0].keyId, 1);
  });

  it('is read-only unless promote is true', async () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'key_unresolved');

    const report = await syncDoctor({
      outbox,
      client: { verifyEventSignature: () => true },
      keyDirectory: {
        async resolve() {
          return { publicKey: '0xaa', publicKeyBundle: null };
        },
      },
    });

    assert.equal(report.promoted, 0);
    assert.equal(outbox.getQuarantinedEvents().length, 1);
    assert.equal(outbox.getPulledEvents().length, 0);
  });

  it('does not promote when key resolution still fails', async () => {
    outbox.storeQuarantinedEvents([sampleEvent()], 'key_unresolved');

    const report = await syncDoctor({
      outbox,
      client: { verifyEventSignature: () => true },
      keyDirectory: {
        async resolve() {
          return { error: 'key_unresolved' };
        },
      },
      promote: true,
    });

    assert.equal(report.promoted, 0);
    assert.equal(outbox.getQuarantinedEvents().length, 1);
  });

  /**
   * Both halves of the report used to run off `getQuarantinedEvents()`, whose
   * default LIMIT is 1000: the counts saturated there and the promote sweep
   * stopped there, with nothing saying either had been truncated.
   */
  describe('beyond one page of quarantine', () => {
    const MANY = 1200;

    function fillQuarantine(reason = 'key_unresolved') {
      const events = [];
      for (let i = 1; i <= MANY; i++) events.push(sampleEvent(i));
      outbox.storeQuarantinedEvents(events, reason);
    }

    it('counts every quarantined event, not the first page of them', async () => {
      fillQuarantine();

      const report = await syncDoctor({ outbox, client: {}, keyDirectory: {} });
      assert.deepEqual(report.quarantined, [{ reason: 'key_unresolved', count: MANY }]);
      assert.equal(report.total, MANY);
    });

    it('promotes every event that now verifies, not the first page of them', async () => {
      fillQuarantine();

      const report = await syncDoctor({
        outbox,
        client: { verifyEventSignature: () => true },
        keyDirectory: {
          async resolve() {
            return { publicKey: '0xaa', publicKeyBundle: null };
          },
        },
        promote: true,
      });

      assert.equal(report.promoted, MANY);
      assert.equal(report.total, 0);
      assert.equal(outbox.getQuarantinedCountsByReason().length, 0);
    });

    it('terminates when nothing can be promoted', async () => {
      fillQuarantine();

      const report = await syncDoctor({
        outbox,
        client: { verifyEventSignature: () => false },
        keyDirectory: {
          async resolve() {
            return { publicKey: '0xaa', publicKeyBundle: null };
          },
        },
        promote: true,
      });

      assert.equal(report.promoted, 0);
      assert.equal(report.total, MANY);
    });

    it('promotes across pages even when some events stay behind', async () => {
      fillQuarantine();
      // Only even sequence numbers verify, so every page mixes both outcomes.
      const report = await syncDoctor({
        outbox,
        client: { verifyEventSignature: (event) => event.sequenceNumber % 2 === 0 },
        keyDirectory: {
          async resolve() {
            return { publicKey: '0xaa', publicKeyBundle: null };
          },
        },
        promote: true,
      });

      assert.equal(report.promoted, MANY / 2);
      assert.equal(report.total, MANY / 2);
      assert.equal(outbox.getPulledEvents(MANY).length, MANY / 2);
    });
  });

  /**
   * After `--promote`, a provably forged event used to still read as
   * `key_unresolved` — the benign-outage diagnosis — in the very report built
   * to surface forgeries. `pull()` re-quarantines with the new reason; doctor
   * must too.
   */
  describe('re-diagnosis', () => {
    it('replaces a stale key_unresolved with signature_invalid', async () => {
      outbox.storeQuarantinedEvents([sampleEvent()], 'key_unresolved');

      const report = await syncDoctor({
        outbox,
        client: { verifyEventSignature: () => false },
        keyDirectory: {
          async resolve() {
            return { publicKey: '0xaa', publicKeyBundle: null };
          },
        },
        promote: true,
      });

      assert.equal(report.promoted, 0);
      assert.equal(report.rediagnosed, 1);
      assert.deepEqual(report.quarantined, [{ reason: 'signature_invalid', count: 1 }]);
      assert.equal(outbox.getQuarantinedEvents()[0].reason, 'signature_invalid');
    });

    it('records a newly named resolution failure', async () => {
      outbox.storeQuarantinedEvents([sampleEvent()], 'key_unresolved');

      const report = await syncDoctor({
        outbox,
        client: { verifyEventSignature: () => true },
        keyDirectory: {
          async resolve() {
            return { error: 'sequencer_key_not_configured' };
          },
        },
        promote: true,
      });

      assert.equal(report.rediagnosed, 1);
      assert.deepEqual(report.quarantined, [{ reason: 'sequencer_key_not_configured', count: 1 }]);
    });

    /**
     * The direction the first round of these tests missed. While the sequencer
     * is unreachable `resolve()` reports `key_unresolved` for every event, so
     * an unconditional rewrite walked every stricter diagnosis down to the
     * benign-outage reason — erasing forgery evidence through the very
     * recovery procedure the docs recommend (`doctor --promote` during an
     * outage, which is when doctor is most likely to be run).
     */
    it('never downgrades a finding about the event to an outage reason', async () => {
      const findings = [
        'signature_invalid',
        'directory_untrusted',
        'peer_key_conflict',
        'key_revoked',
        'key_outside_validity_window',
      ];
      findings.forEach((reason, index) => {
        outbox.storeQuarantinedEvents([sampleEvent(index + 1)], reason);
      });

      const report = await syncDoctor({
        outbox,
        client: { verifyEventSignature: () => true },
        keyDirectory: {
          // The sequencer is unreachable: nothing is learned about any event.
          async resolve() {
            return { error: 'key_unresolved', detail: 'ECONNREFUSED' };
          },
        },
        promote: true,
      });

      assert.equal(report.promoted, 0);
      assert.equal(report.rediagnosed, 0, 'an outage teaches nothing about an event');

      const stored = new Map(
        outbox.getQuarantinedEvents().map((event) => [event.eventId, event.reason]),
      );
      findings.forEach((reason, index) => {
        assert.equal(
          stored.get(sampleEvent(index + 1).eventId),
          reason,
          `${reason} must survive an outage-time doctor --promote`,
        );
      });
      assert.deepEqual(
        report.quarantined.map((entry) => entry.reason).sort(),
        [...findings].sort(),
        'the report must still show what it showed before the sweep',
      );
    });

    it('does not downgrade a finding to sequencer_key_not_configured either', async () => {
      outbox.storeQuarantinedEvents([sampleEvent()], 'signature_invalid');

      const report = await syncDoctor({
        outbox,
        client: { verifyEventSignature: () => true },
        keyDirectory: {
          async resolve() {
            return { error: 'sequencer_key_not_configured' };
          },
        },
        promote: true,
      });

      assert.equal(report.rediagnosed, 0);
      assert.equal(outbox.getQuarantinedEvents()[0].reason, 'signature_invalid');
    });

    it('still records why a key cannot be obtained when that is all we knew', async () => {
      outbox.storeQuarantinedEvents([sampleEvent()], 'key_unresolved');

      const report = await syncDoctor({
        outbox,
        client: { verifyEventSignature: () => true },
        keyDirectory: {
          async resolve() {
            return { error: 'sequencer_key_not_configured' };
          },
        },
        promote: true,
      });

      assert.equal(report.rediagnosed, 1);
      assert.equal(
        outbox.getQuarantinedEvents()[0].reason,
        'sequencer_key_not_configured',
        'one acquisition failure may still sharpen another',
      );
    });

    it('does not count a reason that has not changed', async () => {
      outbox.storeQuarantinedEvents([sampleEvent()], 'signature_invalid');

      const report = await syncDoctor({
        outbox,
        client: { verifyEventSignature: () => false },
        keyDirectory: {
          async resolve() {
            return { publicKey: '0xaa', publicKeyBundle: null };
          },
        },
        promote: true,
      });

      assert.equal(report.rediagnosed, 0);
      assert.deepEqual(report.quarantined, [{ reason: 'signature_invalid', count: 1 }]);
    });
  });
});
