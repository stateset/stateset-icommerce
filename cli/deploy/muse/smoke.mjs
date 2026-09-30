import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { parseArgs } from 'node:util';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

export async function runMuseSmoke({ baseUrl, apiKey, fetchImpl = fetch }) {
  const base = new URL(baseUrl);
  const loopback = ['localhost', '127.0.0.1', '[::1]'].includes(base.hostname);
  assert.ok(
    base.protocol === 'https:' || (base.protocol === 'http:' && loopback),
    'Use HTTPS or loopback HTTP',
  );
  assert.ok(
    !base.username && !base.password && !base.search && !base.hash && base.pathname === '/',
    'Use an origin without credentials, query, or path',
  );
  assert.ok(typeof apiKey === 'string' && apiKey.length >= 16, 'Provide a valid API key');
  const request = async (path, options = {}) =>
    fetchImpl(new URL(path, base), {
      redirect: 'error',
      signal: AbortSignal.timeout(15000),
      ...options,
    });
  const health = await request('/health');
  assert.equal(health.status, 200, 'Health probe failed');
  assert.equal((await health.json()).status, 'ok');
  const headers = { Authorization: `Bearer ${apiKey}` };
  const schemaResponse = await request('/openapi.json', { headers });
  assert.equal(schemaResponse.status, 200, 'OpenAPI discovery failed');
  const schema = await schemaResponse.json();
  assert.equal(schema.openapi, '3.1.0');
  assert.ok(schema.components?.securitySchemes?.bearerAuth, 'Bearer scheme missing');
  assert.equal((await request('/v1/tools')).status, 401, 'Catalog must require authentication');
  const catalogResponse = await request('/v1/tools', { headers });
  assert.equal(catalogResponse.status, 200, 'Authenticated catalog failed');
  const { tools } = await catalogResponse.json();
  assert.ok(Array.isArray(tools) && tools.length > 0, 'Empty tool catalog');
  const names = tools.map((tool) => tool.name).sort();
  assert.deepEqual(
    Object.keys(schema.paths).sort(),
    names.map((name) => `/v1/tools/${name}`).sort(),
    'OpenAPI and runtime allowlists differ',
  );
  const post = async (name, params) => {
    const response = await request(`/v1/tools/${name}`, {
      method: 'POST',
      headers: { ...headers, 'Content-Type': 'application/json' },
      body: JSON.stringify(params),
    });
    assert.equal(response.status, 200, `Tool ${name} failed`);
    const result = await response.json();
    assert.notEqual(result.success, false, `Tool ${name} returned an execution error`);
    return result;
  };
  // Only call explicitly read-only tools that work without resource IDs.
  const reads = tools.filter(
    (tool) => tool.permission === 'read' && !tool.inputSchema?.required?.length,
  );
  assert.ok(
    reads.length > 0,
    'Allowlist needs a read tool with no required arguments for this smoke check',
  );
  await post(reads[0].name, {});
  let previewChecked = false;
  const createProduct = tools.find(
    (tool) => tool.name === 'create_product' && tool.previewOnly === true,
  );
  if (createProduct) {
    const result = await post('create_product', { name: 'Muse connector smoke preview' });
    assert.equal(result.preview, true, 'Write must return a preview');
    previewChecked = true;
  }
  return {
    passed: true,
    origin: base.origin,
    tools: names,
    readChecked: reads[0].name,
    previewChecked,
    appliedWrites: 0,
  };
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  try {
    const { values } = parseArgs({
      options: {
        'base-url': { type: 'string', default: 'http://127.0.0.1:8092' },
        'api-key-file': { type: 'string' },
        help: { type: 'boolean', short: 'h' },
      },
    });
    if (values.help) {
      console.log(
        'Usage: node smoke.mjs --base-url https://commerce.example.com --api-key-file ./muse-keys.txt\nChecks discovery, authentication, a read, and a write preview when available. Never applies writes.',
      );
    } else {
      const apiKey = values['api-key-file']
        ? readFileSync(values['api-key-file'], 'utf8')
            .split(/\r?\n/)
            .map((line) => line.trim())
            .find((line) => line && !line.startsWith('#'))
        : process.env.STATESET_MUSE_API_KEYS?.split(',')[0]?.trim();
      console.log(
        JSON.stringify(await runMuseSmoke({ baseUrl: values['base-url'], apiKey }), null, 2),
      );
    }
  } catch (error) {
    console.error(`Muse connector smoke check failed: ${error.message}`);
    process.exitCode = 1;
  }
}
