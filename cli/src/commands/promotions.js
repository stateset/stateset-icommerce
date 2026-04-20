/**
 * Promotions Commands Module
 */

async function getPromotionByIdentifier(commerce, identifier) {
  try {
    return await commerce.promotions().get(identifier);
  } catch {
    return commerce.promotions().getByCode(identifier);
  }
}

async function getCouponByIdentifier(commerce, identifier) {
  try {
    return await commerce.promotions().getCoupon(identifier);
  } catch {
    return commerce.promotions().getCouponByCode(identifier);
  }
}

export async function execute(action, args, { commerce, output, jsonOutput }) {
  switch (action) {
    case 'list': {
      const [status, type] = args;
      const filter = {};
      if (status) filter.status = status;
      if (type) filter.promotionType = type;
      const promotions = await commerce.promotions().list(filter);
      return formatPromotionList(promotions, { output, jsonOutput });
    }

    case 'get': {
      const identifier = args[0];
      if (!identifier) throw new Error('Usage: promotions get <id|code>');
      const promotion = await getPromotionByIdentifier(commerce, identifier);
      if (!promotion) throw new Error(`Promotion not found: ${identifier}`);
      return formatPromotionDetail(promotion, { jsonOutput });
    }

    case 'active': {
      const promotions = await commerce.promotions().getActive();
      return formatPromotionList(promotions, { output, jsonOutput });
    }

    case 'coupon': {
      const identifier = args[0];
      if (!identifier) throw new Error('Usage: promotions coupon <id|code>');
      const coupon = await getCouponByIdentifier(commerce, identifier);
      if (!coupon) throw new Error(`Coupon not found: ${identifier}`);
      return formatCouponDetail(coupon, { jsonOutput });
    }

    case 'validate': {
      const code = args[0];
      if (!code) throw new Error('Usage: promotions validate <code>');
      const coupon = await commerce.promotions().validateCoupon(code.toUpperCase());
      if (!coupon) {
        return jsonOutput
          ? { valid: false, code }
          : { formatted: `Coupon ${code} is invalid or expired.` };
      }
      return formatCouponDetail(coupon, { jsonOutput, prefix: 'Valid coupon' });
    }

    case 'coupons': {
      const [promotionId, status] = args;
      const filter = {};
      if (promotionId) filter.promotionId = promotionId;
      if (status) filter.status = status;
      const coupons = await commerce.promotions().listCoupons(filter);
      return formatCouponList(coupons, { output, jsonOutput });
    }

    case 'activate': {
      const promotionId = args[0];
      if (!promotionId) throw new Error('Usage: promotions activate <promotionId>');
      const promotion = await commerce.promotions().activate(promotionId);
      return {
        promotion,
        formatted: `Activated promotion ${promotion.name || promotion.id}`,
      };
    }

    case 'deactivate': {
      const promotionId = args[0];
      if (!promotionId) throw new Error('Usage: promotions deactivate <promotionId>');
      const promotion = await commerce.promotions().deactivate(promotionId);
      return {
        promotion,
        formatted: `Deactivated promotion ${promotion.name || promotion.id}`,
      };
    }

    case 'apply': {
      const cartId = args[0];
      if (!cartId) throw new Error('Usage: promotions apply <cartId>');
      const result = await commerce.applyCartPromotions(cartId);
      return formatApplyResult(cartId, result, { output, jsonOutput });
    }

    default:
      throw new Error(
        `Unknown action: promotions ${action}\n\n` +
          'Available actions:\n' +
          '  list [status] [type]      List promotions\n' +
          '  get <id|code>             Get promotion details\n' +
          '  active                    List active promotions\n' +
          '  coupon <id|code>          Get coupon details\n' +
          '  validate <code>           Validate coupon\n' +
          '  coupons [promotionId] [status]  List coupons\n' +
          '  activate <promotionId>    Activate promotion\n' +
          '  deactivate <promotionId>  Deactivate promotion\n' +
          '  apply <cartId>            Apply promotions to cart',
      );
  }
}

