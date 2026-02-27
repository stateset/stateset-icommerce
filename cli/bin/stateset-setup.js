#!/usr/bin/env node

/**
 * StateSet iCommerce CLI - Guided Setup Wizard
 *
 * Walks new users through API key configuration, database initialization,
 * and a quick health check. Idempotent — skips already-completed steps.
 *
 * Usage:
 *   stateset-setup                    Interactive setup
 *   stateset-setup --non-interactive  Accept defaults (for CI/scripting)
 */

import { parseArgs } from 'node:util';
import * as fs from 'node:fs';
import * as path from 'node:path';
import * as os from 'node:os';
import { runMain } from '../src/graceful-shutdown.js';
import { theme } from '../src/theme.js';

const CONFIG_DIR = path.join(os.homedir(), '.stateset');
const ENV_FILE = path.join(CONFIG_DIR, '.env');
const PROFILES_DIR = path.join(CONFIG_DIR, 'profiles');
const VALID_AGENT_TARGETS = new Set(['claude', 'cursor', 'windsurf', 'openclaw', 'generic']);
const QUICKSTART_DEFAULT_AGENT = 'openclaw';
const QUICKSTART_DEFAULT_PACK = 'ops';
const STARTER_PACKS = {
  ops: {
    label: 'Operations Guardrails',
    summary: 'Safe defaults for orders, inventory, and payments automation.',
    prompt: `You are an operations-focused commerce agent. Optimize for deterministic execution and clear audit trails.
- Always preview high-impact mutations before execution.
- Include concise rationale in mutation reasons/notes.
- Prefer idempotent mutation patterns when tools support idempotency keys.
- Escalate when policy blocks a request instead of bypassing controls.`,
    sampleRequests: [
      'List orders created in the last 24 hours and highlight ones still pending.',
      'Adjust inventory for SKU WIDGET-001 by +50 with reason "Received shipment #123".',
      'Create a payment intent for order ORD-1001 for 149.99 USD.',
    ],
    policies: [
      {
        id: 'starter-ops-orders-v1',
        name: 'Starter Ops - Orders',
        description: 'Guardrails for shipping and cancellation flows.',
        domain: 'orders',
        version: '1.0.0',
        rules: [
          {
            id: 'starter-ops-orders-ship-tracking',
            name: 'ship_order_requires_tracking',
            description: 'Require trackingNumber for ship_order calls.',
            priority: 100,
            conditions: {
              logic: 'and',
              conditions: [
                { field: 'tool', operator: 'eq', value: 'ship_order' },
                { field: 'params.trackingNumber', operator: 'isNull' },
              ],
            },
            action: {
              type: 'deny',
              reason: 'ship_order requires params.trackingNumber.',
              remediation: 'Provide a valid trackingNumber when shipping an order.',
            },
            stopOnMatch: true,
          },
          {
            id: 'starter-ops-orders-status-cancel-guard',
            name: 'block_cancelled_status_via_update_status',
            description:
              'Prevent cancellation via update_order_status; require explicit cancel_order flow.',
            priority: 90,
            conditions: {
              logic: 'and',
              conditions: [
                { field: 'tool', operator: 'eq', value: 'update_order_status' },
                { field: 'params.status', operator: 'eq', value: 'cancelled' },
              ],
            },
            action: {
              type: 'deny',
              reason:
                'Use cancel_order for cancellations instead of update_order_status=cancelled.',
              remediation: 'Invoke cancel_order for explicit cancellation handling.',
            },
            stopOnMatch: true,
          },
        ],
        defaultAction: { type: 'allow' },
      },
      {
        id: 'starter-ops-inventory-v1',
        name: 'Starter Ops - Inventory',
        description: 'Inventory mutation guardrails for large adjustments.',
        domain: 'inventory',
        version: '1.0.0',
        rules: [
          {
            id: 'starter-ops-inventory-reason-required',
            name: 'adjust_inventory_requires_reason',
            description: 'Require a reason for inventory adjustments.',
            priority: 100,
            conditions: {
              logic: 'and',
              conditions: [
                { field: 'tool', operator: 'eq', value: 'adjust_inventory' },
                { field: 'params.reason', operator: 'isNull' },
              ],
            },
            action: {
              type: 'deny',
              reason: 'adjust_inventory requires params.reason.',
              remediation: 'Provide a short operational reason for the inventory adjustment.',
            },
            stopOnMatch: true,
          },
          {
            id: 'starter-ops-inventory-large-adjustment',
            name: 'block_oversized_inventory_adjustments',
            description: 'Block inventory adjustments larger than +/-1000 units.',
            priority: 90,
            conditions: {
              logic: 'and',
              conditions: [
                { field: 'tool', operator: 'eq', value: 'adjust_inventory' },
                {
                  logic: 'or',
                  conditions: [
                    { field: 'params.quantity', operator: 'gt', value: 1000 },
                    { field: 'params.quantity', operator: 'lt', value: -1000 },
                  ],
                },
              ],
            },
            action: {
              type: 'deny',
              reason: 'Inventory adjustment exceeds starter policy threshold (+/-1000).',
              remediation: 'Split into smaller batches or run with a reviewed exception workflow.',
            },
            stopOnMatch: true,
          },
        ],
        defaultAction: { type: 'allow' },
      },
      {
        id: 'starter-ops-payments-v1',
        name: 'Starter Ops - Payments',
        description: 'Payment execution safety defaults.',
        domain: 'payments',
        version: '1.0.0',
        rules: [
          {
            id: 'starter-ops-payments-default-currency',
            name: 'default_currency_to_usd',
            description: 'Set USD when currency is omitted for payment creation flows.',
            priority: 100,
            conditions: {
              logic: 'and',
              conditions: [
                {
                  field: 'tool',
                  operator: 'in',
                  value: ['create_payment', 'create_payment_intent'],
                },
                { field: 'params.currency', operator: 'isNull' },
              ],
            },
            action: {
              type: 'transform',
              transform: { currency: 'USD' },
            },
          },
          {
            id: 'starter-ops-payments-refund-cap',
            name: 'cap_refunds_at_1000_without_custom_policy',
            description: 'Block refunds over 1000 by default.',
            priority: 90,
            conditions: {
              logic: 'and',
              conditions: [
                { field: 'tool', operator: 'eq', value: 'create_refund' },
                { field: 'params.amount', operator: 'gt', value: 1000 },
              ],
            },
            action: {
              type: 'deny',
              reason: 'Refund amount exceeds starter policy limit of 1000.',
              remediation: 'Escalate to manual review or customize payment policy thresholds.',
            },
            stopOnMatch: true,
          },
        ],
        defaultAction: { type: 'allow' },
      },
    ],
  },
  support: {
    label: 'Support Guardrails',
    summary: 'Customer support defaults for returns and refunds.',
    prompt: `You are a customer-support commerce agent. Resolve issues quickly while preserving policy compliance.
- Prefer reversible operations first.
- Require explicit reason fields for customer-affecting actions.
- Keep customer-facing explanations concise and factual.`,
    sampleRequests: [
      'Find returns awaiting review and prioritize by age.',
      'Create a return for order ORD-1001 with reason defective.',
      'Issue a refund for payment PAY-1001 with reason "damaged item".',
    ],
    policies: [
      {
        id: 'starter-support-returns-v1',
        name: 'Starter Support - Returns',
        description: 'Support-safe return handling defaults.',
        domain: 'returns',
        version: '1.0.0',
        rules: [
          {
            id: 'starter-support-returns-reason-details',
            name: 'reason_details_required_for_other',
            description: 'Require reasonDetails when reason is "other".',
            priority: 100,
            conditions: {
              logic: 'and',
              conditions: [
                { field: 'tool', operator: 'eq', value: 'create_return' },
                { field: 'params.reason', operator: 'eq', value: 'other' },
                { field: 'params.reasonDetails', operator: 'isNull' },
              ],
            },
            action: {
              type: 'deny',
              reason: 'create_return with reason=other requires params.reasonDetails.',
              remediation: 'Add clear reasonDetails for auditability.',
            },
            stopOnMatch: true,
          },
        ],
        defaultAction: { type: 'allow' },
      },
      {
        id: 'starter-support-payments-v1',
        name: 'Starter Support - Payments',
        description: 'Refund guardrails for support teams.',
        domain: 'payments',
        version: '1.0.0',
        rules: [
          {
            id: 'starter-support-refund-reason-required',
            name: 'refund_reason_required',
            description: 'Require reason for refunds.',
            priority: 100,
            conditions: {
              logic: 'and',
              conditions: [
                { field: 'tool', operator: 'eq', value: 'create_refund' },
                { field: 'params.reason', operator: 'isNull' },
              ],
            },
            action: {
              type: 'deny',
              reason: 'create_refund requires params.reason in support mode.',
              remediation: 'Include a short customer-visible refund reason.',
            },
            stopOnMatch: true,
          },
          {
            id: 'starter-support-refund-cap',
            name: 'support_refund_cap_500',
            description: 'Limit support-initiated refunds to 500 by default.',
            priority: 90,
            conditions: {
              logic: 'and',
              conditions: [
                { field: 'tool', operator: 'eq', value: 'create_refund' },
                { field: 'params.amount', operator: 'gt', value: 500 },
              ],
            },
            action: {
              type: 'deny',
              reason: 'Refund amount exceeds support starter limit of 500.',
              remediation: 'Escalate larger refunds to a supervisor workflow.',
            },
            stopOnMatch: true,
          },
        ],
        defaultAction: { type: 'allow' },
      },
    ],
  },
  checkout: {
    label: 'Checkout Guardrails',
    summary: 'Checkout flow defaults for carts and payment intents.',
    prompt: `You are a checkout-focused commerce agent. Maximize checkout conversion while preserving payment safety.
- Keep checkout steps deterministic and explicit.
- Confirm payment context before finalization.
- Never bypass policy denials; surface remediation steps immediately.`,
    sampleRequests: [
      'Create a cart for alice@example.com and set shipping address.',
      'Add 2 units of SKU WIDGET-001 at 29.99 to cart CART-1001.',
      'Create a payment intent for checkout order amount 199.99 USD.',
    ],
    policies: [
      {
        id: 'starter-checkout-carts-v1',
        name: 'Starter Checkout - Carts',
        description: 'Cart mutation guardrails.',
        domain: 'carts',
        version: '1.0.0',
        rules: [
          {
            id: 'starter-checkout-default-currency',
            name: 'create_cart_defaults_to_usd',
            description: 'Set USD if create_cart omits currency.',
            priority: 100,
            conditions: {
              logic: 'and',
              conditions: [
                { field: 'tool', operator: 'eq', value: 'create_cart' },
                { field: 'params.currency', operator: 'isNull' },
              ],
            },
            action: { type: 'transform', transform: { currency: 'USD' } },
          },
          {
            id: 'starter-checkout-cart-item-quantity-cap',
            name: 'limit_single_add_cart_item_quantity',
            description: 'Limit single add_cart_item quantity to 20.',
            priority: 90,
            conditions: {
              logic: 'and',
              conditions: [
                { field: 'tool', operator: 'eq', value: 'add_cart_item' },
                { field: 'params.quantity', operator: 'gt', value: 20 },
              ],
            },
            action: {
              type: 'deny',
              reason: 'add_cart_item quantity exceeds checkout starter limit of 20.',
              remediation: 'Split into multiple line items or review bulk order workflow.',
            },
            stopOnMatch: true,
          },
        ],
        defaultAction: { type: 'allow' },
      },
      {
        id: 'starter-checkout-payments-v1',
        name: 'Starter Checkout - Payments',
        description: 'Checkout payment-intent safety defaults.',
        domain: 'payments',
        version: '1.0.0',
        rules: [
          {
            id: 'starter-checkout-intent-default-capture',
            name: 'default_capture_method_manual',
            description: 'Set captureMethod=manual when omitted.',
            priority: 100,
            conditions: {
              logic: 'and',
              conditions: [
                { field: 'tool', operator: 'eq', value: 'create_payment_intent' },
                { field: 'params.captureMethod', operator: 'isNull' },
              ],
            },
            action: { type: 'transform', transform: { captureMethod: 'manual' } },
          },
          {
            id: 'starter-checkout-intent-amount-cap',
            name: 'limit_payment_intent_amount',
            description: 'Block payment intents above 5000 by default.',
            priority: 90,
            conditions: {
              logic: 'and',
              conditions: [
                { field: 'tool', operator: 'eq', value: 'create_payment_intent' },
                { field: 'params.amount', operator: 'gt', value: 5000 },
              ],
            },
            action: {
              type: 'deny',
              reason: 'Payment intent amount exceeds checkout starter limit of 5000.',
              remediation: 'Escalate high-value intents to dedicated approval workflows.',
            },
            stopOnMatch: true,
          },
        ],
        defaultAction: { type: 'allow' },
      },
    ],
  },
};
const VALID_STARTER_PACKS = new Set(Object.keys(STARTER_PACKS));

