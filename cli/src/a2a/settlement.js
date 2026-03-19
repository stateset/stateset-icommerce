/**
 * Settlement Service — Bridge between A2A payments and on-chain execution
 *
 * Wraps the chains/ infrastructure (wallet derivation, EVM/Solana transaction
 * execution) into a simple API that the agent runtime can call after accepting
 * a quote.
 *
 * Chain modules are lazy-loaded to avoid pulling in ethers/solana SDKs when
 * settlement is not in use.
 *
 * @example
 * ```javascript
 * import { createSettlementService } from './settlement.js';
 *
 * const settlement = createSettlementService({
 *   chainId: 'base',
 *   agentId: 'agent-001',
 *   simulate: false,
 * });
 *
 * const address = await settlement.getAddress();
 * const balance = await settlement.getBalance();
 * const result = await settlement.settle({
 *   toAddress: '0xSeller...',
 *   amount: 50.00,
 *   memo: 'Quote payment',
 * });
 * // result: { success: true, txHash: '0x...', blockNumber: 12345, explorerUrl: '...' }
 * ```
 */

// ---------------------------------------------------------------------------
// Lazy chain module loader
// ---------------------------------------------------------------------------

let _chainsModule = null;

async function loadChains() {
  if (!_chainsModule) {
    _chainsModule = await import('../chains/index.js');
  }
  return _chainsModule;
}

// ---------------------------------------------------------------------------
// Settlement Service Factory
// ---------------------------------------------------------------------------

/**
 * @typedef {Object} SettlementConfig
 * @property {string} chainId        - Target blockchain (base, solana, set_chain, etc.)
 * @property {string} agentId        - Agent UUID for wallet derivation
 * @property {boolean} [simulate]    - If true, build tx but don't broadcast (default: false)
 * @property {string} [configDir]    - Key/wallet config directory (default: '.stateset')
 * @property {string} [tokenSymbol]  - Override token (default: chain's default payment token)
 * @property {Function} [onProgress] - Progress callback for chain operations
 * @property {Function} [logger]     - Logging function
 */

/**
 * @typedef {Object} SettlementResult
 * @property {boolean} success
 * @property {string} [txHash]
 * @property {number} [blockNumber]
 * @property {string} [explorerUrl]
 * @property {number} [confirmations]
 * @property {boolean} [simulated]
 * @property {string} [intentId]
 * @property {string} [error]
 */

/**
 * Create a settlement service for an A2A agent.
 *
 * @param {SettlementConfig} config
 * @returns {Object} Settlement service instance
 */
export function createSettlementService(config) {
  const {
    chainId,
    agentId,
    simulate = false,
    configDir = '.stateset',
    tokenSymbol,
    onProgress,
    logger = () => {},
  } = config;

  if (!chainId) throw new Error('chainId is required for settlement service');
  if (!agentId) throw new Error('agentId is required for settlement service');

  // Cache for derived wallet address
  let _cachedAddress = null;

  /**
   * Get the derived on-chain wallet address for this agent.
   * Result is cached after first call.
   * @returns {Promise<string>}
   */
  async function getAddress() {
    if (_cachedAddress) return _cachedAddress;
    const chains = await loadChains();
    _cachedAddress = await chains.getWalletAddress(agentId, chainId, { configDir });
    return _cachedAddress;
  }

  /**
   * Get on-chain token balance for this agent's wallet.
   * @returns {Promise<{ balance: string, balanceSmallest: bigint, symbol: string }>}
   */
  async function getBalance() {
    const chains = await loadChains();
    const address = await getAddress();
    const token = tokenSymbol || chains.getDefaultPaymentToken(chainId)?.symbol;
    const result = await chains.getBalance(address, chainId, token, { configDir });
    return {
      balance: result.balance,
      balanceSmallest: result.balanceSmallest,
      symbol: result.symbol || token,
    };
  }

  /**
   * Check if this agent has sufficient on-chain balance for a given amount.
   * @param {number} amount - Amount in human-readable units (e.g. 50.00)
   * @returns {Promise<{ sufficient: boolean, balance: string, required: string, symbol: string }>}
   */
  async function hasSufficientFunds(amount) {
    const chains = await loadChains();
    const address = await getAddress();
    const token = tokenSymbol || chains.getDefaultPaymentToken(chainId)?.symbol;
    return chains.hasSufficientBalance(address, chainId, amount, token, { configDir });
  }

  /**
   * Execute on-chain settlement for an A2A payment.
   *
   * Calls executePayment() from chains/stablecoin.js which handles:
   * wallet derivation → tx building → signing → submission → confirmation.
   *
   * @param {Object} params
   * @param {string} params.toAddress   - Recipient wallet address
   * @param {number} params.amount      - Amount in human-readable units (e.g. 50.00)
   * @param {string} [params.asset]     - Token symbol override
   * @param {string} [params.memo]      - Payment memo for metadata
   * @param {string} [params.paymentId] - A2A payment record ID for traceability
   * @returns {Promise<SettlementResult>}
   */
  async function settle({ toAddress, amount, asset, memo, paymentId }) {
    try {
      const chains = await loadChains();
      const token = asset || tokenSymbol || chains.getDefaultPaymentToken(chainId)?.symbol;

      logger(
        `[settlement] Settling ${amount} ${token} → ${toAddress} on ${chainId}${simulate ? ' (simulate)' : ''}`,
      );

      const result = await chains.executePayment(
        {
          agentId,
          chainId,
          toAddress,
          amount,
          tokenSymbol: token,
          metadata: {
            source: 'a2a_settlement',
            a2a_payment_id: paymentId || null,
            memo: memo || null,
          },
        },
        {
          configDir,
          simulate,
          onProgress:
            onProgress ||
            ((event) => {
              logger(`[settlement] ${event.step}: ${event.message}`);
            }),
        },
      );

      if (!result.success) {
        return {
          success: false,
          error: result.error || 'Settlement failed',
          intentId: result.intentId,
        };
      }

      return {
        success: true,
        txHash: result.txHash || null,
        blockNumber: result.blockNumber || null,
        explorerUrl: result.explorerUrl || null,
        confirmations: result.confirmations || 0,
        simulated: result.simulated || false,
        intentId: result.intentId || null,
      };
    } catch (err) {
      logger(`[settlement] Error: ${err.message}`);
      return {
        success: false,
        error: err.message,
      };
    }
  }

  return {
    settle,
    getBalance,
    getAddress,
    hasSufficientFunds,

    /** @type {string} */
    get chainId() {
      return chainId;
    },

    /** @type {boolean} */
    get isSimulation() {
      return simulate;
    },

    /** @type {string} */
    get agentId() {
      return agentId;
    },
  };
}
