'use client';

import { memo } from 'react';
import { Card, Title, Text, Badge, Grid, Metric, BarChart } from '@tremor/react';
import { CubeIcon, PhotoIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getProducts } from '@/app/actions/commerce';
import { formatCurrency, formatNumber } from '@/lib/utils';
import type {
  ProductCatalogData,
  ProductCategoryDistribution,
  TopProduct,
  TremorColor,
} from '@/lib/types/dashboard-data';
import type { Product } from '@/lib/types';

interface ProductCatalogProps {
  data?: ProductCatalogData;
}

const statusColors: Record<string, string> = {
  active: 'emerald',
  draft: 'gray',
  archived: 'amber',
  out_of_stock: 'red',
};

function ProductCatalogInner({ data: propData }: ProductCatalogProps) {
  const { data, isLoading, error } = useEmbeddedData(
    () => getProducts(),
    { refreshInterval: 60000 }
  );

  const products: Product[] = data || [];
  const catalogData = propData || buildCatalogData(products);

  if (isLoading && !products.length && !propData) {
    return (
      <Card>
        <div className="animate-pulse space-y-4">
          <div className="h-6 bg-gray-200 rounded w-48" />
          <div className="h-64 bg-gray-200 rounded" />
        </div>
      </Card>
    );
  }

  if (error && !propData) {
    return (
      <Card className="border-red-200">
        <Text className="text-red-600">Failed to load product catalog</Text>
      </Card>
    );
  }

  const { summary, categoryDistribution, topProducts } = catalogData;
  const displayProducts: TopProduct[] = products.length > 0
    ? products.slice(0, 9).map(toTopProduct)
    : topProducts;

  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      className="space-y-6"
    >
      {/* Key Metrics */}
      <Grid numItems={2} numItemsSm={4} className="gap-4">
        <Card decoration="top" decorationColor="blue">
          <Text>Total Products</Text>
          <Metric>{formatNumber(summary.totalProducts)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="emerald">
          <Text>Active</Text>
          <Metric>{formatNumber(summary.activeProducts)}</Metric>
        </Card>
        <Card decoration="top" decorationColor="amber">
          <Text>Low Stock</Text>
          <Metric>{summary.lowStockProducts}</Metric>
        </Card>
        <Card decoration="top" decorationColor="purple">
          <Text>Avg Price</Text>
          <Metric>{formatCurrency(summary.avgPrice)}</Metric>
        </Card>
      </Grid>

      {/* Category Distribution */}
      <Card>
        <Title>Products by Category</Title>
        <Text className="text-gray-500 mb-4">Count and inventory value by category</Text>
        {categoryDistribution.length > 0 ? (
          <BarChart
            className="h-64"
            data={categoryDistribution}
            index="category"
            categories={['count', 'inventoryValue']}
            colors={['blue', 'emerald']}
            showAnimation
            valueFormatter={(value) => formatNumber(value)}
          />
        ) : (
          <div className="flex h-64 items-center justify-center rounded-lg border border-dashed border-gray-200 dark:border-gray-700">
            <Text className="text-gray-500">No category coverage is available until products are synced.</Text>
          </div>
        )}
      </Card>

      {/* Product Grid */}
      <Card>
        <div className="flex items-center justify-between mb-4">
          <div>
            <Title>Product Catalog</Title>
            <Text className="text-gray-500">All products in your inventory</Text>
          </div>
          <Badge color="blue" size="lg">
            {formatNumber(summary.totalProducts)} products
          </Badge>
        </div>

        {displayProducts.length > 0 ? (
          <Grid numItems={1} numItemsSm={2} numItemsLg={3} className="gap-4">
            {displayProducts.map((product: TopProduct, index: number) => (
              <motion.div
                key={product.id || index}
                initial={{ opacity: 0, scale: 0.95 }}
                animate={{ opacity: 1, scale: 1 }}
                transition={{ delay: index * 0.05 }}
                className="p-4 border rounded-lg dark:border-gray-700 hover:border-indigo-300 dark:hover:border-indigo-700 transition-colors"
              >
                <div className="flex items-start justify-between mb-3">
                  <div className="w-12 h-12 rounded-lg bg-gray-100 dark:bg-gray-800 flex items-center justify-center">
                    {product.imageUrl ? (
                      // eslint-disable-next-line @next/next/no-img-element
                      <img src={product.imageUrl} alt={product.name} className="w-full h-full object-cover rounded-lg" />
                    ) : (
                      <PhotoIcon className="w-6 h-6 text-gray-400" />
                    )}
                  </div>
                  <Badge color={statusColors[product.status] as TremorColor || 'gray'} size="xs">
                    {product.status}
                  </Badge>
                </div>

                <Text className="font-medium">{product.name}</Text>
                <Text className="text-xs text-gray-500 mt-1">{product.sku || `SKU-${product.id?.slice(0, 8)}`}</Text>

                <div className="flex items-center justify-between mt-3">
                  <div>
                    <Text className="text-lg font-bold">{formatCurrency(product.price)}</Text>
                    {product.compareAtPrice && (
                      <Text className="text-xs text-gray-500 line-through">
                        {formatCurrency(product.compareAtPrice)}
                      </Text>
                    )}
                  </div>
                  <div className="text-right">
                    <div className="flex items-center space-x-1">
                      <CubeIcon className="w-4 h-4 text-gray-400" />
                      <Text className="text-sm">{product.inventory || 0} in stock</Text>
                    </div>
                  </div>
                </div>

                {product.categories && product.categories.length > 0 && (
                  <div className="flex flex-wrap gap-1 mt-2">
                    {product.categories.slice(0, 2).map((cat: string, i: number) => (
                      <Badge key={i} color="gray" size="xs">{cat}</Badge>
                    ))}
                  </div>
                )}
              </motion.div>
            ))}
          </Grid>
        ) : (
          <div className="rounded-lg border border-dashed border-gray-200 p-8 text-center dark:border-gray-700">
            <Title>No products found</Title>
            <Text className="mt-2 text-gray-500">
              No products have been synced into the embedded catalog yet.
            </Text>
          </div>
        )}
      </Card>

      {/* Inventory Snapshot */}
      <Card>
        <Title>Inventory Snapshot</Title>
        <Text className="text-gray-500 mb-4">Products ranked by current inventory value</Text>
        {topProducts.length > 0 ? (
          <div className="overflow-x-auto">
            <table className="w-full">
              <thead>
                <tr className="border-b dark:border-gray-700">
                  <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Product</th>
                  <th className="text-left py-2 px-3 text-sm font-medium text-gray-500">Category</th>
                  <th className="text-right py-2 px-3 text-sm font-medium text-gray-500">Price</th>
                  <th className="text-right py-2 px-3 text-sm font-medium text-gray-500">In Stock</th>
                  <th className="text-right py-2 px-3 text-sm font-medium text-gray-500">Inventory Value</th>
                </tr>
              </thead>
              <tbody>
                {topProducts.slice(0, 5).map((product: TopProduct, index: number) => (
                  <motion.tr
                    key={product.id || index}
                    initial={{ opacity: 0 }}
                    animate={{ opacity: 1 }}
                    transition={{ delay: index * 0.03 }}
                    className="border-b dark:border-gray-700"
                  >
                    <td className="py-3 px-3">
                      <div className="flex items-center space-x-3">
                        <div className="w-8 h-8 rounded bg-gray-100 dark:bg-gray-800 flex items-center justify-center">
                          <CubeIcon className="w-4 h-4 text-gray-400" />
                        </div>
                        <div>
                          <Text className="font-medium">{product.name}</Text>
                          <Text className="text-xs text-gray-500">{product.sku}</Text>
                        </div>
                      </div>
                    </td>
                    <td className="py-3 px-3">
                      <Badge color="gray" size="xs">{product.category}</Badge>
                    </td>
                    <td className="py-3 px-3 text-right font-medium">{formatCurrency(product.price)}</td>
                    <td className="py-3 px-3 text-right">{formatNumber(product.inventory || 0)}</td>
                    <td className="py-3 px-3 text-right font-medium text-emerald-600">
                      {formatCurrency((product.inventory || 0) * product.price)}
                    </td>
                  </motion.tr>
                ))}
              </tbody>
            </table>
          </div>
        ) : (
          <div className="rounded-lg border border-dashed border-gray-200 p-6 text-center dark:border-gray-700">
            <Text className="text-gray-500">Inventory rankings will appear after products are synced.</Text>
          </div>
        )}
      </Card>
    </motion.div>
  );
}

