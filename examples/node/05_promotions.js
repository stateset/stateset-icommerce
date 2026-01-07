#!/usr/bin/env node
/**
 * StateSet iCommerce - Promotions & Discounts Example
 *
 * This example demonstrates the promotions system:
 * - Creating different promotion types (percentage, fixed, BOGO, free shipping)
 * - Managing coupon codes
 * - Applying promotions to carts
 * - Promotion validation and usage tracking
 *
 * Run with: node 05_promotions.js
 */

const { Commerce } = require('@stateset/embedded');

async function main() {
  console.log('=== StateSet iCommerce - Promotions & Discounts ===\n');

  const commerce = new Commerce(':memory:');

  // ============================================
  // Setup: Create products
  // ============================================

  console.log('[Setup] Creating products...');

  const laptop = await commerce.products.create({
    name: 'Pro Laptop',
    variants: [{ sku: 'LAPTOP-001', name: 'Standard', price: 999.99 }]
  });

  const mouse = await commerce.products.create({
    name: 'Wireless Mouse',
    variants: [{ sku: 'MOUSE-001', name: 'Standard', price: 49.99 }]
  });

  const keyboard = await commerce.products.create({
    name: 'Mechanical Keyboard',
    variants: [{ sku: 'KB-001', name: 'Standard', price: 129.99 }]
  });

  const headphones = await commerce.products.create({
    name: 'Wireless Headphones',
    variants: [{ sku: 'HP-001', name: 'Standard', price: 199.99 }]
  });

  console.log('    Products created\n');

  // ============================================
  // 1. Percentage Off Promotion
  // ============================================

  console.log('[1] Creating percentage off promotion...');

  const percentPromo = await commerce.promotions.create({
    name: 'Summer Sale 20% Off',
    description: 'Get 20% off your entire order',
    promotionType: 'percentage_off',
    trigger: 'automatic', // Applied automatically
    target: 'order',
    percentageOff: 0.20, // 20%
    maxDiscountAmount: 100.00, // Cap at $100
    startsAt: new Date().toISOString(),
    endsAt: new Date(Date.now() + 30 * 24 * 60 * 60 * 1000).toISOString(), // 30 days
    totalUsageLimit: 1000,
    perCustomerLimit: 3,
    currency: 'USD',
    priority: 10
  });

  console.log(`    Created: ${percentPromo.name}`);
  console.log(`      Code: ${percentPromo.code}`);
  console.log(`      Type: ${percentPromo.promotionType}`);
  console.log(`      Discount: ${(percentPromo.percentageOff * 100).toFixed(0)}% off (max $${percentPromo.maxDiscountAmount})`);
  console.log(`      Valid: ${percentPromo.startsAt} to ${percentPromo.endsAt}`);
  console.log(`      Limits: ${percentPromo.totalUsageLimit} total, ${percentPromo.perCustomerLimit} per customer\n`);

  // ============================================
  // 2. Fixed Amount Promotion
  // ============================================

  console.log('[2] Creating fixed amount promotion...');

  const fixedPromo = await commerce.promotions.create({
    name: '$50 Off Orders Over $200',
    description: 'Save $50 when you spend $200 or more',
    promotionType: 'fixed_amount_off',
    trigger: 'coupon_code', // Requires coupon
    target: 'order',
    fixedAmountOff: 50.00,
    currency: 'USD',
    priority: 20,
    stacking: 'exclusive' // Cannot combine with other promotions
  });

  console.log(`    Created: ${fixedPromo.name}`);
  console.log(`      Discount: $${fixedPromo.fixedAmountOff} off`);
  console.log(`      Stacking: ${fixedPromo.stacking}\n`);

  // ============================================
  // 3. Buy X Get Y (BOGO) Promotion
  // ============================================

  console.log('[3] Creating BOGO promotion...');

  const bogoPromo = await commerce.promotions.create({
    name: 'Buy 2 Get 1 Free',
    description: 'Buy 2 items, get the 3rd free',
    promotionType: 'buy_x_get_y',
    trigger: 'automatic',
    target: 'line_item',
    buyQuantity: 2,
    getQuantity: 1,
    getDiscountPercent: 1.0, // 100% off = free
    applicableSkus: ['MOUSE-001', 'KB-001'], // Only for these SKUs
    currency: 'USD',
    priority: 5
  });

  console.log(`    Created: ${bogoPromo.name}`);
  console.log(`      Buy: ${bogoPromo.buyQuantity}, Get: ${bogoPromo.getQuantity}`);
  console.log(`      Get discount: ${(bogoPromo.getDiscountPercent * 100).toFixed(0)}% off`);
  console.log(`      Applicable to: Mouse, Keyboard\n`);

  // ============================================
  // 4. Free Shipping Promotion
  // ============================================

  console.log('[4] Creating free shipping promotion...');

  const freeShipPromo = await commerce.promotions.create({
    name: 'Free Shipping Over $100',
    description: 'Free shipping on orders over $100',
    promotionType: 'free_shipping',
    trigger: 'automatic',
    target: 'shipping',
    currency: 'USD',
    priority: 1 // High priority (lower number = applied first)
  });

  console.log(`    Created: ${freeShipPromo.name}`);
  console.log(`      Type: ${freeShipPromo.promotionType}\n`);

  // ============================================
  // 5. Tiered Discount Promotion
  // ============================================

  console.log('[5] Creating tiered discount promotion...');

  const tieredPromo = await commerce.promotions.create({
    name: 'Spend More Save More',
    description: 'The more you spend, the more you save',
    promotionType: 'tiered_discount',
    trigger: 'automatic',
    target: 'order',
    tiers: JSON.stringify([
      { minAmount: 100, discountPercent: 0.05 },  // 5% off $100+
      { minAmount: 250, discountPercent: 0.10 },  // 10% off $250+
      { minAmount: 500, discountPercent: 0.15 }   // 15% off $500+
    ]),
    currency: 'USD',
    priority: 15
  });

  console.log(`    Created: ${tieredPromo.name}`);
  console.log(`      Tiers: 5% at $100+, 10% at $250+, 15% at $500+\n`);

  // ============================================
  // 6. Bundle Promotion
  // ============================================

  console.log('[6] Creating bundle promotion...');

  const bundlePromo = await commerce.promotions.create({
    name: 'Work From Home Bundle',
    description: 'Save 25% when you buy laptop + accessories',
    promotionType: 'bundle',
    trigger: 'automatic',
    target: 'product',
    bundleProductIds: [laptop.id, mouse.id, keyboard.id],
    bundleDiscount: 0.25, // 25% off the bundle
    currency: 'USD',
    priority: 8
  });

  console.log(`    Created: ${bundlePromo.name}`);
  console.log(`      Bundle discount: ${(bundlePromo.bundleDiscount * 100).toFixed(0)}%`);
  console.log(`      Products: Laptop, Mouse, Keyboard\n`);

  // ============================================
  // 7. Create Coupon Codes
  // ============================================

  console.log('[7] Creating coupon codes...');

  // Create coupon for the fixed amount promotion
  const coupon1 = await commerce.promotions.createCoupon({
    promotionId: fixedPromo.id,
    code: 'SAVE50',
    usageLimit: 500,
    perCustomerLimit: 1,
    startsAt: new Date().toISOString(),
    endsAt: new Date(Date.now() + 60 * 24 * 60 * 60 * 1000).toISOString() // 60 days
  });
  console.log(`    Created coupon: ${coupon1.code}`);
  console.log(`      Promotion: ${fixedPromo.name}`);
  console.log(`      Usage limit: ${coupon1.usageLimit}`);

  // Create additional coupon codes
  const coupon2 = await commerce.promotions.createCoupon({
    promotionId: fixedPromo.id,
    code: 'VIP50',
    usageLimit: 100,
    perCustomerLimit: 5, // VIPs get more uses
    metadata: JSON.stringify({ tier: 'vip' })
  });
  console.log(`    Created coupon: ${coupon2.code} (VIP)\n`);

  // ============================================
  // 8. List and Manage Promotions
  // ============================================

  console.log('[8] Managing promotions...');

  // List all promotions
  const allPromos = await commerce.promotions.list();
  console.log(`    Total promotions: ${allPromos.length}`);

  // Get active promotions
  const activePromos = await commerce.promotions.getActive();
  console.log(`    Active promotions: ${activePromos.length}`);

  // Check if promotion is valid
  const isValid = await commerce.promotions.isValid(percentPromo.id);
  console.log(`    ${percentPromo.name} is valid: ${isValid}`);

  // Get promotion by code
  const foundPromo = await commerce.promotions.getByCode(percentPromo.code);
  console.log(`    Found by code: ${foundPromo.name}`);

  // Update promotion
  const updatedPromo = await commerce.promotions.update(percentPromo.id, {
    name: 'Summer Sale 25% Off', // Update name
    percentageOff: 0.25 // Increase to 25%
  });
  console.log(`    Updated: ${updatedPromo.name} - now ${(updatedPromo.percentageOff * 100).toFixed(0)}% off\n`);

  // ============================================
  // 9. Validate Coupon Codes
  // ============================================

  console.log('[9] Validating coupon codes...');

  // Validate coupon
  const validCoupon = await commerce.promotions.validateCoupon('SAVE50');
  if (validCoupon) {
    console.log(`    Coupon SAVE50 is valid`);
    console.log(`      Usage: ${validCoupon.usageCount}/${validCoupon.usageLimit}`);
  }

  // Get coupon by code
  const couponByCode = await commerce.promotions.getCouponByCode('VIP50');
  console.log(`    Found coupon: ${couponByCode.code} (Status: ${couponByCode.status})`);

  // List coupons for promotion
  const promoCoupons = await commerce.promotions.listCoupons({
    promotionId: fixedPromo.id
  });
  console.log(`    Coupons for ${fixedPromo.name}: ${promoCoupons.length}\n`);

  // ============================================
  // 10. Apply Promotions to Cart
  // ============================================

  console.log('[10] Applying promotions to cart...');

  const customer = await commerce.customers.create({
    email: 'shopper@example.com',
    firstName: 'Test',
    lastName: 'Shopper'
  });

  // Simulate cart line items
  const lineItems = [
    {
      id: 'item1',
      productId: laptop.id,
      sku: 'LAPTOP-001',
      quantity: 1,
      unitPrice: 999.99,
      lineTotal: 999.99
    },
    {
      id: 'item2',
      productId: mouse.id,
      sku: 'MOUSE-001',
      quantity: 3, // Buy 2 get 1 free eligible
      unitPrice: 49.99,
      lineTotal: 149.97
    },
    {
      id: 'item3',
      productId: keyboard.id,
      sku: 'KB-001',
      quantity: 1,
      unitPrice: 129.99,
      lineTotal: 129.99
    }
  ];

  const subtotal = lineItems.reduce((sum, item) => sum + item.lineTotal, 0);

  // Apply automatic promotions
  const autoResult = await commerce.promotions.apply({
    customerId: customer.id,
    lineItems: lineItems,
    subtotal: subtotal,
    shippingAmount: 15.99,
    shippingCountry: 'US',
    shippingState: 'CA',
    currency: 'USD'
  });

  console.log('    Automatic promotions applied:');
  console.log(`      Original Subtotal: $${autoResult.originalSubtotal.toFixed(2)}`);
  console.log(`      Total Discount: $${autoResult.totalDiscount.toFixed(2)}`);
  console.log(`      Discounted Subtotal: $${autoResult.discountedSubtotal.toFixed(2)}`);
  console.log(`      Original Shipping: $${autoResult.originalShipping.toFixed(2)}`);
  console.log(`      Shipping Discount: $${autoResult.shippingDiscount.toFixed(2)}`);
  console.log(`      Final Shipping: $${autoResult.finalShipping.toFixed(2)}`);
  console.log(`      Grand Total: $${autoResult.grandTotal.toFixed(2)}`);

  console.log('\n    Applied promotions:');
  for (const promo of autoResult.appliedPromotions) {
    console.log(`      - ${promo.promotionName}`);
    console.log(`        Discount: $${promo.discountAmount.toFixed(2)} (${promo.discountType})`);
  }

  // Apply with coupon code
  console.log('\n    Applying with coupon code SAVE50...');
  const couponResult = await commerce.promotions.apply({
    customerId: customer.id,
    couponCodes: ['SAVE50'],
    lineItems: lineItems,
    subtotal: subtotal,
    shippingAmount: 15.99,
    currency: 'USD'
  });

  console.log(`      Additional savings: $${(couponResult.totalDiscount - autoResult.totalDiscount).toFixed(2)}`);
  console.log(`      New Grand Total: $${couponResult.grandTotal.toFixed(2)}\n`);

  // ============================================
  // 11. Record Promotion Usage
  // ============================================

  console.log('[11] Recording promotion usage...');

  const usage = await commerce.promotions.recordUsage(
    fixedPromo.id,
    coupon1.id,
    customer.id,
    'order_12345',
    null, // cartId
    50.00,
    'USD'
  );

  console.log(`    Usage recorded: ${usage.id}`);
  console.log(`      Promotion: ${usage.promotionId}`);
  console.log(`      Coupon: ${usage.couponId}`);
  console.log(`      Discount: $${usage.discountAmount}`);
  console.log(`      Order: ${usage.orderId}`);

  // Check updated usage count
  const updatedCoupon = await commerce.promotions.getCoupon(coupon1.id);
  console.log(`      Coupon usage: ${updatedCoupon.usageCount}/${updatedCoupon.usageLimit}\n`);

  // ============================================
  // 12. Deactivate and Delete Promotions
  // ============================================

  console.log('[12] Deactivating promotions...');

  // Deactivate a promotion (pause it)
  const deactivatedPromo = await commerce.promotions.deactivate(tieredPromo.id);
  console.log(`    Deactivated: ${deactivatedPromo.name}`);
  console.log(`      Status: ${deactivatedPromo.status}`);

  // Reactivate
  const reactivatedPromo = await commerce.promotions.activate(tieredPromo.id);
  console.log(`    Reactivated: ${reactivatedPromo.name}`);
  console.log(`      Status: ${reactivatedPromo.status}`);

  // Delete a promotion
  await commerce.promotions.delete(bundlePromo.id);
  console.log(`    Deleted: Work From Home Bundle`);

  // Final count
  const finalPromos = await commerce.promotions.list();
  console.log(`    Remaining promotions: ${finalPromos.length}`);

  console.log('\n=== Promotions & Discounts Example Complete ===');
}

main().catch(console.error);
