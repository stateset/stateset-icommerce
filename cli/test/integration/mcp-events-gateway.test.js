import { describe, it, afterEach, beforeEach } from 'node:test';
import assert from 'node:assert/strict';
import { spawn } from 'node:child_process';
import http from 'node:http';
import path from 'node:path';
import os from 'node:os';
import fs from 'node:fs';
import { fileURLToPath } from 'node:url';
import { once } from 'node:events';

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);
const BIN_DIR = path.join(__dirname, '..', '..', 'bin');

async function waitForExit(processHandle, signal = 'SIGINT') {
  if (processHandle.exitCode !== null || processHandle.killed) {
    return;
  }

  processHandle.kill(signal);
  const closed = once(processHandle, 'close');
  const timeout = new Promise((resolve) => {
    setTimeout(resolve, 500);
  });

  await Promise.race([closed, timeout]);

  if (processHandle.exitCode === null && !processHandle.killed) {
    processHandle.kill('SIGKILL');
    await once(processHandle, 'close');
  }
}

const GATEWAY_START_TIMEOUT_MS = 30_000;
const MCP_REQUEST_TIMEOUT_MS = 15_000;

function startGateway(dbPathOrOptions, port = '0') {
  return new Promise((resolve, reject) => {
    const options =
      typeof dbPathOrOptions === 'object' && dbPathOrOptions !== null
        ? dbPathOrOptions
        : { dbPath: dbPathOrOptions, port };
    const dbPath = options.dbPath;
    const resolvedPort = options.port ?? port;
    const useDbEnv = Boolean(options.useDbEnv);
    const extraEnv = options.env || {};
    const args = [path.join(BIN_DIR, 'stateset-mcp-events.js')];
    if (!useDbEnv) {
      args.push('--db', dbPath);
    }
    args.push('--host', '127.0.0.1', '--port', String(resolvedPort));

    const proc = spawn(process.execPath, args, {
      stdio: ['pipe', 'pipe', 'pipe'],
      env: {
        ...process.env,
        ...(useDbEnv ? { DB_PATH: dbPath } : {}),
        ...extraEnv,
      },
    });

    let stdout = '';
    let stderr = '';
    let resolved = false;

    const cleanup = () => {
      clearTimeout(startTimeout);
      proc.stdout.off('data', onStdout);
      proc.stderr.off('data', onStderr);
      proc.off('error', onError);
      proc.off('close', onClose);
    };

    const fail = (error) => {
      if (resolved) return;
      resolved = true;
      cleanup();
      // Never leak a half-started gateway: the caller has no handle to it.
      if (proc.exitCode === null && !proc.killed) {
        proc.kill('SIGKILL');
      }
      reject(error);
    };

    const onStdout = (chunk) => {
      stdout += chunk.toString();
      if (!resolved && stdout.trim()) {
        fail(new Error(`Gateway wrote unexpected stdout before MCP traffic: ${stdout.trim()}`));
      }
    };

    const onStderr = (chunk) => {
      const line = chunk.toString();
      stderr += line;
      const match = stderr.match(/active on http:\/\/127\.0\.0\.1:(\d+)/);
      if (match?.[1]) {
        resolved = true;
        cleanup();
        resolve({
          process: proc,
          port: Number.parseInt(match[1], 10),
          close: () => waitForExit(proc),
        });
        return;
      }
      if (/EADDRINUSE|Address already in use/i.test(line)) {
        fail(new Error(`Port in use while starting gateway: ${line.trim()}`));
      }
    };

    const onError = (error) => {
      fail(error);
    };

    const onClose = (code) => {
      if (!resolved && code !== 0 && code !== null) {
        fail(new Error(`Gateway exited before ready, code=${code}`));
      } else if (!resolved && code === 0) {
        fail(new Error('Gateway exited before ready with status 0'));
      }
    };

    // The gateway builds its full MCP server (hundreds of tool schemas) before
    // it advertises readiness; ~1s on an idle machine, many times that under a
    // loaded parallel test run. Readiness is a boot cost, not a correctness
    // signal, so wait generously.
    const startTimeout = setTimeout(() => {
      fail(
        new Error(
          `Timed out waiting for MCP events gateway to start (${GATEWAY_START_TIMEOUT_MS}ms)`,
        ),
      );
    }, GATEWAY_START_TIMEOUT_MS);

    proc.once('error', onError);
    proc.once('close', onClose);
    proc.stdout.on('data', onStdout);
    proc.stderr.on('data', onStderr);
  });
}

