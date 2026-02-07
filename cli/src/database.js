/**
 * Database Manager for StateSet CLI
 *
 * Provides centralized database management, connection pooling,
 * and multi-database support.
 */

import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);

/** @type {any} */
let _CommerceCtor = null;

function getCommerceCtor() {
  if (_CommerceCtor) return _CommerceCtor;
  let mod;
  try {
    mod = require('@stateset/embedded');
  } catch (err) {
    const msg = err && typeof err.message === 'string' ? err.message : String(err);
    throw new Error(`Failed to load @stateset/embedded. ${msg}`);
  }

  const CommerceCtor = mod.Commerce || mod.default?.Commerce || mod.default;
  if (!CommerceCtor) {
    throw new Error('Failed to resolve Commerce export from @stateset/embedded.');
  }

  _CommerceCtor = CommerceCtor;
  return CommerceCtor;
}

/**
 * DatabaseManager - Centralized database management
 */
export class DatabaseManager {
  constructor(options = {}) {
    this.defaultPath = options.defaultPath || './store.db';
    this.maxConnections = options.maxConnections || 10;
    this.connectionTimeout = options.connectionTimeout || 30000;
    this.connections = new Map();
    this.activeConnection = null;
  }

  /**
   * Get or create a connection
   */
  getConnection(dbPath = this.defaultPath) {
    const resolvedPath = this.resolvePath(dbPath);

    // Return cached connection if available
    if (this.connections.has(resolvedPath)) {
      const conn = this.connections.get(resolvedPath);
      conn.lastUsed = Date.now();
      return conn.commerce;
    }

    // Check connection limit
    if (this.connections.size >= this.maxConnections) {
      this.evictOldest();
    }

    // Create new connection
    const Commerce = getCommerceCtor();
    const commerce = new Commerce(resolvedPath);

    this.connections.set(resolvedPath, {
      commerce,
      path: resolvedPath,
      createdAt: Date.now(),
      lastUsed: Date.now(),
    });

    this.activeConnection = resolvedPath;
    return commerce;
  }

  /**
   * Resolve database path
   */
  resolvePath(dbPath) {
    if (dbPath === ':memory:') {
      return dbPath;
    }

    // Expand ~ to home directory
    if (dbPath.startsWith('~')) {
      dbPath = path.join(os.homedir(), dbPath.slice(1));
    }

    return path.resolve(dbPath);
  }

  /**
   * Check if database exists
   */
  exists(dbPath) {
    if (dbPath === ':memory:') return false;
    const resolvedPath = this.resolvePath(dbPath);
    return fs.existsSync(resolvedPath);
  }

  /**
   * Get database info
   */
  getInfo(dbPath = this.activeConnection || this.defaultPath) {
    const resolvedPath = this.resolvePath(dbPath);

    const info = {
      path: resolvedPath,
      exists: this.exists(dbPath),
      isMemory: dbPath === ':memory:',
      isActive: this.activeConnection === resolvedPath,
    };

    if (info.exists && !info.isMemory) {
      try {
        const stats = fs.statSync(resolvedPath);
        info.size = stats.size;
        info.sizeFormatted = this.formatSize(stats.size);
        info.created = stats.birthtime;
        info.modified = stats.mtime;
      } catch {
        // Ignore stat errors
      }
    }

    // Get record counts if connected
    const conn = this.connections.get(resolvedPath);
    if (conn) {
      try {
        info.counts = {
          customers: conn.commerce.customers.count(),
          orders: conn.commerce.orders.count(),
          products: conn.commerce.products.count(),
          returns: conn.commerce.returns.count(),
        };
      } catch {
        // Ignore count errors
      }
    }

    return info;
  }

  /**
   * Format file size
   */
  formatSize(bytes) {
    const units = ['B', 'KB', 'MB', 'GB'];
    let size = bytes;
    let unitIndex = 0;

    while (size >= 1024 && unitIndex < units.length - 1) {
      size /= 1024;
      unitIndex++;
    }

    return `${size.toFixed(2)} ${units[unitIndex]}`;
  }

  /**
   * Switch active database
   */
  use(dbPath) {
    const resolvedPath = this.resolvePath(dbPath);
    this.getConnection(dbPath); // Ensure connection exists
    this.activeConnection = resolvedPath;
    return this.getInfo(dbPath);
  }

  /**
   * Get current active connection
   */
  current() {
    if (!this.activeConnection) {
      return this.getConnection();
    }
    return this.connections.get(this.activeConnection)?.commerce;
  }

  /**
   * List all connections
   */
  listConnections() {
    return Array.from(this.connections.entries()).map(([path, conn]) => ({
      path,
      active: path === this.activeConnection,
      createdAt: conn.createdAt,
      lastUsed: conn.lastUsed,
    }));
  }

