# Handoff: extracting from the Go prototype

**Source repo:** `~/Library/Mobile Documents/com~apple~CloudDocs/Code/pretty`
(Go, known-broken, multiple overlapping rewrite attempts).
**Destination:** this crate. All citations below are `path:startLine-endLine`
relative to the source repo root, verified 2026-07-18.

The one-line map: **`internal/layouter` contributes the algebra surface,
`internal/doc` contributes the engine ambitions, `internal/doctype`
contributes the text-ingestion story.** Each package has one load-bearing
idea plus scaffolding to leave behind.

---

## 1. `internal/layouter` — take the document surface

### Already absorbed into this crate

- **`Union` as the primitive, `Group` as derived sugar.**
  `internal/layouter/document.go:163-168` (`Union`, with the `flat == broken`
  collapse) and `:170-173` (`Group(docs...) = Union(Flatten(doc), doc)`).
  Ported as `Doc::Choice` + `doc::group` in `src/doc.rs`.
- **`Flatten` semantics.** `internal/layouter/document.go:297-322` — note
  `linebreak → d.flat` (`:313-314`) and `union → d.flat` (`:305-306`).
  Ported as `doc::flatten`; the Rust version returns `None` on hardline
  instead of the Go trick of making `HardLine` its own lazy flat form
  (`:97-99`).

### To take

- **Combinator vocabulary.** `internal/layouter/document.go:101-126` —
  `Space`, `Comma`, `LBrace`, `DQuote`, etc. Port as a `tokens` module when
  real formatters land. Trivial, pure ergonomics.
- **Text flags.** `internal/layouter/document.go:37-53`
  (`FlagWhitespace/FlagSymbol/FlagIndent/FlagNewline/FlagWord`, plus the
  `FlagCustom` split at bit 32). Do not port the bitfield; port the intent —
  text runs carry their kind — onto `TagId`s. This is the embryo of the
  text-classification design that ended up in the koda-console proposal.
- **`Structured()` raw-text lexer.** `internal/layouter/structured.go:13-106`.
  A no-regex state machine (word / symbol / whitespace / indent, states at
  `:23-28`, dispatch loop at `:54-101`) that turns arbitrary text into a
  document. This is the ingestion half of the fill/reflow story — the
  workload where the frontier engine should visibly beat greedy (greedy ties
  it on JSON-style group nesting). Highest-priority port.
- **`Indented` vs `Nested`** — `internal/layouter/document.go:147-153`
  (`Nested`: indent by offset) vs `:155-161` (`Indented`: indent to current
  position, i.e. the classic `align` combinator). `Align` is missing from
  the Rust algebra; cheap to add in the frontier engine (indent := current
  column), expensive to bolt on later.

### Take as an open research question, not code

- **`Contextual` / `Lazy` documents.** `internal/layouter/document.go:128-133`
  and `:175-177`, resolved during layout at `:211-217`. A document whose
  shape depends on the layout context breaks the purity that memoization and
  optimality arguments rest on. The fully commented-out table builder —
  `internal/layouter/table.go:1-183`, see the contextual measuring sketch at
  `:52-101` and cell alignment at `:132-169` — is *why* they wanted it:
  column alignment needs cross-row measurement. Treat "can aligned tables be
  expressed without contextual callbacks" as a research question. Take the
  requirements from `table.go`, not the mechanism.

### Leave behind

- The greedy `Fits`-based union resolution
  (`internal/layouter/document.go:253-259`) — `src/greedy.rs` already *is*
  this, deliberately, as the baseline.
- The flat-width estimator `Width()`
  (`internal/layouter/document.go:265-295`) — byte-length based
  (`len(d.text)`, `:267-268`), superseded by `cost::display_width`.
- `tags.go` (`DelimitedString`), `wrapper.go`, the `Layout`/`Token`
  interface plumbing (`document.go:8-35`).

---

## 2. `internal/doc` — take the measure monoid, discard its verdicts

### Take

