import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { AGENTS } from '../../src/agent-definitions.js';
import { routeToAgent, routeToAgentWithConfidence } from '../../src/agent-router.js';

describe('agents agent definition', () => {
  it('AGENTS has an "agents" entry', () => {
    assert.ok(AGENTS.agents, 'AGENTS should have an "agents" key');
  });

  it('AGENTS has 19 entries', () => {
    const count = Object.keys(AGENTS).length;
    assert.strictEqual(count, 19, `Expected 19 agents, got ${count}: ${Object.keys(AGENTS).join(', ')}`);
  });

  it('agents entry has required fields', () => {
    const agent = AGENTS.agents;
    assert.ok(agent.name);
    assert.ok(agent.description);
    assert.ok(Array.isArray(agent.tools));
    assert.ok(agent.tools.length > 0);
    assert.ok(typeof agent.systemPrompt === 'string');
    assert.ok(agent.systemPrompt.length > 50);
  });

  it('agents entry has 39 tools', () => {
    const count = AGENTS.agents.tools.length;
    assert.strictEqual(count, 39, `Expected 39 tools, got ${count}`);
  });

  it('agents tools include agent_create_runtime', () => {
    assert.ok(
      AGENTS.agents.tools.includes('mcp__stateset-commerce__agent_create_runtime'),
      'Should include agent_create_runtime',
    );
  });

  it('agents tools include register_agent_card', () => {
    assert.ok(
      AGENTS.agents.tools.includes('mcp__stateset-commerce__register_agent_card'),
      'Should include register_agent_card',
    );
  });

  it('agents tools include a2a_request_quote', () => {
    assert.ok(
      AGENTS.agents.tools.includes('mcp__stateset-commerce__a2a_request_quote'),
      'Should include a2a_request_quote',
    );
  });
});

describe('agents router keywords', () => {
  it('routes "create agent runtime" to agents', () => {
    const result = routeToAgent('create agent runtime with budget');
    assert.strictEqual(result, 'agents');
  });

  it('routes "multi-agent negotiation" to agents', () => {
    const result = routeToAgent('set up multi-agent negotiation');
    assert.strictEqual(result, 'agents');
  });

  it('routes "agent marketplace" to agents', () => {
    const result = routeToAgent('discover agents in the agent marketplace');
    assert.strictEqual(result, 'agents');
  });

  it('routes "register agent card" to agents', () => {
    const result = routeToAgent('register agent card for my bot');
    assert.strictEqual(result, 'agents');
  });

  it('does not route "return my order" to agents', () => {
    const result = routeToAgent('return my order');
    assert.notStrictEqual(result, 'agents');
  });

  it('does not route "checkout cart" to agents', () => {
    const result = routeToAgent('checkout my shopping cart');
    assert.notStrictEqual(result, 'agents');
  });

  it('routes with confidence for agent-specific queries', () => {
    const result = routeToAgentWithConfidence('start agent loop for my agent runtime');
    assert.strictEqual(result.primary.agent, 'agents');
    assert.ok(result.primary.score >= 4, `Score should be >= 4, got ${result.primary.score}`);
  });
});
