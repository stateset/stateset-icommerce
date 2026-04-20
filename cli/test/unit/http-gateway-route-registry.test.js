import test from 'node:test';
import assert from 'node:assert/strict';
import { buildHttpRouteDiscoveryDocument } from '../../src/mpp/http.js';
import { ROUTE_PERMISSIONS, getRequiredLevel } from '../../src/channels/http-auth.js';
import { getBuiltinHttpRouteDefinitions } from '../../src/channels/http-gateway.js';

test('builtin HTTP gateway routes expose permission metadata', () => {
  const routes = getBuiltinHttpRouteDefinitions();

  assert.ok(routes.length >= 40);
  for (const route of routes) {
    assert.ok(route.method);
    assert.ok(route.path);
    assert.ok(route.openapiPath);
    assert.ok(route.summary);
    assert.ok(route.description);
    assert.ok(Array.isArray(route.tags));
    assert.equal(route.level, getRequiredLevel(route.path, route.method));
  }
});

test('dynamic gateway routes use OpenAPI path parameters', () => {
  const routes = getBuiltinHttpRouteDefinitions().filter((route) => route.path.includes(':'));

  assert.ok(routes.length > 0);
  for (const route of routes) {
    assert.ok(route.openapiPath.includes('{'));
    assert.ok(!route.openapiPath.includes(':'));
  }
});

test('route registry covers every permission prefix that has a built-in endpoint', () => {
  const routePaths = new Set(getBuiltinHttpRouteDefinitions().map((route) => route.path));
  const expectedPrefixes = [
    '/health',
    '/ready',
    '/openapi.json',
    '/.well-known/service-info',
    '/metrics',
    '/commands',
    '/skills',
    '/agent/queue',
    '/plugins',
    '/daemon',
    '/remote-access',
    '/voice',
    '/browser',
    '/memory',
    '/heartbeat',
  ];

  for (const prefix of expectedPrefixes) {
    assert.ok(Object.hasOwn(ROUTE_PERMISSIONS, prefix));
    assert.ok(
      [...routePaths].some((path) => path === prefix || path.startsWith(`${prefix}/`)),
      `expected a built-in route for ${prefix}`,
    );
  }
});

test('built-in routes are included in gateway OpenAPI discovery', () => {
  const document = buildHttpRouteDiscoveryDocument({
    routes: getBuiltinHttpRouteDefinitions(),
    serverUrl: 'http://127.0.0.1:3000',
  });

  assert.equal(document.openapi, '3.1.0');
  assert.ok(document.paths['/health'].get);
  assert.ok(document.paths['/ready'].get);
  assert.ok(document.paths['/memory/stats'].get);
  assert.ok(document.paths['/heartbeat/checks/{id}/run'].post);
  assert.equal(document.paths['/browser/evaluate'].post['x-stateset-permission-level'], 'admin');
});
