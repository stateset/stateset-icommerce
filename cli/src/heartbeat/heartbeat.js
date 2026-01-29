/**
 * Heartbeat Monitor
 *
 * Periodically runs commerce health checks and emits events when
 * conditions trigger. Designed to plug into the AutonomousEngine
 * event-forwarding pipeline so alerts reach all channels.
 *
 * Events emitted (short form — engine prefixes with "heartbeat:"):
 *   alert            – when a check triggers
 *   check:completed  – after every run
 *   check:error      – on checker failure
 *   check:enabled    – when a check is enabled
 *   check:disabled   – when a check is disabled
 */

import { EventEmitter } from 'events';
import { BUILTIN_CHECKERS } from './checkers.js';

// ============================================================================
// Default check definitions (all disabled by default)
// ============================================================================

const DEFAULT_CHECKS = [
  { id: 'low-stock',          name: 'Low Stock',           checker: 'low-stock',          intervalMs: 3_600_000,  enabled: false, config: { threshold: 10 } },
  { id: 'abandoned-carts',    name: 'Abandoned Carts',     checker: 'abandoned-carts',    intervalMs: 86_400_000, enabled: false, config: { minAgeHours: 24 } },
  { id: 'revenue-milestone',  name: 'Revenue Milestone',   checker: 'revenue-milestone',  intervalMs: 3_600_000,  enabled: false, config: { target: 10000, period: 'month' } },
  { id: 'pending-returns',    name: 'Pending Returns',     checker: 'pending-returns',    intervalMs: 43_200_000, enabled: false, config: { maxAgeDays: 7 } },
  { id: 'overdue-invoices',   name: 'Overdue Invoices',    checker: 'overdue-invoices',   intervalMs: 86_400_000, enabled: false, config: {} },
  { id: 'subscription-churn', name: 'Subscription Churn',  checker: 'subscription-churn', intervalMs: 86_400_000, enabled: false, config: {} },
];

// ============================================================================
// HeartbeatMonitor
// ============================================================================

export class HeartbeatMonitor extends EventEmitter {
  /**
   * @param {Object} opts
   * @param {Array} [opts.checks] - Check definitions (merged with defaults)
   * @param {Object} opts.commerce - StateSet Commerce instance
   * @param {boolean} [opts.verbose=false]
   */
  constructor({ checks = null, commerce, verbose = false } = {}) {
    super();

    this._commerce = commerce;
    this._verbose = verbose;
    this._running = false;

    /** @type {Map<string, Object>} */
    this._timers = new Map();

    // Build check state map from defaults + overrides
    this._checks = new Map();
    const defs = checks || DEFAULT_CHECKS;
    for (const def of defs) {
      this._checks.set(def.id, {
        id: def.id,
        name: def.name || def.id,
        checker: def.checker || def.id,
        intervalMs: def.intervalMs || 3_600_000,
        enabled: def.enabled ?? false,
        config: def.config || {},
        // Runtime state
        lastRunAt: null,
        lastTriggeredAt: null,
        lastResult: null,
        runCount: 0,
        triggerCount: 0,
      });
    }
  }

  // --------------------------------------------------------------------------
  // Lifecycle
  // --------------------------------------------------------------------------

  /**
   * Start all enabled checks.
   */
  start() {
    if (this._running) return;
    this._running = true;

    for (const [id, check] of this._checks) {
      if (check.enabled) {
        this._schedule(id);
      }
    }

    if (this._verbose) {
      const enabled = [...this._checks.values()].filter((c) => c.enabled).length;
      console.log(`[Heartbeat] Started — ${enabled}/${this._checks.size} checks enabled`);
    }

    this.emit('started');
  }

  /**
   * Stop all timers.
   */
  stop() {
    if (!this._running) return;

    for (const [id, timer] of this._timers) {
      clearInterval(timer);
    }
    this._timers.clear();
    this._running = false;

    if (this._verbose) {
      console.log('[Heartbeat] Stopped');
    }

    this.emit('stopped');
  }

  // --------------------------------------------------------------------------
  // Check execution
  // --------------------------------------------------------------------------

  /**
   * Run a single check by ID.
   *
   * @param {string} id
   * @returns {Promise<{ triggered: boolean, data: Object, summary: string } | null>}
   */
  async runCheck(id) {
    const check = this._checks.get(id);
    if (!check) return null;

    const checkerFn = BUILTIN_CHECKERS[check.checker];
    if (!checkerFn) {
      const err = new Error(`Unknown checker: ${check.checker}`);
      this.emit('check:error', { checkId: id, error: err.message });
      return null;
    }

    try {
      const result = await checkerFn(this._commerce, check.config);

      check.lastRunAt = Date.now();
      check.lastResult = result;
      check.runCount++;

      this.emit('check:completed', { checkId: id, checkName: check.name, result });

      if (result.triggered) {
        check.lastTriggeredAt = Date.now();
        check.triggerCount++;
        this.emit('alert', {
          checkId: id,
          checkName: check.name,
          data: result.data,
          summary: result.summary,
        });
      }

      if (this._verbose) {
        const flag = result.triggered ? 'TRIGGERED' : 'ok';
        console.log(`[Heartbeat] ${check.name}: ${flag} — ${result.summary}`);
      }

      return result;
    } catch (err) {
      this.emit('check:error', { checkId: id, error: err.message });
      return null;
    }
  }

  // --------------------------------------------------------------------------
  // Enable / Disable
  // --------------------------------------------------------------------------

  /**
   * Enable a check. Schedules it immediately if monitor is running.
   */
  enableCheck(id) {
    const check = this._checks.get(id);
    if (!check) return false;

    check.enabled = true;
    this.emit('check:enabled', { checkId: id, checkName: check.name });

    if (this._running && !this._timers.has(id)) {
      this._schedule(id);
    }
    return true;
  }

  /**
   * Disable a check. Clears its timer if running.
   */
  disableCheck(id) {
    const check = this._checks.get(id);
    if (!check) return false;

    check.enabled = false;
    this.emit('check:disabled', { checkId: id, checkName: check.name });

    if (this._timers.has(id)) {
      clearInterval(this._timers.get(id));
      this._timers.delete(id);
    }
    return true;
  }

  // --------------------------------------------------------------------------
  // Status
  // --------------------------------------------------------------------------

  /**
   * Overall monitor status.
   */
  getStatus() {
    const checks = this.listChecks();
    return {
      running: this._running,
      checkCount: checks.length,
      enabledCount: checks.filter((c) => c.enabled).length,
      checks,
    };
  }

  /**
   * Get a single check state.
   */
  getCheck(id) {
    const check = this._checks.get(id);
    if (!check) return null;
    return { ...check };
  }

  /**
   * List all check states.
   */
  listChecks() {
    return [...this._checks.values()].map((c) => ({ ...c }));
  }

  // --------------------------------------------------------------------------
  // Internal
  // --------------------------------------------------------------------------

  /**
   * Schedule a check: run immediately, then at its interval.
   */
  _schedule(id) {
    const check = this._checks.get(id);
    if (!check) return;

    // Run immediately
    this.runCheck(id);

    // Then schedule recurring
    const timer = setInterval(() => this.runCheck(id), check.intervalMs);
    timer.unref?.(); // Don't prevent process exit
    this._timers.set(id, timer);
  }
}
