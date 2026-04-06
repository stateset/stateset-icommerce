/**
 * Tests for Application Error Classes
 *
 * @module tests/unit/lib/errors
 */

import { describe, it, expect } from 'vitest';
import { AppError, ValidationError } from '@/lib/shared/errors';

describe('AppError', () => {
  describe('constructor', () => {
    it('creates an error with default values', () => {
      const error = new AppError('Something went wrong');

      expect(error).toBeInstanceOf(Error);
      expect(error).toBeInstanceOf(AppError);
      expect(error.message).toBe('Something went wrong');
      expect(error.statusCode).toBe(500);
      expect(error.code).toBe('INTERNAL_ERROR');
      expect(error.isOperational).toBe(true);
      expect(error.name).toBe('AppError');
    });

    it('creates an error with custom values', () => {
      const error = new AppError('Bad request', 400, 'BAD_REQUEST', true);

      expect(error.message).toBe('Bad request');
      expect(error.statusCode).toBe(400);
      expect(error.code).toBe('BAD_REQUEST');
      expect(error.isOperational).toBe(true);
    });

    it('preserves prototype chain', () => {
      const error = new AppError('test');
      expect(error instanceof AppError).toBe(true);
      expect(error instanceof Error).toBe(true);
    });
  });

  describe('toJSON', () => {
    it('returns message and code', () => {
      const error = new AppError('Not found', 404, 'NOT_FOUND');
      const json = error.toJSON();

      expect(json).toEqual({
        message: 'Not found',
        code: 'NOT_FOUND',
        statusCode: 404,
      });
    });

    it('includes statusCode but not isOperational in JSON', () => {
      const error = new AppError('Error', 500, 'ERR', false);
      const json = error.toJSON();

      expect(json).toHaveProperty('statusCode', 500);
      expect(json).not.toHaveProperty('isOperational');
    });
  });

  describe('factory methods', () => {
    describe('badRequest', () => {
      it('creates a 400 error with default code', () => {
        const error = AppError.badRequest('Invalid input');

        expect(error.statusCode).toBe(400);
        expect(error.code).toBe('BAD_REQUEST');
        expect(error.message).toBe('Invalid input');
        expect(error.isOperational).toBe(true);
      });

      it('accepts a custom code', () => {
        const error = AppError.badRequest('Invalid', 'INVALID_FORMAT');

        expect(error.code).toBe('INVALID_FORMAT');
        expect(error.statusCode).toBe(400);
      });
    });

    describe('unauthorized', () => {
      it('creates a 401 error with default message and code', () => {
        const error = AppError.unauthorized();

        expect(error.statusCode).toBe(401);
        expect(error.code).toBe('UNAUTHORIZED');
        expect(error.message).toBe('Authentication required');
      });

      it('accepts custom message and code', () => {
        const error = AppError.unauthorized('Token expired', 'TOKEN_EXPIRED');

        expect(error.statusCode).toBe(401);
        expect(error.code).toBe('TOKEN_EXPIRED');
        expect(error.message).toBe('Token expired');
      });
    });

    describe('forbidden', () => {
      it('creates a 403 error with default message and code', () => {
        const error = AppError.forbidden();

        expect(error.statusCode).toBe(403);
        expect(error.code).toBe('FORBIDDEN');
        expect(error.message).toBe('Access denied');
      });

      it('accepts custom message and code', () => {
        const error = AppError.forbidden('No permission', 'NO_PERMISSION');

        expect(error.statusCode).toBe(403);
        expect(error.code).toBe('NO_PERMISSION');
        expect(error.message).toBe('No permission');
      });
    });

    describe('notFound', () => {
      it('creates a 404 error with default message and code', () => {
        const error = AppError.notFound();

        expect(error.statusCode).toBe(404);
        expect(error.code).toBe('NOT_FOUND');
        expect(error.message).toBe('Resource not found');
      });

      it('accepts custom message and code', () => {
        const error = AppError.notFound('User not found', 'USER_NOT_FOUND');

        expect(error.statusCode).toBe(404);
        expect(error.code).toBe('USER_NOT_FOUND');
        expect(error.message).toBe('User not found');
      });
    });

    describe('conflict', () => {
      it('creates a 409 error with default code', () => {
        const error = AppError.conflict('Duplicate entry');

        expect(error.statusCode).toBe(409);
        expect(error.code).toBe('CONFLICT');
        expect(error.message).toBe('Duplicate entry');
      });

      it('accepts a custom code', () => {
        const error = AppError.conflict('Already exists', 'DUPLICATE');

        expect(error.code).toBe('DUPLICATE');
        expect(error.statusCode).toBe(409);
      });
    });

    describe('tooManyRequests', () => {
      it('creates a 429 error with default message and code', () => {
        const error = AppError.tooManyRequests();

        expect(error.statusCode).toBe(429);
        expect(error.code).toBe('RATE_LIMITED');
        expect(error.message).toBe('Rate limit exceeded');
      });

      it('accepts custom message and code', () => {
        const error = AppError.tooManyRequests('Slow down', 'THROTTLED');

        expect(error.statusCode).toBe(429);
        expect(error.code).toBe('THROTTLED');
        expect(error.message).toBe('Slow down');
      });
    });

    describe('internal', () => {
      it('creates a 500 error with default message and code', () => {
        const error = AppError.internal();

        expect(error.statusCode).toBe(500);
        expect(error.code).toBe('INTERNAL_ERROR');
        expect(error.message).toBe('Internal server error');
      });

      it('sets isOperational to false', () => {
        const error = AppError.internal();

        expect(error.isOperational).toBe(false);
      });

      it('accepts custom message and code', () => {
        const error = AppError.internal('DB failure', 'DB_ERROR');

        expect(error.statusCode).toBe(500);
        expect(error.code).toBe('DB_ERROR');
        expect(error.message).toBe('DB failure');
        expect(error.isOperational).toBe(false);
      });
    });

    describe('validationError', () => {
      it('creates a 422 error with default code', () => {
        const error = AppError.validationError('Invalid fields');

        expect(error.statusCode).toBe(422);
        expect(error.code).toBe('VALIDATION_ERROR');
        expect(error.message).toBe('Invalid fields');
      });

      it('accepts a custom code', () => {
        const error = AppError.validationError('Bad schema', 'SCHEMA_ERROR');

        expect(error.code).toBe('SCHEMA_ERROR');
        expect(error.statusCode).toBe(422);
      });
    });
  });
});

