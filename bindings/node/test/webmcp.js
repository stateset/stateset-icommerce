const assert = require('node:assert/strict');
const test = require('node:test');

const descriptor = (name, permission = 'read') => ({
  name,
  permission,
  description: `Run ${name}`,
  inputSchema: { type: 'object', properties: {} },
});

function fakeContext(failOn) {
  const tools = new Map();
  return {
    tools,
    async registerTool(tool, { signal }) {
      if (tool.name === failOn || tools.has(tool.name)) throw new Error('Registration failed');
      tools.set(tool.name, tool);
      signal.addEventListener('abort', () => tools.delete(tool.name), { once: true });
    },
  };
}

test('browser entrypoint imports without native bindings and detects missing WebMCP', async () => {
  const { getWebMCPContext, registerWebMCPTools } = await import('@stateset/embedded/webmcp');
  assert.equal(getWebMCPContext(), null);
  const registration = await registerWebMCPTools([], { modelContext: null });
  assert.equal(registration.supported, false);
  registration.dispose();
});

test('read calls preserve decimal strings and forward execution cancellation', async () => {
  const { createWebMCPTools } = await import('../webmcp.mjs');
  const controller = new AbortController();
  const calls = [];
  const [tool] = createWebMCPTools([descriptor('products.list')], {
    executeTool: async (...args) => {
      calls.push(args);
      return { price: '19.99' };
    },
  });
  assert.deepEqual(await tool.execute({ price: '19.99' }, { signal: controller.signal }), {
    price: '19.99',
  });
  assert.equal(calls[0][1].price, '19.99');
  assert.equal(calls[0][2].signal, controller.signal);
  assert.equal(tool.annotations.readOnlyHint, true);
  controller.abort();
  await assert.rejects(tool.execute({}, { signal: controller.signal }), { name: 'AbortError' });
  assert.equal(calls.length, 1);
});

test('writes and unknown permissions preview without calling any executor', async () => {
  const { createWebMCPTools } = await import('../webmcp.mjs');
  let executions = 0;
  const descriptors = [descriptor('orders.create', 'write'), descriptor('unknown', undefined)];
  delete descriptors[1].permission;
  const tools = createWebMCPTools(descriptors, {
    executeTool: () => {
      executions++;
    },
  });
  for (const tool of tools) {
    assert.equal((await tool.execute({ allowApply: true })).preview, true);
    assert.equal(tool.annotations.readOnlyHint, false);
  }
  assert.equal(executions, 0);
  const [write] = createWebMCPTools([descriptors[0]], {
    allowApply: true,
    executeTool: async () => {
      executions++;
      return { applied: true };
    },
  });
  assert.deepEqual(await write.execute({}), { applied: true });
  assert.equal(write.annotations.consequentialHint, true);
  assert.equal(executions, 1);
});

test('descriptor execution, explicit empty filters, validation and canonical names', async () => {
  const { createWebMCPTools } = await import('../webmcp.mjs');
  const descriptors = [{ ...descriptor('orders.get'), execute: async () => 'ok' }];
  assert.equal(await createWebMCPTools(descriptors)[0].execute(), 'ok');
  assert.deepEqual(createWebMCPTools(descriptors, { filter: [] }), []);
  assert.equal(createWebMCPTools(descriptors, { filter: ['orders__get'] }).length, 1);
  assert.throws(() => createWebMCPTools([descriptor('missing')]), /executor/);
  assert.throws(() => createWebMCPTools([...descriptors, ...descriptors]), /unique/);
  assert.throws(() => createWebMCPTools([{ ...descriptors[0], inputSchema: null }]), /JSON Schema/);
  assert.throws(() => createWebMCPTools([{ ...descriptors[0], description: '' }]), /description/);
});

test('registration cleanup preserves unrelated tools and rolls back partial failures', async () => {
  const { registerWebMCPTools } = await import('../webmcp.mjs');
  const context = fakeContext('fail');
  context.tools.set('unrelated', {});
  const options = { modelContext: context, executeTool: async () => ({}) };
  await assert.rejects(
    registerWebMCPTools([descriptor('first'), descriptor('fail')], options),
    /Registration failed/,
  );
  assert.deepEqual([...context.tools.keys()], ['unrelated']);
  const registration = await registerWebMCPTools([descriptor('first')], options);
  assert.deepEqual(registration.toolNames, ['first']);
  registration.dispose();
  registration.dispose();
  assert.deepEqual([...context.tools.keys()], ['unrelated']);
});

test('host abort unregisters tools; an aborted host never registers', async () => {
  const { registerWebMCPTools } = await import('../webmcp.mjs');
  const context = fakeContext();
  const controller = new AbortController();
  const options = {
    modelContext: context,
    signal: controller.signal,
    executeTool: async () => ({}),
  };
  await registerWebMCPTools([descriptor('first')], options);
  controller.abort();
  assert.equal(context.tools.size, 0);
  await assert.rejects(registerWebMCPTools([descriptor('second')], options), {
    name: 'AbortError',
  });
  assert.equal(context.tools.size, 0);
});

