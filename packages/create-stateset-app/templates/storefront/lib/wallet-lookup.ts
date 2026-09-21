import { getCommerce } from './commerce';
import { walletMatches } from './customer-metadata';

export async function getCustomerByWallet(walletAddress: string) {
  const commerce = getCommerce();
  const customers = await commerce.customers.list();
  return (
    customers.find(
      (c) => walletMatches(c.metadata, walletAddress),
    ) || null
  );
}
