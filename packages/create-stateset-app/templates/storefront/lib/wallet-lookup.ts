import { getCommerce } from './commerce';

export async function getCustomerByWallet(walletAddress: string) {
  const commerce = getCommerce();
  const customers = await commerce.customers.list({ limit: 200 });
  return customers.find(
    (c: any) => c.notes?.toLowerCase().includes(walletAddress.toLowerCase())
  ) || null;
}
