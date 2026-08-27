import { convertToModelMessages, streamText, type UIMessage } from 'ai';
import { anthropic } from '@ai-sdk/anthropic';
import { getCommerce } from '@/lib/commerce';

export async function POST(req: Request) {
  if (!process.env.ANTHROPIC_API_KEY) {
    return Response.json({ error: 'Store assistant is not configured' }, { status: 503 });
  }

  const body = await req.json();
  const messages = Array.isArray(body?.messages) ? (body.messages as UIMessage[]).slice(-20) : [];
  if (messages.length === 0) {
    return Response.json({ error: 'At least one message is required' }, { status: 400 });
  }

  const commerce = getCommerce();

  let productContext = '';
  try {
    const products = (await commerce.products.list()).slice(0, 20);
    if (products?.length) {
      const list = products.map((p: any) => {
        const price = p.variants?.[0]?.price;
        return `- ${p.name}${price ? ` ($${price})` : ''}: ${p.description || 'No description'}`;
      });
      productContext = `\n\nAvailable products:\n${list.join('\n')}`;
    }
  } catch {}

  const result = streamText({
    model: anthropic(process.env.ANTHROPIC_MODEL || 'claude-sonnet-4-5'),
    system: `You are a helpful shopping assistant for {{STORE_NAME}}. Help customers find products and answer questions about the store.${productContext}

Be concise and friendly. When recommending products, mention specific items from the catalog with prices. Never claim access to customer, order, payment, or subscription data. Direct account-specific questions to the signed-in account pages.`,
    messages: await convertToModelMessages(messages),
  });

  return result.toUIMessageStreamResponse();
}
