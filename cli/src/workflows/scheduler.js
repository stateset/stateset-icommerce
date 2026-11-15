/**
 * Autonomous Job Scheduler for StateSet Commerce
 *
 * Enables AI agents to schedule recurring tasks:
 * - Cron-based scheduling (e.g., "0 * * * *" for hourly)
 * - Interval-based scheduling (e.g., every 30 minutes)
 * - One-time delayed execution
 * - Persistent job storage (survives restarts)
 */

import { EventEmitter } from 'events';
import { randomUUID } from 'crypto';
import fs from 'fs';
import path from 'path';

// ============================================================================
// Constants
// ============================================================================

const MAX_CRON_ITERATIONS = 525_600; // minutes in a year
const DEFAULT_JOB_TIMEOUT_MS = 5 * 60 * 1000; // 5 minutes
const DEFAULT_TICK_INTERVAL_MS = 60_000; // 1 minute

/**
 * @typedef {'pending' | 'running' | 'retrying' | 'completed' | 'failed' | 'paused' | 'cancelled'} JobStatusValue
 * @typedef {'cron' | 'interval' | 'once'} JobType
 * @typedef {{ minute: string, hour: string, dayOfMonth: string, month: string, dayOfWeek: string }} ParsedCron
 * @typedef {{ agent?: string, request?: string, [key: string]: unknown }} JobAction
 * @typedef {{ id?: string, name: string, description?: string, type?: JobType, schedule?: string | number | null, action: JobAction, enabled?: boolean, maxRetries?: number, retryDelay?: number, timeout?: number, metadata?: Record<string, unknown>, createdAt?: string, lastRunAt?: string | null, nextRunAt?: string | null, runCount?: number, failCount?: number, lastError?: string | null, status?: JobStatusValue }} JobInput
 * @typedef {{ jobId: string, runId?: string, status: JobStatusValue, startedAt: string, completedAt?: string | null, duration?: number | null, output?: unknown, error?: string | null, retryCount?: number }} JobResultInput
 * @typedef {{ storePath?: string | null, tickInterval?: number, maxConcurrentJobs?: number, defaultJobTimeout?: number, executor?: ((action: JobAction, context: { jobId: string, runId: string, signal: AbortSignal, metadata: Record<string, unknown> }) => Promise<unknown>) | null }} SchedulerOptions
 */

/**
 * @param {unknown} error
 * @returns {string}
 */
function getErrorMessage(error) {
  if (error instanceof Error) {
    return error.message;
  }
  return String(error);
}

/**
 * Parse a cron expression into next execution time
 * Supports: minute hour day-of-month month day-of-week
 * @param {string} token
 * @param {number} min
 * @param {number} max
 * @param {string} fieldLabel
 */
function parseCronFieldToken(token, min, max, fieldLabel) {
  if (token.startsWith('*/')) {
    const stepToken = token.slice(2).trim();
    if (!/^\d+$/.test(stepToken)) {
      throw new Error(
        `Invalid cron field token '${token}' for ${fieldLabel}: step value must be a positive integer`,
      );
    }

    const step = Number.parseInt(stepToken, 10);

    if (!Number.isInteger(step) || step <= 0) {
      throw new Error(
        `Invalid cron field token '${token}' for ${fieldLabel}: step must be a positive integer`,
      );
    }

    return;
  }

  if (token.includes('-')) {
    const [startValue, endValue] = token.split('-');

    if (!startValue || !endValue) {
      throw new Error(`Invalid cron field token '${token}' for ${fieldLabel}: malformed range`);
    }

    if (!/^\d+$/.test(startValue) || !/^\d+$/.test(endValue)) {
      throw new Error(
        `Invalid cron field token '${token}' for ${fieldLabel}: range must be integers`,
      );
    }

    const start = Number.parseInt(startValue, 10);
    const end = Number.parseInt(endValue, 10);

    if (!Number.isInteger(start) || !Number.isInteger(end)) {
      throw new Error(
        `Invalid cron field token '${token}' for ${fieldLabel}: range must be integers`,
      );
    }

    if (start < min || start > max || end < min || end > max) {
      throw new Error(
        `Invalid cron field token '${token}' for ${fieldLabel}: values must be between ${min} and ${max}`,
      );
    }

    if (start > end) {
      throw new Error(`Invalid cron field token '${token}' for ${fieldLabel}: range start > end`);
    }

    return;
  }

  if (!/^\d+$/.test(token)) {
    throw new Error(
      `Invalid cron field token '${token}' for ${fieldLabel}: value must be a number between ${min} and ${max}`,
    );
  }

  const parsed = Number.parseInt(token, 10);
  if (!Number.isInteger(parsed) || parsed < min || parsed > max) {
    throw new Error(
      `Invalid cron field token '${token}' for ${fieldLabel}: value must be between ${min} and ${max}`,
    );
  }
}

