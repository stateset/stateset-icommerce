// Tool dispatch for the MCP orchestrator.
//
//   - `wrapTool`: wraps a domain tool handler into an agent-sdk tool with
//     hooks, policy, permission, MPP payment, treasury charge, telemetry,
//     replay logging, and the structured `_agentic` result envelope.
//   - `executeTool` / `executeToolWithPayment`: direct (non-MCP-transport)
//     execution used by the embedded agent toolkit.
//   - `adaptTool`: bridges the module handler signature
//     `({ commerce, params, ... }) => plainObject` to the MCP text-content
//     format before handing it to `wrapTool`.
//
// Extracted from mcp-server.js (pure move — no behaviour change).

import { tool as sdkToolImpl } from '@anthropic-ai/claude-agent-sdk';
import { randomUUID } from 'node:crypto';
import {
  MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
  attachPaymentMetadata,
  createPaymentReceipt,
  executeMppToolWithPayment,
} from '../mpp/index.js';
import { replayEventHash } from './audit-envelope.js';
import { attachPaymentMetadataToResponse } from './mpp-payment.js';
import { buildDeterministicMutationManifest } from './mutation-manifest.js';
import { normalizeToolName } from './policy-helpers.js';
import { compactReplayValue } from './replay-sanitizer.js';

/**
 * Build the dispatch helpers for one server instance.
 *
 * @param {{
 *   toolDomainByName: Record<string, string>,
 *   inferPolicyDomain: (toolName: string) => string,
 *   getToolRuntimeMeta: (toolName: string) => object,
 *   hookRunner?: { hasHooks?: (hook: string) => boolean, run: (hook: string, payload: object) => Promise<object> } | null,
 *   allowApply: boolean,
 *   evaluatePolicy: (toolName: string, params: unknown, extra: object, policyDomain?: string | null) => Promise<object>,
 *   checkPermission: (toolName: string, params: unknown) => Promise<object>,
 *   resolveMppPaymentContext: (input: object) => Promise<object>,
 *   maybeChargeForTool: (toolName: string, extra: object, opts: object) => Promise<object>,
 *   wrapWithTelemetry: (toolName: string, fn: Function) => Function,
 *   addAgenticReplayEvent: (event: object) => Promise<unknown>,
 *   buildToolResultResponse: Function,
 *   attachStructuredToolMetadataToResponse: Function,
 *   executeToolStepInPlan: (input: object) => Promise<object>,
 *   toolContext: object,
 *   sdkTool?: Function,
 * }} deps
 * @returns {{
 *   wrapTool: (name: string, description: string, schema: object, handler: Function, policyDomain?: string | null) => object,
 *   executeTool: (toolName: string, params?: object, options?: object) => Promise<object>,
 *   executeToolWithPayment: (toolName: string, params?: object, options?: object) => Promise<object>,
 *   adaptTool: (toolDef: object) => object,
 * }}
 */
