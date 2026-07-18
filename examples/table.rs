//! Render a statically compiled aligned table in compact and fallback forms.

use pretty::cost::OverflowThenHeight;
use pretty::{frontier, table, text, to_string, Alignment, Column};

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
        let best = frontier::best(&OverflowThenHeight { width }, &doc);
        println!("== width {width}\n{}\n", to_string(&best.out));
    }
}
