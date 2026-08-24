// A2A service binding + intelligence-service wiring for the MCP orchestrator.
//
// The A2A store exposes ~90 methods; the orchestrator surfaces them to the
// domain tool modules through a single `commerce.a2a()` accessor. The
// accessor is *late-bound*: it starts as a thin pass-through over the store
// and is swapped for the "integrated" service (memory, rules, idempotency,
// tracing, cost analytics, introspection) once those modules finish loading.
//
// Extracted from mcp-server.js (pure move — no behaviour change).

/**
 * Build the late-bound A2A service accessor for one server instance.
 *
 * @param {import('../a2a/store.js').A2AStore} a2aStore
 * @returns {{
 *   a2a: () => object,
 *   setFactory: (factory: () => object) => void,
 *   getFactory: () => (() => object),
 * }} `a2a` is what gets registered on the commerce wrapper; `setFactory`
 *   swaps the underlying factory (used once intelligence services are up).
 */
export function createA2AServiceBinding(a2aStore) {
  let createA2AService = () => ({
    createPayment: (p) => a2aStore.createPayment(p),
    getPayment: (id) => a2aStore.getPayment(id),
    updatePayment: (id, u) => a2aStore.updatePayment(id, u),
    listPayments: (f) => a2aStore.listPayments(f),
    sumPayments: (f) => a2aStore.sumPayments(f),
    summarizePayments: (f) => a2aStore.summarizePayments(f),
    createPaymentRequest: (r) => a2aStore.createPaymentRequest(r),
    getPaymentRequest: (id) => a2aStore.getPaymentRequest(id),
    updatePaymentRequest: (id, u) => a2aStore.updatePaymentRequest(id, u),
    listPaymentRequests: (f) => a2aStore.listPaymentRequests(f),
    createQuote: (q) => a2aStore.createQuote(q),
    getQuote: (id) => a2aStore.getQuote(id),
    updateQuote: (id, u) => a2aStore.updateQuote(id, u),
    listQuotes: (f) => a2aStore.listQuotes(f),
    // Escrow methods
    createEscrow: (e) => a2aStore.createEscrow(e),
    getEscrow: (id) => a2aStore.getEscrow(id),
    updateEscrow: (id, u) => a2aStore.updateEscrow(id, u),
    listEscrows: (f) => a2aStore.listEscrows(f),
    // Dispute methods
    createDispute: (d) => a2aStore.createDispute(d),
    getDispute: (id) => a2aStore.getDispute(id),
    updateDispute: (id, u) => a2aStore.updateDispute(id, u),
    listDisputes: (f) => a2aStore.listDisputes(f),
    createEvidence: (e) => a2aStore.createEvidence(e),
    getEvidence: (id) => a2aStore.getEvidence(id),
    listEvidenceByDispute: (id) => a2aStore.listEvidenceByDispute(id),
    // Feedback / reputation methods
    createFeedback: (f) => a2aStore.createFeedback(f),
    getFeedback: (id) => a2aStore.getFeedback(id),
    updateFeedback: (id, u) => a2aStore.updateFeedback(id, u),
    listFeedback: (f) => a2aStore.listFeedback(f),
    getReputationScore: (addr) => a2aStore.getReputationScore(addr),
    upsertReputationScore: (s) => a2aStore.upsertReputationScore(s),
    // Service methods
    createService: (s) => a2aStore.createService(s),
    getService: (id) => a2aStore.getService(id),
    updateService: (id, u) => a2aStore.updateService(id, u),
    listServices: (f) => a2aStore.listServices(f),
    // Notification log methods
    createNotificationLog: (n) => a2aStore.createNotificationLog(n),
    getNotificationLog: (id) => a2aStore.getNotificationLog(id),
    updateNotificationLog: (id, u) => a2aStore.updateNotificationLog(id, u),
    listNotificationLog: (f) => a2aStore.listNotificationLog(f),
    getPendingNotifications: (max, lim) => a2aStore.getPendingNotifications(max, lim),
    // Webhook config methods
    upsertWebhookConfig: (c) => a2aStore.upsertWebhookConfig(c),
    getWebhookConfig: (addr) => a2aStore.getWebhookConfig(addr),
    listWebhookConfigs: (f) => a2aStore.listWebhookConfigs(f),
    // Subscription methods
    createSubscription: (s) => a2aStore.createSubscription(s),
    getSubscription: (id) => a2aStore.getSubscription(id),
    updateSubscription: (id, u) => a2aStore.updateSubscription(id, u),
    listSubscriptions: (f) => a2aStore.listSubscriptions(f),
    getDueSubscriptions: (now) => a2aStore.getDueSubscriptions(now),
    getExpiredTrials: (now) => a2aStore.getExpiredTrials(now),
    // Split payment methods
    createSplitPayment: (s) => a2aStore.createSplitPayment(s),
    getSplitPayment: (id) => a2aStore.getSplitPayment(id),
    updateSplitPayment: (id, u) => a2aStore.updateSplitPayment(id, u),
    listSplitPayments: (f) => a2aStore.listSplitPayments(f),
    createSplitRecipient: (r) => a2aStore.createSplitRecipient(r),
    getSplitRecipient: (id) => a2aStore.getSplitRecipient(id),
    updateSplitRecipient: (id, u) => a2aStore.updateSplitRecipient(id, u),
    listSplitRecipients: (f) => a2aStore.listSplitRecipients(f),
    // Event subscription methods
    createEventSubscription: (s) => a2aStore.createEventSubscription(s),
    getEventSubscription: (id) => a2aStore.getEventSubscription(id),
    updateEventSubscription: (id, u) => a2aStore.updateEventSubscription(id, u),
    listEventSubscriptions: (f) => a2aStore.listEventSubscriptions(f),
    // Event log methods
    createEventLog: (e) => a2aStore.createEventLog(e),
    getEventLog: (id) => a2aStore.getEventLog(id),
    listEventLog: (f) => a2aStore.listEventLog(f),

    // RFQ methods (marketplace)
    createRFQ: (r) => a2aStore.createRFQ(r),
    getRFQ: (id) => a2aStore.getRFQ(id),
    updateRFQ: (id, u) => a2aStore.updateRFQ(id, u),
    listRFQs: (f) => a2aStore.listRFQs(f),
    createRFQResponse: (r) => a2aStore.createRFQResponse(r),
    getRFQResponse: (id) => a2aStore.getRFQResponse(id),
    updateRFQResponse: (id, u) => a2aStore.updateRFQResponse(id, u),
    listRFQResponses: (f) => a2aStore.listRFQResponses(f),

    // SLA methods
    createSLADefinition: (s) => a2aStore.createSLADefinition(s),
    getSLADefinition: (id) => a2aStore.getSLADefinition(id),
    updateSLADefinition: (id, u) => a2aStore.updateSLADefinition(id, u),
    listSLADefinitions: (f) => a2aStore.listSLADefinitions(f),
    createSLAViolation: (v) => a2aStore.createSLAViolation(v),
    getSLAViolation: (id) => a2aStore.getSLAViolation(id),
    updateSLAViolation: (id, u) => a2aStore.updateSLAViolation(id, u),
    listSLAViolations: (f) => a2aStore.listSLAViolations(f),

    // Workflow methods
    createWorkflow: (w) => a2aStore.createWorkflow(w),
    getWorkflow: (id) => a2aStore.getWorkflow(id),
    updateWorkflow: (id, u) => a2aStore.updateWorkflow(id, u),
    listWorkflows: (f) => a2aStore.listWorkflows(f),
    createWorkflowStep: (s) => a2aStore.createWorkflowStep(s),
    getWorkflowStep: (id) => a2aStore.getWorkflowStep(id),
    updateWorkflowStep: (id, u) => a2aStore.updateWorkflowStep(id, u),
    listWorkflowSteps: (f) => a2aStore.listWorkflowSteps(f),
  });

  return {
    a2a: () => createA2AService(),
    setFactory: (factory) => {
      createA2AService = factory;
    },
    getFactory: () => createA2AService,
  };
}

