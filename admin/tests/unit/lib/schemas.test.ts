/**
 * Tests for Zod Validation Schemas
 *
 * @module tests/unit/lib/schemas
 */

import { describe, it, expect } from 'vitest';
import {
  paginationQuerySchema,
  loginSchema,
  registerSchema,
  listSessionsSchema,
  validateBody,
  validateQuery,
} from '@/lib/shared/schemas';

describe('paginationQuerySchema', () => {
  it('accepts valid pagination params', () => {
    const result = paginationQuerySchema.safeParse({ limit: 10, offset: 0 });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.limit).toBe(10);
      expect(result.data.offset).toBe(0);
    }
  });

  it('applies default limit and offset', () => {
    const result = paginationQuerySchema.safeParse({});

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.limit).toBe(20);
      expect(result.data.offset).toBe(0);
    }
  });

  it('coerces string values to numbers', () => {
    const result = paginationQuerySchema.safeParse({ limit: '50', offset: '10' });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.limit).toBe(50);
      expect(result.data.offset).toBe(10);
    }
  });

  it('rejects limit less than 1', () => {
    const result = paginationQuerySchema.safeParse({ limit: 0 });

    expect(result.success).toBe(false);
  });

  it('rejects limit greater than 100', () => {
    const result = paginationQuerySchema.safeParse({ limit: 101 });

    expect(result.success).toBe(false);
  });

  it('rejects negative offset', () => {
    const result = paginationQuerySchema.safeParse({ offset: -1 });

    expect(result.success).toBe(false);
  });

  it('rejects non-integer limit', () => {
    const result = paginationQuerySchema.safeParse({ limit: 10.5 });

    expect(result.success).toBe(false);
  });
});

describe('loginSchema', () => {
  it('accepts valid login credentials', () => {
    const result = loginSchema.safeParse({
      email: 'user@example.com',
      password: 'Secure1pass',
    });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.email).toBe('user@example.com');
      expect(result.data.password).toBe('Secure1pass');
    }
  });

  it('rejects invalid email', () => {
    const result = loginSchema.safeParse({
      email: 'not-an-email',
      password: 'Secure1pass',
    });

    expect(result.success).toBe(false);
    if (!result.success) {
      const emailIssue = result.error.issues.find((i) =>
        i.path.includes('email')
      );
      expect(emailIssue).toBeDefined();
    }
  });

  it('rejects password shorter than 8 characters', () => {
    const result = loginSchema.safeParse({
      email: 'user@example.com',
      password: 'short',
    });

    expect(result.success).toBe(false);
    if (!result.success) {
      const passwordIssue = result.error.issues.find((i) =>
        i.path.includes('password')
      );
      expect(passwordIssue).toBeDefined();
    }
  });

  it('rejects missing email', () => {
    const result = loginSchema.safeParse({
      password: 'Secure1pass',
    });

    expect(result.success).toBe(false);
  });

  it('rejects missing password', () => {
    const result = loginSchema.safeParse({
      email: 'user@example.com',
    });

    expect(result.success).toBe(false);
  });

  it('rejects empty object', () => {
    const result = loginSchema.safeParse({});

    expect(result.success).toBe(false);
  });
});

describe('registerSchema', () => {
  it('accepts valid registration data', () => {
    const result = registerSchema.safeParse({
      email: 'newuser@example.com',
      password: 'Strong1pass',
      firstName: 'John',
      lastName: 'Doe',
    });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.email).toBe('newuser@example.com');
      expect(result.data.firstName).toBe('John');
      expect(result.data.lastName).toBe('Doe');
    }
  });

  it('accepts optional orgName', () => {
    const result = registerSchema.safeParse({
      email: 'newuser@example.com',
      password: 'Strong1pass',
      firstName: 'John',
      lastName: 'Doe',
      orgName: 'Acme Corp',
    });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.orgName).toBe('Acme Corp');
    }
  });

  it('succeeds without orgName', () => {
    const result = registerSchema.safeParse({
      email: 'newuser@example.com',
      password: 'Strong1pass',
      firstName: 'John',
      lastName: 'Doe',
    });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.orgName).toBeUndefined();
    }
  });

  it('rejects empty firstName', () => {
    const result = registerSchema.safeParse({
      email: 'user@example.com',
      password: 'Strong1pass',
      firstName: '',
      lastName: 'Doe',
    });

    expect(result.success).toBe(false);
  });

  it('rejects empty lastName', () => {
    const result = registerSchema.safeParse({
      email: 'user@example.com',
      password: 'Strong1pass',
      firstName: 'John',
      lastName: '',
    });

    expect(result.success).toBe(false);
  });

  it('rejects firstName exceeding 100 characters', () => {
    const result = registerSchema.safeParse({
      email: 'user@example.com',
      password: 'Strong1pass',
      firstName: 'A'.repeat(101),
      lastName: 'Doe',
    });

    expect(result.success).toBe(false);
  });

  it('rejects orgName exceeding 200 characters', () => {
    const result = registerSchema.safeParse({
      email: 'user@example.com',
      password: 'Strong1pass',
      firstName: 'John',
      lastName: 'Doe',
      orgName: 'X'.repeat(201),
    });

    expect(result.success).toBe(false);
  });

  it('rejects invalid email format', () => {
    const result = registerSchema.safeParse({
      email: 'bad-email',
      password: 'Strong1pass',
      firstName: 'John',
      lastName: 'Doe',
    });

    expect(result.success).toBe(false);
  });

  it('rejects short password', () => {
    const result = registerSchema.safeParse({
      email: 'user@example.com',
      password: '1234567',
      firstName: 'John',
      lastName: 'Doe',
    });

    expect(result.success).toBe(false);
  });
});

