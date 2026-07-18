//! Differential properties: the frontier engine against the brute-force
//! oracle and the greedy baseline, over random documents.

use std::rc::Rc;

use proptest::prelude::*;

use laidout::cost::OverflowThenHeight;
use laidout::doc::{align, concat2, count_choices, group, hardline, line, nest, tag, text, Doc};
use laidout::measure::Measurement;
use laidout::render::{cost_of_lines, to_lines};
use laidout::{brute, frontier, greedy};

const MAX_CHOICES: usize = 8;

/// Content with all whitespace removed. Invariant across every layout of a
/// document: breaks and their flat projections only differ in whitespace.
fn stripped(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

fn arb_measurement() -> impl Strategy<Value = Measurement> {
    prop::collection::vec(0u32..40, 1..8).prop_map(|widths| Measurement::from_line_widths(&widths))
}

fn arb_doc() -> impl Strategy<Value = Rc<Doc>> {
    let leaf = prop_oneof!["[a-z]{1,6}".prop_map(text), Just(line()), Just(hardline()),];
    leaf.prop_recursive(4, 24, 3, |inner| {
        prop_oneof![
            (inner.clone(), inner.clone()).prop_map(|(a, b)| concat2(a, b)),
            (1u16..4, inner.clone()).prop_map(|(n, d)| nest(n, d)),
            inner.clone().prop_map(align),
            inner.clone().prop_map(group),
            (0u32..3, inner).prop_map(|(t, d)| tag(t, d)),
        ]
    })
}

#[test]
fn unicode_terminal_width_is_consistent_across_all_engines() {
    let doc = group(concat2(text("界"), concat2(line(), text("e\u{301}"))));
    let cm = OverflowThenHeight { width: 2 };

    let oracle = brute::best(&cm, &doc, count_choices(&doc));
    let greedy = greedy::layout(&cm, &doc, 2);
    let best = frontier::best(&cm, &doc);
    let best_lines = to_lines(&best.out);

    assert_eq!(oracle.text(), "界\ne\u{301}");
    assert_eq!(greedy.lines, best_lines);
    assert_eq!(laidout::to_string(&best.out), oracle.text());
    assert_eq!(greedy.cost, oracle.cost);
    assert_eq!(best.cost, oracle.cost);
    assert_eq!(cost_of_lines(&cm, &best_lines), best.cost);
}

proptest! {
    /// The frontier engine is optimal: it matches exhaustive enumeration.
    #[test]
    fn frontier_matches_brute(doc in arb_doc(), width in 2u32..30) {
        prop_assume!(count_choices(&doc) <= MAX_CHOICES);
        let cm = OverflowThenHeight { width };
        let oracle = brute::best(&cm, &doc, MAX_CHOICES);
        let best = frontier::best(&cm, &doc);
        prop_assert_eq!(&best.cost, &oracle.cost);
    }

    /// The frontier engine never does worse than the greedy baseline.
    #[test]
    fn frontier_at_most_greedy(doc in arb_doc(), width in 2u32..30) {
        prop_assume!(count_choices(&doc) <= MAX_CHOICES);
        let cm = OverflowThenHeight { width };
        let g = greedy::layout(&cm, &doc, width);
        let best = frontier::best(&cm, &doc);
        prop_assert!(best.cost <= g.cost, "frontier {:?} > greedy {:?}", best.cost, g.cost);
    }

    /// Every engine renders the same content modulo whitespace.
    #[test]
    fn content_is_layout_invariant(doc in arb_doc(), width in 2u32..30) {
        prop_assume!(count_choices(&doc) <= MAX_CHOICES);
        let cm = OverflowThenHeight { width };
        let oracle = brute::best(&cm, &doc, MAX_CHOICES);
        let g = greedy::layout(&cm, &doc, width);
        let best = frontier::best(&cm, &doc);
        let expected = stripped(&oracle.text());
        prop_assert_eq!(stripped(&g.lines.join("\n")), expected.clone());
        prop_assert_eq!(stripped(&laidout::to_string(&best.out)), expected);
    }

    /// A frontier candidate's accumulated cost agrees with the cost of its
    /// rendered lines (the incrementality contract, end to end).
    #[test]
    fn frontier_cost_is_self_consistent(doc in arb_doc(), width in 2u32..30) {
        prop_assume!(count_choices(&doc) <= MAX_CHOICES);
        let cm = OverflowThenHeight { width };
        let best = frontier::best(&cm, &doc);
        let lines = to_lines(&best.out);
        prop_assert_eq!(cost_of_lines(&cm, &lines), best.cost);
    }

    /// Same document, same model: same bytes, run to run.
    #[test]
    fn frontier_is_deterministic(doc in arb_doc(), width in 2u32..30) {
        prop_assume!(count_choices(&doc) <= MAX_CHOICES);
        let cm = OverflowThenHeight { width };
        let a = frontier::best(&cm, &doc);
        let b = frontier::best(&cm, &doc);
        prop_assert_eq!(laidout::to_string(&a.out), laidout::to_string(&b.out));
        prop_assert_eq!(a.cost, b.cost);
    }


    /// Bottom-up tree reassociation cannot change a fragment's measure.
    #[test]
    fn measurement_append_is_associative(
        a in arb_measurement(),
        b in arb_measurement(),
        c in arb_measurement(),
    ) {
        prop_assert_eq!(a.append(b).append(c), a.append(b.append(c)));
    }
}
