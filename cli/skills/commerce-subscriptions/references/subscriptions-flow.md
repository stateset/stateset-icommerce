# Subscriptions Flow

## Statuses

trialing -> active -> paused -> active
active -> cancelled

## Intervals

- daily, weekly, monthly, quarterly, annual

## Common Commands

- `stateset --apply "create subscription plan 'Pro Monthly' 29.99 per month"`
- `stateset --apply "create subscription for customer@example.com on plan PLAN-123"`
- `stateset --apply "pause subscription SUB-123"`
- `stateset --apply "resume subscription SUB-123"`
- `stateset --apply "cancel subscription SUB-123"`
