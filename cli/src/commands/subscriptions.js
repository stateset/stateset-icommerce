/**
 * Subscriptions Commands Module
 */

async function getPlan(commerce, identifier) {
  let plan = await commerce.getSubscriptionPlan(identifier);
  if (!plan && typeof commerce.getSubscriptionPlanByCode === 'function') {
    plan = await commerce.getSubscriptionPlanByCode(identifier);
  }
  return plan;
}

async function getSubscription(commerce, identifier) {
  let subscription = await commerce.getSubscription(identifier);
  if (!subscription && typeof commerce.getSubscriptionByNumber === 'function') {
    subscription = await commerce.getSubscriptionByNumber(identifier);
  }
  return subscription;
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'plans': {
      const [status, billingInterval] = args;
      const plans = await commerce.listSubscriptionPlans({ status, billingInterval });
      return formatPlanList(plans, { output, jsonOutput });
    }

    case 'plan': {
      const identifier = args[0];
      if (!identifier) throw new Error('Usage: subscriptions plan <id|code>');
      const plan = await getPlan(commerce, identifier);
      if (!plan) throw new Error(`Subscription plan not found: ${identifier}`);
      return formatPlanDetail(plan, { jsonOutput });
    }

    case 'list': {
      const [customerId, status] = args;
      const subscriptions = await commerce.listSubscriptions({ customerId, status });
      return formatSubscriptionList(subscriptions, { output, jsonOutput });
    }

    case 'get': {
      const identifier = args[0];
      if (!identifier) throw new Error('Usage: subscriptions get <id|number>');
      const subscription = await getSubscription(commerce, identifier);
      if (!subscription) throw new Error(`Subscription not found: ${identifier}`);
      return formatSubscriptionDetail(subscription, { jsonOutput });
    }

    case 'create': {
      const [customerId, planId] = args;
      if (!customerId || !planId) {
        throw new Error('Usage: subscriptions create <customerId> <planId>');
      }
      const subscription = await commerce.createSubscription({ customerId, planId });
      return {
        subscription,
        formatted: `Created subscription ${subscription.subscriptionNumber || subscription.id}`,
      };
    }

    case 'pause': {
      const [subscriptionId, ...reasonParts] = args;
      if (!subscriptionId) throw new Error('Usage: subscriptions pause <subscriptionId> [reason]');
      const reason = reasonParts.join(' ') || undefined;
      const subscription = await commerce.pauseSubscription(subscriptionId, { reason });
      return {
        subscription,
        formatted: `Paused subscription ${subscription.subscriptionNumber || subscription.id}`,
      };
    }

    case 'resume': {
      const subscriptionId = args[0];
      if (!subscriptionId) throw new Error('Usage: subscriptions resume <subscriptionId>');
      const subscription = await commerce.resumeSubscription(subscriptionId);
      return {
        subscription,
        formatted: `Resumed subscription ${subscription.subscriptionNumber || subscription.id}`,
      };
    }

    case 'cancel': {
      const [subscriptionId, immediateFlag] = args;
      if (!subscriptionId) {
        throw new Error('Usage: subscriptions cancel <subscriptionId> [immediate]');
      }
      const immediate = immediateFlag === 'true' || immediateFlag === 'immediate';
      const subscription = await commerce.cancelSubscription(subscriptionId, { immediate });
      return {
        subscription,
        formatted: immediate
          ? `Cancelled subscription ${subscription.subscriptionNumber || subscription.id} immediately`
          : `Cancelled subscription ${subscription.subscriptionNumber || subscription.id} at period end`,
      };
    }

    case 'cycles': {
      const [subscriptionId, status] = args;
      const cycles = await commerce.listBillingCycles({ subscriptionId, status });
      return formatBillingCycles(cycles, { output, jsonOutput });
    }

    case 'events': {
      const subscriptionId = args[0];
      const limit = Number.parseInt(args[1] || '20', 10);
      if (!subscriptionId) throw new Error('Usage: subscriptions events <subscriptionId> [limit]');
      const events = await commerce.getSubscriptionEvents(subscriptionId, limit);
      return formatEvents(events, { output, jsonOutput });
    }

    default:
      throw new Error(
        `Unknown action: subscriptions ${action}\n\n` +
          'Available actions:\n' +
          '  plans [status] [interval]   List subscription plans\n' +
          '  plan <id|code>              Get plan details\n' +
          '  list [customerId] [status]  List subscriptions\n' +
          '  get <id|number>             Get subscription details\n' +
          '  create <customerId> <planId>  Create subscription\n' +
          '  pause <subscriptionId> [reason]  Pause subscription\n' +
          '  resume <subscriptionId>     Resume subscription\n' +
          '  cancel <subscriptionId> [immediate]  Cancel subscription\n' +
          '  cycles [subscriptionId] [status]     List billing cycles\n' +
          '  events <subscriptionId> [limit]      List subscription events',
      );
  }
}

