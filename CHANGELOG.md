# Changelog

All notable changes to Laidout are documented here.

## 0.3.1 - 2026-07-25

### Changed

- Updated the optional `num-bigint` research dependency to 0.5.
- Updated the checkout and artifact-upload GitHub Actions to v7.
- Manifest contract tests now verify feature relationships without coupling
  them to dependency versions.

### Fixed

- Allocation tests now scope measurement to the active test thread, preventing
  unrelated test-harness allocations from affecting results.

## 0.3.0 - 2026-07-25

### Added

- A reusable `Renderer` with owned, `fmt::Write`, and `io::Write` paths.
- Generic classified-text ingestion, token constructors, and aligned tables.
- Explicit-width text runs for host-controlled display measurement.
- DAG-preserving `Doc::map_annotations` and a common `prelude`.
- Stable error-kind enums, capacity constructors, and complete public API
  documentation.
- GitHub workflows for cross-platform verification, dependency policy, API
  compatibility, and trusted publishing.
- Repository contribution, security, conduct, issue, and review policies.

### Changed

- Annotation preparation now interns shared values by identity and no longer
  requires `Eq + Hash`.
- Public error and capacity types are non-exhaustive so compatible variants and
  fields can be added in later releases.
- Release verification captures fresh benchmark evidence from a clean checkout
  instead of depending on untracked local artifacts.
- Benchmark artifacts identify source, harness, adapters, workloads,
  dependencies, toolchain, and host without external design artifacts.
- GitHub Actions dependencies are pinned to immutable commits.
- Published package contents use an explicit allowlist.

### Compatibility

This is a breaking pre-1.0 release. See
[`docs/migration-0.3.md`](docs/migration-0.3.md) for migration guidance.

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