/**
 * @param {string | number} field
 * @param {number} min
 * @param {number} max
 * @param {string} fieldLabel
 * @returns {string}
 */
function parseCronField(field, min, max, fieldLabel) {
  const trimmed = `${field}`.trim();
  if (!trimmed) {
    throw new Error(`Invalid cron field for ${fieldLabel}: empty value`);
  }

  if (trimmed === '*') return trimmed;

  const tokens = trimmed.split(',');
  for (const token of tokens) {
    parseCronFieldToken(token.trim(), min, max, fieldLabel);
  }

  return trimmed;
}

/**
 * @param {string} expression
 * @returns {ParsedCron}
 */
function parseCron(expression) {
  const trimmedExpression = `${expression}`.trim();
  const parts = trimmedExpression.split(/\s+/);

  if (!trimmedExpression) {
    throw new Error('Invalid cron expression: empty expression');
  }

  if (parts.length !== 5) {
    throw new Error(
      `Invalid cron expression: ${expression}. Expected 5 parts (min hour dom month dow)`,
    );
  }

  const [minute, hour, dayOfMonth, month, dayOfWeek] = parts;

  parseCronField(minute, 0, 59, 'minute');
  parseCronField(hour, 0, 23, 'hour');
  parseCronField(dayOfMonth, 1, 31, 'day of month');
  parseCronField(month, 1, 12, 'month');
  parseCronField(dayOfWeek, 0, 6, 'day of week');

  return { minute, hour, dayOfMonth, month, dayOfWeek };
}

/**
 * Check if a cron field matches a value
 */
/**
 * @param {string} field
 * @param {number} value
 * @param {number} _max
 * @returns {boolean}
 */
function cronFieldMatches(field, value, _max) {
  if (field === '*') return true;

  // Handle step values: */5
  if (field.startsWith('*/')) {
    const step = parseInt(field.slice(2), 10);
    return value % step === 0;
  }

  // Handle ranges: 1-5
  if (field.includes('-')) {
    const [start, end] = field.split('-').map((n) => parseInt(n, 10));
    return value >= start && value <= end;
  }

  // Handle lists: 1,3,5
  if (field.includes(',')) {
    return field.split(',').some((token) => cronFieldMatches(token.trim(), value, _max));
  }

  // Direct match
  return parseInt(field, 10) === value;
}

/**
 * Check if current time matches a cron expression
 */
/**
 * @param {ParsedCron} cron
 * @param {Date} [date]
 * @returns {boolean}
 */
function cronMatches(cron, date = new Date()) {
  const { minute, hour, dayOfMonth, month, dayOfWeek } = cron;

  return (
    cronFieldMatches(minute, date.getMinutes(), 59) &&
    cronFieldMatches(hour, date.getHours(), 23) &&
    cronFieldMatches(dayOfMonth, date.getDate(), 31) &&
    cronFieldMatches(month, date.getMonth() + 1, 12) &&
    cronFieldMatches(dayOfWeek, date.getDay(), 6)
  );
}

/**
 * Calculate next execution time for a cron expression
 */
/**
 * @param {string} expression
 * @param {Date} [fromDate]
 * @returns {Date}
 */
function getNextCronTime(expression, fromDate = new Date()) {
  const cron = parseCron(expression);
  const next = new Date(fromDate);
  next.setSeconds(0);
  next.setMilliseconds(0);
  next.setMinutes(next.getMinutes() + 1);

  // Search up to 1 year ahead
  const maxIterations = MAX_CRON_ITERATIONS;
  for (let i = 0; i < maxIterations; i++) {
    if (cronMatches(cron, next)) {
      return next;
    }
    next.setMinutes(next.getMinutes() + 1);
  }

  throw new Error(`Could not find next execution time for: ${expression}`);
}

