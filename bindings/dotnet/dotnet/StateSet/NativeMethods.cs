using System.Reflection;
using System.Runtime.InteropServices;

namespace StateSet.Embedded;

/// <summary>
/// P/Invoke declarations for the native library <c>stateset_dotnet</c>.
/// </summary>
/// <remarks>
/// The library is a thin re-export of the <c>stateset-ffi</c> crate's C ABI
/// (<c>crates/stateset-ffi/src/json_api.rs</c> and <c>crypto_api.rs</c>).
/// The whole commerce surface goes through one entry point,
/// <see cref="stateset_json_call"/>, which takes a method name and a JSON
/// object and returns a JSON envelope that must be released with
/// <see cref="stateset_string_free"/>.
///
/// Library resolution: the default .NET probing (application directory,
/// <c>runtimes/&lt;rid&gt;/native</c> in a NuGet package, then the OS loader
/// path) is used, unless the <c>STATESET_NATIVE_LIB</c> environment variable
/// names the library file explicitly.
/// </remarks>
internal static partial class NativeMethods
{
    internal const string LibraryName = "stateset_dotnet";

    /// <summary>The <c>stateset-ffi</c> ABI major version this binding was written for.</summary>
    internal const uint ExpectedAbiVersion = 1;

    static NativeMethods()
    {
        try
        {
            NativeLibrary.SetDllImportResolver(typeof(NativeMethods).Assembly, Resolve);
        }
        catch (InvalidOperationException)
        {
            // A resolver is already registered for this assembly; keep it.
        }
    }

    private static IntPtr Resolve(string libraryName, Assembly assembly, DllImportSearchPath? searchPath)
    {
        if (libraryName != LibraryName)
        {
            return IntPtr.Zero;
        }
        var explicitPath = Environment.GetEnvironmentVariable("STATESET_NATIVE_LIB");
        if (!string.IsNullOrEmpty(explicitPath))
        {
            return NativeLibrary.Load(explicitPath);
        }
        return IntPtr.Zero; // fall back to default probing
    }

    // ------------------------------------------------------------------
    // Lifecycle, calls and memory
    // ------------------------------------------------------------------

    [LibraryImport(LibraryName, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial int stateset_json_open(string dbPath, out IntPtr handle);

    [LibraryImport(LibraryName, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial IntPtr stateset_json_call(IntPtr handle, string method, string? argsJson);

    [LibraryImport(LibraryName)]
    internal static partial void stateset_destroy(IntPtr handle);

    [LibraryImport(LibraryName)]
    internal static partial void stateset_string_free(IntPtr s);

    /// <summary>Borrowed pointer, valid until the next call on this thread. Never free it.</summary>
    [LibraryImport(LibraryName)]
    internal static partial IntPtr stateset_last_error_message();

    [LibraryImport(LibraryName)]
    internal static partial uint stateset_abi_version();

    // ------------------------------------------------------------------
    // Cross-binding crypto primitives
    // ------------------------------------------------------------------

    [LibraryImport(LibraryName)]
    internal static partial void stateset_crypto_free_buffer(IntPtr ptr, nuint len);

    [LibraryImport(LibraryName, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial int stateset_crypto_jcs_canonicalize(string jsonIn, out IntPtr outPtr, out nuint outLen);

    [LibraryImport(LibraryName, StringMarshalling = StringMarshalling.Utf8)]
    internal static partial int stateset_crypto_payload_plain_hash(string jsonIn, IntPtr saltIn, nuint saltLen, IntPtr outBuf32);

    [LibraryImport(LibraryName)]
    internal static partial int stateset_crypto_merkle_root(IntPtr leavesIn, nuint leafCount, IntPtr outBuf32);
}
