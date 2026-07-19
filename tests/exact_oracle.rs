#![cfg(feature = "research")]

use std::num::NonZeroU32;

use laidout::research::{brute, count_choices, CostModel, ResearchConsumerCostModel};
use laidout::{render, Doc, LayoutStrategy, RenderOptions};
use num_bigint::BigUint;

fn next(state: &mut u64) -> u64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    *state
}

fn generated_doc(state: &mut u64, depth: u8) -> Doc<u32> {
    let value = next(state);
    if depth == 0 {
        return match value % 6 {
            0 => Doc::empty(),
            1 => Doc::text("x"),
            2 => Doc::text("界"),
            3 => Doc::line(),
            4 => Doc::soft_line(),
            _ => Doc::hard_line(),
        };
    }
    match value % 9 {
        0 => Doc::concat([
            generated_doc(state, depth - 1),
            generated_doc(state, depth - 1),
        ]),
        1 => Doc::group(Doc::concat([
            generated_doc(state, depth - 1),
            Doc::line(),
            generated_doc(state, depth - 1),
        ])),
        2 => Doc::fill([
            generated_doc(state, depth - 1),
            generated_doc(state, depth - 1),
        ]),
        3 => Doc::choice(
            generated_doc(state, depth - 1),
            generated_doc(state, depth - 1),
        ),
        4 => Doc::nest((next(state) % 4) as u32, generated_doc(state, depth - 1)),
        5 => Doc::align(generated_doc(state, depth - 1)),
        6 => Doc::annotate((next(state) % 5) as u32, generated_doc(state, depth - 1)),
        7 => Doc::penalize((next(state) % 7) as u32, generated_doc(state, depth - 1)),
        _ => generated_doc(state, 0),
    }
}

#[test]
fn compact_exact_matches_the_unbounded_exhaustive_oracle_for_ten_thousand_documents() {
    let mut state = 0x4d59_5df4_d0f3_3173;
    for case in 0..10_000 {
        let doc = generated_doc(&mut state, 2);
        let width = 1 + (next(&mut state) % 16) as u32;
        let newline_cost = (next(&mut state) % 5) as u32;
        let options = RenderOptions::new(NonZeroU32::new(width).unwrap())
            .with_newline_cost(newline_cost)
            .with_strategy(LayoutStrategy::Exact);
        let compact = render(&doc, options).unwrap();
        let research_model = ResearchConsumerCostModel::new(width).with_newline_cost(newline_cost);
        let oracle = brute::best(&research_model, &doc, count_choices(&doc));

        assert_eq!(compact.text(), oracle.text(), "case {case}");
        assert_eq!(
            BigUint::from(compact.cost().overflow()),
            oracle.cost.overflow,
            "case {case}"
        );
        assert_eq!(
            BigUint::from(compact.cost().burden()),
            oracle.cost.burden,
            "case {case}"
        );
        let compact_spans = compact
            .resolved_spans()
            .map(|(span, tag)| {
                (
                    *tag,
                    span.range.clone(),
                    span.parent.map(|parent| parent.index()),
                )
            })
            .collect::<Vec<_>>();
        let oracle_spans = oracle
            .spans
            .iter()
            .map(|span| (span.tag, span.range.clone(), span.parent))
            .collect::<Vec<_>>();
        assert_eq!(compact_spans, oracle_spans, "case {case}");
    }
}

#[derive(Clone, Copy, Debug)]
struct DeliberatelyUnlawful;

impl CostModel for DeliberatelyUnlawful {
    type Cost = u32;

    fn zero(&self) -> Self::Cost {
        0
    }
    fn add(&self, left: &Self::Cost, right: &Self::Cost) -> Self::Cost {
        left ^ right
    }
    fn text(&self, column: u32, width: u32) -> Self::Cost {
        column ^ width
    }
    fn newline(&self) -> Self::Cost {
        11
    }
    fn penalty(&self, amount: u32) -> Self::Cost {
        amount.rotate_left(3)
    }
}

#[test]
fn arbitrary_models_remain_available_to_nonpruning_research_engines() {
    let doc = Doc::choice(Doc::text("a"), Doc::penalize(2, Doc::text("b")));
    let choices = count_choices(&doc);
    let _ = brute::best(&DeliberatelyUnlawful, &doc, choices);
    let _ = laidout::research::greedy::layout(&DeliberatelyUnlawful, &doc, 80);
}
