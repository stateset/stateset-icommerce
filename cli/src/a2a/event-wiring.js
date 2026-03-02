/**
 * Event Wiring — Bridges agent runtime events to A2A event stream & EventBridge
 *
 * Maps internal runtime EventEmitter events to persistent A2A event stream
 * events and to the autonomous EventBridge for channel notifications.
 *
 * @example
 * ```javascript
 * import { createAgentRuntime, makeCommerceProxy } from './agent-runtime.js';
 * import { createEventStreamService } from './event-stream.js';
 * import { wireRuntimeEvents } from './event-wiring.js';
 *
 * const runtime = createAgentRuntime({ ... });
 * const eventStream = createEventStreamService(store);
 *
 * const { unwire } = wireRuntimeEvents(runtime, eventStream);
 * // All runtime events now flow to the event stream
 * ```
 */

/**
 * Map of runtime event names to their event stream and EventBridge types.
 * @type {Array<{ runtimeEvent: string, streamType: string, bridgeType: string }>}
 */
const EVENT_MAP = [
  {
    runtimeEvent: 'quote:received',
    streamType: 'a2a_runtime.quote_received',
    bridgeType: 'agent.quote.received',
  },
  {
    runtimeEvent: 'quote:provided',
    streamType: 'a2a_runtime.quote_provided',
    bridgeType: 'agent.quote.provided',
  },
  {
    runtimeEvent: 'quote:accepted',
    streamType: 'a2a_runtime.quote_accepted',
    bridgeType: 'agent.quote.accepted',
  },
  {
    runtimeEvent: 'quote:countered',
    streamType: 'a2a_runtime.quote_countered',
    bridgeType: 'agent.quote.countered',
  },
  {
    runtimeEvent: 'quote:declined',
    streamType: 'a2a_runtime.quote_declined',
    bridgeType: 'agent.quote.declined',
  },
  {
    runtimeEvent: 'payment:sent',
    streamType: 'a2a_runtime.payment_sent',
    bridgeType: 'agent.payment.sent',
  },
  {
    runtimeEvent: 'escrow:created',
    streamType: 'a2a_runtime.escrow_created',
    bridgeType: 'agent.escrow.created',
  },
  {
    runtimeEvent: 'escrow:settled',
    streamType: 'a2a_runtime.escrow_settled',
    bridgeType: 'agent.escrow.settled',
  },
  {
    runtimeEvent: 'subscription:created',
    streamType: 'a2a_runtime.subscription_created',
    bridgeType: 'agent.subscription.created',
  },
  {
    runtimeEvent: 'subscription:paused',
    streamType: 'a2a_runtime.subscription_paused',
    bridgeType: 'agent.subscription.paused',
  },
  {
    runtimeEvent: 'subscription:cancelled',
    streamType: 'a2a_runtime.subscription_cancelled',
    bridgeType: 'agent.subscription.cancelled',
  },
  {
    runtimeEvent: 'subscription:billed',
    streamType: 'a2a_runtime.subscription_billed',
    bridgeType: 'agent.subscription.billed',
  },
  {
    runtimeEvent: 'split:created',
    streamType: 'a2a_runtime.split_created',
    bridgeType: 'agent.split.created',
  },
  {
    runtimeEvent: 'split:executed',
    streamType: 'a2a_runtime.split_executed',
    bridgeType: 'agent.split.executed',
  },
  {
    runtimeEvent: 'reputation:rated',
    streamType: 'a2a_runtime.reputation_rated',
    bridgeType: 'agent.reputation.rated',
  },
  {
    runtimeEvent: 'budget:warning',
    streamType: 'a2a_runtime.budget_warning',
    bridgeType: 'agent.budget.warning',
  },
  {
    runtimeEvent: 'service:registered',
    streamType: 'a2a_runtime.service_registered',
    bridgeType: 'agent.service.registered',
  },
  {
    runtimeEvent: 'service:fulfilled',
    streamType: 'a2a_runtime.service_fulfilled',
    bridgeType: 'agent.service.fulfilled',
  },
  {
    runtimeEvent: 'card:registered',
    streamType: 'a2a_runtime.card_registered',
    bridgeType: 'agent.card.registered',
  },
  {
    runtimeEvent: 'card:suspended',
    streamType: 'a2a_runtime.card_suspended',
    bridgeType: 'agent.card.suspended',
  },
  // Marketplace RFQ
  {
    runtimeEvent: 'rfq:broadcast',
    streamType: 'a2a_runtime.rfq_broadcast',
    bridgeType: 'agent.rfq.broadcast',
  },
  {
    runtimeEvent: 'rfq:response',
    streamType: 'a2a_runtime.rfq_response',
    bridgeType: 'agent.rfq.response',
  },
  {
    runtimeEvent: 'rfq:awarded',
    streamType: 'a2a_runtime.rfq_awarded',
    bridgeType: 'agent.rfq.awarded',
  },
  {
    runtimeEvent: 'rfq:expired',
    streamType: 'a2a_runtime.rfq_expired',
    bridgeType: 'agent.rfq.expired',
  },
  // SLA
  {
    runtimeEvent: 'sla:attached',
    streamType: 'a2a_runtime.sla_attached',
    bridgeType: 'agent.sla.attached',
  },
  {
    runtimeEvent: 'sla:breach',
    streamType: 'a2a_runtime.sla_breach',
    bridgeType: 'agent.sla.breach',
  },
  // Workflow
  {
    runtimeEvent: 'workflow:started',
    streamType: 'a2a_runtime.workflow_started',
    bridgeType: 'agent.workflow.started',
  },
  {
    runtimeEvent: 'workflow:completed',
    streamType: 'a2a_runtime.workflow_completed',
    bridgeType: 'agent.workflow.completed',
  },
  // On-chain settlement
  {
    runtimeEvent: 'settlement:pending',
    streamType: 'a2a_runtime.settlement_pending',
    bridgeType: 'agent.settlement.pending',
  },
  {
    runtimeEvent: 'settlement:confirmed',
    streamType: 'a2a_runtime.settlement_confirmed',
    bridgeType: 'agent.settlement.confirmed',
  },
  {
    runtimeEvent: 'settlement:failed',
    streamType: 'a2a_runtime.settlement_failed',
    bridgeType: 'agent.settlement.failed',
  },
  {
    runtimeEvent: 'settlement:insufficient_funds',
    streamType: 'a2a_runtime.settlement_insufficient_funds',
    bridgeType: 'agent.settlement.insufficient_funds',
  },
  // Circuit Breaker
  {
    runtimeEvent: 'circuit:tripped',
    streamType: 'a2a_runtime.circuit_tripped',
    bridgeType: 'agent.circuit.tripped',
  },
  {
    runtimeEvent: 'circuit:reset',
    streamType: 'a2a_runtime.circuit_reset',
    bridgeType: 'agent.circuit.reset',
  },
  {
    runtimeEvent: 'circuit:blocked',
    streamType: 'a2a_runtime.circuit_blocked',
    bridgeType: 'agent.circuit.blocked',
  },
  {
    runtimeEvent: 'circuit:kill_switch',
    streamType: 'a2a_runtime.circuit_kill_switch',
    bridgeType: 'agent.circuit.kill_switch',
  },
];

