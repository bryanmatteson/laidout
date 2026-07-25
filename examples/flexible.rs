use std::num::NonZeroU32;

use laidout::{
    from_text_with_annotations, IngestOptions, RenderOptions, Renderer, TextKind, WidthMode,
};

#[derive(Debug)]
enum Syntax {
    Word,
    Punctuation,
    Space,
    Indent,
    Break,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let options = IngestOptions {
        width_mode: WidthMode::Narrow,
        ..IngestOptions::default()
    };
    let doc = from_text_with_annotations("hello, flexible world", options, |kind| match kind {
        TextKind::Word => Syntax::Word,
        TextKind::Symbol => Syntax::Punctuation,
        TextKind::Whitespace => Syntax::Space,
        TextKind::Indent => Syntax::Indent,
        TextKind::Newline => Syntax::Break,
    })?;
    let prepared = doc.prepare()?;

    let render_options = RenderOptions::new(NonZeroU32::new(12).expect("nonzero width"));
    let mut renderer = Renderer::new();
    let rendered = renderer.render_prepared(&prepared, render_options)?;

    println!("{}", rendered.text());
    for (span, syntax) in rendered.resolved_spans() {
        println!("{:?}: {syntax:?}", span.range);
    }
    Ok(())
}
