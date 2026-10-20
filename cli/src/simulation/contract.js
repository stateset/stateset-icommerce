import { zodToJsonSchema } from 'zod-to-json-schema';
import {
  commerceActionSchema,
  commerceScenarioSchema,
  inventoryPlanSchema,
  inventoryExperimentSchema,
} from './schema.js';
import { SIMULATION_KERNEL_VERSION, SIMULATION_LIMITS } from './limits.js';

// Derived schemas keep the discovery tool aligned with the execution surface.
export function getSimulationKernelContract() {
  const jsonSchema = (schema) =>
    zodToJsonSchema(schema, { $refStrategy: 'none', target: 'jsonSchema7' });
  return {
    success: true,
    specVersion: SIMULATION_KERNEL_VERSION,
    engine: 'stateset-icommerce-native',
    execution: 'stateless-bounded-episodes',
    isolation: 'fresh-private-sqlite-per-scenario',
    liveStoreRead: false,
    liveStoreModified: false,
    externalServicesCalled: false,
    limits: { ...SIMULATION_LIMITS },
    actions: commerceActionSchema.options.map((option) => option.shape.type.value),
    schemas: {
      commerce: jsonSchema(commerceScenarioSchema),
      inventoryPlanning: jsonSchema(inventoryPlanSchema),
      inventoryExperiment: jsonSchema(inventoryExperimentSchema),
    },
    validation:
      'Input admission runs before typed schema parsing: at most 128 KiB, 16384 data nodes and 32 nesting levels (root counts as level one). Plain objects, arrays and finite JSON scalars only; optional undefined values are allowed and conservatively counted as null. Cycles, proxies, accessors, custom array properties and class instances refuse without calling serialization hooks. Canonical requests are byte-checked again after defaults and across complete comparisons. Unique names, aliases and SKUs; known SKU references; exact demand-schedule length; same currency and aggregate work limits are additionally enforced at runtime.',
    money:
      'Decimal strings only; USD, EUR and GBP sale prices support two decimals; purchase costs support six. Quantities are integers.',
    observations: {
      commerce:
        'Each action returns success or rejection, expectationMet, and an aliased observation or redacted reason. Unknown aliases refuse before execution. inspect_state returns native stock, orders, payments and purchase lines. Final state is always returned on successful episode execution.',
      inventoryPlanning:
        'Daily events, independent seeded demand and supplier paths, native record counts and purchase-line reconciliation, exact cash and margin, stock/cash/asset reconciliation. inventoryFlow exposes day-zero-through-horizon per-SKU physical and inbound movements plus firstStockout; independent replay binds projected paths, receipts and sales to native ending balances. Comparisons include exact deltas against the first plan.',
      inventoryExperiment:
        'One private native episode per plan and shared seed; observed p10/p50/p90 outcomes, exact rational money means and unit fill rates, stockout, supply-disrupted and cash-budget-blocked counts, paired metric deltas and comparability flags for paths, horizon, opening cash/inventory, unit economics and product purchase priority. Per-policy pathDiversity counts distinct projected demand, supply and joint paths, including duplicate joint-path runs; these are not independent observations or an effective statistical sample size. Optional shared goals return per-seed checks and policy-level counts. Each seed includes inventoryFlow and cashFlow. Aggregation requires inventoryFlowBalanced alongside other reconciliation evidence. Per-seed summaries can be replayed with simulate_inventory_plan.',
    },
    comparisonConditions:
      'Single-run comparison rows and experiment policies expose comparisonToBaseline against the first plan. It includes sameHorizon, sameOpeningCash, sameCashReserve, sameOpeningInventory, sameUnitEconomics, sameProductPriority, sameDemandPaths and sameSupplyPaths, plus changedConditions listing the mismatched checks as horizon, openingCash, cashReserve, openingInventory, unitEconomics, productPriority, demandPath or supplyPath. Each alternative is checked independently; global flags require every alternative to match. Money is compared exactly with normalized decimal strings; opening inventory and unit economics compare SKU facts independently of product ordering. Product purchase priority is checked separately. Experiment path checks pair all runs by supplied seed, including order-placement day zero for supply. Path equality describes projected trajectories for these runs and does not establish equivalent generating assumptions. These diagnostics cover initial conditions and realized paths; purchasing rules remain visible in the supplied plans. They do not establish causal attribution, statistical significance or a winning policy.',
    inventoryFlow:
      'Every inventory plan and experiment run includes inventoryFlow: firstStockout is null or the first {day,sku,demanded,sold,lostSales} unmet-demand event; same-day ties use product input order. daily contains days 0..days, each with products in input order: sku, openingStock, openingOnOrder, receivedUnits, demanded, sold, lostSales, orderedUnits, closingStock and onOrder. Zero closing stock without unmet demand is not a stockout. Independent replay checks exact projected demand, selected supplier terms, unique purchase aliases, due receipt identities and quantities, split/missed delivery timing, receipts before all sales before purchases, physical stock before sales, purchasing inventory positions and native final per-SKU balances. Summary units and receipt/order counts must match the replay. An impossible intermediate balance refuses even if final totals match. Reports expose reconciliation.inventoryFlowBalanced only after success; experiment aggregation requires it.',
    time: {
      commerce:
        'Actions execute in input order. Native timestamps use the wall clock and are omitted from normalized observations.',
      inventoryPlanning:
        'Virtual day zero replenishment, then receipts before all daily sales before purchases. Native timestamps still use the wall clock.',
    },
    determinism:
      'Aliases and financial outcomes are normalized. Sampled demand uses mulberry32-sku-v1; explicit daily demand ignores the demand seed and keeps the supplied order. Supply uses independent mulberry32-supply-v1 streams indexed by SKU and order-placement day, including day zero. Matching seed and assumptions preserve paths across policy and product-order changes.',
    demand:
      'Each product accepts exactly one demand mode: uniform integer {min,max}, empirical-history resampling {history}, or explicit forecast/scenario {dailyUnits}. Explicit schedules have exactly days integer entries (0..10000), indexed from virtual day one; they are never repeated, truncated or resampled. Their expected horizon units equal the supplied sum. Different seeds still vary any sampled supplier conditions, but fixed inputs may produce duplicate paths.',
    supply:
      'leadTimeDays accepts a fixed integer, {min,max} range or {dailyValues:[...]} schedule, with values 1..60. Optional delivery specifies firstReceiptBps in the same three modes (values 0..10000) and fixed remainderDelayDays (1..60). Explicit schedules contain exactly days+1 bounded integer entries indexed by order-placement days 0..days, including initial purchases on day zero. Values are never repeated, clipped or resampled; they apply when an order is placed, not when it arrives or by purchase ordinal. Existing orders retain their original lead time and first fill. Explicit fields ignore the seed; other sampled fields retain their independent v1 streams. Supply products expose leadTimeMethod and firstReceiptMethod as fixed, uniform-integer or explicit-daily-values. Floor(quantity*bps/10000) units arrive first; the balance remains prepaid in transit until a later receipt. Zero first deliveries never create native zero-quantity receipts. Repeating seeds with fully fixed or explicit conditions does not add distinct supplier paths.',
    purchasing:
      'Optional per-product purchasing specifies reviewIntervalDays (integer 1..60, omitted means daily), minimumOrderQuantity and orderMultiple (default 1). Reviews occur on day zero and multiples of the interval. Low-stock attempts on other days emit purchase_review_deferred with inventoryPosition, reviewIntervalDays, nextReviewDay and unchanged cash; no PO, reservation or payment is created. Quantities are recomputed at the next review; supplier terms use that actual placement day. A next review beyond the horizon is not executed. Calendar weekdays and holidays are not modeled. Schedule checks precede sizing, capacity and financial constraints. Round max(target shortfall, minimum) up to a multiple before checking the optional maximumInventoryPosition (available plus inbound plus the proposed order), then actual cash, then optional plan-level cashReserve (exact nonnegative decimal string, omitted means zero). Spendable cash is max(cash minus cashReserve, zero), shared across all products in input purchase priority. Whole rounded orders are blocked, never shrunk, if cost exceeds spendable cash. Zero-cost purchases remain allowed below the reserve. Starting below the reserve is permitted and does not create funds; sales may release later purchasing capacity. Rounding may exceed target stock. Refused attempts create no native PO and do not spend cash. Reports distinguish constraint, budget and reserve blocks, rounded orders and cumulative extra purchased units. cashReserve controls spending; experiment minCashBalance independently evaluates the actual minimum, including opening cash. Comparisons expose sameCashReserve and reserveBlockedPurchases; experiments also count reserveBlockedRuns. deferredPurchaseReviews counts low-stock off-review attempts, including repeated days; experiments add reviewDeferredRuns and paired count metrics. Independent inventory replay checks review dates, deferral evidence and counts; cash replay verifies deferrals do not change cash.',
    experimentGoals:
      'Optional goals: minFillRateBps (integer 0..10000), productFillRates (one to eight unique {sku,minFillRateBps} targets), minEndingCash, minCashBalance, minGrossMargin (signed decimal string allowed), maxPurchasingSpend. Each targeted SKU must exist in every policy. Product fill rates use that SKU alone; overall service cannot compensate for a targeted product miss. Per-run checks.productFillRates.products exposes exact fractions and status for each target; policy evaluation.byGoal.productFillRates.products adds per-target met/missed/notEvaluableRuns and pooled serviceLevel. Money goals use the experiment currency. Thresholds are inclusive and checked on each seed, with all goals and product targets required in the same run. minCashBalance checks opening cash and the balance after every sale and prepaid purchase, including day zero; it evaluates the supplied policy and does not enforce a spending reserve. Cash-flow reports expose the first minimum and exact daily opening, sales receipts, purchase spending, closing and minimum balances, independently reconciled to the native-backed timeline. Fill rate is sold/demanded without rounding; no demand yields null and not-evaluable, including a product target at threshold zero. Policy serviceLevel pools sold/demanded across seeds. Goal misses are valid outcomes and do not change execution success. No automatic ranking or probability estimate.',
    failureSemantics:
      'An unexpected rejection sets expectationsMet=false but does not stop later actions. An episode may span multiple native transactions; inspect state after rejection. Worker failures return a structured error and remove the private store.',
    workflow: [
      'Describe a hypothesis with explicit initial stock, exact prices and typed actions, or demand and purchasing assumptions.',
      'Run a baseline using simulate_commerce_scenario or simulate_inventory_plan.',
      'Inspect action outcomes and state or planning reconciliation; correct unmet expectations.',
      'Compare alternatives with compare_commerce_scenarios or compare_inventory_plans; keep demand assumptions and seed fixed to compare policies.',
      'Use simulate_inventory_experiment with shared seeds to examine variation and paired policy differences. Inspect pathDiversity: different seeds may repeat the same paths, especially with explicit schedules and fixed supply.',
      'Revise the inputs and run a fresh episode. Simulation does not authorize applying the result to a live store.',
    ],
    limitations: [
      'No persistent sessions, caller-selected database paths, arbitrary code, external providers or real funds.',
      'Purchase receipts and inventory postings are separate transactions. Shipment actions explicitly post held physical stock.',
      'Manual commerce episodes do not enforce a purchasing cash budget; inventory planning does.',
      'Supplier short deliveries are delayed balances, not permanent losses or supplier refunds. Unreceived prepaid units remain in-transit inventory at the horizon.',
      'Each planning episode is a conditional single-path projection. Experiment percentiles summarize supplied seeds and are not calibrated confidence intervals. Explicit schedules can encode user-supplied trends or seasonal scenarios; automatic trend/seasonality fitting, taxes, fees and holding costs are not modeled.',
    ],
  };
}
