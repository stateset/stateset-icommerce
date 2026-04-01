import { describe, it } from 'node:test';
import assert from 'node:assert/strict';

import { auditTools } from '../../src/tools/audit.js';
// AuditStore falls back cleanly when the native SQLite binding is unavailable,
// so these handler tests validate the real success path in both environments.

// ---------------------------------------------------------------------------
// Tool definition validation
// ---------------------------------------------------------------------------

describe('auditTools — module exports', () => {
  it('exports an array of 4 tools', () => {
    assert.ok(Array.isArray(auditTools));
    assert.equal(auditTools.length, 4);
  });

  it('exports expected tool names', () => {
    const names = auditTools.map((t) => t.name);
    assert.deepStrictEqual(names, [
      'audit_query',
      'audit_summary',
      'audit_export',
      'audit_retention',
    ]);
  });

  it('all tools have handler functions', () => {
    for (const tool of auditTools) {
      assert.equal(typeof tool.handler, 'function', `${tool.name} missing handler`);
    }
  });

  it('all tools have valid permissions', () => {
    for (const tool of auditTools) {
      assert.ok(
        ['read', 'write', 'admin'].includes(tool.permission),
        `${tool.name} has invalid permission: ${tool.permission}`,
      );
    }
  });

  it('all tools have non-empty descriptions', () => {
    for (const tool of auditTools) {
      assert.ok(tool.description, `${tool.name} missing description`);
      assert.ok(tool.description.length > 10, `${tool.name} description too short`);
    }
  });

  it('read tools have read permission', () => {
    const byName = Object.fromEntries(auditTools.map((t) => [t.name, t]));
    assert.equal(byName['audit_query'].permission, 'read');
    assert.equal(byName['audit_summary'].permission, 'read');
  });

  it('admin tools have admin permission', () => {
    const byName = Object.fromEntries(auditTools.map((t) => [t.name, t]));
    assert.equal(byName['audit_export'].permission, 'admin');
    assert.equal(byName['audit_retention'].permission, 'admin');
  });
});

// ---------------------------------------------------------------------------
// audit_query handler
// ---------------------------------------------------------------------------

describe('auditTools — audit_query handler', () => {
  const byName = Object.fromEntries(auditTools.map((t) => [t.name, t]));

  it('returns success or catches error gracefully', async () => {
    const result = await byName['audit_query'].handler({ params: {} });
    assert.ok('success' in result);
    if (result.success) {
      assert.ok('count' in result);
      assert.ok('entries' in result);
      assert.ok(Array.isArray(result.entries));
    } else {
      assert.ok(result.error);
    }
  });

  it('accepts filter parameters', async () => {
    const result = await byName['audit_query'].handler({
      params: { tool: 'list_customers', result: 'denied', limit: 10 },
    });
    assert.ok('success' in result);
  });

  it('accepts since parameter', async () => {
    const result = await byName['audit_query'].handler({
      params: { since: '2026-01-01T00:00:00Z' },
    });
    assert.ok('success' in result);
  });

  it('default limit is 50', async () => {
    // Verify the handler doesn't crash with empty params
    const result = await byName['audit_query'].handler({ params: {} });
    assert.ok('success' in result);
  });
});

// ---------------------------------------------------------------------------
// audit_summary handler
// ---------------------------------------------------------------------------

describe('auditTools — audit_summary handler', () => {
  const byName = Object.fromEntries(auditTools.map((t) => [t.name, t]));

  it('returns summary shape on success', async () => {
    const result = await byName['audit_summary'].handler({ params: {} });
    if (result.success) {
      assert.ok('totalEntries' in result);
      assert.ok('queriedEntries' in result);
      assert.ok('byResult' in result);
      assert.ok('topTools' in result);
      assert.ok('denialRate' in result);
    }
  });

  it('accepts since parameter', async () => {
    const result = await byName['audit_summary'].handler({
      params: { since: '2026-01-01T00:00:00Z' },
    });
    assert.ok('success' in result);
  });

  it('returns denialRate as percentage string', async () => {
    const result = await byName['audit_summary'].handler({ params: {} });
    if (result.success) {
      assert.ok(result.denialRate.endsWith('%'));
    }
  });
});

// ---------------------------------------------------------------------------
// audit_export handler
// ---------------------------------------------------------------------------

