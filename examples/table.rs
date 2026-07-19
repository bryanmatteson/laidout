//! Render a statically compiled aligned table through the consumer facade.

use laidout::{render, table, text, Alignment, Column, RenderOptions};

fn main() {
    let doc = table([
        Column::labeled(text("NAME")),
        Column::labeled(text("COUNT")).alignment(Alignment::Right),
        Column::labeled(text("STATE")).alignment(Alignment::Center),
    ])
    .header()
    .row([text("alpha"), text("7"), text("ready")])
    .row([text("beta"), text("123"), text("idle")])
    .build()
    .expect("example cells are flat");

    for width in [40, 8] {
        let rendered = render(&doc, &RenderOptions::new(width)).expect("exact render");
        println!("== width {width}\n{}\n", rendered.text);
    }
}
