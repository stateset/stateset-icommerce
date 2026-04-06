/**
 * Structured Logger
 *
 * Outputs structured JSON logs with automatic request context
 * (requestId, orgId) from AsyncLocalStorage.
 */

import { getRequestContext } from './request-context';

type LogLevel = 'debug' | 'info' | 'warn' | 'error';

interface LogEntry {
  level: LogLevel;
  message: string;
  timestamp: string;
  requestId?: string;
  orgId?: string;
  path?: string;
  method?: string;
  durationMs?: number;
  [key: string]: unknown;
}

function createLogEntry(level: LogLevel, message: string, meta?: Record<string, unknown>): LogEntry {
  const ctx = getRequestContext();
  const entry: LogEntry = {
    level,
    message,
    timestamp: new Date().toISOString(),
  };

  if (ctx) {
    entry.requestId = ctx.requestId;
    if (ctx.orgId) entry.orgId = ctx.orgId;
    if (ctx.path) entry.path = ctx.path;
    if (ctx.method) entry.method = ctx.method;
  }

  if (meta) {
    Object.assign(entry, meta);
  }

  return entry;
}

function emit(entry: LogEntry): void {
  const output = JSON.stringify(entry);
  switch (entry.level) {
    case 'error':
      console.error(output);
      break;
    case 'warn':
      console.warn(output);
      break;
    case 'debug':
      if (process.env.NODE_ENV !== 'production') {
        console.debug(output);
      }
      break;
    default:
      console.log(output);
  }
}

export const logger = {
  debug(message: string, meta?: Record<string, unknown>): void {
    emit(createLogEntry('debug', message, meta));
  },

  info(message: string, meta?: Record<string, unknown>): void {
    emit(createLogEntry('info', message, meta));
  },

  warn(message: string, meta?: Record<string, unknown>): void {
    emit(createLogEntry('warn', message, meta));
  },

  error(message: string, meta?: Record<string, unknown>): void {
    emit(createLogEntry('error', message, meta));
  },
};
