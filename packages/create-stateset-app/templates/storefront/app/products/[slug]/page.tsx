import { digitalProducts } from '@/lib/digital-access.js';
import { CampaignBuyButton } from '@/components/commerce/CampaignBuyButton';
import { getCommerce, getProductBySlug } from '@/lib/commerce';
import { notFound } from 'next/navigation';
import { ProductPurchaseOptions } from '@/components/commerce/ProductPurchaseOptions';

interface Props {
  params: Promise<{ slug: string }>;
}

export default async function ProductPage({ params }: Props) {
  const { slug } = await params;
  const product = await getProductBySlug(slug);

  if (!product) {
    notFound();
  }

  const sku = product.variants?.[0]?.sku || slug.toUpperCase();
  const variant = product.variants?.[0];
  const price = variant?.price;
  const policy = digitalProducts(process.env)[sku];
  const plan = policy?.membershipPlanId
    ? await getCommerce().subscriptions.getPlan(policy.membershipPlanId)
    : null;

  return (
    <div className="container mx-auto px-4 py-8">
      <div className="grid md:grid-cols-2 gap-8">
        <div className="aspect-square bg-gray-100 rounded-lg" />
        <div>
          <h1 className="text-3xl font-bold">{product.name}</h1>
          <p className="mt-4 text-gray-600">{product.description}</p>
          <p className="mt-2 text-sm text-gray-400">SKU: {sku}</p>
          {policy?.membershipPlanId ? (
            plan?.status === 'active' ? (
              <div className="mt-6 space-y-3">
                <p className="text-2xl font-bold">
                  {plan.currency} {variant.priceExact}
                </p>
                <p>
                  {policy.recurring
                    ? 'Renews ' +
                      (plan.billingInterval === 'custom'
                        ? 'every ' + plan.customIntervalDays + ' days'
                        : plan.billingInterval) +
                      '.'
                    : 'Prepaid membership.'}{' '}
                  Tax and any shipping are shown at checkout.
                </p>
                <CampaignBuyButton
                  sku={sku}
                  name={product.name}
                  label={policy.recurring ? 'Subscribe' : 'Buy membership'}
                />
              </div>
            ) : (
              <p>Membership is currently unavailable.</p>
            )
          ) : price ? (
            <div className="mt-6">
              <ProductPurchaseOptions sku={sku} productName={product.name} price={price} />
            </div>
          ) : (
            <p className="mt-4 text-gray-500">Price TBD</p>
          )}
        </div>
      </div>
    </div>
  );
}
