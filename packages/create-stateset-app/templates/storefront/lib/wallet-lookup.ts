import { getCommerce } from './commerce';

export async function getCustomerByWallet(walletAddress: string) {
  const commerce = getCommerce();
  const customers = await commerce.customers.list();
  return (
    customers.find(
      (c: any) => c.metadata?.walletAddress?.toLowerCase() === walletAddress.toLowerCase(),
    ) || null
  );
}
