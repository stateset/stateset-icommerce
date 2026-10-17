import { z } from 'zod';
import { SIMULATION_LIMITS as limits } from './limits.js';
import { roundPurchaseQuantity } from './purchasing.js';
import { boundedSimulationSchema } from './admission.js';

const alias = z.string().regex(/^[a-z][a-z0-9_-]{0,31}$/);
const sku = z.string().regex(/^[A-Za-z0-9][A-Za-z0-9_-]{0,63}$/);
const amount = z
  .string()
  .max(25)
  .regex(/^(0|[1-9]\d{0,17})(\.\d{1,6})?$/);
const catalogPrice = amount.refine(
  (value) => (value.split('.')[1] || '').length <= 2,
  'Sale prices for USD, EUR and GBP support at most two decimal places',
);
const expected = z.enum(['success', 'rejected']).default('success');
export const commerceActionSchema = z.discriminatedUnion('type', [
  z
    .object({
      type: z.literal('place_order'),
      id: alias,
      items: z
        .array(z.object({ sku, quantity: z.number().int().min(1).max(10000) }).strict())
        .min(1)
        .max(16),
      expect: expected,
    })
    .strict(),
  z
    .object({
      type: z.literal('capture_payment'),
      id: alias,
      order: alias,
      key: alias.optional(),
      expect: expected,
    })
    .strict(),
  z
    .object({
      type: z.literal('refund_payment'),
      id: alias,
      payment: alias,
      amount,
      key: alias.optional(),
      expect: expected,
    })
    .strict(),
  z.object({ type: z.literal('retry_payment'), payment: alias, expect: expected }).strict(),
  z.object({ type: z.literal('retry_refund'), refund: alias, expect: expected }).strict(),
  z
    .object({
      type: z.literal('restock'),
      sku,
      quantity: z.number().int().min(1).max(1000000),
      expect: expected,
    })
    .strict(),
  z.object({ type: z.literal('cancel_order'), order: alias, expect: expected }).strict(),
  z.object({ type: z.literal('ship_order'), order: alias, expect: expected }).strict(),
  z.object({ type: z.literal('inspect_state'), expect: expected }).strict(),
  z
    .object({
      type: z.literal('purchase_inventory'),
      id: alias,
      items: z
        .array(
          z
            .object({
              sku,
              quantity: z.number().int().min(1).max(10000),
              unitCost: amount,
            })
            .strict(),
        )
        .min(1)
        .max(16),
      expect: expected,
    })
    .strict(),
  z
    .object({ type: z.literal('receive_purchase_order'), purchase: alias, expect: expected })
    .strict(),
]);

export const commerceScenarioSchema = boundedSimulationSchema(
  z
    .object({
      name: z.string().min(1).max(80),
      currency: z.enum(['USD', 'EUR', 'GBP']).default('USD'),
      inventory: z
        .array(
          z
            .object({
              sku,
              unitPrice: catalogPrice,
              quantity: z.number().int().min(0).max(1000000),
            })
            .strict(),
        )
        .min(1)
        .max(limits.maxCommerceProducts),
      steps: z.array(commerceActionSchema).min(1).max(limits.maxCommerceActions),
    })
    .strict()
    .superRefine((scenario, context) => {
      const products = new Set();
      for (const [index, product] of scenario.inventory.entries()) {
        if (products.has(product.sku))
          context.addIssue({
            code: 'custom',
            path: ['inventory', index, 'sku'],
            message: 'Inventory SKUs must be unique',
          });
        products.add(product.sku);
      }
      const ids = new Set();
      for (const [index, step] of scenario.steps.entries()) {
        if (step.id) {
          if (ids.has(step.id))
            context.addIssue({
              code: 'custom',
              path: ['steps', index, 'id'],
              message: 'Step ids must be unique; use a retry action to replay',
            });
          ids.add(step.id);
        }
        const references = step.items || (step.sku ? [{ sku: step.sku }] : []);
        for (const reference of references) {
          if (!products.has(reference.sku))
            context.addIssue({
              code: 'custom',
              path: ['steps', index],
              message: 'Step references an unknown inventory SKU',
            });
        }
      }
    }),
);

export const commerceComparisonSchema = boundedSimulationSchema(
  z
    .array(commerceScenarioSchema)
    .min(2)
    .max(limits.maxComparedCases)
    .superRefine((scenarios, context) => {
      if (new Set(scenarios.map((scenario) => scenario.name)).size !== scenarios.length)
        context.addIssue({ code: 'custom', message: 'Compared scenario names must be unique' });
      if (new Set(scenarios.map((scenario) => scenario.currency)).size !== 1)
        context.addIssue({
          code: 'custom',
          message: 'Compared scenarios must use the same currency',
        });
      if (
        scenarios.reduce((sum, scenario) => sum + scenario.steps.length, 0) >
        limits.maxCommerceActions
      )
        context.addIssue({
          code: 'custom',
          message: 'A comparison supports at most 100 total steps',
        });
    }),
);

