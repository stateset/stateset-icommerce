import { Commerce, type ProductOutput } from '@stateset/embedded';

let commerce: Commerce | null = null;

export function getCommerce() {
  if (!commerce) {
    commerce = new Commerce(process.env.STATESET_DB_PATH || './store.db');
  }
  return commerce;
}

async function* activeProducts(pageSize: number) {
  let afterCursor: string[] | undefined;
  while (true) {
    const page = await getCommerce().products.list({ status: 'active', limit: pageSize, afterCursor });
    yield* page;
    if (page.length < pageSize) return;
    const last = page[page.length - 1];
    afterCursor = [last.name, last.id];
  }
}

async function withVariants(product: ProductOutput) {
  const variants = await getCommerce().products.getVariants(product.id);
  return { ...product, variants };
}

export async function getProducts(
  opts: {
    search?: string;
    category?: string;
    limit?: number;
  } = {},
) {
  const limit = opts.limit || 20;
  const query = opts.search?.toLowerCase();
  const category = opts.category?.toLowerCase();
  const matches: ProductOutput[] = [];
  for await (const product of activeProducts(query || category ? 100 : limit)) {
    if (category && product.category?.toLowerCase() !== category) continue;
    if (query && !product.name.toLowerCase().includes(query) && !product.description.toLowerCase().includes(query)) continue;
    matches.push(product);
    if (matches.length === limit) break;
  }
  return { products: await Promise.all(matches.map(withVariants)) };
}

export async function getProductBySlug(slug: string) {
  const product = await getCommerce().products.getBySlug(slug);
  if (product) return product.status === 'active' ? withVariants(product) : null;
  // Preserve name-based links from earlier storefronts with custom product slugs.
  for await (const candidate of activeProducts(100)) {
    if (candidate.name.toLowerCase().replace(/\s+/g, '-') === slug) return withVariants(candidate);
  }
  return null;
}
