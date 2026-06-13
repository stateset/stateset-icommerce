// Contract tests for the framework adapter entrypoints.
//
// The openai / generic / langchain / vercel-ai subpaths all accept either a
// Commerce instance or an already-constructed agent toolkit. These tests pin
// the adapter <-> toolkit delegation contract using an in-process toolkit
// double, so they run in every environment — including CI jobs where the
// optional @stateset/cli peer dependency stack is not installed. The
// end-to-end path through the real toolkit is covered by
// test/framework-helpers.js and test/agent-toolkit.js.

const assert = require('node:assert/strict')
const test = require('node:test')

const { Commerce } = require('../index.js')
const { loadToolkitModule } = require('./helpers/toolkit-availability.js')

function createStubToolkit(calls = []) {
  const toolkit = {
    calls,
    getTools({ format } = {}) {
      calls.push(['getTools', format])
      return [
        { function: { name: 'list_customers' } },
        { function: { name: 'create_order' } },
        { function: { name: 'list_orders' } },
      ]
    },
    executeTool(toolName, params, executionOptions) {
      calls.push(['executeTool', toolName, params, executionOptions])
      return { status: 'success', tool: toolName }
    },
    executeToolCalls(toolCalls, executionOptions) {
      calls.push(['executeToolCalls', toolCalls, executionOptions])
      return toolCalls.map((call) => ({ status: 'success', call }))
    },
    executeOpenAIToolCall(toolCall, executionOptions) {
      calls.push(['executeOpenAIToolCall', toolCall, executionOptions])
      return { result: { status: 'success' }, toolCall }
    },
    createToolDescriptors({ filter, executionOptions } = {}) {
      calls.push(['createToolDescriptors', filter, executionOptions])
      const names = Array.isArray(filter) && filter.length > 0 ? filter : ['list_customers']
      return names.map((name) => ({
        name,
        execute: (params) => {
          calls.push(['descriptor.execute', name, params])
          return { status: 'success', tool: name, params }
        },
      }))
    },
    createLangChainTools(config) {
      calls.push(['createLangChainTools', config])
      return [{ name: 'list_customers', config }]
    },
    createVercelAITools(config) {
      calls.push(['createVercelAITools', config])
      return { list_customers: { config } }
    },
  }
  return toolkit
}

test('toolkit-helpers: filterByToolName filters by extracted name', async () => {
  const { filterByToolName } = await import('../toolkit-helpers.mjs')

  const items = [{ id: 'a' }, { id: 'b' }, { id: 'c' }]
  const getName = (item) => item.id

  // No filter (null / undefined / empty array) returns the input unchanged.
  assert.equal(filterByToolName(items, null, getName), items)
  assert.equal(filterByToolName(items, undefined, getName), items)
  assert.equal(filterByToolName(items, [], getName), items)

  // A filter keeps only the named items, preserving input order.
  assert.deepEqual(filterByToolName(items, ['c', 'a'], getName), [{ id: 'a' }, { id: 'c' }])
  assert.deepEqual(filterByToolName(items, ['missing'], getName), [])
})

test('toolkit-helpers: resolveToolkit passes toolkit-like objects through', async () => {
  const { resolveToolkit } = await import('../toolkit-helpers.mjs')

  const stub = createStubToolkit()
  assert.equal(resolveToolkit(stub), stub)

  // Falsy input is rejected with an actionable error.
  assert.throws(() => resolveToolkit(null), /Commerce instance or embedded toolkit is required/)
  assert.throws(() => resolveToolkit(undefined), /Commerce instance or embedded toolkit is required/)
})

test('toolkit-helpers: resolveToolkit builds a toolkit from a Commerce instance', async (t) => {
  const { resolveToolkit } = await import('../toolkit-helpers.mjs')
  const { skipReason } = await loadToolkitModule()

  const commerce = new Commerce(':memory:')
  if (skipReason) {
    // Without the optional @stateset/cli stack the helper must surface the
    // module-load failure instead of silently degrading.
    assert.throws(() => resolveToolkit(commerce), (error) => error.code === 'ERR_MODULE_NOT_FOUND')
    t.diagnostic(`construction path asserted unavailable: ${skipReason}`)
    return
  }
  const toolkit = resolveToolkit(commerce, { allowApply: false })
  assert.equal(typeof toolkit.getTools, 'function')
  assert.equal(typeof toolkit.executeTool, 'function')
})

