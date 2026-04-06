/**
 * Tests for Request Context
 *
 * @module tests/unit/lib/request-context
 */

import { describe, it, expect } from 'vitest';
import { generateRequestId, requestStore, getRequestId } from '@/lib/shared/request-context';

describe('generateRequestId', () => {
  it('returns a string starting with "req_"', () => {
    const id = generateRequestId();
    expect(id).toMatch(/^req_/);
  });

  it('contains a valid UUID after the prefix', () => {
    const id = generateRequestId();
    const uuid = id.replace('req_', '');
    // UUID v4 format: 8-4-4-4-12 hex chars
    expect(uuid).toMatch(
      /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/
    );
  });

  it('generates unique IDs across calls', () => {
    const ids = new Set(Array.from({ length: 100 }, () => generateRequestId()));
    expect(ids.size).toBe(100);
  });
});

describe('getRequestId', () => {
  it('returns "unknown" outside request context', () => {
    expect(getRequestId()).toBe('unknown');
  });

  it('returns the requestId within a context', async () => {
    const id = generateRequestId();
    await requestStore.run({ requestId: id, startTime: Date.now() }, async () => {
      expect(getRequestId()).toBe(id);
    });
  });
});
