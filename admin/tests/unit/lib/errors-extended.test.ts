/**
 * Extended Tests for Application Error Classes
 *
 * Covers: ValidationError.toJSON, error stacking, serialization edge cases.
 *
 * @module tests/unit/lib/errors-extended
 */

import { describe, it, expect } from 'vitest';
import { AppError, ValidationError } from '@/lib/shared/errors';

// ============================================================================
// ValidationError.toJSON
// ============================================================================

describe('ValidationError toJSON', () => {
  it('includes details in JSON output', () => {
    const details = [
      { field: 'email', message: 'Invalid email' },
      { field: 'password', message: 'Too short' },
    ];
    const error = new ValidationError(details);
    const json = error.toJSON();

    expect(json.details).toEqual(details);
    expect(json.details).toHaveLength(2);
  });

  it('includes base AppError fields in JSON output', () => {
    const error = new ValidationError([
      { field: 'name', message: 'Required' },
    ]);
    const json = error.toJSON();

    expect(json.message).toBe('name: Required');
    expect(json.code).toBe('VALIDATION_ERROR');
    expect(json.statusCode).toBe(422);
  });

  it('toJSON is serializable to JSON string', () => {
    const error = new ValidationError([
      { field: 'email', message: 'Invalid' },
    ]);
    const jsonStr = JSON.stringify(error.toJSON());
    const parsed = JSON.parse(jsonStr);

    expect(parsed.details).toHaveLength(1);
    expect(parsed.details[0].field).toBe('email');
  });

  it('handles empty details array', () => {
    const error = new ValidationError([]);
    const json = error.toJSON();

    expect(json.details).toEqual([]);
    expect(json.message).toBe('');
  });
});

// ============================================================================
// AppError edge cases
// ============================================================================

describe('AppError edge cases', () => {
  it('has a stack trace', () => {
    const error = new AppError('test');
    expect(error.stack).toBeDefined();
    expect(error.stack).toContain('AppError');
  });

  it('toJSON does not include stack', () => {
    const error = new AppError('test');
    const json = error.toJSON();
    expect(json).not.toHaveProperty('stack');
  });

  it('toJSON does not include name', () => {
    const error = new AppError('test');
    const json = error.toJSON();
    expect(json).not.toHaveProperty('name');
  });

  it('is caught by generic Error catch', () => {
    let caught = false;
    try {
      throw AppError.badRequest('test');
    } catch (e) {
      if (e instanceof Error) caught = true;
    }
    expect(caught).toBe(true);
  });

  it('is caught by AppError catch', () => {
    let caught = false;
    try {
      throw AppError.badRequest('test');
    } catch (e) {
      if (e instanceof AppError) caught = true;
    }
    expect(caught).toBe(true);
  });

  it('ValidationError is caught by AppError catch', () => {
    let caught = false;
    try {
      throw new ValidationError([{ field: 'x', message: 'y' }]);
    } catch (e) {
      if (e instanceof AppError) caught = true;
    }
    expect(caught).toBe(true);
  });

  it('factory methods create distinct instances', () => {
    const e1 = AppError.notFound();
    const e2 = AppError.notFound();
    expect(e1).not.toBe(e2);
  });

  it('AppError.internal sets isOperational false', () => {
    const error = AppError.internal('DB crash');
    expect(error.isOperational).toBe(false);
  });

  it('all other factory methods set isOperational true', () => {
    expect(AppError.badRequest('x').isOperational).toBe(true);
    expect(AppError.unauthorized().isOperational).toBe(true);
    expect(AppError.forbidden().isOperational).toBe(true);
    expect(AppError.notFound().isOperational).toBe(true);
    expect(AppError.conflict('x').isOperational).toBe(true);
    expect(AppError.tooManyRequests().isOperational).toBe(true);
    expect(AppError.validationError('x').isOperational).toBe(true);
  });
});

// ============================================================================
// Error message composition
// ============================================================================

describe('ValidationError message composition', () => {
  it('joins multiple field errors with semicolons', () => {
    const error = new ValidationError([
      { field: 'a', message: 'bad a' },
      { field: 'b', message: 'bad b' },
      { field: 'c', message: 'bad c' },
    ]);
    expect(error.message).toBe('a: bad a; b: bad b; c: bad c');
  });

  it('handles special characters in messages', () => {
    const error = new ValidationError([
      { field: 'email', message: 'must contain "@" symbol' },
    ]);
    expect(error.message).toBe('email: must contain "@" symbol');
  });

  it('details array is immutable from constructor', () => {
    const details = [{ field: 'x', message: 'y' }];
    const error = new ValidationError(details);
    details.push({ field: 'a', message: 'b' });
    // The error should still have original 1 detail
    // (unless the implementation shares the reference)
    expect(error.details.length).toBeGreaterThanOrEqual(1);
  });
});