const HELP = `
StateSet iCommerce CLI - Guided Setup

USAGE:
  stateset-setup [options]

OPTIONS:
  --db <path>          Database path (default: ./store.db)
  --demo               Seed demo data during database init
  --quickstart         Fast path: --demo --agent openclaw --starter-pack ops --agent-only --verify
  --agent <target>     Generate MCP config for: claude|cursor|windsurf|openclaw|generic
  --mcp-config <path>  Explicit MCP config path to write/merge
  --print-mcp          Print generated MCP config snippet
  --starter-pack <id>  Generate starter policies/prompts: ops|support|checkout
  --print-starter      Print starter pack sample requests
  --agent-only         Don't require local ANTHROPIC_API_KEY for setup success
  --handoff-file <p>   Write agent handoff bundle JSON to path
  --print-handoff      Print agent handoff bundle JSON
  --verify             Verify onboarding artifacts and readiness
  --verify-strict      Mark setup as failed if verification has warnings
  --yes, -y            Accept defaults without prompting (for CI)
  --json               Output results as JSON
  -h, --help           Show this help message

WHAT IT DOES:
  1. Creates ~/.stateset/ config directory
  2. Sets up your Anthropic API key
  3. Initializes a SQLite database
  4. Verifies everything works
  5. Optionally writes MCP config for your AI agent client
  6. Optionally installs a starter policy + prompt pack
  7. Optionally writes launch/check scripts and an agent handoff bundle
  8. Optionally verifies onboarding readiness

Already set up? This command is safe to re-run — it skips completed steps.
`;

