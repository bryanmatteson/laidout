# laidout

An exact pretty-printer in Rust with a consumer-ready rendering facade:
parametric-cost **optimal layout** via memoized Pareto-frontier search, kept
honest by a brute-force oracle and the classical greedy printer. Documents
carry authoritative terminal widths, nested annotations, and branch-local
penalties; exact rendering returns owned text, lossless byte spans, cost,
solver statistics, and typed resource-limit failures.

```rust
use laidout::{concat, render, tag, text, RenderOptions};

let doc = tag(100, concat([text("status "), tag(101, text("界"))]));
let rendered = render(&doc, &RenderOptions::new(80))?;
assert_eq!(rendered.text, "status 界");
assert_eq!(rendered.annotated_runs()[1].tags, [100, 101]);
# Ok::<(), laidout::RenderError>(())
```

## Architecture

One document algebra, one cost definition, three engines:

| Module | Role |
|---|---|
| `doc` | The algebra: measured `TextRun`, `Line`, `Concat`, `Nest`, `Align`, `Choice`, `Tag`, and `Penalty`. `Choice` is the primitive; `group` is sugar via `flatten`. |
| `cost` | Open `CostModel` for non-pruning engines, sealed `LawfulCostModel` for exact search, research `OverflowThenHeight`, and consumer `ConsumerCostModel`. |
| `render` | Structural output events, nested byte spans, annotated runs, shared `cost_of_out`, and the high-level exact `render` facade. |
| `brute` | Exhaustive enumeration of every choice assignment. Exponential. Ground truth. |
| `greedy` | Wadler/Leijen first-fit with a continuation-aware `fits` scan. The baseline to beat. |
| `frontier` | Exact iterative search memoized on `(node, column, indent)`, with deterministic statistics and resource limits. |
| `text` | Fallible raw-text ingestion with configured Unicode width and tab-stop normalization, plus an independent break choice at every horizontal whitespace run. |
| `tags` / `tokens` | Built-in text-kind tags and classified punctuation/whitespace constructors. |
| `measure` | An associative `(height, widest, first, last)` line-shape monoid for bottom-up experiments. |
| `table` | Pure static compilation of flat cells into aligned compact and vertically stacked fallback documents. |
| `corpus` | JSON, Go-like AST, SQL, and indented-prose corpora recovered from the prototype. The AST signature fixture contains a strict optimal-over-greedy case. |

## The proved cost-model contract

Exact cost models are **incremental**: placing a run of width `a + b` at column
`c` costs exactly `text(c, a) + text(c + a, b)`. The overflow models use a
difference-of-squares formulation, so a line ending at column `e` costs
`max(0, e − W)²` no matter how engines chunk it. The shared `cost_of_out`
walk reconstructs text, newline, indentation, and penalty cost events and is
property-tested against frontier accumulation.

Frontier pruning additionally requires the model to be **monotone in column**
(`text(c, w)` nondecreasing in `c`), which makes `(cost, last-column)`
dominance sound. A `LawfulCostModel`'s `add` must be associative with `zero` as its
two-sided identity, and penalties must be nonnegative and independent of text
chunking. The dominant lexicographic component is an exact `BigUint`; bounded
saturation there would violate addition monotonicity at `u64::MAX`.

The exact engine accepts only the sealed `LawfulCostModel` trait. Its two
built-in implementations are proved algebraically and by counterexample-free
SMT obligations in [the cost-model proof](docs/cost-model-proofs.md). Arbitrary
`CostModel` implementations remain usable by brute force, greedy layout, and
output-cost reconstruction, none of which prunes from these laws.

## The recipe

1. **Brute force first.** `brute::best` enumerates everything; nothing subtle
   can hide in it. Every optimization claim is an equality test against it.
2. **Greedy second.** The known-good classical algorithm bounds the frontier
   engine from above: optimal must never cost more.
