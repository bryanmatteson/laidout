use std::num::NonZeroU8;

use laidout::cost::OverflowThenHeight;
use laidout::{
    align, brute, concat, from_text, from_text_with, frontier, greedy, line, render, tags, text,
    to_string, try_text, try_text_with, Doc, IngestOptions, RenderOptions, TextError, WidthMode,
};

#[test]
fn wide_layout_preserves_normalized_text_and_classifies_runs() {
    let input = "  don't λ_2!\r\n\r\nnext\tline\n";
    let doc = from_text(input).unwrap();
    let rendered = render(&doc, &RenderOptions::new(100)).unwrap();
    assert_eq!(rendered.text, "  don't λ_2!\n\nnext    line\n");

    let runs = rendered.annotated_runs();
    assert!(runs
        .iter()
        .any(|run| run.text == "  " && run.tags == [tags::INDENT]));
    assert!(runs
        .iter()
        .any(|run| run.text == "don't" && run.tags == [tags::WORD]));
    assert!(runs
        .iter()
        .any(|run| run.text == "λ_2" && run.tags == [tags::WORD]));
    assert!(runs
        .iter()
        .any(|run| run.text == "!" && run.tags == [tags::SYMBOL]));
    assert!(runs
        .iter()
        .any(|run| run.text == "    " && run.tags == [tags::WHITESPACE]));
}

#[test]
fn frontier_reflows_later_than_greedy_when_that_saves_a_line() {
    let doc = from_text("a bb cccc").unwrap();
    let cm = OverflowThenHeight { width: 6 };
    let greedy = greedy::layout(&cm, &doc, 6);
    let best = frontier::best(&cm, &doc);
    let oracle = brute::best(&cm, &doc, 2);

    assert_eq!(greedy.lines.join("\n"), "a\nbb\ncccc");
    assert_eq!(to_string(&best.out), "a bb\ncccc");
    assert!(best.cost < greedy.cost);
    assert_eq!(best.cost, oracle.cost);
}

#[test]
fn reflowed_indented_text_returns_to_its_content_column() {
    let doc = from_text("  a bb cccc").unwrap();
    let best = frontier::best(&OverflowThenHeight { width: 6 }, &doc);
    assert_eq!(to_string(&best.out), "  a bb\n  cccc");
}

#[test]
fn physical_newlines_and_empty_lines_are_hard_boundaries() {
    let doc = from_text("one two\n\nthree four").unwrap();
    let cm = OverflowThenHeight { width: 100 };
    assert_eq!(
        to_string(&frontier::best(&cm, &doc).out),
        "one two\n\nthree four"
    );
    assert_eq!(
        greedy::layout(&cm, &doc, 100).lines.join("\n"),
        "one two\n\nthree four"
    );
}

#[test]
fn align_has_identical_semantics_in_all_engines() {
    let doc = concat([
        text("prefix: "),
        align(concat([text("one"), line(), text("two")])),
    ]);
    let cm = OverflowThenHeight { width: 80 };
    let expected = "prefix: one\n        two";

    assert_eq!(brute::best(&cm, &doc, 0).text(), expected);
    assert_eq!(greedy::layout(&cm, &doc, 80).lines.join("\n"), expected);
    assert_eq!(to_string(&frontier::best(&cm, &doc).out), expected);
}

#[test]
fn text_construction_rejects_every_terminal_control_with_offsets() {
    for (input, expected_character, expected_offset) in [
        ("a\nb", '\n', 1),
        ("é\tb", '\t', 2),
        ("a\u{1b}b", '\u{1b}', 1),
        ("a\0b", '\0', 1),
        ("a\u{7f}b", '\u{7f}', 1),
    ] {
        assert_eq!(
            try_text(input),
            Err(TextError::ControlCharacter {
                character: expected_character,
                byte_offset: expected_offset,
            })
        );
    }
}

#[test]
fn raw_ingestion_normalizes_carriage_returns_tabs_and_trailing_lines() {
    let options = IngestOptions {
        width_mode: WidthMode::Narrow,
        tab_width: NonZeroU8::new(4).unwrap(),
    };
    let doc = from_text_with("a\tb\rc\r\nd\t\n", options).unwrap();
    let wide = frontier::best(&OverflowThenHeight { width: 100 }, &doc);
    assert_eq!(to_string(&wide.out), "a   b\nc\nd   \n");
}

#[test]
fn tab_stops_advance_under_the_selected_unicode_width_mode() {
    let options = |width_mode| IngestOptions {
        width_mode,
        tab_width: NonZeroU8::new(4).unwrap(),
    };
    let narrow = from_text_with("·\tx", options(WidthMode::Narrow)).unwrap();
    let cjk = from_text_with("·\tx", options(WidthMode::Cjk)).unwrap();

    assert_eq!(
        to_string(&frontier::best(&OverflowThenHeight { width: 80 }, &narrow).out),
        "·   x"
    );
    assert_eq!(
        to_string(&frontier::best(&OverflowThenHeight { width: 80 }, &cjk).out),
        "·  x"
    );
}

#[test]
fn raw_ingestion_rejects_remaining_controls_at_original_byte_offsets() {
    assert_eq!(
        from_text("é\u{1b}bad"),
        Err(TextError::ControlCharacter {
            character: '\u{1b}',
            byte_offset: 2,
        })
    );
}

#[test]
fn narrow_and_cjk_runs_store_distinct_authoritative_widths() {
    let narrow = try_text_with("·", WidthMode::Narrow).unwrap();
    let cjk = try_text_with("·", WidthMode::Cjk).unwrap();
    assert_ne!(narrow, cjk);

    let columns = |doc: &Doc| match doc {
        Doc::Text(run) => run.columns(),
        _ => panic!("expected text"),
    };
    assert_eq!(columns(&narrow), 1);
    assert_eq!(columns(&cjk), 2);
}

#[test]
#[should_panic(expected = "control character '\\n' at UTF-8 byte offset 3")]
fn trusted_text_adapter_panics_with_the_typed_error_message() {
    let _doc = text("not\na document run");
}
