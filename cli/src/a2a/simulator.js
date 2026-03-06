/**
 * Agent Playground / Simulation Mode
 *
 * Provides a structured simulation surface for A2A scenarios with:
 * - deterministic, scoped clock control
 * - failure-profile execution
 * - state snapshots for debugging
 */

import { randomBytes } from 'node:crypto';
import { createAgentRuntime } from './agent-runtime.js';
import { DEMO_SCENARIOS, runDemoScenario } from './demo-scenarios.js';

const SUPPLIER_OFFLINE_SCENARIO = 'supplier-goes-offline';
const DEFAULT_START_TIME = '2026-03-06T00:00:00.000Z';

export const SIMULATION_SCENARIOS = [SUPPLIER_OFFLINE_SCENARIO, ...DEMO_SCENARIOS];

function makeWallet() {
  return '0x' + randomBytes(20).toString('hex');
}

function makeSigningKey() {
  return {
    privateKey: randomBytes(32).toString('hex'),
    publicKey: randomBytes(32).toString('hex'),
  };
}

function toJsonValue(value) {
  if (value === undefined) return null;
  if (value === null || typeof value !== 'object') return value;
  try {
    return JSON.parse(JSON.stringify(value));
  } catch {
    return String(value);
  }
}

function normalizeScenarioName(scenario) {
  return typeof scenario === 'string' ? scenario.trim() : '';
}

function normalizeAgentNames(agentNames, fallback) {
  if (!Array.isArray(agentNames) || agentNames.length === 0) {
    return fallback;
  }

  const cleaned = agentNames
    .map((entry) => (typeof entry === 'string' ? entry.trim() : ''))
    .filter(Boolean);

  return cleaned.length > 0 ? cleaned : fallback;
}

function normalizeDate(startTime) {
  const candidate = startTime ? new Date(startTime) : new Date(DEFAULT_START_TIME);
  if (Number.isNaN(candidate.getTime())) {
    throw new Error(`Invalid simulation start time: ${startTime}`);
  }
  return candidate;
}

export async function withSimulatedClock(startTime, fn) {
  const RealDate = Date;
  const startedAt = normalizeDate(startTime);
  let currentMs = startedAt.getTime();
  let advancedMs = 0;

  class SimulatedDate extends RealDate {
    constructor(...args) {
      if (args.length === 0) {
        super(currentMs);
        return;
      }
      super(...args);
    }

    static now() {
      return currentMs;
    }

    static parse(value) {
      return RealDate.parse(value);
    }

    static UTC(...args) {
      return RealDate.UTC(...args);
    }
  }

  const clock = {
    startedAt: new RealDate(startedAt.getTime()),
    now() {
      return new RealDate(currentMs);
    },
    advanceMs(value) {
      const delta = Number(value);
      if (!Number.isFinite(delta) || delta < 0) {
        throw new Error(`advanceMs requires a non-negative number, received ${value}`);
      }
      currentMs += delta;
      advancedMs += delta;
      return this.now();
    },
    advanceSeconds(value) {
      return this.advanceMs(Number(value) * 1000);
    },
    advanceMinutes(value) {
      return this.advanceMs(Number(value) * 60 * 1000);
    },
    advanceHours(value) {
      return this.advanceMs(Number(value) * 60 * 60 * 1000);
    },
    elapsedMs() {
      return currentMs - startedAt.getTime();
    },
    advancedMs() {
      return advancedMs;
    },
  };

  globalThis.Date = SimulatedDate;
  try {
    return await fn(clock);
  } finally {
    globalThis.Date = RealDate;
  }
}

function normalizeRuntimeEntry(entry) {
  if (!entry) return null;
  if (entry.runtime) return entry;
  return { runtime: entry, role: null };
}

function safeInvoke(fn, fallback) {
  try {
    return fn();
  } catch {
    return fallback;
  }
}

export function captureSimulationSnapshot({ store, runtimes = [], label, metadata = {}, clock }) {
  if (!store) {
    throw new Error('store is required to capture a simulation snapshot');
  }

  const runtimeEntries = runtimes
    .map(normalizeRuntimeEntry)
    .filter(Boolean)
    .map(({ runtime, role }) => ({
      role: role || null,
      name: runtime.name,
      agentId: runtime.agentId,
      walletAddress: runtime.walletAddress,
      running: safeInvoke(() => runtime.isRunning(), false),
      budget: safeInvoke(() => runtime.getBudget(), null),
      settlement: runtime.settlement
        ? {
            chainId: runtime.settlement.chainId,
            simulate: runtime.settlement.isSimulation,
          }
        : null,
    }));

  return {
    label,
    capturedAt: new Date().toISOString(),
    simulatedTime: {
      now: (clock?.now?.() || new Date()).toISOString(),
      elapsedMs: clock?.elapsedMs?.() ?? 0,
      advancedMs: clock?.advancedMs?.() ?? 0,
    },
    metadata: toJsonValue(metadata),
    agents: runtimeEntries,
    services: safeInvoke(() => store.listServices({ active: undefined, limit: 200 }), []),
    quotes: safeInvoke(() => store.listQuotes({ include_expired: true, limit: 200 }), []),
    payments: safeInvoke(() => store.listPayments({ limit: 200 }), []),
    escrows: safeInvoke(() => store.listEscrows({ limit: 200 }), []),
    subscriptions: safeInvoke(() => store.listSubscriptions({ limit: 200 }), []),
    rfqs: safeInvoke(() => store.listRFQs({ limit: 200 }), []),
    rfqResponses: safeInvoke(() => store.listRFQResponses({}), []),
    workflows: safeInvoke(() => store.listWorkflows({ limit: 200 }), []),
    events: {
      count: safeInvoke(() => store.listEventLog({ limit: 1000 }).length, 0),
      recent: safeInvoke(() => store.listEventLog({ limit: 20 }), []),
    },
  };
}

