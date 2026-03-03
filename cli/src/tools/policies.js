/**
 * Policy Tools Module
 *
 * MCP tool definitions for policy engine operations.
 * Enables evaluating, listing, loading, and explaining policies
 * through the AI agent interface.
 */

import { z } from 'zod';
import fs from 'fs';
import path from 'path';
import { parse as parseYAML } from 'yaml';
import { PolicyTemplates } from '../policies/engine.js';

/**
 * Policy tool definitions
 */
export const policyTools = [
  {
    name: 'evaluate_policy',
    description:
      'Evaluate a policy domain against a context object. Returns allow/deny decision with full explanation of which rules matched and why.',
    inputSchema: {
      domain: z
        .string()
        .min(1)
        .describe('Policy domain to evaluate (e.g., "returns", "orders", "inventory")'),
      context: z
        .record(z.unknown())
        .describe('Context object with fields referenced by policy conditions'),
      dryRun: z
        .boolean()
        .optional()
        .default(false)
        .describe('If true, evaluate without recording in history'),
    },
    annotations: {
      title: 'Evaluate Policy',
      readOnlyHint: true,
      openWorldHint: false,
    },
    permission: 'read',
    handler: async ({ params, policyEngine }) => {
      if (!policyEngine) {
        return { success: false, error: 'Policy engine not initialized' };
      }

      const { domain, context, dryRun } = params;
      const result = await policyEngine.evaluate(domain, context, { dryRun });

      return {
        success: true,
        domain: result.domain,
        decision: result.shouldAllow ? 'allow' : 'deny',
        shouldAllow: result.shouldAllow,
        shouldDeny: result.shouldDeny,
        unknownDomain: result.unknownDomain || false,
        reason: result.reason || null,
        matchedRules: result.results.map((r) => ({
          policySetId: r.policySetId,
          policySetName: r.policySetName,
          matched: r.matched,
        })),
        actions: result.actions.map((a) => (typeof a.toJSON === 'function' ? a.toJSON() : a)),
        explanations: result.explanations.map((e) =>
          typeof e.toJSON === 'function' ? e.toJSON() : e,
        ),
        dryRun: result.dryRun,
      };
    },
  },

  {
    name: 'list_policies',
    description:
      'List all registered policy sets. Shows policy set IDs, names, domains, and rule counts.',
    inputSchema: {
      domain: z.string().optional().describe('Filter by domain (e.g., "returns", "orders")'),
    },
    annotations: {
      title: 'List Policies',
      readOnlyHint: true,
      openWorldHint: false,
    },
    permission: 'read',
    handler: async ({ params, policyEngine }) => {
      if (!policyEngine) {
        return { success: false, error: 'Policy engine not initialized' };
      }

      let policySets = policyEngine.listPolicySets();

      if (params.domain) {
        policySets = policySets.filter((ps) => ps.domain === params.domain);
      }

      return {
        success: true,
        count: policySets.length,
        policySets: policySets.map((ps) => ({
          id: ps.id,
          name: ps.name,
          domain: ps.domain,
          description: ps.description,
          ruleCount: ps.rules?.length || 0,
          version: ps.version,
        })),
        unknownDomainMode: policyEngine.unknownDomainMode,
      };
    },
  },

  {
    name: 'register_policy_template',
    description:
      'Activate one of the built-in policy templates. Available templates: autoApproveReturns, inventoryRestock, orderFraudDetection, promotionEligibility, subscriptionRules.',
    inputSchema: {
      templateName: z
        .enum([
          'autoApproveReturns',
          'inventoryRestock',
          'orderFraudDetection',
          'promotionEligibility',
          'subscriptionRules',
        ])
        .describe('Name of the built-in policy template to activate'),
    },
    annotations: {
      title: 'Register Policy Template',
      readOnlyHint: false,
      openWorldHint: false,
    },
    permission: 'write',
    handler: async ({ params, policyEngine }) => {
      if (!policyEngine) {
        return { success: false, error: 'Policy engine not initialized' };
      }

      const template = PolicyTemplates[params.templateName];
      if (!template) {
        return { success: false, error: `Unknown template: ${params.templateName}` };
      }

      const policySet = policyEngine.registerPolicySet(template);

      return {
        success: true,
        message: `Template '${params.templateName}' registered successfully`,
        policySet: {
          id: policySet.id,
          name: policySet.name,
          domain: policySet.domain,
          ruleCount: policySet.rules.length,
        },
      };
    },
  },

  {
    name: 'load_policy_file',
    description:
      'Load a YAML or JSON policy file into the engine. The file must define a valid policy set with domain, rules, and actions.',
    inputSchema: {
      filePath: z
        .string()
        .min(1)
        .describe('Absolute or relative path to the YAML or JSON policy file'),
    },
    annotations: {
      title: 'Load Policy File',
      readOnlyHint: false,
      openWorldHint: false,
    },
    permission: 'write',
    handler: async ({ params, policyEngine }) => {
      if (!policyEngine) {
        return { success: false, error: 'Policy engine not initialized' };
      }

      const resolvedPath = path.resolve(params.filePath);

      if (!fs.existsSync(resolvedPath)) {
        return { success: false, error: `File not found: ${resolvedPath}` };
      }

      const ext = path.extname(resolvedPath).toLowerCase();
      if (!['.yaml', '.yml', '.json'].includes(ext)) {
        return { success: false, error: 'File must be .yaml, .yml, or .json' };
      }

      try {
        const content = fs.readFileSync(resolvedPath, 'utf-8');

        let data;
        if (ext === '.json') {
          data = JSON.parse(content);
        } else {
          data = parseYAML(content);
        }

        if (!data) {
          return { success: false, error: 'File is empty or invalid' };
        }

        const policySet = policyEngine.registerPolicySet(data);

        return {
          success: true,
          message: `Policy file loaded: ${resolvedPath}`,
          policySet: {
            id: policySet.id,
            name: policySet.name,
            domain: policySet.domain,
            ruleCount: policySet.rules.length,
          },
        };
      } catch (error) {
        return { success: false, error: `Failed to load policy file: ${error.message}` };
      }
    },
  },

  {
    name: 'explain_policy_denial',
    description:
      'Re-evaluate a policy domain with verbose per-condition breakdown. Shows which conditions matched, which did not, and the expected vs actual values for each.',
    inputSchema: {
      domain: z.string().min(1).describe('Policy domain to evaluate'),
      context: z
        .record(z.unknown())
        .describe('Context object with fields referenced by policy conditions'),
    },
    annotations: {
      title: 'Explain Policy Denial',
      readOnlyHint: true,
      openWorldHint: false,
    },
    permission: 'read',
    handler: async ({ params, policyEngine }) => {
      if (!policyEngine) {
        return { success: false, error: 'Policy engine not initialized' };
      }

      const { domain, context } = params;

      // Use dry-run so we don't pollute history
      const result = await policyEngine.evaluateDryRun(domain, context);

      // If unknown domain, provide direct explanation
      if (result.unknownDomain) {
        return {
          success: true,
          domain,
          decision: result.shouldAllow ? 'allow' : 'deny',
          unknownDomain: true,
          unknownDomainMode: result.unknownDomainMode,
          reason: result.reason,
          breakdown: [],
        };
      }

      // Build per-rule breakdown
      const breakdown = [];

      for (const policyResult of result.results) {
        const policySet = policyEngine.getPolicySet(policyResult.policySetId);
        if (!policySet) continue;

        for (const rule of policySet.rules) {
          const ruleDetail = rule.matchesWithDetail
            ? rule.matchesWithDetail(context)
            : { matched: false, conditionDetails: [] };

          breakdown.push({
            policySetId: policySet.id,
            policySetName: policySet.name,
            ruleId: rule.id,
            ruleName: rule.name,
            matched: ruleDetail.matched,
            conditions: (ruleDetail.conditionDetails || []).map((c) => ({
              field: c.field,
              operator: c.operator,
              expected: c.expected,
              actual: c.actual,
              matched: c.matched,
            })),
            action: rule.action
              ? typeof rule.action.toJSON === 'function'
                ? rule.action.toJSON()
                : rule.action
              : null,
          });
        }
      }

      // Build explanation for denials
      const denialReasons = result.explanations
        .filter((e) => typeof e.toJSON === 'function')
        .map((e) => e.toJSON())
        .filter((e) => e.decision === 'deny' || e.action?.type === 'deny');

      return {
        success: true,
        domain,
        decision: result.shouldAllow ? 'allow' : 'deny',
        shouldAllow: result.shouldAllow,
        shouldDeny: result.shouldDeny,
        denialReasons,
        breakdown,
      };
    },
  },
];
