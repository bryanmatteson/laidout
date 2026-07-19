# Consumer rendering and annotation contract

Status: implementation-ready design for one coherent agent execution.

## Decision

`laidout` will become usable by terminal consumers before it grows more
research-only layout machinery. The implementation delivers one public render
facade, lossless nested annotations, validated and stable text measurement,
branch-local penalties, exact solver diagnostics, and deterministic resource
limits together.

These are not independent calendar phases. They change the same document,
output, cost, engine, table, and verification contracts, so none is considered
complete until the entire acceptance suite passes.

Source-formatting support remains important. This contract absorbs the API
shape needed by future source mappings and comment layout, but does not pretend
that end-of-line comment queuing is a rendering-only concern.

## Coherence boundary

The implementation includes every surface that defines or consumes a layout:

- `Doc` construction, equality, hashing, flattening, and choice counting;
- terminal-column measurement and raw-text ingestion;
- static table measurement and padding;
- the brute-force, greedy, and frontier engines;
- cost-model events and post-layout cost verification;
- the output rope and annotation flattening;
- the high-level consumer render API, errors, limits, and statistics;
- corpus formatters, examples, property tests, and public documentation.

Changing only the high-level facade is not a valid implementation. In
particular, a render-time width policy cannot coexist with tables measured
under a construction-time global policy, and a branch penalty cannot coexist
with greedy cost reconstructed only from rendered lines.

## Required invariants

1. **One text run has one authoritative display width.** Every engine, table,
   and verifier reads the width stored in the document; no component silently
   remeasures the same run under different policy.
2. **Text never contains terminal layout controls.** Tabs are expanded during
   raw-text ingestion. Line structure remains `Doc::Line`; ANSI escapes and
   other control characters are rejected.
3. **Annotations are structural and lossless.** Nested and repeated tags remain
   distinguishable even when they cover identical byte ranges.
4. **Annotations and penalties do not influence document identity through
   ambient state.** They are ordinary immutable nodes or output events.
5. **All layout cost is reconstructible from output events.** Text, newline,
   indentation, and penalty events participate in the same shared cost walk.
6. **Frontier results remain exact by default.** A resource limit returns an
   error with statistics, never an unlabeled approximate candidate.
7. **The brute-force engine remains the ground truth.** Every new document or
   output event is implemented in the oracle before frontier equality is
   claimed.
8. **Consumer input errors are typed.** The high-level path returns errors for
   invalid text and exhausted limits rather than panicking or emitting a
   partially measured layout.

## Public document and text surface

### Stored text measurement

Replace `Doc::Text(Rc<str>)` with an immutable text-run value whose width is
calculated exactly once:

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct TextRun {
    value: Rc<str>,
    columns: u32,
}

#[non_exhaustive]
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum Doc {
    Empty,
    Text(TextRun),
    // existing structural variants
    Penalty { amount: u32, doc: Rc<Doc> },
}
```

`TextRun` exposes read-only `value()` and `columns()` accessors. Its fields are
not public, so callers cannot forge a width that disagrees with the text.
`Doc` becomes `#[non_exhaustive]`; consumers construct documents through
functions and cannot assume the algebra will never gain a comment-layout node.

