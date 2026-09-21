// The dependency-free toolkit behind @stateset/embedded/{openai,generic,
// langchain,vercel-ai}: descriptors generated from index.d.ts, executed
// straight against a Commerce instance, writes preview-only unless allowed.

const assert = require('node:assert/strict')
const test = require('node:test')
const fs = require('node:fs')
const path = require('node:path')
const { spawnSync } = require('node:child_process')

const { Commerce } = require('../index.js')

const packageRoot = path.resolve(__dirname, '..')
const generatorPath = path.join(packageRoot, 'scripts', 'generate-tool-descriptors.mjs')
const descriptorsPath = path.join(packageRoot, 'tool-descriptors.json')

function commerceGetters() {
  const source = fs.readFileSync(path.join(packageRoot, 'index.d.ts'), 'utf8')
  const start = source.indexOf('export declare class Commerce {')
  const end = source.indexOf('\n}', start)
  const body = source.slice(start, end)
  const getters = []
  for (const match of body.matchAll(/^  get (\w+)\(\): (\w+)$/gm)) {
    // Only getters that hand back a declared class are modules.
    if (new RegExp(`^export declare class ${match[2]} \\{`, 'm').test(source)) {
      getters.push(match[1])
    }
  }
  return getters
}

test('descriptor generation is deterministic and matches the committed catalog', async () => {
  const { generateToolDescriptors } = await import('../scripts/generate-tool-descriptors.mjs')
  const first = JSON.stringify(generateToolDescriptors(), null, 2) + '\n'
  const second = JSON.stringify(generateToolDescriptors(), null, 2) + '\n'
  assert.equal(first, second, 'two runs over the same index.d.ts must agree byte for byte')

  const committed = fs.readFileSync(descriptorsPath, 'utf8')
  assert.equal(
    committed,
    first,
    'tool-descriptors.json is stale: run `node scripts/generate-tool-descriptors.mjs`',
  )

  // The CLI entrypoint (what postbuild.mjs runs) exits cleanly.
  const run = spawnSync(process.execPath, [generatorPath], { encoding: 'utf8', cwd: packageRoot })
  assert.equal(run.status, 0, run.stderr)
  assert.match(run.stdout, /tool-descriptors\.json: \d+ tools/)
})

test('the catalog covers every Commerce getter and skips internals', async () => {
  const { loadToolDescriptors } = await import('../native-toolkit.mjs')
  const { meta, tools } = loadToolDescriptors()
  const getters = commerceGetters()
  assert.ok(getters.length > 50, `expected many getters, found ${getters.length}`)

  const modules = new Set(tools.map((tool) => tool.module))
  for (const getter of getters) {
    assert.ok(modules.has(getter), `no tools generated for commerce.${getter}`)
  }
  assert.deepEqual(
    meta.modules.map((module) => module.getter).sort(),
    [...getters].sort(),
  )
  assert.equal(meta.toolCount, tools.length)
  assert.ok(tools.length > 500, `expected a large catalog, found ${tools.length}`)

  // Sorted, unique, canonical names.
  const names = tools.map((tool) => tool.name)
  assert.deepEqual(names, [...names].sort())
  assert.equal(new Set(names).size, names.length)
  for (const tool of tools) {
    assert.match(tool.name, /^[a-zA-Z0-9]+\.[a-zA-Z0-9]+$/)
    assert.equal(tool.name, `${tool.module}.${tool.method}`)
    assert.ok(!tool.method.startsWith('__'))
    assert.ok(!['constructor', 'open', 'close', 'ref', 'unref'].includes(tool.method))
    assert.equal(tool.parameters.type, 'object')
    assert.equal(tool.parameters.additionalProperties, false)
    assert.deepEqual(Object.keys(tool.parameters.properties), tool.positional)
    assert.equal(typeof tool.readOnly, 'boolean')
    assert.equal(tool.permission, tool.readOnly ? 'read' : 'write')
    assert.ok(!JSON.stringify(tool.parameters).includes('"$ref"'), `${tool.name} still has $ref`)
  }

  // Live handles are not tools; the generator says so.
  assert.ok(!names.includes('events.subscribe'))
  assert.ok(meta.warnings.some((warning) => warning.startsWith('events.subscribe:')))

  // The read-only heuristic and its overrides are explicit.
  const byName = new Map(tools.map((tool) => [tool.name, tool]))
  assert.equal(byName.get('customers.list').readOnly, true)
  assert.equal(byName.get('customers.count').readOnly, true)
  assert.equal(byName.get('products.search').readOnly, true)
  assert.equal(byName.get('promotions.validateCoupon').readOnly, true)
  assert.equal(byName.get('tax.calculate').readOnly, true)
  assert.equal(byName.get('analytics.salesSummary').readOnly, true)
  assert.equal(byName.get('customers.findOrCreate').readOnly, false)
  assert.equal(byName.get('x402.verifyAgent').readOnly, false)
  assert.equal(byName.get('orders.create').readOnly, false)
  assert.equal(byName.get('orders.cancel').readOnly, false)
  assert.equal(byName.get('maintenance.backup').readOnly, false)
})

