import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { MCP_TOOL_PROFILES, resolveMcpToolDomains } from '../../src/mcp/tool-profiles.js';

describe('MCP tool profiles', () => {
  it('provides curated, smaller profiles while retaining an all profile', () => {
    assert.ok(MCP_TOOL_PROFILES.all.length > MCP_TOOL_PROFILES.core.length);
    assert.ok(MCP_TOOL_PROFILES.finance.includes('general-ledger'));
    assert.ok(MCP_TOOL_PROFILES.agents.includes('x402'));
  });

  it('can extend a profile with explicit domains', () => {
    const domains = resolveMcpToolDomains({ profile: 'core', domains: ['general-ledger'] });
    assert.ok(domains.has('customers'));
    assert.ok(domains.has('general-ledger'));
  });

  it('fails closed for misspelled profiles and domains', () => {
    assert.throws(() => resolveMcpToolDomains({ profile: 'unknown' }), /Unknown MCP tool profile/);
    assert.throws(
      () => resolveMcpToolDomains({ profile: 'core', domains: ['not-real'] }),
      /Unknown MCP tool domain/,
    );
  });
});
