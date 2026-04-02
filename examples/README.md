# StateSet iCommerce Examples

Complete examples, guides, and scripts for StateSet Commerce.

## Documentation

| Guide | Description |
|-------|-------------|
| **[CLI Reference](./cli-reference.md)** | Complete command reference (all commands, examples) |
| **[Getting Started with Sync](./getting-started-sync.md)** | Full setup guide for CLI + Sequencer |
| **[Common Workflows](./workflows.md)** | Step-by-step guides (checkout, returns, inventory, etc.) |
| **[Troubleshooting](./troubleshooting.md)** | Solutions to common problems |
| **`examples/agents/openai-embedded-toolkit.mjs`** | Minimal embedded agent example using OpenAI-style JSON-schema tools |
| **`examples/agents/framework-adapters.mjs`** | Minimal Vercel AI and LangChain adapter example using the embedded toolkit |
| **[`examples/agents/README.md`](./agents/README.md)** | Runnable x402 agent demo flows: paid HTTP, local intents, and metered credits |

## Scripts

| Script | Description |
|--------|-------------|
| `setup-sync.sh` | Automated setup (tenant, sync, keys) |
| `seed-demo-data.sh` | Create demo data (10 customers, 20 products, 15 orders) |
| `verify-setup.sh` | Verify all components work |
| `docker-compose.full.yml` | Full stack Docker setup |

Quick start:
```bash
# Generate credentials first
export ADMIN_API_KEY=$(openssl rand -hex 32)
export POSTGRES_PASSWORD=$(openssl rand -hex 16)
export STATESET_TENANT_ID=$(uuidgen)
export STATESET_STORE_ID=$(uuidgen)

# Start services and setup
docker-compose -f docker-compose.full.yml up -d
./setup-sync.sh --api-key $ADMIN_API_KEY --tenant-id $STATESET_TENANT_ID --store-id $STATESET_STORE_ID
./seed-demo-data.sh && ./verify-setup.sh
```

## Language Examples

Complete examples in 9 programming languages, each showing the same workflow:
1. Initialize commerce engine
2. Create customers
3. Create products with variants
4. Set up inventory tracking
5. Create and process orders
6. Analytics and reporting

## Quick Start by Language

### Rust

```bash
cargo run --example basic_usage
```

### Node.js / JavaScript

```bash
cd examples/node
npm install @stateset/embedded
node basic_usage.js
```

### Python

```bash
cd examples/python
pip install stateset-embedded
python basic_usage.py
```

### Go

```bash
cd examples/go
# First build the Rust library
cargo build --release -p stateset-go
# Then run the example
go run basic_usage.go
```

### Kotlin

```bash
cd examples/kotlin
./gradlew run
# Or build a jar:
./gradlew jar
java -jar build/libs/kotlin-0.8.0.jar
```

### Swift

```bash
cd examples/swift
# Link against the Swift package
swift run
# Or compile directly:
swiftc -I ../bindings/swift/Sources -L ../target/release -lstateset_swift BasicUsage.swift -o basic_usage
./basic_usage
```

### C# / .NET

```bash
cd examples/dotnet
dotnet run
```

### Ruby

```bash
cd examples/ruby
gem install stateset_embedded
ruby basic_usage.rb
```

### Java

```bash
cd examples/java
# With Maven
mvn compile exec:java -Dexec.mainClass="com.stateset.examples.BasicUsage"
# Or compile manually
javac -cp path/to/stateset-embedded.jar BasicUsage.java
java -cp .:path/to/stateset-embedded.jar com.stateset.examples.BasicUsage
```

## Example Output

All examples produce similar output:

```
=== StateSet iCommerce Example ===

✓ Commerce initialized

1. Creating customer...
   Created customer: Alice Smith (alice@example.com)

2. Creating products...
   Created product: Premium Widget (premium-widget)
   Created product: Super Gadget (super-gadget)

3. Setting up inventory...
   Created inventory for WIDGET-001 (100 units)
   Created inventory for GADGET-001 (50 units)
   Stock check WIDGET-001: 100 available

4. Creating order...
   Created order ORD-1234567890 (total: $109.97)

5. Processing order...
   Order status: confirmed
   Inventory adjusted
   Order shipped with tracking: TRACK123456

6. Final inventory check...
   WIDGET-001: 98 available (was 100)
   GADGET-001: 49 available (was 50)

7. Analytics...
   Revenue: $109.97
   Orders: 1
   AOV: $109.97

=== Summary ===
Customers: 1
Products: 2
Orders: 1

✓ Example completed successfully!
```

## What Each Example Demonstrates

| Feature | Description |
|---------|-------------|
| **Initialization** | Create commerce instance with SQLite (in-memory or file) |
| **Customers** | Create customer profiles with contact info |
| **Products** | Create products with SKUs, pricing, descriptions |
| **Inventory** | Track stock levels, create items, adjust quantities |
| **Orders** | Create orders with line items, process status changes |
| **Fulfillment** | Reserve inventory, ship orders with tracking |
| **Analytics** | Sales summaries, revenue metrics, order counts |

## Platform Support

| Language | Platforms |
|----------|-----------|
| Rust | Linux, macOS, Windows |
| Node.js | Linux, macOS, Windows |
| Python | Linux, macOS, Windows |
| Go | Linux, macOS, Windows |
| Kotlin | Linux, macOS, Windows, Android |
| Swift | Linux, macOS, iOS |
| C# | Linux, macOS, Windows |
| Ruby | Linux, macOS, Windows |
| Java | Linux, macOS, Windows, Android |

## Building from Source

If you want to build the native libraries from source:

```bash
# Build all default bindings
cargo build --release

# Build specific binding
cargo build --release -p stateset-go
cargo build --release -p stateset-kotlin
cargo build --release -p stateset-swift
cargo build --release -p stateset-dotnet
```

The native libraries will be in `target/release/`.

## CLI with Sequencer Sync

For setting up the CLI with the StateSet Sequencer for distributed, verifiable commerce operations:

**[Getting Started: CLI + Sequencer Sync](./getting-started-sync.md)**

Quick setup:

```bash
# 1. Start the sequencer
cd ~/stateset-sequencer && docker-compose up -d

# 2. Set up your credentials
export STATESET_API_KEY="your-api-key-here"
export STATESET_TENANT_ID=$(uuidgen)
export STATESET_STORE_ID=$(uuidgen)

# 3. Register tenant
curl -X POST http://localhost:8080/admin/tenants \
  -H "X-API-Key: $STATESET_API_KEY" \
  -H "Content-Type: application/json" \
  -d "{\"tenant_id\": \"$STATESET_TENANT_ID\", \"name\": \"my-store\"}"

# 4. Initialize sync
stateset-sync init \
  --sequencer-url http://localhost:8080 \
  --tenant-id $STATESET_TENANT_ID \
  --store-id $STATESET_STORE_ID \
  --api-key $STATESET_API_KEY \
  --db ./store.db

# 5. Generate and register keys
stateset-sync keys:generate
stateset-sync keys:register

# 6. Use the CLI
stateset --apply "create customer alice@example.com Alice Smith"
stateset-sync push
```

## Need Help?

- [Documentation](https://docs.stateset.com)
- [GitHub Issues](https://github.com/stateset/stateset-icommerce/issues)
- [Discord Community](https://discord.gg/stateset)
