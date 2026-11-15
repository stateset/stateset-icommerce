/** Finance and operations are release gates: no skips or known-defect exemptions. */
import { describe, it } from 'node:test';
import assert from 'node:assert/strict';
import { runSmoke } from '../../scripts/mcp-tool-smoke.mjs';
import { ALL_DOMAIN_TOOLS } from '../../src/tools/domain-registry.js';
import { AGENTIC_RUNTIME_TOOLS } from '../../src/mcp/agentic-runtime-tools.js';
import { resolveMcpToolFilter } from '../../src/mcp/tool-profiles.js';
import { validateToolResultContract } from '../../src/mcp/tool-result-contract.js';
for (const profile of ['finance', 'operations']) {
  const include = resolveMcpToolFilter({ profile });
  const tools = [...ALL_DOMAIN_TOOLS, ...AGENTIC_RUNTIME_TOOLS].filter((t) => include(t.name));
  // A missing native binary or server-construction error must fail this gate.
  const rows = await runSmoke({ tools, toolProfile: profile, timeoutMs: 30000 });
  describe(`${profile} profile smoke gate`, () => {
    it('executes every exposed tool', () =>
      assert.deepEqual(rows.map((r) => r.tool).sort(), tools.map((t) => t.name).sort()));
    it('has no crashes or timeouts', () =>
      assert.deepEqual(
        rows
          .filter((r) => ['defect', 'timeout'].includes(r.kind))
          .map((r) => `${r.tool}: ${r.message}`),
        [],
      ));
    it('returns valid tool contracts', () =>
      assert.deepEqual(
        rows
          .filter((r) => r.contract)
          .flatMap((r) =>
            validateToolResultContract(r.contract).map((error) => `${r.tool}: ${error}`),
          ),
        [],
      ));
  });
}
