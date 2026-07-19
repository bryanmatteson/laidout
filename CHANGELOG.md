# Changelog

All notable changes to Laidout are documented here.

## 0.2.0 - 2026-07-19

### Added

- Immutable prepared documents and reusable Growable or Fixed render
  workspaces.
- Column-accurate Fast rendering and exact cost-minimizing rendering.
- Strict rendering with zero allocator calls after capacity reservation.
- Structural annotations, penalties, prepared tables, deterministic resource
  accounting, and typed exhaustion or domain failures.
- A sealed exact-cost production kernel plus research-only arbitrary-precision
  oracle engines and machine-checked cost laws.

### Changed

- Consumer documents use an opaque generic `Doc<A>` and explicit preparation
  before allocation-controlled rendering.
- Cost and output limits are checked instead of saturated or silently clamped.
- Research engines and arbitrary-precision costs require the `research` feature.

### Compatibility

This is a breaking pre-1.0 release. See
[`docs/migration-0.2.md`](docs/migration-0.2.md) for migration guidance.
