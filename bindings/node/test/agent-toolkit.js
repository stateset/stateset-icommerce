const assert = require('node:assert/strict')
const test = require('node:test')

const { Commerce } = require('../index.js')

test('agent toolkit entrypoint exposes the embedded toolkit factory', async () => {
  const toolkitModule = await import('@stateset/embedded/agent-toolkit')

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
