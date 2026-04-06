'use client';

import { Card, Title, Text, Badge } from '@tremor/react';
import { Cog6ToothIcon, CircleStackIcon, CpuChipIcon, ServerIcon, CheckCircleIcon, SparklesIcon } from '@heroicons/react/24/outline';
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
            <Cog6ToothIcon className="w-8 h-8 text-indigo-600" />
            <Title className="text-2xl">Settings</Title>
          </div>
          <Text className="text-gray-600">
            Configure your StateSet embedded commerce engine
          </Text>
        </div>

        {/* Engine Status */}
        <Card className="mb-6">
          <div className="flex items-center justify-between mb-4">
            <div className="flex items-center space-x-3">
              <CpuChipIcon className="w-6 h-6 text-indigo-600" />
              <Title>Embedded Engine Status</Title>
            </div>
            <Badge color="emerald" icon={CheckCircleIcon}>
              Active
            </Badge>
          </div>

          <div className="grid grid-cols-2 md:grid-cols-4 gap-4">
            <div className="p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
              <Text className="text-sm text-gray-500">Version</Text>
              <Text className="text-lg font-semibold">{APP_VERSION}</Text>
            </div>
            <div className="p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
              <Text className="text-sm text-gray-500">Database</Text>
              <Text className="text-lg font-semibold">SQLite</Text>
            </div>
            <div className="p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
              <Text className="text-sm text-gray-500">Latency</Text>
              <Text className="text-lg font-semibold">{systemHealth?.databaseLatency || 0}ms</Text>
            </div>
            <div className="p-4 bg-gray-50 dark:bg-gray-800 rounded-lg">
              <Text className="text-sm text-gray-500">Error Rate</Text>
              <Text className="text-lg font-semibold">{systemHealth?.errorRate || 0}%</Text>
            </div>
          </div>
        </Card>

        {/* Database Configuration */}
        <Card className="mb-6">
          <div className="flex items-center space-x-3 mb-4">
            <CircleStackIcon className="w-6 h-6 text-indigo-600" />
            <Title>Database Configuration</Title>
          </div>

          <div className="space-y-4">
            <div className="flex items-center justify-between p-4 border rounded-lg dark:border-gray-700">
              <div>
                <Text className="font-medium">Database Type</Text>
                <Text className="text-sm text-gray-500">SQLite (Embedded)</Text>
              </div>
              <Badge color="emerald">Active</Badge>
            </div>

            <div className="flex items-center justify-between p-4 border rounded-lg dark:border-gray-700">
              <div>
                <Text className="font-medium">Database Path</Text>
                <Text className="text-sm text-gray-500 font-mono">./data/admin.db</Text>
              </div>
              <Button variant="outline" size="sm">Change</Button>
            </div>

            <div className="flex items-center justify-between p-4 border rounded-lg dark:border-gray-700">
              <div>
                <Text className="font-medium">Active Connections</Text>
                <Text className="text-sm text-gray-500">{systemHealth?.activeConnections || 1} connection(s)</Text>
              </div>
              <Badge color="blue">Healthy</Badge>
            </div>

            <div className="p-4 bg-blue-50 dark:bg-blue-900/20 border border-blue-200 dark:border-blue-800 rounded-lg">
              <div className="flex items-center space-x-2 mb-2">
                <ServerIcon className="w-5 h-5 text-blue-600" />
                <Text className="font-medium text-blue-800 dark:text-blue-200">PostgreSQL Support</Text>
              </div>
              <Text className="text-sm text-blue-600 dark:text-blue-300 mb-3">
                Switch to PostgreSQL for production deployments with high availability and multi-instance support.
              </Text>
              <Button variant="outline" size="sm">Configure PostgreSQL</Button>
            </div>
          </div>
        </Card>

        {/* Embedded Features */}
        <Card className="mb-6">
          <div className="flex items-center space-x-3 mb-4">
            <SparklesIcon className="w-6 h-6 text-indigo-600" />
            <Title>Embedded Features</Title>
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
              <div key={feature.name} className="flex items-center justify-between p-3 border rounded-lg dark:border-gray-700">
                <div>
                  <Text className="font-medium">{feature.name}</Text>
                  <Text className="text-sm text-gray-500">{feature.description}</Text>
                </div>
                <Badge color={feature.status === 'enabled' ? 'emerald' : 'gray'}>
                  {feature.status}
                </Badge>
              </div>
            ))}
          </div>
        </Card>

        {/* API & Bindings */}
        <Card>
          <div className="flex items-center space-x-3 mb-4">
            <CpuChipIcon className="w-6 h-6 text-indigo-600" />
            <Title>Language Bindings</Title>
          </div>

          <Text className="text-gray-500 mb-4">
            StateSet embedded commerce engine is available in multiple languages
          </Text>

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
              <div key={binding.name} className="p-3 border rounded-lg dark:border-gray-700">
                <Text className="font-medium">{binding.name}</Text>
                <Text className="text-xs text-gray-500 font-mono">{binding.package}</Text>
              </div>
            ))}
          </div>
        </Card>
      </motion.div>
    </ErrorBoundary>
  );
}
