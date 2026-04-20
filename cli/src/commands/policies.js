/**
 * Policies Commands Module
 */

let policyEnginePromise = null;

async function getPolicyEngine() {
  if (!policyEnginePromise) {
    policyEnginePromise = (async () => {
      const { PolicyEngine, PolicyTemplates } = await import('../policies/engine.js');
      const engine = new PolicyEngine({ storePath: '.stateset', unknownDomainMode: 'allow' });
      await engine.load();
      return { engine, PolicyTemplates };
    })();
  }
  return policyEnginePromise;
}

function parseJsonArg(value, label) {
  try {
    return JSON.parse(value);
  } catch (error) {
    throw new Error(`Invalid ${label} JSON: ${error.message}`);
  }
}

export async function execute(action, args, { output, jsonOutput }) {
  const { engine, PolicyTemplates } = await getPolicyEngine();

  switch (action) {
    case 'evaluate': {
      const [domain, contextJson] = args;
      if (!domain || !contextJson)
        throw new Error('Usage: policies evaluate <domain> <contextJson>');
      const result = await engine.evaluate(domain, parseJsonArg(contextJson, 'context'), {
        dryRun: false,
      });
      return jsonOutput
        ? result
        : {
            result,
            formatted:
              `Policy evaluation\n` +
              `${'-'.repeat(28)}\n` +
              `Domain:       ${domain}\n` +
              `Decision:     ${result.shouldAllow ? 'allow' : 'deny'}\n` +
              `Matched:      ${result.results.length}\n` +
              `Reason:       ${result.reason || 'N/A'}`,
          };
    }

    case 'list': {
      const domain = args[0];
      let policySets = engine.listPolicySets();
      if (domain) policySets = policySets.filter((policySet) => policySet.domain === domain);
      return formatPolicies(policySets, { output, jsonOutput });
    }

    case 'template': {
      const templateName = args[0];
      if (!templateName) throw new Error('Usage: policies template <templateName>');
      const template = PolicyTemplates[templateName];
      if (!template) throw new Error(`Unknown template: ${templateName}`);
      const policySet = engine.registerPolicySet(template);
      return { policySet, formatted: `Registered policy template ${templateName}` };
    }

    case 'load-file': {
      const filePath = args[0];
      if (!filePath) throw new Error('Usage: policies load-file <filePath>');
      const fs = await import('node:fs');
      const path = await import('node:path');
      const { parse: parseYAML } = await import('yaml');
      const resolvedPath = path.resolve(filePath);
      if (!fs.existsSync(resolvedPath)) throw new Error(`File not found: ${resolvedPath}`);
      const ext = path.extname(resolvedPath).toLowerCase();
      const content = fs.readFileSync(resolvedPath, 'utf-8');
      const data = ext === '.json' ? JSON.parse(content) : parseYAML(content);
      const policySet = engine.registerPolicySet(data);
      return { policySet, formatted: `Loaded policy file ${resolvedPath}` };
    }

    case 'explain': {
      const [domain, contextJson] = args;
      if (!domain || !contextJson)
        throw new Error('Usage: policies explain <domain> <contextJson>');
      const result = await engine.evaluateDryRun(domain, parseJsonArg(contextJson, 'context'));
      return jsonOutput
        ? result
        : {
            result,
            formatted:
              `Policy explanation\n` +
              `${'-'.repeat(30)}\n` +
              `Domain:       ${domain}\n` +
              `Decision:     ${result.shouldAllow ? 'allow' : 'deny'}\n` +
              `Unknown:      ${result.unknownDomain ? 'yes' : 'no'}\n` +
              `Reason:       ${result.reason || 'N/A'}`,
          };
    }

    default:
      throw new Error(
        `Unknown action: policies ${action}\n\n` +
          'Available actions:\n' +
          '  evaluate <domain> <contextJson>      Evaluate policy domain\n' +
          '  list [domain]                        List policy sets\n' +
          '  template <templateName>              Register policy template\n' +
          '  load-file <filePath>                 Load policy file\n' +
          '  explain <domain> <contextJson>       Explain policy decision',
      );
  }
}

function formatPolicies(policySets, { output, jsonOutput }) {
  if (jsonOutput) return policySets;
  if (policySets.length === 0) return { formatted: 'No policies found.' };
  const formatted = output.table(policySets, [
    { key: 'id', header: 'ID' },
    { key: 'name', header: 'Name' },
    { key: 'domain', header: 'Domain' },
    { key: 'version', header: 'Version' },
  ]);
  return { policySets, formatted };
}

export const metadata = {
  name: 'policies',
  aliases: ['policy', 'rules'],
  description: 'Policy engine evaluation and registration commands',
  actions: {
    evaluate: { description: 'Evaluate policy domain', args: ['<domain>', '<contextJson>'] },
    list: { description: 'List policy sets', args: ['[domain]'] },
    template: { description: 'Register policy template', args: ['<templateName>'] },
    'load-file': { description: 'Load policy file', args: ['<filePath>'] },
    explain: { description: 'Explain policy decision', args: ['<domain>', '<contextJson>'] },
  },
};

export default { execute, metadata };
