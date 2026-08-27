'use client';

import { useState, useCallback, useEffect } from 'react';
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

  const {
    isLoading: isConfirming,
    isSuccess,
    isError,
    error: receiptError,
  } = useWaitForTransactionReceipt({
    hash: txHash as `0x${string}` | undefined,
    chainId: activeChain.id,
    query: {
      enabled: !!txHash && paymentStatus === 'pending',
    },
  });

  useEffect(() => {
    if (paymentStatus !== 'pending') return;
    if (isSuccess) setPaymentStatus('success');
    if (isError) {
      setPaymentStatus('error');
      setError(receiptError?.message || 'Transaction failed');
    }
  }, [paymentStatus, isSuccess, isError, receiptError]);

  const sendPayment = useCallback(
    async (amount: string) => {
      try {
        setPaymentStatus('confirming');
        setError(undefined);

        if (/^0x0{40}$/i.test(MERCHANT_ADDRESS)) throw new Error('Store wallet is not configured');
        const amountInUnits = parseUnits(amount, USDC_DECIMALS);

        const hash = await writeContractAsync({
          address: USDC_ADDRESS,
          abi: ERC20_ABI,
          functionName: 'transfer',
          args: [MERCHANT_ADDRESS, amountInUnits],
          chainId: activeChain.id,
        });

        setTxHash(hash);
        setPaymentStatus('pending');
      } catch (err: any) {
        setPaymentStatus('error');
        setError(err?.shortMessage || err?.message || 'Payment failed');
      }
    },
    [writeContractAsync],
  );

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
