# laidout

A research pretty-printer in Rust: parametric-cost **optimal layout** via
memoized Pareto-frontier search, kept honest by a brute-force oracle and the
classical greedy printer. This is the Rust continuation of the `internal/doc`
design from the Go prototype — the measurement/memoization/optimality ideas,
rebuilt on a foundation that can be tested against ground truth from day one.

## Architecture

One document algebra, one cost definition, three engines:

| Module | Role |
|---|---|
| `doc` | The algebra: `Text`, `Line`, `Concat`, `Nest`, `Align`, `Choice`, `Tag`. `Choice` is the primitive; `group` is sugar via `flatten`. |
| `cost` | `CostModel` trait + default `OverflowThenHeight` (lexicographic squared-overflow, then line count). |
| `render` | Shared output rope, line rendering, and `cost_of_lines` so all engines are measured identically. |
| `brute` | Exhaustive enumeration of every choice assignment. Exponential. Ground truth. |
| `greedy` | Wadler/Leijen first-fit with a continuation-aware `fits` scan. The baseline to beat. |
| `frontier` | The engine under study: top-down search memoized on `(node, column, indent, tag)`, returning Pareto frontiers of `(cost, last-column)` candidates. |
| `text` | `from_text`: classified raw-text ingestion with an independent break choice at every horizontal whitespace run. |
| `tags` / `tokens` | Built-in text-kind tags and classified punctuation/whitespace constructors. |
| `measure` | An associative `(height, widest, first, last)` line-shape monoid for bottom-up experiments. |
| `table` | Pure static compilation of flat cells into aligned compact and vertically stacked fallback documents. |
| `corpus` | JSON, Go-like AST, SQL, and indented-prose corpora recovered from the prototype. The AST signature fixture contains a strict optimal-over-greedy case. |

## The cost-model contract

Cost models must be **incremental**: placing a run of width `a + b` at column
`c` costs exactly `text(c, a) + text(c + a, b)`. The default model achieves
this with a difference-of-squares formulation, so a line ending at column `e`
costs `max(0, e − W)²` no matter how the engines chunked it. This is what
lets `cost_of_lines` (post-hoc, per line) and the frontier engine (streaming,
per placement) agree exactly — and it is property-tested
(`frontier_cost_is_self_consistent`).

Frontier pruning additionally assumes the model is **monotone in column**
(`text(c, w)` nondecreasing in `c`), which makes `(cost, last-column)`
dominance sound.

## The recipe

1. **Brute force first.** `brute::best` enumerates everything; nothing subtle
   can hide in it. Every optimization claim is an equality test against it.
2. **Greedy second.** The known-good classical algorithm bounds the frontier
   engine from above: optimal must never cost more.
3. **Frontier last.** The interesting engine is only trusted to the extent
   the differential properties in `tests/properties.rs` hold:
   - `frontier == brute` on cost (optimality),
   - `frontier <= greedy` on cost,
   - whitespace-stripped content identical across all engines and layouts,
   - streaming cost equals post-hoc line cost,
   - byte-for-byte determinism,
   - associative measurement under tree reassociation.

Run everything with `cargo test`.

## Raw-text reflow

`from_text` preserves physical newlines and leading indentation while turning
horizontal whitespace into layout choices:

```rust
use laidout::{from_text, frontier, to_string, OverflowThenHeight};

let doc = from_text("  a bb cccc");
let best = frontier::best(&OverflowThenHeight { width: 6 }, &doc);
assert_eq!(to_string(&best.out), "  a bb\n  cccc");
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

## Notes and known simplifications

- `display_width` uses Unicode terminal-column width, including East Asian wide
  characters and zero-width combining marks, and every engine measures through
  that shared function.
- Memo keys use structurally interned document IDs, so rebuilt-equal subtrees
  share entries. Columns remain raw values (no clamping past the width). The
  Pretty-Expressive-style column clamp is the first performance lever to add
  when profiling on wider corpora.
- A `Line` node always breaks; flat alternatives exist only through `Choice`
  (which is how `group` constructs them). This keeps engine semantics tiny
  at the cost of Wadler's per-group flat mode being a derived notion.
- Tags (`Doc::Tag`) are carried through to output spans and never affect
  layout, mirroring the semantic-token separation in the Go prototype.
- The implemented table compiler accepts flat cell projections. General
  tables whose cells retain internal layout choices still require the
  first-class-node design described in
  [`docs/aligned-tables.md`](docs/aligned-tables.md).

## Reading list

- Wadler, *A prettier printer* (2003) — the algebra and the greedy baseline.
- Bernardy, *A Pretty But Not Greedy Printer* (ICFP 2017) — Pareto frontiers
  over (height, max-width, last-width) measures.
- Porncharoenwase, Nguyen, Torlak, *A Pretty Expressive Printer*
  (OOPSLA 2023) — parametric cost models, column clamping, the incremental
  cost discipline used here.
- Podkopaev & Boulytchev, *Polynomial-time optimal pretty-printing
  combinators with choice* — measure-based memoization.