/**
 * Lazily load the A2A intelligence services and attach them to the commerce
 * wrapper. Modules are dynamically imported so a failure in any of them
 * degrades gracefully instead of blocking server startup.
 *
 * @param {{
 *   commerceWithA2A: object,
 *   a2aStore: import('../a2a/store.js').A2AStore,
 *   setA2AServiceFactory: (factory: () => object) => void,
 * }} deps
 * @returns {Promise<void>} resolves once wiring (or its graceful fallback) is done
 */
export function initializeIntelligenceServices({
  commerceWithA2A,
  a2aStore,
  setA2AServiceFactory,
}) {
  return Promise.all([
    import('../a2a/agent-memory.js'),
    import('../a2a/rules-engine.js'),
    import('../a2a/idempotency.js'),
    import('../a2a/tracing.js'),
    import('../a2a/cost-analytics.js'),
    import('../a2a/introspection.js'),
    import('../a2a/scheduler.js'),
    import('../a2a/messaging.js'),
    import('../a2a/rate-limiter.js'),
    import('../a2a/integration.js'),
  ])
    .then(
      ([
        { createAgentMemory },
        { createRulesEngine },
        { createIdempotencyGuard },
        { createTracingService },
        { createCostAnalytics },
        { createIntrospectionService },
        { createSchedulerService },
        { createMessagingService },
        { createMcpRateLimiter },
        { createIntegratedA2AService },
      ]) => {
        const memory = createAgentMemory();
        const rules = createRulesEngine();
        const idempotency = createIdempotencyGuard({ ttlMs: 86_400_000 });
        const tracing = createTracingService({ maxSpans: 10_000 });
        const costAnalytics = createCostAnalytics();
        const introspection = createIntrospectionService();
        const scheduler = createSchedulerService();
        const messaging = createMessagingService();
        const rateLimiter = createMcpRateLimiter({
          defaultLimits: { requestsPerMinute: 120 },
          toolOverrides: {
            a2a_pay: { requestsPerMinute: 30 },
            a2a_batch_pay: { requestsPerMinute: 10 },
            a2a_scatter: { requestsPerMinute: 20 },
          },
        });

        commerceWithA2A._agentMemory = memory;
        commerceWithA2A._rulesEngine = rules;
        commerceWithA2A._idempotencyGuard = idempotency;
        commerceWithA2A._tracingService = tracing;
        commerceWithA2A._costAnalytics = costAnalytics;
        commerceWithA2A._introspectionService = introspection;
        commerceWithA2A._schedulerService = scheduler;
        commerceWithA2A._messagingService = messaging;
        commerceWithA2A._rateLimiter = rateLimiter;
        commerceWithA2A._store = a2aStore;

        const originalA2A = commerceWithA2A.a2a;
        if (typeof originalA2A === 'function' && typeof createIntegratedA2AService === 'function') {
          const coreA2AInstance = originalA2A();
          const integratedA2A = createIntegratedA2AService(coreA2AInstance, {
            memory,
            rules,
            idempotency,
            tracing,
            costAnalytics,
            introspection,
          });
          setA2AServiceFactory(() => integratedA2A);
        }
      },
    )
    .catch((err) => {
      // Graceful degradation — intelligence services are optional
      console.debug('[mcp-server] Intelligence services init skipped:', err.message);
      commerceWithA2A._store = a2aStore;
    });
}
