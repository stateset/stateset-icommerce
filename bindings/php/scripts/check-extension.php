<?php
/**
 * StateSet Embedded Commerce - Extension Check
 *
 * Checks if the native extension is available and provides guidance.
 */

declare(strict_types=1);

const EXTENSION_NAME = 'stateset_embedded';

if (extension_loaded(EXTENSION_NAME)) {
    echo "\033[32m✓ StateSet native extension is loaded\033[0m" . PHP_EOL;
} else {
    echo "\033[33m⚠ StateSet native extension not found\033[0m" . PHP_EOL;
    echo PHP_EOL;
    echo "The native extension provides significant performance benefits." . PHP_EOL;
    echo "To install the pre-built extension, run:" . PHP_EOL;
    echo PHP_EOL;
    echo "  composer install-extension" . PHP_EOL;
    echo PHP_EOL;
    echo "Or build from source:" . PHP_EOL;
    echo "  cd bindings/php && cargo build --release" . PHP_EOL;
    echo PHP_EOL;
    echo "For more information, see:" . PHP_EOL;
    echo "  https://github.com/stateset/stateset-icommerce/tree/main/bindings/php" . PHP_EOL;
}