const demand = z.union([
  z
    .object({ min: z.number().int().min(0).max(10000), max: z.number().int().min(0).max(10000) })
    .strict()
    .refine((value) => value.min <= value.max, 'Demand min must not exceed max'),
  z.object({ history: z.array(z.number().int().min(0).max(10000)).min(2).max(90) }).strict(),
  z
    .object({
      dailyUnits: z
        .array(z.number().int().min(0).max(10000))
        .min(1)
        .max(limits.maxPlanningDays)
        .describe(
          'Explicit units for virtual days 1..days, in order. Exactly one entry per day; never resampled.',
        ),
    })
    .strict(),
]);

function boundedSupplierCondition(minimum, maximum) {
  const integer = z.number().int().min(minimum).max(maximum);
  return z.union([
    integer,
    z
      .object({ min: integer, max: integer })
      .strict()
      .refine((value) => value.min <= value.max, 'Range min must not exceed max'),
    z
      .object({
        dailyValues: z
          .array(integer)
          .min(2)
          .max(limits.maxPlanningDays + 1)
          .describe(
            'Explicit values for order-placement days 0..days, in order. Exactly days+1 entries, including day zero; never resampled.',
          ),
      })
      .strict(),
  ]);
}

const inventoryPlanInputSchema = z
  .object({
    name: z.string().min(1).max(80),
    currency: z.enum(['USD', 'EUR', 'GBP']).default('USD'),
    days: z.number().int().min(1).max(limits.maxPlanningDays),
    seed: z.number().int().min(0).max(4294967295).default(1),
    startingCash: amount,
    cashReserve: amount
      .describe(
        'Optional shared cash withheld from positive-cost purchases. Spendable cash is max(cash minus reserve, zero); omitted means zero. Does not fund an opening shortfall or resize orders.',
      )
      .optional(),
    products: z
      .array(
        z
          .object({
            sku,
            initialStock: z.number().int().min(0).max(1000000),
            unitPrice: catalogPrice.refine(
              (value) => /[1-9]/.test(value),
              'Sale price must be positive',
            ),
            unitCost: amount,
            demand,
            reorderPoint: z.number().int().min(0).max(999999),
            targetStock: z.number().int().min(1).max(1000000),
            leadTimeDays: boundedSupplierCondition(1, 60),
            delivery: z
              .object({
                firstReceiptBps: boundedSupplierCondition(0, 10000).describe(
                  'First delivery fill in basis points; floor(quantity * bps / 10000) units arrive first',
                ),
                remainderDelayDays: z.number().int().min(1).max(60),
              })
              .strict()
              .optional(),
            purchasing: z
              .object({
                minimumOrderQuantity: z
                  .number()
                  .int()
                  .min(1)
                  .max(limits.maxPurchaseQuantity)
                  .default(1),
                orderMultiple: z.number().int().min(1).max(limits.maxPurchaseQuantity).default(1),
                maximumInventoryPosition: z.number().int().min(1).max(1000000).optional(),
                reviewIntervalDays: z
                  .number()
                  .int()
                  .min(1)
                  .max(limits.maxPlanningDays)
                  .describe(
                    'Review purchases on day zero and every interval days thereafter. Omitted means daily. Eligible off-review purchases wait without creating a native purchase order or spending cash.',
                  )
                  .optional(),
              })
              .strict()
              .optional(),
          })
          .strict()
          .refine(
            (value) => value.targetStock > value.reorderPoint,
            'Target stock must exceed reorder point',
          )
          .refine(
            (value) =>
              roundPurchaseQuantity(
                value.targetStock,
                value.purchasing?.minimumOrderQuantity,
                value.purchasing?.orderMultiple,
              ) <= limits.maxPurchaseQuantity,
            'Rounded target/minimum quantity must not exceed 1000000 units',
          ),
      )
      .min(1)
      .max(limits.maxPlanningProducts),
  })
  .strict();

function validateInventoryPlan(plan, context) {
  if (new Set(plan.products.map((product) => product.sku)).size !== plan.products.length)
    context.addIssue({ code: 'custom', message: 'Product SKUs must be unique' });
  if (plan.days * plan.products.length > limits.maxProductDays)
    context.addIssue({ code: 'custom', message: 'A plan supports at most 120 product-days' });
  plan.products.forEach((product, index) => {
    if (product.demand.dailyUnits && product.demand.dailyUnits.length !== plan.days)
      context.addIssue({
        code: 'custom',
        path: ['products', index, 'demand', 'dailyUnits'],
        message: 'Explicit demand must contain exactly one entry per planning day',
      });
    for (const [condition, path] of [
      [product.leadTimeDays, ['leadTimeDays']],
      [product.delivery?.firstReceiptBps, ['delivery', 'firstReceiptBps']],
    ]) {
      if (condition?.dailyValues && condition.dailyValues.length !== plan.days + 1)
        context.addIssue({
          code: 'custom',
          path: ['products', index, ...path, 'dailyValues'],
          message:
            'Explicit supplier conditions must contain exactly days+1 entries for order-placement days 0..days',
        });
    }
  });
}

