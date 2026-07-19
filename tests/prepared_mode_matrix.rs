use std::num::NonZeroU32;
use std::panic::{catch_unwind, AssertUnwindSafe};

use laidout::{
    render, render_into, Doc, IndentPolicy, LayoutStrategy, RenderError, RenderOptions,
    RenderWorkspace, WorkspaceMode, WorkspaceResource,
};

fn options(width: u32, strategy: LayoutStrategy) -> RenderOptions {
    RenderOptions::new(NonZeroU32::new(width).expect("nonzero width")).with_strategy(strategy)
}

#[test]
fn fast_and_exact_share_the_prepared_document_and_structural_annotations() {
    let doc = Doc::group(Doc::concat([
        Doc::annotate("label", Doc::text("alpha")),
        Doc::line(),
        Doc::annotate("value", Doc::text("beta")),
    ]));
    let prepared = doc.prepare().expect("prepare");
    let mut workspace = RenderWorkspace::growable();

    let fast = render_into(&prepared, options(8, LayoutStrategy::Fast), &mut workspace)
        .expect("fast render");
    assert_eq!(fast.text(), "alpha\nbeta");
    assert_eq!(
        fast.resolved_spans()
            .map(|(_, value)| *value)
            .collect::<Vec<_>>(),
        ["label", "value"]
    );
    drop(fast);
    assert!(
        workspace.capacity().fast.work_items >= 4,
        "{:?}",
        workspace.capacity()
    );

    let exact = render_into(&prepared, options(8, LayoutStrategy::Exact), &mut workspace)
        .expect("exact render");
    assert_eq!(exact.text(), "alpha\nbeta");
    assert_eq!(exact.cost().overflow(), 0);
    drop(exact);
    assert!(
        workspace.capacity().fast.work_items >= 4,
        "{:?}",
        workspace.capacity()
    );

    assert!(workspace.capacity().fast.work_items > 0);
    assert!(workspace.capacity().exact.memo_entries > 0);
    workspace.set_mode(WorkspaceMode::Fixed);
    let fixed = render_into(&prepared, options(8, LayoutStrategy::Fast), &mut workspace)
        .expect("warmed fixed render");
    assert_eq!(fixed.text(), "alpha\nbeta");
}

#[test]
fn owned_render_keeps_annotation_values_without_clone_bounds() {
    #[derive(Debug, Eq, Hash, PartialEq)]
    struct NotClone(&'static str);

    let doc = Doc::annotate(
        NotClone("outer"),
        Doc::annotate(NotClone("inner"), Doc::text("x")),
    );
    let rendered = render(&doc, options(80, LayoutStrategy::Exact)).expect("owned render");
    assert_eq!(rendered.text(), "x");
    assert_eq!(rendered.spans().len(), 2);
    assert_eq!(
        rendered
            .resolved_spans()
            .map(|(_, value)| value.0)
            .collect::<Vec<_>>(),
        ["outer", "inner"]
    );
}

#[test]
fn clamp_policy_is_applied_when_indent_is_entered() {
    let doc = Doc::<u32>::nest(
        100,
        Doc::concat([Doc::text("a"), Doc::hard_line(), Doc::text("b")]),
    );
    let rendered = render(
        &doc,
        options(4, LayoutStrategy::Fast).with_indent_policy(IndentPolicy::ClampToWidthMinusOne),
    )
    .expect("render");
    assert_eq!(rendered.text(), "a\n   b");
}

#[test]
fn panicking_visitor_rolls_back_and_leaves_the_workspace_reusable() {
    struct Panics;
    impl laidout::LayoutVisitor<u32> for Panics {
        type Error = std::convert::Infallible;
        fn text(
            &mut self,
            _text: &str,
            _annotations: laidout::ActiveAnnotations<'_, u32>,
        ) -> Result<(), Self::Error> {
            panic!("visitor panic probe")
        }
        fn newline(
            &mut self,
            _indent: u32,
            _annotations: laidout::ActiveAnnotations<'_, u32>,
        ) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    let doc = Doc::<u32>::text("x");
    let prepared = doc.prepare().unwrap();
    let mut workspace = RenderWorkspace::growable();
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let layout = laidout::solve_into(
            &prepared,
            options(80, LayoutStrategy::Exact),
            &mut workspace,
        )
        .unwrap();
        let _ = layout.visit(&mut Panics);
    }));
    assert!(panic.is_err());

    let rendered = render_into(
        &prepared,
        options(80, LayoutStrategy::Exact),
        &mut workspace,
    )
    .unwrap();
    assert_eq!(rendered.text(), "x");
}

