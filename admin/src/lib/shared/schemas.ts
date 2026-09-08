/**
 * Shared Zod Schemas
 *
 * Validation schemas used across API routes for request body and query validation.
 */

import { z } from 'zod';

import { ValidationError } from './errors';

// ============================================================================
// Pagination
// ============================================================================

export const paginationQuerySchema = z.object({
  limit: z.coerce.number().int().min(1).max(100).default(20),
  offset: z.coerce.number().int().min(0).default(0),
});

export type PaginationQuery = z.infer<typeof paginationQuerySchema>;

// ============================================================================
// Auth Schemas
// ============================================================================

export const loginSchema = z.object({
  email: z.string().email('Invalid email address'),
  password: z
    .string()
    .min(8, 'Password must be at least 8 characters')
    .regex(/[a-z]/, 'Password must contain at least one lowercase letter')
    .regex(/[A-Z]/, 'Password must contain at least one uppercase letter')
    .regex(/[0-9]/, 'Password must contain at least one digit'),
});

export const registerSchema = z.object({
  email: z.string().email('Invalid email address'),
  password: z
    .string()
    .min(8, 'Password must be at least 8 characters')
    .regex(/[a-z]/, 'Password must contain at least one lowercase letter')
    .regex(/[A-Z]/, 'Password must contain at least one uppercase letter')
    .regex(/[0-9]/, 'Password must contain at least one digit'),
  firstName: z.string().min(1, 'First name is required').max(100),
  lastName: z.string().min(1, 'Last name is required').max(100),
  orgName: z.string().min(1, 'Organization name is required').max(200).optional(),
});

export const forgotPasswordSchema = z.object({
  email: z.string().email('Invalid email address'),
});

export const resetPasswordSchema = z.object({
  token: z.string().min(1, 'Token is required'),
  password: z
    .string()
    .min(8, 'Password must be at least 8 characters')
    .regex(/[a-z]/, 'Password must contain at least one lowercase letter')
    .regex(/[A-Z]/, 'Password must contain at least one uppercase letter')
    .regex(/[0-9]/, 'Password must contain at least one digit'),
});

export const verifyEmailSchema = z.object({
  token: z.string().min(1, 'Verification token is required'),
});

// ============================================================================
// Session Schemas
// ============================================================================

export const sessionStatusSchema = z.enum([
  'pending',
  'running',
  'rotating',
  'paused',
  'completed',
  'failed',
  'cancelled',
]);

export const listSessionsSchema = paginationQuerySchema.extend({
  status: sessionStatusSchema.optional(),
  org_id: z.string().optional(),
  search: z.string().max(200).optional(),
});

export const cancelSessionSchema = z.object({
  action: z.literal('cancel'),
});

// ============================================================================
// Agent Chat Schemas
// ============================================================================

export const agentChatMessageSchema = z.object({
  message: z.string().min(1, 'Message is required').max(10000),
  chatId: z.string().optional(),
  context: z.record(z.unknown()).optional(),
});

export const confirmActionSchema = z.object({
  chatId: z.string().min(1),
  confirmed: z.boolean(),
});

// ============================================================================
// Billing Schemas
// ============================================================================

export const createSubscriptionSchema = z.object({
  planId: z.string().min(1, 'Plan ID is required'),
  paymentMethodId: z.string().min(1, 'Payment method is required'),
});

export const updateSubscriptionSchema = z.object({
  planId: z.string().optional(),
  cancelAtPeriodEnd: z.boolean().optional(),
});

// ============================================================================
// Integration Schemas
// ============================================================================

export const integrationCredentialSchema = z.object({
  provider: z.string().min(1, 'Provider is required'),
  credentials: z.record(z.string()),
  name: z.string().min(1).max(200).optional(),
});

// ============================================================================
// Autonomous Session Schemas
// ============================================================================

export const createAutonomousSessionSchema = z.object({
  name: z.string().min(1, 'Session name is required').max(200),
  description: z.string().max(1000).optional(),
  budgetConfig: z
    .object({
      costCapCents: z.number().int().min(0).optional(),
      iterationLimit: z.number().int().min(1).optional(),
      durationLimitSeconds: z.number().int().min(1).optional(),
    })
    .optional(),
});