async function runSupplierGoesOfflineSimulation({
  store,
  commerce,
  clock,
  agentNames,
  advanceHours = 2,
  deadlineMinutes = 60,
  captureSnapshots = true,
  log = () => {},
}) {
  if (!store) throw new Error('store is required');
  if (!commerce) throw new Error('commerce is required');

  const [supplierName, buyerName] = normalizeAgentNames(agentNames, ['inventory', 'procurement']);
  const snapshots = [];
  const steps = [];
  const runtimeEntries = [];

  const supplier = createAgentRuntime({
    name: supplierName,
    walletAddress: makeWallet(),
    signingKey: makeSigningKey(),
    commerce,
    budget: { daily: 10_000, perTransaction: 2_500 },
    autoRegisterCard: true,
    agentDescription: 'Simulated supplier runtime for inventory replenishment.',
    agentSkills: ['supply', 'inventory', 'quote'],
    logger: () => {},
  });
  const procurement = createAgentRuntime({
    name: buyerName,
    walletAddress: makeWallet(),
    signingKey: makeSigningKey(),
    commerce,
    budget: { daily: 10_000, perTransaction: 2_500 },
    autoRegisterCard: true,
    agentDescription: 'Simulated procurement runtime for RFQ fallback testing.',
    agentSkills: ['procurement', 'buy', 'rfq'],
    logger: () => {},
  });

  runtimeEntries.push({ role: 'supplier', runtime: supplier });
  runtimeEntries.push({ role: 'buyer', runtime: procurement });

  const maybeCaptureSnapshot = (label, metadata = {}) => {
    if (!captureSnapshots) return null;
    const snapshot = captureSimulationSnapshot({
      store,
      runtimes: runtimeEntries,
      label,
      metadata,
      clock,
    });
    snapshots.push(snapshot);
    return snapshot;
  };

  const recordStep = async (name, action, snapshotLabel) => {
    const startedAt = new Date().toISOString();
    try {
      const result = await action();
      const entry = {
        name,
        status: 'completed',
        startedAt,
        completedAt: new Date().toISOString(),
        result: toJsonValue(result),
      };
      if (snapshotLabel) {
        const snapshot = maybeCaptureSnapshot(snapshotLabel, { step: name });
        if (snapshot) {
          entry.snapshotLabel = snapshot.label;
        }
      }
      steps.push(entry);
      return result;
    } catch (error) {
      steps.push({
        name,
        status: 'failed',
        startedAt,
        completedAt: new Date().toISOString(),
        error: error.message,
      });
      throw error;
    }
  };

  let registeredService = null;
  let rfqResult = null;
  let responseCollection = null;

  try {
    maybeCaptureSnapshot('initial', { scenario: SUPPLIER_OFFLINE_SCENARIO });

    registeredService = await recordStep(
      'register-supplier-service',
      () =>
        supplier.registerService({
          name: `${supplierName} replenishment`,
          category: 'inventory',
          description: 'Warehouse replenishment and restock quoting.',
          pricingModel: 'quote',
          pricingDetails: { basePrice: 480, unitPrice: 2.4 },
        }),
      'service-registered',
    );
    log(`[simulate] registered supplier service ${registeredService.id}`);

    rfqResult = await recordStep(
      'broadcast-rfq',
      () =>
        procurement.broadcastRFQ({
          items: [
            {
              description: 'Restock SKU-RED-42',
              sku: 'SKU-RED-42',
              quantity: 200,
              unitPrice: 2.4,
            },
          ],
          sellerFilter: 'inventory',
          deadlineMinutes,
          scoringCriteria: 'fastest',
        }),
      'rfq-broadcast',
    );
    log(`[simulate] RFQ ${rfqResult.rfq.id} contacted ${rfqResult.sellersContacted} supplier(s)`);

    await recordStep(
      'inject-supplier-offline',
      () => {
        const updatedService = store.updateService(registeredService.id, {
          active: 0,
        });
        return {
          serviceId: updatedService.id,
          active: Boolean(updatedService.active),
          reason: 'supplier intentionally taken offline',
        };
      },
      'supplier-offline',
    );
    log(`[simulate] supplier ${supplier.name} marked offline`);

    await recordStep(
      'advance-simulated-time',
      () => ({
        advancedTo: clock.advanceHours(advanceHours).toISOString(),
        advancedHours: advanceHours,
      }),
      'time-advanced',
    );

    await recordStep('expire-rfq-window', () => procurement.tick(), 'rfq-window-expired');

    responseCollection = await recordStep(
      'collect-rfq-responses',
      () => procurement.collectRFQResponses(rfqResult.rfq.id),
      'responses-collected',
    );

    const finalRfq = store.getRFQ(rfqResult.rfq.id);
    const response = rfqResult.responses[0] || null;
    const finalQuote = response?.quote_id ? store.getQuote(response.quote_id) : null;
    const finalService = store.getService(registeredService.id);
    const fallbackRecommended =
      finalRfq?.status === 'expired' && (responseCollection?.scoredCount || 0) === 0;

    maybeCaptureSnapshot('final', { scenario: SUPPLIER_OFFLINE_SCENARIO });

    return {
      success: true,
      scenario: SUPPLIER_OFFLINE_SCENARIO,
      failureProfile: {
        name: SUPPLIER_OFFLINE_SCENARIO,
        targetAgent: supplier.name,
      },
      agents: {
        supplier: {
          name: supplier.name,
          agentId: supplier.agentId,
          walletAddress: supplier.walletAddress,
        },
        buyer: {
          name: procurement.name,
          agentId: procurement.agentId,
          walletAddress: procurement.walletAddress,
        },
      },
      simulatedTime: {
        startedAt: clock.startedAt.toISOString(),
        endedAt: clock.now().toISOString(),
        elapsedMs: clock.elapsedMs(),
        advancedMs: clock.advancedMs(),
      },
      outcome: {
        serviceId: registeredService.id,
        rfqId: rfqResult.rfq.id,
        requestedQuoteId: response?.quote_id || null,
        contactedSuppliers: rfqResult.sellersContacted,
        finalRfqStatus: finalRfq?.status || null,
        finalQuoteStatus: finalQuote?.status || null,
        supplierActive: Boolean(finalService?.active),
        scoredResponses: responseCollection?.scoredCount || 0,
        fallbackRecommended,
      },
      steps,
      snapshots,
    };
  } finally {
    supplier.destroy();
    procurement.destroy();
  }
}

