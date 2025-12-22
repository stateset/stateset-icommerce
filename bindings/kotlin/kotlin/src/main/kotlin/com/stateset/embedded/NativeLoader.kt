package com.stateset.embedded

import java.io.File
import java.nio.file.Files

/**
 * Native library loader for StateSet Embedded Commerce
 */
internal object NativeLoader {
    private var loaded = false

    @Synchronized
    fun load() {
        if (loaded) return

        val libName = System.mapLibraryName("stateset_kotlin")
        val osName = System.getProperty("os.name").lowercase()
        val osArch = System.getProperty("os.arch").lowercase()

        val platform = when {
            osName.contains("linux") -> "linux"
            osName.contains("mac") || osName.contains("darwin") -> "macos"
            osName.contains("windows") -> "windows"
            else -> throw UnsupportedOperationException("Unsupported OS: $osName")
        }

        val arch = when {
            osArch.contains("aarch64") || osArch.contains("arm64") -> "arm64"
            osArch.contains("amd64") || osArch.contains("x86_64") -> "x86_64"
            else -> "x86_64"
        }

        // Try loading from system path first
        try {
            System.loadLibrary("stateset_kotlin")
            loaded = true
            return
        } catch (e: UnsatisfiedLinkError) {
            // Fall through to bundled library
        }

        // Try loading bundled library from resources
        val resourcePath = "/native/$platform-$arch/$libName"
        val altResourcePath = "/native/$libName"

        val inputStream = NativeLoader::class.java.getResourceAsStream(resourcePath)
            ?: NativeLoader::class.java.getResourceAsStream(altResourcePath)
            ?: throw UnsatisfiedLinkError("Native library not found: $resourcePath")

        val tempDir = Files.createTempDirectory("stateset-kotlin").toFile()
        tempDir.deleteOnExit()

        val tempFile = File(tempDir, libName)
        tempFile.deleteOnExit()

        inputStream.use { input ->
            tempFile.outputStream().use { output ->
                input.copyTo(output)
            }
        }

        System.load(tempFile.absolutePath)
        loaded = true
    }
}