/**
 * Job status enumeration.
 *
 * - `pending`   — scheduled, waiting for `nextRunAt` (or added disabled).
 * - `running`   — an execution is in flight.
 * - `retrying`  — the last attempt failed and one retry is armed; `nextRunAt`
 *                 is when it fires. Exactly `maxRetries` retries follow a
 *                 failed run; the tick never runs a job that is waiting on its
 *                 retry.
 * - `completed` / `failed` / `cancelled` — outcome of the last run. A
 *                 recurring job stays enabled and runs again at `nextRunAt`;
 *                 a one-time job is disabled.
 * - `paused`    — disabled by `pauseJob`; stays paused (even if a run was in
 *                 flight when it was paused) until `resumeJob`.
 */
/** @type {{ PENDING: JobStatusValue, RUNNING: JobStatusValue, RETRYING: JobStatusValue, COMPLETED: JobStatusValue, FAILED: JobStatusValue, PAUSED: JobStatusValue, CANCELLED: JobStatusValue }} */
export const JobStatus = {
  PENDING: 'pending',
  RUNNING: 'running',
  RETRYING: 'retrying',
  COMPLETED: 'completed',
  FAILED: 'failed',
  PAUSED: 'paused',
  CANCELLED: 'cancelled',
};

/**
 * Job definition
 */
export class Job {
  /**
   * @param {JobInput} param0
   */
  constructor({
    id = randomUUID(),
    name,
    description = '',
    type = 'cron', // 'cron', 'interval', 'once'
    schedule = null, // cron expression or interval ms
    action, // { agent, request } or function name
    enabled = true,
    maxRetries = 3,
    retryDelay = 5000,
    timeout = DEFAULT_JOB_TIMEOUT_MS,
    metadata = {},
    createdAt = new Date().toISOString(),
    lastRunAt = null,
    nextRunAt = null,
    runCount = 0,
    failCount = 0,
    lastError = null,
    status = JobStatus.PENDING,
  }) {
    this.id = id;
    this.name = name;
    this.description = description;
    this.type = type;
    this.schedule = schedule;
    this.action = action;
    this.enabled = enabled;
    this.maxRetries = maxRetries;
    this.retryDelay = retryDelay;
    this.timeout = timeout;
    this.metadata = metadata;
    this.createdAt = createdAt;
    this.lastRunAt = lastRunAt;
    this.nextRunAt = nextRunAt;
    this.runCount = runCount;
    this.failCount = failCount;
    this.lastError = lastError;
    this.status = status;
  }

  /**
   * Calculate next run time based on schedule type
   * @param {Date} [fromDate]
   * @returns {Date | null}
   */
  calculateNextRun(fromDate = new Date()) {
    if (!this.enabled) return null;

    switch (this.type) {
      case 'cron': {
        if (typeof this.schedule !== 'string') {
          throw new Error(`Invalid cron schedule for job ${this.id}`);
        }
        return getNextCronTime(this.schedule, fromDate);
      }

      case 'interval': {
        if (typeof this.schedule !== 'number') {
          throw new Error(`Invalid interval schedule for job ${this.id}`);
        }
        return new Date(fromDate.getTime() + this.schedule);
      }

      case 'once': {
        // One-time jobs: schedule is the target date
        if (this.schedule === null || this.schedule === undefined) {
          return null;
        }
        const targetDate = new Date(this.schedule);
        return targetDate > fromDate ? targetDate : null;
      }

      default:
        throw new Error(`Unknown job type: ${this.type}`);
    }
  }

  toJSON() {
    return {
      id: this.id,
      name: this.name,
      description: this.description,
      type: this.type,
      schedule: this.schedule,
      action: this.action,
      enabled: this.enabled,
      maxRetries: this.maxRetries,
      retryDelay: this.retryDelay,
      timeout: this.timeout,
      metadata: this.metadata,
      createdAt: this.createdAt,
      lastRunAt: this.lastRunAt,
      nextRunAt: this.nextRunAt,
      runCount: this.runCount,
      failCount: this.failCount,
      lastError: this.lastError,
      status: this.status,
    };
  }
}

/**
 * Job execution result
 */
export class JobResult {
  /**
   * @param {JobResultInput} param0
   */
  constructor({
    jobId,
    runId = randomUUID(),
    status,
    startedAt,
    completedAt = null,
    duration = null,
    output = null,
    error = null,
    retryCount = 0,
  }) {
    this.jobId = jobId;
    this.runId = runId;
    this.status = status;
    this.startedAt = startedAt;
    this.completedAt = completedAt;
    this.duration = duration;
    this.output = output;
    this.error = error;
    this.retryCount = retryCount;
  }
}

/**
 * A promise that rejects with an AbortError when `signal` aborts (and never
 * resolves). Listener is `once`, so it is released as soon as it fires.
 * @param {AbortSignal} signal
 * @returns {Promise<never>}
 */
