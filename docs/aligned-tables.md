# Aligned tables without contextual callbacks

## Decision boundary

Do not add `Contextual` or lazy callback documents. A callback whose result
depends on the current column or remaining width makes document identity
context-dependent, which breaks structural memoization and weakens the
optimality argument.

The open question is narrower: **can a pure table description be compiled or
solved into ordinary document choices while preserving global column
alignment and optimal layout?**

## Requirements recovered from the Go prototype

A table surface must support:

- zero or more rows and an optional header;
- per-column left, right, or center alignment;
- per-column minimum padding;
- missing column specifications filled with left-aligned defaults;
- column widths derived across headers and rows using display width, never
  byte length;
- placement at the current column, equivalent to `Align` for subsequent rows;
- a compact aligned layout when it fits the available width;
- an explicit fallback layout when it does not fit;
- semantic tags preserved through padding and both layout alternatives.

## Pure formulations worth testing

### Static compilation for flat cells

Require every cell to have a flat projection. The builder measures those
projections, calculates column widths once, pads each cell, and returns a
`Choice(compact_table, fallback_rows)` made entirely from ordinary `Doc`
nodes.

This is sufficient for terminal status tables whose cells are atomic text.
It is not a general solution: a cell with no flat projection or with a
cost-sensitive internal layout cannot participate without losing choices.

### A first-class `Table` node

Store rows and column specifications as immutable data. An engine solves cell
frontiers, carries a vector of column widths as part of the candidate measure,
and emits aligned rows only after combining all cells.

This keeps the document pure and structurally hashable, but expands every
engine and the brute-force oracle. The state-space risk is the cross-product
of cell frontiers and width vectors; dominance rules must be specified before
implementation.

## Required verifier

Any implementation must include:

1. exact fixtures for headers, ragged rows, and left/right/center alignment;
2. a non-ASCII display-width fixture;
3. nested placement proving subsequent rows align to the table's starting
   column;
4. a compact-fit and overflow-fallback fixture for the same table;
5. tag-preservation checks through padding and fallback;
6. brute-force equality for bounded tables;
7. deterministic output and structural-memo reuse for rebuilt-equal tables;
8. an invalid fixture for a non-flattenable cell if static compilation is
   chosen.

## STOP criteria

Stop and redesign if any implementation:

- invokes user code during layout;
- includes mutable or ambient layout context in document identity;
- uses byte length for column width;
- commits to a compact/fallback branch before the active cost model can score
  both;
- prunes a width-vector candidate without a proved dominance relation; or
- cannot be represented in the brute-force oracle used as ground truth.

The static flat-cell compiler is the smallest coherent implementation. A
general table combinator requires the first-class-node design and its full
cross-engine verifier; it should not be approximated with callbacks.
