/**
 * Agent Runtime MCP Tools
 *
 * Tools for creating, managing, and orchestrating autonomous AI agent
 * runtimes. Agents can negotiate, pay, discover services, and compose
 * into supply chains — all through natural language via MCP.
 */

import { z } from 'zod';
import crypto from 'node:crypto';

// Session-scoped runtime registry (keyed by name)
const runtimes = new Map();

/**
 * Resolve a runtime by name or agentId.
 */
function getRuntime(nameOrId) {
  if (runtimes.has(nameOrId)) return runtimes.get(nameOrId);
  for (const rt of runtimes.values()) {
    if (rt.agentId === nameOrId) return rt;
  }
  return null;
}

const _STRATEGY_NAMES = [
  'always-accept',
  'budget-gated',
  'negotiator',
  'best-of-n',
  'reputation-aware',
  'dynamic-pricing',
];

async function resolveStrategy(strategyName, options = {}) {
  const {
    createAlwaysAcceptStrategy,
    createBudgetGatedStrategy,
    createNegotiatorStrategy,
    createBestOfNStrategy,
  } = await import('../a2a/strategies.js');

  switch (strategyName) {
    case 'always-accept':
      return createAlwaysAcceptStrategy(options);
    case 'budget-gated':
      return createBudgetGatedStrategy(options);
    case 'negotiator':
      return createNegotiatorStrategy(options);
    case 'best-of-n':
      return createBestOfNStrategy(options);
    case 'reputation-aware': {
      // Dynamically import — may not exist yet during Phase 1
      try {
        const mod = await import('../a2a/strategies.js');
        if (mod.createReputationAwareStrategy) {
          return mod.createReputationAwareStrategy(options);
        }
      } catch {
        /* fallback */
      }
      return createBudgetGatedStrategy(options);
    }
    case 'dynamic-pricing': {
      try {
        const mod = await import('../a2a/strategies.js');
        if (mod.createDynamicPricingStrategy) {
          return mod.createDynamicPricingStrategy(options);
        }
      } catch {
        /* fallback */
      }
      return createBudgetGatedStrategy(options);
    }
    default:
      return createAlwaysAcceptStrategy();
  }
}