function rejectOnAbort(signal) {
  return new Promise((_resolve, reject) => {
    const fail = () => {
      const abortError = new Error('Job aborted');
      abortError.name = 'AbortError';
      reject(abortError);
    };
    if (signal.aborted) {
      fail();
      return;
    }
    signal.addEventListener('abort', fail, { once: true });
  });
}

/**
 * Autonomous Scheduler
 *
 * Manages job scheduling, execution, and persistence
 */
export class Scheduler extends EventEmitter {
  /**
   * @param {SchedulerOptions} [param0]
   */
  constructor(param0 = {}) {
    super();
    const {
      storePath = null,
      tickInterval = DEFAULT_TICK_INTERVAL_MS,
      maxConcurrentJobs = 5,
      executor = null, // Function to execute job actions
      defaultJobTimeout = DEFAULT_JOB_TIMEOUT_MS,
    } = param0;

    this.storePath = storePath;
    this.tickInterval = tickInterval;
    this.maxConcurrentJobs = maxConcurrentJobs;
    this.executor = executor;
    /** Executor timeout applied to jobs added without an explicit `timeout`. */
    this.defaultJobTimeout = defaultJobTimeout;

    /** @type {Map<string, Job>} */
    this.jobs = new Map();
    /** @type {Map<string, { runId: string, abortController: AbortController, startedAt: Date, cancelled?: boolean }>} */
    this.runningJobs = new Map();
    /** @type {JobResult[]} */
    this.jobHistory = [];
    /**
     * The armed retry per job (at most one). Keyed by job id so pausing,
     * cancelling or removing a job can disarm its retry.
     * @type {Map<string, ReturnType<typeof setTimeout>>}
     */
    this.retryTimers = new Map();
    /** @type {ReturnType<typeof setInterval> | null} */
    this.tickTimer = null;
    this.isRunning = false;
  }

  /**
   * Load jobs from persistent storage
   * @returns {Promise<void>}
   */
  async load() {
    if (!this.storePath) return;

    const jobsFile = path.join(this.storePath, 'jobs.json');
    const historyFile = path.join(this.storePath, 'job-history.json');

    try {
      try {
        const data = JSON.parse(await fs.promises.readFile(jobsFile, 'utf-8'));
        for (const jobData of data) {
          const job = new Job(jobData);
          this.jobs.set(job.id, job);
        }
        this.emit('loaded', { jobCount: this.jobs.size });
      } catch (error) {
        const ioError = /** @type {{ code?: string }} */ (error);
        if (ioError.code !== 'ENOENT') {
          throw error;
        }
      }

      try {
        this.jobHistory = JSON.parse(await fs.promises.readFile(historyFile, 'utf-8'));
      } catch (error) {
        const ioError = /** @type {{ code?: string }} */ (error);
        if (ioError.code !== 'ENOENT') {
          throw error;
        }
      }
    } catch (error) {
      this.emit('error', { type: 'load', error });
    }
  }

  /**
   * Save jobs to persistent storage
   * @returns {Promise<void>}
   */
  async save() {
    if (!this.storePath) return;

    try {
      await fs.promises.mkdir(this.storePath, { recursive: true });

      const jobsFile = path.join(this.storePath, 'jobs.json');
      const historyFile = path.join(this.storePath, 'job-history.json');

      const jobsData = Array.from(this.jobs.values()).map((j) => j.toJSON());
      await fs.promises.writeFile(jobsFile, JSON.stringify(jobsData, null, 2));

      // Keep last 1000 history entries
      const recentHistory = this.jobHistory.slice(-1000);
      await fs.promises.writeFile(historyFile, JSON.stringify(recentHistory, null, 2));

      this.emit('saved', { jobCount: this.jobs.size });
    } catch (error) {
      this.emit('error', { type: 'save', error });
    }
  }

  /**
   * Add a new job
   * @param {Job | JobInput} jobConfig
   * @returns {Job}
   */
  addJob(jobConfig) {
    const job =
      jobConfig instanceof Job
        ? jobConfig
        : new Job({ timeout: this.defaultJobTimeout, ...jobConfig });

    // Calculate initial next run time
    job.nextRunAt = job.calculateNextRun()?.toISOString() || null;

    this.jobs.set(job.id, job);
    this.emit('job:added', { job });
    this.save();

    return job;
  }

