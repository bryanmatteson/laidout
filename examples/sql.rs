use std::num::NonZeroU32;

use laidout::corpus::sql::{complex_query, Formatter};
use laidout::{render, LayoutStrategy, RenderOptions};

fn main() {
    let doc = Formatter::default().format_query(&complex_query());
    for width in [40, 80, 120] {
        let rendered = render(
            &doc,
            RenderOptions::new(NonZeroU32::new(width).unwrap())
                .with_strategy(LayoutStrategy::Exact),
        )
        .expect("render SQL");
        println!("== width {width}\n{}\n", rendered.text());
    }
}
