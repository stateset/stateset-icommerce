import {
  runCommerceSimulation,
  compareCommerceSimulations,
  commerceScenarioSchema,
  commerceComparisonSchema,
  runInventorySimulation,
  compareInventorySimulations,
  inventoryPlanSchema,
  inventoryPlanComparisonSchema,
  getSimulationKernelContract,
  inventoryExperimentSchema,
  runInventoryExperiment,
} from '../simulation/index.js';

export const simulationTools = [
  {
    name: 'simulate_inventory_experiment',
    description:
      'Evaluate one to three inventory policies across shared seeds through independent native embedded stores. Demand may be sampled or an explicit daily forecast schedule. Supplier lead times and first fills may be fixed, sampled or explicit dailyValues schedules indexed by order-placement days 0..days. Supports split deliveries, purchasing constraints, per-SKU reviewIntervalDays and a shared cashReserve spending policy. Reviews start on day zero; off-review low-stock attempts defer without native POs or spending. Reports deferredPurchaseReviews and reviewDeferredRuns. Returns empirical percentiles, exact money means and unit fill rates, paired deltas, global and per-policy comparison conditions, path-diversity counts, daily cash and per-SKU inventory flows, first stockouts, minimum cash and reserve-blocked counts. Independent replay checks receipt timing and physical availability before sales as well as final balances. Each comparisonToBaseline lists changedConditions for capital, stock, prices, priority, reserve, horizon and realized paths. Optional per-seed goals include minCashBalance and productFillRates: unique {sku,minFillRateBps} targets present in every policy. Reports expose per-target checks and pooled service; all targets must pass on the same seed, and overall fill cannot hide a critical SKU miss. Zero-demand targets are not evaluable. At most 12 runs and 180 total product-days. Path matches do not establish equivalent assumptions, independence or confidence bounds. No live-store reads, real purchases or external services.',
    inputSchema: {
      experiment: inventoryExperimentSchema.describe(
        'Named policies without per-plan seeds; supply two to eight distinct shared seeds and optional inclusive per-run goals. Aggregate work limits apply.',
      ),
    },
    permission: 'read',
    policyDomain: 'simulations',
    handler: async ({ params }) => runInventoryExperiment(params.experiment),
  },
  {
    name: 'get_simulation_kernel_contract',
    description:
      'Discover the embedded commerce simulation kernel: typed action and planning schemas, execution limits, observations, failure and time semantics, and the model iteration workflow. Pure discovery; no live store or worker access.',
    inputSchema: {},
    permission: 'read',
    policyDomain: 'simulations',
    handler: async () => getSimulationKernelContract(),
  },
  {
    name: 'simulate_commerce_scenario',
    description:
      'Execute a bounded synthetic commerce episode using the real embedded engine in a fresh private store. Typed purchases, receipts, orders, shipment stock postings, captures, refunds, retries and state inspection. No live-store access or external payments. Discover schemas with get_simulation_kernel_contract.',
    inputSchema: {
      scenario: commerceScenarioSchema.describe(
        'Synthetic initial inventory and up to 100 typed commerce actions; no database paths or arbitrary code',
      ),
    },
    permission: 'read',
    policyDomain: 'simulations',
    handler: async ({ params }) => runCommerceSimulation(params.scenario),
  },
  {
    name: 'compare_commerce_scenarios',
    description:
      'Compare two or three bounded what-if commerce scenarios in separate private native stores. All scenarios use one currency and at most 100 total steps. The live store and external services are never used.',
    inputSchema: {
      scenarios: commerceComparisonSchema.describe(
        'Distinct named scenarios with the same currency; outcomes are conditional simulations, not predictions',
      ),
    },
    permission: 'read',
    policyDomain: 'simulations',
    handler: async ({ params }) => compareCommerceSimulations(params.scenarios),
  },
  {
    name: 'simulate_inventory_plan',
    description:
      'Project demand, stockouts, sales, purchase orders, split receipts and cash through the real embedded engine in a private store. Supply demand ranges, history or an exact dailyUnits forecast schedule, exact costs and reorder policy. Lead times and first-delivery fills accept fixed values, ranges or dailyValues schedules with exactly days+1 entries for order-placement days 0..days. Optional purchasing sets reviewIntervalDays (1..60, omitted means daily), minimum quantities, case packs and an inventory-position cap. Reviews occur on day zero and interval multiples; off-review low-stock attempts defer with nextReviewDay and unchanged cash before sizing or financial checks. Reports deferredPurchaseReviews; orders recompute current stock on review days before whole-order cash. Optional plan cashReserve withholds shared cash without shrinking orders. Reports daily cash and inventory flows, first stockout, minimum cash, supplier methods and reserve-blocked attempts. Inventory replay checks projected demand, purchase/receipt identities, supplier terms, daily event ordering and physical availability before sales against final native balances. Native purchase lines, sizing constraints, stock, reserves, cash flows and financial values must reconcile. Conditional projections; no live store or supplier access.',
    inputSchema: {
      plan: inventoryPlanSchema.describe(
        'Up to 120 product-days; exact money strings; synthetic assumptions only',
      ),
    },
    permission: 'read',
    policyDomain: 'simulations',
    handler: async ({ params }) => runInventorySimulation(params.plan),
  },
  {
    name: 'compare_inventory_plans',
    description:
      'Compare two or three inventory and purchasing policies using independent private embedded stores. The same seed, SKU and demand assumptions produce common demand paths across policy changes and product ordering. Returns exact metric deltas and global condition flags, plus each row comparisonToBaseline with changedConditions for opening cash, inventory, unit economics, purchase priority, cash reserve, horizon and realized demand/supply paths. Money is compared exactly; SKU facts are independent of product ordering, while purchase priority is checked separately. At most 180 product-days total; no real purchases or live-store writes.',
    inputSchema: {
      plans: inventoryPlanComparisonSchema.describe(
        'Distinct named plans in one currency with bounded demand and replenishment assumptions',
      ),
    },
    permission: 'read',
    policyDomain: 'simulations',
    handler: async ({ params }) => compareInventorySimulations(params.plans),
  },
];
