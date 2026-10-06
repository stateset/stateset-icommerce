using System.Globalization;
using System.Runtime.InteropServices;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace StateSet.Embedded;

/// <summary>
/// StateSet Embedded Commerce for .NET: the Rust commerce engine
/// (<c>stateset-embedded</c>) running in-process against a SQLite file.
/// </summary>
/// <remarks>
/// Every call goes through the native library (<c>stateset_dotnet</c>) and
/// persists to the database passed to the constructor; nothing is kept in
/// managed memory. Use <c>":memory:"</c> for an ephemeral store.
///
/// Instances are thread-safe: the engine handle is reference-counted on the
/// native side, and <see cref="Dispose"/> waits for in-flight calls.
/// </remarks>
/// <example>
/// <code>
/// using var commerce = new StateSetCommerce("store.db");
/// var customer = commerce.Customers.Create("alice@example.com", "Alice", "Smith");
/// var order = commerce.Orders.Create(customer.Id, new[]
/// {
///     new CreateOrderItem { ProductId = productId, Sku = "WIDGET-001", Name = "Widget", Quantity = 2, UnitPrice = 29.99m },
/// });
/// </code>
/// </example>
public sealed class StateSetCommerce : IDisposable
{
    private IntPtr _handle;
    private int _disposed;

    internal static readonly JsonSerializerOptions JsonOptions = CreateJsonOptions();

    /// <summary>Customers.</summary>
    public CustomersApi Customers { get; }

    /// <summary>Products and variants.</summary>
    public ProductsApi Products { get; }

    /// <summary>Inventory items, stock levels, adjustments and reservations.</summary>
    public InventoryApi Inventory { get; }

    /// <summary>Carts and checkout.</summary>
    public CartsApi Carts { get; }

    /// <summary>Orders.</summary>
    public OrdersApi Orders { get; }

    /// <summary>Payments and refunds.</summary>
    public PaymentsApi Payments { get; }

    /// <summary>Returns (RMAs).</summary>
    public ReturnsApi Returns { get; }

    /// <summary>Shipments.</summary>
    public ShipmentsApi Shipments { get; }

    /// <summary>
    /// Open (or create) a store at <paramref name="dbPath"/>.
    /// </summary>
    /// <param name="dbPath">SQLite file path, or <c>":memory:"</c>.</param>
    /// <exception cref="StateSetException">The store could not be opened.</exception>
    public StateSetCommerce(string dbPath)
    {
        ArgumentNullException.ThrowIfNull(dbPath);
        var abi = NativeMethods.stateset_abi_version();
        if (abi != NativeMethods.ExpectedAbiVersion)
        {
            throw new StateSetException(
                $"native library ABI {abi} does not match the binding's ABI {NativeMethods.ExpectedAbiVersion}");
        }
        var rc = NativeMethods.stateset_json_open(dbPath, out _handle);
        if (rc != 0 || _handle == IntPtr.Zero)
        {
            var detail = Marshal.PtrToStringUTF8(NativeMethods.stateset_last_error_message()) ?? "unknown error";
            throw new StateSetException((StateSetErrorCode)rc, "open_failed", $"failed to open store '{dbPath}': {detail}");
        }

        Customers = new CustomersApi(this);
        Products = new ProductsApi(this);
        Inventory = new InventoryApi(this);
        Carts = new CartsApi(this);
        Orders = new OrdersApi(this);
        Payments = new PaymentsApi(this);
        Returns = new ReturnsApi(this);
        Shipments = new ShipmentsApi(this);
    }

    /// <summary>The engine and JSON-API versions reported by the native library.</summary>
    public JsonElement Version => Call<JsonElement>("meta.version", null);

    /// <summary>Every method name the native JSON surface accepts.</summary>
    public IReadOnlyList<string> NativeMethodNames => Call<List<string>>("meta.methods", null);