test('native Commerce catalog round-trips through WebMCP while writes remain previews', async () => {
  const { Commerce } = require('../index.js');
  const { createNativeToolkit } = await import('../native-toolkit.mjs');
  const { createWebMCPTools } = await import('../webmcp.mjs');
  const commerce = new Commerce(':memory:');
  try {
    await commerce.products.create({ name: 'WebMCP coffee' });
    const toolkit = createNativeToolkit(commerce, {
      allowApply: true,
      filter: ['products.list', 'products.create'],
    });
    const tools = createWebMCPTools(toolkit.createToolDescriptors());
    const products = await tools.find((tool) => tool.name === 'products.list').execute({});
    assert.equal(products[0].name, 'WebMCP coffee');
    assert.equal(
      (
        await tools
          .find((tool) => tool.name === 'products.create')
          .execute({ input: { name: 'Preview' } })
      ).preview,
      true,
    );
    assert.equal((await commerce.products.list()).length, 1);
  } finally {
    commerce.close();
  }
});

test('feature detection prefers document but falls back to a usable legacy context', async () => {
  const { getWebMCPContext } = await import('../webmcp.mjs');
  const saved = new Map(
    ['document', 'navigator'].map((name) => [
      name,
      Object.getOwnPropertyDescriptor(globalThis, name),
    ]),
  );
  const modern = fakeContext();
  const legacy = fakeContext();
  try {
    Object.defineProperty(globalThis, 'document', {
      configurable: true,
      value: { modelContext: modern },
    });
    Object.defineProperty(globalThis, 'navigator', {
      configurable: true,
      value: { modelContext: legacy },
    });
    assert.equal(getWebMCPContext(), modern);
    Object.defineProperty(globalThis, 'document', {
      configurable: true,
      value: { modelContext: {} },
    });
    assert.equal(getWebMCPContext(), legacy);
  } finally {
    for (const [name, value] of saved) {
      if (value) Object.defineProperty(globalThis, name, value);
      else delete globalThis[name];
    }
  }
});

test('legacy contexts ignoring signals unregister only successfully owned tools', async () => {
  const { registerWebMCPTools } = await import('../webmcp.mjs');
  const tools = new Map([['unrelated', {}]]);
  const context = {
    registerTool(tool) {
      if (tools.has(tool.name)) throw new Error('Duplicate');
      tools.set(tool.name, tool);
    },
    unregisterTool(name) {
      tools.delete(name);
    },
  };
  const options = { modelContext: context, executeTool: async () => ({}) };
  await assert.rejects(
    registerWebMCPTools([descriptor('owned'), descriptor('unrelated')], options),
    /Duplicate/,
  );
  assert.deepEqual([...tools.keys()], ['unrelated']);
  const registration = await registerWebMCPTools([descriptor('owned')], options);
  registration.dispose();
  assert.deepEqual([...tools.keys()], ['unrelated']);
});

test('host cancellation during asynchronous legacy registration removes the late tool', async () => {
  const { registerWebMCPTools } = await import('../webmcp.mjs');
  const tools = new Map();
  const host = new AbortController();
  let finish;
  const context = {
    registerTool(tool) {
      return new Promise((resolve) => {
        finish = () => {
          tools.set(tool.name, tool);
          resolve();
        };
      });
    },
    unregisterTool(name) {
      tools.delete(name);
    },
  };
  const pending = registerWebMCPTools([descriptor('first'), descriptor('second')], {
    modelContext: context,
    signal: host.signal,
    executeTool: async () => ({}),
  });
  host.abort();
  finish();
  await assert.rejects(pending, { name: 'AbortError' });
  assert.equal(tools.size, 0);
});

test('malformed catalogs fail clearly and execution cancellation rejects a late result', async () => {
  const { createWebMCPTools, registerWebMCPTools } = await import('../webmcp.mjs');
  for (const invalid of [null, [], { name: 42 }]) {
    assert.throws(() => createWebMCPTools([invalid], { filter: ['valid'] }), TypeError);
  }
  const host = new AbortController();
  host.abort();
  await assert.rejects(registerWebMCPTools([], { modelContext: null, signal: host.signal }), {
    name: 'AbortError',
  });
  const execution = new AbortController();
  let finish;
  const [tool] = createWebMCPTools([descriptor('first')], {
    executeTool: () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  });
  const pending = tool.execute({}, { signal: execution.signal });
  execution.abort();
  finish({ value: 'late' });
  await assert.rejects(pending, { name: 'AbortError' });
});