export const sessionActionSchema = z.object({
  action: z.enum(['start', 'pause', 'cancel']),
});

// ============================================================================
// Path Parameter Schemas
// ============================================================================

/**
 * Safe ID schema — alphanumeric, hyphens, underscores, dots only.
 * Prevents path traversal and injection attacks in URL-interpolated IDs.
 */
export const safeIdSchema = z
  .string()
  .min(1, 'ID is required')
  .max(200, 'ID too long')
  .regex(/^[a-zA-Z0-9_.-]+$/, 'ID contains invalid characters');

// ============================================================================
// Validation Helpers
// ============================================================================

export type ValidationResult<T> =
  | { success: true; data: T }
  | { success: false; errors: Array<{ field: string; message: string }> };

/**
 * Validate request body against a Zod schema.
 */
export function validateBody<T>(body: unknown, schema: z.ZodType<T>): ValidationResult<T> {
  const result = schema.safeParse(body);
  if (result.success) {
    return { success: true, data: result.data };
  }

  const errors = result.error.issues.map((issue) => ({
    field: issue.path.join('.') || 'body',
    message: issue.message,
  }));

  return { success: false, errors };
}

/**
 * Validate query parameters against a Zod schema.
 */
export function validateQuery<T>(
  params: URLSearchParams,
  schema: z.ZodType<T>,
): ValidationResult<T> {
  const raw: Record<string, string> = {};
  params.forEach((value, key) => {
    raw[key] = value;
  });

  const result = schema.safeParse(raw);
  if (result.success) {
    return { success: true, data: result.data };
  }

  const errors = result.error.issues.map((issue) => ({
    field: issue.path.join('.') || 'query',
    message: issue.message,
  }));

  return { success: false, errors };
}

// ============================================================================
// Money primitives
// ============================================================================

/**
 * Exact-decimal money as a string. This is the primitive to reach for on any
 * new money field: a JSON number is an IEEE-754 double, so `0.1 + 0.2` and
 * `10.999999999999998` are representable amounts and a cent goes missing
 * somewhere downstream. A string keeps the operator's digits exactly as typed
 * until the engine parses them as a decimal.
 *
 * Accepts an optional leading `-` (refunds and credit memos are negative),
 * 1–15 integer digits, and up to 6 fractional digits (enough for USDC).
 */
export const decimalAmountSchema = z
  .string()
  .regex(
    /^-?\d{1,15}(\.\d{1,6})?$/,
    'Amount must be an exact decimal string (e.g. "10.99"), not a float',
  );

/** Non-negative integer count of a currency's minor units (cents, satoshi, …). */
export const minorUnitsSchema = z
  .number()
  .int('Amount must be an integer number of minor units')
  .min(0, 'Amount cannot be negative')
  .max(Number.MAX_SAFE_INTEGER, 'Amount exceeds the safe integer range');

/** ISO-4217-style code, plus room for stablecoin tickers like USDC. */
export const currencyCodeSchema = z
  .string()
  .regex(/^[A-Z]{3,5}$/, 'Currency must be a 3–5 letter uppercase code');

/**
 * Money on a field the embedded engine already types as `number`
 * (`Product.price`, `Subscription.totalAmount`, …). We cannot change those
 * signatures from here, so instead of accepting any double we require the
 * value's decimal form to be a well-formed, non-negative amount with at most
 * two fractional digits. That rejects `NaN`, `Infinity`, `1e21`, negative
 * prices, and float artefacts like `10.999999999999998` — the values that
 * silently corrupt a ledger — while leaving `10.99` alone.
 *
 * Prefer `decimalAmountSchema` for anything new.
 */
export const moneyNumberSchema = z
  .number()
  .refine((n) => Number.isFinite(n), 'Amount must be a finite number')
  .refine((n) => n >= 0, 'Amount cannot be negative')
  .refine(
    (n) => /^\d+(\.\d{1,2})?$/.test(String(n)),
    'Amount carries more precision than a currency minor unit allows',
  );

export const moneySchema = z.object({
  amount: decimalAmountSchema,
  currency: currencyCodeSchema,
});

// ============================================================================
// Commerce primitives
// ============================================================================

/** Stock-keeping unit. Same path-safety rule as `safeIdSchema`, plus `/`. */
export const skuSchema = z
  .string()
  .min(1, 'SKU is required')
  .max(100, 'SKU too long')
  .regex(/^[a-zA-Z0-9_./-]+$/, 'SKU contains invalid characters');