async function runDemoScenarioSimulation({
  scenario,
  store,
  commerce,
  clock,
  settlement = null,
  captureSnapshots = true,
  log = () => {},
}) {
  const snapshots = [];

  if (captureSnapshots) {
    snapshots.push(
      captureSimulationSnapshot({
        store,
        label: 'initial',
        metadata: { scenario },
        clock,
      }),
    );
  }

  const startedAt = new Date().toISOString();
  const result = await runDemoScenario(scenario, commerce, { log, settlement });

  if (captureSnapshots) {
    snapshots.push(
      captureSimulationSnapshot({
        store,
        label: 'final',
        metadata: { scenario },
        clock,
      }),
    );
  }

  return {
    success: true,
    scenario,
    simulatedTime: {
      startedAt,
      endedAt: new Date().toISOString(),
      elapsedMs: clock.elapsedMs(),
      advancedMs: clock.advancedMs(),
    },
    steps: [
      {
        name: 'run-demo-scenario',
        status: 'completed',
        startedAt,
        completedAt: new Date().toISOString(),
      },
    ],
    outcome: toJsonValue(result),
    snapshots,
  };
}

export async function runSimulationScenario({
  scenario,
  store,
  commerce,
  startTime,
  agentNames,
  advanceHours,
  deadlineMinutes,
  captureSnapshots = true,
  settlement = null,
  log = () => {},
}) {
  const normalizedScenario = normalizeScenarioName(scenario);
  if (!SIMULATION_SCENARIOS.includes(normalizedScenario)) {
    throw new Error(
      `Unknown simulation scenario: ${normalizedScenario}. Available: ${SIMULATION_SCENARIOS.join(', ')}`,
    );
  }
  if (!store) throw new Error('store is required');
  if (!commerce) throw new Error('commerce is required');

  return withSimulatedClock(startTime, async (clock) => {
    if (normalizedScenario === SUPPLIER_OFFLINE_SCENARIO) {
      return runSupplierGoesOfflineSimulation({
        store,
        commerce,
        clock,
        agentNames,
        advanceHours,
        deadlineMinutes,
        captureSnapshots,
        log,
      });
    }

    return runDemoScenarioSimulation({
      scenario: normalizedScenario,
      store,
      commerce,
      clock,
      settlement,
      captureSnapshots,
      log,
    });
  });
}
