use pretty::corpus::fluid::PROSE;
use pretty::cost::OverflowThenHeight;
use pretty::{from_text, frontier, greedy, to_string, words};

#[test]
fn recovered_prose_round_trips_wide_and_reflows_without_overflow() {
    let doc = from_text(PROSE);
    let wide = frontier::best(&OverflowThenHeight { width: 10_000 }, &doc);
    assert_eq!(to_string(&wide.out), PROSE);

    let cm = OverflowThenHeight { width: 40 };
    let greedy = greedy::layout(&cm, &doc, 40);
    let best = frontier::best(&cm, &doc);
    let rendered = to_string(&best.out);

    assert_eq!(best.cost.0, 0);
    assert!(best.cost <= greedy.cost);
    assert_eq!(words(&rendered), words(PROSE));
    assert!(rendered.lines().all(|line| line.chars().count() <= 40));
}
