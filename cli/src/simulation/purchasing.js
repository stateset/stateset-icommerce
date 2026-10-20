import {
  compareDecimals,
  multiplyDecimals,
  subtractDecimals,
  clampAtZero,
  formatDecimal,
} from '../utils/exact-decimal.js';
import { SIMULATION_LIMITS } from './limits.js';

export function roundPurchaseQuantity(requested, minimum = 1, multiple = 1) {
  return Math.ceil(Math.max(requested, minimum) / multiple) * multiple;
}

// Accepts a validated product. The quantity is a bounded integer; every cost
// operand stays an exact decimal string. Constraints apply before native writes.
export function decideReplenishment(product, position, cash, cashReserve = '0.00', day = 0) {
  if (!Number.isInteger(day) || day < 0 || day > SIMULATION_LIMITS.maxPlanningDays)
    throw new Error('Purchase review day must be within the simulation horizon');
  if (!Number.isSafeInteger(position) || position < 0)
    throw new Error('Native inventory position must be a nonnegative safe integer');
  if (position > product.reorderPoint) return { kind: 'none' };
  const reviewIntervalDays = product.purchasing?.reviewIntervalDays ?? 1;
  if (day % reviewIntervalDays !== 0)
    return {
      kind: 'review-deferred',
      inventoryPosition: position,
      reviewIntervalDays,
      nextReviewDay: day + reviewIntervalDays - (day % reviewIntervalDays),
    };
  const {
    minimumOrderQuantity = 1,
    orderMultiple = 1,
    maximumInventoryPosition,
  } = product.purchasing || {};
  const requestedQuantity = product.targetStock - position;
  const quantity = roundPurchaseQuantity(requestedQuantity, minimumOrderQuantity, orderMultiple);
  if (
    !Number.isSafeInteger(quantity) ||
    quantity <= 0 ||
    quantity > SIMULATION_LIMITS.maxPurchaseQuantity
  )
    throw new Error('Rounded purchase quantity exceeds the simulation bound');
  const spend = multiplyDecimals(product.unitCost, String(quantity));
  const decision = {
    quantity,
    requestedQuantity,
    extraUnits: quantity - requestedQuantity,
    minimumOrderQuantity,
    orderMultiple,
    inventoryPosition: position,
    requiredPosition: position + quantity,
    spend,
  };
  if (
    maximumInventoryPosition !== undefined &&
    decision.requiredPosition > maximumInventoryPosition
  )
    return {
      ...decision,
      kind: 'constraint-blocked',
      reason: 'maximum_inventory_position',
      maximumInventoryPosition,
    };
  if (compareDecimals(spend, cash) > 0) return { ...decision, kind: 'budget-blocked' };
  const spendableCash = clampAtZero(subtractDecimals(cash, cashReserve));
  if (compareDecimals(spend, spendableCash) > 0)
    return {
      ...decision,
      kind: 'reserve-blocked',
      cashReserve: formatDecimal(cashReserve),
      spendableCash,
    };
  return { ...decision, kind: 'purchase' };
}
