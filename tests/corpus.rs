//! The ported JSON corpus, exercised at several widths.

use pretty::corpus::{complex_value, format, Value};
use pretty::cost::OverflowThenHeight;
use pretty::{brute, frontier, greedy, to_string};

fn stripped(s: &str) -> String {
    s.chars().filter(|c| !c.is_whitespace()).collect()
}

#[test]
fn small_array_breaks_exactly_when_needed() {
    let doc = format(&Value::Arr(vec![
        Value::Int(1),
        Value::Int(2),
        Value::Int(3),
    ]));

    let wide = frontier::best(&OverflowThenHeight { width: 10 }, &doc);
    assert_eq!(to_string(&wide.out), "[1, 2, 3]");

    let narrow = frontier::best(&OverflowThenHeight { width: 5 }, &doc);
    assert_eq!(to_string(&narrow.out), "[\n  1,\n  2,\n  3\n]");
}

#[test]
fn complex_json_flat_at_generous_width() {
    let doc = format(&complex_value());
    let cm = OverflowThenHeight { width: 400 };
    let best = frontier::best(&cm, &doc);
    let text = to_string(&best.out);
    assert_eq!(
        text.lines().count(),
        1,
        "everything fits on one line:\n{text}"
    );
    assert_eq!(best.cost.0, 0, "no overflow at width 400");
}

#[test]
fn complex_json_frontier_never_worse_than_greedy() {
    let doc = format(&complex_value());
    for width in [20u32, 30, 40, 60, 80, 120] {
        let cm = OverflowThenHeight { width };
        let g = greedy::layout(&cm, &doc, width);
        let best = frontier::best(&cm, &doc);
        assert!(
            best.cost <= g.cost,
            "width {width}: frontier {:?} > greedy {:?}",
            best.cost,
            g.cost
        );
        assert_eq!(
            stripped(&to_string(&best.out)),
            stripped(&g.lines.join("\n")),
            "width {width}: engines disagree on content"
        );
    }
}

#[test]
fn subcorpus_matches_brute_force() {
    // The config sub-object is small enough for exhaustive enumeration.
    let config = Value::Obj(vec![
        ("timeout".into(), Value::Int(30)),
        ("retries".into(), Value::Int(3)),
        (
            "servers".into(),
            Value::Arr(vec![Value::Obj(vec![
                ("host".into(), Value::Str("server1".into())),
                ("port".into(), Value::Int(8080)),
            ])]),
        ),
    ]);
    let doc = format(&config);
    for width in [10u32, 25, 45, 90] {
        let cm = OverflowThenHeight { width };
        let oracle = brute::best(&cm, &doc, 16);
        let best = frontier::best(&cm, &doc);
        assert_eq!(
            best.cost, oracle.cost,
            "width {width}: frontier is not optimal"
        );
    }
}

#[test]
fn complex_json_readable_at_40() {
    let doc = format(&complex_value());
    let cm = OverflowThenHeight { width: 40 };
    let best = frontier::best(&cm, &doc);
    let text = to_string(&best.out);
    // No overflow is achievable at 40, so the optimum must have none —
    // and must not break lines it doesn't have to.
    assert_eq!(best.cost.0, 0, "unexpected overflow:\n{text}");
    for line in text.lines() {
        assert!(
            pretty::cost::display_width(line) <= 40,
            "line exceeds width: {line:?}"
        );
    }
}