export const inventoryPlanSchema = boundedSimulationSchema(
  inventoryPlanInputSchema.superRefine(validateInventoryPlan),
);
export const inventoryPolicySchema = inventoryPlanInputSchema
  .omit({ seed: true })
  .superRefine(validateInventoryPlan);

export const inventoryPlanComparisonSchema = boundedSimulationSchema(
  z
    .array(inventoryPlanSchema)
    .min(2)
    .max(limits.maxComparedCases)
    .superRefine((plans, context) => {
      if (new Set(plans.map((plan) => plan.name)).size !== plans.length)
        context.addIssue({ code: 'custom', message: 'Compared plan names must be unique' });
      if (new Set(plans.map((plan) => plan.currency)).size !== 1)
        context.addIssue({ code: 'custom', message: 'Compared plans must use the same currency' });
      if (
        plans.reduce((sum, plan) => sum + plan.days * plan.products.length, 0) >
        limits.maxComparedProductDays
      )
        context.addIssue({
          code: 'custom',
          message: 'A comparison supports at most 180 total product-days',
        });
    }),
);

export const inventoryExperimentSchema = boundedSimulationSchema(
  z
    .object({
      name: z.string().min(1).max(80),
      plans: z.array(inventoryPolicySchema).min(1).max(limits.maxComparedCases),
      seeds: z.array(z.number().int().min(0).max(4294967295)).min(2).max(limits.maxExperimentSeeds),
      goals: z
        .object({
          minFillRateBps: z.number().int().min(0).max(10000).optional(),
          productFillRates: z
            .array(z.object({ sku, minFillRateBps: z.number().int().min(0).max(10000) }).strict())
            .min(1)
            .max(limits.maxPlanningProducts)
            .describe(
              'Optional per-SKU minimum unit fill rates, checked independently on every seed. Unique SKUs must exist in every compared policy. Zero demand is not evaluable, even at threshold zero.',
            )
            .optional(),
          minEndingCash: amount.optional(),
          minCashBalance: amount
            .describe(
              'Minimum cash at opening and after every sale and prepaid purchase, including day zero; evaluated without changing the purchasing policy',
            )
            .optional(),
          minGrossMargin: z
            .string()
            .max(26)
            .regex(/^-?(0|[1-9]\d{0,17})(\.\d{1,6})?$/)
            .optional(),
          maxPurchasingSpend: amount.optional(),
        })
        .strict()
        .refine(
          (goals) => Object.values(goals).some((value) => value !== undefined),
          'Supply at least one experiment goal',
        )
        .describe(
          'Optional inclusive per-run thresholds, shared by every policy. Product fill-rate targets require their SKUs in every policy. All targets and other goals must pass on the same seed. Fill rate is sold/demanded; zero demand is not evaluable. Goal misses are successful simulation outcomes.',
        )
        .optional(),
    })
    .strict()
    .superRefine((experiment, context) => {
      const productGoals = experiment.goals?.productFillRates ?? [];
      const goalSkus = new Set();
      for (const [index, goal] of productGoals.entries()) {
        if (goalSkus.has(goal.sku))
          context.addIssue({
            code: 'custom',
            path: ['goals', 'productFillRates', index, 'sku'],
            message: 'Product fill-rate goal SKUs must be unique',
          });
        goalSkus.add(goal.sku);
        if (experiment.plans.some((plan) => !plan.products.some((item) => item.sku === goal.sku)))
          context.addIssue({
            code: 'custom',
            path: ['goals', 'productFillRates', index, 'sku'],
            message: 'Product fill-rate goal SKU must exist in every policy',
          });
      }
      if (new Set(experiment.seeds).size !== experiment.seeds.length)
        context.addIssue({
          code: 'custom',
          path: ['seeds'],
          message: 'Experiment seeds must be unique',
        });
      if (new Set(experiment.plans.map((plan) => plan.name)).size !== experiment.plans.length)
        context.addIssue({
          code: 'custom',
          path: ['plans'],
          message: 'Experiment plan names must be unique',
        });
      if (new Set(experiment.plans.map((plan) => plan.currency)).size !== 1)
        context.addIssue({
          code: 'custom',
          path: ['plans'],
          message: 'Experiment plans must use one currency',
        });
      if (experiment.plans.length * experiment.seeds.length > limits.maxExperimentRuns)
        context.addIssue({
          code: 'custom',
          message: 'An experiment supports at most 12 native runs',
        });
      const productDays =
        experiment.plans.reduce((sum, plan) => sum + plan.days * plan.products.length, 0) *
        experiment.seeds.length;
      if (productDays > limits.maxExperimentProductDays)
        context.addIssue({
          code: 'custom',
          message: 'An experiment supports at most 180 total product-days',
        });
    }),
);
