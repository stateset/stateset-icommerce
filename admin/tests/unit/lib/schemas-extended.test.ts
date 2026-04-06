/**
 * Extended Tests for Zod Validation Schemas
 *
 * Covers schemas not yet tested: forgotPasswordSchema, resetPasswordSchema,
 * verifyEmailSchema, cancelSessionSchema, agentChatMessageSchema,
 * confirmActionSchema, billing schemas, integrationCredentialSchema,
 * autonomous session schemas, safeIdSchema, and password regex rules.
 *
 * @module tests/unit/lib/schemas-extended
 */

import { describe, it, expect } from 'vitest';
import {
  forgotPasswordSchema,
  resetPasswordSchema,
  verifyEmailSchema,
  cancelSessionSchema,
  agentChatMessageSchema,
  confirmActionSchema,
  createSubscriptionSchema,
  updateSubscriptionSchema,
  integrationCredentialSchema,
  createAutonomousSessionSchema,
  sessionActionSchema,
  safeIdSchema,
  loginSchema,
} from '@/lib/shared/schemas';

// ============================================================================
// Password regex rules (loginSchema edge cases)
// ============================================================================

describe('loginSchema password rules', () => {
  it('rejects password without uppercase letter', () => {
    const result = loginSchema.safeParse({
      email: 'user@test.com',
      password: 'alllowercase1',
    });
    expect(result.success).toBe(false);
  });

  it('rejects password without lowercase letter', () => {
    const result = loginSchema.safeParse({
      email: 'user@test.com',
      password: 'ALLUPPERCASE1',
    });
    expect(result.success).toBe(false);
  });

  it('rejects password without digit', () => {
    const result = loginSchema.safeParse({
      email: 'user@test.com',
      password: 'NoDigitsHere',
    });
    expect(result.success).toBe(false);
  });

  it('accepts password with all three: lowercase, uppercase, digit', () => {
    const result = loginSchema.safeParse({
      email: 'user@test.com',
      password: 'ValidPass1',
    });
    expect(result.success).toBe(true);
  });
});

// ============================================================================
// forgotPasswordSchema
// ============================================================================

describe('forgotPasswordSchema', () => {
  it('accepts a valid email', () => {
    const result = forgotPasswordSchema.safeParse({ email: 'user@example.com' });
    expect(result.success).toBe(true);
  });

  it('rejects invalid email', () => {
    const result = forgotPasswordSchema.safeParse({ email: 'not-email' });
    expect(result.success).toBe(false);
  });

  it('rejects missing email', () => {
    const result = forgotPasswordSchema.safeParse({});
    expect(result.success).toBe(false);
  });

  it('rejects empty string', () => {
    const result = forgotPasswordSchema.safeParse({ email: '' });
    expect(result.success).toBe(false);
  });
});

// ============================================================================
// resetPasswordSchema
// ============================================================================

describe('resetPasswordSchema', () => {
  it('accepts valid token and password', () => {
    const result = resetPasswordSchema.safeParse({
      token: 'reset-token-abc',
      password: 'NewPassword1',
    });
    expect(result.success).toBe(true);
  });

  it('rejects missing token', () => {
    const result = resetPasswordSchema.safeParse({ password: 'NewPassword1' });
    expect(result.success).toBe(false);
  });

  it('rejects empty token', () => {
    const result = resetPasswordSchema.safeParse({
      token: '',
      password: 'NewPassword1',
    });
    expect(result.success).toBe(false);
  });

  it('rejects weak password (no uppercase)', () => {
    const result = resetPasswordSchema.safeParse({
      token: 'valid-token',
      password: 'alllower1',
    });
    expect(result.success).toBe(false);
  });

  it('rejects short password', () => {
    const result = resetPasswordSchema.safeParse({
      token: 'valid-token',
      password: 'Sh1',
    });
    expect(result.success).toBe(false);
  });
});

// ============================================================================
// verifyEmailSchema
// ============================================================================

describe('verifyEmailSchema', () => {
  it('accepts a valid token', () => {
    const result = verifyEmailSchema.safeParse({ token: 'verify-abc-123' });
    expect(result.success).toBe(true);
  });

  it('rejects missing token', () => {
    const result = verifyEmailSchema.safeParse({});
    expect(result.success).toBe(false);
  });

  it('rejects empty token', () => {
    const result = verifyEmailSchema.safeParse({ token: '' });
    expect(result.success).toBe(false);
  });
});

// ============================================================================
// cancelSessionSchema
// ============================================================================

describe('cancelSessionSchema', () => {
  it('accepts action: cancel', () => {
    const result = cancelSessionSchema.safeParse({ action: 'cancel' });
    expect(result.success).toBe(true);
  });

  it('rejects action: start', () => {
    const result = cancelSessionSchema.safeParse({ action: 'start' });
    expect(result.success).toBe(false);
  });

  it('rejects missing action', () => {
    const result = cancelSessionSchema.safeParse({});
    expect(result.success).toBe(false);
  });
});