function formatPlanList(plans, { output, jsonOutput }) {
  if (jsonOutput) return plans;
  if (plans.length === 0) return { formatted: 'No subscription plans found.' };
  const formatted = output.table(plans, [
    { key: 'id', header: 'ID' },
    { key: 'code', header: 'Code' },
    { key: 'name', header: 'Name' },
    { key: 'status', header: 'Status' },
    { key: 'billingInterval', header: 'Interval' },
    { key: 'price', header: 'Price', align: 'right' },
  ]);
  return { plans, formatted };
}

function formatPlanDetail(plan, { jsonOutput }) {
  if (jsonOutput) return plan;
  return {
    plan,
    formatted:
      `Plan: ${plan.name}\n` +
      `${'-'.repeat(32)}\n` +
      `ID:          ${plan.id}\n` +
      `Code:        ${plan.code}\n` +
      `Status:      ${plan.status}\n` +
      `Interval:    ${plan.billingInterval}\n` +
      `Price:       ${plan.price} ${plan.currency}\n` +
      `Trial days:  ${plan.trialDays ?? 0}`,
  };
}

function formatSubscriptionList(subscriptions, { output, jsonOutput }) {
  if (jsonOutput) return subscriptions;
  if (subscriptions.length === 0) return { formatted: 'No subscriptions found.' };
  const formatted = output.table(subscriptions, [
    { key: 'id', header: 'ID' },
    { key: 'subscriptionNumber', header: 'Number' },
    { key: 'customerId', header: 'Customer' },
    { key: 'planName', header: 'Plan' },
    { key: 'status', header: 'Status' },
    { key: 'nextBillingDate', header: 'Next Billing' },
  ]);
  return { subscriptions, formatted };
}

function formatSubscriptionDetail(subscription, { jsonOutput }) {
  if (jsonOutput) return subscription;
  return {
    subscription,
    formatted:
      `Subscription: ${subscription.subscriptionNumber || subscription.id}\n` +
      `${'-'.repeat(40)}\n` +
      `Customer:       ${subscription.customerId}\n` +
      `Plan:           ${subscription.planName || subscription.planId || 'N/A'}\n` +
      `Status:         ${subscription.status}\n` +
      `Price:          ${subscription.price} ${subscription.currency}\n` +
      `Next billing:   ${subscription.nextBillingDate || 'N/A'}`,
  };
}

function formatBillingCycles(cycles, { output, jsonOutput }) {
  if (jsonOutput) return cycles;
  if (cycles.length === 0) return { formatted: 'No billing cycles found.' };
  const formatted = output.table(cycles, [
    { key: 'id', header: 'ID' },
    { key: 'cycleNumber', header: 'Cycle', align: 'right' },
    { key: 'status', header: 'Status' },
    { key: 'periodStart', header: 'Start' },
    { key: 'periodEnd', header: 'End' },
    { key: 'total', header: 'Total', align: 'right' },
  ]);
  return { cycles, formatted };
}

function formatEvents(events, { output, jsonOutput }) {
  if (jsonOutput) return events;
  if (events.length === 0) return { formatted: 'No subscription events found.' };
  const formatted = output.table(events, [
    { key: 'id', header: 'ID' },
    { key: 'eventType', header: 'Type' },
    { key: 'triggeredBy', header: 'Triggered By' },
    { key: 'createdAt', header: 'Created' },
  ]);
  return { events, formatted };
}

export const metadata = {
  name: 'subscriptions',
  aliases: ['subs', 'billing'],
  description: 'Subscription and billing commands',
  actions: {
    plans: { description: 'List subscription plans', args: ['[status]', '[billingInterval]'] },
    plan: { description: 'Get subscription plan', args: ['<id|code>'] },
    list: { description: 'List subscriptions', args: ['[customerId]', '[status]'] },
    get: { description: 'Get subscription', args: ['<id|number>'] },
    create: { description: 'Create subscription', args: ['<customerId>', '<planId>'] },
    pause: { description: 'Pause subscription', args: ['<subscriptionId>', '[reason]'] },
    resume: { description: 'Resume subscription', args: ['<subscriptionId>'] },
    cancel: { description: 'Cancel subscription', args: ['<subscriptionId>', '[immediate]'] },
    cycles: { description: 'List billing cycles', args: ['[subscriptionId]', '[status]'] },
    events: { description: 'List subscription events', args: ['<subscriptionId>', '[limit]'] },
  },
};

export default { execute, metadata };
