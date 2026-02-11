/**
 * Safe JSON Parsing Utilities
 *
 * Prevents crashes from malformed JSON in user/external data.
 * Uses ValidationError from errors.js for structured error reporting.
 */

import { ValidationError } from '../errors.js';
import { logger } from '../logger.js';

/**
 * Safely parse JSON with error handling
 * @param {string} jsonString - JSON string to parse
 * @param {object} [options]
 * @param {*} [options.fallback=null] - Value to return on error
 * @param {boolean} [options.throwOnError=false] - Throw ValidationError instead of returning fallback
 * @param {string} [options.context='JSON data'] - Context for error messages
 * @returns {*} Parsed object or fallback
 */
export function safeJsonParse(jsonString, options = {}) {
  const { fallback = null, throwOnError = false, context = 'JSON data' } = options;

  if (typeof jsonString !== 'string') {
    const msg = `${context}: expected string, got ${typeof jsonString}`;
    if (throwOnError) {
      throw new ValidationError(msg, {
        field: context,
        expected: 'string',
        received: typeof jsonString,
      });
    }
    logger.warn(msg);
    return fallback;
  }

  try {
    return JSON.parse(jsonString);
  } catch (error) {
    const msg = `${context}: ${error.message}`;
    if (throwOnError) {
      throw new ValidationError(msg, {
        field: context,
        cause: error,
      });
    }
    logger.warn(msg);
    return fallback;
  }
}

/**
 * Parse JSON from file content with file path in error context
 * @param {string} content - File content
 * @param {string} filePath - File path for context
 * @param {object} [options] - Options (passed to safeJsonParse)
 * @returns {*} Parsed object or fallback
 */
export function safeJsonParseFile(content, filePath, options = {}) {
  return safeJsonParse(content, {
    ...options,
    context: options.context || `File ${filePath}`,
  });
}

/**
 * Parse JSON that must be valid (throws on error).
 * Use for internal/trusted data where invalid JSON indicates a bug.
 * @param {string} jsonString - JSON string
 * @param {string} [context='JSON data'] - Context for error
 * @returns {*} Parsed object
 */
export function strictJsonParse(jsonString, context = 'JSON data') {
  return safeJsonParse(jsonString, { throwOnError: true, context });
}
