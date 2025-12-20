package com.stateset.embedded;

import java.io.*;
import java.nio.file.*;

/**
 * Loads the native StateSet library.
 */
final class NativeLoader {

    private static boolean loaded = false;

    private NativeLoader() {}

    /**
     * Load the native library.
     */
    static synchronized void load() {
        if (loaded) {
            return;
        }

        String osName = System.getProperty("os.name").toLowerCase();
        String osArch = System.getProperty("os.arch").toLowerCase();

        String libName;
        String libExtension;

        if (osName.contains("win")) {
            libName = "stateset_java";
            libExtension = ".dll";
        } else if (osName.contains("mac")) {
            libName = "libstateset_java";
            libExtension = ".dylib";
        } else {
            libName = "libstateset_java";
            libExtension = ".so";
        }

        // Try loading from java.library.path first
        try {
            System.loadLibrary("stateset_java");
            loaded = true;
            return;
        } catch (UnsatisfiedLinkError e) {
            // Continue to try other methods
        }

        // Try loading from classpath
        String resourcePath = "/native/" + getPlatformDir() + "/" + libName + libExtension;
        try (InputStream is = NativeLoader.class.getResourceAsStream(resourcePath)) {
            if (is != null) {
                Path tempDir = Files.createTempDirectory("stateset-java");
                Path tempLib = tempDir.resolve(libName + libExtension);
                Files.copy(is, tempLib, StandardCopyOption.REPLACE_EXISTING);
                System.load(tempLib.toAbsolutePath().toString());
                tempLib.toFile().deleteOnExit();
                tempDir.toFile().deleteOnExit();
                loaded = true;
                return;
            }
        } catch (IOException e) {
            // Continue to try other methods
        }

        // Try loading from current directory
        String currentDir = System.getProperty("user.dir");
        Path localLib = Paths.get(currentDir, libName + libExtension);
        if (Files.exists(localLib)) {
            System.load(localLib.toAbsolutePath().toString());
            loaded = true;
            return;
        }

        // Try target/release directory (development)
        Path targetLib = Paths.get(currentDir, "target", "release", libName + libExtension);
        if (Files.exists(targetLib)) {
            System.load(targetLib.toAbsolutePath().toString());
            loaded = true;
            return;
        }

        throw new UnsatisfiedLinkError(
            "Failed to load StateSet native library. " +
            "Tried: java.library.path, classpath resource (" + resourcePath + "), " +
            "current directory, and target/release directory."
        );
    }

    private static String getPlatformDir() {
        String osName = System.getProperty("os.name").toLowerCase();
        String osArch = System.getProperty("os.arch").toLowerCase();

        String os;
        if (osName.contains("win")) {
            os = "windows";
        } else if (osName.contains("mac")) {
            os = "darwin";
        } else {
            os = "linux";
        }

        String arch;
        if (osArch.contains("aarch64") || osArch.contains("arm64")) {
            arch = "arm64";
        } else if (osArch.contains("amd64") || osArch.contains("x86_64")) {
            arch = "x86_64";
        } else {
            arch = osArch;
        }

        return os + "-" + arch;
    }
}
