use std::num::NonZeroU32;

use laidout::{render, table, Alignment, Column, Doc, RenderOptions};

fn main() {
    let doc = table([
        Column::labeled(Doc::text("NAME")),
        Column::labeled(Doc::text("COUNT")).alignment(Alignment::Right),
        Column::labeled(Doc::text("STATE")).alignment(Alignment::Center),
    ])
    .header()
    .row([Doc::text("alpha"), Doc::text("7"), Doc::text("ready")])
    .row([Doc::text("beta"), Doc::text("123"), Doc::text("idle")])
    .build()
    .expect("compile table");

    for width in [80, 12] {
        let rendered = render(&doc, RenderOptions::new(NonZeroU32::new(width).unwrap()))
            .expect("render table");
        println!("== width {width}\n{}\n", rendered.text());
    }
}