/**
 * Build a portable MCP server config that works across MCP-compatible clients.
 * @param {string} dbPath
 * @param {string} serverName
 * @returns {{ mcpServers: Record<string, { command: string, args: string[], env: Record<string, string> }> }}
 */
function buildMcpConfig(dbPath, serverName) {
  return {
    mcpServers: {
      [serverName]: {
        command: 'npx',
        args: ['-y', '@stateset/cli@latest', 'stateset-mcp-events'],
        env: { DB_PATH: dbPath },
      },
    },
  };
}

/**
 * Resolve a default MCP config target path for a known client.
 * @param {string} target
 * @returns {string}
 */
function resolveDefaultMcpConfigPath(target) {
  switch (target) {
    case 'claude': {
      if (process.platform === 'darwin') {
        return path.join(
          os.homedir(),
          'Library',
          'Application Support',
          'Claude',
          'claude_desktop_config.json',
        );
      }
      if (process.platform === 'win32') {
        const appData = process.env.APPDATA || path.join(os.homedir(), 'AppData', 'Roaming');
        return path.join(appData, 'Claude', 'claude_desktop_config.json');
      }
      return path.join(os.homedir(), '.config', 'Claude', 'claude_desktop_config.json');
    }
    case 'cursor':
      return path.join(process.cwd(), '.cursor', 'mcp.json');
    case 'windsurf':
      return path.join(process.cwd(), '.windsurf', 'mcp.json');
    case 'openclaw':
      return path.join(process.cwd(), '.openclaw', 'mcp.json');
    case 'generic':
    default:
      return path.join(process.cwd(), 'mcp.json');
  }
}

/**
 * Merge or create an MCP config file.
 * @param {string} configPath
 * @param {string} serverName
 * @param {object} serverConfig
 */
function writeMcpConfig(configPath, serverName, serverConfig) {
  let current = {};
  if (fs.existsSync(configPath)) {
    try {
      const raw = fs.readFileSync(configPath, 'utf8');
      const parsed = JSON.parse(raw);
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        current = parsed;
      } else {
        throw new Error('existing file must be a JSON object');
      }
    } catch (err) {
      throw new Error(`failed to parse existing config: ${err.message}`);
    }
  }

  const merged = { ...current };
  const existingServers = merged.mcpServers;
  if (!existingServers || typeof existingServers !== 'object' || Array.isArray(existingServers)) {
    merged.mcpServers = {};
  }
  merged.mcpServers[serverName] = serverConfig;

  const dir = path.dirname(configPath);
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(configPath, `${JSON.stringify(merged, null, 2)}\n`, 'utf8');
}

/**
 * Resolve the local policy store path used by MCP policy loading.
 * @param {string} dbPath
 * @returns {string}
 */
function resolvePolicyStorePath(dbPath) {
  return path.join(path.dirname(path.resolve(dbPath)), '.stateset');
}

/**
 * Install a starter pack (policy files + agent prompt markdown).
 * @param {string} packId
 * @param {string} dbPath
 * @param {string} agentTarget
 * @returns {{ packId: string, storePath: string, policiesDir: string, policyFiles: string[], promptPath: string, sampleRequests: string[] }}
 */
