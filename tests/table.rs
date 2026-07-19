use laidout::cost::OverflowThenHeight;
use laidout::{
    brute, choice, concat, count_choices, frontier, greedy, hardline, line, table, tag, text,
    to_string, try_text_with, Alignment, AnnotationSpan, Column, SolveLimits, Table, TableError,
    WidthMode,
};
use num_bigint::BigUint;
use proptest::prelude::*;

fn aligned_table() -> std::rc::Rc<laidout::Doc> {
    table([
        Column::labeled(text("NAME")),
        Column::labeled(text("COUNT"))
            .alignment(Alignment::Right)
            .min_padding(1),
        Column::labeled(text("STATE")).alignment(Alignment::Center),
    ])
    .header()
    .row([text("alpha"), text("7"), text("on")])
    .row([text("β"), text("123")])
    .build()
    .unwrap()
}

#[test]
fn compact_layout_aligns_headers_ragged_rows_and_all_alignment_modes() {
    let doc = aligned_table();
    let best = frontier::best(&OverflowThenHeight { width: 80 }, &doc);
    assert_eq!(
        to_string(&best.out),
        "NAME   COUNT STATE\nalpha      7  on\nβ        123"
    );
}

#[test]
fn the_same_table_uses_its_stacked_fallback_when_compact_overflows() {
    let doc = aligned_table();
    let best = frontier::best(&OverflowThenHeight { width: 5 }, &doc);
    assert_eq!(
        to_string(&best.out),
        "NAME\nCOUNT\nSTATE\nalpha\n7\non\nβ\n123"
    );
    assert_eq!(best.cost.0, BigUint::from(0u8));
}

#[test]
fn missing_column_specs_use_two_space_left_aligned_defaults() {
    let doc = Table::new([Column::new()])
        .row([text("a"), text("bbb")])
        .row([text("long"), text("c")])
        .build()
        .unwrap();
    let best = frontier::best(&OverflowThenHeight { width: 80 }, &doc);
    assert_eq!(to_string(&best.out), "a     bbb\nlong  c");
}

#[test]
fn unicode_terminal_width_aligns_wide_and_combining_text() {
    let doc = table([
        Column::labeled(text("界")),
        Column::labeled(text("e\u{301}")),
        Column::labeled(text("H")),
    ])
    .header()
    .row([text("x"), text("y"), text("z")])
    .build()
    .unwrap();
    let best = frontier::best(&OverflowThenHeight { width: 80 }, &doc);
    assert_eq!(to_string(&best.out), "界  e\u{301}  H\nx   y  z");
}

#[test]
fn nested_table_rows_align_to_the_table_starting_column() {
    let table = table([Column::new(), Column::new()])
        .row([text("a"), text("b")])
        .row([text("c"), text("d")])
        .build()
        .unwrap();
    let doc = concat([text("prefix "), table]);
    let best = frontier::best(&OverflowThenHeight { width: 80 }, &doc);
    assert_eq!(to_string(&best.out), "prefix a  b\n       c  d");
}

#[test]
fn tags_survive_compact_padding_and_fallback_layouts() {
    let doc = table([Column::new(), Column::new()])
        .row([tag(100, text("a")), tag(101, text("b"))])
        .build()
        .unwrap();

    let compact = laidout::render_with(
        &doc,
        &OverflowThenHeight { width: 80 },
        SolveLimits::default(),
    )
    .unwrap();
    assert_eq!(
        compact.spans,
        vec![
            AnnotationSpan {
                tag: 100,
                range: 0..1,
                parent: None,
            },
            AnnotationSpan {
                tag: laidout::tags::WHITESPACE,
                range: 1..3,
                parent: None,
            },
            AnnotationSpan {
                tag: 101,
                range: 3..4,
                parent: None,
            },
        ]
    );

    let fallback = laidout::render_with(
        &doc,
        &OverflowThenHeight { width: 1 },
        SolveLimits::default(),
    )
    .unwrap();
    assert_eq!(
        fallback.spans,
        vec![
            AnnotationSpan {
                tag: 100,
                range: 0..1,
                parent: None,
            },
            AnnotationSpan {
                tag: 101,
                range: 2..3,
                parent: None,
            },
        ]
    );
}