describe('ValidationError', () => {
  it('extends AppError', () => {
    const error = new ValidationError([
      { field: 'email', message: 'Required' },
    ]);

    expect(error).toBeInstanceOf(AppError);
    expect(error).toBeInstanceOf(ValidationError);
    expect(error).toBeInstanceOf(Error);
  });

  it('sets name to ValidationError', () => {
    const error = new ValidationError([
      { field: 'name', message: 'Too short' },
    ]);

    expect(error.name).toBe('ValidationError');
  });

  it('sets statusCode to 422', () => {
    const error = new ValidationError([
      { field: 'email', message: 'Invalid' },
    ]);

    expect(error.statusCode).toBe(422);
    expect(error.code).toBe('VALIDATION_ERROR');
  });

  it('stores details array', () => {
    const details = [
      { field: 'email', message: 'Invalid email' },
      { field: 'password', message: 'Too short' },
    ];
    const error = new ValidationError(details);

    expect(error.details).toEqual(details);
    expect(error.details).toHaveLength(2);
  });

  it('constructs message from details', () => {
    const error = new ValidationError([
      { field: 'email', message: 'Invalid email' },
      { field: 'password', message: 'Too short' },
    ]);

    expect(error.message).toBe('email: Invalid email; password: Too short');
  });

  it('handles single detail', () => {
    const error = new ValidationError([
      { field: 'name', message: 'Required' },
    ]);

    expect(error.message).toBe('name: Required');
    expect(error.details).toHaveLength(1);
  });

  it('preserves prototype chain', () => {
    const error = new ValidationError([
      { field: 'a', message: 'b' },
    ]);
    expect(error instanceof ValidationError).toBe(true);
    expect(error instanceof AppError).toBe(true);
  });
});
