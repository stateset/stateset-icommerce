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
import { checkCartMembershipRail, grantPurchasedAccess } from '@/lib/digital-access.js';
import { recordStorefrontSale } from '@/lib/merchant-integrations.js';
import { CheckoutError, executeCheckout, normalizeCheckoutEmail } from '@/lib/checkout-service.js';
import { getCheckoutConfiguration } from '@/lib/checkout-config.js';
import { pricingEnvironment } from '@/lib/store-pricing.js';
import { getShippingProvider } from '@/lib/shipping.js';
import { getTaxProvider } from '@/lib/tax.js';

export const runtime = 'nodejs';

export async function POST(request: NextRequest) {
  try {
    if (process.env.STATESET_CHECKOUT_PROVIDER && process.env.STATESET_CHECKOUT_PROVIDER !== 'base') {
      throw new CheckoutError('USDC checkout is disabled for this store', 409);
    }
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
    const config = getCheckoutConfiguration(process.env);

    const client = createPublicClient({ chain: base, transport: http(config.rpcUrl) });
    const commerce = getCommerce();
    await checkCartMembershipRail(commerce,process.env,cartId);
    const pricing = await pricingEnvironment(commerce);
    const result = await executeCheckout({
      commerce, taxProvider:getTaxProvider(pricing), shippingProvider:getShippingProvider(pricing),
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
    const order = await commerce.orders.get(result.orderId);
    if (!order) throw new Error('Checkout order is unavailable');
    const customer = await commerce.customers.get(order.customerId);
    const [payment] = await commerce.payments.list({ orderId: order.id });
    if (payment) await grantPurchasedAccess(commerce, process.env, order, payment);
    await recordStorefrontSale(commerce, order, customer?.email || normalizeCheckoutEmail(email));
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
