import { describe, it, beforeEach, mock } from 'node:test';
import assert from 'node:assert/strict';
import {
  wireRuntimeEvents,
  createWiredAgentRuntime,
  EVENT_MAP,
} from '../../src/a2a/event-wiring.js';
import { EventEmitter } from 'node:events';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function makeFakeRuntime(overrides = {}) {
  const emitter = new EventEmitter();
  return {
    agentId: 'agent-001',
    name: 'TestAgent',
    walletAddress: '0xTestWallet',
    on: emitter.on.bind(emitter),
    off: emitter.off.bind(emitter),
    emit: emitter.emit.bind(emitter),
    destroy: mock.fn(),
    _emitter: emitter,
    ...overrides,
  };
}

function makeFakeEventStream() {
  return {
    pushEvent: mock.fn(),
  };
}

function makeFakeEventBridge() {
  return {
    sendCommerceEvent: mock.fn(),
  };
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

describe('event-wiring', () => {
  describe('EVENT_MAP', () => {
    it('exports an array of event mappings', () => {
      assert.ok(Array.isArray(EVENT_MAP));
      assert.ok(EVENT_MAP.length >= 16);
    });

    it('each mapping has runtimeEvent, streamType, bridgeType', () => {
      for (const mapping of EVENT_MAP) {
        assert.ok(mapping.runtimeEvent, `Missing runtimeEvent in mapping`);
        assert.ok(mapping.streamType, `Missing streamType for ${mapping.runtimeEvent}`);
        assert.ok(mapping.bridgeType, `Missing bridgeType for ${mapping.runtimeEvent}`);
      }
    });

    it('streamType starts with a2a_runtime.', () => {
      for (const mapping of EVENT_MAP) {
        assert.ok(
          mapping.streamType.startsWith('a2a_runtime.'),
          `${mapping.streamType} should start with a2a_runtime.`,
        );
      }
    });

    it('bridgeType starts with agent.', () => {
      for (const mapping of EVENT_MAP) {
        assert.ok(
          mapping.bridgeType.startsWith('agent.'),
          `${mapping.bridgeType} should start with agent.`,
        );
      }
    });

    it('has no duplicate runtimeEvent entries', () => {
      const seen = new Set();
      for (const mapping of EVENT_MAP) {
        assert.ok(!seen.has(mapping.runtimeEvent), `Duplicate: ${mapping.runtimeEvent}`);
        seen.add(mapping.runtimeEvent);
      }
    });

    it('includes budget:exceeded wiring', () => {
      const mapping = EVENT_MAP.find((entry) => entry.runtimeEvent === 'budget:exceeded');
      assert.deepStrictEqual(mapping, {
        runtimeEvent: 'budget:exceeded',
        streamType: 'a2a_runtime.budget_exceeded',
        bridgeType: 'agent.budget.exceeded',
      });
    });
  });

  describe('wireRuntimeEvents', () => {
    let runtime, stream, bridge;

    beforeEach(() => {
      runtime = makeFakeRuntime();
      stream = makeFakeEventStream();
      bridge = makeFakeEventBridge();
    });

    it('throws if runtime is null', () => {
      assert.throws(() => wireRuntimeEvents(null, stream, bridge), /runtime is required/);
    });

    it('returns unwire function and eventMap', () => {
      const result = wireRuntimeEvents(runtime, stream, bridge);
      assert.ok(typeof result.unwire === 'function');
      assert.deepStrictEqual(result.eventMap, EVENT_MAP);
    });

    it('pushes events to stream when runtime emits', () => {
      wireRuntimeEvents(runtime, stream, bridge);
      runtime.emit('quote:accepted', { quote: { id: 'q1' }, payment: { id: 'p1' } });

      assert.strictEqual(stream.pushEvent.mock.callCount(), 1);
      const call = stream.pushEvent.mock.calls[0].arguments[0];
      assert.strictEqual(call.eventType, 'a2a_runtime.quote_accepted');
      assert.strictEqual(call.agentAddress, '0xTestWallet');
      assert.strictEqual(call.payload.agentId, 'agent-001');
      assert.strictEqual(call.payload.agentName, 'TestAgent');
      assert.ok(call.payload.timestamp);
    });

    it('sends events to bridge when runtime emits', () => {
      wireRuntimeEvents(runtime, stream, bridge);
      runtime.emit('payment:sent', { payment: { id: 'p1' } });

      assert.strictEqual(bridge.sendCommerceEvent.mock.callCount(), 1);
      const args = bridge.sendCommerceEvent.mock.calls[0].arguments;
      assert.strictEqual(args[0], 'agent.payment.sent');
      assert.strictEqual(args[1].walletAddress, '0xTestWallet');
    });

    it('handles stream-only mode (no bridge)', () => {
      wireRuntimeEvents(runtime, stream, null);
      runtime.emit('escrow:created', { escrow: { id: 'e1' }, amount: 100 });

      assert.strictEqual(stream.pushEvent.mock.callCount(), 1);
    });

    it('handles bridge-only mode (no stream)', () => {
      wireRuntimeEvents(runtime, null, bridge);
      runtime.emit('escrow:settled', { escrowId: 'e1' });

      assert.strictEqual(bridge.sendCommerceEvent.mock.callCount(), 1);
    });

    it('handles neither stream nor bridge gracefully', () => {
      wireRuntimeEvents(runtime, null, null);
      // Should not throw
      runtime.emit('split:created', { splitPayment: {} });
    });

    it('unwire removes all listeners', () => {
      const { unwire } = wireRuntimeEvents(runtime, stream, bridge);
      unwire();

      runtime.emit('quote:accepted', { quote: { id: 'q2' } });
      assert.strictEqual(stream.pushEvent.mock.callCount(), 0);
      assert.strictEqual(bridge.sendCommerceEvent.mock.callCount(), 0);
    });

    it('unwire can be called multiple times safely', () => {
      const { unwire } = wireRuntimeEvents(runtime, stream, bridge);
      unwire();
      unwire(); // Should not throw
    });

    it('maps all advertised event types correctly', () => {
      wireRuntimeEvents(runtime, stream, bridge);

      for (const mapping of EVENT_MAP) {
        stream.pushEvent.mock.resetCalls();
        bridge.sendCommerceEvent.mock.resetCalls();

        runtime.emit(mapping.runtimeEvent, { test: true });

        assert.strictEqual(
          stream.pushEvent.mock.callCount(),
          1,
          `Stream not called for ${mapping.runtimeEvent}`,
        );
        assert.strictEqual(
          bridge.sendCommerceEvent.mock.callCount(),
          1,
          `Bridge not called for ${mapping.runtimeEvent}`,
        );

        const streamCall = stream.pushEvent.mock.calls[0].arguments[0];
        assert.strictEqual(streamCall.eventType, mapping.streamType);

        const bridgeArgs = bridge.sendCommerceEvent.mock.calls[0].arguments;
        assert.strictEqual(bridgeArgs[0], mapping.bridgeType);
      }
    });

    it('includes runtime metadata in event payloads', () => {
      wireRuntimeEvents(runtime, stream, bridge);
      runtime.emit('budget:warning', { type: 'daily', spent: 400, limit: 500 });

      const payload = stream.pushEvent.mock.calls[0].arguments[0].payload;
      assert.strictEqual(payload.agentId, 'agent-001');
      assert.strictEqual(payload.agentName, 'TestAgent');
      assert.strictEqual(payload.walletAddress, '0xTestWallet');
      assert.strictEqual(payload.type, 'daily');
      assert.strictEqual(payload.spent, 400);
    });

    it('preserves budget:exceeded rail metadata in stream and bridge payloads', () => {
      wireRuntimeEvents(runtime, stream, bridge);
      runtime.emit('budget:exceeded', {
        type: 'balance',
        asset: 'ZEC',
        network: 'zcash',
        limit: 1.25,
        attempted: 2,
        remaining: 1.25,
        operation: 'subscription:create',
      });

      const streamPayload = stream.pushEvent.mock.calls[0].arguments[0].payload;
      assert.strictEqual(streamPayload.type, 'balance');
      assert.strictEqual(streamPayload.asset, 'ZEC');
      assert.strictEqual(streamPayload.network, 'zcash');
      assert.strictEqual(streamPayload.limit, 1.25);
      assert.strictEqual(streamPayload.operation, 'subscription:create');

      const bridgePayload = bridge.sendCommerceEvent.mock.calls[0].arguments[1];
      assert.strictEqual(bridgePayload.asset, 'ZEC');
      assert.strictEqual(bridgePayload.network, 'zcash');
      assert.strictEqual(bridgePayload.attempted, 2);
    });

    it('survives stream.pushEvent throwing', () => {
      stream.pushEvent = mock.fn(() => {
        throw new Error('stream error');
      });
      wireRuntimeEvents(runtime, stream, bridge);

      // Should not throw
      runtime.emit('reputation:rated', { ratedAddress: '0x...', score: 5 });

      // Bridge should still be called
      assert.strictEqual(bridge.sendCommerceEvent.mock.callCount(), 1);
    });

    it('survives bridge.sendCommerceEvent throwing', () => {
      bridge.sendCommerceEvent = mock.fn(() => {
        throw new Error('bridge error');
      });
      wireRuntimeEvents(runtime, stream, bridge);

      // Should not throw
      runtime.emit('service:registered', { service: {} });

      // Stream should still be called
      assert.strictEqual(stream.pushEvent.mock.callCount(), 1);
    });
  });

  describe('createWiredAgentRuntime', () => {
    it('creates runtime with events wired', async () => {
      const stream = makeFakeEventStream();
      const bridge = makeFakeEventBridge();

      const fakeCalls = [];
      const fakeCreateFn = (params) => {
        fakeCalls.push(params);
        return makeFakeRuntime({ name: params.name });
      };

      const { runtime, unwire } = await createWiredAgentRuntime(
        {
          name: 'TestBot',
          walletAddress: '0xTest',
          signingKey: { privateKey: 'a', publicKey: 'b' },
          commerce: {},
        },
        stream,
        bridge,
        fakeCreateFn,
      );

      assert.strictEqual(runtime.name, 'TestBot');
      assert.strictEqual(fakeCalls.length, 1);

      runtime.emit('payment:sent', { payment: {} });
      assert.strictEqual(stream.pushEvent.mock.callCount(), 1);

      unwire();
      runtime.emit('payment:sent', { payment: {} });
      assert.strictEqual(stream.pushEvent.mock.callCount(), 1); // No new call
    });

    it('destroy also unwires', async () => {
      const stream = makeFakeEventStream();
      const fakeRuntime = makeFakeRuntime();
      const fakeCreateFn = () => fakeRuntime;

      const { runtime } = await createWiredAgentRuntime(
        { name: 'Bot', walletAddress: '0x1', signingKey: {}, commerce: {} },
        stream,
        null,
        fakeCreateFn,
      );

      runtime.emit('quote:accepted', { quote: {} });
      assert.strictEqual(stream.pushEvent.mock.callCount(), 1);

      runtime.destroy();

      stream.pushEvent.mock.resetCalls();
      runtime.emit('quote:accepted', { quote: {} });
      assert.strictEqual(stream.pushEvent.mock.callCount(), 0);
    });
  });
});
