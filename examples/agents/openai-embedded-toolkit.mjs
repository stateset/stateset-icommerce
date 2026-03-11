import { Commerce } from '@stateset/embedded';
import { createEmbeddedAgentToolkit } from '@stateset/cli/agent-toolkit';

const commerce = new Commerce(':memory:');
const toolkit = createEmbeddedAgentToolkit({
  commerce,
  allowApply: false,
});

const tools = toolkit.getTools({ format: 'openai' });
console.log('Exported tools:', tools.length);
console.log('First tool:', tools[0]?.function?.name);

const execution = await toolkit.executeOpenAIToolCall({
  call_id: 'demo_call_1',
  function: {
    name: 'list_customers',
    arguments: '{}',
  },
});

console.log('Tool status:', execution.result.status);
console.log('Responses API payload:');
console.log(JSON.stringify(execution.outputMessage, null, 2));
