import { Commerce } from '@stateset/embedded';
import { createEmbeddedAgentToolkit } from '@stateset/cli/agent-toolkit';

const commerce = new Commerce(':memory:');
const toolkit = createEmbeddedAgentToolkit({
  commerce,
  allowApply: false,
});

const vercelTools = toolkit.createVercelAITools({
  tool: (definition) => definition,
  filter: ['list_customers'],
});

class DynamicStructuredTool {
  constructor(config) {
    Object.assign(this, config);
  }
}

const langChainTools = toolkit.createLangChainTools({
  DynamicStructuredTool,
  filter: ['list_customers'],
});

console.log('Vercel AI tool keys:', Object.keys(vercelTools));
console.log('LangChain tool count:', langChainTools.length);

const result = await vercelTools.list_customers.execute({});
console.log('Tool status:', result.status);
