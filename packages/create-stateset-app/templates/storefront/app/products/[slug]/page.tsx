import type { ProductVariantOutput } from '@stateset/embedded';
import Link from 'next/link';
import { AddToCartButton } from '@/components/commerce/AddToCartButton';
import { importedOneTimeProduct } from '@/lib/next-catalog.js';
import { digitalProducts } from '@/lib/digital-access.js';
import { CampaignBuyButton } from '@/components/commerce/CampaignBuyButton';
import { getCommerce, getProductBySlug } from '@/lib/commerce';
import { notFound } from 'next/navigation';
import { ProductPurchaseOptions } from '@/components/commerce/ProductPurchaseOptions';

interface Props {
  params: Promise<{ slug: string }>;
  searchParams: Promise<{ variant?: string }>;
}

export default async function ProductPage({ params, searchParams }: Props) {
  const { slug } = await params;
  const product = await getProductBySlug(slug);

  if (!product || product.status !== 'active') {
    notFound();
  }

  const requested = (await searchParams).variant;
  const variants: ProductVariantOutput[] = product.variants;
  const variant = requested
    ? variants.find((v) => v.sku === requested)
    : variants.find((v) => v.isDefault) || variants[0];
  if (requested && !variant) notFound();
  const sku = variant?.sku || slug.toUpperCase();
  const imported = await importedOneTimeProduct(getCommerce(), product.id);
  const stock = imported && variant ? await getCommerce().inventory.getStock(variant.sku) : null;
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
          {variants.length > 1 && (
            <nav aria-label="Product variants" className="mt-3 flex flex-wrap gap-3">
              {variants.map((v) => (
                <Link
                  className="rounded border p-2"
                  key={v.id}
                  aria-current={v.sku === sku ? 'page' : undefined}
                  href={{ pathname: '/products/' + product.slug, query: { variant: v.sku } }}
                >
                  {v.name} ({v.sku})
                </Link>
              ))}
            </nav>
          )}
          {imported && variant ? (
            <div className="mt-6 space-y-3">
              <p className="text-2xl font-bold">USD {variant.priceExact}</p>
              {variant.compareAtPriceExact && (
                <p>
                  Retail price <s>USD {variant.compareAtPriceExact}</s>
                </p>
              )}
              {stock && Number(stock.totalAvailable) > 0 ? (
                <AddToCartButton
                  key={sku}
                  sku={sku}
                  productName={product.name}
                  price={variant.price}
                />
              ) : (
                <p>Out of stock</p>
              )}
            </div>
          ) : policy?.membershipPlanId ? (
            plan?.status === 'active' ? (
              <div className="mt-6 space-y-3">
                <p className="text-2xl font-bold">
                  {plan.currency} {variant?.priceExact}
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
