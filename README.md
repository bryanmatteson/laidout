# laidout

Laidout is a standalone prepared-document layout kernel. Applications build an
opaque immutable `Doc<A>` with their own annotation type, prepare it once, and
render with either a column-accurate Fast strategy or an exact cost-minimizing
strategy. A reusable workspace supports ordinary Growable rendering and a
strict Fixed mode with zero allocator calls after reservation. Laidout 0.2
supports Rust 1.85 and newer.

```rust
use std::num::NonZeroU32;
use laidout::{Doc, LayoutStrategy, RenderOptions, RenderWorkspace, render_into};

#[derive(Debug, Eq, Hash, PartialEq)]
enum Mark { Label, Value }

let source = Doc::group(Doc::concat([
    Doc::annotate(Mark::Label, Doc::text("status")),
    Doc::line(),
    Doc::annotate(Mark::Value, Doc::text("ready")),
]));
let prepared = source.prepare()?;
let options = RenderOptions::new(NonZeroU32::new(12).unwrap())
    .with_strategy(LayoutStrategy::Exact);
let mut workspace = RenderWorkspace::growable();
let rendered = render_into(&prepared, options, &mut workspace)?;
assert_eq!(rendered.text(), "status ready");
# Ok::<(), Box<dyn std::error::Error>>(())
```

The borrowed result and its annotation resolvers remain valid only while the
workspace is exclusively borrowed. Use `render()` for an ergonomic owned
result. For predictable repeated rendering, warm the exact prepared
document/width/strategy, retain `workspace.capacity()`, then switch to Fixed:

```rust
# use std::num::NonZeroU32;
# use laidout::{Doc, LayoutStrategy, RenderOptions, RenderWorkspace, WorkspaceMode, render_into};
let prepared = Doc::<u32>::group(Doc::concat([
    Doc::text("alpha"), Doc::line(), Doc::text("beta"),
])).prepare()?;
let options = RenderOptions::new(NonZeroU32::new(8).unwrap())
    .with_strategy(LayoutStrategy::Fast);
let mut workspace = RenderWorkspace::growable();
drop(render_into(&prepared, options, &mut workspace)?);
let frozen = workspace.capacity();
workspace.set_mode(WorkspaceMode::Fixed);
drop(render_into(&prepared, options, &mut workspace)?); // zero allocator calls
assert_eq!(workspace.capacity(), frozen);
# Ok::<(), Box<dyn std::error::Error>>(())
```

Insufficient Fixed storage returns a typed `WorkspaceExhausted`; it never
changes strategy or publishes an approximate layout. `release_capacity()`
drops retained high-water storage. A later Growable call can regrow it, while a
later Fixed call reports ordinary capacity exhaustion.

## Semantics

- Text is validated and measured once. Engines and tables use the stored
  Unicode display width; line breaks remain explicit structural document nodes.
- Fast groups use prepared first-stop summaries and never rescan the pending
  document. `Fill` probes only its next child. Zero-width text consumes zero
  columns and no synthetic scan budget exists.
- Exact rendering uses stable Pareto frontiers, ordered-choice ties, structural
  annotations and penalties, and a checked `(u128 overflow, u64 burden)` cost.
- Preparation rejects consumer documents whose complete candidate output or
  cost domain cannot be represented. The `research` feature retains unbounded
  two-`BigUint` costs and generic oracle engines.
- Laidout has no downstream console dependency or vocabulary. The standalone
  example and integration test use an application-owned annotation enum.

The proof and verification surfaces are [the prepared cost proof](docs/cost-model-proofs.md),
[the 0.2 migration guide](docs/migration-0.2.md), and
[`examples/standalone.rs`](examples/standalone.rs).

```sh
cargo test --all-targets
cargo test --all-targets --features research
sh proofs/check-cost-model-laws.sh
```

Laidout is MIT licensed.
