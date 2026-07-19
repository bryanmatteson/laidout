# Cost-model proofs

This document proves the laws required by the exact Pareto-frontier solver for
the two built-in cost models. The corresponding SMT obligations are in
[`proofs/cost_model_laws.smt2`](../proofs/cost_model_laws.smt2) and are checked
with:

```sh
sh proofs/check-cost-model-laws.sh
```

The command requires `z3` on `PATH`. Every query asks for a counterexample. All
eight results must be `unsat`.
Property tests remain useful implementation regressions, but they are not the
proof.

## Cost domain

Let `N` be the unbounded natural numbers, represented by `BigUint`. Let
`B_m = {0, ..., m}` be a bounded natural component, represented by `u32` for
line breaks or `u64` for consumer burden. Define bounded addition by

```text
x ⊞ y = min(m, x + y).
```

A cost is a pair `(o, b)` in `N × B_m`, ordered lexicographically. Addition is

```text
(o₁, b₁) ⊕ (o₂, b₂) = (o₁ + o₂, b₁ ⊞ b₂),
zero = (0, 0).
```

The dominant component must be unbounded. Component-wise saturating arithmetic
on `(u64, u64)` is not monotone under lexicographic order. For example,

```text
a = (0, u64::MAX) < b = (1, 0)
c = (u64::MAX, 0)

sat(a + c) = (u64::MAX, u64::MAX)
sat(b + c) = (u64::MAX, 0)
```

so adding `c` reverses the order. `src/cost.rs` therefore accumulates the
dominant component with exact `BigUint` addition.

## Identity and associativity

Natural-number addition has identity `0` and is associative. For the bounded
component,

```text
0 ⊞ x = x ⊞ 0 = x
(x ⊞ y) ⊞ z = min(m, x + y + z) = x ⊞ (y ⊞ z).
```

Pair equality is component-wise, so `(0, 0)` is a two-sided identity and `⊕`
is associative.

## Monotonicity of addition

Assume `(o₁, b₁) ≤ (o₂, b₂)` and add `(r, s)` to both sides.

- If `o₁ < o₂`, exact natural addition gives `o₁ + r < o₂ + r`; the secondary
  components cannot affect the lexicographic result.
- If `o₁ = o₂`, lexicographic ordering gives `b₁ ≤ b₂`. The function
  `x ↦ min(m, x + s)` is monotone, so `b₁ ⊞ s ≤ b₂ ⊞ s`.

Thus addition is monotone in one argument. Both component additions are
commutative, so the same proof establishes monotonicity in the other argument.

## Text incrementality and column monotonicity

For configured page width `W`, define the overflow potential

```text
q(c) = max(0, c - W)
P(c) = q(c)²
T(c, w) = P(c + w) - P(c).
```

Both built-in models return `(T(c, w), 0)` for a text run.

For any split `w = a + b`, terms telescope:

```text
T(c, a) + T(c + a, b)
= P(c + a) - P(c) + P(c + a + b) - P(c + a)
= P(c + a + b) - P(c)
= T(c, a + b).
```

Therefore text cost is independent of text chunking.

To prove monotonicity in the starting column, consider the first difference
`D(c) = P(c + 1) - P(c)`:

```text
D(c) = 0                  when c < W
D(c) = 2(c - W) + 1      when c ≥ W.
```

`D` is nondecreasing. Since

```text
T(c, w) = Σ D(c + i), for 0 ≤ i < w,
```

shifting `c` right cannot decrease any summand. Hence `c₁ ≤ c₂` implies
`T(c₁, w) ≤ T(c₂, w)`. The same representation also proves `T(c, w) ≥ 0`.

## Newlines and penalties

`OverflowThenHeight` maps a newline to `(0, 1)` and every penalty to `(0, 0)`.
`ConsumerCostModel` maps a newline to `(0, newline_cost)` and a penalty of
amount `p` to `(0, p)`. All values are natural, so they are nonnegative. The
penalty expressions contain only `p`, making them independent of column and
text chunking by construction.

## Correspondence to Rust arithmetic

Columns and run widths are `u32`, and every engine rejects `col + width` when
it does not fit in `u32`. Therefore `q(c) ≤ u32::MAX`. The largest square used
inside `text` is

```text
(2³² - 1)² = 2⁶⁴ - 2³³ + 1 < 2⁶⁴ - 1 = u64::MAX.
```

The local `u64` multiplication and subtraction are consequently exact before
conversion to `BigUint`. Aggregation of the dominant component is unbounded.
Secondary accumulation uses Rust's saturating addition, exactly the `⊞`
operation proved above.

The exact solver requires the sealed `LawfulCostModel` trait. Downstream
`CostModel` implementations remain available to brute-force rendering,
greedy rendering, and output-cost reconstruction, but cannot enter the pruning
engine. This makes the proof set closed: every model accepted by the exact
solver is one of the implementations proved here.
