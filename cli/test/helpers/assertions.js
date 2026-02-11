/**
 * Custom Assertions for Tests
 *
 * Provides domain-specific assertions for tool results.
 */

import assert from 'node:assert/strict';

/**
 * Assert that a tool result indicates success.
 * @param {object} result - Tool handler result
 * @param {string} [message] - Optional assertion message
 */
export function assertSuccess(result, message) {
  assert.ok(result, message || 'Expected non-null result');
  assert.ok(!result.error, message || `Unexpected error: ${result.error}`);
  if ('success' in result) {
    assert.strictEqual(result.success, true, message || 'Expected success: true');
  }
}

/**
 * Assert that a tool result indicates an error.
 * @param {object} result - Tool handler result
 * @param {string} [expectedMessage] - Substring expected in error message
 */
export function assertError(result, expectedMessage) {
  assert.ok(result, 'Expected non-null result');
  assert.ok(result.error, `Expected error in result, got: ${JSON.stringify(result)}`);
  if (expectedMessage) {
    assert.ok(
      result.error.toLowerCase().includes(expectedMessage.toLowerCase()),
      `Expected error containing "${expectedMessage}", got "${result.error}"`,
    );
  }
}

/**
 * Assert that a tool result is a preview (dry-run / no --apply).
 * @param {object} result - Tool handler result
 */
export function assertPreview(result) {
  assert.ok(result, 'Expected non-null result');
  const isPreview =
    result.preview ||
    result.wouldCreate ||
    result.wouldUpdate ||
    result.wouldDelete ||
    (result.error && result.hint);
  assert.ok(isPreview, `Expected preview response, got: ${JSON.stringify(result)}`);
}

/**
 * Assert that a result contains a specific field.
 * @param {object} result - Tool handler result
 * @param {string} field - Field name
 */
export function assertHasField(result, field) {
  assert.ok(result, 'Expected non-null result');
  assert.ok(field in result, `Expected result to have field '${field}', keys: ${Object.keys(result).join(', ')}`);
}

/**
 * Assert that a result list has expected count.
 * @param {object} result - Tool handler result
 * @param {string} listField - Field name containing the array
 * @param {number} expectedCount - Expected array length
 */
export function assertListCount(result, listField, expectedCount) {
  assertHasField(result, listField);
  assert.ok(Array.isArray(result[listField]), `Expected ${listField} to be an array`);
  assert.strictEqual(
    result[listField].length,
    expectedCount,
    `Expected ${expectedCount} items in ${listField}, got ${result[listField].length}`,
  );
}
