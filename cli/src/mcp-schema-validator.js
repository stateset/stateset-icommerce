/**
 * Enhanced Schema Validator with AI-Friendly Error Explanations
 * Provides detailed, actionable error messages for agent troubleshooting
 */

import { z } from 'zod';

export class EnhancedValidator {
  constructor() {
    this.errorCache = new Map();
    this.commonPatterns = this.buildCommonPatterns();
  }

  /**
   * Validate data against schema with rich error context
   * @param {Object} schema - Zod schema
   * @param {Object} data - Data to validate
   * @returns {Object} - { valid: boolean, errors: Array, suggestions: Array }
   */
  validate(schema, data, path = '') {
    try {
      const result = schema.passthrough().safeParse(data);

      if (result.success) {
        return {
          valid: true,
          data: result.data,
          errors: [],
          suggestions: [],
        };
      }

      const errors = this.formatErrors(result.error.errors, path);
      const suggestions = this.generateSuggestions(errors, data);

      return {
        valid: false,
        errors,
        suggestions,
        context: this.buildErrorContext(errors, data),
      };
    } catch (error) {
      return {
        valid: false,
        errors: [
          {
            path,
            code: 'VALIDATION_ERROR',
            message: error.message,
            severity: 'critical',
          },
        ],
        suggestions: ['Review the schema configuration'],
      };
    }
  }

  /**
   * Format Zod errors into user-friendly messages
   */
  formatErrors(errors, basePath = '') {
    return errors.map((error, index) => {
      const path =
        error.path.length > 0
          ? `${basePath ? basePath + '.' : ''}${error.path.join('.')}`
          : basePath || 'root';

      const formattedError = {
        path,
        code: error.code,
        message: this.formatErrorMessage(error),
        severity: this.determineSeverity(error.code),
        received: error.received,
        rule: this.explainRule(error),
        examples: this.getExamples(error.code),
        index: index + 1,
      };

      return formattedError;
    });
  }

  /**
   * Format error message based on error code
   */
  formatErrorMessage(error) {
    const path = Array.isArray(error.path) ? error.path.join('.') : error.path;
    const received = JSON.stringify(error.received);

    const messages = {
      invalid_type: `${path ? `Field "${path}"` : 'Value'} must be of type ${this.formatExpectedType(error.expected)}. Received: ${received}`,
      invalid_enum: `${path ? `Field "${path}"` : 'Value'} must be one of: ${this.formatEnumValues(error.expected)}. Received: ${received}`,
      too_small: `${path ? `Field "${path}"` : 'Value'} is too small. Minimum: ${error.minimum}. Received: ${error.type || this.formatValue(error.received)}`,
      too_big: `${path ? `Field "${path}"` : 'Value'} is too big. Maximum: ${error.maximum}. Received: ${error.type || this.formatValue(error.received)}`,
      invalid_string: `${path ? `Field "${path}"` : 'Value'} is not a valid ${error.validation}. Received: ${received}`,
      invalid_email: `${path ? `Field "${path}"` : 'Value'} is not a valid email address. Example: user@example.com`,
      invalid_url: `${path ? `Field "${path}"` : 'Value'} is not a valid URL. Example: https://example.com`,
      invalid_date: `${path ? `Field "${path}"` : 'Value'} is not a valid date. Example: 2024-01-25 or ISO 8601: 2024-01-25T10:30:00Z`,
      invalid_uuid: `${path ? `Field "${path}"` : 'Value'} is not a valid UUID. Example: 550e8400-e29b-41d4-a716-446655440000`,
      required: `Required field "${path}" is missing or is null/undefined`,
      custom: `${path ? `Field "${path}"` : 'Value'} failed custom validation: ${error.message}`,
    };

    return messages[error.code] || error.message || `Validation error at ${path}`;
  }

