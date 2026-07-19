use std::num::NonZeroU32;

use laidout::{render, Doc, LayoutStrategy, RenderOptions};
use proptest::prelude::*;

fn fast_options(width: u32) -> RenderOptions {
    RenderOptions::new(NonZeroU32::new(width).unwrap()).with_strategy(LayoutStrategy::Fast)
}

#[test]
fn zero_width_text_is_charged_only_by_display_columns() {
    let doc = Doc::<u32>::group(Doc::concat([
        Doc::text("\u{200b}\u{200b}\u{200b}\u{200b}"),
        Doc::line(),
        Doc::text("x"),
    ]));
    let rendered = render(&doc, fast_options(2)).expect("fast render");
    assert_eq!(rendered.text(), "\u{200b}\u{200b}\u{200b}\u{200b} x");
    assert_eq!(rendered.stats().fit_checks, 1);
}

#[test]
fn hard_line_in_a_group_never_disappears() {
    let doc = Doc::<u32>::group(Doc::concat([
        Doc::text("a"),
        Doc::hard_line(),
        Doc::text("b"),
    ]));
    let rendered = render(&doc, fast_options(80)).expect("fast render");
    assert_eq!(rendered.text(), "a\nb");
}

proptest! {
    #[test]
    fn prepared_group_summary_matches_a_direct_continuation_scan(
        words in prop::collection::vec("[a-z]{0,8}", 1..8),
        trailer in "[a-z]{0,8}",
        width in 1u32..80,
    ) {
        let body = Doc::concat(words.iter().enumerate().flat_map(|(index, word)| {
            let mut nodes: Vec<Doc<u32>> = Vec::new();
            if index != 0 { nodes.push(Doc::line()); }
            nodes.push(Doc::text(word));
            nodes
        }));
        let doc = Doc::concat([Doc::group(body), Doc::text(&trailer)]);
        let rendered = render(&doc, fast_options(width)).unwrap();

        let flat_body = words.join(" ");
        let flat = format!("{flat_body}{trailer}");
        let expected = if unicode_width::UnicodeWidthStr::width(flat.as_str()) <= width as usize {
            flat
        } else {
            format!("{}{trailer}", words.join("\n"))
        };
        prop_assert_eq!(rendered.text(), expected);
        prop_assert_eq!(rendered.stats().fit_checks, 1);
    }
}

#[test]
fn fill_probes_only_the_next_child_at_each_boundary() {
    let doc = Doc::<u32>::fill([
        Doc::text("aa"),
        Doc::text("b"),
        Doc::text("cccc"),
        Doc::text("d"),
    ]);
    let rendered = render(&doc, fast_options(6)).unwrap();
    assert_eq!(rendered.text(), "aa b\ncccc d");
    assert_eq!(rendered.stats().fit_checks, 3);
}

fn zero_width_decisions(count: usize) -> Doc<u32> {
    Doc::concat((0..count).map(|_| {
        Doc::group(Doc::concat([
            Doc::text("\u{200b}"),
            Doc::line(),
            Doc::text("x"),
        ]))
    }))
}

#[test]
fn nested_zero_width_work_has_exactly_one_fit_check_per_decision() {
    for count in [2_000usize, 4_000] {
        let rendered = render(&zero_width_decisions(count), fast_options(u32::MAX)).unwrap();
        assert_eq!(rendered.stats().fit_checks, count as u64);
        assert_eq!(rendered.text().matches('x').count(), count);
        assert!(!rendered.text().contains('\n'));
    }
}
