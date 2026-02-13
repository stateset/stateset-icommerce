'use client';

import { createContext, useContext, useState, useEffect, useCallback, ReactNode } from 'react';
import { useAccount } from 'wagmi';

interface Customer {
  id: string;
  email: string;
  firstName?: string;
  lastName?: string;
  notes?: string;
}

interface CustomerContextType {
  customer: Customer | null;
  orders: any[];
  subscriptions: any[];
  isLoading: boolean;
  isAuthenticated: boolean;
  refreshCustomer: () => Promise<void>;
  refreshSubscriptions: () => Promise<void>;
}

const CustomerContext = createContext<CustomerContextType>({
  customer: null,
  orders: [],
  subscriptions: [],
  isLoading: false,
  isAuthenticated: false,
  refreshCustomer: async () => {},
  refreshSubscriptions: async () => {},
});

export function CustomerProvider({ children }: { children: ReactNode }) {
  const { address, isConnected } = useAccount();
  const [customer, setCustomer] = useState<Customer | null>(null);
  const [orders, setOrders] = useState<any[]>([]);
  const [subscriptions, setSubscriptions] = useState<any[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const fetchCustomer = useCallback(async () => {
    if (!address) return;
    setIsLoading(true);
    try {
      const response = await fetch(`/api/customers/by-wallet?address=${address}`);
      const data = await response.json();
      if (data.customer) {
        setCustomer(data.customer);
        setOrders(data.orders || []);
        if (data.customer.id) {
          try {
            const subRes = await fetch(`/api/subscriptions?customerId=${data.customer.id}`);
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
  }, [address]);

  const refreshSubscriptions = useCallback(async () => {
    if (!customer?.id) return;
    try {
      const response = await fetch(`/api/subscriptions?customerId=${customer.id}`);
      const data = await response.json();
      setSubscriptions(data.subscriptions || []);
    } catch {}
  }, [customer?.id]);

  useEffect(() => {
    if (isConnected && address) {
      fetchCustomer();
    } else {
      setCustomer(null);
      setOrders([]);
      setSubscriptions([]);
    }
  }, [isConnected, address, fetchCustomer]);

  return (
    <CustomerContext.Provider
      value={{
        customer,
        orders,
        subscriptions,
        isLoading,
        isAuthenticated: isConnected,
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