function formatPromotionList(promotions, { output, jsonOutput }) {
  if (jsonOutput) return promotions;
  if (promotions.length === 0) return { formatted: 'No promotions found.' };
  const formatted = output.table(
    promotions.map((promotion) => ({
      id: promotion.id,
      code: promotion.code || 'N/A',
      name: promotion.name,
      status: promotion.status,
      type: promotion.promotionType || promotion.type,
    })),
    [
      { key: 'id', header: 'ID' },
      { key: 'code', header: 'Code' },
      { key: 'name', header: 'Name' },
      { key: 'status', header: 'Status' },
      { key: 'type', header: 'Type' },
    ],
  );
  return { promotions, formatted };
}

function formatPromotionDetail(promotion, { jsonOutput }) {
  if (jsonOutput) return promotion;
  return {
    promotion,
    formatted:
      `Promotion: ${promotion.name}\n` +
      `${'-'.repeat(40)}\n` +
      `ID:          ${promotion.id}\n` +
      `Code:        ${promotion.code || 'N/A'}\n` +
      `Status:      ${promotion.status}\n` +
      `Type:        ${promotion.promotionType}\n` +
      `Trigger:     ${promotion.trigger}\n` +
      `Starts:      ${promotion.startsAt || 'N/A'}\n` +
      `Ends:        ${promotion.endsAt || 'N/A'}`,
  };
}

function formatCouponList(coupons, { output, jsonOutput }) {
  if (jsonOutput) return coupons;
  if (coupons.length === 0) return { formatted: 'No coupons found.' };
  const formatted = output.table(coupons, [
    { key: 'id', header: 'ID' },
    { key: 'code', header: 'Code' },
    { key: 'promotionId', header: 'Promotion' },
    { key: 'status', header: 'Status' },
    { key: 'usageCount', header: 'Used', align: 'right' },
    { key: 'usageLimit', header: 'Limit', align: 'right' },
  ]);
  return { coupons, formatted };
}

function formatCouponDetail(coupon, { jsonOutput, prefix = 'Coupon' }) {
  if (jsonOutput) return coupon;
  return {
    coupon,
    formatted:
      `${prefix}: ${coupon.code}\n` +
      `${'-'.repeat(32)}\n` +
      `ID:          ${coupon.id}\n` +
      `Promotion:   ${coupon.promotionId}\n` +
      `Status:      ${coupon.status}\n` +
      `Used:        ${coupon.usageCount}\n` +
      `Limit:       ${coupon.usageLimit ?? 'N/A'}`,
  };
}

function formatApplyResult(cartId, result, { output: _output, jsonOutput }) {
  if (jsonOutput) return { cartId, result };
  return {
    cartId,
    result,
    formatted:
      `Applied promotions to cart ${cartId}\n` +
      `${'-'.repeat(36)}\n` +
      `Original subtotal:   ${result.originalSubtotal}\n` +
      `Discount:            ${result.totalDiscount}\n` +
      `Discounted subtotal: ${result.discountedSubtotal}\n` +
      `Grand total:         ${result.grandTotal}`,
  };
}

export const metadata = {
  name: 'promotions',
  aliases: ['promo', 'discounts'],
  description: 'Promotions and coupon commands',
  actions: {
    list: { description: 'List promotions', args: ['[status]', '[type]'] },
    get: { description: 'Get promotion by ID or code', args: ['<id|code>'] },
    active: { description: 'List active promotions', args: [] },
    coupon: { description: 'Get coupon by ID or code', args: ['<id|code>'] },
    validate: { description: 'Validate coupon code', args: ['<code>'] },
    coupons: { description: 'List coupons', args: ['[promotionId]', '[status]'] },
    activate: { description: 'Activate promotion', args: ['<promotionId>'] },
    deactivate: { description: 'Deactivate promotion', args: ['<promotionId>'] },
    apply: { description: 'Apply promotions to cart', args: ['<cartId>'] },
  },
};

export default { execute, metadata };