  /**
   * Explain validation rules for agents
   */
  explainRule(error) {
    const explanations = {
      invalid_type: `This field expects a specific data type. Common types include: string, number, boolean, array, object.`,
      invalid_enum: `This field only accepts specific predefined values. Choose from the provided options.`,
      too_small: `This field has a minimum value constraint. Ensure the value meets or exceeds the minimum requirement.`,
      too_big: `This field has a maximum value constraint. Ensure the value does not exceed the maximum limit.`,
      invalid_string: `This field requires string data to match a specific format (email, URL, UUID, regex pattern, etc.).`,
      invalid_email: `Email addresses must follow standard format (local-part@domain). Additional rules: valid domain, no invalid characters.`,
      invalid_url: `URLs must include protocol (http:// or https://), domain, and valid characters.`,
      invalid_date: `Dates should be in a standard format: YYYY-MM-DD or ISO 8601 for timestamps.`,
      invalid_uuid: `UUIDs must be 32 hexadecimal characters, separated by hyphens in the pattern 8-4-4-4-12.`,
      required: `This field cannot be omitted, null, or undefined. It must be provided with a valid value.`,
      custom: `This field has custom business logic validation that must be satisfied.`,
    };

    return explanations[error.code] || 'Standard validation rule';
  }

  /**
   * Generate actionable suggestions based on errors
   */
  generateSuggestions(errors, data) {
    const suggestions = [];

    errors.forEach((error) => {
      const suggestion = this.buildSuggestion(error, data);
      if (suggestion) {
        suggestions.push({
          error: error.path,
          suggestion,
          priority: error.severity === 'critical' ? 'high' : 'medium',
          fix: this.provideFix(error, data),
        });
      }
    });

    return suggestions;
  }

  /**
   * Build specific suggestions per error type
   */
  buildSuggestion(error, _data) {
    const suggestionMap = {
      invalid_email: 'Provide a valid email address with @ symbol and domain',
      invalid_url: 'Include protocol (http:// or https://) and full domain path',
      too_small: `Increase ${error.path} to meet minimum requirement of ${error.minimum}`,
      too_big: `Reduce ${error.path} to stay within maximum limit of ${error.maximum}`,
      invalid_enum: `Select one of: ${this.formatEnumValues(error.expected)}`,
      invalid_uuid: 'Use UUID v4 format or generate with uuid v4() library',
      invalid_date: 'Use YYYY-MM-DD format or ISO 8601 timestamp',
      required: `Provide a value for ${error.path} or use a default value`,
      invalid_type: `Convert ${error.path} to ${this.formatExpectedType(error.expected)}`,
    };

    return suggestionMap[error.code];
  }

  /**
   * Provide concrete fix suggestions with code examples
   */
  provideFix(error, _data) {
    const fixes = {
      invalid_email: {
        wrong: 'user(at)example.com',
        correct: 'user@example.com',
        code: `const email = 'john.doe@example.com'; // Standard format`,
      },
      invalid_url: {
        wrong: 'example.com',
        correct: 'https://example.com',
        code: `const url = 'https://example.com/path'; // With protocol`,
      },
      too_small: {
        wrong: error.received,
        correct: error.minimum,
        code: `const value = ${error.minimum}; // Meets minimum`,
      },
      too_big: {
        wrong: error.received,
        correct: error.maximum,
        code: `const value = ${error.maximum}; // Stays within limit`,
      },
      invalid_type: {
        wrong: JSON.stringify(error.received),
        correct: this.formatExpectedType(error.expected),
        code: `const value = ${this.getExampleForType(error.expected)}; // Correct type`,
      },
      required: {
        wrong: `null or undefined`,
        correct: `valid value`,
        code: `const ${error.path.split('.').pop()} = 'some-value'; // Provide value`,
      },
    };

    return fixes[error.code];
  }

