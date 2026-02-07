import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';

const DEFAULT_STATE = {
  version: 1,
  daily: {},
  history: [],
  balance: null,
};

function ensureDir(filePath) {
  const dir = path.dirname(filePath);
  if (!fs.existsSync(dir)) {
    fs.mkdirSync(dir, { recursive: true });
  }
}

function readJson(filePath) {
  try {
    const raw = fs.readFileSync(filePath, 'utf8');
    return JSON.parse(raw);
  } catch (error) {
    if (error?.code === 'ENOENT') return null;
    throw error;
  }
}

function writeJson(filePath, data) {
  ensureDir(filePath);
  fs.writeFileSync(filePath, JSON.stringify(data, null, 2));
}

function todayKey() {
  return new Date().toISOString().slice(0, 10);
}

export function getDefaultBudgetStateFile() {
  return path.join(os.homedir(), '.stateset', 'x402', 'budget.json');
}

export function createBudgetState({
  filePath = getDefaultBudgetStateFile(),
  startingBalance,
} = {}) {
  const persisted = readJson(filePath) || {};
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

  const recordSpend = (amount, metadata = {}) => {
    const key = todayKey();
    const numericAmount = Number(amount);
    state.daily[key] = Number(state.daily[key] || 0) + numericAmount;
    state.history.push({
      amount: numericAmount,
      timestamp: new Date().toISOString(),
      ...metadata,
    });
    if (state.balance !== null) {
      state.balance = Number(state.balance) - numericAmount;
    }
    if (state.history.length > 1000) {
      state.history = state.history.slice(-1000);
    }
    save();
  };

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
