# Changelog

All notable changes to StateSet iCommerce will be documented in this file.

This project follows Keep a Changelog and Semantic Versioning.

## [Unreleased]

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
