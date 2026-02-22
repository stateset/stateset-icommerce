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
      const plans = await commerce.listSubscriptionPlans({ status, billingInterval });
      return {
        success: true,
        count: plans.length,
        plans: plans.map((p) => ({
          id: p.id,
          code: p.code,
          name: p.name,
          status: p.status,
          billingInterval: p.billingInterval,
          price: p.price,
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
      const plan = await commerce.getSubscriptionPlan(planId);
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
      const plan = await commerce.createSubscriptionPlan({
        name,
        billingInterval,
        price: price.toString(),
        currency,
        trialDays,
        description,
        setupFee: setupFee?.toString(),
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
      const plan = await commerce.activateSubscriptionPlan(planId);
      return { success: true, message: `Plan "${plan.name}" activated`, plan };
    },
  },
  {
    name: 'archive_subscription_plan',
    description:
      'Archive a subscription plan (no new subscriptions, existing ones continue). Requires --apply flag.',
    inputSchema: { planId: z.string().min(1).describe('Plan ID to archive') },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { planId } = params;
      if (!allowApply)
        return {
          success: false,
          error: 'Archive operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldArchive: planId,
        };
      const plan = await commerce.archiveSubscriptionPlan(planId);
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
      const subscriptions = await commerce.listSubscriptions({ customerId, planId, status });
      return {
        count: subscriptions.length,
        subscriptions: subscriptions.map((s) => ({
          id: s.id,
          subscriptionNumber: s.subscriptionNumber,
          customerId: s.customerId,
          planName: s.planName,
          status: s.status,
          price: s.price,
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
      const subscription = await commerce.getSubscription(subscriptionId);
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
      const subscription = await commerce.createSubscription({
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
      const subscription = await commerce.pauseSubscription(subscriptionId, {
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
      const subscription = await commerce.resumeSubscription(subscriptionId);
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
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { subscriptionId, immediate, reason } = params;
      if (!allowApply)
        return {
          success: false,
          error: 'Cancel operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldCancel: subscriptionId,
        };
      const subscription = await commerce.cancelSubscription(subscriptionId, { immediate, reason });
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
      const subscription = await commerce.skipBillingCycle(subscriptionId, { reason });
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
      const cycles = await commerce.listBillingCycles({ subscriptionId, status });
      return {
        count: cycles.length,
        cycles: cycles.map((c) => ({
          id: c.id,
          cycleNumber: c.cycleNumber,
          status: c.status,
          periodStart: c.periodStart,
          periodEnd: c.periodEnd,
          total: c.total,
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
      const cycle = await commerce.getBillingCycle(cycleId);
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
      const events = await commerce.getSubscriptionEvents(subscriptionId, limit);
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