test('orders.create schema expands its input and marks unitPriceExact', async () => {
  const { loadToolDescriptors } = await import('../native-toolkit.mjs')
  const tool = loadToolDescriptors().tools.find((entry) => entry.name === 'orders.create')
  assert.ok(tool)
  assert.deepEqual(tool.positional, ['input'])
  assert.deepEqual(tool.parameters.required, ['input'])

  const input = tool.parameters.properties.input
  assert.equal(input.type, 'object')
  assert.ok(input.required.includes('customerId'))
  assert.ok(input.required.includes('items'))
  assert.equal(input.properties.items.type, 'array')

  const item = input.properties.items.items
  assert.equal(item.type, 'object')
  assert.equal(item.properties.unitPrice.type, 'number')
  assert.match(item.properties.unitPrice.description, /prefer the `unitPriceExact` string/)
  assert.equal(item.properties.unitPrice['x-money'], 'float')
  assert.equal(item.properties.unitPriceExact.type, 'string')
  assert.equal(item.properties.unitPriceExact['x-money'], 'exact')
  assert.deepEqual(item.required, ['sku', 'name', 'quantity'])

  // Optional `| undefined | null` positional params become nullable, not required.
  const ship = loadToolDescriptors().tools.find((entry) => entry.name === 'orders.ship')
  assert.deepEqual(ship.positional, ['id', 'trackingNumber'])
  assert.deepEqual(ship.parameters.required, ['id'])
  assert.deepEqual(ship.parameters.properties.trackingNumber.type, ['string', 'null'])
})

