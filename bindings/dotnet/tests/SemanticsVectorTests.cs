using System.Globalization;
using System.Text.Json;
using StateSet.Embedded;
using Xunit;

namespace StateSet.Tests;

/// <summary>
/// The .NET binding against the shared semantic corpus
/// (<c>bindings/test-vectors/semantics-v1.json</c>), which pins the meaning of
/// money across every binding and is kept honest against the engine by
/// <c>crates/stateset-embedded/tests/semantics_vectors.rs</c>. Categories the
/// .NET surface cannot reach are DECLARED with the reason, and the declaration
/// is checked against the corpus so a new category cannot slip in unnoticed.
/// </summary>
/// <remarks>
/// C# has a real "absent" (<c>null</c>), so unlike Go the blank-currency row is
/// asserted: an explicit <c>""</c> must be refused.
/// </remarks>
public sealed class SemanticsVectorTests
{
    private static readonly Dictionary<string, string> NotReachable = new()
    {
        ["currency_decimals"] = "the .NET surface exposes no currency-scale accessor",
        ["canadian_tax_rates"] = "the .NET surface exposes no Canadian tax lookup",
    };

    private static readonly string[] Asserted =
    {
        "decimal_render", "money_scale_enforced", "rejected_inputs", "accepted_inputs",
    };

    private static JsonElement LoadCorpus()
    {
        var dir = new DirectoryInfo(Directory.GetCurrentDirectory());
        while (dir != null)
        {
            var candidate = Path.Combine(dir.FullName, "bindings", "test-vectors", "semantics-v1.json");
            if (File.Exists(candidate))
            {
                using var doc = JsonDocument.Parse(File.ReadAllText(candidate));
                Assert.Equal(1, doc.RootElement.GetProperty("version").GetInt32());
                return doc.RootElement.Clone();
            }
            dir = dir.Parent;
        }
        throw new FileNotFoundException("could not locate bindings/test-vectors/semantics-v1.json");
    }

    private static IEnumerable<JsonElement> Rows(JsonElement corpus, string category) =>
        corpus.GetProperty("categories").GetProperty(category).GetProperty("rows").EnumerateArray();

    private static (StateSetCommerce, Customer) Store()
    {
        var c = new StateSetCommerce(":memory:");
        return (c, c.Customers.Create("vectors@example.com", "V", "E"));
    }

    private static CreateOrderItem Line(Customer c, decimal price, int qty = 1) =>
        new() { ProductId = c.Id, Sku = "SKU-1", Name = "Widget", Quantity = qty, UnitPrice = price };

    private static decimal Dec(string s) => decimal.Parse(s, NumberStyles.Number, CultureInfo.InvariantCulture);

    [Fact]
    public void EveryCategoryIsAccountedFor()
    {
        var present = LoadCorpus().GetProperty("categories").EnumerateObject().Select(p => p.Name).OrderBy(n => n);
        var accounted = Asserted.Concat(NotReachable.Keys).OrderBy(n => n);
        Assert.Equal(accounted, present);
    }

    [Fact]
    public void DecimalRender()
    {
        var corpus = LoadCorpus();
        var (commerce, cust) = Store();
        using var _ = commerce;
        var checkedRows = 0;
        foreach (var row in Rows(corpus, "decimal_render"))
        {
            if (!row.TryGetProperty("money_scale_ok", out var ok) || !ok.GetBoolean()) continue;
            var ops = row.GetProperty("operands").EnumerateArray().Select(o => o.GetString()!).ToArray();
            List<CreateOrderItem> items;
            switch (row.GetProperty("op").GetString())
            {
                case "add":
                    items = ops.Select(o => Line(cust, Dec(o))).ToList();
                    break;
                case "mul":
                    items = new() { Line(cust, Dec(ops[0]), (int)Dec(ops[1])) };
                    break;
                default:
                    continue;
            }
            var order = commerce.Orders.Create(cust.Id, items);
            Assert.True(Dec(row.GetProperty("expected").GetString()!) == order.TotalAmount,
                $"{row.GetProperty("id")}: total {order.TotalAmount}, corpus says {row.GetProperty("expected")}");
            checkedRows++;
        }
        Assert.True(checkedRows > 0);
    }

    [Fact]
    public void MoneyScaleEnforced()
    {
        var (commerce, cust) = Store();
        using var _ = commerce;
        foreach (var row in Rows(LoadCorpus(), "money_scale_enforced"))
        {
            if (row.GetProperty("currency").GetString() != "USD") continue;
            var amount = Dec(row.GetProperty("amount").GetString()!);
            var mustReject = row.GetProperty("must_reject").GetBoolean();
            if (mustReject)
            {
                Assert.ThrowsAny<StateSetException>(() => commerce.Orders.Create(cust.Id, new[] { Line(cust, amount) }));
            }
            else
            {
                commerce.Orders.Create(cust.Id, new[] { Line(cust, amount) });
            }
        }
    }

    [Fact]
    public void RejectedInputs()
    {
        var (commerce, cust) = Store();
        using var _ = commerce;
        foreach (var row in Rows(LoadCorpus(), "rejected_inputs"))
        {
            var value = row.GetProperty("value").GetString()!;
            switch (row.GetProperty("kind").GetString())
            {
                case "currency":
                    Assert.ThrowsAny<StateSetException>(() =>
                        commerce.Orders.Create(cust.Id, new[] { Line(cust, 10.00m) }, currency: value));
                    break;
                case "uuid":
                    Assert.ThrowsAny<StateSetException>(() =>
                        commerce.Orders.Create(value, new[] { Line(cust, 10.00m) }));
                    break;
                default:
                    continue; // timestamps and dates have no order-path field
            }
        }
        Assert.Empty(commerce.Orders.List());
    }

    [Fact]
    public void AcceptedInputs()
    {
        var (commerce, cust) = Store();
        using var _ = commerce;
        foreach (var row in Rows(LoadCorpus(), "accepted_inputs"))
        {
            if (row.GetProperty("kind").GetString() != "currency") continue;
            var order = commerce.Orders.Create(cust.Id, new[] { Line(cust, 10.00m) },
                currency: row.GetProperty("value").GetString());
            Assert.Equal(row.GetProperty("normalizes_to").GetString(), order.Currency);
        }
    }
}
