# Migrating from Laidout 0.2 to 0.3

Laidout 0.3 removes integration constraints while making public evolution
explicit. It is a breaking pre-1.0 release.

## Annotation bounds removed

`Doc<A>::prepare`, `render`, text ingestion, and tables no longer require
`A: Eq + Hash`. Annotation values are interned by shared `Arc` identity.
Repeated uses of the same annotation node remain one prepared annotation;
separately constructed equal values are no longer value-deduplicated.

Remove now-unnecessary derives or bounds. Code that depended on value
deduplication must share one annotated subtree or perform
application-level interning before constructing the document.

Use `Doc::map_annotations` to translate an existing document without flattening
its shared DAG:

```rust
use laidout::Doc;

let numeric = Doc::annotate(7_u32, Doc::text("value"));
let named = numeric.map_annotations(|tag| format!("token-{tag}"));
assert_eq!(named.prepare()?.annotation_count(), 1);
# Ok::<(), laidout::PrepareError>(())
```

## Text, tokens, and tables can use your annotation type

`from_text_with_annotations` classifies text with a
`FnMut(TextKind) -> A`. The `_with` token constructors and `Column<A>` /
`Table<A>` carry the same application-owned annotation type.

The existing `from_text`, `tokens::*`, `Column::new`, and `Column::labeled`
functions still produce built-in `u32` tags.

Generic code uses:

```rust
# use laidout::{Column, Doc, Table};
enum Mark { Header, Cell }

let table = Table::new([
    Column::with_header(Doc::annotate(Mark::Header, Doc::text("name"))),
])
.header()
.row([Doc::annotate(Mark::Cell, Doc::text("laidout"))]);
let doc = table.build()?;
# Ok::<(), laidout::TableError>(())
```

## Host-controlled display width

Use `Doc::text_with_columns(text, columns)` or
`TextRun::try_with_columns(text, columns)` when a host terminal or editor has
already measured a run. These constructors still reject embedded control
characters, but trust the supplied column count.

## Reuse allocations with `Renderer`

The free `render` function remains the shortest owned path. For repeated work,
replace an ad hoc workspace plus owned copying with `Renderer`:

```rust
# use std::num::NonZeroU32;
# use laidout::{Doc, RenderOptions, Renderer};
let prepared = Doc::<u32>::text("ready").prepare()?;
let options = RenderOptions::new(NonZeroU32::new(80).unwrap());
let mut renderer = Renderer::new();
let output = renderer.render_prepared(&prepared, options)?;
assert_eq!(output.text(), "ready");
# Ok::<(), Box<dyn std::error::Error>>(())
```

Allocation-controlled consumers retain direct access to `render_into` and
`solve_into`.

## Match errors and construct capacities defensively

Public error enums and resource enums are now `#[non_exhaustive]`. Add a
wildcard arm to matches outside the crate. Use each error's `kind()` method
when a stable category is sufficient.

Capacity structs are also non-exhaustive. Replace struct literals with:

- `FastSolveCapacity::new`
- `ExactSolveCapacity::new`
- `VisitCapacity::new`
- `MaterializeCapacity::new`
- `RenderCapacity::new`

The fields remain readable for telemetry and assertions.