test('the generator handles literal-union aliases, enums and interface extends', async () => {
  const { generateToolDescriptors } = await import('../scripts/generate-tool-descriptors.mjs')
  const source = `
/** Order state. */
export type OrderState = 'open' | 'closed'
export type NullableState = OrderState | null
export enum Priority { Low = 'low', High = 'high' }
export interface Base {
  /** Base id. */
  id: string
  tags?: Array<string>
}
export interface Widget extends Base {
  state: OrderState
  priority?: Priority
  amount?: number
  amountExact?: string
  nested: Base
  [key: string]: unknown
}
export interface Handle extends AsyncIterable<Widget> {
  recv(): Promise<Widget | null>
}
export declare class Widgets {
  /** Make a widget. */
  create(input: Widget): Promise<Widget>
  get(id: string, state?: NullableState): Promise<Widget | null>
  listByPriority(priority: Priority, limit?: number | undefined | null): Promise<Array<Widget>>
  watch(): Promise<Handle>
  __internal(): void
  close(): void
}
export declare class Handle {
  close(): void
}
export declare class Commerce {
  constructor(dbPath: string)
  get isClosed(): boolean
  /** Widgets. */
  get widgets(): Widgets
}
`
  const catalog = generateToolDescriptors(source)
  assert.deepEqual(catalog.meta.modules, [{ getter: 'widgets', className: 'Widgets' }])
  assert.deepEqual(
    catalog.tools.map((tool) => tool.name),
    ['widgets.create', 'widgets.get', 'widgets.listByPriority'],
  )

  const widget = catalog.definitions.Widget
  assert.deepEqual(widget.properties.id, { type: 'string', description: 'Base id.' })
  assert.deepEqual(widget.properties.tags, { type: 'array', items: { type: 'string' } })
  assert.deepEqual(widget.properties.state, { type: 'string', enum: ['open', 'closed'], description: 'Order state.' })
  assert.deepEqual(widget.properties.priority, { type: 'string', enum: ['low', 'high'] })
  assert.match(widget.properties.amount.description, /prefer the `amountExact` string/)
  assert.deepEqual(widget.properties.nested, { $ref: '#/definitions/Base' })
  assert.deepEqual(widget.required, ['id', 'state', 'nested'])
  assert.equal(widget.additionalProperties, true)

  const get = catalog.tools.find((tool) => tool.name === 'widgets.get')
  assert.deepEqual(get.parameters.properties.state, {
    type: ['string', 'null'],
    enum: ['open', 'closed', null],
    description: 'Order state.',
  })
  assert.deepEqual(get.parameters.required, ['id'])
  assert.equal(get.readOnly, true)

  const create = catalog.tools.find((tool) => tool.name === 'widgets.create')
  assert.equal(create.description, 'Make a widget.')
  assert.equal(create.readOnly, false)

  const list = catalog.tools.find((tool) => tool.name === 'widgets.listByPriority')
  assert.deepEqual(list.parameters.properties.limit.type, ['number', 'null'])
  assert.equal(list.description, 'List by priority (Widgets.listByPriority). Returns Array<Widget>.')

  assert.ok(catalog.meta.warnings.some((warning) => warning.startsWith('widgets.watch: skipped')))
})

test('a read tool executes for real against an in-memory store', async () => {
  const { createNativeToolkit } = await import('../native-toolkit.mjs')
  const commerce = new Commerce(':memory:')
  const toolkit = createNativeToolkit(commerce)
  assert.equal(toolkit.backend, 'native')
  assert.equal(toolkit.allowApply, false)

  assert.deepEqual(await toolkit.executeTool('customers.list', {}), [])
  assert.equal(await toolkit.executeTool('customers.count'), 0)

  // The store is real: a customer created through the SDK shows up.
  const created = await commerce.customers.create({
    email: 'ada@example.com',
    firstName: 'Ada',
    lastName: 'Lovelace',
  })
  const listed = await toolkit.executeTool('customers.list', {})
  assert.equal(listed.length, 1)
  assert.equal(listed[0].id, created.id)

  // Positional mapping and both name spellings.
  const fetched = await toolkit.executeTool('customers__get', { id: created.id })
  assert.equal(fetched.email, 'ada@example.com')
  const byEmail = await toolkit.executeTool('customers.getByEmail', JSON.stringify({ email: 'ada@example.com' }))
  assert.equal(byEmail.id, created.id)
  const missing = await toolkit.executeTool('customers.get', { id: '00000000-0000-4000-8000-000000000000' })
  assert.equal(missing, null)
})

test('a write tool previews without allowApply and executes with it', async () => {
  const { createNativeToolkit } = await import('../native-toolkit.mjs')
  const commerce = new Commerce(':memory:')
  const input = { email: 'grace@example.com', firstName: 'Grace', lastName: 'Hopper' }

  const preview = await createNativeToolkit(commerce).executeTool('customers.create', { input })
  assert.equal(preview.preview, true)
  assert.equal(preview.tool, 'customers.create')
  assert.deepEqual(preview.params, { input })
  assert.match(preview.note, /allowApply: true/)
  assert.equal(await commerce.customers.count(), 0, 'preview must not touch the store')

  const applied = createNativeToolkit(commerce, { allowApply: true })
  assert.equal(applied.allowApply, true)
  const created = await applied.executeTool('customers.create', { input })
  assert.equal(created.email, 'grace@example.com')
  assert.equal(await commerce.customers.count(), 1)

  // Multi-positional write: (id, input) maps back in order.
  const updated = await applied.executeTool('customers.update', {
    id: created.id,
    input: { firstName: 'Grace B.' },
  })
  assert.equal(updated.firstName, 'Grace B.')

  // void results are reported, not dropped.
  assert.deepEqual(await applied.executeTool('customers.delete', { id: created.id }), { ok: true })
  assert.equal(await commerce.customers.count(), 0)
})

