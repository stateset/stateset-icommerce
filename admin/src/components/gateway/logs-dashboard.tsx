'use client';

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
        <h3 className="font-ds-display text-2xl font-semibold text-ds-foreground">Logs</h3>
        <p className="text-sm text-ds-muted-foreground">
          Real-time gateway activity derived from metrics polling
        </p>
      </div>

      <LiveLogViewer />
    </motion.div>
  );
}