/** A whole number of units, at least one. Line items, reservations, releases. */
export const quantitySchema = z
  .number()
  .int('Quantity must be a whole number of units')
  .positive('Quantity must be greater than zero')
  .max(1_000_000, 'Quantity exceeds the per-line maximum');

/**
 * A signed stock adjustment. Negative is legal — that is a shrinkage or
 * write-off — but zero is not (it is a no-op that still writes an audit row),
 * and neither is a fraction: the engine counts discrete units.
 */
export const stockDeltaSchema = z
  .number()
  .int('Adjustment must be a whole number of units')
  .refine((n) => n !== 0, 'Adjustment must be non-zero')
  .refine((n) => Math.abs(n) <= 1_000_000, 'Adjustment exceeds the maximum');

export const emailSchema = z.string().email('Invalid email address');

/** Free-text operator input: a reason, a note. Bounded so it cannot be a payload. */
export const reasonSchema = z.string().min(1, 'Reason is required').max(1000, 'Reason too long');

export const addressSchema = z.object({
  line1: z.string().min(1, 'Address line 1 is required').max(200),
  line2: z.string().max(200).optional(),
  city: z.string().min(1, 'City is required').max(120),
  state: z.string().min(1, 'State is required').max(120),
  postalCode: z.string().min(1, 'Postal code is required').max(32),
  country: z.string().min(1, 'Country is required').max(60),
});

export const orderStatusSchema = z.enum([
  'pending',
  'confirmed',
  'processing',
  'shipped',
  'delivered',
  'cancelled',
]);

export const returnReasonCategorySchema = z.enum([
  'defective',
  'wrong_item',
  'not_as_described',
  'changed_mind',
  'other',
]);

export const refundMethodSchema = z.enum(['original', 'store_credit', 'exchange']);

export const returnItemConditionSchema = z.enum(['new', 'opened', 'damaged', 'used']);

export const productStatusSchema = z.enum(['active', 'draft', 'archived']);

export const subscriptionStatusSchema = z.enum(['active', 'paused', 'cancelled', 'expired']);

export const subscriptionFrequencySchema = z.enum([
  'weekly',
  'biweekly',
  'monthly',
  'quarterly',
  'annually',
]);

// ============================================================================
// Commerce server-action argument schemas
// ============================================================================
//
// Server actions are a public, unauthenticated-by-default RPC surface: Next
// exposes each one at a stable action id and the browser can post arbitrary
// JSON to it. TypeScript parameter types are erased at that boundary, so the
// only thing standing between a hostile payload and the embedded engine is a
// runtime schema. Each action validates its arguments as one object so the
// resulting `ValidationError.details` name the offending field.
//
// `Partial<T>` params use `.passthrough()`: those actions forward whatever
// the caller sends, and a stripping schema would silently *drop* unknown
// fields instead of rejecting them — a much worse failure than letting an
// unrecognized key through to the engine, which ignores it.

export const createOrderArgsSchema = z.object({
  customerId: safeIdSchema,
  items: z
    .array(z.object({ productId: safeIdSchema, quantity: quantitySchema }))
    .min(1, 'An order needs at least one line item')
    .max(500, 'Too many line items'),
  shippingAddress: addressSchema.optional(),
  billingAddress: addressSchema.optional(),
});

export const updateOrderStatusArgsSchema = z.object({
  orderId: safeIdSchema,
  status: orderStatusSchema,
});

export const cancelOrderArgsSchema = z.object({
  orderId: safeIdSchema,
  reason: reasonSchema.optional(),
});

export const adjustInventoryArgsSchema = z.object({
  sku: skuSchema,
  quantity: stockDeltaSchema,
  reason: reasonSchema.optional(),
});

export const inventoryMovementArgsSchema = z.object({
  sku: skuSchema,
  quantity: quantitySchema,
  orderId: safeIdSchema,
});

export const createReturnArgsSchema = z.object({
  orderId: safeIdSchema,
  items: z
    .array(
      z.object({
        productId: safeIdSchema,
        quantity: quantitySchema,
        reason: reasonSchema.optional(),
      }),
    )
    .min(1, 'A return needs at least one line item')
    .max(500, 'Too many line items'),
  reason: reasonSchema,
  reasonCategory: returnReasonCategorySchema,
});

