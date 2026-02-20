/**
 * Promotions & Discounts Tools Module
 */

import { z } from 'zod';

export const promotionTools = [
  {
    name: 'list_promotions',
    description:
      'List all promotions. Shows active, paused, and scheduled promotions with their discount details.',
    inputSchema: {
      status: z
        .enum(['active', 'paused', 'draft', 'expired', 'scheduled'])
        .optional()
        .describe('Filter by promotion status'),
      type: z
        .enum([
          'percentage_off',
          'fixed_amount_off',
          'buy_x_get_y',
          'free_shipping',
          'tiered_discount',
        ])
        .optional()
        .describe('Filter by promotion type'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const filter = {};
      if (params.status) filter.status = params.status;
      if (params.type) filter.promotionType = params.type;
      const promotions = await commerce.promotions().list(filter);
      return {
        success: true,
        count: promotions.length,
        promotions: promotions.map((p) => ({
          id: p.id,
          code: p.code,
          name: p.name,
          type: p.promotionType,
          status: p.status,
          trigger: p.trigger,
          percentageOff: p.percentageOff,
          fixedAmountOff: p.fixedAmountOff,
          startsAt: p.startsAt,
          endsAt: p.endsAt,
          usageCount: p.usageCount,
        })),
      };
    },
  },
  {
    name: 'get_promotion',
    description: 'Get a promotion by ID or internal code.',
    inputSchema: { identifier: z.string().describe('Promotion ID (UUID) or internal code') },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { identifier } = params;
      let promotion;
      try {
        promotion = await commerce.promotions().get(identifier);
      } catch (err) {
        console.debug(
          '[promotions] Promotion get by ID failed, trying code lookup:',
          err.message || err,
        );
        promotion = await commerce.promotions().getByCode(identifier);
      }
      if (!promotion) return { success: false, error: 'Promotion not found' };
      return {
        success: true,
        promotion: {
          id: promotion.id,
          code: promotion.code,
          name: promotion.name,
          description: promotion.description,
          type: promotion.promotionType,
          status: promotion.status,
          trigger: promotion.trigger,
          target: promotion.target,
          percentageOff: promotion.percentageOff,
          fixedAmountOff: promotion.fixedAmountOff,
          maxDiscount: promotion.maxDiscountAmount,
          startsAt: promotion.startsAt,
          endsAt: promotion.endsAt,
          usageCount: promotion.usageCount,
          usageLimit: promotion.totalUsageLimit,
          conditions: promotion.conditions,
        },
      };
    },
  },
  {
    name: 'create_promotion',
    description:
      'Create a new promotion. Supports percentage off, fixed amount off, BOGO, free shipping, and tiered discounts.',
    inputSchema: {
      name: z.string().describe('Promotion name (e.g., "Summer Sale")'),
      type: z
        .enum([
          'percentage_off',
          'fixed_amount_off',
          'buy_x_get_y',
          'free_shipping',
          'tiered_discount',
        ])
        .describe('Type of discount'),
      trigger: z
        .enum(['automatic', 'coupon_code', 'both'])
        .default('automatic')
        .describe('How the promotion is triggered'),
      percentageOff: z
        .number()
        .min(0)
        .max(1)
        .optional()
        .describe('Percentage discount (0.20 = 20% off)'),
      fixedAmountOff: z.number().optional().describe('Fixed amount discount in dollars'),
      maxDiscountAmount: z.number().optional().describe('Maximum discount cap'),
      description: z.string().optional().describe('Public description'),
      startsAt: z.string().optional().describe('Start date (ISO 8601)'),
      endsAt: z.string().optional().describe('End date (ISO 8601)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply)
        return {
          success: false,
          error: 'Create operation not allowed. The --apply flag must be set to create promotions.',
          hint: 'Run with --apply to enable write operations.',
          wouldCreate: params,
        };
      const typeMap = {
        percentage_off: 'PercentageOff',
        fixed_amount_off: 'FixedAmountOff',
        buy_x_get_y: 'BuyXGetY',
        free_shipping: 'FreeShipping',
        tiered_discount: 'TieredDiscount',
      };
      const triggerMap = { automatic: 'Automatic', coupon_code: 'CouponCode', both: 'Both' };
      const promotion = await commerce.promotions().create({
        name: params.name,
        description: params.description,
        promotionType: typeMap[params.type],
        trigger: triggerMap[params.trigger],
        target: 'Order',
        stacking: 'Stackable',
        percentageOff: params.percentageOff,
        fixedAmountOff: params.fixedAmountOff,
        maxDiscountAmount: params.maxDiscountAmount,
        startsAt: params.startsAt ? new Date(params.startsAt) : null,
        endsAt: params.endsAt ? new Date(params.endsAt) : null,
        priority: 1,
      });
      return {
        success: true,
        message: 'Promotion created successfully (status: draft)',
        hint: 'Use activate_promotion to make it live',
        promotion: {
          id: promotion.id,
          code: promotion.code,
          name: promotion.name,
          type: promotion.promotionType,
          status: promotion.status,
        },
      };
    },
  },
  {
    name: 'activate_promotion',
    description: 'Activate a promotion to make it available for use.',
    inputSchema: { promotionId: z.string().describe('Promotion ID to activate') },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { promotionId } = params;
      if (!allowApply)
        return {
          success: false,
          error: 'Activate operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldActivate: promotionId,
        };
      const promotion = await commerce.promotions().activate(promotionId);
      return {
        success: true,
        message: 'Promotion activated',
        promotion: { id: promotion.id, name: promotion.name, status: promotion.status },
      };
    },
  },
  {
    name: 'deactivate_promotion',
    description: 'Pause/deactivate a promotion.',
    inputSchema: { promotionId: z.string().describe('Promotion ID to deactivate') },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { promotionId } = params;
      if (!allowApply)
        return {
          success: false,
          error: 'Deactivate operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldDeactivate: promotionId,
        };
      const promotion = await commerce.promotions().deactivate(promotionId);
      return {
        success: true,
        message: 'Promotion deactivated',
        promotion: { id: promotion.id, name: promotion.name, status: promotion.status },
      };
    },
  },
  {
    name: 'create_coupon',
    description: 'Create a coupon code for a promotion.',
    inputSchema: {
      promotionId: z.string().describe('Promotion ID to create coupon for'),
      code: z.string().describe('Coupon code (e.g., "SUMMER25")'),
      usageLimit: z.number().optional().describe('Maximum number of times this coupon can be used'),
      perCustomerLimit: z.number().optional().describe('Max uses per customer'),
      startsAt: z.string().optional().describe('Coupon valid from (ISO 8601)'),
      endsAt: z.string().optional().describe('Coupon valid until (ISO 8601)'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply)
        return {
          success: false,
          error: 'Create operation not allowed. The --apply flag must be set to create coupons.',
          hint: 'Run with --apply to enable write operations.',
          wouldCreate: params,
        };
      const coupon = await commerce.promotions().createCoupon({
        promotionId: params.promotionId,
        code: params.code.toUpperCase(),
        usageLimit: params.usageLimit,
        perCustomerLimit: params.perCustomerLimit,
        startsAt: params.startsAt ? new Date(params.startsAt) : null,
        endsAt: params.endsAt ? new Date(params.endsAt) : null,
      });
      return {
        success: true,
        message: 'Coupon code created',
        coupon: {
          id: coupon.id,
          code: coupon.code,
          promotionId: coupon.promotionId,
          usageLimit: coupon.usageLimit,
          usageCount: coupon.usageCount,
          status: coupon.status,
        },
      };
    },
  },
  {
    name: 'validate_coupon',
    description: 'Check if a coupon code is valid and can be used.',
    inputSchema: { code: z.string().describe('Coupon code to validate') },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { code } = params;
      const coupon = await commerce.promotions().validateCoupon(code.toUpperCase());
      if (!coupon)
        return { success: true, valid: false, message: 'Invalid or expired coupon code' };
      const promotion = await commerce.promotions().get(coupon.promotionId);
      return {
        success: true,
        valid: true,
        coupon: {
          code: coupon.code,
          promotionName: promotion?.name,
          discountType: promotion?.promotionType,
          percentageOff: promotion?.percentageOff,
          fixedAmountOff: promotion?.fixedAmountOff,
          usageRemaining: coupon.usageLimit ? coupon.usageLimit - coupon.usageCount : 'unlimited',
        },
      };
    },
  },
  {
    name: 'list_coupons',
    description: 'List coupon codes with optional filters.',
    inputSchema: {
      promotionId: z.string().optional().describe('Filter by promotion ID'),
      status: z
        .enum(['active', 'expired', 'depleted', 'disabled'])
        .optional()
        .describe('Filter by status'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const filter = {};
      if (params.promotionId) filter.promotionId = params.promotionId;
      if (params.status) filter.status = params.status;
      const coupons = await commerce.promotions().listCoupons(filter);
      return {
        success: true,
        count: coupons.length,
        coupons: coupons.map((c) => ({
          id: c.id,
          code: c.code,
          promotionId: c.promotionId,
          status: c.status,
          usageCount: c.usageCount,
          usageLimit: c.usageLimit,
          startsAt: c.startsAt,
          endsAt: c.endsAt,
        })),
      };
    },
  },
  {
    name: 'get_active_promotions',
    description: 'Get all currently active promotions.',
    inputSchema: {},
    permission: 'read',
    handler: async ({ commerce }) => {
      const promotions = await commerce.promotions().getActive();
      return {
        success: true,
        count: promotions.length,
        promotions: promotions.map((p) => ({
          id: p.id,
          name: p.name,
          code: p.code,
          type: p.promotionType,
          trigger: p.trigger,
          percentageOff: p.percentageOff,
          fixedAmountOff: p.fixedAmountOff,
          endsAt: p.endsAt,
        })),
      };
    },
  },
  {
    name: 'apply_cart_promotions',
    description:
      'Calculate and apply all applicable promotions to a cart. Uses coupon codes on the cart and automatic promotions.',
    inputSchema: { cartId: z.string().describe('Cart ID to apply promotions to') },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      const { cartId } = params;
      if (!allowApply)
        return {
          success: false,
          error: 'Apply operation not allowed. The --apply flag must be set.',
          hint: 'Run with --apply to enable write operations.',
          wouldApplyTo: cartId,
        };
      const result = await commerce.applyCartPromotions(cartId);
      return {
        success: true,
        cartId,
        originalSubtotal: result.originalSubtotal,
        totalDiscount: result.totalDiscount,
        discountedSubtotal: result.discountedSubtotal,
        shippingDiscount: result.shippingDiscount,
        grandTotal: result.grandTotal,
        appliedPromotions: result.appliedPromotions.map((p) => ({
          name: p.promotionName,
          type: p.discountType,
          discountAmount: p.discountAmount,
          description: p.description,
          couponCode: p.couponCode,
        })),
        rejectedPromotions:
          result.rejectedPromotions?.map((p) => ({
            name: p.promotionName,
            reason: p.rejectionReason,
          })) || [],
      };
    },
  },
];

export default promotionTools;
