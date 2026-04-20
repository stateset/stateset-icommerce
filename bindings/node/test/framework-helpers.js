const assert = require('node:assert/strict')
const test = require('node:test')

const { Commerce } = require('../index.js')

test('framework helper subpaths expose engine-first adapters', async () => {
  const [openai, generic, langchain, vercelAi] = await Promise.all([
    import('@stateset/embedded/openai'),
    import('@stateset/embedded/generic'),
    import('@stateset/embedded/langchain'),
    import('@stateset/embedded/vercel-ai'),
  ])

  const commerce = new Commerce(':memory:')
  const openaiTools = openai.createOpenAITools(commerce, {
    filter: ['list_customers'],
  })
  assert.deepEqual(
    openaiTools.map((tool) => tool.function.name),
    ['list_customers'],
  )

  const openaiExecution = await openai.executeOpenAIToolCall(commerce, {
    call_id: 'node_openai_1',
    function: {
      name: 'list_customers',
      arguments: '{}',
    },
  })
  assert.equal(openaiExecution.result.status, 'success')

  const descriptors = generic.createToolDescriptors(commerce, {
    filter: ['list_customers'],
  })
  assert.equal(descriptors[0].name, 'list_customers')

  const registry = generic.createCallableRegistry(commerce, {
    filter: ['list_customers'],
  })
  assert.ok(typeof registry.list_customers === 'function')
  const genericResult = await registry.list_customers({})
  assert.equal(genericResult.status, 'success')

  class DynamicStructuredTool {
    constructor(config) {
      Object.assign(this, config)
    }
  }

  const langChainTools = langchain.createLangChainTools(commerce, {
    DynamicStructuredTool,
    filter: ['list_customers'],
  })
  assert.equal(langChainTools[0].name, 'list_customers')

  const vercelTools = vercelAi.createVercelAITools(commerce, {
    tool: (definition) => definition,
    filter: ['list_customers'],
  })
  assert.ok(vercelTools.list_customers)
})
