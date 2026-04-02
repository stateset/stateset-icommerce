#!/usr/bin/env node

import http from 'node:http';
import crypto from 'node:crypto';
import {
  buildExactEvmPaymentRequired,
  createBudgetState,
  createX402Agent,
  createExactEvmResourceServerHandler,
  decodeBase64Json,
} from '../../cli/src/x402/index.js';
import { getChain } from '../../cli/src/chains/config.js';
import { deriveEvmWalletFromSeed } from '../../cli/src/chains/wallet.js';
import {
  closeServer,
  createDemoTxHash,
  createTempPath,
  formatUsdc,
  isMain,
  printKeyValue,
  printSection,
  shortHex,
} from './x402-demo-helpers.mjs';

const DEMO_CHAIN_ID = 'base_sepolia';
const DEMO_NETWORK = 'eip155:84532';
const DEMO_AMOUNT = '10000';

export async function runExactHttpFlowDemo() {
  printSection('Agent Flow 1: Exact x402 v2 Paid HTTP');

  const chain = getChain(DEMO_CHAIN_ID);
  const buyerSeed = crypto.randomBytes(32);
  const sellerSeed = crypto.randomBytes(32);
  const buyerWallet = deriveEvmWalletFromSeed(buyerSeed, DEMO_CHAIN_ID);
  const sellerWallet = deriveEvmWalletFromSeed(sellerSeed, DEMO_CHAIN_ID);
  const budgetState = createBudgetState({
    filePath: createTempPath('stateset-x402-budget'),
    startingBalance: 50_000,
  });

  const premiumHandler = createExactEvmResourceServerHandler({
    paymentRequired: (req) =>
      buildExactEvmPaymentRequired({
        url: `http://${req.headers.host}${req.url}`,
        description: 'Live supplier market snapshot',
        mimeType: 'application/json',
        amount: DEMO_AMOUNT,
        asset: chain.tokens.USDC.address,
        network: DEMO_NETWORK,
        payTo: sellerWallet.address,
        maxTimeoutSeconds: 60,
        extra: {
          assetTransferMethod: 'eip3009',
          name: 'USDC',
          version: '2',
        },
      }),
    checkOnchain: false,
    settlePayment: async ({ paymentPayload, paymentRequirements }) => ({
      success: true,
      payer: paymentPayload.payload.authorization.from,
      transaction: createDemoTxHash(),
      network: String(paymentRequirements.network),
      amount: String(paymentRequirements.amount),
      extensions: {
        receipt: {
          demo: true,
          settledAt: new Date().toISOString(),
          facilitator: 'local-demo',
        },
      },
    }),
    onRequest: async ({ req, paymentPayload, settlement }) => {
      const url = new URL(req.url, `http://${req.headers.host}`);
      return {
        status: 200,
        body: {
          sku: url.searchParams.get('sku') || 'SKU-DEMO-001',
          sourceAgent: 'market-data-agent',
          payer: paymentPayload.payload.authorization.from,
          quotedUnitCostUsd: 12.34,
          txHash: settlement.transaction,
        },
      };
    },
  });

  const server = http.createServer(async (req, res) => {
    if (req.url?.startsWith('/premium/market-snapshot')) {
      await premiumHandler(req, res);
      return;
    }

    res.statusCode = 404;
    res.setHeader('Content-Type', 'application/json');
    res.end(JSON.stringify({ error: 'Not Found' }));
  });

  let port;
  try {
    port = await new Promise((resolve, reject) => {
      server.listen(0, '127.0.0.1', () => {
        const address = server.address();
        if (!address || typeof address === 'string') {
          reject(new Error('Failed to resolve demo server port'));
          return;
        }
        resolve(address.port);
      });
    });

    const agent = createX402Agent({
      agentId: 'inventory-planner-agent',
      payerAddress: buyerWallet.address,
      signingKey: {
        privateKey: buyerSeed,
        publicKey: crypto.randomBytes(32),
      },
      validateUrl: false,
      budgetState,
      maxAmountPerCall: 25_000,
      dailyBudget: 50_000,
    });

    const response = await agent.fetch(
      `http://127.0.0.1:${port}/premium/market-snapshot?sku=SKU-RESTOCK-42`,
      {
        method: 'GET',
        headers: {
          accept: 'application/json',
        },
      },
    );
    const payload = await response.json();
    const paymentResponse = decodeBase64Json(response.headers.get('PAYMENT-RESPONSE'));

    printKeyValue('Buyer agent', 'inventory-planner-agent');
    printKeyValue('Buyer wallet', buyerWallet.address);
    printKeyValue('Seller agent', 'market-data-agent');
    printKeyValue('Seller wallet', sellerWallet.address);
    printKeyValue('Charged amount', formatUsdc(DEMO_AMOUNT));
    printKeyValue('HTTP status', response.status);
    printKeyValue('Receipt tx hash', shortHex(paymentResponse.transaction));
    printKeyValue('Budget balance', formatUsdc(budgetState.getBalance()));
    console.log('Unlocked payload         ' + JSON.stringify(payload, null, 2));

    return {
      buyerWallet: buyerWallet.address,
      sellerWallet: sellerWallet.address,
      paymentResponse,
      payload,
      budgetBalance: budgetState.getBalance(),
    };
  } finally {
    await closeServer(server);
  }
}

if (isMain(import.meta)) {
  runExactHttpFlowDemo().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