  /**
   * Remove a job
   * @param {string} jobId
   * @returns {boolean}
   */
  removeJob(jobId) {
    const job = this.jobs.get(jobId);
    if (!job) return false;

    // Cancel if running or waiting on a retry
    if (this.runningJobs.has(jobId) || this.retryTimers.has(jobId)) {
      this.cancelJob(jobId);
    }

    this.jobs.delete(jobId);
    this.emit('job:removed', { jobId });
    this.save();

    return true;
  }

  /**
   * Update a job
   * @param {string} jobId
   * @param {Partial<JobInput>} updates
   * @returns {Job | null}
   */
  updateJob(jobId, updates) {
    const job = this.jobs.get(jobId);
    if (!job) return null;

    Object.assign(job, updates);

    // Recalculate next run if schedule changed
    if (updates.schedule || updates.type || updates.enabled !== undefined) {
      job.nextRunAt = job.calculateNextRun()?.toISOString() || null;
    }

    this.emit('job:updated', { job });
    this.save();

    return job;
  }

  /**
   * Pause a job. Disarms a pending retry; a run already in flight finishes,
   * but the job stays paused afterwards and is not retried.
   * @param {string} jobId
   * @returns {Job | null}
   */
  pauseJob(jobId) {
    if (!this.jobs.has(jobId)) return null;
    this._clearRetry(jobId);
    return this.updateJob(jobId, { enabled: false, status: JobStatus.PAUSED });
  }

  /**
   * Resume a paused job (or enable a job that was added disabled).
   *
   * A job that is already enabled — running, retrying, or scheduled after its
   * last run — has nothing to resume and is returned unchanged. A one-time job
   * that already ran (completed, failed or cancelled) cannot be resumed.
   *
   * @param {string} jobId
   * @returns {Job | null} The job, or null when it does not exist
   * @throws {Error} When the job is disabled for a reason other than a pause
   */
  resumeJob(jobId) {
    const job = this.jobs.get(jobId);
    if (!job) return null;

    const addedDisabled = !job.enabled && job.status === JobStatus.PENDING;
    if (job.status === JobStatus.PAUSED || addedDisabled) {
      return this.updateJob(jobId, { enabled: true, status: JobStatus.PENDING });
    }
    if (job.enabled) {
      return job;
    }
    throw new Error(
      `Cannot resume job ${jobId}: it is ${job.status}; only a paused job can be resumed`,
    );
  }

  /**
   * Cancel a job's current run, or its pending retry.
   *
   * Cancelling stops this run; it does not re-run it. A one-time job is
   * disabled (its only run was cancelled). A recurring job stays enabled and
   * next runs at its next scheduled time after the cancellation — use
   * `pauseJob` to stop it altogether. A paused recurring job stays paused.
   *
   * @param {string} jobId
   * @returns {boolean} Whether there was a run or retry to cancel
   */
  cancelJob(jobId) {
    const running = this.runningJobs.get(jobId);
    const disarmedRetry = this._clearRetry(jobId);
    if (!running && !disarmedRetry) return false;

    if (running) {
      running.cancelled = true;
      if (running.abortController) {
        running.abortController.abort('cancelled');
      }
    }

    const job = this.jobs.get(jobId);
    if (job) {
      this._settleCancelled(job, new Date());
      this.save();
    }

    this.emit('job:cancelled', { jobId });
    return true;
  }

  /**
   * Disarm a job's pending retry.
   * @private
   * @param {string} jobId
   * @returns {boolean} Whether a retry was armed
   */
  _clearRetry(jobId) {
    const timer = this.retryTimers.get(jobId);
    if (timer === undefined) return false;
    clearTimeout(timer);
    this.retryTimers.delete(jobId);
    return true;
  }

  /**
   * A recurring job paused while a run was in flight stays paused whatever
   * that run's outcome; a one-time job's run is its whole life, so the outcome
   * stands.
   * @private
   * @param {Job} job
   * @returns {boolean}
   */
  _staysPaused(job) {
    return job.type !== 'once' && job.status === JobStatus.PAUSED;
  }

  /**
   * Record a cancelled run so it does not run again straight away.
   * @private
   * @param {Job} job
   * @param {Date} at
   */
  _settleCancelled(job, at) {
    if (this._staysPaused(job)) return;
    job.status = JobStatus.CANCELLED;
    if (job.type === 'once') {
      job.enabled = false;
      job.nextRunAt = null;
    } else {
      job.nextRunAt = job.calculateNextRun(at)?.toISOString() || null;
    }
  }

