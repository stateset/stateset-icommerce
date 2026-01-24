#!/usr/bin/env node
/**
 * StateSet Sandbox runner for the iCommerce -> Sequencer -> Arc L1 demo.
 *
 * Usage:
 *   SANDBOX_API_KEY=sk_test_... node cli/examples/icommerce_sequencer_arc_sandbox.mjs
 *
 * Optional env:
 *   SANDBOX_URL=https://api.sandbox.stateset.app
 *   SANDBOX_TIMEOUT_SECONDS=300
 *   SEQUENCER_URL, ARC_CHAIN_ID, ARC_EXPLORER_URL, TENANT_ID, STORE_ID
 */

import fs from 'fs';
import path from 'path';
import { request as httpRequest } from 'http';
import { request as httpsRequest } from 'https';
import { fileURLToPath } from 'url';
import { URL } from 'url';

const SANDBOX_URL = process.env.SANDBOX_URL || 'https://api.sandbox.stateset.app';
const SANDBOX_API_KEY = process.env.SANDBOX_API_KEY;
const SANDBOX_TIMEOUT_SECONDS = parseInt(process.env.SANDBOX_TIMEOUT_SECONDS || '300', 10);
const SANDBOX_READY_TIMEOUT_MS = parseInt(process.env.SANDBOX_READY_TIMEOUT_MS || '60000', 10);
const SANDBOX_RETRY_TIMEOUT_MS = parseInt(process.env.SANDBOX_RETRY_TIMEOUT_MS || '30000', 10);

// Colors
const CYAN = '\x1b[36m';
const GREEN = '\x1b[32m';
const YELLOW = '\x1b[33m';
const RED = '\x1b[31m';
const NC = '\x1b[0m';

function printHeader(text) {
    console.log(`\n${CYAN}=== ${text} ===${NC}\n`);
}

function printStep(num, text) {
    console.log(`\n${YELLOW}Step ${num}: ${text}${NC}\n`);
}

function sleep(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
}

function isSandboxNotFoundError(err) {
    return err && typeof err.message === 'string' && err.message.includes('Sandbox not found');
}

async function retryOnNotFound(action, fn, timeoutMs = SANDBOX_RETRY_TIMEOUT_MS) {
    const startedAt = Date.now();
    let lastError;

    while (Date.now() - startedAt < timeoutMs) {
        try {
            return await fn();
        } catch (err) {
            lastError = err;
            if (!isSandboxNotFoundError(err)) {
                throw err;
            }
        }

        await sleep(1500);
    }

    if (lastError) {
        throw lastError;
    }

    throw new Error(`${action} timed out`);
}

async function sandboxRequest(method, pathName, body) {
    const url = `${SANDBOX_URL.replace(/\/$/, '')}/api/v1${pathName}`;
    const payload = body ? JSON.stringify(body) : '';
    const headers = {
        'Content-Type': 'application/json',
        'Authorization': `ApiKey ${SANDBOX_API_KEY}`
    };
    if (payload) {
        headers['Content-Length'] = Buffer.byteLength(payload).toString();
    }

    const response = await requestText(url, {
        method,
        headers,
        body: payload || undefined
    });

    const text = response.body;
    let data = {};
    if (text) {
        try {
            data = JSON.parse(text);
        } catch (err) {
            throw new Error(`Sandbox API returned non-JSON response (${response.statusCode})`);
        }
    }

    if (response.statusCode < 200 || response.statusCode >= 300) {
        const message = data?.error?.message || data?.error || `HTTP ${response.statusCode}`;
        throw new Error(`Sandbox API error: ${message}`);
    }

    return data;
}

async function waitForSandboxReady(sandboxId) {
    const startedAt = Date.now();
    let lastError;

    while (Date.now() - startedAt < SANDBOX_READY_TIMEOUT_MS) {
        try {
            const status = await sandboxRequest('GET', `/sandbox/${sandboxId}/status`);
            if (status.status === 'running') {
                await retryOnNotFound(
                    'Sandbox execute readiness',
                    () => sandboxRequest('POST', `/sandbox/${sandboxId}/execute`, { command: 'echo sandbox-ready' })
                );
                return status;
            }
        } catch (err) {
            lastError = err;
        }

        await sleep(1500);
    }

    if (lastError) {
        throw lastError;
    }

    throw new Error('Timed out waiting for sandbox to be ready');
}

async function writeFileViaExecute(sandboxId, filePath, content) {
    const encoded = Buffer.from(content, 'utf8').toString('base64');
    const command = `printf %s '${encoded}' | base64 -d > ${filePath}`;
    const result = await retryOnNotFound(
        'Sandbox file write',
        () => sandboxRequest('POST', `/sandbox/${sandboxId}/execute`, { command })
    );

    if (result.exit_code !== 0) {
        const stderr = result.stderr ? result.stderr.trim() : '';
        throw new Error(`Sandbox write failed (exit ${result.exit_code}): ${stderr || 'no stderr'}`);
    }
}

