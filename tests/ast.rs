use std::num::NonZeroU32;

use laidout::{render, LayoutStrategy, RenderOptions};

#[path = "../support/ast.rs"]
#[allow(dead_code)]
mod corpus;
use corpus::{asymmetric_file, sample_file, Formatter};

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
fn go_like_sample_retains_its_structural_sections() {
    let doc = Formatter::default().format_file(&sample_file());
    let text = render_at(&doc, 100, LayoutStrategy::Exact);
    assert!(text.starts_with("package main\n\nimport ("));
    assert!(text.contains("func (p Processor) processData"));
    assert!(text.ends_with("    // ... formatted statements\n}"));
}

#[test]
fn asymmetric_signature_is_valid_in_both_strategies() {
    let doc = Formatter::default().format_file(&asymmetric_file());
    let fast = render_at(&doc, 18, LayoutStrategy::Fast);
    let exact = render_at(&doc, 18, LayoutStrategy::Exact);
    for text in [&fast, &exact] {
        assert!(text.starts_with("package p"));
        assert!(text.contains("func f"));
        assert!(text.contains("work()"));
    }
    assert!(exact.lines().count() <= fast.lines().count());
}