  /**
   * Arm the job's single retry. While it is armed the job reads `retrying`,
   * `nextRunAt` says when the retry fires, and `getDueJobs` skips it, so the
   * tick cannot start extra attempts (each of which used to arm yet another
   * retry). Pause, cancel and remove disarm it.
   * @private
   * @param {Job} job
   * @param {number} retryCount - The attempt number the retry will run as
   * @param {number} delayMs
   */
  _armRetry(job, retryCount, delayMs) {
    this._clearRetry(job.id);
    job.status = JobStatus.RETRYING;
    job.nextRunAt = new Date(Date.now() + delayMs).toISOString();

    const retryTimer = setTimeout(() => {
      if (this.retryTimers.get(job.id) !== retryTimer) return;
      this.retryTimers.delete(job.id);
      if (this.jobs.get(job.id) !== job || !job.enabled || this.runningJobs.has(job.id)) return;
      this.executeJob(job, retryCount).catch((error) => {
        this.emit('error', { type: 'execution', jobId: job.id, error });
      });
    }, delayMs);
    this.retryTimers.set(job.id, retryTimer);
    if (typeof retryTimer.unref === 'function') {
      retryTimer.unref();
    }
  }

  /**
   * Get a job by ID
   * @param {string} jobId
   * @returns {Job | undefined}
   */
  getJob(jobId) {
    return this.jobs.get(jobId);
  }

  /**
   * List all jobs
   * @param {{ status?: JobStatusValue | null, enabled?: boolean | null }} [param0]
   * @returns {Job[]}
   */
  listJobs({ status = null, enabled = null } = {}) {
    let jobs = Array.from(this.jobs.values());

    if (status !== null) {
      jobs = jobs.filter((j) => j.status === status);
    }

    if (enabled !== null) {
      jobs = jobs.filter((j) => j.enabled === enabled);
    }

    return jobs.sort((a, b) => {
      if (!a.nextRunAt) return 1;
      if (!b.nextRunAt) return -1;
      return new Date(a.nextRunAt).getTime() - new Date(b.nextRunAt).getTime();
    });
  }

  /**
   * Get jobs due for execution
   * @param {Date} [now]
   * @returns {Job[]}
   */
  getDueJobs(now = new Date()) {
    return Array.from(this.jobs.values()).filter((job) => {
      if (!job.enabled) return false;
      if (!job.nextRunAt) return false;
      if (this.runningJobs.has(job.id)) return false;
      // Waiting on its retry: the retry timer runs it, not the tick.
      if (this.retryTimers.has(job.id)) return false;

      return new Date(job.nextRunAt) <= now;
    });
  }

