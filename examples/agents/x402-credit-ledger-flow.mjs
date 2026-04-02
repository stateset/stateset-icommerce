#!/usr/bin/env node

import embedded from '../../bindings/node/index.js';
import {
  formatUsdc,
  isMain,
  printKeyValue,
  printSection,
  walletAddressFromPublicKey,
} from './x402-demo-helpers.mjs';

const { Commerce, vesHybridGenerateSigningKeypair } = embedded;

export async function runCreditLedgerFlowDemo() {
  printSection('Agent Flow 3: x402 Credit Ledger Metering');

  const commerce = new Commerce(':memory:');

  const keys = vesHybridGenerateSigningKeypair();
  const payerAddress = walletAddressFromPublicKey(keys.ed25519PublicKey);

  const initialBalance = await commerce.x402.getCreditBalance({
    payerAddress,
    asset: 'usdc',
    network: 'set_chain',
  });
  const deposit = await commerce.x402.creditAccount({
    payerAddress,
    amount: 300_000,
    asset: 'usdc',
    network: 'set_chain',
    reason: 'Seed prepaid budget for supplier discovery agent',
    referenceId: 'budget-seed-001',
  });

  const debits = [
    {
      amount: 50_000,
      reason: 'Supplier quote search',
      referenceId: 'call-001',
    },
    {
      amount: 70_000,
      reason: 'Lead-time risk report',
      referenceId: 'call-002',
    },
    {
      amount: 30_000,
      reason: 'Safety stock recommendation',
      referenceId: 'call-003',
    },
  ];

  for (const debit of debits) {
    await commerce.x402.debitAccount({
      payerAddress,
      asset: 'usdc',
      network: 'set_chain',
      amount: debit.amount,
      reason: debit.reason,
      referenceId: debit.referenceId,
    });
  }

  const finalBalance = await commerce.x402.getCreditBalance({
    payerAddress,
    asset: 'usdc',
    network: 'set_chain',
  });
  const transactions = await commerce.x402.listCreditTransactions({
    payerAddress,
    asset: 'usdc',
    network: 'set_chain',
    limit: 10,
  });

  printKeyValue('Payer wallet', payerAddress);
  printKeyValue('Opening balance', formatUsdc(initialBalance));
  printKeyValue('Deposited', formatUsdc(deposit.amount));
  printKeyValue('Debits applied', debits.length);
  printKeyValue('Closing balance', formatUsdc(finalBalance));
  console.log('Ledger entries           ' + JSON.stringify(transactions, null, 2));

  return {
    payerAddress,
    finalBalance,
    transactions,
  };
}

if (isMain(import.meta)) {
  runCreditLedgerFlowDemo().catch((error) => {
    console.error(error);
    process.exitCode = 1;
  });
}