function requestText(url, options) {
    return new Promise((resolve, reject) => {
        const parsed = new URL(url);
        const isHttps = parsed.protocol === 'https:';
        const request = isHttps ? httpsRequest : httpRequest;

        const req = request({
            method: options.method,
            hostname: parsed.hostname,
            port: parsed.port || (isHttps ? 443 : 80),
            path: `${parsed.pathname}${parsed.search}`,
            headers: options.headers
        }, (res) => {
            let body = '';
            res.setEncoding('utf8');
            res.on('data', (chunk) => {
                body += chunk;
            });
            res.on('end', () => {
                resolve({
                    statusCode: res.statusCode || 0,
                    headers: res.headers,
                    body
                });
            });
        });

        req.on('error', reject);

        if (options.body) {
            req.write(options.body);
        }
        req.end();
    });
}

function pickEnv(keys) {
    const env = {};
    for (const key of keys) {
        if (process.env[key]) {
            env[key] = String(process.env[key]);
        }
    }
    return env;
}

async function main() {
    printHeader('Sandbox Runner - iCommerce Sequencer Arc Demo');

    if (!SANDBOX_API_KEY) {
        console.error(`${RED}Error:${NC} SANDBOX_API_KEY is required`);
        process.exit(1);
    }

    const __filename = fileURLToPath(import.meta.url);
    const __dirname = path.dirname(__filename);
    const demoPath = path.join(__dirname, 'icommerce_sequencer_arc_e2e.mjs');
    const demoSource = fs.readFileSync(demoPath, 'utf8');

    const passThroughEnv = pickEnv([
        'SEQUENCER_URL',
        'ARC_CHAIN_ID',
        'ARC_EXPLORER_URL',
        'TENANT_ID',
        'STORE_ID'
    ]);

    let sandboxId;
    try {
        printStep(1, 'Create sandbox');
        const sandbox = await sandboxRequest('POST', '/sandbox/create', {
            cpus: '2',
            memory: '2Gi',
            timeout_seconds: SANDBOX_TIMEOUT_SECONDS,
            env: {
                DEMO_RESULTS_DIR: '/workspace',
                ...passThroughEnv
            }
        });

        sandboxId = sandbox.sandbox_id;
        console.log(`${GREEN}[OK]${NC} Sandbox ID: ${sandboxId}`);

        printStep(2, 'Wait for sandbox to be ready');
        await waitForSandboxReady(sandboxId);
        console.log(`${GREEN}[OK]${NC} Sandbox is running`);

        printStep(3, 'Upload demo script');
        const demoTarget = '/workspace/icommerce_sequencer_arc_e2e.mjs';
        try {
            await retryOnNotFound('Sandbox file upload', () =>
                sandboxRequest('POST', `/sandbox/${sandboxId}/files`, {
                    files: [
                        {
                            path: demoTarget,
                            content: Buffer.from(demoSource).toString('base64')
                        }
                    ]
                })
            );
            console.log(`${GREEN}[OK]${NC} Uploaded to ${demoTarget}`);
        } catch (err) {
            console.error(`${YELLOW}Warning:${NC} /files upload failed (${err.message}). Falling back to /execute.`);
            await writeFileViaExecute(sandboxId, demoTarget, demoSource);
            console.log(`${GREEN}[OK]${NC} Uploaded via /execute to ${demoTarget}`);
        }

        printStep(4, 'Execute demo inside sandbox');
        const result = await retryOnNotFound(
            'Sandbox demo execution',
            () => sandboxRequest('POST', `/sandbox/${sandboxId}/execute`, {
                command: 'node /workspace/icommerce_sequencer_arc_e2e.mjs'
            })
        );

        if (result.stdout) {
            console.log(result.stdout.trimEnd());
        }

        if (result.stderr) {
            console.error(`${RED}stderr:${NC}`);
            console.error(result.stderr.trimEnd());
        }

        console.log(`${GREEN}[OK]${NC} Exit code: ${result.exit_code}`);
    } finally {
        if (sandboxId) {
            try {
                printStep(5, 'Stop sandbox');
                await sandboxRequest('POST', `/sandbox/${sandboxId}/stop`);
                console.log(`${GREEN}[OK]${NC} Sandbox stopped`);
            } catch (err) {
                console.error(`${RED}Warning:${NC} Failed to stop sandbox (${err.message})`);
            }
        }
    }
}

main().catch((err) => {
    console.error(`${RED}Error:${NC} ${err.message}`);
    process.exit(1);
});
