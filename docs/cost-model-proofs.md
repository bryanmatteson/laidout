# Prepared consumer cost proof

The default kernel uses a private checked cost `(u128 overflow, u64 burden)`.
The `research` feature separately retains mathematically unbounded two-`BigUint`
models. The machine-checked counterexample queries are in
[`proofs/cost_model_laws.smt2`](../proofs/cost_model_laws.smt2):

```sh
sh proofs/check-cost-model-laws.sh
```

All thirteen queries must return `unsat`; `sat`, `unknown`, or a missing query is
a failed proof gate.

## Accepted prepared domain

Preparation accepts a consumer document only after conservative expansion of
every reachable derivation, including losing choices and repeated occurrences
of shared DAG nodes. It establishes:

- every candidate line-ending column fits `u32`;
- every candidate output length fits `usize` on the supported 32- or 64-bit
  target;
- the sum of squared-overflow contributions fits `u128`;
- every newline-cost and explicit-penalty sum fits `u64`.

An unrepresentable acceptance bound returns `PrepareError` before solving.
Advisory work-count metadata may instead be `None`; it is not used to justify
cost arithmetic. Every runtime addition, column addition, multiplication, and
subtraction remains checked and returns `CostDomainViolation` if the prepared
invariant is ever broken.

For a `u32` column and width, the overflow potential is based on
`q(c) = max(0, c - page_width)`. Thus `q(c) <= 2^32 - 1`, and one line contributes
at most

```text
(2^32 - 1)^2 = 2^64 - 2^33 + 1 < 2^64.
```

On a 64-bit target a materializable string has fewer than `2^64` bytes and no
more than `2^64` lines, so total overflow is strictly below `2^128`. The 32-bit
case is smaller. Preparation calculates the document-specific bound with
checked recurrence rather than relying only on this target-wide ceiling.
Other pointer widths fail compilation explicitly.

## Exact pair algebra

Within the accepted domain, addition is ordinary natural-number addition:

```text
(o1, b1) + (o2, b2) = (o1 + o2, b1 + b2).
```

Zero is a two-sided identity and addition is associative. Lexicographic order
is preserved weakly and strictly in both arguments. If the dominant components
differ, adding the same value preserves that strict difference. If they are
equal, ordinary addition preserves the weak or strict order of the burden
components. Commutativity gives the symmetric argument.

Strict preservation is necessary for preferred-tie pruning. Saturation is
therefore forbidden. For `a = (0, 1)`, `b = (0, 0)`, and
`c = (0, u64::MAX)`, saturation turns the initially strict `b < a` into an
equal result after extension, which can change the preferred derivation.

## Text, newline, and penalty laws

Let `P(c) = q(c)^2` and `T(c, w) = P(c + w) - P(c)`. Text splitting telescopes:

```text
T(c, a) + T(c + a, b) = T(c, a + b).
```

The first difference of `P` is zero before overflow and
`2(c - page_width) + 1` afterward. It is nondecreasing, so moving a run right
cannot reduce its cost. Text costs are nonnegative. Newline burden and explicit
penalty burden are nonnegative and depend only on their declared scalar, never
on column or text fragmentation.

## Rust correspondence and extension boundary

The SMT file proves the integer laws, strict translations, the `u32` square
bound, checked `u32` addition correspondence, the high-bit bound for the
zero-extended square, and strict lexicographic preservation for checked
`u128`/`u64` bit-vector addition. These obligations match the operations in
`src/cost.rs`.

`ConsumerCost` exposes read-only components but no constructor or addition.
The prepared model is crate-private and does not implement the public research
`CostModel` trait, so callers cannot request an unrepresentable infallible
operation. With `research`, both components use `BigUint`; arbitrary models
remain usable by exhaustive and greedy engines, while the pruning frontier is
closed by the sealed `LawfulCostModel` marker.