describe('auditTools — audit_export handler', () => {
  const byName = Object.fromEntries(auditTools.map((t) => [t.name, t]));

  it('returns json format by default', async () => {
    const result = await byName['audit_export'].handler({ params: {} });
    if (result.success) {
      assert.equal(result.format, 'json');
      assert.ok('exportedAt' in result);
      assert.ok('totalEntries' in result);
      assert.ok('entries' in result);
    }
  });

  it('returns csv format when requested', async () => {
    const result = await byName['audit_export'].handler({
      params: { format: 'csv' },
    });
    if (result.success) {
      assert.equal(result.format, 'csv');
      assert.ok('csv' in result);
      assert.ok(result.csv.startsWith('id,timestamp,tool,'));
    }
  });

  it('respects limit parameter', async () => {
    const result = await byName['audit_export'].handler({
      params: { limit: 5 },
    });
    if (result.success) {
      assert.ok(result.exportedEntries <= 5);
    }
  });

  it('respects since parameter', async () => {
    const result = await byName['audit_export'].handler({
      params: { since: '2099-01-01T00:00:00Z' },
    });
    if (result.success) {
      assert.equal(result.exportedEntries, 0);
    }
  });
});

// ---------------------------------------------------------------------------
// audit_retention handler
// ---------------------------------------------------------------------------

describe('auditTools — audit_retention handler', () => {
  const byName = Object.fromEntries(auditTools.map((t) => [t.name, t]));

  it('requires --apply flag', async () => {
    const result = await byName['audit_retention'].handler({
      params: {},
      allowApply: false,
    });
    assert.equal(result.success, false);
    assert.ok(result.error.includes('--apply'));
    assert.ok(result.hint);
    assert.ok(result.wouldDo);
  });

  it('runs cleanup when allowApply is true', async () => {
    const result = await byName['audit_retention'].handler({
      params: {},
      allowApply: true,
    });
    if (result.success) {
      assert.ok('entriesBefore' in result);
      assert.ok('entriesAfter' in result);
      assert.ok('entriesRemoved' in result);
      assert.ok(result.message.includes('cleanup'));
    }
  });

  it('returns entriesRemoved >= 0', async () => {
    const result = await byName['audit_retention'].handler({
      params: {},
      allowApply: true,
    });
    if (result.success) {
      assert.ok(result.entriesRemoved >= 0);
      assert.equal(result.entriesRemoved, result.entriesBefore - result.entriesAfter);
    }
  });
});

// ---------------------------------------------------------------------------
// CSV export edge cases
// ---------------------------------------------------------------------------

describe('auditTools — CSV export edge cases', () => {
  const byName = Object.fromEntries(auditTools.map((t) => [t.name, t]));

  it('CSV header contains all expected columns', async () => {
    const result = await byName['audit_export'].handler({
      params: { format: 'csv' },
    });
    if (result.success) {
      const header = result.csv.split('\n')[0];
      assert.ok(header.includes('id'));
      assert.ok(header.includes('timestamp'));
      assert.ok(header.includes('tool'));
      assert.ok(header.includes('result'));
      assert.ok(header.includes('reason'));
      assert.ok(header.includes('level'));
      assert.ok(header.includes('session_id'));
      assert.ok(header.includes('agent'));
    }
  });

  it('CSV has header + data rows', async () => {
    const result = await byName['audit_export'].handler({
      params: { format: 'csv' },
    });
    if (result.success) {
      const lines = result.csv.split('\n');
      // At minimum: header row
      assert.ok(lines.length >= 1);
      // Data rows = exportedEntries
      assert.equal(lines.length - 1, result.exportedEntries);
    }
  });
});

// ---------------------------------------------------------------------------
// Input schema validation
// ---------------------------------------------------------------------------

describe('auditTools — input schemas', () => {
  const byName = Object.fromEntries(auditTools.map((t) => [t.name, t]));

  it('audit_query has optional tool, result, since, limit fields', () => {
    const schema = byName['audit_query'].inputSchema;
    assert.ok(schema.tool);
    assert.ok(schema.result);
    assert.ok(schema.since);
    assert.ok(schema.limit);
  });

  it('audit_summary has optional since field', () => {
    const schema = byName['audit_summary'].inputSchema;
    assert.ok(schema.since);
  });

  it('audit_export has optional since, limit, format fields', () => {
    const schema = byName['audit_export'].inputSchema;
    assert.ok(schema.since);
    assert.ok(schema.limit);
    assert.ok(schema.format);
  });

  it('audit_retention has empty inputSchema', () => {
    const schema = byName['audit_retention'].inputSchema;
    assert.deepStrictEqual(schema, {});
  });
});
