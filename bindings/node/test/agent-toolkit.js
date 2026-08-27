const assert = require('node:assert/strict')
const test = require('node:test')

const { Commerce } = require('../index.js')
const { loadToolkitModule } = require('./helpers/toolkit-availability.js')

test('agent toolkit entrypoint exposes the embedded toolkit factory', async (t) => {
  const { toolkitModule, skipReason } = await loadToolkitModule()
  if (skipReason) {
    t.skip(skipReason)
    return
  }

  assert.equal(typeof toolkitModule.createEmbeddedAgentToolkit, 'function')
  assert.equal(toolkitModule.createEmbeddedAgentKit, toolkitModule.createEmbeddedAgentToolkit)
  assert.equal(toolkitModule.default, toolkitModule.createEmbeddedAgentToolkit)

  const toolkit = toolkitModule.createEmbeddedAgentToolkit({
    commerce: new Commerce(':memory:'),
    allowApply: false,
  })

  const tools = toolkit.getTools({ format: 'openai' })
  assert.ok(tools.some((tool) => tool.function?.name === 'list_customers'))
})

test('apply-enabled toolkits require and enforce explicit capability scopes', async (t) => {
  const { toolkitModule, skipReason } = await loadToolkitModule()
  if (skipReason) {
    t.skip(skipReason)
    return
  }

  assert.throws(
    () =>
      toolkitModule.createEmbeddedAgentToolkit({
        commerce: new Commerce(':memory:'),
        allowApply: true,
      }),
    /requires explicit capabilities/,
  )

  const toolkit = toolkitModule.createEmbeddedAgentToolkit({
    commerce: new Commerce(':memory:'),
    allowApply: true,
    capabilities: ['read:*', 'payments.create'],
  })
  const names = new Set(toolkit.getRawTools().map((tool) => tool.name))
  assert.ok(names.has('list_customers'))
  assert.ok(names.has('create_payment'))
  assert.equal(names.has('create_customer'), false)
  assert.equal(names.has('create_refund'), false)
  await assert.rejects(
    toolkit.executeTool('create_customer', {
      email: 'blocked@example.com',
      firstName: 'Blocked',
      lastName: 'Agent',
    }),
    /outside this toolkit's capability scope/,
  )
})