// ============================================================================
// agentChatMessageSchema
// ============================================================================

describe('agentChatMessageSchema', () => {
  it('accepts a valid message', () => {
    const result = agentChatMessageSchema.safeParse({ message: 'Hello agent' });
    expect(result.success).toBe(true);
  });

  it('accepts message with optional chatId', () => {
    const result = agentChatMessageSchema.safeParse({
      message: 'Hello',
      chatId: 'chat-1',
    });
    expect(result.success).toBe(true);
    if (result.success) expect(result.data.chatId).toBe('chat-1');
  });

  it('accepts message with optional context', () => {
    const result = agentChatMessageSchema.safeParse({
      message: 'Hello',
      context: { orderId: 'ord-123' },
    });
    expect(result.success).toBe(true);
  });

  it('rejects empty message', () => {
    const result = agentChatMessageSchema.safeParse({ message: '' });
    expect(result.success).toBe(false);
  });

  it('rejects message exceeding 10000 characters', () => {
    const result = agentChatMessageSchema.safeParse({
      message: 'x'.repeat(10001),
    });
    expect(result.success).toBe(false);
  });

  it('accepts message at exactly 10000 characters', () => {
    const result = agentChatMessageSchema.safeParse({
      message: 'x'.repeat(10000),
    });
    expect(result.success).toBe(true);
  });
});

// ============================================================================
// confirmActionSchema
// ============================================================================

describe('confirmActionSchema', () => {
  it('accepts valid confirmed=true', () => {
    const result = confirmActionSchema.safeParse({
      chatId: 'chat-abc',
      confirmed: true,
    });
    expect(result.success).toBe(true);
  });

  it('accepts valid confirmed=false', () => {
    const result = confirmActionSchema.safeParse({
      chatId: 'chat-abc',
      confirmed: false,
    });
    expect(result.success).toBe(true);
  });

  it('rejects missing chatId', () => {
    const result = confirmActionSchema.safeParse({ confirmed: true });
    expect(result.success).toBe(false);
  });

  it('rejects empty chatId', () => {
    const result = confirmActionSchema.safeParse({
      chatId: '',
      confirmed: true,
    });
    expect(result.success).toBe(false);
  });

  it('rejects missing confirmed', () => {
    const result = confirmActionSchema.safeParse({ chatId: 'chat-1' });
    expect(result.success).toBe(false);
  });
});

// ============================================================================
// createSubscriptionSchema
// ============================================================================

describe('createSubscriptionSchema', () => {
  it('accepts valid subscription data', () => {
    const result = createSubscriptionSchema.safeParse({
      planId: 'plan-pro',
      paymentMethodId: 'pm-stripe-123',
    });
    expect(result.success).toBe(true);
  });

  it('rejects missing planId', () => {
    const result = createSubscriptionSchema.safeParse({
      paymentMethodId: 'pm-123',
    });
    expect(result.success).toBe(false);
  });

  it('rejects missing paymentMethodId', () => {
    const result = createSubscriptionSchema.safeParse({ planId: 'plan-pro' });
    expect(result.success).toBe(false);
  });

  it('rejects empty planId', () => {
    const result = createSubscriptionSchema.safeParse({
      planId: '',
      paymentMethodId: 'pm-123',
    });
    expect(result.success).toBe(false);
  });
});

// ============================================================================
// updateSubscriptionSchema
// ============================================================================

describe('updateSubscriptionSchema', () => {
  it('accepts planId only', () => {
    const result = updateSubscriptionSchema.safeParse({ planId: 'plan-new' });
    expect(result.success).toBe(true);
  });

  it('accepts cancelAtPeriodEnd only', () => {
    const result = updateSubscriptionSchema.safeParse({
      cancelAtPeriodEnd: true,
    });
    expect(result.success).toBe(true);
  });

  it('accepts both fields', () => {
    const result = updateSubscriptionSchema.safeParse({
      planId: 'plan-new',
      cancelAtPeriodEnd: false,
    });
    expect(result.success).toBe(true);
  });

  it('accepts empty object (all optional)', () => {
    const result = updateSubscriptionSchema.safeParse({});
    expect(result.success).toBe(true);
  });
});

// ============================================================================
// integrationCredentialSchema
// ============================================================================

describe('integrationCredentialSchema', () => {
  it('accepts valid credentials', () => {
    const result = integrationCredentialSchema.safeParse({
      provider: 'shopify',
      credentials: { apiKey: 'sk-abc', secret: 'secret-123' },
    });
    expect(result.success).toBe(true);
  });

  it('rejects missing provider', () => {
    const result = integrationCredentialSchema.safeParse({
      credentials: { key: 'val' },
    });
    expect(result.success).toBe(false);
  });

  it('rejects empty provider', () => {
    const result = integrationCredentialSchema.safeParse({
      provider: '',
      credentials: { key: 'val' },
    });
    expect(result.success).toBe(false);
  });

  it('accepts optional name', () => {
    const result = integrationCredentialSchema.safeParse({
      provider: 'stripe',
      credentials: { key: 'val' },
      name: 'My Stripe Connection',
    });
    expect(result.success).toBe(true);
    if (result.success) expect(result.data.name).toBe('My Stripe Connection');
  });

  it('rejects name exceeding 200 characters', () => {
    const result = integrationCredentialSchema.safeParse({
      provider: 'stripe',
      credentials: { key: 'val' },
      name: 'X'.repeat(201),
    });
    expect(result.success).toBe(false);
  });
});

