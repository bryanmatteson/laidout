use std::num::NonZeroU32;

use laidout::{render, table, Alignment, Column, Doc, LayoutStrategy, RenderOptions, TableError};

fn options(width: u32) -> RenderOptions {
    RenderOptions::new(NonZeroU32::new(width).unwrap()).with_strategy(LayoutStrategy::Exact)
}

fn aligned_table() -> Doc {
    table([
        Column::labeled(Doc::text("NAME")),
        Column::labeled(Doc::text("COUNT")).alignment(Alignment::Right),
        Column::labeled(Doc::text("STATE")).alignment(Alignment::Center),
    ])
    .header()
    .row([Doc::text("alpha"), Doc::text("7"), Doc::text("ready")])
    .row([Doc::text("beta"), Doc::text("123"), Doc::text("idle")])
    .build()
    .unwrap()
}

#[test]
fn table_compact_and_fallback_layouts_preserve_cell_order() {
    let doc = aligned_table();
    let wide = render(&doc, options(80)).unwrap();
    assert_eq!(
        wide.text(),
        "NAME   COUNT  STATE\nalpha      7  ready\nbeta     123  idle"
    );
    let narrow = render(&doc, options(5)).unwrap();
    assert_eq!(
        narrow.text().split_whitespace().collect::<Vec<_>>(),
        ["NAME", "COUNT", "STATE", "alpha", "7", "ready", "beta", "123", "idle"]
    );
}

#[test]
fn table_rejects_choice_bearing_and_nonflattenable_cells_directly() {
    let choice = table([Column::new()])
        .row([Doc::choice(Doc::text("a"), Doc::text("b"))])
        .build();
    assert_eq!(
        choice,
        Err(TableError::ChoiceBearingCell { row: 0, column: 0 })
    );

    let hard_line = table([Column::new()])
        .row([Doc::concat([
            Doc::text("a"),
            Doc::hard_line(),
            Doc::text("b"),
        ])])
        .build();
    assert_eq!(
        hard_line,
        Err(TableError::NonFlattenableCell { row: 0, column: 0 })
    );
}

#[test]
fn table_measurement_uses_stored_unicode_columns() {
    let doc = table([Column::new(), Column::new()])
        .row([Doc::text("界"), Doc::text("x")])
        .row([Doc::text("a"), Doc::text("y")])
        .build()
        .unwrap();
    let rendered = render(&doc, options(80)).unwrap();
    assert_eq!(rendered.text(), "界  x\na   y");
}
