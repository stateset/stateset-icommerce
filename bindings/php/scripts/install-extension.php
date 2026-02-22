<?php
/**
 * StateSet Embedded Commerce - Extension Installer
 *
 * Downloads and installs the pre-built native extension for your platform.
 *
 * Usage:
 *   php scripts/install-extension.php
 *   composer install-extension
 */

declare(strict_types=1);

namespace StateSet\Installer;

const VERSION = '0.7.3';
const EXTENSION_NAME = 'stateset_embedded';
const GITHUB_REPO = 'stateset/stateset-icommerce';

class ExtensionInstaller
{
    private string $version;
    private string $platform;
    private string $phpVersion;
    private string $arch;

    public function __construct(string $version = VERSION)
    {
        $this->version = $version;
        $this->detectPlatform();
    }

    private function detectPlatform(): void
    {
        $this->phpVersion = PHP_MAJOR_VERSION . '.' . PHP_MINOR_VERSION;

        // Detect OS
        if (PHP_OS_FAMILY === 'Windows') {
            $this->platform = 'windows';
        } elseif (PHP_OS_FAMILY === 'Darwin') {
            $this->platform = 'darwin';
        } else {
            $this->platform = 'linux';
        }

        // Detect architecture
        if (PHP_INT_SIZE === 8) {
            $this->arch = php_uname('m') === 'arm64' || php_uname('m') === 'aarch64'
                ? 'arm64'
                : 'x86_64';
        } else {
            $this->arch = 'x86';
        }
    }

    public function getDownloadUrl(): string
    {
        $phpMajorMinor = str_replace('.', '', $this->phpVersion);
        $artifact = "{$this->platform}-{$this->arch}-php{$phpMajorMinor}";
        $ext = $this->platform === 'windows' ? 'zip' : 'tar.gz';

        return sprintf(
            'https://github.com/%s/releases/download/php-v%s/%s-%s.%s',
            GITHUB_REPO,
            $this->version,
            EXTENSION_NAME,
            $artifact,
            $ext
        );
    }

    public function getExtensionDir(): string
    {
        $dir = ini_get('extension_dir');
        if (!$dir) {
            // Fallback
            $dir = PHP_EXTENSION_DIR;
        }
        return $dir;
    }

    public function getExtensionFilename(): string
    {
        if ($this->platform === 'windows') {
            return 'php_' . EXTENSION_NAME . '.dll';
        } elseif ($this->platform === 'darwin') {
            return EXTENSION_NAME . '.dylib';
        }
        return EXTENSION_NAME . '.so';
    }

    public function install(): bool
    {
        $this->info("StateSet Embedded Commerce Extension Installer");
        $this->info("=============================================");
        $this->info("Version: {$this->version}");
        $this->info("Platform: {$this->platform}");
        $this->info("Architecture: {$this->arch}");
        $this->info("PHP Version: {$this->phpVersion}");
        $this->info("");

        // Check if already installed
        if (extension_loaded(EXTENSION_NAME)) {
            $this->success("Extension already loaded!");
            return true;
        }

        $url = $this->getDownloadUrl();
        $this->info("Download URL: {$url}");

        // Download
        $tempFile = sys_get_temp_dir() . '/' . EXTENSION_NAME . '_download';
        $this->info("Downloading extension...");

        $context = stream_context_create([
            'http' => [
                'method' => 'GET',
                'header' => "User-Agent: StateSet-PHP-Installer\r\n",
                'follow_location' => true,
            ]
        ]);

        $content = @file_get_contents($url, false, $context);
        if ($content === false) {
            $this->error("Failed to download extension from: {$url}");
            $this->info("");
            $this->info("You may need to build from source:");
            $this->info("  cd bindings/php && cargo build --release");
            return false;
        }

        file_put_contents($tempFile, $content);
        $this->success("Downloaded successfully!");

        // Extract
        $tempDir = sys_get_temp_dir() . '/' . EXTENSION_NAME . '_extract';
        @mkdir($tempDir, 0755, true);

        if ($this->platform === 'windows') {
            $zip = new \ZipArchive();
            if ($zip->open($tempFile) === true) {
                $zip->extractTo($tempDir);
                $zip->close();
            }
        } else {
            $phar = new \PharData($tempFile);
            $phar->extractTo($tempDir, null, true);
        }

        // Find and copy extension
        $extFile = $this->getExtensionFilename();
        $sourcePath = $tempDir . '/' . $extFile;

        if (!file_exists($sourcePath)) {
            // Try without the extension in path
            $files = glob($tempDir . '/*' . pathinfo($extFile, PATHINFO_EXTENSION));
            if (!empty($files)) {
                $sourcePath = $files[0];
            }
        }

        if (!file_exists($sourcePath)) {
            $this->error("Extension file not found in archive");
            return false;
        }

        $destDir = $this->getExtensionDir();
        $destPath = $destDir . '/' . $extFile;

        $this->info("Installing to: {$destPath}");

        if (!is_writable($destDir)) {
            $this->warn("Extension directory is not writable: {$destDir}");
            $this->info("");
            $this->info("Please run with elevated permissions:");
            $this->info("  sudo php scripts/install-extension.php");
            $this->info("");
            $this->info("Or manually copy:");
            $this->info("  sudo cp {$sourcePath} {$destPath}");

            // Copy to local directory as fallback
            $localPath = __DIR__ . '/../ext/' . $extFile;
            @mkdir(dirname($localPath), 0755, true);
            if (copy($sourcePath, $localPath)) {
                $this->success("Copied to local directory: {$localPath}");
                $this->info("Add to php.ini: extension={$localPath}");
            }
            return false;
        }

        if (copy($sourcePath, $destPath)) {
            chmod($destPath, 0755);
            $this->success("Extension installed successfully!");
            $this->info("");
            $this->info("Add this line to your php.ini:");
            $this->info("  extension=" . EXTENSION_NAME);
            $this->info("");
            $this->info("Or load dynamically:");
            $this->info("  dl('" . EXTENSION_NAME . "');");
            return true;
        }

        $this->error("Failed to copy extension");
        return false;
    }

    private function info(string $message): void
    {
        echo $message . PHP_EOL;
    }

    private function success(string $message): void
    {
        echo "\033[32m✓ {$message}\033[0m" . PHP_EOL;
    }

    private function warn(string $message): void
    {
        echo "\033[33m⚠ {$message}\033[0m" . PHP_EOL;
    }

    private function error(string $message): void
    {
        echo "\033[31m✗ {$message}\033[0m" . PHP_EOL;
    }
}

// Run installer
if (php_sapi_name() === 'cli' && realpath($argv[0]) === realpath(__FILE__)) {
    $installer = new ExtensionInstaller();
    $success = $installer->install();
    exit($success ? 0 : 1);
}
