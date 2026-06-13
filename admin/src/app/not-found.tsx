// Root app-router 404 page. Rendered for unknown routes and `notFound()`
// calls from server components.

import Link from 'next/link';

export default function NotFound() {
  return (
    <div className="flex min-h-[60vh] items-center justify-center p-6">
      <div className="w-full max-w-md rounded-lg border border-gray-200 bg-white p-6 text-center dark:border-gray-800 dark:bg-gray-900">
        <p className="mb-2 text-sm font-medium text-gray-500 dark:text-gray-400">404</p>
        <h2 className="mb-2 text-lg font-semibold text-gray-900 dark:text-gray-50">
          Page not found
        </h2>
        <p className="mb-4 text-sm text-gray-600 dark:text-gray-300">
          The page you are looking for does not exist or may have been moved.
        </p>
        <Link
          href="/"
          className="inline-flex h-9 items-center justify-center rounded-md bg-indigo-600 px-4 py-2 text-sm font-medium text-white shadow transition-colors hover:bg-indigo-700 focus-visible:outline-none focus-visible:ring-1 focus-visible:ring-ring dark:bg-indigo-500 dark:hover:bg-indigo-600"
        >
          Back to dashboard
        </Link>
      </div>
    </div>
  );
}
