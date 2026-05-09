using System.IO;
using System.Security.Cryptography;
using System.Text.Json;
using StateSet.Embedded;
using Xunit;

namespace StateSet.Tests;

/// <summary>
/// Cross-binding compatibility test for the .NET P/Invoke binding.
/// </summary>
/// <remarks>
/// Reads the language-neutral corpus at
/// <c>bindings/test-vectors/v1.json</c> and asserts the .NET binding
/// produces byte-equal hex digests to Rust ground truth for every entry.
/// Counterparts: Rust (<c>crates/stateset-crypto/tests/cross_binding_vectors.rs</c>),
/// Node, Python, Go, WASM, Java, and Kotlin.
/// </remarks>
public class CryptoVectorTests
{
    /// <summary>
    /// The corpus is at workspace-root <c>bindings/test-vectors/v1.json</c>;
    /// xUnit runs from <c>bindings/dotnet/tests/bin/.../</c>, so we walk up
    /// from the current directory until we find <c>bindings/test-vectors/v1.json</c>.
    /// </summary>
    private static string FindCorpus()
    {
        var dir = new DirectoryInfo(Directory.GetCurrentDirectory());
        while (dir != null)
        {
            var candidate = Path.Combine(
                dir.FullName, "bindings", "test-vectors", "v1.json");
            if (File.Exists(candidate)) return candidate;
            dir = dir.Parent;
        }
        throw new FileNotFoundException(
            "could not locate bindings/test-vectors/v1.json from " +
            Directory.GetCurrentDirectory());
    }

    private static JsonDocument LoadCorpus()
    {
        var raw = File.ReadAllText(FindCorpus());
        var doc = JsonDocument.Parse(raw);
        Assert.Equal(1, doc.RootElement.GetProperty("version").GetInt32());
        return doc;
    }

    private static string Hex(byte[] bytes)
    {
        var sb = new System.Text.StringBuilder(bytes.Length * 2);
        foreach (var b in bytes)
        {
            sb.AppendFormat("{0:x2}", b);
        }
        return sb.ToString();
    }

    private static byte[] FromHex(string s)
    {
        var len = s.Length;
        var bytes = new byte[len / 2];
        for (var i = 0; i < len; i += 2)
        {
            bytes[i / 2] = System.Convert.ToByte(s.Substring(i, 2), 16);
        }
        return bytes;
    }

    [Fact]
    public void CorpusIsPresentAndVersionOne()
    {
        using var doc = LoadCorpus();
        var cats = doc.RootElement.GetProperty("categories");
        Assert.Equal(JsonValueKind.Array, cats.GetProperty("canonical_json").ValueKind);
        Assert.Equal(JsonValueKind.Array, cats.GetProperty("payload_plain_hash").ValueKind);
        Assert.Equal(JsonValueKind.Array, cats.GetProperty("merkle_root").ValueKind);
    }

    [Fact]
    public void CanonicalJsonVectorsMatchGroundTruth()
    {
        using var doc = LoadCorpus();
        using var sha = SHA256.Create();
        var vectors = doc.RootElement
            .GetProperty("categories")
            .GetProperty("canonical_json");
        foreach (var v in vectors.EnumerateArray())
        {
            var id = v.GetProperty("id").GetString()!;
            var input = v.GetProperty("input").GetRawText();
            var expected = v.GetProperty("expected_hex").GetString()!;

            var canonical = Crypto.JcsCanonicalize(input);
            var digest = sha.ComputeHash(canonical);
            Assert.Equal(expected, Hex(digest));
            _ = id; // identifies the vector in failure context
        }
    }

    [Fact]
    public void PayloadPlainHashVectorsMatchGroundTruth()
    {
        using var doc = LoadCorpus();
        var vectors = doc.RootElement
            .GetProperty("categories")
            .GetProperty("payload_plain_hash");
        foreach (var v in vectors.EnumerateArray())
        {
            var id = v.GetProperty("id").GetString()!;
            var input = v.GetProperty("input").GetRawText();
            var expected = v.GetProperty("expected_hex").GetString()!;
            byte[]? salt = null;
            if (v.TryGetProperty("salt_hex", out var saltEl) &&
                saltEl.ValueKind == JsonValueKind.String)
            {
                salt = FromHex(saltEl.GetString()!);
            }

            var digest = Crypto.PayloadPlainHash(input, salt);
            Assert.Equal(expected, Hex(digest));
            _ = id;
        }
    }

    [Fact]
    public void MerkleRootVectorsMatchGroundTruth()
    {
        using var doc = LoadCorpus();
        var vectors = doc.RootElement
            .GetProperty("categories")
            .GetProperty("merkle_root");
        foreach (var v in vectors.EnumerateArray())
        {
            var id = v.GetProperty("id").GetString()!;
            var leavesHex = v.GetProperty("leaves_hex");
            var expected = v.GetProperty("expected_hex").GetString()!;

            var leaves = new byte[leavesHex.GetArrayLength()][];
            int i = 0;
            foreach (var h in leavesHex.EnumerateArray())
            {
                leaves[i++] = FromHex(h.GetString()!);
            }
            var root = Crypto.MerkleRoot(leaves);
            Assert.Equal(expected, Hex(root));
            _ = id;
        }
    }
}
