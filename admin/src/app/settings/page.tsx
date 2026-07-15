'use client';

import { Card, CardContent, StatusPill } from '@stateset/design';
import { Cog6ToothIcon, CircleStackIcon, CpuChipIcon, ServerIcon, SparklesIcon } from '@heroicons/react/24/outline';
import { motion } from 'framer-motion';
import { Button } from '@/components/ui/button';
import { ErrorBoundary } from '@/components/ui/error-boundary';
import { useEmbeddedData } from '@/hooks/use-embedded-data';
import { getSystemHealth } from '@/app/actions/commerce';
import type { SystemHealth } from '@/lib/types';
import { APP_VERSION } from '@/lib/version';

export default function SettingsPage() {
  const { data: systemHealth } = useEmbeddedData<SystemHealth>(
    () => getSystemHealth(),
    { refreshInterval: 10000 }
  );

  return (
    <ErrorBoundary>
      <motion.div
        initial={{ opacity: 0, y: 20 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.5 }}
      >
        <div className="mb-6">
          <div className="flex items-center space-x-2 mb-2">
            <Cog6ToothIcon className="w-8 h-8 text-ds-primary" />
            <h3 className="font-ds-display text-2xl font-semibold text-ds-foreground">Settings</h3>
          </div>
          <p className="text-sm text-ds-muted-foreground">
            Configure your StateSet embedded commerce engine
          </p>
        </div>

        {/* Engine Status */}
        <Card className="mb-6">
          <CardContent className="p-5">
            <div className="flex items-center justify-between mb-4">
              <div className="flex items-center space-x-3">
                <CpuChipIcon className="w-6 h-6 text-ds-primary" />
                <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Embedded Engine Status</h3>
              </div>
              <StatusPill status="ok">Active</StatusPill>
            </div>

            <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
              <div className="p-4 bg-ds-muted rounded-lg">
                <p className="text-sm text-ds-muted-foreground">Version</p>
                <p className="text-lg font-semibold text-ds-foreground">{APP_VERSION}</p>
              </div>
              <div className="p-4 bg-ds-muted rounded-lg">
                <p className="text-sm text-ds-muted-foreground">Database</p>
                <p className="text-lg font-semibold text-ds-foreground">SQLite</p>
              </div>
              <div className="p-4 bg-ds-muted rounded-lg">
                <p className="text-sm text-ds-muted-foreground">Latency</p>
                <p className="text-lg font-semibold text-ds-foreground">{systemHealth?.databaseLatency || 0}ms</p>
              </div>
              <div className="p-4 bg-ds-muted rounded-lg">
                <p className="text-sm text-ds-muted-foreground">Error Rate</p>
                <p className="text-lg font-semibold text-ds-foreground">{systemHealth?.errorRate || 0}%</p>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Database Configuration */}
        <Card className="mb-6">
          <CardContent className="p-5">
            <div className="flex items-center space-x-3 mb-4">
              <CircleStackIcon className="w-6 h-6 text-ds-primary" />
              <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Database Configuration</h3>
            </div>

            <div className="space-y-4">
              <div className="flex items-center justify-between p-4 border border-ds-enterprise-line rounded-lg">
                <div>
                  <p className="text-sm font-medium text-ds-foreground">Database Type</p>
                  <p className="text-sm text-ds-muted-foreground">SQLite (Embedded)</p>
                </div>
                <StatusPill status="ok">Active</StatusPill>
              </div>

              <div className="flex items-center justify-between p-4 border border-ds-enterprise-line rounded-lg">
                <div>
                  <p className="text-sm font-medium text-ds-foreground">Database Path</p>
                  <p className="text-sm text-ds-muted-foreground font-mono">./data/admin.db</p>
                </div>
                <Button variant="outline" size="sm">Change</Button>
              </div>

              <div className="flex items-center justify-between p-4 border border-ds-enterprise-line rounded-lg">
                <div>
                  <p className="text-sm font-medium text-ds-foreground">Active Connections</p>
                  <p className="text-sm text-ds-muted-foreground">{systemHealth?.activeConnections || 1} connection(s)</p>
                </div>
                <StatusPill status="ok">Healthy</StatusPill>
              </div>

              <div className="p-4 bg-ds-info/10 border border-ds-info/25 rounded-lg">
                <div className="flex items-center space-x-2 mb-2">
                  <ServerIcon className="w-5 h-5 text-ds-info" />
                  <p className="text-sm font-medium text-ds-foreground">PostgreSQL Support</p>
                </div>
                <p className="text-sm text-ds-muted-foreground mb-3">
                  Switch to PostgreSQL for production deployments with high availability and multi-instance support.
                </p>
                <Button variant="outline" size="sm">Configure PostgreSQL</Button>
              </div>
            </div>
          </CardContent>
        </Card>

        {/* Embedded Features */}
        <Card className="mb-6">
          <CardContent className="p-5">
            <div className="flex items-center space-x-3 mb-4">
              <SparklesIcon className="w-6 h-6 text-ds-primary" />
              <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Embedded Features</h3>
            </div>

            <div className="space-y-3">
              {[
              { name: 'Orders Management', status: 'enabled', description: 'Full order lifecycle management' },
              { name: 'Inventory Tracking', status: 'enabled', description: 'Real-time inventory with reservations' },
              { name: 'Returns Processing', status: 'enabled', description: 'RMA workflow automation' },
              { name: 'Customer Management', status: 'enabled', description: 'Customer profiles and segmentation' },
              { name: 'Subscriptions', status: 'enabled', description: 'Recurring billing management' },
              { name: 'Analytics Engine', status: 'enabled', description: 'Built-in forecasting and metrics' },
              { name: 'Promotions', status: 'enabled', description: 'Discount codes and campaigns' },
              { name: 'Tax Calculation', status: 'enabled', description: 'Automatic tax computation' },
              ].map((feature) => (
                <div key={feature.name} className="flex items-center justify-between p-3 border border-ds-enterprise-line rounded-lg">
                  <div>
                    <p className="text-sm font-medium text-ds-foreground">{feature.name}</p>
                    <p className="text-sm text-ds-muted-foreground">{feature.description}</p>
                  </div>
                  <StatusPill status={feature.status === 'enabled' ? 'ok' : 'idle'}>
                    {feature.status}
                  </StatusPill>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>

        {/* API & Bindings */}
        <Card>
          <CardContent className="p-5">
            <div className="flex items-center space-x-3 mb-4">
              <CpuChipIcon className="w-6 h-6 text-ds-primary" />
              <h3 className="font-ds-display text-base font-semibold text-ds-foreground">Language Bindings</h3>
            </div>

            <p className="text-sm text-ds-muted-foreground mb-4">
              StateSet embedded commerce engine is available in multiple languages
            </p>

            <div className="grid grid-cols-2 md:grid-cols-4 gap-3">
              {[
              { name: 'Node.js', package: '@stateset/embedded' },
              { name: 'Python', package: 'stateset-embedded' },
              { name: 'Ruby', package: 'stateset-embedded' },
              { name: 'PHP', package: 'stateset/embedded' },
              { name: 'Java', package: 'com.stateset:embedded' },
              { name: 'Rust', package: 'stateset-embedded' },
              { name: 'WASM', package: '@stateset/wasm' },
              { name: 'CLI', package: 'stateset-cli' },
              ].map((binding) => (
                <div key={binding.name} className="p-3 border border-ds-enterprise-line rounded-lg">
                  <p className="text-sm font-medium text-ds-foreground">{binding.name}</p>
                  <p className="text-xs text-ds-muted-foreground font-mono">{binding.package}</p>
                </div>
              ))}
            </div>
          </CardContent>
        </Card>
      </motion.div>
    </ErrorBoundary>
  );
}