describe('listSessionsSchema', () => {
  it('accepts empty query (uses defaults)', () => {
    const result = listSessionsSchema.safeParse({});

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.limit).toBe(20);
      expect(result.data.offset).toBe(0);
      expect(result.data.status).toBeUndefined();
      expect(result.data.org_id).toBeUndefined();
      expect(result.data.search).toBeUndefined();
    }
  });

  it('accepts valid status filter', () => {
    const result = listSessionsSchema.safeParse({ status: 'running' });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.status).toBe('running');
    }
  });

  it('accepts all valid status values', () => {
    const validStatuses = [
      'pending',
      'running',
      'rotating',
      'paused',
      'completed',
      'failed',
      'cancelled',
    ];

    for (const status of validStatuses) {
      const result = listSessionsSchema.safeParse({ status });
      expect(result.success).toBe(true);
    }
  });

  it('rejects invalid status value', () => {
    const result = listSessionsSchema.safeParse({ status: 'invalid' });

    expect(result.success).toBe(false);
  });

  it('accepts org_id filter', () => {
    const result = listSessionsSchema.safeParse({ org_id: 'org-123' });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.org_id).toBe('org-123');
    }
  });

  it('accepts search filter', () => {
    const result = listSessionsSchema.safeParse({ search: 'test query' });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.search).toBe('test query');
    }
  });

  it('rejects search exceeding 200 characters', () => {
    const result = listSessionsSchema.safeParse({
      search: 'x'.repeat(201),
    });

    expect(result.success).toBe(false);
  });

  it('includes pagination params', () => {
    const result = listSessionsSchema.safeParse({
      limit: '10',
      offset: '5',
      status: 'completed',
    });

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.limit).toBe(10);
      expect(result.data.offset).toBe(5);
      expect(result.data.status).toBe('completed');
    }
  });
});

describe('validateBody', () => {
  it('returns success with parsed data for valid input', () => {
    const result = validateBody(
      { email: 'user@test.com', password: 'MyPassword1' },
      loginSchema
    );

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.email).toBe('user@test.com');
      expect(result.data.password).toBe('MyPassword1');
    }
  });

  it('returns errors for invalid input', () => {
    const result = validateBody(
      { email: 'bad', password: '123' },
      loginSchema
    );

    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.errors.length).toBeGreaterThan(0);
      expect(result.errors[0]).toHaveProperty('field');
      expect(result.errors[0]).toHaveProperty('message');
    }
  });

  it('returns field names in error details', () => {
    const result = validateBody({ email: 'bad' }, loginSchema);

    expect(result.success).toBe(false);
    if (!result.success) {
      const fields = result.errors.map((e) => e.field);
      expect(fields).toContain('email');
    }
  });

  it('handles null input gracefully', () => {
    const result = validateBody(null, loginSchema);

    expect(result.success).toBe(false);
  });

  it('handles undefined input gracefully', () => {
    const result = validateBody(undefined, loginSchema);

    expect(result.success).toBe(false);
  });
});

describe('validateQuery', () => {
  it('returns success with parsed data for valid params', () => {
    const params = new URLSearchParams({ limit: '10', offset: '0' });
    const result = validateQuery(params, paginationQuerySchema);

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.limit).toBe(10);
      expect(result.data.offset).toBe(0);
    }
  });

  it('applies defaults for missing params', () => {
    const params = new URLSearchParams();
    const result = validateQuery(params, paginationQuerySchema);

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.limit).toBe(20);
      expect(result.data.offset).toBe(0);
    }
  });

  it('returns errors for invalid query params', () => {
    const params = new URLSearchParams({ limit: '999' });
    const result = validateQuery(params, paginationQuerySchema);

    expect(result.success).toBe(false);
    if (!result.success) {
      expect(result.errors.length).toBeGreaterThan(0);
      expect(result.errors[0]).toHaveProperty('field');
      expect(result.errors[0]).toHaveProperty('message');
    }
  });

  it('converts URLSearchParams to object for parsing', () => {
    const params = new URLSearchParams({
      limit: '50',
      offset: '25',
    });
    const result = validateQuery(params, paginationQuerySchema);

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.limit).toBe(50);
      expect(result.data.offset).toBe(25);
    }
  });

  it('validates listSessionsSchema with query params', () => {
    const params = new URLSearchParams({
      limit: '10',
      offset: '0',
      status: 'running',
      org_id: 'org-1',
    });
    const result = validateQuery(params, listSessionsSchema);

    expect(result.success).toBe(true);
    if (result.success) {
      expect(result.data.status).toBe('running');
      expect(result.data.org_id).toBe('org-1');
    }
  });

  it('rejects invalid status in query params', () => {
    const params = new URLSearchParams({ status: 'bogus' });
    const result = validateQuery(params, listSessionsSchema);

    expect(result.success).toBe(false);
    if (!result.success) {
      const statusError = result.errors.find((e) => e.field === 'status');
      expect(statusError).toBeDefined();
    }
  });
});
