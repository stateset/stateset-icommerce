/**
 * Shared Zod Schemas
 *
 * Validation schemas used across API routes for request body and query validation.
 */

import { z } from 'zod';

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
  password: z.string()
    .min(8, 'Password must be at least 8 characters')
    .regex(/[a-z]/, 'Password must contain at least one lowercase letter')
    .regex(/[A-Z]/, 'Password must contain at least one uppercase letter')
    .regex(/[0-9]/, 'Password must contain at least one digit'),
});

export const registerSchema = z.object({
  email: z.string().email('Invalid email address'),
  password: z.string()
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
  password: z.string()
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
export function validateBody<T>(
  body: unknown,
  schema: z.ZodType<T>
): ValidationResult<T> {
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
  schema: z.ZodType<T>
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
