import { Commerce } from '@stateset/embedded';

let commerce: Commerce | null = null;

export function getCommerce() {
  if (!commerce) {
    commerce = new Commerce(process.env.STATESET_DB_PATH || './store.db');
  }
  return commerce;
}

export async function getProducts(
  opts: {
    search?: string;
    category?: string;
    limit?: number;
  } = {},
) {
  const c = getCommerce();
  const products = await c.products.list();
  const enriched: any[] = await Promise.all(
    products.map(async (product) => ({
      ...product,
      variants: await c.products.getVariants(product.id),
    })),
  );
  let filtered = enriched.slice(0, opts.limit || 20);

  if (opts.search) {
    const q = opts.search.toLowerCase();
    filtered = filtered.filter(
      (p: any) => p.name?.toLowerCase().includes(q) || p.description?.toLowerCase().includes(q),
    );
  }

  if (opts.category) {
    const cat = opts.category.toLowerCase();
    filtered = filtered.filter(
      (p: any) =>
        p.category?.toLowerCase() === cat || p.tags?.some((t: string) => t.toLowerCase() === cat),
    );
  }

  return { products: filtered };
}

export async function getProductBySlug(slug: string) {
  const c = getCommerce();
  const products: any[] = await Promise.all(
    (await c.products.list()).map(async (product) => ({
      ...product,
      variants: await c.products.getVariants(product.id),
    })),
  );
  return (
    products.find(
      (p: any) => p.slug === slug || p.name?.toLowerCase().replace(/\s+/g, '-') === slug,
    ) || null
  );
}
