# Changelog

All notable changes to StateSet iCommerce will be documented in this file.

This project follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

## [0.1.9] - 2025-01-09

### Fixed
- Safer Decimal to f64 conversions across all bindings (Node, Python, Ruby, PHP, Java, Kotlin, Swift) using `to_f64_or_nan` helper instead of `unwrap_or(0.0)`.
- Improved JNI error handling in Java bindings with `jni_or_throw` helper for better exception propagation.
- General Ledger parsing now uses proper error propagation (`parse_required`, `parse_optional`) instead of silent defaults.

### Changed
- All binding code now consistently handles numeric conversion edge cases.

## [0.1.8] - 2025-01-01

### Added
- mdBook-based documentation scaffold with API reference pointers and versioning notes.
- Docs build and version snapshot scripts under `docs/scripts/`.

## [0.1.7] - 2025-12-20

### Added
- 34 new MCP tools across Payments, Shipments, Suppliers/POs, Invoices, Warranties, and Manufacturing.
- Expanded agent and CLI coverage for additional commerce domains.

## [0.1.6] - 2025-12-20

### Added
- Java bindings via JNI.
- Ruby and PHP binding releases with native extensions.

### Fixed
- JNI memory management for thread-safe handles.
- Product variant handling in the Product API.
- Cart total calculations using `grand_total`.
