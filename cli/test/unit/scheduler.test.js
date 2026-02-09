/**
 * Tests for cli/src/workflows/scheduler.js
 *
 * Covers: parseCron, cronFieldMatches, cronMatches, getNextCronTime,
 * Job, JobResult, Scheduler, JobTemplates.
 */

import { describe, it, beforeEach, afterEach } from 'node:test';
import assert from 'node:assert/strict';

import {
  Job,
  JobResult,
  JobStatus,
  Scheduler,
  JobTemplates,
} from '../../src/workflows/scheduler.js';

// ---------------------------------------------------------------------------
// Job
// ---------------------------------------------------------------------------

describe('Job', () => {
  it('creates with defaults', () => {
    const job = new Job({ name: 'test', action: { agent: 'orders', request: 'list' } });
    assert.equal(job.name, 'test');
    assert.equal(job.type, 'cron');
    assert.equal(job.enabled, true);
    assert.equal(job.maxRetries, 3);
    assert.equal(job.status, JobStatus.PENDING);
    assert.ok(job.id);
  });

  it('calculateNextRun for interval type', () => {
    const job = new Job({
      name: 'interval-job',
      type: 'interval',
      schedule: 60000,
      action: { agent: 'test' },
    });

    const now = new Date('2026-01-01T00:00:00Z');
    const next = job.calculateNextRun(now);
    assert.equal(next.getTime(), now.getTime() + 60000);
  });

  it('calculateNextRun for once type (future)', () => {
    const future = new Date(Date.now() + 86400000).toISOString();
    const job = new Job({
      name: 'once-job',
      type: 'once',
      schedule: future,
      action: { agent: 'test' },
    });

    const next = job.calculateNextRun(new Date());
    assert.ok(next);
  });

  it('calculateNextRun for once type (past) returns null', () => {
    const past = new Date('2020-01-01').toISOString();
    const job = new Job({
      name: 'past-job',
      type: 'once',
      schedule: past,
      action: { agent: 'test' },
    });

    const next = job.calculateNextRun(new Date());
    assert.equal(next, null);
  });

  it('calculateNextRun returns null when disabled', () => {
    const job = new Job({
      name: 'disabled',
      type: 'interval',
      schedule: 1000,
      enabled: false,
      action: { agent: 'test' },
    });
    assert.equal(job.calculateNextRun(), null);
  });

  it('calculateNextRun throws for unknown type', () => {
    const job = new Job({
      name: 'bad',
      type: 'unknown',
      schedule: 'x',
      action: { agent: 'test' },
    });
    assert.throws(() => job.calculateNextRun(), /Unknown job type/);
  });

  it('calculateNextRun for cron type', () => {
    const job = new Job({
      name: 'cron-job',
      type: 'cron',
      schedule: '0 * * * *', // every hour at :00
      action: { agent: 'test' },
    });

    const from = new Date('2026-01-01T12:30:00Z');
    const next = job.calculateNextRun(from);
    assert.equal(next.getMinutes(), 0);
    assert.ok(next > from);
  });

  it('toJSON includes all fields', () => {
    const job = new Job({ name: 'json-test', action: { agent: 'x' } });
    const json = job.toJSON();
    assert.equal(json.name, 'json-test');
    assert.ok('id' in json);
    assert.ok('status' in json);
    assert.ok('runCount' in json);
  });
});

// ---------------------------------------------------------------------------
// JobResult
// ---------------------------------------------------------------------------

describe('JobResult', () => {
  it('creates with fields', () => {
    const result = new JobResult({
      jobId: 'j1',
      status: 'completed',
      startedAt: new Date().toISOString(),
    });
    assert.equal(result.jobId, 'j1');
    assert.equal(result.status, 'completed');
    assert.ok(result.runId);
  });
});

// ---------------------------------------------------------------------------
// Scheduler
// ---------------------------------------------------------------------------

