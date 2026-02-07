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

/**
 * Parse a cron expression into next execution time
 * Supports: minute hour day-of-month month day-of-week
 */
function parseCron(expression) {
  const parts = expression.trim().split(/\s+/);
  if (parts.length !== 5) {
    throw new Error(
      `Invalid cron expression: ${expression}. Expected 5 parts (min hour dom month dow)`,
    );
  }

  const [minute, hour, dayOfMonth, month, dayOfWeek] = parts;

  return { minute, hour, dayOfMonth, month, dayOfWeek };
}

/**
 * Check if a cron field matches a value
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
    const values = field.split(',').map((n) => parseInt(n, 10));
    return values.includes(value);
  }

  // Direct match
  return parseInt(field, 10) === value;
}

/**
 * Check if current time matches a cron expression
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
function getNextCronTime(expression, fromDate = new Date()) {
  const cron = parseCron(expression);
  const next = new Date(fromDate);
  next.setSeconds(0);
  next.setMilliseconds(0);
  next.setMinutes(next.getMinutes() + 1);

  // Search up to 1 year ahead
  const maxIterations = 525600; // minutes in a year
  for (let i = 0; i < maxIterations; i++) {
    if (cronMatches(cron, next)) {
      return next;
    }
    next.setMinutes(next.getMinutes() + 1);
  }

  throw new Error(`Could not find next execution time for: ${expression}`);
}

/**
 * Job status enumeration
 */
export const JobStatus = {
  PENDING: 'pending',
  RUNNING: 'running',
  COMPLETED: 'completed',
  FAILED: 'failed',
  PAUSED: 'paused',
  CANCELLED: 'cancelled',
};

/**
 * Job definition
 */