  /**
   * Execute a job
   * @param {Job} job
   * @param {number} [retryCount]
   * @returns {Promise<JobResult>}
   */
  async executeJob(job, retryCount = 0) {
    const runId = randomUUID();
    const startedAt = new Date();

    // A run started any other way (runNow during a backoff) supersedes an
    // armed retry, so the job never has two attempts in flight.
    this._clearRetry(job.id);

    // Mark as running
    job.status = JobStatus.RUNNING;
    const abortController = new AbortController();
    this.runningJobs.set(job.id, { runId, abortController, startedAt });

    this.emit('job:started', { job, runId });

    /** @type {JobResult | null} */
    let result = null;
    let timeoutId = null;
    let timedOut = false;

    try {
      // Set up timeout
      timeoutId = setTimeout(() => {
        timedOut = true;
        abortController.abort('timeout');
      }, job.timeout);

      if (typeof timeoutId.unref === 'function') {
        timeoutId.unref();
      }

      // Execute the action, racing it against the abort signal so a timeout or
      // cancel is enforced even when the executor ignores `signal`. A late
      // settlement from such an executor is discarded.
      let output;
      if (this.executor) {
        const execution = Promise.resolve(
          this.executor(job.action, {
            jobId: job.id,
            runId,
            signal: abortController.signal,
            metadata: job.metadata,
          }),
        );
        execution.catch(() => {});
        output = await Promise.race([execution, rejectOnAbort(abortController.signal)]);
      } else {
        throw new Error('No executor configured');
      }

      // If the run was cancelled/timed out while the executor ignored the signal,
      // treat it as an abort rather than a successful completion.
      if (abortController.signal.aborted) {
        const abortError = new Error('Job aborted');
        abortError.name = 'AbortError';
        throw abortError;
      }

      // Success
      const completedAt = new Date();
      result = new JobResult({
        jobId: job.id,
        runId,
        status: JobStatus.COMPLETED,
        startedAt: startedAt.toISOString(),
        completedAt: completedAt.toISOString(),
        duration: completedAt.getTime() - startedAt.getTime(),
        output,
        retryCount,
      });

      if (!this._staysPaused(job)) {
        job.status = JobStatus.COMPLETED;
      }
      job.lastRunAt = completedAt.toISOString();
      job.runCount++;
      job.lastError = null;

      // Calculate next run (null while paused: calculateNextRun honours enabled)
      job.nextRunAt = job.calculateNextRun(completedAt)?.toISOString() || null;

      // One-time jobs: disable after completion
      if (job.type === 'once') {
        job.enabled = false;
      }

      this.emit('job:completed', { job, result });
    } catch (error) {
      const completedAt = new Date();
      const running = this.runningJobs.get(job.id);
      const cancelReason = abortController.signal.reason;
      const wasCancelled = running?.cancelled === true || cancelReason === 'cancelled';
      const wasTimeout =
        timedOut ||
        cancelReason === 'timeout' ||
        (error instanceof Error && error.name === 'AbortError' && !wasCancelled);

      if (wasCancelled) {
        result = new JobResult({
          jobId: job.id,
          runId,
          status: JobStatus.CANCELLED,
          startedAt: startedAt.toISOString(),
          completedAt: completedAt.toISOString(),
          duration: completedAt.getTime() - startedAt.getTime(),
          error: 'Job cancelled',
          retryCount,
        });

        this._settleCancelled(job, completedAt);
        job.lastRunAt = completedAt.toISOString();
        job.lastError = null;
      } else {
        const errorMessage = wasTimeout ? 'Job timed out' : getErrorMessage(error);

        result = new JobResult({
          jobId: job.id,
          runId,
          status: JobStatus.FAILED,
          startedAt: startedAt.toISOString(),
          completedAt: completedAt.toISOString(),
          duration: completedAt.getTime() - startedAt.getTime(),
          error: errorMessage,
          retryCount,
        });

        job.lastError = errorMessage;
        job.failCount++;

        // Retry — unless the job was paused or removed while this attempt ran.
        const stillScheduled = this.jobs.get(job.id) === job && job.status !== JobStatus.PAUSED;
        if (stillScheduled && retryCount < job.maxRetries) {
          this.emit('job:retry', { job, result, nextRetry: retryCount + 1 });
          this._armRetry(job, retryCount + 1, job.retryDelay * Math.pow(2, retryCount)); // Exponential backoff
        } else {
          job.lastRunAt = completedAt.toISOString();

          if (job.type === 'once') {
            job.status = JobStatus.FAILED;
            job.enabled = false;
          } else if (!this._staysPaused(job)) {
            // A recurring job keeps its schedule; `failed` stays visible
            // until the next scheduled run starts.
            job.status = JobStatus.FAILED;
            job.nextRunAt = job.calculateNextRun(completedAt)?.toISOString() || null;
          }

          this.emit('job:failed', { job, result });
        }
      }
    } finally {
      if (timeoutId) {
        clearTimeout(timeoutId);
      }
      this.runningJobs.delete(job.id);
      if (result) {
        this.jobHistory.push(result);
      }
      this.save();
    }

    if (!result) {
      throw new Error(`No result produced for job ${job.id}`);
    }
    return result;
  }

  /**
   * Run a job immediately (manual trigger)
   * @param {string} jobId
   * @returns {Promise<JobResult>}
   */
  async runNow(jobId) {
    const job = this.jobs.get(jobId);
    if (!job) {
      throw new Error(`Job not found: ${jobId}`);
    }

    if (this.runningJobs.has(jobId)) {
      throw new Error(`Job is already running: ${jobId}`);
    }

    return this.executeJob(job);
  }

  /**
   * Scheduler tick - check for due jobs
   * @returns {Promise<void>}
   */
  async tick() {
    if (!this.isRunning) return;

    const now = new Date();
    const dueJobs = this.getDueJobs(now);

    // Respect concurrency limit
    const availableSlots = this.maxConcurrentJobs - this.runningJobs.size;
    const jobsToRun = dueJobs.slice(0, availableSlots);

    for (const job of jobsToRun) {
      // Don't await - run in parallel
      this.executeJob(job).catch((error) => {
        this.emit('error', { type: 'execution', jobId: job.id, error });
      });
    }

    if (dueJobs.length > availableSlots) {
      this.emit('warning', {
        message: `${dueJobs.length - availableSlots} jobs waiting due to concurrency limit`,
      });
    }
  }

