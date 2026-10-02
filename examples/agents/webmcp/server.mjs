// Local read-only demo. Run: node examples/agents/webmcp/server.mjs
import http from 'node:http';
import { readFile } from 'node:fs/promises';
import { Commerce } from '../../../bindings/node/index.js';
import { createNativeToolkit } from '../../../bindings/node/native-toolkit.mjs';

const commerce = new Commerce(':memory:');
await commerce.products.create({
  name: 'StateSet Demo Coffee',
  description: 'A browser-accessible catalog product.',
});
const toolkit = createNativeToolkit(commerce, {
  allowApply: false,
  filter: ['products.list', 'products.get'],
});
const descriptors = toolkit.createToolDescriptors();
const allowed = new Set(descriptors.map(({ name }) => name));
const port = Number(process.env.PORT || 8091);
const origin = `http://127.0.0.1:${port}`;
const json = (res, status, data) => {
  res.writeHead(status, { 'Content-Type': 'application/json', 'Cache-Control': 'no-store' });
  res.end(JSON.stringify(data));
};

const server = http.createServer(async (req, res) => {
  try {
    // This demo is deliberately restricted to its loopback origin and public catalog.
    if (
      req.headers.host !== `127.0.0.1:${port}` ||
      (req.headers.origin && req.headers.origin !== origin)
    ) {
      return json(res, 403, { error: 'Origin not allowed' });
    }
    if (req.method === 'GET' && req.url === '/') {
      res.writeHead(200, { 'Content-Type': 'text/html; charset=utf-8' });
      return res.end(await readFile(new URL('./index.html', import.meta.url)));
    }
    if (req.method === 'GET' && req.url === '/webmcp.mjs') {
      res.writeHead(200, { 'Content-Type': 'text/javascript; charset=utf-8' });
      return res.end(await readFile(new URL('../../../bindings/node/webmcp.mjs', import.meta.url)));
    }
    if (req.method === 'GET' && req.url === '/api/tools') {
      return json(
        res,
        200,
        descriptors.map(({ name, description, inputSchema, permission }) => ({
          name,
          description,
          inputSchema,
          permission,
        })),
      );
    }
    if (req.method === 'POST' && req.url === '/api/execute') {
      if (req.headers['content-type'] !== 'application/json') {
        return json(res, 415, { error: 'Expected application/json' });
      }
      let body = '';
      for await (const chunk of req) {
        body += chunk;
        if (Buffer.byteLength(body) > 16384) return json(res, 413, { error: 'Request too large' });
      }
      let call;
      try {
        call = JSON.parse(body);
      } catch {
        return json(res, 400, { error: 'Invalid JSON' });
      }
      if (!allowed.has(call?.name)) return json(res, 403, { error: 'Tool not allowed' });
      if (!call.params || typeof call.params !== 'object' || Array.isArray(call.params)) {
        return json(res, 400, { error: 'Expected object params' });
      }
      return json(res, 200, await toolkit.executeTool(call.name, call.params));
    }
    json(res, 404, { error: 'Not found' });
  } catch (error) {
    json(res, 500, { error: error.message });
  }
});
server.listen(port, '127.0.0.1', () => console.log(`StateSet WebMCP demo: ${origin}`));
for (const event of ['SIGINT', 'SIGTERM']) {
  process.once(event, () =>
    server.close(() => {
      commerce.close();
    }),
  );
}