3. **Frontier last.** The cost algebra is proved first; the engine is then
   checked by the differential properties in `tests/properties.rs`:
   - `frontier == brute` on cost (optimality),
   - `frontier <= greedy` on cost,
   - whitespace-stripped content identical across all engines and layouts,
   - accumulated cost equals structural output-event cost,
   - selected text and nested spans equal brute force,
   - byte-for-byte determinism,
   - associative measurement under tree reassociation.

Run the Rust suite with `cargo test` and the algebra proof with
`sh proofs/check-cost-model-laws.sh`.

## Raw-text reflow

`from_text` preserves physical newlines and leading indentation while turning
horizontal whitespace into layout choices:

```rust
use laidout::{from_text, render, RenderOptions};

let doc = from_text("  a bb cccc")?;
let rendered = render(&doc, &RenderOptions::new(6))?;
assert_eq!(rendered.text, "  a bb\n  cccc");
# Ok::<(), Box<dyn std::error::Error>>(())
```

At width 6, first-fit greedy uses three lines for `a bb cccc`; the optimal
frontier uses two. `tests/text.rs` locks that research result against the
brute-force oracle where applicable.

## Aligned tables

Tables measure flat cell projections once, then compile to ordinary
`Choice`, `Concat`, `HardLine`, `Tag`, and `Align` nodes:

```rust
use laidout::{table, text, Alignment, Column};

let doc = table([
    Column::labeled(text("NAME")),
    Column::labeled(text("COUNT")).alignment(Alignment::Right),
])
.header()
.row([text("alpha"), text("7")])
.row([text("beta"), text("123")])
.build()
.expect("cells have flat projections");
```

The compact branch aligns columns; the fallback stacks cells when compact
layout would overflow. Non-flattenable headers and cells return a
coordinate-bearing `TableError`. Choice-bearing cells are also rejected: the
static compiler never silently discards a cell's layout alternatives.

## Text and annotation guarantees

- A `TextRun` is measured once under narrow or CJK Unicode-width rules. Engines
  and tables consume its stored columns and never silently remeasure its text.
- `try_text` rejects control characters. `from_text` is fallible, normalizes
  carriage returns to line structure, and expands tabs to configured source
  tab stops before constructing text runs.
- Nested, repeated, identical-range, and empty tags survive as preorder
  `AnnotationSpan` values. `annotated_runs()` is the terminal styling surface
  and reports active tags outer-to-inner.
- Memo keys use structurally interned document IDs, so rebuilt-equal subtrees
  share entries. Columns remain exact raw values; resource limits return an
  error and never relabel an approximate result as exact.
- A `Line` node always breaks; flat alternatives exist only through `Choice`
  (which is how `group` constructs them). This keeps engine semantics tiny
  at the cost of Wadler's per-group flat mode being a derived notion.
- The implemented table compiler accepts flat cell projections. General
  tables whose cells retain internal layout choices still require the
  first-class-node design described in
  [`docs/aligned-tables.md`](docs/aligned-tables.md).

## Design documents

- [Consumer rendering and annotation contract](docs/consumer-rendering.md) —
  the implemented contract for the public render facade, nested spans, stable
  text measurement, penalties, and exact solver diagnostics.
- [Aligned tables without contextual callbacks](docs/aligned-tables.md) — the
  completed static flat-cell design and the boundary for future rich tables.

## Reading list

- Wadler, *A prettier printer* (2003) — the algebra and the greedy baseline.
- Bernardy, *A Pretty But Not Greedy Printer* (ICFP 2017) — Pareto frontiers
  over (height, max-width, last-width) measures.
- Porncharoenwase, Nguyen, Torlak, *A Pretty Expressive Printer*
  (OOPSLA 2023) — parametric cost models, column clamping, the incremental
  cost discipline used here.
- Podkopaev & Boulytchev, *Polynomial-time optimal pretty-printing
  combinators with choice* — measure-based memoization.