- **`Measurement` and its composition law.**
  `internal/doc/measure.go:3-9` (the measure: width, remaining, break,
  height, last-line) and `:15-38` (`Measurement.Add` — the monoid append,
  handling the broken/unbroken last-line cases). This is the same measure
  Bernardy (ICFP 2017) builds Pareto frontiers over, and this composition
  has already been debugged once. The current Rust engine is top-down and
  carries only `last` per candidate (the cost model absorbs the rest); port
  `Add` when building the bottom-up frontier variant for comparison.
- **The memoization intent.** `internal/doc/document.go:3-7` (every
  `Document` must provide `Hash() uint64`); `internal/doc/context.go:14-19`
  (`LayoutKey{State, Next, Position, Mode}` — the memo key shape) and
  `:52-67` (`Chain.Layout`: cache lookup around continuation layout).
  Note: `internal/doc/cache.go` is an **empty file** (one `package` line) —
  the caching all lives in `context.go`. The Rust engine memoizes on `Rc`
  pointer identity, which only shares within one physically-shared tree;
  structural hashing (hash-consing) turns rebuilt-but-equal subtrees
  (every `", "` separator a formatter constructs) into memo hits. The
  cleanest statement of per-node structural hashing is actually in the
  *other* package: `internal/doctype/hash.go:11-51`.
- **The examples corpus.** `internal/doc/examples/json.go:17-142` — already
  ported to `src/corpus.rs` (minus the `maxInline` heuristic at `:57-64`,
  which an optimal engine makes obsolete).
  Next: `internal/doc/examples/ast.go:43-166` (Go-like source formatting:
  `Stack`ed decls `:43-53`, grouped imports `:63-91`, function signatures
  with softline-wrapped params `:124-166`) — expression/signature trees
  produce the asymmetric choices where optimal visibly wins. The expected
  output is documented in the trailing comment `:199-216`.
  `internal/doc/examples/sql.go` is 4 lines, effectively empty — nothing to
  take, but `internal/doctype/sql_test.go:1-388` has SQL corpus material.

### Discard

- **The heuristic verdicts.** `internal/doc/measure.go:40-54` — `IsCompact`
  (`Width <= LastLine*2`), `HasGoodBreaks` (`LastLine < Width/2`),
  `PreferFlat` (`Width <= lineLength*3/4`). Hand-tuned guesses standing in
  for a cost model; unverifiable, and exactly what the `CostModel` trait +
  brute-force oracle replace. Same for the guessy `Context.Measure`
  (`internal/doc/context.go:162-186`), which fabricates `Height: 2` when any
  break exists.
- **The CPS `Chain`/`Context` machinery.** `internal/doc/context.go:21-67`
  (continuation chain), `:74-124` (context threading), `:201-241` (the
  mode-switch replay in `ShouldRenderBreakAtPosition`). This is the part
  that entangled measurement, caching, and rendering so tightly the package
  never reached a testable state — it doesn't even compile
  (`context.go:44-46` references a `c.cache` field that doesn't exist on
  `Chain`; `:97-110` has a missing return). The frontier engine
  (`src/frontier.rs`) is its replacement, not its port.
- **`Tags` as a null-separated string.** `internal/doc/tags.go:12-30` and
  the ~400 lines of set operations on it. A Go allocation trick, not a
  design; interned `TagId`s cover it. One semantic worth remembering: tags
  *merge* when nested (`tags.go:98-138`, sorted-merge `Add`), whereas the
  Rust `Tag` currently shadows the outer tag. If overlapping annotations are
  ever needed, port the merge semantics, never the encoding.

---

## 3. `internal/doctype` — take the spec for text ingestion

This package is two rewrite attempts deep and mostly dead (`DocFluid`'s core
is commented out — `internal/doctype/fluid.go:19-24` — and its
`Chain`/`Layout`/`Measure` methods are stubs, `:33-43`). But it is the only
place that answers "how does *unstructured* text become a document."

### Take

- **`StructuredText`.** `internal/doctype/fluid.go:144-241`. The complete
  ingestion state machine: word/symbol/indent lexing (`:193-232`), per-line
  indentation capture (`:199-207`), and — the key move — whitespace emitted
  as `Union(Text(ws), Newline)` (`:174-175`): every inter-word gap is an
  independent break opportunity, which is fill/reflow semantics expressed in
  the plain algebra with no special `Fill` node. This plus
  `layouter/structured.go` is the specification for a Rust `from_text`
  builder.
