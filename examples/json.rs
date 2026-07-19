use std::num::NonZeroU32;

use laidout::{render, LayoutStrategy, RenderOptions};

#[path = "../support/json.rs"]
#[allow(dead_code)]
mod corpus;
use corpus::{complex_value, format};

fn main() {
    let doc = format(&complex_value());
    for width in [20, 40, 80] {
        let rendered = render(
            &doc,
            RenderOptions::new(NonZeroU32::new(width).unwrap())
                .with_strategy(LayoutStrategy::Exact),
        )
        .expect("render JSON");
        println!("== width {width}\n{}\n", rendered.text());
    }
}