/**
 * Wire a runtime's events to an A2A event stream and/or EventBridge.
 *
 * @param {Object} runtime - Agent runtime from createAgentRuntime()
 * @param {Object} [eventStream] - Event stream service (pushEvent method)
 * @param {Object} [eventBridge] - EventBridge instance (sendCommerceEvent method)
 * @returns {{ unwire: Function, eventMap: typeof EVENT_MAP }}
 */
export function wireRuntimeEvents(runtime, eventStream, eventBridge) {
  if (!runtime) throw new Error('runtime is required');

  const handlers = [];

  for (const mapping of EVENT_MAP) {
    const handler = (data) => {
      const payload = {
        agentId: runtime.agentId,
        agentName: runtime.name,
        walletAddress: runtime.walletAddress,
        timestamp: new Date().toISOString(),
        ...data,
      };

      // Push to A2A event stream (persistent log + SSE)
      if (eventStream?.pushEvent) {
        try {
          eventStream.pushEvent({
            eventType: mapping.streamType,
            agentAddress: runtime.walletAddress,
            payload,
          });
        } catch (err) {
          console.debug(
            `[event-wiring] Stream push failed for ${mapping.streamType}: ${err.message}`,
          );
        }
      }

      // Forward to EventBridge (channel notifications)
      if (eventBridge?.sendCommerceEvent) {
        try {
          eventBridge.sendCommerceEvent(mapping.bridgeType, payload);
        } catch (err) {
          console.debug(
            `[event-wiring] Bridge send failed for ${mapping.bridgeType}: ${err.message}`,
          );
        }
      }
    };

    runtime.on(mapping.runtimeEvent, handler);
    handlers.push({ event: mapping.runtimeEvent, handler });
  }

  function unwire() {
    for (const { event, handler } of handlers) {
      runtime.off(event, handler);
    }
    handlers.length = 0;
  }

  return { unwire, eventMap: EVENT_MAP };
}

/**
 * Create an agent runtime with events already wired to stream/bridge.
 *
 * @param {Object} runtimeParams - Parameters for createAgentRuntime()
 * @param {Object} [eventStream] - Event stream service
 * @param {Object} [eventBridge] - EventBridge instance
 * @param {Function} [createFn] - Runtime factory (default: lazy import)
 * @returns {Promise<{ runtime: Object, unwire: Function }>}
 */
export async function createWiredAgentRuntime(runtimeParams, eventStream, eventBridge, createFn) {
  if (!createFn) {
    const mod = await import('./agent-runtime.js');
    createFn = mod.createAgentRuntime;
  }

  const runtime = createFn(runtimeParams);
  const { unwire } = wireRuntimeEvents(runtime, eventStream, eventBridge);

  // Override destroy to also unwire
  const originalDestroy = runtime.destroy;
  runtime.destroy = () => {
    unwire();
    originalDestroy();
  };

  return { runtime, unwire };
}

export { EVENT_MAP };
