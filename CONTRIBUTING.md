# Contributing to StateSet iCommerce

Thank you for your interest in contributing to StateSet iCommerce! This document provides guidelines and instructions for contributing.

## Table of Contents

- [Code of Conduct](#code-of-conduct)
- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Project Structure](#project-structure)
- [Making Changes](#making-changes)
- [Testing](#testing)
- [Pull Request Process](#pull-request-process)
- [Style Guidelines](#style-guidelines)
- [Reporting Issues](#reporting-issues)

## Code of Conduct

Please be respectful and constructive in all interactions. We're building software for AI agents and humans alike—let's make the community welcoming for everyone.

## Getting Started

1. **Fork the repository** on GitHub
2. **Clone your fork** locally:
   ```bash
   git clone https://github.com/YOUR_USERNAME/stateset-icommerce.git
   cd stateset-icommerce
   ```
3. **Add the upstream remote**:
   ```bash
   git remote add upstream https://github.com/stateset/stateset-icommerce.git
   ```

## Development Setup

### Prerequisites

- **Rust 1.70+** - Install via [rustup](https://rustup.rs/)
- **Node.js 18+** - For CLI and Node.js bindings
- **Python 3.8+** - For Python bindings (optional)
- **wasm-pack** - For WebAssembly bindings (optional)
- **Java 11+** - For Java/Kotlin bindings (optional)
- **Go 1.20+** - For Go bindings (optional)
- **.NET SDK 8.0+** - For C#/.NET bindings (optional)
- **Swift 5.7+** - For Swift bindings (macOS only, optional)

### Building the Rust Crates

```bash
# Build default workspace members (core crates)
cargo build

# Build with all features
cargo build --all-features

# Build specific crate
cargo build -p stateset-core
cargo build -p stateset-db
cargo build -p stateset-embedded
```

To build a specific binding, target it explicitly:

```bash
cargo build -p stateset-java
cargo build -p stateset-kotlin
cargo build -p stateset-go
cargo build -p stateset-dotnet
cargo build -p stateset-swift
```

### Building the Node.js Binding

```bash
cd bindings/node
npm install
npm run build
```

### Building the Python Binding

```bash
cd bindings/python
pip install maturin
maturin develop
```

### Building the WASM Binding

```bash
cd bindings/wasm
wasm-pack build --target nodejs
```

### Setting Up the CLI

```bash
cd cli
npm install
npm link

# Verify installation
stateset --help
```

## Project Structure

```
stateset-icommerce/
├── crates/
│   ├── stateset-core/       # Pure domain models & business logic
│   │   └── src/models/      # 14 domain modules (orders, inventory, etc.)
│   ├── stateset-db/         # Database layer (SQLite + PostgreSQL)
│   │   ├── src/sqlite/      # SQLite implementations
│   │   ├── src/postgres/    # PostgreSQL implementations
│   │   └── migrations/      # SQL migration files
│   └── stateset-embedded/   # High-level unified API
├── bindings/
│   ├── node/                # Node.js/NAPI bindings
│   ├── python/              # Python/PyO3 bindings
│   └── wasm/                # WebAssembly bindings
├── cli/
│   ├── bin/                 # CLI entry points
│   ├── src/                 # MCP server & utilities
│   └── .claude/             # AI agents & skills
└── examples/                # Usage examples
```

### Crate Responsibilities

| Crate | Purpose |
|-------|---------|
| `stateset-core` | Pure domain models with no I/O dependencies |
| `stateset-db` | Database implementations (SQLite, PostgreSQL) |
| `stateset-embedded` | High-level API that combines core + db |

## Making Changes

### Branch Naming

- `feature/description` - New features
- `fix/description` - Bug fixes
- `docs/description` - Documentation updates
- `refactor/description` - Code refactoring

### Commit Messages

Write clear, descriptive commit messages:

```
feat(orders): add bulk order creation support

- Implement batch insert for order items
- Add transaction support for atomicity
- Update tests for new functionality
```

Prefix commits with:
- `feat:` - New features
- `fix:` - Bug fixes
- `docs:` - Documentation
- `test:` - Test additions/changes
- `refactor:` - Code refactoring
- `chore:` - Maintenance tasks

## Testing

### Running Tests

```bash
# Run all tests
cargo test

# Run tests for a specific crate
cargo test -p stateset-core
cargo test -p stateset-db
cargo test -p stateset-embedded

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test test_order_lifecycle
```

CI note: Swift bindings run on macOS only when a PR has the `ci-swift` label (they run on pushes to main/master).

### Node.js Binding Tests

```bash
cd bindings/node
npm test
```

### Python Binding Tests

```bash
cd bindings/python
pytest
```

### Writing Tests

- Place unit tests in the same file as the code being tested
- Place integration tests in the `tests/` directory
- Test both success and error cases
- Use descriptive test names

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_creation_with_valid_items() {
        // Arrange
        let items = vec![...];
        
        // Act
        let order = Order::new(items);
        
        // Assert
        assert_eq!(order.status, OrderStatus::Pending);
    }
}
```

## Pull Request Process

1. **Sync with upstream**:
   ```bash
   git fetch upstream
   git rebase upstream/main
   ```

2. **Create a feature branch**:
   ```bash
   git checkout -b feature/your-feature
   ```

3. **Make your changes** and commit them

4. **Ensure tests pass**:
   ```bash
   cargo test
   cargo clippy -- -D warnings
   cargo fmt --check
   ```

5. **Push your branch**:
   ```bash
   git push origin feature/your-feature
   ```

6. **Open a Pull Request** on GitHub with:
   - Clear description of changes
   - Link to any related issues
   - Screenshots/examples if applicable

7. **Address review feedback** and update your PR as needed

## Style Guidelines

### Rust

- Follow standard Rust conventions
- Use `cargo fmt` to format code
- Use `cargo clippy` to catch common issues
- Document public APIs with doc comments

```rust
/// Creates a new order with the given items.
///
/// # Arguments
///
/// * `customer_id` - The ID of the customer placing the order
/// * `items` - A list of items to include in the order
///
/// # Returns
///
/// Returns `Ok(Order)` on success, or an error if validation fails.
///
/// # Example
///
/// ```
/// let order = commerce.orders().create(CreateOrder {
///     customer_id: uuid!("..."),
///     items: vec![...],
///     ..Default::default()
/// })?;
/// ```
pub fn create(&self, input: CreateOrder) -> Result<Order, CommerceError> {
    // ...
}
```

### JavaScript/TypeScript

- Use ES modules
- Follow existing code style in the CLI
- Add JSDoc comments for public functions

### SQL Migrations

- Number migrations sequentially (001_, 002_, etc.)
- Include both up and down migrations when possible
- Test migrations against both SQLite and PostgreSQL

## Reporting Issues

When reporting issues, please include:

1. **Description** - Clear description of the problem
2. **Steps to reproduce** - Minimal steps to reproduce the issue
3. **Expected behavior** - What you expected to happen
4. **Actual behavior** - What actually happened
5. **Environment** - OS, Rust version, etc.
6. **Code sample** - Minimal code that demonstrates the issue

```markdown
## Description
Orders fail to create when using PostgreSQL backend

## Steps to Reproduce
1. Initialize Commerce with PostgreSQL
2. Create a customer
3. Attempt to create an order

## Expected Behavior
Order should be created successfully

## Actual Behavior
Returns error: "column 'currency' does not exist"

## Environment
- OS: Ubuntu 22.04
- Rust: 1.75.0
- PostgreSQL: 15.0
```

## Areas for Contribution

Looking for ways to contribute? Here are some areas we'd love help with:

- **New domain models** - Extend the commerce capabilities
- **Database optimizations** - Query performance, indexing
- **Additional bindings** - Go, Ruby, Java, etc.
- **Documentation** - Tutorials, examples, API docs
- **Testing** - Increase test coverage
- **CLI improvements** - New commands, better UX
- **AI agents** - New specialized agents or skills

## Questions?

- Open a [GitHub Discussion](https://github.com/stateset/stateset-icommerce/discussions)
- File an [Issue](https://github.com/stateset/stateset-icommerce/issues)
- Email: hello@stateset.io

---

Thank you for contributing to StateSet iCommerce! 🚀
