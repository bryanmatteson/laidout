# Prepared layout kernel and Termosaic integration

- Status: normative implementation contract
- Owners: `laidout` and `termosaic`
- Supersedes and incorporates: `docs/consumer-rendering.md` for the prepared/default consumer path; the retained research compatibility surface is governed by the explicit `research` sections below
- Inputs: the typed/prepared/workspace proposal, the zero-allocation refinement, and the live behavior of both repositories
- Review disposition: corrected and agent-executable; no unresolved design decisions
- Change control: once step 1 captures a baseline, this file is immutable for that implementation run; any normative edit requires design-plan rereview, a new plan digest, and recapture of affected baseline evidence before production migration resumes

## 1. Objective

Turn Laidout into a typed, prepared, workspace-driven layout kernel and make Termosaic consume that kernel without an intermediate plain-text rendering.

Laidout remains a standalone library. The dependency is strictly one-way: Termosaic depends on Laidout, while Laidout's manifest, default/research features, source modules, public types, errors, examples, and verification require no Termosaic crate or Termosaic type. The neutral Laidout API supports owned rendering, borrowed materialization, and arbitrary annotation visitors directly; Termosaic is one adapter built on that public surface.

The completed data path is:

```text
termosaic::Doc
    -> termosaic::PreparedDoc
    -> laidout::solve_into(RenderWorkspace: Growable | Fixed)
    -> LayoutRef::visit(TermosaicVisitor)
    -> HumanWriter reusable ANSI buffer
    -> one logical write_human call
    -> at most one progress suspension and one payload sink-lock scope
```

Preparation and optional reservation occur before the fixed measured boundary. Once adequately reserved, fixed solving, visiting, borrowed materialization, ANSI rendering, and failure cleanup perform no allocator calls. Growable execution uses the same algorithms and storage, but may enlarge only the named exhausted resource and reports allocation failure without changing strategy or publishing partial output.

The memory policy and layout strategy are independent. Fast and Exact each work with Growable and Fixed workspaces. A fixed operation either completes without growing any container or returns a typed capacity error and leaves every reusable buffer empty and ready for the next operation.

## 2. Success contract

The work is complete only when all of these statements are true:

1. Laidout exposes an opaque generic `Doc<A>` backed by `Arc` and a reusable `PreparedDoc<A>`.
2. `PreparedDoc<A>` owns compact prepared nodes, edges, text, and `Arc<A>` annotation references. Rendering never clones an `A` value.
3. The consumer solver uses a private model with checked prepared-domain `u128` overflow and `u64` burden. The public research solver retains `BigUint` for both unbounded components.
4. `RenderWorkspace` exposes first-class Growable and Fixed modes, and Fast and Exact work in all four combinations with identical strategy-specific results.
5. Fixed `solve_into` and `render_into` perform exactly zero allocator calls after successful preparation and reservation.
6. Growable `solve_into` and `render_into` enlarge only the exhausted typed resource, retain the new capacity for reuse, and never restart with or fall back to another strategy.
7. Laidout Fast fitting is column-accurate and has no semantic scan budget. Termosaic preserves its current document bytes except for the precisely classified synthetic-budget divergence described by V29.
8. Exact solving remains oracle-equal to Laidout's lawful exhaustive model for accepted prepared documents.
9. Preferred ties are stable and independent of hash-table iteration order.
10. Every fixed workspace resource reports its own typed exhaustion error before growth or partial publication, and every growable allocation failure identifies the resource it could not enlarge.
11. A failed solve, visit, materialization, or ANSI render leaves the Laidout workspace and Termosaic writer reusable.
12. Termosaic exposes ergonomic Fast and Exact document writing plus retained `HumanWriter::doc_strict`. In Fixed mode, solving, visiting, ANSI generation, terminal-newline handling, and cleanup perform exactly zero allocator calls. The complete `doc_strict` call also performs zero calls when no progress handle is active and the injected sink is preinitialized and nonallocating. Active progress remains semantically supported but is outside the allocator guarantee because `indicatif` controls that suspension path.
13. Owned Laidout rendering and ergonomic Termosaic document writing remain available as allocating wrappers.
14. Existing Termosaic document fixtures preserve exact bytes, including ANSI run coalescing, indentation, hard lines, and the terminal newline, except where V29 proves that the first changed fit decision was caused solely by exhaustion of the old synthetic budget. The canonical V29 fixture pins exact old/new bytes for that authorized behavior class.
15. Laidout integration remains exclusive to Termosaic's human-output path; machine-text and opaque-byte APIs, instance-owned width/theme/destination policy, and the absence of process-global output/workspace state remain intact.
16. All valid and invalid verification rows in section 14 are automated and pass.
17. A standalone Laidout consumer with its own annotation type can prepare, reserve/warm, run Fast or Exact in Growable or Fixed mode, visit events, and materialize owned/borrowed output without depending on Termosaic.
18. Final destruction of library-owned source-graph edges is iterative, including unique deep chains, branching graphs, shared DAGs, and concurrent final drops of cloned roots.
19. Every selected layout is a lossless structural witness: text, newlines, indentation, penalties, nested annotations, and zero-width annotations can be replayed to reconstruct the rendered bytes, spans, and exact consumer cost independently of solver state.
20. Exact pruning publishes plan nodes only for candidates retained in a completed frontier; candidates rejected during dominance create no persistent plan nodes.
21. The `research` feature retains arbitrary downstream `CostModel` support in brute-force and generic greedy engines, while only sealed proved models enter the research pruning frontier.

## 3. Scope and change classification

### 3.1 Included surfaces

- Generic `Arc`-owned Laidout documents and annotations.
- Prepared dense document IR with checked consumer-domain metadata.
- Fast and exact consumer solvers over the same reusable Growable/Fixed workspace abstraction.
- Allocation-free plan visitation and borrowed text/span materialization.
- Owned compatibility wrappers built on the prepared kernel.
- A Termosaic document wrapper over `laidout::Doc<TokenId>`.
- A Termosaic prepared-document wrapper and retained reusable writer storage.
- Growable ergonomic ANSI rendering and allocation-free fixed ANSI rendering for `doc_strict`.
- Error, fixture, feature, documentation, example, and test migration required by those public API changes.
- Counting-allocator verification in both repositories.
- A standalone Laidout example and external-consumer integration test with no Termosaic dependency.
- Iterative destruction of arbitrarily deep Laidout source graphs.

### 3.2 Surfaces outside this contract

These items are not acceptance requirements and must not be silently pulled into this change:

- Migrating Termosaic fixed or adaptive tables onto Laidout.
- Zero-allocation strict APIs for `spans`, `line`, tables, JSON, machine text, or opaque bytes.
- Incremental layout emission to an external sink. Document bytes are fully buffered before `write_human`, but `Write::write_all` may perform multiple short writes and may fail after a partial external write.
- Workspace-backed arbitrary precision for compact documents outside the checked `u128` consumer domain; those documents remain accepted only by `research`.
- Incremental mutation of a prepared document.
- Persistence of strict scratch space in `DestinationState` for temporary `console.human(...).doc(...)` chains.
- A `line_suffix` operation. It is absent from the `0.2.0` algebra and cannot be emulated by post-render string surgery because suffixes consume columns and can change the optimal layout.
- A bounded/beam approximate solver reported through the exact API. No candidate cap, column clamp outside the selected indentation policy, or tainted result is labeled Exact.
- First-class rich table nodes whose cells retain layout choices; existing Laidout table helpers are migrated only to the new document handle and Termosaic tables remain on their current path.

### 3.3 Removed or replaced surfaces

- Laidout's public enum-shaped `Doc` becomes an opaque generic handle. Construction moves to methods/free constructors.
- Termosaic's public enum-shaped `Doc` becomes an opaque wrapper. Existing variant spellings are migrated to constructors.
- The current generic allocating frontier engine remains only in the `research` API; it is not used by strict consumer rendering.
- The current Termosaic `layout(doc) -> Vec<LayoutEvent>` production path is replaced for document writes. A frozen copy remains only in a test target as the independent Fast compatibility oracle; no production module calls or exports it.
- The prior consumer contract's `#[non_exhaustive]` error policy is intentionally replaced for the new default kernel: every public error/resource/component enum governed here is exhaustive in `0.2.0`, enabling a total Termosaic mapping with no wildcard. Adding a variant later is an explicit SemVer change. The research `Out` compatibility surface remains separately governed by `research` and does not leak into that mapping.

## 4. Governed surface manifest

The implementation must inspect and update every applicable row. A row may be marked unchanged in the implementation report only with a reason.

| Repository | Surface | Required treatment |
|---|---|---|
| Laidout | `Cargo.toml`, `.gitignore` | Define standalone default consumer dependencies and the `research` feature boundary, including `thiserror`, with no Termosaic dependency or feature; keep the library lockfile intentionally untracked and record the resolved dependency graph in verification evidence. |
| Laidout | `src/doc.rs`, `src/text.rs` | Introduce opaque generic `Doc<A>`, all-`Arc` ownership, constructors, iterative structural traits/destruction, and text validation. |
| Laidout | new `src/prepare.rs` | Own prepared IR, interning, typed IDs, mode-aware fit summaries, bounds, and preparation errors. |
| Laidout | `src/cost.rs` | Add the private bounded consumer cost and retain public research models behind `research`. |
| Laidout | new `src/fast.rs` | Implement prepared consumer Fast semantics, constant-time summary decisions, and mode-aware workspace stacks with cached continuation aggregates. |
| Laidout | `src/greedy.rs` | Retain the allocating arbitrary-`CostModel` greedy engine under `research`, including structural penalty/annotation output and cost reconstruction. |
| Laidout | new `src/exact.rs` | Implement prepared consumer Exact semantics with mode-aware arenas and a deterministic open-addressed memo table. |
| Laidout | `src/frontier.rs` | Retain the allocating generic lawful-model frontier under `research` as the exact compatibility oracle. |
| Laidout | `src/brute.rs` | Keep the exhaustive oracle under `research`; use it for differential tests. |
| Laidout | `src/render.rs` | Add `solve_into`, visitor traversal, `render_into`, borrowed results, and owned wrappers. |
| Laidout | new `src/workspace.rs` | Own independent Fast/Exact capacities, Growable/Fixed modes, arenas, fixed memo, checkpoints, retained-capacity release, and typed resource errors. |
| Laidout | `src/table.rs`, `src/text.rs`, `src/tokens.rs`, `src/tags.rs`, `src/corpus.rs` | Migrate every returned/accepted document from `Rc<Doc>` to opaque `Doc<u32>` and preserve the built-in tag contract. |
| Laidout | `src/measure.rs` | Move research-engine measurement behind `research`; add consumer workspace measurements used by completion evidence. |
| Laidout | `src/lib.rs` | Export the consumer API and conditionally export research APIs. |
| Laidout | `README.md`, `docs/*.md`, `examples/*.rs` | Explain standalone preparation, reservation, strict lifetime rules, column-accurate Fast fitting, proofs, wrappers, and migrate examples without requiring or naming a Termosaic type. Replace `docs/consumer-rendering.md` with a short supersession pointer plus any research-only migration notes not duplicated here, so this plan is the only live consumer-path authority. |
| Laidout | new `examples/standalone.rs`, new `tests/standalone_consumer.rs` | Prove the default public kernel is independently usable with an application-owned annotation type and visitor. |
| Laidout | `tests/*.rs`, new `tests/prepared_mode_matrix.rs`, `tests/fast_summary.rs`, `tests/source_drop.rs`, `tests/fixed_allocation.rs`, `tests/kernel_source_contract.rs`, `tests/exact_oracle.rs`, `tests/fixtures/kernel-invalid-cases.toml` | Migrate public API tests, prove summary/reference-scan equivalence, prove stack-safe source destruction, close the public error vocabulary, and add the named verifier targets. |
| Laidout | new `benches/prepared_kernel.rs`, new `benches/support/harness_v1.rs`, new `benches/adapters/laidout_0_1_baseline.rs`, new `benches/adapters/prepared_kernel.rs`, new `benches/workloads/prepared-kernel-v1.toml`, new `docs/benchmark-dependency-transition-0.2.toml` | Keep measurement/schema code immutable, freeze a self-contained current-engine baseline adapter before migration, hash the separate final adapter, freeze workload identity, classify the planned dependency transition, and enforce the Exact/deep/frontier thresholds without comparing mismatched evidence. |
| Termosaic | `Cargo.toml`, `.gitignore` | Add Laidout, take the breaking version, pin the private `console` dependency exactly to `0.16.4` for the ANSI oracle, keep the library lockfile intentionally untracked, and record the resolved dependency graph in verification evidence. |
| Termosaic | `src/layout.rs`, new `tests/support/baseline_layout.rs`, `tests/layout_compat.rs`, new `tests/fixtures/retired-fast-scan-budget.toml` | Replace the enum with the wrapper, preserve constructors and text-layout helpers, remove the superseded production interpreter, retain its frozen test-only baseline oracle, classify the authorized synthetic-budget divergence, and pin its canonical old/new bytes. |
| Termosaic | `src/render.rs` | Add nonallocating ANSI emission; retain `render_text` as an owned wrapper. |
| Termosaic | `src/console_api.rs` | Add reusable layout/ANSI storage and scalar run state, ergonomic Fast/Exact APIs, warming/capacity/release APIs, strict errors, and one-handoff document integration. |
| Termosaic | `src/progress.rs` | Keep progress rendering functional through the owned ANSI wrapper or migrate it explicitly. |
| Termosaic | `src/lib.rs` | Export Termosaic-owned `PreparedDoc`, capacities, strategies, resource identifiers, cost components, solve counters, and error types without re-exporting Laidout types. |
| Termosaic | `tests/contract.rs`, `tests/source_contract.rs`, new `tests/fixed_allocation.rs` | Preserve output bytes, update closed public-source contracts, and add the isolated fixed allocator target. |
| Termosaic | `tests/fixtures/console-invalid-cases.toml` | Add every new public error and invalid capacity/preparation case required by the source contract. |
| Termosaic | new `tests/fixtures/console-ansi-style-matrix.toml` | Freeze the complete direct-emitter byte grammar against the currently resolved direct dependency, `console 0.16.4`, and record the oracle package/version and dependency-graph digest in the fixture. |
| Termosaic | `scripts/verify.sh` | Keep the repository verifier authoritative and make pre-publication package verification resolve the sibling Laidout checkout through a command-line crates.io patch. |
| Termosaic | `README.md`, examples, remaining tests | Migrate enum variants and document retained-writer strict usage. |
| Termosaic | new `benches/human_doc.rs`, new `benches/support/harness_v1.rs`, new `benches/adapters/termosaic_0_1_baseline.rs`, new `benches/adapters/prepared_kernel.rs`, new `benches/workloads/human-doc-v1.toml`, new `docs/benchmark-dependency-transition-0.2.toml` | Compare prepared Fast solve plus ANSI generation with a frozen self-contained production adapter while sharing immutable measurement/schema code and the same environment/dependency/adapter validation used by Laidout. |
| Both | new `docs/migration-0.2.md` | Record every breaking constructor, result, feature, error, dependency, and Fast scan-budget migration with old/new examples. |

## 5. Normative semantic invariants

### 5.1 Document algebra

The prepared kernel represents these operations without semantic loss:

- empty;
- validated single-line text with cached display width;
- line, soft line, and hard line;
- sequence;
- group;
- fill;
- nest and align;
- ordered choice;
- annotation;
- penalty.

`Seq` is n-ary. Public `concat` flattens adjacent sequences and removes empty children while preserving source order. Preparation hash-conses equivalent source nodes by opcode, payload, and already-canonical child IDs, but never reorders children, choices, annotations, or penalties.

`HardLine` never flattens. A group containing a reachable hard line cannot select a flat projection that erases that line.

The flat projection is defined once and is shared by preparation, Fast, Exact, the deprecated `flatten` wrapper, and the research oracle:

```text
flat(Empty | Text)             = the same node
flat(Line)                     = one-space Text
flat(SoftLine)                 = Empty
flat(HardLine)                 = none
flat(Seq(children))            = Seq(flat(child) for child), or none
flat(Group(child))             = flat(child)
flat(Fill(children))           = Fill(flat(child) for child), or none
flat(Nest(n, child))           = Nest(n, flat(child)), or none
flat(Align(child))             = Align(flat(child)), or none
flat(Choice(preferred, _))     = flat(preferred)
flat(Annotate(a, child))       = Annotate(a, flat(child)), or none
flat(Penalty(p, child))        = Penalty(p, flat(child)), or none
```

`Fill` boundaries remain independently breakable in a flat projection; flattening affects line nodes inside its children, not the boundary policy. Retaining `Nest` and `Align` in that projection is therefore observable when a fill boundary breaks.

Single-line text validation rejects every `char::is_control()` value, including line feed, carriage return, tab, escape, and delete, and reports its UTF-8 byte offset. Raw-text ingestion normalizes CRLF/lone CR to hard-line structure, expands tabs to the configured source-line tab stop, and then rejects any remaining control. Display width is computed under the selected `WidthMode` during construction or preparation and is not recomputed during strict rendering.

### 5.2 Fast semantics and the Termosaic migration boundary

Fast strategy is a deterministic semantic contract, not merely an unspecified heuristic:

- The root starts in broken mode; only `Group` changes line/soft-line projection for its child.
- `Line` emits one space in flat mode and a newline in broken mode.
- `SoftLine` emits nothing in flat mode and a newline in broken mode.
- `HardLine` always emits a newline.
- `Group` makes its fit decision against the complete remaining continuation, matching the current Termosaic interpreter.
- `Fill` decides each boundary by probing only the next child under the current indentation and token context. It does not test the entire remaining fill continuation.
- An explicit `Choice` probes the preferred branch followed by the pending continuation and selects the alternative only when the preferred branch does not fit. Fast never consults penalties or exact costs to choose a branch.
- During a probe, flat `Line`/`SoftLine` charge their flat text; the first broken line opportunity in the pending continuation ends the probe successfully. A `HardLine` reached inside the candidate makes that candidate fail, while a `HardLine` first reached in the continuation ends the probe successfully. Reaching an internal `Fill` boundary likewise ends that local child probe successfully, which is what keeps fill decisions local.
- Zero-width graphemes consume zero columns. Preparation caches the complete display width of each text record, so a probe tests the record in constant time without rescanning graphemes.
- Fitting returns false immediately when `column > width`; otherwise it initializes `remaining_columns = width - column`. A text record fits exactly when its cached display width is at most `remaining_columns`, after which that width is subtracted. There is no second scan-unit counter, cap, or policy.
- Every fit decision is constant-time during rendering. It reads prepared summaries for the candidate and current continuation; it never walks prepared nodes, uses a second fit stack, or falls back to a bounded or unbounded scan.
- An unbreakable text run wider than the configured width is emitted intact.
- `Nest` uses checked arithmetic. `Align` captures the current logical column. Under `Preserve`, those indentation values remain exact. Under `ClampToWidthMinusOne`, `Nest` stores `min(current_indent + amount, width - 1)` when entered and `Align` stores `min(current_column, width - 1)` when entered; every later newline and memo key uses that effective value. This preserves Termosaic's current cap rather than applying a late display-only clamp.
- The source order of children and ordered alternatives is observable and stable.

