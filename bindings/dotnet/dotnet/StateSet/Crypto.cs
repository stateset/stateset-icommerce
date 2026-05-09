using System.Runtime.InteropServices;

namespace StateSet.Embedded;

/// <summary>
/// Cross-binding cryptographic primitives.
/// </summary>
/// <remarks>
/// Thin wrappers over the C-FFI exports in <c>bindings/dotnet/src/lib.rs</c>
/// that delegate to the <c>stateset-crypto</c> Rust crate. The same set of
/// primitives is exported from every StateSet binding and verified against
/// the language-neutral test corpus at
/// <c>bindings/test-vectors/v1.json</c>.
///
/// All methods are thread-safe and have no class state. On invalid input
/// or runtime failure they throw <see cref="InvalidOperationException"/>.
/// </remarks>
public static class Crypto
{
    /// <summary>
    /// Return the RFC 8785 JCS canonical-form bytes for a JSON string.
    /// </summary>
    /// <param name="json">JSON value as a UTF-8 string.</param>
    /// <returns>The canonical UTF-8 bytes.</returns>
    /// <exception cref="InvalidOperationException">
    /// Thrown if <paramref name="json"/> is invalid or canonicalization fails.
    /// </exception>
    public static byte[] JcsCanonicalize(string json)
    {
        var rc = NativeMethods.stateset_crypto_jcs_canonicalize(
            json,
            out IntPtr outPtr,
            out nuint outLen);
        if (rc != 0)
        {
            throw new InvalidOperationException(
                $"jcs_canonicalize failed (rc={rc}): invalid JSON or canonicalization error");
        }
        try
        {
            var managed = new byte[(int)outLen];
            Marshal.Copy(outPtr, managed, 0, (int)outLen);
            return managed;
        }
        finally
        {
            NativeMethods.stateset_crypto_free_buffer(outPtr, outLen);
        }
    }

    /// <summary>
    /// Compute the VES v1.0 payload-plain hash of a JSON payload.
    /// </summary>
    /// <param name="json">JSON payload as a UTF-8 string.</param>
    /// <param name="salt">16 bytes of salt, or <c>null</c> for unsalted.</param>
    /// <returns>The 32-byte digest.</returns>
    /// <exception cref="InvalidOperationException">
    /// Thrown if <paramref name="json"/> is invalid or <paramref name="salt"/> is
    /// not exactly 16 bytes.
    /// </exception>
    public static byte[] PayloadPlainHash(string json, byte[]? salt)
    {
        var output = new byte[32];
        IntPtr saltPtr = IntPtr.Zero;
        nuint saltLen = 0;
        GCHandle saltHandle = default;
        GCHandle outHandle = GCHandle.Alloc(output, GCHandleType.Pinned);
        try
        {
            if (salt != null)
            {
                if (salt.Length != 16)
                {
                    throw new InvalidOperationException(
                        $"salt must be exactly 16 bytes, got {salt.Length}");
                }
                saltHandle = GCHandle.Alloc(salt, GCHandleType.Pinned);
                saltPtr = saltHandle.AddrOfPinnedObject();
                saltLen = (nuint)salt.Length;
            }
            var rc = NativeMethods.stateset_crypto_payload_plain_hash(
                json,
                saltPtr,
                saltLen,
                outHandle.AddrOfPinnedObject());
            if (rc != 0)
            {
                throw new InvalidOperationException(
                    $"payload_plain_hash failed (rc={rc}): invalid JSON or hash error");
            }
            return output;
        }
        finally
        {
            if (saltHandle.IsAllocated) saltHandle.Free();
            outHandle.Free();
        }
    }

    /// <summary>
    /// Compute the merkle root of a list of 32-byte leaves.
    /// </summary>
    /// <param name="leaves">The leaf digests (each 32 bytes).</param>
    /// <returns>The 32-byte merkle root.</returns>
    /// <exception cref="InvalidOperationException">
    /// Thrown if any leaf is not exactly 32 bytes.
    /// </exception>
    public static byte[] MerkleRoot(byte[][] leaves)
    {
        var output = new byte[32];
        var outHandle = GCHandle.Alloc(output, GCHandleType.Pinned);
        try
        {
            if (leaves.Length == 0)
            {
                var rc0 = NativeMethods.stateset_crypto_merkle_root(
                    IntPtr.Zero,
                    0,
                    outHandle.AddrOfPinnedObject());
                if (rc0 != 0)
                {
                    throw new InvalidOperationException(
                        $"merkle_root failed for empty leaves (rc={rc0})");
                }
                return output;
            }
            var flat = new byte[leaves.Length * 32];
            for (int i = 0; i < leaves.Length; i++)
            {
                if (leaves[i].Length != 32)
                {
                    throw new InvalidOperationException(
                        $"merkle_root: leaf {i} must be 32 bytes, got {leaves[i].Length}");
                }
                Buffer.BlockCopy(leaves[i], 0, flat, i * 32, 32);
            }
            var flatHandle = GCHandle.Alloc(flat, GCHandleType.Pinned);
            try
            {
                var rc = NativeMethods.stateset_crypto_merkle_root(
                    flatHandle.AddrOfPinnedObject(),
                    (nuint)leaves.Length,
                    outHandle.AddrOfPinnedObject());
                if (rc != 0)
                {
                    throw new InvalidOperationException(
                        $"merkle_root failed (rc={rc})");
                }
                return output;
            }
            finally
            {
                flatHandle.Free();
            }
        }
        finally
        {
            outHandle.Free();
        }
    }
}
