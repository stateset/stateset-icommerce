'use client';

import { useState, useCallback } from 'react';
import { useAccount, useReadContract, useWriteContract, useWaitForTransactionReceipt } from 'wagmi';
import { parseUnits } from 'viem';
import { USDC_ADDRESS, USDC_DECIMALS, MERCHANT_ADDRESS, ERC20_ABI, activeChain } from '@/lib/wagmi';

type PaymentStatus = 'idle' | 'confirming' | 'pending' | 'success' | 'error';

export function useUSDCPayment() {
  const { address } = useAccount();
  const [paymentStatus, setPaymentStatus] = useState<PaymentStatus>('idle');
  const [txHash, setTxHash] = useState<string | undefined>();
  const [error, setError] = useState<string | undefined>();

  const { data: usdcBalance } = useReadContract({
    address: USDC_ADDRESS,
    abi: ERC20_ABI,
    functionName: 'balanceOf',
    args: address ? [address] : undefined,
    chainId: activeChain.id,
  });

  const { writeContractAsync } = useWriteContract();

  const { isLoading: isConfirming } = useWaitForTransactionReceipt({
    hash: txHash as `0x${string}` | undefined,
    chainId: activeChain.id,
    query: {
      enabled: !!txHash && paymentStatus === 'pending',
    },
  });

  const sendPayment = useCallback(async (amount: number) => {
    try {
      setPaymentStatus('confirming');
      setError(undefined);

      const amountInUnits = parseUnits(amount.toString(), USDC_DECIMALS);

      const hash = await writeContractAsync({
        address: USDC_ADDRESS,
        abi: ERC20_ABI,
        functionName: 'transfer',
        args: [MERCHANT_ADDRESS, amountInUnits],
        chainId: activeChain.id,
      });

      setTxHash(hash);
      setPaymentStatus('pending');

      // Wait for confirmation
      const checkReceipt = async () => {
        try {
          const response = await fetch(
            `https://base-mainnet.g.alchemy.com/v2/demo`,
            {
              method: 'POST',
              headers: { 'Content-Type': 'application/json' },
              body: JSON.stringify({
                jsonrpc: '2.0',
                method: 'eth_getTransactionReceipt',
                params: [hash],
                id: 1,
              }),
            }
          );
          const data = await response.json();
          if (data.result?.status === '0x1') {
            setPaymentStatus('success');
            return;
          }
          if (data.result?.status === '0x0') {
            setPaymentStatus('error');
            setError('Transaction reverted');
            return;
          }
        } catch {}
        setTimeout(checkReceipt, 3000);
      };
      setTimeout(checkReceipt, 5000);
    } catch (err: any) {
      setPaymentStatus('error');
      setError(err?.shortMessage || err?.message || 'Payment failed');
    }
  }, [writeContractAsync]);

  const reset = useCallback(() => {
    setPaymentStatus('idle');
    setTxHash(undefined);
    setError(undefined);
  }, []);

  return {
    paymentStatus,
    txHash,
    error,
    usdcBalance: usdcBalance as bigint | undefined,
    isConfirming,
    sendPayment,
    reset,
  };
}
