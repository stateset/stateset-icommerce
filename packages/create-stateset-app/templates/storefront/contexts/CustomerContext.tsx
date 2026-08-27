'use client';

import { createContext, useContext, useState, useEffect, useCallback, ReactNode } from 'react';
import { useAccount, useSignMessage } from 'wagmi';
import { WALLET_AUTH_TTL_MS, walletAuthMessage } from '@/lib/wallet-auth';

interface Customer {
  id: string;
  email: string;
  firstName?: string;
  lastName?: string;
  metadata?: { walletAddress?: string };
}

interface CustomerContextType {
  customer: Customer | null;
  orders: any[];
  subscriptions: any[];
  isLoading: boolean;
  isAuthenticated: boolean;
  authenticate: () => Promise<void>;
  authenticatedFetch: (input: RequestInfo | URL, init?: RequestInit) => Promise<Response>;
  refreshCustomer: () => Promise<void>;
  refreshSubscriptions: () => Promise<void>;
}

const CustomerContext = createContext<CustomerContextType>({
  customer: null,
  orders: [],
  subscriptions: [],
  isLoading: false,
  isAuthenticated: false,
  authenticate: async () => {},
  authenticatedFetch: fetch,
  refreshCustomer: async () => {},
  refreshSubscriptions: async () => {},
});

export function CustomerProvider({ children }: { children: ReactNode }) {
  const { address, isConnected } = useAccount();
  const { signMessageAsync } = useSignMessage();
  const [customer, setCustomer] = useState<Customer | null>(null);
  const [orders, setOrders] = useState<any[]>([]);
  const [subscriptions, setSubscriptions] = useState<any[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [walletAuth, setWalletAuth] = useState<{
    address: string;
    timestamp: string;
    signature: string;
  } | null>(null);

  const authenticatedFetch = useCallback(
    (input: RequestInfo | URL, init: RequestInit = {}) => {
      if (!walletAuth) return Promise.reject(new Error('Wallet authentication required'));
      const headers = new Headers(init.headers);
      headers.set('x-wallet-timestamp', walletAuth.timestamp);
      headers.set('x-wallet-signature', walletAuth.signature);
      return fetch(input, { ...init, headers });
    },
    [walletAuth],
  );

  const fetchCustomer = useCallback(async () => {
    if (!address || !walletAuth || walletAuth.address !== address.toLowerCase()) return;
    setIsLoading(true);
    try {
      const response = await authenticatedFetch(`/api/customers/by-wallet?address=${address}`);
      const data = await response.json();
      if (data.customer) {
        setCustomer(data.customer);
        setOrders(data.orders || []);
        if (data.customer.id) {
          try {
            const subRes = await authenticatedFetch(
              `/api/subscriptions?customerId=${data.customer.id}&wallet=${address}`,
            );
            const subData = await subRes.json();
            setSubscriptions(subData.subscriptions || []);
          } catch {
            setSubscriptions([]);
          }
        }
      } else {
        setCustomer(null);
        setOrders([]);
        setSubscriptions([]);
      }
    } catch {
      setCustomer(null);
      setOrders([]);
      setSubscriptions([]);
    } finally {
      setIsLoading(false);
    }
  }, [address, walletAuth, authenticatedFetch]);

  const authenticate = useCallback(async () => {
    if (!address) return;
    const timestamp = Date.now().toString();
    const signature = await signMessageAsync({ message: walletAuthMessage(address, timestamp) });
    setWalletAuth({ address: address.toLowerCase(), timestamp, signature });
  }, [address, signMessageAsync]);

  const refreshSubscriptions = useCallback(async () => {
    if (!customer?.id) return;
    try {
      const response = await authenticatedFetch(
        `/api/subscriptions?customerId=${customer.id}&wallet=${address}`,
      );
      const data = await response.json();
      setSubscriptions(data.subscriptions || []);
    } catch {}
  }, [customer?.id, address, authenticatedFetch]);

  useEffect(() => {
    if (isConnected && address && walletAuth?.address === address.toLowerCase()) {
      fetchCustomer();
    } else {
      setCustomer(null);
      setOrders([]);
      setSubscriptions([]);
    }
  }, [isConnected, address, walletAuth, fetchCustomer]);

  useEffect(() => {
    if (!address || walletAuth?.address !== address.toLowerCase()) setWalletAuth(null);
  }, [address, walletAuth?.address]);

  useEffect(() => {
    if (!walletAuth) return;
    const remaining = Number(walletAuth.timestamp) + WALLET_AUTH_TTL_MS - Date.now();
    const timer = setTimeout(() => setWalletAuth(null), Math.max(0, remaining));
    return () => clearTimeout(timer);
  }, [walletAuth]);

  return (
    <CustomerContext.Provider
      value={{
        customer,
        orders,
        subscriptions,
        isLoading,
        isAuthenticated: isConnected && walletAuth?.address === address?.toLowerCase(),
        authenticate,
        authenticatedFetch,
        refreshCustomer: fetchCustomer,
        refreshSubscriptions,
      }}
    >
      {children}
    </CustomerContext.Provider>
  );
}

export function useCustomer() {
  return useContext(CustomerContext);
}
