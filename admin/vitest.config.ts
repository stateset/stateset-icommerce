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
        'src/lib/shared/**/*.{ts,tsx}',
        'src/lib/stateset-api-url.ts',
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