  /**
   * Close a connection
   */
  close(dbPath) {
    const resolvedPath = this.resolvePath(dbPath);
    if (this.connections.has(resolvedPath)) {
      this.connections.delete(resolvedPath);
      if (this.activeConnection === resolvedPath) {
        this.activeConnection = null;
      }
      return true;
    }
    return false;
  }

  /**
   * Close all connections
   */
  closeAll() {
    this.connections.clear();
    this.activeConnection = null;
  }

  /**
   * Evict oldest connection
   */
  evictOldest() {
    let oldest = null;
    let oldestTime = Infinity;

    for (const [path, conn] of this.connections) {
      // Don't evict active connection
      if (path === this.activeConnection) continue;

      if (conn.lastUsed < oldestTime) {
        oldest = path;
        oldestTime = conn.lastUsed;
      }
    }

    if (oldest) {
      this.connections.delete(oldest);
    }
  }

  /**
   * Backup database
   */
  backup(dbPath = this.activeConnection, backupDir = null) {
    const resolvedPath = this.resolvePath(dbPath);

    if (!this.exists(dbPath)) {
      throw new Error(`Database does not exist: ${resolvedPath}`);
    }

    const targetDir = backupDir || path.join(os.homedir(), '.stateset', 'backups');
    if (!fs.existsSync(targetDir)) {
      fs.mkdirSync(targetDir, { recursive: true });
    }

    const timestamp = new Date().toISOString().replace(/[:.]/g, '-');
    const basename = path.basename(resolvedPath, '.db');
    const backupPath = path.join(targetDir, `${basename}-${timestamp}.db`);

    fs.copyFileSync(resolvedPath, backupPath);

    return {
      source: resolvedPath,
      backup: backupPath,
      size: fs.statSync(backupPath).size,
    };
  }

  /**
   * Restore from backup
   */
  restore(backupPath, targetPath = null) {
    if (!fs.existsSync(backupPath)) {
      throw new Error(`Backup does not exist: ${backupPath}`);
    }

    const target = targetPath || this.defaultPath;
    const resolvedTarget = this.resolvePath(target);

    // Close any existing connection
    this.close(target);

    // Copy backup to target
    fs.copyFileSync(backupPath, resolvedTarget);

    return {
      backup: backupPath,
      restored: resolvedTarget,
      size: fs.statSync(resolvedTarget).size,
    };
  }

  /**
   * List available backups
   */
  listBackups(backupDir = null) {
    const targetDir = backupDir || path.join(os.homedir(), '.stateset', 'backups');

    if (!fs.existsSync(targetDir)) {
      return [];
    }

    return fs
      .readdirSync(targetDir)
      .filter((f) => f.endsWith('.db'))
      .map((f) => {
        const fullPath = path.join(targetDir, f);
        const stats = fs.statSync(fullPath);
        return {
          name: f,
          path: fullPath,
          size: stats.size,
          sizeFormatted: this.formatSize(stats.size),
          created: stats.birthtime,
        };
      })
      .sort((a, b) => b.created - a.created);
  }

  /**
   * Validate database schema
   */
  async validate(dbPath = this.activeConnection) {
    const commerce = this.getConnection(dbPath);
    const issues = [];

    // Check core tables exist by attempting operations
    const checks = [
      { name: 'customers', fn: () => commerce.customers.count() },
      { name: 'orders', fn: () => commerce.orders.count() },
      { name: 'products', fn: () => commerce.products.count() },
      { name: 'inventory', fn: () => commerce.inventory.getStock('__test__') },
      { name: 'returns', fn: () => commerce.returns.count() },
    ];

    for (const check of checks) {
      try {
        await check.fn();
      } catch (error) {
        issues.push({
          table: check.name,
          error: error.message,
        });
      }
    }

    return {
      valid: issues.length === 0,
      issues,
    };
  }

  /**
   * Get database statistics
   */
  async getStats(dbPath = this.activeConnection) {
    const commerce = this.getConnection(dbPath);
    const info = this.getInfo(dbPath);

    return {
      ...info,
      statistics: {
        customers: {
          total: commerce.customers.count(),
        },
        orders: {
          total: commerce.orders.count(),
        },
        products: {
          total: commerce.products.count(),
        },
        returns: {
          total: commerce.returns.count(),
        },
      },
    };
  }
}

/**
 * Create a database manager
 */
export function createDatabaseManager(options = {}) {
  return new DatabaseManager(options);
}

/**
 * Global database manager singleton
 */
let globalManager = null;

/**
 * Get the global database manager
 */
export function getGlobalManager() {
  if (!globalManager) {
    globalManager = createDatabaseManager();
  }
  return globalManager;
}

/**
 * Quick helper to get a commerce instance
 */
export function getCommerce(dbPath) {
  return getGlobalManager().getConnection(dbPath);
}

export default {
  DatabaseManager,
  createDatabaseManager,
  getGlobalManager,
  getCommerce,
};
