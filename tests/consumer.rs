use laidout::render::cost_of_out;
use laidout::{
    brute, choice, concat, concat2, empty, greedy, hardline, nest, penalize, render, render_with,
    tag, text, AnnotationSpan, ConsumerCostModel, CostModel, OverflowThenHeight, RenderError,
    RenderOptions, SolveLimitKind, SolveLimits,
};
use num_bigint::BigUint;

#[test]
fn nested_repeated_identical_and_empty_annotations_are_lossless() {
    let doc = tag(
        10,
        concat([
            text("A"),
            tag(10, tag(11, text("界"))),
            nest(2, concat([hardline(), tag(12, text("z"))])),
            tag(13, empty()),
        ]),
    );
    let rendered = render(&doc, &RenderOptions::new(80)).unwrap();

    assert_eq!(rendered.text, "A界\n  z");
    assert_eq!(
        rendered.spans,
        vec![
            AnnotationSpan {
                tag: 10,
                range: 0..8,
                parent: None,
            },
            AnnotationSpan {
                tag: 10,
                range: 1..4,
                parent: Some(0),
            },
            AnnotationSpan {
                tag: 11,
                range: 1..4,
                parent: Some(1),
            },
            AnnotationSpan {
                tag: 12,
                range: 7..8,
                parent: Some(0),
            },
            AnnotationSpan {
                tag: 13,
                range: 8..8,
                parent: Some(0),
            },
        ]
    );

    let runs = rendered.annotated_runs();
    assert_eq!(
        runs.iter()
            .map(|run| (run.text, run.range.clone(), run.tags.clone()))
            .collect::<Vec<_>>(),
        vec![
            ("A", 0..1, vec![10]),
            ("界", 1..4, vec![10, 10, 11]),
            ("\n  ", 4..7, vec![10]),
            ("z", 7..8, vec![10, 12]),
        ]
    );
    assert_eq!(
        runs.iter().map(|run| run.text).collect::<String>(),
        rendered.text
    );
    assert!(runs.iter().all(|run| !run.text.is_empty()));

    let baseline = OverflowThenHeight { width: 80 };
    let oracle = brute::best(&baseline, &doc, 0);
    let greedy = greedy::layout(&baseline, &doc, 80);
    assert_eq!(oracle.spans, rendered.spans);
    assert_eq!(greedy.spans, rendered.spans);
}

#[test]
fn penalties_select_branches_without_changing_width_or_losing_events() {
    let lower_penalty = choice(tag(1, penalize(3, text("same"))), tag(2, text("same")));
    let rendered = render(&lower_penalty, &RenderOptions::new(80)).unwrap();
    assert_eq!(rendered.text, "same");
    assert_eq!(rendered.spans[0].tag, 2);
    assert_eq!(rendered.cost.burden, 0);

    let trade_line_for_penalty = choice(
        penalize(2, text("ab")),
        concat([text("a"), hardline(), text("b")]),
    );
    let rendered = render(&trade_line_for_penalty, &RenderOptions::new(80)).unwrap();
    assert_eq!(rendered.text, "a\nb");
    assert_eq!(rendered.cost.burden, 1);

    let overflow_dominates_burden = choice(
        text("ab"),
        penalize(100, concat([text("a"), hardline(), text("b")])),
    );
    let rendered = render(&overflow_dominates_burden, &RenderOptions::new(1)).unwrap();
    assert_eq!(rendered.text, "a\nb");
    assert_eq!(rendered.cost.overflow, BigUint::from(0u8));
    assert_eq!(rendered.cost.burden, 101);

    let baseline = render_with(
        &choice(tag(1, penalize(99, text("same"))), tag(2, text("same"))),
        &OverflowThenHeight { width: 80 },
        SolveLimits::default(),
    )
    .unwrap();
    assert_eq!(baseline.spans[0].tag, 1);

    let model = ConsumerCostModel::new(80);
    let greedy = greedy::layout(&model, &penalize(7, text("x")), 80);
    assert_eq!(greedy.cost.burden, 7);
    assert_eq!(greedy.cost, cost_of_out(&model, &greedy.out));
    assert_eq!(penalize(0, text("x")), text("x"));
}

#[test]
fn equal_cost_layouts_keep_the_earlier_branch_even_when_it_ends_farther_right() {
    let doc = choice(tag(1, text("preferred")), tag(2, text("x")));
    let cm = OverflowThenHeight { width: 80 };
    let oracle = brute::best(&cm, &doc, 1);
    let exact = render_with(&doc, &cm, SolveLimits::default()).unwrap();

    assert_eq!(oracle.text(), "preferred");
    assert_eq!(exact.text, oracle.text());
    assert_eq!(exact.spans, oracle.spans);
    assert_eq!(exact.spans[0].tag, 1);
}

fn assert_cost_laws<M: CostModel>(model: &M) {
    let zero = model.zero();
    let a = model.text(2, 3);
    let b = model.newline();
    let c = model.penalty(4);
    assert_eq!(model.add(&zero, &a), a);
    assert_eq!(model.add(&a, &zero), a);
    assert_eq!(
        model.add(&model.add(&a, &b), &c),
        model.add(&a, &model.add(&b, &c))
    );
    assert_eq!(
        model.text(5, 7),
        model.add(&model.text(5, 3), &model.text(8, 4))
    );
    assert!(model.text(3, 4) <= model.text(7, 4));
    assert!(model.penalty(0) <= model.penalty(4));
}