function createMcpClient(processHandle, options = {}) {
  const requestTimeoutMs = options.requestTimeoutMs || MCP_REQUEST_TIMEOUT_MS;
  let nextId = 0;
  const pending = new Map();
  let buffer = '';

  const closePending = (error) => {
    for (const [, pendingRequest] of pending) {
      clearTimeout(pendingRequest.timeout);
      pendingRequest.reject(error);
    }
    pending.clear();
  };

  const onStdout = (chunk) => {
    buffer += chunk.toString();
    const lines = buffer.split('\n');
    buffer = lines.pop();

    for (const line of lines) {
      const trimmed = line.trim();
      if (!trimmed) continue;

      let message;
      try {
        message = JSON.parse(trimmed);
      } catch (error) {
        continue;
      }

      const requestId = `${message.id}`;
      if (message.id === undefined || !pending.has(requestId)) {
        continue;
      }

      const entry = pending.get(requestId);
      clearTimeout(entry.timeout);
      pending.delete(requestId);
      if (message.error) {
        entry.reject(
          new Error(
            `MCP request ${entry.method} failed: ${message.error.message || JSON.stringify(message.error)}`,
          ),
        );
        return;
      }

      entry.resolve(message);
    }
  };

  processHandle.stdout.on('data', onStdout);

  const request = (method, params = undefined) => {
    const requestId = ++nextId;
    const requestIdValue = String(requestId);
    const requestPayload = {
      jsonrpc: '2.0',
      id: requestIdValue,
      method,
    };
    if (params !== undefined) {
      requestPayload.params = params;
    }

    return new Promise((resolve, reject) => {
      const timeout = setTimeout(() => {
        pending.delete(requestIdValue);
        reject(new Error(`Timed out waiting for MCP response to ${method} (id=${requestId})`));
      }, requestTimeoutMs);

      pending.set(requestIdValue, {
        method,
        resolve,
        reject,
        timeout,
      });

      processHandle.stdin.write(`${JSON.stringify(requestPayload)}\n`);
    });
  };

  const notify = (method, params = undefined) => {
    const notificationPayload = {
      jsonrpc: '2.0',
      method,
    };
    if (params !== undefined) {
      notificationPayload.params = params;
    }
    processHandle.stdin.write(`${JSON.stringify(notificationPayload)}\n`);
  };

  const close = () => {
    processHandle.stdout.off('data', onStdout);
    closePending(new Error('MCP client closed'));
  };

  return {
    request,
    notify,
    close,
  };
}