export function createToolDispatch({
  toolDomainByName: TOOL_DOMAIN_BY_TOOL_NAME,
  inferPolicyDomain,
  getToolRuntimeMeta,
  hookRunner,
  allowApply,
  evaluatePolicy,
  checkPermission,
  resolveMppPaymentContext,
  maybeChargeForTool,
  wrapWithTelemetry,
  addAgenticReplayEvent,
  buildToolResultResponse,
  attachStructuredToolMetadataToResponse,
  executeToolStepInPlan,
  toolContext,
  sdkTool = sdkToolImpl,
}) {
  const wrapTool = (name, description, schema, handler, policyDomain = null) => {
    return sdkTool(name, description, schema, async (args, extra) => {
      const startedAt = Date.now();
      let nextArgs = args;
      let policy = null;
      let permission = null;
      let charge = null;
      const runtimeMeta = getToolRuntimeMeta(name);
      const sessionIdFromArgs =
        args &&
        typeof args === 'object' &&
        !Array.isArray(args) &&
        typeof args.sessionId === 'string'
          ? args.sessionId
          : null;
      const effectiveSessionId = extra?.sessionId || sessionIdFromArgs || null;
      const buildMutationManifest = (
        paramsValue = nextArgs,
        policyValue = policy,
        permissionValue = permission,
        phase = 'execute',
      ) => {
        if (runtimeMeta.sideEffect !== 'write') return null;
        return buildDeterministicMutationManifest({
          toolName: name,
          params: paramsValue || {},
          policy: policyValue || null,
          permission: permissionValue || null,
          runtimeMeta,
          phase,
        });
      };
      const logEvent = async (status, payload = {}) => {
        const mutationManifest =
          payload?.mutationManifest !== undefined
            ? payload.mutationManifest
            : buildMutationManifest(
                payload?.params || nextArgs,
                payload?.policy || policy,
                payload?.permission || permission,
                status,
              );
        await addAgenticReplayEvent({
          eventId: randomUUID(),
          tool: name,
          status,
          requestId: extra?.requestId || null,
          sessionId: effectiveSessionId,
          policyDomain: policyDomain || inferPolicyDomain(name),
          occurredAt: new Date().toISOString(),
          elapsedMs: Date.now() - startedAt,
          params: compactReplayValue(payload?.params || args || {}),
          paramsHash: replayEventHash(payload?.params || args || {}),
          result: payload?.result,
          resultHash: replayEventHash(payload?.result || {}),
          policy: compactReplayValue(payload?.policy || null),
          permission: compactReplayValue(payload?.permission || null),
          charge: compactReplayValue(payload?.charge || null),
          error: payload?.error || null,
          notes: compactReplayValue({
            ...(payload?.notes || {}),
            mutationManifest,
          }),
          source: 'mcp_server',
          agentic: true,
        });
      };
      const baseToolContext = {
        tool: name,
        args,
        requestId: extra?.requestId,
        sessionId: effectiveSessionId,
      };

      try {
        if (hookRunner?.hasHooks?.('before_tool_call')) {
          const hookResult = await hookRunner.run('before_tool_call', {
            tool: baseToolContext.tool,
            params: nextArgs,
            allowApply,
            requestId: baseToolContext.requestId,
            sessionId: baseToolContext.sessionId,
          });
          if (hookResult?.params) nextArgs = hookResult.params;
          if (hookResult?.blocked || hookResult?.allowed === false) {
            const payload = {
              error: hookResult?.reason || 'Tool execution blocked by hook',
              tool: name,
            };
            await logEvent('blocked', {
              params: nextArgs,
              error: payload.error,
              notes: {
                hook: {
                  allowed: hookResult?.allowed,
                  reason: hookResult?.reason || null,
                  blocked: true,
                },
              },
            });
            return buildToolResultResponse(
              payload,
              'blocked',
              startedAt,
              {
                requestId: baseToolContext.requestId,
                sessionId: baseToolContext.sessionId,
                policy,
                permission,
                charge,
                mutationManifest: buildMutationManifest(nextArgs, policy, permission, 'blocked'),
                name,
                meta: {
                  hook: {
                    allowed: hookResult?.allowed,
                    reason: hookResult?.reason || null,
                    blocked: true,
                  },
                },
              },
              true,
            );
          }
        }

        policy = await evaluatePolicy(name, nextArgs, extra, policyDomain);
        if (!policy.allowed) {
          const payload = {
            error: policy.reason || 'Tool execution blocked by policy',
            remediation: policy.remediation || null,
            tool: name,
            policy: {
              domain: policy.domain,
              actions: policy.actions || [],
              explanations: policy.explanations || [],
              transformAudit: policy.transformAudit || [],
              evaluation: policy.evaluation || null,
              decisionBundle: policy.policyDecisionBundle || null,
            },
          };
          await logEvent('policy_block', {
            params: nextArgs,
            policy: payload.policy,
            error: payload.error,
            remediation: payload.remediation,
          });
          return buildToolResultResponse(
            payload,
            'policy_block',
            startedAt,
            {
              requestId: baseToolContext.requestId,
              sessionId: baseToolContext.sessionId,
              policy,
              permission,
              charge,
              mutationManifest: buildMutationManifest(nextArgs, policy, permission, 'policy_block'),
              name,
              meta: {
                policy: payload.policy,
              },
            },
            true,
          );
        }

        nextArgs = policy.params;

        permission = await checkPermission(name, nextArgs);
        if (!permission.allowed) {
          const payload = {
            error: permission.reason || 'Permission denied',
            tool: name,
          };
          if (permission.preview) {
            payload.preview = true;
            if (permission.wouldDo) {
              payload.wouldDo = permission.wouldDo;
            }
            await logEvent('preview', {
              params: nextArgs,
              permission,
              policy,
              error: payload.error,
            });
          } else {
            await logEvent('permission_block', {
              params: nextArgs,
              permission,
              policy,
              error: payload.error,
            });
          }
          return buildToolResultResponse(
            payload,
            permission.preview ? 'preview' : 'permission_block',
            startedAt,
            {
              requestId: baseToolContext.requestId,
              sessionId: baseToolContext.sessionId,
              policy,
              permission,
              charge,
              mutationManifest: buildMutationManifest(
                nextArgs,
                policy,
                permission,
                permission.preview ? 'preview' : 'permission_block',
              ),
              name,
            },
            true,
          );
        }

        const mpp = await resolveMppPaymentContext({
          toolName: name,
          description,
          params: nextArgs,
          extra,
          requestId: baseToolContext.requestId,
          sessionId: baseToolContext.sessionId,
        });

        if (mpp?.pricing && !mpp.authorized) {
          await logEvent('payment_required', {
            params: nextArgs,
            permission,
            policy,
            charge: {
              paymentRequired: true,
              pricing: mpp.pricing,
              challenge: mpp.challenge,
            },
            error: mpp?.verification?.reason || MPP_JSONRPC_PAYMENT_REQUIRED_MESSAGE,
          });
          return buildToolResultResponse(
            {
              ...mpp.errorPayload,
              tool: name,
            },
            'payment_required',
            startedAt,
            {
              requestId: baseToolContext.requestId,
              sessionId: baseToolContext.sessionId,
              policy,
              permission,
              charge: {
                paymentRequired: true,
                pricing: mpp.pricing,
                challenge: mpp.challenge,
              },
              mutationManifest: buildMutationManifest(
                nextArgs,
                policy,
                permission,
                'payment_required',
              ),
              name,
            },
            true,
          );
        }

        charge = await maybeChargeForTool(name, extra, {
          allowChargeWrite: Boolean(mpp?.authorized),
          paymentCredential: mpp?.credential || null,
        });
        if (charge?.blocked) {
          await logEvent('treasury_block', {
            params: nextArgs,
            permission,
            charge: {
              blocked: charge.blocked,
              reason: charge.reason || null,
            },
            error: charge.reason || 'Treasury charge blocked',
          });
          return buildToolResultResponse(
            {
              error: charge.reason || 'Treasury charge blocked',
              tool: name,
              charge,
            },
            'treasury_block',
            startedAt,
            {
              requestId: baseToolContext.requestId,
              sessionId: baseToolContext.sessionId,
              policy,
              permission,
              charge,
              mutationManifest: buildMutationManifest(
                nextArgs,
                policy,
                permission,
                'treasury_block',
              ),
              name,
            },
            true,
          );
        }

        const wrapped = wrapWithTelemetry(name, handler);
        let result = await wrapped(nextArgs, extra);
        if (mpp?.authorized && charge?.charged) {
          const receipt = createPaymentReceipt({
            challenge: mpp.challenge,
            credential: mpp.credential,
            charge,
            toolName: name,
            requestId: baseToolContext.requestId,
            sessionId: baseToolContext.sessionId,
          });
          result = attachPaymentMetadata(result, {
            protocol: 'mpp',
            receipt,
            credentialId: mpp?.credential?.credentialId || null,
          });
        }
        if (hookRunner?.hasHooks?.('after_tool_call')) {
          await hookRunner.run('after_tool_call', {
            tool: name,
            params: nextArgs,
            result,
            requestId: extra?.requestId,
            sessionId: effectiveSessionId,
          });
        }
        await logEvent('success', {
          params: nextArgs,
          permission,
          charge,
          result: compactReplayValue(result),
          policy: {
            allowed: policy.allowed,
            domain: policy.domain,
            actions: policy.actions || [],
            decisionBundle: policy.policyDecisionBundle || null,
          },
        });
        let maybeStructured = attachStructuredToolMetadataToResponse(result, 'success', startedAt, {
          requestId: baseToolContext.requestId,
          sessionId: baseToolContext.sessionId,
          policy,
          permission,
          charge,
          mutationManifest: buildMutationManifest(nextArgs, policy, permission, 'success'),
          name,
        });
        if (mpp?.authorized && charge?.charged) {
          maybeStructured = attachPaymentMetadataToResponse(maybeStructured, {
            protocol: 'mpp',
            receipt: result?._meta?.payment?.receipt || null,
            credentialId: mpp?.credential?.credentialId || null,
          });
        }
        return maybeStructured;
      } catch (error) {
        if (hookRunner?.hasHooks?.('after_tool_call')) {
          await hookRunner.run('after_tool_call', {
            tool: name,
            params: nextArgs,
            error: error.message,
            requestId: extra?.requestId,
            sessionId: effectiveSessionId,
          });
        }
        await logEvent('error', {
          params: nextArgs,
          permission,
          charge,
          policy: policy
            ? {
                allowed: policy.allowed,
                domain: policy.domain,
                actions: policy.actions || [],
                decisionBundle: policy.policyDecisionBundle || null,
              }
            : null,
          mutationManifest: buildMutationManifest(nextArgs, policy, permission, 'error'),
          error: error.message,
        });
        throw error;
      }
    });
  };

  const executeTool = async (toolName, params = {}, options = {}) => {
    const requestId = options.requestId || randomUUID();
    const sessionId = options.sessionId || requestId;
    const dryRun = options.dryRun === true;
    const normalizedToolName = normalizeToolName(toolName);

    const outcome = await executeToolStepInPlan({
      toolName: normalizedToolName,
      params,
      policyDomain: options.policyDomain || null,
      requestId,
      sessionId,
      dryRun,
      stepIndex: 0,
      includeHooks: options.includeHooks ?? true,
      isRollback: options.isRollback || false,
      extra: options.extra || {},
    });

    await addAgenticReplayEvent({
      eventId: randomUUID(),
      tool: normalizedToolName,
      status: outcome.status,
      requestId,
      sessionId,
      policyDomain:
        outcome?.policy?.domain || options.policyDomain || inferPolicyDomain(normalizedToolName),
      occurredAt: new Date().toISOString(),
      elapsedMs: outcome.elapsedMs || 0,
      params: compactReplayValue(outcome.params || params || {}),
      paramsHash: outcome.paramsHash || replayEventHash(outcome.params || params || {}),
      result: compactReplayValue(outcome.result || null),
      resultHash: outcome.resultHash || replayEventHash(outcome.result || null),
      policy: compactReplayValue(outcome.policy || null),
      permission: compactReplayValue(outcome.permission || null),
      charge: compactReplayValue(outcome.charge || null),
      error: outcome.error || null,
      notes: compactReplayValue({
        directExecution: true,
        dryRun,
        includeHooks: options.includeHooks ?? true,
      }),
      source: 'embedded_agent_toolkit',
      agentic: true,
    });

    return {
      success:
        outcome.status === 'success' ||
        outcome.status === 'dry_run_success' ||
        outcome.status === 'rollback_success',
      requestId,
      sessionId,
      ...outcome,
    };
  };

  const executeToolWithPayment = async (toolName, params = {}, options = {}) => {
    const { payment = {}, ...executionOptions } = options || {};
    return executeMppToolWithPayment({
      executor: executeTool,
      toolName: normalizeToolName(toolName),
      params,
      executionOptions,
      payment,
    });
  };

  /**
   * Convert a domain tool definition into an SDK-wrapped MCP tool.
   * Bridges the module handler signature `({ commerce, params, ... }) => plainObject`
   * to the MCP format `(args, extra) => { content: [{ type: 'text', ... }] }`.
   */
  const adaptTool = (toolDef) => {
    const { name, description, inputSchema, handler } = toolDef;
    const _policyDomain =
      toolDef?.policyDomain || TOOL_DOMAIN_BY_TOOL_NAME[name] || inferPolicyDomain(name);

    return wrapTool(name, description, inputSchema, async (args, extra) => {
      try {
        const result = await handler({
          ...toolContext,
          params: args,
          extra,
        });
        return {
          content: [{ type: 'text', text: JSON.stringify(result, null, 2) }],
        };
      } catch (error) {
        return {
          content: [
            { type: 'text', text: JSON.stringify({ success: false, error: error.message }) },
          ],
        };
      }
    });
  };

  return { wrapTool, executeTool, executeToolWithPayment, adaptTool };
}