### Width policy and constructors

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WidthMode {
    #[default]
    Narrow,
    Cjk,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IngestOptions {
    pub width_mode: WidthMode,
    pub tab_width: NonZeroU8,
}

pub fn try_text(value: impl AsRef<str>) -> Result<Rc<Doc>, TextError>;
pub fn try_text_with(
    value: impl AsRef<str>,
    width_mode: WidthMode,
) -> Result<Rc<Doc>, TextError>;

// Convenience for trusted literals. Panics with the TextError message.
pub fn text(value: impl AsRef<str>) -> Rc<Doc>;

pub fn from_text(input: &str) -> Result<Rc<Doc>, TextError>;
pub fn from_text_with(
    input: &str,
    options: IngestOptions,
) -> Result<Rc<Doc>, TextError>;
```

`try_text*` rejects every `char::is_control()` character, including line feed,
carriage return, tab, escape, and delete. `text` keeps the existing ergonomic
literal constructor but is explicitly the panicking adapter over `try_text`.
Untrusted consumer input uses the fallible functions.

`from_text_with` applies these rules before constructing text nodes:

1. normalize CRLF and lone CR to LF;
2. treat LF as `hardline` structure;
3. expand each tab to the next configured source-line tab stop;
4. advance the source column using the selected Unicode width mode;
5. reject any remaining control character with its UTF-8 byte offset;
6. classify and tag the normalized text as today.

A tab at source column `c` expands to
`tab_width - (c % tab_width)` ASCII spaces. The wide-layout guarantee becomes
equality with the normalized input, not byte-for-byte equality with literal
tabs or carriage returns.

`TextError` is `#[non_exhaustive]` and distinguishes at least control
character plus original-input byte offset and display-width overflow.
`IngestOptions::default` uses eight-column tab stops; callers must not infer
the value from terminal state.

### Table compatibility

The static table compiler obtains cell widths from `TextRun::columns()` while
walking flat projections. It never calls a global width function. Padding is
ASCII space text with its exact stored width.

Two equal text runs therefore include the same value and width. The same
string constructed under narrow and CJK modes may be structurally unequal,
which is required: their table and frontier states are not interchangeable.

## Penalty and cost contract

Add an ordinary branch-local cost node:

```rust
pub fn penalize(amount: u32, doc: Rc<Doc>) -> Rc<Doc>;
```

`penalize(0, doc)` returns `doc`. Flattening preserves nonzero penalties around
the flattened child, and choice counting ignores them.

Extend the cost model:

```rust
pub trait CostModel {
    type Cost: Clone + Ord + Debug;

    fn zero(&self) -> Self::Cost;
    fn add(&self, a: &Self::Cost, b: &Self::Cost) -> Self::Cost;
    fn text(&self, col: u32, width: u32) -> Self::Cost;
    fn newline(&self) -> Self::Cost;
    fn penalty(&self, amount: u32) -> Self::Cost;
}

pub trait LawfulCostModel: CostModel { /* sealed */ }
```

The exact-model laws are binding for `LawfulCostModel`:

- `zero` is a two-sided identity and `add` is associative;
- `add` is monotone in both arguments;
- text cost is incremental under splitting;
- when `c1 <= c2`, `text(c1, width) <= text(c2, width)` for the same width;
- penalty cost is nonnegative and independent of column and text chunking.

The existing `OverflowThenHeight` remains the research baseline and maps
penalties to zero. Add a consumer default whose ordered cost is:

```rust
use num_bigint::BigUint;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ConsumerCost {
    pub overflow: BigUint,
    pub burden: u64,
}
```

Overflow remains lexicographically dominant. `burden` is the sum of configured
newline cost and explicit penalties, allowing a consumer to trade a stylistic
preference against additional lines without ever trading away overflow. The
default newline cost is one burden unit, so penalty amounts have a documented
unit.

Every built-in cost model receives an algebraic proof, machine-checked SMT
obligations, adversarial overflow regressions, and property tests for the laws
above. Exact frontier entry points require the sealed `LawfulCostModel` trait,
so arbitrary external implementations cannot enter the pruning engine.
External `CostModel` implementations remain valid for brute-force and greedy
engines, which do not rely on the laws to discard candidates.

## Output and annotation contract

Replace leaf-local optional tags with structural events:

```rust
#[non_exhaustive]
#[derive(Clone, Debug)]
pub enum Out {
    Empty,
    Text(TextRun),
    Newline(u32),
    Cat(Rc<Out>, Rc<Out>),
    Tagged(TagId, Rc<Out>),
    Penalty(u32),
}
```

`Doc::Tag` produces `Out::Tagged`; active tag state disappears from greedy
frames, frontier memo keys, and solver calls. `Doc::Penalty` emits an
`Out::Penalty` event as well as contributing cost during frontier search.

The shared output walk produces:

```rust
pub struct AnnotationSpan {
    pub tag: TagId,
    pub range: Range<usize>,
    pub parent: Option<usize>,
}

pub struct Rendered<C> {
    pub text: String,
    pub spans: Vec<AnnotationSpan>,
    pub cost: C,
    pub stats: SolveStats,
}

pub struct AnnotatedRun<'a> {
    pub text: &'a str,
    pub range: Range<usize>,
    pub tags: Vec<TagId>,
}
```

Ranges are UTF-8 byte ranges into `Rendered::text`. Spans appear in document
preorder, and `parent` indexes an earlier span. This preserves identical,
nested, repeated, and empty annotations without coalescing them. A tag around
a line break includes the emitted LF and generated indentation inside its
range.

Every range must be within `text`, begin and end on UTF-8 boundaries, and be
contained by its parent. `Rendered::annotated_runs()` returns a
`Vec<AnnotatedRun<'_>>`, splitting the text at span boundaries into nonempty
runs whose active tag IDs are ordered outer to inner. This is the terminal
consumer's styling interface; consumers do not have to reconstruct the nesting
tree themselves.

The canonical shared verifier becomes `cost_of_out`, which walks text,
newlines, indentation, and penalty events. `cost_of_lines` must not remain on
the semantic path because lines cannot reconstruct branch-local penalties.

## Consumer render facade

The crate root exports a high-level exact renderer:

```rust
#[non_exhaustive]
pub struct RenderOptions {
    // Constructed through RenderOptions::new(width) and builder methods.
    width: u32,
    newline_cost: u32,
    limits: SolveLimits,
}

pub fn render(
    doc: &Rc<Doc>,
    options: &RenderOptions,
) -> Result<Rendered<ConsumerCost>, RenderError>;

pub fn render_with<M: LawfulCostModel>(
    doc: &Rc<Doc>,
    model: &M,
    limits: SolveLimits,
) -> Result<Rendered<M::Cost>, RenderError>;
```

`render` constructs the consumer cost model from `RenderOptions` and invokes
the exact frontier engine. `render_with` is the advanced path for research and
custom models. Low-level engines remain public for differential testing, but
README examples and ordinary consumers use `render`.

`Rendered` owns its text and can be retained independently of the document.
Writing plain output is `writer.write_all(rendered.text.as_bytes())`; styled
terminal output iterates `annotated_runs()`. A separate streaming layout API is
not part of this contract because it does not change the valid document or
rendered artifact shape.

## Exact limits and diagnostics

The frontier engine records deterministic statistics:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SolveStats {
    pub solve_calls: u64,
    pub memo_hits: u64,
    pub memo_misses: u64,
    pub candidates_generated: u64,
    pub candidates_pruned: u64,
    pub peak_frontier: usize,
    pub memo_entries: usize,
    pub interned_documents: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SolveLimits {
    pub max_solve_calls: Option<u64>,
    pub max_candidates: Option<u64>,
    pub max_memo_entries: Option<usize>,
}
```

No wall-clock duration appears in `SolveStats`; the report is deterministic.
`solve_calls` counts every solve invocation, including memo hits;
`candidates_generated` counts candidates before pruning;
`candidates_pruned` counts the candidates removed by pruning; and
`peak_frontier` records the largest retained frontier. Memo and interning
counts describe the state retained at successful completion or error.

Limits are checked immediately after incrementing the event they count. A
maximum of `N` permits exactly `N` events; event `N + 1` returns a typed
`RenderError::LimitExceeded` containing the limit kind, configured value, and
current statistics. It returns no `Rendered` value and caches no partial result
under a completed memo key.

Unlimited exact solving remains the default. Column clamping is not smuggled
into `SolveLimits`.

## Engine obligations

### Brute force

- Expansion preserves `Tag` and `Penalty` nodes.
- A resolved document renders to `Out`, not only to `Vec<String>`.
- Cost is calculated through `cost_of_out`.
- Its selected text, spans, and cost are the oracle for bounded documents.

### Greedy

- `fits` ignores annotations and penalties because neither occupies columns.
- Layout emits `Tagged` and `Penalty` output events.
- Final cost comes from `cost_of_out`; it is not reconstructed from lines.
- Greedy remains a baseline and is not allowed to change optimal semantics.

### Frontier

- Memo keys contain interned document ID, column, and indentation only.
- Tag nesting lives in `Out`, not ambient solver state.
- Penalty candidates add `CostModel::penalty(amount)` and emit the event.
- Candidate generation, pruning, memoization, and interning update statistics.
- A limit error aborts the solve and propagates without selecting a candidate.

### Shared output

- One iterative traversal materializes text, spans, and reconstructed cost.
- Frontier's accumulated cost must equal reconstructed output cost.
- Rendering must not recurse on document depth and overflow the call stack for
  deeply concatenated consumer documents.

## Breaking migration contract

The crate is pre-release, so this implementation makes the coherent break
instead of retaining two semantic paths:

- `Doc::Text` changes shape to contain `TextRun`;
- `Doc` and `Out` become non-exhaustive;
- `Out::Text(_, Option<TagId>)` is removed rather than deprecated;
- low-level engine state and `frontier::Engine::solve` drop the active-tag
  argument;
- `CostModel` implementors must define `penalty`;
- exact frontier APIs require the sealed `LawfulCostModel` boundary;
- `from_text` becomes fallible and normalizes tabs and carriage returns;
- global string remeasurement is removed from engine and table paths;
- `cost_of_out` replaces line-only cost reconstruction;
- crate-root examples use `render` and `Rendered`, while low-level engine APIs
  remain available for research and oracle tests.

No compatibility adapter may reconstruct nested annotations from the old
innermost-tag representation: that information has already been lost. No
adapter may accept raw tabs while reporting their width as a fixed column.

## Source-formatting forward visibility

The following work is visible but runtime-gated:

- `gated:line-suffix-layout` — an end-of-line comment queue that participates
  in width, cost, choices, all three engines, and the brute-force oracle;
- `gated:bounded-approximation` — any clamped or tainted solver mode;
- `gated:rich-table-layout` — first-class tables whose cells retain choices;
- `gated:streaming-sink` — incremental delivery of the already-defined
  rendered event stream.

These gates do not defer current API shape. `Doc`, `Out`, and error enums are
non-exhaustive; annotations can represent source-map IDs; the render facade
already returns `Result`; and costs retain non-text events.

`line_suffix` must not later be implemented as post-render string surgery. A
suffix consumes columns and can change which choice is optimal. Its separate
design must specify pending-queue state, flushing at line boundaries and end of
document, interaction with nested choices, and oracle equivalence before the
node becomes valid input.

## Verifier matrix

Every row below is an implementation obligation, not an example backlog.

| Surface | Required valid fixtures | Required invalid or negative fixtures |
|---|---|---|
| Text construction | ASCII, wide Unicode, combining marks, ambiguous narrow/CJK width | LF, tab, ESC, NUL, lone control, width overflow report the correct byte offset or error |
| Raw ingestion | spaces, tabs at multiple source columns, CRLF, lone CR, empty lines, trailing newline | zero tab width is unrepresentable; remaining controls return `TextError` |
| Stored widths | every engine and table uses `TextRun::columns` | no call site remeasures `TextRun::value` during layout |
| Annotations | disjoint, nested, repeated ID, identical range, empty span, Unicode range, span across newline | out-of-bounds, non-character-boundary, or non-containing parent never appears |
| Annotated runs | terminal styling sees the correct outer-to-inner stack at every boundary | no empty run and no byte omitted or duplicated |
| Penalties | lower-penalty identical branch wins; penalty trades against configured newline burden; zero penalty canonicalizes | greedy post-hoc cost cannot omit a penalty; penalties never alter width |
| Cost laws | built-in models satisfy identity, associativity, incrementality, and column monotonicity under generated inputs | a test-only violating model fails the corresponding law property |
| Frontier | exact cost, text, and spans equal brute force on bounded generated documents | no pruning rule discards an oracle winner |
| Limits | each individual limit can be hit deterministically and returns its current stats | no partial `Rendered`, completed memo entry, or approximate result is returned |
| Tables | narrow and CJK text align using stored widths; tags survive compact and fallback forms | a table cannot be built with a width policy that later disagrees with its text nodes |
| Compatibility | existing JSON, AST, SQL, prose, and table fixtures retain intended output after normalized-input updates | no test is weakened to ignore cost, annotation, or content disagreement |
| Consumer facade | one example renders styled nested Unicode text using only crate-root APIs | example contains no direct `frontier::Engine`, `Out` traversal, or manual span reconstruction |

Property generators must include `Penalty`, nested `Tag`, both width modes,
and normalized raw text. The bounded oracle cap remains explicit; tests do not
raise it merely to hide unexpected choice growth.

## Dependency-ordered implementation

This order describes dependencies inside one execution. No intermediate item
is a releasable phase.

1. Introduce `TextRun`, width modes, fallible constructors, normalized
   ingestion, and table use of stored widths.
2. Introduce structural `Tagged` and `Penalty` output events plus span and
   cost materialization.
3. Extend `Doc`, `CostModel`, both built-in models, flattening, and all three
   engines for penalties and structural annotations.
4. Add exact statistics, limits, errors, and the high-level render facade.
5. Migrate corpora, examples, tests, and README usage to the consumer surface.
6. Run the complete verifier matrix and remove obsolete tag-state and
   line-only costing paths.

## Acceptance commands

All commands must pass from the repository root:

```text
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
cargo run --example consumer
sh proofs/check-cost-model-laws.sh
```

The consumer example must assert its expected text and span structure before
printing anything. A visually plausible terminal screenshot is not a
substitute for byte-range assertions.

## STOP criteria

Stop and redesign rather than forcing the verifier to pass if any of these is
true:

- a text value can be measured differently by a table and an engine;
- raw tab or ANSI control bytes can reach `Out::Text`;
- nested tags are flattened, coalesced, or represented only by the innermost
  ID;
- tag state remains part of frontier memoization;
- a penalty changes branch choice but cannot be reconstructed from `Out`;
- a built-in cost model violates incrementality or column monotonicity;
- frontier differs from brute force on cost, selected text, or spans;
- a resource limit returns a candidate, marks a partial memo result complete,
  or is nondeterministic for the same document;
- table tests pass only by reverting to global string remeasurement;
- `line_suffix` is introduced as post-render rewriting;
- a new error class exists only to turn a failing fixture into a passing one;
- public examples still require low-level frontier or rope knowledge.

Completion means the consumer facade is the documented default, every
interacting surface above implements the same contract, and all acceptance
commands and negative fixtures pass. Partial implementation is not completion.
