# Autonomous Engine Runbook

## Startup Checklist

1. Confirm database path exists or is writable.
2. Start engine:
   - `stateset-autonomous start --db ./.stateset/commerce.db --port 3000`
3. Check status:
   - `stateset-autonomous status`

## Incident Triage

- Jobs not running: check scheduler is enabled.
- Workflows stuck: list workflow instances and inspect last transition.
- Approvals stuck: list pending approvals and take action.
- Webhooks failing: verify port and event payloads.

## Safe Mode

Disable subsystems when needed:

- `stateset-autonomous start --no-scheduler`
- `stateset-autonomous start --no-workflows`
- `stateset-autonomous start --no-approvals`
- `stateset-autonomous start --no-webhooks`

## Recovery

- Pause scheduled jobs if they are causing churn.
- Re-enable policies after validating rules.
- Re-run failed jobs with MCP `run_job_now`.

## Logging and Audit

- Use `--verbose` on start.
- Capture job run IDs and workflow instance IDs.