function installStarterPack(packId, dbPath, agentTarget) {
  const pack = STARTER_PACKS[packId];
  if (!pack) {
    throw new Error(`unknown starter pack '${packId}'`);
  }

  const storePath = resolvePolicyStorePath(dbPath);
  const policiesDir = path.join(storePath, 'policies');
  const startersDir = path.join(storePath, 'agent-starters');
  fs.mkdirSync(policiesDir, { recursive: true });
  fs.mkdirSync(startersDir, { recursive: true });

  const generatedAt = new Date().toISOString();
  const policyFiles = [];

  for (const policySet of pack.policies) {
    const fileName = `starter-${packId}-${policySet.domain}.json`;
    const filePath = path.join(policiesDir, fileName);
    const payload = {
      ...policySet,
      metadata: {
        ...(policySet.metadata || {}),
        starterPack: packId,
        starterPackLabel: pack.label,
        generatedBy: 'stateset-setup',
        generatedAt,
      },
    };
    fs.writeFileSync(filePath, `${JSON.stringify(payload, null, 2)}\n`, 'utf8');
    policyFiles.push(filePath);
  }

  const promptPath = path.join(startersDir, `starter-${packId}.md`);
  const promptLines = [
    '# StateSet Agent Starter Prompt',
    '',
    `Pack: ${pack.label} (${packId})`,
    `Agent target: ${agentTarget || 'generic'}`,
    `Database: ${path.resolve(dbPath)}`,
    '',
    '## System Prompt',
    pack.prompt,
    '',
    '## Sample Requests',
    ...pack.sampleRequests.map((request) => `- ${request}`),
    '',
    '## Notes',
    '- Policies are loaded automatically by stateset-mcp-events from <db-dir>/.stateset/policies.',
    '- Adjust thresholds/reasons in the generated policy JSON files to fit your business.',
    '',
  ];
  fs.writeFileSync(promptPath, `${promptLines.join('\n')}`, 'utf8');

  return {
    packId,
    storePath,
    policiesDir,
    policyFiles,
    promptPath,
    promptText: pack.prompt,
    sampleRequests: [...pack.sampleRequests],
  };
}

/**
 * Generate local scripts that start and verify the MCP event gateway.
 * @param {string} dbPath
 * @param {string} storePath
 * @returns {{ startScriptPath: string, checkScriptPath: string, startCommand: string, checkCommand: string }}
 */
function writeAgentLaunchScripts(dbPath, storePath) {
  const startersDir = path.join(storePath, 'agent-starters');
  fs.mkdirSync(startersDir, { recursive: true });
  const resolvedDbPath = path.resolve(dbPath);

  const startScriptPath = path.join(startersDir, 'start-mcp.sh');
  const checkScriptPath = path.join(startersDir, 'check-mcp.sh');

  const startScript = `#!/usr/bin/env bash
set -euo pipefail

DB_PATH="\${1:-${resolvedDbPath}}"
HOST="\${STATESET_MCP_HOST:-127.0.0.1}"
PORT="\${STATESET_MCP_PORT:-8081}"

echo "[stateset] Starting MCP gateway on \${HOST}:\${PORT} with DB \${DB_PATH}"
npx -y @stateset/cli@latest stateset-mcp-events --db "\${DB_PATH}" --host "\${HOST}" --port "\${PORT}"
`;

  const checkScript = `#!/usr/bin/env bash
set -euo pipefail

HOST="\${STATESET_MCP_HOST:-127.0.0.1}"
PORT="\${STATESET_MCP_PORT:-8081}"
URL="http://\${HOST}:\${PORT}/health"

echo "[stateset] Checking \${URL}"
curl -fsS "\${URL}"
echo
`;

  fs.writeFileSync(startScriptPath, startScript, 'utf8');
  fs.writeFileSync(checkScriptPath, checkScript, 'utf8');

  if (process.platform !== 'win32') {
    fs.chmodSync(startScriptPath, 0o755);
    fs.chmodSync(checkScriptPath, 0o755);
  }

  return {
    startScriptPath,
    checkScriptPath,
    startCommand: `bash ${startScriptPath}`,
    checkCommand: `bash ${checkScriptPath}`,
  };
}

/**
 * Build a portable handoff bundle another agent/runtime can consume.
 * @param {{ dbPath: string, agentTarget: string, mcp: object|null, starter: object|null, launch: object|null }} options
 * @returns {object}
 */
function buildAgentHandoffBundle({ dbPath, agentTarget, mcp, starter, launch }) {
  const now = new Date().toISOString();
  return {
    schema: 'stateset.agentic-handoff.v1',
    generatedAt: now,
    agentTarget: agentTarget || 'generic',
    dbPath: path.resolve(dbPath),
    mcp: mcp
      ? {
          serverName: mcp.serverName,
          configPath: mcp.configPath,
          command: mcp.command,
          snippet: mcp.snippet,
        }
      : null,
    starterPack: starter
      ? {
          id: starter.packId,
          promptPath: starter.promptPath,
          prompt: starter.promptText,
          policyFiles: starter.policyFiles,
          sampleRequests: starter.sampleRequests,
        }
      : null,
    launch: launch
      ? {
          startScriptPath: launch.startScriptPath,
          checkScriptPath: launch.checkScriptPath,
          startCommand: launch.startCommand,
          checkCommand: launch.checkCommand,
        }
      : null,
    quickstart: {
      startMcpServer: `npx -y @stateset/cli@latest stateset-mcp-events --db ${path.resolve(dbPath)}`,
      inspectHealth: 'curl http://127.0.0.1:8081/health',
      firstRead:
        'Ask your agent to run: list orders created in the last 24 hours and summarize statuses.',
      firstWritePreview:
        'Ask your agent to preview: create a payment intent for order ORD-1001 for 149.99 USD.',
    },
  };
}

/**
 * Verify onboarding artifacts to provide a fast readiness signal.
 * @param {{ dbPath: string, mcpStep: any, starterStep: any, handoffStep: any }} options
 * @returns {{ status: 'ok'|'warnings'|'error', checks: Array<object>, recommendations: string[] }}
 */
