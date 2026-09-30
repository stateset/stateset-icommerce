// Start server.mjs and a Chromium instance with WebMCP testing and remote debugging enabled.
// Install puppeteer-core in this directory, then run: node browser-smoke.mjs
import assert from 'node:assert/strict';
import puppeteer from 'puppeteer-core';

const browser = await puppeteer.connect({
  browserURL: process.env.BROWSER_URL || 'http://127.0.0.1:9227',
});
const page = await browser.newPage();
try {
  await page.goto(process.env.BASE_URL || 'http://127.0.0.1:8091');
  await page.waitForFunction(
    () => !document.querySelector('#status').textContent.includes('Connecting'),
  );
  const status = await page.$eval('#status', (element) => element.textContent);
  if (process.env.EXPECT_WEBMCP === '0') {
    assert.match(status, /WebMCP is unavailable/);
    await page.click('#catalog');
    await page.waitForFunction(() =>
      document.querySelector('#output').textContent.includes('StateSet Demo Coffee'),
    );
    console.log(`Ordinary-browser fallback checks passed (${await browser.version()}).`);
  } else {
    assert.match(status, /^Registered:/, 'Chromium must have native WebMCP enabled');
    const result = JSON.parse(
      await page.evaluate(async () => {
        const mc = document.modelContext;
        if (!mc?.getTools || !mc?.executeTool)
          throw new Error('Native WebMCP consumer API required');
        const { registerWebMCPTools } = await import('/webmcp.mjs');
        const tools = await mc.getTools();
        const list = tools.find((tool) => tool.name === 'products.list');
        const products = JSON.parse(await mc.executeTool(list, '{}'));
        const get = tools.find((tool) => tool.name === 'products.get');
        const product = JSON.parse(
          await mc.executeTool(get, JSON.stringify({ id: products[0].id })),
        );
        let executions = 0;
        const descriptor = (name, permission = 'read') => ({
          name,
          permission,
          description: 'StateSet browser smoke test',
          inputSchema: { type: 'object', properties: {} },
        });
        const registrations = [];
        try {
          const unrelated = await registerWebMCPTools([descriptor('stateset_smoke_unrelated')], {
            executeTool: async () => ({ ok: true }),
          });
          registrations.push(unrelated);
          const preview = await registerWebMCPTools([descriptor('stateset_smoke_write', 'write')], {
            executeTool: async () => {
              executions++;
              return {};
            },
          });
          registrations.push(preview);
          const write = (await mc.getTools()).find((tool) => tool.name === 'stateset_smoke_write');
          const previewResult = JSON.parse(await mc.executeTool(write, '{}'));
          preview.dispose();
          let rollbackError = false;
          try {
            const registration = await registerWebMCPTools(
              [descriptor('stateset_smoke_partial'), descriptor('stateset_smoke_unrelated')],
              { executeTool: async () => ({}) },
            );
            registrations.push(registration);
          } catch {
            rollbackError = true;
          }
          const remaining = (await mc.getTools()).map((tool) => tool.name);
          return JSON.stringify({
            names: tools.map((tool) => tool.name),
            productName: product.name,
            products,
            preview: previewResult.preview,
            executions,
            rollbackError,
            remaining,
            output: document.querySelector('#output').textContent,
          });
        } finally {
          for (const registration of registrations) registration.dispose();
        }
      }),
    );
    assert.deepEqual(result.names.sort(), ['products.get', 'products.list']);
    assert.equal(result.productName, 'StateSet Demo Coffee');
    assert.equal(result.preview, true);
    assert.equal(result.executions, 0);
    assert.equal(result.rollbackError, true);
    assert.ok(result.remaining.includes('stateset_smoke_unrelated'));
    assert.ok(!result.remaining.includes('stateset_smoke_partial'));
    assert.ok(!result.remaining.includes('stateset_smoke_write'));
    assert.match(result.output, /StateSet Demo Coffee/);
    const afterCleanup = await page.evaluate(async () =>
      (await document.modelContext.getTools()).map((tool) => tool.name),
    );
    assert.deepEqual(afterCleanup.sort(), ['products.get', 'products.list']);
    console.log(
      `Native WebMCP checks passed (${await browser.version()}): discovery, list/get, visible result, write preview, rollback, cleanup.`,
    );
  }
} finally {
  await page.close();
  browser.disconnect();
}
