use std::num::NonZeroU32;

use laidout::{render, LayoutStrategy, RenderOptions};

#[path = "../support/sql.rs"]
#[allow(dead_code)]
mod corpus;
use corpus::{complex_query, Formatter};

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
