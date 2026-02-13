/** @type {import('next').NextConfig} */
const nextConfig = {
  output: 'standalone',
  images: {
    remotePatterns: [
      { protocol: 'https', hostname: 'images.unsplash.com' },
    ],
  },
  serverExternalPackages: ['@stateset/embedded', 'better-sqlite3'],
};

module.exports = nextConfig;
