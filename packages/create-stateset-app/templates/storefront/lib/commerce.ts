import { StateSetCommerce } from '@stateset/embedded';

let commerce: ReturnType<typeof StateSetCommerce.create> | null = null;

export function getCommerce() {
  if (!commerce) {
    commerce = StateSetCommerce.create({
      dbPath: process.env.STATESET_DB_PATH || './store.db',
    });
  }
  return commerce;
}

export async function getProducts(opts: {
  search?: string;
  category?: string;
  limit?: number;
} = {}) {
  const c = getCommerce();
  const products = await c.products.list({ limit: opts.limit || 20 });
  let filtered = products;

  if (opts.search) {
    const q = opts.search.toLowerCase();
    filtered = filtered.filter(
      (p: any) =>
        p.name?.toLowerCase().includes(q) ||
        p.description?.toLowerCase().includes(q)
    );
  }

  if (opts.category) {
    const cat = opts.category.toLowerCase();
    filtered = filtered.filter(
      (p: any) =>
        p.category?.toLowerCase() === cat ||
        p.tags?.some((t: string) => t.toLowerCase() === cat)
    );
  }

  return { products: filtered };
}

export async function getProductBySlug(slug: string) {
  const c = getCommerce();
  const products = await c.products.list({ limit: 200 });
  return products.find(
    (p: any) =>
      p.slug === slug ||
      p.name?.toLowerCase().replace(/\s+/g, '-') === slug
  ) || null;
}
