/**
 * What each optional A2A service needs, as surfaced to agents.
 *
 * The MCP server (src/mcp/a2a-service.js) attaches every A2A service it can
 * build from what it has. A few need inputs it may not have — an agent
 * wallet, a durable data directory, a sequencer client — and the tools backed
 * by them answer with these messages, which name the missing input and how to
 * provide it, instead of a bare "not initialized".
 */

export const A2A_SERVICE_REQUIREMENTS = Object.freeze({
  billingExecutor:
    "Billing executor not initialized: it pays due subscriptions from this agent's wallet, " +
    'so it needs an agent wallet. Pass agentConfig { walletAddress, signingKey } to ' +
    'createStatesetMcpServer (the stateset-mcp CLI has no wallet flags).',
  batchService:
    'Batch service not initialized: batch payments and quote requests are sent from this ' +
    "agent's wallet. Pass agentConfig { walletAddress, signingKey } to createStatesetMcpServer " +
    '(the stateset-mcp CLI has no wallet flags).',
  checkpointService:
    'Checkpoint service not initialized: checkpoints are files kept beside the database, and ' +
    'this server has no file-backed database. Start it with --db <file> or pass ' +
    'policyStorePath to createStatesetMcpServer.',
  sequencerClient:
    'No x402 sequencer client is configured for this server. Pass agentConfig.sequencerClient ' +
    'to createStatesetMcpServer, or use stateset-x402-mcp with X402_SEQUENCER_URL.',
  tickOptimizer:
    'Tick optimizer not initialized: no tick loop in this MCP server is instrumented with a ' +
    'tick optimizer, so there are no tick metrics to report.',
  // The built-in saga definitions (src/a2a/saga.js) call methods no service
  // here provides: subscription -> billing.processPayment and
  // subscriptions.activateSubscription/deactivateSubscription; rfq ->
  // marketplace.collectResponses/awardWinner/revokeAward/cancelRFQ. The
  // purchase saga needs a wallet-bound A2A service and releases escrow even
  // when await_fulfillment reports the conditions unmet.
  sagaExecute:
    'Saga execution is not available: the built-in purchase/subscription/rfq sagas call ' +
    'service methods this server does not provide. Use the individual quote, escrow and ' +
    'RFQ tools instead.',
});

export default A2A_SERVICE_REQUIREMENTS;