export const returnIdArgsSchema = z.object({ returnId: safeIdSchema });

export const rejectReturnArgsSchema = z.object({
  returnId: safeIdSchema,
  reason: reasonSchema,
});

export const receiveReturnArgsSchema = z.object({
  returnId: safeIdSchema,
  items: z
    .array(z.object({ productId: safeIdSchema, condition: returnItemConditionSchema }))
    .min(1, 'Receiving a return needs at least one line item')
    .max(500, 'Too many line items'),
});

export const processRefundArgsSchema = z.object({
  returnId: safeIdSchema,
  method: refundMethodSchema.optional(),
});

/** Shared shape for `Partial<Customer>` create/update payloads. */
const customerFieldsSchema = z
  .object({
    id: safeIdSchema.optional(),
    email: emailSchema.optional(),
    firstName: z.string().max(100).optional(),
    lastName: z.string().max(100).optional(),
    phone: z.string().max(40).optional(),
    defaultAddress: addressSchema.optional(),
    addresses: z.array(addressSchema).max(50).optional(),
    tags: z.array(z.string().max(60)).max(100).optional(),
    totalSpent: moneyNumberSchema.optional(),
    averageOrderValue: moneyNumberSchema.optional(),
  })
  .passthrough();

export const createCustomerArgsSchema = z.object({
  params: customerFieldsSchema.refine(
    (p) => typeof p.email === 'string',
    'A new customer needs an email address',
  ),
});

export const updateCustomerArgsSchema = z.object({
  customerId: safeIdSchema,
  params: customerFieldsSchema,
});

export const createSubscriptionArgsSchema = z.object({
  params: z
    .object({
      id: safeIdSchema.optional(),
      customerId: safeIdSchema.optional(),
      status: subscriptionStatusSchema.optional(),
      plan: z.string().min(1).max(200).optional(),
      planId: safeIdSchema.optional(),
      frequency: subscriptionFrequencySchema.optional(),
      quantity: quantitySchema.optional(),
      totalAmount: moneyNumberSchema.optional(),
      items: z
        .array(z.object({ productId: safeIdSchema, quantity: quantitySchema }))
        .max(500)
        .optional(),
    })
    .passthrough(),
});

export const subscriptionIdArgsSchema = z.object({ subscriptionId: safeIdSchema });

export const cancelSubscriptionArgsSchema = z.object({
  subscriptionId: safeIdSchema,
  reason: reasonSchema.optional(),
});

/** Shared shape for `Partial<Product>` create/update payloads. */
const productFieldsSchema = z
  .object({
    id: safeIdSchema.optional(),
    sku: skuSchema.optional(),
    name: z.string().min(1).max(300).optional(),
    description: z.string().max(10000).optional(),
    price: moneyNumberSchema.optional(),
    compareAtPrice: moneyNumberSchema.optional(),
    costPrice: moneyNumberSchema.optional(),
    currency: currencyCodeSchema.optional(),
    category: z.string().max(200).optional(),
    tags: z.array(z.string().max(60)).max(100).optional(),
    status: productStatusSchema.optional(),
    images: z.array(z.string().max(2000)).max(100).optional(),
  })
  .passthrough();

export const createProductArgsSchema = z.object({ params: productFieldsSchema });

export const updateProductArgsSchema = z.object({
  productId: safeIdSchema,
  params: productFieldsSchema,
});

export const productIdArgsSchema = z.object({ productId: safeIdSchema });

// ============================================================================
// Server-action validation helper
// ============================================================================

/**
 * Validate a server action's arguments, throwing on failure.
 *
 * `validateBody` returns a result object because an API route turns it into
 * a 4xx response. A server action has no response object to shape — it throws,
 * and `<ErrorBoundary />` / the caller's try-catch renders the message. So
 * this wrapper raises the same `ValidationError` the API layer produces,
 * carrying per-field `details`, and never returns invalid data.
 */
export function validateArgs<T>(args: unknown, schema: z.ZodType<T>): T {
  const result = validateBody(args, schema);
  if (!result.success) {
    throw new ValidationError(result.errors);
  }
  return result.data;
}
