const path = require('path');

/** @type {import('next').NextConfig} */
const nextConfig = {
  output: 'standalone',
  // The design system ships raw .jsx ESM source (single source of truth), so Next
  // must transpile it rather than treat it as a pre-built node_modules package.
  transpilePackages: ['@stateset/design'],
  experimental: {
    serverComponentsExternalPackages: ['@stateset/embedded'],
  },
  env: {
    NEXT_PUBLIC_STATESET_API_URL: process.env.NEXT_PUBLIC_STATESET_API_URL || 'https://api.sandbox.stateset.app',
  },
  webpack: (config, { isServer }) => {
    if (isServer) {
      config.externals.push('@stateset/embedded');
    }
    // @stateset/design is consumed via a file: symlink and ships its own react in
    // node_modules; on the CLIENT, force a single React instance so the transpiled
    // primitives share the host's React — otherwise hooks break. The server keeps
    // Next's own vendored React canary (which the public package lacks, e.g. cache()).
    if (!isServer) {
      config.resolve.alias = {
        ...config.resolve.alias,
        react: path.resolve(__dirname, 'node_modules/react'),
        'react-dom': path.resolve(__dirname, 'node_modules/react-dom'),
        'react/jsx-runtime': path.resolve(__dirname, 'node_modules/react/jsx-runtime'),
        'react/jsx-dev-runtime': path.resolve(__dirname, 'node_modules/react/jsx-dev-runtime'),
      };
    }
    return config;
  },
  async headers() {
    return [
      {
        source: '/(.*)',
        headers: [
          {
            key: 'X-Frame-Options',
            value: 'DENY',
          },
          {
            key: 'X-Content-Type-Options',
            value: 'nosniff',
          },
          {
            key: 'Referrer-Policy',
            value: 'strict-origin-when-cross-origin',
          },
          {
            key: 'X-DNS-Prefetch-Control',
            value: 'on',
          },
          {
            key: 'Strict-Transport-Security',
            value: 'max-age=63072000; includeSubDomains; preload',
          },
          {
            key: 'Permissions-Policy',
            value: 'camera=(), microphone=(), geolocation=(), browsing-topics=()',
          },
          {
            key: 'Cross-Origin-Opener-Policy',
            value: 'same-origin',
          },
          {
            key: 'Cross-Origin-Embedder-Policy',
            value: 'credentialless',
          },
          {
            key: 'X-Permitted-Cross-Domain-Policies',
            value: 'none',
          },
        ],
      },
    ];
  },
};

// Wrap with bundle analyzer when ANALYZE=true
const withBundleAnalyzer =
  process.env.ANALYZE === 'true'
    ? require('@next/bundle-analyzer')({ enabled: true })
    : (config) => config;

module.exports = withBundleAnalyzer(nextConfig);