function openEventsStream(port, query = '', options = {}) {
  return new Promise((resolve, reject) => {
    const req = http.request(
      {
        hostname: '127.0.0.1',
        port,
        path: `/events${query}`,
        method: 'GET',
        headers: {
          Accept: 'text/event-stream',
          ...(options.headers || {}),
        },
      },
      (res) => {
        if (res.statusCode !== 200) {
          reject(new Error(`Failed to open /events stream: HTTP ${res.statusCode}`));
          res.destroy();
          return;
        }
        const events = [];
        let chunkBuffer = '';
        const parseChunk = (chunk) => {
          chunkBuffer += chunk.toString();
          const frames = chunkBuffer.split('\n\n');
          chunkBuffer = frames.pop();
          for (const frame of frames) {
            if (!frame.trim()) continue;
            const lines = frame.split('\n');
            let eventType = 'message';
            let data = '';
            for (const line of lines) {
              if (line.startsWith('event:')) {
                eventType = line.slice(6).trim();
              }
              if (line.startsWith('data:')) {
                const value = line.slice(5).trimStart();
                if (value) {
                  data = data ? `${data}\n${value}` : value;
                }
              }
            }
            let parsedData = null;
            if (data) {
              try {
                parsedData = JSON.parse(data);
              } catch {}
            }

            events.push({
              eventType,
              data,
              payload: parsedData,
            });

            if (eventType === 'connected' && !resolved) {
              resolved = true;
              clearTimeout(startupTimeout);
              resolve(streamState);
            }
          }
        };

        const onData = (chunk) => {
          parseChunk(chunk);
        };

        const onEnd = () => {
          if (!resolved) {
            cleanup();
            reject(new Error('SSE stream ended before connected event was received'));
          }
        };

        const onResponseError = () => {
          if (!resolved) {
            cleanup();
            reject(new Error('SSE events stream error'));
          }
        };

        const streamState = {
          response: res,
          headers: res.headers,
          events,
          close: () => {
            cleanup();
            res.destroy();
          },
        };

        const cleanup = () => {
          clearTimeout(startupTimeout);
          req.off('error', onRequestError);
          res.off('data', onData);
          res.off('error', onResponseError);
          res.off('end', onEnd);
        };

        let resolved = false;
        const startupTimeout = setTimeout(() => {
          if (!resolved) {
            cleanup();
            reject(new Error('SSE events stream did not send connected event'));
            req.destroy();
          }
        }, 4000);

        res.on('data', onData);
        res.on('error', onResponseError);
        res.on('end', onEnd);
      },
    );

    const onRequestError = (error) => {
      req.destroy();
      reject(error);
    };

    req.on('error', onRequestError);
    req.end();
  });
}

function requestJson(port, route, options = {}) {
  return new Promise((resolve, reject) => {
    const req = http.request(
      {
        hostname: '127.0.0.1',
        port,
        path: route,
        method: options.method || 'GET',
        headers: options.headers || {},
      },
      (res) => {
        const chunks = [];
        res.on('data', (chunk) => chunks.push(chunk));
        res.on('end', () => {
          const bodyRaw = Buffer.concat(chunks).toString('utf-8');
          try {
            resolve({
              status: res.statusCode,
              headers: res.headers,
              body: bodyRaw ? JSON.parse(bodyRaw) : null,
            });
          } catch (error) {
            reject(error);
          }
        });
      },
    );
    req.on('error', reject);
    req.end();
  });
}

function waitForStreamEvent(stream, predicate, timeoutMs = 4000) {
  return new Promise((resolve, reject) => {
    const startTime = Date.now();
    const check = () => {
      const match = stream.events.find((entry) => predicate(entry));
      if (match) {
        resolve(match);
        return;
      }

      if (Date.now() - startTime >= timeoutMs) {
        reject(new Error('Timed out waiting for matching SSE event'));
        return;
      }

      setTimeout(check, 50);
    };

    check();
  });
}

function parseToolResponseContent(response) {
  const raw = response?.result?.content?.[0]?.text;
  if (typeof raw !== 'string') {
    return null;
  }

  try {
    return JSON.parse(raw);
  } catch (error) {
    return {
      error: error.message,
      raw,
    };
  }
}

