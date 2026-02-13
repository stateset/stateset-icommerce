import { streamText } from 'ai';
import { anthropic } from '@ai-sdk/anthropic';
import { getCommerce } from '@/lib/commerce';

export async function POST(req: Request) {
  const { messages, walletAddress } = await req.json();

  const commerce = getCommerce();

  let customerContext = '';
  if (walletAddress) {
    try {
      const customers = await commerce.customers.list({ limit: 100 });
      const customer = customers.find(
        (c: any) => c.notes?.toLowerCase().includes(walletAddress.toLowerCase())
      );
      if (customer) {
        const orders = await commerce.orders.list({ customerId: customer.id, limit: 5 });
        customerContext = `\nCustomer: ${customer.firstName || ''} ${customer.lastName || ''} (${customer.email}). Recent orders: ${orders.length}.`;
      }
    } catch {}
  }

  let productContext = '';
  try {
    const { products } = await commerce.products.list({ limit: 20 });
    if (products?.length) {
      const list = products.map((p: any) => {
        const price = p.variants?.[0]?.price;
        return `- ${p.name}${price ? ` ($${price})` : ''}: ${p.description || 'No description'}`;
      });
      productContext = `\n\nAvailable products:\n${list.join('\n')}`;
    }
  } catch {}

  const result = streamText({
    model: anthropic('claude-sonnet-4-20250514'),
    system: `You are a helpful shopping assistant for {{STORE_NAME}}. Help customers find products, check order status, and answer questions about the store.${customerContext}${productContext}

Be concise and friendly. When recommending products, mention specific items from the catalog with prices. If asked about order status and you have customer info, reference their recent activity.`,
    messages,
  });

  return result.toDataStreamResponse();
}
