use std::num::NonZeroU32;

use laidout::corpus::{complex_value, format};
use laidout::{render, LayoutStrategy, RenderOptions};

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