function buildCatalogData(products: Product[]): ProductCatalogData {
  const topProducts = products.map(toTopProduct);

  if (topProducts.length === 0) {
    return {
      summary: {
        totalProducts: 0,
        activeProducts: 0,
        lowStockProducts: 0,
        avgPrice: 0,
      },
      categoryDistribution: [],
      topProducts: [],
      products: [],
    };
  }

  const categoryDistributionMap = new Map<string, ProductCategoryDistribution>();

  for (const product of topProducts) {
    const category = product.category || 'Uncategorized';
    const current = categoryDistributionMap.get(category) || {
      category,
      count: 0,
      inventoryValue: 0,
    };

    current.count += 1;
    current.inventoryValue += (product.inventory || 0) * product.price;
    categoryDistributionMap.set(category, current);
  }

  return {
    summary: {
      totalProducts: topProducts.length,
      activeProducts: topProducts.filter((product) => product.status === 'active').length,
      lowStockProducts: topProducts.filter((product) => (product.inventory || 0) > 0 && (product.inventory || 0) <= 10).length,
      avgPrice: roundAverage(topProducts.map((product) => product.price)),
    },
    categoryDistribution: Array.from(categoryDistributionMap.values()).sort((a, b) => b.count - a.count),
    topProducts: [...topProducts]
      .sort((a, b) => (b.inventory || 0) * b.price - (a.inventory || 0) * a.price)
      .slice(0, 9),
    products: topProducts,
  };
}

function roundAverage(values: number[]): number {
  if (values.length === 0) {
    return 0;
  }

  const total = values.reduce((sum, value) => sum + value, 0);
  return Number((total / values.length).toFixed(2));
}

function toTopProduct(product: Product): TopProduct {
  const inventory = (product.variants || []).reduce(
    (total, variant) => total + (variant.inventoryQuantity || 0),
    0,
  );
  return {
    id: product.id,
    name: product.name,
    sku: product.sku,
    category: product.category || 'Uncategorized',
    price: product.price,
    inventory,
    unitsSold: 0,
    revenue: 0,
    status: product.status,
    compareAtPrice: product.compareAtPrice,
    categories: [product.category, ...(product.tags || [])].filter(Boolean) as string[],
  };
}

const ProductCatalog = memo(ProductCatalogInner);
export default ProductCatalog;