test('openai adapter delegates to the toolkit', async () => {
  const openai = await import('@stateset/embedded/openai')
  const calls = []
  const stub = createStubToolkit(calls)

  const tools = openai.createOpenAITools(stub, { filter: ['list_customers', 'list_orders'] })
  assert.deepEqual(
    tools.map((tool) => tool.function.name),
    ['list_customers', 'list_orders'],
  )
  assert.deepEqual(calls[0], ['getTools', 'openai'])

  const unfiltered = openai.createOpenAITools(stub)
  assert.equal(unfiltered.length, 3)

  const toolCall = { call_id: 'call_1', function: { name: 'list_customers', arguments: '{}' } }
  const execution = await openai.executeOpenAIToolCall(stub, toolCall, {
    executionOptions: { traceId: 't-1' },
  })
  assert.equal(execution.result.status, 'success')
  assert.deepEqual(calls.at(-1), ['executeOpenAIToolCall', toolCall, { traceId: 't-1' }])

  const batch = [toolCall]
  const batchResult = await openai.executeOpenAIToolCalls(stub, batch)
  assert.equal(batchResult.length, 1)
  assert.deepEqual(calls.at(-1), ['executeToolCalls', batch, {}])
})

test('generic adapter exposes descriptors, registry and execution helpers', async () => {
  const generic = await import('@stateset/embedded/generic')
  const calls = []
  const stub = createStubToolkit(calls)

  const descriptors = generic.createToolDescriptors(stub, {
    filter: ['list_customers'],
    executionOptions: { traceId: 't-2' },
  })
  assert.equal(descriptors.length, 1)
  assert.equal(descriptors[0].name, 'list_customers')
  assert.deepEqual(calls[0], ['createToolDescriptors', ['list_customers'], { traceId: 't-2' }])

  const registry = generic.createCallableRegistry(stub, { filter: ['list_customers'] })
  assert.deepEqual(Object.keys(registry), ['list_customers'])
  const viaRegistry = await registry.list_customers({ limit: 1 })
  assert.deepEqual(viaRegistry, { status: 'success', tool: 'list_customers', params: { limit: 1 } })

  // Registry callables default missing params to an empty object.
  const viaRegistryDefault = await registry.list_customers()
  assert.deepEqual(viaRegistryDefault.params, {})

  const direct = await generic.executeTool(stub, 'list_customers', { limit: 2 }, {
    executionOptions: { traceId: 't-3' },
  })
  assert.deepEqual(direct, { status: 'success', tool: 'list_customers' })
  assert.deepEqual(calls.at(-1), ['executeTool', 'list_customers', { limit: 2 }, { traceId: 't-3' }])

  const batch = [{ name: 'list_customers', params: {} }]
  await generic.executeToolCalls(stub, batch)
  assert.deepEqual(calls.at(-1), ['executeToolCalls', batch, {}])
})

test('langchain adapter forwards tool class, filter and execution options', async () => {
  const langchain = await import('@stateset/embedded/langchain')
  const calls = []
  const stub = createStubToolkit(calls)

  class DynamicStructuredTool {}
  const tools = langchain.createLangChainTools(stub, {
    DynamicStructuredTool,
    filter: ['list_customers'],
    executionOptions: { traceId: 't-4' },
  })
  assert.equal(tools[0].name, 'list_customers')
  assert.deepEqual(calls[0], [
    'createLangChainTools',
    { DynamicStructuredTool, filter: ['list_customers'], executionOptions: { traceId: 't-4' } },
  ])
})

test('vercel-ai adapter forwards tool factory, filter and execution options', async () => {
  const vercelAi = await import('@stateset/embedded/vercel-ai')
  const calls = []
  const stub = createStubToolkit(calls)

  const tool = (definition) => definition
  const tools = vercelAi.createVercelAITools(stub, {
    tool,
    filter: ['list_customers'],
    executionOptions: { traceId: 't-5' },
  })
  assert.ok(tools.list_customers)
  assert.deepEqual(calls[0], [
    'createVercelAITools',
    { tool, filter: ['list_customers'], executionOptions: { traceId: 't-5' } },
  ])
})