// ============================================================================
// createAutonomousSessionSchema
// ============================================================================

describe('createAutonomousSessionSchema', () => {
  it('accepts valid session with name only', () => {
    const result = createAutonomousSessionSchema.safeParse({
      name: 'My Session',
    });
    expect(result.success).toBe(true);
  });

  it('accepts optional description', () => {
    const result = createAutonomousSessionSchema.safeParse({
      name: 'Session',
      description: 'A test session for order processing',
    });
    expect(result.success).toBe(true);
  });

  it('rejects description exceeding 1000 characters', () => {
    const result = createAutonomousSessionSchema.safeParse({
      name: 'Session',
      description: 'X'.repeat(1001),
    });
    expect(result.success).toBe(false);
  });

  it('rejects empty name', () => {
    const result = createAutonomousSessionSchema.safeParse({ name: '' });
    expect(result.success).toBe(false);
  });

  it('rejects name exceeding 200 characters', () => {
    const result = createAutonomousSessionSchema.safeParse({
      name: 'N'.repeat(201),
    });
    expect(result.success).toBe(false);
  });

  it('accepts budgetConfig with all fields', () => {
    const result = createAutonomousSessionSchema.safeParse({
      name: 'Session',
      budgetConfig: {
        costCapCents: 5000,
        iterationLimit: 100,
        durationLimitSeconds: 3600,
      },
    });
    expect(result.success).toBe(true);
  });

  it('accepts budgetConfig with partial fields', () => {
    const result = createAutonomousSessionSchema.safeParse({
      name: 'Session',
      budgetConfig: { iterationLimit: 50 },
    });
    expect(result.success).toBe(true);
  });

  it('rejects negative costCapCents', () => {
    const result = createAutonomousSessionSchema.safeParse({
      name: 'Session',
      budgetConfig: { costCapCents: -1 },
    });
    expect(result.success).toBe(false);
  });

  it('rejects iterationLimit of 0', () => {
    const result = createAutonomousSessionSchema.safeParse({
      name: 'Session',
      budgetConfig: { iterationLimit: 0 },
    });
    expect(result.success).toBe(false);
  });
});

// ============================================================================
// sessionActionSchema
// ============================================================================

describe('sessionActionSchema', () => {
  it('accepts action: start', () => {
    const result = sessionActionSchema.safeParse({ action: 'start' });
    expect(result.success).toBe(true);
  });

  it('accepts action: pause', () => {
    const result = sessionActionSchema.safeParse({ action: 'pause' });
    expect(result.success).toBe(true);
  });

  it('accepts action: cancel', () => {
    const result = sessionActionSchema.safeParse({ action: 'cancel' });
    expect(result.success).toBe(true);
  });

  it('rejects unknown action', () => {
    const result = sessionActionSchema.safeParse({ action: 'restart' });
    expect(result.success).toBe(false);
  });

  it('rejects missing action', () => {
    const result = sessionActionSchema.safeParse({});
    expect(result.success).toBe(false);
  });
});

// ============================================================================
// safeIdSchema
// ============================================================================

describe('safeIdSchema', () => {
  it('accepts alphanumeric ID', () => {
    expect(safeIdSchema.safeParse('order123').success).toBe(true);
  });

  it('accepts ID with hyphens', () => {
    expect(safeIdSchema.safeParse('order-123-abc').success).toBe(true);
  });

  it('accepts ID with underscores', () => {
    expect(safeIdSchema.safeParse('order_123_abc').success).toBe(true);
  });

  it('accepts ID with dots', () => {
    expect(safeIdSchema.safeParse('v1.0.0').success).toBe(true);
  });

  it('rejects empty string', () => {
    expect(safeIdSchema.safeParse('').success).toBe(false);
  });

  it('rejects ID exceeding 200 characters', () => {
    expect(safeIdSchema.safeParse('a'.repeat(201)).success).toBe(false);
  });

  it('rejects ID with path traversal characters', () => {
    expect(safeIdSchema.safeParse('../etc/passwd').success).toBe(false);
  });

  it('rejects ID with spaces', () => {
    expect(safeIdSchema.safeParse('my order').success).toBe(false);
  });

  it('rejects ID with special characters', () => {
    expect(safeIdSchema.safeParse('order@#$').success).toBe(false);
  });

  it('rejects ID with slashes', () => {
    expect(safeIdSchema.safeParse('path/to/id').success).toBe(false);
  });
});
