/**
 * Subscriptions & Billing Tools Module
 */

import { z } from 'zod';

const billingIntervalEnum = z.enum([
  'weekly',
  'biweekly',
  'monthly',
  'bimonthly',
  'quarterly',
  'semiannual',
  'annual',
]);

// Fields the binding's UpdateSubscriptionPlanInput / UpdateSubscriptionInput
// accept, mapped to their kind. `number` fields (money included) are `number`
// in the binding, so decimal strings from a model are coerced here rather than
// rejected by napi; anything else is refused instead of being silently dropped.
const PLAN_UPDATE_FIELDS = {
  name: 'string',
  description: 'string',
  price: 'number',
  setupFee: 'number',
  trialDays: 'number',
  trialRequiresPaymentMethod: 'boolean',
  minCycles: 'number',
  maxCycles: 'number',
  discountPercent: 'number',
  discountAmount: 'number',
};

const SUBSCRIPTION_UPDATE_FIELDS = {
  status: 'string',
  price: 'number',
  paymentMethodId: 'string',
  nextBillingDate: 'string',
  discountPercent: 'number',
  discountAmount: 'number',
  couponCode: 'string',
};

const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

function toBindingInput(updates, fields) {
  const input = {};
  const unsupported = [];
  for (const [key, value] of Object.entries(updates ?? {})) {
    if (!Object.hasOwn(fields, key)) {
      unsupported.push(key);
      continue;
    }
    if (value === undefined || value === null) continue;
    if (fields[key] === 'number' && typeof value === 'string' && value.trim() !== '') {
      const n = Number(value);
      if (!Number.isFinite(n)) {
        unsupported.push(`${key} (not a number: ${JSON.stringify(value)})`);
        continue;
      }
      input[key] = n;
    } else {
      input[key] = value;
    }
  }
  return { input, unsupported };
}

