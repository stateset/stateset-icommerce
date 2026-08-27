import React from 'react';

import { cn } from '../utils/cn.js';

/**
 * AgentActionStream — the canonical "agent action language" for the platform.
 *
 * One consistent visual grammar for what an autonomous agent is doing and did:
 * a customer/agent message, a reasoning step, a tool/MCP call, an executed
 * commerce action, an NSR guardrail check, a human-approval gate, and a final
 * resolution. Reused across chat, traces, the agent console, and proof views so
 * "the agent took an action" looks the same everywhere.
 *
 * Status is conveyed by TEXT (the chip label), never by color alone.
 *
 * @typedef {'message'|'reasoning'|'tool_call'|'action'|'guardrail'|'approval'|'resolution'} AgentActionKind
 *
 * @typedef {Object} AgentActionStep
 * @property {AgentActionKind} kind
 * @property {string} text                                   Human-readable line.
 * @property {'customer'|'agent'} [role]                     For kind 'message'.
 * @property {string} [tool]                                 e.g. 'recharge.pauseSubscription'.
 * @property {'running'|'success'|'blocked'|'pending'} [status] For 'tool_call'/'action'.
 * @property {'allowed'|'blocked'} [verdict]                 For 'guardrail'.
 * @property {string} [meta]                                 Secondary line (e.g. resolution stats).
 *
 * @param {Object} props
 * @param {AgentActionStep[]} props.steps
 * @param {string} [props.title]
 * @param {boolean} [props.live]        Show a live indicator in the header.
 * @param {boolean} [props.connector]   Draw the connecting rail between steps (default true).
 * @param {string} [props.className]
 */

const ICON = {
  customer: '·',
  agent: '·',
  reasoning: '▸',
  success: '✓',
  running: '◌',
  blocked: '✕',
  guardOk: '✓',
  guardBlock: '✕',
  approval: '◷',
  resolution: '●',
};

const TONE = {
  neutral: {
    node: 'border-ds-border bg-ds-muted text-ds-muted-foreground',
    chip: 'bg-ds-muted text-ds-muted-foreground',
    text: 'text-ds-foreground',
  },
  agent: {
    node: 'border-ds-brand-200 bg-ds-brand-50 text-ds-brand-700 dark:border-ds-brand-700 dark:bg-ds-brand-950/30 dark:text-ds-brand-300',
    chip: 'bg-ds-brand-50 text-ds-brand-700 dark:bg-ds-brand-950/30 dark:text-ds-brand-300',
    text: 'text-ds-foreground',
  },
  muted: {
    node: 'border-ds-border bg-transparent text-ds-muted-foreground',
    chip: '',
    text: 'text-ds-muted-foreground',
  },
  success: {
    node: 'border-ds-success/30 bg-ds-success/10 text-ds-success',
    chip: 'bg-ds-success/10 text-ds-success',
    text: 'text-ds-foreground',
  },
  warning: {
    node: 'border-ds-warning/30 bg-ds-warning/10 text-ds-warning',
    chip: 'bg-ds-warning/10 text-ds-warning',
    text: 'text-ds-foreground',
  },
  danger: {
    node: 'border-ds-destructive/30 bg-ds-destructive/10 text-ds-destructive',
    chip: 'bg-ds-destructive/10 text-ds-destructive',
    text: 'text-ds-foreground',
  },
};

/** @param {AgentActionStep} step */
function resolveStep(step) {
  switch (step.kind) {
    case 'message':
      return step.role === 'customer'
        ? { glyph: ICON.customer, chip: 'Customer', tone: TONE.neutral, strong: true }
        : { glyph: ICON.agent, chip: 'Agent', tone: TONE.agent, strong: true };
    case 'reasoning':
      return { glyph: ICON.reasoning, chip: null, tone: TONE.muted };
    case 'tool_call':
    case 'action': {
      if (step.status === 'blocked')
        return { glyph: ICON.blocked, chip: 'Blocked', tone: TONE.danger };
      if (step.status === 'running' || step.status === 'pending')
        return { glyph: ICON.running, chip: 'Running', tone: TONE.warning };
      return { glyph: ICON.success, chip: 'Executed', tone: TONE.success, strong: true };
    }
    case 'guardrail':
      return step.verdict === 'blocked'
        ? { glyph: ICON.guardBlock, chip: 'Guardrail · blocked', tone: TONE.danger }
        : { glyph: ICON.guardOk, chip: 'Guardrail', tone: TONE.success };
    case 'approval':
      return { glyph: ICON.approval, chip: 'Awaiting approval', tone: TONE.warning };
    case 'resolution':
      return { glyph: ICON.resolution, chip: 'Resolved', tone: TONE.success, strong: true };
    default:
      return { glyph: ICON.reasoning, chip: null, tone: TONE.muted };
  }
}

export function AgentActionStream({
  steps = [],
  title,
  live = false,
  connector = true,
  className = '',
  ...props
}) {
  return (
    <section
      className={cn('overflow-hidden rounded-2xl border border-ds-border bg-ds-card', className)}
      aria-label={title || 'Agent activity'}
      {...props}>
      {(title || live) && (
        <header className="flex items-center gap-2 border-b border-ds-border px-5 py-3">
          {live && (
            <span className="h-2 w-2 flex-shrink-0 rounded-full bg-ds-success" aria-hidden="true" />
          )}
          <span className="text-xs font-semibold uppercase tracking-[0.14em] text-ds-muted-foreground">
            {title || 'Resolution'}
          </span>
          {live && (
            <span className="ml-auto text-[11px] uppercase tracking-[0.12em] text-ds-muted-foreground">
              live
            </span>
          )}
        </header>
      )}

      <ol className="px-5 py-4">
        {steps.map((step, i) => {
          const meta = resolveStep(step);
          const isLast = i === steps.length - 1;
          const isResolution = step.kind === 'resolution';
          return (
            <li
              key={i}
              className={cn(
                'relative grid grid-cols-[26px_1fr] gap-3 pb-4 last:pb-0',
                isResolution && 'mt-2 border-t border-ds-border pt-4',
              )}>
              {connector && !isLast && !isResolution && (
                <span
                  aria-hidden="true"
                  className="absolute left-[12px] top-[28px] bottom-0 w-px bg-ds-border"
                />
              )}
              <span
                aria-hidden="true"
                className={cn(
                  'z-10 flex h-[26px] w-[26px] flex-shrink-0 items-center justify-center rounded-full border text-[13px] leading-none',
                  meta.tone.node,
                )}>
                {meta.glyph}
              </span>
              <div className="min-w-0 pt-[3px]">
                {meta.chip && (
                  <span
                    className={cn(
                      'mr-2 inline-block rounded px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-[0.1em] align-middle',
                      meta.tone.chip,
                    )}>
                    {meta.chip}
                  </span>
                )}
                <span
                  className={cn(
                    'align-middle text-sm leading-snug',
                    meta.tone.text,
                    meta.strong && 'font-medium',
                  )}>
                  {step.text}
                </span>
                {step.tool && (
                  <code className="ml-2 inline-block rounded bg-ds-muted px-1.5 py-0.5 align-middle font-mono text-[11px] text-ds-muted-foreground">
                    {step.tool}
                  </code>
                )}
                {step.meta && (
                  <div className="mt-1 font-mono text-[11px] text-ds-muted-foreground">
                    {step.meta}
                  </div>
                )}
              </div>
            </li>
          );
        })}
      </ol>
    </section>
  );
}

export default AgentActionStream;