test('errors surface as { error: { code, message, details } } carrying err.code', async () => {
  const { createNativeToolkit } = await import('../native-toolkit.mjs')
  const commerce = new Commerce(':memory:')
  const toolkit = createNativeToolkit(commerce, { allowApply: true })

  // Engine validation error: the binding's own err.code comes through.
  const invalid = await toolkit.executeTool('customers.get', { id: 'not-a-uuid' })
  assert.equal(invalid.error.code, 'VALIDATION')
  assert.equal(typeof invalid.error.message, 'string')
  assert.equal(invalid.error.tool, 'customers.get')
  assert.ok('details' in invalid.error)

  const notFound = await toolkit.executeTool('customers.update', {
    id: '00000000-0000-4000-8000-000000000000',
    input: { firstName: 'Nobody' },
  })
  assert.equal(notFound.error.code, 'NOT_FOUND')

  // Toolkit-level errors use the same envelope.
  assert.equal((await toolkit.executeTool('customers.nope', {})).error.code, 'TOOL_NOT_FOUND')
  const unknownParam = await toolkit.executeTool('customers.get', { id: 'x', bogus: 1 })
  assert.equal(unknownParam.error.code, 'INVALID_ARGUMENT')
  assert.deepEqual(unknownParam.error.details, { unknown: ['bogus'], expected: ['id'] })
  const missingParam = await toolkit.executeTool('customers.get', {})
  assert.equal(missingParam.error.code, 'INVALID_ARGUMENT')
  assert.deepEqual(missingParam.error.details, { missing: ['id'] })
})

test('tool formats validate and filters accept both name spellings', async () => {
  const { createNativeToolkit } = await import('../native-toolkit.mjs')
  const commerce = new Commerce(':memory:')
  const toolkit = createNativeToolkit(commerce)

  const openai = toolkit.getTools({ format: 'openai' })
  assert.ok(openai.length > 500)
  for (const tool of openai) {
    assert.equal(tool.type, 'function')
    assert.match(tool.function.name, /^[a-zA-Z0-9_-]{1,64}$/, `${tool.function.name} is not OpenAI-safe`)
    assert.equal(typeof tool.function.description, 'string')
    assert.equal(tool.function.parameters.type, 'object')
    assert.ok(tool.function.parameters.properties)
  }
  const create = openai.find((tool) => tool.function.name === 'orders__create')
  assert.ok(create.function.parameters.properties.input.properties.items)

  const anthropic = toolkit.getTools({ format: 'anthropic' })
  assert.equal(anthropic[0].input_schema.type, 'object')
  assert.match(anthropic[0].name, /^[a-zA-Z0-9_-]{1,128}$/)

  const mcp = toolkit.getTools({ format: 'mcp' })
  const mcpCreate = mcp.find((tool) => tool.name === 'orders__create')
  assert.equal(mcpCreate.annotations.readOnlyHint, false)
  assert.equal(mcpCreate.title, 'orders.create')
  assert.equal(mcp.find((tool) => tool.name === 'orders__list').annotations.readOnlyHint, true)

  const generic = toolkit.getTools({ format: 'generic' })
  assert.equal(generic.find((tool) => tool.name === 'orders.create').wireName, 'orders__create')
  assert.equal(toolkit.getTools().length, generic.length)
  assert.throws(() => toolkit.getTools({ format: 'bogus' }), /Unknown tool format/)

  const filtered = createNativeToolkit(commerce, { filter: ['orders.*', 'customers__list'] })
  const names = filtered.getRawTools().map((tool) => tool.name)
  assert.ok(names.includes('orders.create'))
  assert.ok(names.includes('customers.list'))
  assert.ok(!names.includes('customers.create'))
  assert.equal((await filtered.executeTool('customers.create', { input: {} })).error.code, 'TOOL_NOT_FOUND')
})

