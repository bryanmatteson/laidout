use std::num::NonZeroU32;

use laidout::corpus::ast::{asymmetric_file, sample_file, Formatter};
use laidout::{render, LayoutStrategy, RenderOptions};

fn main() {
    for (name, file) in [("sample", sample_file()), ("asymmetric", asymmetric_file())] {
        let doc = Formatter::default().format_file(&file);
        for width in [18, 40, 100] {
            let rendered = render(
                &doc,
                RenderOptions::new(NonZeroU32::new(width).unwrap())
                    .with_strategy(LayoutStrategy::Exact),
            )
            .expect("render AST");
            println!("== {name} width {width}\n{}\n", rendered.text());
        }
    }
}
