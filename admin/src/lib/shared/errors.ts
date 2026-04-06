/**
 * Application Error Classes
 *
 * Standardized error types for consistent error handling and response shapes.
 */

export class AppError extends Error {
  public readonly statusCode: number;
  public readonly code: string;
  public readonly isOperational: boolean;

  constructor(
    message: string,
    statusCode: number = 500,
    code: string = 'INTERNAL_ERROR',
    isOperational: boolean = true
  ) {
    super(message);
    this.name = 'AppError';
    this.statusCode = statusCode;
    this.code = code;
    this.isOperational = isOperational;
    Object.setPrototypeOf(this, AppError.prototype);
  }

  toJSON(): { message: string; code: string; statusCode: number } {
    return {
      message: this.message,
      code: this.code,
      statusCode: this.statusCode,
    };
  }

  // Common factory methods
  static badRequest(message: string, code: string = 'BAD_REQUEST'): AppError {
    return new AppError(message, 400, code);
  }

  static unauthorized(message: string = 'Authentication required', code: string = 'UNAUTHORIZED'): AppError {
    return new AppError(message, 401, code);
  }

  static forbidden(message: string = 'Access denied', code: string = 'FORBIDDEN'): AppError {
    return new AppError(message, 403, code);
  }

  static notFound(message: string = 'Resource not found', code: string = 'NOT_FOUND'): AppError {
    return new AppError(message, 404, code);
  }

  static conflict(message: string, code: string = 'CONFLICT'): AppError {
    return new AppError(message, 409, code);
  }

  static tooManyRequests(message: string = 'Rate limit exceeded', code: string = 'RATE_LIMITED'): AppError {
    return new AppError(message, 429, code);
  }

  static internal(message: string = 'Internal server error', code: string = 'INTERNAL_ERROR'): AppError {
    return new AppError(message, 500, code, false);
  }

  static validationError(message: string, code: string = 'VALIDATION_ERROR'): AppError {
    return new AppError(message, 422, code);
  }
}

export class ValidationError extends AppError {
  public readonly details: Array<{ field: string; message: string }>;

  constructor(details: Array<{ field: string; message: string }>) {
    const message = details.map((d) => `${d.field}: ${d.message}`).join('; ');
    super(message, 422, 'VALIDATION_ERROR');
    this.name = 'ValidationError';
    this.details = details;
    Object.setPrototypeOf(this, ValidationError.prototype);
  }

  override toJSON(): { message: string; code: string; statusCode: number; details: Array<{ field: string; message: string }> } {
    return {
      ...super.toJSON(),
      details: this.details,
    };
  }
}