  /**
   * Build error context for debugging
   */
  buildErrorContext(errors, data) {
    const context = {
      errorCount: errors.length,
      severitySummary: this.summarizeSeverity(errors),
      affectedFields: errors.map((e) => e.path).filter((v, i, a) => a.indexOf(v) === i),
      dataSample: this.extractSafeSample(data),
      timestamp: new Date().toISOString(),
    };

    return context;
  }

  /**
   * Summarize error severity
   */
  summarizeSeverity(errors) {
    const summary = errors.reduce((acc, error) => {
      acc[error.severity] = (acc[error.severity] || 0) + 1;
      return acc;
    }, {});

    return summary;
  }

  /**
   * Extract safe data sample (avoid exposing sensitive data)
   */
  extractSafeSample(data, maxDepth = 2, maxKeys = 5) {
    const extract = (obj, depth = 0) => {
      if (depth >= maxDepth) {
        return '[max depth reached]';
      }

      if (typeof obj !== 'object' || obj === null) {
        return obj;
      }

      if (Array.isArray(obj)) {
        return obj.slice(0, 3).map((item) => extract(item, depth + 1));
      }

      const keys = Object.keys(obj).slice(0, maxKeys);
      const result = {};

      for (const key of keys) {
        // Mask sensitive fields
        const isSensitive = /password|secret|token|key|credit/i.test(key);

        if (isSensitive) {
          result[key] = '[REDACTED]';
        } else {
          result[key] = extract(obj[key], depth + 1);
        }
      }

      return result;
    };

    return extract(data);
  }

  /**
   * Determine error severity
   */
  determineSeverity(code) {
    const severityMap = {
      required: 'critical',
      invalid_type: 'high',
      invalid_enum: 'high',
      too_small: 'medium',
      too_big: 'medium',
      invalid_string: 'medium',
      invalid_email: 'medium',
      invalid_url: 'medium',
      invalid_date: 'medium',
      invalid_uuid: 'medium',
      custom: 'medium',
    };

    return severityMap[code] || 'low';
  }

  /**
   * Format enum values for display
   */
  formatEnumValues(values) {
    if (Array.isArray(values)) {
      return values.map((v) => (typeof v === 'string' ? `"${v}"` : v)).join(', ');
    }
    return String(values);
  }

  /**
   * Format expected type for display
   */
  formatExpectedType(expected) {
    const typeMap = {
      string: 'text/string',
      number: 'number',
      boolean: 'true/false',
      array: 'list/array',
      object: 'object/map',
      date: 'date/datetime',
    };

    return typeMap[expected] || expected;
  }

  /**
   * Format value for display
   */
  formatValue(value) {
    if (value === null || value === undefined) {
      return 'null/undefined';
    }
    if (typeof value === 'string') {
      return `"${value.length > 50 ? value.substring(0, 50) + '...' : value}"`;
    }
    if (typeof value === 'object') {
      return Array.isArray(value) ? `array (${value.length} items)` : 'object';
    }
    return String(value);
  }

  /**
   * Get example for type
   */
  getExampleForType(type) {
    const examples = {
      string: "'example text'",
      number: '123.45',
      boolean: 'true',
      array: '[1, 2, 3]',
      object: '{ key: "value" }',
      date: 'new Date()',
      email: "'user@example.com'",
      url: "'https://example.com'",
      uuid: "'550e8400-e29b-41d4-a716-446655440000'",
    };

    return examples[type] || `'${type} example'`;
  }

  /**
   * Get examples for specific error codes
   */
  getExamples(code) {
    const examples = {
      invalid_email: ['john.doe@example.com', 'user@domain.co.uk', 'name+tag@gmail.com'],
      invalid_url: ['https://example.com', 'https://example.com/path', 'http://localhost:8080'],
      invalid_uuid: [
        '550e8400-e29b-41d4-a716-446655440000',
        '00000000-0000-0000-0000-000000000000',
      ],
    };

    return examples[code] || [];
  }

