import { Commerce } from '@stateset/embedded';
import { loadStoreEnvironment } from '../lib/store-environment.js';

async function seed() {
  await loadStoreEnvironment({ development: process.env.NODE_ENV !== 'production' });
  const commerce = new Commerce(process.env.STATESET_DB_PATH || './store.db');
  console.log('Seeding database with sample products...\n');
  try {
    const products = [
      {
        name: 'Classic T-Shirt',
        slug: 'classic-t-shirt',
        price: '29.99',
        category: 'apparel',
        description: 'A comfortable everyday essential in premium cotton.',
      },
      {
        name: 'Premium Hoodie',
        slug: 'premium-hoodie',
        price: '79.99',
        category: 'apparel',
        description: 'Heavyweight fleece hoodie for cooler days.',
      },
      {
        name: 'Canvas Sneakers',
        slug: 'canvas-sneakers',
        price: '59.99',
        category: 'footwear',
        description: 'Timeless canvas sneakers with rubber sole.',
      },
      {
        name: 'Leather Wallet',
        slug: 'leather-wallet',
        price: '49.99',
        category: 'accessories',
        description: 'Full-grain leather bifold wallet.',
      },
      {
        name: 'Wireless Earbuds',
        slug: 'wireless-earbuds',
        price: '99.99',
        category: 'electronics',
        description: 'True wireless earbuds with noise cancellation.',
      },
      {
        name: 'Smart Watch',
        slug: 'smart-watch',
        price: '199.99',
        category: 'electronics',
        description: 'Fitness tracking smartwatch with heart rate monitor.',
      },
      {
        name: 'Backpack',
        slug: 'backpack',
        price: '89.99',
        category: 'accessories',
        description: 'Water-resistant backpack with laptop compartment.',
      },
      {
        name: 'Sunglasses',
        slug: 'sunglasses',
        price: '149.99',
        category: 'accessories',
        description: 'Polarized UV400 sunglasses with titanium frame.',
      },
      {
        name: 'Water Bottle',
        slug: 'water-bottle',
        price: '24.99',
        category: 'lifestyle',
        description: 'Insulated stainless steel bottle, keeps drinks cold 24hrs.',
      },
      {
        name: 'Notebook Set',
        slug: 'notebook-set',
        price: '19.99',
        category: 'lifestyle',
        description: 'Set of 3 ruled notebooks, 120 pages each.',
      },
    ];

    let failures = 0;
    for (const product of products) {
      try {
        const sku = product.slug.toUpperCase();
        const existing = await commerce.products.getBySlug(product.slug);
        const storedProduct =
          existing ||
          (await commerce.products.create({
            name: product.name,
            slug: product.slug,
            description: product.description,
            category: product.category,
            variants: [{ sku, name: 'Default', priceExact: product.price, isDefault: true }],
          }));
        if (!existing) await commerce.products.activate(storedProduct.id);
        const variant = await commerce.products.getVariantBySku(sku);
        if (!variant || variant.productId !== storedProduct.id) {
          throw new Error(
            'Existing sample product does not own its expected SKU; resolve the catalog conflict before retrying.',
          );
        }
        const stock = await commerce.inventory.getStock(sku);
        if (!stock)
          await commerce.inventory.createItem({
            sku,
            name: product.name,
            initialQuantity: 100,
          });
        console.log(`  ${existing ? '~' : '+'} ${product.name} ($${product.price})`);
      } catch (err) {
        failures += 1;
        console.error(`  ! ${product.name} - ${err.message}`);
      }
    }

    if (failures)
      throw new Error(`Seeding failed for ${failures} product(s); fix the errors and retry.`);
    console.log('\nDone! Run `npm run doctor` to check your store configuration.');
  } finally {
    await commerce.close();
  }
}

seed().catch((error) => {
  console.error(error.message);
  process.exitCode = 1;
});
