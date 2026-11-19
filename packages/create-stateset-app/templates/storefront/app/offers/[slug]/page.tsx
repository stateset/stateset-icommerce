import { cookies } from 'next/headers';
import { notFound, redirect } from 'next/navigation';
import { getCommerce } from '@/lib/commerce';
import {
  activeOffer,
  campaignConfig,
  campaignCookie,
  decodeCampaign,
  catalogOffer,
} from '@/lib/campaigns.js';
import { CampaignBuyButton } from '@/components/commerce/CampaignBuyButton';

export default async function Offer({ params }: { params: Promise<{ slug: string }> }) {
  const { slug } = await params;
  const commerce = getCommerce();
  if (!(await campaignConfig(commerce, process.env, slug))) notFound();
  const attribution = decodeCampaign(process.env, (await cookies()).get(campaignCookie)?.value);
  if (attribution?.campaignId !== slug) redirect('/c/' + slug);
  const variant = await activeOffer(commerce, process.env, attribution);
  if (!variant) notFound();
  const item = await catalogOffer(getCommerce(), variant.sku);
  if (!item) notFound();
  return (
    <main className="mx-auto max-w-xl space-y-6 px-4 py-14">
      <h1 className="text-4xl font-bold">{variant.title}</h1>
      {variant.description && <p>{variant.description}</p>}
      <p>
        {item.name} · USD {item.unitPrice}
      </p>
      <CampaignBuyButton sku={item.sku} name={item.name} />
    </main>
  );
}
