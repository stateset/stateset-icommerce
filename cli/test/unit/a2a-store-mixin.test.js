/**
 * Tests for cli/src/a2a/store/mixin.js — applyStoreMixins.
 *
 * The A2A store is decomposed into domain classes whose prototype members are
 * copied onto A2AStore.prototype. These tests pin the descriptor semantics so
 * the decomposition stays behaviourally identical to the original single class.
 */

import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { applyStoreMixins } from '../../src/a2a/store/mixin.js';
import { A2AStore } from '../../src/a2a/store.js';

describe('applyStoreMixins', () => {
  it('copies prototype methods with class-method descriptors', () => {
    class Target {}
    class Source {
      hello() {
        return `hi ${this.name}`;
      }
      get upper() {
        return this.name.toUpperCase();
      }
    }
    const returned = applyStoreMixins(Target, Source);
    assert.equal(returned, Target);

    const desc = Object.getOwnPropertyDescriptor(Target.prototype, 'hello');
    assert.ok(desc);
    assert.equal(desc.enumerable, false);
    assert.equal(desc.writable, true);
    assert.equal(desc.configurable, true);

    const getter = Object.getOwnPropertyDescriptor(Target.prototype, 'upper');
    assert.ok(getter && typeof getter.get === 'function');

    const t = new Target();
    t.name = 'dom';
    assert.equal(t.hello(), 'hi dom');
    assert.equal(t.upper, 'DOM');
  });

  it('never copies the constructor', () => {
    class Target {}
    class Source {
      constructor() {
        this.built = true;
      }
      m() {}
    }
    applyStoreMixins(Target, Source);
    assert.equal(Target.prototype.constructor, Target);
    assert.equal(new Target().built, undefined);
  });

  it('applies sources in order (later sources win)', () => {
    class Target {}
    class A {
      v() {
        return 'a';
      }
    }
    class B {
      v() {
        return 'b';
      }
    }
    applyStoreMixins(Target, A, B);
    assert.equal(new Target().v(), 'b');
  });
});

describe('A2AStore decomposition surface', () => {
  it('exposes every domain method on the prototype as a non-enumerable function', () => {
    const expected = [
      '_migrateQuotes',
      '_migrateEscrows',
      '_migrateAgentCards',
      'createPayment',
      'createPaymentRequest',
      'createQuote',
      'createEscrow',
      'releaseEscrowAtomic',
      'createDispute',
      'createEvidence',
      'createFeedback',
      'upsertReputationScore',
      'createService',
      'createRFQ',
      'createRFQResponse',
      'createNotificationLog',
      'replayDLQEntry',
      'upsertWebhookConfig',
      'createSubscription',
      'createSplitPayment',
      'createSplitRecipient',
      'createEventSubscription',
      'createEventLog',
      'registerAgent',
      'discoverAgents',
      'createSLADefinition',
      'createSLAViolation',
      'createWorkflow',
      'createWorkflowStep',
    ];
    for (const name of expected) {
      const desc = Object.getOwnPropertyDescriptor(A2AStore.prototype, name);
      assert.ok(desc, `missing ${name}`);
      assert.equal(typeof desc.value, 'function', `${name} not a function`);
      assert.equal(desc.enumerable, false, `${name} should be non-enumerable`);
    }
    assert.deepEqual(Object.keys(A2AStore.prototype), []);
  });
});