test('OpenAI tool-call and batch execution shapes match the CLI toolkit', async () => {
  const { createNativeToolkit } = await import('../native-toolkit.mjs')
  const commerce = new Commerce(':memory:')
  const toolkit = createNativeToolkit(commerce)

  const execution = await toolkit.executeOpenAIToolCall({
    call_id: 'call_1',
    function: { name: 'customers__list', arguments: '{}' },
  })
  assert.equal(execution.callId, 'call_1')
  assert.equal(execution.name, 'customers.list')
  assert.deepEqual(execution.result, [])
  assert.deepEqual(execution.outputMessage, {
    type: 'function_call_output',
    call_id: 'call_1',
    output: '[]',
  })

  const batch = await toolkit.executeToolCalls([
    { id: 'a', function: { name: 'customers.count', arguments: '' } },
    { name: 'customers.create', params: { input: { email: 'x@example.com', firstName: 'X', lastName: 'Y' } } },
  ])
  assert.equal(batch[0].result, 0)
  assert.equal(batch[1].name, 'customers.create')
  assert.equal(batch[1].result.preview, true)
})

test('adapter entrypoints run standalone on the native toolkit', async () => {
  const [{ createNativeToolkit }, openai, generic, langchain, vercelAi] = await Promise.all([
    import('../native-toolkit.mjs'),
    import('@stateset/embedded/openai'),
    import('@stateset/embedded/generic'),
    import('@stateset/embedded/langchain'),
    import('@stateset/embedded/vercel-ai'),
  ])
  const commerce = new Commerce(':memory:')
  const toolkit = createNativeToolkit(commerce, { allowApply: true })

  const tools = openai.createOpenAITools(toolkit, { filter: ['customers.list', 'customers__count'] })
  assert.deepEqual(
    tools.map((tool) => tool.function.name),
    ['customers__count', 'customers__list'],
  )
  const execution = await openai.executeOpenAIToolCall(toolkit, {
    call_id: 'c1',
    function: { name: 'customers__count', arguments: '{}' },
  })
  assert.equal(execution.result, 0)

  const registry = generic.createCallableRegistry(toolkit, { filter: ['customers.create', 'customers.count'] })
  assert.deepEqual(Object.keys(registry).sort(), ['customers.count', 'customers.create'])
  const created = await registry['customers.create']({
    input: { email: 'lin@example.com', firstName: 'Lin', lastName: 'Q' },
  })
  assert.equal(created.email, 'lin@example.com')
  assert.equal(await registry['customers.count'](), 1)

  class DynamicStructuredTool {
    constructor(config) {
      Object.assign(this, config)
    }
  }
  const [lcTool] = langchain.createLangChainTools(toolkit, {
    DynamicStructuredTool,
    filter: ['customers.list'],
  })
  assert.equal(lcTool.name, 'customers__list')
  assert.equal(lcTool.schema.type, 'object')
  assert.equal(JSON.parse(await lcTool.func({})).length, 1)

  const wrapped = []
  const vercelTools = vercelAi.createVercelAITools(toolkit, {
    tool: (definition) => definition,
    jsonSchema: (schema) => {
      wrapped.push(schema)
      return { jsonSchema: schema }
    },
    filter: ['customers.get'],
  })
  assert.ok(vercelTools.customers__get)
  assert.equal(wrapped.length, 1)
  assert.deepEqual(vercelTools.customers__get.inputSchema, { jsonSchema: wrapped[0] })
  assert.deepEqual(vercelTools.customers__get.parameters, { jsonSchema: wrapped[0] })
  const viaVercel = await vercelTools.customers__get.execute({ id: created.id })
  assert.equal(viaVercel.email, 'lin@example.com')
})