#[test]
fn tables_align_from_each_runs_stored_width_without_global_remeasurement() {
    let cjk = try_text_with("·", WidthMode::Cjk).unwrap();
    let narrow = try_text_with("·", WidthMode::Narrow).unwrap();
    let doc = table([Column::new(), Column::new()])
        .row([cjk, text("x")])
        .row([narrow, text("y")])
        .build()
        .unwrap();

    let best = frontier::best(&OverflowThenHeight { width: 80 }, &doc);
    assert_eq!(to_string(&best.out), "·  x\n·   y");
}

#[test]
fn bounded_table_matches_brute_force_and_is_deterministic() {
    let doc = aligned_table();
    assert_eq!(count_choices(&doc), 1);
    for width in [5, 12, 80] {
        let cm = OverflowThenHeight { width };
        let oracle = brute::best(&cm, &doc, 1);
        let first = frontier::best(&cm, &doc);
        let second = frontier::best(&cm, &doc);
        assert_eq!(first.cost, oracle.cost, "width {width}");
        assert_eq!(to_string(&first.out), to_string(&second.out));
    }
}

#[test]
fn zero_row_and_header_only_tables_have_defined_output() {
    assert_eq!(
        Table::new([Column::new()]).build().unwrap(),
        laidout::empty()
    );

    let header_only = table([Column::labeled(text("A")), Column::labeled(text("B"))])
        .header()
        .build()
        .unwrap();
    let best = frontier::best(&OverflowThenHeight { width: 80 }, &header_only);
    assert_eq!(to_string(&best.out), "A  B");
}

#[test]
fn non_flattenable_headers_and_cells_report_their_coordinates() {
    let header_error = table([Column::labeled(hardline())]).header().build();
    assert_eq!(
        header_error,
        Err(TableError::NonFlattenableHeader { column: 0 })
    );

    let cell_error = table([Column::new(), Column::new()])
        .row([text("ok"), hardline()])
        .build();
    assert_eq!(
        cell_error,
        Err(TableError::NonFlattenableCell { row: 0, column: 1 })
    );
}

#[test]
fn choice_bearing_headers_and_cells_are_rejected_without_losing_alternatives() {
    let header_error = table([Column::labeled(choice(text("wide"), text("narrow")))])
        .header()
        .build();
    assert_eq!(
        header_error,
        Err(TableError::ChoiceBearingHeader { column: 0 })
    );

    let cell_error = table([Column::new(), Column::new()])
        .row([
            text("ok"),
            tag(7, concat([text("nested"), choice(text("a"), text("b"))])),
        ])
        .build();
    assert_eq!(
        cell_error,
        Err(TableError::ChoiceBearingCell { row: 0, column: 1 })
    );

    let choice_free_projection = table([Column::new()])
        .row([concat([text("a"), line(), text("b")])])
        .build()
        .unwrap();
    let best = frontier::best(&OverflowThenHeight { width: 80 }, &choice_free_projection);
    assert_eq!(to_string(&best.out), "a b");
}

proptest! {
    #[test]
    fn randomized_ragged_tables_match_the_oracle_and_preserve_content(
        rows in prop::collection::vec(
            prop::collection::vec("[a-z]{1,5}", 0..4),
            0..5,
        ),
        width in 1u32..40,
    ) {
        let mut builder = Table::new([]);
        for row in &rows {
            builder.push_row(row.iter().map(text));
        }
        let doc = builder.build().unwrap();
        let cm = OverflowThenHeight { width };
        let choices = count_choices(&doc);
        let oracle = brute::best(&cm, &doc, choices);
        let best = frontier::best(&cm, &doc);
        let greedy = greedy::layout(&cm, &doc, width);

        let expected = rows.iter().flatten().cloned().collect::<Vec<_>>();
        let best_words = laidout::words(&to_string(&best.out));
        let greedy_words = laidout::words(&greedy.lines.join("\n"));

        prop_assert_eq!(&best.cost, &oracle.cost);
        prop_assert!(best.cost <= greedy.cost);
        prop_assert_eq!(best_words, expected.clone());
        prop_assert_eq!(greedy_words, expected);
    }
}
