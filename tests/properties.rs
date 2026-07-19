//! Differential properties: the frontier engine against the brute-force
//! oracle and the greedy baseline, over random documents.

use std::rc::Rc;
use std::{cmp, num::NonZeroU8};

use proptest::prelude::*;

use laidout::cost::{ConsumerCostModel, CostModel, OverflowThenHeight};
use laidout::doc::{
    align, choice, concat2, count_choices, group, hardline, line, nest, penalize, tag, text,
    try_text_with, Doc, WidthMode,
};
use laidout::measure::Measurement;
use laidout::render::{cost_of_out, to_lines};
use laidout::{brute, from_text_with, frontier, greedy, render_with, IngestOptions, SolveLimits};

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
    let raw_narrow = from_text_with(
        "a\t·\r\nb",
        IngestOptions {
            width_mode: WidthMode::Narrow,
            tab_width: NonZeroU8::new(4).unwrap(),
        },
    )
    .unwrap();
    let raw_cjk = from_text_with(
        "a\t·\r\nb",
        IngestOptions {
            width_mode: WidthMode::Cjk,
            tab_width: NonZeroU8::new(4).unwrap(),
        },
    )
    .unwrap();
    let leaf = prop_oneof![
        "[a-z]{1,6}".prop_map(text),
        Just(try_text_with("·", WidthMode::Narrow).unwrap()),
        Just(try_text_with("·", WidthMode::Cjk).unwrap()),
        Just(raw_narrow),
        Just(raw_cjk),
        Just(line()),
        Just(hardline()),
    ];
    leaf.prop_recursive(4, 24, 3, |inner| {
        prop_oneof![
            (inner.clone(), inner.clone()).prop_map(|(a, b)| concat2(a, b)),
            inner
                .clone()
                .prop_map(|doc| { choice(tag(6, concat2(doc.clone(), text(" "))), tag(7, doc),) }),
            (1u16..4, inner.clone()).prop_map(|(n, d)| nest(n, d)),
            inner.clone().prop_map(align),
            inner.clone().prop_map(group),
            (0u32..3, inner.clone()).prop_map(|(t, d)| tag(t, d)),
            (0u32..4, inner).prop_map(|(amount, d)| penalize(amount, d)),
        ]
    })
}

fn assert_generated_cost_laws<M: CostModel>(
    model: &M,
    col: u32,
    first_width: u32,
    second_width: u32,
    other_col: u32,
    penalty: u32,
) {
    let zero = model.zero();
    let text = model.text(col, first_width);
    let newline = model.newline();
    let burden = model.penalty(penalty);
    assert_eq!(model.add(&zero, &text), text);
    assert_eq!(model.add(&text, &zero), text);
    assert_eq!(
        model.add(&model.add(&text, &newline), &burden),
        model.add(&text, &model.add(&newline, &burden))
    );
    assert_eq!(
        model.text(col, first_width + second_width),
        model.add(
            &model.text(col, first_width),
            &model.text(col + first_width, second_width),
        )
    );
    let left_col = cmp::min(col, other_col);
    let right_col = cmp::max(col, other_col);
    let left = model.text(left_col, first_width);
    let right = model.text(right_col, first_width);
    assert!(left <= right);
    assert!(model.add(&left, &newline) <= model.add(&right, &newline));
    assert!(model.add(&newline, &left) <= model.add(&newline, &right));
    assert!(zero <= burden);
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
    assert_eq!(cost_of_out(&cm, &best.out), best.cost);
}

proptest! {
    /// The frontier engine is optimal: it matches exhaustive enumeration.
    #[test]
    fn frontier_matches_brute(doc in arb_doc(), width in 2u32..30) {
        prop_assume!(count_choices(&doc) <= MAX_CHOICES);
        let cm = OverflowThenHeight { width };
        let oracle = brute::best(&cm, &doc, MAX_CHOICES);
        let best = frontier::best(&cm, &doc);
        let rendered = render_with(&doc, &cm, SolveLimits::default()).unwrap();
        prop_assert_eq!(&best.cost, &oracle.cost);
        prop_assert_eq!(rendered.text, oracle.text());
        prop_assert_eq!(rendered.spans, oracle.spans);
    }

    /// Penalties remain oracle-equivalent under the consumer model that
    /// assigns them nonzero branch-local burden.
    #[test]
    fn consumer_frontier_matches_brute(doc in arb_doc(), width in 2u32..30) {
        prop_assume!(count_choices(&doc) <= MAX_CHOICES);
        let cm = ConsumerCostModel::new(width);
        let oracle = brute::best(&cm, &doc, MAX_CHOICES);
        let rendered = render_with(&doc, &cm, SolveLimits::default()).unwrap();
        prop_assert_eq!(&rendered.cost, &oracle.cost);
        prop_assert_eq!(rendered.text, oracle.text());
        prop_assert_eq!(rendered.spans, oracle.spans);
    }

    #[test]
    fn built_in_models_satisfy_generated_cost_laws(
        target in 0u32..40,
        col in 0u32..40,
        first_width in 0u32..20,
        second_width in 0u32..20,
        other_col in 0u32..40,
        penalty in 0u32..20,
        newline_cost in 0u32..10,
    ) {
        assert_generated_cost_laws(
            &OverflowThenHeight { width: target },
            col,
            first_width,
            second_width,
            other_col,
            penalty,
        );
        assert_generated_cost_laws(
            &ConsumerCostModel::new(target).with_newline_cost(newline_cost),
            col,
            first_width,
            second_width,
            other_col,
            penalty,
        );
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
        prop_assert_eq!(cost_of_out(&cm, &best.out), best.cost);
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
