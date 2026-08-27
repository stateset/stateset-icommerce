import { NextRequest, NextResponse } from 'next/server';
import { createPublicClient, http } from 'viem';
import { base } from 'viem/chains';
import { getCommerce } from '@/lib/commerce';
import { decimalToUnits } from '@/lib/money.js';
import {
  assertTransactionHash,
  normalizeAddress,
  verifyUsdcTransfer,
} from '@/lib/usdc-verification.js';
import { USDC_ADDRESS } from '@/lib/wagmi';
import { CheckoutError, executeCheckout } from '@/lib/checkout-service.js';

export const runtime = 'nodejs';

function requiredServerConfig() {
  const rpcUrl = process.env.STATESET_BASE_RPC_URL;
  const merchantAddress = process.env.NEXT_PUBLIC_STORE_WALLET_ADDRESS;
  if (!rpcUrl) throw new Error('STATESET_BASE_RPC_URL is required');
  if (!merchantAddress || /^0x0{40}$/i.test(merchantAddress))
    throw new Error('NEXT_PUBLIC_STORE_WALLET_ADDRESS must be configured');
  return {
    rpcUrl,
    merchantAddress: normalizeAddress(merchantAddress),
    minimumConfirmations: Number(process.env.STATESET_USDC_CONFIRMATIONS || '2'),
  };
}

export async function POST(request: NextRequest) {
  try {
    const {
      cartId,
      email,
      txHash: rawTxHash,
      walletAddress,
      shippingAddress,
      shippingMethodId,
    } = await request.json();
    if (!cartId || !email || !rawTxHash || !walletAddress || !shippingAddress) {
      return NextResponse.json(
        { error: 'cartId, email, txHash, walletAddress, and shippingAddress are required' },
        { status: 400 },
      );
    }
    const txHash = assertTransactionHash(rawTxHash);
    const payerAddress = normalizeAddress(walletAddress);
    const config = requiredServerConfig();
    if (!Number.isInteger(config.minimumConfirmations) || config.minimumConfirmations < 1)
      throw new Error('STATESET_USDC_CONFIRMATIONS must be a positive integer');

    const client = createPublicClient({ chain: base, transport: http(config.rpcUrl) });
    const result = await executeCheckout({
      commerce: getCommerce(),
      cartId,
      email,
      txHash,
      payerAddress,
      shippingAddress,
      shippingMethodId,
      verifySettlement: async ({ totals }: { totals: { total: string } }) => {
        const [receipt, currentBlock] = await Promise.all([
          client.getTransactionReceipt({ hash: txHash as `0x${string}` }),
          client.getBlockNumber(),
        ]);
        return verifyUsdcTransfer({
          receipt,
          currentBlock,
          tokenAddress: USDC_ADDRESS,
          merchantAddress: config.merchantAddress,
          payerAddress,
          expectedAmountUnits: decimalToUnits(totals.total, 6),
          minimumConfirmations: config.minimumConfirmations,
        });
      },
    });
    return NextResponse.json(result);
  } catch (error) {
    console.error('Checkout error:', error);
    const message = error instanceof Error ? error.message : 'Checkout failed';
    const clientError =
      /invalid|required|does not contain|did not succeed|confirmations|already used|not active/i.test(
        message,
      );
    const conflict = /already used|not active/i.test(message);
    const status =
      error instanceof CheckoutError ? error.status : conflict ? 409 : clientError ? 400 : 500;
    return NextResponse.json({ error: message }, { status });
  }
}