The implementation follows the precomputation result used by PPrint—bottom-up width requirements make group decisions constant-time—but extends it for this contract's continuation-aware groups and local fills. A plain ideal width is insufficient because it cannot distinguish a candidate `HardLine` from a break in the pending continuation. Repeated strict scans are not an acceptable fallback: Swierstra and Chitil identify nested groups rescanning the same prefixes as the source of width-dependent running time, and describe Oppen's shared-scanning approach as the general streaming remedy. Laidout already prepares the complete finite document, so it uses the smaller prepared analogue below: a composable first-stop summary per prepared node and a cached aggregate for the pending rendering stack. Research references: [PPrint's constant-time requirement](https://ocaml.org/p/pprint/latest/doc/pprint/PPrint/index.html), [PPrint's stored requirement implementation](https://ocaml.org/p/pprint/20230830/doc/src/pprint/PPrintEngine.ml.html), [Strictly Pretty](https://lindig.github.io/papers/strictly-pretty-2000.pdf), and [Linear, bounded, functional pretty-printing](https://doi.org/10.1017/S0956796808006990).

The private summary algebra is normative:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ModeFitSummary {
    flat: FitSummary,
    broken: FitSummary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FitSummary {
    columns: u32,
    stop: FitStop,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FitStop {
    End,
    Break,
    HardLine,
}
```

`end(n)`, `break(n)`, and `hard_line(n)` abbreviate `FitSummary { columns: n, stop: End | Break | HardLine }`. `then(a, b)` returns `a` unchanged when `a.stop != End`; otherwise it returns `FitSummary { columns: checked_add(a.columns, b.columns), stop: b.stop }`. The checked addition is performed during preparation and maps overflow to `PrepareError::RepresentationExceeded { resource: PreparedResource::DisplayColumns, .. }`. The identity is `end(0)`. Preparation computes both modes in iterative postorder with these exact equations:

```text
summary(Empty, mode)                     = end(0)
summary(Text(t), mode)                   = end(t.display_width)
summary(Line, Flat)                      = end(1)
summary(SoftLine, Flat)                  = end(0)
summary(Line | SoftLine, Broken)         = break(0)
summary(HardLine, mode)                  = hard_line(0)
summary(Seq(children), mode)             = fold_then(summary(child, mode))
summary(Group(child), mode)              = summary(child, Flat)
summary(Fill([]), mode)                  = end(0)
summary(Fill([child]), mode)             = summary(child, mode)
summary(Fill([first, ...]), mode)         = then(summary(first, mode), break(0))
summary(Nest(_, child) | Align(child) |
        Annotate(_, child) |
        Penalty(_, child), mode)         = summary(child, mode)
summary(Choice(preferred, _), mode)      = summary(preferred, mode)
```

The `Choice` equation is specifically probe projection: a probe observes the preferred branch under its inherited mode and does not recursively make a second width-dependent choice. Rendering a `Choice` still performs its own fit decision when the node is reached normally. A `Group` probe uses its child's flat summary with the current stack aggregate as `k`; a `Fill` boundary uses the next child's flat summary with identity as `k`, matching the current Termosaic local candidate mode; a normally rendered `Choice` uses the preferred child's summary in the current frame mode with the current stack aggregate as `k`.

The Fast work stack maintains the aggregate summary of its pending continuation in O(1): each pushed work item stores the aggregate that preceded it, and the current aggregate is `then(summary(top), ... then(summary(bottom), identity))`. A fill-boundary work item has `break(0)`. Pop restores the stored prior aggregate; pushing reversed sequence children composes their summaries in execution order. This adds constant metadata to the existing Fast work item but no workspace resource or auxiliary arena.

Given candidate summary `c`, continuation summary `k`, and `remaining_columns`, the decision is exactly:

```text
if c.columns > remaining_columns: false
else if c.stop == HardLine:       false
else if c.stop == Break:          true
else:                             k.columns <= remaining_columns - c.columns
```

Any `Break`, `HardLine`, or `End` reached in `k` is success after its preceding columns fit, so `k.stop` does not affect that last comparison. Each call increments `SolveStats::fit_checks` once. Preparation and normal Fast traversal are linear in reachable prepared nodes/edges; each Group, Fill boundary, or normally rendered Choice adds one O(1) fit check. There is no compatibility path, dynamic-summary fallback, or render-time node scan.

Laidout verifies its complete Fast semantics independently with crate-local reference tests for every document operation; those tests and the standalone example have no Termosaic dependency. Cross-repository compatibility is established only in Termosaic by running a frozen copy of its baseline interpreter as a test-only oracle across all existing fixtures and generated documents in the baseline algebra. Because the new wrapper is opaque, those generated differential cases use a test-only neutral `CompatibilityDoc` containing exactly `Text`, the three line kinds, `Concat`, `Group`, `Fill`, `Nest`, and `Token`, with independent conversions to public `termosaic::Doc` constructors and the frozen `BaselineDoc` enum; the oracle never introspects Laidout internals.

The test-only baseline oracle records each probe's result as `Fits`, `ColumnOverflow`, `SyntheticBudgetExhausted`, or `HardLine`, without changing its rendered bytes. A text charge reports `ColumnOverflow` whenever its display width exceeds the remaining columns; it reports `SyntheticBudgetExhausted` only when those columns fit but the old secondary counter does not. When old and new output differ, the differential harness finds the first different Group/Fill choice and permits it only when the baseline trace is `SyntheticBudgetExhausted` and replaying that same probe with only display-column charging returns `Fits`. All other first-divergence reasons fail. Generated cases deliberately cover both classified and byte-equal Unicode inputs, so V29 governs the full behavior class rather than whitelisting one document.

`tests/fixtures/retired-fast-scan-budget.toml` pins the canonical case: at width two, `group(concat([text("\u{200b}\u{200b}\u{200b}\u{200b}"), line(), text("x")]))` changes from the old document bytes `"\u{200b}\u{200b}\u{200b}\u{200b}\nx\n"` to the column-correct bytes `"\u{200b}\u{200b}\u{200b}\u{200b} x\n"`. The fixture asserts both through independent old/new paths and explains that capacity exhaustion is an error rather than a layout choice. New `Empty`, `Align`, `Choice`, and `Penalty` behavior is verified against the normative rules and Laidout reference tests rather than fabricated old behavior. Every existing fixture outside the classified V29 behavior retains its pinned bytes. The oracle has no production dependency edge, carries an explicit frozen-baseline comment, and changes only when an authorized Termosaic byte-semantics change updates both the fixture corpus and this contract.

### 5.3 Exact semantics

Exact strategy enumerates the same layout set as Laidout's current lawful document algebra and returns the minimum consumer cost. `Group(child)` is exactly `Choice(flat(child), child)` when the flat projection exists and is exactly `child` otherwise. `Fill([c0, ..., cn])` inserts an ordered `Choice(space, newline)` at each boundary, with space preferred. At every equal-cost tie, the earliest preferred source-order derivation wins.

Hash slot order, address values, arena reuse, and candidate pruning order may not determine a tie. Candidate generation is source-order stable, frontier pruning is stable, and memo ranges are published only after the complete frontier for a key is finalized.

`Fill` remains a distinct prepared node. Its exact interpretation is an ordered choice of space versus newline at each boundary, with space preferred when costs tie. Its Fast interpretation remains the local next-child rule in section 5.2.

### 5.4 Annotation semantics

Annotations are nested structurally. Event callbacks expose the active stack outermost-to-innermost. `innermost()` returns `Option<(AnnotationId, &A)>`; Termosaic's rooted prepared wrapper makes that option present and resolves `A = TokenId`. Newline callbacks receive the active stack for general consumers; Termosaic intentionally emits no token styling for newline bytes.

Rendering never clones an annotation payload. Empty annotations still produce zero-width spans in borrowed materialization, retaining source preorder and parentage. Span IDs equal their index in `spans()`, every parent index is smaller than its child, all ranges are UTF-8 boundaries within the rendered text, and a child range is contained by its parent.

### 5.5 Termosaic byte semantics

The direct visitor must preserve these existing output rules:

- Adjacent text fragments with the same innermost token form one ANSI run. The run state machine emits one prefix, all fragments, and one suffix; document fragmentation alone cannot add reset/prefix sequences.
- A token change, newline, or end-of-document closes the active ANSI run.
- Newlines are unstyled.
- Indentation is emitted only where the current Termosaic interpreter emits it; consecutive or trailing newlines do not acquire newly observable trailing spaces.
- The writer appends one LF if and only if the completed byte buffer does not already end in LF. Existing multiple trailing LFs are preserved.
- The entire ANSI byte slice is sent through one existing `write_human` call. That creates at most one progress suspension and one payload sink-lock scope. An active progress backend may take additional sink locks to clear/redraw its own record. The enclosed payload `Write::write_all` may legally call an underlying short-writing sink more than once.
- Only `HumanWriter` accepts documents. `MachineTextWriter` and `OpaqueWriter` retain their typed safety boundaries and never route data through Laidout or ANSI styling.
- Width and terminal capabilities come from that writer's `DestinationState`; token styles come from that writer's `Theme`. No global workspace, theme, width, sink, or rendering policy is introduced.

## 6. Target ownership and public API

Names and semantic relationships in this section are normative. Private field layout may change only when the verifier-visible behavior remains identical.

### 6.1 Source and prepared documents

```rust
#[derive(Clone)]
pub struct Doc<A = u32>(Option<Arc<Node<A>>>);

#[derive(Clone)]
pub struct PreparedDoc<A = u32>(Arc<PreparedData<A>>);

impl<A> Doc<A> {
    pub fn prepare(&self) -> Result<PreparedDoc<A>, PrepareError>
    where
        A: Eq + Hash;
}

impl<A> PreparedDoc<A> {
    pub fn bounds(&self) -> PreparedBounds;
}
```

The private source representation is n-ary as well as the prepared representation:

```rust
enum Node<A> {
    Empty,
    Text(TextRun),
    Break(FlatAlternative),
    HardLine,
    Seq(Box<[Doc<A>]>),
    Group(Doc<A>),
    Fill(Box<[Doc<A>]>),
    Nest { indent: u32, child: Doc<A> },
    Align(Doc<A>),
    Choice { preferred: Doc<A>, alternative: Doc<A> },
    Annotate { annotation: Arc<A>, child: Doc<A> },
    Penalty { amount: u32, child: Doc<A> },
}
```

Every source node, edge handle, annotation, and shared text payload uses `Arc`, not `Rc`; the n-ary child containers themselves are owned boxed slices because the enclosing node is already shared. An annotation node stores `Arc<A>`. `Doc::annotate` accepts `A` and wraps it once; an additional `Doc::annotate_shared(Arc<A>, Doc<A>)` constructor supports already-shared application values. `Clone` is a handle clone.

Source structural identity is defined over the constructor-canonical algebra. `concat` flattens nested `Seq` nodes and removes `Empty`; zero children return `Empty`, one child returns that child, and two or more children produce one `Seq`. `join` interleaves then delegates to `concat`. `penalize(0, child)` returns `child`; nonzero penalties remain structural. No other constructor silently collapses an operation: `Fill([])`, `Group(Empty)`, and `Annotate(a, Empty)` remain structurally distinct. Pointer sharing is not identity. Structural `PartialEq`/`Eq` use an iterative work stack plus a visited pointer-pair set, so a shared DAG is compared once per reachable pair while rebuilt-equal graphs still compare equal. Structural `Hash` computes per-node fingerprints in iterative postorder with a pointer memo, feeds opcode, payload, and ordered child fingerprints through FNV-1a 64 (`offset = 0xcbf29ce484222325`, `prime = 0x100000001b3`, bytewise xor then wrapping multiply), then writes the root fingerprint into the caller's `Hasher`; equal rebuilt graphs therefore hash equally without expanded-path traversal. `Debug` prints the root canonical operation expression without an enclosing crate/type label, assigns preorder node numbers, and prints a numbered shared reference on repeat. This is the ordinary public `Debug for Doc<A>` implementation, not a separate formatter hook. Debug text is diagnostic rather than a stable serialization, but never contains addresses. These traits never recurse and are linear in reachable nodes/edges rather than expanded DAG paths; fingerprint collisions are not used for equality or preparation correctness.

The private `Option` is a destruction slot, not a nullable public state: every live `Doc` returned by a constructor contains `Some(root)`, and methods treat `None` as reachable only while that value's `Drop` implementation is running. `Drop` takes the root, processes a current `Arc<Node<A>>`, and keeps only pending siblings in a local `Vec`. It calls `Arc::into_inner`; an owned node has each child `Doc` root taken and transferred into the same worklist before any child wrapper drops, while a still-shared node needs no descendant traversal by that owner. The first child becomes the next current node and only additional children enter the `Vec`, so arbitrarily deep unary chains require no worklist growth. Because every cloned `Doc` uses `Arc::into_inner`, its exactly-one-inner guarantee also covers concurrent final drops of cloned roots. Library-owned source edges therefore never recurse on the call stack. Branching destruction may allocate or deallocate outside every rendering allocation guarantee, and destructors implemented by a consumer's annotation type `A` remain that consumer's responsibility.

The opaque constructor surface covers the complete algebra and is the migration target for existing free functions:

```rust
impl<A> Doc<A> {
    pub fn empty() -> Self;
    pub fn text(value: impl AsRef<str>) -> Self;
    pub fn try_text(value: impl AsRef<str>) -> Result<Self, TextError>;
    pub fn try_text_with(
        value: impl AsRef<str>,
        width_mode: WidthMode,
    ) -> Result<Self, TextError>;
    pub fn line() -> Self;
    pub fn soft_line() -> Self;
    pub fn hard_line() -> Self;
    pub fn concat(docs: impl IntoIterator<Item = Self>) -> Self;
    pub fn join(separator: Self, docs: impl IntoIterator<Item = Self>) -> Self;
    pub fn group(doc: Self) -> Self;
    pub fn fill(docs: impl IntoIterator<Item = Self>) -> Self;
    pub fn nest(indent: u32, doc: Self) -> Self;
    pub fn align(doc: Self) -> Self;
    pub fn choice(preferred: Self, alternative: Self) -> Self;
    pub fn annotate(annotation: A, doc: Self) -> Self;
    pub fn annotate_shared(annotation: Arc<A>, doc: Self) -> Self;
    pub fn penalize(amount: u32, doc: Self) -> Self;
}
```

The text policy and direct errors remain public and exact:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum WidthMode { #[default] Narrow, Cjk }

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum TextError {
    ControlCharacter { character: char, byte_offset: usize },
    DisplayWidthOverflow { byte_offset: usize },
}
```

`try_text*` applies the control and width rules in section 5.1. `text` is exactly `try_text(...).unwrap_or_else(|error| panic!("{error}"))` for trusted literals. The existing fallible `text::from_text`/`from_text_with` ingestion keeps `IngestOptions { width_mode, tab_width }`, eight-column default tabs, CR/LF normalization, tab expansion, typed remaining-control rejection, hard-line structure, and current token classification. No global terminal policy enters construction or preparation.

Existing free functions remain in `0.2.0` as `#[deprecated(since = "0.2.0", note = "use Doc methods")]` forwarding wrappers. Their complete compatibility surface remains fixed at `Doc<u32>`, not `Arc<Doc<u32>>`: this preserves inference for calls such as `text("x")`, `line()`, and `concat([])`. Arbitrary-annotation consumers use the generic `Doc::<A>` methods, where the type is explicit or inferred from the handle. This contract makes no removal promise for a later release.

`PreparedData<A>` owns the following logical tables; private field packing can combine tables only when the same IDs, bounds, ownership, and iteration order remain verifier-identical:

```rust
struct PreparedData<A> {
    nodes: Box<[PreparedNode]>,
    edges: Box<[NodeId]>,
    texts: Box<[PreparedText]>,
    annotations: Box<[Arc<A>]>,
    fit_summaries: Box<[ModeFitSummary]>,
    root: NodeId,
    bounds: PreparedBounds,
}
```

Prepared nodes contain typed compact IDs, never generic annotation values. Interning compares `A` through `Eq + Hash` and stores cloned `Arc<A>` handles; it does not clone `A`.

The dense IR has these semantic variants:

```rust
enum PreparedNode {
    Empty,
    Text(TextId),
    Break(FlatAlternative), // Line => Space, SoftLine => Empty
    HardLine,
    Seq(EdgeRange),
    Group(NodeId),
    Fill(EdgeRange),
    Nest { indent: u32, child: NodeId },
    Align(NodeId),
    Choice { preferred: NodeId, alternative: NodeId },
    Annotate { annotation: AnnotationId, child: NodeId },
    Penalty { amount: u32, child: NodeId },
}

enum FlatAlternative { Space, Empty }
struct EdgeRange { start: u32, len: u32 }
struct PreparedText {
    value: Arc<str>,
    byte_len: usize,
    display_width: u32,
}
```

An `EdgeRange` is checked before slicing. First-preorder encounter assigns every interned ID; hash-map iteration is never used to assign IDs.

Prepared storage uses typed compact IDs. Only the IDs that appear in result metadata are public:

```rust
struct NodeId(u32);
struct TextId(u32);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct AnnotationId(u32);
struct PlanId(u32);
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct SpanId(u32);
```

IDs are indexes, not globally branded capabilities. `NodeId`, `TextId`, and `PlanId` remain crate-private. `SpanId::index() -> usize` lets a consumer follow `parent` inside the result's `spans()` slice; no public API accepts a free `SpanId` and resolves data for it. `AnnotationId` appears as result metadata, but has no public index getter or direct lookup by arbitrary ID. `ActiveAnnotations` yields `(AnnotationId, &A)` pairs, and `RenderedRef::resolved_spans()` resolves only the spans owned by that same result. This prevents accidental cross-document annotation resolution without adding a runtime document cookie to every ID.

Preparation is iterative. It rejects node, edge, text, annotation, display-width, indentation, output, candidate-cost, and ID bounds that cannot be represented by the consumer kernel. Each rejection has a direct `PrepareError` variant with the offending bound and maximum where representable without allocation.

`PreparedBounds` is `Clone + Copy + Debug + Eq + PartialEq`, has private fields, and exposes these getters:

```rust
impl PreparedBounds {
    pub fn output_bytes_upper(self) -> usize;
    pub fn line_count_upper(self) -> usize;
    pub fn spans_upper(self) -> usize;
    pub fn annotation_depth(self) -> usize;
    pub fn fast_work_items(self) -> usize;
    pub fn solve_work_items_upper(self) -> Option<u128>;
    pub fn fast_fit_checks_upper(self) -> Option<u128>;
    pub fn single_plan_nodes_upper(self) -> usize;
    pub fn visit_work_items(self) -> usize;
    pub fn candidate_emissions_upper(self) -> Option<u128>;
    pub fn overflow_cost_upper(self) -> u128;
    pub fn burden_cost_upper(self) -> u64;
}
```

These values are conservative upper bounds unless named exact and are valid for every `NonZeroU32` width, every `u32` newline cost, both indentation policies, and every strategy. `fast_work_items` is peak simultaneous Fast stack storage and seeds capacity. The three semantic-work getters return a checked `u128` bound when representable and `None` when the symbolic expansion exceeds `u128`; they are diagnostics/test oracles, not preparation acceptance conditions or reservation sizes. Actual `u64` statistics and source precedence are checked during rendering and return typed `StatisticsOverflow` instead of making Fast reject a compact document because an unused Exact derivation count is enormous. Exact memo entries, simultaneous retained candidates, frontier scratch, and total candidate plan nodes remain runtime width-dependent resources and are learned through typed exhaustion or representative measurement.

The mandatory preparation error shape is:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PreparedResource {
    Nodes,
    Edges,
    Texts,
    Annotations,
    TextBytes,
    DisplayColumns,
    OutputBytes,
    LineCount,
    Indentation,
    AnnotationDepth,
    SpanOccurrences,
    PlanNodes,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CostComponent {
    Overflow,
    Burden,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PrepareError {
    RepresentationExceeded {
        resource: PreparedResource,
        required: Option<u128>,
        maximum: u128,
    },
    CostDomainExceeded {
        component: CostComponent,
        required: Option<u128>,
        maximum: u128,
    },
}
```

`required: None` means the checked computation overflowed before an exact requirement was representable; it is valid in either variant. These variants and fields are exact for `0.2.0`; implementation-specific context belongs in private helpers, not extra public variants or fields.
`PreparedResource`, `CostComponent`, and `PrepareError` are exhaustive in `0.2.0` so Termosaic can map them without a catch-all. `CostDomainExceeded` reports `Overflow` with `u128::MAX` or `Burden` with `u64::MAX` converted losslessly to `u128`; callers never infer the failed component from the numeric maximum.

Error signatures in this plan elide the required per-variant `#[error("...")]` attributes for readability. Every enum deriving `thiserror::Error` receives a static format string over its existing fields; formatting may not allocate inside a strict measured call. Exact display prose is not a compatibility surface, and tests match structured variants and fields.

`Doc::text` retains its current trusted-convenience behavior and panics with the corresponding `TextError`; untrusted input uses `try_text`. `TagId` remains a compatibility alias for `u32`, and deprecated free `tag`, `concat2`, `flatten`, `group`, `count_choices`, text-ingest, token, and table helpers are migrated to opaque documents rather than deleted. Helpers whose semantics are only meaningful to the old generic engines move under `research` and receive release-note migration entries.

Compatibility helpers have an explicit annotation type instead of an inferred migration:

- `text::from_text`, `text::from_text_with`, `tokens::*`, `tags::*`, and `corpus::*` return `Doc<u32>` and preserve the existing built-in tag IDs.
- `Column`, `Table`, `table`, and `Table::build` remain the built-in-tag convenience surface and accept or return `Doc<u32>`; they are not silently generalized to arbitrary `A` in this change.
- Core constructors on `Doc<A>` remain generic. Applications needing custom annotations inside tables construct ordinary generic documents themselves.
- `SolveLimits` and limit-bearing generic engine constructors move under `research`; the default consumer surface has only workspace capacities.

### 6.2 Rendering options

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutStrategy {
    Fast,
    Exact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IndentPolicy {
    Preserve,
    ClampToWidthMinusOne,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RenderOptions {
    width: NonZeroU32,
    newline_cost: u32,
    strategy: LayoutStrategy,
    indent_policy: IndentPolicy,
}

impl RenderOptions {
    pub fn new(width: NonZeroU32) -> Self;
    pub fn with_newline_cost(self, newline_cost: u32) -> Self;
    pub fn with_strategy(self, strategy: LayoutStrategy) -> Self;
    pub fn with_indent_policy(self, policy: IndentPolicy) -> Self;
    pub fn width(&self) -> NonZeroU32;
    pub fn newline_cost(&self) -> u32;
    pub fn strategy(&self) -> LayoutStrategy;
    pub fn indent_policy(&self) -> IndentPolicy;
}
```

`RenderOptions::new(width)` selects Exact, newline cost one, and `IndentPolicy::Preserve`, preserving Laidout's exact-render default. Termosaic always selects `ClampToWidthMinusOne`. Width is a `NonZeroU32`; Termosaic's existing `NonZeroU16` width converts losslessly, and zero never enters the solver or error vocabulary. No option can restore the retired scan-budget behavior.

Solver limits are represented by capacities, not by a second limit mechanism. Exact work exhaustion reports the exhausted workspace resource.

The consumer statistics shape and counter rules are complete rather than inherited implicitly from the old frontier engine:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SolveStats {
    pub work_items: u64,
    pub fit_checks: u64,
    pub memo_hits: u64,
    pub memo_misses: u64,
    pub candidates_generated: u64,
    pub candidates_pruned: u64,
    pub peak_frontier: usize,
    pub memo_entries: usize,
    pub plan_nodes: usize,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum SolveCounter {
    WorkItems,
    FitChecks,
    MemoHits,
    MemoMisses,
    CandidatesGenerated,
    CandidatesPruned,
}
```

`work_items` is an implementation-independent semantic charge: one for beginning interpretation of a prepared node, plus one for each Exact candidate presented to a dominance builder. Private continuation pops, memo probes, index moves, and callback dispatch do not change it. `fit_checks` counts each constant-time Group, Fill-boundary, or Choice fit decision in Fast; the memo and candidate fields are zero in Fast; `fit_checks` is zero in Exact. `candidates_generated` counts each Exact candidate before dominance, and `candidates_pruned` counts generated candidates absent from the completed retained frontier. `peak_frontier` and `memo_entries` describe completed Exact frontiers. `plan_nodes` counts only immutable nodes committed for Fast's selected layout or for candidates retained in completed Exact frontiers; candidates removed before frontier publication do not increment it.

Every `u64` counter update uses one checked helper immediately before its charged semantic action. Beginning a node preflights `WorkItems`; a Fast decision preflights `FitChecks`; a memo result preflights the corresponding hit/miss counter. Exact candidate presentation atomically preflights `WorkItems` and then `CandidatesGenerated` without updating either, assigns precedence from the old generated count, then increments both and calls the builder. That order selects `WorkItems` when both would overflow. Rejection/removal preflights the complete `CandidatesPruned` delta before changing the AVL frontier. Overflow returns `RenderError::StatisticsOverflow` with the pre-action snapshot; no counter saturates and no corresponding solver mutation occurs. A capacity or growth error likewise carries the snapshot immediately before the failed write. Growable and Fixed executions of the same strategy/input/options therefore report identical statistics when both complete, and counter/capacity failures have a deterministic snapshot.

### 6.3 Workspace mode and capacity

Memory policy and layout strategy are orthogonal. One workspace retains separate Fast and Exact solve storage plus shared plan, visit, and materialization storage. A Fast-only caller leaves every Exact capacity at zero and allocates no memo or candidate arena; a workspace that uses both strategies retains both sets for reuse.

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorkspaceMode {
    Growable,
    Fixed,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderCapacity {
    pub fast: FastSolveCapacity,
    pub exact: ExactSolveCapacity,
    pub plan_nodes: usize,
    pub visit: VisitCapacity,
    pub materialize: MaterializeCapacity,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FastSolveCapacity {
    pub work_items: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExactSolveCapacity {
    pub memo_entries: usize,
    pub retained_candidates: usize,
    pub frontier_scratch: usize,
    pub work_items: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VisitCapacity {
    pub work_items: usize,
    pub annotation_depth: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MaterializeCapacity {
    pub output_bytes: usize,
    pub spans: usize,
}
```

`RenderWorkspace` is the concrete consumer workspace. It is not generic over arbitrary public cost models because the fixed guarantee cannot be extended to cost values whose operations allocate.

Capacity units are logical entries. `frontier_scratch = n` reserves `n` scratch slots, each containing one pending candidate plus its intrusive dominance-tree, source-order-list, and free-list metadata, as one transactional resource; `memo_entries = n` means `n` usable memo entries independent of raw slot count; every other field counts values of the struct or byte named by the field. `capacity()` never exposes raw tree slots, raw hash slots, or allocator spare capacity.

Plan, retained-candidate, frontier-scratch, and span indexes are `u32`; reservation above `u32::MAX` for one of those resources returns `ReserveError::CapacityOverflow` before allocation. Work stacks, annotation depth, output bytes, memo raw slots, and platform allocation sizes use `usize` with checked conversions. The invalid-capacity fixture covers the first unrepresentable request for every narrower or derived backing representation.

```rust
pub struct RenderWorkspace { /* retained storage and logical limits */ }

impl RenderWorkspace {
    pub fn growable() -> Self;
    pub fn fixed(capacity: RenderCapacity) -> Result<Self, ReserveError>;
    pub fn mode(&self) -> WorkspaceMode;
    pub fn set_mode(&mut self, mode: WorkspaceMode);
    pub fn reserve(&mut self, capacity: RenderCapacity) -> Result<(), ReserveError>;
    pub fn capacity(&self) -> RenderCapacity;
    pub fn release_capacity(&mut self);
}

#[derive(Debug, thiserror::Error)]
pub enum ReserveError {
    CapacityOverflow {
        resource: WorkspaceResource,
        requested: usize,
    },
    Allocation {
        resource: WorkspaceResource,
        requested: usize,
        source: TryReserveError,
    },
}
```

`WorkspaceMode`, every capacity component, `WorkspaceResource`, and `ReserveError` are exhaustive public vocabulary in `0.2.0`. `ReserveError` is mapped variant-for-variant by Termosaic.

`growable()` creates an empty workspace without allocating. `fixed(capacity)` allocates outside rendering and installs the requested logical limits. `set_mode` is a scalar, nonallocating transition: changing to Fixed freezes the current logical capacities, and changing to Growable permits typed resource growth. Rust's result borrow prevents a mode change while a `LayoutRef` or `RenderedRef` exists.

`reserve` ensures at least the requested per-resource logical capacity without shrinking another resource or changing the mode. It is callable only while no borrowed result exists, which the mutable borrow enforces, and begins from reset logical state. Construction and `reserve` may allocate and are never part of a fixed measured operation. For every resource needing more backing, `reserve` allocates a staged empty replacement at the requested derived backing size; it does not call in-place `try_reserve` on the live store. Only after every stage succeeds does it swap all replacements and raise logical capacities. An allocation failure drops the stages and leaves all prior backing stores, limits, contents, and mode unchanged.

In Growable mode, the checked storage helper for an exhausted logical resource uses already-retained spare backing capacity when possible; only if the backing is also insufficient does it use `try_reserve` for that resource. On success it raises the public logical capacity to the exact `required` value, never to allocator-dependent spare `Vec::capacity()`. It continues the same strategy execution, does not restart the solve, discard retained capacities, or allocate an unused strategy arena. In Fixed mode, the same helper returns `WorkspaceExhausted` before calling an allocator. Solver statistics count semantic work identically in both modes and do not count reserve attempts. This exact-required rule makes `capacity()` and warm/freeze snapshots deterministic even when allocators choose different backing growth.

All-zero `RenderCapacity::default()` is a useful empty value, not a hidden operating default. Owned wrappers seed Growable workspaces from `PreparedBounds`; exact width-dependent resources then grow as observed. Calling `capacity()` after a representative Growable solve and switching to Fixed is the supported warm-and-freeze workflow.

`release_capacity` resets logical operation state, drops every retained backing allocation, sets every logical capacity to zero, and preserves the workspace mode. It requires the same exclusive mutable access as `reserve`, performs no allocation, and may deallocate. A later Growable operation can grow again; a later Fixed operation reports the ordinary typed zero-capacity exhaustion. This is the explicit release valve for a long-lived workspace that observed an exceptional high-water mark.

### 6.4 Borrowed solve, visit, and materialization

```rust
pub fn solve_into<'p, 'w, A>(
    prepared: &'p PreparedDoc<A>,
    options: RenderOptions,
    workspace: &'w mut RenderWorkspace,
) -> Result<LayoutRef<'p, 'w, A>, RenderError>;

pub trait LayoutVisitor<A> {
    type Error;

    fn enter_annotation(
        &mut self,
        id: AnnotationId,
        annotation: &A,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn exit_annotation(
        &mut self,
        id: AnnotationId,
        annotation: &A,
    ) -> Result<(), Self::Error> {
        Ok(())
    }

    fn penalty(&mut self, amount: u32) -> Result<(), Self::Error> {
        Ok(())
    }

    fn text(
        &mut self,
        text: &str,
        annotations: ActiveAnnotations<'_, A>,
    ) -> Result<(), Self::Error>;

    fn newline(
        &mut self,
        indent: u32,
        annotations: ActiveAnnotations<'_, A>,
    ) -> Result<(), Self::Error>;
}

#[derive(Debug)]
pub enum VisitError<E> {
    Workspace(RenderError),
    Visitor(E),
}

impl<'p, 'w, A> LayoutRef<'p, 'w, A> {
    pub fn cost(&self) -> ConsumerCost;
    pub fn stats(&self) -> SolveStats;
    pub fn visit<V: LayoutVisitor<A>>(
        self,
        visitor: &mut V,
    ) -> Result<(), VisitError<V::Error>>;
}

pub fn render_into<'p, 'w, A>(
    prepared: &'p PreparedDoc<A>,
    options: RenderOptions,
    workspace: &'w mut RenderWorkspace,
) -> Result<RenderedRef<'p, 'w, A>, RenderError>;
```

`LayoutRef` and `RenderedRef` borrow the workspace. A second render cannot begin until the result is dropped. Plan and span IDs become invalid when the workspace is reset; Rust lifetimes prevent their escape through the safe API.

Both borrowed results are rollback guards. Dropping an unvisited `LayoutRef` or any `RenderedRef` resets all associated logical workspace state without deallocating. `LayoutRef::visit` installs an internal RAII cleanup guard before invoking the first callback; that guard performs the same reset on success, visitor error, workspace error, or panic unwind. A `catch_unwind` verifier proves same-workspace reuse after a panicking visitor. The next render always resets operation generations and logical lengths before reading them, so deliberately `mem::forget`ting a result can retain capacity but cannot expose stale ranges or skip initialization when the mutable borrow later becomes available.

Rollback covers Laidout workspace state, not arbitrary side effects already performed by a caller's visitor before it returns an error or panics. Visitors that require external atomicity buffer privately and publish only after `visit` succeeds. Termosaic follows that rule: its callbacks touch only the retained ANSI buffer, and any visit failure clears that buffer before any destination handoff.

`ActiveAnnotations` is a borrowed view over the visit stack and supports ID/value iteration plus `innermost()`. Its references are valid only for the callback invocation. Structural callbacks are ordered in plan preorder: `enter_annotation` fires before the annotated child, `exit_annotation` after it, and `penalty` immediately before its child. Both annotation callbacks fire for an empty child, and every selected penalty fires even though neither produces bytes. The default no-op implementations keep text/newline-only visitors source-compatible while making the full witness available to consumers that need it. During `text` and `newline`, `ActiveAnnotations` includes every entered and not-yet-exited annotation, outermost to innermost.

In Fixed mode, the zero-allocation guarantee for `LayoutRef::visit` covers Laidout traversal and callback dispatch. In Growable mode, visit-stack and annotation-stack expansion follows the same named-resource growth contract as solving. An arbitrary downstream visitor may allocate inside its callbacks; Termosaic's fixed visitor is separately proved nonallocating and included in the end-to-end measured boundary.

`RenderedRef` exposes:

- `text() -> &str`;
- `spans() -> &[AnnotationSpan]`;
- `resolved_spans()`, a nonallocating iterator yielding `(&AnnotationSpan, &A)` for this result;
- `cost() -> ConsumerCost`;
- `stats() -> SolveStats`.

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnnotationSpan {
    pub annotation: AnnotationId,
    pub range: Range<usize>,
    pub parent: Option<SpanId>,
}
```

`VisitError<E>` implements `Display` when `E: Display` and `Error` when `E: Error + 'static`; constructing or matching it imposes no error-trait bound on a visitor callback type.

The common plan walker used by both `visit` and materialization independently replays prepared text widths, newline indentation, newline cost, and penalty events through the private checked model; it reads `PreparedText::display_width` and never remeasures Unicode during rendering. Completion requires the replayed cost to equal the solver's stored root cost. `LayoutRef::visit` reports a mismatch as `VisitError::Workspace(RenderError::CostWitnessMismatch)` after cleanup; Termosaic therefore writes nothing and clears its visitor buffer. The materializer additionally writes UTF-8 and spans directly into reserved workspace storage, opening a span on `enter_annotation` and closing it on `exit_annotation`, so nested and zero-width annotations remain lossless. A materialization mismatch returns `RenderError::CostWitnessMismatch` and rolls back. It does not construct annotation-run vectors. The strict borrowed surface exposes text, spans, resolved spans, and visitation only; the allocating owned wrapper retains the current convenient `annotated_runs()` projection.

A newline plan node establishes its logical indentation column immediately for solving and cost reconstruction. Laidout's general materializer writes those indentation spaces after every newline, preserving its existing plain-text semantics. Termosaic retains its current physical-byte policy by deferring indentation in a scalar until the next nonempty text callback; another newline or end-of-document discards the deferred spaces. Exact Termosaic layout therefore optimizes the same logical columns used by its current fitting interpreter even when trailing or consecutive physical indentation is elided.

### 6.5 Owned wrappers and research API

The allocating convenience surface remains:

```rust
pub fn render<A>(
    doc: &Doc<A>,
    options: RenderOptions,
) -> Result<Rendered<A>, OwnedRenderError>
where
    A: Eq + Hash;

#[derive(Debug, thiserror::Error)]
pub enum OwnedRenderError {
    Prepare(PrepareError),
    Reserve(ReserveError),
    Render(RenderError),
    OwnedOutputReserve {
        resource: OwnedOutputResource,
        requested: usize,
        source: TryReserveError,
    },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OwnedOutputResource { Text, Spans, Annotations }
```

`Rendered<A>` has private fields containing a `String`, a span vector, a `Vec<Arc<A>>` annotation table, `ConsumerCost`, and `SolveStats`. Its exact access surface is `text()`, `spans()`, `resolved_spans()`, `cost()`, `stats()`, and allocating `annotated_runs()`. `resolved_spans()` has the same result-scoped semantics as the borrowed method. `annotated_runs()` returns `Vec<AnnotatedRun<'_, A>>`, where each run has public `text: &str`, `range: Range<usize>`, and `annotations: Vec<&A>`. Runs are nonempty, contiguous byte ranges covering the text once, and annotations are ordered outermost-to-innermost. Empty rendered text returns no runs, while zero-width annotations remain visible through `spans()`/`resolved_spans()`. No method exposes the raw annotation table or resolves a caller-supplied ID.

Owned-result trait compatibility is explicit: `Rendered<A>: Clone` without an `A: Clone` bound because annotations are `Arc<A>`; `Debug` requires `A: Debug`; `PartialEq` requires `A: PartialEq`; and `Eq` requires `A: Eq`. Equality includes text, spans, resolved annotation payload order, cost, and statistics. The migration must not accidentally replace the current conditional traits with an unconditional omission or an `A: Clone` requirement.

The wrapper prepares, seeds a Growable workspace from `PreparedBounds`, completes one strategy execution while growing named resources as required, and copies the borrowed result into those owned containers without requiring `A: Clone`. It never restarts with or changes strategy as a fallback.

The `research` Cargo feature owns:

- the public downstream `CostModel` extension point and the public-but-sealed `LawfulCostModel` marker accepted by research frontier pruning;
- `ResearchConsumerCost`, whose overflow and burden components are both `BigUint`, and the research `OverflowThenHeight`, whose overflow and height components are both `BigUint`;
- the generic allocating frontier engine;
- the arbitrary-`CostModel` brute-force and greedy engines, plus law/differential support;
- `measure::PreparedFootprint` and `measure::WorkspaceFootprint`, copy-only breakdowns of logical bytes for the dense prepared tables and logical/actual retained backing bytes for every consumer workspace resource, plus `measure::prepared_footprint(&PreparedDoc<A>)` and `measure::workspace_footprint(&RenderWorkspace)`. The getters perform no allocation, are used by the release benchmark, and expose no payload values or mutable storage.

The current public `Out`, output-construction helpers, `render_with`, `cost_of_out`, allocating generic frontier/greedy/brute results, and `to_lines`/`to_string` helpers move under `research`. The default owned `Rendered<A>` retains a convenient allocating annotated-run iterator. The strict borrowed API does not expose `Out` or allocate annotated-run vectors.

The namespace is unambiguous: feature-gated compatibility items live under `laidout::research`, including `research::Rendered<C>`, `Out`, `CostModel`, `LawfulCostModel`, built-in research models, generic engines, and their output helpers. The crate root owns the new annotation-parameterized `Rendered<A>`, `ConsumerCost`, prepared kernel, and deprecated document-constructor helpers. There is no pair of conditionally competing root `Rendered` definitions. `docs/migration-0.2.md` gives exact old-root to `research::*` import replacements.

“Generic” in the retained research engines continues to mean generic over the cost model. Their compatibility signatures operate on `Doc<u32>` and the existing `TagId = u32` output events; this change does not invent a second arbitrary-annotation research rope. Arbitrary downstream models remain accepted by brute and greedy, which do not prune; the generic frontier continues to require sealed `LawfulCostModel`. Consumer `solve_into`, `render_into`, and owned `render` are the generic-annotation APIs. Differential tests instantiate them with `A = u32` when comparing to research.

Default consumer builds do not depend on `num-bigint`. Public research accumulation is mathematically unbounded in every additive component and does not claim zero allocation.

## 7. Prepared-domain cost proof

### 7.1 Representation

The prepared consumer path exposes a read-only cost value with private fields and uses a crate-private model:

```rust
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ConsumerCost {
    overflow: u128,
    burden: u64,
}

impl ConsumerCost {
    pub fn overflow(self) -> u128;
    pub fn burden(self) -> u64;
}

struct PreparedConsumerCostModel { /* checked prepared-domain operations */ }
```

`ConsumerCost` has no public constructor or addition method. Neither it nor `PreparedConsumerCostModel` implements the unrestricted public `CostModel` trait. Arbitrary callers therefore cannot request an unrepresentable addition through an infallible lawful-algebra interface.

Every overflow and burden addition and every overflow multiplication is checked. Any failed checked operation returns `RenderError::CostDomainViolation` in every build profile; accepted preparation must make that path unreachable, but debug/test builds do not replace the typed failure with a panic. The prepared consumer algebra never saturates either component. In particular, saturating the secondary component is forbidden: for preferred `a = (0, 1)`, later `b = (0, 0)`, and shared suffix `c = (0, u64::MAX)`, `b < a` before extension but `a + c == b + c` after saturating addition, so pruning `a` can change the preferred equal-cost result.

### 7.2 Bound

The consumer kernel supports `target_pointer_width = "32"` and `"64"`; another pointer width fails compilation with an explicit message instead of silently extending the proof. Preparation also rejects output-byte bounds greater than `usize::MAX`. For every `u32` consumer width, preparation requires every candidate line-ending column to fit in `u32`. Therefore `q(c) = max(0, c - width) <= 2^32 - 1`, and the maximum overflow contribution of one materializable line is:

```text
(2^32 - 1)^2 = 2^64 - 2^33 + 1 < 2^64
```

A materializable byte string has fewer than `2^64` bytes on the widest supported target, so it contains at most `2^64 - 1` newline boundaries and at most `2^64` lines. The 32-bit case is strictly smaller. Therefore:

```text
total overflow < 2^64 * 2^64 = 2^128
```

Preparation computes conservative overflow and burden upper bounds for every candidate reachable from the prepared root, including losing alternatives and compact shared-DAG expansions. It uses checked `u128` arithmetic over prepared metadata and does not allocate a `BigUint`. The overflow bound must fit `u128`; the burden bound, including every newline-cost and explicit-penalty contribution, must fit `u64`. If either bound cannot be established or represented, preparation returns `PrepareError::CostDomainExceeded` with the failed `CostComponent` and relevant maximum. Accepted documents therefore close every private checked operation without saturation.

The research oracle uses `BigUint` for both components, then converts a result to `ConsumerCost` only after separately checking `overflow <= u128::MAX` and `burden <= u64::MAX`. This is the second implementation route used to prove that preparation did not omit a losing or shared-DAG derivation from either bound.

### 7.3 Required proof artifacts

`docs/cost-model-proofs.md` must be updated with:

1. the exact accepted prepared domain;
2. the derivation above for every target-width class the crate supports;
3. closure of every checked text, concatenation, newline, and penalty operation in the accepted domain;
4. weak and strict translation preservation in both arguments for lexicographic addition, including `a < b => a + c < b + c` and `a < b => c + a < c + b`, which is the law required by preferred-tie pruning;
5. an explanation of why the private model is not an unrestricted lawful algebra;
6. the feature boundary retaining `BigUint` for unbounded research use.

Automated proof support must include exhaustive reduced-domain checks, property tests against the two-`BigUint` research costs, and the repository's existing SMT proof harness. Each checked arithmetic site has a property that reaches its maximum accepted boundary and one preparation test that rejects the first invalid boundary. A compact shared DAG whose expanded burden first exceeds `u64::MAX` is a mandatory rejection fixture, so source-node sharing cannot hide an invalid additive domain.

The SMT artifact retains integer-domain algebra queries and adds bit-vector queries for each Rust `u128`/`u64` checked operation under the prepared preconditions. It includes the strict-translation obligations that the old saturating secondary component falsifies. Every query asks for a counterexample and must return `unsat`; an unknown result, an omitted arithmetic site, or a model that proves only weak monotonicity fails V05.

## 8. Preparation contract

Preparation performs all work that can allocate or scale with source graph discovery:

1. Traverse the `Arc` graph iteratively and assign dense IDs.
2. Verify opaque-node invariants and cache UTF-8 byte length and display width. Safe source constructors have already rejected invalid text.
3. Flatten sequences, canonicalize empties, and retain `Fill` as a distinct node.
4. Intern text and annotation references without cloning payload values.
5. Preserve ordered-choice and child generation order.
6. Compute both-mode fit summaries, maximum annotation depth, traversal depth, span occurrences, output bytes, line count, indentation, Fast work-stack demand, semantic solve-work demand, Fast fit-decision demand, single-layout plan demand, all-solve candidate-emission demand, and consumer cost bounds.
7. Reject every representation or consumer-domain overflow directly.

Preparation may conservatively reject a compact DAG whose unused alternative describes output or cost outside the consumer domain. That rejection is intentional for the prepared consumer API. The `research` API remains available for unbounded documents.

Preparation does not promise that every finite workspace is sufficient. Bounds supply reservation guidance; runtime still checks each resource because exact frontier size can depend on width and strategy.

### 8.1 Normative bound algebra

Preparation computes bounds from the complete semantic derivation domain, not only from reachable source nodes. The reference evaluator first treats Exact semantics as a symbolic tree without materializing it: `Group(child)` is `Alt(flat(child), child)` when the projection exists, `Fill([c0, ..., cn])` is `Seq(c0, Alt(space, newline), ..., cn)`, source `Choice` is `Alt`, and annotations, penalties, nests, and aligns are unary nodes. A shared source node is counted once for each semantic edge occurrence in this symbolic expansion. Every operation below uses checked `u128`; overflow in the advisory derivation/work recurrences produces `None` metadata without rejecting preparation, while overflow in a representation/output/cost acceptance bound produces `required: None` in its typed preparation error. Nothing wraps or saturates.

For symbolic expression `x`, the evaluator computes derivation count `L(x)`, exhaustive no-memo/no-prune node interpretations `I(x)`, candidates presented to a frontier builder `E(x)`, and implicit binary choices `C(x)`. The recurrence is normative:

```text
leaf:
    L = 1
    I = 1
    E = 1
    C = 0

unary(x):
    L = L(x)
    I = 1 + I(x)
    E = E(x) + L(x)
    C = C(x)

Alt(a, b):
    L = L(a) + L(b)
    I = 1 + I(a) + I(b)
    E = E(a) + E(b) + L(a) + L(b)
    C = 1 + C(a) + C(b)

Seq([]) is the Empty leaf.
For Seq([x0, ..., xn]), start L0 = 1, I0 = 1, E0 = 0, C0 = 0;
for each child x in source order:
    I_next = I_prev + L_prev * I(x)
    E_next = E_prev + L_prev * E(x) + L_prev * L(x)
    L_next = L_prev * L(x)
    C_next = C_prev + C(x)
```

`candidate_emissions_upper = Some(E(root))` and `solve_work_items_upper = checked(I(root) + E(root))` when those `u128` values exist, otherwise the corresponding getter returns `None`. Memoization and dominance can only remove work from this reference expansion; they may not create a derivation or candidate absent from it. Generated small-domain tests run the reference evaluator, an intentionally no-memo/no-prune enumerator, and the production solver and require `actual <= bound` whenever the metadata is `Some`; reduced-counter compact-DAG tests independently reach each actual `StatisticsOverflow` path without constructing astronomical documents.

For any additive selected-layout quantity `M`, leaves carry the quantity's stated weight, unary nodes add their own weight, `Seq` adds child maxima, and `Alt` takes the larger child maximum. This rule computes:

- raw UTF-8 text bytes, with a flat line's space weighing one and a newline weighing zero;
- display-column mass, with text weighing its prepared width;
- newline occurrences;
- penalty burden, with a penalty weighing its `u32` amount;
- annotation/span occurrences;
- total nest increments.

Annotation depth uses `Annotate(x) = 1 + depth(x)`, other unary nodes preserve depth, `Seq` takes the maximum child depth, and `Alt` takes the maximum branch depth. `single_plan_nodes_upper` uses the same selected-layout recurrence except that every output leaf weighs one, `Annotate` and `Penalty` add one, `Nest` and `Align` add zero, and a nonempty `Seq` adds one `Concat` for every child after the first. `visit_work_items` is conservatively equal to `single_plan_nodes_upper`.

`fast_work_items` is computed separately by an abstract execution of the exact Fast stack transitions in section 5.2. It carries only node/mode/indent placeholders, explores both outcomes of each width-dependent Group/Choice/Fill decision, and takes the maximum simultaneous stack length after each specified reversed-child or fill-boundary push. It includes decision and continuation frames and uses the same source child order as production. The abstract executor cannot inspect text bytes or a concrete width. Generated tests compare its bound with the peak actual stack occupancy for every decision outcome on small documents. These are reservation bounds, not guesses based on a representative width.

Let `T`, `D`, `J`, `L`, `P`, and `S` be the selected-layout maxima for raw text bytes, display-column mass, total nest increments, newline occurrences, penalty burden, and spans respectively. The checked consumer bounds are:

```text
column_upper       = D + J
output_bytes_upper = T + L * (1 + column_upper)
line_count_upper   = L + 1
spans_upper        = S
burden_cost_upper  = L * u32::MAX + P
overflow_cost_upper = line_count_upper * column_upper * column_upper
```

Align can capture but never increase the current column, so total prepared display columns plus nest increments is a conservative upper bound for every candidate column and indentation. Preparation requires `column_upper <= u32::MAX`, `output_bytes_upper <= usize::MAX`, `line_count_upper <= usize::MAX`, `spans_upper <= min(usize::MAX, u32::MAX)`, `single_plan_nodes_upper <= min(usize::MAX, u32::MAX)`, `burden_cost_upper <= u64::MAX`, and `overflow_cost_upper <= u128::MAX`. `SpanOccurrences` and `PlanNodes` report the two compact-index failures directly. Exact may still require more total memo-retained plan nodes than one selected layout; that width-dependent condition remains a runtime `WorkspaceCapacityOverflow { resource: PlanNodes, .. }`. Preparation checks every conversion explicitly. Bounds may be conservative across mutually exclusive alternatives, but never depend on which alternative ultimately wins.

`fast_fit_checks_upper` is computed on the original prepared algebra: leaves contribute zero; `Seq` sums; `Group` contributes one plus the maximum of the flat and broken child projections; `Fill` sums child checks plus one for each boundary; `Choice` contributes one plus the maximum branch checks; and other unary nodes preserve the child count. Its getter returns `None` on checked `u128` overflow, and actual Fast execution remains guarded by `SolveCounter::FitChecks`. The deprecated research `count_choices` uses `C(root)` from the exact symbolic expansion, including implicit Group and Fill choices. It performs an independent iterative checked `usize` recurrence and panics with the static message `choice count exceeds usize::MAX` if conversion fails. `brute::best` independently counts only through `max_choices + 1` before enumeration, so it cannot use wrapped or saturated choice counts to bypass its limit.

The implementation keeps this algebra in one `prepare::bounds::reference` module used by preparation and tests. Any optimized DAG-aware evaluator must be property-equal to that reference on generated small graphs, return `None` at the first reduced advisory-work overflow, and reject the first reduced representation/output/cost overflow with the exact typed component. The mandatory compact-DAG fixtures cover repeated zero-byte choices, repeated penalties, annotations around empty output, maximum nesting, and a shared subtree whose expanded burden is the first value above `u64::MAX`.

## 9. Workspace and transaction contract

### 9.1 Mode-aware storage

The consumer path owns reusable storage for:

- the Fast traversal stack, whose entries carry cached continuation summaries;
- Exact work stack;
- fixed-capacity memo slots;
- retained exact candidates;
- separate frontier scratch candidates;
- plan nodes;
- visitor work and annotation stacks;
- materialized bytes and spans.

Every mutation goes through one checked helper that compares logical length or occupancy to both the logical capacity and actual backing capacity before writing. In Fixed mode the helper fails before an allocator call. In Growable mode it first uses retained spare backing, otherwise transactionally enlarges only that backing store with `try_reserve`, updates its logical capacity after success, and then performs the write. A helper may call `Vec::push` only after proving that it cannot grow. Growable-capable arenas store indexes and ranges, never self-referential pointers or slices that reallocation could invalidate. Direct unchecked pushes, `HashMap::insert`, `format!`, `.to_string()`, collecting iterators, and any operation that can bypass the mode policy are forbidden inside solving, visiting, and materialization.

### 9.2 Memo table

Exact memoization uses workspace-owned open addressing with:

- keys `(NodeId, column, indent)`; Fast mode is carried in Fast work frames and never enters the Exact memo;
- a fixed deterministic integer hash with repository-owned test vectors;
- an explicit slot count and maximum occupancy/load rule;
- `u32` generation counters for nonallocating clear;
- no dependency on `std::collections::HashMap` in strict solving.

The table uses power-of-two raw slot counts, linear probing, and a maximum usable load of 70%. Zero usable entries has zero slots; otherwise reservation chooses the smallest power of two of at least eight slots for which `floor(slots * 7 / 10) >= requested_entries`, with every arithmetic step checked. The repository-owned hash is SplitMix64 over the complete key:

```text
mix(x):
    z = x + 0x9e3779b97f4a7c15
    z = (z xor (z >> 30)) * 0xbf58476d1ce4e5b9
    z = (z xor (z >> 27)) * 0x94d049bb133111eb
    return z xor (z >> 31)

hash(node, column, indent) =
    mix(((u64(node) << 32) | u64(column)) xor mix(u64(indent)))
```

All additions and multiplications in `mix` are wrapping `u64` operations by definition. The initial slot is `hash & (slots - 1)`; probes advance by one modulo the slot count. Checked-in vectors cover zero, maxima, and mixed keys, and a tiny-table collision fixture pins probing and rehash behavior.

The reported `memo_entries` capacity denotes usable entries, not raw slots. Reservation computes raw slots and checks arithmetic before allocation.

When a Growable Exact solve needs another usable memo entry, growth allocates a staged slot array, deterministically rehashes only completed entries, and swaps it in after success. Allocation or capacity arithmetic failure leaves the original table and generation valid. Fixed mode never stages or rehashes for growth.

Generation-counter wrap performs a deterministic full slot clear and restarts at generation one. That path is allocation-free and has a test-only reduced counter-width verifier; wrap never revives stale entries.

Memo values refer only to completed ranges in retained candidate storage. Frontier scratch is disjoint from retained candidates. A dominated, failed, or incomplete candidate cannot modify an already published range.

### 9.3 Checkpoint and rollback

At entry, each consumer operation:

1. reads the workspace mode and selected strategy without changing either;
2. resets logical lengths or establishes a checkpoint without deallocation;
3. performs only checked writes;
4. publishes a borrowed result only after completion.

On any solver, visitor, materialization, downstream visitor, or Growable allocation error, rollback clears all logical lengths, open annotations, memo generations, pending published ranges, and plan roots needed for safe reuse. Successfully enlarged capacities are retained; a failed transactional enlargement leaves that resource's previous capacity intact.

After an error, rendering the same valid document with sufficient capacity in the same workspace must return exactly the same result as rendering in a fresh workspace.

### 9.4 Errors

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum WorkspaceResource {
    FastWorkItems,
    ExactWorkItems,
    MemoEntries,
    RetainedCandidates,
    FrontierScratch,
    PlanNodes,
    VisitWorkItems,
    AnnotationDepth,
    OutputBytes,
    Spans,
}

#[derive(Debug, thiserror::Error)]
pub enum RenderError {
    WorkspaceCapacityOverflow {
        resource: WorkspaceResource,
        required: Option<u128>,
        maximum: u128,
        stats: SolveStats,
    },
    WorkspaceExhausted {
        resource: WorkspaceResource,
        capacity: usize,
        required: usize,
        stats: SolveStats,
    },
    WorkspaceGrowthFailed {
        resource: WorkspaceResource,
        capacity: usize,
        required: usize,
        stats: SolveStats,
        source: TryReserveError,
    },
    StatisticsOverflow {
        counter: SolveCounter,
        required: u128,
        maximum: u64,
        stats: SolveStats,
    },
    CostDomainViolation,
    CostWitnessMismatch,
}
```

`WorkspaceCapacityOverflow` is mode-independent and occurs before allocator policy when a logical requirement cannot fit the resource's compact index or backing representation; `required: None` means even the requirement computation overflowed. `WorkspaceExhausted` is Fixed-only, and `WorkspaceGrowthFailed` is Growable-only. `StatisticsOverflow` is also mode-independent, always reports the attempted value (`u64::MAX + 1` in production) and the last complete pre-action statistics, and never masquerades as workspace exhaustion. These public error, resource, component, and counter enums are exhaustive in `0.2.0`; do not add `#[non_exhaustive]`. Termosaic maps every variant into an owned public error without a catch-all, and both repositories' closed error fixtures fail when a variant is missing or added. A later variant addition is therefore an explicit SemVer change. Error construction and display during the strict measured call may not allocate. Tests compare structured fields, not only display strings.

Visitor failures are represented as `VisitError::Workspace(RenderError)` or `VisitError::Visitor(E)`. Fixed materialization failure maps to `WorkspaceExhausted`; Growable materialization allocation failure maps to `WorkspaceGrowthFailed`; both identify the correct materialization resource.

All public Laidout error enums governed here are exhaustive in `0.2.0`. `tests/fixtures/kernel-invalid-cases.toml` contains one uniquely identified, directly triggered case for every public `TextError`, `PrepareError`, `ReserveError`, `RenderError`, `VisitError`, `OwnedRenderError`, and `TableError` variant. A source-contract test derives the live variant vocabulary and requires exact fixture equality. Allocation failures use a scoped failing allocator in an isolated serial test binary; invariant-only `CostDomainViolation` and `CostWitnessMismatch` use distinct test-only corrupted prepared/plan fixtures that cannot enter the public constructor path.

## 10. Solver and plan arenas

### 10.1 Fast

Fast solving writes a winning plan into the shared plan arena while interpreting prepared nodes. It uses only `FastSolveCapacity`, common plan storage, and later visit storage. It never initializes or touches Exact memo/candidate resources.

Its rendering interpreter consumes the same projection used to derive the prepared fit summaries, so line behavior cannot drift. Stack work is charged per visited prepared item, while each fit decision reads one candidate summary and the stack's cached continuation aggregate. Each text record contributes only its cached display width, including zero for zero-width graphemes. Fast has no fit traversal, fit-stack exhaustion, or width-dependent fallback path.

### 10.2 Exact

Exact frontier construction streams candidates through an arena-backed dominance builder, performs stable incremental pruning, and copies only the completed frontier into retained candidate storage. It does not build all pairwise products before pruning.

Retained candidates contain compact cost, end-state metadata, a checked `u64` source-order precedence equal to `candidates_generated` immediately before that counter is incremented, and `PlanId`. A scratch candidate instead contains the same comparison fields plus an inline `PendingPlan`; it does not publish a plan node before dominance. Plan nodes live in one reusable arena. Candidate ranges contain indexes, never slices or pointers invalidated by arena reuse. Choice emits the complete preferred frontier before the alternative; concatenation emits the left/right Cartesian product in lexicographic source-precedence order; unary nodes preserve child order. Final selection compares `(cost, precedence)`.

The preference-aware dominance reference is exact:

```text
earlier a dominates later b when
    (a.last == b.last and a.cost <= b.cost)
 or (a.last <  b.last and a.cost <  b.cost)

new b removes retained a when
    b.last <= a.last and b.cost < a.cost
```

Equal-cost shorter output does not dominate a different-column earlier derivation; that is the regression that previously lost preferred text/spans. Completed frontiers are stored in source-precedence order.

The production builder is a deterministic intrusive AVL tree keyed by `last`, stored entirely in the `frontier_scratch` slot arena. The dominance invariant permits at most one live candidate for a given `last`. Each live slot contains its inline `PendingPlan`, left/right/parent slot indexes, AVL height, source-precedence previous/next indexes, and a free-list link. Predecessor/same-column lookup, insertion, rotation, and removal are O(log F) for live frontier size `F`; a contiguous run of dominated successors is removed in O((k + 1) log F), and each removed candidate can be charged only once to the emission that originally inserted it. No hash, randomized priority, address, or standard allocating tree participates.

Candidates also form an intrusive doubly linked list in emission/source-precedence order. Finalization walks that list directly, commits survivor plans, and copies retained candidates without a sort or second arena. A candidate is compared read-only first. If it is dominated, no slot or plan is published. If it survives, the builder identifies the complete removal set and proves that a free slot exists—or transactionally grows/reports `FrontierScratch`—before mutating the AVL tree or precedence list; removed slots may satisfy that need. Thus a capacity/growth failure leaves the current frontier unchanged. AVL rotations, all deletion shapes, same-column replacement, free-slot reuse, source-list order, and the direct dominance predicate have checked deterministic unit vectors plus generated oracle comparison.

Precedence is the checked actual emission ordinal, not an exhaustive prepared-domain rank. If the next candidate cannot be assigned a `u64` ordinal, the atomic counter preflight returns `StatisticsOverflow { counter: CandidatesGenerated, .. }` before frontier mutation. A test-only reduced counter reaches that first invalid emission. Precedence is never saturated or derived from hash/slot order.

### 10.3 Plan publication

The consumer plan vocabulary is fixed:

```rust
enum PlanNode {
    Empty,
    Text(TextId),
    Newline { indent: u32 },
    Concat { left: PlanId, right: PlanId },
    Annotate { annotation: AnnotationId, child: PlanId },
    Penalty { amount: u32, child: PlanId },
}

enum PendingPlan {
    Existing(PlanId),
    Empty,
    Text(TextId),
    Newline { indent: u32 },
    Concat { left: PlanId, right: PlanId },
    Annotate { annotation: AnnotationId, child: PlanId },
    Penalty { amount: u32, child: PlanId },
}
```

The frontier builder compares and prunes scratch candidates while their plan is pending. After the frontier is complete, finalization first counts surviving non-`Existing` wrappers and retained candidates, checks compact-index arithmetic, and ensures the complete additional `PlanNodes` and `RetainedCandidates` capacity in that order before appending either. Fixed failure and Growable allocation failure therefore occur with no partial plan/frontier publication; a successfully enlarged backing remains retained under the ordinary rollback rule. Once preflight succeeds, finalization commits one immutable `PlanNode` for each surviving pending wrapper and copies the resulting candidates into retained storage; `Existing` reuses its ID without appending. Candidates discarded before completion create no plan nodes and consume no retained-candidate capacity. Child frontier plan IDs already exist before a parent wrapper is built, so every committed edge points to an already-published lower `PlanId`. Retained memo frontiers keep their committed plans because they remain valid solver state, even when the final root selects another frontier member. Fast commits only the nodes in its selected layout.

Penalty nodes are structural cost witnesses, not text events. Traversal emits their callback in source order and then visits the child. A `LayoutRef` contains the completed root plan ID, root `ConsumerCost`, the scalar cost parameters from `RenderOptions`, and `SolveStats`, and borrows both prepared data and workspace. No safe API exposes a plan ID after the borrow ends.

Plan traversal is iterative and uses `VisitCapacity`. Insufficient visit capacity can therefore fail after a successful solve; rollback still restores the complete workspace contract.

## 11. Termosaic integration

### 11.1 Document wrappers

Termosaic owns these public wrappers:

```rust
#[derive(Clone, Eq, PartialEq)]
pub struct Doc(laidout::Doc<TokenId>);

#[derive(Clone)]
pub struct PreparedDoc(laidout::PreparedDoc<TokenId>);

impl Doc {
    pub fn empty() -> Self;
    pub fn text(value: impl AsRef<str>) -> Self;
    pub fn from_text(value: impl AsRef<str>, layout: TextLayout) -> Self;
    pub fn line() -> Self;
    pub fn soft_line() -> Self;
    pub fn hard_line() -> Self;
    pub fn token(token: TokenId, doc: Self) -> Self;
    pub fn group(doc: Self) -> Self;
    pub fn fill(docs: impl IntoIterator<Item = Self>) -> Self;
    pub fn nest(indent: u16, doc: Self) -> Self;
    pub fn align(doc: Self) -> Self;
    pub fn concat(docs: impl IntoIterator<Item = Self>) -> Self;
    pub fn join(separator: Self, docs: impl IntoIterator<Item = Self>) -> Self;
    pub fn choice(preferred: Self, alternative: Self) -> Self;
    pub fn penalize(amount: u32, doc: Self) -> Self;
    pub fn prepare(&self) -> Result<PreparedDoc, HumanPrepareError>;
}
```

The tuple fields are private. Laidout is a mandatory internal implementation dependency, not part of Termosaic's public vocabulary: no public signature, field, re-export, error source, Debug representation, or documentation example names a `laidout::*` type. `tests/source_contract.rs` adds `laidout` to the private-backend identifier rejection list. Termosaic owns its strategy, capacity, resource, and error types and maps them exhaustively at the private seam.

Existing `text`, `from_text`, `token`, `group`, `nest`, `concat`, `join`, and `fill` constructors retain the signatures above and their current byte/sanitization behavior. `empty`, `align`, ordered `choice`, and `penalize` expose the useful remainder of Laidout's algebra without exposing raw annotations; Termosaic continues to own token semantics through `token`. All internal tests, examples, and documentation migrate `Doc::Line`, `Doc::SoftLine`, and `Doc::HardLine` to methods.

`Doc::text` and every `from_text` path remain infallible. After sanitization, Termosaic preserves an empty logical value as one empty Laidout text node, matching the current `Doc::Text(HumanText(""))` equality/Debug distinction; the plan walker emits no `text` callback for that zero-byte node. A nonempty value is split only at UTF-8 scalar boundaries into the smallest ordered sequence of nonempty Laidout text runs whose individual display widths fit `u32`; ordinary inputs remain one run and rendered bytes are unchanged. It never splits a scalar or inserts layout opportunities. Preparation still rejects a complete candidate line whose aggregate column/cost domain exceeds the checked consumer bounds, and the ergonomic writer reports the existing typed `HumanPrepareError` path rather than panicking in the constructor. Migration documentation names this new typed failure for inputs larger than the prepared consumer domain.

Termosaic's derived-looking traits are implemented over its canonical wrapped algebra. `Clone`, `Eq`, and `PartialEq` delegate to Laidout structural behavior, so `Doc::concat([]) == Doc::empty()` and `Doc::concat([x]) == x`; this is an intentional `0.2.0` source-semantic change from enum-tree identity and is covered by migration examples. Custom `Debug` is exactly `formatter.debug_tuple("Doc").field(&self.0).finish()`: Laidout's ordinary `Debug` supplies the canonical operation expression and Termosaic supplies the owned outer label. The result contains no `laidout` path, address, or externally reusable handle/index; deterministic result-local `#n` sharing labels are permitted. Debug output is not a serialization contract.

`HumanText` sanitization and line-ending normalization remain at the Termosaic boundary. The wrapper never allows unchecked text with embedded newlines into Laidout.

### 11.2 Retained writer state and capacity

```rust
pub struct HumanWriter {
    destination: Arc<DestinationState>,
    theme: Theme,
    layout: laidout::RenderWorkspace,
    ansi: Vec<u8>,
    ansi_limit: usize,
    active_run: Option<ActiveAnsiRun>,
    deferred_indent: Option<u32>,
    last_was_newline: bool,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HumanRenderCapacity {
    pub fast: HumanFastCapacity,
    pub exact: HumanExactCapacity,
    pub plan_nodes: usize,
    pub visit: HumanVisitCapacity,
    pub ansi_bytes: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HumanFastCapacity {
    pub work_items: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HumanExactCapacity {
    pub memo_entries: usize,
    pub retained_candidates: usize,
    pub frontier_scratch: usize,
    pub work_items: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HumanVisitCapacity {
    pub work_items: usize,
    pub annotation_depth: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LayoutStrategy {
    Fast,
    Exact,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HumanLayoutOptions {
    strategy: LayoutStrategy,
    newline_cost: u32,
}

impl HumanLayoutOptions {
    pub fn fast() -> Self;
    pub fn exact() -> Self;
    pub fn with_newline_cost(self, newline_cost: u32) -> Self;
    pub fn strategy(&self) -> LayoutStrategy;
    pub fn newline_cost(&self) -> u32;
}
```

Both `HumanLayoutOptions` constructors use newline cost one. Options never expose width or indentation policy: width remains instance-owned destination policy, and the private adapter always selects `ClampToWidthMinusOne`. Termosaic deliberately adopts Laidout's column-accurate Fast fitting; there is no compatibility switch for the retired scan budget.

Termosaic privately converts its capacity and strategy types to the corresponding Laidout values and maps them back field-for-field. It supplies `MaterializeCapacity { output_bytes: 0, spans: 0 }`, because document writing visits a plan and never materializes text or spans. A newly constructed `HumanWriter` owns an empty Growable layout workspace and an empty ANSI buffer without eagerly allocating either strategy's storage.

`HumanWriter::reserve_rendering` may allocate and is transactional across both layers: it stages replacement ANSI backing first, invokes Laidout's transactional `reserve`, and swaps the ANSI backing and logical limit only after both succeed. `warm_rendering` performs a no-write Growable solve and ANSI visit, clears logical state, and returns the retained capacity snapshot. Repeated warming is monotone: it never shrinks a previously retained resource. The strict guarantee applies when the caller retains that `HumanWriter`, prepares the document, reserves directly or warms representative documents, and then invokes strict rendering. A fresh `Console::human()` still creates a fresh writer; the ergonomic chained form is not claimed to be allocation-free.

`HumanWriter::release_rendering_capacity` calls Laidout's `release_capacity`, drops the retained ANSI backing, resets its ANSI logical limit to zero, and leaves destination, theme, and writer usability unchanged. It performs no allocation and may deallocate. This lets a retained writer shed an exceptional Growable or Exact high-water mark without reacquiring a destination handle; subsequent ergonomic calls regrow normally and subsequent strict calls require reservation or warming again.

The writer API is:

```rust
impl HumanWriter {
    pub fn reserve_rendering(
        &mut self,
        capacity: HumanRenderCapacity,
    ) -> Result<(), HumanReserveError>;

    pub fn rendering_capacity(&self) -> HumanRenderCapacity;

    pub fn release_rendering_capacity(&mut self);

    pub fn warm_rendering(
        &mut self,
        doc: &PreparedDoc,
        options: HumanLayoutOptions,
    ) -> Result<HumanRenderCapacity, HumanLayoutError>;

    pub fn doc(&mut self, doc: &Doc) -> Result<(), ConsoleError>;

    pub fn doc_with(
        &mut self,
        doc: &Doc,
        options: HumanLayoutOptions,
    ) -> Result<(), ConsoleError>;

    pub fn doc_prepared(
        &mut self,
        doc: &PreparedDoc,
        options: HumanLayoutOptions,
    ) -> Result<(), HumanRenderError>;

    pub fn doc_strict(
        &mut self,
        doc: &PreparedDoc,
        options: HumanLayoutOptions,
    ) -> Result<(), HumanRenderError>;
}
```

`doc` forwards to `doc_with` using `HumanLayoutOptions::fast()`, preserving its existing default strategy and all output outside the V29 behavior class. `doc_with` prepares and delegates to `doc_prepared`. `doc_prepared` selects Growable mode for both Laidout and ANSI storage and supports either strategy without restarting or substituting the other. `warm_rendering` uses the same Growable visitor path but performs no destination write. `doc_strict` selects Fixed mode at entry and never prepares, reserves, grows, or silently changes the requested strategy. A later ergonomic call may select Growable again; mode changes themselves are scalar and nonallocating.

The primary strict sizing workflow is therefore executable without guessing arena sizes:

```rust
let prepared = doc.prepare()?;
let options = HumanLayoutOptions::exact();
writer.warm_rendering(&prepared, options)?; // may allocate, writes nothing
writer.doc_strict(&prepared, options)?;     // fixed, zero library allocations
```

`doc` retains its current `Result<(), ConsoleError>` signature. Add `ConsoleError::HumanLayout(HumanLayoutError)` for preparation, solving, visiting, or ANSI failures on the ergonomic path. `doc_with` maps `HumanRenderError::Layout(error)` to that variant and maps `HumanRenderError::Destination(error)` back to the original `ConsoleError` unchanged. `HumanRenderError`, used directly by prepared and strict calls, keeps layout and destination failures separate without recursion or string conversion. `HumanReserveError` remains the direct error from explicit reservation.

The required error distinctions are:

```rust
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HumanPreparedResource {
    Nodes,
    Edges,
    Texts,
    Annotations,
    TextBytes,
    DisplayColumns,
    OutputBytes,
    LineCount,
    Indentation,
    AnnotationDepth,
    SpanOccurrences,
    PlanNodes,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HumanCostComponent {
    Overflow,
    Burden,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HumanSolveCounter {
    WorkItems,
    FitChecks,
    MemoHits,
    MemoMisses,
    CandidatesGenerated,
    CandidatesPruned,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum HumanWorkspaceResource {
    FastWorkItems,
    ExactWorkItems,
    MemoEntries,
    RetainedCandidates,
    FrontierScratch,
    PlanNodes,
    VisitWorkItems,
    AnnotationDepth,
    OutputBytes,
    Spans,
    AnsiBytes,
}

#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum HumanPrepareError {
    RepresentationExceeded {
        resource: HumanPreparedResource,
        required: Option<u128>,
        maximum: u128,
    },
    CostDomainExceeded {
        component: HumanCostComponent,
        required: Option<u128>,
        maximum: u128,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum HumanReserveError {
    CapacityOverflow {
        resource: HumanWorkspaceResource,
        requested: usize,
    },
    Allocation {
        resource: HumanWorkspaceResource,
        requested: usize,
        source: TryReserveError,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum HumanLayoutError {
    Prepare(HumanPrepareError),
    WorkspaceCapacityOverflow {
        resource: HumanWorkspaceResource,
        required: Option<u128>,
        maximum: u128,
    },
    WorkspaceExhausted {
        resource: HumanWorkspaceResource,
        capacity: usize,
        required: usize,
    },
    WorkspaceGrowthFailed {
        resource: HumanWorkspaceResource,
        capacity: usize,
        required: usize,
        source: TryReserveError,
    },
    StatisticsOverflow {
        counter: HumanSolveCounter,
        required: u128,
        maximum: u64,
    },
    CostDomainViolation,
    CostWitnessMismatch,
    AnsiSizeOverflow {
        current: usize,
        additional: usize,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum HumanRenderError {
    Layout(HumanLayoutError),
    Destination(ConsoleError),
}
```

Every Laidout prepare, cost-component, solve-counter, reserve, render, workspace-resource, and strategy variant has a total private mapping to the corresponding Termosaic-owned variant. Laidout `VisitError::Workspace` uses the render mapping; `VisitError::Visitor` maps the Termosaic visitor's typed ANSI error. Fixed ANSI exhaustion maps to `WorkspaceExhausted { resource: AnsiBytes, .. }`; Growable ANSI `try_reserve` failure maps to `WorkspaceGrowthFailed { resource: AnsiBytes, .. }`. `HumanLayoutError` intentionally omits Laidout's full solver statistics because Termosaic does not otherwise expose that engine diagnostic surface, but preserves the overflowing counter and attempted/maximum values. `CostWitnessMismatch` remains distinct from arithmetic-domain failure, so an internal replay inconsistency cannot be mislabeled as caller capacity. The enum variants and fields shown above are exact and exhaustive for `0.2.0`; `#[source]` attributes do not change their shape.

Every new public error type and variant is added to Termosaic's closed source-contract fixture in `tests/fixtures/console-invalid-cases.toml` and to the parser assertions in `tests/source_contract.rs`.

### 11.3 ANSI visitor

Termosaic preparation wraps the source root in the default token annotation. Annotation interning therefore gives unannotated text and explicitly default-token text the same `AnnotationId`, preserving their current coalescing behavior.

Every `warm_rendering`, `doc_prepared`, and `doc_strict` attempt creates a stack-local nonallocating `HumanRenderAttempt` guard before solving. The guard owns exclusive access to the ANSI buffer and scalar visitor state for the complete solve/visit/handoff scope; its `Drop` clears buffer length, active run, deferred indentation, and last-byte state without shrinking capacity. Normal success, every typed return, visitor panic, and destination panic therefore take the same cleanup path. Laidout's independent borrowed-result guard resets the layout workspace. The Termosaic guard exposes the completed byte slice to `write_human` but is not disarmed, so cleanup still occurs after a successful or failed handoff.

The Termosaic visitor obtains `(AnnotationId, &TokenId)` from `active_annotations.innermost()`; no token is cloned. `ActiveAnsiRun` contains only that ID and a `reset_count: u8` in the range `0..=2`, enough to close the exact prefix sequence without retaining or cloning a token/style. Before processing the first nonempty text after a newline, it appends deferred indentation with a capacity-checked resize or fixed-space chunks; those bytes precede any ANSI prefix. On a new ID it then closes the previous style and opens the new style, followed by the text bytes. A newline closes the active style, discards any previous deferred indentation, appends an unstyled newline, stores the new indentation scalar, and clears the active run. End-of-document discards deferred indentation. No second text buffer is needed, and Laidout never calls `text` with an empty slice.

The direct byte encoder preserves the currently resolved `console 0.16.4` forced-style grammar exactly:

1. Disabled color, empty text, or the default style emits raw text and no reset.
2. Normal ANSI foreground emits `ESC[30m` through `ESC[37m`. ANSI-256 emits `ESC[38;5;<n>m`; true color emits `ESC[38;2;<r>;<g>;<b>m`.
3. Attributes follow the foreground as separate sequences in the fixed order bold `ESC[1m`, dim `ESC[2m`, underline `ESC[4m`.
4. Any normal foreground or attribute sequence is closed by exactly one `ESC[0m`.
5. A bright ANSI fallback preserves the existing nested wrapper: outer `ESC[90m` through `ESC[97m`, then the corresponding normal `ESC[30m` through `ESC[37m`, then attributes, text, the inner `ESC[0m`, and the outer `ESC[0m`. It therefore uses `reset_count = 2`.
6. `Color::Rgb` uses true color at `TrueColor`, the existing deterministic quantized index at `Ansi256`, and its declared ANSI fallback at `Ansi16`. Forced styling makes stdout and stderr byte-identical for the same style; stream-specific destination policy still decides whether styling is enabled.

A frozen golden matrix covers no foreground plus all eight attribute combinations, every ANSI color plus all attribute combinations, representative RGB values at all three depths, empty text, both streams, and fragmented/coalesced runs. The baseline `console::Style` implementation remains a test-only oracle for the matrix and direct-emitter differential tests throughout `0.2.0`; production document and span rendering uses the direct encoder.

Neither mode uses `format!`, `.to_string()`, a temporary `String`, a styled span vector, or a layout-event vector. ANSI numeric fields are written with a stack-local integer encoder or fixed byte tables.

Before each buffer extension, the visitor checks `current + additional`, then the logical ANSI capacity. Arithmetic overflow returns `AnsiSizeOverflow`. In Fixed mode, a representable one-past-limit requirement returns `WorkspaceExhausted` with `AnsiBytes`, `capacity`, and `required` before an allocator call. In Growable mode, the visitor uses retained spare `Vec` capacity when possible, otherwise calls `try_reserve`; after success it raises `ansi_limit` to exactly `required`, not allocator-dependent spare capacity. Allocation failure maps to `WorkspaceGrowthFailed`. Any failure clears the Termosaic ANSI buffer, resets active-run and deferred-indent state, and causes the consumed Laidout visit to roll back; successfully retained logical capacity remains available.

The visitor tracks whether the last emitted byte is a newline and enforces the terminal-newline rule without a second allocation or second sink write.

### 11.4 Destination integration

After a successful visit, `doc_prepared` and `doc_strict` call the existing `DestinationState::write_human` exactly once; `doc_with` reaches that path after preparation. `warm_rendering` never calls the destination. The destination retains its current progress suspension and locking policy. No output is written if preparation, solving, visiting, styling, capacity, or growth checks fail. A short-writing sink may receive multiple underlying `Write::write` calls from that one `write_all`; the contract is one logical handoff and one payload lock scope, not one trait-method invocation. Active progress clear/redraw locks are separate backend activity.

After `write_human` returns, whether successfully or with a destination error, `doc_prepared` and `doc_strict` clear the ANSI buffer and scalar visitor state while retaining capacity. `warm_rendering` performs the same cleanup without calling `write_human`. A destination error can represent a partial operating-system write and is not claimed to be externally transactional; it does not poison the reusable writer.

The unconditional allocator assertion ends after Laidout solving/visiting and Termosaic ANSI generation/cleanup logic; it excludes the progress backend and caller-provided `Write`. A second end-to-end assertion measures the complete `doc_strict` call with no active progress handle and a preinitialized fixed-capacity sink whose `write` cannot allocate. With an active progress handle, the production `indicatif::ProgressBar::suspend` path is exercised for byte ordering, one suspension, one payload lock scope, separately counted clear/redraw locks, and recovery, but it is not replaced by a fake adapter and is not labeled allocation-free. The allocator behavior of an arbitrary external `Write` remains outside the library guarantee.

`render_text` remains as an owned compatibility wrapper for progress and non-document call sites. It delegates to a new `render_text_into` byte emitter, computes the exact additional byte requirement with checked arithmetic, and calls `try_reserve` before emission when retained capacity is insufficient. Existing progress output is not allowed to regress while document rendering migrates.

## 12. Dependency and feature contract

Laidout's manifest boundary is exact:

```toml
[features]
default = []
research = ["dep:num-bigint"]

[dependencies]
num-bigint = { version = "0.4.6", optional = true }
thiserror = "2"
unicode-width = "0.2"

[dev-dependencies]
proptest = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
static_assertions = "1"
toml = "0.9"
trybuild = "1"
```

The default library is the complete compact standalone consumer kernel. `num-bigint`, the generic cost traits/engines, brute oracle, old `Out` surface, and research measurements compile only with `research`. Default tests cannot import those APIs accidentally; oracle tests are feature-gated and are also run explicitly with `research`. Laidout has no `termosaic` dependency, dev-dependency, feature, build script probe, conditional compilation key, public vocabulary, example import, or test import. Its default and research verification both complete from the Laidout checkout alone.

Laidout declares `[[bench]] name = "prepared_kernel", harness = false, required-features = ["research"]`. Termosaic declares `[[bench]] name = "human_doc", harness = false`. This keeps default all-target checks buildable while making the benchmark commands in section 15 exact.

Termosaic depends on Laidout's default consumer surface. The dependency declaration must work both in the local workspace/test setup and as a publishable crates.io version; a temporary path-only dependency is not an accepted final state.

Both crates take a pre-1.0 breaking release at `0.2.0`. During cross-repository development Termosaic declares `laidout = { version = "0.2.0", path = "../laidout" }`; the path is local-development metadata and the version is the publish contract. Laidout remains absent from Termosaic's public API despite being a required dependency. Actual registry publication is outside implementation acceptance and occurs in dependency order, Laidout before Termosaic.

Termosaic's `scripts/verify.sh` retains package verification as a hard gate. When the sibling checkout exists, the script canonicalizes it before invoking Cargo so CLI-config path resolution is independent of the packaged archive's extraction directory:

```sh
laidout_path="$(cd ../laidout && pwd -P)"
cargo package --allow-dirty \
  --config "patch.crates-io.laidout.path='$laidout_path'"
```

Cargo's version selection must choose `laidout 0.2.0`; a wrong package name/version makes the package command fail. Without a sibling checkout the script performs ordinary `cargo package --allow-dirty`, which must resolve the registry version or fail. The gate is never skipped. Laidout's own `cargo package --allow-dirty` passes first.

Termosaic's existing feature set (`json`, `progress`, `serde`, `adaptive-tables`) keeps its behavior. `laidout` is a nonoptional dependency. Direct `unicode-width` and `unicode-segmentation` remain normal Termosaic dependencies because fixed/adaptive table formatting still uses them; removing the production document interpreter does not move those table responsibilities. `console` remains private for terminal detection and compatibility helpers and is pinned as `console = "=0.16.4"`; the test-only baseline-style oracle imports that same resolved package, while the checked-in matrix records `oracle_package = "console"`, `oracle_version = "0.16.4"`, and the dependency-graph digest. An intentional oracle update changes the exact manifest pin, regenerates the complete matrix, and updates this contract together. `indicatif`, `comfy-table`, and `serde_json` remain private optional backends. Termosaic adds `static_assertions = "1"` and `trybuild = "1"` to dev-dependencies; its existing `serde_json`, `toml`, and `walkdir` dev-dependencies support the artifact and source-contract harnesses. The exact source-contract vocabulary is updated in the same change, including rejection of public `laidout` identifiers.

No public API claims `Send` or `Sync` without compile-time assertions. The all-`Arc` source/prepared graph is `Send + Sync` exactly when `A: Send + Sync`; `RenderWorkspace`, `LayoutRef`, and `RenderedRef` are not required to be `Sync`. Termosaic preserves the current `Send + Sync` auto-traits of `Console`, `HumanWriter`, `MachineTextWriter`, and `OpaqueWriter`, verified with static assertions after reusable storage is added.

## 13. Implementation dependency graph

This is dependency order, not a calendar or permission to stop at an intermediate state.

1. **Lock baseline oracles and measurements.** Preserve current Laidout exact outputs/costs/ties and current Termosaic layout/ANSI byte fixtures. Freeze the baseline Termosaic enum/interpreter and resolved `console 0.16.4` ANSI grammar in test targets, add the neutral `CompatibilityDoc` conversion harness, add generated differential cases, freeze each benchmark's `harness_v1`, workload manifest, and self-contained `0.1` baseline adapter, and capture the exact section 15 baseline artifacts before replacing production paths. The Laidout baseline adapter snapshots only the current document/cost/frontier/output code required by the checked workloads and imports no mutable production layout module; the Termosaic adapter uses its neutral workload AST plus the frozen interpreter/ANSI oracle rather than the later public wrapper. Final prepared adapters are added separately and never edit either frozen baseline adapter or measurement harness. The Laidout pre-harness production baseline is commit `38c55341e02fd9e4869e060d36e88886c7ed1ea3`, `src` tree `8a6ba8b5d7ad618e409118d37d8731d0ca319302`, and manifest blob `e679d0b11b963edc27ef243fa4ee8c10a2f4b442`; the Termosaic pre-harness baseline is commit `06fed46e2eb9aee88945c76c251a2eeeded9097d`, full tree `f099a7e545d3bfd26227827410f0c395a9ea44e5`, `src` tree `a06a417a9ad025a27fa7f70bc58e7720b2891e07`, and manifest blob `cdfe7265fb4204c3dd311263b2deda99f45f0774`. Baseline capture permits only the new harness/workload/baseline-adapter/test-oracle files, benchmark declarations, and an empty Laidout `research` feature needed by the benchmark target; it verifies the pinned production `src` tree and requires the resolved runtime dependency graph and production behavior to remain baseline-identical. Check: baseline tests, the exhaustive style matrix, both neutral-AST conversions, frozen-adapter equivalence to the live pre-change implementation, baseline identity checks, and schema-valid benchmark artifact generation pass with no production behavior change. Once captured, any measurement-harness, workload, or baseline-adapter change invalidates both artifacts and requires recapture against these pinned source identities before production migration continues; adding or changing only the final adapter invalidates only the candidate artifact.
2. **Move Laidout ownership to `Arc` and generic annotations.** Make `Doc<A>` opaque and n-ary, add constructors, iterative structural traits, and iterative final destruction, migrate the fixed-`u32` text/token/table/corpus helpers, migrate examples/tests, and add conditional `Send + Sync` assertions. Check: source graph, deep equality/hash/debug/drop, concurrent cloned-root drop, compatibility-helper, and public-constructor tests pass.
3. **Add preparation.** Implement dense typed IDs, `Seq`, distinct `Fill`, annotation/text interning, iterative traversal, both-mode fit summaries, the section 8.1 reference bound algebra, optional semantic-work metadata, and direct `PrepareError`s. Check: canonicalization, no-payload-clone, optimized/reference/no-prune bound equivalence when representable, `None` on advisory `u128` overflow without rejection, summary/reference-scan equivalence, compact shared-DAG expansion bounds, deep-DAG behavior, and every first-invalid acceptance-bound test pass.
4. **Establish the cost domain.** Implement private checked `(u128, u64)` cost without saturation, complete overflow/burden preparation bounds, two-`BigUint` research models, the research feature split, updated proof document, property/SMT comparisons including strict translation, and boundary rejection. Check: the proof verifier, old-saturation counterexample, compact-DAG burden rejection, and reduced/exhaustive comparisons pass.
5. **Add workspace infrastructure.** Implement independent Fast/Exact capacities, Growable/Fixed modes, checked storage helpers, deterministic memo growth/fixed exhaustion, generation clearing, checkpoints, rollback, per-resource errors, and explicit retained-capacity release. Check: all four strategy/mode combinations compile; direct unit tests grow or exhaust every resource as required, demonstrate same-workspace recovery, and prove release returns every logical/actual retained resource to empty without allocation.
6. **Add prepared Fast without replacing research greedy.** Implement `src/fast.rs` with the shared prepared projection, O(1) prepared-summary fit decisions, cached continuation aggregates on ordinary work-stack entries, plan emission, checked semantic counters, and indentation policy without a fit stack or semantic scan cutoff; keep the arbitrary-model allocating greedy engine feature-gated under `research`. Check: Laidout's independent scan oracle agrees on every generated decision, long zero-width text remains column-correct, nested zero-width probes satisfy the section 15 linear-scaling gate, `fit_checks` equals the number of semantic decisions, reduced-counter overflow is typed without changing a fit decision, insufficient Fixed work/plan capacity reports its existing typed resource rather than selecting a different layout, and an intentionally unsealed `CostModel` still runs through research brute/greedy but cannot compile against research frontier.
7. **Port Exact.** Implement stable streaming frontiers, separate scratch/retained arenas, inline pending plans, survivor-only plan commitment, memo publication, checked actual-emission precedence/statistics, and preferred ties. Check: brute/research oracle equality for text, structural events, reconstructed cost, annotations, and ties passes; reduced counters fail before frontier mutation; a dominance-heavy fixture proves pruned scratch candidates append no plan nodes.
8. **Add visitor and materializer.** Implement `LayoutRef`, `ActiveAnnotations`, structural annotation/penalty callbacks, `visit`, `RenderedRef`, spans including zero-width spans, independent cost replay, resolvers, mode-aware growth/exhaustion, and owned wrappers with their conditional trait bounds. Check: structural visitor/materializer/cost equivalence, corrupted-witness error, borrow compile-fail tests, fixed exhaustion, Growable allocation-failure injection, and all four strategy/mode result comparisons pass.
9. **Close the standalone Laidout boundary.** Add the custom-annotation standalone example and external-consumer test, document all four strategy/mode combinations, and enforce the absence of Termosaic coupling in Laidout's manifest and source. Check: the example and test pass with `--no-default-features`, both borrowed and owned paths are exercised, and the dependency/source scan is empty.
10. **Migrate Termosaic documents.** Add private wrapper fields, the full Termosaic-safe algebra, UTF-8-safe infallible text chunking, canonical equality/custom Debug, Termosaic-owned strategy/capacity/resource/error types including cost-witness mismatch, exhaustive private mappings, the Laidout dependency, source migrations, and retained old-layout oracle tests. Check: constructor/trait migration cases pass; aggregate domain overflow is typed; all old/new rendering differences are classified by the first-decision V29 trace rule; the canonical fixture asserts exact authorized old/new bytes; every case outside that class is byte-equal; new choice/penalty tests distinguish Fast from Exact intentionally; and the public-source scan rejects every `laidout` identifier.
11. **Add Growable and strict ANSI writing.** Add retained writer storage, reserve/warm/capacity/release APIs, `doc_with`, `doc_prepared`, `doc_strict`, scalar ANSI run coalescing, byte encoder, terminal-newline handling, typed errors, and one-handoff destination integration. Check: Fast/Exact ANSI fixtures, warm-without-write, capacity/growth failures, high-water release and regrowth, real progress suspension, short-writing sinks, and reuse tests pass.
12. **Enforce zero allocation.** Add isolated counting-allocator test binaries and fixed sinks; measure each fixed boundary. Check: Fast and Exact `solve_into`, `render_into`, internal styled fixed generation, and no-progress/nonalloc-sink `doc_strict` report zero calls; every undersized failure reports the expected resource with zero calls; a prewarmed Growable workspace performs no unnecessary growth; and the real active-progress path passes semantic tests without an allocator claim.
13. **Remove superseded production paths and close documentation.** Delete the old Termosaic production interpreter/event allocation path, retain its frozen test-only oracle, update all docs/examples/error fixtures and both `migration-0.2.md` files, replace `docs/consumer-rendering.md` with its supersession pointer, record the retired scan budget and its exact counterexample, run both full matrices, and execute the validated baseline/final release performance gates. Check: no competing consumer-path authority, old enum variant, fit-unit field/policy, or old document-layout call remains outside the named test oracle or migration history; Laidout's standalone docs never require Termosaic; no public Termosaic signature names Laidout; and every benchmark identity and threshold gate in section 15 passes.

## 14. Verification matrix

Every row is required. “Failure” rows are first-class acceptance, not optional defensive tests.

| ID | Contract | Valid case | Invalid/failure case | Required verifier |
|---|---|---|---|---|
| V01 | Ownership and structural identity | `Doc<String>` and `PreparedDoc<String>` clone handles without cloning payloads; deep rebuilt graphs compare/hash/debug iteratively; concat and zero-penalty identities plus conditional `Rendered<A>` traits hold. | Compile-time trait assertions reject missing `Send`/`Sync` bounds or an accidental `A: Clone` result bound; deep operations never overflow; Termosaic Debug leaks no backend name, address, or reusable internal handle. | Unit, stress, migration, and static-assertion tests. |
| V02 | Text safety and column fitting | Unicode, combining marks, zero-width graphemes, and Termosaic's distinct empty text node preserve bytes/display widths; raw ingest normalizes CR/LF and tabs structurally; nonempty Termosaic text chunks only at UTF-8 boundaries without changing bytes; every Fast decision uses prepared summaries. | Every single-line control (including tab/escape/delete) and individual-run display-width overflow returns direct Laidout `TextError`; aggregate Termosaic consumer-domain overflow becomes typed preparation failure, never a constructor panic; empty text produces no callback; generated summaries cannot diverge from the reference scan. | Unit, property, huge-input reduced-bound, and migration tests. |
| V03 | Preparation | Deep shared DAG becomes dense prepared IR with both-mode fit summaries iteratively; representable normative work bounds agree with a no-memo/no-prune enumerator and bound actual counters/storage; advisory overflow returns `None` without rejecting a usable Fast document. | Every ID/table/summary/acceptance-bound conversion and both `CostDomainExceeded` components have a first-invalid test; compact sharing cannot hide burden, span, output, or plan expansion, and reduced actual counters fail with `StatisticsOverflow` rather than saturation. | Unit/property tests with reduced test-only domains and compact-DAG fixtures. |
| V04 | Annotation identity | Nested repeated owned tokens resolve through result-scoped iterators without payload clone. | There is no free-ID public resolver; API/source test prevents adding one without provenance validation. | Unit plus public API/source tests. |
| V05 | Cost law | Private checked `(u128, u64)` cost equals the two-`BigUint` research model on accepted random documents; weak and strict translation laws hold in both arguments. | The old secondary-saturation counterexample is rejected by the proof; first overflow or burden out-of-domain document, including a compact shared DAG, fails preparation; neither component saturates. | Proptest, exhaustive small domain, compact-DAG boundary tests, and SMT harness. |
| V06 | Fast lines/groups | All Termosaic group/line fixtures outside the classified V29 behavior are byte-equal, and prepared candidate-plus-continuation summaries equal the reference scan. | Hardline-in-group never disappears; overwide run remains intact; nested groups never trigger a node scan; an unclassified old/new divergence fails. | Old/new differential, summary-oracle property, and source-contract tests. |
| V07 | Fast fill | Each fill boundary uses the next child's flat summary only and matches the local reference probe. | A continuation-sensitive counterexample proves it does not become group fitting, and a later overwide child proves the first internal boundary ends the local summary. | Focused unit and generated differential tests. |
| V08 | Indentation | Preserve and clamp policies produce their specified output. | `NonZeroU32` prevents width 0; checked nest overflow rejects or clamps only by policy. | Boundary and compile-time type tests. |
| V09 | Exact result | Text, replayed structural events, reconstructed cost, spans, and end state equal the brute oracle. | Adversarial dominated candidates and corrupted cost witnesses never alter a successful result; a mismatch returns `CostWitnessMismatch`. | Exhaustive/property differential and corrupted-fixture tests. |
| V10 | Preferred ties | Ordered choices choose earliest equal-cost derivation. | Randomized hash/arena layouts cannot change ties. | Repeated deterministic tests. |
| V11 | Memo | Completed frontiers are reusable and deterministic before and after Growable rehash. | Fixed slot/load exhaustion reports `MemoEntries`; injected Growable allocation failure preserves the old table. | Unit tests with tiny capacities and allocation-failure injection. |
| V12 | Strategy/mode matrix | Fast/Exact × Growable/Fixed return identical per-strategy text, spans, cost, ties, and semantic stats; switching strategies retains independent arenas; releasing capacity empties every logical/actual backing and ordinary Growable reuse regrows it. | One-less Fixed capacity reports the exact resource; injected Growable failure reports the attempted resource; first unrepresentable compact index reports `WorkspaceCapacityOverflow`; every reduced semantic counter reports the exact `StatisticsOverflow` before its action in both modes; none causes strategy fallback; released Fixed storage reports ordinary typed exhaustion. | Four-way table-driven differential, capacity, counter-overflow, release, and regrowth tests. |
| V13 | Plan/visit | Text/newline plus enter/exit-annotation and penalty events reconstruct the selected structural output in preorder, including empty annotations and selected penalties. | Plan, visit-stack, and annotation-depth exhaustion fail before the corresponding workspace mutation/callback; visitor failure or panic resets Laidout state, and Termosaic discards its private buffered prefix. | Unit, event-golden, panic, and reuse tests. |
| V14 | Materialization and cost witness | Text/spans/resolution equal the owned oracle and independent event replay equals stored solver cost. | Output-byte and span exhaustion report separately; corrupted plan/cost mismatch reports `CostWitnessMismatch`; all failures roll back. | Unit, cost-reconstruction, corrupted-fixture, and reuse tests. |
| V15 | Borrowing and cleanup | Result access is valid while workspace is borrowed; visitor success/error/panic and forgotten-result recovery leave the next entry initialized. | A second render or escaped result fails to compile; a panicking visitor cannot poison reuse. | `trybuild`, `catch_unwind`, `mem::forget`, and reuse tests. |
| V16 | ANSI grammar/coalescing | The exhaustive style matrix and fragmented same-token text equal the exactly pinned `console 0.16.4` oracle bytes. | Oracle version/graph mismatch, token changes, or newlines fail closed; styles flush exactly once, bright styles close twice, and no newline is styled. | Versioned golden fixture, dependency check, and direct-oracle tests. |
| V17 | Termosaic newline | Existing consecutive/trailing newline indentation and final LF remain exact. | Capacity failure writes zero bytes. | Contract fixtures plus recording sink. |
| V18 | Writer reuse and warming | `warm_rendering` writes nothing, returns monotone capacity, and makes the same Fast/Exact `doc_strict` succeed; releasing rendering capacity empties Laidout and ANSI backing, preserves destination/theme, and allows later Growable reuse. | One-less ANSI capacity reports `AnsiBytes`; Growable ANSI allocation failure is distinct; both reset scalar run state; strict use after release exhausts rather than growing. | Table-driven warm/freeze, high-water release/regrowth, failure-injection, and reuse tests. |
| V19 | Buffered destination handoff | One document causes one logical `write_human`, at most one progress suspension, and one payload lock scope; a short-writing sink still receives complete ordered bytes. Active progress may take separate clear/redraw locks. | Any pre-write error causes zero payload writes/suspensions; an injected write error may expose a partial external write but does not poison reuse. | Instrumented short-write, failing-write, payload-lock, and real-progress tests. |
| V20 | Owned and prepared wrappers | Laidout `render`, Termosaic `doc`, `doc_with`, and `doc_prepared` preserve strategy-specific output, including the V29 correction; prepared Growable reuse exposes both strategies without re-preparation. | Growth never silently switches Fast/Exact, restarts a solve, writes partial output, hides a non-capacity error, or reintroduces the old scan-budget result. | Integration tests. |
| V21 | Feature/package/target surface | Laidout default excludes `num-bigint` and research-only engines; research includes two-`BigUint` APIs plus arbitrary-model brute/greedy and sealed frontier; doctests pass in both feature sets; native 64-bit and `i686-unknown-linux-gnu` checks cover the kernel and Termosaic mapping on supported pointer widths; both lockfile-free packages verify and Termosaic packaging resolves Laidout through the sibling patch. | An unsealed model entering frontier, a missing retained research API, missing target/tool, wrong sibling metadata, registry resolution, unsupported pointer width, stale lockfile assumption, or unsupported feature combination blocks instead of skipping a gate. | Compile-pass/fail API tests, doctests, cross-target checks in both repositories, `cargo tree`/metadata, feature matrix, and package commands. |
| V22 | Termosaic public source contract | New Termosaic-owned types, variants, and constructors are represented while no public signature names Laidout or another private backend. | Missing/extra public error fixture entries or a public `laidout` identifier fail closed. | Termosaic `source_contract`. |
| V23 | Zero allocation | Adequately reserved Fixed Fast/Exact solve/render and internal ANSI generation record zero allocation/reallocation/deallocation calls; complete no-progress/nonalloc-sink `doc_strict` also records zero; prewarmed Growable calls do not grow unnecessarily. | Every undersized Fixed call also allocates zero and returns its typed capacity error; active progress has no zero-allocation claim. | Isolated serial allocation binaries plus separate real-progress semantics. |
| V24 | Deep input and destruction | Deep sequences/annotations prepare, solve, visit, drop borrowed results, and finally destroy unique/shared source graphs without call-stack growth; concurrent cloned-root drops destroy every node and annotation exactly once. | Capacity exhaustion is typed, never stack overflow or panic; no library-owned source edge is destroyed recursively. | `source_drop` plus prepared-kernel stress tests with drop counters and small-stack threads. |
| V25 | Output-class isolation | Human documents use the instance's width, theme, stream, and destination while machine text and opaque bytes preserve their existing behavior. | Machine/opaque writers expose no document method; source and compile-fail tests reject cross-class routing or process-global workspace/policy state. | Contract, source-contract, and compile-fail tests. |
| V26 | Closed Laidout failures | Every public kernel error variant has one direct structured fixture and deterministic trigger, including every `SolveCounter` through reduced limits. | Missing/extra fixture entries, allocator-failure ambiguity, counter saturation, or a catch-all error variant fails closed. | Laidout source-contract fixture, reduced bounds/counters, and scoped allocator injection. |
| V27 | Speed and retained memory | Prepared steady-state Fast/Exact meet section 15; deep and nested zero-width cases scale within the ratios; one `fit_check` occurs per decision; dominance-heavy cases commit plan nodes only for completed-frontier survivors. | Harness/baseline-adapter/environment/dependency mismatch, threshold regression, render-time fit traversal, superlinear scaling, plan-node publication for pruned scratch candidates, or unreported retained-capacity increase fails acceptance. | Schema-versioned baseline/final artifacts, immutable harness and role-adapter validation, dominance-heavy plan-count test, and raw samples. |
| V28 | Standalone Laidout | An external consumer with a custom annotation enum uses owned render, borrowed materialization, visitation, Fast/Exact, and Growable/Fixed with `default = []`. | Any Laidout dependency, feature, source item, example, or test import that requires or conditionally names Termosaic fails the boundary scan; undersized Fixed use returns Laidout's own typed error. | `standalone_consumer`, `standalone` example, manifest/source scan, and `cargo tree`. |
| V29 | Retired Fast scan budget | The canonical width-two U+200B document produces exact new bytes `"\u{200b}\u{200b}\u{200b}\u{200b} x\n"`; all workspace modes agree when sufficient, generated classified cases fit by display columns, and nested cases use prepared summaries with linear decision counts. | The frozen oracle proves the canonical old bytes were `"\u{200b}\u{200b}\u{200b}\u{200b}\nx\n"`; every old/new mismatch must have `SyntheticBudgetExhausted` as its first differing decision and pass column-only replay; no public option, compatibility field, resource error, alternate path, or runtime cutoff can restore the artifact. | `retired-fast-scan-budget.toml`, traced generated differential tests, summary/reference-scan properties, source scan, migration docs, and zero-width benchmark. |

### 14.1 Allocation harness rules

Allocation assertions live in dedicated integration-test binaries so parallel tests and test-harness activity cannot contaminate counters. Each measured region is serial and uses a thread-local or explicitly scoped counting allocator guard.

Preparation, workspace construction/reservation, writer construction/reservation, theme setup, destination setup, and expected-result construction happen before the guard begins. Assertion formatting happens after it ends.

Termosaic uses a preallocated recording sink whose `write` method cannot grow. One measured boundary ends immediately before destination handoff and covers Laidout solving, plan visiting, ANSI generation, terminal-newline handling, and rollback cleanup. A second boundary covers the complete `doc_strict` call with no active progress handle and the same sink. No fake progress adapter appears in allocator evidence. The actual `indicatif` suspension path is covered by semantic/locking tests outside the zero-call assertion.

The harness records allocation, reallocation, and deallocation calls. Acceptance requires zero for all three, not merely zero net bytes.

## 15. Required commands and evidence

Before implementation step 2 changes production source, run the baseline capture commands below. The immutable harnesses and frozen baseline adapters verify the pinned source identities from step 1, write atomically, and refuse to overwrite an existing artifact whose digest differs:

```sh
mkdir -p target/plan-evidence/prepared-kernel
cargo bench --bench prepared_kernel --features research -- \
  capture --role baseline --implementation laidout-0.1-baseline \
  --output target/plan-evidence/prepared-kernel/laidout-baseline.json
```

From `/Users/bryanmatteson/Code/termosaic`:

```sh
mkdir -p target/plan-evidence/prepared-kernel
cargo bench --bench human_doc --all-features -- \
  capture --role baseline --implementation termosaic-0.1-baseline \
  --output target/plan-evidence/prepared-kernel/termosaic-baseline.json
```

The baseline files are acceptance inputs. Deleting them, changing a measurement harness, workload manifest, or frozen baseline adapter after capture, or failing an identity check stops implementation until both baselines are recaptured from the pinned production sources. A final prepared adapter is role-specific evidence and may be added after the corresponding API exists; changing it invalidates and recaptures only that final artifact.

Run from `/Users/bryanmatteson/Code/laidout`:

```sh
cargo fmt --all -- --check
cargo test --all-targets
cargo test --all-targets --features research
cargo test --doc --no-default-features
cargo test --doc --features research
cargo test --test prepared_mode_matrix
cargo test --test fast_summary
cargo test --test source_drop
cargo test --test fixed_allocation -- --test-threads=1
cargo test --test kernel_source_contract
cargo test --no-default-features --test standalone_consumer
cargo run --no-default-features --example standalone
cargo test --features research --test exact_oracle
cargo clippy --all-targets --all-features -- -D warnings
cargo check --target i686-unknown-linux-gnu --no-default-features --lib
cargo check --target i686-unknown-linux-gnu --features research --lib
cargo tree -e normal
cargo tree -e normal --features research
cargo tree -e all --no-default-features
cargo tree -e all --all-features
cargo package --allow-dirty
sh proofs/check-cost-model-laws.sh
cargo bench --bench prepared_kernel --features research -- \
  capture --role final --implementation prepared-kernel \
  --output target/plan-evidence/prepared-kernel/laidout-final.json
cargo bench --bench prepared_kernel --features research -- \
  compare \
  --baseline target/plan-evidence/prepared-kernel/laidout-baseline.json \
  --candidate target/plan-evidence/prepared-kernel/laidout-final.json \
  --dependency-transition docs/benchmark-dependency-transition-0.2.toml
if rg -ni --glob '!prepared-layout-kernel-plan.md' '\btermosaic\b' \
  Cargo.toml src examples tests benches README.md docs; then exit 1; fi
if rg -n 'FastFitPolicy|fast_fit_units|FastFitUnits|FastFitItems|fast_fit_items|fit_items|scan_units' \
  /Users/bryanmatteson/Code/laidout/src \
  /Users/bryanmatteson/Code/termosaic/src; then exit 1; fi
```

Run from `/Users/bryanmatteson/Code/termosaic`:

```sh
./scripts/verify.sh
cargo test --doc --all-features
cargo test --all-features --test layout_compat
cargo test --all-features --test fixed_allocation -- --test-threads=1
cargo check --target i686-unknown-linux-gnu --all-features --lib
cargo tree -p termosaic -i console@0.16.4 --all-features
cargo bench --bench human_doc --all-features -- \
  capture --role final --implementation prepared-kernel \
  --output target/plan-evidence/prepared-kernel/termosaic-final.json
cargo bench --bench human_doc --all-features -- \
  compare \
  --baseline target/plan-evidence/prepared-kernel/termosaic-baseline.json \
  --candidate target/plan-evidence/prepared-kernel/termosaic-final.json \
  --dependency-transition docs/benchmark-dependency-transition-0.2.toml
```

The named test binaries, workloads, and benchmarks above are required artifact names, not placeholders to choose during implementation. The `i686-unknown-linux-gnu` standard library is a verifier prerequisite installed with `rustup target add i686-unknown-linux-gnu`; if the target or Rustup is unavailable, V21 is blocked rather than skipped. The 32-bit checks plus the native 64-bit suite exercise both supported pointer-width proof classes, while `kernel_source_contract` requires the explicit compile error for every other pointer width. The Laidout scan intentionally covers its manifest, library, examples, tests, benchmarks, README, and ordinary docs but excludes this joint integration plan; any other match is a dependency-direction failure, including a renamed dependency or conditional probe. Add `trybuild` as a Laidout and Termosaic dev-dependency for the compile-fail rows. The narrow commands do not replace the full commands above or Termosaic's repository-owned verifier.

Both benchmarks use checked-in `harness = false` launchers with no network service or unstable compiler feature. `harness_v1.rs` owns clocks, allocator accounting, sampling, schema writing, and workload dispatch and is byte-identical between roles within one repository. Each role adapter implements the same private benchmark trait and owns only document construction plus the measured baseline/prepared call. The launcher contains only argument parsing and role-adapter registration; a source-contract test rejects clocks, allocator counters, sampling loops, or workload logic outside `harness_v1.rs`. The frozen baseline adapter, harness, and workload are captured before production migration; the final adapter and its registration are added after its API exists. Baseline and final cases run in separate processes under the same Cargo feature set. Final prepared cases drop the source `Doc` after preparation and warm the workspace before steady-state sampling. Each JSON artifact has `schema_version = 1` and records role, repository, implementation name, pinned baseline identity, production source digest over the governed source/manifest paths, plan digest, immutable-harness digest, role-adapter digest, baseline-adapter digest, workload-manifest digest, dependency-graph digest, `rustc -vV`, target triple, operating-system version, CPU brand/core count, feature set, allocator mode, case parameters, 30 raw independent samples, median and p95 nanoseconds, allocation traffic, steady retained live bytes before each sample, peak live delta, and peak total live bytes. Laidout additionally records prepared logical bytes by table, workspace logical/actual retained bytes by resource, and their combined retained total; Termosaic records its public retained human-workspace capacities while allocator live bytes cover the private prepared wrapper.

`compare` rejects schema, immutable-harness, baseline-adapter, workload, toolchain, target, operating-system, CPU, feature-set, allocator, or case-set mismatches before evaluating a threshold. It requires `role = baseline` to name the frozen adapter and `role = final` to name the separately hashed prepared adapter; an adapter digest is never treated as though the two implementations should be identical. It recomputes the current frozen baseline-adapter digest and rejects a stale baseline artifact, while a changed final-adapter digest requires only a new candidate capture. It parses `cargo metadata --locked` only when a lockfile is tracked; for these lockfile-free libraries it hashes ordinary `cargo metadata` plus the complete resolved package graph. The dependency-transition manifest permits only the direct manifest changes required by section 12 and transitives reachable solely through those added roots; every shared package must retain the same resolved version, including `console 0.16.4`. An unclassified graph difference invalidates the comparison. A comparison is valid when sample coefficient of variation is at most 5%; otherwise the harness repeats up to three 30-sample groups and uses the lowest-CV group. The final handoff preserves all four raw artifacts and their SHA-256 digests; summary prose is not a substitute for them.

The release performance gates are:

1. Prepared steady-state Termosaic Fast solve plus ANSI generation on the existing representative document corpus has median time no greater than 1.15 times the frozen production interpreter baseline and peak total live bytes no greater than 1.10 times baseline.
2. Prepared steady-state Laidout Exact on the existing JSON corpus at widths 20 and 40 has median time no greater than 1.15 times the current frontier baseline and peak total live bytes no greater than 1.10 times baseline.
3. For unique 2,000- and 4,000-leaf sequences, the 4,000/2,000 ratio is at most 2.5 for preparation time, fixed solve time, and combined prepared-plus-workspace retained logical bytes. No measured phase may return to the old quadratic structural-hashing curve.
4. For the checked-in 64- and 128-state nondominated-frontier documents, the 128/64 median-time ratio is at most 3.0 and retained logical bytes at most 2.25.
5. For checked-in nested zero-width Fast documents with exactly 2,000 and 4,000 semantic fit decisions, `fit_checks` is exactly 2,000 and 4,000, the 4,000/2,000 ratio is at most 2.5 for median solve time and at most 2.25 for combined prepared-plus-workspace retained logical bytes, and both cases retain their expected column-correct flat output. The test-only reference scanner must examine more than linearly many projected items on the same fixtures, proving that the gate would detect accidental reintroduction of scanning rather than merely timing two easy inputs.
6. Fixed allocation counts and semantic equality remain hard gates even if a timing gate passes. A noisy benchmark that cannot meet the sampling rule is reported as a V27 blocker, not silently accepted.

Also run:

```sh
rg -n 'Doc::(Line|SoftLine|HardLine)|layout\(doc|Vec<LayoutEvent|Rc<Doc>|Rc<Out>' \
  --glob '*.rs' \
  /Users/bryanmatteson/Code/laidout \
  /Users/bryanmatteson/Code/termosaic
```

Any remaining match must be in an explicitly documented research API, migration oracle, or historical prose. Production consumer/document-writing matches fail acceptance.

## 16. Stop conditions

Stop and return a blocker report instead of weakening the contract when any of these occurs:

- Fast output outside the exact V29 first-divergence classification cannot be made byte-equal without changing another Termosaic behavior the user has not authorized changing.
- Any production path requires synthetic fit units, a semantic scan counter, a render-time fit traversal, or a public compatibility option to complete the migration.
- The prepared summary algebra or cached continuation aggregate disagrees with the independent iterative reference scan for any generated Group, Fill, Choice, mode, candidate/continuation boundary, or `HardLine` case.
- The optimized preparation evaluator disagrees with the section 8.1 reference recurrence, a `Some` advisory bound is exceeded, or an accepted document exceeds any representation/output/cost bound during solving, visiting, or materialization.
- Exact pruning requires a law not established for the private prepared cost, including strict translation preservation in either argument.
- A proposed accepted domain reaches unchecked overflow or saturation in either cost component for any winning, losing, or shared-DAG candidate.
- Any semantic statistic or source-precedence value wraps, saturates, changes a layout decision, or mutates solver state before returning its typed `StatisticsOverflow`.
- A selected plan cannot replay penalties, nested annotations, zero-width annotations, rendered bytes, and exact cost independently of solver state.
- Exact dominance requires publishing a persistent plan node before the candidate survives frontier finalization.
- A dependency or public API migration cannot be published coherently across the two crates.
- Laidout's complete default kernel, examples, or verification require a Termosaic dependency, type, feature, build-time probe, or compatibility default.
- A measured allocator call comes from a required in-boundary operation and no bounded replacement has been specified.
- Workspace rollback cannot restore deterministic reuse after an error.
- Final destruction of a library-owned source edge recurses, leaks, double-drops, or cannot tolerate concurrent final drops of cloned roots.
- A Fast or Exact result changes solely because the workspace is Growable rather than Fixed.
- Growable resource expansion requires restarting, changing strategy, or invalidating a previously published memo range.
- Termosaic cannot map the complete Laidout error/resource vocabulary without exposing a Laidout type, adding a catch-all variant, or losing one of the distinctions in section 11.2.
- The ANSI oracle does not resolve exactly to `console 0.16.4`, or a direct-encoder byte differs from the versioned complete style matrix without an authorized byte-contract change.
- A baseline/final benchmark pair has a mismatched schema, immutable harness, frozen baseline adapter, workload, environment, feature set, case set, or unclassified dependency graph, or names the wrong role adapter.
- A section 15 performance or retained-memory threshold fails after a valid low-noise comparison.

The blocker report names the failed verification ID, minimal reproducer, affected public contract, and the smallest decision needed. It must not mark the item complete, substitute a different strategy, permit hidden growth, saturate either cost component, discard structural witness events, loosen a preparation bound, recapture against a changed baseline, or move work outside the measured boundary merely to satisfy the counter.

## 17. Completion evidence

The final implementation handoff includes:

- the governed-surface manifest with every row marked changed or intentionally unchanged;
- proof and differential test results;
- proof that every representable section 8.1 bound dominates production counters/storage, advisory overflow becomes `None` without preparation rejection, and every reduced actual counter returns first-invalid `StatisticsOverflow` before its semantic action;
- structural event golden results and independent selected-plan cost reconstruction, including penalties and empty annotations;
- dominance-heavy evidence that pruned scratch candidates append no plan nodes;
- exact allocator-call counts for fixed Fast/Exact `solve_into`, `render_into`, successful `doc_strict`, each fixed undersized failure family, and prewarmed Growable calls;
- all four schema-versioned benchmark artifacts, their SHA-256 digests, validated source/environment/dependency identities, and before/after peak retained workspace and representative throughput measurements for all four strategy/mode combinations;
- all command results from section 15;
- a list of public breaking changes and migration examples;
- confirmation that `docs/consumer-rendering.md` no longer competes as a second live consumer-path authority;
- confirmation that preparation and reservation were outside every measured boundary;
- confirmation that Growable expansion touched only the named resource and that warm-and-freeze capacity snapshots reproduced the warmed renders;
- confirmation that workspace/writer capacity release returned logical and actual retained storage to empty, then allowed ordinary regrowth and reuse;
- deep unique, branching, shared-DAG, and concurrent cloned-root destruction results showing no call-stack growth and exactly-once payload destruction;
- standalone Laidout example/test output for a custom annotation type in all four strategy/mode combinations, plus the empty dependency/source scan;
- the `fast_summary` generated-decision corpus result, exact decision counts for nested stress documents, and confirmation that Fast has no fit traversal resource or fallback path;
- exact canonical V29 old/new bytes, generated first-divergence classification results, an empty production fit-unit/policy/fit-stack scan, and migration documentation naming the authorized behavioral correction;
- confirmation that Termosaic performed one logical document handoff, at most one real progress suspension, and one payload sink-lock scope, with separate accounting for progress clear/redraw locks and short-write behavior.

The pre-change and final benchmark artifacts must demonstrate every section 15 threshold. A regression is a failed acceptance gate, not merely a note in the handoff. Zero allocator calls and semantic equality remain independently mandatory.

## 18. Risks

These are explicit operating tradeoffs, not permissions to weaken correctness or allocation claims:

| Risk | Consequence | Required control | Residual tradeoff |
|---|---|---|---|
| Fixed capacity is insufficient | The render returns the named `WorkspaceExhausted` before allocation and Termosaic writes no document bytes. | Reserve directly or warm the exact prepared document/width/strategy, freeze the observed capacity, test every one-less resource, and treat exhaustion as an ordinary retry or request failure outside the strict boundary. | Finite memory cannot guarantee success for every exact frontier; zero allocation and unbounded automatic growth are mutually exclusive. |
| Growable storage expands or retains a high-water mark | A cold or exceptional render can add allocation latency, fail with typed growth error, and keep resident memory until released. | Grow only the named resource without restart, expose logical/actual footprint telemetry, reuse workspaces by workload class, and call `release_capacity` or `release_rendering_capacity` after an exceptional peak. Use Fixed for bounded latency. | Releasing capacity forfeits warm state and the next Growable use may allocate again. |
| Consumer preparation rejects an out-of-domain compact graph | Preparation fails before solving even when the ultimately winning layout would be small, reducing accepted input expressiveness but never returning a wrong layout. | Bound every winning and losing layout's representation/output/cost with the normative checked recurrence; report the exceeded prepared resource or cost component; test compact shared-DAG overflow/burden boundaries; and keep two-component unbounded `BigUint` behavior under `research`. | The consumer kernel deliberately accepts only documents whose complete candidate representation and cost domain is auditable without saturation; exhaustive work count alone is not a rejection reason. |
| Actual semantic work exceeds a `u64` counter | An astronomically expanded zero-byte/shared document can prepare successfully but rendering stops with the named `StatisticsOverflow`; no approximate or partial layout is returned. | Keep exhaustive work metadata advisory, check every actual counter/precedence update before its action, expose the exact counter through both crates, use reduced-counter tests for every path, and apply ordinary workspace/time quotas to untrusted Exact workloads. | Accepting useful Fast documents with many choices is more important than rejecting them from an unused exhaustive-count estimate; a truly greater-than-`u64` execution cannot return complete `SolveStats`. |
| A borrowed result holds the workspace exclusively | The same workspace cannot render concurrently or be reused while `LayoutRef`/`RenderedRef` is alive. | Visit or consume the result promptly, use the owned wrapper when it must outlive the operation, and provision one caller-owned workspace per concurrent lane when parallelism is required. | Borrowing is the mechanism that prevents stale arena references and makes zero-copy output safe. |
| A visitor performs external side effects before failing | Laidout can reset its workspace but cannot undo writes, counters, or other effects already performed by arbitrary callback code. | Document visitor callbacks as prefix-observable on failure, require atomic consumers to buffer until success, and verify Termosaic writes only after a successful visit and clears its private buffer on error/panic. | General visitors control their own side effects; only Laidout state and the Termosaic destination contract are transactional. |
| A caller repeatedly creates fresh `HumanWriter`s | Each later document operation loses prepared scratch reuse and may prepare/grow again, increasing latency and allocation traffic. | Make a retained `HumanWriter` the documented steady-state path, benchmark both fresh and retained use, and use its release method when retained memory must be shed. | The short-lived `Console::human()` path remains an allocating ergonomic convenience rather than a strict-performance API. |
| Opaque/canonical `Doc` migration breaks source compatibility | Downstream enum construction or `Rc<Doc>` code fails to compile; equality/Debug observations of wrapper-only concat nodes change; mismatched releases can produce incompatible document types. | Ship coordinated `0.2.0` releases in dependency order, retain deprecated Laidout free constructors, specify canonical equality/custom Termosaic Debug, provide compile and behavioral migration examples, validate packaged resolution, and update fixtures/docs atomically. | This is an intentional pre-1.0 source-semantic break; preserving enum-tree identity would conflict with the canonical shared kernel. |
| Infallible Termosaic text exceeds the prepared domain | Construction still succeeds and preserves bytes, but preparation/writing can now return `HumanPrepareError` for an aggregate line whose columns, output, or cost cannot fit the checked consumer domain. | Chunk sanitized text only at UTF-8 boundaries, never insert a break, test the boundary with reduced limits, document the typed failure, and offer Laidout `research` only to callers that truly need unbounded exact costs. | A compact zero-allocation consumer kernel cannot accept every materializable 64-bit string while proving exact bounded arithmetic. |
| V29 removes the synthetic zero-width fit budget | A narrow class of human output changes line breaks, so golden output can change even though display-column fitting becomes correct. | Pin canonical old/new bytes, require the generated first-divergence classifier and column-only replay, document the change in both migrations, and provide no compatibility switch. Callers needing a break express it structurally. | The old bytes are not preserved because doing so would reintroduce the bug and a second Fast semantics. |
| Exact frontiers have large width-dependent state | Fixed mode can exhaust; Growable mode can consume substantial time and retained memory or encounter allocation failure. | Keep Termosaic Fast as the default, allocate no Exact arenas on Fast-only use, stream/prune frontiers, expose per-resource footprint/capacity, set Fixed quotas for untrusted workloads, and release exceptional high-water storage. | Exactness can require a large nondominated frontier; no candidate cap may be reported as an exact result. |
| Structural witness nodes add retained plan state | Selected penalties and empty annotations now remain visible, and every completed memo frontier retains the plans needed for reuse. | Store pending plan expressions inline in scratch, commit only completed-frontier survivors, reuse existing child IDs, include structural nodes in preparation/capacity bounds, and enforce dominance-heavy plan-count gates. | Lossless replay has a real per-survivor memory cost; omitting those nodes would make output and cost unverifiable. |
| Benchmark or dependency drift creates a false comparison | Performance gates can pass or fail because the harness, baseline adapter, workload, toolchain, CPU, oracle, or resolved packages changed rather than because the engine improved. | Freeze the measurement harness, workload, and self-contained baseline adapter first; keep final code in a separately hashed adapter; pin source identities and `console 0.16.4`; record complete raw metadata/graphs; permit only the checked-in dependency transition; reject mismatches before thresholds; and preserve artifact digests. | A changed harness, baseline adapter, workload, or environment requires a fresh valid baseline; changing only final implementation code requires a fresh candidate. Cross-machine timing numbers are evidence only after identity validation. |
| Active progress is outside the allocator proof | `doc_strict` with a live progress bar may allocate or add backend-controlled latency even when layout and ANSI generation do not. | State the measured boundary precisely, require no active progress for the end-to-end zero-call guarantee, and exercise the real `indicatif` path separately for ordering, locking, suspension count, short writes, and recovery. | Closing this last allocator boundary would require owning or replacing the progress implementation; the plan does not mislabel an external backend as allocation-free. |
| Destruction needs a branching worklist | Final source destruction may allocate/deallocate outside rendering in proportion to pending siblings, and a consumer-defined annotation destructor can have its own behavior. | Use the `Option<Arc<Node<A>>>` take slot, `Arc::into_inner`, a current-node loop, and only a sibling `Vec`; verify unique/shared/concurrent graphs on small-stack threads with exactly-once drop counters. Deep unary destruction grows no worklist. | Library-owned edges are stack-safe; allocator behavior during source destruction and arbitrary `A::drop` are explicitly outside the render-time zero-allocation guarantee. |
