/**
 * Agentic replay log.
 *
 * In-memory ring buffer plus a persistent JSONL append log. Records every
 * agentic tool call with a signed event signature, fans events out to the
 * MCP event stream, and supports filtered listing that merges on-disk
 * records with the in-memory ring (deduped by eventId).
 *
 * Pulled out of mcp-server.js so the orchestrator stays focused on
 * orchestration. Callers inject `signAuditArtifact` and the optional
 * `mcpEventStream` so this module stays pure + unit-testable.
 */

import fs from 'node:fs/promises';
import path from 'node:path';

import { replayEventHash } from './audit-envelope.js';
import { normalizeToolName } from './policy-helpers.js';

export const DEFAULT_REPLAY_LOG_FILE = 'agentic-tool-calls.jsonl';
export const DEFAULT_REPLAY_BUFFER_SIZE = 400;

/**
 * Build a publish helper for the (optional) MCP event stream. Exported so
 * mcp-server.js can use the same shape if it ever needs to publish events
 * outside the replay-log path.
 *
 * @param {Object|null} mcpEventStream - object with a `.publish(event)` method
 * @returns {(event: Object) => void}
 */
export function createEventStreamPublisher(mcpEventStream) {
  return (event) => {
    if (!mcpEventStream?.publish || typeof mcpEventStream.publish !== 'function') {
      return;
    }
    try {
      mcpEventStream.publish({
        status: event?.status || 'event',
        tool: event?.tool || null,
        requestId: event?.requestId || null,
        sessionId: event?.sessionId || null,
        timestamp: event?.occurredAt || event?.timestamp || new Date().toISOString(),
        result: event?.result || null,
        error: event?.error || null,
        policy: event?.policy || null,
        permission: event?.permission || null,
        charge: event?.charge || null,
        params: event?.params || null,
        notes: event?.notes || null,
        source: event?.source || 'mcp_server',
      });
    } catch (error) {
      console.warn('[MCP Server] Failed to publish event stream event:', error.message);
    }
  };
}

/**
 * @typedef {Object} ReplayLogOptions
 * @property {string} logPath - Absolute path to the JSONL file.
 * @property {number} [bufferSize] - Max events held in memory (default 400).
 * @property {Object|null} [telemetry] - Optional telemetry sink; calls
 *   `.logCustomEvent('agentic_replay_read_error', { error, path })` on
 *   non-ENOENT read failures.
 * @property {(payload: Object) => { signature: string }} signAuditArtifact
 *   Signer for the event-signature envelope.
 * @property {Object|null} [mcpEventStream] - Optional event stream with
 *   `.publish(event)`; events are fanned out after persistence.
 */

/**
 * @param {ReplayLogOptions} options
 */
export function createReplayLog({
  logPath,
  bufferSize = DEFAULT_REPLAY_BUFFER_SIZE,
  telemetry = null,
  signAuditArtifact,
  mcpEventStream = null,
}) {
  if (typeof logPath !== 'string' || !logPath) {
    throw new TypeError('createReplayLog requires `logPath` (absolute path)');
  }
  if (typeof signAuditArtifact !== 'function') {
    throw new TypeError('createReplayLog requires `signAuditArtifact(payload)`');
  }

  const ringBuffer = [];
  let pendingAppend = Promise.resolve();
  const publishToEventStream = createEventStreamPublisher(mcpEventStream);

  const persistEvent = async (event) => {
    pendingAppend = pendingAppend
      .catch((err) => {
        console.debug('replay log append failed:', err.message);
      })
      .then(async () => {
        await fs.mkdir(path.dirname(logPath), { recursive: true });
        await fs.appendFile(logPath, `${JSON.stringify(event)}\n`);
      });
    return pendingAppend;
  };

  const addEvent = async (event) => {
    if (!event || typeof event !== 'object') return;
    const paramsHash = event.paramsHash || replayEventHash(event.params || {});
    const resultHash = event.resultHash || replayEventHash(event.result || {});
    const signaturePayload = {
      tool: event.tool || null,
      status: event.status || null,
      requestId: event.requestId || null,
      sessionId: event.sessionId || null,
      occurredAt: event.occurredAt || null,
      policyDomain: event.policyDomain || null,
      paramsHash,
      resultHash,
      source: event.source || null,
    };
    const sanitized = {
      ...event,
      paramsHash,
      resultHash,
      eventSignature: event.eventSignature || signAuditArtifact(signaturePayload).signature,
    };
    ringBuffer.push(sanitized);
    if (ringBuffer.length > bufferSize) {
      ringBuffer.shift();
    }
    publishToEventStream(sanitized);
    await persistEvent(sanitized);
  };

  const listEvents = async (options = {}) => {
    const limit = Math.max(1, Math.min(bufferSize, Number(options.limit) || 20));
    const targetTool = options?.tool ? normalizeToolName(options.tool) : null;
    const targetEventId = options?.eventId || null;
    const requestId = options?.requestId || null;
    const sessionId = options?.sessionId || null;
    const status = options?.status || null;
    const targetPlanSignature = options?.planSignature || null;
    const targetExecutionSignature = options?.executionSignature || null;

    const matches = (event) => {
      if (targetTool && event?.tool !== targetTool) return false;
      if (targetEventId && event?.eventId !== targetEventId) return false;
      if (requestId && event?.requestId !== requestId) return false;
      if (sessionId && event?.sessionId !== sessionId) return false;
      if (status && event?.status !== status) return false;
      if (targetPlanSignature) {
        const eventPlanSignature = event?.planSignature || event?.notes?.planSignature;
        if (!eventPlanSignature || eventPlanSignature !== targetPlanSignature) {
          return false;
        }
      }
      if (targetExecutionSignature) {
        const eventExecutionSignature =
          event?.executionSignature || event?.notes?.executionSignature;
        if (!eventExecutionSignature || eventExecutionSignature !== targetExecutionSignature) {
          return false;
        }
      }
      return true;
    };

    let fileEvents = [];
    try {
      const raw = await fs.readFile(logPath, 'utf8');
      if (raw?.trim()) {
        fileEvents = raw
          .split('\n')
          .filter((line) => line.trim())
          .map((line) => {
            try {
              return JSON.parse(line);
            } catch (error) {
              return { _parseError: error.message, raw: line };
            }
          })
          .filter(matches);
      }
    } catch (error) {
      if (error?.code !== 'ENOENT') {
        if (telemetry) {
          telemetry.logCustomEvent('agentic_replay_read_error', {
            error: error.message,
            path: logPath,
          });
        }
      }
    }

    const merged = [...fileEvents, ...ringBuffer].filter(matches);
    const deduped = [];
    const seen = new Set();
    for (const evt of merged) {
      if (!evt?.eventId) {
        deduped.push(evt);
        continue;
      }
      if (seen.has(evt.eventId)) continue;
      seen.add(evt.eventId);
      deduped.push(evt);
    }

    const order = deduped
      .filter((event) => event.occurredAt)
      .sort((a, b) => (a.occurredAt < b.occurredAt ? 1 : -1));
    const remaining = limit ? order.slice(0, limit) : order;

    return {
      generatedAt: new Date().toISOString(),
      count: remaining.length,
      events: remaining,
      filters: {
        limit,
        tool: targetTool || null,
        eventId: targetEventId,
        requestId,
        sessionId,
        planSignature: targetPlanSignature,
        executionSignature: targetExecutionSignature,
        status,
      },
      source: {
        path: logPath,
        inMemoryBuffer: ringBuffer.length,
      },
    };
  };

  return {
    getLogPath: () => logPath,
    addEvent,
    listEvents,
    persistEvent,
    publishToEventStream,
  };
}
