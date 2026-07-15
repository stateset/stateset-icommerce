'use client';

import { memo } from 'react';
import {
  Card,
  CardContent,
  MetricCard,
  Badge,
  StatusPill,
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from '@stateset/design';
import { BarChart } from '@tremor/react';
import { CubeIcon, PhotoIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getProducts } from '@/app/actions/commerce';
import { formatCurrency, formatNumber } from '@/lib/utils';
import type {
  ProductCatalogData,
  ProductCategoryDistribution,
  TopProduct,
} from '@/lib/types/dashboard-data';
import type { Product } from '@/lib/types';

interface ProductCatalogProps {
  data?: ProductCatalogData;
}

type StatusPillStatus = 'ok' | 'run' | 'warn' | 'fail' | 'review' | 'idle';

const statusPills: Record<string, StatusPillStatus> = {
  active: 'ok',
  draft: 'idle',
  archived: 'warn',
  out_of_stock: 'fail',
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
        <CardContent>
          <div className="animate-pulse space-y-4">
            <div className="h-6 bg-ds-muted rounded w-48" />
            <div className="h-64 bg-ds-muted rounded" />
          </div>
        </CardContent>
      </Card>
    );
  }

  if (error && !propData) {
    return (
      <Card className="border-ds-status-fail/30">
        <CardContent>
          <p className="text-sm text-ds-status-fail">Failed to load product catalog</p>
        </CardContent>
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
      <div className="grid grid-cols-2 sm:grid-cols-4 gap-4">
        <MetricCard label="Total Products" value={formatNumber(summary.totalProducts)} tone="primary" />
        <MetricCard label="Active" value={formatNumber(summary.activeProducts)} tone="success" />
        <MetricCard label="Low Stock" value={summary.lowStockProducts} tone="warning" />
        <MetricCard label="Avg Price" value={formatCurrency(summary.avgPrice)} tone="accent" />
      </div>

      {/* Category Distribution */}
      <Card>
        <CardContent>
          <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Products by Category</h3>
          <p className="text-sm text-ds-muted-foreground mb-4">Count and inventory value by category</p>
          {categoryDistribution.length > 0 ? (
            <BarChart
              className="h-64"
              data={categoryDistribution}
              index="category"
              categories={['count', 'inventoryValue']}
              colors={['indigo', 'emerald']}
              showAnimation
              valueFormatter={(value) => formatNumber(value)}
            />
          ) : (
            <div className="flex h-64 items-center justify-center rounded-lg border border-dashed border-ds-enterprise-line">
              <p className="text-sm text-ds-muted-foreground">No category coverage is available until products are synced.</p>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Product Grid */}
      <Card>
        <CardContent>
          <div className="flex items-center justify-between mb-4">
            <div>
              <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Product Catalog</h3>
              <p className="text-sm text-ds-muted-foreground">All products in your inventory</p>
            </div>
            <Badge variant="primary">
              {formatNumber(summary.totalProducts)} products
            </Badge>
          </div>

          {displayProducts.length > 0 ? (
            <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-4">
              {displayProducts.map((product: TopProduct, index: number) => (
                <motion.div
                  key={product.id || index}
                  initial={{ opacity: 0, scale: 0.95 }}
                  animate={{ opacity: 1, scale: 1 }}
                  transition={{ delay: index * 0.05 }}
                  className="p-4 border border-ds-enterprise-line rounded-lg hover:border-ds-brand-300 transition-colors"
                >
                  <div className="flex items-start justify-between mb-3">
                    <div className="w-12 h-12 rounded-lg bg-ds-muted flex items-center justify-center">
                      {product.imageUrl ? (
                        // eslint-disable-next-line @next/next/no-img-element
                        <img src={product.imageUrl} alt={product.name} className="w-full h-full object-cover rounded-lg" />
                      ) : (
                        <PhotoIcon className="w-6 h-6 text-ds-muted-foreground" />
                      )}
                    </div>
                    <StatusPill status={statusPills[product.status] || 'idle'}>
                      {product.status}
                    </StatusPill>
                  </div>

                  <p className="text-sm font-medium text-ds-foreground">{product.name}</p>
                  <p className="text-xs text-ds-muted-foreground mt-1">{product.sku || `SKU-${product.id?.slice(0, 8)}`}</p>

                  <div className="flex items-center justify-between mt-3">
                    <div>
                      <p className="text-lg font-bold text-ds-foreground">{formatCurrency(product.price)}</p>
                      {product.compareAtPrice && (
                        <p className="text-xs text-ds-muted-foreground line-through">
                          {formatCurrency(product.compareAtPrice)}
                        </p>
                      )}
                    </div>
                    <div className="text-right">
                      <div className="flex items-center space-x-1">
                        <CubeIcon className="w-4 h-4 text-ds-muted-foreground" />
                        <p className="text-sm text-ds-foreground">{product.inventory || 0} in stock</p>
                      </div>
                    </div>
                  </div>

                  {product.categories && product.categories.length > 0 && (
                    <div className="flex flex-wrap gap-1 mt-2">
                      {product.categories.slice(0, 2).map((cat: string, i: number) => (
                        <Badge key={i} variant="outline">{cat}</Badge>
                      ))}
                    </div>
                  )}
                </motion.div>
              ))}
            </div>
          ) : (
            <div className="rounded-lg border border-dashed border-ds-enterprise-line p-8 text-center">
              <h3 className="font-ds-display text-base font-semibold text-ds-foreground">No products found</h3>
              <p className="mt-2 text-sm text-ds-muted-foreground">
                No products have been synced into the embedded catalog yet.
              </p>
            </div>
          )}
        </CardContent>
      </Card>

      {/* Inventory Snapshot */}
      <Card>
        <CardContent>
          <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Inventory Snapshot</h3>
          <p className="text-sm text-ds-muted-foreground mb-4">Products ranked by current inventory value</p>
          {topProducts.length > 0 ? (
            <div className="overflow-x-auto">
              <Table>
                <TableHeader>
                  <TableRow>
                    <TableHead>Product</TableHead>
                    <TableHead>Category</TableHead>
                    <TableHead className="text-right">Price</TableHead>
                    <TableHead className="text-right">In Stock</TableHead>
                    <TableHead className="text-right">Inventory Value</TableHead>
                  </TableRow>
                </TableHeader>
                <TableBody>
                  {topProducts.slice(0, 5).map((product: TopProduct, index: number) => (
                    <TableRow key={product.id || index}>
                      <TableCell>
                        <div className="flex items-center space-x-3">
                          <div className="w-8 h-8 rounded bg-ds-muted flex items-center justify-center">
                            <CubeIcon className="w-4 h-4 text-ds-muted-foreground" />
                          </div>
                          <div>
                            <p className="text-sm font-medium text-ds-foreground">{product.name}</p>
                            <p className="text-xs text-ds-muted-foreground">{product.sku}</p>
                          </div>
                        </div>
                      </TableCell>
                      <TableCell>
                        <Badge variant="outline">{product.category}</Badge>
                      </TableCell>
                      <TableCell tone="numeric" className="font-medium">{formatCurrency(product.price)}</TableCell>
                      <TableCell tone="numeric">{formatNumber(product.inventory || 0)}</TableCell>
                      <TableCell tone="numeric" className="font-medium text-ds-status-ok">
                        {formatCurrency((product.inventory || 0) * product.price)}
                      </TableCell>
                    </TableRow>
                  ))}
                </TableBody>
              </Table>
            </div>
          ) : (
            <div className="rounded-lg border border-dashed border-ds-enterprise-line p-6 text-center">
              <p className="text-sm text-ds-muted-foreground">Inventory rankings will appear after products are synced.</p>
            </div>
          )}
        </CardContent>
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