export class Job {
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
    timeout = 300000, // 5 minutes default
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
   */
  calculateNextRun(fromDate = new Date()) {
    if (!this.enabled) return null;

    switch (this.type) {
      case 'cron':
        return getNextCronTime(this.schedule, fromDate);

      case 'interval':
        return new Date(fromDate.getTime() + this.schedule);

      case 'once': {
        // One-time jobs: schedule is the target date
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
 * Autonomous Scheduler
 *
 * Manages job scheduling, execution, and persistence
 */
export class Scheduler extends EventEmitter {
  constructor({
    storePath = null,
    tickInterval = 60000, // Check every minute
    maxConcurrentJobs = 5,
    executor = null, // Function to execute job actions
  }) {
    super();

    this.storePath = storePath;
    this.tickInterval = tickInterval;
    this.maxConcurrentJobs = maxConcurrentJobs;
    this.executor = executor;

    this.jobs = new Map();
    this.runningJobs = new Map();
    this.jobHistory = [];
    this.tickTimer = null;
    this.isRunning = false;
  }

  /**
   * Load jobs from persistent storage
   */
  async load() {
    if (!this.storePath) return;

    const jobsFile = path.join(this.storePath, 'jobs.json');
    const historyFile = path.join(this.storePath, 'job-history.json');

    try {
      if (fs.existsSync(jobsFile)) {
        const data = JSON.parse(fs.readFileSync(jobsFile, 'utf-8'));
        for (const jobData of data) {
          const job = new Job(jobData);
          this.jobs.set(job.id, job);
        }
        this.emit('loaded', { jobCount: this.jobs.size });
      }

      if (fs.existsSync(historyFile)) {
        this.jobHistory = JSON.parse(fs.readFileSync(historyFile, 'utf-8'));
      }
    } catch (error) {
      this.emit('error', { type: 'load', error });
    }
  }

  /**
   * Save jobs to persistent storage
   */
  async save() {
    if (!this.storePath) return;

    try {
      fs.mkdirSync(this.storePath, { recursive: true });

      const jobsFile = path.join(this.storePath, 'jobs.json');
      const historyFile = path.join(this.storePath, 'job-history.json');

      const jobsData = Array.from(this.jobs.values()).map((j) => j.toJSON());
      fs.writeFileSync(jobsFile, JSON.stringify(jobsData, null, 2));

      // Keep last 1000 history entries
      const recentHistory = this.jobHistory.slice(-1000);
      fs.writeFileSync(historyFile, JSON.stringify(recentHistory, null, 2));

      this.emit('saved', { jobCount: this.jobs.size });
    } catch (error) {
      this.emit('error', { type: 'save', error });
    }
  }

  /**
   * Add a new job
   */
  addJob(jobConfig) {
    const job = jobConfig instanceof Job ? jobConfig : new Job(jobConfig);

    // Calculate initial next run time
    job.nextRunAt = job.calculateNextRun()?.toISOString() || null;

    this.jobs.set(job.id, job);
    this.emit('job:added', { job });
    this.save();

    return job;
  }

  /**
   * Remove a job
   */
  removeJob(jobId) {
    const job = this.jobs.get(jobId);
    if (!job) return false;

    // Cancel if running
    if (this.runningJobs.has(jobId)) {
      this.cancelJob(jobId);
    }

    this.jobs.delete(jobId);
    this.emit('job:removed', { jobId });
    this.save();

    return true;
  }

  /**
   * Update a job
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
   * Pause a job
   */
  pauseJob(jobId) {
    return this.updateJob(jobId, { enabled: false, status: JobStatus.PAUSED });
  }

  /**
   * Resume a job
   */
  resumeJob(jobId) {
    return this.updateJob(jobId, { enabled: true, status: JobStatus.PENDING });
  }

  /**
   * Cancel a running job
   */
  cancelJob(jobId) {
    const running = this.runningJobs.get(jobId);
    if (!running) return false;

    if (running.abortController) {
      running.abortController.abort();
    }

    this.runningJobs.delete(jobId);

    const job = this.jobs.get(jobId);
    if (job) {
      job.status = JobStatus.CANCELLED;
      this.save();
    }

    this.emit('job:cancelled', { jobId });
    return true;
  }

  /**
   * Get a job by ID
   */
  getJob(jobId) {
    return this.jobs.get(jobId);
  }

  /**
   * List all jobs
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
      return new Date(a.nextRunAt) - new Date(b.nextRunAt);
    });
  }

  /**
   * Get jobs due for execution
   */
  getDueJobs(now = new Date()) {
    return Array.from(this.jobs.values()).filter((job) => {
      if (!job.enabled) return false;
      if (!job.nextRunAt) return false;
      if (this.runningJobs.has(job.id)) return false;

      return new Date(job.nextRunAt) <= now;
    });
  }

  /**
   * Execute a job
   */
  async executeJob(job, retryCount = 0) {
    const runId = randomUUID();
    const startedAt = new Date();

    // Mark as running
    job.status = JobStatus.RUNNING;
    const abortController = new AbortController();
    this.runningJobs.set(job.id, { runId, abortController, startedAt });

    this.emit('job:started', { job, runId });

    let result;

    try {
      // Set up timeout
      const timeoutId = setTimeout(() => {
        abortController.abort();
      }, job.timeout);

      // Execute the action
      let output;
      if (this.executor) {
        output = await this.executor(job.action, {
          jobId: job.id,
          runId,
          signal: abortController.signal,
          metadata: job.metadata,
        });
      } else {
        throw new Error('No executor configured');
      }

      clearTimeout(timeoutId);

      // Success
      const completedAt = new Date();
      result = new JobResult({
        jobId: job.id,
        runId,
        status: JobStatus.COMPLETED,
        startedAt: startedAt.toISOString(),
        completedAt: completedAt.toISOString(),
        duration: completedAt - startedAt,
        output,
        retryCount,
      });

      job.status = JobStatus.COMPLETED;
      job.lastRunAt = completedAt.toISOString();
      job.runCount++;
      job.lastError = null;

      // Calculate next run
      job.nextRunAt = job.calculateNextRun(completedAt)?.toISOString() || null;

      // One-time jobs: disable after completion
      if (job.type === 'once') {
        job.enabled = false;
      }

      this.emit('job:completed', { job, result });
    } catch (error) {
      const completedAt = new Date();
      const errorMessage = error.name === 'AbortError' ? 'Job timed out' : error.message;

      result = new JobResult({
        jobId: job.id,
        runId,
        status: JobStatus.FAILED,
        startedAt: startedAt.toISOString(),
        completedAt: completedAt.toISOString(),
        duration: completedAt - startedAt,
        error: errorMessage,
        retryCount,
      });

      job.lastError = errorMessage;
      job.failCount++;

      // Retry logic
      if (retryCount < job.maxRetries) {
        this.emit('job:retry', { job, result, nextRetry: retryCount + 1 });

        // Schedule retry
        setTimeout(
          () => {
            this.executeJob(job, retryCount + 1);
          },
          job.retryDelay * Math.pow(2, retryCount),
        ); // Exponential backoff
      } else {
        job.status = JobStatus.FAILED;
        job.lastRunAt = completedAt.toISOString();

        // Still calculate next run for recurring jobs
        if (job.type !== 'once') {
          job.nextRunAt = job.calculateNextRun(completedAt)?.toISOString() || null;
          job.status = JobStatus.PENDING; // Reset for next scheduled run
        } else {
          job.enabled = false;
        }

        this.emit('job:failed', { job, result });
      }
    } finally {
      this.runningJobs.delete(job.id);
      this.jobHistory.push(result);
      this.save();
    }

    return result;
  }

  /**
   * Run a job immediately (manual trigger)
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
  }

  /**
   * Stop the scheduler
   */
  stop() {
    if (!this.isRunning) return;

    this.isRunning = false;

    if (this.tickTimer) {
      clearInterval(this.tickTimer);
      this.tickTimer = null;
    }

    this.emit('stopped');
  }

  /**
   * Get scheduler status
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