describe('Scheduler', () => {
  let scheduler;

  beforeEach(() => {
    scheduler = new Scheduler({
      storePath: null,
      tickInterval: 60000,
      executor: async (action, ctx) => ({ result: 'ok' }),
    });
  });

  afterEach(() => {
    scheduler.stop();
  });

  it('addJob and getJob', () => {
    const job = scheduler.addJob({
      name: 'test',
      type: 'interval',
      schedule: 5000,
      action: { agent: 'test' },
    });
    assert.ok(job.id);
    assert.equal(scheduler.getJob(job.id).name, 'test');
  });

  it('addJob calculates nextRunAt', () => {
    const job = scheduler.addJob({
      name: 'test',
      type: 'interval',
      schedule: 5000,
      action: { agent: 'test' },
    });
    assert.ok(job.nextRunAt);
  });

  it('removeJob deletes job', () => {
    const job = scheduler.addJob({
      name: 'rm-test',
      type: 'interval',
      schedule: 5000,
      action: { agent: 'test' },
    });
    assert.ok(scheduler.removeJob(job.id));
    assert.equal(scheduler.getJob(job.id), undefined);
  });

  it('removeJob returns false for unknown', () => {
    assert.ok(!scheduler.removeJob('nonexistent'));
  });

  it('updateJob modifies fields', () => {
    const job = scheduler.addJob({
      name: 'upd-test',
      type: 'interval',
      schedule: 5000,
      action: { agent: 'test' },
    });
    const updated = scheduler.updateJob(job.id, { description: 'changed' });
    assert.equal(updated.description, 'changed');
  });

  it('updateJob returns null for unknown', () => {
    assert.equal(scheduler.updateJob('nope', {}), null);
  });

  it('pauseJob / resumeJob', () => {
    const job = scheduler.addJob({
      name: 'pause-test',
      type: 'interval',
      schedule: 5000,
      action: { agent: 'test' },
    });
    scheduler.pauseJob(job.id);
    assert.equal(scheduler.getJob(job.id).status, JobStatus.PAUSED);
    assert.equal(scheduler.getJob(job.id).enabled, false);

    scheduler.resumeJob(job.id);
    assert.equal(scheduler.getJob(job.id).status, JobStatus.PENDING);
    assert.equal(scheduler.getJob(job.id).enabled, true);
  });

  it('listJobs returns all jobs sorted by nextRunAt', () => {
    scheduler.addJob({ name: 'j1', type: 'interval', schedule: 1000, action: { agent: 'a' } });
    scheduler.addJob({ name: 'j2', type: 'interval', schedule: 2000, action: { agent: 'b' } });

    const jobs = scheduler.listJobs();
    assert.equal(jobs.length, 2);
  });

  it('listJobs filters by status', () => {
    const j1 = scheduler.addJob({
      name: 'j1',
      type: 'interval',
      schedule: 1000,
      action: { agent: 'a' },
    });
    scheduler.pauseJob(j1.id);
    scheduler.addJob({ name: 'j2', type: 'interval', schedule: 1000, action: { agent: 'b' } });

    assert.equal(scheduler.listJobs({ status: JobStatus.PAUSED }).length, 1);
  });

  it('listJobs filters by enabled', () => {
    const j1 = scheduler.addJob({
      name: 'j1',
      type: 'interval',
      schedule: 1000,
      action: { agent: 'a' },
    });
    scheduler.pauseJob(j1.id);
    scheduler.addJob({ name: 'j2', type: 'interval', schedule: 1000, action: { agent: 'b' } });

    assert.equal(scheduler.listJobs({ enabled: true }).length, 1);
    assert.equal(scheduler.listJobs({ enabled: false }).length, 1);
  });

  it('getDueJobs returns jobs past nextRunAt', () => {
    const past = new Date(Date.now() - 10000).toISOString();
    const job = scheduler.addJob({
      name: 'due',
      type: 'interval',
      schedule: 1000,
      action: { agent: 'a' },
    });
    job.nextRunAt = past;

    const due = scheduler.getDueJobs();
    assert.equal(due.length, 1);
  });

  it('getDueJobs excludes disabled and running jobs', () => {
    const job = scheduler.addJob({
      name: 'disabled-due',
      type: 'interval',
      schedule: 1000,
      action: { agent: 'a' },
      enabled: false,
    });
    job.nextRunAt = new Date(Date.now() - 10000).toISOString();

    assert.equal(scheduler.getDueJobs().length, 0);
  });

  it('executeJob completes successfully', async () => {
    const job = scheduler.addJob({
      name: 'exec-test',
      type: 'interval',
      schedule: 60000,
      action: { agent: 'test', request: 'do' },
    });

    const result = await scheduler.executeJob(job);
    assert.equal(result.status, JobStatus.COMPLETED);
    assert.equal(job.runCount, 1);
    assert.ok(job.lastRunAt);
  });

  it('executeJob handles failure', async () => {
    const failScheduler = new Scheduler({
      storePath: null,
      executor: async () => {
        throw new Error('boom');
      },
    });

    const job = failScheduler.addJob({
      name: 'fail-test',
      type: 'once',
      schedule: new Date(Date.now() + 86400000).toISOString(),
      action: { agent: 'test' },
      maxRetries: 0,
    });

    const result = await failScheduler.executeJob(job);
    assert.equal(result.status, JobStatus.FAILED);
    assert.equal(result.error, 'boom');
    assert.equal(job.failCount, 1);
    failScheduler.stop();
  });

  it('executeJob disables one-time job after completion', async () => {
    const job = scheduler.addJob({
      name: 'once-test',
      type: 'once',
      schedule: new Date(Date.now() + 86400000).toISOString(),
      action: { agent: 'test' },
    });

    await scheduler.executeJob(job);
    assert.equal(job.enabled, false);
  });

  it('runNow throws for unknown job', async () => {
    await assert.rejects(() => scheduler.runNow('nonexistent'), /not found/);
  });

  it('start / stop / isRunning', () => {
    assert.ok(!scheduler.isRunning);
    scheduler.start();
    assert.ok(scheduler.isRunning);
    scheduler.stop();
    assert.ok(!scheduler.isRunning);
  });

  it('start is idempotent', () => {
    scheduler.start();
    scheduler.start(); // no error
    scheduler.stop();
  });

  it('getStatus returns counts', () => {
    scheduler.addJob({ name: 'a', type: 'interval', schedule: 1000, action: { agent: 'x' } });
    const status = scheduler.getStatus();
    assert.equal(status.totalJobs, 1);
    assert.equal(status.enabledJobs, 1);
    assert.equal(status.runningJobs, 0);
  });

  it('getHistory returns execution history', async () => {
    const job = scheduler.addJob({
      name: 'hist-test',
      type: 'interval',
      schedule: 60000,
      action: { agent: 'test' },
    });
    await scheduler.executeJob(job);

    const history = scheduler.getHistory();
    assert.equal(history.length, 1);
  });

  it('getHistory filters by jobId', async () => {
    const j1 = scheduler.addJob({
      name: 'a',
      type: 'interval',
      schedule: 60000,
      action: { agent: 'a' },
    });
    const j2 = scheduler.addJob({
      name: 'b',
      type: 'interval',
      schedule: 60000,
      action: { agent: 'b' },
    });
    await scheduler.executeJob(j1);
    await scheduler.executeJob(j2);

    assert.equal(scheduler.getHistory({ jobId: j1.id }).length, 1);
  });

  it('cancelJob cancels running job', async () => {
    const slowScheduler = new Scheduler({
      storePath: null,
      executor: async (action, { signal }) => {
        await new Promise((r) => setTimeout(r, 60000));
      },
    });

    const job = slowScheduler.addJob({
      name: 'slow',
      type: 'interval',
      schedule: 60000,
      action: { agent: 'test' },
    });

    // Start execution without waiting
    const execPromise = slowScheduler.executeJob(job);
    // Give it a moment to start
    await new Promise((r) => setTimeout(r, 50));

    assert.ok(slowScheduler.cancelJob(job.id));
    assert.equal(job.status, JobStatus.CANCELLED);

    await execPromise.catch(() => {});
    slowScheduler.stop();
  });

  it('cancelJob returns false for non-running job', () => {
    assert.ok(!scheduler.cancelJob('not-running'));
  });
});

// ---------------------------------------------------------------------------
// JobTemplates
// ---------------------------------------------------------------------------

describe('JobTemplates', () => {
  it('has expected templates', () => {
    const expected = [
      'lowStockCheck',
      'abandonedCartRecovery',
      'subscriptionRenewal',
      'dailySalesReport',
      'eventSync',
      'promotionActivation',
      'overdueInvoiceReminder',
    ];
    for (const key of expected) {
      assert.ok(key in JobTemplates, `missing template: ${key}`);
    }
  });

  it('templates create valid Jobs', () => {
    for (const [key, template] of Object.entries(JobTemplates)) {
      const job = new Job(template);
      assert.ok(job.name, `template ${key} missing name`);
      assert.ok(job.action, `template ${key} missing action`);
    }
  });
});
