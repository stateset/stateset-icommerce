#!/usr/bin/env node

import http from 'http';
import {
  buildExactEvmPaymentRequired,
  caip2ToChainId,
  createExactEvmResourceServerHandler,
} from '../src/x402/index.js';
import { getChain } from '../src/chains/config.js';

const PORT = Number(process.env.X402_PORT || 8404);
const NETWORK = process.env.X402_NETWORK || 'eip155:84532';
const PAYEE_ADDRESS = process.env.X402_PAYEE_ADDRESS;
const FACILITATOR_PRIVATE_KEY = process.env.X402_FACILITATOR_PRIVATE_KEY;
const PAYMENT_AMOUNT = String(process.env.X402_AMOUNT || '10000');
const MAX_TIMEOUT_SECONDS = Number(process.env.X402_MAX_TIMEOUT_SECONDS || '60');
const VERIFY_ONCHAIN = process.env.X402_VERIFY_ONCHAIN !== 'false';

if (!PAYEE_ADDRESS) {
  console.error('X402_PAYEE_ADDRESS is required');
  process.exit(1);
}

if (!FACILITATOR_PRIVATE_KEY) {
  console.error('X402_FACILITATOR_PRIVATE_KEY is required');
  process.exit(1);
}

const chainId = caip2ToChainId(NETWORK);
const chain = chainId ? getChain(chainId) : null;
if (!chain) {
  console.error(`Unsupported X402_NETWORK: ${NETWORK}`);
  process.exit(1);
}

const asset = process.env.X402_ASSET || chain.tokens?.USDC?.address;
if (!asset || asset === 'native') {
  console.error(`No ERC-20 asset configured for ${NETWORK}`);
  process.exit(1);
}

const premiumHandler = createExactEvmResourceServerHandler({
  paymentRequired: (req) =>
    buildExactEvmPaymentRequired({
      url: `http://${req.headers.host}${req.url}`,
      description: 'Premium API access',
      mimeType: 'application/json',
      amount: PAYMENT_AMOUNT,
      asset,
      network: NETWORK,
      payTo: PAYEE_ADDRESS,
      maxTimeoutSeconds: MAX_TIMEOUT_SECONDS,
      extra: {
        assetTransferMethod: 'eip3009',
        name: 'USDC',
        version: '2',
      },
    }),
  facilitatorPrivateKey: FACILITATOR_PRIVATE_KEY,
  checkOnchain: VERIFY_ONCHAIN,
  onRequest: async () => ({
    status: 200,
    body: {
      data: 'Premium data unlocked',
      status: 'paid',
      network: NETWORK,
    },
  }),
});

http
  .createServer(async (req, res) => {
    if (req.url !== '/premium') {
      res.statusCode = 404;
      res.setHeader('Content-Type', 'application/json');
      res.end(JSON.stringify({ error: 'Not Found' }));
      return;
    }

    await premiumHandler(req, res);
  })
  .listen(PORT, () => {
    console.log(`x402 v2 exact server running on http://localhost:${PORT}/premium`);
    console.log(`Network: ${NETWORK}`);
    console.log(`Asset: ${asset}`);
    console.log(`Payee: ${PAYEE_ADDRESS}`);
    console.log(`Onchain verification: ${VERIFY_ONCHAIN ? 'enabled' : 'disabled'}`);
  });
