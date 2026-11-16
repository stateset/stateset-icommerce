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

  it('throws when cron expression has wrong number of fields', () => {
    const job = new Job({
      name: 'bad-cron',
      type: 'cron',
      schedule: '*/15 * * *', // missing day-of-week
      action: { agent: 'test' },
    });
    assert.throws(() => job.calculateNextRun(new Date()), /Expected 5 parts/);
  });

  it('throws when cron expression contains out-of-range values', () => {
    const job = new Job({
      name: 'bad-cron-value',
      type: 'cron',
      schedule: '60 * * * *', // minute out of range
      action: { agent: 'test' },
    });
    assert.throws(() => job.calculateNextRun(new Date()), /value must be between 0 and 59/);
  });

  it('throws when cron expression contains malformed ranges', () => {
    const job = new Job({
      name: 'bad-cron-range',
      type: 'cron',
      schedule: '1- 2 * * *', // malformed range with space
      action: { agent: 'test' },
    });
    assert.throws(() => job.calculateNextRun(new Date()), /malformed range|range must be/);
  });

  it('throws when cron expression contains invalid step values', () => {
    const job = new Job({
      name: 'bad-cron-step',
      type: 'cron',
      schedule: '*/0 * * * *', // zero-step invalid
      action: { agent: 'test' },
    });
    assert.throws(() => job.calculateNextRun(new Date()), /step must be a positive integer/);
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

  it('applies the production default job timeout of 5 minutes', () => {
    const job = scheduler.addJob({
      name: 'default-timeout',
      type: 'interval',
      schedule: 60000,
      action: { agent: 'test' },
    });
    assert.equal(job.timeout, 5 * 60 * 1000);
  });

  it('defaultJobTimeout applies to jobs added without an explicit timeout', () => {
    const custom = new Scheduler({ storePath: null, defaultJobTimeout: 250 });
    const inherited = custom.addJob({
      name: 'inherits',
      type: 'interval',
      schedule: 60000,
      action: { agent: 'test' },
    });
    const explicit = custom.addJob({
      name: 'explicit',
      type: 'interval',
      schedule: 60000,
      timeout: 9000,
      action: { agent: 'test' },
    });
    assert.equal(inherited.timeout, 250);
    assert.equal(explicit.timeout, 9000);
  });

  it('defaultJobTimeout bounds an executor that ignores the abort signal', async () => {
    // The executor's timer stays ref'd (the scheduler's own timeout timer is
    // unref'd, so an unref'd executor would let the event loop drain mid-test)
    // and is cleared afterwards so it cannot outlive the test.
    let executorTimer = null;
    const stubborn = new Scheduler({
      storePath: null,
      defaultJobTimeout: 100,
      executor: () =>
        new Promise((resolve) => {
          executorTimer = setTimeout(() => resolve({ ok: true }), 60000);
        }),
    });
    const job = stubborn.addJob({
      name: 'stubborn',
      type: 'interval',
      schedule: 60000,
      maxRetries: 0,
      action: { agent: 'test' },
    });
    const started = Date.now();
    try {
      const result = await stubborn.executeJob(job);
      assert.equal(result.status, JobStatus.FAILED);
      assert.equal(result.error, 'Job timed out');
      assert.ok(
        Date.now() - started < 2000,
        'must resolve on the configured timeout, not the executor',
      );
    } finally {
      clearTimeout(executorTimer);
      stubborn.stop();
    }
  });

  it('cancelJob cancels running job', async () => {
    // The scheduler-level timeout is the safety net: even if a regression stops
    // the abort signal from reaching the executor, this test cannot hang.
    const slowScheduler = new Scheduler({
      storePath: null,
      defaultJobTimeout: 500,
      executor: async (action, { signal }) =>
        new Promise((resolve, reject) => {
          const timer = setTimeout(() => resolve({ ok: true }), 1000);
          const onAbort = () => {
            clearTimeout(timer);
            const abortError = new Error('aborted');
            abortError.name = 'AbortError';
            reject(abortError);
          };
          signal?.addEventListener('abort', onAbort, { once: true });
        }),
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

    const started = Date.now();
    assert.ok(slowScheduler.cancelJob(job.id));
    assert.equal(job.status, JobStatus.CANCELLED);

    const result = await execPromise;
    assert.equal(result.status, JobStatus.CANCELLED);
    assert.ok(Date.now() - started < 2000, 'cancellation must not wait for the executor timer');
    slowScheduler.stop();
  });

  it('cancelJob returns false for non-running job', () => {
    assert.ok(!scheduler.cancelJob('not-running'));
  });
});

// ---------------------------------------------------------------------------
// Lifecycle guards: retries, cancel, pause, remove, resume
// ---------------------------------------------------------------------------

describe('Scheduler lifecycle guards', () => {
  const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
  const due = () => new Date(Date.now() - 1000).toISOString();
  /** @type {Scheduler[]} */
  let schedulers;

  function make(options) {
    const s = new Scheduler({ storePath: null, ...options });
    s.on('error', () => {});
    schedulers.push(s);
    return s;
  }

  /** An executor that blocks until its run is aborted. */
  const untilAborted =
    (counter) =>
    async (_action, { signal }) => {
      counter.calls++;
      await new Promise((resolve) => signal.addEventListener('abort', resolve, { once: true }));
    };

  beforeEach(() => {
    schedulers = [];
  });

  afterEach(() => {
    for (const s of schedulers) s.stop();
  });

  it('retries exactly maxRetries times even while the tick keeps firing', async () => {
    let calls = 0;
    const s = make({
      tickInterval: 20,
      executor: async () => {
        calls++;
        throw new Error('boom');
      },
    });
    const job = s.addJob({
      name: 'flaky',
      type: 'interval',
      schedule: 60_000,
      action: { agent: 'test' },
      maxRetries: 1,
      retryDelay: 200,
    });
    job.nextRunAt = due();
    s.start();

    await sleep(120); // inside the backoff window
    assert.equal(calls, 1, 'the tick must not re-run a job waiting on its retry');
    assert.equal(job.status, JobStatus.RETRYING);
    assert.ok(new Date(job.nextRunAt) > new Date(), 'nextRunAt is the retry time');
    assert.deepEqual(s.getDueJobs(new Date(Date.now() + 10_000)), []);

    await sleep(400);
    assert.equal(calls, 2, 'one run plus exactly one retry');
    assert.equal(job.status, JobStatus.FAILED);
    assert.equal(job.enabled, true);
    assert.ok(new Date(job.nextRunAt) > new Date(Date.now() + 30_000));
  });

  it('a recurring job that exhausts its retries reads failed, not pending', async () => {
    const s = make({
      executor: async () => {
        throw new Error('boom');
      },
    });
    const job = s.addJob({
      name: 'recurring',
      type: 'interval',
      schedule: 60_000,
      action: { agent: 'test' },
      maxRetries: 0,
    });
    const result = await s.executeJob(job);
    assert.equal(result.status, JobStatus.FAILED);
    assert.equal(job.status, JobStatus.FAILED);
    assert.equal(job.enabled, true);
    assert.ok(job.nextRunAt, 'still scheduled');
  });

  it('a cancelled one-time job does not run again', async () => {
    const counter = { calls: 0 };
    const s = make({ tickInterval: 20, executor: untilAborted(counter) });
    const job = s.addJob({
      name: 'once',
      type: 'once',
      schedule: due(),
      action: { agent: 'test' },
      maxRetries: 0,
    });
    job.nextRunAt = due();
    s.start();
    await sleep(50);
    assert.equal(s.cancelJob(job.id), true);
    await sleep(120);

    assert.equal(counter.calls, 1);
    assert.equal(job.status, JobStatus.CANCELLED);
    assert.equal(job.enabled, false);
    assert.equal(job.nextRunAt, null);
  });

  it('a cancelled recurring run is not re-run on the next tick', async () => {
    const counter = { calls: 0 };
    const s = make({ tickInterval: 20, executor: untilAborted(counter) });
    const job = s.addJob({
      name: 'recurring',
      type: 'interval',
      schedule: 60_000,
      action: { agent: 'test' },
      maxRetries: 0,
    });
    job.nextRunAt = due();
    s.start();
    await sleep(50);
    assert.equal(s.cancelJob(job.id), true);
    await sleep(120);

    assert.equal(counter.calls, 1);
    assert.equal(job.status, JobStatus.CANCELLED);
    assert.equal(job.enabled, true, 'cancelling a run does not disable a recurring job');
    assert.ok(new Date(job.nextRunAt) > new Date(Date.now() + 30_000));
  });

  for (const [name, stop] of [
    ['pause', (s, job) => s.pauseJob(job.id)],
    ['remove', (s, job) => s.removeJob(job.id)],
    ['cancel', (s, job) => assert.equal(s.cancelJob(job.id), true)],
  ]) {
    it(`${name} during the retry backoff disarms the retry`, async () => {
      let calls = 0;
      const s = make({
        executor: async () => {
          calls++;
          throw new Error('boom');
        },
      });
      const job = s.addJob({
        name,
        type: 'once',
        schedule: new Date(Date.now() + 86_400_000).toISOString(),
        action: { agent: 'test' },
        maxRetries: 2,
        retryDelay: 60,
      });
      await s.runNow(job.id);
      assert.equal(job.status, JobStatus.RETRYING);
      stop(s, job);
      assert.equal(s.retryTimers.size, 0);
      await sleep(200);

      assert.equal(calls, 1, `no retry may fire after ${name}`);
      if (name === 'pause') {
        assert.equal(job.status, JobStatus.PAUSED);
        assert.equal(job.enabled, false);
      }
      if (name === 'cancel') {
        assert.equal(job.status, JobStatus.CANCELLED);
        assert.equal(job.enabled, false);
      }
    });
  }

  it('runNow during the backoff supersedes the armed retry', async () => {
    let calls = 0;
    const s = make({
      executor: async () => {
        calls++;
        if (calls === 1) throw new Error('first attempt fails');
        return { ok: true };
      },
    });
    const job = s.addJob({
      name: 'manual',
      type: 'interval',
      schedule: 60_000,
      action: { agent: 'test' },
      maxRetries: 1,
      retryDelay: 60,
    });
    await s.runNow(job.id);
    assert.equal(job.status, JobStatus.RETRYING);
    assert.equal((await s.runNow(job.id)).status, JobStatus.COMPLETED);
    await sleep(150);
    assert.equal(calls, 2);
    assert.equal(job.status, JobStatus.COMPLETED);
  });

  it('a recurring job paused mid-run stays paused, is not retried, and can be resumed', async () => {
    let release;
    let calls = 0;
    const s = make({
      executor: () => {
        calls++;
        return new Promise((_resolve, reject) => {
          release = () => reject(new Error('boom'));
        });
      },
    });
    const job = s.addJob({
      name: 'paused-mid-run',
      type: 'interval',
      schedule: 60_000,
      action: { agent: 'test' },
      maxRetries: 3,
      retryDelay: 20,
    });
    const run = s.executeJob(job);
    await sleep(10);
    s.pauseJob(job.id);
    release();
    assert.equal((await run).status, JobStatus.FAILED);
    await sleep(80);

    assert.equal(calls, 1);
    assert.equal(job.status, JobStatus.PAUSED);
    assert.equal(job.enabled, false);
    s.resumeJob(job.id);
    assert.equal(job.status, JobStatus.PENDING);
    assert.equal(job.enabled, true);
    assert.ok(job.nextRunAt);
  });

  /**
   * runNow runs a job even while it is disabled. A failed manual run used to
   * arm a retry whose timer then skipped the disabled job, leaving it
   * `retrying` with no timer — and resumeJob refused a `retrying` job, so it
   * was stuck. A successful or cancelled manual run likewise overwrote the
   * paused/added-disabled status that resumeJob looks for.
   */
  it('a failed manual run of a disabled job arms no retry and leaves it resumable', async () => {
    let calls = 0;
    const s = make({
      executor: async () => {
        calls++;
        throw new Error('boom');
      },
    });
    const job = s.addJob({
      name: 'disabled-manual',
      type: 'interval',
      schedule: 60_000,
      action: { agent: 'test' },
      enabled: false,
      maxRetries: 1,
      retryDelay: 20,
    });

    const result = await s.runNow(job.id);
    assert.equal(result.status, JobStatus.FAILED);
    assert.equal(s.retryTimers.size, 0, 'no retry is armed for a disabled job');
    assert.equal(job.status, JobStatus.PENDING, 'back to its added-disabled status');
    assert.equal(job.enabled, false);
    assert.equal(job.nextRunAt, null);
    assert.equal(job.lastError, 'boom');
    assert.equal(job.failCount, 1);
    assert.ok(job.lastRunAt);

    await sleep(80); // past the retry delay
    assert.equal(calls, 1, 'nothing ran after the manual run');
    assert.equal(job.status, JobStatus.PENDING);

    s.resumeJob(job.id);
    assert.equal(job.enabled, true);
    assert.equal(job.status, JobStatus.PENDING);
    assert.ok(job.nextRunAt);
  });

  for (const [outcome, executor, expected] of [
    ['successful', async () => ({ ok: true }), JobStatus.COMPLETED],
    [
      'failed',
      async () => {
        throw new Error('boom');
      },
      JobStatus.FAILED,
    ],
  ]) {
    it(`a ${outcome} manual run of a paused job leaves it paused and resumable`, async () => {
      const s = make({ executor });
      const job = s.addJob({
        name: `paused-${outcome}`,
        type: 'interval',
        schedule: 60_000,
        action: { agent: 'test' },
        maxRetries: 2,
        retryDelay: 20,
      });
      s.pauseJob(job.id);

      assert.equal((await s.runNow(job.id)).status, expected);
      assert.equal(job.status, JobStatus.PAUSED);
      assert.equal(job.enabled, false);
      assert.equal(job.nextRunAt, null);
      assert.equal(s.retryTimers.size, 0);
      assert.ok(job.lastRunAt);

      s.resumeJob(job.id);
      assert.equal(job.enabled, true);
      assert.equal(job.status, JobStatus.PENDING);
      assert.ok(job.nextRunAt);
    });
  }

  it('a cancelled manual run of a disabled job leaves it resumable', async () => {
    const counter = { calls: 0 };
    const s = make({ executor: untilAborted(counter) });
    const job = s.addJob({
      name: 'disabled-cancelled',
      type: 'interval',
      schedule: 60_000,
      action: { agent: 'test' },
      enabled: false,
    });
    const run = s.runNow(job.id);
    await sleep(10);
    assert.equal(s.cancelJob(job.id), true);
    assert.equal((await run).status, JobStatus.CANCELLED);

    assert.equal(job.status, JobStatus.PENDING);
    assert.equal(job.enabled, false);
    s.resumeJob(job.id);
    assert.equal(job.enabled, true);
  });

  it('a failed manual run of a disabled one-time job settles as failed without a retry', async () => {
    let calls = 0;
    const s = make({
      executor: async () => {
        calls++;
        throw new Error('boom');
      },
    });
    const job = s.addJob({
      name: 'disabled-once',
      type: 'once',
      schedule: new Date(Date.now() + 86_400_000).toISOString(),
      action: { agent: 'test' },
      enabled: false,
      maxRetries: 2,
      retryDelay: 20,
    });

    assert.equal((await s.runNow(job.id)).status, JobStatus.FAILED);
    await sleep(80);
    assert.equal(calls, 1);
    assert.equal(s.retryTimers.size, 0);
    assert.equal(job.status, JobStatus.FAILED);
    assert.equal(job.enabled, false);
  });

  it('resumeJob only resumes a paused (or added-disabled) job', async () => {
    const counter = { calls: 0 };
    const s = make({ executor: untilAborted(counter) });

    const running = s.addJob({
      name: 'running',
      type: 'interval',
      schedule: 60_000,
      action: { agent: 'test' },
      maxRetries: 0,
    });
    const run = s.executeJob(running);
    await sleep(10);
    assert.equal(s.resumeJob(running.id), running);
    assert.equal(running.status, JobStatus.RUNNING, 'resume must not relabel a running job');
    s.cancelJob(running.id);
    await run;

    const done = s.addJob({
      name: 'done',
      type: 'once',
      schedule: new Date(Date.now() + 86_400_000).toISOString(),
      action: { agent: 'test' },
    });
    done.enabled = false;
    done.status = JobStatus.COMPLETED;
    assert.throws(() => s.resumeJob(done.id), /only a paused job can be resumed/);
    assert.equal(done.status, JobStatus.COMPLETED);
    assert.equal(done.enabled, false);

    const disabled = s.addJob({
      name: 'disabled',
      type: 'interval',
      schedule: 60_000,
      action: { agent: 'test' },
      enabled: false,
    });
    s.resumeJob(disabled.id);
    assert.equal(disabled.enabled, true);
    assert.equal(disabled.status, JobStatus.PENDING);
    assert.ok(disabled.nextRunAt);

    assert.equal(s.resumeJob('missing'), null);
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
