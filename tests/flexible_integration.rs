use std::fmt;
use std::num::NonZeroU32;

use laidout::{
    from_text_with_annotations, render, Column, Doc, IngestOptions, LayoutStrategy, RenderOptions,
    Renderer, Table, TextKind,
};

fn options(width: u32) -> RenderOptions {
    RenderOptions::new(NonZeroU32::new(width).expect("test width is nonzero"))
        .with_strategy(LayoutStrategy::Exact)
}

struct DynamicMark(Box<dyn fmt::Display>);

#[test]
fn annotations_need_no_standard_comparison_or_formatting_traits() {
    let doc = Doc::annotate(
        DynamicMark(Box::new("dynamic")),
        Doc::group(Doc::concat([
            Doc::text("hello"),
            Doc::line(),
            Doc::text("world"),
        ])),
    );
    let prepared = doc.prepare().expect("prepare dynamic annotation");
    let rendered = render(&doc, options(20)).expect("render dynamic annotation");

    assert_eq!(prepared.annotation_count(), 1);
    assert_eq!(rendered.text(), "hello world");
    let resolved = rendered.resolved_spans().collect::<Vec<_>>();
    assert_eq!(resolved.len(), 1);
    assert_eq!(resolved[0].1 .0.to_string(), "dynamic");
}

#[test]
fn annotation_mapping_preserves_shared_nodes_and_maps_once() {
    let annotation = std::sync::Arc::new(7_u32);
    let source = Doc::concat([
        Doc::annotate_shared(annotation.clone(), Doc::text("x")),
        Doc::annotate_shared(annotation, Doc::text("x")),
    ]);
    let mut calls = 0;
    let mapped = source.map_annotations(|value| {
        calls += 1;
        format!("tag-{value}")
    });
    assert_eq!(
        mapped
            .prepare()
            .expect("prepare mapped document")
            .annotation_count(),
        1
    );
    let rendered = render(&mapped, options(80)).expect("render mapped document");

    assert_eq!(calls, 1);
    assert_eq!(rendered.text(), "xx");
    assert_eq!(rendered.resolved_spans().count(), 2);
    assert!(rendered
        .resolved_spans()
        .all(|(_, annotation)| annotation == "tag-7"));
}

#[derive(Debug)]
enum Mark {
    Word,
    Symbol,
    Space,
    Indent,
    Break,
    Header,
    Cell,
}

fn mark(kind: TextKind) -> Mark {
    match kind {
        TextKind::Word => Mark::Word,
        TextKind::Symbol => Mark::Symbol,
        TextKind::Whitespace => Mark::Space,
        TextKind::Indent => Mark::Indent,
        TextKind::Newline => Mark::Break,
    }
}

#[test]
fn ingestion_and_tables_share_an_application_annotation_type() {
    let ingested = from_text_with_annotations("hello, world", IngestOptions::default(), mark)
        .expect("ingest text");
    let ingested_rendered = render(&ingested, options(80)).expect("render ingested text");
    assert_eq!(ingested_rendered.text(), "hello, world");

    let table = Table::new([Column::with_header(Doc::annotate(
        Mark::Header,
        Doc::text("message"),
    ))])
    .header()
    .row([Doc::annotate(Mark::Cell, Doc::text("hello, world"))]);
    let doc = table.build().expect("build generic table");
    let rendered = render(&doc, options(80)).expect("render generic table");

    assert_eq!(rendered.text(), "message\nhello, world");
    assert_eq!(rendered.resolved_spans().count(), 2);
}

#[test]
fn explicit_columns_override_unicode_measurement() {
    let measured = Doc::<u32>::group(Doc::concat([Doc::text("界"), Doc::line(), Doc::text("x")]));
    let host_measured = Doc::<u32>::group(Doc::concat([
        Doc::text_with_columns("界", 1),
        Doc::line(),
        Doc::text("x"),
    ]));

    assert_eq!(
        render(&measured, options(3))
            .expect("render Unicode-measured text")
            .text(),
        "界\nx"
    );
    assert_eq!(
        render(&host_measured, options(3))
            .expect("render host-measured text")
            .text(),
        "界 x"
    );
}

#[test]
fn renderer_reuses_workspace_and_supports_owned_and_sink_output() {
    let prepared = Doc::<u32>::group(Doc::concat([
        Doc::text("alpha"),
        Doc::line(),
        Doc::text("beta"),
    ]))
    .prepare()
    .expect("prepare document");
    let mut renderer = Renderer::new();

    let owned = renderer
        .render_prepared(&prepared, options(8))
        .expect("render owned output");
    assert_eq!(owned.text(), "alpha\nbeta");
    let capacity = renderer.workspace().capacity();

    let mut formatted = String::new();
    renderer
        .write_fmt(&prepared, options(20), &mut formatted)
        .expect("write fmt output");
    assert_eq!(formatted, "alpha beta");

    let mut bytes = Vec::new();
    renderer
        .write_io(&prepared, options(8), &mut bytes)
        .expect("write io output");
    assert_eq!(bytes, b"alpha\nbeta");
    assert!(renderer.workspace().capacity().fast.work_items >= capacity.fast.work_items);

    let parts = owned.into_parts();
    assert_eq!(parts.text, "alpha\nbeta");
}
