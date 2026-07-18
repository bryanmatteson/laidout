use laidout::cost::OverflowThenHeight;
use laidout::render::Out;
use laidout::{
    align, brute, concat, from_text, frontier, greedy, line, tags, text, to_string, Doc,
};

fn text_tags(out: &Out, tags_out: &mut Vec<(String, Option<u32>)>) {
    match out {
        Out::Empty | Out::Newline(_) => {}
        Out::Text(value, tag) => tags_out.push((value.to_string(), *tag)),
        Out::Cat(left, right) => {
            text_tags(left, tags_out);
            text_tags(right, tags_out);
        }
    }
}

#[test]
fn wide_layout_preserves_text_and_classifies_runs() {
    let input = "  don't λ_2!\r\n\r\nnext\tline\n";
    let doc = from_text(input);
    let best = frontier::best(&OverflowThenHeight { width: 100 }, &doc);
    assert_eq!(to_string(&best.out), "  don't λ_2!\n\nnext\tline\n");

    let mut runs = Vec::new();
    text_tags(&best.out, &mut runs);
    assert!(runs.contains(&("  ".into(), Some(tags::INDENT))));
    assert!(runs.contains(&("don't".into(), Some(tags::WORD))));
    assert!(runs.contains(&("λ_2".into(), Some(tags::WORD))));
    assert!(runs.contains(&("!".into(), Some(tags::SYMBOL))));
    assert!(runs.contains(&("\t".into(), Some(tags::WHITESPACE))));
}

#[test]
fn frontier_reflows_later_than_greedy_when_that_saves_a_line() {
    let doc = from_text("a bb cccc");
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
    let doc = from_text("  a bb cccc");
    let best = frontier::best(&OverflowThenHeight { width: 6 }, &doc);
    assert_eq!(to_string(&best.out), "  a bb\n  cccc");
}

#[test]
fn physical_newlines_and_empty_lines_are_hard_boundaries() {
    let doc = from_text("one two\n\nthree four");
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
#[should_panic(expected = "text must not contain line feeds")]
fn raw_text_still_rejects_embedded_line_feeds() {
    let _doc: std::rc::Rc<Doc> = text("not\na document run");
}
