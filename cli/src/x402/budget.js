import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';

/**
 * @typedef {{ amount: number, timestamp: string, [key: string]: unknown }} BudgetHistoryEntry
 * @typedef {{ version: number, daily: Record<string, number>, history: BudgetHistoryEntry[], balance: number | null }} BudgetStateData
 * @typedef {{ filePath?: string, startingBalance?: number | null }} BudgetStateOptions
 * @typedef {{
 *   filePath: string,
 *   state: BudgetStateData,
 *   getSpentToday: () => number,
 *   getBalance: () => number | null,
 *   recordSpend: (amount: number, metadata?: Record<string, unknown>) => void,
 *   listHistory: (limit?: number) => BudgetHistoryEntry[],
 *   save: () => void,
 * }} BudgetState
 */

/** @type {BudgetStateData} */
const DEFAULT_STATE = {
  version: 1,
  daily: {},
  history: [],
  balance: null,
};

/**
 * @param {string} filePath
 */
function ensureDir(filePath) {
  const dir = path.dirname(filePath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
}

/**
 * @param {string} filePath
 * @returns {BudgetStateData | null}
 */
function readJson(filePath) {
  try {
    const raw = fs.readFileSync(filePath, 'utf8');
    return /** @type {BudgetStateData} */ (JSON.parse(raw));
  } catch (error) {
    const fsError = /** @type {NodeJS.ErrnoException} */ (error);
    if (fsError?.code === 'ENOENT') return null;
    throw error;
  }
}

/**
 * @param {string} filePath
 * @param {BudgetStateData} data
 */
function writeJson(filePath, data) {
  ensureDir(filePath);
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2));
}

/**
 * @returns {string}
 */
function todayKey() {
  return new Date().toISOString().slice(0, 10);
}

/**
 * @returns {string}
 */
export function getDefaultBudgetStateFile() {
  return path.join(os.homedir(), '.stateset', 'x402', 'budget.json');
}

/**
 * @param {BudgetStateOptions} [options]
 * @returns {BudgetState}
 */
export function createBudgetState({
  filePath = getDefaultBudgetStateFile(),
  startingBalance,
} = {}) {
  /** @type {Partial<BudgetStateData>} */
  const persisted = readJson(filePath) || {};
  /** @type {BudgetStateData} */
  const state = {
    ...DEFAULT_STATE,
    daily: { ...DEFAULT_STATE.daily },
    history: [...DEFAULT_STATE.history],
    ...persisted,
  };

  if (startingBalance !== undefined && startingBalance !== null && state.balance === null) {
    state.balance = Number(startingBalance);
  }

  const save = () => writeJson(filePath, state);

  const getSpentToday = () => {
    const key = todayKey();
    return Number(state.daily[key] || 0);
  };

  const getBalance = () => (state.balance === null ? null : Number(state.balance));

  /**
   * @param {number} amount
   * @param {Record<string, unknown>} [metadata]
   */
  const recordSpend = (amount, metadata = {}) => {
    const key = todayKey();
    const numericAmount = Number(amount);
    state.daily[key] = Number(state.daily[key] || 0) + numericAmount;
    state.history.push(
      /** @type {BudgetHistoryEntry} */ ({
        amount: numericAmount,
        timestamp: new Date().toISOString(),
        ...metadata,
      }),
    );
    if (state.balance !== null) {
      state.balance = Number(state.balance) - numericAmount;
    }
    if (state.history.length > 1000) {
      state.history = state.history.slice(-1000);
    }
    save();
  };

  /**
   * @param {number} [limit]
   * @returns {BudgetHistoryEntry[]}
   */
  const listHistory = (limit = 50) => {
    return state.history.slice(-limit).reverse();
  };

  return {
    filePath,
    state,
    getSpentToday,
    getBalance,
    recordSpend,
    listHistory,
    save,
  };
}