  /**
   * Build common patterns for error detection
   */
  buildCommonPatterns() {
    return {
      email: /^[^\s@]+@[^\s@]+\.[^\s@]+$/,
      url: /^https?:\/\/.+/,
      uuid: /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i,
      phone: /^\+?[\d\s-()]+$/,
      zipCode: /^\d{5}(-\d{4})?$/,
    };
  }

  /**
   * Batch validate multiple items
   */
  validateBatch(schema, items) {
    const results = [];
    const summary = {
      total: items.length,
      valid: 0,
      invalid: 0,
      commonErrors: new Map(),
    };

    items.forEach((item, index) => {
      const result = this.validate(schema, item, `items[${index}]`);
      results.push(result);

      if (result.valid) {
        summary.valid++;
      } else {
        summary.invalid++;

        result.errors.forEach((error) => {
          const key = `${error.code}:${error.path}`;
          summary.commonErrors.set(key, (summary.commonErrors.get(key) || 0) + 1);
        });
      }
    });

    return {
      results,
      summary: {
        ...summary,
        commonErrors: Array.from(summary.commonErrors.entries())
          .map(([code, count]) => ({ code, count }))
          .sort((a, b) => b.count - a.count),
      },
    };
  }

  /**
   * Create validation report
   */
  createReport(schema, data, metadata = {}) {
    const result = this.validate(schema, data);

    return {
      timestamp: new Date().toISOString(),
      metadata,
      result: {
        valid: result.valid,
        errors: result.errors,
        suggestions: result.suggestions,
        context: result.context,
      },
      summary: {
        total: result.errors.length,
        critical: result.errors.filter((e) => e.severity === 'critical').length,
        high: result.errors.filter((e) => e.severity === 'high').length,
        medium: result.errors.filter((e) => e.severity === 'medium').length,
        low: result.errors.filter((e) => e.severity === 'low').length,
      },
    };
  }
}

/**
 * Predefined validators for common commerce entities
 */
export const CommerceValidators = {
  /**
   * Customer validator
   */
  customer: z.object({
    email: z.string().email('Must be a valid email address'),
    firstName: z.string().min(1, 'First name is required').max(100, 'First name too long'),
    lastName: z.string().min(1, 'Last name is required').max(100, 'Last name too long'),
    phone: z
      .string()
      .regex(/^\+?[\d\s-()]+$/, 'Invalid phone format')
      .optional(),
    acceptsMarketing: z.boolean().optional(),
  }),

  /**
   * Order validator
   */
  order: z.object({
    customerId: z.string().uuid('Must be a valid UUID'),
    items: z
      .array(
        z.object({
          sku: z.string().min(1, 'SKU is required'),
          name: z.string().min(1, 'Item name is required'),
          quantity: z.number().int().positive('Quantity must be positive'),
          unitPrice: z.number().nonnegative('Price cannot be negative'),
        }),
      )
      .min(1, 'Order must have at least one item'),
    currency: z.string().length(3, 'Currency code must be 3 characters (e.g., USD)'),
    notes: z.string().optional(),
  }),

  /**
   * Product validator
   */
  product: z.object({
    name: z.string().min(1, 'Product name is required').max(500, 'Name too long'),
    slug: z
      .string()
      .regex(/^[a-z0-9-]+$/, 'Slug must contain lowercase letters, numbers, and hyphens only'),
    status: z.enum(['active', 'inactive', 'draft', 'archived'], 'Invalid product status'),
    variants: z
      .array(
        z.object({
          sku: z.string().min(1, 'SKU is required'),
          price: z.number().nonnegative('Price cannot be negative'),
        }),
      )
      .optional(),
  }),

  /**
   * Inventory validator
   */
  inventoryAdjustment: z.object({
    sku: z.string().min(1, 'SKU is required'),
    quantity: z.number().int('Quantity must be an integer'),
    reason: z.string().min(1, 'Reason is required').max(500, 'Reason too long'),
  }),
};

export default EnhancedValidator;