- **The line/group builder model.** `internal/doctype/fluid.go:54-142`
  (`textline`: indent + docs per line, `:54-76`; `GroupBuilder`: commit
  lines, diff indentation between consecutive lines, `:78-142`).
  Half-finished (the `res < 0` dedent case at `:134-135` is an empty stub)
  — take the shape, finish the logic in Rust.
- **`BuildIndentTree`.** `internal/doctype/indent.go:8-59` — group
  consecutive same-indent lines, join with `HardLine`, wrap on indent
  change; `applyIndentation` at `:62-84` (apply indent reps outside-in).
  Small, finished, and covered by `internal/doctype/indent_test.go:1-167`.
  This is the import path for indented prose/source.
- **Structural hashing.** `internal/doctype/hash.go:11-51` — per-node-type
  hash builders (`DocText` hashes content+flags, `DocUnion` hashes both
  arms, `DocConcat` hashes the sequence). The template for hash-consed memo
  keys in the Rust engine (backlog item 3 below), together with the
  `internal/hash` builder package it leans on.
- **Test corpora.** `internal/doctype/fluid_test.go:1-215` and
  `internal/doctype/sql_test.go:1-388` — harvest as fixtures even though the
  implementations they exercise are abandoned.

### Take only if round-tripping source text

- **The `Indentation` type's tab/space fidelity** (mixed runs,
  `Compare`/`Difference`, used at `fluid.go:124-136` and
  `indent.go:62-84`; rendering split in
  `internal/doc/context.go:257-297`). For terminal output, plain column
  counts suffice; this matters only for source-preserving reformatting.

### Leave behind

- The `Hash/Chain/Layout` CPS `Document` interface (same dead end as
  `internal/doc`), `DocFluid` itself (`fluid.go:10-43`), and the
  `immutable.List` machinery.

---

## Extraction backlog, in order

1. **`from_text` fluid builder** — spec: `internal/doctype/fluid.go:144-241`
   + `internal/layouter/structured.go:13-106` + `internal/doctype/indent.go:8-59`.
   Creates the per-word-choice workload where frontier beats greedy — the
   research result this crate exists to demonstrate.
2. **`Align` node** — spec: `internal/layouter/document.go:155-161`
   (`Indented`).
3. **Hash-consed memo keys** — spec: `internal/doctype/hash.go:11-51` +
   `internal/doc/document.go:3-7`; replaces `Rc`-pointer identity in
   `src/frontier.rs`.
4. **AST corpus** — port `internal/doc/examples/ast.go:43-166`.
5. **Bernardy measure monoid** — port `internal/doc/measure.go:3-38`
   (`Add` only, never `:40-54`) for a bottom-up frontier engine variant to
   compare against the current top-down one.
6. **Open question** — aligned tables without contextual callbacks;
   requirements in `internal/layouter/table.go:11-23` and `:132-169`.

## What already made the jump

| Go source | This crate |
|---|---|
| `layouter/document.go:163-173` (`Union`/`Group`) | `src/doc.rs` (`Choice`, `group`) |
| `layouter/document.go:297-322` (`Flatten`) | `src/doc.rs` (`flatten`, `Option`-returning) |
| `doc` package's optimality/memoization ambition | `src/frontier.rs` (memoized Pareto frontiers, verified against `src/brute.rs`) |
| `doc/measure.go:40-54` heuristics | **replaced** by `src/cost.rs` (`CostModel`, incrementality contract) |
| `layouter/document.go:253-259` (greedy `Fits`) | `src/greedy.rs` (continuation-aware, kept as baseline) |
| `doc/examples/json.go:17-142` | `src/corpus.rs` (minus `maxInline`, `:57-64`) |
| Tags orthogonal to layout (`doc/tags.go`, `layouter` `annotated`) | `Doc::Tag` + `Option<TagId>` on output spans |