fn capacity_document() -> Doc<u32> {
    Doc::group(Doc::concat([
        Doc::annotate(1, Doc::text("alpha")),
        Doc::line(),
        Doc::nest(
            2,
            Doc::annotate(
                2,
                Doc::choice(
                    Doc::penalize(4, Doc::text("beta gamma")),
                    Doc::text("delta"),
                ),
            ),
        ),
    ]))
}

fn assert_one_less(
    strategy: LayoutStrategy,
    resource: WorkspaceResource,
    reduce: impl FnOnce(&mut laidout::RenderCapacity),
) {
    let prepared = capacity_document().prepare().unwrap();
    let render_options = options(8, strategy);
    let mut warm = RenderWorkspace::growable();
    drop(render_into(&prepared, render_options, &mut warm).unwrap());
    let mut capacity = warm.capacity();
    reduce(&mut capacity);
    let mut fixed = RenderWorkspace::fixed(capacity).unwrap();
    let error = match render_into(&prepared, render_options, &mut fixed) {
        Ok(_) => panic!("one-less {resource:?} capacity unexpectedly succeeded"),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        RenderError::WorkspaceExhausted {
            resource: actual,
            ..
        } if actual == resource
    ));
}

#[test]
fn every_fixed_resource_reports_its_one_less_capacity_directly() {
    assert_one_less(
        LayoutStrategy::Fast,
        WorkspaceResource::FastWorkItems,
        |capacity| capacity.fast.work_items -= 1,
    );
    for (resource, reduce) in [
        (
            WorkspaceResource::ExactWorkItems,
            (|capacity: &mut laidout::RenderCapacity| capacity.exact.work_items -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::MemoEntries,
            (|capacity: &mut laidout::RenderCapacity| capacity.exact.memo_entries -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::RetainedCandidates,
            (|capacity: &mut laidout::RenderCapacity| capacity.exact.retained_candidates -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::FrontierScratch,
            (|capacity: &mut laidout::RenderCapacity| capacity.exact.frontier_scratch -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::PlanNodes,
            (|capacity: &mut laidout::RenderCapacity| capacity.plan_nodes -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::VisitWorkItems,
            (|capacity: &mut laidout::RenderCapacity| capacity.visit.work_items -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::AnnotationDepth,
            (|capacity: &mut laidout::RenderCapacity| capacity.visit.annotation_depth -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::OutputBytes,
            (|capacity: &mut laidout::RenderCapacity| capacity.materialize.output_bytes -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
        (
            WorkspaceResource::Spans,
            (|capacity: &mut laidout::RenderCapacity| capacity.materialize.spans -= 1)
                as fn(&mut laidout::RenderCapacity),
        ),
    ] {
        assert_one_less(LayoutStrategy::Exact, resource, reduce);
    }
}

#[test]
fn release_preserves_mode_and_growable_reuse_regrows_named_storage() {
    let prepared = capacity_document().prepare().unwrap();
    let render_options = options(8, LayoutStrategy::Exact);
    let mut workspace = RenderWorkspace::growable();
    drop(render_into(&prepared, render_options, &mut workspace).unwrap());
    assert_ne!(workspace.capacity(), Default::default());
    workspace.set_mode(WorkspaceMode::Fixed);
    workspace.release_capacity();
    assert_eq!(workspace.capacity(), Default::default());
    assert!(matches!(
        render_into(&prepared, render_options, &mut workspace),
        Err(RenderError::WorkspaceExhausted { .. })
    ));

    workspace.set_mode(WorkspaceMode::Growable);
    let rendered = render_into(&prepared, render_options, &mut workspace).unwrap();
    assert!(!rendered.text().is_empty());
    drop(rendered);
    assert_ne!(workspace.capacity(), Default::default());
}

#[test]
fn empty_text_produces_no_text_callback() {
    #[derive(Default)]
    struct Counter(usize);
    impl laidout::LayoutVisitor<u32> for Counter {
        type Error = std::convert::Infallible;
        fn text(
            &mut self,
            _text: &str,
            _annotations: laidout::ActiveAnnotations<'_, u32>,
        ) -> Result<(), Self::Error> {
            self.0 += 1;
            Ok(())
        }
        fn newline(
            &mut self,
            _indent: u32,
            _annotations: laidout::ActiveAnnotations<'_, u32>,
        ) -> Result<(), Self::Error> {
            Ok(())
        }
    }

    let prepared = Doc::<u32>::text("").prepare().unwrap();
    let mut workspace = RenderWorkspace::growable();
    let layout =
        laidout::solve_into(&prepared, options(80, LayoutStrategy::Fast), &mut workspace).unwrap();
    let mut counter = Counter::default();
    layout.visit(&mut counter).unwrap();
    assert_eq!(counter.0, 0);
}
