/**
 * Safe Number Parsing Utilities
 *
 * Validates parsed numbers to prevent NaN propagation and range violations.
 * Uses ValidationError from errors.js for structured error reporting.
 */

import { ValidationError } from '../errors.js';

/**
 * Safely parse integer with validation
 * @param {*} value - Value to parse
 * @param {object} [options]
 * @param {number} [options.fallback=0] - Fallback value if invalid
 * @param {number} [options.radix=10] - Radix for parsing
 * @param {number} [options.min=-Infinity] - Minimum allowed value
 * @param {number} [options.max=Infinity] - Maximum allowed value
 * @param {boolean} [options.throwOnError=false] - Throw ValidationError on invalid
 * @param {string} [options.field='value'] - Field name for error messages
 * @returns {number} Parsed integer or fallback
 */
export function safeParseInt(value, options = {}) {
  const {
    fallback = 0,
    radix = 10,
    min = -Infinity,
    max = Infinity,
    throwOnError = false,
    field = 'value',
  } = options;

  const parsed = parseInt(value, radix);

  if (Number.isNaN(parsed)) {
    if (throwOnError) {
      throw new ValidationError(`Invalid integer for ${field}`, {
        field,
        expected: 'integer',
        received: value,
      });
    }
    return fallback;
  }

  if (parsed < min || parsed > max) {
    if (throwOnError) {
      throw new ValidationError(`${field} out of range [${min}, ${max}]`, {
        field,
        expected: `${min} to ${max}`,
        received: parsed,
      });
    }
    return fallback;
  }

  return parsed;
}

/**
 * Safely parse float with validation
 * @param {*} value - Value to parse
 * @param {object} [options]
 * @param {number} [options.fallback=0] - Fallback value if invalid
 * @param {number} [options.min=-Infinity] - Minimum allowed value
 * @param {number} [options.max=Infinity] - Maximum allowed value
 * @param {boolean} [options.throwOnError=false] - Throw ValidationError on invalid
 * @param {string} [options.field='value'] - Field name for error messages
 * @returns {number} Parsed float or fallback
 */
export function safeParseFloat(value, options = {}) {
  const {
    fallback = 0,
    min = -Infinity,
    max = Infinity,
    throwOnError = false,
    field = 'value',
  } = options;

  const parsed = parseFloat(value);

  if (Number.isNaN(parsed) || !Number.isFinite(parsed)) {
    if (throwOnError) {
      throw new ValidationError(`Invalid number for ${field}`, {
        field,
        expected: 'finite number',
        received: value,
      });
    }
    return fallback;
  }

  if (parsed < min || parsed > max) {
    if (throwOnError) {
      throw new ValidationError(`${field} out of range [${min}, ${max}]`, {
        field,
        expected: `${min} to ${max}`,
        received: parsed,
      });
    }
    return fallback;
  }

  return parsed;
}

/**
 * Parse integer that must be valid (throws on error)
 * @param {*} value - Value to parse
 * @param {string} [field='value'] - Field name
 * @param {object} [options] - Additional options (min, max, radix)
 * @returns {number} Parsed integer
 */
export function strictParseInt(value, field = 'value', options = {}) {
  return safeParseInt(value, { ...options, throwOnError: true, field });
}

/**
 * Parse float that must be valid (throws on error)
 * @param {*} value - Value to parse
 * @param {string} [field='value'] - Field name
 * @param {object} [options] - Additional options (min, max)
 * @returns {number} Parsed float
 */
export function strictParseFloat(value, field = 'value', options = {}) {
  return safeParseFloat(value, { ...options, throwOnError: true, field });
}
