use std::num::{NonZeroU32, NonZeroU8};

use laidout::{
    from_text, from_text_with, render, tags, Doc, IngestOptions, LayoutStrategy, RenderOptions,
    TextError, WidthMode,
};

fn options(width: u32) -> RenderOptions {
    RenderOptions::new(NonZeroU32::new(width).unwrap()).with_strategy(LayoutStrategy::Exact)
}

#[test]
fn wide_ingestion_preserves_normalized_text_and_classifies_runs() {
    let doc = from_text("  don't λ_2!\r\n\tnext\tline\r").unwrap();
    let rendered = render(&doc, options(100)).unwrap();
    assert_eq!(rendered.text(), "  don't λ_2!\n        next    line\n");
    let runs = rendered.annotated_runs();
    assert!(runs
        .iter()
        .any(|run| { run.text == "  " && run.annotations.as_slice() == [&tags::INDENT] }));
    assert!(runs
        .iter()
        .any(|run| { run.text == "don't" && run.annotations.as_slice() == [&tags::WORD] }));
    assert!(runs
        .iter()
        .any(|run| { run.text == "!" && run.annotations.as_slice() == [&tags::SYMBOL] }));
}

#[test]
fn tab_expansion_uses_source_column_and_configured_stop() {
    let doc = from_text_with(
        "界\tx",
        IngestOptions {
            width_mode: WidthMode::Narrow,
            tab_width: NonZeroU8::new(4).unwrap(),
        },
    )
    .unwrap();
    assert_eq!(render(&doc, options(80)).unwrap().text(), "界  x");
}

#[test]
fn single_line_text_rejects_every_control_with_a_byte_offset() {
    for (value, offset) in [("a\n", 1), ("界\t", 3), ("x\u{7f}", 1), ("\u{1b}x", 0)] {
        assert!(matches!(
            Doc::<u32>::try_text(value),
            Err(TextError::ControlCharacter { byte_offset, .. }) if byte_offset == offset
        ));
    }
}

#[test]
fn explicit_cjk_width_changes_fit_without_changing_bytes() {
    let narrow = Doc::<u32>::try_text_with("·", WidthMode::Narrow).unwrap();
    let cjk = Doc::<u32>::try_text_with("·", WidthMode::Cjk).unwrap();
    let narrow_doc = Doc::group(Doc::concat([narrow, Doc::line(), Doc::text("x")]));
    let cjk_doc = Doc::group(Doc::concat([cjk, Doc::line(), Doc::text("x")]));
    assert_eq!(render(&narrow_doc, options(3)).unwrap().text(), "· x");
    assert_eq!(render(&cjk_doc, options(3)).unwrap().text(), "·\nx");
}