describe('stateset-mcp-events integration', () => {
  let dbPath;
  let gateway;
  let cleanupDir;

  beforeEach(() => {
    cleanupDir = path.join(
      os.tmpdir(),
      `stateset-mcp-events-${Date.now()}-${Math.random().toString(16).slice(2)}`,
    );
    fs.mkdirSync(cleanupDir, { recursive: true });
    dbPath = path.join(cleanupDir, 'store.db');
  });

  afterEach(async () => {
    if (gateway?.close) {
      await gateway.close();
      gateway = null;
    }

    if (cleanupDir && fs.existsSync(cleanupDir)) {
      fs.rmSync(cleanupDir, { recursive: true, force: true });
    }
  });

  it('returns healthy status at /health', async () => {
    const started = await startGateway(dbPath);
    gateway = started;
    const response = await requestJson(started.port, '/health');

    assert.equal(response.status, 200);
    assert.equal(response.body.status, 'ok');
    assert.equal(response.body.stream, 'stateset-mcp');
  });

  it('accepts DB_PATH from the environment when --db is omitted', async () => {
    const started = await startGateway({ dbPath, useDbEnv: true });
    gateway = started;

    const response = await requestJson(started.port, '/ready');
    assert.equal(response.status, 200);
    assert.equal(response.body.status, 'ready');
  });

  it('returns empty history and subscriptions by default', async () => {
    const started = await startGateway(dbPath);
    gateway = started;

    const history = await requestJson(started.port, '/history');
    assert.equal(history.status, 200);
    assert.equal(history.body.count, 0);
    assert.deepEqual(history.body.events, []);

    const subscriptions = await requestJson(started.port, '/subscriptions');
    assert.equal(subscriptions.status, 200);
    assert.equal(subscriptions.body.count, 0);
    assert.deepEqual(subscriptions.body.subscriptions, []);
  });

  it('creates an SSE subscription and exposes it via /subscriptions', async () => {
    const started = await startGateway(dbPath);
    gateway = started;

    const stream = await openEventsStream(started.port, '?session=session-1&types=success,error');
    try {
      const subscriptions = await requestJson(started.port, '/subscriptions?session=session-1');

      assert.equal(subscriptions.status, 200);
      assert.equal(subscriptions.body.count, 1);
      assert.equal(Array.isArray(subscriptions.body.subscriptions), true);
      assert.equal(subscriptions.body.subscriptions[0].sessionId, 'session-1');
      assert.deepEqual(subscriptions.body.subscriptions[0].eventTypes, ['success', 'error']);
    } finally {
      stream.close();
    }
  });

  it('only echoes CORS headers for loopback or explicitly allowed origins', async () => {
    const started = await startGateway(dbPath);
    gateway = started;

    const loopbackResponse = await requestJson(started.port, '/health', {
      headers: { Origin: 'http://localhost:3000' },
    });
    assert.equal(loopbackResponse.headers['access-control-allow-origin'], 'http://localhost:3000');

    const remoteResponse = await requestJson(started.port, '/health', {
      headers: { Origin: 'https://evil.example' },
    });
    assert.equal(remoteResponse.headers['access-control-allow-origin'], undefined);

    const allowedResponse = await requestJson(started.port, '/ready', {
      headers: { Origin: 'https://allowed.example' },
    });
    assert.equal(allowedResponse.headers['access-control-allow-origin'], undefined);
  });

  it('allows explicitly configured non-loopback origins while keeping SSE local by default', async () => {
    const started = await startGateway({
      dbPath,
      env: {
        STATESET_MCP_ALLOWED_ORIGINS: 'https://allowed.example',
      },
    });
    gateway = started;

    const response = await requestJson(started.port, '/health', {
      headers: { Origin: 'https://allowed.example' },
    });
    assert.equal(response.headers['access-control-allow-origin'], 'https://allowed.example');

    const stream = await openEventsStream(started.port, '', {
      headers: { Origin: 'http://localhost:3000' },
    });
    try {
      assert.equal(stream.headers['access-control-allow-origin'], 'http://localhost:3000');
    } finally {
      stream.close();
    }
  });

  it('emits MCP tool execution events to history after calling a tool', async () => {
    const started = await startGateway(dbPath);
    gateway = started;

    const stream = await openEventsStream(started.port, '?types=success,error');
    const mcpClient = createMcpClient(started.process);

    try {
      const initResponse = await mcpClient.request('initialize', {
        protocolVersion: '2024-11-05',
        capabilities: {},
        clientInfo: {
          name: 'stateset-mcp-events-test',
          version: '0.0.1',
        },
      });
      assert.equal(initResponse?.result?.protocolVersion, '2024-11-05');

      mcpClient.notify('notifications/initialized');

      const toolsResponse = await mcpClient.request('tools/list');
      const targetTool = toolsResponse.result?.tools?.find((tool) =>
        `${tool?.name || ''}`.endsWith('agentic_get_event_history'),
      );
      assert.ok(targetTool, 'agentic_get_event_history tool should exist');

      const toolCallResponse = await mcpClient.request('tools/call', {
        name: targetTool.name,
        arguments: {},
      });
      assert.equal(Array.isArray(toolCallResponse.result?.content), true);

      const history = await requestJson(started.port, '/history?types=success');
      assert.equal(history.status, 200);
      assert.equal(history.body.count > 0, true);
      assert.ok(
        history.body.events.some(
          (entry) =>
            entry.type === 'success' &&
            typeof entry.tool === 'string' &&
            entry.tool.endsWith('agentic_get_event_history'),
        ),
      );
      const globalHistory = await requestJson(
        started.port,
        '/history?session=__global__&types=success',
      );
      assert.equal(globalHistory.status, 200);
      assert.equal(globalHistory.body.count > 0, true);

      const scopedHistory = await requestJson(
        started.port,
        '/history?session=some-other-session&types=success',
      );
      assert.equal(scopedHistory.status, 200);
      assert.equal(scopedHistory.body.count, 0);

      const streamEvent = await waitForStreamEvent(
        stream,
        (entry) =>
          entry.eventType === 'success' &&
          typeof entry.payload?.tool === 'string' &&
          entry.payload.tool.endsWith('agentic_get_event_history'),
        8000,
      );
      assert.equal(streamEvent.payload.tool.endsWith('agentic_get_event_history'), true);
    } finally {
      mcpClient.close();
      stream.close();
    }
  });

  it('subscribes and unsubscribes MCP event streams through agentic tools', async () => {
    const started = await startGateway(dbPath);
    gateway = started;

    const stream = await openEventsStream(started.port, '?types=success,error');
    const mcpClient = createMcpClient(started.process);
    const sessionId = 'subscription-tool-session';

    try {
      const initResponse = await mcpClient.request('initialize', {
        protocolVersion: '2024-11-05',
        capabilities: {},
        clientInfo: {
          name: 'stateset-mcp-events-subscription-test',
          version: '0.0.1',
        },
      });
      assert.equal(initResponse?.result?.protocolVersion, '2024-11-05');

      mcpClient.notify('notifications/initialized');

      const toolsResponse = await mcpClient.request('tools/list');
      const subscribeTool = toolsResponse.result?.tools?.find((tool) =>
        `${tool?.name || ''}`.endsWith('agentic_subscribe_events'),
      );
      assert.ok(subscribeTool, 'agentic_subscribe_events tool should exist');

      const unsubscribeTool = toolsResponse.result?.tools?.find((tool) =>
        `${tool?.name || ''}`.endsWith('agentic_unsubscribe_events'),
      );
      assert.ok(unsubscribeTool, 'agentic_unsubscribe_events tool should exist');

      const subscribeResponse = await mcpClient.request('tools/call', {
        name: subscribeTool.name,
        arguments: {
          sessionId,
          eventTypes: ['success'],
        },
      });
      const subscribePayload = parseToolResponseContent(subscribeResponse);
      assert.equal(subscribePayload?.success, true);
      assert.equal(subscribePayload?.subscription?.sessionId, sessionId);
      const subscriptionId = subscribePayload.subscription?.id;
      assert.ok(subscriptionId, 'subscription id should be returned');

      const activeSubscriptions = await requestJson(
        started.port,
        `/subscriptions?session=${sessionId}`,
      );
      assert.equal(activeSubscriptions.status, 200);
      assert.equal(activeSubscriptions.body.count, 1);
      assert.equal(activeSubscriptions.body.subscriptions[0].id, subscriptionId);

      const globalSubscriptions = await requestJson(started.port, '/subscriptions');
      assert.equal(globalSubscriptions.status, 200);
      assert.equal(globalSubscriptions.body.count, 0);

      const unsubscribeResponse = await mcpClient.request('tools/call', {
        name: unsubscribeTool.name,
        arguments: {
          subscriptionId,
        },
      });
      const unsubscribePayload = parseToolResponseContent(unsubscribeResponse);
      assert.equal(unsubscribePayload?.success, true);
      assert.equal(unsubscribePayload?.subscription?.id, subscriptionId);

      const afterUnsubscribe = await requestJson(
        started.port,
        `/subscriptions?session=${sessionId}`,
      );
      assert.equal(afterUnsubscribe.status, 200);
      assert.equal(afterUnsubscribe.body.count, 0);
      assert.equal(Array.isArray(afterUnsubscribe.body.subscriptions), true);
    } finally {
      mcpClient.close();
      stream.close();
    }
  });

  it('lists MCP event subscriptions through MCP tools', async () => {
    const started = await startGateway(dbPath);
    gateway = started;

    const mcpClient = createMcpClient(started.process);
    const sessionId = 'list-events-session';
    let subscriptionId = null;

    try {
      const initResponse = await mcpClient.request('initialize', {
        protocolVersion: '2024-11-05',
        capabilities: {},
        clientInfo: {
          name: 'stateset-mcp-events-list-subscriptions-test',
          version: '0.0.1',
        },
      });
      assert.equal(initResponse?.result?.protocolVersion, '2024-11-05');

      mcpClient.notify('notifications/initialized');

      const toolsResponse = await mcpClient.request('tools/list');
      const subscribeTool = toolsResponse.result?.tools?.find((tool) =>
        `${tool?.name || ''}`.endsWith('agentic_subscribe_events'),
      );
      assert.ok(subscribeTool, 'agentic_subscribe_events tool should exist');

      const listTool = toolsResponse.result?.tools?.find((tool) =>
        `${tool?.name || ''}`.endsWith('agentic_list_event_subscriptions'),
      );
      assert.ok(listTool, 'agentic_list_event_subscriptions tool should exist');

      const subscribeResponse = await mcpClient.request('tools/call', {
        name: subscribeTool.name,
        arguments: {
          sessionId,
          eventTypes: ['success', 'error'],
        },
      });
      const subscribePayload = parseToolResponseContent(subscribeResponse);
      assert.equal(subscribePayload?.success, true);
      subscriptionId = subscribePayload?.subscription?.id;
      assert.ok(subscriptionId);

      const listResponse = await mcpClient.request('tools/call', {
        name: listTool.name,
        arguments: {
          sessionId,
        },
      });
      const listPayload = parseToolResponseContent(listResponse);
      assert.equal(Array.isArray(listPayload?.subscriptions), true);
      assert.equal(listPayload?.subscriptions.length, 1);
      assert.equal(listPayload.subscriptions[0].id, subscriptionId);
      assert.equal(listPayload.subscriptions[0].sessionId, sessionId);
      assert.deepEqual(listPayload.subscriptions[0].eventTypes, ['success', 'error']);
    } finally {
      if (subscriptionId) {
        const toolsResponse = await mcpClient.request('tools/list');
        const unsubscribeTool = toolsResponse.result?.tools?.find((tool) =>
          `${tool?.name || ''}`.endsWith('agentic_unsubscribe_events'),
        );
        if (unsubscribeTool) {
          await mcpClient.request('tools/call', {
            name: unsubscribeTool.name,
            arguments: {
              subscriptionId,
            },
          });
        }
      }

      mcpClient.close();
    }
  });
});
