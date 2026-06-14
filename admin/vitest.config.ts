import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['./tests/setup.ts'],
    include: ['tests/**/*.test.{ts,tsx}'],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      include: [
        'src/middleware.ts',
        'src/app/api/**/*.{ts,tsx}',
        'src/app/actions/active-org.ts',
        'src/app/actions/organizations.ts',
        'src/lib/shared/**/*.{ts,tsx}',
        'src/lib/stateset-api-url.ts',
        // Design-system components: now under coverage so regressions in
        // variant/className composition fail the build.
        'src/components/ui/button.tsx',
        'src/components/ui/badge.tsx',
        'src/components/ui/card.tsx',
        'src/components/ui/progress.tsx',
        'src/components/ui/loading-skeleton.tsx',
        'src/components/ui/error-boundary.tsx',
        // Audit-log helpers — pure filter + CSV serializer functions.
        'src/components/operations/audit-log-client.tsx',
        // Order CSV helpers — used by the bulk-orders page.
        'src/lib/orders/csv.ts',
        // Generic CSV helpers + canonical column specs for entity exports.
        'src/lib/csv/csv.ts',
        'src/lib/csv/specs.ts',
        // Operational client components with non-trivial action gating
        // and selection state.
        'src/components/returns/rma-inbox-client.tsx',
        'src/components/shared/org-switcher.tsx',
        'src/components/orders/bulk-orders-client.tsx',
        // Export Hub layout — three EntityCards wired to entity-specific
        // CSV server actions. Component-tested for layout/wiring.
        'src/components/export/export-hub-client.tsx',
        // Build & Release page — surfaces the engine's /version
        // endpoint with a sigstore trust badge.
        'src/app/build-info/page.tsx',
        // Operational + analytics dashboard widgets (added with the A+
        // elevation — minimal smoke + behavior tests cover error/data
        // branches; raise the bar over time as they evolve).
        'src/components/agents/agent-performance.tsx',
        'src/components/customers/customer-health-score.tsx',
        'src/components/export/csv-export-button.tsx',
        'src/components/finance/financial-reconciliation.tsx',
        'src/components/finance/roi-calculator.tsx',
        'src/components/gateway/channel-status-card.tsx',
        'src/components/gateway/connection-status.tsx',
        'src/components/inventory/inventory-analytics.tsx',
        'src/components/operations/exception-management.tsx',
        'src/components/orders/order-pipeline.tsx',
        'src/components/returns/returns-management.tsx',
        'src/components/shared/top-bar.tsx',
        'src/components/shared/simulated-data-badge.tsx',
        'src/components/mobile-nav.tsx',
      ],
      exclude: [
        'src/**/*.d.ts',
        'src/**/index.ts',
        'node_modules',
      ],
      thresholds: {
        statements: 80,
        branches: 70,
        functions: 70,
        lines: 80,
      },
    },
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
    },
  },
});