export const agentRuntimeTools = [
  // ==========================================================================
  // Lifecycle
  // ==========================================================================
  {
    name: 'agent_create_runtime',
    description:
      'Create an autonomous AI agent runtime with a wallet, negotiation strategy, and budget. The agent can then register services, discover other agents, negotiate quotes, and make payments autonomously.',
    inputSchema: {
      name: z.string().min(1).max(100).describe('Agent name (e.g., "DataForge AI")'),
      strategy: z
        .enum([
          'always-accept',
          'budget-gated',
          'negotiator',
          'best-of-n',
          'reputation-aware',
          'dynamic-pricing',
        ])
        .optional()
        .default('budget-gated')
        .describe('Negotiation strategy'),
      strategyOptions: z
        .record(z.string(), z.any())
        .optional()
        .default({})
        .describe('Strategy configuration (e.g., { markup: 1.5, basePrice: 50 })'),
      budgetDaily: z.number().positive().optional().describe('Maximum daily spend in USDC'),
      budgetMonthly: z.number().positive().optional().describe('Maximum monthly spend in USDC'),
      budgetPerTransaction: z
        .number()
        .positive()
        .optional()
        .describe('Maximum per-transaction spend'),
      startingBalance: z.number().nonnegative().optional().describe('Starting balance in USDC'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Creating agent runtimes requires --apply flag.',
          wouldCreate: { name: params.name, strategy: params.strategy },
        };
      }

      if (runtimes.has(params.name)) {
        return {
          success: false,
          error: `Agent "${params.name}" already exists. Use agent_destroy_runtime first.`,
        };
      }

      const { createAgentRuntime } = await import('../a2a/agent-runtime.js');

      const walletAddress = '0x' + crypto.randomBytes(20).toString('hex');
      const signingKey = {
        privateKey: crypto.randomBytes(32).toString('hex'),
        publicKey: crypto.randomBytes(32).toString('hex'),
      };

      const strategy = await resolveStrategy(params.strategy, params.strategyOptions);

      const rt = createAgentRuntime({
        name: params.name,
        walletAddress,
        signingKey,
        commerce,
        budget: {
          daily: params.budgetDaily ?? Infinity,
          monthly: params.budgetMonthly ?? Infinity,
          perTransaction: params.budgetPerTransaction ?? Infinity,
          startingBalance: params.startingBalance ?? null,
        },
        strategy,
        logger: () => {},
      });

      runtimes.set(params.name, rt);

      return {
        success: true,
        message: `Agent "${params.name}" created with ${params.strategy} strategy`,
        agent: {
          name: rt.name,
          agentId: rt.agentId,
          walletAddress: rt.walletAddress,
          strategy: params.strategy,
          budget: rt.getBudget(),
        },
      };
    },
  },

  {
    name: 'agent_destroy_runtime',
    description: 'Destroy an agent runtime and clean up resources.',
    inputSchema: {
      name: z.string().min(1).describe('Agent name or ID to destroy'),
    },
    permission: 'delete',
    handler: async ({ params }) => {
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      rt.destroy();
      runtimes.delete(rt.name);
      return { success: true, message: `Agent "${rt.name}" destroyed.` };
    },
  },

  {
    name: 'agent_list_runtimes',
    description: 'List all active agent runtimes in this session.',
    inputSchema: {},
    permission: 'read',
    handler: async () => {
      const agents = [];
      for (const rt of runtimes.values()) {
        agents.push({
          name: rt.name,
          agentId: rt.agentId,
          walletAddress: rt.walletAddress,
          strategy: rt.getStrategy().name,
          running: rt.isRunning(),
          budget: rt.getBudget(),
        });
      }
      return { success: true, agents, count: agents.length };
    },
  },

  {
    name: 'agent_get_status',
    description:
      'Get detailed status of an agent runtime including budget, strategy, and registered services.',
    inputSchema: {
      name: z.string().min(1).describe('Agent name or ID'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      return {
        success: true,
        agent: {
          name: rt.name,
          agentId: rt.agentId,
          walletAddress: rt.walletAddress,
          strategy: rt.getStrategy().name,
          running: rt.isRunning(),
          budget: rt.getBudget(),
          services: rt.listMyServices(),
        },
      };
    },
  },

  // ==========================================================================
  // Strategy
  // ==========================================================================
  {
    name: 'agent_set_strategy',
    description:
      "Change an agent's negotiation strategy. Available: always-accept, budget-gated, negotiator, best-of-n, reputation-aware.",
    inputSchema: {
      name: z.string().min(1).describe('Agent name or ID'),
      strategy: z
        .enum([
          'always-accept',
          'budget-gated',
          'negotiator',
          'best-of-n',
          'reputation-aware',
          'dynamic-pricing',
        ])
        .describe('New strategy to use'),
      options: z
        .record(z.string(), z.any())
        .optional()
        .default({})
        .describe('Strategy options (e.g., { targetDiscount: 0.2, maxRounds: 3 })'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return { success: false, error: 'Changing strategy requires --apply flag.' };
      }
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      const strategy = await resolveStrategy(params.strategy, params.options);
      rt.setStrategy(strategy);
      return {
        success: true,
        message: `Strategy changed to "${params.strategy}" for agent "${rt.name}"`,
      };
    },
  },

  {
    name: 'agent_get_budget',
    description: 'Get the current budget status of an agent: spent today, remaining, limits.',
    inputSchema: {
      name: z.string().min(1).describe('Agent name or ID'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      return { success: true, budget: rt.getBudget() };
    },
  },

  // ==========================================================================
  // Service Loop
  // ==========================================================================
  {
    name: 'agent_tick',
    description:
      'Process one autonomous cycle for an agent. The agent will respond to pending quotes, evaluate received offers, and auto-fulfill accepted deals.',
    inputSchema: {
      name: z.string().min(1).describe('Agent name or ID'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return { success: false, error: 'Agent tick requires --apply flag.' };
      }
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      const processed = await rt.tick();
      return {
        success: true,
        message: `Agent "${rt.name}" processed ${processed} item(s)`,
        processed,
      };
    },
  },

  {
    name: 'agent_start_loop',
    description:
      "Start the agent's autonomous polling loop. The agent will continuously process incoming work.",
    inputSchema: {
      name: z.string().min(1).describe('Agent name or ID'),
      intervalMs: z
        .number()
        .int()
        .positive()
        .optional()
        .default(5000)
        .describe('Poll interval in milliseconds'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return { success: false, error: 'Starting agent loop requires --apply flag.' };
      }
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      if (rt.isRunning()) {
        return { success: false, error: `Agent "${rt.name}" is already running.` };
      }
      rt.start();
      return { success: true, message: `Agent "${rt.name}" loop started.` };
    },
  },

  {
    name: 'agent_stop_loop',
    description: "Stop the agent's autonomous polling loop.",
    inputSchema: {
      name: z.string().min(1).describe('Agent name or ID'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return { success: false, error: 'Stopping agent loop requires --apply flag.' };
      }
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      rt.stop();
      return { success: true, message: `Agent "${rt.name}" loop stopped.` };
    },
  },

  // ==========================================================================
  // Services
  // ==========================================================================
  {
    name: 'agent_register_service',
    description:
      'Register a service in the A2A marketplace so other agents can discover and purchase it.',
    inputSchema: {
      name: z.string().min(1).describe('Agent name or ID'),
      serviceName: z.string().min(1).max(200).describe('Service name (e.g., "Sentiment Analysis")'),
      description: z.string().optional().describe('Service description'),
      category: z
        .string()
        .min(1)
        .describe('Service category (e.g., "analytics", "data-collection")'),
      pricingModel: z.enum(['quote', 'fixed', 'subscription']).optional().default('quote'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return { success: false, error: 'Registering services requires --apply flag.' };
      }
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      const svc = rt.registerService({
        name: params.serviceName,
        description: params.description || '',
        category: params.category,
        pricingModel: params.pricingModel,
      });
      return {
        success: true,
        message: `Service "${params.serviceName}" registered in category "${params.category}"`,
        service: svc,
      };
    },
  },

  {
    name: 'agent_discover_services',
    description: 'Search the A2A marketplace for services by category or capability.',
    inputSchema: {
      name: z.string().min(1).describe('Agent name or ID performing the search'),
      category: z.string().optional().describe('Filter by service category'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      const filter = {};
      if (params.category) filter.category = params.category;
      const services = rt.discoverServices(filter);
      return { success: true, services, count: services.length };
    },
  },

  // ==========================================================================
  // Advanced Commerce
  // ==========================================================================
  {
    name: 'agent_create_escrow_deal',
    description:
      'Create an escrow-backed transaction between agents. Funds are held until conditions are met (seller fulfilled, buyer confirmed, time lock, or milestone).',
    inputSchema: {
      buyerName: z.string().min(1).describe('Buyer agent name or ID'),
      sellerAddress: z.string().min(1).describe('Seller wallet address (0x...)'),
      amount: z.number().positive().describe('Amount in USDC'),
      conditions: z
        .array(
          z.object({
            type: z.enum(['seller_fulfilled', 'buyer_confirmed', 'time_lock', 'milestone']),
            quoteId: z.string().optional(),
            deadline: z.string().optional(),
            description: z.string().optional(),
          }),
        )
        .min(1)
        .describe('Release conditions'),
      expiresInHours: z.number().positive().optional().default(72),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return { success: false, error: 'Creating escrow deals requires --apply flag.' };
      }
      const rt = getRuntime(params.buyerName);
      if (!rt) {
        return { success: false, error: `Agent "${params.buyerName}" not found.` };
      }
      if (!rt.canAfford(params.amount)) {
        return { success: false, error: `Agent "${rt.name}" cannot afford $${params.amount}.` };
      }
      if (typeof rt.createEscrowDeal === 'function') {
        const result = await rt.createEscrowDeal(params);
        return { success: true, ...result };
      }
      // Fallback: use underlying A2A service directly
      const result = await rt.a2a.createConditionalPayment({
        sellerAddress: params.sellerAddress,
        amount: params.amount,
        conditions: params.conditions,
        expiresInHours: params.expiresInHours,
      });
      rt.recordSpend(params.amount, { type: 'escrow', escrowId: result.escrow?.id });
      return { success: true, message: `Escrow created for $${params.amount} USDC`, ...result };
    },
  },

  {
    name: 'agent_subscribe_to_service',
    description:
      "Subscribe an agent to another agent's recurring service (e.g., daily data feed, monthly analytics).",
    inputSchema: {
      subscriberName: z.string().min(1).describe('Subscriber agent name'),
      providerAddress: z.string().min(1).describe('Service provider wallet address'),
      planName: z.string().min(1).describe('Subscription plan name'),
      amount: z.number().positive().describe('Recurring amount in USDC'),
      interval: z
        .enum(['weekly', 'biweekly', 'monthly', 'quarterly', 'annual'])
        .optional()
        .default('monthly'),
      trialDays: z.number().int().nonnegative().optional().default(0),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return { success: false, error: 'Creating subscriptions requires --apply flag.' };
      }
      const rt = getRuntime(params.subscriberName);
      if (!rt) {
        return { success: false, error: `Agent "${params.subscriberName}" not found.` };
      }
      if (typeof rt.subscribeTo === 'function') {
        const result = await rt.subscribeTo(params);
        return { success: true, ...result };
      }
      // Fallback: use subscription service directly
      const { createA2ASubscriptionService } = await import('../a2a/subscriptions.js');
      const subSvc = createA2ASubscriptionService(commerce.a2a());
      const sub = await subSvc.createSubscription({
        subscriberAddress: rt.walletAddress,
        providerAddress: params.providerAddress,
        planName: params.planName,
        amount: params.amount,
        asset: 'USDC',
        network: 'set_chain',
        billingInterval: params.interval,
        trialDays: params.trialDays,
      });
      return { success: true, message: `Subscribed to "${params.planName}"`, subscription: sub };
    },
  },

  {
    name: 'agent_rate_counterparty',
    description: 'Rate another agent after a transaction. Builds reputation in the marketplace.',
    inputSchema: {
      raterName: z.string().min(1).describe('Agent giving the rating'),
      ratedAddress: z.string().min(1).describe('Wallet address of agent being rated'),
      score: z.number().int().min(1).max(5).describe('Overall score (1-5)'),
      transactionId: z.string().optional().describe('Related quote or payment ID'),
      comment: z.string().max(500).optional().describe('Feedback comment'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return { success: false, error: 'Rating agents requires --apply flag.' };
      }
      const rt = getRuntime(params.raterName);
      if (!rt) {
        return { success: false, error: `Agent "${params.raterName}" not found.` };
      }
      if (typeof rt.rateCounterparty === 'function') {
        const result = await rt.rateCounterparty(params);
        return { success: true, ...result };
      }
      // Fallback: use reputation service directly
      const { createReputationService } = await import('../a2a/reputation.js');
      const repSvc = createReputationService(commerce.a2a());
      const feedback = await repSvc.rateAgent({
        agentAddress: params.ratedAddress,
        reviewerAddress: rt.walletAddress,
        transactionType: 'quote',
        transactionId: params.transactionId || crypto.randomUUID(),
        score: params.score,
        comment: params.comment || '',
      });
      return { success: true, message: `Rated agent ${params.score}/5`, feedback };
    },
  },

  {
    name: 'agent_get_reputation',
    description: "Get an agent's reputation score, trust tier, and feedback summary.",
    inputSchema: {
      address: z.string().min(1).describe('Agent wallet address to look up'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const { createReputationService } = await import('../a2a/reputation.js');
      const a2aProxy = commerce.a2a();
      const repSvc = createReputationService(a2aProxy);
      const result = await repSvc.getReputation(params.address);
      return { success: true, reputation: result?.reputation || result };
    },
  },

  {
    name: 'agent_create_split_deal',
    description:
      'Create a multi-party payment split. Revenue from a deal is distributed to multiple agents.',
    inputSchema: {
      payerName: z.string().min(1).describe('Payer agent name'),
      totalAmount: z.number().positive().describe('Total amount to split'),
      recipients: z
        .array(
          z.object({
            address: z.string().min(1).describe('Recipient wallet address'),
            percentage: z.number().min(0).max(100).describe('Percentage share'),
          }),
        )
        .min(2)
        .describe('Recipients with percentage shares (must sum to 100)'),
      memo: z.string().optional().describe('Split payment memo'),
    },
    permission: 'write',
    handler: async ({ commerce, params, allowApply }) => {
      if (!allowApply) {
        return { success: false, error: 'Creating split deals requires --apply flag.' };
      }
      const rt = getRuntime(params.payerName);
      if (!rt) {
        return { success: false, error: `Agent "${params.payerName}" not found.` };
      }
      if (!rt.canAfford(params.totalAmount)) {
        return {
          success: false,
          error: `Agent "${rt.name}" cannot afford $${params.totalAmount}.`,
        };
      }
      // Map schema field 'percentage' → 'percent' for splits service
      const mappedRecipients = params.recipients.map((r) => ({
        address: r.address,
        percent: r.percentage,
      }));
      if (typeof rt.createSplitDeal === 'function') {
        const result = await rt.createSplitDeal({ ...params, recipients: mappedRecipients });
        return { success: true, ...result };
      }
      // Fallback: use splits service directly
      const { createSplitPaymentService } = await import('../a2a/splits.js');
      const splitSvc = createSplitPaymentService(commerce.a2a());
      const split = await splitSvc.createSplitPayment({
        payerAddress: rt.walletAddress,
        totalAmount: params.totalAmount,
        asset: 'USDC',
        network: 'set_chain',
        splitType: 'percentage',
        recipients: mappedRecipients,
        memo: params.memo || '',
      });
      return { success: true, message: `Split payment created for $${params.totalAmount}`, split };
    },
  },

  {
    name: 'agent_get_event_history',
    description: "Get an agent's event stream history — all quotes, payments, and state changes.",
    inputSchema: {
      name: z.string().min(1).describe('Agent name or ID'),
      limit: z.number().int().positive().optional().default(50).describe('Max events to return'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      const a2a = commerce.a2a();
      const events =
        typeof a2a.listEventLog === 'function'
          ? a2a.listEventLog({ agent_address: rt.walletAddress, limit: params.limit })
          : [];
      return { success: true, events, count: events.length };
    },
  },

  // ==========================================================================
  // On-Chain Settlement
  // ==========================================================================
  {
    name: 'agent_enable_settlement',
    description:
      'Enable on-chain stablecoin settlement for an agent runtime. ' +
      'The agent will settle payments on the specified blockchain using derived wallets.',
    inputSchema: {
      type: 'object',
      properties: {
        name: { type: 'string', description: 'Agent name' },
        chainId: {
          type: 'string',
          description: 'Target blockchain (base, solana, set_chain, ethereum, arbitrum)',
          default: 'base',
        },
        simulate: {
          type: 'boolean',
          description: 'Simulate without broadcasting (default: true)',
          default: true,
        },
        tokenSymbol: {
          type: 'string',
          description: 'Override token (default: chain stablecoin)',
        },
      },
      required: ['name'],
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return {
          success: false,
          error: 'Enabling settlement requires --apply flag.',
          wouldEnable: {
            name: params.name,
            chainId: params.chainId || 'base',
            simulate: params.simulate !== false,
          },
        };
      }

      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }

      const { createSettlementService } = await import('../a2a/settlement.js');

      const settlement = createSettlementService({
        chainId: params.chainId || 'base',
        agentId: rt.agentId,
        simulate: params.simulate !== false,
        tokenSymbol: params.tokenSymbol || undefined,
        configDir: '.stateset',
      });

      // Attach to runtime
      rt.settlement = settlement;

      let address = null;
      try {
        address = await settlement.getAddress();
      } catch (_err) {
        // Address derivation may fail without key material — non-fatal
      }

      return {
        success: true,
        message: `Settlement enabled for "${params.name}" on ${settlement.chainId}${settlement.isSimulation ? ' (simulation)' : ''}`,
        settlement: {
          chainId: settlement.chainId,
          walletAddress: address,
          simulate: settlement.isSimulation,
          tokenSymbol: params.tokenSymbol || 'default',
        },
      };
    },
  },
  {
    name: 'agent_get_chain_balance',
    description:
      'Get the on-chain stablecoin balance for an agent runtime with settlement enabled.',
    inputSchema: {
      type: 'object',
      properties: {
        name: { type: 'string', description: 'Agent name' },
      },
      required: ['name'],
    },
    permission: 'read',
    handler: async ({ params }) => {
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      if (!rt.settlement) {
        return {
          success: false,
          error: `Agent "${params.name}" has no settlement service. Use agent_enable_settlement first.`,
        };
      }

      try {
        const balance = await rt.settlement.getBalance();
        const address = await rt.settlement.getAddress();

        return {
          success: true,
          agent: params.name,
          chainId: rt.settlement.chainId,
          walletAddress: address,
          balance: balance.balance,
          symbol: balance.symbol,
        };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ==========================================================================
  // Marketplace (RFQ)
  // ==========================================================================
  {
    name: 'agent_broadcast_rfq',
    description:
      'Broadcast a Request for Quotation (RFQ) to multiple sellers in the marketplace. Sellers matching the filter will receive quote requests.',
    inputSchema: {
      name: z.string().min(1).describe('Buyer agent name or ID'),
      items: z
        .array(
          z.object({
            description: z.string().min(1),
            quantity: z.number().int().positive().optional().default(1),
          }),
        )
        .min(1)
        .describe('Items to request quotes for'),
      sellerFilter: z
        .string()
        .optional()
        .describe('Category filter for sellers (e.g., "analytics")'),
      maxResponses: z
        .number()
        .int()
        .positive()
        .optional()
        .default(10)
        .describe('Max seller responses'),
      deadlineMinutes: z
        .number()
        .int()
        .positive()
        .optional()
        .default(60)
        .describe('RFQ deadline in minutes'),
      scoringCriteria: z
        .enum(['cheapest', 'best_value', 'fastest'])
        .optional()
        .default('cheapest')
        .describe('How to rank responses'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return { success: false, error: 'Broadcasting RFQs requires --apply flag.' };
      }
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      if (typeof rt.broadcastRFQ !== 'function') {
        return { success: false, error: 'Agent runtime does not support broadcastRFQ.' };
      }
      const result = await rt.broadcastRFQ(params);
      return {
        success: true,
        message: `RFQ broadcast to ${result.sellersContacted} sellers`,
        ...result,
      };
    },
  },

  {
    name: 'agent_collect_rfq_responses',
    description: 'Collect and score all responses for an RFQ broadcast.',
    inputSchema: {
      name: z.string().min(1).describe('Buyer agent name or ID'),
      rfqId: z.string().min(1).describe('RFQ ID to collect responses for'),
    },
    permission: 'read',
    handler: async ({ params }) => {
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      if (typeof rt.collectRFQResponses !== 'function') {
        return { success: false, error: 'Agent runtime does not support collectRFQResponses.' };
      }
      const result = rt.collectRFQResponses(params.rfqId);
      return { success: true, ...result };
    },
  },

  {
    name: 'agent_award_rfq',
    description:
      "Award an RFQ to the best-scored (or specified) seller. Accepts the winner's quote and declines all others.",
    inputSchema: {
      name: z.string().min(1).describe('Buyer agent name or ID'),
      rfqId: z.string().min(1).describe('RFQ ID to award'),
      winnerId: z.string().optional().describe('Force a specific response/quote as winner'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return { success: false, error: 'Awarding RFQs requires --apply flag.' };
      }
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      if (typeof rt.awardRFQ !== 'function') {
        return { success: false, error: 'Agent runtime does not support awardRFQ.' };
      }
      const result = await rt.awardRFQ(params.rfqId, params.winnerId);
      return { success: true, message: `RFQ awarded to ${result.winnerAddress}`, ...result };
    },
  },

  {
    name: 'agent_get_marketplace_metrics',
    description:
      'Get marketplace performance metrics for a registered service (success rate, response time, etc.).',
    inputSchema: {
      serviceId: z.string().min(1).describe('Service ID to get metrics for'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      try {
        const { createMarketplaceService } = await import('../a2a/marketplace.js');
        const mktSvc = createMarketplaceService(commerce.a2a(), null);
        const metrics = mktSvc.getServiceMetrics(params.serviceId);
        return { success: true, metrics };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ==========================================================================
  // SLA
  // ==========================================================================
  {
    name: 'agent_attach_sla',
    description:
      'Attach a Service Level Agreement to a registered service. Defines performance thresholds and penalties.',
    inputSchema: {
      name: z.string().min(1).describe('Agent name or ID (service owner)'),
      serviceId: z.string().min(1).describe('Service ID to attach SLA to'),
      responseTimeMs: z
        .number()
        .int()
        .positive()
        .optional()
        .describe('Max response time in milliseconds'),
      uptimePercent: z.number().min(0).max(100).optional().describe('Minimum uptime percentage'),
      qualityMinScore: z.number().min(1).max(5).optional().describe('Minimum quality score (1-5)'),
      penaltyPercent: z
        .number()
        .min(0)
        .max(100)
        .optional()
        .default(5)
        .describe('Penalty as % of transaction value'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return { success: false, error: 'Attaching SLAs requires --apply flag.' };
      }
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      if (typeof rt.attachSLA !== 'function') {
        return { success: false, error: 'Agent runtime does not support attachSLA.' };
      }
      const result = rt.attachSLA(params);
      return { success: true, message: `SLA attached to service ${params.serviceId}`, ...result };
    },
  },

  {
    name: 'agent_check_sla_compliance',
    description: 'Check if a service is meeting its SLA commitments.',
    inputSchema: {
      serviceId: z.string().min(1).describe('Service ID to check compliance for'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      try {
        const { createSLAService } = await import('../a2a/sla.js');
        const slaSvc = createSLAService(commerce.a2a());
        const compliance = slaSvc.checkCompliance(params.serviceId);
        return { success: true, ...compliance };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  // ==========================================================================
  // Workflows
  // ==========================================================================
  {
    name: 'agent_create_workflow',
    description:
      'Create a multi-agent workflow with DAG-based step dependencies. Steps execute in topological order.',
    inputSchema: {
      name: z.string().min(1).describe('Agent name or ID (workflow owner)'),
      workflowName: z.string().min(1).max(200).describe('Workflow name'),
      steps: z
        .array(
          z.object({
            name: z.string().min(1).describe('Step name (unique within workflow)'),
            type: z
              .enum(['quote_request', 'payment', 'condition_check', 'transform'])
              .optional()
              .default('quote_request'),
            agentAddress: z.string().optional().describe('Target agent address for this step'),
            params: z.record(z.string(), z.any()).optional().describe('Step parameters'),
            dependsOn: z.array(z.string()).optional().describe('Step names this step depends on'),
          }),
        )
        .min(1)
        .describe('Workflow step definitions'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return { success: false, error: 'Creating workflows requires --apply flag.' };
      }
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      if (typeof rt.createWorkflow !== 'function') {
        return { success: false, error: 'Agent runtime does not support createWorkflow.' };
      }
      const result = rt.createWorkflow({ name: params.workflowName, steps: params.steps });
      return {
        success: true,
        message: `Workflow "${params.workflowName}" created with ${params.steps.length} steps`,
        ...result,
      };
    },
  },

  {
    name: 'agent_execute_workflow',
    description:
      'Execute a workflow. Steps run in dependency order with parallel fan-out where possible.',
    inputSchema: {
      name: z.string().min(1).describe('Agent name or ID'),
      workflowId: z.string().min(1).describe('Workflow ID to execute'),
      context: z.record(z.string(), z.any()).optional().describe('Initial context passed to steps'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return { success: false, error: 'Executing workflows requires --apply flag.' };
      }
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      if (typeof rt.executeWorkflow !== 'function') {
        return { success: false, error: 'Agent runtime does not support executeWorkflow.' };
      }
      const result = await rt.executeWorkflow(params.workflowId, params.context);
      return { success: true, ...result };
    },
  },

  {
    name: 'agent_get_workflow_status',
    description: 'Get the current status and progress of a workflow.',
    inputSchema: {
      workflowId: z.string().min(1).describe('Workflow ID'),
    },
    permission: 'read',
    handler: async ({ commerce, params }) => {
      try {
        const { createWorkflowService } = await import('../a2a/workflows.js');
        const wfSvc = createWorkflowService(commerce.a2a(), null);
        const status = wfSvc.getWorkflowStatus(params.workflowId);
        return { success: true, ...status };
      } catch (err) {
        return { success: false, error: err.message };
      }
    },
  },

  {
    name: 'agent_set_dynamic_pricing',
    description:
      'Configure dynamic pricing for an agent. Sets volume breaks, reputation tiers, peak hours, and loyalty tiers.',
    inputSchema: {
      name: z.string().min(1).describe('Agent name or ID'),
      volumeBreaks: z
        .array(
          z.object({
            minQty: z.number().int().positive(),
            discount: z.number().min(0).max(1),
          }),
        )
        .optional()
        .describe('Volume discount tiers'),
      reputationTiers: z
        .record(z.string(), z.number())
        .optional()
        .describe('Trust tier → markup adjustment'),
      peakHours: z
        .object({
          start: z.number().int().min(0).max(23),
          end: z.number().int().min(0).max(23),
          surgeMultiplier: z.number().positive(),
        })
        .optional()
        .describe('Peak hours surge pricing'),
      loyaltyTiers: z
        .record(z.string(), z.number())
        .optional()
        .describe('Transaction count → discount'),
    },
    permission: 'write',
    handler: async ({ params, allowApply }) => {
      if (!allowApply) {
        return { success: false, error: 'Setting dynamic pricing requires --apply flag.' };
      }
      const rt = getRuntime(params.name);
      if (!rt) {
        return { success: false, error: `Agent "${params.name}" not found.` };
      }
      const pricingConfig = {};
      if (params.volumeBreaks) pricingConfig.volumeBreaks = params.volumeBreaks;
      if (params.reputationTiers) pricingConfig.reputationTiers = params.reputationTiers;
      if (params.peakHours) pricingConfig.peakHours = params.peakHours;
      if (params.loyaltyTiers) {
        // Convert string keys to numbers
        pricingConfig.loyaltyTiers = {};
        for (const [k, v] of Object.entries(params.loyaltyTiers)) {
          pricingConfig.loyaltyTiers[Number(k)] = v;
        }
      }
      const { createDynamicPricingStrategy } = await import('../a2a/strategies.js');
      const strategy = createDynamicPricingStrategy(pricingConfig);
      rt.setStrategy(strategy);
      return {
        success: true,
        message: `Dynamic pricing strategy applied to "${rt.name}"`,
        config: pricingConfig,
      };
    },
  },
];

/**
 * Get the runtime registry (for testing).
 */
export function _getRuntimeRegistry() {
  return runtimes;
}