export const subscriptionTools = [
  {
    name: 'list_subscription_plans',
    description:
      'List all subscription plans. Filter by status (draft, active, archived) or billing interval.',
    inputSchema: {
      status: z.enum(['draft', 'active', 'archived']).optional().describe('Filter by plan status'),
      billingInterval: billingIntervalEnum.optional().describe('Filter by billing interval'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { status, billingInterval } = params;
      const plans = await commerce.subscriptions.listPlans({ status, billingInterval });
      return {
        success: true,
        count: plans.length,
        plans: plans.map((p) => ({
          id: p.id,
          code: p.code,
          name: p.name,
          status: p.status,
          billingInterval: p.billingInterval,
          price: p.priceExact ?? p.price,
          currency: p.currency,
          trialDays: p.trialDays,
        })),
      };
    },
  },
  {
    name: 'get_subscription_plan',
    description: 'Get details for a specific subscription plan.',
    inputSchema: { planId: z.string().min(1).describe('Plan ID or code') },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { planId } = params;
      // getPlan rejects a non-UUID argument outright, so route codes to getPlanByCode.
      const plan = UUID_RE.test(planId)
        ? await commerce.subscriptions.getPlan(planId)
        : await commerce.subscriptions.getPlanByCode(planId);
      if (!plan) return { success: false, error: 'Plan not found' };
      return { success: true, plan };
    },
  },
  {
    name: 'create_subscription_plan',
    description: 'Create a new subscription plan. Requires --apply flag.',
    inputSchema: {
      name: z.string().min(1).describe('Plan name'),
      billingInterval: billingIntervalEnum.describe('Billing interval'),
      price: z.number().positive().describe('Price per billing cycle'),
      currency: z.string().max(10).optional().describe('Currency code (default: USD)'),
      trialDays: z.number().int().min(0).optional().describe('Trial period in days'),
      description: z.string().max(5000).optional().describe('Plan description'),
      setupFee: z.number().positive().optional().describe('One-time setup fee'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { name, billingInterval, price, currency, trialDays, description, setupFee } = params;
      if (!allowApply)
        return {
          success: false,
          error: 'Create operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldCreate: { name, billingInterval, price },
        };
      // The binding's CreateSubscriptionPlanInput takes money as `number`.
      const plan = await commerce.subscriptions.createPlan({
        name,
        billingInterval,
        price,
        currency,
        trialDays,
        description,
        setupFee,
      });
      return { success: true, message: `Created subscription plan "${plan.name}"`, plan };
    },
  },
  {
    name: 'activate_subscription_plan',
    description:
      'Activate a subscription plan (make it available for new subscriptions). Requires --apply flag.',
    inputSchema: { planId: z.string().min(1).describe('Plan ID to activate') },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { planId } = params;
      if (!allowApply)
        return {
          success: false,
          error: 'Activate operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldActivate: planId,
        };
      const plan = await commerce.subscriptions.activatePlan(planId);
      return { success: true, message: `Plan "${plan.name}" activated`, plan };
    },
  },
  {
    name: 'update_subscription_plan',
    description: 'Update an existing subscription plan. Requires --apply flag.',
    inputSchema: {
      planId: z.string().min(1).describe('Plan ID'),
      updates: z
        .record(z.string(), z.any())
        .describe(
          'Partial plan fields: name, description, price, setupFee, trialDays, trialRequiresPaymentMethod, minCycles, maxCycles, discountPercent (fraction 0-1), discountAmount',
        ),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Update operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldUpdate: { planId: params.planId, updates: params.updates },
        };
      }
      const { input, unsupported } = toBindingInput(params.updates, PLAN_UPDATE_FIELDS);
      if (unsupported.length > 0) {
        return {
          success: false,
          error: `Unsupported plan update field(s): ${unsupported.join(', ')}`,
          allowedFields: Object.keys(PLAN_UPDATE_FIELDS),
        };
      }
      const plan = await commerce.subscriptions.updatePlan(params.planId, input);
      return { success: true, message: `Plan "${plan.name}" updated`, plan };
    },
  },
  {
    name: 'archive_subscription_plan',
    description:
      'Archive a subscription plan (no new subscriptions, existing ones continue). Requires --apply flag.',
    inputSchema: { planId: z.string().min(1).describe('Plan ID to archive') },
    permission: 'delete',
    handler: async ({ commerce, params, allowApply }) => {
      const { planId } = params;
      if (!allowApply)
        return {
          success: false,
          error: 'Archive operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldArchive: planId,
        };
      const plan = await commerce.subscriptions.archivePlan(planId);
      return { success: true, message: `Plan "${plan.name}" archived`, plan };
    },
  },
  {
    name: 'list_subscriptions',
    description: 'List subscriptions. Filter by customer, plan, or status.',
    inputSchema: {
      customerId: z.string().optional().describe('Filter by customer ID'),
      planId: z.string().optional().describe('Filter by plan ID'),
      status: z
        .enum(['trial', 'active', 'paused', 'past_due', 'cancelled', 'expired', 'pending'])
        .optional()
        .describe('Filter by status'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { customerId, planId, status } = params;
      const subscriptions = await commerce.subscriptions.list({ customerId, planId, status });
      return {
        count: subscriptions.length,
        subscriptions: subscriptions.map((s) => ({
          id: s.id,
          subscriptionNumber: s.subscriptionNumber,
          customerId: s.customerId,
          planName: s.planName,
          status: s.status,
          price: s.priceExact ?? s.price,
          currency: s.currency,
          nextBillingDate: s.nextBillingDate,
          billingCycleCount: s.billingCycleCount,
        })),
      };
    },
  },
  {
    name: 'get_subscription',
    description: 'Get details for a specific subscription.',
    inputSchema: { subscriptionId: z.string().min(1).describe('Subscription ID or number') },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { subscriptionId } = params;
      // get rejects a non-UUID argument outright, so route numbers to getByNumber.
      const subscription = UUID_RE.test(subscriptionId)
        ? await commerce.subscriptions.get(subscriptionId)
        : await commerce.subscriptions.getByNumber(subscriptionId);
      if (!subscription) return { success: false, error: 'Subscription not found' };
      return subscription;
    },
  },
  {
    name: 'create_subscription',
    description: 'Create a new subscription for a customer. Requires --apply flag.',
    inputSchema: {
      customerId: z.string().min(1).describe('Customer ID'),
      planId: z.string().min(1).describe('Plan ID'),
      paymentMethodId: z.string().optional().describe('Payment method ID from payment provider'),
      skipTrial: z.boolean().optional().describe('Skip trial period'),
      couponCode: z.string().optional().describe('Coupon code to apply'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { customerId, planId, paymentMethodId, skipTrial, couponCode } = params;
      if (!allowApply)
        return {
          success: false,
          error: 'Subscribe operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldSubscribe: { customerId, planId },
        };
      const subscription = await commerce.subscriptions.subscribe({
        customerId,
        planId,
        paymentMethodId,
        skipTrial,
        couponCode,
      });
      return {
        success: true,
        message: `Created subscription ${subscription.subscriptionNumber}`,
        subscription,
      };
    },
  },
  {
    name: 'pause_subscription',
    description: 'Pause a subscription (stops billing, can resume later). Requires --apply flag.',
    inputSchema: {
      subscriptionId: z.string().min(1).describe('Subscription ID'),
      resumeAt: z.string().optional().describe('ISO date when to auto-resume'),
      reason: z.string().optional().describe('Reason for pausing'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { subscriptionId, resumeAt, reason } = params;
      if (!allowApply)
        return {
          success: false,
          error: 'Pause operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldPause: subscriptionId,
        };
      const subscription = await commerce.subscriptions.pause(subscriptionId, {
        resumeAt: resumeAt ? new Date(resumeAt).toISOString() : undefined,
        reason,
      });
      return {
        success: true,
        message: `Subscription ${subscription.subscriptionNumber} paused`,
        subscription,
      };
    },
  },
  {
    name: 'update_subscription',
    description:
      'Update subscription fields (status, price, paymentMethodId, nextBillingDate, discountPercent as a 0-1 fraction, discountAmount, couponCode). Requires --apply flag.',
    inputSchema: {
      subscriptionId: z.string().min(1).describe('Subscription ID'),
      updates: z
        .record(z.string(), z.any())
        .describe('Partial subscription fields to update (see description for allowed keys)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Update operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldUpdate: { subscriptionId: params.subscriptionId, updates: params.updates },
        };
      }
      const { input, unsupported } = toBindingInput(params.updates, SUBSCRIPTION_UPDATE_FIELDS);
      if (unsupported.length > 0) {
        return {
          success: false,
          error: `Unsupported subscription update field(s): ${unsupported.join(', ')}`,
          allowedFields: Object.keys(SUBSCRIPTION_UPDATE_FIELDS),
        };
      }
      const subscription = await commerce.subscriptions.update(params.subscriptionId, input);
      return {
        success: true,
        message: `Subscription ${subscription.subscriptionNumber} updated`,
        subscription,
      };
    },
  },
  {
    name: 'resume_subscription',
    description: 'Resume a paused subscription. Requires --apply flag.',
    inputSchema: { subscriptionId: z.string().min(1).describe('Subscription ID') },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { subscriptionId } = params;
      if (!allowApply)
        return {
          success: false,
          error: 'Resume operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldResume: subscriptionId,
        };
      const subscription = await commerce.subscriptions.resume(subscriptionId);
      return {
        success: true,
        message: `Subscription ${subscription.subscriptionNumber} resumed`,
        subscription,
      };
    },
  },
  {
    name: 'cancel_subscription',
    description:
      'Cancel a subscription. By default cancels at end of period. Requires --apply flag.',
    inputSchema: {
      subscriptionId: z.string().min(1).describe('Subscription ID'),
      immediate: z
        .boolean()
        .optional()
        .describe('Cancel immediately (default: false, cancels at period end)'),
      reason: z.string().optional().describe('Reason for cancellation'),
    },
    permission: 'delete',
    handler: async ({ commerce, params, allowApply }) => {
      const { subscriptionId, immediate, reason } = params;
      if (!allowApply)
        return {
          success: false,
          error: 'Cancel operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldCancel: subscriptionId,
        };
      const subscription = await commerce.subscriptions.cancel(subscriptionId, {
        immediate,
        reason,
      });
      return {
        success: true,
        message: immediate
          ? `Subscription ${subscription.subscriptionNumber} cancelled immediately`
          : `Subscription ${subscription.subscriptionNumber} will cancel at period end`,
        subscription,
      };
    },
  },
  {
    name: 'skip_billing_cycle',
    description: 'Skip the next billing cycle for a subscription. Requires --apply flag.',
    inputSchema: {
      subscriptionId: z.string().min(1).describe('Subscription ID'),
      reason: z.string().optional().describe('Reason for skipping'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { subscriptionId, reason } = params;
      if (!allowApply)
        return {
          success: false,
          error: 'Skip operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldSkip: subscriptionId,
        };
      const subscription = await commerce.subscriptions.skipBilling(subscriptionId, { reason });
      return {
        success: true,
        message: `Next billing cycle skipped for ${subscription.subscriptionNumber}`,
        nextBillingDate: subscription.nextBillingDate,
        subscription,
      };
    },
  },
  {
    name: 'list_billing_cycles',
    description: 'List billing cycles for a subscription.',
    inputSchema: {
      subscriptionId: z.string().min(1).describe('Subscription ID'),
      status: z
        .enum(['scheduled', 'processing', 'paid', 'failed', 'skipped', 'refunded', 'voided'])
        .optional()
        .describe('Filter by status'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { subscriptionId, status } = params;
      const cycles = await commerce.subscriptions.listBillingCycles({ subscriptionId, status });
      return {
        count: cycles.length,
        cycles: cycles.map((c) => ({
          id: c.id,
          cycleNumber: c.cycleNumber,
          status: c.status,
          periodStart: c.periodStart,
          periodEnd: c.periodEnd,
          total: c.totalExact ?? c.total,
          currency: c.currency,
          billedAt: c.billedAt,
        })),
      };
    },
  },
  {
    name: 'get_billing_cycle',
    description: 'Get details for a specific billing cycle.',
    inputSchema: { cycleId: z.string().min(1).describe('Billing cycle ID') },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { cycleId } = params;
      const cycle = await commerce.subscriptions.getBillingCycle(cycleId);
      if (!cycle) return { success: false, error: 'Billing cycle not found' };
      return cycle;
    },
  },
  {
    name: 'get_subscription_events',
    description: 'Get event history (audit log) for a subscription.',
    inputSchema: {
      subscriptionId: z.string().min(1).describe('Subscription ID'),
      limit: z.number().optional().describe('Maximum events to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { subscriptionId, limit } = params;
      // The binding's getEvents takes no limit; apply it here.
      const all = await commerce.subscriptions.getEvents(subscriptionId);
      const events = typeof limit === 'number' && limit >= 0 ? all.slice(0, limit) : all;
      return {
        count: events.length,
        events: events.map((e) => ({
          id: e.id,
          eventType: e.eventType,
          description: e.description,
          triggeredBy: e.triggeredBy,
          createdAt: e.createdAt,
        })),
      };
    },
  },
];

export default subscriptionTools;
