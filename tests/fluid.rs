use std::num::NonZeroU32;

use laidout::{from_text, render, LayoutStrategy, RenderOptions};

#[path = "../support/fluid.rs"]
mod corpus;
use corpus::PROSE;

#[test]
fn recovered_prose_round_trips_wide_and_reflows_by_columns() {
    let doc = from_text(PROSE).unwrap();
    let wide = render(
        &doc,
        RenderOptions::new(NonZeroU32::new(10_000).unwrap()).with_strategy(LayoutStrategy::Exact),
    )
    .unwrap();
    assert_eq!(wide.text(), PROSE.replace('\t', "        "));

    let narrow = render(
        &doc,
        RenderOptions::new(NonZeroU32::new(40).unwrap()).with_strategy(LayoutStrategy::Exact),
    )
    .unwrap();
    assert_eq!(
        narrow.text().split_whitespace().collect::<Vec<_>>(),
        PROSE.split_whitespace().collect::<Vec<_>>()
    );
    assert!(narrow
        .text()
        .lines()
        .all(|line| laidout::cost::display_width(line) <= 40));
}
