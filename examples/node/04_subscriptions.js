#!/usr/bin/env node
/**
 * StateSet iCommerce - Subscriptions Example
 *
 * This example demonstrates subscription management:
 * - Creating subscription plans
 * - Subscribing customers to plans
 * - Managing subscription lifecycle (pause, resume, cancel)
 * - Billing cycles and events
 * - Trial periods and discounts
 *
 * Run with: node 04_subscriptions.js
 */

const { Commerce } = require('@stateset/embedded');

async function main() {
  console.log('=== StateSet iCommerce - Subscriptions ===\n');

  const commerce = new Commerce(':memory:');

  // ============================================
  // Setup: Create customers
  // ============================================

  console.log('[Setup] Creating customers...');

  const customer1 = await commerce.customers.create({
    email: 'subscriber1@example.com',
    firstName: 'John',
    lastName: 'Subscriber'
  });

  const customer2 = await commerce.customers.create({
    email: 'subscriber2@example.com',
    firstName: 'Jane',
    lastName: 'Member'
  });

  console.log('    Customers created\n');

  // ============================================
  // 1. Create Subscription Plans
  // ============================================

  console.log('[1] Creating subscription plans...');

  // Basic monthly plan
  const basicPlan = await commerce.subscriptions.createPlan({
    name: 'Basic Plan',
    description: 'Access to basic features',
    code: 'BASIC-MONTHLY',
    billingInterval: 'monthly',
    price: 9.99,
    currency: 'USD',
    trialDays: 14,
    trialRequiresPaymentMethod: false
  });
  console.log(`    Created: ${basicPlan.name} - $${basicPlan.price}/${basicPlan.billingInterval}`);
  console.log(`      Code: ${basicPlan.code}`);
  console.log(`      Trial: ${basicPlan.trialDays} days`);

  // Pro monthly plan with setup fee
  const proPlan = await commerce.subscriptions.createPlan({
    name: 'Pro Plan',
    description: 'Full access to all features',
    code: 'PRO-MONTHLY',
    billingInterval: 'monthly',
    price: 29.99,
    setupFee: 49.99,
    currency: 'USD',
    trialDays: 7,
    trialRequiresPaymentMethod: true,
    discountPercent: 0.10 // 10% discount
  });
  console.log(`    Created: ${proPlan.name} - $${proPlan.price}/${proPlan.billingInterval}`);
  console.log(`      Setup Fee: $${proPlan.setupFee}`);
  console.log(`      Discount: ${(proPlan.discountPercent * 100).toFixed(0)}%`);

  // Annual plan
  const annualPlan = await commerce.subscriptions.createPlan({
    name: 'Pro Annual',
    description: 'Full access, billed annually',
    code: 'PRO-ANNUAL',
    billingInterval: 'yearly',
    price: 299.99,
    currency: 'USD',
    trialDays: 30,
    minCycles: 1,
    maxCycles: null // No limit
  });
  console.log(`    Created: ${annualPlan.name} - $${annualPlan.price}/${annualPlan.billingInterval}`);

  // Custom interval plan (every 2 weeks)
  const customPlan = await commerce.subscriptions.createPlan({
    name: 'Bi-Weekly Delivery',
    description: 'Products delivered every 2 weeks',
    code: 'BIWEEKLY',
    billingInterval: 'custom',
    customIntervalDays: 14,
    price: 24.99,
    currency: 'USD'
  });
  console.log(`    Created: ${customPlan.name} - $${customPlan.price} every ${customPlan.customIntervalDays} days\n`);

  // ============================================
  // 2. List and Update Plans
  // ============================================

  console.log('[2] Managing plans...');

  // List all plans
  const allPlans = await commerce.subscriptions.listPlans();
  console.log(`    Total plans: ${allPlans.length}`);

  // Get plan by code
  const foundPlan = await commerce.subscriptions.getPlanByCode('PRO-MONTHLY');
  console.log(`    Found by code: ${foundPlan.name}`);

  // Update plan
  const updatedPlan = await commerce.subscriptions.updatePlan(basicPlan.id, {
    name: 'Basic Plan (Updated)',
    description: 'Updated description',
    trialDays: 21 // Extend trial
  });
  console.log(`    Updated: ${updatedPlan.name} - Trial now ${updatedPlan.trialDays} days`);

  // Activate plan (if draft)
  await commerce.subscriptions.activatePlan(basicPlan.id);
  console.log(`    Activated: ${basicPlan.name}\n`);

  // ============================================
  // 3. Subscribe Customers
  // ============================================

  console.log('[3] Creating subscriptions...');

  // Subscribe customer 1 to basic plan (with trial)
  const sub1 = await commerce.subscriptions.subscribe({
    customerId: customer1.id,
    planId: basicPlan.id,
    paymentMethodId: 'pm_card_visa'
  });
  console.log(`    ${customer1.firstName} subscribed to ${basicPlan.name}`);
  console.log(`      Subscription #: ${sub1.subscriptionNumber}`);
  console.log(`      Status: ${sub1.status}`);
  console.log(`      Trial ends: ${sub1.trialEndsAt}`);
  console.log(`      Next billing: ${sub1.nextBillingDate}`);

  // Subscribe customer 2 to pro plan (skip trial)
  const sub2 = await commerce.subscriptions.subscribe({
    customerId: customer2.id,
    planId: proPlan.id,
    paymentMethodId: 'pm_card_mastercard',
    skipTrial: true
  });
  console.log(`    ${customer2.firstName} subscribed to ${proPlan.name}`);
  console.log(`      Subscription #: ${sub2.subscriptionNumber}`);
  console.log(`      Status: ${sub2.status}`);
  console.log(`      Price: $${sub2.price} (includes ${(sub2.discountPercent * 100).toFixed(0)}% discount)`);

  // Subscribe with coupon code
  const sub3 = await commerce.subscriptions.subscribe({
    customerId: customer1.id,
    planId: annualPlan.id,
    paymentMethodId: 'pm_card_visa',
    couponCode: 'WELCOME20',
    skipTrial: true
  });
  console.log(`    ${customer1.firstName} also subscribed to ${annualPlan.name}`);
  console.log(`      Coupon: ${sub3.couponCode}\n`);

  // ============================================
  // 4. List Subscriptions
  // ============================================

  console.log('[4] Listing subscriptions...');

  // List all subscriptions
  const allSubs = await commerce.subscriptions.list();
  console.log(`    Total subscriptions: ${allSubs.length}`);

  // Filter by customer
  const customer1Subs = await commerce.subscriptions.list({
    customerId: customer1.id
  });
  console.log(`    ${customer1.firstName}'s subscriptions: ${customer1Subs.length}`);

  // Filter by plan
  const proSubs = await commerce.subscriptions.list({
    planId: proPlan.id
  });
  console.log(`    Pro Plan subscribers: ${proSubs.length}`);

  // Get subscription by number
  const foundSub = await commerce.subscriptions.getByNumber(sub1.subscriptionNumber);
  console.log(`    Found by number: ${foundSub.subscriptionNumber}\n`);

  // ============================================
  // 5. Pause Subscription
  // ============================================

  console.log('[5] Pausing subscription...');

  const pausedSub = await commerce.subscriptions.pause(sub1.id, {
    reason: 'Customer requested pause',
    resumeAt: new Date(Date.now() + 30 * 24 * 60 * 60 * 1000).toISOString() // Resume in 30 days
  });
  console.log(`    Paused: ${pausedSub.subscriptionNumber}`);
  console.log(`      Status: ${pausedSub.status}`);
  console.log(`      Paused at: ${pausedSub.pausedAt}`);
  console.log(`      Resume at: ${pausedSub.resumeAt}\n`);

  // ============================================
  // 6. Resume Subscription
  // ============================================

  console.log('[6] Resuming subscription...');

  const resumedSub = await commerce.subscriptions.resume(sub1.id);
  console.log(`    Resumed: ${resumedSub.subscriptionNumber}`);
  console.log(`      Status: ${resumedSub.status}`);
  console.log(`      Next billing: ${resumedSub.nextBillingDate}\n`);

  // ============================================
  // 7. Skip Billing Cycle
  // ============================================

  console.log('[7] Skipping billing cycle...');

  const skippedSub = await commerce.subscriptions.skipBilling(sub2.id, {
    reason: 'Customer vacation'
  });
  console.log(`    Skipped billing for: ${skippedSub.subscriptionNumber}`);
  console.log(`      Next billing: ${skippedSub.nextBillingDate}\n`);

  // ============================================
  // 8. Update Subscription
  // ============================================

  console.log('[8] Updating subscription...');

  const updatedSub = await commerce.subscriptions.update(sub2.id, {
    discountPercent: 0.15, // Increase discount to 15%
    price: 27.99 // Custom price
  });
  console.log(`    Updated: ${updatedSub.subscriptionNumber}`);
  console.log(`      New price: $${updatedSub.price}`);
  console.log(`      New discount: ${(updatedSub.discountPercent * 100).toFixed(0)}%\n`);

  // ============================================
  // 9. Billing Cycles
  // ============================================

  console.log('[9] Billing cycles...');

  // List billing cycles for a subscription
  const billingCycles = await commerce.subscriptions.listBillingCycles({
    subscriptionId: sub2.id
  });
  console.log(`    Billing cycles for ${sub2.subscriptionNumber}: ${billingCycles.length}`);

  if (billingCycles.length > 0) {
    const cycle = billingCycles[0];
    console.log(`      Cycle #${cycle.cycleNumber}:`);
    console.log(`        Status: ${cycle.status}`);
    console.log(`        Period: ${cycle.periodStart} to ${cycle.periodEnd}`);
    console.log(`        Total: $${cycle.total} ${cycle.currency}`);
  }
  console.log('');

  // ============================================
  // 10. Subscription Events
  // ============================================

  console.log('[10] Subscription events...');

  const events = await commerce.subscriptions.getEvents(sub2.id, 10);
  console.log(`    Events for ${sub2.subscriptionNumber}: ${events.length}`);

  for (const event of events.slice(0, 5)) {
    console.log(`      ${event.eventType}: ${event.description}`);
    console.log(`        At: ${event.createdAt}`);
  }
  console.log('');

  // ============================================
  // 11. Cancel Subscription
  // ============================================

  console.log('[11] Canceling subscription...');

  // Cancel at end of period
  const cancelledSub = await commerce.subscriptions.cancel(sub1.id, {
    reason: 'Customer requested cancellation',
    immediate: false, // Keep active until period ends
    feedback: 'Price too high'
  });
  console.log(`    Cancelled: ${cancelledSub.subscriptionNumber}`);
  console.log(`      Status: ${cancelledSub.status}`);
  console.log(`      Ends at: ${cancelledSub.endsAt}`);
  console.log(`      Cancelled at: ${cancelledSub.cancelledAt}`);

  // Immediate cancellation
  const immediateCancelSub = await commerce.subscriptions.cancel(sub3.id, {
    reason: 'Refund requested',
    immediate: true
  });
  console.log(`    Immediately cancelled: ${immediateCancelSub.subscriptionNumber}`);
  console.log(`      Status: ${immediateCancelSub.status}\n`);

  // ============================================
  // 12. Archive Plan
  // ============================================

  console.log('[12] Archiving plan...');

  const archivedPlan = await commerce.subscriptions.archivePlan(customPlan.id);
  console.log(`    Archived: ${archivedPlan.name}`);
  console.log(`      Status: ${archivedPlan.status}`);

  // List only active plans
  const activePlans = await commerce.subscriptions.listPlans({
    status: 'active'
  });
  console.log(`    Active plans remaining: ${activePlans.length}`);

  console.log('\n=== Subscriptions Example Complete ===');
}

main().catch(console.error);
