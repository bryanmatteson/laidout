use std::num::NonZeroU32;

use laidout::{render, LayoutStrategy, RenderOptions};

#[path = "../support/json.rs"]
#[allow(dead_code)]
mod corpus;
use corpus::{complex_value, format, Value};

fn render_at(doc: &laidout::Doc, width: u32, strategy: LayoutStrategy) -> String {
    render(
        doc,
        RenderOptions::new(NonZeroU32::new(width).unwrap()).with_strategy(strategy),
    )
    .unwrap()
    .text()
    .to_owned()
}

#[test]
fn small_array_uses_flat_and_broken_group_projections() {
    let doc = format(&Value::Arr(vec![
        Value::Int(1),
        Value::Int(2),
        Value::Int(3),
    ]));
    assert_eq!(render_at(&doc, 10, LayoutStrategy::Exact), "[1, 2, 3]");
    assert_eq!(
        render_at(&doc, 5, LayoutStrategy::Exact),
        "[\n  1,\n  2,\n  3\n]"
    );
}

#[test]
fn complex_json_is_valid_and_strategy_invariant_in_content() {
    let doc = format(&complex_value());
    for strategy in [LayoutStrategy::Fast, LayoutStrategy::Exact] {
        let text = render_at(&doc, 40, strategy);
        let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(parsed["name"], "Example");
        assert_eq!(parsed["enabled"], true);
        assert_eq!(parsed["config"]["servers"].as_array().unwrap().len(), 2);
    }
}

#[test]
fn complex_json_is_flat_at_a_generous_width() {
    let doc = format(&complex_value());
    let text = render_at(&doc, 400, LayoutStrategy::Exact);
    assert_eq!(text.lines().count(), 1);
}
