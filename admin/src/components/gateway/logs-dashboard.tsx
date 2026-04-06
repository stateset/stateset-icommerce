'use client';

import { Title, Text } from '@tremor/react';
import { motion } from 'framer-motion';
import { LiveLogViewer } from './live-log-viewer';

export default function LogsDashboard() {
  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.5 }}
      className="space-y-6"
    >
      <div>
        <Title className="text-2xl">Logs</Title>
        <Text className="text-gray-500">
          Real-time gateway activity derived from metrics polling
        </Text>
      </div>

      <LiveLogViewer />
    </motion.div>
  );
}
