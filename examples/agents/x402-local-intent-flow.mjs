#!/usr/bin/env node

import crypto from 'node:crypto';
import embedded from '../../bindings/node/index.js';
import {
  createDemoTxHash,
  isMain,
  printKeyValue,
  printSection,
  shortHex,
  toHex,
  walletAddressFromPublicKey,
} from './x402-demo-helpers.mjs';

const { Commerce, ed25519Sign, vesHybridGenerateSigningKeypair, vesX402ComputeSigningHash } =
  embedded;

function signingHashInputFromIntent(intent) {
  return {
    payerAddress: intent.payerAddress,
    payeeAddress: intent.payeeAddress,
    amount: intent.amount,
    asset: intent.asset,
    network: intent.network,
    chainId: intent.chainId,
    validUntil: intent.validUntil,
    nonce: intent.nonce,
    resourceUri: intent.resourceUri,
    resourceMethod: intent.resourceMethod,
  };
}

export async function runLocalIntentFlowDemo() {
  printSection('Agent Flow 2: Local x402 Intent Lifecycle');

  const commerce = new Commerce(':memory:');

  const buyerKeys = vesHybridGenerateSigningKeypair();
  const sellerKeys = vesHybridGenerateSigningKeypair();
  const buyerAddress = walletAddressFromPublicKey(buyerKeys.ed25519PublicKey);
  const sellerAddress = walletAddressFromPublicKey(sellerKeys.ed25519PublicKey);
  const orderId = crypto.randomUUID();

  const buyerCard = await commerce.x402.registerAgent({
    name: 'procurement-agent',
    description: 'Buys restock inventory from verified suppliers',
    walletAddress: buyerAddress,
    publicKey: toHex(buyerKeys.ed25519PublicKey),
    supportedNetworks: ['set_chain'],
    supportedAssets: ['usdc'],
    a2ASkills: ['buy', 'quote'],
    trustLevel: 'verified',
    endpointProtocol: 'mcp',
  });
  const sellerCard = await commerce.x402.registerAgent({
    name: 'supplier-agent',
    description: 'Receives x402 purchase intents for restock workflows',
    walletAddress: sellerAddress,
    publicKey: toHex(sellerKeys.ed25519PublicKey),
    supportedNetworks: ['set_chain'],
    supportedAssets: ['usdc'],
    a2ASkills: ['sell', 'quote', 'fulfill', 'ship'],
    trustLevel: 'verified',
    endpointProtocol: 'mcp',
  });

  const createdIntent = await commerce.x402.createIntent({
    payerAddress: buyerAddress,
    payeeAddress: sellerAddress,
    amount: 2_500_000,
    asset: 'usdc',
    network: 'set_chain',
    orderId,
    description: 'Restock 25 safety sensors for purchase order PO-1001',
    validitySeconds: 900,
  });
  const storedIntent = await commerce.x402.getIntent(createdIntent.id);
  const signingHash = vesX402ComputeSigningHash(signingHashInputFromIntent(storedIntent));
  const signature = ed25519Sign(signingHash, buyerKeys.ed25519PrivateKey);

  const signedIntent = await commerce.x402.signIntent(createdIntent.id, {
    signature: toHex(signature),
    publicKey: toHex(buyerKeys.ed25519PublicKey),
  });
  const settledIntent = await commerce.x402.markSettled(
    createdIntent.id,
    createDemoTxHash(),
    84532001,
  );
  const listedIntents = await commerce.x402.listIntents({
    payerAddress: buyerAddress,
    limit: 5,
  });
  const persistedSigningHash =
    signedIntent.signingHash || storedIntent.signingHash || createdIntent.signingHash || '';

  printKeyValue('Buyer card', `${buyerCard.name} (${shortHex(buyerCard.id)})`);
  printKeyValue('Seller card', `${sellerCard.name} (${shortHex(sellerCard.id)})`);
  printKeyValue('Intent id', createdIntent.id);
  printKeyValue('Order id', orderId);
  printKeyValue('Signing hash', shortHex(persistedSigningHash));
  printKeyValue(
    'Hash recomputed',
    persistedSigningHash === toHex(signingHash) ? 'yes' : 'no',
  );
  printKeyValue('Signed status', signedIntent.status);
  printKeyValue('Settled status', settledIntent.status);
  printKeyValue('Intent count', listedIntents.length);
  console.log('Latest intent            ' + JSON.stringify(listedIntents[0], null, 2));

  return {
    buyerCard,
    sellerCard,
    intentId: createdIntent.id,
    listedIntents,
  };
}

if (isMain(import.meta)) {
  runLocalIntentFlowDemo().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