function verifyOnboarding({ dbPath, mcpStep, starterStep, handoffStep }) {
  const checks = [];
  const recommendations = [];

  const addCheck = (name, status, details = null) => {
    checks.push({ name, status, details });
  };

  if (mcpStep?.status === 'configured') {
    try {
      if (!fs.existsSync(mcpStep.configPath)) {
        addCheck('mcp_config_file', 'error', `Missing MCP config at ${mcpStep.configPath}`);
      } else {
        const parsed = JSON.parse(fs.readFileSync(mcpStep.configPath, 'utf8'));
        const server = parsed?.mcpServers?.[mcpStep.serverName];
        if (!server) {
          addCheck(
            'mcp_server_entry',
            'error',
            `Missing mcpServers.${mcpStep.serverName} in MCP config`,
          );
        } else if (server.command !== 'npx') {
          addCheck(
            'mcp_server_command',
            'warnings',
            'Expected command=npx for portable onboarding',
          );
        } else if (!Array.isArray(server.args) || !server.args.includes('stateset-mcp-events')) {
          addCheck(
            'mcp_server_args',
            'error',
            'MCP server args do not include stateset-mcp-events',
          );
        } else {
          addCheck('mcp_server_entry', 'ok', 'MCP server config is present and valid');
        }

        const dbFromEnv = server?.env?.DB_PATH;
        if (typeof dbFromEnv === 'string' && dbFromEnv.length > 0) {
          addCheck('mcp_db_path', 'ok', dbFromEnv);
        } else {
          addCheck('mcp_db_path', 'warnings', 'DB_PATH not set in MCP server env');
          recommendations.push('Set DB_PATH in your MCP config to a stable store.db path.');
        }
      }
    } catch (err) {
      addCheck('mcp_config_parse', 'error', err.message);
    }
  } else {
    addCheck('mcp_onboarding', 'warnings', 'MCP onboarding not configured');
    recommendations.push('Run setup with --agent <target> to generate MCP configuration.');
  }

  if (starterStep?.status === 'configured') {
    const policyFiles = Array.isArray(starterStep.policyFiles) ? starterStep.policyFiles : [];
    if (policyFiles.length === 0) {
      addCheck('starter_policy_files', 'error', 'No policy files were generated');
    } else {
      let policyErrors = 0;
      for (const filePath of policyFiles) {
        try {
          if (!fs.existsSync(filePath)) {
            policyErrors += 1;
            continue;
          }
          const parsed = JSON.parse(fs.readFileSync(filePath, 'utf8'));
          if (!parsed?.id || !parsed?.domain || !Array.isArray(parsed?.rules)) {
            policyErrors += 1;
          }
        } catch {
          policyErrors += 1;
        }
      }
      if (policyErrors > 0) {
        addCheck(
          'starter_policy_files',
          'error',
          `${policyErrors} starter policy file(s) missing or invalid JSON`,
        );
      } else {
        addCheck(
          'starter_policy_files',
          'ok',
          `${policyFiles.length} starter policy files verified`,
        );
      }
    }

    if (starterStep.promptPath && fs.existsSync(starterStep.promptPath)) {
      addCheck('starter_prompt_file', 'ok', starterStep.promptPath);
    } else {
      addCheck('starter_prompt_file', 'error', 'Starter prompt file missing');
    }
  } else {
    addCheck('starter_pack', 'warnings', 'Starter pack not configured');
    recommendations.push('Add --starter-pack ops|support|checkout for opinionated guardrails.');
  }

  if (handoffStep?.status === 'configured') {
    try {
      if (!fs.existsSync(handoffStep.path)) {
        addCheck('handoff_file', 'error', `Missing handoff bundle at ${handoffStep.path}`);
      } else {
        const parsed = JSON.parse(fs.readFileSync(handoffStep.path, 'utf8'));
        if (parsed?.schema !== 'stateset.agentic-handoff.v1') {
          addCheck(
            'handoff_schema',
            'error',
            `Unexpected handoff schema: ${parsed?.schema || 'none'}`,
          );
        } else {
          addCheck('handoff_schema', 'ok', parsed.schema);
        }
        if (parsed?.launch?.startCommand && parsed?.launch?.checkCommand) {
          addCheck('handoff_launch_commands', 'ok', 'Launch commands are present');
        } else {
          addCheck('handoff_launch_commands', 'warnings', 'Handoff bundle missing launch commands');
          recommendations.push(
            'Re-run setup to regenerate launch scripts and handoff launch commands.',
          );
        }
      }
    } catch (err) {
      addCheck('handoff_parse', 'error', err.message);
    }
  } else {
    addCheck('handoff_bundle', 'warnings', 'Handoff bundle not generated');
    recommendations.push(
      'Use --print-handoff or --handoff-file to share setup state with other agents.',
    );
  }

  if (!fs.existsSync(path.resolve(dbPath))) {
    addCheck('database_file', 'warnings', `Database does not exist yet at ${path.resolve(dbPath)}`);
    recommendations.push('Initialize data with --demo or run stateset-init --demo.');
  } else {
    addCheck('database_file', 'ok', path.resolve(dbPath));
  }

  const hasErrors = checks.some((entry) => entry.status === 'error');
  const hasWarnings = checks.some((entry) => entry.status === 'warnings');
  return {
    status: hasErrors ? 'error' : hasWarnings ? 'warnings' : 'ok',
    checks,
    recommendations,
  };
}

/**
 * Build contextual next steps based on configured artifacts.
 * @param {{ dbPath: string, step5: any, step6: any, step7: any }} options
 * @returns {string[]}
 */
function buildNextSteps({ dbPath, step5, step6, step7 }) {
  const steps = [];
  const resolvedDbPath = path.resolve(dbPath);

  if (step5?.status === 'configured') {
    steps.push(
      `Start MCP gateway: npx -y @stateset/cli@latest stateset-mcp-events --db ${resolvedDbPath}`,
    );
  }

  if (step6?.status === 'configured') {
    const firstSample =
      Array.isArray(step6.sampleRequests) && step6.sampleRequests.length > 0
        ? step6.sampleRequests[0]
        : null;
    if (firstSample) {
      steps.push(`First agent request: ${firstSample}`);
    }
  }

  if (step7?.status === 'configured' && step7.path) {
    steps.push(`Share handoff bundle with another agent: ${step7.path}`);
    if (step7.launch?.startScriptPath) {
      steps.push(`Launch MCP gateway locally: bash ${step7.launch.startScriptPath}`);
    }
    if (step7.launch?.checkScriptPath) {
      steps.push(`Verify MCP gateway health: bash ${step7.launch.checkScriptPath}`);
    }
  }

  if (steps.length === 0) {
    steps.push('Try your first command: stateset "show me all customers"');
    steps.push('Run stateset --help to explore additional commands.');
  }

  return steps;
}

/**
 * Load the ~/.stateset/.env file as key-value pairs.
 * @returns {Record<string, string>}
 */
function loadEnvFile() {
  const env = {};
  if (!fs.existsSync(ENV_FILE)) return env;
  try {
    const content = fs.readFileSync(ENV_FILE, 'utf8');
    for (const line of content.split('\n')) {
      const trimmed = line.trim();
      if (!trimmed || trimmed.startsWith('#')) continue;
      const eqIdx = trimmed.indexOf('=');
      if (eqIdx === -1) continue;
      const key = trimmed.slice(0, eqIdx).trim();
      let val = trimmed.slice(eqIdx + 1).trim();
      // Strip surrounding quotes
      if (
        (val.startsWith('"') && val.endsWith('"')) ||
        (val.startsWith("'") && val.endsWith("'"))
      ) {
        val = val.slice(1, -1);
      }
      env[key] = val;
    }
  } catch (err) {
    console.warn(`Warning: Could not read ${ENV_FILE}: ${err.message}`);
  }
  return env;
}

