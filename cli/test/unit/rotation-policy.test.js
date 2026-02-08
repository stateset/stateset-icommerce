/**
 * Tests for cli/src/sync/rotation-policy.js
 *
 * Covers: RotationPolicyManager — policies, usage tracking,
 * rotation checks, scheduled rotations.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';
import fs from 'fs/promises';
import path from 'path';
import os from 'os';

import { RotationPolicyManager, getRotationPolicyManager } from '../../src/sync/rotation-policy.js';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

let tmpDir;

async function makeTmpDir() {
  tmpDir = path.join(os.tmpdir(), `rotation-policy-test-${Date.now()}-${Math.random().toString(36).slice(2)}`);
  await fs.mkdir(tmpDir, { recursive: true });
  return tmpDir;
}

async function cleanupTmpDir() {
  if (tmpDir) {
    await fs.rm(tmpDir, { recursive: true, force: true });
    tmpDir = null;
  }
}

// ---------------------------------------------------------------------------
// Default policy
// ---------------------------------------------------------------------------

describe('RotationPolicyManager — defaults', () => {
  it('getDefaultPolicy returns sensible defaults', () => {
    const rpm = new RotationPolicyManager('/tmp/not-used');
    const policy = rpm.getDefaultPolicy();
    assert.equal(policy.maxAgeHours, 720);
    assert.equal(policy.warningThresholdHours, 24);
    assert.equal(policy.gracePeriodHours, 72);
    assert.equal(policy.enforceExpiry, true);
    assert.equal(policy.autoRotate, false);
  });
});

// ---------------------------------------------------------------------------
// Policy CRUD
// ---------------------------------------------------------------------------

describe('RotationPolicyManager — policy CRUD', () => {
  beforeEach(async () => { await makeTmpDir(); });
  afterEach(async () => { await cleanupTmpDir(); });

  it('setPolicy creates and returns merged policy', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    const p = await rpm.setPolicy('agent-1', 'signing', { maxAgeHours: 48 });
    assert.equal(p.maxAgeHours, 48);
    assert.equal(p.warningThresholdHours, 24); // default kept
  });

  it('getPolicy returns defaults when no custom policy', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    const p = await rpm.getPolicy('agent-1', 'signing');
    assert.equal(p.maxAgeHours, 720);
  });

  it('getPolicy returns custom policy after set', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    await rpm.setPolicy('agent-1', 'signing', { maxAgeHours: 100 });
    const p = await rpm.getPolicy('agent-1', 'signing');
    assert.equal(p.maxAgeHours, 100);
  });

  it('removePolicy reverts to defaults', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    await rpm.setPolicy('agent-1', 'signing', { maxAgeHours: 100 });
    await rpm.removePolicy('agent-1', 'signing');
    const p = await rpm.getPolicy('agent-1', 'signing');
    assert.equal(p.maxAgeHours, 720);
  });

  it('listPolicies returns all configured policies', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    await rpm.setPolicy('agent-1', 'signing', { maxAgeHours: 48 });
    await rpm.setPolicy('agent-1', 'encryption', { maxAgeHours: 24 });
    await rpm.setPolicy('agent-2', 'signing', { maxAgeHours: 96 });
    const list = await rpm.listPolicies();
    assert.equal(list.length, 3);
    assert.ok(list.some((p) => p.agentId === 'agent-1' && p.keyType === 'signing'));
    assert.ok(list.some((p) => p.agentId === 'agent-2'));
  });

  it('separate key types are independent', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    await rpm.setPolicy('a', 'signing', { maxAgeHours: 10 });
    await rpm.setPolicy('a', 'encryption', { maxAgeHours: 20 });
    assert.equal((await rpm.getPolicy('a', 'signing')).maxAgeHours, 10);
    assert.equal((await rpm.getPolicy('a', 'encryption')).maxAgeHours, 20);
  });
});

// ---------------------------------------------------------------------------
// Usage tracking
// ---------------------------------------------------------------------------

describe('RotationPolicyManager — usage tracking', () => {
  beforeEach(async () => { await makeTmpDir(); });
  afterEach(async () => { await cleanupTmpDir(); });

  it('recordUsage increments count', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    await rpm.recordUsage('agent-1', 'signing', 1);
    await rpm.recordUsage('agent-1', 'signing', 1);
    const usage = await rpm.getUsage('agent-1', 'signing', 1);
    assert.equal(usage.usageCount, 2);
    assert.ok(usage.lastUsedAt);
  });

  it('getUsage returns zero for untracked key', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    const usage = await rpm.getUsage('agent-1', 'signing', 99);
    assert.equal(usage.usageCount, 0);
    assert.equal(usage.lastUsedAt, null);
  });

  it('resetUsage clears counter', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    await rpm.recordUsage('agent-1', 'signing', 1);
    await rpm.resetUsage('agent-1', 'signing', 1);
    const usage = await rpm.getUsage('agent-1', 'signing', 1);
    assert.equal(usage.usageCount, 0);
  });
});

// ---------------------------------------------------------------------------
// Rotation checks
// ---------------------------------------------------------------------------

describe('RotationPolicyManager — shouldRotate', () => {
  beforeEach(async () => { await makeTmpDir(); });
  afterEach(async () => { await cleanupTmpDir(); });

  it('returns false when key is fresh', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    await rpm.setPolicy('a', 'signing', { maxAgeHours: 24 });
    const result = await rpm.shouldRotate('a', 1, 'signing', {
      createdAt: new Date().toISOString(),
    });
    assert.equal(result.shouldRotate, false);
  });

  it('returns true with age_limit reason when key is old', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    await rpm.setPolicy('a', 'signing', { maxAgeHours: 1 });
    const twoHoursAgo = new Date(Date.now() - 2 * 60 * 60 * 1000);
    const result = await rpm.shouldRotate('a', 1, 'signing', {
      createdAt: twoHoursAgo.toISOString(),
    });
    assert.equal(result.shouldRotate, true);
    assert.equal(result.reason, 'age_limit');
  });

  it('returns true with usage_limit reason when usage exceeded', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    await rpm.setPolicy('a', 'signing', { maxAgeHours: null, maxUsageCount: 3 });
    await rpm.recordUsage('a', 'signing', 1);
    await rpm.recordUsage('a', 'signing', 1);
    await rpm.recordUsage('a', 'signing', 1);
    const result = await rpm.shouldRotate('a', 1, 'signing', {
      createdAt: new Date().toISOString(),
    });
    assert.equal(result.shouldRotate, true);
    assert.equal(result.reason, 'usage_limit');
  });

  it('returns false when under usage limit', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    await rpm.setPolicy('a', 'signing', { maxAgeHours: null, maxUsageCount: 10 });
    await rpm.recordUsage('a', 'signing', 1);
    const result = await rpm.shouldRotate('a', 1, 'signing', {
      createdAt: new Date().toISOString(),
    });
    assert.equal(result.shouldRotate, false);
  });
});

// ---------------------------------------------------------------------------
// Expiry dates
// ---------------------------------------------------------------------------

describe('RotationPolicyManager — getExpiryDate', () => {
  beforeEach(async () => { await makeTmpDir(); });
  afterEach(async () => { await cleanupTmpDir(); });

  it('returns null when no maxAgeHours', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    await rpm.setPolicy('a', 'signing', { maxAgeHours: null });
    const date = await rpm.getExpiryDate('a', 'signing', {
      createdAt: new Date().toISOString(),
    });
    assert.equal(date, null);
  });

  it('returns correct expiry date', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    await rpm.setPolicy('a', 'signing', { maxAgeHours: 24 });
    const createdAt = new Date('2026-01-01T00:00:00Z');
    const date = await rpm.getExpiryDate('a', 'signing', {
      createdAt: createdAt.toISOString(),
    });
    assert.equal(date.toISOString(), '2026-01-02T00:00:00.000Z');
  });
});

// ---------------------------------------------------------------------------
// Scheduled rotations
// ---------------------------------------------------------------------------

describe('RotationPolicyManager — scheduled rotations', () => {
  beforeEach(async () => { await makeTmpDir(); });
  afterEach(async () => { await cleanupTmpDir(); });

  it('scheduleRotation creates a pending rotation', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    const rot = await rpm.scheduleRotation('agent-1', 'signing', 1, 'age_limit');
    assert.ok(rot.id);
    assert.equal(rot.agentId, 'agent-1');
    assert.equal(rot.status, 'pending');
    assert.equal(rot.reason, 'age_limit');
  });

  it('scheduleRotation rejects duplicate pending', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    await rpm.scheduleRotation('agent-1', 'signing', 1, 'age_limit');
    await assert.rejects(
      () => rpm.scheduleRotation('agent-1', 'signing', 2, 'manual'),
      /Pending rotation already exists/,
    );
  });

  it('completeRotation marks rotation as completed', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    const rot = await rpm.scheduleRotation('agent-1', 'signing', 1, 'age_limit');
    await rpm.completeRotation(rot.id, 2);
    const pending = await rpm.getPendingRotations('agent-1');
    assert.equal(pending.length, 0);
  });

  it('completeRotation throws for non-pending', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    const rot = await rpm.scheduleRotation('agent-1', 'signing', 1, 'age_limit');
    await rpm.completeRotation(rot.id, 2);
    await assert.rejects(
      () => rpm.completeRotation(rot.id, 3),
      /not pending/,
    );
  });

  it('failRotation marks rotation as failed', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    const rot = await rpm.scheduleRotation('agent-1', 'signing', 1, 'age_limit');
    await rpm.failRotation(rot.id, 'key gen failed');
    const list = await rpm.listRotations({ status: 'failed' });
    assert.equal(list.length, 1);
    assert.equal(list[0].errorMessage, 'key gen failed');
  });

  it('cancelRotation marks rotation as cancelled', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    const rot = await rpm.scheduleRotation('agent-1', 'signing', 1, 'age_limit');
    await rpm.cancelRotation(rot.id);
    const pending = await rpm.getPendingRotations('agent-1');
    assert.equal(pending.length, 0);
  });

  it('cancelRotation throws for non-pending', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    const rot = await rpm.scheduleRotation('agent-1', 'signing', 1, 'age_limit');
    await rpm.cancelRotation(rot.id);
    await assert.rejects(
      () => rpm.cancelRotation(rot.id),
      /Cannot cancel/,
    );
  });

  it('getPendingRotations filters by agent', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    await rpm.scheduleRotation('agent-1', 'signing', 1, 'age_limit');
    await rpm.scheduleRotation('agent-2', 'signing', 1, 'manual');
    const pending1 = await rpm.getPendingRotations('agent-1');
    assert.equal(pending1.length, 1);
    const pendingAll = await rpm.getPendingRotations();
    assert.equal(pendingAll.length, 2);
  });

  it('getDueRotations returns only past-due', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    const past = new Date(Date.now() - 1000);
    const future = new Date(Date.now() + 60000);
    await rpm.scheduleRotation('agent-1', 'signing', 1, 'age_limit', past);
    await rpm.scheduleRotation('agent-2', 'signing', 1, 'manual', future);
    const due = await rpm.getDueRotations();
    assert.equal(due.length, 1);
    assert.equal(due[0].agentId, 'agent-1');
  });

  it('listRotations supports status and limit filters', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    const r1 = await rpm.scheduleRotation('agent-1', 'signing', 1, 'age_limit');
    await rpm.scheduleRotation('agent-2', 'signing', 1, 'manual');
    await rpm.completeRotation(r1.id, 2);
    assert.equal((await rpm.listRotations({ status: 'pending' })).length, 1);
    assert.equal((await rpm.listRotations({ status: 'completed' })).length, 1);
    assert.equal((await rpm.listRotations({ limit: 1 })).length, 1);
  });

  it('cleanupRotations removes old non-pending rotations', async () => {
    const rpm = new RotationPolicyManager(tmpDir);
    const rot = await rpm.scheduleRotation('agent-1', 'signing', 1, 'age_limit');
    await rpm.completeRotation(rot.id, 2);
    // Clean with maxAgeDays=0 removes everything completed
    const removed = await rpm.cleanupRotations(0);
    assert.equal(removed, 1);
  });
});

// ---------------------------------------------------------------------------
// Singleton
// ---------------------------------------------------------------------------

describe('getRotationPolicyManager', () => {
  it('returns a RotationPolicyManager', () => {
    const rpm = getRotationPolicyManager('/tmp/test-singleton');
    assert.ok(rpm instanceof RotationPolicyManager);
  });

  it('returns same instance for same configDir', () => {
    const a = getRotationPolicyManager('/tmp/test-singleton-same');
    const b = getRotationPolicyManager('/tmp/test-singleton-same');
    assert.equal(a, b);
  });
});
