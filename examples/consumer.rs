use std::num::NonZeroU32;

use laidout::{render, Doc, LayoutStrategy, RenderOptions};

#[derive(Debug, Eq, Hash, PartialEq)]
enum Style {
    Emphasis,
    Accent,
}

fn main() {
    let doc = Doc::annotate(
        Style::Emphasis,
        Doc::concat([
            Doc::text("status "),
            Doc::annotate(Style::Accent, Doc::text("界")),
            Doc::text(": ready"),
        ]),
    );
    let rendered = render(
        &doc,
        RenderOptions::new(NonZeroU32::new(80).unwrap()).with_strategy(LayoutStrategy::Exact),
    )
    .expect("render consumer document");
    assert_eq!(rendered.text(), "status 界: ready");
    assert_eq!(rendered.spans().len(), 2);
    println!("{}", rendered.text());
}