#[test]
fn built_in_cost_models_obey_the_frontier_laws() {
    assert_cost_laws(&OverflowThenHeight { width: 8 });
    assert_cost_laws(&ConsumerCostModel::new(8).with_newline_cost(3));
}

#[derive(Clone, Copy, Debug)]
struct NonIncremental;

impl CostModel for NonIncremental {
    type Cost = u64;

    fn zero(&self) -> Self::Cost {
        0
    }

    fn add(&self, a: &Self::Cost, b: &Self::Cost) -> Self::Cost {
        a + b
    }

    fn text(&self, col: u32, width: u32) -> Self::Cost {
        u64::from(col) * u64::from(width)
    }

    fn newline(&self) -> Self::Cost {
        0
    }

    fn penalty(&self, amount: u32) -> Self::Cost {
        u64::from(amount)
    }
}

#[test]
fn a_test_model_that_breaks_incrementality_fails_the_law_fixture() {
    let model = NonIncremental;
    assert_ne!(
        model.text(5, 7),
        model.add(&model.text(5, 3), &model.text(8, 4))
    );
}

#[test]
fn arbitrary_models_remain_available_to_non_pruning_engines() {
    let model = NonIncremental;
    let doc = choice(text("left"), text("right"));

    let oracle = brute::best(&model, &doc, 1);
    let greedy = laidout::greedy::layout(&model, &doc, 80);

    assert_eq!(oracle.text(), "left");
    assert_eq!(greedy.lines.join("\n"), "left");
}

fn limit_error(
    doc: &std::rc::Rc<laidout::Doc>,
    limits: SolveLimits,
) -> (SolveLimitKind, u64, laidout::SolveStats) {
    match render(doc, &RenderOptions::new(12).with_limits(limits)).unwrap_err() {
        RenderError::LimitExceeded {
            kind,
            configured,
            stats,
        } => (kind, configured, stats),
        error => panic!("unexpected render error: {error}"),
    }
}

#[test]
fn exact_limits_fail_deterministically_after_the_configured_event() {
    let doc = concat([
        choice(
            text("alpha beta"),
            concat([text("alpha"), hardline(), text("beta")]),
        ),
        text(" "),
        choice(text("gamma"), concat([text("g"), hardline(), text("amma")])),
    ]);
    let baseline = render(&doc, &RenderOptions::new(12)).unwrap();
    assert!(baseline.stats.solve_calls > 1);
    assert!(baseline.stats.candidates_generated > 1);
    assert!(baseline.stats.memo_entries > 1);

    let exact_limits = SolveLimits {
        max_solve_calls: Some(baseline.stats.solve_calls),
        max_candidates: Some(baseline.stats.candidates_generated),
        max_memo_entries: Some(baseline.stats.memo_entries),
    };
    let exact = render(&doc, &RenderOptions::new(12).with_limits(exact_limits)).unwrap();
    assert_eq!(exact.stats, baseline.stats);

    let call_limit = baseline.stats.solve_calls - 1;
    let call_error = limit_error(
        &doc,
        SolveLimits {
            max_solve_calls: Some(call_limit),
            ..SolveLimits::default()
        },
    );
    assert_eq!(call_error.0, SolveLimitKind::SolveCalls);
    assert_eq!(call_error.1, call_limit);
    assert_eq!(call_error.2.solve_calls, call_limit + 1);

    let candidate_limit = baseline.stats.candidates_generated - 1;
    let candidate_error = limit_error(
        &doc,
        SolveLimits {
            max_candidates: Some(candidate_limit),
            ..SolveLimits::default()
        },
    );
    assert_eq!(candidate_error.0, SolveLimitKind::Candidates);
    assert_eq!(candidate_error.1, candidate_limit);
    assert_eq!(candidate_error.2.candidates_generated, candidate_limit + 1);

    let memo_limit = baseline.stats.memo_entries - 1;
    let memo_error = limit_error(
        &doc,
        SolveLimits {
            max_memo_entries: Some(memo_limit),
            ..SolveLimits::default()
        },
    );
    assert_eq!(memo_error.0, SolveLimitKind::MemoEntries);
    assert_eq!(memo_error.1, memo_limit as u64);
    assert_eq!(memo_error.2.memo_entries, memo_limit);

    assert_eq!(
        limit_error(
            &doc,
            SolveLimits {
                max_candidates: Some(candidate_limit),
                ..SolveLimits::default()
            }
        ),
        candidate_error
    );
}

#[test]
fn exact_render_handles_deep_concat_without_recursive_layout_or_output_walks() {
    let doc = (0..3_000).fold(empty(), |doc, _| concat2(doc, text("x")));
    let rendered = render(&doc, &RenderOptions::new(3_000)).unwrap();
    assert_eq!(rendered.text.len(), 3_000);
    assert!(rendered.text.bytes().all(|byte| byte == b'x'));
}
