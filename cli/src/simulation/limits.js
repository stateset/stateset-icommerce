// Shared by validation, execution and the model-facing kernel contract.
export const SIMULATION_KERNEL_VERSION = 1;
export const SIMULATION_LIMITS = Object.freeze({
  maxInputBytes: 131072,
  maxInputNodes: 16384,
  maxInputDepth: 32,
  maxActiveWorkers: 2,
  timeoutMs: 30000,
  maxComparedCases: 3,
  maxCommerceActions: 100,
  maxCommerceProducts: 32,
  maxPlanningProducts: 8,
  maxPlanningDays: 60,
  maxProductDays: 120,
  maxComparedProductDays: 180,
  maxExperimentSeeds: 8,
  maxExperimentRuns: 12,
  maxExperimentProductDays: 180,
  maxPurchaseQuantity: 1000000,
});
