# Migrating Laidout from 0.1 to 0.2

Laidout 0.2 is a breaking pre-1.0 release. The default crate is now the compact
standalone prepared kernel; generic cost-model research APIs are optional.

## Documents and annotations

`Doc<A>` is an opaque `Arc`-owned handle parameterized by the application’s
annotation value. Replace enum matching and `Rc<Doc>` construction with
constructors:

```rust
// 0.1: Rc::new(Doc::Concat(left, right))
// 0.2:
let doc = laidout::Doc::concat([left, right]);
let tagged = laidout::Doc::annotate(MyAnnotation::Label, doc);
```

The deprecated free constructors remain short migration adapters. `TagId` is a
compatibility alias for `u32`; new consumers use their own annotation type.
N-ary `concat` and `join` are canonical and avoid left-deep source trees.

## Prepare once, render repeatedly

`render(&doc, options)` still returns an owned result. Repeated callers use
`Doc::prepare`, `RenderWorkspace`, and `render_into`/`solve_into`.
Growable mode expands only the named exhausted resource. Fixed mode never
allocates and returns `WorkspaceExhausted` when the retained capacity is too
small. Call `release_capacity` after an exceptional high-water workload.

`Rendered` fields are private in 0.2; use `text()`, `spans()`,
`resolved_spans()`, `cost()`, and `stats()`. Borrowed results cannot outlive or
be used across another mutable workspace call.

## Fast behavior correction

Fast fitting is display-column based and uses prepared summaries. The retired
synthetic scan budget broke groups containing many zero-width scalars even when
they fit. At width two, the canonical correction is:

```text
0.1: "\u{200b}\u{200b}\u{200b}\u{200b}\nx"
0.2: "\u{200b}\u{200b}\u{200b}\u{200b} x"
```

There is no compatibility switch. Express a required break with `hard_line()`.

## Research feature

Add `features = ["research"]` and move old generic imports under
`laidout::research`:

- `CostModel`, `LawfulCostModel`, `OverflowThenHeight`, and unbounded consumer
  cost types;
- `brute`, `greedy`, and sealed `frontier` engines;
- `Out`, generic rendered results, and output helpers;
- prepared/workspace footprint telemetry under `laidout::measure`.

The default kernel has no `num-bigint` dependency. Its checked consumer cost is
read-only and preparation proves every reachable derivation fits the compact
domain before solving.
