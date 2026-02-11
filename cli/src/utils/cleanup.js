/**
 * Resource Cleanup Utilities
 *
 * Manages event listeners, timers, and cleanup callbacks to prevent
 * memory leaks in long-running processes (gateways, workers, daemons).
 */

/**
 * Manages timers with tracking for cleanup
 */
export class CleanupTimer {
  constructor() {
    /** @type {Set<ReturnType<typeof setTimeout>>} */
    this._timers = new Set();
  }

  /**
   * @param {Function} fn
   * @param {number} delay
   * @param {...*} args
   * @returns {ReturnType<typeof setTimeout>}
   */
  setTimeout(fn, delay, ...args) {
    const id = setTimeout(() => {
      this._timers.delete(id);
      fn(...args);
    }, delay);
    this._timers.add(id);
    return id;
  }

  /**
   * @param {Function} fn
   * @param {number} delay
   * @param {...*} args
   * @returns {ReturnType<typeof setInterval>}
   */
  setInterval(fn, delay, ...args) {
    const id = setInterval(fn, delay, ...args);
    this._timers.add(id);
    return id;
  }

  /** @param {ReturnType<typeof setTimeout>} id */
  clearTimeout(id) {
    clearTimeout(id);
    this._timers.delete(id);
  }

  /** @param {ReturnType<typeof setInterval>} id */
  clearInterval(id) {
    clearInterval(id);
    this._timers.delete(id);
  }

  clearAll() {
    for (const id of this._timers) {
      clearTimeout(id);
    }
    this._timers.clear();
  }

  get activeCount() {
    return this._timers.size;
  }
}

/**
 * Manages event listeners with tracking for cleanup
 */
export class CleanupListeners {
  constructor() {
    /** @type {Array<{emitter: *, event: string, handler: Function}>} */
    this._listeners = [];
  }

  /**
   * Register an event listener on an EventEmitter
   * @param {import('node:events').EventEmitter} emitter
   * @param {string} event
   * @param {Function} handler
   */
  on(emitter, event, handler) {
    emitter.on(event, handler);
    this._listeners.push({ emitter, event, handler });
  }

  /**
   * Register a one-time event listener
   * @param {import('node:events').EventEmitter} emitter
   * @param {string} event
   * @param {Function} handler
   */
  once(emitter, event, handler) {
    const wrappedHandler = (...args) => {
      this._listeners = this._listeners.filter(
        (l) => !(l.emitter === emitter && l.event === event && l.handler === wrappedHandler),
      );
      handler(...args);
    };
    emitter.once(event, wrappedHandler);
    this._listeners.push({ emitter, event, handler: wrappedHandler });
  }

  removeAll() {
    for (const { emitter, event, handler } of this._listeners) {
      try {
        emitter.removeListener(event, handler);
      } catch {
        // Emitter may already be destroyed
      }
    }
    this._listeners = [];
  }

  get activeCount() {
    return this._listeners.length;
  }
}

/**
 * Combined cleanup manager for timers, listeners, and arbitrary callbacks.
 *
 * Usage:
 *   const cleanup = new CleanupManager();
 *   cleanup.setInterval(poll, 5000);
 *   cleanup.on(server, 'error', handleError);
 *   cleanup.onCleanup(() => db.close());
 *   // ...later:
 *   await cleanup.cleanup();
 */
export class CleanupManager {
  constructor() {
    this.timers = new CleanupTimer();
    this.listeners = new CleanupListeners();
    /** @type {Array<() => void | Promise<void>>} */
    this._callbacks = [];
    this._cleaned = false;
  }

  /** @param {Function} fn @param {number} delay @returns {ReturnType<typeof setTimeout>} */
  setTimeout(fn, delay, ...args) {
    return this.timers.setTimeout(fn, delay, ...args);
  }

  /** @param {Function} fn @param {number} delay @returns {ReturnType<typeof setInterval>} */
  setInterval(fn, delay, ...args) {
    return this.timers.setInterval(fn, delay, ...args);
  }

  /** @param {import('node:events').EventEmitter} emitter @param {string} event @param {Function} handler */
  on(emitter, event, handler) {
    this.listeners.on(emitter, event, handler);
  }

  /** @param {import('node:events').EventEmitter} emitter @param {string} event @param {Function} handler */
  once(emitter, event, handler) {
    this.listeners.once(emitter, event, handler);
  }

  /**
   * Register a cleanup callback
   * @param {() => void | Promise<void>} callback
   */
  onCleanup(callback) {
    this._callbacks.push(callback);
  }

  /**
   * Run all cleanup: clear timers, remove listeners, run callbacks.
   * Safe to call multiple times (idempotent after first call).
   */
  async cleanup() {
    if (this._cleaned) return;
    this._cleaned = true;

    this.timers.clearAll();
    this.listeners.removeAll();

    for (const callback of this._callbacks) {
      try {
        await callback();
      } catch (error) {
        console.warn('Cleanup callback error:', error.message);
      }
    }
    this._callbacks = [];
  }

  get isCleanedUp() {
    return this._cleaned;
  }
}