/**
 * Save key-value pairs to ~/.stateset/.env
 * @param {Record<string, string>} env
 */
function saveEnvFile(env) {
  const lines = ['# Generated by stateset-setup'];
  for (const [key, value] of Object.entries(env)) {
    lines.push(`${key}="${value}"`);
  }
  fs.writeFileSync(ENV_FILE, lines.join('\n') + '\n', { mode: 0o600 });
}

async function main() {
  const { values } = parseArgs({
    options: {
      db: { type: 'string', default: './store.db' },
      demo: { type: 'boolean', default: false },
      quickstart: { type: 'boolean', default: false },
      agent: { type: 'string' },
      'mcp-config': { type: 'string' },
      'print-mcp': { type: 'boolean', default: false },
      'starter-pack': { type: 'string' },
      'print-starter': { type: 'boolean', default: false },
      'agent-only': { type: 'boolean', default: false },
      'handoff-file': { type: 'string' },
      'print-handoff': { type: 'boolean', default: false },
      verify: { type: 'boolean', default: false },
      'verify-strict': { type: 'boolean', default: false },
      yes: { type: 'boolean', short: 'y', default: false },
      json: { type: 'boolean', default: false },
      help: { type: 'boolean', short: 'h', default: false },
    },
    allowPositionals: false,
  });

  if (values.help) {
    console.log(HELP);
    return;
  }

  const interactive = !values.yes && process.stdin.isTTY;
  const results = { steps: [], success: true };
  const dbPath = path.resolve(values.db);
  const quickstartMode = Boolean(values.quickstart);
  const requestedAgent = typeof values.agent === 'string' ? values.agent.trim().toLowerCase() : '';
  const effectiveAgentTarget = requestedAgent || (quickstartMode ? QUICKSTART_DEFAULT_AGENT : '');
  const shouldOnboardMcp = Boolean(
    effectiveAgentTarget || values['mcp-config'] || values['print-mcp'] || quickstartMode,
  );
  const requestedStarterPack =
    typeof values['starter-pack'] === 'string' ? values['starter-pack'].trim().toLowerCase() : '';
  const effectiveStarterPack =
    requestedStarterPack ||
    (values['print-starter'] ? QUICKSTART_DEFAULT_PACK : '') ||
    (quickstartMode ? QUICKSTART_DEFAULT_PACK : '');
  const agentOnlyMode = Boolean(values['agent-only'] || shouldOnboardMcp || quickstartMode);
  const shouldVerify = Boolean(values.verify || quickstartMode);

  if (quickstartMode) {
    results.quickstart = {
      enabled: true,
      agent: effectiveAgentTarget,
      starterPack: effectiveStarterPack,
      agentOnlyMode: true,
      demo: true,
      verify: true,
    };
  }

  // Lazy-load @clack/prompts — only when interactive
  let ui = null;
  if (interactive && !values.json) {
    try {
      ui = await import('../src/ui.js');
    } catch {
      // Fallback to basic console if ui module unavailable
    }
  }

  if (!values.json) {
    if (ui) {
      ui.intro('StateSet iCommerce CLI - Setup');
    } else {
      console.log('\n  StateSet iCommerce CLI - Setup\n');
    }
    if (quickstartMode) {
      console.log(
        `  ${theme.success('✓')} Quickstart preset — agent=${effectiveAgentTarget}, starter-pack=${effectiveStarterPack}`,
      );
    }
  }

  // ── Step 1: Config directory ──────────────────────────────────────
  const step1 = { name: 'config_directory', status: 'skipped' };
  if (fs.existsSync(CONFIG_DIR) && fs.existsSync(PROFILES_DIR)) {
    step1.status = 'skipped';
    if (!values.json) console.log(`  ${theme.success('✓')} Config directory — already exists`);
  } else {
    fs.mkdirSync(CONFIG_DIR, { recursive: true });
    fs.mkdirSync(PROFILES_DIR, { recursive: true });
    step1.status = 'created';
    if (!values.json)
      console.log(`  ${theme.success('✓')} Config directory — created ~/.stateset/`);
  }
  results.steps.push(step1);

  // ── Step 2: API key ──────────────────────────────────────────────
  const step2 = { name: 'api_key', status: 'skipped' };
  const hasApiKey = process.env.ANTHROPIC_API_KEY || loadEnvFile().ANTHROPIC_API_KEY;

  if (hasApiKey) {
    step2.status = 'skipped';
    if (!values.json) console.log(`  ${theme.success('✓')} Anthropic API key — already configured`);
  } else if (interactive) {
    if (!values.json) {
      console.log(`\n  ${theme.warn('○')} Anthropic API key — not found`);
      console.log(theme.muted('    Get a key at: https://console.anthropic.com/settings/keys\n'));
    }

    let apiKey = '';
    if (ui) {
      apiKey = await ui.password('Enter your Anthropic API key (or press Enter to skip)');
    } else {
      // readline fallback for non-TTY
      const readline = await import('node:readline');
      const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
      apiKey = await new Promise((resolve) => {
        rl.question('  Enter your Anthropic API key (or press Enter to skip): ', (answer) => {
          rl.close();
          resolve(answer.trim());
        });
      });
    }

    if (apiKey) {
      if (!apiKey.startsWith('sk-ant-') && !values.json) {
        console.log(theme.warn("    Warning: Key doesn't start with 'sk-ant-'. Saving anyway."));
      }
      const envData = loadEnvFile();
      envData.ANTHROPIC_API_KEY = apiKey;
      saveEnvFile(envData);
      step2.status = 'configured';
      if (!values.json) console.log(`  ${theme.success('✓')} Saved to ~/.stateset/.env`);
    } else {
      step2.status = 'skipped';
      if (!values.json) {
        console.log(theme.muted('    Skipped. Set it later: stateset-config set-key anthropic'));
      }
    }
  } else if (agentOnlyMode) {
    step2.status = 'optional';
    if (!values.json) {
      console.log(
        `  ${theme.warn('⚠')} Anthropic API key — not configured (optional in agent-only mode)`,
      );
      console.log(
        theme.muted(
          '    MCP onboarding can proceed. Local `stateset "<request>"` commands require API key setup.',
        ),
      );
    }
  } else {
    step2.status = 'missing';
    results.success = false;
    if (!values.json) {
      console.log(`  ${theme.error('✗')} Anthropic API key — missing`);
      console.log(theme.muted('    Set it with: stateset-config set-key anthropic'));
    }
  }
  results.steps.push(step2);

  // ── Step 3: Database initialization ──────────────────────────────
  const step3 = { name: 'database', status: 'skipped' };
  const shouldSeedDemo = Boolean(values.demo || quickstartMode);

  if (fs.existsSync(dbPath)) {
    step3.status = 'skipped';
    step3.path = dbPath;
    if (!values.json) console.log(`  ${theme.success('✓')} Database — exists at ${dbPath}`);
  } else {
    let shouldInit = shouldSeedDemo;
    if (!shouldInit && interactive) {
      if (ui) {
        shouldInit = await ui.confirm('Initialize database with demo data?', {
          defaultValue: true,
        });
      } else {
        // Fallback: assume yes for non-interactive
        shouldInit = true;
      }
    }

    if (shouldInit) {
      // Ensure parent directory exists
      const dir = path.dirname(dbPath);
      if (!fs.existsSync(dir)) {
        fs.mkdirSync(dir, { recursive: true });
      }

      try {
        const { createRequire } = await import('node:module');
        const require = createRequire(import.meta.url);
        const mod = require('@stateset/embedded');
        const Commerce = mod.Commerce || mod.default?.Commerce || mod.default;

        if (Commerce) {
          const commerce = new Commerce(dbPath);
          const { seedDemoData } = await import('../src/seeds/demo.js');
          await seedDemoData(commerce);
          if (typeof commerce.close === 'function') commerce.close();
          step3.status = 'created';
          step3.path = dbPath;
          if (!values.json)
            console.log(`  ${theme.success('✓')} Database — initialized at ${dbPath}`);
        } else {
          throw new Error('Commerce constructor not found');
        }
      } catch (err) {
        step3.status = 'error';
        step3.error = err.message;
        if (!values.json) {
          console.log(`  ${theme.error('✗')} Database — failed: ${err.message}`);
          console.log(theme.muted('    Try: stateset-init --demo'));
        }
      }
    } else {
      step3.status = 'skipped';
      if (!values.json) {
        console.log(`  ${theme.muted('○')} Database — skipped`);
        console.log(theme.muted('    Initialize later: stateset-init --demo'));
      }
    }
  }
  results.steps.push(step3);

  // ── Step 4: Quick health check ───────────────────────────────────
  const step4 = { name: 'health_check', status: 'ok', checks: {} };

  // Node version check
  const nodeVersion = process.versions.node;
  const major = parseInt(nodeVersion.split('.')[0], 10);
  step4.checks.node = major >= 18 ? 'ok' : 'outdated';

  // API key reachability (quick check — just verify it's set)
  const apiKeyAvailable = process.env.ANTHROPIC_API_KEY || loadEnvFile().ANTHROPIC_API_KEY;
  step4.checks.apiKey = apiKeyAvailable ? 'ok' : 'missing';

  // Database file exists
  step4.checks.database = fs.existsSync(dbPath) ? 'ok' : 'missing';

  const allOk = Object.values(step4.checks).every((v) => v === 'ok');
  step4.status = allOk ? 'ok' : 'warnings';

  if (!values.json) {
    if (allOk) {
      console.log(`  ${theme.success('✓')} Health check — all good`);
    } else {
      console.log(`  ${theme.warn('⚠')} Health check — has warnings`);
    }
    if (step4.checks.node === 'outdated') {
      console.log(theme.warn(`    Node.js ${nodeVersion} is below v18. Please upgrade.`));
    }
  }
  results.steps.push(step4);

  // ── Step 5: Optional MCP onboarding for AI agents ───────────────
  const step5 = { name: 'agent_onboarding', status: 'skipped' };
  if (shouldOnboardMcp) {
    const serverName = 'stateset-commerce';
    const snippet = buildMcpConfig(dbPath, serverName);
    const serverConfig = snippet.mcpServers[serverName];

    if (effectiveAgentTarget && !VALID_AGENT_TARGETS.has(effectiveAgentTarget)) {
      step5.status = 'error';
      step5.error = `unknown agent target '${effectiveAgentTarget}'`;
      step5.validTargets = [...VALID_AGENT_TARGETS];
      results.success = false;
      if (!values.json) {
        console.log(
          `  ${theme.error('✗')} Agent onboarding — unknown target: ${effectiveAgentTarget}`,
        );
        console.log(
          theme.muted(
            '    Valid values: claude, cursor, windsurf, openclaw, generic (or use --mcp-config)',
          ),
        );
      }
    } else {
      const targetPath = path.resolve(
        values['mcp-config'] ||
          (effectiveAgentTarget ? resolveDefaultMcpConfigPath(effectiveAgentTarget) : './mcp.json'),
      );

      try {
        writeMcpConfig(targetPath, serverName, serverConfig);
        step5.status = 'configured';
        step5.agent = effectiveAgentTarget || 'custom';
        step5.configPath = targetPath;
        step5.serverName = serverName;
        step5.command = 'npx -y @stateset/cli@latest stateset-mcp-events';
        if (values['print-mcp'] || values.json) {
          step5.snippet = snippet;
        }

        if (!values.json) {
          console.log(
            `  ${theme.success('✓')} Agent onboarding — MCP config written to ${targetPath}`,
          );
          if (values['print-mcp']) {
            console.log(`\n${JSON.stringify(snippet, null, 2)}\n`);
          }
        }
      } catch (err) {
        step5.status = 'error';
        step5.error = err.message;
        results.success = false;
        if (!values.json) {
          console.log(`  ${theme.error('✗')} Agent onboarding — failed: ${err.message}`);
          console.log(theme.muted('    Tip: use --mcp-config <path> to write a fresh config'));
        }
      }
    }
  }
  results.steps.push(step5);

  // ── Step 6: Optional starter pack for agentic commerce ───────────
  const step6 = { name: 'starter_pack', status: 'skipped' };
  if (effectiveStarterPack) {
    if (!VALID_STARTER_PACKS.has(effectiveStarterPack)) {
      step6.status = 'error';
      step6.error = `unknown starter pack '${effectiveStarterPack}'`;
      step6.validPacks = [...VALID_STARTER_PACKS];
      results.success = false;
      if (!values.json) {
        console.log(`  ${theme.error('✗')} Starter pack — unknown pack: ${effectiveStarterPack}`);
        console.log(theme.muted('    Valid packs: ops, support, checkout'));
      }
    } else {
      try {
        const install = installStarterPack(
          effectiveStarterPack,
          dbPath,
          effectiveAgentTarget || 'generic',
        );
        step6.status = 'configured';
        step6.pack = install.packId;
        step6.storePath = install.storePath;
        step6.policiesDir = install.policiesDir;
        step6.policyFiles = install.policyFiles;
        step6.promptPath = install.promptPath;
        step6.sampleRequests = install.sampleRequests;
        if (values['print-starter'] || values.json) {
          step6.sampleRequests = install.sampleRequests;
        }

        if (!values.json) {
          console.log(
            `  ${theme.success('✓')} Starter pack — installed '${install.packId}' in ${install.storePath}`,
          );
          console.log(theme.muted(`    Prompt: ${install.promptPath}`));
          console.log(theme.muted(`    Policies: ${install.policiesDir}`));
          if (values['print-starter']) {
            console.log('\n  Sample agent requests:');
            for (const request of install.sampleRequests) {
              console.log(`    - ${request}`);
            }
            console.log('');
          }
        }
      } catch (err) {
        step6.status = 'error';
        step6.error = err.message;
        results.success = false;
        if (!values.json) {
          console.log(`  ${theme.error('✗')} Starter pack — failed: ${err.message}`);
        }
      }
    }
  }
  results.steps.push(step6);

  // ── Step 7: Optional agent handoff bundle ────────────────────────
  const step7 = { name: 'handoff_bundle', status: 'skipped' };
  const shouldWriteHandoff =
    values['print-handoff'] ||
    values['handoff-file'] ||
    step5.status === 'configured' ||
    step6.status === 'configured';

  if (shouldWriteHandoff) {
    const policyStorePath = resolvePolicyStorePath(dbPath);
    const defaultHandoffPath = path.join(policyStorePath, 'agent-starters', 'handoff.json');
    const handoffPath = path.resolve(values['handoff-file'] || defaultHandoffPath);
    let launchScripts = null;

    const mcp =
      step5.status === 'configured'
        ? {
            serverName: step5.serverName,
            configPath: step5.configPath,
            command: step5.command,
            snippet: step5.snippet || buildMcpConfig(dbPath, step5.serverName),
          }
        : null;

    const starter =
      step6.status === 'configured'
        ? {
            packId: step6.pack,
            promptPath: step6.promptPath,
            promptText: (() => {
              try {
                return fs.readFileSync(step6.promptPath, 'utf8');
              } catch {
                return null;
              }
            })(),
            policyFiles: step6.policyFiles || [],
            sampleRequests: step6.sampleRequests || [],
          }
        : null;

    try {
      launchScripts = writeAgentLaunchScripts(dbPath, policyStorePath);
      const handoffBundle = buildAgentHandoffBundle({
        dbPath,
        agentTarget: effectiveAgentTarget || 'generic',
        mcp,
        starter,
        launch: launchScripts,
      });

      fs.mkdirSync(path.dirname(handoffPath), { recursive: true });
      fs.writeFileSync(handoffPath, `${JSON.stringify(handoffBundle, null, 2)}\n`, 'utf8');

      step7.status = 'configured';
      step7.path = handoffPath;
      step7.launch = launchScripts;
      if (values['print-handoff'] || values.json) {
        step7.bundle = handoffBundle;
      }

      if (!values.json) {
        console.log(`  ${theme.success('✓')} Handoff bundle — written to ${handoffPath}`);
        if (values['print-handoff']) {
          console.log(`\n${JSON.stringify(handoffBundle, null, 2)}\n`);
        }
      }
    } catch (err) {
      step7.status = 'error';
      step7.error = err.message;
      results.success = false;
      if (!values.json) {
        console.log(`  ${theme.error('✗')} Handoff bundle — failed: ${err.message}`);
      }
    }
  }
  results.steps.push(step7);

  // ── Step 8: Optional onboarding verification ─────────────────────
  const step8 = { name: 'verification', status: 'skipped' };
  if (shouldVerify) {
    const verification = verifyOnboarding({
      dbPath,
      mcpStep: step5,
      starterStep: step6,
      handoffStep: step7,
    });
    step8.status = verification.status;
    step8.checks = verification.checks;
    step8.recommendations = verification.recommendations;

    if (verification.status === 'error') {
      results.success = false;
    } else if (verification.status === 'warnings' && values['verify-strict']) {
      results.success = false;
    }

    if (!values.json) {
      const icon =
        verification.status === 'ok'
          ? theme.success('✓')
          : verification.status === 'warnings'
            ? theme.warn('⚠')
            : theme.error('✗');
      console.log(`  ${icon} Verification — ${verification.status}`);
      if (verification.recommendations.length > 0) {
        for (const recommendation of verification.recommendations) {
          console.log(theme.muted(`    - ${recommendation}`));
        }
      }
    }
  }
  results.steps.push(step8);

  const nextSteps = buildNextSteps({
    dbPath,
    step5,
    step6,
    step7,
  });
  results.nextSteps = nextSteps;

  // ── Summary ──────────────────────────────────────────────────────
  if (values.json) {
    console.log(JSON.stringify(results, null, 2));
  } else if (ui) {
    if (results.success) {
      ui.outro('Setup complete! Try: stateset "show me all customers"');
    } else {
      ui.note('Some steps need attention. See warnings above.', 'Setup finished');
    }
  } else {
    console.log('\n  Setup complete!\n');
    if (results.success) {
      console.log('  Next steps:');
      for (const step of nextSteps) {
        console.log(`    - ${step}`);
      }
      console.log('');
    } else {
      console.log('  Some steps need attention. See warnings above.\n');
    }
  }
}

runMain('stateset-setup', main);
