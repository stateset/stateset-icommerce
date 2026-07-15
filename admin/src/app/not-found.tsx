// Root app-router 404 page. Rendered for unknown routes and `notFound()`
// calls from server components.

import { Card, Button } from '@stateset/design';

export default function NotFound() {
  return (
    <div className="flex min-h-[60vh] items-center justify-center p-6">
      <Card className="w-full max-w-md p-6 text-center">
        <p className="mb-2 text-sm font-semibold uppercase tracking-ds-kicker text-ds-accent">404</p>
        <h2 className="mb-2 font-ds-display text-lg font-semibold text-ds-foreground">
          Page not found
        </h2>
        <p className="mb-4 text-sm text-ds-muted-foreground">
          The page you are looking for does not exist or may have been moved.
        </p>
        <Button href="/">Back to dashboard</Button>
      </Card>
    </div>
  );
}