  /**
   * Start the scheduler
   * @returns {void}
   */
  start() {
    if (this.isRunning) return;

    this.isRunning = true;
    this.emit('started');

    // Initial tick
    this.tick();

    // Set up recurring tick
    this.tickTimer = setInterval(() => {
      this.tick();
    }, this.tickInterval);
    if (this.tickTimer.unref) this.tickTimer.unref();
  }

  /**
   * Stop the scheduler
   * @returns {void}
   */
  stop() {
    if (!this.isRunning) return;

    this.isRunning = false;

    if (this.tickTimer) {
      clearInterval(this.tickTimer);
      this.tickTimer = null;
    }

    // A job left `retrying` runs (as a fresh attempt) at its `nextRunAt` once
    // the scheduler is started again.
    for (const retryTimer of this.retryTimers.values()) {
      clearTimeout(retryTimer);
    }
    this.retryTimers.clear();

    this.emit('stopped');
  }

  /**
   * Get scheduler status
   * @returns {{ isRunning: boolean, totalJobs: number, enabledJobs: number, runningJobs: number, pendingJobs: number, recentHistory: JobResult[] }}
   */
  getStatus() {
    return {
      isRunning: this.isRunning,
      totalJobs: this.jobs.size,
      enabledJobs: Array.from(this.jobs.values()).filter((j) => j.enabled).length,
      runningJobs: this.runningJobs.size,
      pendingJobs: this.getDueJobs().length,
      recentHistory: this.jobHistory.slice(-10),
    };
  }

  /**
   * Get job execution history
   * @param {{ jobId?: string | null, limit?: number, status?: JobStatusValue | null }} [param0]
   * @returns {JobResult[]}
   */
  getHistory({ jobId = null, limit = 100, status = null } = {}) {
    let history = this.jobHistory;

    if (jobId) {
      history = history.filter((h) => h.jobId === jobId);
    }

    if (status) {
      history = history.filter((h) => h.status === status);
    }

    return history.slice(-limit);
  }
}

/**
 * Pre-defined job templates for common commerce operations
 */
export const JobTemplates = {
  // Inventory management
  lowStockCheck: {
    name: 'Low Stock Monitor',
    description: 'Check for low stock items and create alerts',
    type: 'cron',
    schedule: '0 * * * *', // Every hour
    action: {
      agent: 'inventory',
      request: 'Check for low stock items and list any products below their reorder point',
    },
  },

  // Order management
  abandonedCartRecovery: {
    name: 'Abandoned Cart Recovery',
    description: 'Find and process abandoned carts',
    type: 'cron',
    schedule: '0 9 * * *', // Daily at 9 AM
    action: {
      agent: 'checkout',
      request: 'List abandoned carts from the last 24 hours and summarize recovery opportunities',
    },
  },

  // Subscription management
  subscriptionRenewal: {
    name: 'Subscription Renewal Processor',
    description: 'Process due subscription renewals',
    type: 'cron',
    schedule: '0 0 * * *', // Daily at midnight
    action: {
      agent: 'subscriptions',
      request: 'List subscriptions due for renewal today and process billing cycles',
    },
  },

  // Analytics
  dailySalesReport: {
    name: 'Daily Sales Report',
    description: 'Generate daily sales summary',
    type: 'cron',
    schedule: '0 6 * * *', // Daily at 6 AM
    action: {
      agent: 'analytics',
      request:
        'Generate a sales summary for yesterday including revenue, order count, and top products',
    },
  },

  // Sync
  eventSync: {
    name: 'Event Sync',
    description: 'Sync pending events to sequencer',
    type: 'interval',
    schedule: 300000, // Every 5 minutes
    action: {
      agent: 'sync',
      request: 'Push any pending events to the sequencer and report sync status',
    },
  },

  // Promotions
  promotionActivation: {
    name: 'Promotion Scheduler',
    description: 'Activate/deactivate scheduled promotions',
    type: 'cron',
    schedule: '*/15 * * * *', // Every 15 minutes
    action: {
      agent: 'promotions',
      request:
        'Check for promotions that should be activated or deactivated based on their scheduled dates',
    },
  },

  // Invoice management
  overdueInvoiceReminder: {
    name: 'Overdue Invoice Processor',
    description: 'Process overdue invoices',
    type: 'cron',
    schedule: '0 10 * * 1', // Every Monday at 10 AM
    action: {
      agent: 'invoices',
      request: 'List overdue invoices and summarize accounts receivable status',
    },
  },
};

export default Scheduler;
