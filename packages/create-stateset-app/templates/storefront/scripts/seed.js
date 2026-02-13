const { Commerce } = require('@stateset/embedded');

async function seed() {
  const commerce = new Commerce(process.env.DATABASE_PATH || './store.db');
  console.log('Seeding database with sample products...\n');

  const products = [
    { name: 'Classic T-Shirt', slug: 'classic-t-shirt', price: 29.99, category: 'apparel', description: 'A comfortable everyday essential in premium cotton.' },
    { name: 'Premium Hoodie', slug: 'premium-hoodie', price: 79.99, category: 'apparel', description: 'Heavyweight fleece hoodie for cooler days.' },
    { name: 'Canvas Sneakers', slug: 'canvas-sneakers', price: 59.99, category: 'footwear', description: 'Timeless canvas sneakers with rubber sole.' },
    { name: 'Leather Wallet', slug: 'leather-wallet', price: 49.99, category: 'accessories', description: 'Full-grain leather bifold wallet.' },
    { name: 'Wireless Earbuds', slug: 'wireless-earbuds', price: 99.99, category: 'electronics', description: 'True wireless earbuds with noise cancellation.' },
    { name: 'Smart Watch', slug: 'smart-watch', price: 199.99, category: 'electronics', description: 'Fitness tracking smartwatch with heart rate monitor.' },
    { name: 'Backpack', slug: 'backpack', price: 89.99, category: 'accessories', description: 'Water-resistant backpack with laptop compartment.' },
    { name: 'Sunglasses', slug: 'sunglasses', price: 149.99, category: 'accessories', description: 'Polarized UV400 sunglasses with titanium frame.' },
    { name: 'Water Bottle', slug: 'water-bottle', price: 24.99, category: 'lifestyle', description: 'Insulated stainless steel bottle, keeps drinks cold 24hrs.' },
    { name: 'Notebook Set', slug: 'notebook-set', price: 19.99, category: 'lifestyle', description: 'Set of 3 ruled notebooks, 120 pages each.' },
  ];

  for (const product of products) {
    try {
      const sku = product.slug.toUpperCase();
      await commerce.products.create({
        name: product.name,
        slug: product.slug,
        description: product.description,
        category: product.category,
        variants: [{ sku, name: 'Default', price: product.price, isDefault: true }],
      });
      await commerce.inventory.createItem({
        sku,
        name: product.name,
        initialQuantity: 100,
      });
      console.log(`  + ${product.name} ($${product.price})`);
    } catch (err) {
      if (err.message?.includes('UNIQUE constraint')) {
        console.log(`  ~ ${product.name} (already exists)`);
      } else {
        console.error(`  ! ${product.name} - ${err.message}`);
      }
    }
  }

  // Seed tax jurisdictions for US states
  try {
    const taxStates = [
      { code: 'CA', name: 'California', rate: 0.0725 },
      { code: 'NY', name: 'New York', rate: 0.08 },
      { code: 'TX', name: 'Texas', rate: 0.0625 },
      { code: 'FL', name: 'Florida', rate: 0.06 },
      { code: 'WA', name: 'Washington', rate: 0.065 },
    ];

    for (const state of taxStates) {
      try {
        await commerce.tax.createJurisdiction({
          code: state.code,
          name: state.name,
          country: 'US',
          rate: state.rate,
          type: 'state',
        });
        console.log(`  + Tax: ${state.name} (${(state.rate * 100).toFixed(2)}%)`);
      } catch {
        // Jurisdiction may already exist
      }
    }
  } catch {
    console.log('  ~ Tax jurisdictions: skipped (may already exist)');
  }

  console.log('\nDone! Run `npm run dev` to start your store.');
}

seed().catch(console.error);
