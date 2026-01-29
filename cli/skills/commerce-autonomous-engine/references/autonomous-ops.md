# Autonomous Engine Ops

## Start

- `stateset-autonomous start --db ./.stateset/commerce.db --port 3000`

## Status

- `stateset-autonomous status`

## Jobs

- list: use MCP `list_scheduled_jobs`
- create: MCP `create_scheduled_job`
- run now: MCP `run_job_now`

## Workflows

- list: MCP `list_workflows`
- instances: MCP `list_workflow_instances`

## Policies

- list: MCP `list_policies`
- evaluate: MCP `evaluate_policy`

## Approvals

- list: MCP `list_pending_approvals`
- create: MCP `create_approval_request`
- approve/deny: MCP `approve_request` / `deny_request`