    /// <summary>
    /// Invoke a native method and deserialize its result.
    /// </summary>
    /// <exception cref="StateSetException">The engine refused the call.</exception>
    internal T Call<T>(string method, object? args)
    {
        var result = CallRaw(method, args);
        return result.ValueKind == JsonValueKind.Null
            ? default!
            : result.Deserialize<T>(JsonOptions)!;
    }

    /// <summary>Like <see cref="Call{T}"/> but returns null when the engine found nothing.</summary>
    internal T? CallOptional<T>(string method, object? args) where T : class
    {
        var result = CallRaw(method, args);
        return result.ValueKind == JsonValueKind.Null ? null : result.Deserialize<T>(JsonOptions);
    }

    private JsonElement CallRaw(string method, object? args)
    {
        if (Volatile.Read(ref _disposed) != 0)
        {
            throw new ObjectDisposedException(nameof(StateSetCommerce));
        }
        var argsJson = args is null ? null : JsonSerializer.Serialize(args, args.GetType(), JsonOptions);
        var ptr = NativeMethods.stateset_json_call(_handle, method, argsJson);
        if (ptr == IntPtr.Zero)
        {
            var detail = Marshal.PtrToStringUTF8(NativeMethods.stateset_last_error_message()) ?? "no envelope returned";
            throw new StateSetException($"{method}: {detail}");
        }
        string envelope;
        try
        {
            envelope = Marshal.PtrToStringUTF8(ptr) ?? "";
        }
        finally
        {
            NativeMethods.stateset_string_free(ptr);
        }

        using var doc = JsonDocument.Parse(envelope);
        var root = doc.RootElement;
        if (root.GetProperty("ok").GetBoolean())
        {
            return root.GetProperty("result").Clone();
        }
        var error = root.GetProperty("error");
        var code = (StateSetErrorCode)error.GetProperty("code").GetInt32();
        var kind = error.GetProperty("kind").GetString() ?? "internal_error";
        var message = error.GetProperty("message").GetString() ?? "unknown error";
        throw code switch
        {
            StateSetErrorCode.NotFound => new StateSetNotFoundException(kind, message),
            StateSetErrorCode.InvalidArgument => new StateSetValidationException(kind, message),
            _ => new StateSetException(code, kind, message),
        };
    }

    /// <summary>Close the store and release the native handle.</summary>
    public void Dispose()
    {
        if (Interlocked.Exchange(ref _disposed, 1) == 0)
        {
            NativeMethods.stateset_destroy(_handle);
            _handle = IntPtr.Zero;
        }
        GC.SuppressFinalize(this);
    }

    /// <summary>Releases the native handle if <see cref="Dispose"/> was not called.</summary>
    ~StateSetCommerce()
    {
        if (Interlocked.Exchange(ref _disposed, 1) == 0)
        {
            NativeMethods.stateset_destroy(_handle);
        }
    }

    private static JsonSerializerOptions CreateJsonOptions()
    {
        var options = new JsonSerializerOptions
        {
            PropertyNamingPolicy = JsonNamingPolicy.SnakeCaseLower,
            DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        };
        options.Converters.Add(new DecimalStringConverter());
        options.Converters.Add(new JsonStringEnumConverter(JsonNamingPolicy.SnakeCaseLower));
        return options;
    }
}

/// <summary>
/// Writes <see cref="decimal"/> as an exact JSON string ("19.99") and reads
/// either a string or a number, so money never passes through a float.
/// </summary>
internal sealed class DecimalStringConverter : JsonConverter<decimal>
{
    public override decimal Read(ref Utf8JsonReader reader, Type typeToConvert, JsonSerializerOptions options)
    {
        if (reader.TokenType == JsonTokenType.String)
        {
            var s = reader.GetString();
            if (decimal.TryParse(s, NumberStyles.Number | NumberStyles.AllowExponent, CultureInfo.InvariantCulture, out var d))
            {
                return d;
            }
            throw new JsonException($"'{s}' is not a decimal");
        }
        return reader.GetDecimal();
    }

    public override void Write(Utf8JsonWriter writer, decimal value, JsonSerializerOptions options) =>
        writer.WriteStringValue(value.ToString(CultureInfo.InvariantCulture));
}
